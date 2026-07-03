use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{
    ConsolidationError, EmbeddingError, GateError, LlmError, ObservabilityError, StoreError,
};
use crate::types::{
    ConsolidationCandidate, ContradictionEntry, ExportFormat, Memory, MemoryCandidate,
    MemoryHealthReport, MemoryId, MemoryScope, MemoryState, MemoryType, ProvenanceLevel,
    PurgeReport, ResolutionStatus, ScoredMemory, SearchQuery,
};

/// Patch operation for an optional metadata field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OptionalFieldUpdate<T> {
    /// Replace the field with a new value.
    Set(T),
    /// Clear the field so it becomes `None`.
    Clear,
}

/// Partial metadata update applied by [`MemoryStore::update_metadata`].
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MetadataUpdate {
    /// Replacement tag list for the memory, if provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Replacement workflow or application status value, or a clear operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<OptionalFieldUpdate<String>>,
    /// Replacement custom metadata map for the memory, if provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_metadata: Option<HashMap<String, String>>,
    /// Replacement importance score in the inclusive range `0.0..=1.0`, if provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub importance_score: Option<f32>,
    /// Replacement reliability score in the inclusive range `0.0..=1.0`, if provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reliability_score: Option<f32>,
    /// Replacement lifecycle state, if provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<MemoryState>,
}

/// Filter used by [`MemoryStore::list`] to enumerate stored memories.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryFilter {
    /// Restrict results to a single scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    /// Restrict results to memories in a single lifecycle state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<MemoryState>,
    /// Restrict results to the listed semantic memory categories.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_types: Option<Vec<MemoryType>>,
    /// Restrict results to the listed provenance tiers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_levels: Option<Vec<ProvenanceLevel>>,
    /// Require all listed tags to be present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Restrict results to a specific workflow or application status value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Restrict results to a specific tenant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    /// Restrict results to a specific user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Restrict results to a specific agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Maximum number of results to return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

/// Outcome produced by a [`SalienceGate`] evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GateDecision {
    /// Store the candidate as an active memory.
    Accept {
        /// Similar existing memory surfaced as a likely duplicate, when applicable.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        similar_to: Option<MemoryId>,
        /// Similarity score associated with `similar_to`, when applicable.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        similarity: Option<f32>,
    },
    /// Store the candidate directly as a dormant memory.
    Archive,
    /// Merge the candidate into an existing memory instead of creating a new one.
    Merge {
        /// The existing memory to update.
        target_id: MemoryId,
        /// The merged content that should replace the target's current content.
        enriched_content: String,
        /// Optional broader scope to promote the merged result into.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        promote_to: Option<MemoryScope>,
    },
    /// Store the candidate independently and record a contradiction with an existing memory.
    Contradiction {
        /// The existing memory that conflicts with the candidate.
        conflicting_id: MemoryId,
        /// Human-readable explanation for the contradiction record.
        description: String,
    },
    /// Discard the candidate instead of storing it.
    Reject {
        /// Human-readable explanation for the rejection.
        reason: String,
    },
}

/// Change planned or executed during a consolidation pass.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ConsolidationAction {
    /// Several memories were merged into one resulting record.
    Merged {
        /// The source memories consumed by the merge.
        source_ids: Vec<MemoryId>,
        /// The resulting merged memory.
        result: Memory,
    },
    /// A memory received a new summary without replacing its full content.
    Summarized {
        /// The summarized memory.
        source_id: MemoryId,
        /// The new summary content.
        new_summary: String,
    },
    /// A memory was transitioned to dormant storage.
    MadeDormant {
        /// The memory that became dormant.
        id: MemoryId,
        /// Human-readable explanation for the state change.
        reason: String,
    },
    /// Two memories were linked with a semantic relationship.
    Linked {
        /// The source side of the relation.
        source_id: MemoryId,
        /// The target side of the relation.
        target_id: MemoryId,
        /// The relation label recorded between the memories.
        relation: String,
    },
    /// A candidate pair was classified as contradictory and should be journaled instead of merged.
    Contradiction {
        /// The first conflicting memory.
        memory_a_id: MemoryId,
        /// The second conflicting memory.
        memory_b_id: MemoryId,
        /// Human-readable explanation of the conflict.
        description: String,
    },
}

/// Primary storage contract for the core memory CRUD, search, lifecycle, and contradiction flows.
///
/// This trait intentionally defines the stable store-wide baseline surface used by the
/// gate, CLI, and other pluggable components. The current crate implements several
/// additional advanced capabilities directly on [`crate::SqliteMemoryStore`] that are
/// not yet promoted into public traits, including:
///
/// - correction application and correction-history queries
/// - retrieval-feedback learning and learned-weight reporting
/// - poisoning detection and quarantine remediation
/// - cross-agent share export and share-import review workflows
///
/// Treat `MemoryStore` as the core contract, not as the complete capability surface of
/// the current SQLite-backed implementation.
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Return the explicit write scope bound to this store instance.
    fn scope(&self) -> MemoryScope;

    /// Store a new memory that has already passed through the salience gate.
    async fn store(&self, memory: Memory) -> Result<MemoryId, StoreError>;

    /// Update an existing memory's content and create a versioned history entry.
    async fn update_content(
        &self,
        id: &MemoryId,
        new_content: &str,
        changed_by: &str,
        reason: &str,
    ) -> Result<(), StoreError>;

    /// Update mutable metadata fields without replacing the memory body.
    async fn update_metadata(
        &self,
        id: &MemoryId,
        updates: MetadataUpdate,
    ) -> Result<(), StoreError>;

    /// Get a single memory by ID and update its access tracking.
    async fn get(&self, id: &MemoryId) -> Result<Option<Memory>, StoreError>;

    /// Get a single memory by ID without updating access tracking.
    async fn get_raw(&self, id: &MemoryId) -> Result<Option<Memory>, StoreError>;

    /// List memories using the provided filter without updating access tracking.
    async fn list(&self, filter: MemoryFilter) -> Result<Vec<Memory>, StoreError>;

    /// Execute hybrid search and return ranked results.
    async fn search(&self, query: SearchQuery) -> Result<Vec<ScoredMemory>, StoreError>;

    /// Find memories similar to an embedding without updating access tracking.
    async fn find_similar(
        &self,
        embedding: &[f32],
        threshold: f32,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>, StoreError>;

    /// Store or replace the embedding associated with a memory.
    async fn store_embedding(&self, id: &MemoryId, embedding: &[f32]) -> Result<(), StoreError>;

    /// List memories that require embedding recomputation.
    async fn get_stale_embeddings(&self, limit: usize) -> Result<Vec<MemoryId>, StoreError>;

    /// Transition a memory from active retrieval to dormant storage.
    async fn make_dormant(&self, id: &MemoryId) -> Result<(), StoreError>;

    /// Reactivate a dormant memory so it participates in normal retrieval.
    async fn reactivate(&self, id: &MemoryId) -> Result<(), StoreError>;

    /// Permanently delete a memory and all related records.
    async fn hard_delete(&self, id: &MemoryId) -> Result<(), StoreError>;

    /// Delete all data associated with a user and report the removed records.
    async fn purge_user(&self, user_id: &str) -> Result<PurgeReport, StoreError>;

    /// Delete all data managed by this store.
    async fn purge_all(&self) -> Result<PurgeReport, StoreError>;

    /// Produce an operational health report for this store.
    async fn health_report(&self) -> Result<MemoryHealthReport, StoreError>;

    /// List contradictions recorded by the store, optionally filtered by status.
    async fn list_contradictions(
        &self,
        status: Option<ResolutionStatus>,
    ) -> Result<Vec<ContradictionEntry>, StoreError>;

    /// Record a contradiction between two existing memories.
    async fn record_contradiction(
        &self,
        a_id: &MemoryId,
        b_id: &MemoryId,
        description: &str,
    ) -> Result<(), StoreError>;

    /// Update the resolution status for an existing contradiction.
    async fn update_contradiction_status(
        &self,
        contradiction_id: &str,
        status: ResolutionStatus,
        note: Option<&str>,
    ) -> Result<(), StoreError>;
}

/// Provider contract for embedding models used by search and novelty checks.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate an embedding vector for the supplied text.
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;

    /// Generate embeddings for multiple texts, sequentially by default.
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }

    /// Return the dimensionality produced by this provider.
    fn dimensions(&self) -> usize;

    /// Return the provider model identifier.
    fn model_id(&self) -> &str;
}

/// Provider contract for text-generation models used during consolidation and contradiction checks.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Generate a completion for the supplied prompt.
    async fn complete(&self, prompt: &str) -> Result<String, LlmError>;

    /// Return the provider name used for logging and display.
    fn name(&self) -> &str;

    /// Return the active model identifier used for logging and display.
    fn model(&self) -> &str;
}

/// Write-time decision gate that protects the store from low-value or duplicate memories.
#[async_trait]
pub trait SalienceGate: Send + Sync {
    /// Evaluate a candidate memory and decide whether it should be accepted, archived, merged, or rejected.
    async fn evaluate(
        &self,
        candidate: &MemoryCandidate,
        store: &dyn MemoryStore,
    ) -> Result<GateDecision, GateError>;
}

/// Periodic consolidation contract for deduplication, summarization, and linking.
#[async_trait]
pub trait MemoryConsolidator: Send + Sync {
    /// Consolidate a batch of memories and return the actions taken or proposed.
    async fn consolidate(
        &self,
        memories: &[ConsolidationCandidate],
    ) -> Result<Vec<ConsolidationAction>, ConsolidationError>;
}

/// Read-only observability and export contract for external tooling.
pub trait MemoryObservability: Send + Sync {
    /// Produce a health report for a single scope.
    fn health_report(&self, scope: MemoryScope) -> Result<MemoryHealthReport, ObservabilityError>;

    /// List contradictions visible through the observability surface.
    fn list_contradictions(
        &self,
        status: Option<ResolutionStatus>,
    ) -> Result<Vec<ContradictionEntry>, ObservabilityError>;

    /// Export memories for a scope in the requested format.
    fn export_memories(
        &self,
        scope: MemoryScope,
        format: ExportFormat,
    ) -> Result<Vec<u8>, ObservabilityError>;

    /// Purge all data for a specific user through the observability surface.
    fn purge_user(&self, user_id: &str) -> Result<PurgeReport, ObservabilityError>;

    /// Purge all data for a specific scope through the observability surface.
    fn purge_scope(&self, scope: MemoryScope) -> Result<PurgeReport, ObservabilityError>;
}
