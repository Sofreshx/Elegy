use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use elegy_memory::{EmbeddingError, EmbeddingProvider};
use elegy_memory_mcp::{
    memory_tools::{MemoryBinding, DEFAULT_NAMESPACE},
    server::{ElegyMemoryMcpServer, NoopWriteAuditor},
};
use rmcp::{
    model::CallToolRequestParams, service::RunningService, ClientHandler, RoleClient, ServiceExt,
};
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::{task::JoinHandle, time::Instant};

#[derive(Default, Clone)]
struct TestClient;

impl ClientHandler for TestClient {}

struct TestSession {
    _temp_dir: TempDir,
    client: RunningService<RoleClient, TestClient>,
    server_task: JoinHandle<()>,
}

impl TestSession {
    async fn new(provider: Arc<dyn EmbeddingProvider>) -> Self {
        let temp_dir = TempDir::new().expect("tempdir should create");
        let db_path = temp_dir.path().join("memory.db");
        let binding =
            MemoryBinding::new(DEFAULT_NAMESPACE, "bench-agent").expect("binding should build");
        let repository = Arc::new(
            elegy_memory_mcp::memory_tools::MemoryRepository::new_with_embedding_provider(
                &db_path, binding, provider,
            )
            .expect("repository should build"),
        );
        let server = ElegyMemoryMcpServer::new(repository, Arc::new(NoopWriteAuditor));
        let (server_transport, client_transport) = tokio::io::duplex(4096);
        let server_task = tokio::spawn(async move {
            let service = server
                .serve(server_transport)
                .await
                .expect("server should initialize");
            service.waiting().await.expect("server should run cleanly");
        });
        let client = TestClient
            .serve(client_transport)
            .await
            .expect("client should initialize");
        Self {
            _temp_dir: temp_dir,
            client,
            server_task,
        }
    }

    async fn call_tool(&self, tool_name: &'static str, arguments: Value) -> Value {
        let result = self
            .client
            .call_tool(
                CallToolRequestParams::new(tool_name).with_arguments(
                    arguments
                        .as_object()
                        .cloned()
                        .expect("tool arguments should be a JSON object"),
                ),
            )
            .await
            .expect("tool call should succeed");
        result
            .structured_content
            .expect("tool result should include structured content")
    }

    async fn shutdown(self) {
        self.client
            .cancel()
            .await
            .expect("client should cancel cleanly");
        self.server_task.await.expect("server task should join");
    }
}

#[derive(Debug)]
struct DeterministicEmbeddingProvider;

impl DeterministicEmbeddingProvider {
    fn hash_to_seed(text: &str) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in text.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    fn deterministic_embedding(text: &str) -> Vec<f32> {
        let seed = Self::hash_to_seed(text);
        let mut embedding = Vec::with_capacity(768);
        let mut state = seed;
        for _ in 0..768 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let normalized = ((state >> 33) as f32) / (1u64 << 31) as f32;
            embedding.push(normalized * 2.0 - 1.0);
        }
        embedding
    }
}

#[async_trait]
impl EmbeddingProvider for DeterministicEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(Self::deterministic_embedding(text))
    }

    fn dimensions(&self) -> usize {
        768
    }

    fn model_id(&self) -> &str {
        "bench-deterministic-stub"
    }
}

fn percentile(sorted_durations: &[Duration], p: f64) -> Duration {
    if sorted_durations.is_empty() {
        return Duration::ZERO;
    }
    let index =
        ((sorted_durations.len() as f64) * p).min((sorted_durations.len() - 1) as f64) as usize;
    sorted_durations[index]
}

#[tokio::test]
async fn large_corpus_benchmark() -> anyhow::Result<()> {
    let scale = match std::env::var("ELEGY_MEMORY_BENCH_SCALE") {
        Ok(value) => match value.trim().parse::<usize>() {
            Ok(n) if n > 0 => n,
            _ => {
                println!(
                    "skipping large-corpus benchmark: \
                     set ELEGY_MEMORY_BENCH_SCALE=10000 or =100000 to run it"
                );
                return Ok(());
            }
        },
        Err(_) => {
            println!(
                "skipping large-corpus benchmark: \
                 set ELEGY_MEMORY_BENCH_SCALE=10000 or =100000 to run it"
            );
            return Ok(());
        }
    };

    let provider: Arc<dyn EmbeddingProvider> = Arc::new(DeterministicEmbeddingProvider);
    let session = TestSession::new(provider).await;

    // Seed phase
    let seed_start = Instant::now();
    for i in 0..scale {
        let topic = i % 50;
        let content = format!(
            "memory record {i} concerning topic {topic}: this is a realistic-length content \
             string for benchmarking purposes, providing enough text to exercise the storage and \
             retrieval pipeline at scale with varied and distinguishable records"
        );
        let _ = session
            .call_tool(
                "memory_store",
                json!({
                    "content": content,
                    "memoryType": "observation",
                    "importance": 0.5
                }),
            )
            .await;
        if (i + 1) % (scale / 10).max(1) == 0 || i + 1 == scale {
            println!("seeded {}/{} memories", i + 1, scale);
        }
    }
    let seed_duration = seed_start.elapsed();

    // Collect stored ids for recall benchmarks
    let list_result = session
        .call_tool("memory_list", json!({"limit": 100}))
        .await;
    let stored_ids: Vec<String> = list_result["memories"]
        .as_array()
        .expect("memories should be an array")
        .iter()
        .filter_map(|m| m["id"].as_str().map(String::from))
        .collect();
    assert!(!stored_ids.is_empty(), "should have stored memories");

    // Measurement phase — 100 iterations each
    let iterations = 100;
    let mut search_durations = Vec::with_capacity(iterations);
    let mut list_durations = Vec::with_capacity(iterations);
    let mut stats_durations = Vec::with_capacity(iterations);
    let mut recall_durations = Vec::with_capacity(iterations);

    for i in 0..iterations {
        // memory_search
        let query = format!("topic {}", i % 50);
        let start = Instant::now();
        let _ = session
            .call_tool("memory_search", json!({"query": query, "limit": 10}))
            .await;
        search_durations.push(start.elapsed());

        // memory_list
        let start = Instant::now();
        let _ = session.call_tool("memory_list", json!({"limit": 20})).await;
        list_durations.push(start.elapsed());

        // memory_stats
        let start = Instant::now();
        let _ = session.call_tool("memory_stats", json!({})).await;
        stats_durations.push(start.elapsed());

        // memory_recall
        let id = &stored_ids[i % stored_ids.len()];
        let start = Instant::now();
        let _ = session.call_tool("memory_recall", json!({"id": id})).await;
        recall_durations.push(start.elapsed());
    }

    // Report
    search_durations.sort();
    list_durations.sort();
    stats_durations.sort();
    recall_durations.sort();

    let search_p50 = percentile(&search_durations, 0.50);
    let search_p95 = percentile(&search_durations, 0.95);
    let search_p99 = percentile(&search_durations, 0.99);

    let list_p50 = percentile(&list_durations, 0.50);
    let list_p95 = percentile(&list_durations, 0.95);
    let list_p99 = percentile(&list_durations, 0.99);

    let stats_p50 = percentile(&stats_durations, 0.50);
    let stats_p95 = percentile(&stats_durations, 0.95);
    let stats_p99 = percentile(&stats_durations, 0.99);

    let recall_p50 = percentile(&recall_durations, 0.50);
    let recall_p95 = percentile(&recall_durations, 0.95);
    let recall_p99 = percentile(&recall_durations, 0.99);

    println!("=== LARGE CORPUS BENCHMARK RESULTS ===");
    println!(
        "{}",
        serde_json::json!({
            "scale": scale,
            "seed_duration_ms": seed_duration.as_millis(),
            "operations": {
                "memory_search": {
                    "p50_ms": search_p50.as_secs_f64() * 1000.0,
                    "p95_ms": search_p95.as_secs_f64() * 1000.0,
                    "p99_ms": search_p99.as_secs_f64() * 1000.0,
                },
                "memory_list": {
                    "p50_ms": list_p50.as_secs_f64() * 1000.0,
                    "p95_ms": list_p95.as_secs_f64() * 1000.0,
                    "p99_ms": list_p99.as_secs_f64() * 1000.0,
                },
                "memory_stats": {
                    "p50_ms": stats_p50.as_secs_f64() * 1000.0,
                    "p95_ms": stats_p95.as_secs_f64() * 1000.0,
                    "p99_ms": stats_p99.as_secs_f64() * 1000.0,
                },
                "memory_recall": {
                    "p50_ms": recall_p50.as_secs_f64() * 1000.0,
                    "p95_ms": recall_p95.as_secs_f64() * 1000.0,
                    "p99_ms": recall_p99.as_secs_f64() * 1000.0,
                }
            }
        })
    );
    println!("=== END LARGE CORPUS BENCHMARK RESULTS ===");

    session.shutdown().await;
    Ok(())
}
