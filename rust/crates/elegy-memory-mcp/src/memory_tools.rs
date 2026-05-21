use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use elegy_memory::{
    CorrectionDisposition, CorrectionRecord, DefaultSalienceGate, EmbeddingProvider, GateDecision,
    GateError, Memory, MemoryCandidate, MemoryFilter, MemoryScope, MemoryState, MemoryStore,
    MemoryType, ProvenanceLevel, ResolutionStatus, SalienceGate, ScoredMemory, SearchQuery,
    SensitivityLevel, SqliteMemoryStore, StoreError,
};
use rmcp::model::JsonObject;
use rmcp::schemars;
use rmcp::ErrorData;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

pub const DEFAULT_NAMESPACE: &str = "claude-ai-remote";
pub const DEFAULT_AGENT_ID: &str = DEFAULT_NAMESPACE;
pub const SCOPE_OVERRIDE_ERROR_MESSAGE: &str =
    "scope override not permitted — this connector is pinned to a configured agent namespace";

const DEFAULT_SEARCH_LIMIT: usize = 10;
const DEFAULT_LIST_LIMIT: usize = 20;
const DEFAULT_STORE_IMPORTANCE: f32 = 0.5;
const PREVIEW_LIMIT: usize = 140;

#[derive(Clone)]
pub struct MemoryRepository {
    store: SqliteMemoryStore,
    binding: MemoryBinding,
    embedding_mode: RepositoryEmbeddingMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepositoryEmbeddingMode {
    ProviderBacked,
    NoProvider,
}

impl MemoryRepository {
    pub fn new(path: impl AsRef<Path>, binding: MemoryBinding) -> Result<Self, StoreError> {
        Self::new_with_optional_embedding_provider(path, binding, None)
    }

    pub fn new_with_embedding_provider(
        path: impl AsRef<Path>,
        binding: MemoryBinding,
        embedding_provider: Arc<dyn EmbeddingProvider>,
    ) -> Result<Self, StoreError> {
        Self::new_with_optional_embedding_provider(path, binding, Some(embedding_provider))
    }

    fn new_with_optional_embedding_provider(
        path: impl AsRef<Path>,
        binding: MemoryBinding,
        embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
    ) -> Result<Self, StoreError> {
        Ok(Self {
            store: SqliteMemoryStore::new_with_optional_embedding_provider(
                path,
                MemoryScope::Agent,
                embedding_provider.clone(),
            )?,
            binding,
            embedding_mode: if embedding_provider.is_some() {
                RepositoryEmbeddingMode::ProviderBacked
            } else {
                RepositoryEmbeddingMode::NoProvider
            },
        })
    }

    pub fn namespace(&self) -> &str {
        self.binding.namespace()
    }

    pub fn agent_id(&self) -> &str {
        self.binding.agent_id()
    }

    fn namespace_owned(&self) -> String {
        self.namespace().to_string()
    }

    fn agent_id_owned(&self) -> String {
        self.agent_id().to_string()
    }

    fn embedding_status(&self, memory: &Memory) -> EmbeddingStatus {
        if !memory.embedding_stale {
            EmbeddingStatus::Ready
        } else {
            match self.embedding_mode {
                RepositoryEmbeddingMode::ProviderBacked => EmbeddingStatus::Failed,
                RepositoryEmbeddingMode::NoProvider => EmbeddingStatus::SkippedNoProvider,
            }
        }
    }

    pub(crate) async fn search(
        &self,
        args: &MemorySearchArgs,
    ) -> Result<Vec<MemorySearchMatch>, StoreError> {
        let query = args.query.trim();
        if query.is_empty() {
            return Err(StoreError::Validation(
                "query must not be empty".to_string(),
            ));
        }
        if args.limit() == 0 {
            return Ok(Vec::new());
        }

        let matches = self
            .store
            .search(SearchQuery {
                text: query.to_string(),
                embedding: None,
                scope: MemoryScope::Agent,
                state_filter: args.state_filter(),
                type_filter: args.memory_types.clone().map(tool_memory_types),
                max_results: args.limit(),
                context_config: None,
                session_id: None,
                agent_id: Some(self.agent_id_owned()),
            })
            .await?;

        Ok(matches
            .into_iter()
            .filter(|result| self.is_visible_memory(&result.memory))
            .map(
                |ScoredMemory {
                     memory,
                     score,
                     similarity,
                 }| {
                    MemorySearchMatch {
                        memory,
                        score,
                        similarity,
                    }
                },
            )
            .collect())
    }

    pub(crate) async fn recall(&self, id: &str) -> Result<Option<Memory>, StoreError> {
        let parsed_id = Uuid::parse_str(id).map_err(|error| {
            StoreError::Validation(format!("invalid memory id `{id}`: {error}"))
        })?;
        let Some(memory) = self.store.get_raw(&parsed_id).await? else {
            return Ok(None);
        };
        if self.is_visible_memory(&memory) {
            Ok(Some(memory))
        } else {
            Ok(None)
        }
    }

    pub(crate) async fn list(&self, args: &MemoryListArgs) -> Result<Vec<Memory>, StoreError> {
        self.list_visible_memories(
            args.state_filter(),
            args.memory_types.clone().map(tool_memory_types),
            Some(args.limit()),
        )
        .await
    }

    pub(crate) async fn stats(&self) -> Result<MemoryStatsSnapshot, StoreError> {
        let active_memories = self
            .list_visible_memories(Some(MemoryState::Active), None, None)
            .await?;
        let visible_memories = self.list_visible_memories(None, None, None).await?;
        let contradictions = self
            .store
            .list_contradictions(Some(ResolutionStatus::Unresolved))
            .await?;

        let mut unresolved_contradictions = 0_u64;
        for contradiction in contradictions {
            let memory_a = self.store.get_raw(&contradiction.memory_a_id).await?;
            let memory_b = self.store.get_raw(&contradiction.memory_b_id).await?;
            if memory_a
                .as_ref()
                .is_some_and(|memory| self.is_visible_memory(memory))
                && memory_b
                    .as_ref()
                    .is_some_and(|memory| self.is_visible_memory(memory))
            {
                unresolved_contradictions += 1;
            }
        }

        let mut type_counts = BTreeMap::new();
        for memory in &visible_memories {
            *type_counts
                .entry(display_memory_type(memory.memory_type))
                .or_insert(0) += 1;
        }

        Ok(MemoryStatsSnapshot {
            total_count: visible_memories.len() as u64,
            active_count: active_memories.len() as u64,
            dormant_count: visible_memories
                .iter()
                .filter(|memory| memory.state == MemoryState::Dormant)
                .count() as u64,
            stale_embeddings_count: visible_memories
                .iter()
                .filter(|memory| memory.embedding_stale)
                .count() as u64,
            unresolved_contradictions,
            oldest_active_memory: active_memories
                .iter()
                .map(|memory| memory.created_at)
                .min()
                .map(|value| value.to_rfc3339()),
            newest_memory: visible_memories
                .iter()
                .map(|memory| memory.created_at)
                .max()
                .map(|value| value.to_rfc3339()),
            type_counts,
        })
    }

    pub(crate) async fn store_memory(
        &self,
        args: &MemoryStoreArgs,
    ) -> Result<MemoryStoreResponse, StoreError> {
        let candidate = args.to_candidate()?;
        let gate = DefaultSalienceGate::new(self.store.scope_config()?);
        let decision = gate
            .evaluate(&candidate, &self.store)
            .await
            .map_err(map_gate_error)?;

        match decision {
            GateDecision::Merge {
                target_id,
                enriched_content,
                promote_to: _,
            } => {
                self.store
                    .update_content(
                        &target_id,
                        &enriched_content,
                        "mcp:memory_store",
                        "merged by salience gate from MCP memory_store",
                    )
                    .await?;
                let memory = self.require_visible_memory(&target_id).await?;
                Ok(MemoryStoreResponse::new(
                    self,
                    "merged",
                    "merge".to_string(),
                    memory,
                ))
            }
            GateDecision::Contradiction {
                conflicting_id,
                description,
            } => {
                let memory = self.build_memory_from_candidate(&candidate, MemoryState::Active);
                let id = memory.id;
                self.store.store(memory).await?;
                if let Err(error) = self
                    .store
                    .record_contradiction(&conflicting_id, &id, &description)
                    .await
                {
                    let _ = self.store.hard_delete(&id).await;
                    return Err(error);
                }
                let memory = self.require_visible_memory(&id).await?;
                Ok(MemoryStoreResponse::new(
                    self,
                    "added",
                    format_contradiction_gate_result(conflicting_id),
                    memory,
                ))
            }
            GateDecision::Archive => {
                let memory = self.build_memory_from_candidate(&candidate, MemoryState::Dormant);
                let id = memory.id;
                self.store.store(memory).await?;
                let memory = self.require_visible_memory(&id).await?;
                Ok(MemoryStoreResponse::new(
                    self,
                    "added",
                    "archived".to_string(),
                    memory,
                ))
            }
            GateDecision::Accept {
                similar_to,
                similarity,
            } => {
                let memory = self.build_memory_from_candidate(&candidate, MemoryState::Active);
                let id = memory.id;
                self.store.store(memory).await?;
                let memory = self.require_visible_memory(&id).await?;
                Ok(MemoryStoreResponse::new(
                    self,
                    "added",
                    format_gate_result(similar_to, similarity),
                    memory,
                ))
            }
            GateDecision::Reject { reason } => Err(StoreError::Validation(format!(
                "memory rejected by safety gate: {reason}"
            ))),
        }
    }

    pub(crate) async fn update_memory(
        &self,
        args: &MemoryUpdateArgs,
    ) -> Result<MemoryUpdateResponse, StoreError> {
        let id = parse_memory_id(&args.id)?;
        let _existing = self.require_visible_memory(&id).await?;
        let content = require_non_empty_text("content", &args.content)?;
        let reason = normalized_reason(args.reason.as_deref(), "updated via MCP memory_update");
        self.store
            .update_content(&id, content, "mcp:memory_update", &reason)
            .await?;
        let memory = self.require_visible_memory(&id).await?;
        Ok(MemoryUpdateResponse {
            namespace: self.namespace_owned(),
            updated: true,
            memory: MemoryDetail::from(memory),
        })
    }

    pub(crate) async fn delete_memory(
        &self,
        args: &MemoryDeleteArgs,
    ) -> Result<MemoryDeleteResponse, StoreError> {
        let id = parse_memory_id(&args.id)?;
        let _existing = self.require_visible_memory(&id).await?;
        self.store.hard_delete(&id).await?;
        Ok(MemoryDeleteResponse {
            namespace: self.namespace_owned(),
            id: id.to_string(),
            deleted: true,
        })
    }

    pub(crate) async fn correct_memory(
        &self,
        args: &MemoryCorrectArgs,
    ) -> Result<MemoryCorrectResponse, StoreError> {
        let id = parse_memory_id(&args.id)?;
        let existing = self.require_visible_memory(&id).await?;
        let content = require_non_empty_text("content", &args.content)?;
        if existing.content.trim() == content {
            return Err(StoreError::Validation(
                "corrected content must differ from the current memory content".to_string(),
            ));
        }
        let reason = args
            .reason
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let correction = self
            .store
            .correct_memory(&id, content, "mcp:memory_correct", reason)?;
        let memory = self.require_visible_memory(&id).await?;
        Ok(MemoryCorrectResponse {
            namespace: self.namespace_owned(),
            correction: MemoryCorrectionSummary::from(correction),
            memory: MemoryDetail::from(memory),
        })
    }

    async fn list_visible_memories(
        &self,
        state: Option<MemoryState>,
        memory_types: Option<Vec<MemoryType>>,
        limit: Option<usize>,
    ) -> Result<Vec<Memory>, StoreError> {
        let mut memories = self
            .store
            .list(MemoryFilter {
                scope: Some(MemoryScope::Agent),
                state,
                memory_types,
                provenance_levels: None,
                tags: None,
                status: None,
                tenant_id: None,
                user_id: None,
                agent_id: Some(self.agent_id_owned()),
                limit,
            })
            .await?;
        memories.retain(|memory| self.is_visible_memory(memory));
        Ok(memories)
    }

    fn is_visible_memory(&self, memory: &Memory) -> bool {
        memory.scope == MemoryScope::Agent
            && memory.agent_id.as_deref() == Some(self.agent_id())
            && memory.state != MemoryState::Deleted
    }

    fn build_memory_from_candidate(
        &self,
        candidate: &MemoryCandidate,
        state: MemoryState,
    ) -> Memory {
        let now_rfc3339 = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| panic!("current timestamp should format"));
        let now = now_rfc3339
            .parse()
            .unwrap_or_else(|_| panic!("current timestamp should parse"));
        Memory {
            id: Uuid::new_v4(),
            content: candidate.content.clone(),
            summary: candidate.summary.clone(),
            scope: MemoryScope::Agent,
            memory_type: candidate.memory_type,
            provenance: candidate.provenance,
            importance_score: candidate.importance_score,
            reliability_score: candidate.provenance.base_reliability(),
            sensitivity: candidate.sensitivity,
            state,
            tags: candidate.tags.clone(),
            status: None,
            custom_metadata: candidate.custom_metadata.clone(),
            access_count: 0,
            corroboration_count: 0,
            embedding_stale: true,
            created_at: now,
            updated_at: now,
            last_accessed_at: None,
            tenant_id: None,
            user_id: None,
            agent_id: Some(self.agent_id_owned()),
        }
    }

    async fn require_visible_memory(&self, id: &Uuid) -> Result<Memory, StoreError> {
        let memory = self
            .store
            .get_raw(id)
            .await?
            .ok_or(StoreError::NotFound(*id))?;
        if self.is_visible_memory(&memory) {
            Ok(memory)
        } else {
            Err(StoreError::NotFound(*id))
        }
    }
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryStoreArgs {
    pub(crate) content: String,
    #[serde(default)]
    pub(crate) summary: Option<String>,
    #[serde(default)]
    pub(crate) memory_type: Option<ToolMemoryType>,
    #[serde(default)]
    pub(crate) importance: Option<f32>,
    #[serde(default)]
    pub(crate) provenance: Option<ToolProvenance>,
    #[serde(default)]
    pub(crate) sensitivity: Option<ToolSensitivity>,
    #[serde(default)]
    pub(crate) tags: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) custom_metadata: Option<BTreeMap<String, String>>,
}

impl MemoryStoreArgs {
    fn to_candidate(&self) -> Result<MemoryCandidate, StoreError> {
        Ok(MemoryCandidate {
            content: require_non_empty_text("content", &self.content)?.to_string(),
            summary: normalized_optional_text(self.summary.as_deref()),
            memory_type: self
                .memory_type
                .unwrap_or_else(default_store_memory_type)
                .into(),
            provenance: self
                .provenance
                .unwrap_or_else(default_store_provenance)
                .into(),
            importance_score: validate_importance(
                self.importance.unwrap_or_else(default_store_importance),
            )?,
            sensitivity: self
                .sensitivity
                .unwrap_or_else(default_store_sensitivity)
                .into(),
            tags: self
                .tags
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .filter_map(|tag| normalized_optional_text(Some(tag.as_str())))
                .collect(),
            custom_metadata: self
                .custom_metadata
                .clone()
                .unwrap_or_default()
                .into_iter()
                .collect(),
            embedding: None,
        })
    }
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryUpdateArgs {
    pub(crate) id: String,
    pub(crate) content: String,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryCorrectArgs {
    pub(crate) id: String,
    pub(crate) content: String,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryDeleteArgs {
    pub(crate) id: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemorySearchArgs {
    pub(crate) query: String,
    #[serde(default)]
    pub(crate) limit: Option<usize>,
    #[serde(default)]
    pub(crate) include_dormant: Option<bool>,
    #[serde(default)]
    pub(crate) memory_types: Option<Vec<ToolMemoryType>>,
}

impl MemorySearchArgs {
    fn limit(&self) -> usize {
        self.limit.unwrap_or_else(default_search_limit)
    }

    fn include_dormant(&self) -> bool {
        self.include_dormant.unwrap_or(false)
    }

    fn state_filter(&self) -> Option<MemoryState> {
        if self.include_dormant() {
            None
        } else {
            Some(MemoryState::Active)
        }
    }
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryRecallArgs {
    pub(crate) id: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryListArgs {
    #[serde(default)]
    pub(crate) limit: Option<usize>,
    #[serde(default)]
    pub(crate) include_dormant: Option<bool>,
    #[serde(default)]
    pub(crate) state: Option<ToolMemoryState>,
    #[serde(default)]
    pub(crate) memory_types: Option<Vec<ToolMemoryType>>,
}

impl MemoryListArgs {
    fn limit(&self) -> usize {
        self.limit.unwrap_or_else(default_list_limit)
    }

    fn include_dormant(&self) -> bool {
        self.include_dormant.unwrap_or(false)
    }

    fn state_filter(&self) -> Option<MemoryState> {
        if let Some(state) = self.state {
            Some(state.into())
        } else if self.include_dormant() {
            None
        } else {
            Some(MemoryState::Active)
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryStatsArgs {}

#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ToolMemoryState {
    Active,
    Dormant,
}

impl From<ToolMemoryState> for MemoryState {
    fn from(value: ToolMemoryState) -> Self {
        match value {
            ToolMemoryState::Active => Self::Active,
            ToolMemoryState::Dormant => Self::Dormant,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ToolMemoryType {
    Fact,
    Preference,
    Decision,
    Procedure,
    Observation,
}

impl From<ToolMemoryType> for MemoryType {
    fn from(value: ToolMemoryType) -> Self {
        match value {
            ToolMemoryType::Fact => Self::Fact,
            ToolMemoryType::Preference => Self::Preference,
            ToolMemoryType::Decision => Self::Decision,
            ToolMemoryType::Procedure => Self::Procedure,
            ToolMemoryType::Observation => Self::Observation,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ToolProvenance {
    UserStated,
    AgentObserved,
    Consolidated,
    Imported,
    AgentInferred,
}

impl From<ToolProvenance> for ProvenanceLevel {
    fn from(value: ToolProvenance) -> Self {
        match value {
            ToolProvenance::UserStated => Self::UserStated,
            ToolProvenance::AgentObserved => Self::AgentObserved,
            ToolProvenance::Consolidated => Self::Consolidated,
            ToolProvenance::Imported => Self::Imported,
            ToolProvenance::AgentInferred => Self::AgentInferred,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ToolSensitivity {
    Low,
    Medium,
    High,
    Critical,
}

impl From<ToolSensitivity> for SensitivityLevel {
    fn from(value: ToolSensitivity) -> Self {
        match value {
            ToolSensitivity::Low => Self::Low,
            ToolSensitivity::Medium => Self::Medium,
            ToolSensitivity::High => Self::High,
            ToolSensitivity::Critical => Self::Critical,
        }
    }
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchResponse {
    pub namespace: String,
    pub count: usize,
    pub query: String,
    pub include_dormant: bool,
    pub results: Vec<SearchResultRow>,
}

impl MemorySearchResponse {
    pub fn new(
        repository: &MemoryRepository,
        args: &MemorySearchArgs,
        matches: Vec<MemorySearchMatch>,
    ) -> Self {
        Self {
            namespace: repository.namespace_owned(),
            count: matches.len(),
            query: args.query.clone(),
            include_dormant: args.include_dormant(),
            results: matches.into_iter().map(SearchResultRow::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultRow {
    pub id: String,
    pub score: f32,
    pub similarity: f32,
    pub state: String,
    pub memory_type: String,
    pub preview: String,
}

impl From<MemorySearchMatch> for SearchResultRow {
    fn from(value: MemorySearchMatch) -> Self {
        Self {
            id: value.memory.id.to_string(),
            score: value.score,
            similarity: value.similarity,
            state: display_memory_state(value.memory.state),
            memory_type: display_memory_type(value.memory.memory_type),
            preview: preview(&value.memory.content),
        }
    }
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRecallResponse {
    pub namespace: String,
    pub found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryDetail>,
}

impl MemoryRecallResponse {
    pub fn from_memory(repository: &MemoryRepository, memory: Option<Memory>) -> Self {
        Self {
            namespace: repository.namespace_owned(),
            found: memory.is_some(),
            memory: memory.map(MemoryDetail::from),
        }
    }
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryListResponse {
    pub namespace: String,
    pub count: usize,
    pub include_dormant: bool,
    pub memories: Vec<ListRow>,
}

impl MemoryListResponse {
    pub fn new(
        repository: &MemoryRepository,
        args: &MemoryListArgs,
        memories: Vec<Memory>,
    ) -> Self {
        Self {
            namespace: repository.namespace_owned(),
            count: memories.len(),
            include_dormant: args.include_dormant(),
            memories: memories.into_iter().map(ListRow::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListRow {
    pub id: String,
    pub state: String,
    pub memory_type: String,
    pub provenance: String,
    pub importance: f32,
    pub updated_at: String,
    pub preview: String,
}

impl From<Memory> for ListRow {
    fn from(value: Memory) -> Self {
        Self {
            id: value.id.to_string(),
            state: display_memory_state(value.state),
            memory_type: display_memory_type(value.memory_type),
            provenance: display_provenance(value.provenance),
            importance: value.importance_score,
            updated_at: value.updated_at.to_rfc3339(),
            preview: preview(&value.content),
        }
    }
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDetail {
    pub id: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub state: String,
    pub memory_type: String,
    pub provenance: String,
    pub importance: f32,
    pub reliability: f32,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_accessed_at: Option<String>,
}

impl From<Memory> for MemoryDetail {
    fn from(value: Memory) -> Self {
        Self {
            id: value.id.to_string(),
            content: value.content,
            summary: value.summary,
            state: display_memory_state(value.state),
            memory_type: display_memory_type(value.memory_type),
            provenance: display_provenance(value.provenance),
            importance: value.importance_score,
            reliability: value.reliability_score,
            tags: value.tags,
            status: value.status,
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
            last_accessed_at: value.last_accessed_at.map(|value| value.to_rfc3339()),
        }
    }
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStatsResponse {
    pub namespace: String,
    pub scope: &'static str,
    pub agent_id: String,
    pub total_count: u64,
    pub active_count: u64,
    pub dormant_count: u64,
    pub stale_embeddings_count: u64,
    pub unresolved_contradictions: u64,
    pub type_counts: BTreeMap<String, u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_active_memory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newest_memory: Option<String>,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStoreResponse {
    pub namespace: String,
    pub action: &'static str,
    pub gate_result: String,
    pub embedding_status: EmbeddingStatus,
    pub memory: MemoryDetail,
}

impl MemoryStoreResponse {
    fn new(
        repository: &MemoryRepository,
        action: &'static str,
        gate_result: String,
        memory: Memory,
    ) -> Self {
        let embedding_status = repository.embedding_status(&memory);
        Self {
            namespace: repository.namespace_owned(),
            action,
            gate_result,
            embedding_status,
            memory: MemoryDetail::from(memory),
        }
    }
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryUpdateResponse {
    pub namespace: String,
    pub updated: bool,
    pub memory: MemoryDetail,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCorrectResponse {
    pub namespace: String,
    pub correction: MemoryCorrectionSummary,
    pub memory: MemoryDetail,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCorrectionSummary {
    pub id: String,
    pub memory_id: String,
    pub disposition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_memory_id: Option<String>,
    pub corrected_at: String,
}

impl From<CorrectionRecord> for MemoryCorrectionSummary {
    fn from(value: CorrectionRecord) -> Self {
        Self {
            id: value.id,
            memory_id: value.memory_id.to_string(),
            disposition: display_correction_disposition(value.disposition),
            related_memory_id: value.related_memory_id.map(|id| id.to_string()),
            corrected_at: value.corrected_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDeleteResponse {
    pub namespace: String,
    pub id: String,
    pub deleted: bool,
}

/// Reports whether the stored memory has a usable embedding immediately after the write path.
#[derive(Debug, Clone, Copy, Serialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingStatus {
    Ready,
    Failed,
    SkippedNoProvider,
}

impl MemoryStatsResponse {
    pub fn from_repository(repository: &MemoryRepository, value: MemoryStatsSnapshot) -> Self {
        Self {
            namespace: repository.namespace_owned(),
            scope: "agent",
            agent_id: repository.agent_id_owned(),
            total_count: value.total_count,
            active_count: value.active_count,
            dormant_count: value.dormant_count,
            stale_embeddings_count: value.stale_embeddings_count,
            unresolved_contradictions: value.unresolved_contradictions,
            type_counts: value.type_counts,
            oldest_active_memory: value.oldest_active_memory,
            newest_memory: value.newest_memory,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemorySearchMatch {
    pub memory: Memory,
    pub score: f32,
    pub similarity: f32,
}

#[derive(Debug, Clone)]
pub struct MemoryStatsSnapshot {
    pub total_count: u64,
    pub active_count: u64,
    pub dormant_count: u64,
    pub stale_embeddings_count: u64,
    pub unresolved_contradictions: u64,
    pub oldest_active_memory: Option<String>,
    pub newest_memory: Option<String>,
    pub type_counts: BTreeMap<String, u64>,
}

pub(crate) fn parse_tool_arguments<T: DeserializeOwned>(
    raw_arguments: JsonObject,
) -> Result<T, ErrorData> {
    let value = Value::Object(raw_arguments);
    reject_scope_overrides(&value)?;
    serde_json::from_value(value).map_err(|error| {
        ErrorData::invalid_params(format!("failed to deserialize parameters: {error}"), None)
    })
}

pub(crate) fn map_store_error(error: StoreError) -> ErrorData {
    match error {
        StoreError::NotFound(id) => ErrorData::invalid_params(
            format!("memory `{id}` was not found in the configured agent namespace"),
            None,
        ),
        StoreError::Validation(message) => ErrorData::invalid_params(message, None),
        other => ErrorData::internal_error(other.to_string(), None),
    }
}

fn reject_scope_overrides(value: &Value) -> Result<(), ErrorData> {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if is_forbidden_override_key(key) {
                    return Err(ErrorData::invalid_params(
                        SCOPE_OVERRIDE_ERROR_MESSAGE,
                        None,
                    ));
                }
                reject_scope_overrides(value)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for item in items {
                reject_scope_overrides(item)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn is_forbidden_override_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    normalized.contains("scope")
        || normalized.contains("namespace")
        || matches!(normalized.as_str(), "agent" | "agentid")
}

fn default_search_limit() -> usize {
    DEFAULT_SEARCH_LIMIT
}

fn default_list_limit() -> usize {
    DEFAULT_LIST_LIMIT
}

fn default_store_importance() -> f32 {
    DEFAULT_STORE_IMPORTANCE
}

fn default_store_memory_type() -> ToolMemoryType {
    ToolMemoryType::Observation
}

fn default_store_provenance() -> ToolProvenance {
    ToolProvenance::UserStated
}

fn default_store_sensitivity() -> ToolSensitivity {
    ToolSensitivity::Low
}

fn tool_memory_types(memory_types: Vec<ToolMemoryType>) -> Vec<MemoryType> {
    memory_types.into_iter().map(Into::into).collect()
}

fn preview(content: &str) -> String {
    let mut preview = String::new();
    let mut characters = content.chars().peekable();

    while preview.chars().count() < PREVIEW_LIMIT {
        let Some(character) = characters.next() else {
            return preview;
        };
        preview.push(character);
    }

    if characters.peek().is_some() {
        preview.push('…');
    }

    preview
}

fn display_memory_state(state: MemoryState) -> String {
    match state {
        MemoryState::Active => "active",
        MemoryState::Dormant => "dormant",
        MemoryState::Deleted => "deleted",
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

fn format_gate_result(similar_to: Option<Uuid>, similarity: Option<f32>) -> String {
    match (similar_to, similarity) {
        (Some(memory_id), Some(similarity)) => {
            format!("accepted (similar to {memory_id}, cosine={similarity:.3})")
        }
        (Some(memory_id), None) => format!("accepted (similar to {memory_id})"),
        _ => "accepted".to_string(),
    }
}

fn format_contradiction_gate_result(conflicting_id: Uuid) -> String {
    format!("contradiction (conflicts with {conflicting_id})")
}

fn parse_memory_id(id: &str) -> Result<Uuid, StoreError> {
    Uuid::parse_str(id)
        .map_err(|error| StoreError::Validation(format!("invalid memory id `{id}`: {error}")))
}

fn require_non_empty_text<'a>(field: &str, value: &'a str) -> Result<&'a str, StoreError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(StoreError::Validation(format!("{field} must not be empty")))
    } else {
        Ok(trimmed)
    }
}

fn normalized_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn normalized_reason(value: Option<&str>, fallback: &str) -> String {
    normalized_optional_text(value).unwrap_or_else(|| fallback.to_string())
}

fn validate_importance(value: f32) -> Result<f32, StoreError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(StoreError::Validation(
            "importance must be finite and within 0.0..=1.0".to_string(),
        ))
    }
}

fn map_gate_error(error: GateError) -> StoreError {
    match error {
        GateError::Store(error) => error,
        GateError::InvalidCandidate(message) => StoreError::Validation(message),
        GateError::Embedding(error) => {
            StoreError::Validation(format!("gate embedding evaluation failed: {error}"))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryBinding {
    namespace: String,
    agent_id: String,
}

impl Default for MemoryBinding {
    fn default() -> Self {
        Self {
            namespace: DEFAULT_NAMESPACE.to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
        }
    }
}

impl MemoryBinding {
    pub fn new(
        namespace: impl Into<String>,
        agent_id: impl Into<String>,
    ) -> Result<Self, MemoryBindingError> {
        let namespace = namespace.into();
        let agent_id = agent_id.into();

        if namespace.trim().is_empty() {
            return Err(MemoryBindingError::EmptyNamespace);
        }
        if agent_id.trim().is_empty() {
            return Err(MemoryBindingError::EmptyAgentId);
        }

        Ok(Self {
            namespace: namespace.trim().to_string(),
            agent_id: agent_id.trim().to_string(),
        })
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MemoryBindingError {
    #[error("memory namespace must not be empty")]
    EmptyNamespace,
    #[error("memory agent_id must not be empty")]
    EmptyAgentId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, sync::Mutex};

    use async_trait::async_trait;
    use elegy_memory::{EmbeddingError, EmbeddingProvider};
    use tempfile::TempDir;

    #[tokio::test]
    async fn store_memory_clears_stale_embeddings_when_provider_succeeds() {
        let temp_dir = TempDir::new().expect("tempdir should create");
        let db_path = temp_dir.path().join("memory.db");
        let content = "Store-time embeddings should be fresh immediately.";
        let provider = Arc::new(StubEmbeddingProvider::new([(content, axis_embedding())]));
        let repository = test_repository(&db_path, "test-agent", provider.clone());

        let response = repository
            .store_memory(&MemoryStoreArgs {
                content: content.to_string(),
                summary: None,
                memory_type: None,
                importance: Some(0.7),
                provenance: None,
                sensitivity: None,
                tags: None,
                custom_metadata: None,
            })
            .await
            .expect("memory store should succeed");
        let stats = repository.stats().await.expect("stats should load");

        assert_eq!(response.action, "added");
        assert_eq!(response.embedding_status, EmbeddingStatus::Ready);
        assert_eq!(stats.stale_embeddings_count, 0);
        assert_eq!(provider.calls(), vec![content.to_string()]);
    }

    #[tokio::test]
    async fn store_memory_reports_failed_embeddings_when_provider_errors() {
        let temp_dir = TempDir::new().expect("tempdir should create");
        let db_path = temp_dir.path().join("memory.db");
        let repository = test_repository(
            &db_path,
            "failed-embedding-agent",
            Arc::new(StubEmbeddingProvider::new(std::iter::empty::<(&str, Vec<f32>)>())),
        );

        let response = repository
            .store_memory(&MemoryStoreArgs {
                content: "Provider failures should surface as failed embedding status.".to_string(),
                summary: None,
                memory_type: None,
                importance: Some(0.7),
                provenance: None,
                sensitivity: None,
                tags: None,
                custom_metadata: None,
            })
            .await
            .expect("memory store should still succeed when embeddings fail");
        let stats = repository.stats().await.expect("stats should load");

        assert_eq!(response.action, "added");
        assert_eq!(response.embedding_status, EmbeddingStatus::Failed);
        assert_eq!(stats.stale_embeddings_count, 1);
    }

    #[tokio::test]
    async fn store_memory_reports_skipped_no_provider_without_embedding_provider() {
        let temp_dir = TempDir::new().expect("tempdir should create");
        let db_path = temp_dir.path().join("memory.db");
        let repository = MemoryRepository::new(
            &db_path,
            MemoryBinding::new(DEFAULT_NAMESPACE, "no-provider-agent")
                .expect("binding should build"),
        )
        .expect("repository should build");

        let response = repository
            .store_memory(&MemoryStoreArgs {
                content: "No provider should surface skipped embedding status.".to_string(),
                summary: None,
                memory_type: None,
                importance: Some(0.7),
                provenance: None,
                sensitivity: None,
                tags: None,
                custom_metadata: None,
            })
            .await
            .expect("memory store should succeed without provider");
        let stats = repository.stats().await.expect("stats should load");

        assert_eq!(response.action, "added");
        assert_eq!(response.embedding_status, EmbeddingStatus::SkippedNoProvider);
        assert_eq!(stats.stale_embeddings_count, 1);
    }

    #[tokio::test]
    async fn semantic_search_recalls_concept_only_matches() {
        let temp_dir = TempDir::new().expect("tempdir should create");
        let db_path = temp_dir.path().join("memory.db");
        let content = "Arabica espresso with chocolate finish.";
        let query = "fragrant hot drink";
        let provider = Arc::new(StubEmbeddingProvider::new([
            (content, axis_embedding()),
            (query, axis_embedding()),
        ]));
        let repository = test_repository(&db_path, "semantic-agent", provider);

        let stored = repository
            .store_memory(&MemoryStoreArgs {
                content: content.to_string(),
                summary: None,
                memory_type: Some(ToolMemoryType::Fact),
                importance: Some(0.8),
                provenance: None,
                sensitivity: None,
                tags: None,
                custom_metadata: None,
            })
            .await
            .expect("semantic memory should store");
        let matches = repository
            .search(&MemorySearchArgs {
                query: query.to_string(),
                limit: Some(5),
                include_dormant: None,
                memory_types: None,
            })
            .await
            .expect("semantic search should succeed");

        assert!(!matches.is_empty());
        assert_eq!(matches[0].memory.id.to_string(), stored.memory.id);
    }

    #[tokio::test]
    async fn semantic_search_preserves_agent_isolation() {
        let temp_dir = TempDir::new().expect("tempdir should create");
        let db_path = temp_dir.path().join("memory.db");
        let visible_content = "Visible arabica memory for configured agent.";
        let hidden_content = "Hidden arabica memory for another agent.";
        let query = "fragrant hot drink";
        let provider = Arc::new(StubEmbeddingProvider::new([
            (visible_content, axis_embedding()),
            (query, axis_embedding()),
        ]));
        let repository = test_repository(&db_path, "visible-agent", provider);

        let visible = repository
            .store_memory(&MemoryStoreArgs {
                content: visible_content.to_string(),
                summary: None,
                memory_type: Some(ToolMemoryType::Fact),
                importance: Some(0.8),
                provenance: None,
                sensitivity: None,
                tags: None,
                custom_metadata: None,
            })
            .await
            .expect("visible memory should store");

        let hidden_store =
            SqliteMemoryStore::new(&db_path, MemoryScope::Agent).expect("hidden store should open");
        let hidden_memory = test_agent_memory(hidden_content, "other-agent");
        let hidden_id = hidden_memory.id;
        hidden_store
            .store(hidden_memory)
            .await
            .expect("hidden memory should store");
        hidden_store
            .store_embedding(&hidden_id, &axis_embedding())
            .await
            .expect("hidden embedding should store");

        let matches = repository
            .search(&MemorySearchArgs {
                query: query.to_string(),
                limit: Some(5),
                include_dormant: None,
                memory_types: None,
            })
            .await
            .expect("semantic search should succeed");

        let result_ids = matches
            .into_iter()
            .map(|result| result.memory.id.to_string())
            .collect::<Vec<_>>();
        assert_eq!(result_ids, vec![visible.memory.id]);
    }

    fn test_repository(
        db_path: &Path,
        agent_id: &str,
        provider: Arc<dyn EmbeddingProvider>,
    ) -> MemoryRepository {
        MemoryRepository::new_with_embedding_provider(
            db_path,
            MemoryBinding::new(DEFAULT_NAMESPACE, agent_id).expect("binding should build"),
            provider,
        )
        .expect("repository should build")
    }

    fn test_agent_memory(content: &str, agent_id: &str) -> Memory {
        let now = "2026-05-10T12:00:00Z"
            .parse()
            .expect("fixture timestamp should parse");
        Memory {
            id: Uuid::new_v4(),
            content: content.to_string(),
            summary: None,
            scope: MemoryScope::Agent,
            memory_type: MemoryType::Fact,
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
            embedding_stale: false,
            created_at: now,
            updated_at: now,
            last_accessed_at: None,
            tenant_id: None,
            user_id: None,
            agent_id: Some(agent_id.to_string()),
        }
    }

    fn axis_embedding() -> Vec<f32> {
        let mut embedding = vec![0.0; 768];
        embedding[0] = 1.0;
        embedding
    }

    #[derive(Debug)]
    struct StubEmbeddingProvider {
        responses: HashMap<String, Vec<f32>>,
        calls: Mutex<Vec<String>>,
    }

    impl StubEmbeddingProvider {
        fn new<I, S>(responses: I) -> Self
        where
            I: IntoIterator<Item = (S, Vec<f32>)>,
            S: Into<String>,
        {
            Self {
                responses: responses
                    .into_iter()
                    .map(|(text, embedding)| (text.into(), embedding))
                    .collect(),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("stub calls lock").clone()
        }
    }

    #[async_trait]
    impl EmbeddingProvider for StubEmbeddingProvider {
        async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
            let trimmed = text.trim().to_string();
            self.calls
                .lock()
                .expect("stub calls lock")
                .push(trimmed.clone());
            self.responses.get(&trimmed).cloned().ok_or_else(|| {
                EmbeddingError::Provider(format!("missing stub embedding for `{trimmed}`"))
            })
        }

        fn dimensions(&self) -> usize {
            768
        }

        fn model_id(&self) -> &str {
            "mcp-test-stub"
        }
    }
}
