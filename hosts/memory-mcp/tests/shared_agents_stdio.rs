use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use rmcp::{
    model::{CallToolRequestParams, CallToolResult},
    service::RunningService,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ClientHandler, RoleClient, ServiceExt,
};
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::{io::AsyncReadExt, process::Command, task::JoinHandle};

#[derive(Default, Clone)]
struct TestClient;
impl ClientHandler for TestClient {}

struct Session {
    client: RunningService<RoleClient, TestClient>,
    stderr: JoinHandle<std::io::Result<String>>,
}

impl Session {
    async fn spawn(db: &Path, agent: &str, shared: bool) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_elegy-memory-mcp-stdio"));
        command
            .env("ELEGY_DB_PATH", db)
            .env("ELEGY_MCP_AGENT_ID", agent)
            .env_remove("ELEGY_MCP_SHARED_SPACE")
            .env_remove("ELEGY_MCP_SHARED_AGENTS")
            .env("ELEGY_MCP_READ_SCOPE", "agent")
            .env("ELEGY_EMBEDDING_BOOT_POLICY", "off")
            .env("RUST_LOG", "info");
        if shared {
            command
                .env("ELEGY_MCP_SHARED_SPACE", "fictional-team")
                .env("ELEGY_MCP_SHARED_AGENTS", r#"["agent-a","agent-b"]"#);
        }
        let (transport, stderr) = TokioChildProcess::builder(command.configure(|_| {}))
            .stderr(Stdio::piped())
            .spawn()
            .expect("stdio child should spawn");
        let mut stderr = stderr.expect("stderr pipe");
        let stderr_task = tokio::spawn(async move {
            let mut text = String::new();
            stderr.read_to_string(&mut text).await?;
            Ok(text)
        });
        let client = tokio::time::timeout(Duration::from_secs(10), TestClient.serve(transport))
            .await
            .expect("client initialization timeout")
            .expect("client initialize");
        Self {
            client,
            stderr: stderr_task,
        }
    }

    async fn call(&self, name: &'static str, args: Value) -> CallToolResult {
        self.client
            .call_tool(
                CallToolRequestParams::new(name).with_arguments(
                    args.as_object()
                        .expect("fixture arguments must be an object")
                        .clone(),
                ),
            )
            .await
            .expect("tool call")
    }

    async fn value(&self, name: &'static str, args: Value) -> Value {
        self.call(name, args)
            .await
            .structured_content
            .expect("structured result")
    }

    async fn assert_denied(&self, name: &'static str, args: Value) {
        if let Ok(result) = self
            .client
            .call_tool(
                CallToolRequestParams::new(name).with_arguments(
                    args.as_object()
                        .expect("fixture arguments must be an object")
                        .clone(),
                ),
            )
            .await
        {
            assert!(result.is_error.unwrap_or(false), "{name} should be denied");
        }
    }

    async fn close(self) {
        self.client.cancel().await.expect("cancel");
        self.stderr
            .await
            .expect("stderr join")
            .expect("stderr read");
    }
}

async fn failed_start(db: &Path, agent: &str, envs: &[(&str, &str)]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_elegy-memory-mcp-stdio"));
    command
        .env("ELEGY_DB_PATH", db)
        .env("ELEGY_MCP_AGENT_ID", agent)
        .env_remove("ELEGY_MCP_SHARED_SPACE")
        .env_remove("ELEGY_MCP_SHARED_AGENTS")
        .env("ELEGY_MCP_READ_SCOPE", "agent")
        .env("ELEGY_EMBEDDING_BOOT_POLICY", "off");
    for (key, value) in envs {
        command.env(key, value);
    }
    tokio::time::timeout(Duration::from_secs(10), command.output())
        .await
        .expect("startup timeout")
        .expect("process output")
}

#[tokio::test]
async fn distinct_shared_stdio_actors_read_but_cannot_mutate_each_other() {
    let temp = TempDir::new().unwrap();
    let db = temp.path().join("memory.db");
    let a = Session::spawn(&db, "agent-a", true).await;
    let stored = a
        .value("memory_store", json!({"content":"agent-a shared fact"}))
        .await;
    let id_a = stored["memory"]["id"].as_str().unwrap().to_string();
    assert_eq!(stored["memory"]["agentId"], json!("agent-a"));
    a.close().await;

    let b = Session::spawn(&db, "agent-b", true).await;
    let listed = b.value("memory_list", json!({"limit":50})).await;
    assert_eq!(listed["memories"][0]["agentId"], json!("agent-a"));
    assert_eq!(
        b.value("memory_recall", json!({"id":id_a})).await["memory"]["agentId"],
        json!("agent-a")
    );
    b.assert_denied("memory_update", json!({"id":id_a,"content":"mutated"}))
        .await;
    b.assert_denied("memory_correct", json!({"id":id_a,"content":"corrected"}))
        .await;
    b.assert_denied("memory_delete", json!({"id":id_a})).await;
    let _ = b
        .value("memory_search", json!({"query":"agent-a shared fact"}))
        .await;
    let own = b
        .value("memory_store", json!({"content":"agent-b own fact"}))
        .await;
    let id_b = own["memory"]["id"].as_str().unwrap().to_string();
    assert_eq!(own["memory"]["agentId"], json!("agent-b"));
    let _ = b.value("memory_stats", json!({})).await;
    let _ = b.value("memory_consolidate", json!({})).await;
    b.close().await;

    let a_again = Session::spawn(&db, "agent-a", true).await;
    assert_eq!(
        a_again.value("memory_recall", json!({"id":id_b})).await["memory"]["agentId"],
        json!("agent-b")
    );
    a_again.close().await;
}

#[tokio::test]
async fn non_shared_actor_cannot_recall_shared_rows() {
    let temp = TempDir::new().unwrap();
    let db = temp.path().join("memory.db");
    let a = Session::spawn(&db, "agent-a", true).await;
    let stored = a
        .value(
            "memory_store",
            json!({"content":"shared row hidden from c"}),
        )
        .await;
    let id = stored["memory"]["id"].as_str().unwrap().to_string();
    a.close().await;
    let c = Session::spawn(&db, "agent-c", false).await;
    let recall = c.value("memory_recall", json!({"id":id})).await;
    assert_eq!(recall["found"], json!(false));
    c.close().await;
}

#[tokio::test]
async fn shared_configuration_fails_closed_before_database_creation() {
    let temp = TempDir::new().unwrap();
    let db = temp.path().join("agents-without-space.db");
    let output = failed_start(
        &db,
        "agent-a",
        &[("ELEGY_MCP_SHARED_AGENTS", r#"["agent-a","agent-b"]"#)],
    )
    .await;
    assert!(!output.status.success(), "agents-without-space should fail");
    assert!(
        !db.exists(),
        "agents-without-space must fail before opening a fresh database"
    );

    let cases = [
        ("missing", "fictional-team", None),
        ("malformed", "fictional-team", Some("not-json")),
        ("missing-caller", "fictional-team", Some(r#"["agent-b"]"#)),
        (
            "duplicate",
            "fictional-team",
            Some(r#"["agent-a","agent-a"]"#),
        ),
        ("blank-space", " ", Some(r#"["agent-a","agent-b"]"#)),
        ("empty-agents", "fictional-team", Some("[]")),
        ("blank-agent", "fictional-team", Some(r#"["agent-a",""]"#)),
    ];
    for (name, space, agents) in cases {
        let temp = TempDir::new().unwrap();
        let db: PathBuf = temp.path().join(format!("{name}.db"));
        let mut envs = vec![("ELEGY_MCP_SHARED_SPACE", space)];
        if let Some(agents) = agents {
            envs.push(("ELEGY_MCP_SHARED_AGENTS", agents));
        }
        let output = failed_start(&db, "agent-a", &envs).await;
        assert!(!output.status.success(), "{name} should fail");
        assert!(
            !db.exists(),
            "{name} must fail before opening a fresh database"
        );
    }
}

#[tokio::test]
async fn shared_stdio_surface_still_exposes_nine_tools() {
    let temp = TempDir::new().unwrap();
    let session = Session::spawn(&temp.path().join("memory.db"), "agent-a", true).await;
    let tools = session
        .client
        .peer()
        .list_tools(None)
        .await
        .expect("list tools");
    assert_eq!(tools.tools.len(), 9);
    session.close().await;
}
