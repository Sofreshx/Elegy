use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::{self, Read, Write},
    path::PathBuf,
    process::ExitCode,
    sync::Arc,
};

use chrono::Utc;
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::runtime::Builder;
use uuid::Uuid;

use crate::{
    ConsolidationAction, DefaultSalienceGate, EmbeddingError, EmbeddingProvider, GateDecision,
    GateError, LlmConsolidator, LlmProvider, Memory, MemoryCandidate, MemoryConsolidator,
    MemoryFilter, MemoryHealthReport, MemoryId, MemoryScope, MemoryState, MemoryStore, MemoryType,
    MemoryVersion, OllamaEmbeddingProvider, OllamaLlmProvider, OpenAiEmbeddingProvider,
    OpenAiLlmProvider, PromotionEngine, ProvenanceLevel, ResolutionStatus, SalienceGate,
    ScoredMemory, SearchQuery, SensitivityLevel, SimpleConsolidator, SqliteMemoryStore, StoreError,
    DEFAULT_OLLAMA_BASE_URL, DEFAULT_OLLAMA_LLM_BASE_URL, DEFAULT_OLLAMA_LLM_MODEL,
    DEFAULT_OLLAMA_MODEL, DEFAULT_OPENAI_BASE_URL, DEFAULT_OPENAI_DIMENSIONS,
    DEFAULT_OPENAI_LLM_BASE_URL, DEFAULT_OPENAI_LLM_MODEL, DEFAULT_OPENAI_MODEL,
};

const DEFAULT_IMPORTANCE: f32 = 0.5;
const DEFAULT_LIMIT: usize = 20;
const DEFAULT_REEMBED_LIMIT: usize = 100;
const PREVIEW_LIMIT: usize = 80;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Consolidation(#[from] crate::ConsolidationError),
    #[error("{0}")]
    Store(#[from] StoreError),
    #[error("{0}")]
    Gate(#[from] GateError),
    #[error("{0}")]
    Embedding(#[from] EmbeddingError),
    #[error("{0}")]
    Llm(#[from] crate::LlmError),
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
        #[arg(long)]
        all_scopes: bool,
    },
    /// Re-embed stale memories when a provider is configured.
    Reembed {
        #[command(flatten)]
        store: StoreArgs,
        #[arg(long, default_value_t = DEFAULT_REEMBED_LIMIT)]
        limit: usize,
    },
    /// List unresolved contradiction records or resolve one by id.
    Contradictions {
        #[command(flatten)]
        store: StoreArgs,
        /// Optional contradiction action. Omit to list unresolved contradictions.
        #[arg(value_enum)]
        action: Option<ContradictionsAction>,
        /// Contradiction id to resolve.
        #[arg(long)]
        id: Option<String>,
        /// Memory id to keep active while dormanting the other side.
        #[arg(long)]
        keep: Option<String>,
        /// Resolve without dormanting either memory.
        #[arg(long = "keep-both")]
        keep_both: bool,
    },
    /// Import memories from a JSON file (or stdin when --input is omitted).
    Import {
        #[command(flatten)]
        store: StoreArgs,
        /// Path to a JSON file to import. Reads from stdin when omitted.
        #[arg(long)]
        input: Option<PathBuf>,
        /// Bypass the salience gate and insert every item directly. Format A keeps exported states.
        #[arg(long)]
        force: bool,
    },
    /// Promote memories automatically or force a manual promotion.
    Promote {
        #[command(flatten)]
        store: StoreArgs,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, value_enum)]
        to: Option<CliScope>,
        #[arg(long, default_value_t = DEFAULT_LIMIT)]
        limit: usize,
    },
    /// Consolidate near-duplicate memories.
    Consolidate {
        #[command(flatten)]
        store: StoreArgs,
        #[arg(long)]
        cross_scope: bool,
        #[arg(long, default_value_t = 50)]
        consolidate_limit: usize,
        #[arg(long)]
        dry_run: bool,
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
    /// OpenAI API key when `--embedding-provider openai` is enabled.
    #[arg(long)]
    openai_api_key: Option<String>,
    /// OpenAI model when `--embedding-provider openai` is enabled. Defaults to
    /// text-embedding-3-small.
    #[arg(long)]
    openai_model: Option<String>,
    /// OpenAI base URL when `--embedding-provider openai` is enabled. Defaults to
    /// https://api.openai.com. Set to a custom URL for LM Studio / vLLM compatibility.
    #[arg(long)]
    openai_url: Option<String>,
    /// Expected embedding dimensions when `--embedding-provider openai` is enabled.
    /// Defaults to 1536.
    #[arg(long)]
    openai_dimensions: Option<usize>,
    /// Optional LLM provider used for contradiction checks and consolidation.
    #[arg(long = "llm-provider", value_enum)]
    llm_provider: Option<CliLlmProvider>,
    /// LLM model when `--llm-provider` is enabled. Defaults to `qwen3:8b` for Ollama and
    /// `gpt-4.1-mini` for OpenAI.
    #[arg(long = "llm-model")]
    llm_model: Option<String>,
    /// Ollama base URL when `--llm-provider ollama` is enabled.
    #[arg(long = "llm-ollama-url")]
    llm_ollama_url: Option<String>,
    /// OpenAI API key when `--llm-provider openai` is enabled.
    #[arg(long = "llm-openai-api-key")]
    llm_openai_api_key: Option<String>,
    /// OpenAI-compatible base URL when `--llm-provider openai` is enabled.
    #[arg(long = "llm-openai-url")]
    llm_openai_url: Option<String>,
    /// Optional session identifier used to track cross-session access-driven promotions.
    #[arg(long)]
    session_id: Option<String>,
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
    Openai,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum)]
enum CliLlmProvider {
    Ollama,
    Openai,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, ValueEnum)]
enum ContradictionsAction {
    Resolve,
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
    session_id: Option<String>,
    store: SqliteMemoryStore,
    embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
    embedding_provider_label: Option<String>,
    llm_provider: Option<Arc<dyn LlmProvider>>,
    llm_provider_label: Option<String>,
}

impl StoreContext {
    fn has_embedding_provider(&self) -> bool {
        self.embedding_provider.is_some()
    }

    fn embedding_provider_label(&self) -> &str {
        self.embedding_provider_label.as_deref().unwrap_or("none")
    }

    fn llm_provider_label(&self) -> &str {
        self.llm_provider_label.as_deref().unwrap_or("none")
    }
}

impl std::fmt::Debug for StoreContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreContext")
            .field("db_path", &self.db_path)
            .field("scope", &self.scope)
            .field("session_id", &self.session_id)
            .field("store", &self.store)
            .field("has_embedding_provider", &self.embedding_provider.is_some())
            .field("embedding_provider_label", &self.embedding_provider_label)
            .field("has_llm_provider", &self.llm_provider.is_some())
            .field("llm_provider_label", &self.llm_provider_label)
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
    gate_result: String,
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
    per_scope_reports: Vec<MemoryHealthReport>,
    type_counts: BTreeMap<String, u64>,
    average_importance: Option<f32>,
    oldest_memory_age_days: Option<i64>,
    database_size_human: String,
    most_accessed_memory: Option<HealthMostAccessedMemory>,
    stale_memories: Vec<HealthMemoryPreview>,
    contradiction_summaries: Vec<HealthContradictionSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthMemoryPreview {
    id: String,
    memory_type: String,
    preview: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthMostAccessedMemory {
    id: String,
    preview: String,
    access_count: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthContradictionSummary {
    id: String,
    memory_a_id: String,
    memory_b_id: String,
    summary: String,
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
struct PromoteResponse {
    db_path: String,
    requested_scope: String,
    promoted: Vec<Memory>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConsolidateResponse {
    db_path: String,
    scope: String,
    strategy: String,
    cross_scope: bool,
    dry_run: bool,
    merged_count: usize,
    merged_ids: Vec<String>,
    contradiction_count: usize,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolveContradictionResponse {
    contradiction_id: String,
    resolution_status: String,
    kept_both: bool,
    kept_memory_id: Option<String>,
    dormant_memory_id: Option<String>,
}

/// Summary returned by the `import` command.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportResponse {
    db_path: String,
    scope: String,
    total: usize,
    imported: usize,
    merged: usize,
    contradictions: usize,
    skipped: usize,
    errors: Vec<String>,
}

/// Wrapper for Format A imports: an export-shape object with a `memories` array.
#[derive(Deserialize)]
struct ImportFormatA {
    memories: Vec<Memory>,
}

enum ImportItem {
    FormatA(Box<Memory>),
    FormatB {
        content: String,
        memory_type: MemoryType,
        importance: f32,
        provenance: ProvenanceLevel,
    },
}

/// A single entry in a Format B import array when the entry is an object.
#[derive(Deserialize)]
struct ImportFormatBObject {
    content: String,
    /// Memory type — accepts prompt-shaped values like `"fact"` plus common case variations.
    #[serde(rename = "type")]
    memory_type: Option<String>,
    /// Importance score in `0.0..=1.0`. Defaults to `DEFAULT_IMPORTANCE` when absent.
    importance: Option<f32>,
    /// Provenance level — accepts CLI-style values like `"user-stated"` plus common case
    /// variations.
    provenance: Option<String>,
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
        Command::Export {
            store,
            output,
            all_scopes,
        } => execute_export_command(open_store(store)?, output, all_scopes, cli.format),
        Command::Reembed { store, limit } => {
            execute_reembed_command(open_store(store)?, limit, cli.format)
        }
        Command::Contradictions {
            store,
            action,
            id,
            keep,
            keep_both,
        } => execute_contradictions_command(
            open_store(store)?,
            action,
            id,
            keep,
            keep_both,
            cli.format,
        ),
        Command::Import {
            store,
            input,
            force,
        } => execute_import_command(open_store(store)?, input, force, cli.format),
        Command::Promote {
            store,
            id,
            to,
            limit,
        } => execute_promote_command(
            open_store(store)?,
            id,
            to.map(Into::into),
            limit,
            cli.format,
        ),
        Command::Consolidate {
            store,
            cross_scope,
            consolidate_limit,
            dry_run,
        } => execute_consolidate_command(
            open_store(store)?,
            cross_scope,
            consolidate_limit,
            dry_run,
            cli.format,
        ),
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

    let gate = DefaultSalienceGate::new_with_optional_providers(
        ctx.store.scope_config()?,
        ctx.embedding_provider.clone(),
        ctx.llm_provider.clone(),
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
            promote_to,
        } => {
            run_async(ctx.store.update_content(
                &target_id,
                &enriched_content,
                "cli:add",
                "merged by salience gate from CLI add",
            ))?;
            if let Some(promote_to) = promote_to {
                let _ = ctx.store.promote_memory_to(
                    &target_id,
                    promote_to,
                    "cli:add",
                    "scope-aware merge promotion from CLI add",
                    ctx.session_id.as_deref(),
                )?;
            }
            let memory =
                run_async(ctx.store.get_raw(&target_id))?.ok_or(StoreError::NotFound(target_id))?;
            AddResponse {
                action: "merged",
                gate_result: "merge".to_string(),
                memory,
            }
        }
        GateDecision::Contradiction {
            conflicting_id,
            description,
        } => {
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
            if let Err(error) = run_async(ctx.store.record_contradiction(
                &conflicting_id,
                &id,
                &description,
            )) {
                let _ = run_async(ctx.store.hard_delete(&id));
                return Err(error);
            }
            let stored = run_async(ctx.store.get_raw(&id))?.ok_or(StoreError::NotFound(id))?;
            AddResponse {
                action: "added",
                gate_result: format_contradiction_gate_result(conflicting_id),
                memory: stored,
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
            let stored = run_async(ctx.store.get_raw(&id))?.ok_or(StoreError::NotFound(id))?;
            AddResponse {
                action: "added",
                gate_result: "archived".to_string(),
                memory: stored,
            }
        }
        GateDecision::Accept {
            similar_to,
            similarity,
        } => {
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
            let stored = run_async(ctx.store.get_raw(&id))?.ok_or(StoreError::NotFound(id))?;
            AddResponse {
                action: "added",
                gate_result: format_gate_result(similar_to, similarity),
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
    let memory = run_async(ctx.store.get_raw(&id))?.ok_or(StoreError::NotFound(id))?;
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
    let per_scope_reports = load_per_scope_reports(&ctx)?;
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
    let contradictions = run_async(
        ctx.store
            .list_contradictions(Some(ResolutionStatus::Unresolved)),
    )?;
    let mut type_counts = BTreeMap::new();
    let mut total_importance = 0.0_f32;
    let now = Utc::now();
    for memory in &memories {
        *type_counts
            .entry(display_memory_type(memory.memory_type))
            .or_insert(0) += 1;
        total_importance += memory.importance_score;
    }
    let average_importance =
        (!memories.is_empty()).then_some(total_importance / memories.len() as f32);
    let oldest_memory_age_days = memories
        .iter()
        .map(|memory| &memory.created_at)
        .min()
        .map(|created_at| now.signed_duration_since(*created_at).num_days().max(0));
    let most_accessed_memory = memories
        .iter()
        .max_by_key(|memory| memory.access_count)
        .map(|memory| HealthMostAccessedMemory {
            id: memory.id.to_string(),
            preview: preview(&memory.content),
            access_count: memory.access_count,
        });
    let mut stale_memory_refs: Vec<_> = memories
        .iter()
        .filter(|memory| memory.embedding_stale)
        .collect();
    stale_memory_refs
        .sort_by_key(|memory| (memory.created_at.timestamp_millis(), memory.id.as_u128()));
    let stale_memories = stale_memory_refs
        .into_iter()
        .take(3)
        .map(|memory| HealthMemoryPreview {
            id: memory.id.to_string(),
            memory_type: display_memory_type(memory.memory_type),
            preview: preview(&memory.content),
        })
        .collect();
    let mut contradiction_refs: Vec<_> = contradictions.iter().collect();
    contradiction_refs.sort_by_key(|contradiction| contradiction.detected_at.timestamp_millis());
    let contradiction_summaries = contradiction_refs
        .into_iter()
        .take(3)
        .map(|contradiction| HealthContradictionSummary {
            id: contradiction.id.clone(),
            memory_a_id: contradiction.memory_a_id.to_string(),
            memory_b_id: contradiction.memory_b_id.to_string(),
            summary: preview(&contradiction.description),
        })
        .collect();
    let database_size_human = human_readable_size(report.total_storage_bytes);
    let response = HealthResponse {
        report,
        per_scope_reports,
        type_counts,
        average_importance,
        oldest_memory_age_days,
        database_size_human,
        most_accessed_memory,
        stale_memories,
        contradiction_summaries,
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
    all_scopes: bool,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let scopes = if all_scopes {
        vec![
            MemoryScope::Session,
            MemoryScope::Workspace,
            MemoryScope::User,
            MemoryScope::Agent,
        ]
    } else {
        vec![ctx.scope]
    };
    let mut memories = Vec::new();
    for scope in &scopes {
        let scoped_store = SqliteMemoryStore::new(&ctx.db_path, *scope)?;
        memories.extend(run_async(scoped_store.list(MemoryFilter {
            scope: Some(*scope),
            state: None,
            memory_types: None,
            provenance_levels: None,
            tags: None,
            status: None,
            tenant_id: None,
            user_id: None,
            agent_id: None,
            limit: None,
        }))?);
    }
    let response = ExportResponse {
        exported_at: Utc::now().to_rfc3339(),
        db_path: ctx.db_path.display().to_string(),
        scope: if all_scopes {
            "all".to_string()
        } else {
            display_scope(ctx.scope)
        },
        memories,
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

fn execute_promote_command(
    ctx: StoreContext,
    id: Option<String>,
    to: Option<MemoryScope>,
    limit: usize,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let engine = PromotionEngine;
    let promoted = match (id, to) {
        (Some(id), Some(to_scope)) => {
            let id = parse_memory_id(&id)?;
            engine
                .promote_to(
                    &ctx.store,
                    &id,
                    to_scope,
                    "cli:promote",
                    "manual CLI promote override",
                    ctx.session_id.as_deref(),
                )?
                .into_iter()
                .collect()
        }
        (None, None) => engine.run(&ctx.store, Some(limit), ctx.session_id.as_deref())?,
        _ => return Err(CliError::Validation(
            "`promote` requires both --id and --to for manual promotion, or neither for auto mode"
                .to_string(),
        )),
    };
    let response = PromoteResponse {
        db_path: ctx.db_path.display().to_string(),
        requested_scope: display_scope(ctx.scope),
        promoted,
    };
    match format {
        OutputFormat::Text => print_promote_text(&response),
        OutputFormat::Json => print_json("promote", &response)?,
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_consolidate_command(
    ctx: StoreContext,
    cross_scope: bool,
    consolidate_limit: usize,
    dry_run: bool,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    validate_limit(consolidate_limit, "consolidate-limit")?;
    let scopes = if cross_scope {
        ctx.scope.visible_scopes().to_vec()
    } else {
        vec![ctx.scope]
    };
    let candidates = ctx.store.list_consolidation_candidates(&scopes, None)?;
    let (strategy, actions) = if let Some(llm_provider) = ctx.llm_provider.clone() {
        let mut consolidator = LlmConsolidator::new(ctx.store.scope_config()?, llm_provider)
            .with_cross_scope(cross_scope)
            .with_pair_limit(Some(consolidate_limit));
        if let Some(embedding_provider) = ctx.embedding_provider.clone() {
            consolidator = consolidator.with_embedding_provider(embedding_provider);
        }
        (
            format!("llm ({})", ctx.llm_provider_label()),
            run_async(consolidator.consolidate(&candidates))?,
        )
    } else {
        (
            "simple".to_string(),
            run_async(
                SimpleConsolidator::default()
                    .with_cross_scope(cross_scope)
                    .with_pair_limit(Some(consolidate_limit))
                    .consolidate(&candidates),
            )?,
        )
    };
    let mut merged_ids = Vec::new();
    let mut contradiction_count = 0usize;
    for action in &actions {
        match action {
            ConsolidationAction::Merged { source_ids, result } => {
                merged_ids.push(result.id.to_string());
                if dry_run {
                    continue;
                }
                let current = run_async(ctx.store.get_raw(&result.id))?
                    .ok_or(StoreError::NotFound(result.id))?;
                if current.content != result.content {
                    run_async(ctx.store.update_content(
                        &result.id,
                        &result.content,
                        "cli:consolidate",
                        "consolidated duplicate memories",
                    ))?;
                }
                if current.scope != result.scope {
                    let _ = ctx.store.promote_memory_to(
                        &result.id,
                        result.scope,
                        "cli:consolidate",
                        "cross-scope consolidation promotion",
                        ctx.session_id.as_deref(),
                    )?;
                }
                for source_id in source_ids {
                    run_async(ctx.store.hard_delete(source_id))?;
                }
            }
            ConsolidationAction::Contradiction {
                memory_a_id,
                memory_b_id,
                description,
            } => {
                contradiction_count += 1;
                if dry_run {
                    continue;
                }
                run_async(
                    ctx.store
                        .record_contradiction(memory_a_id, memory_b_id, description),
                )?;
            }
            _ => {}
        }
    }
    if !dry_run {
        ctx.store.mark_consolidation_run()?;
    }
    let response = ConsolidateResponse {
        db_path: ctx.db_path.display().to_string(),
        scope: display_scope(ctx.scope),
        strategy,
        cross_scope,
        dry_run,
        merged_count: merged_ids.len(),
        merged_ids,
        contradiction_count,
    };
    match format {
        OutputFormat::Text => print_consolidate_text(&response),
        OutputFormat::Json => print_json("consolidate", &response)?,
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_contradictions_command(
    ctx: StoreContext,
    action: Option<ContradictionsAction>,
    contradiction_id: Option<String>,
    keep: Option<String>,
    keep_both: bool,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    match action {
        None => {
            if contradiction_id.is_some() || keep.is_some() || keep_both {
                return Err(CliError::Validation(
                    "resolution flags require `contradictions resolve`".to_string(),
                ));
            }
            execute_list_contradictions_command(ctx, format)
        }
        Some(ContradictionsAction::Resolve) => {
            execute_resolve_contradiction_command(ctx, contradiction_id, keep, keep_both, format)
        }
    }
}

fn execute_list_contradictions_command(
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

fn execute_resolve_contradiction_command(
    ctx: StoreContext,
    contradiction_id: Option<String>,
    keep: Option<String>,
    keep_both: bool,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let contradiction_id = contradiction_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            CliError::Validation(
                "`contradictions resolve` requires --id <contradiction_uuid>".to_string(),
            )
        })?;

    if keep.is_some() == keep_both {
        return Err(CliError::Validation(
            "`contradictions resolve` requires exactly one of --keep <memory_uuid> or --keep-both"
                .to_string(),
        ));
    }

    let contradiction = load_contradiction_by_id(&ctx, &contradiction_id)?;
    if contradiction.resolution_status != ResolutionStatus::Unresolved {
        return Err(CliError::Validation(format!(
            "contradiction {} is already {}",
            contradiction.id,
            display_resolution_status(contradiction.resolution_status)
        )));
    }

    let response = if keep_both {
        run_async(ctx.store.update_contradiction_status(
            &contradiction.id,
            ResolutionStatus::ResolvedByUser,
            Some("user resolved contradiction and kept both memories active"),
        ))?;

        ResolveContradictionResponse {
            contradiction_id: contradiction.id,
            resolution_status: display_resolution_status(ResolutionStatus::ResolvedByUser),
            kept_both: true,
            kept_memory_id: None,
            dormant_memory_id: None,
        }
    } else {
        let keep_value = keep.ok_or_else(|| {
            CliError::Validation(
                "`contradictions resolve` requires exactly one of --keep <memory_uuid> or --keep-both"
                    .to_string(),
            )
        })?;
        let keep_id = parse_memory_id(&keep_value)?;
        let dormant_id = if keep_id == contradiction.memory_a_id {
            contradiction.memory_b_id
        } else if keep_id == contradiction.memory_b_id {
            contradiction.memory_a_id
        } else {
            return Err(CliError::Validation(format!(
                "memory {keep_id} is not part of contradiction {}; expected {} or {}",
                contradiction.id, contradiction.memory_a_id, contradiction.memory_b_id
            )));
        };

        run_async(ctx.store.make_dormant(&dormant_id))?;
        if let Err(error) = run_async(ctx.store.update_contradiction_status(
            &contradiction.id,
            ResolutionStatus::ResolvedByUser,
            Some(&format!(
                "user kept memory {keep_id} active and made memory {dormant_id} dormant"
            )),
        )) {
            return match run_async(ctx.store.reactivate(&dormant_id)) {
                Ok(()) => Err(CliError::Validation(format!(
                    "failed to resolve contradiction {}: {error}; rolled back dormant transition for memory {dormant_id}",
                    contradiction.id
                ))),
                Err(rollback_error) => Err(CliError::Validation(format!(
                    "failed to resolve contradiction {}: {error}; rollback reactivation for memory {dormant_id} also failed: {rollback_error}",
                    contradiction.id
                ))),
            };
        }

        ResolveContradictionResponse {
            contradiction_id: contradiction.id,
            resolution_status: display_resolution_status(ResolutionStatus::ResolvedByUser),
            kept_both: false,
            kept_memory_id: Some(keep_id.to_string()),
            dormant_memory_id: Some(dormant_id.to_string()),
        }
    };

    match format {
        OutputFormat::Text => print_resolve_contradiction_text(&ctx, &response),
        OutputFormat::Json => print_json("contradictions.resolve", &response)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_import_command(
    ctx: StoreContext,
    input: Option<PathBuf>,
    force: bool,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    // ── 1. Read raw JSON text ────────────────────────────────────────────────
    let json_str = if let Some(ref path) = input {
        fs::read_to_string(path).map_err(|e| {
            CliError::Validation(format!(
                "failed to read import file {}: {e}",
                path.display()
            ))
        })?
    } else {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        buf
    };

    // ── 2. Parse root JSON value ─────────────────────────────────────────────
    let root: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| CliError::Validation(format!("malformed JSON: {e}")))?;

    // ── 3. Detect format and normalise into per-item results ─────────────────
    //
    // Format A — root object with a `memories` array (matches the export shape).
    // Format B — root array of bare strings or simple `{ content, type?, importance?, provenance? }` objects.
    //
    // Each element is a normalized import item or an item-level validation error.
    type ItemResult = Result<ImportItem, String>;

    let items: Vec<ItemResult> = if root.is_object() {
        let format_a: ImportFormatA = serde_json::from_value(root).map_err(|e| {
            CliError::Validation(format!(
                "invalid Format A import (expected object with `memories` array): {e}"
            ))
        })?;
        format_a
            .memories
            .into_iter()
            .map(|memory| {
                if memory.content.trim().is_empty() {
                    Err("memory has empty content".to_string())
                } else {
                    Ok(ImportItem::FormatA(Box::new(memory)))
                }
            })
            .collect()
    } else if let Some(array) = root.as_array() {
        array
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                if let Some(s) = entry.as_str() {
                    let content = s.trim().to_string();
                    if content.is_empty() {
                        return Err(format!("item {i}: empty string content"));
                    }
                    Ok(ImportItem::FormatB {
                        content,
                        memory_type: MemoryType::Observation,
                        importance: DEFAULT_IMPORTANCE,
                        provenance: ProvenanceLevel::Imported,
                    })
                } else if entry.is_object() {
                    let obj: ImportFormatBObject = serde_json::from_value(entry.clone())
                        .map_err(|e| format!("item {i}: invalid object: {e}"))?;
                    let content = obj.content.trim().to_string();
                    if content.is_empty() {
                        return Err(format!("item {i}: empty content field"));
                    }
                    if let Some(imp) = obj.importance {
                        if !imp.is_finite() || !(0.0..=1.0).contains(&imp) {
                            return Err(format!(
                                "item {i}: importance must be a finite value in 0.0..=1.0"
                            ));
                        }
                    }
                    let memory_type = parse_import_memory_type(obj.memory_type.as_deref())
                        .map_err(|error| format!("item {i}: {error}"))?;
                    let provenance = parse_import_provenance(obj.provenance.as_deref())
                        .map_err(|error| format!("item {i}: {error}"))?;
                    Ok(ImportItem::FormatB {
                        content,
                        memory_type: memory_type.unwrap_or(MemoryType::Observation),
                        importance: obj.importance.unwrap_or(DEFAULT_IMPORTANCE),
                        provenance: provenance.unwrap_or(ProvenanceLevel::Imported),
                    })
                } else {
                    Err(format!("item {i}: expected string or object, got {entry}"))
                }
            })
            .collect()
    } else {
        return Err(CliError::Validation(
            "import JSON must be an object with a `memories` field (Format A) \
             or an array of strings/objects (Format B)"
                .to_string(),
        ));
    };

    // ── 4. Process items ─────────────────────────────────────────────────────
    let total = items.len();
    let mut imported = 0usize;
    let mut merged = 0usize;
    let mut contradictions = 0usize;
    let mut skipped = 0usize;
    let mut errors: Vec<String> = Vec::new();

    if force {
        // Force path: bypass the salience gate and store every item directly as Active.
        for item in items {
            match item {
                Err(e) => {
                    skipped += 1;
                    errors.push(e);
                }
                Ok(ImportItem::FormatA(memory)) => {
                    let memory = *memory;
                    let memory = build_import_memory(
                        ctx.scope,
                        memory.content,
                        memory.memory_type,
                        memory.importance_score,
                        memory.provenance,
                        MemoryState::Active,
                    );
                    match run_async(ctx.store.store(memory)) {
                        Ok(_) => imported += 1,
                        Err(e) => {
                            skipped += 1;
                            errors.push(e.to_string());
                        }
                    }
                }
                Ok(ImportItem::FormatB {
                    content,
                    memory_type,
                    importance,
                    provenance,
                }) => {
                    let memory = build_import_memory(
                        ctx.scope,
                        content,
                        memory_type,
                        importance,
                        provenance,
                        MemoryState::Active,
                    );
                    match run_async(ctx.store.store(memory)) {
                        Ok(_) => imported += 1,
                        Err(e) => {
                            skipped += 1;
                            errors.push(e.to_string());
                        }
                    }
                }
            }
        }
    } else {
        // Normal path: restore Format A exports directly and keep Format B on the salience gate.
        let gate = DefaultSalienceGate::new_with_optional_providers(
            ctx.store.scope_config()?,
            ctx.embedding_provider.clone(),
            ctx.llm_provider.clone(),
        );
        for item in items {
            match item {
                Err(e) => {
                    skipped += 1;
                    errors.push(e);
                }
                Ok(ImportItem::FormatA(memory)) => {
                    let mut memory = *memory;
                    memory.scope = ctx.scope;
                    match run_async(ctx.store.store(memory)) {
                        Ok(_) => imported += 1,
                        Err(error) => {
                            skipped += 1;
                            errors.push(error.to_string());
                        }
                    }
                }
                Ok(ImportItem::FormatB {
                    content,
                    memory_type,
                    importance,
                    provenance,
                }) => {
                    let candidate = MemoryCandidate {
                        content: content.clone(),
                        summary: None,
                        memory_type,
                        provenance,
                        importance_score: importance,
                        sensitivity: SensitivityLevel::Low,
                        tags: Vec::new(),
                        custom_metadata: Default::default(),
                        embedding: None,
                    };
                    let decision = match run_async(gate.evaluate(&candidate, &ctx.store)) {
                        Ok(d) => d,
                        Err(e) => {
                            skipped += 1;
                            errors.push(e.to_string());
                            continue;
                        }
                    };
                    match decision {
                        GateDecision::Merge {
                            target_id,
                            enriched_content,
                            promote_to,
                        } => {
                            match run_async(ctx.store.update_content(
                                &target_id,
                                &enriched_content,
                                "cli:import",
                                "merged by salience gate from CLI import",
                            )) {
                                Ok(_) => {
                                    if let Some(promote_to) = promote_to {
                                        let _ = ctx.store.promote_memory_to(
                                            &target_id,
                                            promote_to,
                                            "cli:import",
                                            "scope-aware merge promotion from CLI import",
                                            ctx.session_id.as_deref(),
                                        )?;
                                    }
                                    merged += 1
                                }
                                Err(e) => {
                                    skipped += 1;
                                    errors.push(e.to_string());
                                }
                            }
                        }
                        GateDecision::Contradiction {
                            conflicting_id,
                            description,
                        } => {
                            let memory = build_import_memory(
                                ctx.scope,
                                content,
                                memory_type,
                                importance,
                                provenance,
                                MemoryState::Active,
                            );
                            let id = memory.id;
                            match run_async(ctx.store.store(memory)) {
                                Ok(_) => {
                                    match run_async(ctx.store.record_contradiction(
                                        &conflicting_id,
                                        &id,
                                        &description,
                                    )) {
                                        Ok(_) => {
                                            imported += 1;
                                            contradictions += 1;
                                        }
                                        Err(error) => {
                                            let _ = run_async(ctx.store.hard_delete(&id));
                                            skipped += 1;
                                            errors.push(error.to_string());
                                        }
                                    }
                                }
                                Err(error) => {
                                    skipped += 1;
                                    errors.push(error.to_string());
                                }
                            }
                        }
                        GateDecision::Archive => {
                            let memory = build_import_memory(
                                ctx.scope,
                                content,
                                memory_type,
                                importance,
                                provenance,
                                MemoryState::Dormant,
                            );
                            match run_async(ctx.store.store(memory)) {
                                Ok(_) => imported += 1,
                                Err(e) => {
                                    skipped += 1;
                                    errors.push(e.to_string());
                                }
                            }
                        }
                        GateDecision::Accept { .. } => {
                            let memory = build_import_memory(
                                ctx.scope,
                                content,
                                memory_type,
                                importance,
                                provenance,
                                MemoryState::Active,
                            );
                            match run_async(ctx.store.store(memory)) {
                                Ok(_) => imported += 1,
                                Err(e) => {
                                    skipped += 1;
                                    errors.push(e.to_string());
                                }
                            }
                        }
                        GateDecision::Reject { .. } => {
                            // Exact duplicate detected by the gate — content already present.
                            // Not stored; counted as merged since the existing record is kept.
                            merged += 1;
                        }
                    }
                }
            }
        }
    }

    // ── 5. Emit response ─────────────────────────────────────────────────────
    let response = ImportResponse {
        db_path: ctx.db_path.display().to_string(),
        scope: display_scope(ctx.scope),
        total,
        imported,
        merged,
        contradictions,
        skipped,
        errors,
    };

    match format {
        OutputFormat::Text => print_import_text(&response),
        OutputFormat::Json => print_json("import", &response)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn build_import_memory(
    scope: MemoryScope,
    content: String,
    memory_type: MemoryType,
    importance: f32,
    provenance: ProvenanceLevel,
    state: MemoryState,
) -> Memory {
    let now = Utc::now();
    Memory {
        id: Uuid::new_v4(),
        content,
        summary: None,
        scope,
        memory_type,
        provenance,
        importance_score: importance,
        reliability_score: provenance.base_reliability(),
        sensitivity: SensitivityLevel::Low,
        state,
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
    }
}

fn open_store(args: StoreArgs) -> Result<StoreContext, CliError> {
    let db_path = normalize_path(args.db.clone().unwrap_or_else(default_db_path));
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let scope: MemoryScope = args.scope.into();
    let (embedding_provider, embedding_provider_label) = resolve_embedding_provider(&args)?;
    let (llm_provider, llm_provider_label) = resolve_llm_provider(&args)?;
    let store = match embedding_provider.clone() {
        Some(provider) => {
            SqliteMemoryStore::new_with_embedding_provider(&db_path, scope, provider)?
        }
        None => SqliteMemoryStore::new(&db_path, scope)?,
    };
    Ok(StoreContext {
        db_path,
        scope,
        session_id: args.session_id,
        store,
        embedding_provider,
        embedding_provider_label,
        llm_provider,
        llm_provider_label,
    })
}

#[allow(clippy::type_complexity)]
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
            let provider = Arc::new(OllamaEmbeddingProvider::new(
                base_url.clone(),
                model.clone(),
            )?) as Arc<dyn EmbeddingProvider>;
            Ok((
                Some(provider),
                Some(format!("ollama ({model} @ {base_url})")),
            ))
        }
        Some(CliEmbeddingProvider::Openai) => {
            let base_url = args
                .openai_url
                .clone()
                .unwrap_or_else(|| DEFAULT_OPENAI_BASE_URL.to_string());
            let model = args
                .openai_model
                .clone()
                .unwrap_or_else(|| DEFAULT_OPENAI_MODEL.to_string());
            let dimensions = args.openai_dimensions.unwrap_or(DEFAULT_OPENAI_DIMENSIONS);
            let api_key = args.openai_api_key.clone().ok_or_else(|| {
                CliError::Validation(
                    "--openai-api-key is required when --embedding-provider openai is set"
                        .to_string(),
                )
            })?;
            let provider = Arc::new(OpenAiEmbeddingProvider::new_with_config(
                base_url.clone(),
                model.clone(),
                dimensions,
                api_key,
            )?) as Arc<dyn EmbeddingProvider>;
            Ok((
                Some(provider),
                Some(format!("openai ({model} @ {base_url}, {dimensions}d)")),
            ))
        }
        None => {
            if args.ollama_url.is_some() || args.ollama_model.is_some() {
                return Err(CliError::Validation(
                    "Ollama configuration requires --embedding-provider ollama".to_string(),
                ));
            }
            if args.openai_api_key.is_some()
                || args.openai_model.is_some()
                || args.openai_url.is_some()
                || args.openai_dimensions.is_some()
            {
                return Err(CliError::Validation(
                    "OpenAI configuration requires --embedding-provider openai".to_string(),
                ));
            }
            Ok((None, None))
        }
    }
}

#[allow(clippy::type_complexity)]
fn resolve_llm_provider(
    args: &StoreArgs,
) -> Result<(Option<Arc<dyn LlmProvider>>, Option<String>), CliError> {
    match args.llm_provider {
        Some(CliLlmProvider::Ollama) => {
            let base_url = args
                .llm_ollama_url
                .clone()
                .unwrap_or_else(|| DEFAULT_OLLAMA_LLM_BASE_URL.to_string());
            let model = args
                .llm_model
                .clone()
                .unwrap_or_else(|| DEFAULT_OLLAMA_LLM_MODEL.to_string());
            let provider = Arc::new(OllamaLlmProvider::new(base_url.clone(), model.clone())?)
                as Arc<dyn LlmProvider>;
            Ok((
                Some(provider),
                Some(format!("ollama ({model} @ {base_url})")),
            ))
        }
        Some(CliLlmProvider::Openai) => {
            let base_url = args
                .llm_openai_url
                .clone()
                .unwrap_or_else(|| DEFAULT_OPENAI_LLM_BASE_URL.to_string());
            let model = args
                .llm_model
                .clone()
                .unwrap_or_else(|| DEFAULT_OPENAI_LLM_MODEL.to_string());
            let api_key = args.llm_openai_api_key.clone().ok_or_else(|| {
                CliError::Validation(
                    "--llm-openai-api-key is required when --llm-provider openai is set"
                        .to_string(),
                )
            })?;
            let provider = Arc::new(OpenAiLlmProvider::new_with_config(
                base_url.clone(),
                model.clone(),
                api_key,
            )?) as Arc<dyn LlmProvider>;
            Ok((
                Some(provider),
                Some(format!("openai ({model} @ {base_url})")),
            ))
        }
        None => {
            if args.llm_model.is_some()
                || args.llm_ollama_url.is_some()
                || args.llm_openai_api_key.is_some()
                || args.llm_openai_url.is_some()
            {
                return Err(CliError::Validation(
                    "LLM configuration requires --llm-provider ollama or --llm-provider openai"
                        .to_string(),
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

fn normalize_import_enum_value(value: &str) -> String {
    let mut normalized: String = value
        .trim()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect();
    normalized.make_ascii_lowercase();
    normalized
}

fn parse_import_memory_type(value: Option<&str>) -> Result<Option<MemoryType>, String> {
    let Some(value) = value else {
        return Ok(None);
    };

    match normalize_import_enum_value(value).as_str() {
        "fact" => Ok(Some(MemoryType::Fact)),
        "preference" => Ok(Some(MemoryType::Preference)),
        "decision" => Ok(Some(MemoryType::Decision)),
        "procedure" => Ok(Some(MemoryType::Procedure)),
        "observation" => Ok(Some(MemoryType::Observation)),
        _ => Err(format!(
            "invalid type `{value}` (expected fact, preference, decision, procedure, or observation)"
        )),
    }
}

fn parse_import_provenance(value: Option<&str>) -> Result<Option<ProvenanceLevel>, String> {
    let Some(value) = value else {
        return Ok(None);
    };

    match normalize_import_enum_value(value).as_str() {
        "userstated" => Ok(Some(ProvenanceLevel::UserStated)),
        "agentobserved" => Ok(Some(ProvenanceLevel::AgentObserved)),
        "consolidated" => Ok(Some(ProvenanceLevel::Consolidated)),
        "imported" => Ok(Some(ProvenanceLevel::Imported)),
        "agentinferred" => Ok(Some(ProvenanceLevel::AgentInferred)),
        _ => Err(format!(
            "invalid provenance `{value}` (expected user-stated, agent-observed, consolidated, imported, or agent-inferred)"
        )),
    }
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
        session_id: ctx.session_id.clone(),
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
            session_id: ctx.session_id.clone(),
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
            "reembed requires an embedding provider; rerun with --embedding-provider ollama or --embedding-provider openai"
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
                    CliError::Validation(format!(
                        "failed to store embedding for memory {id}: {error}"
                    ))
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

fn load_contradiction_by_id(
    ctx: &StoreContext,
    contradiction_id: &str,
) -> Result<crate::ContradictionEntry, CliError> {
    run_async(ctx.store.list_contradictions(None))?
        .into_iter()
        .find(|contradiction| contradiction.id == contradiction_id)
        .ok_or_else(|| CliError::Validation(format!("contradiction not found: {contradiction_id}")))
}

fn load_per_scope_reports(ctx: &StoreContext) -> Result<Vec<MemoryHealthReport>, CliError> {
    let mut reports = Vec::new();
    for scope in [
        MemoryScope::Session,
        MemoryScope::Workspace,
        MemoryScope::User,
        MemoryScope::Agent,
    ] {
        let scoped_store = SqliteMemoryStore::new(&ctx.db_path, scope)?;
        reports.push(run_async(scoped_store.health_report())?);
    }
    Ok(reports)
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

fn display_resolution_status(status: ResolutionStatus) -> String {
    match status {
        ResolutionStatus::Unresolved => "unresolved",
        ResolutionStatus::ResolvedByUser => "resolved-by-user",
        ResolutionStatus::ResolvedBySystem => "resolved-by-system",
        ResolutionStatus::Dismissed => "dismissed",
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

fn format_gate_result(similar_to: Option<MemoryId>, similarity: Option<f32>) -> String {
    match (similar_to, similarity) {
        (Some(memory_id), Some(similarity)) => {
            format!("accepted (similar to {memory_id}, cosine={similarity:.3})")
        }
        (Some(memory_id), None) => format!("accepted (similar to {memory_id})"),
        _ => "accepted".to_string(),
    }
}

fn format_contradiction_gate_result(conflicting_id: MemoryId) -> String {
    format!("contradiction (conflicts with {conflicting_id})")
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
    if response.stale_memories.is_empty() {
        println!("stale memory previews: none");
    } else {
        println!("stale memory previews:");
        for memory in &response.stale_memories {
            println!(
                "- {id} [{memory_type}]: {preview}",
                id = memory.id,
                memory_type = memory.memory_type,
                preview = memory.preview
            );
        }
    }
    println!(
        "unresolved contradictions: {}",
        report.unresolved_contradictions
    );
    if response.contradiction_summaries.is_empty() {
        println!("contradiction summaries: none");
    } else {
        println!("contradiction summaries:");
        for contradiction in &response.contradiction_summaries {
            println!(
                "- {id} ({memory_a_id} <-> {memory_b_id}): {summary}",
                id = contradiction.id,
                memory_a_id = contradiction.memory_a_id,
                memory_b_id = contradiction.memory_b_id,
                summary = contradiction.summary
            );
        }
    }
    match response.average_importance {
        Some(average_importance) => println!("average importance: {average_importance:.3}"),
        None => println!("average importance: n/a"),
    }
    match response.oldest_memory_age_days {
        Some(oldest_memory_age_days) => {
            println!("oldest memory age (days): {oldest_memory_age_days}")
        }
        None => println!("oldest memory age (days): n/a"),
    }
    match &response.most_accessed_memory {
        Some(memory) => println!(
            "most accessed memory: {id} ({access_count}): {preview}",
            id = memory.id,
            access_count = memory.access_count,
            preview = memory.preview
        ),
        None => println!("most accessed memory: n/a"),
    }
    println!("storage bytes: {}", report.total_storage_bytes);
    println!("database size: {}", response.database_size_human);
    println!("budget usage ratio: {:.3}", report.budget_usage_ratio);
    println!("per-scope reports:");
    for scope_report in &response.per_scope_reports {
        println!(
            "- {}: active={} dormant={} stale_embeddings={} unresolved_contradictions={}",
            display_scope(scope_report.scope),
            scope_report.active_count,
            scope_report.dormant_count,
            scope_report.stale_embeddings_count,
            scope_report.unresolved_contradictions
        );
    }
    println!("type counts:");
    for (memory_type, count) in &response.type_counts {
        println!("- {memory_type}: {count}");
    }
}

fn human_readable_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    if unit_index == 0 {
        format!("{bytes} {}", UNITS[unit_index])
    } else {
        format!("{size:.1} {}", UNITS[unit_index])
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

fn print_resolve_contradiction_text(ctx: &StoreContext, response: &ResolveContradictionResponse) {
    println!("db: {}", ctx.db_path.display());
    println!("scope: {}", display_scope(ctx.scope));
    println!("resolved contradiction: {}", response.contradiction_id);
    println!("status: {}", response.resolution_status);
    if response.kept_both {
        println!("kept both memories active");
    } else {
        if let Some(kept_memory_id) = &response.kept_memory_id {
            println!("kept memory: {kept_memory_id}");
        }
        if let Some(dormant_memory_id) = &response.dormant_memory_id {
            println!("dormant memory: {dormant_memory_id}");
        }
    }
}

fn print_import_text(response: &ImportResponse) {
    println!("db: {}", response.db_path);
    println!("scope: {}", response.scope);
    println!("total: {}", response.total);
    println!("imported: {}", response.imported);
    println!("merged: {}", response.merged);
    println!("contradictions: {}", response.contradictions);
    println!("skipped: {}", response.skipped);
    if !response.errors.is_empty() {
        println!("errors:");
        for error in &response.errors {
            println!("  - {error}");
        }
    }
}

fn print_promote_text(response: &PromoteResponse) {
    println!("db: {}", response.db_path);
    println!("requested scope: {}", response.requested_scope);
    println!("promoted: {}", response.promoted.len());
    for memory in &response.promoted {
        println!("- {} -> {}", memory.id, display_scope(memory.scope));
    }
}

fn print_consolidate_text(response: &ConsolidateResponse) {
    println!("db: {}", response.db_path);
    println!("scope: {}", response.scope);
    println!("strategy: {}", response.strategy);
    println!("cross-scope: {}", response.cross_scope);
    println!("dry-run: {}", response.dry_run);
    println!("merged: {}", response.merged_count);
    println!("contradictions: {}", response.contradiction_count);
    for id in &response.merged_ids {
        println!("- {id}");
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
        env, fs,
        path::PathBuf,
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    use async_trait::async_trait;
    use chrono::{Duration, Utc};
    use clap::Parser;
    use uuid::Uuid;

    use super::{
        build_search_response, execute_add_command, execute_import_command,
        format_contradiction_gate_result, format_gate_result, open_store, reembed_stale_memories,
        run_async, Cli, CliEmbeddingProvider, CliLlmProvider, CliScope, Command, OutputFormat,
        StoreArgs, StoreContext,
    };
    use crate::{
        EmbeddingError, EmbeddingProvider, Memory, MemoryFilter, MemoryScope, MemoryState,
        MemoryStore, MemoryType, ProvenanceLevel, ResolutionStatus, SensitivityLevel,
        SqliteMemoryStore, DEFAULT_OLLAMA_BASE_URL, DEFAULT_OLLAMA_LLM_BASE_URL,
        DEFAULT_OLLAMA_LLM_MODEL, DEFAULT_OLLAMA_MODEL, DEFAULT_OPENAI_BASE_URL,
        DEFAULT_OPENAI_DIMENSIONS, DEFAULT_OPENAI_LLM_BASE_URL, DEFAULT_OPENAI_LLM_MODEL,
        DEFAULT_OPENAI_MODEL,
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
            openai_api_key: None,
            openai_model: None,
            openai_url: None,
            openai_dimensions: None,
            llm_provider: None,
            llm_model: None,
            llm_ollama_url: None,
            llm_openai_api_key: None,
            llm_openai_url: None,
            session_id: None,
        })
        .expect("open provider-backed store");

        assert!(ctx.has_embedding_provider());
        let label = ctx.embedding_provider_label();
        assert!(label.contains(DEFAULT_OLLAMA_BASE_URL));
        assert!(label.contains(DEFAULT_OLLAMA_MODEL));

        cleanup_temp_path(&db_path);
    }

    #[test]
    fn open_store_constructs_openai_provider_with_defaults() {
        let db_path = unique_temp_path("elegy-memory-cli-open-store-openai");
        let ctx = open_store(StoreArgs {
            db: Some(db_path.clone()),
            scope: CliScope::Workspace,
            embedding_provider: Some(CliEmbeddingProvider::Openai),
            ollama_url: None,
            ollama_model: None,
            openai_api_key: Some("sk-test-key".to_string()),
            openai_model: None,
            openai_url: None,
            openai_dimensions: None,
            llm_provider: None,
            llm_model: None,
            llm_ollama_url: None,
            llm_openai_api_key: None,
            llm_openai_url: None,
            session_id: None,
        })
        .expect("open OpenAI provider-backed store");

        assert!(ctx.has_embedding_provider());
        let label = ctx.embedding_provider_label();
        assert!(label.contains(DEFAULT_OPENAI_BASE_URL));
        assert!(label.contains(DEFAULT_OPENAI_MODEL));
        assert!(label.contains(&DEFAULT_OPENAI_DIMENSIONS.to_string()));

        cleanup_temp_path(&db_path);
    }

    #[test]
    fn stray_openai_flags_without_embedding_provider_openai_are_rejected() {
        let db_path = unique_temp_path("elegy-memory-cli-stray-openai-flags");
        let error = open_store(StoreArgs {
            db: Some(db_path.clone()),
            scope: CliScope::Workspace,
            embedding_provider: None,
            ollama_url: None,
            ollama_model: None,
            openai_api_key: Some("sk-stray-key".to_string()),
            openai_model: None,
            openai_url: None,
            openai_dimensions: None,
            llm_provider: None,
            llm_model: None,
            llm_ollama_url: None,
            llm_openai_api_key: None,
            llm_openai_url: None,
            session_id: None,
        })
        .expect_err("stray openai flags should be rejected");

        assert!(
            error.to_string().contains("--embedding-provider openai"),
            "expected rejection message, got: {error}"
        );
        cleanup_temp_path(&db_path);
    }

    #[test]
    fn cli_parses_llm_flags_for_consolidate() {
        let cli = Cli::try_parse_from([
            "elegy-memory",
            "consolidate",
            "--llm-provider",
            "openai",
            "--llm-model",
            "gpt-4.1-mini",
            "--llm-openai-api-key",
            "sk-test-key",
            "--llm-openai-url",
            "http://localhost:1234",
        ])
        .expect("cli should parse llm flags");

        match cli.command {
            Command::Consolidate { store, .. } => {
                assert_eq!(store.llm_provider, Some(CliLlmProvider::Openai));
                assert_eq!(store.llm_model.as_deref(), Some("gpt-4.1-mini"));
                assert_eq!(store.llm_openai_api_key.as_deref(), Some("sk-test-key"));
                assert_eq!(
                    store.llm_openai_url.as_deref(),
                    Some("http://localhost:1234")
                );
            }
            command => panic!("expected consolidate command, got {command:?}"),
        }
    }

    #[test]
    fn open_store_constructs_ollama_llm_provider_with_defaults() {
        let db_path = unique_temp_path("elegy-memory-cli-open-store-ollama-llm");
        let ctx = open_store(StoreArgs {
            db: Some(db_path.clone()),
            scope: CliScope::Workspace,
            embedding_provider: None,
            ollama_url: None,
            ollama_model: None,
            openai_api_key: None,
            openai_model: None,
            openai_url: None,
            openai_dimensions: None,
            llm_provider: Some(CliLlmProvider::Ollama),
            llm_model: None,
            llm_ollama_url: None,
            llm_openai_api_key: None,
            llm_openai_url: None,
            session_id: None,
        })
        .expect("open Ollama llm-backed store");

        assert_eq!(
            ctx.llm_provider_label(),
            format!("ollama ({DEFAULT_OLLAMA_LLM_MODEL} @ {DEFAULT_OLLAMA_LLM_BASE_URL})")
        );
        cleanup_temp_path(&db_path);
    }

    #[test]
    fn open_store_constructs_openai_llm_provider_with_defaults() {
        let db_path = unique_temp_path("elegy-memory-cli-open-store-openai-llm");
        let ctx = open_store(StoreArgs {
            db: Some(db_path.clone()),
            scope: CliScope::Workspace,
            embedding_provider: None,
            ollama_url: None,
            ollama_model: None,
            openai_api_key: None,
            openai_model: None,
            openai_url: None,
            openai_dimensions: None,
            llm_provider: Some(CliLlmProvider::Openai),
            llm_model: None,
            llm_ollama_url: None,
            llm_openai_api_key: Some("sk-test-key".to_string()),
            llm_openai_url: None,
            session_id: None,
        })
        .expect("open OpenAI llm-backed store");

        assert_eq!(
            ctx.llm_provider_label(),
            format!("openai ({DEFAULT_OPENAI_LLM_MODEL} @ {DEFAULT_OPENAI_LLM_BASE_URL})")
        );
        cleanup_temp_path(&db_path);
    }

    #[test]
    fn stray_llm_flags_without_llm_provider_are_rejected() {
        let db_path = unique_temp_path("elegy-memory-cli-stray-llm-flags");
        let error = open_store(StoreArgs {
            db: Some(db_path.clone()),
            scope: CliScope::Workspace,
            embedding_provider: None,
            ollama_url: None,
            ollama_model: None,
            openai_api_key: None,
            openai_model: None,
            openai_url: None,
            openai_dimensions: None,
            llm_provider: None,
            llm_model: Some("qwen3:8b".to_string()),
            llm_ollama_url: None,
            llm_openai_api_key: None,
            llm_openai_url: None,
            session_id: None,
        })
        .expect_err("stray llm flags should be rejected");

        assert!(error.to_string().contains("--llm-provider"));
        cleanup_temp_path(&db_path);
    }

    #[test]
    fn format_gate_result_surfaces_likely_duplicate_warning_details() {
        let similar_to = Uuid::nil();

        assert_eq!(
            format_gate_result(Some(similar_to), Some(0.8249)),
            format!("accepted (similar to {similar_to}, cosine=0.825)")
        );
        assert_eq!(format_gate_result(None, None), "accepted");
    }

    #[test]
    fn format_gate_result_surfaces_contradiction_details() {
        let conflicting_id = Uuid::nil();
        assert_eq!(
            format_contradiction_gate_result(conflicting_id),
            format!("contradiction (conflicts with {conflicting_id})")
        );
    }

    #[test]
    fn provider_backed_search_response_is_not_marked_keyword_only() {
        let db_path = unique_temp_path("elegy-memory-cli-search-provider");
        let provider = Arc::new(StubEmbeddingProvider::new([
            (
                "semantic launch checklist",
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
            (
                "semantic probe",
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
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
            session_id: None,
            store,
            embedding_provider: Some(provider.clone()),
            embedding_provider_label: Some("stub".to_string()),
            llm_provider: None,
            llm_provider_label: None,
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
            (
                "older stale memory",
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
            (
                "newer stale memory",
                StubEmbeddingResponse::Embedding(vec![0.5; 768]),
            ),
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
            session_id: None,
            store,
            embedding_provider: Some(provider.clone()),
            embedding_provider_label: Some("stub".to_string()),
            llm_provider: None,
            llm_provider_label: None,
        };
        let response = reembed_stale_memories(&ctx, 1).expect("re-embed stale memories");

        assert_eq!(response.stale_found, 1);
        assert_eq!(response.reembedded_count, 1);
        assert_eq!(response.reembedded_ids, vec![older_id.to_string()]);

        let older_memory = ctx.store.get_raw(&older_id);
        let older_memory = run_async(older_memory)
            .expect("load older memory")
            .expect("older memory exists");
        let newer_memory = ctx.store.get_raw(&newer_id);
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
            session_id: None,
            store,
            embedding_provider: None,
            embedding_provider_label: None,
            llm_provider: None,
            llm_provider_label: None,
        };
        let error = reembed_stale_memories(&ctx, 5).expect_err("provider should be required");

        assert!(error.to_string().contains("--embedding-provider ollama"));

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
            session_id: None,
            store,
            embedding_provider: Some(provider),
            embedding_provider_label: Some("stub".to_string()),
            llm_provider: None,
            llm_provider_label: None,
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

    #[test]
    fn import_without_force_merges_identical_content_via_stub_provider() {
        let db_path = unique_temp_path("elegy-memory-cli-import-merge");
        let content = "import deduplication test content";

        let provider = Arc::new(StubEmbeddingProvider::new([(
            content,
            StubEmbeddingResponse::Embedding(vec![1.0; 768]),
        )]));

        // Store the memory with its embedding using a provider-backed store.
        let store = SqliteMemoryStore::new_with_embedding_provider(
            &db_path,
            MemoryScope::Workspace,
            provider.clone(),
        )
        .expect("create provider-backed store");
        run_async(store.store(sample_memory(content))).expect("store original memory");
        drop(store);

        // Write a Format B JSON file with the same content.
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let json_path = env::temp_dir().join(format!("elegy-import-merge-{unique}.json"));
        fs::write(&json_path, format!("[\"{content}\"]")).expect("write import JSON");

        // Import without force — the gate should detect the duplicate and merge.
        let store = SqliteMemoryStore::new_with_embedding_provider(
            &db_path,
            MemoryScope::Workspace,
            provider.clone(),
        )
        .expect("reopen store");
        let ctx = StoreContext {
            db_path: db_path.clone(),
            scope: MemoryScope::Workspace,
            session_id: None,
            store,
            embedding_provider: Some(provider),
            embedding_provider_label: Some("stub".to_string()),
            llm_provider: None,
            llm_provider_label: None,
        };
        execute_import_command(ctx, Some(json_path.clone()), false, OutputFormat::Text)
            .expect("import should succeed");

        // Verify the memory count did not increase (content was merged, not doubled).
        let check_store =
            SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("reopen for check");
        let memories = run_async(check_store.list(MemoryFilter {
            scope: Some(MemoryScope::Workspace),
            state: None,
            memory_types: None,
            provenance_levels: None,
            tags: None,
            status: None,
            tenant_id: None,
            user_id: None,
            agent_id: None,
            limit: None,
        }))
        .expect("list memories");

        assert_eq!(
            memories.len(),
            1,
            "should still have 1 memory after merging duplicate, got {}",
            memories.len()
        );

        cleanup_temp_path(&db_path);
        let _ = fs::remove_file(&json_path);
    }

    #[test]
    fn add_records_contradiction_and_keeps_both_memories() {
        let db_path = unique_temp_path("elegy-memory-cli-add-contradiction");
        let existing_content = "Backend is C# with gRPC";
        let candidate_content = "Backend is Python with Flask";
        let provider = Arc::new(StubEmbeddingProvider::new([
            (
                existing_content,
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
            (
                candidate_content,
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
        ]));
        let store = SqliteMemoryStore::new_with_embedding_provider(
            &db_path,
            MemoryScope::Workspace,
            provider.clone(),
        )
        .expect("create provider-backed store");
        let existing_id =
            run_async(store.store(sample_memory(existing_content))).expect("store existing memory");

        let ctx = StoreContext {
            db_path: db_path.clone(),
            scope: MemoryScope::Workspace,
            session_id: None,
            store: store.clone(),
            embedding_provider: Some(provider),
            embedding_provider_label: Some("stub".to_string()),
            llm_provider: None,
            llm_provider_label: None,
        };
        execute_add_command(
            ctx,
            candidate_content.to_string(),
            MemoryType::Observation,
            0.8,
            ProvenanceLevel::UserStated,
            OutputFormat::Json,
        )
        .expect("add should succeed");

        let memories = run_async(store.list(MemoryFilter {
            scope: Some(MemoryScope::Workspace),
            state: None,
            memory_types: None,
            provenance_levels: None,
            tags: None,
            status: None,
            tenant_id: None,
            user_id: None,
            agent_id: None,
            limit: None,
        }))
        .expect("list memories");
        assert_eq!(memories.len(), 2);
        assert!(memories
            .iter()
            .all(|memory| memory.state == MemoryState::Active));
        assert!(memories
            .iter()
            .any(|memory| memory.content == candidate_content && memory.id != existing_id));

        let contradictions =
            run_async(store.list_contradictions(Some(ResolutionStatus::Unresolved)))
                .expect("list contradictions");
        assert_eq!(contradictions.len(), 1);
        assert_eq!(contradictions[0].memory_a_id, existing_id);
        assert!(contradictions[0].description.contains("python"));

        cleanup_temp_path(&db_path);
    }

    #[test]
    fn import_without_force_records_contradiction_and_keeps_both_memories() {
        let db_path = unique_temp_path("elegy-memory-cli-import-contradiction");
        let existing_content = "Cap RTSS 120fps";
        let candidate_content = "Cap RTSS 60fps";
        let provider = Arc::new(StubEmbeddingProvider::new([
            (
                existing_content,
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
            (
                candidate_content,
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
        ]));
        let store = SqliteMemoryStore::new_with_embedding_provider(
            &db_path,
            MemoryScope::Workspace,
            provider.clone(),
        )
        .expect("create provider-backed store");
        let existing_id =
            run_async(store.store(sample_memory(existing_content))).expect("store existing memory");

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let json_path = env::temp_dir().join(format!("elegy-import-contradiction-{unique}.json"));
        fs::write(&json_path, format!("[\"{candidate_content}\"]")).expect("write import JSON");

        let ctx = StoreContext {
            db_path: db_path.clone(),
            scope: MemoryScope::Workspace,
            session_id: None,
            store: store.clone(),
            embedding_provider: Some(provider),
            embedding_provider_label: Some("stub".to_string()),
            llm_provider: None,
            llm_provider_label: None,
        };
        execute_import_command(ctx, Some(json_path.clone()), false, OutputFormat::Json)
            .expect("import should succeed");

        let memories = run_async(store.list(MemoryFilter {
            scope: Some(MemoryScope::Workspace),
            state: None,
            memory_types: None,
            provenance_levels: None,
            tags: None,
            status: None,
            tenant_id: None,
            user_id: None,
            agent_id: None,
            limit: None,
        }))
        .expect("list memories");
        assert_eq!(memories.len(), 2);
        assert!(memories.iter().any(
            |memory| memory.content == candidate_content && memory.state == MemoryState::Active
        ));

        let contradictions =
            run_async(store.list_contradictions(Some(ResolutionStatus::Unresolved)))
                .expect("list contradictions");
        assert_eq!(contradictions.len(), 1);
        assert_eq!(contradictions[0].memory_a_id, existing_id);
        assert!(contradictions[0].description.contains("60fps"));

        cleanup_temp_path(&db_path);
        let _ = fs::remove_file(&json_path);
    }
}
