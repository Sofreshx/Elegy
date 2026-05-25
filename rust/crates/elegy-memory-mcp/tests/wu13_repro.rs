use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
    process::Stdio,
    time::Duration,
};

use anyhow::{anyhow, ensure, Context};
use reqwest::Client;
use rmcp::{
    model::CallToolRequestParams,
    service::RunningService,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ClientHandler, RoleClient, ServiceExt,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::{io::AsyncReadExt, process::Command, task::JoinHandle, time::Instant};

const TOOL_TIMEOUT: Duration = Duration::from_secs(30);
const FIXTURE_RELATIVE_PATH: &str = "tests/fixtures/retrieval_benchmark.v1.json";
const ENGLISH_SUITE_KEY: &str = "en-expanded-v1";
const FRENCH_SUITE_KEY: &str = "fr-short-v1.1";

#[derive(Default, Clone)]
struct TestClient;

impl ClientHandler for TestClient {}

struct StdioChildSession {
    _temp_dir: TempDir,
    client: RunningService<RoleClient, TestClient>,
    stderr_task: JoinHandle<std::io::Result<String>>,
}

impl StdioChildSession {
    async fn spawn(agent_id: &str) -> Self {
        let temp_dir = TempDir::new().expect("tempdir should create");
        let db_path = temp_dir.path().join("memory.db");
        std::fs::write(&db_path, b"").expect("db placeholder should write");

        let mut command = Command::new(env!("CARGO_BIN_EXE_elegy-memory-mcp-stdio"));
        command
            .env("ELEGY_DB_PATH", &db_path)
            .env("ELEGY_MCP_AGENT_ID", agent_id)
            .env("RUST_LOG", "info");

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
        .map_err(|_| anyhow!("tool `{tool_name}` timed out after {TOOL_TIMEOUT:?}"))?
        .map_err(|error| anyhow!("tool `{tool_name}` failed: {error}"))?;
        let elapsed = started.elapsed();
        let content = result
            .structured_content
            .ok_or_else(|| anyhow!("tool `{tool_name}` returned no structured content"))?;
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RetrievalBenchmarkFixture {
    version: String,
    suites: Vec<RetrievalBenchmarkSuiteFixture>,
}

impl RetrievalBenchmarkFixture {
    fn suite(&self, key: &str) -> anyhow::Result<&RetrievalBenchmarkSuiteFixture> {
        self.suites
            .iter()
            .find(|suite| suite.key == key)
            .with_context(|| format!("fixture suite `{key}` should exist"))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RetrievalBenchmarkSuiteFixture {
    key: String,
    language: String,
    description: String,
    agent_id: String,
    memories: Vec<RetrievalBenchmarkMemoryFixture>,
    queries: Vec<RetrievalBenchmarkQueryFixture>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RetrievalBenchmarkMemoryFixture {
    key: String,
    content: String,
    memory_type: String,
    importance: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RetrievalBenchmarkQueryFixture {
    key: String,
    query: String,
    target_memory_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RetrievalBenchmarkReport {
    fixture_version: String,
    suites: Vec<RetrievalBenchmarkSuiteReport>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RetrievalBenchmarkSuiteReport {
    key: String,
    language: String,
    description: String,
    memory_count: usize,
    query_count: usize,
    total_count: usize,
    stale_embeddings_count: usize,
    top1_count: usize,
    recall_at_3_count: usize,
    target_mapping: BTreeMap<String, String>,
    store_outcomes: Vec<RetrievalBenchmarkStoreOutcome>,
    query_outcomes: Vec<RetrievalBenchmarkQueryOutcome>,
    top1_occurrences_by_memory_key: BTreeMap<String, usize>,
    top3_occurrences_by_memory_key: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RetrievalBenchmarkStoreOutcome {
    memory_key: String,
    memory_id: String,
    action: String,
    gate_result: String,
    embedding_status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RetrievalBenchmarkQueryOutcome {
    query_key: String,
    query: String,
    target_memory_key: String,
    target_memory_id: String,
    top1_matches_target: bool,
    recall_at_3: bool,
    target_rank_in_top3: Option<usize>,
    top1_score_margin: Option<f64>,
    top1_similarity_margin: Option<f64>,
    top_results: Vec<RetrievalBenchmarkTopResult>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RetrievalBenchmarkTopResult {
    rank: usize,
    memory_key: String,
    memory_id: String,
    score: f64,
    similarity: f64,
    preview: String,
}

fn benchmark_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_RELATIVE_PATH)
}

fn load_benchmark_fixture() -> anyhow::Result<RetrievalBenchmarkFixture> {
    let path = benchmark_fixture_path();
    let raw_fixture = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read benchmark fixture {}", path.display()))?;
    serde_json::from_str(&raw_fixture)
        .with_context(|| format!("failed to parse benchmark fixture {}", path.display()))
}

fn validate_benchmark_fixture(fixture: &RetrievalBenchmarkFixture) -> anyhow::Result<()> {
    ensure!(
        !fixture.version.trim().is_empty(),
        "fixture version should not be empty"
    );

    let mut suite_keys = HashSet::new();
    for suite in &fixture.suites {
        ensure!(
            suite_keys.insert(suite.key.clone()),
            "duplicate suite key `{}` in benchmark fixture",
            suite.key
        );
        ensure!(
            !suite.memories.is_empty(),
            "suite `{}` should contain memories",
            suite.key
        );
        ensure!(
            !suite.queries.is_empty(),
            "suite `{}` should contain queries",
            suite.key
        );

        let mut memory_keys = HashSet::new();
        for memory in &suite.memories {
            ensure!(
                memory_keys.insert(memory.key.clone()),
                "duplicate memory key `{}` in suite `{}`",
                memory.key,
                suite.key
            );
            ensure!(
                !memory.content.trim().is_empty(),
                "memory `{}` in suite `{}` should have content",
                memory.key,
                suite.key
            );
        }

        let mut query_keys = HashSet::new();
        for query in &suite.queries {
            ensure!(
                query_keys.insert(query.key.clone()),
                "duplicate query key `{}` in suite `{}`",
                query.key,
                suite.key
            );
            ensure!(
                memory_keys.contains(&query.target_memory_key),
                "query `{}` in suite `{}` targets unknown memory `{}`",
                query.key,
                suite.key,
                query.target_memory_key
            );
            ensure!(
                !query.query.trim().is_empty(),
                "query `{}` in suite `{}` should not be empty",
                query.key,
                suite.key
            );
        }
    }

    let english = fixture.suite(ENGLISH_SUITE_KEY)?;
    ensure!(
        english.memories.len() == 36,
        "suite `{ENGLISH_SUITE_KEY}` should contain exactly 36 memories, found {}",
        english.memories.len()
    );
    ensure!(
        english.queries.len() == 12,
        "suite `{ENGLISH_SUITE_KEY}` should contain exactly 12 queries, found {}",
        english.queries.len()
    );

    let french = fixture.suite(FRENCH_SUITE_KEY)?;
    ensure!(
        french.memories.len() == 30,
        "suite `{FRENCH_SUITE_KEY}` should contain exactly 30 memories, found {}",
        french.memories.len()
    );
    ensure!(
        french.queries.len() == 10,
        "suite `{FRENCH_SUITE_KEY}` should contain exactly 10 queries, found {}",
        french.queries.len()
    );

    Ok(())
}

fn parse_string_field(value: &Value, key: &str) -> anyhow::Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .with_context(|| format!("expected string field `{key}` in {value}"))
}

fn parse_nested_string_field(value: &Value, object_key: &str, key: &str) -> anyhow::Result<String> {
    value
        .get(object_key)
        .and_then(Value::as_object)
        .and_then(|object| object.get(key))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .with_context(|| format!("expected string field `{object_key}.{key}` in {value}"))
}

fn parse_usize_field(value: &Value, key: &str) -> anyhow::Result<usize> {
    let raw_value = value
        .get(key)
        .and_then(Value::as_u64)
        .with_context(|| format!("expected numeric field `{key}` in {value}"))?;
    usize::try_from(raw_value).with_context(|| format!("field `{key}` is too large for usize"))
}

fn parse_top_results(
    results: &[Value],
    memory_key_by_id: &HashMap<String, String>,
) -> anyhow::Result<Vec<RetrievalBenchmarkTopResult>> {
    let mut top_results = Vec::with_capacity(results.len());
    for (index, result) in results.iter().enumerate() {
        let memory_id = parse_string_field(result, "id")?;
        let memory_key = memory_key_by_id.get(&memory_id).cloned().with_context(|| {
            format!("search result `{memory_id}` is not part of the fixture run")
        })?;
        let score = result
            .get("score")
            .and_then(Value::as_f64)
            .with_context(|| format!("expected numeric `score` in search result {result}"))?;
        let similarity = result
            .get("similarity")
            .and_then(Value::as_f64)
            .with_context(|| format!("expected numeric `similarity` in search result {result}"))?;
        let preview = result
            .get("preview")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        top_results.push(RetrievalBenchmarkTopResult {
            rank: index + 1,
            memory_key,
            memory_id,
            score,
            similarity,
            preview,
        });
    }
    Ok(top_results)
}

async fn run_benchmark_suite(
    suite: &RetrievalBenchmarkSuiteFixture,
) -> anyhow::Result<RetrievalBenchmarkSuiteReport> {
    let session = StdioChildSession::spawn(&suite.agent_id).await;
    let mut memory_id_by_key = HashMap::new();
    let mut memory_key_by_id = HashMap::new();
    let mut store_outcomes = Vec::with_capacity(suite.memories.len());

    for memory in &suite.memories {
        let (stored, _elapsed) = session
            .call_tool_timed(
                "memory_store",
                json!({
                    "content": memory.content,
                    "memoryType": memory.memory_type,
                    "importance": memory.importance
                }),
            )
            .await
            .with_context(|| format!("memory_store should succeed for `{}`", memory.key))?;

        let action = parse_string_field(&stored, "action")?;
        let gate_result = parse_string_field(&stored, "gateResult")?;
        let embedding_status = parse_string_field(&stored, "embeddingStatus")?;
        let memory_id = parse_nested_string_field(&stored, "memory", "id")?;

        ensure!(
            action == "added",
            "fixture memory `{}` should be added, got action `{action}`",
            memory.key
        );
        ensure!(
            gate_result == "accepted",
            "fixture memory `{}` should be accepted, got gateResult `{gate_result}`",
            memory.key
        );
        ensure!(
            embedding_status == "ready",
            "fixture memory `{}` should embed successfully, got embeddingStatus `{embedding_status}`",
            memory.key
        );

        memory_id_by_key.insert(memory.key.clone(), memory_id.clone());
        memory_key_by_id.insert(memory_id.clone(), memory.key.clone());
        store_outcomes.push(RetrievalBenchmarkStoreOutcome {
            memory_key: memory.key.clone(),
            memory_id,
            action,
            gate_result,
            embedding_status,
        });
    }

    let (stats, _stats_elapsed) = session
        .call_tool_timed("memory_stats", json!({}))
        .await
        .with_context(|| format!("memory_stats should succeed for suite `{}`", suite.key))?;
    let total_count = parse_usize_field(&stats, "totalCount")?;
    let stale_embeddings_count = parse_usize_field(&stats, "staleEmbeddingsCount")?;
    ensure!(
        total_count == suite.memories.len(),
        "suite `{}` should store {} memories, memory_stats reported {}",
        suite.key,
        suite.memories.len(),
        total_count
    );
    ensure!(
        stale_embeddings_count == 0,
        "suite `{}` should keep staleEmbeddingsCount at 0, got {}",
        suite.key,
        stale_embeddings_count
    );

    let mut top1_occurrences_by_memory_key = suite
        .memories
        .iter()
        .map(|memory| (memory.key.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut top3_occurrences_by_memory_key = suite
        .memories
        .iter()
        .map(|memory| (memory.key.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut target_mapping = BTreeMap::new();
    let mut query_outcomes = Vec::with_capacity(suite.queries.len());

    for query in &suite.queries {
        let target_memory_id = memory_id_by_key
            .get(&query.target_memory_key)
            .cloned()
            .with_context(|| {
                format!(
                    "query `{}` in suite `{}` should resolve target `{}`",
                    query.key, suite.key, query.target_memory_key
                )
            })?;
        target_mapping.insert(query.key.clone(), query.target_memory_key.clone());

        let (search, _elapsed) = session
            .call_tool_timed(
                "memory_search",
                json!({
                    "query": query.query,
                    "limit": 5
                }),
            )
            .await
            .with_context(|| format!("memory_search should succeed for query `{}`", query.key))?;

        let raw_results = search["results"]
            .as_array()
            .with_context(|| format!("search results missing for query `{}`", query.key))?;
        ensure!(
            raw_results.len() >= 3,
            "query `{}` in suite `{}` should return at least 3 results, got {}",
            query.key,
            suite.key,
            raw_results.len()
        );
        let top_results = parse_top_results(raw_results, &memory_key_by_id)?;
        for result in top_results.iter().take(3) {
            let occurrences = top3_occurrences_by_memory_key
                .get_mut(&result.memory_key)
                .with_context(|| {
                    format!(
                        "top-3 result `{}` should map back to a fixture memory key",
                        result.memory_key
                    )
                })?;
            *occurrences += 1;
        }

        let top1 = top_results
            .first()
            .with_context(|| format!("query `{}` should return at least one result", query.key))?;
        let top1_occurrences = top1_occurrences_by_memory_key
            .get_mut(&top1.memory_key)
            .with_context(|| {
                format!(
                    "top-1 result `{}` should map back to a fixture memory key",
                    top1.memory_key
                )
            })?;
        *top1_occurrences += 1;

        let target_rank_in_top3 = top_results
            .iter()
            .take(3)
            .position(|result| result.memory_id == target_memory_id)
            .map(|rank| rank + 1);
        let top1_matches_target = top1.memory_id == target_memory_id;
        let top1_score_margin = top_results.get(1).map(|second| top1.score - second.score);
        let top1_similarity_margin = top_results
            .get(1)
            .map(|second| top1.similarity - second.similarity);

        query_outcomes.push(RetrievalBenchmarkQueryOutcome {
            query_key: query.key.clone(),
            query: query.query.clone(),
            target_memory_key: query.target_memory_key.clone(),
            target_memory_id,
            top1_matches_target,
            recall_at_3: target_rank_in_top3.is_some(),
            target_rank_in_top3,
            top1_score_margin,
            top1_similarity_margin,
            top_results,
        });
    }

    let stderr = session.shutdown().await;
    if !stderr.trim().is_empty() {
        println!("--- BENCH STDERR {} ---\n{stderr}", suite.key);
    }

    let top1_count = query_outcomes
        .iter()
        .filter(|outcome| outcome.top1_matches_target)
        .count();
    let recall_at_3_count = query_outcomes
        .iter()
        .filter(|outcome| outcome.recall_at_3)
        .count();

    Ok(RetrievalBenchmarkSuiteReport {
        key: suite.key.clone(),
        language: suite.language.clone(),
        description: suite.description.clone(),
        memory_count: suite.memories.len(),
        query_count: suite.queries.len(),
        total_count,
        stale_embeddings_count,
        top1_count,
        recall_at_3_count,
        target_mapping,
        store_outcomes,
        query_outcomes,
        top1_occurrences_by_memory_key,
        top3_occurrences_by_memory_key,
    })
}

async fn run_benchmark_fixture(
    fixture: &RetrievalBenchmarkFixture,
) -> anyhow::Result<RetrievalBenchmarkReport> {
    let mut suite_reports = Vec::with_capacity(fixture.suites.len());
    for suite in &fixture.suites {
        suite_reports.push(run_benchmark_suite(suite).await?);
    }

    Ok(RetrievalBenchmarkReport {
        fixture_version: fixture.version.clone(),
        suites: suite_reports,
    })
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
    ensure!(
        response.status().is_success(),
        "ollama embeddings endpoint returned {}",
        response.status()
    );
    let payload: Value = response.json().await?;
    let len = payload["embedding"].as_array().map_or(0, Vec::len);
    ensure!(len == 768, "expected 768 embedding dimensions, got {len}");
    Ok(true)
}

#[tokio::test]
async fn versioned_retrieval_benchmark_fixture_runs_through_stdio() -> anyhow::Result<()> {
    if !ensure_ollama_is_up().await? {
        return Ok(());
    }

    let fixture = load_benchmark_fixture()?;
    validate_benchmark_fixture(&fixture)?;
    let report = run_benchmark_fixture(&fixture).await?;

    println!("--- RETRIEVAL BENCHMARK REPORT START ---");
    println!("{}", serde_json::to_string_pretty(&report)?);
    println!("--- RETRIEVAL BENCHMARK REPORT END ---");
    Ok(())
}

#[tokio::test]
async fn reproduce_under_load_and_measure_semantic_margins() -> anyhow::Result<()> {
    if !ensure_ollama_is_up().await? {
        return Ok(());
    }
    let session = StdioChildSession::spawn("wu13-repro-agent").await;

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
            .ok_or_else(|| anyhow!("missing stored id"))?
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
            .ok_or_else(|| anyhow!("search results missing"))?;
        let top1 = results.first().ok_or_else(|| anyhow!("empty results"))?;
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
    let session = StdioChildSession::spawn("wu13-repro-agent").await;

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
                .ok_or_else(|| anyhow!("missing stored id"))?
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
        .ok_or_else(|| anyhow!("no stored ids recorded"))?;
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
