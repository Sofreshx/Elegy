use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::{self, Read, Write},
    path::PathBuf,
    process::ExitCode,
    sync::{Arc, OnceLock},
};

use chrono::Utc;
use clap::{Args, Parser, Subcommand, ValueEnum};
use elegy_core::{
    build_cli_failure_envelope, build_cli_machine_context, build_cli_success_envelope,
    CliFailureKind, CliMachineContext, CliMachineEnvelope,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::runtime::Builder;
use uuid::Uuid;

use crate::{
    embedding::{prepare_embedding_input, EmbeddingTask},
    storage::{LearnedWeightValues, LearnedWeightsReport, Migration, ReembedMigration},
    ConsolidationAction, CorrectionDisposition, CorrectionRecord, DefaultSalienceGate,
    EmbeddingError, EmbeddingProvider, ExportFormat, GateDecision, GateError, LlmConsolidator,
    LlmProvider, Memory, MemoryCandidate, MemoryConsolidator, MemoryFilter, MemoryHealthReport,
    MemoryId, MemoryObservability, MemoryScope, MemoryState, MemoryStore, MemoryType,
    MemoryVersion, OllamaEmbeddingProvider, OllamaLlmProvider, OpenAiEmbeddingProvider,
    OpenAiLlmProvider, PromotionEngine, ProvenanceLevel, ResolutionStatus, SalienceGate,
    ScoredMemory, SearchQuery, SensitivityLevel, ShareConfig, SimpleConsolidator,
    SqliteMemoryStore, StoreError, DEFAULT_OLLAMA_BASE_URL, DEFAULT_OLLAMA_LLM_BASE_URL,
    DEFAULT_OLLAMA_LLM_MODEL, DEFAULT_OLLAMA_MODEL, DEFAULT_OPENAI_BASE_URL,
    DEFAULT_OPENAI_DIMENSIONS, DEFAULT_OPENAI_LLM_BASE_URL, DEFAULT_OPENAI_LLM_MODEL,
    DEFAULT_OPENAI_MODEL,
};

const DEFAULT_IMPORTANCE: f32 = 0.5;
const DEFAULT_LIMIT: usize = 20;
const DEFAULT_REEMBED_LIMIT: usize = 100;
const PREVIEW_LIMIT: usize = 80;

static CLI_MACHINE_CONTEXT: OnceLock<MachineContext> = OnceLock::new();

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

impl From<rusqlite::Error> for CliError {
    fn from(e: rusqlite::Error) -> Self {
        CliError::Store(StoreError::from(e))
    }
}

#[derive(Parser, Debug)]
#[command(name = "elegy-memory")]
#[command(about = "MVP CLI for the Elegy memory store")]
struct Cli {
    #[arg(long, value_enum, global = true, default_value_t = OutputFormat::Text)]
    format: OutputFormat,
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    non_interactive: bool,
    #[arg(long, global = true)]
    correlation_id: Option<String>,
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
        #[arg(long)]
        include_dormant: bool,
        #[arg(long, default_value_t = DEFAULT_LIMIT)]
        limit: usize,
    },
    /// Inspect a single memory and show its version and correction history.
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
    /// Export memories in JSON, SQLite, or .elegy format.
    Export {
        #[command(flatten)]
        store: StoreArgs,
        /// Output file path. Writes to stdout when omitted (JSON only).
        #[arg(long)]
        output: Option<PathBuf>,
        /// Export all scopes instead of just the active scope.
        #[arg(long)]
        all_scopes: bool,
        /// Export format.
        #[arg(long, value_enum, default_value_t = CliExportFormat::Json)]
        export_format: CliExportFormat,
    },
    /// Re-embed stale memories when a provider is configured.
    Reembed {
        #[command(flatten)]
        store: StoreArgs,
        #[arg(long)]
        limit: Option<usize>,
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
    /// Rollback a memory to a previous version.
    Rollback {
        #[command(flatten)]
        store: StoreArgs,
        /// Memory ID to rollback.
        id: String,
        /// Version number to restore.
        #[arg(long)]
        version: i64,
    },
    /// Corroborate a memory with another, boosting its reliability.
    Corroborate {
        #[command(flatten)]
        store: StoreArgs,
        /// ID of the memory to corroborate (receives the reliability boost).
        id: String,
        /// ID of the corroborating memory.
        #[arg(long)]
        with: String,
    },
    /// Enforce storage budget: dormant low-scoring active memories and purge excess dormant ones.
    Budget {
        #[command(flatten)]
        store: StoreArgs,
    },
    /// Apply a user correction to a memory.
    Correct {
        #[command(flatten)]
        store: StoreArgs,
        /// Memory ID to correct.
        id: String,
        /// New corrected content.
        content: String,
        /// Who is making the correction.
        #[arg(long, default_value = "cli-user")]
        by: String,
        /// Optional reason for the correction.
        #[arg(long)]
        reason: Option<String>,
    },
    /// Record relevance feedback for a memory returned by search.
    Feedback {
        #[command(flatten)]
        store: StoreArgs,
        /// Memory ID the feedback is about.
        id: String,
        /// The query that returned this memory.
        #[arg(long)]
        query: String,
        /// Whether the memory was relevant to the query.
        #[arg(long)]
        relevant: bool,
    },
    /// Show learned scoring weights based on accumulated feedback.
    Weights {
        #[command(flatten)]
        store: StoreArgs,
    },
    /// Traverse the memory link graph starting from a given memory.
    Traverse {
        #[command(flatten)]
        store: StoreArgs,
        /// Starting memory ID.
        id: String,
        /// Maximum depth to traverse.
        #[arg(long, default_value_t = 3)]
        depth: u32,
        /// Optional relation type filter.
        #[arg(long)]
        relation: Option<String>,
    },
    /// Run poisoning detection heuristics on the memory store.
    DetectPoisoning {
        #[command(flatten)]
        store: StoreArgs,
        /// Make implicated low-trust active memories dormant after detection.
        #[arg(long = "quarantine", visible_alias = "remediate")]
        quarantine: bool,
    },
    /// Delete a link between two memories by link ID.
    DeleteLink {
        #[command(flatten)]
        store: StoreArgs,
        /// Link ID to delete.
        id: String,
    },
    /// Export memories for sharing with other agents, filtered by sensitivity and reliability.
    ShareExport {
        #[command(flatten)]
        store: StoreArgs,
        /// Output file path. Writes to stdout when omitted.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Maximum sensitivity level to include (default: medium).
        #[arg(long, value_enum, default_value_t = CliSensitivityLevel::Medium)]
        max_sensitivity: CliSensitivityLevel,
        /// Minimum reliability score to include (default: 0.5).
        #[arg(long, default_value_t = 0.5)]
        min_reliability: f32,
    },
    /// Import shared memories from a JSON file or stdin.
    ShareImport {
        #[command(flatten)]
        store: StoreArgs,
        /// Input file path. Reads from stdin when omitted.
        #[arg(long)]
        input: Option<PathBuf>,
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

#[derive(Clone, Debug)]
struct MachineContext {
    format: OutputFormat,
    machine: CliMachineContext,
    command: &'static str,
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
enum CliExportFormat {
    /// JSON export (default).
    #[default]
    Json,
    /// SQLite portable database file.
    Sqlite,
    /// Portable .elegy archive (JSON envelope with links and versions).
    Elegy,
}

impl From<CliExportFormat> for ExportFormat {
    fn from(value: CliExportFormat) -> Self {
        match value {
            CliExportFormat::Json => ExportFormat::Json,
            CliExportFormat::Sqlite => ExportFormat::Sqlite,
            CliExportFormat::Elegy => ExportFormat::Elegy,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
enum CliSensitivityLevel {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

impl From<CliSensitivityLevel> for SensitivityLevel {
    fn from(value: CliSensitivityLevel) -> Self {
        match value {
            CliSensitivityLevel::Low => SensitivityLevel::Low,
            CliSensitivityLevel::Medium => SensitivityLevel::Medium,
            CliSensitivityLevel::High => SensitivityLevel::High,
            CliSensitivityLevel::Critical => SensitivityLevel::Critical,
        }
    }
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

    #[allow(dead_code)]
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
    corrections: Vec<CorrectionHistoryRow>,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RollbackResponse {
    memory_id: String,
    restored_version: i64,
    content_preview: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CorroborateResponse {
    memory_id: String,
    corroborating_id: String,
    new_reliability: f32,
    corroboration_count: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BudgetResponse {
    dormanted: u64,
    deleted: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CorrectResponse {
    correction: CorrectionHistoryRow,
    corrected_memory_state: String,
    new_reliability: f32,
    embedding_stale: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CorrectionHistoryRow {
    id: String,
    memory_id: String,
    corrected_at: String,
    corrected_by: String,
    reason: String,
    disposition: String,
    previous_content: String,
    corrected_content: String,
    outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    related_memory_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    related_memory_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    related_memory_preview: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FeedbackResponse {
    feedback_id: String,
    memory_id: String,
    was_relevant: bool,
    learning: WeightsSummaryResponse,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WeightsResponse {
    learning: WeightsSummaryResponse,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WeightsSummaryResponse {
    strategy: String,
    status_detail: String,
    sample_size: usize,
    relevant_samples: usize,
    irrelevant_samples: usize,
    learning_confidence: f64,
    effective_weights: WeightValuesResponse,
    default_weights: WeightValuesResponse,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WeightValuesResponse {
    similarity_weight: f64,
    recency_weight: f64,
    access_weight: f64,
    priority_weight: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TraverseResponse {
    start_id: String,
    max_depth: u32,
    node_count: usize,
    nodes: Vec<TraverseNodeRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TraverseNodeRow {
    id: String,
    depth: u32,
    content_preview: String,
    memory_type: String,
    link_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PoisoningResponse {
    alert_count: usize,
    alerts: Vec<PoisoningAlertRow>,
    remediation: Option<PoisoningRemediationRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PoisoningAlertRow {
    id: String,
    detected_at: String,
    alert_type: String,
    severity: f32,
    description: String,
    affected_count: usize,
    memory_ids: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PoisoningRemediationRow {
    quarantined_count: usize,
    quarantined_ids: Vec<String>,
    skipped_count: usize,
    skipped_ids: Vec<String>,
    actions: Vec<PoisoningRemediationActionRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PoisoningRemediationActionRow {
    memory_id: String,
    action: String,
    reason: String,
    alert_ids: Vec<String>,
    alert_types: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteLinkResponse {
    link_id: String,
    deleted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShareExportResponse {
    exported_count: usize,
    max_sensitivity: String,
    min_reliability: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShareImportResponse {
    imported_count: usize,
    review_count: usize,
    quarantined_count: usize,
    skipped_count: usize,
    new_ids: Vec<String>,
    review_ids: Vec<String>,
    quarantined_ids: Vec<String>,
    skipped_reasons: Vec<String>,
    outcomes: Vec<ShareImportOutcomeRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShareImportOutcomeRow {
    memory_id: Option<String>,
    disposition: String,
    reason: String,
    related_memory_id: Option<String>,
}

pub fn run_from_env() -> Result<ExitCode, CliError> {
    run_from(std::env::args_os())
}

pub fn run_from<I, T>(args: I) -> Result<ExitCode, CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    let format = resolve_output_format(cli.json, cli.format);
    let machine = build_cli_machine_context(
        cli.non_interactive,
        cli.correlation_id.clone(),
        "elegy-memory",
    );
    let context = MachineContext {
        format,
        machine,
        command: command_name(&cli.command),
    };
    let _ = CLI_MACHINE_CONTEXT.set(context.clone());
    dispatch(cli)
}

fn dispatch(cli: Cli) -> Result<ExitCode, CliError> {
    let context = current_machine_context();
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
            context.format,
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
            context.format,
        ),
        Command::List {
            store,
            memory_type,
            state,
            include_dormant,
            limit,
        } => execute_list_command(
            open_store(store)?,
            memory_type.map(Into::into),
            state.map(Into::into),
            include_dormant,
            limit,
            context.format,
        ),
        Command::Inspect { store, id } => {
            execute_inspect_command(open_store(store)?, id, context.format)
        }
        Command::Purge { store, yes } => {
            execute_purge_command(open_store(store)?, yes, context.format)
        }
        Command::Health { store } => execute_health_command(open_store(store)?, context.format),
        Command::Export {
            store,
            output,
            all_scopes,
            export_format,
        } => execute_export_command(
            open_store(store)?,
            output,
            all_scopes,
            export_format.into(),
            context.format,
        ),
        Command::Reembed { store, limit } => {
            execute_reembed_command(open_store(store)?, limit, context.format)
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
            context.format,
        ),
        Command::Import {
            store,
            input,
            force,
        } => execute_import_command(open_store(store)?, input, force, context.format),
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
            context.format,
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
            context.format,
        ),
        Command::Rollback { store, id, version } => {
            execute_rollback_command(open_store(store)?, id, version, context.format)
        }
        Command::Corroborate { store, id, with } => {
            execute_corroborate_command(open_store(store)?, id, with, context.format)
        }
        Command::Budget { store } => execute_budget_command(open_store(store)?, context.format),
        Command::Correct {
            store,
            id,
            content,
            by,
            reason,
        } => execute_correct_command(open_store(store)?, id, content, by, reason, context.format),
        Command::Feedback {
            store,
            id,
            query,
            relevant,
        } => execute_feedback_command(open_store(store)?, id, query, relevant, context.format),
        Command::Weights { store } => execute_weights_command(open_store(store)?, context.format),
        Command::Traverse {
            store,
            id,
            depth,
            relation,
        } => execute_traverse_command(open_store(store)?, id, depth, relation, context.format),
        Command::DetectPoisoning { store, quarantine } => {
            execute_detect_poisoning_command(open_store(store)?, quarantine, context.format)
        }
        Command::DeleteLink { store, id } => {
            execute_delete_link_command(open_store(store)?, id, context.format)
        }
        Command::ShareExport {
            store,
            output,
            max_sensitivity,
            min_reliability,
        } => execute_share_export_command(
            open_store(store)?,
            output,
            max_sensitivity.into(),
            min_reliability,
            context.format,
        ),
        Command::ShareImport { store, input } => {
            execute_share_import_command(open_store(store)?, input, context.format)
        }
    }
}

fn resolve_output_format(json: bool, format: OutputFormat) -> OutputFormat {
    if json {
        OutputFormat::Json
    } else {
        format
    }
}

fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Add { .. } => "add",
        Command::Search { .. } => "search",
        Command::List { .. } => "list",
        Command::Inspect { .. } => "inspect",
        Command::Purge { .. } => "purge",
        Command::Health { .. } => "health",
        Command::Export { .. } => "export",
        Command::Reembed { .. } => "reembed",
        Command::Contradictions { action, .. } => match action {
            Some(ContradictionsAction::Resolve) => "contradictions.resolve",
            None => "contradictions",
        },
        Command::Import { .. } => "import",
        Command::Promote { .. } => "promote",
        Command::Consolidate { .. } => "consolidate",
        Command::Rollback { .. } => "rollback",
        Command::Corroborate { .. } => "corroborate",
        Command::Budget { .. } => "budget",
        Command::Correct { .. } => "correct",
        Command::Feedback { .. } => "feedback",
        Command::Weights { .. } => "weights",
        Command::Traverse { .. } => "traverse",
        Command::DetectPoisoning { .. } => "detect-poisoning",
        Command::DeleteLink { .. } => "delete-link",
        Command::ShareExport { .. } => "share-export",
        Command::ShareImport { .. } => "share-import",
    }
}

fn current_machine_context() -> &'static MachineContext {
    // SAFETY: CLI_MACHINE_CONTEXT is always set in run_from_env() before any
    // dispatch path that calls this function. All call sites are private and
    // only reachable after initialization.
    CLI_MACHINE_CONTEXT
        .get()
        .expect("memory CLI machine context should be initialized during run")
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
        OutputFormat::Json => print_success_json("add", &response)?,
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
        OutputFormat::Json => print_success_json("search", &response)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_list_command(
    ctx: StoreContext,
    memory_type: Option<MemoryType>,
    state: Option<MemoryState>,
    include_dormant: bool,
    limit: usize,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    validate_limit(limit, "limit")?;
    // Resolve effective state filter:
    // - explicit --state takes priority
    // - --include-dormant with no --state: no state filter (returns active + dormant)
    // - default: active only
    let effective_state = if state.is_some() {
        state
    } else if include_dormant {
        None
    } else {
        Some(MemoryState::Active)
    };
    let memories = run_async(ctx.store.list(MemoryFilter {
        scope: Some(ctx.scope),
        state: effective_state,
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
        OutputFormat::Json => print_success_json("list", &response)?,
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
    let corrections =
        build_correction_history_rows(&ctx, ctx.store.list_corrections(Some(&id), usize::MAX)?)?;
    let response = InspectResponse {
        memory,
        versions,
        corrections,
    };

    match format {
        OutputFormat::Text => print_inspect_text(&response),
        OutputFormat::Json => print_success_json("inspect", &response)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_purge_command(
    ctx: StoreContext,
    yes: bool,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    if !yes && current_machine_context().machine.non_interactive {
        return Err(CliError::Validation(
            "purge requires --yes when --non-interactive is set".to_string(),
        ));
    }

    if !yes && !confirm_purge(&ctx)? {
        match format {
            OutputFormat::Text => println!("Purge cancelled."),
            OutputFormat::Json => print_success_json(
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
        OutputFormat::Json => print_success_json("purge", &response)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_health_command(ctx: StoreContext, format: OutputFormat) -> Result<ExitCode, CliError> {
    let report = run_async(MemoryStore::health_report(&ctx.store))?;
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
    let contradictions = run_async(MemoryStore::list_contradictions(
        &ctx.store,
        Some(ResolutionStatus::Unresolved),
    ))?;
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
        OutputFormat::Json => print_success_json("health", &response)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_export_command(
    ctx: StoreContext,
    output: Option<PathBuf>,
    all_scopes: bool,
    export_format: ExportFormat,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    match export_format {
        ExportFormat::Json => {
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
                    OutputFormat::Json => print_success_json(
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
        ExportFormat::Sqlite | ExportFormat::Elegy => {
            if all_scopes {
                return Err(CliError::Validation(
                    "SQLite and .elegy exports operate on a single scope; remove --all-scopes"
                        .to_string(),
                ));
            }
            let output_path = output.ok_or_else(|| {
                CliError::Validation(
                    "--output is required for SQLite and .elegy export formats".to_string(),
                )
            })?;
            let store = SqliteMemoryStore::new(&ctx.db_path, ctx.scope)?;
            let bytes = store
                .export_memories(ctx.scope, export_format)
                .map_err(|error| CliError::Validation(format!("export failed: {error}")))?;
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&output_path, &bytes)?;
            match format {
                OutputFormat::Text => {
                    println!(
                        "Exported {} bytes to {}",
                        bytes.len(),
                        output_path.display()
                    );
                }
                OutputFormat::Json => print_success_json(
                    "export",
                    &serde_json::json!({
                        "outputPath": output_path.display().to_string(),
                        "byteCount": bytes.len(),
                        "scope": display_scope(ctx.scope),
                        "format": format!("{export_format:?}"),
                    }),
                )?,
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn execute_reembed_command(
    ctx: StoreContext,
    limit: Option<usize>,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let effective_limit = limit.unwrap_or(DEFAULT_REEMBED_LIMIT);
    if limit.is_some() {
        eprintln!(
            "warning: --limit ignored; reembed on the migration path is all-or-nothing \
             (all stale memories are re-embedded regardless of limit)"
        );
    }
    let response = reembed_stale_memories(&ctx, effective_limit)?;

    match format {
        OutputFormat::Text => print_reembed_text(&response),
        OutputFormat::Json => print_success_json("reembed", &response)?,
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
        OutputFormat::Json => print_success_json("promote", &response)?,
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
                    run_async(ctx.store.make_dormant(source_id))?;
                    let _ = ctx.store.record_link(&result.id, source_id, "supersedes");
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
        OutputFormat::Json => print_success_json("consolidate", &response)?,
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
    let contradictions = run_async(MemoryStore::list_contradictions(
        &ctx.store,
        Some(ResolutionStatus::Unresolved),
    ))?;
    match format {
        OutputFormat::Text => print_contradictions_text(&ctx, &contradictions),
        OutputFormat::Json => print_success_json("contradictions", &contradictions)?,
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

        let _ = ctx.store.record_link(&keep_id, &dormant_id, "supersedes");

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
        OutputFormat::Json => print_success_json("contradictions.resolve", &response)?,
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
        OutputFormat::Json => print_success_json("import", &response)?,
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
        agent_id: None,
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
            agent_id: None,
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
    let provider = Arc::clone(provider);

    let scope = ctx.scope;
    let scope_db = scope_to_db_cli(scope);
    let provider_label = ctx.embedding_provider_label.clone().unwrap_or_default();

    let mut connection = Connection::open(&ctx.db_path)?;
    let stale_before: usize = connection.query_row(
        "SELECT COUNT(*) FROM memories WHERE scope = ?1 AND state = 'active' AND embedding_stale = 1",
        [scope_db],
        |row| row.get::<_, i64>(0),
    )?.try_into().unwrap_or(0);

    if stale_before == 0 {
        return Ok(ReembedResponse {
            db_path: ctx.db_path.display().to_string(),
            scope: display_scope(scope),
            provider: provider_label,
            requested_limit: limit,
            stale_found: 0,
            reembedded_count: 0,
            reembedded_ids: Vec::new(),
        });
    }

    let generator = {
        let provider = Arc::clone(&provider);
        move |content: &str| -> Result<(Vec<f32>, usize), StoreError> {
            let prepared =
                prepare_embedding_input(provider.as_ref(), EmbeddingTask::Document, content);
            let rt = Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| StoreError::Migration(format!("failed to build runtime: {e}")))?;
            rt.block_on(async {
                provider
                    .embed(prepared.as_ref())
                    .await
                    .map_err(|e| StoreError::Migration(format!("embedding generation failed: {e}")))
            })
            .map(|v| {
                let dims = v.len();
                (v, dims)
            })
        }
    };

    let profile_id = format!("cli-reembed-{}", Utc::now().timestamp_millis());
    let migration = ReembedMigration::new(Box::new(generator), profile_id).with_scope(scope);

    let txn = connection.transaction()?;
    migration.run(&txn)?;
    migration.verify(&txn)?;
    txn.commit()?;

    let stale_after: usize = connection.query_row(
        "SELECT COUNT(*) FROM memories WHERE scope = ?1 AND state = 'active' AND embedding_stale = 1",
        [scope_db],
        |row| row.get::<_, i64>(0),
    )?.try_into().unwrap_or(0);

    let reembedded_count = stale_before.saturating_sub(stale_after);
    let reembedded_ids: Vec<String> = if reembedded_count > 0 {
        let mut stmt = connection.prepare(
            "SELECT id FROM memories WHERE scope = ?1 AND state = 'active' AND embedding_stale = 0 ORDER BY updated_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![scope_db, reembedded_count as i64],
            |row| row.get::<_, String>(0),
        )?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }
        ids
    } else {
        Vec::new()
    };

    Ok(ReembedResponse {
        db_path: ctx.db_path.display().to_string(),
        scope: display_scope(scope),
        provider: provider_label,
        requested_limit: limit,
        stale_found: stale_before,
        reembedded_count,
        reembedded_ids,
    })
}

fn scope_to_db_cli(scope: MemoryScope) -> &'static str {
    match scope {
        MemoryScope::Session => "session",
        MemoryScope::Workspace => "workspace",
        MemoryScope::User => "user",
        MemoryScope::Agent => "agent",
    }
}

#[allow(dead_code)]
async fn generate_document_embedding(
    provider: &dyn EmbeddingProvider,
    content: &str,
) -> Result<Vec<f32>, EmbeddingError> {
    let prepared_input = prepare_embedding_input(provider, EmbeddingTask::Document, content);
    provider.embed(prepared_input.as_ref()).await
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
    run_async(MemoryStore::list_contradictions(&ctx.store, None))?
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
        reports.push(run_async(MemoryStore::health_report(&scoped_store))?);
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

fn build_correction_history_rows(
    ctx: &StoreContext,
    corrections: Vec<CorrectionRecord>,
) -> Result<Vec<CorrectionHistoryRow>, CliError> {
    corrections
        .into_iter()
        .map(|correction| correction_history_row(ctx, correction))
        .collect()
}

fn correction_history_row(
    ctx: &StoreContext,
    correction: CorrectionRecord,
) -> Result<CorrectionHistoryRow, CliError> {
    let related_memory = correction
        .related_memory_id
        .map(|related_id| {
            run_async(ctx.store.get_raw(&related_id)).map(|memory| (related_id, memory))
        })
        .transpose()?;

    let (related_memory_id, related_memory_state, related_memory_preview) = match related_memory {
        Some((related_id, Some(memory))) => (
            Some(related_id.to_string()),
            Some(display_state(memory.state)),
            Some(preview(&memory.content)),
        ),
        Some((related_id, None)) => (Some(related_id.to_string()), None, None),
        None => (None, None, None),
    };

    Ok(CorrectionHistoryRow {
        id: correction.id,
        memory_id: correction.memory_id.to_string(),
        corrected_at: correction.corrected_at.to_rfc3339(),
        corrected_by: correction.corrected_by,
        reason: correction.reason,
        disposition: display_correction_disposition(correction.disposition),
        previous_content: correction.previous_content,
        corrected_content: correction.corrected_content,
        outcome: describe_correction_outcome(correction.disposition),
        related_memory_id,
        related_memory_state,
        related_memory_preview,
    })
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

fn display_correction_disposition(disposition: CorrectionDisposition) -> String {
    match disposition {
        CorrectionDisposition::Applied => "applied",
        CorrectionDisposition::Archived => "archived",
        CorrectionDisposition::Merged => "merged",
        CorrectionDisposition::Contradiction => "contradiction",
    }
    .to_string()
}

fn describe_correction_outcome(disposition: CorrectionDisposition) -> String {
    match disposition {
        CorrectionDisposition::Applied => {
            "applied in place; the corrected memory remains the active canonical row"
        }
        CorrectionDisposition::Archived => {
            "applied, then archived by the safety gate; the corrected memory is now dormant"
        }
        CorrectionDisposition::Merged => {
            "merged into the related memory; the corrected memory was archived to dormant"
        }
        CorrectionDisposition::Contradiction => {
            "applied in place and journaled as a contradiction against the related memory"
        }
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
    println!("correction history: {}", response.corrections.len());
    for correction in &response.corrections {
        println!(
            "- {} at {} by {} [{}]",
            correction.id, correction.corrected_at, correction.corrected_by, correction.disposition
        );
        if !correction.reason.is_empty() {
            println!("  reason: {}", correction.reason);
        }
        println!("  previous: {}", preview(&correction.previous_content));
        println!("  corrected: {}", preview(&correction.corrected_content));
        if let Some(related_memory_id) = &correction.related_memory_id {
            match (
                correction.related_memory_state.as_deref(),
                correction.related_memory_preview.as_deref(),
            ) {
                (Some(state), Some(related_preview)) => {
                    println!("  related memory: {related_memory_id} ({state}) — {related_preview}");
                }
                (Some(state), None) => {
                    println!("  related memory: {related_memory_id} ({state})");
                }
                _ => {
                    println!("  related memory: {related_memory_id}");
                }
            }
        }
        println!("  outcome: {}", correction.outcome);
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

fn print_json<T>(envelope: &CliMachineEnvelope<T>) -> Result<(), CliError>
where
    T: Serialize,
{
    println!("{}", serde_json::to_string_pretty(envelope)?);
    Ok(())
}

fn print_success_json<T>(command: &'static str, data: &T) -> Result<(), CliError>
where
    T: Serialize,
{
    let machine = &current_machine_context().machine;
    let data = serde_json::to_value(data)?;
    print_json(&build_cli_success_envelope(machine, [command], data))
}

pub fn emit_machine_failure(error: &CliError) -> Result<(), CliError> {
    let context = current_machine_context();
    let kind = match error {
        CliError::Validation(_) | CliError::InvalidId { .. } => CliFailureKind::InvalidInput,
        CliError::Consolidation(_)
        | CliError::Store(_)
        | CliError::Gate(_)
        | CliError::Embedding(_)
        | CliError::Llm(_)
        | CliError::Io(_)
        | CliError::Json(_) => CliFailureKind::Runtime,
    };

    print_json(&build_cli_failure_envelope::<serde_json::Value, _>(
        &context.machine,
        [context.command],
        kind,
        error.to_string(),
        None,
    ))
}

pub fn has_machine_context() -> bool {
    CLI_MACHINE_CONTEXT.get().is_some()
}

fn execute_rollback_command(
    ctx: StoreContext,
    id: String,
    version: i64,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let memory_id = parse_memory_id(&id)?;
    let version_u32 = u32::try_from(version).map_err(|_| {
        CliError::Validation(format!(
            "version must be a non-negative 32-bit integer, got {version}"
        ))
    })?;
    ctx.store.rollback_to_version(&memory_id, version_u32)?;
    let memory =
        run_async(ctx.store.get_raw(&memory_id))?.ok_or(StoreError::NotFound(memory_id))?;
    match format {
        OutputFormat::Text => {
            println!("Rolled back {} to version {version}", memory.id);
            println!("Content: {}", preview(&memory.content));
        }
        OutputFormat::Json => {
            print_success_json(
                "rollback",
                &RollbackResponse {
                    memory_id: memory.id.to_string(),
                    restored_version: version,
                    content_preview: preview(&memory.content),
                },
            )?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_corroborate_command(
    ctx: StoreContext,
    id: String,
    with: String,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let memory_id = parse_memory_id(&id)?;
    let corroborating_id = parse_memory_id(&with)?;
    ctx.store.corroborate(&memory_id, &corroborating_id)?;
    let memory =
        run_async(ctx.store.get_raw(&memory_id))?.ok_or(StoreError::NotFound(memory_id))?;
    match format {
        OutputFormat::Text => {
            println!("Corroborated {} with {}", memory.id, corroborating_id);
            println!(
                "Reliability: {:.2} | Corroboration count: {}",
                memory.reliability_score, memory.corroboration_count
            );
        }
        OutputFormat::Json => {
            print_success_json(
                "corroborate",
                &CorroborateResponse {
                    memory_id: memory.id.to_string(),
                    corroborating_id: corroborating_id.to_string(),
                    new_reliability: memory.reliability_score,
                    corroboration_count: memory.corroboration_count,
                },
            )?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_budget_command(ctx: StoreContext, format: OutputFormat) -> Result<ExitCode, CliError> {
    let (dormanted, deleted) = ctx.store.enforce_budget()?;
    match format {
        OutputFormat::Text => {
            println!("Budget enforcement complete");
            println!("  Dormanted: {dormanted}");
            println!("  Deleted: {deleted}");
        }
        OutputFormat::Json => {
            print_success_json("budget", &BudgetResponse { dormanted, deleted })?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_correct_command(
    ctx: StoreContext,
    id: String,
    content: String,
    by: String,
    reason: Option<String>,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let memory_id = parse_memory_id(&id)?;
    let correction = ctx
        .store
        .correct_memory(&memory_id, &content, &by, reason.as_deref())?;
    let updated =
        run_async(ctx.store.get_raw(&memory_id))?.ok_or(StoreError::NotFound(memory_id))?;
    let correction_row = correction_history_row(&ctx, correction)?;
    match format {
        OutputFormat::Text => {
            println!("Corrected memory {memory_id}");
            println!("  Correction ID: {}", correction_row.id);
            println!("  Disposition: {}", correction_row.disposition);
            println!("  Current state: {}", display_state(updated.state));
            println!("  New reliability: {:.2}", updated.reliability_score);
            println!("  Embedding stale: {}", updated.embedding_stale);
            if let Some(related_memory_id) = &correction_row.related_memory_id {
                match (
                    correction_row.related_memory_state.as_deref(),
                    correction_row.related_memory_preview.as_deref(),
                ) {
                    (Some(state), Some(related_preview)) => {
                        println!(
                            "  Related memory: {related_memory_id} ({state}) — {related_preview}"
                        );
                    }
                    (Some(state), None) => {
                        println!("  Related memory: {related_memory_id} ({state})");
                    }
                    _ => {
                        println!("  Related memory: {related_memory_id}");
                    }
                }
            }
            println!("  Outcome: {}", correction_row.outcome);
        }
        OutputFormat::Json => {
            print_success_json(
                "correct",
                &CorrectResponse {
                    correction: correction_row,
                    corrected_memory_state: display_state(updated.state),
                    new_reliability: updated.reliability_score,
                    embedding_stale: updated.embedding_stale,
                },
            )?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_feedback_command(
    ctx: StoreContext,
    id: String,
    query: String,
    relevant: bool,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let memory_id = parse_memory_id(&id)?;
    let feedback = ctx.store.record_feedback(&memory_id, &query, relevant)?;
    let learning_report = ctx.store.learned_weights_report()?;
    match format {
        OutputFormat::Text => {
            println!(
                "Recorded {} feedback for {memory_id}",
                if relevant { "positive" } else { "negative" }
            );
            print_weights_summary_text(&learning_report);
        }
        OutputFormat::Json => {
            print_success_json(
                "feedback",
                &FeedbackResponse {
                    feedback_id: feedback.id,
                    memory_id: memory_id.to_string(),
                    was_relevant: relevant,
                    learning: weights_summary_response(&learning_report),
                },
            )?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_weights_command(ctx: StoreContext, format: OutputFormat) -> Result<ExitCode, CliError> {
    let learning_report = ctx.store.learned_weights_report()?;
    match format {
        OutputFormat::Text => {
            print_weights_summary_text(&learning_report);
        }
        OutputFormat::Json => {
            print_success_json(
                "weights",
                &WeightsResponse {
                    learning: weights_summary_response(&learning_report),
                },
            )?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn print_weights_summary_text(report: &LearnedWeightsReport) {
    println!(
        "Scoring mode: {} (samples: {} total = {} relevant / {} irrelevant, confidence: {:.2})",
        if report.using_defaults {
            "defaults"
        } else {
            "learned"
        },
        report.sample_size,
        report.relevant_samples,
        report.irrelevant_samples,
        report.confidence,
    );
    println!("Reason: {}", report.status_detail);
    println!("Effective live weights:");
    print_weight_values_text(report.effective_weights);
    println!("Default weights:");
    print_weight_values_text(report.default_weights);
}

fn print_weight_values_text(weights: LearnedWeightValues) {
    println!("  similarity_weight: {:.4}", weights.similarity_weight);
    println!("  recency_weight: {:.4}", weights.recency_weight);
    println!("  access_weight: {:.4}", weights.access_weight);
    println!("  priority_weight: {:.4}", weights.priority_weight);
}

fn weights_summary_response(report: &LearnedWeightsReport) -> WeightsSummaryResponse {
    WeightsSummaryResponse {
        strategy: if report.using_defaults {
            "defaults".to_string()
        } else {
            "learned".to_string()
        },
        status_detail: report.status_detail.clone(),
        sample_size: report.sample_size,
        relevant_samples: report.relevant_samples,
        irrelevant_samples: report.irrelevant_samples,
        learning_confidence: report.confidence,
        effective_weights: weight_values_response(report.effective_weights),
        default_weights: weight_values_response(report.default_weights),
    }
}

fn weight_values_response(weights: LearnedWeightValues) -> WeightValuesResponse {
    WeightValuesResponse {
        similarity_weight: weights.similarity_weight,
        recency_weight: weights.recency_weight,
        access_weight: weights.access_weight,
        priority_weight: weights.priority_weight,
    }
}

fn execute_traverse_command(
    ctx: StoreContext,
    id: String,
    depth: u32,
    relation: Option<String>,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let memory_id = parse_memory_id(&id)?;
    let result = ctx
        .store
        .traverse_links(&memory_id, depth, relation.as_deref())?;
    match format {
        OutputFormat::Text => {
            println!("Graph traversal from {memory_id} (max depth: {depth})");
            println!("Found {} connected memories", result.nodes.len());
            for node in &result.nodes {
                println!(
                    "  [depth={}] {} — {}",
                    node.depth,
                    node.memory.id,
                    preview(&node.memory.content)
                );
            }
        }
        OutputFormat::Json => {
            let nodes: Vec<TraverseNodeRow> = result
                .nodes
                .iter()
                .map(|node| TraverseNodeRow {
                    id: node.memory.id.to_string(),
                    depth: node.depth,
                    content_preview: preview(&node.memory.content),
                    memory_type: display_memory_type(node.memory.memory_type),
                    link_count: node.incoming_links.len(),
                })
                .collect();
            print_success_json(
                "traverse",
                &TraverseResponse {
                    start_id: memory_id.to_string(),
                    max_depth: depth,
                    node_count: nodes.len(),
                    nodes,
                },
            )?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_detect_poisoning_command(
    ctx: StoreContext,
    quarantine: bool,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let alerts = ctx.store.detect_poisoning()?;
    let remediation = quarantine
        .then(|| ctx.store.remediate_poisoning(&alerts))
        .transpose()?;
    match format {
        OutputFormat::Text => {
            if alerts.is_empty() {
                println!("No poisoning indicators detected.");
            } else {
                println!("Detected {} poisoning alert(s):", alerts.len());
                for alert in &alerts {
                    println!(
                        "  [{:.1}] {:?} — {} ({} memories affected)",
                        alert.severity,
                        alert.alert_type,
                        alert.description,
                        alert.memory_ids.len()
                    );
                    println!("    alert_id: {}", alert.id);
                    println!("    detected_at: {}", alert.detected_at.to_rfc3339());
                    println!(
                        "    memory_ids: {}",
                        alert
                            .memory_ids
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                if !quarantine {
                    println!(
                        "Run `detect-poisoning --quarantine` to dormant-quarantine implicated low-trust active memories (`--remediate` alias also works)."
                    );
                }
                if let Some(remediation) = &remediation {
                    println!(
                        "Quarantine remediated {} low-trust active memories and skipped {} memories.",
                        remediation.quarantined_ids.len(),
                        remediation.skipped_ids.len()
                    );
                    for action in &remediation.actions {
                        println!(
                            "  {} {} — {}",
                            action.action.as_str(),
                            action.memory_id,
                            action.reason
                        );
                        if !action.alert_ids.is_empty() {
                            println!("    alert_ids: {}", action.alert_ids.join(", "));
                        }
                        if !action.alert_types.is_empty() {
                            println!(
                                "    alert_types: {}",
                                action
                                    .alert_types
                                    .iter()
                                    .map(crate::storage::display_poisoning_alert_type)
                                    .map(str::to_string)
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                        }
                    }
                }
            }
        }
        OutputFormat::Json => {
            let rows: Vec<PoisoningAlertRow> = alerts
                .iter()
                .map(|alert| PoisoningAlertRow {
                    id: alert.id.clone(),
                    detected_at: alert.detected_at.to_rfc3339(),
                    alert_type: crate::storage::display_poisoning_alert_type(&alert.alert_type)
                        .to_string(),
                    severity: alert.severity,
                    description: alert.description.clone(),
                    affected_count: alert.memory_ids.len(),
                    memory_ids: alert.memory_ids.iter().map(ToString::to_string).collect(),
                })
                .collect();
            print_success_json(
                "detect-poisoning",
                &PoisoningResponse {
                    alert_count: rows.len(),
                    alerts: rows,
                    remediation: remediation.map(|report| PoisoningRemediationRow {
                        quarantined_count: report.quarantined_ids.len(),
                        quarantined_ids: report
                            .quarantined_ids
                            .iter()
                            .map(ToString::to_string)
                            .collect(),
                        skipped_count: report.skipped_ids.len(),
                        skipped_ids: report.skipped_ids.iter().map(ToString::to_string).collect(),
                        actions: report
                            .actions
                            .into_iter()
                            .map(|action| PoisoningRemediationActionRow {
                                memory_id: action.memory_id.to_string(),
                                action: action.action.as_str().to_string(),
                                reason: action.reason,
                                alert_ids: action.alert_ids,
                                alert_types: action
                                    .alert_types
                                    .iter()
                                    .map(crate::storage::display_poisoning_alert_type)
                                    .map(str::to_string)
                                    .collect(),
                            })
                            .collect(),
                    }),
                },
            )?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_delete_link_command(
    ctx: StoreContext,
    id: String,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let deleted = ctx.store.delete_link(&id)?;
    match format {
        OutputFormat::Text => {
            if deleted {
                println!("Deleted link {id}");
            } else {
                println!("Link {id} not found");
            }
        }
        OutputFormat::Json => {
            print_success_json(
                "delete-link",
                &DeleteLinkResponse {
                    link_id: id,
                    deleted,
                },
            )?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_share_export_command(
    ctx: StoreContext,
    output: Option<PathBuf>,
    max_sensitivity: SensitivityLevel,
    min_reliability: f32,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let store = SqliteMemoryStore::new(&ctx.db_path, ctx.scope)?;
    let config = ShareConfig {
        max_sensitivity,
        min_reliability,
        type_filter: None,
        tag_filter: None,
    };
    let memories = store.export_for_sharing(&config)?;
    let payload = serde_json::to_string_pretty(&memories)?;

    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &payload)?;
        match format {
            OutputFormat::Text => {
                println!(
                    "Exported {} memories for sharing to {}",
                    memories.len(),
                    path.display()
                );
            }
            OutputFormat::Json => print_success_json(
                "share-export",
                &ShareExportResponse {
                    exported_count: memories.len(),
                    max_sensitivity: format!("{max_sensitivity:?}"),
                    min_reliability,
                },
            )?,
        }
    } else {
        match format {
            OutputFormat::Text => println!("{payload}"),
            OutputFormat::Json => print_success_json(
                "share-export",
                &serde_json::json!({
                    "exportedCount": memories.len(),
                    "memories": memories,
                }),
            )?,
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_share_import_command(
    ctx: StoreContext,
    input: Option<PathBuf>,
    format: OutputFormat,
) -> Result<ExitCode, CliError> {
    let raw = if let Some(path) = &input {
        fs::read_to_string(path)?
    } else {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        buf
    };

    let memories: Vec<Memory> = serde_json::from_str(&raw)?;
    let report = ctx.store.import_shared_with_report(&memories)?;

    match format {
        OutputFormat::Text => {
            println!(
                "Imported {} shared memories as dormant review entries ({} quarantined, {} skipped)",
                report.new_ids.len(),
                report.quarantined_ids.len(),
                report.skipped_reasons.len()
            );
            for outcome in &report.outcomes {
                match outcome.memory_id {
                    Some(memory_id) => println!(
                        "  {} {} — {}",
                        outcome.disposition.as_str(),
                        memory_id,
                        outcome.reason
                    ),
                    None => println!("  {} — {}", outcome.disposition.as_str(), outcome.reason),
                }
                if let Some(related_memory_id) = outcome.related_memory_id {
                    println!("    related_memory_id: {related_memory_id}");
                }
            }
        }
        OutputFormat::Json => print_success_json(
            "share-import",
            &ShareImportResponse {
                imported_count: report.new_ids.len(),
                review_count: report.review_ids.len(),
                quarantined_count: report.quarantined_ids.len(),
                skipped_count: report.skipped_reasons.len(),
                new_ids: report.new_ids.iter().map(ToString::to_string).collect(),
                review_ids: report.review_ids.iter().map(ToString::to_string).collect(),
                quarantined_ids: report
                    .quarantined_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                skipped_reasons: report.skipped_reasons,
                outcomes: report
                    .outcomes
                    .into_iter()
                    .map(|outcome| ShareImportOutcomeRow {
                        memory_id: outcome.memory_id.map(|id| id.to_string()),
                        disposition: outcome.disposition.as_str().to_string(),
                        reason: outcome.reason,
                        related_memory_id: outcome.related_memory_id.map(|id| id.to_string()),
                    })
                    .collect(),
            },
        )?,
    }

    Ok(ExitCode::SUCCESS)
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
        format_contradiction_gate_result, format_gate_result, generate_document_embedding,
        open_store, reembed_stale_memories, run_async, Cli, CliEmbeddingProvider, CliLlmProvider,
        CliScope, Command, OutputFormat, StoreArgs, StoreContext,
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
        model_id: &'static str,
        responses: HashMap<String, StubEmbeddingResponse>,
        calls: Mutex<Vec<String>>,
    }

    impl StubEmbeddingProvider {
        fn new<I, S>(responses: I) -> Self
        where
            I: IntoIterator<Item = (S, StubEmbeddingResponse)>,
            S: Into<String>,
        {
            Self::new_with_model("stub-embedding-provider", responses)
        }

        fn new_with_model<I, S>(model_id: &'static str, responses: I) -> Self
        where
            I: IntoIterator<Item = (S, StubEmbeddingResponse)>,
            S: Into<String>,
        {
            Self {
                model_id,
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
            self.model_id
        }
    }

    #[tokio::test]
    async fn reembed_path_uses_document_prefix_for_nomic_models() {
        let provider = StubEmbeddingProvider::new_with_model(
            "nomic-embed-text:latest",
            [(
                "search_document: Semantic document body",
                StubEmbeddingResponse::Embedding(vec![0.42; 768]),
            )],
        );

        let embedding = generate_document_embedding(&provider, "  Semantic document body  ")
            .await
            .expect("document embedding should succeed");

        assert_eq!(embedding.len(), 768);
        assert_eq!(
            provider.calls(),
            vec!["search_document: Semantic document body".to_string()]
        );
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
            ("test", StubEmbeddingResponse::Embedding(vec![0.0; 1])),
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

        assert_eq!(response.stale_found, 2);
        assert_eq!(response.reembedded_count, 2);
        assert_eq!(response.reembedded_ids.len(), 2);

        let older_memory = ctx.store.get_raw(&older_id);
        let older_memory = run_async(older_memory)
            .expect("load older memory")
            .expect("older memory exists");
        let newer_memory = ctx.store.get_raw(&newer_id);
        let newer_memory = run_async(newer_memory)
            .expect("load newer memory")
            .expect("newer memory exists");
        assert!(!older_memory.embedding_stale);
        assert!(!newer_memory.embedding_stale);

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
        let _memory_id = memory.id;
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
        assert!(
            message.contains("reembed provider unavailable at start"),
            "expected fail-fast health check error, got: {message}"
        );
        assert!(
            message.contains("stub embed failure")
                || message.contains("missing stub embedding for"),
            "expected stub failure in error, got: {message}"
        );

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
            OutputFormat::Text,
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
        execute_import_command(ctx, Some(json_path.clone()), false, OutputFormat::Text)
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
