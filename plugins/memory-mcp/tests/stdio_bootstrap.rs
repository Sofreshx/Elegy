use std::{process::Stdio, time::Duration};

use axum::{routing::get, Json, Router};
use rmcp::{
    model::CallToolRequestParams,
    service::RunningService,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ClientHandler, RoleClient, ServiceExt,
};
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::{io::AsyncReadExt, net::TcpListener, process::Command, task::JoinHandle};

#[derive(Default, Clone)]
struct TestClient;

impl ClientHandler for TestClient {}

struct StdioChildSession {
    _temp_dir: TempDir,
    client: RunningService<RoleClient, TestClient>,
    stderr_task: JoinHandle<std::io::Result<String>>,
}

impl StdioChildSession {
    async fn spawn(extra_env: &[(&str, &str)]) -> Self {
        let temp_dir = TempDir::new().expect("tempdir should create");
        let db_path = temp_dir.path().join("memory.db");
        std::fs::write(&db_path, b"").expect("db placeholder should write");

        let mut command = Command::new(env!("CARGO_BIN_EXE_elegy-memory-mcp-stdio"));
        command
            .env("ELEGY_DB_PATH", &db_path)
            .env("ELEGY_MCP_AGENT_ID", "stdio-bootstrap-agent")
            .env("RUST_LOG", "info");
        for (name, value) in extra_env {
            command.env(name, value);
        }

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

    async fn call_tool_with_arguments(&self, tool_name: &'static str, arguments: Value) -> Value {
        let result = self
            .client
            .call_tool(
                CallToolRequestParams::new(tool_name).with_arguments(
                    arguments
                        .as_object()
                        .cloned()
                        .expect("tool arguments should be an object"),
                ),
            )
            .await
            .expect("tool call should succeed");
        result
            .structured_content
            .expect("tool result should include structured content")
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

struct OllamaTagsServer {
    base_url: String,
    task: JoinHandle<()>,
}

impl OllamaTagsServer {
    async fn spawn(model_names: &[&str]) -> Self {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("listener should bind");
        let address = listener
            .local_addr()
            .expect("listener should expose local address");
        let names = model_names
            .iter()
            .map(|name| (*name).to_string())
            .collect::<Vec<_>>();
        let app = Router::new().route(
            "/api/tags",
            get(move || {
                let names = names.clone();
                async move {
                    Json(json!({
                        "models": names
                            .into_iter()
                            .map(|name| json!({ "name": name }))
                            .collect::<Vec<_>>()
                    }))
                }
            }),
        );
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("ollama tags stub should run");
        });

        Self {
            base_url: format!("http://{address}"),
            task,
        }
    }
}

impl Drop for OllamaTagsServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn stdio_output_with_env(extra_env: &[(&str, &str)]) -> std::process::Output {
    let temp_dir = TempDir::new().expect("tempdir should create");
    let db_path = temp_dir.path().join("memory.db");
    std::fs::write(&db_path, b"").expect("db placeholder should write");

    let mut command = Command::new(env!("CARGO_BIN_EXE_elegy-memory-mcp-stdio"));
    command
        .env("ELEGY_DB_PATH", &db_path)
        .env("ELEGY_MCP_AGENT_ID", "stdio-bootstrap-cli-agent")
        .env("RUST_LOG", "info");
    for (name, value) in extra_env {
        command.env(name, value);
    }

    tokio::time::timeout(Duration::from_secs(10), command.output())
        .await
        .expect("stdio command should not hang")
        .expect("stdio command should return output")
}

#[tokio::test]
async fn stdio_binary_fails_fast_when_ollama_is_unreachable() {
    let output = stdio_output_with_env(&[("OLLAMA_URL", "http://127.0.0.1:1")]).await;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("Ollama not reachable at http://127.0.0.1:1"));
    assert!(stderr.contains("ELEGY_ALLOW_NO_EMBEDDINGS=true"));
}

#[tokio::test]
async fn stdio_binary_fails_fast_when_embedding_model_is_missing() {
    let ollama_server = OllamaTagsServer::spawn(&["some-other-model:latest"]).await;
    let output = stdio_output_with_env(&[
        ("OLLAMA_URL", &ollama_server.base_url),
        ("ELEGY_EMBEDDING_MODEL", "nomic-embed-text"),
    ])
    .await;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("Model nomic-embed-text not pulled"));
    assert!(stderr.contains("ollama pull nomic-embed-text"));
}

#[tokio::test]
async fn stdio_binary_degraded_mode_starts_and_reports_skipped_no_provider() {
    let session = StdioChildSession::spawn(&[
        ("OLLAMA_URL", "http://127.0.0.1:1"),
        ("ELEGY_ALLOW_NO_EMBEDDINGS", "true"),
    ])
    .await;

    let stored = session
        .call_tool_with_arguments(
            "memory_store",
            json!({
                "content": "Degraded mode should stay available without embeddings."
            }),
        )
        .await;

    assert_eq!(stored["action"], json!("added"));
    assert_eq!(stored["embeddingStatus"], json!("skipped_no_provider"));

    let stderr = session.shutdown().await;
    assert!(stderr.contains("WARNING: Running in degraded mode without embedding provider"));
}
