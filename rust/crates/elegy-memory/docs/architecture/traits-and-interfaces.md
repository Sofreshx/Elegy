# Traits and Interfaces

This document mirrors the current Rust contracts in `src/traits.rs` and the key types they depend on.

## Core Traits

### `MemoryStore`

```rust
#[async_trait]
pub trait MemoryStore: Send + Sync {
    fn scope(&self) -> MemoryScope;

    async fn store(&self, memory: Memory) -> Result<MemoryId, StoreError>;

    async fn update_content(
        &self,
        id: &MemoryId,
        new_content: &str,
        changed_by: &str,
        reason: &str,
    ) -> Result<(), StoreError>;

    async fn update_metadata(
        &self,
        id: &MemoryId,
        updates: MetadataUpdate,
    ) -> Result<(), StoreError>;

    async fn get(&self, id: &MemoryId) -> Result<Option<Memory>, StoreError>;
    async fn get_raw(&self, id: &MemoryId) -> Result<Option<Memory>, StoreError>;
    async fn list(&self, filter: MemoryFilter) -> Result<Vec<Memory>, StoreError>;

    async fn search(&self, query: SearchQuery) -> Result<Vec<ScoredMemory>, StoreError>;

    async fn find_similar(
        &self,
        embedding: &[f32],
        threshold: f32,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>, StoreError>;

    async fn store_embedding(&self, id: &MemoryId, embedding: &[f32]) -> Result<(), StoreError>;
    async fn get_stale_embeddings(&self, limit: usize) -> Result<Vec<MemoryId>, StoreError>;

    async fn make_dormant(&self, id: &MemoryId) -> Result<(), StoreError>;
    async fn reactivate(&self, id: &MemoryId) -> Result<(), StoreError>;
    async fn hard_delete(&self, id: &MemoryId) -> Result<(), StoreError>;

    async fn purge_user(&self, user_id: &str) -> Result<PurgeReport, StoreError>;
    async fn purge_all(&self) -> Result<PurgeReport, StoreError>;

    async fn health_report(&self) -> Result<MemoryHealthReport, StoreError>;

    async fn list_contradictions(
        &self,
        status: Option<ResolutionStatus>,
    ) -> Result<Vec<ContradictionEntry>, StoreError>;

    async fn record_contradiction(
        &self,
        a_id: &MemoryId,
        b_id: &MemoryId,
        description: &str,
    ) -> Result<(), StoreError>;

    async fn update_contradiction_status(
        &self,
        contradiction_id: &str,
        status: ResolutionStatus,
        note: Option<&str>,
    ) -> Result<(), StoreError>;
}
```

Supporting request types used by the trait:

```rust
pub enum OptionalFieldUpdate<T> {
    Set(T),
    Clear,
}

pub struct MetadataUpdate {
    pub tags: Option<Vec<String>>,
    pub status: Option<OptionalFieldUpdate<String>>,
    pub custom_metadata: Option<HashMap<String, String>>,
    pub importance_score: Option<f32>,
    pub reliability_score: Option<f32>,
    pub state: Option<MemoryState>,
}

pub struct MemoryFilter {
    pub scope: Option<MemoryScope>,
    pub state: Option<MemoryState>,
    pub memory_types: Option<Vec<MemoryType>>,
    pub provenance_levels: Option<Vec<ProvenanceLevel>>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub agent_id: Option<String>,
    pub limit: Option<usize>,
}
```

### `EmbeddingProvider`

```rust
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }

    fn dimensions(&self) -> usize;
    fn model_id(&self) -> &str;
}
```

Current provider implementations in the crate:

- `OllamaEmbeddingProvider`
- `OpenAiEmbeddingProvider`

### `LlmProvider`

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, prompt: &str) -> Result<String, LlmError>;
    fn name(&self) -> &str;
    fn model(&self) -> &str;
}
```

Current provider implementations in the crate:

- `OllamaLlmProvider` (`/api/generate`, default model `qwen3:8b`)
- `OpenAiLlmProvider` (`/v1/chat/completions`, default model `gpt-4.1-mini`)

### `SalienceGate`

`SalienceGate::evaluate` is async and returns structured decisions that the CLI/store can act on directly.

```rust
#[async_trait]
pub trait SalienceGate: Send + Sync {
    async fn evaluate(
        &self,
        candidate: &MemoryCandidate,
        store: &dyn MemoryStore,
    ) -> Result<GateDecision, GateError>;
}

pub enum GateDecision {
    Accept {
        similar_to: Option<MemoryId>,
        similarity: Option<f32>,
    },
    Archive,
    Merge {
        target_id: MemoryId,
        enriched_content: String,
        promote_to: Option<MemoryScope>,
    },
    Contradiction {
        conflicting_id: MemoryId,
        description: String,
    },
    Reject {
        reason: String,
    },
}
```

`DefaultSalienceGate` currently uses scope-configured thresholds with conservative semantics plus scope-aware precedence:

- warn at `0.80`
- merge at `0.85`
- duplicate threshold constant `0.99`
- archive below salience `0.20`
- archive low-importance `AgentInferred` below `0.50`
- reject near-duplicates that already exist in a broader visible scope
- surface `promote_to` when a merge should lift the canonical result into a broader scope
- when an `LlmProvider` is configured, ask the LLM to classify high-similarity pairs as `AGREE`, `CONTRADICT`, or `UNRELATED` before falling back to the heuristic contradiction checks
- degrade visibly to the heuristic path when the LLM provider fails or returns an unusable verdict

### `MemoryConsolidator`

```rust
#[async_trait]
pub trait MemoryConsolidator: Send + Sync {
    async fn consolidate(
        &self,
        memories: &[ConsolidationCandidate],
    ) -> Result<Vec<ConsolidationAction>, ConsolidationError>;
}

pub enum ConsolidationAction {
    Merged {
        source_ids: Vec<MemoryId>,
        result: Memory,
    },
    Summarized {
        source_id: MemoryId,
        new_summary: String,
    },
    MadeDormant {
        id: MemoryId,
        reason: String,
    },
    Linked {
        source_id: MemoryId,
        target_id: MemoryId,
        relation: String,
    },
    Contradiction {
        memory_a_id: MemoryId,
        memory_b_id: MemoryId,
        description: String,
    },
}
```

`SimpleConsolidator` still provides the default implementation. It now supports:

- same-scope-only consolidation by default
- optional cross-scope consolidation, where merged results take the highest scope in the pair
- optional pair-limit capping for CLI `--consolidate-limit`

`LlmConsolidator` is now the optional Tier 2 implementation:

- candidate pairs are still selected by embedding cosine similarity
- each qualifying pair is sent to an `LlmProvider` with a constrained consolidation prompt
- `CONTRADICTION: ...` responses become `ConsolidationAction::Contradiction`
- empty / garbled / failed LLM calls visibly fall back to the same merge semantics as `SimpleConsolidator`

## Promotion Engine

The crate now exposes a lightweight `PromotionEngine` helper around `SqliteMemoryStore`:

- `run(store, limit, trigger_session_id)` evaluates automatic promotions
- `promote_to(store, id, to_scope, changed_by, reason, trigger_session_id)` applies a manual override

### `MemoryObservability`

```rust
pub trait MemoryObservability: Send + Sync {
    fn health_report(&self, scope: MemoryScope) -> Result<MemoryHealthReport, ObservabilityError>;

    fn list_contradictions(
        &self,
        status: Option<ResolutionStatus>,
    ) -> Result<Vec<ContradictionEntry>, ObservabilityError>;

    fn export_memories(
        &self,
        scope: MemoryScope,
        format: ExportFormat,
    ) -> Result<Vec<u8>, ObservabilityError>;

    fn purge_user(&self, user_id: &str) -> Result<PurgeReport, ObservabilityError>;
    fn purge_scope(&self, scope: MemoryScope) -> Result<PurgeReport, ObservabilityError>;
}
```

`MemoryObservability` is currently a **definition-only** trait with no concrete implementation. The equivalent functionality is available through the async `MemoryStore` methods (`health_report`, `list_contradictions`, `purge_user`, `purge_all`) and the CLI export command. Implementing this synchronous observability facade for external tooling is planned for a future milestone.

## Key Types

```rust
pub type MemoryId = uuid::Uuid;

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

pub struct ScoredMemory {
    pub memory: Memory,
    pub score: f32,
    pub similarity: f32,
}

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
```

`MemoryHealthReport` is the base store-facing type. The CLI now derives additional presentation fields on top of it, including average importance, stale-memory previews, contradiction previews, oldest age, most-accessed memory, and human-readable database size.

## Scope Configuration Surface

The store loads these tuning values from `scope_config`:

```rust
pub struct ScopeConfig {
    pub decay_lambda_base: f32,
    pub similarity_weight: f32,
    pub recency_weight: f32,
    pub access_weight: f32,
    pub priority_weight: f32,
    pub memory_context_ratio: f32,
    pub response_reserve: u32,
    pub salience_threshold: f32,
    pub novelty_doubt_threshold: f32,
    pub merge_similarity_threshold: f32,
    pub duplicate_similarity_threshold: f32,
    pub agent_inferred_importance_threshold: f32,
}
```

