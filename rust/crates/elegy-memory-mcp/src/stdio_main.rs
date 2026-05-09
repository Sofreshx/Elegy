use std::{env, path::PathBuf, sync::Arc};

use anyhow::{bail, Context};
use elegy_memory_mcp::{
    memory_tools::{MemoryBinding, MemoryRepository, DEFAULT_NAMESPACE},
    server::{ElegyMemoryMcpServer, NoopWriteAuditor, WriteAuditor},
};
use rmcp::ServiceExt;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const ELEGY_DB_PATH: &str = "ELEGY_DB_PATH";
const ELEGY_MCP_AGENT_ID: &str = "ELEGY_MCP_AGENT_ID";
const OLLAMA_URL: &str = "OLLAMA_URL";
const RUST_LOG: &str = "RUST_LOG";
const DEFAULT_AGENT_ID: &str = "default-agent";
const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
#[cfg(test)]
const EXPECTED_TOOL_NAMES: [&str; 8] = [
    "memory_correct",
    "memory_delete",
    "memory_list",
    "memory_recall",
    "memory_search",
    "memory_stats",
    "memory_store",
    "memory_update",
];

#[tokio::main]
async fn main() {
    init_logging();

    if let Err(startup_error) = run().await {
        let error_message = format!("{startup_error:#}");
        error!(error = %error_message, "startup failed");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let config = StdioConfig::from_env().context("loading stdio configuration")?;
    env::set_var(OLLAMA_URL, &config.ollama_url);
    let runtime = build_stdio_runtime(&config).context("building stdio MCP server")?;

    info!(
        db_path = %config.db_path.display(),
        ollama_url = %config.ollama_url,
        memory_namespace = runtime.memory_repository.namespace(),
        memory_agent_id = runtime.memory_repository.agent_id(),
        "elegy-memory-mcp stdio starting"
    );

    let running_service = runtime
        .server
        .serve(rmcp::transport::stdio())
        .await
        .context("starting MCP stdio transport")?;
    let quit_reason = running_service
        .waiting()
        .await
        .context("running MCP stdio transport")?;

    info!(?quit_reason, "elegy-memory-mcp stdio stopped");
    Ok(())
}

struct StdioServerRuntime {
    memory_repository: Arc<MemoryRepository>,
    server: ElegyMemoryMcpServer,
}

fn build_stdio_runtime(config: &StdioConfig) -> anyhow::Result<StdioServerRuntime> {
    let binding = MemoryBinding::new(DEFAULT_NAMESPACE, &config.agent_id)
        .context("configuring stdio memory binding")?;
    let memory_repository = Arc::new(
        MemoryRepository::new(&config.db_path, binding)
            .context("initializing stdio memory repository")?,
    );
    let write_auditor: Arc<dyn WriteAuditor> = Arc::new(NoopWriteAuditor);

    Ok(StdioServerRuntime {
        server: ElegyMemoryMcpServer::new(Arc::clone(&memory_repository), write_auditor),
        memory_repository,
    })
}

#[derive(Debug)]
struct StdioConfig {
    db_path: PathBuf,
    agent_id: String,
    ollama_url: String,
}

impl StdioConfig {
    fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            db_path: required_path_env(ELEGY_DB_PATH)?,
            agent_id: configured_agent_id()?,
            ollama_url: optional_string_env(OLLAMA_URL, DEFAULT_OLLAMA_URL)?,
        })
    }
}

fn init_logging() {
    tracing_subscriber::registry()
        .with(stdio_log_filter())
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(std::io::stderr),
        )
        .init();
}

fn stdio_log_filter() -> EnvFilter {
    match env::var(RUST_LOG) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                EnvFilter::new("info")
            } else {
                match EnvFilter::try_new(trimmed) {
                    Ok(filter) => filter,
                    Err(parse_error) => {
                        eprintln!(
                            "warning: {RUST_LOG}={trimmed:?} is invalid ({parse_error}); defaulting to info"
                        );
                        EnvFilter::new("info")
                    }
                }
            }
        }
        Err(env::VarError::NotPresent) => EnvFilter::new("info"),
        Err(env::VarError::NotUnicode(_)) => {
            eprintln!("warning: {RUST_LOG} must be valid Unicode; defaulting to info");
            EnvFilter::new("info")
        }
    }
}

fn required_path_env(name: &'static str) -> anyhow::Result<PathBuf> {
    match env::var(name) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                bail!("{name} is required and must not be empty");
            }
            Ok(PathBuf::from(trimmed))
        }
        Err(env::VarError::NotPresent) => bail!("{name} is required"),
        Err(env::VarError::NotUnicode(_)) => bail!("{name} must be valid Unicode"),
    }
}

fn configured_agent_id() -> anyhow::Result<String> {
    match env::var(ELEGY_MCP_AGENT_ID) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                bail!("{ELEGY_MCP_AGENT_ID} must not be empty when set");
            }
            Ok(trimmed.to_string())
        }
        Err(env::VarError::NotPresent) => {
            warn!(
                default_agent_id = DEFAULT_AGENT_ID,
                "{ELEGY_MCP_AGENT_ID} is not set; defaulting agent binding"
            );
            Ok(DEFAULT_AGENT_ID.to_string())
        }
        Err(env::VarError::NotUnicode(_)) => {
            bail!("{ELEGY_MCP_AGENT_ID} must be valid Unicode")
        }
    }
}

fn optional_string_env(name: &'static str, default_value: &'static str) -> anyhow::Result<String> {
    match env::var(name) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Ok(default_value.to_string())
            } else {
                Ok(trimmed.to_string())
            }
        }
        Err(env::VarError::NotPresent) => Ok(default_value.to_string()),
        Err(env::VarError::NotUnicode(_)) => bail!("{name} must be valid Unicode"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::{ClientHandler, ServiceExt};
    use tempfile::TempDir;

    #[derive(Default, Clone)]
    struct TestClient;

    impl ClientHandler for TestClient {}

    #[tokio::test]
    async fn stdio_runtime_initializes_and_lists_expected_tools_over_duplex_transport() {
        let temp_dir = TempDir::new().expect("tempdir should create");
        let db_path = temp_dir.path().join("memory.db");
        std::fs::write(&db_path, b"").expect("db placeholder should write");
        let config = StdioConfig {
            db_path,
            agent_id: "stdio-test-agent".to_string(),
            ollama_url: DEFAULT_OLLAMA_URL.to_string(),
        };
        let runtime = build_stdio_runtime(&config).expect("stdio runtime should build");

        assert_eq!(runtime.memory_repository.namespace(), DEFAULT_NAMESPACE);
        assert_eq!(runtime.memory_repository.agent_id(), "stdio-test-agent");

        let (server_transport, client_transport) = tokio::io::duplex(4096);
        let server_task = tokio::spawn(async move {
            let service = runtime
                .server
                .serve(server_transport)
                .await
                .expect("server should initialize");
            service.waiting().await.expect("server should run cleanly");
        });

        let client_service = TestClient
            .serve(client_transport)
            .await
            .expect("client should initialize");
        let mut tool_names = client_service
            .list_all_tools()
            .await
            .expect("client should list tools")
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();
        tool_names.sort();
        let expected_tool_names = EXPECTED_TOOL_NAMES
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();

        assert_eq!(tool_names.len(), EXPECTED_TOOL_NAMES.len());
        assert_eq!(tool_names, expected_tool_names);

        client_service.cancel().await.expect("client should cancel");
        server_task.await.expect("server task should join");
    }
}
