use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
    sync::Arc,
};

use chrono::Utc;
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use thiserror::Error;
use tokio::runtime::Builder;
use uuid::Uuid;

use crate::{
    DefaultSalienceGate, EmbeddingError, EmbeddingProvider, GateDecision, GateError, Memory,
    MemoryCandidate, MemoryFilter, MemoryHealthReport, MemoryId, MemoryScope, MemoryState,
    MemoryStore, MemoryType, MemoryVersion, OllamaEmbeddingProvider, ProvenanceLevel,
    ResolutionStatus, SalienceGate, ScoredMemory, SearchQuery, SensitivityLevel, SqliteMemoryStore,
    StoreError, DEFAULT_OLLAMA_BASE_URL, DEFAULT_OLLAMA_MODEL,
};

const DEFAULT_IMPORTANCE: f32 = 0.5;
const DEFAULT_LIMIT: usize = 20;
const DEFAULT_REEMBED_LIMIT: usize = 100;
const PREVIEW_LIMIT: usize = 80;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Store(#[from] StoreError),
    #[error("{0}")]
    Gate(#[from] GateError),
    #[error("{0}")]
    Embedding(#[from] EmbeddingError),
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid memory id `{value}`: {source}")]
    InvalidId {
        value: String,
        #[source]
        source: uuid::Error,
    },
    #[error("{0}")]
    Validation(String),
}

#[derive(Parser, Debug)]
#[command(name = "elegy-memory")]
#[command(about = "MVP CLI for the Elegy memory store")]
struct Cli {
    #[arg(long, value_enum, global = true, default_value_t = OutputFormat::Text)]
    format: OutputFormat,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Add a memory to the store.
    Add {
        #[command(flatten)]
        store: StoreArgs,
        content: String,
        #[arg(long = "type", value_enum, default_value_t = CliMemoryType::Observation)]
        memory_type: CliMemoryType,
        #[arg(long, default_value_t = DEFAULT_IMPORTANCE)]
        importance: f32,
        #[arg(long, value_enum, default_value_t = CliProvenance::UserStated)]
        provenance: CliProvenance,
    },
    /// Search memories with keyword matching plus provider-backed embeddings when configured.
    Search {
        #[command(flatten)]
        store: StoreArgs,
        query: String,
        #[arg(long, default_value_t = DEFAULT_LIMIT)]
        limit: usize,
        #[arg(long)]
        include_dormant: bool,
    },
    /// List memories using simple filters.
    List {
        #[command(flatten)]
        store: StoreArgs,
        #[arg(long = "type", value_enum)]
        memory_type: Option<CliMemoryType>,
        #[arg(long, value_enum)]
        state: Option<CliMemoryState>,
        #[arg(long, default_value_t = DEFAULT_LIMIT)]
        limit: usize,
    },
    /// Inspect a single memory and show its version history.
    Inspect {
        #[command(flatten)]
        store: StoreArgs,
        id: String,
    },
    /// Purge the configured database after confirmation.
    Purge {
        #[command(flatten)]
        store: StoreArgs,
        #[arg(long)]
        yes: bool,
    },
    /// Show a health summary for the current scope.
    Health {
        #[command(flatten)]
        store: StoreArgs,
    },
    /// Export memories as JSON to stdout or a file.
    Export {
        #[command(flatten)]
        store: StoreArgs,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Re-embed stale memories when a provider is configured.
    Reembed {
        #[command(flatten)]
        store: StoreArgs,
        #[arg(long, default_value_t = DEFAULT_REEMBED_LIMIT)]
        limit: usize,
    },
    /// List unresolved contradiction records.
    Contradictions {
        #[command(flatten)]
        store: StoreArgs,
    },
}

#[derive(Args, Clone, Debug)]
struct StoreArgs {
    /// SQLite database path. Defaults to ~/.elegy/memory.db
    #[arg(long)]
    db: Option<PathBuf>,
    /// Memory scope to operate on. Workspace is used by default in this MVP CLI.
    #[arg(long, value_enum, default_value_t = CliScope::Workspace)]
    scope: CliScope,
    /// Embedding provider to enable for provider-backed store search and re-embedding.
    #[arg(long = "embedding-provider", alias = "provider", value_enum)]
    embedding_provider: Option<CliEmbeddingProvider>,
    /// Ollama base URL when `--embedding-provider ollama` is enabled. Defaults to
    /// http://localhost:11434.
    #[arg(long)]
    ollama_url: Option<String>,
    /// Ollama model when `--embedding-provider ollama` is enabled. Defaults to
    /// nomic-embed-text.
    #[arg(long)]
    ollama_model: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum)]
enum CliScope {
    Session,
    Workspace,
    User,
    Agent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum)]
enum CliMemoryType {
    Fact,
    Preference,
    Decision,
    Procedure,
    Observation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum)]
enum CliMemoryState {
    Active,
    Dormant,
    Deleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum)]
enum CliProvenance {
    UserStated,
    AgentObserved,
    Consolidated,
    Imported,
    AgentInferred,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum)]
enum CliEmbeddingProvider {
    Ollama,
}

impl From<CliScope> for MemoryScope {
    fn from(value: CliScope) -> Self {
        match value {
            CliScope::Session => Self::Session,
            CliScope::Workspace => Self::Workspace,
            CliScope::User => Self::User,
            CliScope::Agent => Self::Agent,
        }
    }
}

impl From<CliMemoryType> for MemoryType {
    fn from(value: CliMemoryType) -> Self {
        match value {
            CliMemoryType::Fact => Self::Fact,
            CliMemoryType::Preference => Self::Preference,
            CliMemoryType::Decision => Self::Decision,
            CliMemoryType::Procedure => Self::Procedure,
            CliMemoryType::Observation => Self::Observation,
        }
    }
}

impl From<CliMemoryState> for MemoryState {
    fn from(value: CliMemoryState) -> Self {
        match value {
            CliMemoryState::Active => Self::Active,
            CliMemoryState::Dormant => Self::Dormant,
            CliMemoryState::Deleted => Self::Deleted,
        }
    }
}

impl From<CliProvenance> for ProvenanceLevel {
    fn from(value: CliProvenance) -> Self {
        match value {
            CliProvenance::UserStated => Self::UserStated,
            CliProvenance::AgentObserved => Self::AgentObserved,
            CliProvenance::Consolidated => Self::Consolidated,
            CliProvenance::Imported => Self::Imported,
            CliProvenance::AgentInferred => Self::AgentInferred,
        }
    }
}

struct StoreContext {
    db_path: PathBuf,
    scope: MemoryScope,
    store: SqliteMemoryStore,
    embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
    embedding_provider_label: Option<String>,
}

impl StoreContext {
    fn has_embedding_provider(&self) -> bool {
        self.embedding_provider.is_some()
    }

    fn embedding_provider_label(&self) -> &str {
        self.embedding_provider_label
            .as_deref()
            .unwrap_or("none")
    }
}

impl std::fmt::Debug for StoreContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreContext")
            .field("db_path", &self.db_path)
            .field("scope", &self.scope)
            .field("store", &self.store)
            .field("has_embedding_provider", &self.embedding_provider.is_some())
            .field("embedding_provider_label", &self.embedding_provider_label)
            .finish()
    }
}

#[derive(Serialize)]
struct JsonEnvelope<T>
where
    T: Serialize,
{
    command: &'static str,
    data: T,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AddResponse {
    action: &'static str,
    gate_result: &'static str,
    memory: Memory,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResponse {
    scope: String,
    query: String,
    keyword_only: bool,
    include_dormant: bool,
    results: Vec<SearchResultRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResultRow {
    id: String,
    score: f32,
    similarity: f32,
    state: String,
    memory_type: String,
    preview: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListResponse {
    scope: String,
    count: usize,
    memories: Vec<ListRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListRow {
    id: String,
    state: String,
    memory_type: String,
    provenance: String,
    importance: f32,
    updated_at: String,
    preview: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InspectResponse {
    memory: Memory,
    versions: Vec<MemoryVersion>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    report: MemoryHealthReport,
    type_counts: BTreeMap<String, u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportResponse {
    exported_at: String,
    db_path: String,
    scope: String,
    memories: Vec<Memory>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PurgeResponse {
    db_path: String,
    scope: String,
    report: crate::PurgeReport,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReembedResponse {
    db_path: String,
    scope: String,
    provider: String,
    requested_limit: usize,
    stale_found: usize,
    reembedded_count: usize,
    reembedded_ids: Vec<String>,
}

pub fn run_from_env() -> Result<ExitCode, CliError> {
    run_from(std::env::args_os())
}

pub fn run_from<I, T>(args: I) -> Result<ExitCode, CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    dispatch(Cli::parse_from(args))
}

fn dispatch(cli: Cli) -> Result<ExitCode, CliError> {
    match cli.command {
        Command::Add {
            store,
            content,
            memory_type,
            importance,
            provenance,
        } => execute_add_command(
            open_store(store)?,
            content,
            memory_type.into(),
            importance,
            provenance.into(),
            cli.format,
        ),
        Command::Search {
            store,
            query,
            limit,
            include_dormant,
        } => execute_search_command(
            open_store(store)?,
            query,
            limit,
            include_dormant,
            cli.format,
        ),
        Command::List {
            store,
            memory_type,
            state,
            limit,
        } => execute_list_command(
            open_store(store)?,
            memory_type.map(Into::into),
            state.map(Into::into),
            limit,
            cli.format,
        ),
        Command::Inspect { store, id } => {
            execute_inspect_command(open_store(store)?, id, cli.format)
        }
        Command::Purge { store, yes } => execute_purge_command(open_store(store)?, yes, cli.format),
        Command::Health { store } => execute_health_command(open_store(store)?, cli.format),
        Command::Export { store, output } => {
            execute_export_command(open_store(store)?, output, cli.format)
        }
        Command::Reembed {
            store,
            limit,
        } => execute_reembed_command(open_store(store)?, limit, cli.format),
        Command::Contradictions { store } => {
            execute_contradictions_command(open_store(store)?, cli.format)
        }
    }
}

fn execute_add_command(
    ctx: StoreContext,
    content: String,
    memory_type: MemoryType,
    importance: f32,
    provenance: ProvenanceLevel,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    validate_importance(importance)?;
    let trimmed_content = content.trim().to_string();
    if trimmed_content.is_empty() {
        return Err(CliError::Validation(
            "memory content must not be empty".to_string(),
        ));
    }

    let gate = DefaultSalienceGate::new_with_optional_embedding_provider(
        ctx.store.scope_config()?,
        ctx.embedding_provider.clone(),
    );
    let candidate = MemoryCandidate {
        content: trimmed_content.clone(),
        summary: None,
        memory_type,
        provenance,
        importance_score: importance,
        sensitivity: SensitivityLevel::Low,
        tags: Vec::new(),
        custom_metadata: Default::default(),
        embedding: None,
    };

    let decision = run_async(gate.evaluate(&candidate, &ctx.store))?;
    let response = match decision {
        GateDecision::Merge {
            target_id,
            enriched_content,
        } => {
            run_async(ctx.store.update_content(
                &target_id,
                &enriched_content,
                "cli:add",
                "merged by salience gate from CLI add",
            ))?;
            let memory = run_async(ctx.store.get_raw(&target_id))?
                .ok_or_else(|| StoreError::NotFound(target_id))?;
            AddResponse {
                action: "merged",
                gate_result: "merge",
                memory,
            }
        }
        GateDecision::Archive => {
            let now = Utc::now();
            let memory = Memory {
                id: Uuid::new_v4(),
                content: trimmed_content,
                summary: None,
                scope: ctx.scope,
                memory_type,
                provenance,
                importance_score: importance,
                reliability_score: provenance.base_reliability(),
                sensitivity: SensitivityLevel::Low,
                state: MemoryState::Dormant,
                tags: Vec::new(),
                status: None,
                custom_metadata: Default::default(),
                access_count: 0,
                corroboration_count: 0,
                embedding_stale: true,
                created_at: now,
                updated_at: now,
                last_accessed_at: None,
                tenant_id: None,
                user_id: None,
                agent_id: None,
            };
            let id = memory.id;
            run_async(ctx.store.store(memory))?;
            let stored =
                run_async(ctx.store.get_raw(&id))?.ok_or_else(|| StoreError::NotFound(id))?;
            AddResponse {
                action: "added",
                gate_result: "archived",
                memory: stored,
            }
        }
        GateDecision::Accept => {
            let now = Utc::now();
            let memory = Memory {
                id: Uuid::new_v4(),
                content: trimmed_content,
                summary: None,
                scope: ctx.scope,
                memory_type,
                provenance,
                importance_score: importance,
                reliability_score: provenance.base_reliability(),
                sensitivity: SensitivityLevel::Low,
                state: MemoryState::Active,
                tags: Vec::new(),
                status: None,
                custom_metadata: Default::default(),
                access_count: 0,
                corroboration_count: 0,
                embedding_stale: true,
                created_at: now,
                updated_at: now,
                last_accessed_at: None,
                tenant_id: None,
                user_id: None,
                agent_id: None,
            };
            let id = memory.id;
            run_async(ctx.store.store(memory))?;
            let stored =
                run_async(ctx.store.get_raw(&id))?.ok_or_else(|| StoreError::NotFound(id))?;
            AddResponse {
                action: "added",
                gate_result: "accepted",
                memory: stored,
            }
        }
        GateDecision::Reject { reason } => {
            return Err(CliError::Validation(format!(
                "unexpected non-MVP reject decision from salience gate: {reason}"
            )));
        }
    };

    match format {
        OutputFormat::Text => print_add_text(&ctx, &response),
        OutputFormat::Json => print_json("add", &response)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_search_command(
    ctx: StoreContext,
    query: String,
    limit: usize,
    include_dormant: bool,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let response = build_search_response(&ctx, query, limit, include_dormant)?;

    match format {
        OutputFormat::Text => print_search_text(&response),
        OutputFormat::Json => print_json("search", &response)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_list_command(
    ctx: StoreContext,
    memory_type: Option<MemoryType>,
    state: Option<MemoryState>,
    limit: usize,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    validate_limit(limit, "limit")?;
    let memories = run_async(ctx.store.list(MemoryFilter {
        scope: Some(ctx.scope),
        state,
        memory_types: memory_type.map(|kind| vec![kind]),
        provenance_levels: None,
        tags: None,
        status: None,
        tenant_id: None,
        user_id: None,
        agent_id: None,
        limit: Some(limit),
    }))?;

    let response = ListResponse {
        scope: display_scope(ctx.scope),
        count: memories.len(),
        memories: memories.into_iter().map(list_row).collect(),
    };

    match format {
        OutputFormat::Text => print_list_text(&response),
        OutputFormat::Json => print_json("list", &response)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_inspect_command(
    ctx: StoreContext,
    raw_id: String,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let id = parse_memory_id(&raw_id)?;
    let memory = run_async(ctx.store.get_raw(&id))?.ok_or_else(|| StoreError::NotFound(id))?;
    let versions = ctx.store.list_versions(&id)?;
    let response = InspectResponse { memory, versions };

    match format {
        OutputFormat::Text => print_inspect_text(&response),
        OutputFormat::Json => print_json("inspect", &response)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_purge_command(
    ctx: StoreContext,
    yes: bool,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    if !yes && !confirm_purge(&ctx)? {
        match format {
            OutputFormat::Text => println!("Purge cancelled."),
            OutputFormat::Json => print_json(
                "purge",
                &serde_json::json!({
                    "status": "cancelled",
                    "dbPath": ctx.db_path.display().to_string(),
                    "scope": display_scope(ctx.scope),
                }),
            )?,
        }
        return Ok(ExitCode::from(1));
    }

    let response = PurgeResponse {
        db_path: ctx.db_path.display().to_string(),
        scope: display_scope(ctx.scope),
        report: run_async(ctx.store.purge_all())?,
    };

    match format {
        OutputFormat::Text => print_purge_text(&response),
        OutputFormat::Json => print_json("purge", &response)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_health_command(ctx: StoreContext, format: OutputFormat) -> Result<ExitCode, CliError> {
    let report = run_async(ctx.store.health_report())?;
    let memories = run_async(ctx.store.list(MemoryFilter {
        scope: Some(ctx.scope),
        state: None,
        memory_types: None,
        provenance_levels: None,
        tags: None,
        status: None,
        tenant_id: None,
        user_id: None,
        agent_id: None,
        limit: None,
    }))?;
    let mut type_counts = BTreeMap::new();
    for memory in memories {
        *type_counts
            .entry(display_memory_type(memory.memory_type))
            .or_insert(0) += 1;
    }
    let response = HealthResponse {
        report,
        type_counts,
    };

    match format {
        OutputFormat::Text => print_health_text(&response),
        OutputFormat::Json => print_json("health", &response)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_export_command(
    ctx: StoreContext,
    output: Option<PathBuf>,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let response = ExportResponse {
        exported_at: Utc::now().to_rfc3339(),
        db_path: ctx.db_path.display().to_string(),
        scope: display_scope(ctx.scope),
        memories: run_async(ctx.store.list(MemoryFilter {
            scope: Some(ctx.scope),
            state: None,
            memory_types: None,
            provenance_levels: None,
            tags: None,
            status: None,
            tenant_id: None,
            user_id: None,
            agent_id: None,
            limit: None,
        }))?,
    };
    let payload = serde_json::to_string_pretty(&response)?;

    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, payload)?;
        match format {
            OutputFormat::Text => {
                println!(
                    "Exported {} memories to {}",
                    response.memories.len(),
                    path.display()
                );
            }
            OutputFormat::Json => print_json(
                "export",
                &serde_json::json!({
                    "outputPath": path.display().to_string(),
                    "memoryCount": response.memories.len(),
                    "scope": response.scope,
                }),
            )?,
        }
    } else {
        println!("{payload}");
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_reembed_command(
    ctx: StoreContext,
    limit: usize,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let response = reembed_stale_memories(&ctx, limit)?;

    match format {
        OutputFormat::Text => print_reembed_text(&response),
        OutputFormat::Json => print_json("reembed", &response)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_contradictions_command(
    ctx: StoreContext,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let contradictions = run_async(
        ctx.store
            .list_contradictions(Some(ResolutionStatus::Unresolved)),
    )?;
    match format {
        OutputFormat::Text => print_contradictions_text(&ctx, &contradictions),
        OutputFormat::Json => print_json("contradictions", &contradictions)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn open_store(args: StoreArgs) -> Result<StoreContext, CliError> {
    let db_path = normalize_path(args.db.clone().unwrap_or_else(default_db_path));
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let scope: MemoryScope = args.scope.into();
    let (embedding_provider, embedding_provider_label) = resolve_embedding_provider(&args)?;
    let store = match embedding_provider.clone() {
        Some(provider) => SqliteMemoryStore::new_with_embedding_provider(&db_path, scope, provider)?,
        None => SqliteMemoryStore::new(&db_path, scope)?,
    };
    Ok(StoreContext {
        db_path,
        scope,
        store,
        embedding_provider,
        embedding_provider_label,
    })
}

fn resolve_embedding_provider(
    args: &StoreArgs,
) -> Result<(Option<Arc<dyn EmbeddingProvider>>, Option<String>), CliError> {
    match args.embedding_provider {
        Some(CliEmbeddingProvider::Ollama) => {
            let base_url = args
                .ollama_url
                .clone()
                .unwrap_or_else(|| DEFAULT_OLLAMA_BASE_URL.to_string());
            let model = args
                .ollama_model
                .clone()
                .unwrap_or_else(|| DEFAULT_OLLAMA_MODEL.to_string());
            let provider =
                Arc::new(OllamaEmbeddingProvider::new(base_url.clone(), model.clone())?)
                    as Arc<dyn EmbeddingProvider>;
            Ok((
                Some(provider),
                Some(format!("ollama ({model} @ {base_url})")),
            ))
        }
        None => {
            if args.ollama_url.is_some() || args.ollama_model.is_some() {
                return Err(CliError::Validation(
                    "Ollama configuration requires --embedding-provider ollama".to_string(),
                ));
            }
            Ok((None, None))
        }
    }
}

fn default_db_path() -> PathBuf {
    home_dir().join(".elegy").join("memory.db")
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let raw = path.to_string_lossy();
    if raw == "~" {
        return default_db_path();
    }
    if let Some(stripped) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        return home_dir().join(stripped);
    }
    path
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn run_async<F, T, E>(future: F) -> Result<T, CliError>
where
    F: std::future::Future<Output = Result<T, E>>,
    CliError: From<E>,
{
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(CliError::Io)?;
    runtime.block_on(future).map_err(CliError::from)
}

fn validate_importance(importance: f32) -> Result<(), CliError> {
    if importance.is_finite() && (0.0..=1.0).contains(&importance) {
        return Ok(());
    }
    Err(CliError::Validation(
        "--importance must be a finite value in the inclusive range 0.0..=1.0".to_string(),
    ))
}

fn validate_limit(limit: usize, flag: &str) -> Result<(), CliError> {
    if limit == 0 {
        return Err(CliError::Validation(format!(
            "--{flag} must be greater than zero"
        )));
    }
    Ok(())
}

fn build_search_response(
    ctx: &StoreContext,
    query: String,
    limit: usize,
    include_dormant: bool,
) -> Result<SearchResponse, CliError> {
    validate_limit(limit, "limit")?;
    let trimmed_query = query.trim().to_string();
    if trimmed_query.is_empty() {
        return Err(CliError::Validation(
            "search query must not be empty".to_string(),
        ));
    }

    let mut results = run_async(ctx.store.search(SearchQuery {
        text: trimmed_query.clone(),
        embedding: None,
        scope: ctx.scope,
        state_filter: Some(MemoryState::Active),
        type_filter: None,
        max_results: limit,
        context_config: None,
    }))?;

    if include_dormant {
        results.extend(run_async(ctx.store.search(SearchQuery {
            text: trimmed_query.clone(),
            embedding: None,
            scope: ctx.scope,
            state_filter: Some(MemoryState::Dormant),
            type_filter: None,
            max_results: limit,
            context_config: None,
        }))?);
        sort_scored_memories(&mut results);
        results.truncate(limit);
    }

    Ok(SearchResponse {
        scope: display_scope(ctx.scope),
        query: trimmed_query,
        keyword_only: !ctx.has_embedding_provider(),
        include_dormant,
        results: results.into_iter().map(search_result_row).collect(),
    })
}

fn reembed_stale_memories(ctx: &StoreContext, limit: usize) -> Result<ReembedResponse, CliError> {
    validate_limit(limit, "limit")?;
    let provider = ctx.embedding_provider.as_ref().ok_or_else(|| {
        CliError::Validation(
            "reembed requires an embedding provider; rerun with --embedding-provider ollama"
                .to_string(),
        )
    })?;

    run_async::<_, ReembedResponse, CliError>(async {
        let stale_ids = ctx.store.get_stale_embeddings(limit).await?;
        let mut reembedded_ids = Vec::with_capacity(stale_ids.len());
        for id in &stale_ids {
            let memory = ctx
                .store
                .get_raw(id)
                .await
                .map_err(|error| {
                    CliError::Validation(format!("failed to load memory {id}: {error}"))
                })?
                .ok_or_else(|| {
                    CliError::Validation(format!("failed to load memory {id}: not found"))
                })?;
            let embedding = provider.embed(&memory.content).await.map_err(|error| {
                CliError::Validation(format!(
                    "failed to generate embedding for memory {id}: {error}"
                ))
            })?;
            ctx.store
                .store_embedding(id, &embedding)
                .await
                .map_err(|error| {
                    CliError::Validation(format!("failed to store embedding for memory {id}: {error}"))
                })?;
            reembedded_ids.push(id.to_string());
        }

        Ok(ReembedResponse {
            db_path: ctx.db_path.display().to_string(),
            scope: display_scope(ctx.scope),
            provider: ctx.embedding_provider_label().to_string(),
            requested_limit: limit,
            stale_found: stale_ids.len(),
            reembedded_count: reembedded_ids.len(),
            reembedded_ids,
        })
    })
}

fn parse_memory_id(raw: &str) -> Result<MemoryId, CliError> {
    Uuid::parse_str(raw).map_err(|source| CliError::InvalidId {
        value: raw.to_string(),
        source,
    })
}

fn confirm_purge(ctx: &StoreContext) -> Result<bool, CliError> {
    let mut stdout = io::stdout();
    write!(
        stdout,
        "This will purge all data in {} (scope: {}). Type `purge` to confirm: ",
        ctx.db_path.display(),
        display_scope(ctx.scope)
    )?;
    stdout.flush()?;

    let mut confirmation = String::new();
    io::stdin().read_line(&mut confirmation)?;
    Ok(confirmation.trim() == "purge")
}

fn sort_scored_memories(results: &mut [ScoredMemory]) {
    results.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| right.similarity.total_cmp(&left.similarity))
            .then_with(|| right.memory.updated_at.cmp(&left.memory.updated_at))
            .then_with(|| right.memory.id.cmp(&left.memory.id))
    });
}

fn search_result_row(result: ScoredMemory) -> SearchResultRow {
    SearchResultRow {
        id: result.memory.id.to_string(),
        score: result.score,
        similarity: result.similarity,
        state: display_state(result.memory.state),
        memory_type: display_memory_type(result.memory.memory_type),
        preview: preview(&result.memory.content),
    }
}

fn list_row(memory: Memory) -> ListRow {
    ListRow {
        id: memory.id.to_string(),
        state: display_state(memory.state),
        memory_type: display_memory_type(memory.memory_type),
        provenance: display_provenance(memory.provenance),
        importance: memory.importance_score,
        updated_at: memory.updated_at.to_rfc3339(),
        preview: preview(&memory.content),
    }
}

fn preview(content: &str) -> String {
    let mut preview = String::new();
    let mut chars = content.chars().peekable();
    while preview.chars().count() < PREVIEW_LIMIT {
        let Some(character) = chars.next() else {
            break;
        };
        preview.push(character);
    }
    if chars.peek().is_some() {
        preview.push('…');
    }
    preview.replace('\n', " ")
}

fn display_scope(scope: MemoryScope) -> String {
    match scope {
        MemoryScope::Session => "session",
        MemoryScope::Workspace => "workspace",
        MemoryScope::User => "user",
        MemoryScope::Agent => "agent",
    }
    .to_string()
}

fn display_memory_type(memory_type: MemoryType) -> String {
    match memory_type {
        MemoryType::Fact => "fact",
        MemoryType::Preference => "preference",
        MemoryType::Decision => "decision",
        MemoryType::Procedure => "procedure",
        MemoryType::Observation => "observation",
    }
    .to_string()
}

fn display_state(state: MemoryState) -> String {
    match state {
        MemoryState::Active => "active",
        MemoryState::Dormant => "dormant",
        MemoryState::Deleted => "deleted",
    }
    .to_string()
}

fn display_provenance(provenance: ProvenanceLevel) -> String {
    match provenance {
        ProvenanceLevel::UserStated => "user-stated",
        ProvenanceLevel::AgentObserved => "agent-observed",
        ProvenanceLevel::Consolidated => "consolidated",
        ProvenanceLevel::Imported => "imported",
        ProvenanceLevel::AgentInferred => "agent-inferred",
    }
    .to_string()
}

fn print_add_text(ctx: &StoreContext, response: &AddResponse) {
    println!(
        "{} memory {} in {}",
        response.action,
        response.memory.id,
        ctx.db_path.display()
    );
    println!("scope: {}", display_scope(response.memory.scope));
    println!("state: {}", display_state(response.memory.state));
    println!("type: {}", display_memory_type(response.memory.memory_type));
    println!("importance: {:.2}", response.memory.importance_score);
    println!(
        "provenance: {}",
        display_provenance(response.memory.provenance)
    );
    println!("gate: {}", response.gate_result);
    println!("content: {}", response.memory.content);
}

fn print_search_text(response: &SearchResponse) {
    println!("search scope: {}", response.scope);
    println!("query: {}", response.query);
    if response.keyword_only {
        println!("mode: keyword-only FTS5");
    } else {
        println!("mode: hybrid keyword + provider-backed embedding search");
    }
    println!("include dormant: {}", response.include_dormant);
    if response.results.is_empty() {
        println!("no results");
        return;
    }
    for result in &response.results {
        println!(
            "- {} [{} | {}] score={:.3} similarity={:.3}",
            result.id, result.state, result.memory_type, result.score, result.similarity
        );
        println!("  {}", result.preview);
    }
}

fn print_reembed_text(response: &ReembedResponse) {
    println!("db: {}", response.db_path);
    println!("scope: {}", response.scope);
    println!("provider: {}", response.provider);
    println!("requested limit: {}", response.requested_limit);
    println!("stale found: {}", response.stale_found);
    println!("re-embedded: {}", response.reembedded_count);
    if response.reembedded_ids.is_empty() {
        println!("no stale memories required re-embedding");
        return;
    }
    for id in &response.reembedded_ids {
        println!("- {id}");
    }
}

fn print_list_text(response: &ListResponse) {
    println!("scope: {}", response.scope);
    println!("count: {}", response.count);
    if response.memories.is_empty() {
        println!("no memories");
        return;
    }
    for memory in &response.memories {
        println!(
            "- {} [{} | {} | {}] importance={:.2} updated={}",
            memory.id,
            memory.state,
            memory.memory_type,
            memory.provenance,
            memory.importance,
            memory.updated_at
        );
        println!("  {}", memory.preview);
    }
}

fn print_inspect_text(response: &InspectResponse) {
    let memory = &response.memory;
    println!("id: {}", memory.id);
    println!("scope: {}", display_scope(memory.scope));
    println!("state: {}", display_state(memory.state));
    println!("type: {}", display_memory_type(memory.memory_type));
    println!("provenance: {}", display_provenance(memory.provenance));
    println!("importance: {:.2}", memory.importance_score);
    println!("reliability: {:.2}", memory.reliability_score);
    println!("embedding stale: {}", memory.embedding_stale);
    println!("created: {}", memory.created_at.to_rfc3339());
    println!("updated: {}", memory.updated_at.to_rfc3339());
    println!("content:\n{}", memory.content);
    println!("version history: {}", response.versions.len());
    for version in &response.versions {
        println!(
            "- v{} at {} by {}",
            version.version_number,
            version.changed_at.to_rfc3339(),
            version.changed_by
        );
        if !version.change_reason.is_empty() {
            println!("  reason: {}", version.change_reason);
        }
        println!("  content: {}", preview(&version.content));
    }
}

fn print_purge_text(response: &PurgeResponse) {
    println!("purged database: {}", response.db_path);
    println!("scope: {}", response.scope);
    println!("memories deleted: {}", response.report.memories_deleted);
    println!("versions deleted: {}", response.report.versions_deleted);
    println!("links deleted: {}", response.report.links_deleted);
    println!(
        "contradictions deleted: {}",
        response.report.contradictions_deleted
    );
    println!("embeddings deleted: {}", response.report.embeddings_deleted);
}

fn print_health_text(response: &HealthResponse) {
    let report = &response.report;
    println!("scope: {}", display_scope(report.scope));
    println!("active: {}", report.active_count);
    println!("dormant: {}", report.dormant_count);
    println!("stale embeddings: {}", report.stale_embeddings_count);
    println!(
        "unresolved contradictions: {}",
        report.unresolved_contradictions
    );
    println!("storage bytes: {}", report.total_storage_bytes);
    println!("budget usage ratio: {:.3}", report.budget_usage_ratio);
    println!("type counts:");
    for (memory_type, count) in &response.type_counts {
        println!("- {}: {}", memory_type, count);
    }
}

fn print_contradictions_text(ctx: &StoreContext, contradictions: &[crate::ContradictionEntry]) {
    println!("db: {}", ctx.db_path.display());
    println!("scope: {}", display_scope(ctx.scope));
    println!("unresolved contradictions: {}", contradictions.len());
    if contradictions.is_empty() {
        println!("no unresolved contradictions");
        return;
    }
    for contradiction in contradictions {
        println!(
            "- {}: {} <-> {} at {}",
            contradiction.id,
            contradiction.memory_a_id,
            contradiction.memory_b_id,
            contradiction.detected_at.to_rfc3339()
        );
        println!("  {}", contradiction.description);
    }
}

fn print_json<T>(command: &'static str, data: &T) -> Result<(), CliError>
where
    T: Serialize,
{
    println!(
        "{}",
        serde_json::to_string_pretty(&JsonEnvelope { command, data })?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        env,
        fs,
        path::PathBuf,
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    use async_trait::async_trait;
    use chrono::{Duration, Utc};

    use super::{
        build_search_response, open_store, reembed_stale_memories, run_async,
        CliEmbeddingProvider, CliScope, StoreArgs, StoreContext,
    };
    use crate::{
        EmbeddingError, EmbeddingProvider, Memory, MemoryScope, MemoryState, MemoryStore,
        MemoryType, ProvenanceLevel, SqliteMemoryStore, SensitivityLevel,
        DEFAULT_OLLAMA_BASE_URL, DEFAULT_OLLAMA_MODEL,
    };

    #[derive(Debug, Clone)]
    enum StubEmbeddingResponse {
        Embedding(Vec<f32>),
        Failure(String),
    }

    #[derive(Debug)]
    struct StubEmbeddingProvider {
        responses: HashMap<String, StubEmbeddingResponse>,
        calls: Mutex<Vec<String>>,
    }

    impl StubEmbeddingProvider {
        fn new<I, S>(responses: I) -> Self
        where
            I: IntoIterator<Item = (S, StubEmbeddingResponse)>,
            S: Into<String>,
        {
            Self {
                responses: responses
                    .into_iter()
                    .map(|(text, response)| (text.into(), response))
                    .collect(),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("stub provider calls lock").clone()
        }
    }

    #[async_trait]
    impl EmbeddingProvider for StubEmbeddingProvider {
        async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
            let trimmed = text.trim().to_string();
            self.calls
                .lock()
                .expect("stub provider calls lock")
                .push(trimmed.clone());
            match self.responses.get(&trimmed) {
                Some(StubEmbeddingResponse::Embedding(embedding)) => Ok(embedding.clone()),
                Some(StubEmbeddingResponse::Failure(message)) => {
                    Err(EmbeddingError::Provider(message.clone()))
                }
                None => Err(EmbeddingError::Provider(format!(
                    "missing stub embedding for `{trimmed}`"
                ))),
            }
        }

        fn dimensions(&self) -> usize {
            768
        }

        fn model_id(&self) -> &str {
            "stub-embedding-provider"
        }
    }

    #[test]
    fn open_store_constructs_ollama_provider_with_defaults() {
        let db_path = unique_temp_path("elegy-memory-cli-open-store");
        let ctx = open_store(StoreArgs {
            db: Some(db_path.clone()),
            scope: CliScope::Workspace,
            embedding_provider: Some(CliEmbeddingProvider::Ollama),
            ollama_url: None,
            ollama_model: None,
        })
        .expect("open provider-backed store");

        assert!(ctx.has_embedding_provider());
        let label = ctx.embedding_provider_label();
        assert!(label.contains(DEFAULT_OLLAMA_BASE_URL));
        assert!(label.contains(DEFAULT_OLLAMA_MODEL));

        cleanup_temp_path(&db_path);
    }

    #[test]
    fn provider_backed_search_response_is_not_marked_keyword_only() {
        let db_path = unique_temp_path("elegy-memory-cli-search-provider");
        let provider = Arc::new(StubEmbeddingProvider::new([
            (
                "semantic launch checklist",
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
            ("semantic probe", StubEmbeddingResponse::Embedding(vec![1.0; 768])),
        ]));
        let store = SqliteMemoryStore::new_with_embedding_provider(
            &db_path,
            MemoryScope::Workspace,
            provider.clone(),
        )
        .expect("create provider-backed store");

        let memory = sample_memory("semantic launch checklist");
        let memory_id = memory.id;
        run_async(store.store(memory)).expect("store semantic memory");

        let ctx = StoreContext {
            db_path: db_path.clone(),
            scope: MemoryScope::Workspace,
            store,
            embedding_provider: Some(provider.clone()),
            embedding_provider_label: Some("stub".to_string()),
        };
        let response = build_search_response(&ctx, "semantic probe".to_string(), 5, false)
            .expect("build provider-backed search response");

        assert!(!response.keyword_only);
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].id, memory_id.to_string());
        assert_eq!(
            provider.calls(),
            vec![
                "semantic launch checklist".to_string(),
                "semantic probe".to_string(),
            ]
        );

        cleanup_temp_path(&db_path);
    }

    #[test]
    fn reembed_stale_memories_updates_embeddings_and_respects_limit() {
        let db_path = unique_temp_path("elegy-memory-cli-reembed");
        let provider = Arc::new(StubEmbeddingProvider::new([
            ("older stale memory", StubEmbeddingResponse::Embedding(vec![1.0; 768])),
            ("newer stale memory", StubEmbeddingResponse::Embedding(vec![0.5; 768])),
        ]));
        let store =
            SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");

        let older = sample_memory("older stale memory");
        let older_id = older.id;
        run_async(store.store(older)).expect("store older stale memory");

        let mut newer = sample_memory("newer stale memory");
        newer.updated_at = Utc::now() + Duration::milliseconds(1);
        let newer_id = newer.id;
        run_async(store.store(newer)).expect("store newer stale memory");

        let ctx = StoreContext {
            db_path: db_path.clone(),
            scope: MemoryScope::Workspace,
            store,
            embedding_provider: Some(provider.clone()),
            embedding_provider_label: Some("stub".to_string()),
        };
        let response = reembed_stale_memories(&ctx, 1).expect("re-embed stale memories");

        assert_eq!(response.stale_found, 1);
        assert_eq!(response.reembedded_count, 1);
        assert_eq!(response.reembedded_ids, vec![older_id.to_string()]);

        let older_memory = ctx
            .store
            .get_raw(&older_id);
        let older_memory = run_async(older_memory)
            .expect("load older memory")
            .expect("older memory exists");
        let newer_memory = ctx
            .store
            .get_raw(&newer_id);
        let newer_memory = run_async(newer_memory)
            .expect("load newer memory")
            .expect("newer memory exists");
        assert!(!older_memory.embedding_stale);
        assert!(newer_memory.embedding_stale);

        cleanup_temp_path(&db_path);
    }

    #[test]
    fn reembed_requires_provider_configuration() {
        let db_path = unique_temp_path("elegy-memory-cli-reembed-no-provider");
        let store =
            SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");
        run_async(store.store(sample_memory("stale memory"))).expect("store stale memory");

        let ctx = StoreContext {
            db_path: db_path.clone(),
            scope: MemoryScope::Workspace,
            store,
            embedding_provider: None,
            embedding_provider_label: None,
        };
        let error = reembed_stale_memories(&ctx, 5).expect_err("provider should be required");

        assert!(error
            .to_string()
            .contains("--embedding-provider ollama"));

        cleanup_temp_path(&db_path);
    }

    #[test]
    fn reembed_surfaces_provider_failures_with_memory_id() {
        let db_path = unique_temp_path("elegy-memory-cli-reembed-failure");
        let store =
            SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");
        let memory = sample_memory("failing stale memory");
        let memory_id = memory.id;
        run_async(store.store(memory)).expect("store failing memory");

        let provider = Arc::new(StubEmbeddingProvider::new([(
            "failing stale memory",
            StubEmbeddingResponse::Failure("stub embed failure".to_string()),
        )]));
        let ctx = StoreContext {
            db_path: db_path.clone(),
            scope: MemoryScope::Workspace,
            store,
            embedding_provider: Some(provider),
            embedding_provider_label: Some("stub".to_string()),
        };
        let error = reembed_stale_memories(&ctx, 5).expect_err("provider failure should surface");

        let message = error.to_string();
        assert!(message.contains(&memory_id.to_string()));
        assert!(message.contains("stub embed failure"));

        cleanup_temp_path(&db_path);
    }

    fn sample_memory(content: &str) -> Memory {
        let now = Utc::now();
        Memory {
            id: uuid::Uuid::new_v4(),
            content: content.to_string(),
            summary: None,
            scope: MemoryScope::Workspace,
            memory_type: MemoryType::Observation,
            provenance: ProvenanceLevel::UserStated,
            importance_score: 0.8,
            reliability_score: ProvenanceLevel::UserStated.base_reliability(),
            sensitivity: SensitivityLevel::Low,
            state: MemoryState::Active,
            tags: Vec::new(),
            status: None,
            custom_metadata: HashMap::new(),
            access_count: 0,
            corroboration_count: 0,
            embedding_stale: true,
            created_at: now,
            updated_at: now,
            last_accessed_at: None,
            tenant_id: None,
            user_id: None,
            agent_id: None,
        }
    }

    fn unique_temp_path(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_nanos();
        env::temp_dir().join(format!("{prefix}-{unique}.sqlite3"))
    }

    fn cleanup_temp_path(path: &PathBuf) {
        let _ = fs::remove_file(path);
    }
}
