use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use elegy_memory::{EmbeddingError, EmbeddingProvider};
use elegy_memory_mcp::{
    memory_tools::{MemoryBinding, MemoryRepository, DEFAULT_NAMESPACE},
    server::{ElegyMemoryMcpServer, NoopWriteAuditor},
};
use rmcp::{
    model::CallToolRequestParams, service::RunningService, ClientHandler, RoleClient, ServiceExt,
};
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::task::JoinHandle;

#[derive(Default, Clone)]
struct TestClient;

impl ClientHandler for TestClient {}

struct TestSession {
    _temp_dir: TempDir,
    client: RunningService<RoleClient, TestClient>,
    server_task: JoinHandle<()>,
}

impl TestSession {
    async fn new(provider: Arc<dyn EmbeddingProvider>, agent_id: &str) -> Self {
        let temp_dir = TempDir::new().expect("tempdir should create");
        let db_path = temp_dir.path().join("memory.db");
        let binding =
            MemoryBinding::new(DEFAULT_NAMESPACE, agent_id).expect("binding should build");
        let repository = Arc::new(
            MemoryRepository::new_with_embedding_provider(&db_path, binding, provider)
                .expect("repository should build"),
        );
        Self::from_repository(temp_dir, repository).await
    }

    async fn new_without_provider(agent_id: &str) -> Self {
        let temp_dir = TempDir::new().expect("tempdir should create");
        let db_path = temp_dir.path().join("memory.db");
        let binding =
            MemoryBinding::new(DEFAULT_NAMESPACE, agent_id).expect("binding should build");
        let repository =
            Arc::new(MemoryRepository::new(&db_path, binding).expect("repository should build"));
        Self::from_repository(temp_dir, repository).await
    }

    async fn from_repository(temp_dir: TempDir, repository: Arc<MemoryRepository>) -> Self {
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

    async fn call_tool(&self, tool_name: &'static str) -> Value {
        self.call_tool_with_arguments(tool_name, json!({})).await
    }

    async fn call_tool_with_arguments(&self, tool_name: &'static str, arguments: Value) -> Value {
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
        self.server_task
            .await
            .expect("server task should join cleanly");
    }
}

#[derive(Debug)]
struct StubEmbeddingProvider {
    responses: HashMap<String, Vec<f32>>,
}

impl StubEmbeddingProvider {
    fn new<I, S>(responses: I) -> Self
    where
        I: IntoIterator<Item = (S, Vec<f32>)>,
        S: Into<String>,
    {
        Self {
            responses: responses
                .into_iter()
                .map(|(text, embedding)| (text.into(), embedding))
                .collect(),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for StubEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let trimmed = text.trim().to_string();
        self.responses.get(&trimmed).cloned().ok_or_else(|| {
            EmbeddingError::Provider(format!("missing stub embedding for `{trimmed}`"))
        })
    }

    fn dimensions(&self) -> usize {
        768
    }

    fn model_id(&self) -> &str {
        "phase4-regression-stub"
    }
}

fn axis_embedding(axis: usize) -> Vec<f32> {
    let mut embedding = vec![0.0; 768];
    embedding[axis] = 1.0;
    embedding
}

#[derive(Debug)]
struct FailingEmbeddingProvider;

#[async_trait]
impl EmbeddingProvider for FailingEmbeddingProvider {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Err(EmbeddingError::Provider(
            "simulated embedding outage".to_string(),
        ))
    }

    fn dimensions(&self) -> usize {
        768
    }

    fn model_id(&self) -> &str {
        "phase4-failing-provider"
    }
}

#[tokio::test]
async fn memory_store_accepts_explicit_null_defaulted_fields() {
    let content = "Explicit null defaults now deserialize across MCP memory tools.";
    let provider: Arc<dyn EmbeddingProvider> =
        Arc::new(StubEmbeddingProvider::new([(content, axis_embedding(0))]));
    let session = TestSession::new(provider, "phase4-null-agent").await;

    let stored = session
        .call_tool_with_arguments(
            "memory_store",
            json!({
                "content": content,
                "summary": null,
                "memoryType": null,
                "importance": null,
                "provenance": null,
                "sensitivity": null,
                "tags": null,
                "customMetadata": null
            }),
        )
        .await;

    assert_eq!(stored["action"], json!("added"));
    assert_eq!(stored["gateResult"], json!("accepted"));
    assert_eq!(stored["embeddingStatus"], json!("ready"));
    assert_eq!(stored["memory"]["memoryType"], json!("observation"));
    assert_eq!(stored["memory"]["provenance"], json!("user-stated"));
    assert_eq!(stored["memory"]["importance"], json!(0.5));
    assert_eq!(stored["memory"]["tags"], json!([]));
    assert!(
        stored["memory"].get("summary").is_none(),
        "null summary should normalize to omission"
    );

    let stats = session.call_tool("memory_stats").await;
    assert_eq!(stats["totalCount"], json!(1));
    assert_eq!(stats["staleEmbeddingsCount"], json!(0));

    session.shutdown().await;
}

#[tokio::test]
async fn concept_only_semantic_search_recalls_stored_memory_in_top_results() {
    let target_content = "Arabica espresso with chocolate finish.";
    let distractor_content = "Granite cliff beside the ocean at sunrise.";
    let query = "fragrant hot drink";
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(StubEmbeddingProvider::new([
        (target_content, axis_embedding(0)),
        (distractor_content, axis_embedding(1)),
        (query, axis_embedding(0)),
    ]));
    let session = TestSession::new(provider, "phase4-semantic-agent").await;

    let target = session
        .call_tool_with_arguments(
            "memory_store",
            json!({
                "content": target_content,
                "memoryType": "fact",
                "importance": 0.8
            }),
        )
        .await;
    let distractor = session
        .call_tool_with_arguments(
            "memory_store",
            json!({
                "content": distractor_content,
                "memoryType": "fact",
                "importance": 0.8
            }),
        )
        .await;

    let search = session
        .call_tool_with_arguments(
            "memory_search",
            json!({
                "query": query,
                "limit": 3
            }),
        )
        .await;

    let results = search["results"]
        .as_array()
        .expect("search results should be an array");
    assert!(
        !results.is_empty(),
        "concept-only semantic search should return at least one result"
    );
    assert_eq!(results[0]["id"], target["memory"]["id"]);
    assert_ne!(results[0]["id"], distractor["memory"]["id"]);
    assert!(
        results[0]["similarity"]
            .as_f64()
            .expect("top result similarity should be numeric")
            > 0.5,
        "top semantic result should clear the Phase 3 similarity bar"
    );

    let stats = session.call_tool("memory_stats").await;
    assert_eq!(stats["totalCount"], json!(2));
    assert_eq!(stats["staleEmbeddingsCount"], json!(0));

    session.shutdown().await;
}

#[tokio::test]
async fn memory_store_reports_failed_embedding_status_when_provider_errors() {
    let session = TestSession::new(
        Arc::new(FailingEmbeddingProvider),
        "phase4-failed-embedding-agent",
    )
    .await;

    let stored = session
        .call_tool_with_arguments(
            "memory_store",
            json!({
                "content": "Embedding failures should still surface as structured MCP responses."
            }),
        )
        .await;

    assert_eq!(stored["action"], json!("added"));
    assert_eq!(stored["embeddingStatus"], json!("failed"));
    let stats = session.call_tool("memory_stats").await;
    assert_eq!(stats["staleEmbeddingsCount"], json!(1));

    session.shutdown().await;
}

#[tokio::test]
async fn memory_store_reports_skipped_no_provider_when_embeddings_are_disabled() {
    let session = TestSession::new_without_provider("phase4-no-provider-agent").await;

    let stored = session
        .call_tool_with_arguments(
            "memory_store",
            json!({
                "content": "Disabled embeddings should be explicit to MCP clients."
            }),
        )
        .await;

    assert_eq!(stored["action"], json!("added"));
    assert_eq!(stored["embeddingStatus"], json!("skipped_no_provider"));
    let stats = session.call_tool("memory_stats").await;
    assert_eq!(stats["staleEmbeddingsCount"], json!(1));

    session.shutdown().await;
}
