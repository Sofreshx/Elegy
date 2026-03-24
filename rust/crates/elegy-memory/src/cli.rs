use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use chrono::Utc;
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use thiserror::Error;
use tokio::runtime::Builder;
use uuid::Uuid;

use crate::{
    DefaultSalienceGate, GateDecision, GateError, Memory, MemoryCandidate, MemoryFilter,
    MemoryHealthReport, MemoryId, MemoryScope, MemoryState, MemoryStore, MemoryType,
    MemoryVersion, ProvenanceLevel, ResolutionStatus, ScoredMemory, SearchQuery,
    SalienceGate, SensitivityLevel, SqliteMemoryStore, StoreError,
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
    /// Search with keyword-only FTS5 matching in the CLI MVP.
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
        #[arg(long)]
        provider: Option<String>,
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

#[derive(Debug)]
struct StoreContext {
    db_path: PathBuf,
    scope: MemoryScope,
    store: SqliteMemoryStore,
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
        } => execute_search_command(open_store(store)?, query, limit, include_dormant, cli.format),
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
        Command::Inspect { store, id } => execute_inspect_command(open_store(store)?, id, cli.format),
        Command::Purge { store, yes } => execute_purge_command(open_store(store)?, yes, cli.format),
        Command::Health { store } => execute_health_command(open_store(store)?, cli.format),
        Command::Export { store, output } => {
            execute_export_command(open_store(store)?, output, cli.format)
        }
        Command::Reembed {
            store,
            provider,
            limit,
        } => execute_reembed_command(open_store(store)?, provider, limit),
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

    let gate = DefaultSalienceGate::new(ctx.store.scope_config()?);
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
            let stored = run_async(ctx.store.get_raw(&id))?
                .ok_or_else(|| StoreError::NotFound(id))?;
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
            let stored = run_async(ctx.store.get_raw(&id))?
                .ok_or_else(|| StoreError::NotFound(id))?;
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

    let response = SearchResponse {
        scope: display_scope(ctx.scope),
        query: trimmed_query,
        keyword_only: true,
        include_dormant,
        results: results.into_iter().map(search_result_row).collect(),
    };

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
    let response = HealthResponse { report, type_counts };

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
                println!("Exported {} memories to {}", response.memories.len(), path.display());
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
    provider: Option<String>,
    limit: usize,
) -> Result<ExitCode, CliError> {
    validate_limit(limit, "limit")?;
    let stale = run_async(ctx.store.get_stale_embeddings(limit))?;
    let provider = provider.unwrap_or_else(|| "none".to_string());
    Err(CliError::Validation(format!(
        "reembed is not wired in the MVP CLI yet; provider `{provider}` is unavailable and {} stale memories are queued",
        stale.len()
    )))
}

fn execute_contradictions_command(
    ctx: StoreContext,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let contradictions =
        run_async(ctx.store.list_contradictions(Some(ResolutionStatus::Unresolved)))?;
    match format {
        OutputFormat::Text => print_contradictions_text(&ctx, &contradictions),
        OutputFormat::Json => print_json("contradictions", &contradictions)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn open_store(args: StoreArgs) -> Result<StoreContext, CliError> {
    let db_path = normalize_path(args.db.unwrap_or_else(default_db_path));
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let scope: MemoryScope = args.scope.into();
    let store = SqliteMemoryStore::new(&db_path, scope)?;
    Ok(StoreContext {
        db_path,
        scope,
        store,
    })
}

fn default_db_path() -> PathBuf {
    home_dir().join(".elegy").join("memory.db")
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let raw = path.to_string_lossy();
    if raw == "~" {
        return default_db_path();
    }
    if let Some(stripped) = raw
        .strip_prefix("~/")
        .or_else(|| raw.strip_prefix("~\\"))
    {
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
    println!("provenance: {}", display_provenance(response.memory.provenance));
    println!("gate: {}", response.gate_result);
    println!("content: {}", response.memory.content);
}

fn print_search_text(response: &SearchResponse) {
    println!("search scope: {}", response.scope);
    println!("query: {}", response.query);
    println!("mode: keyword-only FTS5 (no query embedding in CLI MVP)");
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
