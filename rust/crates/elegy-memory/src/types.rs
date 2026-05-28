use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Default weight applied to vector similarity during retrieval scoring.
pub const DEFAULT_SIMILARITY_WEIGHT: f32 = 0.4;

/// Default weight applied to recency during retrieval scoring.
pub const DEFAULT_RECENCY_WEIGHT: f32 = 0.25;

/// Default weight applied to access frequency during retrieval scoring.
pub const DEFAULT_ACCESS_WEIGHT: f32 = 0.15;

/// Default weight applied to priority (`importance × reliability`) during retrieval scoring.
pub const DEFAULT_PRIORITY_WEIGHT: f32 = 0.2;

/// Default fraction of remaining model context allocated to memory injection.
pub const DEFAULT_MEMORY_CONTEXT_RATIO: f32 = 0.10;

/// Default number of tokens reserved for the model response.
pub const DEFAULT_RESPONSE_RESERVE: u32 = 4_096;

/// Default importance threshold below which the salience gate archives memories.
pub const DEFAULT_SALIENCE_THRESHOLD: f32 = 0.2;

/// Default lower bound of the likely-duplicate warning band used by the salience gate.
pub const DEFAULT_NOVELTY_DOUBT_THRESHOLD: f32 = 0.80;

/// Default semantic similarity threshold used for conservative merge decisions.
pub const DEFAULT_MERGE_SIMILARITY_THRESHOLD: f32 = 0.85;

/// Default semantic similarity threshold used for exact-duplicate rejection.
pub const DEFAULT_DUPLICATE_SIMILARITY_THRESHOLD: f32 = 0.99;

/// Default fixed lambda used for MVP recency decay.
pub const DEFAULT_DECAY_LAMBDA_BASE: f32 = 0.10;

/// Default importance threshold below which inferred memories are archived.
pub const DEFAULT_AGENT_INFERRED_IMPORTANCE_THRESHOLD: f32 = 0.5;

/// Stable identifier for a memory record.
///
/// Implementations should use UUID v4 values.
pub type MemoryId = Uuid;

/// Core persisted memory record stored and retrieved by the memory engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    /// Unique memory identifier.
    pub id: MemoryId,
    /// Canonical memory content used for retrieval and updates.
    pub content: String,
    /// Optional shorter summary suitable for prompt injection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Scope that owns this memory.
    pub scope: MemoryScope,
    /// High-level category for the memory.
    pub memory_type: MemoryType,
    /// Origin and trust tier for this memory.
    pub provenance: ProvenanceLevel,
    /// LLM-assigned salience score in the inclusive range `0.0..=1.0`.
    pub importance_score: f32,
    /// System-computed trust score in the inclusive range `0.0..=1.0`.
    pub reliability_score: f32,
    /// Sensitivity classification for privacy handling.
    pub sensitivity: SensitivityLevel,
    /// Current lifecycle state.
    pub state: MemoryState,
    /// Free-form discovery and filtering tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Optional workflow or application-specific status label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Extensible string metadata associated with the memory.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub custom_metadata: HashMap<String, String>,
    /// Number of successful retrievals or direct accesses.
    pub access_count: u32,
    /// Number of independent corroborations recorded for this memory.
    pub corroboration_count: u32,
    /// Whether the stored embedding must be recomputed.
    pub embedding_stale: bool,
    /// Creation timestamp in UTC-compatible RFC3339 form.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp in UTC-compatible RFC3339 form.
    pub updated_at: DateTime<Utc>,
    /// Most recent access timestamp, if this memory has been retrieved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_accessed_at: Option<DateTime<Utc>>,
    /// Optional tenant identifier reserved for multi-tenant backends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    /// Optional user identifier for per-user memories and purge flows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Optional agent identifier for agent-scoped procedural memories.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

/// Isolation boundary that determines where a memory lives and how long it lasts.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MemoryScope {
    /// Ephemeral memory bound to the current interaction session.
    Session,
    /// Project-specific memory retained with a workspace.
    Workspace,
    /// Cross-workspace memory associated with a user.
    User,
    /// Procedural memory associated with an agent identity.
    Agent,
}

impl MemoryScope {
    /// Returns the relative hierarchy rank for this scope.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Session => 0,
            Self::Workspace => 1,
            Self::User => 2,
            Self::Agent => 3,
        }
    }

    /// Returns the next broader scope in the hierarchy, if any.
    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Session => Some(Self::Workspace),
            Self::Workspace => Some(Self::User),
            Self::User => Some(Self::Agent),
            Self::Agent => None,
        }
    }

    /// Returns the scopes visible from this scope, ordered from nearest to broadest.
    #[must_use]
    pub const fn visible_scopes(self) -> &'static [Self] {
        match self {
            Self::Session => &[Self::Session, Self::Workspace, Self::User, Self::Agent],
            Self::Workspace => &[Self::Workspace, Self::User, Self::Agent],
            Self::User => &[Self::User, Self::Agent],
            Self::Agent => &[Self::Agent],
        }
    }

    /// Returns true when `target` is broader than `self`.
    #[must_use]
    pub const fn can_promote_to(self, target: Self) -> bool {
        self.rank() < target.rank()
    }

    /// Returns the broader of the two scopes.
    #[must_use]
    pub const fn max(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }
}

/// Semantic category for a memory item.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MemoryType {
    /// Objective or verifiable information.
    Fact,
    /// Subjective preference expressed or demonstrated by the user.
    Preference,
    /// Decision captured as an intentional choice.
    Decision,
    /// Procedural knowledge describing how to do something.
    Procedure,
    /// Agent observation or non-authoritative inference candidate.
    Observation,
}

/// Provenance tier used to seed a memory's reliability score.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProvenanceLevel {
    /// Information explicitly stated by the user.
    UserStated,
    /// Information directly observed by the agent.
    AgentObserved,
    /// Information produced by consolidation of existing memories.
    Consolidated,
    /// Information imported from an external source.
    Imported,
    /// Information inferred by the agent.
    AgentInferred,
}

impl ProvenanceLevel {
    /// Returns the architecture-defined base reliability for this provenance tier.
    #[must_use]
    pub const fn base_reliability(self) -> f32 {
        match self {
            Self::UserStated => 1.0,
            Self::AgentObserved => 0.8,
            Self::Consolidated => 0.7,
            Self::Imported => 0.6,
            Self::AgentInferred => 0.5,
        }
    }
}

/// Sensitivity classification for privacy and purge handling.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SensitivityLevel {
    /// Low-sensitivity data suitable for standard handling.
    #[default]
    Low,
    /// Moderately sensitive data that may require additional review.
    Medium,
    /// High-sensitivity data that should be handled conservatively.
    High,
    /// Critically sensitive data requiring the strictest controls.
    Critical,
}

/// Lifecycle state for a memory record.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MemoryState {
    /// Memory is eligible for normal retrieval.
    #[default]
    Active,
    /// Memory is archived and excluded from default retrieval.
    Dormant,
    /// Memory has been logically deleted or purged.
    Deleted,
}

/// Resolution state for contradiction tracking.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ResolutionStatus {
    /// Contradiction has not been resolved yet.
    #[default]
    Unresolved,
    /// Contradiction was resolved by an explicit user decision.
    ResolvedByUser,
    /// Contradiction was resolved automatically by the system.
    ResolvedBySystem,
    /// Contradiction entry was intentionally dismissed.
    Dismissed,
}

/// Candidate memory produced before storage and salience-gate evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCandidate {
    /// Canonical candidate content.
    pub content: String,
    /// Optional short summary for prompt injection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Semantic category for the candidate.
    pub memory_type: MemoryType,
    /// Origin and trust tier for the candidate.
    pub provenance: ProvenanceLevel,
    /// LLM-assigned salience score in the inclusive range `0.0..=1.0`.
    pub importance_score: f32,
    /// Sensitivity classification for the candidate.
    pub sensitivity: SensitivityLevel,
    /// Free-form tags for organization and filtering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Extensible string metadata associated with the candidate.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub custom_metadata: HashMap<String, String>,
    /// Optional precomputed embedding vector for novelty checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
}

/// Stored memory supplied to a consolidator with an optional embedding payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConsolidationCandidate {
    /// Retrieved memory record to consider during consolidation.
    pub memory: Memory,
    /// Optional embedding used by implementations that perform semantic deduplication.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
}

/// Memory returned from search with its final rank score and similarity signal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScoredMemory {
    /// Retrieved memory payload.
    pub memory: Memory,
    /// Final combined retrieval score.
    pub score: f32,
    /// Raw semantic similarity component used during ranking.
    pub similarity: f32,
}

/// Operational health snapshot for a memory store or scope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryHealthReport {
    /// Scope covered by the report.
    pub scope: MemoryScope,
    /// Number of active memories.
    pub active_count: u64,
    /// Number of dormant memories.
    pub dormant_count: u64,
    /// Total bytes consumed by persisted storage.
    pub total_storage_bytes: u64,
    /// Fraction of the configured active-memory budget in use.
    pub budget_usage_ratio: f32,
    /// Number of unresolved contradiction entries.
    pub unresolved_contradictions: u64,
    /// Number of memories waiting for re-embedding.
    pub stale_embeddings_count: u64,
    /// Timestamp of the last consolidation pass, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_consolidation: Option<DateTime<Utc>>,
    /// Oldest currently active memory, if any exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oldest_active_memory: Option<DateTime<Utc>>,
    /// Most recently created memory, if any exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newest_memory: Option<DateTime<Utc>>,
}

/// Logged contradiction between two memory records.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContradictionEntry {
    /// Stable contradiction identifier.
    pub id: String,
    /// Identifier of the first contradictory memory.
    pub memory_a_id: MemoryId,
    /// Identifier of the second contradictory memory.
    pub memory_b_id: MemoryId,
    /// Timestamp when the contradiction was detected.
    pub detected_at: DateTime<Utc>,
    /// Human-readable contradiction description.
    pub description: String,
    /// Current resolution status.
    pub resolution_status: ResolutionStatus,
    /// Resolution timestamp, if the contradiction has been closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<DateTime<Utc>>,
    /// Optional note describing how the contradiction was resolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_note: Option<String>,
}

/// Summary of records removed by a purge operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PurgeReport {
    /// Number of memories deleted.
    pub memories_deleted: u64,
    /// Number of version rows deleted.
    pub versions_deleted: u64,
    /// Number of link rows deleted.
    pub links_deleted: u64,
    /// Number of contradiction rows deleted.
    pub contradictions_deleted: u64,
    /// Number of embeddings deleted.
    pub embeddings_deleted: u64,
}

/// Query-time context budgeting configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryContextConfig {
    /// Fraction of remaining context allocated to memory injection.
    pub memory_context_ratio: f32,
    /// Total model context window in tokens.
    pub model_max_tokens: u32,
    /// Tokens already consumed by prompts, conversation, or caller-provided context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub already_used_tokens: Option<u32>,
    /// Tokens reserved for the model response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_reserve: Option<u32>,
}

impl MemoryContextConfig {
    /// Creates a context-budget configuration using architecture defaults.
    #[must_use]
    pub fn new(model_max_tokens: u32) -> Self {
        Self {
            memory_context_ratio: DEFAULT_MEMORY_CONTEXT_RATIO,
            model_max_tokens,
            already_used_tokens: Some(0),
            response_reserve: Some(DEFAULT_RESPONSE_RESERVE),
        }
    }

    /// Returns the effective number of already-used tokens.
    #[must_use]
    pub fn effective_already_used_tokens(&self) -> u32 {
        self.already_used_tokens.unwrap_or(0)
    }

    /// Returns the effective response reserve.
    #[must_use]
    pub fn effective_response_reserve(&self) -> u32 {
        self.response_reserve.unwrap_or(DEFAULT_RESPONSE_RESERVE)
    }
}

/// Search request issued against a memory store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    /// Raw text query.
    pub text: String,
    /// Optional precomputed embedding for the query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    /// Scope to search within.
    pub scope: MemoryScope,
    /// Optional lifecycle-state filter. Implementations should default to `Active`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_filter: Option<MemoryState>,
    /// Optional memory-type filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_filter: Option<Vec<MemoryType>>,
    /// Maximum number of results requested before context-budget trimming.
    pub max_results: usize,
    /// Optional context budgeting parameters for prompt injection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_config: Option<MemoryContextConfig>,
    /// Optional session identifier used to record cross-session access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Optional agent identifier filter for agent-scoped searches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

/// Export encoding supported by observability and portability flows.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    /// Structured JSON export.
    #[default]
    Json,
    /// SQLite-backed portable export.
    Sqlite,
    /// Portable `.elegy` archive (ZIP containing metadata + SQLite).
    Elegy,
}

/// Scope-level tuning values persisted in the configuration table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScopeConfig {
    /// Fixed lambda used for MVP recency decay.
    pub decay_lambda_base: f32,
    /// Weight applied to vector similarity during retrieval.
    pub similarity_weight: f32,
    /// Weight applied to recency during retrieval.
    pub recency_weight: f32,
    /// Weight applied to access frequency during retrieval.
    pub access_weight: f32,
    /// Weight applied to priority (`importance × reliability`) during retrieval.
    pub priority_weight: f32,
    /// Fraction of remaining context reserved for memory injection.
    pub memory_context_ratio: f32,
    /// Tokens reserved for the model response.
    pub response_reserve: u32,
    /// Importance threshold below which new memories should be archived.
    pub salience_threshold: f32,
    /// Lower bound of the novelty doubt zone. Similarities below this are accepted as new.
    pub novelty_doubt_threshold: f32,
    /// Similarity threshold at or above which a new memory may be merged.
    pub merge_similarity_threshold: f32,
    /// Similarity threshold at or above which a new memory may be rejected as duplicate.
    pub duplicate_similarity_threshold: f32,
    /// Importance threshold below which `AgentInferred` memories should be archived.
    pub agent_inferred_importance_threshold: f32,
}

impl Default for ScopeConfig {
    fn default() -> Self {
        Self {
            decay_lambda_base: DEFAULT_DECAY_LAMBDA_BASE,
            similarity_weight: DEFAULT_SIMILARITY_WEIGHT,
            recency_weight: DEFAULT_RECENCY_WEIGHT,
            access_weight: DEFAULT_ACCESS_WEIGHT,
            priority_weight: DEFAULT_PRIORITY_WEIGHT,
            memory_context_ratio: DEFAULT_MEMORY_CONTEXT_RATIO,
            response_reserve: DEFAULT_RESPONSE_RESERVE,
            salience_threshold: DEFAULT_SALIENCE_THRESHOLD,
            novelty_doubt_threshold: DEFAULT_NOVELTY_DOUBT_THRESHOLD,
            merge_similarity_threshold: DEFAULT_MERGE_SIMILARITY_THRESHOLD,
            duplicate_similarity_threshold: DEFAULT_DUPLICATE_SIMILARITY_THRESHOLD,
            agent_inferred_importance_threshold: DEFAULT_AGENT_INFERRED_IMPORTANCE_THRESHOLD,
        }
    }
}

/// Historical snapshot captured when a memory's content changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryVersion {
    /// Stable version-row identifier.
    pub id: String,
    /// Identifier of the memory this version belongs to.
    pub memory_id: MemoryId,
    /// Monotonic version number for the memory.
    pub version_number: u32,
    /// Previous content captured before the latest update.
    pub content: String,
    /// Actor that initiated the change.
    pub changed_by: String,
    /// Human-readable reason for the change.
    pub change_reason: String,
    /// Timestamp when the version row was recorded.
    pub changed_at: DateTime<Utc>,
}

/// A directional link between two memory records in the proto-graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLink {
    /// Stable link-row identifier.
    pub id: String,
    /// Origin memory of the relationship.
    pub source_id: MemoryId,
    /// Destination memory of the relationship.
    pub target_id: MemoryId,
    /// Kind of relationship (e.g. `supersedes`, `contradicts`, `corroborates`).
    pub relation_type: String,
    /// Strength or confidence of the link (default 1.0).
    pub weight: f32,
    /// Timestamp when the link was created.
    pub created_at: DateTime<Utc>,
}

/// Alert raised when memory poisoning patterns are detected.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PoisoningAlert {
    /// Unique alert identifier.
    pub id: String,
    /// Type of poisoning pattern detected.
    pub alert_type: PoisoningAlertType,
    /// Human-readable description of the detected anomaly.
    pub description: String,
    /// Severity score in the inclusive range `0.0..=1.0`.
    pub severity: f32,
    /// Memory identifiers implicated in the alert.
    pub memory_ids: Vec<MemoryId>,
    /// Timestamp when the alert was generated.
    pub detected_at: DateTime<Utc>,
}

/// Classification of poisoning detection heuristics.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PoisoningAlertType {
    /// Abnormally high write frequency from a single source.
    FrequencyAnomaly,
    /// Provenance trust level does not match content confidence.
    TrustMismatch,
    /// Bulk overwrite of existing memories in a short window.
    BulkOverwrite,
    /// Content contradicts a large number of established memories.
    MassContradiction,
}

/// Final safety disposition recorded for a user correction.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionDisposition {
    /// The corrected content replaced the existing memory body and stayed in its current lane.
    Applied,
    /// The corrected content was accepted, but the write-time gate archived the memory.
    Archived,
    /// The corrected content merged into another existing memory.
    Merged,
    /// The corrected content was applied and journaled as a contradiction.
    Contradiction,
}

/// Record of a user-initiated correction applied to a memory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionRecord {
    /// Unique correction identifier.
    pub id: String,
    /// Memory that was corrected.
    pub memory_id: MemoryId,
    /// Content before the correction.
    pub previous_content: String,
    /// Content after the correction.
    pub corrected_content: String,
    /// Actor who performed the correction.
    pub corrected_by: String,
    /// Human-readable reason for the correction.
    pub reason: String,
    /// Final safety disposition chosen for the correction.
    pub disposition: CorrectionDisposition,
    /// Related memory referenced by the correction outcome, when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related_memory_id: Option<MemoryId>,
    /// Timestamp when the correction was applied.
    pub corrected_at: DateTime<Utc>,
}

/// Feedback signal recorded after a memory is retrieved and evaluated for relevance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalFeedback {
    /// Unique feedback identifier.
    pub id: String,
    /// Memory that was retrieved and evaluated.
    pub memory_id: MemoryId,
    /// Whether the memory was relevant to the query context.
    pub relevant: bool,
    /// Optional query text that produced the retrieval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_text: Option<String>,
    /// Timestamp when the feedback was recorded.
    pub recorded_at: DateTime<Utc>,
}

/// Result of a graph traversal starting from a given memory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphTraversalResult {
    /// Starting memory of the traversal.
    pub start_id: MemoryId,
    /// Maximum traversal depth requested.
    pub max_depth: u32,
    /// Nodes discovered during traversal, ordered by depth.
    pub nodes: Vec<GraphNode>,
}

/// A single node in a graph traversal result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    /// The memory at this graph position.
    pub memory: Memory,
    /// Depth at which this node was discovered (0 = start).
    pub depth: u32,
    /// Links that led to this node from the previous depth level.
    pub incoming_links: Vec<MemoryLink>,
}

/// Selective export configuration for cross-agent memory sharing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShareConfig {
    /// Maximum sensitivity level to include in the export.
    pub max_sensitivity: SensitivityLevel,
    /// Minimum reliability score to include.
    pub min_reliability: f32,
    /// Optional memory type filter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_filter: Option<Vec<MemoryType>>,
    /// Optional tag filter (all listed tags must be present).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_filter: Option<Vec<String>>,
}

impl Default for ShareConfig {
    fn default() -> Self {
        Self {
            max_sensitivity: SensitivityLevel::Medium,
            min_reliability: 0.5,
            type_filter: None,
            tag_filter: None,
        }
    }
}

/// Portable archive format for `.elegy` exports.
///
/// Contains a self-describing snapshot of memories, their relationships,
/// and version history for a single scope.  The payload is serialized as
/// JSON so that no additional compression dependency is required.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ElegyArchive {
    /// Format version for forward compatibility.
    pub format_version: String,
    /// ISO-8601 timestamp of when the export was created.
    pub exported_at: DateTime<Utc>,
    /// The scope that was exported.
    pub scope: MemoryScope,
    /// All active and dormant memories in the scope.
    pub memories: Vec<Memory>,
    /// Relationships between exported memories.
    pub links: Vec<MemoryLink>,
    /// Version history for exported memories.
    pub versions: Vec<MemoryVersion>,
}

/// Prompt-compatibility alias for contradiction records.
pub type ContradictionRecord = ContradictionEntry;

/// Prompt-compatibility alias for search queries.
pub type MemorySearchQuery = SearchQuery;

/// Prompt-compatibility alias for scored search results.
pub type MemorySearchResult = ScoredMemory;
