use std::{process::Stdio, time::Duration};

use reqwest::Client;
use rmcp::{
    model::CallToolRequestParams,
    service::RunningService,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ClientHandler, RoleClient, ServiceExt,
};
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::{io::AsyncReadExt, process::Command, task::JoinHandle, time::Instant};

const TOOL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Default, Clone)]
struct TestClient;

impl ClientHandler for TestClient {}

struct StdioChildSession {
    _temp_dir: TempDir,
    client: RunningService<RoleClient, TestClient>,
    stderr_task: JoinHandle<std::io::Result<String>>,
}

impl StdioChildSession {
    async fn spawn() -> Self {
        let temp_dir = TempDir::new().expect("tempdir should create");
        let db_path = temp_dir.path().join("memory.db");
        std::fs::write(&db_path, b"").expect("db placeholder should write");

        let mut command = Command::new(env!("CARGO_BIN_EXE_elegy-memory-mcp-stdio"));
        command
            .env("ELEGY_DB_PATH", &db_path)
            .env("ELEGY_MCP_AGENT_ID", "wu13-repro-agent")
            .env(
                "RUST_LOG",
                "trace,elegy_memory=trace,elegy_memory_mcp=trace,rmcp=debug",
            );

        let (transport, stderr) = TokioChildProcess::builder(command.configure(|_| {}))
            .stderr(Stdio::piped())
            .spawn()
            .expect("stdio child should spawn");
        let mut stderr = stderr.expect("stderr must be piped");
        let stderr_task = tokio::spawn(async move {
            let mut output = String::new();
            stderr.read_to_string(&mut output).await?;
            Ok(output)
        });
        let client = TestClient
            .serve(transport)
            .await
            .expect("stdio client should initialize");

        Self {
            _temp_dir: temp_dir,
            client,
            stderr_task,
        }
    }

    async fn call_tool_timed(
        &self,
        tool_name: &'static str,
        arguments: Value,
    ) -> anyhow::Result<(Value, Duration)> {
        let started = Instant::now();
        let result = tokio::time::timeout(
            TOOL_TIMEOUT,
            self.client.call_tool(
                CallToolRequestParams::new(tool_name).with_arguments(
                    arguments
                        .as_object()
                        .cloned()
                        .expect("tool arguments should be an object"),
                ),
            ),
        )
        .await
        .map_err(|_| anyhow::anyhow!("tool `{tool_name}` timed out after {TOOL_TIMEOUT:?}"))?
        .map_err(|error| anyhow::anyhow!("tool `{tool_name}` failed: {error}"))?;
        let elapsed = started.elapsed();
        let content = result
            .structured_content
            .ok_or_else(|| anyhow::anyhow!("tool `{tool_name}` returned no structured content"))?;
        Ok((content, elapsed))
    }

    async fn shutdown(self) -> String {
        self.client
            .cancel()
            .await
            .expect("client should cancel cleanly");
        self.stderr_task
            .await
            .expect("stderr task should join")
            .expect("stderr should read")
    }
}

async fn ensure_ollama_is_up() -> anyhow::Result<bool> {
    let response = Client::new()
        .post("http://localhost:11434/api/embeddings")
        .json(&json!({
            "model": "nomic-embed-text",
            "prompt": "test"
        }))
        .send()
        .await;
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            eprintln!("skipping real-Ollama regression: Ollama is unavailable ({error})");
            return Ok(false);
        }
    };
    anyhow::ensure!(
        response.status().is_success(),
        "ollama embeddings endpoint returned {}",
        response.status()
    );
    let payload: Value = response.json().await?;
    let len = payload["embedding"].as_array().map_or(0, Vec::len);
    anyhow::ensure!(len == 768, "expected 768 embedding dimensions, got {len}");
    Ok(true)
}

#[tokio::test]
async fn reproduce_under_load_and_measure_semantic_margins() -> anyhow::Result<()> {
    if !ensure_ollama_is_up().await? {
        return Ok(());
    }
    let session = StdioChildSession::spawn().await;

    let memories = [
        ("coffee", "Arabica espresso crema from a barista station."),
        (
            "rust",
            "Borrow checker blocks aliased mutable references and lifetime leaks.",
        ),
        ("climbing", "Quickdraw belay sequence on a granite route."),
        ("soup", "Miso dashi bowl with tofu and seaweed."),
    ];

    let mut ids = Vec::new();
    println!("--- STORES ---");
    for (label, content) in memories {
        let (stored, elapsed) = session
            .call_tool_timed(
                "memory_store",
                json!({
                    "content": content,
                    "memoryType": "fact",
                    "importance": 0.8
                }),
            )
            .await?;
        let id = stored["memory"]["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing stored id"))?
            .to_string();
        println!(
            "{label}: {} ms, status={}, id={}",
            elapsed.as_millis(),
            stored["embeddingStatus"],
            id
        );
        ids.push(id);
    }

    let searches = [
        ("coffee", "caffeine latte mug"),
        ("rust", "compiler alias safety"),
        ("climbing", "rope harness crag"),
        ("soup", "umami stock supper"),
    ];

    println!("--- SEARCHES ---");
    for (expected, query) in searches {
        let (search, elapsed) = session
            .call_tool_timed(
                "memory_search",
                json!({
                    "query": query,
                    "limit": 4
                }),
            )
            .await?;
        let results = search["results"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("search results missing"))?;
        let top1 = results
            .first()
            .ok_or_else(|| anyhow::anyhow!("empty results"))?;
        let top2 = results
            .get(1)
            .cloned()
            .unwrap_or_else(|| json!({"similarity": 0.0}));
        let top1_id = top1["id"].as_str().unwrap_or("<missing>");
        let top1_similarity = top1["similarity"].as_f64().unwrap_or_default();
        let top2_similarity = top2["similarity"].as_f64().unwrap_or_default();
        let margin = top1_similarity - top2_similarity;
        println!(
            "query={query:?} expected={expected} elapsed={}ms top1={} sim1={:.6} sim2={:.6} margin={:.6}",
            elapsed.as_millis(),
            top1_id,
            top1_similarity,
            top2_similarity,
            margin
        );
        assert_eq!(
            top1["preview"].as_str().unwrap_or_default(),
            memories
                .iter()
                .find(|(label, _)| label == &expected)
                .map(|(_, content)| *content)
                .unwrap_or_default(),
            "semantic search should rank the expected concept first for {query:?}"
        );
        assert!(
            margin > 0.05,
            "semantic search margin for {query:?} was too small: {margin:.6}"
        );
    }

    println!("--- POST-LOAD CHECKS ---");
    let (stats, stats_elapsed) = session.call_tool_timed("memory_stats", json!({})).await?;
    println!(
        "memory_stats: {} ms, totalCount={}, staleEmbeddingsCount={}",
        stats_elapsed.as_millis(),
        stats["totalCount"],
        stats["staleEmbeddingsCount"]
    );

    for id in &ids {
        let (_deleted, elapsed) = session
            .call_tool_timed("memory_delete", json!({ "id": id }))
            .await?;
        println!("memory_delete {}: {} ms", id, elapsed.as_millis());
    }

    let stderr = session.shutdown().await;
    println!("--- STDERR ---\n{stderr}");
    Ok(())
}

#[tokio::test]
async fn stress_server_reactivity_after_45_embedding_calls() -> anyhow::Result<()> {
    if !ensure_ollama_is_up().await? {
        return Ok(());
    }
    let session = StdioChildSession::spawn().await;

    let mut ids = Vec::new();
    for round in 0..5 {
        for (topic, content) in [
            (
                "coffee",
                format!("Round {round} arabica espresso with cocoa finish."),
            ),
            (
                "rust",
                format!("Round {round} rust borrow checker and mutable aliasing."),
            ),
            (
                "climbing",
                format!("Round {round} granite ocean cliff climb at sunrise."),
            ),
            (
                "soup",
                format!("Round {round} tofu miso broth with sesame."),
            ),
        ] {
            let (stored, elapsed) = session
                .call_tool_timed(
                    "memory_store",
                    json!({
                        "content": content,
                        "memoryType": "fact",
                        "importance": 0.8
                    }),
                )
                .await?;
            println!(
                "round={round} store={topic} elapsed={}ms status={}",
                elapsed.as_millis(),
                stored["embeddingStatus"]
            );
            assert_eq!(
                stored["embeddingStatus"],
                json!("ready"),
                "embedding should stay ready during sustained load"
            );
            let id = stored["memory"]["id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing stored id"))?
                .to_string();
            ids.push(id);
        }

        for query in [
            "fragrant breakfast drink",
            "ownership lifetime safety",
            "outdoor rope route",
            "savory japanese broth",
            "morning roasted beverage",
        ] {
            let (_search, elapsed) = session
                .call_tool_timed(
                    "memory_search",
                    json!({
                        "query": query,
                        "limit": 4
                    }),
                )
                .await?;
            println!(
                "round={round} search={query:?} elapsed={}ms",
                elapsed.as_millis()
            );
        }
    }

    let (stats, elapsed) = session.call_tool_timed("memory_stats", json!({})).await?;
    println!(
        "post-stress memory_stats: {} ms totalCount={}",
        elapsed.as_millis(),
        stats["totalCount"]
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "memory_stats should stay responsive after sustained load"
    );

    let first_id = ids
        .first()
        .ok_or_else(|| anyhow::anyhow!("no stored ids recorded"))?;
    let (_deleted, delete_elapsed) = session
        .call_tool_timed("memory_delete", json!({ "id": first_id }))
        .await?;
    println!(
        "post-stress memory_delete {}: {} ms",
        first_id,
        delete_elapsed.as_millis()
    );
    assert!(
        delete_elapsed < Duration::from_secs(5),
        "memory_delete should stay responsive after sustained load"
    );

    let stderr = session.shutdown().await;
    println!("--- STRESS STDERR ---\n{stderr}");
    Ok(())
}
