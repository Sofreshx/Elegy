# Traits and Interfaces

All core behaviors in elegy-memory are defined as traits. This document is the contract specification. Implementations must respect these contracts.

## Core Traits

### MemoryStore

The primary storage abstraction. Handles CRUD, search, health, and purge operations.

```rust
#[async_trait]
pub trait MemoryStore: Send + Sync {
    // === Write ===
    
    /// Store a new memory. The memory MUST have already passed through
    /// the SalienceGate. Implementations should NOT re-validate.
    /// Returns the stored memory's ID.
    async fn store(&self, memory: Memory) -> Result<MemoryId, StoreError>;
    
    /// Update an existing memory's content. Automatically:
    /// - Creates a version entry in memory_versions
    /// - Sets embedding_stale = true
    /// - Updates updated_at timestamp
    async fn update_content(&self, id: &MemoryId, new_content: &str, changed_by: &str, reason: &str) -> Result<(), StoreError>;
    
    /// Update metadata fields (tags, status, custom_metadata, importance, reliability, state)
    async fn update_metadata(&self, id: &MemoryId, updates: MetadataUpdate) -> Result<(), StoreError>;
    
    // === Read ===
    
    /// Get a single memory by ID. Increments access_count and updates last_accessed_at.
    async fn get(&self, id: &MemoryId) -> Result<Option<Memory>, StoreError>;
    
    /// Get a memory by ID without updating access tracking. For internal use only.
    async fn get_raw(&self, id: &MemoryId) -> Result<Option<Memory>, StoreError>;
    
    /// List memories with filters. Does NOT update access tracking.
    async fn list(&self, filter: MemoryFilter) -> Result<Vec<Memory>, StoreError>;
    
    // === Search ===
    
    /// Hybrid search: vector similarity + FTS5 keyword + scoring.
    /// Returns memories ranked by final score. Updates access_count for returned memories.
    /// Respects context budget (max_results derived from memory_context_ratio × model_max_tokens).
    async fn search(&self, query: SearchQuery) -> Result<Vec<ScoredMemory>, StoreError>;
    
    /// Find memories similar to a given embedding. Used internally by the salience gate
    /// for novelty checking. Does NOT update access tracking.
    async fn find_similar(&self, embedding: &[f32], threshold: f32, limit: usize) -> Result<Vec<ScoredMemory>, StoreError>;
    
    // === Embedding Management ===
    
    /// Store or update an embedding for a memory.
    async fn store_embedding(&self, id: &MemoryId, embedding: &[f32]) -> Result<(), StoreError>;
    
    /// Get all memories with stale embeddings.
    async fn get_stale_embeddings(&self, limit: usize) -> Result<Vec<MemoryId>, StoreError>;
    
    // === Lifecycle ===
    
    /// Transition a memory to Dormant state.
    async fn make_dormant(&self, id: &MemoryId) -> Result<(), StoreError>;
    
    /// Reactivate a Dormant memory to Active state.
    async fn reactivate(&self, id: &MemoryId) -> Result<(), StoreError>;
    
    /// Hard delete a memory and all its versions, links, embeddings.
    async fn hard_delete(&self, id: &MemoryId) -> Result<(), StoreError>;
    
    // === Purge (GDPR) ===
    
    /// Delete ALL data for a user. Irreversible. Returns a report of what was deleted.
    async fn purge_user(&self, user_id: &str) -> Result<PurgeReport, StoreError>;
    
    /// Delete ALL data in this store. Irreversible.
    async fn purge_all(&self) -> Result<PurgeReport, StoreError>;
    
    // === Health ===
    
    /// Generate a health report for this store.
    async fn health_report(&self) -> Result<MemoryHealthReport, StoreError>;
    
    /// List contradictions with optional status filter.
    async fn list_contradictions(&self, status: Option<ResolutionStatus>) -> Result<Vec<ContradictionEntry>, StoreError>;
    
    /// Record a contradiction between two memories.
    async fn record_contradiction(&self, a_id: &MemoryId, b_id: &MemoryId, description: &str) -> Result<(), StoreError>;
}
```

### EmbeddingProvider

Generates vector embeddings from text. Must be swappable (OpenAI, Ollama, local model, etc.)

```rust
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate an embedding vector for the given text.
    /// Returns a vector of f32 with length == self.dimensions().
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
    
    /// Batch embed multiple texts. Default implementation calls embed() in sequence.
    /// Providers should override for efficiency.
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }
    
    /// The number of dimensions this provider produces.
    fn dimensions(&self) -> usize;
    
    /// The model identifier (e.g., "text-embedding-3-small", "all-MiniLM-L6-v2").
    fn model_id(&self) -> &str;
}
```

### SalienceGate

The write-time filter. Every memory candidate passes through this before storage.

```rust
pub trait SalienceGate: Send + Sync {
    /// Evaluate a memory candidate and decide whether to accept, archive, merge, or reject.
    /// This method should:
    /// 1. Check novelty (cosine similarity with existing memories)
    /// 2. Check salience (importance threshold)
    /// 3. Check provenance (trust-based filtering)
    fn evaluate(&self, candidate: &MemoryCandidate, store: &dyn MemoryStore) -> Result<GateDecision, GateError>;
}

pub enum GateDecision {
    /// Store as Active memory
    Accept,
    /// Store as Dormant (cold storage)
    Archive,
    /// Merge with existing memory (update instead of create)
    Merge { target_id: MemoryId, enriched_content: String },
    /// Do not store (exact duplicate, truly useless)
    Reject { reason: String },
}
```

### MemoryConsolidator

Handles periodic memory cleanup, deduplication, and summarization.

```rust
#[async_trait]
pub trait MemoryConsolidator: Send + Sync {
    /// Consolidate a batch of memories. May merge, summarize, or prune.
    /// Returns the list of changes made.
    async fn consolidate(&self, memories: &[Memory]) -> Result<Vec<ConsolidationAction>, ConsolidationError>;
}

pub enum ConsolidationAction {
    Merged { source_ids: Vec<MemoryId>, result: Memory },
    Summarized { source_id: MemoryId, new_summary: String },
    MadeDormant { id: MemoryId, reason: String },
    Linked { source_id: MemoryId, target_id: MemoryId, relation: String },
}
```

### MemoryObservability

Exposes stats and data for external tools (dashboards, CLIs, monitoring).

```rust
pub trait MemoryObservability: Send + Sync {
    fn health_report(&self, scope: MemoryScope) -> Result<MemoryHealthReport, ObservabilityError>;
    fn list_contradictions(&self, status: Option<ResolutionStatus>) -> Result<Vec<ContradictionEntry>, ObservabilityError>;
    fn export_memories(&self, scope: MemoryScope, format: ExportFormat) -> Result<Vec<u8>, ObservabilityError>;
    fn purge_user(&self, user_id: &str) -> Result<PurgeReport, ObservabilityError>;
    fn purge_scope(&self, scope: MemoryScope) -> Result<PurgeReport, ObservabilityError>;
}
```

## Core Types

```rust
pub type MemoryId = String; // UUID v4

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: MemoryId,
    pub content: String,
    pub summary: Option<String>,
    pub scope: MemoryScope,
    pub memory_type: MemoryType,
    pub provenance: ProvenanceLevel,
    pub importance_score: f32,
    pub reliability_score: f32,
    pub sensitivity: SensitivityLevel,
    pub state: MemoryState,
    pub tags: Vec<String>,
    pub status: Option<String>,
    pub custom_metadata: HashMap<String, String>,
    pub access_count: u32,
    pub corroboration_count: u32,
    pub embedding_stale: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum MemoryScope { Session, Workspace, User, Agent }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum MemoryType { Fact, Preference, Decision, Procedure, Observation }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ProvenanceLevel {
    UserStated,     // 1.0 base reliability
    AgentObserved,  // 0.8
    Consolidated,   // 0.7
    Imported,       // 0.6
    AgentInferred,  // 0.5
}

impl ProvenanceLevel {
    pub fn base_reliability(&self) -> f32 {
        match self {
            Self::UserStated => 1.0,
            Self::AgentObserved => 0.8,
            Self::Consolidated => 0.7,
            Self::Imported => 0.6,
            Self::AgentInferred => 0.5,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SensitivityLevel { Low, Medium, High, Critical }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum MemoryState { Active, Dormant, Deleted }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ResolutionStatus { Unresolved, ResolvedByUser, ResolvedBySystem, Dismissed }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCandidate {
    pub content: String,
    pub summary: Option<String>,
    pub memory_type: MemoryType,
    pub provenance: ProvenanceLevel,
    pub importance_score: f32,
    pub sensitivity: SensitivityLevel,
    pub tags: Vec<String>,
    pub custom_metadata: HashMap<String, String>,
    pub embedding: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredMemory {
    pub memory: Memory,
    pub score: f32,
    pub similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryHealthReport {
    pub scope: MemoryScope,
    pub active_count: u64,
    pub dormant_count: u64,
    pub total_storage_bytes: u64,
    pub budget_usage_ratio: f32,
    pub unresolved_contradictions: u64,
    pub stale_embeddings_count: u64,
    pub last_consolidation: Option<DateTime<Utc>>,
    pub oldest_active_memory: Option<DateTime<Utc>>,
    pub newest_memory: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContradictionEntry {
    pub id: String,
    pub memory_a_id: MemoryId,
    pub memory_b_id: MemoryId,
    pub detected_at: DateTime<Utc>,
    pub description: String,
    pub resolution_status: ResolutionStatus,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgeReport {
    pub memories_deleted: u64,
    pub versions_deleted: u64,
    pub links_deleted: u64,
    pub contradictions_deleted: u64,
    pub embeddings_deleted: u64,
}

pub struct MemoryContextConfig {
    /// Fraction of remaining context to allocate to memory (0.0 - 1.0)
    pub memory_context_ratio: f32,  // Default: 0.10
    /// Model's max context window in tokens (provided by caller)
    pub model_max_tokens: u32,
    /// Tokens already consumed by system prompt, conversation, etc. (optional)
    pub already_used_tokens: Option<u32>,  // Default: 0
    /// Tokens reserved for model response (optional)
    pub response_reserve: Option<u32>,  // Default: 4096
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    pub embedding: Option<Vec<f32>>,   // Pre-computed, or None to compute at search time
    pub scope: MemoryScope,
    pub state_filter: Option<MemoryState>,  // Default: Active only
    pub type_filter: Option<Vec<MemoryType>>,
    pub max_results: usize,
    pub context_config: Option<MemoryContextConfig>,
}

pub enum ExportFormat { Json, Sqlite }
```

## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Memory not found: {0}")]
    NotFound(MemoryId),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Schema migration failed: {0}")]
    Migration(String),
}

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("Embedding provider error: {0}")]
    Provider(String),
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
}

#[derive(Debug, thiserror::Error)]
pub enum GateError {
    #[error("Store error during gate evaluation: {0}")]
    Store(#[from] StoreError),
    #[error("Embedding error during novelty check: {0}")]
    Embedding(#[from] EmbeddingError),
}
```

