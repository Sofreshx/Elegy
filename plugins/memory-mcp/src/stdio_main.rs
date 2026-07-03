use std::{env, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{bail, Context};
use elegy_memory::{EmbeddingProvider, OllamaEmbeddingProvider, DEFAULT_OLLAMA_MODEL};
use elegy_memory_mcp::{
    memory_tools::{MemoryBinding, MemoryRepository, DEFAULT_NAMESPACE},
    server::{ElegyMemoryMcpServer, NoopWriteAuditor, WriteAuditor},
};
use reqwest::Client;
use rmcp::ServiceExt;
use serde::Deserialize;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const ELEGY_DB_PATH: &str = "ELEGY_DB_PATH";
const ELEGY_MCP_AGENT_ID: &str = "ELEGY_MCP_AGENT_ID";
const ELEGY_EMBEDDING_MODEL: &str = "ELEGY_EMBEDDING_MODEL";
const ELEGY_ALLOW_NO_EMBEDDINGS: &str = "ELEGY_ALLOW_NO_EMBEDDINGS";
const OLLAMA_URL: &str = "OLLAMA_URL";
const RUST_LOG: &str = "RUST_LOG";
const DEFAULT_AGENT_ID: &str = "default-agent";
const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
const OLLAMA_BOOT_TIMEOUT: Duration = Duration::from_secs(5);
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
    let runtime = build_stdio_runtime(&config)
        .await
        .context("building stdio MCP server")?;

    info!(
        db_path = %config.db_path.display(),
        ollama_url = %config.ollama_url,
        embedding_model = %config.embedding_model,
        allow_no_embeddings = config.allow_no_embeddings,
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

enum StdioEmbeddingBootstrap {
    ProviderBacked(Arc<dyn EmbeddingProvider>),
    DisabledNoProvider,
}

async fn build_stdio_runtime(config: &StdioConfig) -> anyhow::Result<StdioServerRuntime> {
    let embedding_bootstrap = resolve_embedding_bootstrap(config).await?;
    build_stdio_runtime_with_bootstrap(config, embedding_bootstrap)
}

fn build_embedding_provider(config: &StdioConfig) -> anyhow::Result<Arc<dyn EmbeddingProvider>> {
    Ok(Arc::new(
        OllamaEmbeddingProvider::new(&config.ollama_url, &config.embedding_model).with_context(
            || {
                format!(
                    "configuring Ollama embedding provider for {} with model {}",
                    config.ollama_url, config.embedding_model
                )
            },
        )?,
    ))
}

async fn resolve_embedding_bootstrap(
    config: &StdioConfig,
) -> anyhow::Result<StdioEmbeddingBootstrap> {
    if config.allow_no_embeddings {
        warn!(
            "WARNING: Running in degraded mode without embedding provider. Semantic search will not work. All memory_store calls will return embeddingStatus: skipped_no_provider."
        );
        return Ok(StdioEmbeddingBootstrap::DisabledNoProvider);
    }

    verify_ollama_bootstrap(config).await?;
    Ok(StdioEmbeddingBootstrap::ProviderBacked(
        build_embedding_provider(config)?,
    ))
}

fn build_stdio_runtime_with_bootstrap(
    config: &StdioConfig,
    embedding_bootstrap: StdioEmbeddingBootstrap,
) -> anyhow::Result<StdioServerRuntime> {
    let binding = MemoryBinding::new(DEFAULT_NAMESPACE, &config.agent_id)
        .context("configuring stdio memory binding")?;
    let memory_repository = Arc::new(match embedding_bootstrap {
        StdioEmbeddingBootstrap::ProviderBacked(embedding_provider) => {
            MemoryRepository::new_with_embedding_provider(
                &config.db_path,
                binding,
                embedding_provider,
            )
            .context("initializing stdio memory repository")?
        }
        StdioEmbeddingBootstrap::DisabledNoProvider => {
            MemoryRepository::new(&config.db_path, binding)
                .context("initializing stdio memory repository")?
        }
    });
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
    embedding_model: String,
    allow_no_embeddings: bool,
}

impl StdioConfig {
    fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            db_path: required_path_env(ELEGY_DB_PATH)?,
            agent_id: configured_agent_id()?,
            ollama_url: optional_string_env(OLLAMA_URL, DEFAULT_OLLAMA_URL)?,
            embedding_model: optional_string_env(ELEGY_EMBEDDING_MODEL, DEFAULT_OLLAMA_MODEL)?,
            allow_no_embeddings: optional_bool_env(ELEGY_ALLOW_NO_EMBEDDINGS, false)?,
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

fn optional_bool_env(name: &'static str, default_value: bool) -> anyhow::Result<bool> {
    match env::var(name) {
        Ok(value) => parse_bool_env(name, &value),
        Err(env::VarError::NotPresent) => Ok(default_value),
        Err(env::VarError::NotUnicode(_)) => bail!("{name} must be valid Unicode"),
    }
}

fn parse_bool_env(name: &'static str, value: &str) -> anyhow::Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => bail!("{name} must be one of 0, 1, true, false, yes, no, on, off"),
    }
}

async fn verify_ollama_bootstrap(config: &StdioConfig) -> anyhow::Result<()> {
    let tags_url = format!("{}/api/tags", config.ollama_url.trim_end_matches('/'));
    let client = Client::builder()
        .connect_timeout(OLLAMA_BOOT_TIMEOUT)
        .timeout(OLLAMA_BOOT_TIMEOUT)
        .build()
        .context("building Ollama bootstrap HTTP client")?;
    let response = client
        .get(&tags_url)
        .send()
        .await
        .with_context(|| {
            format!(
                "Ollama not reachable at {}. Start Ollama (open Ollama Desktop or run 'ollama serve'). Required model: {}. To start in degraded mode without embeddings, set ELEGY_ALLOW_NO_EMBEDDINGS=true.",
                config.ollama_url, config.embedding_model
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        bail!(
            "Ollama not reachable at {}. Start Ollama (open Ollama Desktop or run 'ollama serve'). Required model: {}. To start in degraded mode without embeddings, set ELEGY_ALLOW_NO_EMBEDDINGS=true. /api/tags returned {}.",
            config.ollama_url,
            config.embedding_model,
            status
        );
    }

    let payload: OllamaTagsResponse = response
        .json()
        .await
        .context("decoding Ollama /api/tags response")?;
    if !payload
        .models
        .iter()
        .any(|model| ollama_model_matches(&model.name, &config.embedding_model))
    {
        bail!(
            "Model {} not pulled. Run: 'ollama pull {}'. To start in degraded mode without embeddings, set ELEGY_ALLOW_NO_EMBEDDINGS=true.",
            config.embedding_model,
            config.embedding_model
        );
    }

    info!(
        ollama_url = %config.ollama_url,
        embedding_model = %config.embedding_model,
        "Ollama reachable and embedding model available"
    );
    Ok(())
}

fn ollama_model_matches(available_model: &str, required_model: &str) -> bool {
    let available_model = available_model.trim();
    let required_model = required_model.trim();
    if available_model == required_model {
        return true;
    }
    if required_model.contains(':') {
        return false;
    }

    available_model
        .split(':')
        .next()
        .is_some_and(|model_name| model_name == required_model)
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaModelEntry>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelEntry {
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Json, Router};
    use rmcp::{ClientHandler, ServiceExt};
    use tempfile::TempDir;
    use tokio::net::TcpListener;

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
            embedding_model: DEFAULT_OLLAMA_MODEL.to_string(),
            allow_no_embeddings: false,
        };
        let runtime = build_stdio_runtime_with_bootstrap(
            &config,
            StdioEmbeddingBootstrap::ProviderBacked(
                build_embedding_provider(&config).expect("embedding provider should build"),
            ),
        )
        .expect("stdio runtime should build");

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

    #[test]
    fn ollama_model_match_accepts_tagged_default_model() {
        assert!(ollama_model_matches(
            "nomic-embed-text:latest",
            DEFAULT_OLLAMA_MODEL,
        ));
        assert!(ollama_model_matches(
            "nomic-embed-text:v1",
            DEFAULT_OLLAMA_MODEL
        ));
        assert!(!ollama_model_matches(
            "other-model:latest",
            DEFAULT_OLLAMA_MODEL
        ));
    }

    #[tokio::test]
    async fn verify_ollama_bootstrap_accepts_available_model() {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("listener should bind");
        let address = listener
            .local_addr()
            .expect("listener should expose address");
        let app = Router::new().route(
            "/api/tags",
            get(|| async {
                Json(serde_json::json!({
                    "models": [
                        {"name": "nomic-embed-text:latest"},
                        {"name": "other-model:latest"}
                    ]
                }))
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("tag server should run");
        });

        let config = StdioConfig {
            db_path: PathBuf::from("unused.db"),
            agent_id: "stdio-test-agent".to_string(),
            ollama_url: format!("http://{address}"),
            embedding_model: DEFAULT_OLLAMA_MODEL.to_string(),
            allow_no_embeddings: false,
        };

        verify_ollama_bootstrap(&config)
            .await
            .expect("bootstrap should accept available model");
        server.abort();
    }
}
