use std::{env, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{bail, Context};
use elegy_memory::{
    embedding::CircuitBreakerEmbeddingProvider, EmbeddingProvider, MemoryScope,
    OllamaEmbeddingProvider, DEFAULT_OLLAMA_MODEL,
};
use elegy_memory_mcp::{
    memory_tools::{MemoryBinding, MemoryRepository},
    server::{ElegyMemoryMcpServer, NoopWriteAuditor, WriteAuditor},
};
use reqwest::Client;
use rmcp::ServiceExt;
use serde::Deserialize;
use tracing::{error, info, warn};

const ELEGY_DB_PATH: &str = "ELEGY_DB_PATH";
const ELEGY_MCP_AGENT_ID: &str = "ELEGY_MCP_AGENT_ID";
const ELEGY_MCP_READ_SCOPE: &str = "ELEGY_MCP_READ_SCOPE";
const ELEGY_EMBEDDING_MODEL: &str = "ELEGY_EMBEDDING_MODEL";
const ELEGY_EMBEDDING_BOOT_POLICY: &str = "ELEGY_EMBEDDING_BOOT_POLICY";
const ELEGY_ALLOW_NO_EMBEDDINGS: &str = "ELEGY_ALLOW_NO_EMBEDDINGS";
const OLLAMA_URL: &str = "OLLAMA_URL";
const DEFAULT_AGENT_ID: &str = "default-agent";
const DEFAULT_READ_SCOPE: &str = "session";
const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
const OLLAMA_BOOT_TIMEOUT: Duration = Duration::from_secs(5);
// Bounded to stay well under typical MCP client initialize handshake timeouts.
const OLLAMA_BOOT_RETRY_ATTEMPTS: u32 = 3;
const OLLAMA_BOOT_RETRY_BACKOFF: Duration = Duration::from_secs(2);
#[cfg(test)]
const EXPECTED_TOOL_NAMES: [&str; 9] = [
    "memory_consolidate",
    "memory_correct",
    "memory_delete",
    "memory_list",
    "memory_recall",
    "memory_search",
    "memory_stats",
    "memory_store",
    "memory_update",
];

fn main() {
    elegy_memory_mcp::observability::init_logging(
        elegy_memory_mcp::observability::LogFormat::from_env(),
    );

    match elegy_memory::runtime::block_on(run()) {
        Ok(Ok(())) => {}
        Ok(Err(startup_error)) => {
            let error_message = format!("{startup_error:#}");
            error!(error = %error_message, "startup failed");
            std::process::exit(1);
        }
        Err(runtime_error) => {
            error!(error = %runtime_error, "failed to start shared tokio runtime");
            std::process::exit(1);
        }
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
        embedding_boot_policy = %config.embedding_boot_policy,
        memory_namespace = runtime.memory_repository.namespace(),
        memory_agent_id = runtime.memory_repository.agent_id(),
        memory_read_scope = ?config.read_scope,
        "elegy-memory-mcp stdio starting"
    );

    let running_service = runtime
        .server
        .serve(rmcp::transport::stdio())
        .await
        .context("starting MCP stdio transport")?;
    let stop_reason = run_until_shutdown(running_service).await?;
    info!(?stop_reason, "elegy-memory-mcp stdio stopped");
    Ok(())
}

#[derive(Debug)]
enum StopReason {
    ClientQuit,
    SigInt,
    #[cfg(unix)]
    SigTerm,
}

async fn run_until_shutdown(
    running_service: rmcp::service::RunningService<rmcp::RoleServer, ElegyMemoryMcpServer>,
) -> anyhow::Result<StopReason> {
    // `RunningService::waiting`/`cancel` both take `self` by value, so a signal
    // future cannot share a `select!` with `.waiting()` directly. Instead, race the
    // signal in a background task against the non-consuming cancellation token, and
    // let `.waiting()` observe the resulting cancellation (or the client's own
    // disconnect) so cleanup is always awaited to completion either way.
    let cancellation_token = running_service.cancellation_token();
    let signal_task: tokio::task::JoinHandle<StopReason> = tokio::spawn(async move {
        let reason = wait_for_os_signal().await;
        cancellation_token.cancel();
        reason
    });

    running_service
        .waiting()
        .await
        .context("running MCP stdio transport")?;

    signal_task.abort();
    let stop_reason = match signal_task.await {
        Ok(reason) => reason,
        Err(_) => StopReason::ClientQuit,
    };
    Ok(stop_reason)
}

async fn wait_for_os_signal() -> StopReason {
    #[cfg(unix)]
    {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sigterm) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        warn!("received SIGINT, shutting down");
                        StopReason::SigInt
                    }
                    _ = sigterm.recv() => {
                        warn!("received SIGTERM, shutting down");
                        StopReason::SigTerm
                    }
                }
            }
            Err(error) => {
                warn!(error = %error, "failed to register SIGTERM handler; watching SIGINT only");
                tokio::signal::ctrl_c().await.ok();
                warn!("received SIGINT, shutting down");
                StopReason::SigInt
            }
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.ok();
        warn!("received SIGINT, shutting down");
        StopReason::SigInt
    }
}

struct StdioServerRuntime {
    memory_repository: Arc<MemoryRepository>,
    server: ElegyMemoryMcpServer,
}

enum StdioEmbeddingBootstrap {
    ProviderBacked(Arc<dyn EmbeddingProvider>),
    DisabledNoProvider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmbeddingBootPolicy {
    Require,
    Prefer,
    Off,
}

impl std::fmt::Display for EmbeddingBootPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            EmbeddingBootPolicy::Require => "require",
            EmbeddingBootPolicy::Prefer => "prefer",
            EmbeddingBootPolicy::Off => "off",
        })
    }
}

async fn build_stdio_runtime(config: &StdioConfig) -> anyhow::Result<StdioServerRuntime> {
    let embedding_bootstrap = resolve_embedding_bootstrap(config).await?;
    build_stdio_runtime_with_bootstrap(config, embedding_bootstrap)
}

fn build_embedding_provider(config: &StdioConfig) -> anyhow::Result<Arc<dyn EmbeddingProvider>> {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(
        OllamaEmbeddingProvider::new(&config.ollama_url, &config.embedding_model).with_context(
            || {
                format!(
                    "configuring Ollama embedding provider for {} with model {}",
                    config.ollama_url, config.embedding_model
                )
            },
        )?,
    );
    Ok(Arc::new(CircuitBreakerEmbeddingProvider::from_env(
        provider,
    )))
}

async fn resolve_embedding_bootstrap(
    config: &StdioConfig,
) -> anyhow::Result<StdioEmbeddingBootstrap> {
    resolve_embedding_bootstrap_with_retry(
        config,
        OLLAMA_BOOT_RETRY_ATTEMPTS,
        OLLAMA_BOOT_RETRY_BACKOFF,
    )
    .await
}

async fn resolve_embedding_bootstrap_with_retry(
    config: &StdioConfig,
    attempts: u32,
    backoff: Duration,
) -> anyhow::Result<StdioEmbeddingBootstrap> {
    match config.embedding_boot_policy {
        EmbeddingBootPolicy::Off => {
            warn!(
                "WARNING: Running in degraded mode without embedding provider. Semantic search will not work. All memory_store calls will return embeddingStatus: skipped_no_provider."
            );
            Ok(StdioEmbeddingBootstrap::DisabledNoProvider)
        }
        EmbeddingBootPolicy::Require => {
            verify_ollama_bootstrap(config).await?;
            Ok(StdioEmbeddingBootstrap::ProviderBacked(
                build_embedding_provider(config)?,
            ))
        }
        EmbeddingBootPolicy::Prefer => {
            let attempts = attempts.max(1);
            let mut last_error = None;
            for attempt in 1..=attempts {
                match verify_ollama_bootstrap(config).await {
                    Ok(()) => {
                        return Ok(StdioEmbeddingBootstrap::ProviderBacked(
                            build_embedding_provider(config)?,
                        ));
                    }
                    Err(error) => {
                        warn!(
                            attempt,
                            attempts,
                            error = %format!("{error:#}"),
                            "embedding provider boot probe failed; will retry"
                        );
                        last_error = Some(error);
                        if attempt < attempts {
                            tokio::time::sleep(backoff).await;
                        }
                    }
                }
            }
            warn!(
                error = %format!("{:#}", last_error.expect("retry loop runs at least once")),
                "WARNING: embedding provider unavailable after {attempts} attempts; starting in degraded mode. Semantic search will not work. All memory_store calls will return embeddingStatus: skipped_no_provider."
            );
            Ok(StdioEmbeddingBootstrap::DisabledNoProvider)
        }
    }
}

fn build_stdio_runtime_with_bootstrap(
    config: &StdioConfig,
    embedding_bootstrap: StdioEmbeddingBootstrap,
) -> anyhow::Result<StdioServerRuntime> {
    let binding = MemoryBinding::new(&config.agent_id, &config.agent_id)
        .context("configuring stdio memory binding")?
        .with_read_scope(config.read_scope)
        .including_unowned(true);
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
    read_scope: MemoryScope,
    ollama_url: String,
    embedding_model: String,
    embedding_boot_policy: EmbeddingBootPolicy,
}

impl StdioConfig {
    fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            db_path: required_path_env(ELEGY_DB_PATH)?,
            agent_id: configured_agent_id()?,
            read_scope: configured_read_scope()?,
            ollama_url: optional_string_env(OLLAMA_URL, DEFAULT_OLLAMA_URL)?,
            embedding_model: optional_string_env(ELEGY_EMBEDDING_MODEL, DEFAULT_OLLAMA_MODEL)?,
            embedding_boot_policy: configured_embedding_boot_policy()?,
        })
    }
}

/// Resolve the base scope reads expand from.
///
/// Defaults to the widest range: this binary is a single-user local surface, so
/// hiding the narrower scopes only strands knowledge the CLI already wrote.
/// Writes stay pinned to `MemoryScope::Agent` regardless.
fn configured_read_scope() -> anyhow::Result<MemoryScope> {
    let value = optional_string_env(ELEGY_MCP_READ_SCOPE, DEFAULT_READ_SCOPE)?;
    match value.trim().to_ascii_lowercase().as_str() {
        "session" => Ok(MemoryScope::Session),
        "workspace" => Ok(MemoryScope::Workspace),
        "user" => Ok(MemoryScope::User),
        "agent" => Ok(MemoryScope::Agent),
        _ => bail!("{ELEGY_MCP_READ_SCOPE} must be one of session, workspace, user, agent"),
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

fn parse_bool_env(name: &'static str, value: &str) -> anyhow::Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => bail!("{name} must be one of 0, 1, true, false, yes, no, on, off"),
    }
}

fn configured_embedding_boot_policy() -> anyhow::Result<EmbeddingBootPolicy> {
    match env::var(ELEGY_EMBEDDING_BOOT_POLICY) {
        Ok(value) => parse_boot_policy_env(&value),
        Err(env::VarError::NotPresent) => legacy_allow_no_embeddings_policy(),
        Err(env::VarError::NotUnicode(_)) => {
            bail!("{ELEGY_EMBEDDING_BOOT_POLICY} must be valid Unicode")
        }
    }
}

fn legacy_allow_no_embeddings_policy() -> anyhow::Result<EmbeddingBootPolicy> {
    match env::var(ELEGY_ALLOW_NO_EMBEDDINGS) {
        Ok(value) => {
            let allow = parse_bool_env(ELEGY_ALLOW_NO_EMBEDDINGS, &value)?;
            warn!(
                "{ELEGY_ALLOW_NO_EMBEDDINGS} is deprecated; set {ELEGY_EMBEDDING_BOOT_POLICY}=require|prefer|off instead"
            );
            Ok(if allow {
                EmbeddingBootPolicy::Off
            } else {
                EmbeddingBootPolicy::Require
            })
        }
        Err(env::VarError::NotPresent) => Ok(EmbeddingBootPolicy::Prefer),
        Err(env::VarError::NotUnicode(_)) => {
            bail!("{ELEGY_ALLOW_NO_EMBEDDINGS} must be valid Unicode")
        }
    }
}

fn parse_boot_policy_env(value: &str) -> anyhow::Result<EmbeddingBootPolicy> {
    match value.trim().to_ascii_lowercase().as_str() {
        "require" => Ok(EmbeddingBootPolicy::Require),
        "prefer" => Ok(EmbeddingBootPolicy::Prefer),
        "off" => Ok(EmbeddingBootPolicy::Off),
        _ => bail!("{ELEGY_EMBEDDING_BOOT_POLICY} must be one of require, prefer, off"),
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
                "Ollama not reachable at {}. Start Ollama (open Ollama Desktop or run 'ollama serve'). Required model: {}. Set ELEGY_EMBEDDING_BOOT_POLICY=off to start without embeddings, or =prefer to retry and degrade automatically (default).",
                config.ollama_url, config.embedding_model
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        bail!(
            "Ollama not reachable at {}. Start Ollama (open Ollama Desktop or run 'ollama serve'). Required model: {}. Set ELEGY_EMBEDDING_BOOT_POLICY=off to start without embeddings, or =prefer to retry and degrade automatically (default). /api/tags returned {}.",
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
            "Model {} not pulled. Run: 'ollama pull {}'. Set ELEGY_EMBEDDING_BOOT_POLICY=off to start without embeddings, or =prefer to retry and degrade automatically (default).",
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
            read_scope: MemoryScope::Session,
            ollama_url: DEFAULT_OLLAMA_URL.to_string(),
            embedding_model: DEFAULT_OLLAMA_MODEL.to_string(),
            embedding_boot_policy: EmbeddingBootPolicy::Prefer,
        };
        let runtime = build_stdio_runtime_with_bootstrap(
            &config,
            StdioEmbeddingBootstrap::ProviderBacked(
                build_embedding_provider(&config).expect("embedding provider should build"),
            ),
        )
        .expect("stdio runtime should build");

        assert_eq!(runtime.memory_repository.namespace(), "stdio-test-agent");
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

    #[tokio::test]
    async fn stdio_fallback_namespace_is_default_agent_when_elegy_mcp_agent_id_is_unset() {
        let temp_dir = TempDir::new().expect("tempdir should create");
        let db_path = temp_dir.path().join("memory.db");
        std::fs::write(&db_path, b"").expect("db placeholder should write");
        let config = StdioConfig {
            db_path,
            agent_id: DEFAULT_AGENT_ID.to_string(),
            read_scope: MemoryScope::Session,
            ollama_url: DEFAULT_OLLAMA_URL.to_string(),
            embedding_model: DEFAULT_OLLAMA_MODEL.to_string(),
            embedding_boot_policy: EmbeddingBootPolicy::Off,
        };
        let runtime = build_stdio_runtime_with_bootstrap(
            &config,
            StdioEmbeddingBootstrap::DisabledNoProvider,
        )
        .expect("stdio runtime should build with degraded mode");

        assert_eq!(runtime.memory_repository.namespace(), DEFAULT_AGENT_ID);
        assert_eq!(runtime.memory_repository.agent_id(), DEFAULT_AGENT_ID);

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
        let tool_names = client_service
            .list_all_tools()
            .await
            .expect("client should list tools")
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();
        assert_eq!(tool_names.len(), EXPECTED_TOOL_NAMES.len());

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
            read_scope: MemoryScope::Session,
            ollama_url: format!("http://{address}"),
            embedding_model: DEFAULT_OLLAMA_MODEL.to_string(),
            embedding_boot_policy: EmbeddingBootPolicy::Require,
        };

        verify_ollama_bootstrap(&config)
            .await
            .expect("bootstrap should accept available model");
        server.abort();
    }

    #[test]
    fn parse_boot_policy_env_accepts_known_values_case_insensitively() {
        assert!(matches!(
            parse_boot_policy_env("Require"),
            Ok(EmbeddingBootPolicy::Require)
        ));
        assert!(matches!(
            parse_boot_policy_env("prefer"),
            Ok(EmbeddingBootPolicy::Prefer)
        ));
        assert!(matches!(
            parse_boot_policy_env(" OFF "),
            Ok(EmbeddingBootPolicy::Off)
        ));
    }

    #[test]
    fn parse_boot_policy_env_rejects_unknown_values() {
        assert!(parse_boot_policy_env("sometimes").is_err());
    }

    #[tokio::test]
    async fn resolve_embedding_bootstrap_prefer_degrades_after_exhausting_retries() {
        let config = StdioConfig {
            db_path: PathBuf::from("unused.db"),
            agent_id: "stdio-test-agent".to_string(),
            read_scope: MemoryScope::Session,
            ollama_url: "http://127.0.0.1:1".to_string(),
            embedding_model: DEFAULT_OLLAMA_MODEL.to_string(),
            embedding_boot_policy: EmbeddingBootPolicy::Prefer,
        };

        let started = std::time::Instant::now();
        let bootstrap =
            resolve_embedding_bootstrap_with_retry(&config, 2, Duration::from_millis(10))
                .await
                .expect("prefer policy should degrade instead of failing");

        assert!(matches!(
            bootstrap,
            StdioEmbeddingBootstrap::DisabledNoProvider
        ));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn resolve_embedding_bootstrap_off_skips_ollama_entirely() {
        let config = StdioConfig {
            db_path: PathBuf::from("unused.db"),
            agent_id: "stdio-test-agent".to_string(),
            read_scope: MemoryScope::Session,
            ollama_url: "http://127.0.0.1:1".to_string(),
            embedding_model: DEFAULT_OLLAMA_MODEL.to_string(),
            embedding_boot_policy: EmbeddingBootPolicy::Off,
        };

        let bootstrap = resolve_embedding_bootstrap_with_retry(&config, 3, Duration::from_secs(30))
            .await
            .expect("off policy should never touch the network");

        assert!(matches!(
            bootstrap,
            StdioEmbeddingBootstrap::DisabledNoProvider
        ));
    }
}
