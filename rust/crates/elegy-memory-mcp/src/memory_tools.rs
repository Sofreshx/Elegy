use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use elegy_memory::{
    CorrectionDisposition, CorrectionRecord, DefaultSalienceGate, GateDecision, GateError, Memory,
    MemoryCandidate, MemoryFilter, MemoryScope, MemoryState, MemoryStore, MemoryType,
    ProvenanceLevel, ResolutionStatus, SalienceGate, SensitivityLevel, SqliteMemoryStore,
    StoreError,
};
use rmcp::model::JsonObject;
use rmcp::schemars;
use rmcp::ErrorData;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub(crate) const FIXED_NAMESPACE: &str = "claude-ai-remote";
pub(crate) const SCOPE_OVERRIDE_ERROR_MESSAGE: &str =
    "scope override not permitted — this connector is hardwired to 'claude-ai-remote'";

const DEFAULT_SEARCH_LIMIT: usize = 10;
const DEFAULT_LIST_LIMIT: usize = 20;
const DEFAULT_STORE_IMPORTANCE: f32 = 0.5;
const PREVIEW_LIMIT: usize = 140;

#[derive(Clone)]
pub(crate) struct ClaudeRemoteMemoryRepository {
    store: SqliteMemoryStore,
}

impl ClaudeRemoteMemoryRepository {
    pub(crate) fn new(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Ok(Self {
            store: SqliteMemoryStore::new(path, MemoryScope::Agent)?,
        })
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
        if args.limit == 0 {
            return Ok(Vec::new());
        }

        let memories = self
            .list_visible_memories(
                args.state_filter(),
                args.memory_types.clone().map(tool_memory_types),
                None,
            )
            .await?;
        let normalized_query = normalize_text(query);
        let tokens = query_tokens(&normalized_query);

        let mut matches = memories
            .into_iter()
            .filter_map(|memory| {
                score_memory(&memory, &normalized_query, &tokens).map(|(score, similarity)| {
                    MemorySearchMatch {
                        memory,
                        score,
                        similarity,
                    }
                })
            })
            .collect::<Vec<_>>();

        matches.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| right.similarity.total_cmp(&left.similarity))
                .then_with(|| right.memory.updated_at.cmp(&left.memory.updated_at))
                .then_with(|| right.memory.id.cmp(&left.memory.id))
        });
        matches.truncate(args.limit);
        Ok(matches)
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
            Some(args.limit),
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
                Ok(MemoryStoreResponse {
                    namespace: FIXED_NAMESPACE,
                    action: "merged",
                    gate_result: "merge".to_string(),
                    memory: MemoryDetail::from(memory),
                })
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
                Ok(MemoryStoreResponse {
                    namespace: FIXED_NAMESPACE,
                    action: "added",
                    gate_result: format_contradiction_gate_result(conflicting_id),
                    memory: MemoryDetail::from(memory),
                })
            }
            GateDecision::Archive => {
                let memory = self.build_memory_from_candidate(&candidate, MemoryState::Dormant);
                let id = memory.id;
                self.store.store(memory).await?;
                let memory = self.require_visible_memory(&id).await?;
                Ok(MemoryStoreResponse {
                    namespace: FIXED_NAMESPACE,
                    action: "added",
                    gate_result: "archived".to_string(),
                    memory: MemoryDetail::from(memory),
                })
            }
            GateDecision::Accept {
                similar_to,
                similarity,
            } => {
                let memory = self.build_memory_from_candidate(&candidate, MemoryState::Active);
                let id = memory.id;
                self.store.store(memory).await?;
                let memory = self.require_visible_memory(&id).await?;
                Ok(MemoryStoreResponse {
                    namespace: FIXED_NAMESPACE,
                    action: "added",
                    gate_result: format_gate_result(similar_to, similarity),
                    memory: MemoryDetail::from(memory),
                })
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
            namespace: FIXED_NAMESPACE,
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
            namespace: FIXED_NAMESPACE,
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
            namespace: FIXED_NAMESPACE,
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
                agent_id: Some(FIXED_NAMESPACE.to_string()),
                limit,
            })
            .await?;
        memories.retain(|memory| self.is_visible_memory(memory));
        Ok(memories)
    }

    fn is_visible_memory(&self, memory: &Memory) -> bool {
        memory.scope == MemoryScope::Agent
            && memory.agent_id.as_deref() == Some(FIXED_NAMESPACE)
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
            agent_id: Some(FIXED_NAMESPACE.to_string()),
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
pub(crate) struct MemoryStoreArgs {
    pub(crate) content: String,
    #[serde(default)]
    pub(crate) summary: Option<String>,
    #[serde(default = "default_store_memory_type")]
    pub(crate) memory_type: ToolMemoryType,
    #[serde(default = "default_store_importance")]
    pub(crate) importance: f32,
    #[serde(default = "default_store_provenance")]
    pub(crate) provenance: ToolProvenance,
    #[serde(default = "default_store_sensitivity")]
    pub(crate) sensitivity: ToolSensitivity,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    #[serde(default)]
    pub(crate) custom_metadata: BTreeMap<String, String>,
}

impl MemoryStoreArgs {
    fn to_candidate(&self) -> Result<MemoryCandidate, StoreError> {
        Ok(MemoryCandidate {
            content: require_non_empty_text("content", &self.content)?.to_string(),
            summary: normalized_optional_text(self.summary.as_deref()),
            memory_type: self.memory_type.into(),
            provenance: self.provenance.into(),
            importance_score: validate_importance(self.importance)?,
            sensitivity: self.sensitivity.into(),
            tags: self
                .tags
                .iter()
                .filter_map(|tag| normalized_optional_text(Some(tag.as_str())))
                .collect(),
            custom_metadata: self.custom_metadata.clone().into_iter().collect(),
            embedding: None,
        })
    }
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MemoryUpdateArgs {
    pub(crate) id: String,
    pub(crate) content: String,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MemoryCorrectArgs {
    pub(crate) id: String,
    pub(crate) content: String,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MemoryDeleteArgs {
    pub(crate) id: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MemorySearchArgs {
    pub(crate) query: String,
    #[serde(default = "default_search_limit")]
    pub(crate) limit: usize,
    #[serde(default)]
    pub(crate) include_dormant: bool,
    #[serde(default)]
    pub(crate) memory_types: Option<Vec<ToolMemoryType>>,
}

impl MemorySearchArgs {
    fn state_filter(&self) -> Option<MemoryState> {
        if self.include_dormant {
            None
        } else {
            Some(MemoryState::Active)
        }
    }
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MemoryRecallArgs {
    pub(crate) id: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MemoryListArgs {
    #[serde(default = "default_list_limit")]
    pub(crate) limit: usize,
    #[serde(default)]
    pub(crate) include_dormant: bool,
    #[serde(default)]
    pub(crate) state: Option<ToolMemoryState>,
    #[serde(default)]
    pub(crate) memory_types: Option<Vec<ToolMemoryType>>,
}

impl MemoryListArgs {
    fn state_filter(&self) -> Option<MemoryState> {
        if let Some(state) = self.state {
            Some(state.into())
        } else if self.include_dormant {
            None
        } else {
            Some(MemoryState::Active)
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MemoryStatsArgs {}

#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ToolMemoryState {
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
pub(crate) enum ToolMemoryType {
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
pub(crate) enum ToolProvenance {
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
pub(crate) enum ToolSensitivity {
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
pub(crate) struct MemorySearchResponse {
    pub(crate) namespace: &'static str,
    pub(crate) count: usize,
    pub(crate) query: String,
    pub(crate) include_dormant: bool,
    pub(crate) results: Vec<SearchResultRow>,
}

impl MemorySearchResponse {
    pub(crate) fn new(args: &MemorySearchArgs, matches: Vec<MemorySearchMatch>) -> Self {
        Self {
            namespace: FIXED_NAMESPACE,
            count: matches.len(),
            query: args.query.clone(),
            include_dormant: args.include_dormant,
            results: matches.into_iter().map(SearchResultRow::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchResultRow {
    pub(crate) id: String,
    pub(crate) score: f32,
    pub(crate) similarity: f32,
    pub(crate) state: String,
    pub(crate) memory_type: String,
    pub(crate) preview: String,
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
pub(crate) struct MemoryRecallResponse {
    pub(crate) namespace: &'static str,
    pub(crate) found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) memory: Option<MemoryDetail>,
}

impl MemoryRecallResponse {
    pub(crate) fn from_memory(memory: Option<Memory>) -> Self {
        Self {
            namespace: FIXED_NAMESPACE,
            found: memory.is_some(),
            memory: memory.map(MemoryDetail::from),
        }
    }
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryListResponse {
    pub(crate) namespace: &'static str,
    pub(crate) count: usize,
    pub(crate) include_dormant: bool,
    pub(crate) memories: Vec<ListRow>,
}

impl MemoryListResponse {
    pub(crate) fn new(args: &MemoryListArgs, memories: Vec<Memory>) -> Self {
        Self {
            namespace: FIXED_NAMESPACE,
            count: memories.len(),
            include_dormant: args.include_dormant,
            memories: memories.into_iter().map(ListRow::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListRow {
    pub(crate) id: String,
    pub(crate) state: String,
    pub(crate) memory_type: String,
    pub(crate) provenance: String,
    pub(crate) importance: f32,
    pub(crate) updated_at: String,
    pub(crate) preview: String,
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
pub(crate) struct MemoryDetail {
    pub(crate) id: String,
    pub(crate) content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) summary: Option<String>,
    pub(crate) state: String,
    pub(crate) memory_type: String,
    pub(crate) provenance: String,
    pub(crate) importance: f32,
    pub(crate) reliability: f32,
    pub(crate) tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_accessed_at: Option<String>,
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
pub(crate) struct MemoryStatsResponse {
    pub(crate) namespace: &'static str,
    pub(crate) scope: &'static str,
    pub(crate) agent_id: &'static str,
    pub(crate) total_count: u64,
    pub(crate) active_count: u64,
    pub(crate) dormant_count: u64,
    pub(crate) stale_embeddings_count: u64,
    pub(crate) unresolved_contradictions: u64,
    pub(crate) type_counts: BTreeMap<String, u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) oldest_active_memory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) newest_memory: Option<String>,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryStoreResponse {
    pub(crate) namespace: &'static str,
    pub(crate) action: &'static str,
    pub(crate) gate_result: String,
    pub(crate) memory: MemoryDetail,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryUpdateResponse {
    pub(crate) namespace: &'static str,
    pub(crate) updated: bool,
    pub(crate) memory: MemoryDetail,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryCorrectResponse {
    pub(crate) namespace: &'static str,
    pub(crate) correction: MemoryCorrectionSummary,
    pub(crate) memory: MemoryDetail,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryCorrectionSummary {
    pub(crate) id: String,
    pub(crate) memory_id: String,
    pub(crate) disposition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) related_memory_id: Option<String>,
    pub(crate) corrected_at: String,
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
pub(crate) struct MemoryDeleteResponse {
    pub(crate) namespace: &'static str,
    pub(crate) id: String,
    pub(crate) deleted: bool,
}

impl From<MemoryStatsSnapshot> for MemoryStatsResponse {
    fn from(value: MemoryStatsSnapshot) -> Self {
        Self {
            namespace: FIXED_NAMESPACE,
            scope: "agent",
            agent_id: FIXED_NAMESPACE,
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
pub(crate) struct MemorySearchMatch {
    pub(crate) memory: Memory,
    pub(crate) score: f32,
    pub(crate) similarity: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct MemoryStatsSnapshot {
    pub(crate) total_count: u64,
    pub(crate) active_count: u64,
    pub(crate) dormant_count: u64,
    pub(crate) stale_embeddings_count: u64,
    pub(crate) unresolved_contradictions: u64,
    pub(crate) oldest_active_memory: Option<String>,
    pub(crate) newest_memory: Option<String>,
    pub(crate) type_counts: BTreeMap<String, u64>,
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
            format!("memory `{id}` was not found in the fixed claude-ai-remote namespace"),
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

fn normalize_text(value: &str) -> String {
    value.to_lowercase()
}

fn query_tokens(query: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    query
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .filter_map(|token| {
            let token = token.to_string();
            seen.insert(token.clone()).then_some(token)
        })
        .collect()
}

fn score_memory(memory: &Memory, query: &str, tokens: &[String]) -> Option<(f32, f32)> {
    let haystacks = [
        normalize_text(&memory.content),
        memory
            .summary
            .as_deref()
            .map(normalize_text)
            .unwrap_or_default(),
        normalize_text(&memory.tags.join(" ")),
    ];

    let exact_match = haystacks.iter().any(|haystack| haystack == query);
    let phrase_match =
        !query.is_empty() && haystacks.iter().any(|haystack| haystack.contains(query));
    let token_hits = tokens
        .iter()
        .filter(|token| {
            haystacks
                .iter()
                .any(|haystack| haystack.contains(token.as_str()))
        })
        .count();

    if !exact_match && !phrase_match && token_hits == 0 {
        return None;
    }

    let token_ratio = if tokens.is_empty() {
        if phrase_match || exact_match {
            1.0
        } else {
            0.0
        }
    } else {
        token_hits as f32 / tokens.len() as f32
    };
    let similarity = if exact_match {
        1.0
    } else {
        ((if phrase_match { 0.45 } else { 0.0 }) + (token_ratio * 0.55)).min(1.0)
    };
    let score = similarity
        + (memory.importance_score.clamp(0.0, 1.0) * 0.05)
        + (((memory.access_count as f32) + 1.0).ln() * 0.01);

    Some((score, similarity))
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
