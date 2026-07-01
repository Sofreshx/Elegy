use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{
    params, params_from_iter, Connection, OptionalExtension, Row, Transaction, TransactionBehavior,
};
use serde::Serialize;
use serde_json::Value;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

use crate::{
    validation::validate_entity, AcceptanceKind, AcceptanceView, AttachWorktreeInput,
    BlockedCandidate, DiscoveryCheckpointRecord, DiscoveryClassification, DiscoveryRecord,
    DiscoveryRelationshipKind, DiscoveryRelationshipRecord, DiscoverySourceEntry, DiscoveryStatus,
    DiscoveryView, EffortTier, EntityType, EvidenceKind, EvidenceView, FileScopeRecord,
    FileScopeSelectorType, GoalRecord, GoalStatus, GoalView, GraphEdgeView, GraphNodeView,
    InsightRecord, InsightStatus, InsightType, InsightView, IssueRecord, IssueStatus, IssueView,
    MutationResult, PlanRecord, PlanStatus, PlanView, PlanningEdgeKind, PlanningEvent,
    PlanningGraphEdge, PlanningGraphNode, PlanningHealthReport, PlanningNodeKind,
    PlanningStoreError, Priority, ProjectRunEvidence, ProjectRunRecord, ProjectRunStatus,
    ProjectRunView, ProjectionFormat, RenderedProjection, ReviewPointRecord, ReviewPointStatus,
    RoadmapRecord, RoadmapSectionRecord, RoadmapStatus, RoadmapView, RunnableCandidates,
    RunnableWorkPointCandidate, ScopeRecord, SessionSummary, Severity, TagInfo, TodoRecord,
    TodoStatus, ValidationFinding, ValidationReport, ValidationRunReport, ValidationSeverity,
    VerificationState, WorkGraph, WorkGraphEdge, WorkGraphNode, WorkPointKind, WorkPointRecord,
    WorkPointStatus, WorkPointView, WorktreeRecord, WorktreeStatus,
};

pub const CURRENT_SCHEMA_VERSION: &str = "11";
const DEFAULT_LEASE_SECONDS: i64 = 900;
const SCHEMA_VERSION_KEY: &str = "schema_version";
const DEFAULT_SCOPE_KEY: &str = "default";
const SQLITE_MAX_VARIABLES: usize = 999;
const FILE_SCOPE_QUERY_FIXED_VARIABLES: usize = 1;

#[derive(Clone, Debug)]
pub struct PlanningStore {
    db_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct CreateGoalInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub correlation_id: String,
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    pub rejection_criteria: Vec<String>,
    pub status: GoalStatus,
    pub tags: Vec<String>,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreateRoadmapInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub goal_id: String,
    pub correlation_id: String,
    pub title: String,
    pub summary: String,
    pub status: RoadmapStatus,
    pub tags: Vec<String>,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AddRoadmapSectionInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub roadmap_id: String,
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub ordering: Option<i64>,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AddWorkPointInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub roadmap_id: String,
    pub section_id: Option<String>,
    pub title: String,
    pub summary: String,
    pub status: WorkPointStatus,
    pub ordering: Option<i64>,
    pub dependency_ids: Vec<String>,
    pub validation_expectations: Vec<String>,
    pub effort_tier: EffortTier,
    pub kind: Option<WorkPointKind>,
    pub priority: Option<Priority>,
    pub repairs_work_point_ids: Vec<String>,
    pub supersedes_work_point_ids: Vec<String>,
    pub blocks_work_point_ids: Vec<String>,
    pub file_scopes: Vec<FileScopeRecord>,
    pub tags: Vec<String>,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ReviseWorkPointInput {
    pub work_point_id: String,
    pub active_scope_key: Option<String>,
    pub dependency_ids: Option<Vec<String>>,
    pub clear_dependencies: bool,
    pub blocks_work_point_ids: Option<Vec<String>>,
    pub clear_blocks: bool,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreatePlanInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub goal_id: String,
    pub roadmap_id: String,
    pub correlation_id: String,
    pub title: String,
    pub summary: String,
    pub scope: String,
    pub assumptions: Vec<String>,
    pub stop_conditions: Vec<String>,
    pub validation_steps: Vec<String>,
    pub targeted_work_point_ids: Vec<String>,
    pub effort_tier: EffortTier,
    pub routing_hint: Option<String>,
    pub allow_parallel_overlap: bool,
    pub file_scopes: Vec<FileScopeRecord>,
    pub status: PlanStatus,
    pub tags: Vec<String>,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreateTodoInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub plan_id: Option<String>,
    pub work_point_id: Option<String>,
    pub title: String,
    pub summary: String,
    pub status: TodoStatus,
    pub priority: Priority,
    pub effort_tier: EffortTier,
    pub file_scopes: Vec<FileScopeRecord>,
    pub evidence_refs: Vec<String>,
    pub tags: Vec<String>,
    pub ordering: Option<i64>,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreateIssueInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub correlation_id: String,
    pub title: String,
    pub summary: String,
    pub status: IssueStatus,
    pub severity: Severity,
    pub related_entity_type: Option<EntityType>,
    pub related_entity_id: Option<String>,
    pub tags: Vec<String>,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreateDiscoveryInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub correlation_id: String,
    pub classification: DiscoveryClassification,
    pub verification_state: VerificationState,
    pub severity: Severity,
    pub claim: String,
    pub impact: Option<String>,
    pub next_action: Option<String>,
    pub verification_step: Option<String>,
    pub recurrence_key: Option<String>,
    pub fingerprint: Option<String>,
    pub observed_at: Vec<String>,
    pub occurrence_count: Option<i64>,
    pub source_lineage: Vec<DiscoverySourceEntry>,
    pub review_date: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct CreateDiscoveryRelationshipInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub source_id: String,
    pub target_id: String,
    pub relationship_kind: DiscoveryRelationshipKind,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Clone, Debug)]
pub struct CreateDiscoveryCheckpointInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub run_id: String,
    pub event: String,
    pub snapshot: Option<serde_json::Value>,
}

#[derive(Clone, Debug)]
pub struct CreateReviewPointInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub attached_entity_type: EntityType,
    pub attached_entity_id: String,
    pub title: String,
    pub summary: String,
    pub status: ReviewPointStatus,
    pub severity: Severity,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreateInsightInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub correlation_id: String,
    pub title: String,
    pub content: String,
    pub insight_type: InsightType,
    pub parent_entity_type: EntityType,
    pub parent_entity_id: String,
    pub tags: Vec<String>,
    pub status: InsightStatus,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SearchInput {
    pub scope_key: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub since: Option<String>,
    pub latest: Option<usize>,
    pub tag: Option<String>,
    pub fts: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ClaimProjectRunInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub goal_id: String,
    pub roadmap_id: String,
    pub work_point_id: String,
    pub repo_id: Option<String>,
    pub branch: Option<String>,
    pub worktree_id: Option<String>,
    pub session_id: Option<String>,
    pub run_id: Option<String>,
    pub profile_id: Option<String>,
    pub correlation_id: Option<String>,
    pub owner_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub lease_seconds: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct ActivateProjectRunInput {
    pub project_run_id: String,
    pub active_scope_key: Option<String>,
    pub run_id: Option<String>,
    pub fencing_token: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct HeartbeatProjectRunInput {
    pub project_run_id: String,
    pub active_scope_key: Option<String>,
    pub run_id: Option<String>,
    pub fencing_token: Option<i64>,
    pub lease_seconds: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct ReleaseProjectRunInput {
    pub project_run_id: String,
    pub status: ProjectRunStatus,
    pub evidence: Option<ProjectRunEvidence>,
    pub active_scope_key: Option<String>,
    pub run_id: Option<String>,
    pub fencing_token: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct AddEvidenceInput {
    pub project_run_id: String,
    pub evidence: ProjectRunEvidence,
    pub active_scope_key: Option<String>,
    pub run_id: Option<String>,
    pub fencing_token: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct CreateScopeInput {
    pub scope_key: String,
    pub scope_type: Option<String>,
    pub parent_scope_key: Option<String>,
    pub metadata: Option<Value>,
    pub tags: Vec<String>,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct UpdateStatusInput {
    pub entity_type: EntityType,
    pub entity_id: String,
    pub status: String,
    pub evidence_refs: Option<Vec<String>>,
    pub active_scope_key: Option<String>,
    pub run_id: Option<String>,
    pub override_transition: bool,
    pub reason: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RevisePlanInput {
    pub plan_id: String,
    pub active_scope_key: Option<String>,
    pub scope_key: Option<String>,
    pub assumptions: Option<Vec<String>>,
    pub stop_conditions: Option<Vec<String>>,
    pub validation_steps: Option<Vec<String>>,
    pub targeted_work_point_ids: Option<Vec<String>>,
    pub effort_tier: Option<EffortTier>,
    pub routing_hint: Option<String>,
    pub clear_routing_hint: bool,
    pub allow_parallel_overlap: Option<bool>,
    pub file_scopes: Option<Vec<FileScopeRecord>>,
    pub clear_file_scopes: bool,
    pub tags: Option<Vec<String>>,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreateGraphNodeInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub correlation_id: String,
    pub kind: PlanningNodeKind,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub payload: serde_json::Value,
    pub tags: Vec<String>,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreateGraphEdgeInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub correlation_id: String,
    pub kind: PlanningEdgeKind,
    pub source_node_id: String,
    pub target_node_id: String,
    pub status: String,
    pub payload: serde_json::Value,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct UpdateGraphNodeStatusInput {
    pub node_id: String,
    pub correlation_id: String,
    pub active_scope_key: Option<String>,
    pub status: String,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct UpdateGraphEdgeStatusInput {
    pub edge_id: String,
    pub correlation_id: String,
    pub active_scope_key: Option<String>,
    pub status: String,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ReviseGraphNodeInput {
    pub node_id: String,
    pub correlation_id: String,
    pub active_scope_key: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub status: Option<String>,
    pub payload: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
    pub clear_tags: bool,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ReviseGraphEdgeInput {
    pub edge_id: String,
    pub correlation_id: String,
    pub active_scope_key: Option<String>,
    pub status: Option<String>,
    pub payload: Option<serde_json::Value>,
    pub run_id: Option<String>,
}

/// Typed input for creating an acceptance graph node.
#[derive(Clone, Debug)]
pub struct CreateAcceptanceInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub correlation_id: String,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub acceptance_kind: AcceptanceKind,
    pub description: String,
    pub verification_policy: String,
    pub required_evidence_kinds: Vec<EvidenceKind>,
    pub waiver: Option<String>,
    pub tags: Vec<String>,
    pub run_id: Option<String>,
}

/// Typed input for creating an evidence graph node.
#[derive(Clone, Debug)]
pub struct CreateEvidenceInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub correlation_id: String,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub evidence_kind: EvidenceKind,
    pub reference: String,
    pub content: String,
    pub captured_at: String,
    pub tags: Vec<String>,
    pub run_id: Option<String>,
}

/// Input for linking a concrete acceptance to an abstract acceptance (Satisfies edge).
#[derive(Clone, Debug)]
pub struct SatisfyAcceptanceInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub correlation_id: String,
    pub concrete_node_id: String,
    pub abstract_node_id: String,
    pub rationale: String,
    pub run_id: Option<String>,
}

/// Input for attaching an evidence node to a target (EvidencedBy edge).
#[derive(Clone, Debug)]
pub struct AttachEvidenceInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub correlation_id: String,
    pub evidence_node_id: String,
    pub target_node_id: String,
    pub rationale: String,
    pub run_id: Option<String>,
}

/// Input for finalizing a graph node (transitioning to a terminal status).
#[derive(Clone, Debug)]
pub struct FinalizeGraphNodeInput {
    pub node_id: String,
    pub correlation_id: String,
    pub active_scope_key: Option<String>,
    pub status: String,
    /// Optional accepted risk rationale. Only applies to acceptance/evidence gaps.
    pub accepted_risk: Option<String>,
    pub run_id: Option<String>,
}

impl PlanningStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            db_path: path.into(),
        }
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn init(&self) -> Result<(), PlanningStoreError> {
        let _ = self.open_connection()?;
        Ok(())
    }

    pub fn create_goal(
        &self,
        input: CreateGoalInput,
    ) -> Result<MutationResult<GoalRecord>, PlanningStoreError> {
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("title", &input.title)?;
        require_non_empty("description", &input.description)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let scope_key = normalized_scope_key(input.scope_key);
        ensure_scope_exists(&transaction, &scope_key)?;
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let record = GoalRecord {
            id: id.clone(),
            scope_key,
            correlation_id: input.correlation_id,
            title: input.title.trim().to_string(),
            description: input.description.trim().to_string(),
            acceptance_criteria: normalize_string_list(input.acceptance_criteria),
            rejection_criteria: normalize_string_list(input.rejection_criteria),
            status: input.status,
            tags: normalize_string_list(input.tags),
            revision: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        transaction.execute(
            r#"
            INSERT INTO goals (
                id, scope_key, correlation_id, title, description, acceptance_criteria_json,
                rejection_criteria_json, status, tags_json, revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
            params![
                record.id,
                record.scope_key,
                record.correlation_id,
                record.title,
                record.description,
                to_json_text(&record.acceptance_criteria)?,
                to_json_text(&record.rejection_criteria)?,
                record.status.as_str(),
                to_json_text(&record.tags)?,
                record.revision,
                record.created_at,
                record.updated_at,
            ],
        )?;

        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::Goal,
                &id,
                EntityType::Goal,
                &id,
                &record.correlation_id,
                input.run_id,
                "goal.created",
                serde_json::to_value(&record)?,
            )?,
        )?;

        let validation = validate_and_store(&transaction, EntityType::Goal, &id)?;
        rebuild_tag_index_for_entity(&transaction, EntityType::Goal, &id, &record.tags)?;
        upsert_fts_entry(
            &transaction,
            "entities_fts",
            &id,
            &record.title,
            &record.description,
        )?;
        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn create_roadmap(
        &self,
        input: CreateRoadmapInput,
    ) -> Result<MutationResult<RoadmapRecord>, PlanningStoreError> {
        require_non_empty("goalId", &input.goal_id)?;
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("title", &input.title)?;
        require_non_empty("summary", &input.summary)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.scope_key.clone());
        let inherited_scope_key = ensure_referenced_entity_in_scope(
            &transaction,
            EntityType::Goal,
            &input.goal_id,
            "goalId",
            &active_scope_key,
        )?;
        // Preflight: GOAL-NOT-ACTIVE — reject if goal is not active
        {
            let goal = load_goal(&transaction, &input.goal_id)?;
            if matches!(
                goal.status,
                GoalStatus::Invalidated | GoalStatus::Superseded | GoalStatus::Abandoned
            ) {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "{}",
                    serde_json::json!({
                        "code": "GOAL_NOT_ACTIVE",
                        "message": format!("goal `{}` is not active (status: {})", input.goal_id, goal.status.as_str()),
                        "goalId": input.goal_id,
                        "status": "invalid",
                    })
                )));
            }
        }

        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let record = RoadmapRecord {
            id: id.clone(),
            scope_key: inherited_scope_key,
            goal_id: input.goal_id,
            correlation_id: input.correlation_id,
            title: input.title.trim().to_string(),
            summary: input.summary.trim().to_string(),
            status: input.status,
            tags: normalize_string_list(input.tags),
            revision: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        transaction.execute(
            r#"
            INSERT INTO roadmaps (
                id, scope_key, goal_id, correlation_id, title, summary, status, tags_json,
                revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                record.id,
                record.scope_key,
                record.goal_id,
                record.correlation_id,
                record.title,
                record.summary,
                record.status.as_str(),
                to_json_text(&record.tags)?,
                record.revision,
                record.created_at,
                record.updated_at,
            ],
        )?;

        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::Roadmap,
                &id,
                EntityType::Roadmap,
                &id,
                &record.correlation_id,
                input.run_id,
                "roadmap.created",
                serde_json::to_value(&record)?,
            )?,
        )?;

        let validation = refresh_validation_target(&transaction, EntityType::Roadmap, &id)?;
        let _ = refresh_validation_target(&transaction, EntityType::Goal, &record.goal_id)?;
        rebuild_tag_index_for_entity(&transaction, EntityType::Roadmap, &id, &record.tags)?;
        upsert_fts_entry(
            &transaction,
            "entities_fts",
            &id,
            &record.title,
            &record.summary,
        )?;
        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn add_roadmap_section(
        &self,
        input: AddRoadmapSectionInput,
    ) -> Result<MutationResult<RoadmapSectionRecord>, PlanningStoreError> {
        require_non_empty("roadmapId", &input.roadmap_id)?;
        require_non_empty("slug", &input.slug)?;
        require_non_empty("title", &input.title)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.scope_key.clone());
        let inherited_scope_key = ensure_referenced_entity_in_scope(
            &transaction,
            EntityType::Roadmap,
            &input.roadmap_id,
            "roadmapId",
            &active_scope_key,
        )?;
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let ordering = input.ordering.unwrap_or(next_ordering(
            &transaction,
            "roadmap_sections",
            "roadmap_id",
            &input.roadmap_id,
        )?);
        let record = RoadmapSectionRecord {
            id: id.clone(),
            scope_key: inherited_scope_key,
            roadmap_id: input.roadmap_id,
            slug: input.slug.trim().to_string(),
            title: input.title.trim().to_string(),
            summary: input.summary.trim().to_string(),
            ordering,
            revision: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        transaction.execute(
            r#"
            INSERT INTO roadmap_sections (
                id, scope_key, roadmap_id, slug, title, summary, ordering_index, revision,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                record.id,
                record.scope_key,
                record.roadmap_id,
                record.slug,
                record.title,
                record.summary,
                record.ordering,
                record.revision,
                record.created_at,
                record.updated_at,
            ],
        )?;

        let correlation_id = roadmap_correlation_id(&transaction, &record.roadmap_id)?;
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::RoadmapSection,
                &id,
                EntityType::Roadmap,
                &record.roadmap_id,
                &correlation_id,
                input.run_id,
                "roadmap.section-added",
                serde_json::to_value(&record)?,
            )?,
        )?;

        let _ = refresh_validation_target(&transaction, EntityType::RoadmapSection, &id)?;
        let validation =
            refresh_validation_target(&transaction, EntityType::Roadmap, &record.roadmap_id)?;
        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    fn would_create_cycle(
        current: &str,
        target: &str,
        graph: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
    ) -> bool {
        if current == target {
            return true;
        }
        if !visited.insert(current.to_string()) {
            return false;
        }
        if let Some(blocked) = graph.get(current) {
            for next in blocked {
                if Self::would_create_cycle(next, target, graph, visited) {
                    return true;
                }
            }
        }
        false
    }

    pub fn add_work_point(
        &self,
        input: AddWorkPointInput,
    ) -> Result<MutationResult<WorkPointRecord>, PlanningStoreError> {
        require_non_empty("roadmapId", &input.roadmap_id)?;
        require_non_empty("title", &input.title)?;
        require_non_empty("summary", &input.summary)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.scope_key.clone());
        let inherited_scope_key = ensure_referenced_entity_in_scope(
            &transaction,
            EntityType::Roadmap,
            &input.roadmap_id,
            "roadmapId",
            &active_scope_key,
        )?;
        if let Some(section_id) = &input.section_id {
            ensure_referenced_entity_in_scope(
                &transaction,
                EntityType::RoadmapSection,
                section_id,
                "sectionId",
                &active_scope_key,
            )?;
            ensure_section_belongs_to_roadmap(&transaction, section_id, &input.roadmap_id)?;
        }
        // Reject cross-roadmap work-point dependencies
        for dep_id in &input.dependency_ids {
            let trimmed = dep_id.trim();
            if trimmed.is_empty() {
                continue;
            }
            match load_work_point(&transaction, trimmed) {
                Ok(dep) => {
                    if dep.roadmap_id != input.roadmap_id {
                        return Err(PlanningStoreError::InvalidInput(format!(
                            "dependency work point `{}` belongs to roadmap `{}`, not `{}`. Cross-roadmap work-point dependencies are not supported.",
                            trimmed, dep.roadmap_id, input.roadmap_id
                        )));
                    }
                }
                Err(PlanningStoreError::NotFound { .. }) => {
                    // Non-existent deps are advisory validation findings, not write-time blockers
                }
                Err(e) => return Err(e),
            }
        }
        // Reject missing or cross-roadmap block targets. Block relationships are structural:
        // unlike dependencies, dangling block edges would make runnable selection misleading.
        for blocked_id in &input.blocks_work_point_ids {
            let trimmed = blocked_id.trim();
            if trimmed.is_empty() {
                continue;
            }
            match load_work_point(&transaction, trimmed) {
                Err(PlanningStoreError::NotFound { .. }) => {
                    return Err(PlanningStoreError::InvalidInput(format!(
                        "blocked work point `{}` does not exist",
                        trimmed
                    )));
                }
                Err(e) => return Err(e),
                Ok(blocked) => {
                    if blocked.roadmap_id != input.roadmap_id {
                        return Err(PlanningStoreError::InvalidInput(format!(
                            "blocked work point `{}` belongs to roadmap `{}`, not `{}`. Cross-roadmap block relationships are not supported.",
                            trimmed, blocked.roadmap_id, input.roadmap_id
                        )));
                    }
                }
            }
        }
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let ordering = input.ordering.unwrap_or(next_ordering(
            &transaction,
            "work_points",
            "roadmap_id",
            &input.roadmap_id,
        )?);
        let record = WorkPointRecord {
            id: id.clone(),
            scope_key: inherited_scope_key,
            roadmap_id: input.roadmap_id,
            section_id: input.section_id,
            title: input.title.trim().to_string(),
            summary: input.summary.trim().to_string(),
            status: input.status,
            ordering,
            dependency_ids: normalize_string_list(input.dependency_ids),
            validation_expectations: normalize_string_list(input.validation_expectations),
            effort_tier: input.effort_tier,
            kind: input.kind.unwrap_or(WorkPointKind::Feature),
            priority: input.priority.unwrap_or(Priority::Medium),
            repairs_work_point_ids: normalize_string_list(input.repairs_work_point_ids),
            supersedes_work_point_ids: normalize_string_list(input.supersedes_work_point_ids),
            blocks_work_point_ids: normalize_string_list(input.blocks_work_point_ids),
            file_scopes: normalize_file_scopes(input.file_scopes),
            tags: normalize_string_list(input.tags),
            revision: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        // Cycle detection for blocks_work_point_ids
        if !record.blocks_work_point_ids.is_empty() {
            // Collect all blocks_work_point_ids relationships in the scope
            let mut all_blocks: HashMap<String, Vec<String>> = HashMap::new();
            {
                let mut stmt = transaction.prepare(
                    "SELECT id, blocks_work_point_ids FROM work_points WHERE scope_key = ?1 AND blocks_work_point_ids != '[]'",
                )?;
                let rows = stmt.query_map(params![record.scope_key], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                for row in rows {
                    let (wid, blocks_json) = row?;
                    let blocked: Vec<String> =
                        serde_json::from_str(&blocks_json).unwrap_or_default();
                    if !blocked.is_empty() {
                        all_blocks.insert(wid, blocked);
                    }
                }
            }
            // Add the current work point's blocks
            all_blocks.insert(record.id.clone(), record.blocks_work_point_ids.clone());

            // Check: if any blocked ID can reach back to this work point via transitive blocks, it's a cycle
            for blocked_id in &record.blocks_work_point_ids {
                if Self::would_create_cycle(
                    blocked_id,
                    &record.id,
                    &all_blocks,
                    &mut HashSet::new(),
                ) {
                    return Err(PlanningStoreError::InvalidInput(format!(
                        "{}",
                        serde_json::json!({
                            "code": "INVALID_BLOCK_CYCLE",
                            "message": format!("blocking work point '{}' would create a cycle via '{}'", record.id, blocked_id),
                            "workPointId": record.id,
                            "blockedId": blocked_id,
                        })
                    )));
                }
            }
        }

        transaction.execute(
            r#"
            INSERT INTO work_points (
                id, scope_key, roadmap_id, section_id, title, summary, status, ordering_index,
                dependency_ids_json, validation_expectations_json, effort_tier, kind, priority,
                repairs_work_point_ids, supersedes_work_point_ids, blocks_work_point_ids, tags_json,
                revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
            "#,
            params![
                record.id,
                record.scope_key,
                record.roadmap_id,
                record.section_id,
                record.title,
                record.summary,
                record.status.as_str(),
                record.ordering,
                to_json_text(&record.dependency_ids)?,
                to_json_text(&record.validation_expectations)?,
                record.effort_tier.as_str(),
                record.kind.as_str(),
                record.priority.as_str(),
                to_json_text(&record.repairs_work_point_ids)?,
                to_json_text(&record.supersedes_work_point_ids)?,
                to_json_text(&record.blocks_work_point_ids)?,
                to_json_text(&record.tags)?,
                record.revision,
                record.created_at,
                record.updated_at,
            ],
        )?;

        replace_entity_file_scopes(
            &transaction,
            &record.scope_key,
            EntityType::WorkPoint,
            &record.id,
            &record.file_scopes,
            &now,
        )?;

        let correlation_id = roadmap_correlation_id(&transaction, &record.roadmap_id)?;
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::WorkPoint,
                &id,
                EntityType::Roadmap,
                &record.roadmap_id,
                &correlation_id,
                input.run_id,
                "roadmap.work-point-added",
                serde_json::to_value(&record)?,
            )?,
        )?;

        let _ = refresh_validation_target(&transaction, EntityType::WorkPoint, &id)?;
        if let Some(section_id) = &record.section_id {
            let _ =
                refresh_validation_target(&transaction, EntityType::RoadmapSection, section_id)?;
        }
        for dependent_work_point_id in list_work_point_dependents(&transaction, &id)? {
            let _ = refresh_validation_target(
                &transaction,
                EntityType::WorkPoint,
                &dependent_work_point_id,
            )?;
        }
        for plan_id in list_plans_targeting_work_point(&transaction, &id)? {
            let _ = refresh_validation_target(&transaction, EntityType::Plan, &plan_id)?;
        }
        let validation =
            refresh_validation_target(&transaction, EntityType::Roadmap, &record.roadmap_id)?;
        rebuild_tag_index_for_entity(&transaction, EntityType::WorkPoint, &id, &record.tags)?;
        upsert_fts_entry(
            &transaction,
            "entities_fts",
            &id,
            &record.title,
            &record.summary,
        )?;
        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn revise_work_point(
        &self,
        input: ReviseWorkPointInput,
    ) -> Result<MutationResult<WorkPointRecord>, PlanningStoreError> {
        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;

        // Load existing record
        let mut record = load_work_point(&transaction, &input.work_point_id)?;

        // Scope enforcement: if active_scope_key is provided, ensure it matches
        if let Some(ref active_scope) = input.active_scope_key {
            let active_scope = normalized_scope_key(Some(active_scope.clone()));
            if record.scope_key != active_scope {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "work point `{}` is in scope `{}`, not `{}`",
                    input.work_point_id, record.scope_key, active_scope
                )));
            }
        }

        // Validate mutual exclusivity
        if input.clear_dependencies && input.dependency_ids.is_some() {
            return Err(PlanningStoreError::InvalidInput(
                "--clear-dependencies cannot be combined with providing new dependency IDs"
                    .to_string(),
            ));
        }
        if input.clear_blocks && input.blocks_work_point_ids.is_some() {
            return Err(PlanningStoreError::InvalidInput(
                "--clear-blocks cannot be combined with providing new blocked work point IDs"
                    .to_string(),
            ));
        }

        // Compute new dependencies
        let new_deps = if input.clear_dependencies {
            Vec::new()
        } else if let Some(ref dep_ids) = input.dependency_ids {
            let trimmed: Vec<String> = dep_ids
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            // Validate all deps exist
            for dep_id in &trimmed {
                match load_work_point(&transaction, dep_id) {
                    Err(PlanningStoreError::NotFound { .. }) => {
                        return Err(PlanningStoreError::InvalidInput(format!(
                            "dependency work point `{}` does not exist",
                            dep_id
                        )));
                    }
                    Err(e) => return Err(e),
                    Ok(_) => {}
                }
            }

            // Reject cross-roadmap dependencies
            for dep_id in &trimmed {
                let dep = load_work_point(&transaction, dep_id)?;
                if dep.roadmap_id != record.roadmap_id {
                    return Err(PlanningStoreError::InvalidInput(format!(
                        "dependency work point `{}` belongs to roadmap `{}`, not `{}`. Cross-roadmap work-point dependencies are not supported.",
                        dep_id, dep.roadmap_id, record.roadmap_id
                    )));
                }
            }

            trimmed
        } else {
            record.dependency_ids.clone()
        };

        // Compute new blocks
        let new_blocks = if input.clear_blocks {
            Vec::new()
        } else if let Some(ref block_ids) = input.blocks_work_point_ids {
            let trimmed: Vec<String> = block_ids
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            // Validate all blocked work points exist
            for blocked_id in &trimmed {
                match load_work_point(&transaction, blocked_id) {
                    Err(PlanningStoreError::NotFound { .. }) => {
                        return Err(PlanningStoreError::InvalidInput(format!(
                            "blocked work point `{}` does not exist",
                            blocked_id
                        )));
                    }
                    Err(e) => return Err(e),
                    Ok(dep) => {
                        if dep.roadmap_id != record.roadmap_id {
                            return Err(PlanningStoreError::InvalidInput(format!(
                                "blocked work point `{}` belongs to roadmap `{}`, not `{}`. Cross-roadmap block relationships are not supported.",
                                blocked_id, dep.roadmap_id, record.roadmap_id
                            )));
                        }
                    }
                }
            }

            trimmed
        } else {
            record.blocks_work_point_ids.clone()
        };

        // Cycle detection for blocks_work_point_ids (only when blocks changed)
        if new_blocks != record.blocks_work_point_ids && !new_blocks.is_empty() {
            let mut all_blocks: HashMap<String, Vec<String>> = HashMap::new();
            {
                let mut stmt = transaction.prepare(
                    "SELECT id, blocks_work_point_ids FROM work_points WHERE scope_key = ?1 AND blocks_work_point_ids != '[]'",
                )?;
                let rows = stmt.query_map(params![record.scope_key], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                for row in rows {
                    let (wid, blocks_json) = row?;
                    let blocked: Vec<String> =
                        serde_json::from_str(&blocks_json).unwrap_or_default();
                    if !blocked.is_empty() {
                        all_blocks.insert(wid, blocked);
                    }
                }
            }
            // Use the new blocks for the current work point
            all_blocks.insert(record.id.clone(), new_blocks.clone());

            for blocked_id in &new_blocks {
                if Self::would_create_cycle(
                    blocked_id,
                    &record.id,
                    &all_blocks,
                    &mut HashSet::new(),
                ) {
                    return Err(PlanningStoreError::InvalidInput(format!(
                        "{}",
                        serde_json::json!({
                            "code": "INVALID_BLOCK_CYCLE",
                            "message": format!("blocking work point '{}' would create a cycle via '{}'", record.id, blocked_id),
                            "workPointId": record.id,
                            "blockedId": blocked_id,
                        })
                    )));
                }
            }
        }

        let now = now_string()?;

        transaction.execute(
            "UPDATE work_points SET dependency_ids_json = ?1, blocks_work_point_ids = ?2, revision = revision + 1, updated_at = ?3 WHERE id = ?4",
            params![to_json_text(&new_deps)?, to_json_text(&new_blocks)?, now, record.id],
        )?;

        record.dependency_ids = new_deps;
        record.blocks_work_point_ids = new_blocks;
        record.revision += 1;
        record.updated_at = now.clone();

        // Record event
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::WorkPoint,
                &record.id,
                EntityType::Roadmap,
                &record.roadmap_id,
                &roadmap_correlation_id(&transaction, &record.roadmap_id)?,
                input.run_id,
                "work-point.revised",
                serde_json::to_value(&record)?,
            )?,
        )?;

        // Re-validate
        let _ = refresh_validation_target(&transaction, EntityType::WorkPoint, &record.id)?;
        for dependent_id in list_work_point_dependents(&transaction, &record.id)? {
            let _ = refresh_validation_target(&transaction, EntityType::WorkPoint, &dependent_id)?;
        }
        for plan_id in list_plans_targeting_work_point(&transaction, &record.id)? {
            let _ = refresh_validation_target(&transaction, EntityType::Plan, &plan_id)?;
        }
        let validation =
            refresh_validation_target(&transaction, EntityType::Roadmap, &record.roadmap_id)?;

        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn create_plan(
        &self,
        input: CreatePlanInput,
    ) -> Result<MutationResult<PlanRecord>, PlanningStoreError> {
        require_non_empty("goalId", &input.goal_id)?;
        require_non_empty("roadmapId", &input.roadmap_id)?;
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("title", &input.title)?;
        require_non_empty("summary", &input.summary)?;
        require_non_empty("scope", &input.scope)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.scope_key.clone());
        ensure_referenced_entity_in_scope(
            &transaction,
            EntityType::Goal,
            &input.goal_id,
            "goalId",
            &active_scope_key,
        )?;
        let inherited_scope_key = ensure_referenced_entity_in_scope(
            &transaction,
            EntityType::Roadmap,
            &input.roadmap_id,
            "roadmapId",
            &active_scope_key,
        )?;

        // Preflight: GOAL-ROADMAP-MISMATCH
        {
            let roadmap = load_roadmap(&transaction, &input.roadmap_id)?;
            if roadmap.goal_id != input.goal_id {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "{}",
                    serde_json::json!({
                        "code": "GOAL_ROADMAP_MISMATCH",
                        "message": format!("plan goal `{}` does not match roadmap goal `{}`", input.goal_id, roadmap.goal_id),
                        "status": "invalid",
                    })
                )));
            }
        }

        // Preflight: WORK-POINT-MISSING and WORK-POINT-ROADMAP-MISMATCH
        for wp_id in &input.targeted_work_point_ids {
            let trimmed = wp_id.trim();
            if trimmed.is_empty() {
                continue;
            }
            match load_work_point(&transaction, trimmed) {
                Ok(wp) => {
                    if wp.roadmap_id != input.roadmap_id {
                        return Err(PlanningStoreError::InvalidInput(format!(
                            "{}",
                            serde_json::json!({
                                "code": "WORK_POINT_ROADMAP_MISMATCH",
                                "message": format!("targeted work point `{}` belongs to roadmap `{}`, not `{}`", trimmed, wp.roadmap_id, input.roadmap_id),
                                "workPointId": trimmed,
                                "status": "invalid",
                            })
                        )));
                    }
                }
                Err(PlanningStoreError::NotFound { .. }) => {
                    return Err(PlanningStoreError::InvalidInput(format!(
                        "{}",
                        serde_json::json!({
                            "code": "WORK_POINT_MISSING",
                            "message": format!("targeted work point `{}` does not exist", trimmed),
                            "workPointId": trimmed,
                            "status": "invalid",
                        })
                    )));
                }
                Err(e) => return Err(e),
            }
        }

        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let record = PlanRecord {
            id: id.clone(),
            scope_key: inherited_scope_key,
            goal_id: input.goal_id,
            roadmap_id: input.roadmap_id,
            correlation_id: input.correlation_id,
            title: input.title.trim().to_string(),
            summary: input.summary.trim().to_string(),
            scope: input.scope.trim().to_string(),
            assumptions: normalize_string_list(input.assumptions),
            stop_conditions: normalize_string_list(input.stop_conditions),
            validation_steps: normalize_string_list(input.validation_steps),
            targeted_work_point_ids: normalize_string_list(input.targeted_work_point_ids),
            effort_tier: input.effort_tier,
            routing_hint: input
                .routing_hint
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            allow_parallel_overlap: input.allow_parallel_overlap,
            file_scopes: normalize_file_scopes(input.file_scopes),
            status: input.status,
            tags: normalize_string_list(input.tags),
            revision: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        transaction.execute(
            r#"
            INSERT INTO plans (
                id, scope_key, goal_id, roadmap_id, correlation_id, title, summary, scope,
                assumptions_json, stop_conditions_json, validation_steps_json,
                targeted_work_point_ids_json, effort_tier, routing_hint, allow_parallel_overlap,
                status, tags_json, revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
            "#,
            params![
                record.id,
                record.scope_key,
                record.goal_id,
                record.roadmap_id,
                record.correlation_id,
                record.title,
                record.summary,
                record.scope,
                to_json_text(&record.assumptions)?,
                to_json_text(&record.stop_conditions)?,
                to_json_text(&record.validation_steps)?,
                to_json_text(&record.targeted_work_point_ids)?,
                record.effort_tier.as_str(),
                record.routing_hint,
                if record.allow_parallel_overlap { 1 } else { 0 },
                record.status.as_str(),
                to_json_text(&record.tags)?,
                record.revision,
                record.created_at,
                record.updated_at,
            ],
        )?;

        replace_entity_file_scopes(
            &transaction,
            &record.scope_key,
            EntityType::Plan,
            &record.id,
            &record.file_scopes,
            &now,
        )?;

        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::Plan,
                &id,
                EntityType::Plan,
                &id,
                &record.correlation_id,
                input.run_id,
                "plan.created",
                serde_json::to_value(&record)?,
            )?,
        )?;

        let validation = refresh_validation_target(&transaction, EntityType::Plan, &id)?;
        rebuild_tag_index_for_entity(&transaction, EntityType::Plan, &id, &record.tags)?;
        upsert_fts_entry(
            &transaction,
            "entities_fts",
            &id,
            &record.title,
            &record.summary,
        )?;
        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn create_todo(
        &self,
        input: CreateTodoInput,
    ) -> Result<MutationResult<TodoRecord>, PlanningStoreError> {
        require_non_empty("title", &input.title)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.scope_key.clone());
        let plan_scope_key = input
            .plan_id
            .as_ref()
            .map(|plan_id| {
                ensure_referenced_entity_in_scope(
                    &transaction,
                    EntityType::Plan,
                    plan_id,
                    "planId",
                    &active_scope_key,
                )
            })
            .transpose()?;
        let work_point_scope_key = input
            .work_point_id
            .as_ref()
            .map(|work_point_id| {
                ensure_referenced_entity_in_scope(
                    &transaction,
                    EntityType::WorkPoint,
                    work_point_id,
                    "workPointId",
                    &active_scope_key,
                )
            })
            .transpose()?;
        let scope_key = if input.plan_id.is_some() {
            plan_scope_key.unwrap_or_else(|| active_scope_key.clone())
        } else if input.work_point_id.is_some() {
            work_point_scope_key.unwrap_or_else(|| active_scope_key.clone())
        } else {
            ensure_scope_exists(&transaction, &active_scope_key)?;
            active_scope_key.clone()
        };
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let ordering_group_key = input
            .plan_id
            .as_deref()
            .unwrap_or(input.work_point_id.as_deref().unwrap_or("__global__"));
        let ordering = input
            .ordering
            .unwrap_or(next_todo_ordering(&transaction, ordering_group_key)?);
        let record = TodoRecord {
            id: id.clone(),
            scope_key,
            plan_id: input.plan_id,
            work_point_id: input.work_point_id,
            title: input.title.trim().to_string(),
            summary: input.summary.trim().to_string(),
            status: input.status,
            priority: input.priority,
            effort_tier: input.effort_tier,
            file_scopes: normalize_file_scopes(input.file_scopes),
            evidence_refs: normalize_string_list(input.evidence_refs),
            tags: normalize_string_list(input.tags),
            ordering,
            revision: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        transaction.execute(
            r#"
            INSERT INTO todos (
                id, scope_key, plan_id, work_point_id, title, summary, status, priority,
                effort_tier, evidence_refs_json, tags_json, ordering_index, revision,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
            params![
                record.id,
                record.scope_key,
                record.plan_id,
                record.work_point_id,
                record.title,
                record.summary,
                record.status.as_str(),
                record.priority.as_str(),
                record.effort_tier.as_str(),
                to_json_text(&record.evidence_refs)?,
                to_json_text(&record.tags)?,
                record.ordering,
                record.revision,
                record.created_at,
                record.updated_at,
            ],
        )?;

        replace_entity_file_scopes(
            &transaction,
            &record.scope_key,
            EntityType::Todo,
            &record.id,
            &record.file_scopes,
            &now,
        )?;

        let (aggregate_type, aggregate_id, correlation_id) = if let Some(plan_id) = &record.plan_id
        {
            (
                EntityType::Plan,
                plan_id.clone(),
                plan_correlation_id(&transaction, plan_id)?,
            )
        } else if let Some(work_point_id) = &record.work_point_id {
            (
                EntityType::WorkPoint,
                work_point_id.clone(),
                work_point_correlation_id(&transaction, work_point_id)?,
            )
        } else {
            (EntityType::Todo, id.clone(), format!("corr-{}", id))
        };
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::Todo,
                &id,
                aggregate_type,
                &aggregate_id,
                &correlation_id,
                input.run_id,
                "todo.created",
                serde_json::to_value(&record)?,
            )?,
        )?;

        let validation = refresh_validation_target(&transaction, EntityType::Todo, &id)?;
        rebuild_tag_index_for_entity(&transaction, EntityType::Todo, &id, &record.tags)?;
        upsert_fts_entry(
            &transaction,
            "entities_fts",
            &id,
            &record.title,
            &record.summary,
        )?;
        if let Some(plan_id) = &record.plan_id {
            let _ = refresh_validation_target(&transaction, EntityType::Plan, plan_id)?;
        }
        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn create_issue(
        &self,
        input: CreateIssueInput,
    ) -> Result<MutationResult<IssueRecord>, PlanningStoreError> {
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("title", &input.title)?;
        require_non_empty("summary", &input.summary)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.scope_key.clone());
        let scope_key = if let (Some(entity_type), Some(entity_id)) =
            (input.related_entity_type, input.related_entity_id.as_ref())
        {
            ensure_referenced_entity_in_scope(
                &transaction,
                entity_type,
                entity_id,
                "relatedEntityId",
                &active_scope_key,
            )?
        } else {
            ensure_scope_exists(&transaction, &active_scope_key)?;
            active_scope_key.clone()
        };
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let record = IssueRecord {
            id: id.clone(),
            scope_key,
            correlation_id: input.correlation_id,
            title: input.title.trim().to_string(),
            summary: input.summary.trim().to_string(),
            status: input.status,
            severity: input.severity,
            related_entity_type: input.related_entity_type,
            related_entity_id: input
                .related_entity_id
                .map(|value| value.trim().to_string()),
            tags: normalize_string_list(input.tags),
            revision: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        transaction.execute(
            r#"
            INSERT INTO issues (
                id, scope_key, correlation_id, title, summary, status, severity,
                related_entity_type, related_entity_id, tags_json, revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
                record.id,
                record.scope_key,
                record.correlation_id,
                record.title,
                record.summary,
                record.status.as_str(),
                record.severity.as_str(),
                record
                    .related_entity_type
                    .map(|value| value.as_str().to_string()),
                record.related_entity_id,
                to_json_text(&record.tags)?,
                record.revision,
                record.created_at,
                record.updated_at,
            ],
        )?;

        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::Issue,
                &id,
                EntityType::Issue,
                &id,
                &record.correlation_id,
                input.run_id,
                "issue.recorded",
                serde_json::to_value(&record)?,
            )?,
        )?;

        let validation = refresh_validation_target(&transaction, EntityType::Issue, &id)?;
        rebuild_tag_index_for_entity(&transaction, EntityType::Issue, &id, &record.tags)?;
        upsert_fts_entry(
            &transaction,
            "entities_fts",
            &id,
            &record.title,
            &record.summary,
        )?;
        if record.related_entity_type == Some(EntityType::Plan) {
            if let Some(related_entity_id) = &record.related_entity_id {
                let _ =
                    refresh_validation_target(&transaction, EntityType::Plan, related_entity_id)?;
            }
        }
        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn create_review_point(
        &self,
        input: CreateReviewPointInput,
    ) -> Result<MutationResult<ReviewPointRecord>, PlanningStoreError> {
        require_non_empty("attachedEntityId", &input.attached_entity_id)?;
        require_non_empty("title", &input.title)?;
        require_non_empty("summary", &input.summary)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.scope_key.clone());
        let inherited_scope_key = ensure_referenced_entity_in_scope(
            &transaction,
            input.attached_entity_type,
            &input.attached_entity_id,
            "attachedEntityId",
            &active_scope_key,
        )?;
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let record = ReviewPointRecord {
            id: id.clone(),
            scope_key: inherited_scope_key,
            attached_entity_type: input.attached_entity_type,
            attached_entity_id: input.attached_entity_id,
            title: input.title.trim().to_string(),
            summary: input.summary.trim().to_string(),
            status: input.status,
            severity: input.severity,
            revision: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        transaction.execute(
            r#"
            INSERT INTO review_points (
                id, scope_key, attached_entity_type, attached_entity_id, title, summary, status,
                severity, revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                record.id,
                record.scope_key,
                record.attached_entity_type.as_str(),
                record.attached_entity_id,
                record.title,
                record.summary,
                record.status.as_str(),
                record.severity.as_str(),
                record.revision,
                record.created_at,
                record.updated_at,
            ],
        )?;

        let correlation_id = attached_entity_correlation_id(
            &transaction,
            record.attached_entity_type,
            &record.attached_entity_id,
        )?;
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::ReviewPoint,
                &id,
                record.attached_entity_type,
                &record.attached_entity_id,
                &correlation_id,
                input.run_id,
                "review-point.recorded",
                serde_json::to_value(&record)?,
            )?,
        )?;

        let _ = refresh_validation_target(&transaction, EntityType::ReviewPoint, &id)?;
        let validation = refresh_validation_target(
            &transaction,
            record.attached_entity_type,
            &record.attached_entity_id,
        )?;
        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn create_insight(
        &self,
        input: CreateInsightInput,
    ) -> Result<MutationResult<InsightRecord>, PlanningStoreError> {
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("title", &input.title)?;
        require_non_empty("content", &input.content)?;
        // Preflight: EMPTY-CONTENT — reject whitespace-only content
        if input.content.trim().is_empty() {
            return Err(PlanningStoreError::InvalidInput(format!(
                "{}",
                serde_json::json!({
                    "code": "EMPTY_CONTENT",
                    "message": "insight content must not be empty or whitespace-only",
                    "status": "invalid",
                })
            )));
        }

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.scope_key.clone());
        let inherited_scope_key = ensure_referenced_entity_in_scope(
            &transaction,
            input.parent_entity_type,
            &input.parent_entity_id,
            "parentEntityId",
            &active_scope_key,
        )?;
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let record = InsightRecord {
            id: id.clone(),
            scope_key: inherited_scope_key,
            correlation_id: input.correlation_id,
            title: input.title.trim().to_string(),
            content: input.content.trim().to_string(),
            insight_type: input.insight_type,
            parent_entity_type: input.parent_entity_type,
            parent_entity_id: input.parent_entity_id.clone(),
            tags: normalize_string_list(input.tags),
            status: input.status,
            revision: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        transaction.execute(
            r#"
            INSERT INTO insights (
                id, scope_key, correlation_id, title, content, insight_type,
                parent_entity_type, parent_entity_id, tags_json, status,
                revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
                record.id,
                record.scope_key,
                record.correlation_id,
                record.title,
                record.content,
                record.insight_type.as_str(),
                record.parent_entity_type.as_str(),
                record.parent_entity_id,
                to_json_text(&record.tags)?,
                record.status.as_str(),
                record.revision,
                record.created_at,
                record.updated_at,
            ],
        )?;

        rebuild_tag_index_for_entity(&transaction, EntityType::Insight, &id, &record.tags)?;
        upsert_fts_entry(
            &transaction,
            "insights_fts",
            &id,
            &record.title,
            &record.content,
        )?;

        let correlation_id = attached_entity_correlation_id(
            &transaction,
            record.parent_entity_type,
            &record.parent_entity_id,
        )?;
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::Insight,
                &id,
                record.parent_entity_type,
                &record.parent_entity_id,
                &correlation_id,
                input.run_id,
                "insight.recorded",
                serde_json::to_value(&record)?,
            )?,
        )?;

        let _ = refresh_validation_target(&transaction, EntityType::Insight, &id)?;
        let validation = refresh_validation_target(
            &transaction,
            record.parent_entity_type,
            &record.parent_entity_id,
        )?;
        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn insight(&self, id: &str) -> Result<InsightView, PlanningStoreError> {
        let connection = self.open_connection()?;
        let insight = load_insight(&connection, id)?;
        let validation = load_validation_report(&connection, EntityType::Insight, id)?;
        Ok(InsightView {
            insight,
            validation,
        })
    }

    pub fn list_insights_for_entity(
        &self,
        entity_type: EntityType,
        entity_id: &str,
        scope_key: &str,
    ) -> Result<Vec<InsightRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        list_insights_for_entity_in_scope(&connection, entity_type, entity_id, scope_key)
    }

    pub fn list_insights_in_scope(
        &self,
        scope_key: &str,
    ) -> Result<Vec<InsightRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let scope_key = normalized_scope_key(Some(scope_key.to_string()));
        let mut statement = connection.prepare(
            "SELECT id, scope_key, correlation_id, title, content, insight_type, parent_entity_type, parent_entity_id, tags_json, status, revision, created_at, updated_at FROM insights WHERE scope_key = ?1 ORDER BY created_at DESC",
        )?;
        let rows = statement.query_map(params![scope_key], |row| {
            Ok(InsightRecord {
                id: row.get(0)?,
                scope_key: row.get(1)?,
                correlation_id: row.get(2)?,
                title: row.get(3)?,
                content: row.get(4)?,
                insight_type: parse_insight_type(row.get::<_, String>(5)?)?,
                parent_entity_type: parse_entity_type(row.get::<_, String>(6)?)?,
                parent_entity_id: row.get(7)?,
                tags: parse_json_column(row.get::<_, String>(8)?)?,
                status: parse_insight_status(row.get::<_, String>(9)?)?,
                revision: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })?;
        collect_rows(rows)
    }

    pub fn search_insights(
        &self,
        input: &SearchInput,
    ) -> Result<Vec<crate::SearchResult>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let scope_key = normalized_scope_key(input.scope_key.clone());
        let mut sql = String::from(
            "SELECT id, title, status, updated_at, created_at FROM insights WHERE scope_key = ?1",
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(scope_key.clone())];
        let mut param_index = 2;

        if let Some(title) = &input.title {
            sql.push_str(&format!(" AND title LIKE ?{param_index}"));
            param_values.push(Box::new(format!("%{title}%")));
            param_index += 1;
        }
        if let Some(status) = &input.status {
            sql.push_str(&format!(" AND status = ?{param_index}"));
            param_values.push(Box::new(status.clone()));
            param_index += 1;
        }
        if let Some(since) = &input.since {
            sql.push_str(&format!(" AND updated_at >= ?{param_index}"));
            param_values.push(Box::new(since.clone()));
            param_index += 1;
        }
        if let Some(tag) = &input.tag {
            sql.push_str(&format!(
                " AND id IN (SELECT entity_id FROM tag_index WHERE scope_key = ?1 AND tag = ?{param_index})"
            ));
            param_values.push(Box::new(tag.clone()));
            param_index += 1;
        }
        if let Some(fts) = &input.fts {
            if let Some(rowids) = search_entity_fts(&connection, "insights_fts", fts)? {
                if rowids.is_empty() {
                    return Ok(Vec::new());
                }
                let placeholders = rowids
                    .iter()
                    .map(|_| format!("?{param_index}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                sql.push_str(&format!(" AND id IN ({placeholders})"));
                for rowid in &rowids {
                    param_values.push(Box::new(rowid.clone()));
                    param_index += 1;
                }
            }
        }

        let _ = param_index;
        sql.push_str(" ORDER BY updated_at DESC, id ASC");
        if let Some(limit) = input.latest {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(param_values), |row| {
            Ok(crate::SearchResult {
                entity_type: "insight".to_string(),
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                updated_at: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        collect_rows(rows)
    }

    pub fn list_tags(
        &self,
        scope_key: &str,
        entity_type: Option<&str>,
    ) -> Result<Vec<TagInfo>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        let (sql, param_values): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(et) =
            entity_type
        {
            (
                "SELECT tag, COUNT(DISTINCT entity_id) AS entity_count FROM tag_index WHERE scope_key = ?1 AND entity_type = ?2 GROUP BY tag ORDER BY entity_count DESC, tag ASC".to_string(),
                vec![Box::new(normalized), Box::new(et.to_string())],
            )
        } else {
            (
                "SELECT tag, COUNT(DISTINCT entity_id) AS entity_count FROM tag_index WHERE scope_key = ?1 GROUP BY tag ORDER BY entity_count DESC, tag ASC".to_string(),
                vec![Box::new(normalized)],
            )
        };
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(param_values), |row| {
            Ok(TagInfo {
                tag: row.get(0)?,
                entity_count: row.get(1)?,
            })
        })?;
        collect_rows(rows)
    }

    pub fn search_by_tag(
        &self,
        scope_key: &str,
        tag: &str,
    ) -> Result<Vec<crate::SearchResult>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        let mut statement = connection.prepare(
            "SELECT entity_type, entity_id FROM tag_index WHERE scope_key = ?1 AND tag = ?2 ORDER BY entity_type ASC, entity_id ASC",
        )?;
        let rows = statement.query_map(params![normalized, tag], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut results = Vec::new();
        for row in rows {
            let (entity_type_str, entity_id) = row?;
            let title = resolve_search_result(&connection, &entity_type_str, &entity_id)?;
            results.push(crate::SearchResult {
                entity_type: entity_type_str,
                id: entity_id,
                title,
                status: String::new(),
                updated_at: String::new(),
                created_at: String::new(),
            });
        }
        Ok(results)
    }

    pub fn context_bundle(
        &self,
        entity_type: EntityType,
        entity_id: &str,
        scope_key: &str,
    ) -> Result<crate::EntityContextBundle, PlanningStoreError> {
        let connection = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        let entity_json = load_entity_json(&connection, entity_type, entity_id)?;
        let parent_summary = load_parent_summary(&connection, entity_type, entity_id)?;
        let children = load_children_json(&connection, entity_type, entity_id, &normalized)?;
        let insights =
            list_insights_for_entity_in_scope(&connection, entity_type, entity_id, &normalized)?;
        let related_insights = if let Some(ref parent) = parent_summary {
            if let (Some(ptype), Some(pid)) = (parent.get("entityType"), parent.get("id")) {
                if let Ok(parent_et) = ptype.as_str().unwrap_or("").parse::<EntityType>() {
                    let parent_id = pid.as_str().unwrap_or("");
                    list_insights_for_entity_in_scope(
                        &connection,
                        parent_et,
                        parent_id,
                        &normalized,
                    )
                    .unwrap_or_default()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        let tags = load_entity_tags(&connection, entity_type, entity_id)?;

        // Load active discoveries for this entity
        let mut disc_stmt = connection.prepare(
            "SELECT dn.id FROM discovery_nodes dn
             INNER JOIN discovery_relationships dr ON dr.source_id = dn.id
             WHERE dr.target_id = ?1 AND dr.relationship_kind = 'applies-to'
             AND dn.status IN ('candidate', 'triaged', 'reopened')
             AND dn.scope_key = ?2
             ORDER BY CASE dn.severity 
               WHEN 'critical' THEN 0 
               WHEN 'high' THEN 1 
               WHEN 'medium' THEN 2 
               ELSE 3 
             END, dn.created_at DESC
             LIMIT 20",
        )?;
        let disc_rows = disc_stmt.query_map(params![entity_id, normalized], |row| {
            row.get::<_, String>(0)
        })?;
        let mut entity_discoveries = Vec::new();
        for disc_id in disc_rows.flatten() {
            if let Ok(disc) = load_discovery(&connection, &disc_id) {
                entity_discoveries.push(disc);
            }
        }

        let validation = load_validation_report(&connection, entity_type, entity_id)?;
        let entity_tokens = estimate_tokens(&entity_json.to_string());
        let children_tokens: usize = children
            .iter()
            .map(|c| estimate_tokens(&c.to_string()))
            .sum();
        let insight_tokens: usize = insights.iter().map(|i| estimate_tokens(&i.content)).sum();
        let related_tokens: usize = related_insights
            .iter()
            .map(|i| estimate_tokens(&i.content))
            .sum();
        Ok(crate::EntityContextBundle {
            entity_type,
            entity_id: entity_id.to_string(),
            entity: entity_json,
            parent_summary,
            children,
            insights,
            related_insights,
            tags,
            discoveries: entity_discoveries,
            validation,
            token_estimate: crate::TokenEstimate {
                entity_tokens,
                related_tokens: children_tokens,
                insight_tokens: insight_tokens + related_tokens,
                total_tokens: entity_tokens + children_tokens + insight_tokens + related_tokens,
            },
        })
    }

    pub fn session_context(
        &self,
        correlation_id: &str,
        scope_key: &str,
    ) -> Result<crate::SessionContextBundle, PlanningStoreError> {
        let connection = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        let mut statement = connection.prepare(
            "SELECT DISTINCT entity_type, entity_id FROM planning_events WHERE correlation_id = ?1 AND scope_key = ?2 ORDER BY rowid ASC",
        )?;
        let rows = statement.query_map(params![correlation_id, normalized], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut entities_touched = Vec::new();
        for row in rows {
            let (entity_type_str, entity_id) = row?;
            let title = resolve_search_result(&connection, &entity_type_str, &entity_id)?;
            entities_touched.push(crate::SearchResult {
                entity_type: entity_type_str,
                id: entity_id,
                title,
                status: String::new(),
                updated_at: String::new(),
                created_at: String::new(),
            });
        }
        let mut insight_statement = connection.prepare(
            "SELECT id, scope_key, correlation_id, title, content, insight_type, parent_entity_type, parent_entity_id, tags_json, status, revision, created_at, updated_at FROM insights WHERE correlation_id = ?1 AND scope_key = ?2 ORDER BY created_at ASC, id ASC",
        )?;
        let insight_rows =
            insight_statement.query_map(params![correlation_id, normalized], row_to_insight)?;
        let insights_recorded = collect_rows(insight_rows)?;
        let mut error_count = 0usize;
        let mut warning_count = 0usize;
        for entity in &entities_touched {
            if let Ok(et) = entity.entity_type.parse::<EntityType>() {
                if let Ok(report) = load_validation_report(&connection, et, &entity.id) {
                    error_count += report
                        .findings
                        .iter()
                        .filter(|f| f.severity == ValidationSeverity::Error)
                        .count();
                    warning_count += report
                        .findings
                        .iter()
                        .filter(|f| f.severity == ValidationSeverity::Warning)
                        .count();
                }
            }
        }
        let validation_summary = crate::SessionValidationSummary {
            error_count,
            warning_count,
        };
        let entity_tokens: usize = entities_touched
            .iter()
            .map(|e| estimate_tokens(&e.title))
            .sum();
        let insight_tokens: usize = insights_recorded
            .iter()
            .map(|i| estimate_tokens(&i.content))
            .sum();
        let total_tokens = entity_tokens + insight_tokens;

        // Phase 6: Extended session context fields

        // Active project runs for this scope
        let lease_now = now_string()?;
        let mut project_run_stmt = connection.prepare(
            "SELECT id FROM project_runs WHERE scope_key = ?1 AND status IN ('claimed', 'active', 'interrupted') AND julianday(lease_expires_at) > julianday(?2) AND session_id IS NOT NULL ORDER BY claimed_at DESC LIMIT 10",
        )?;
        let run_rows = project_run_stmt.query_map(params![normalized, lease_now], |row| {
            row.get::<_, String>(0)
        })?;
        let mut active_project_runs = Vec::new();
        for run_id in run_rows.flatten() {
            if let Ok(run) = load_project_run(&connection, &run_id) {
                active_project_runs.push(run);
            }
        }

        // Active work points
        let mut wp_stmt = connection.prepare(
            "SELECT id FROM work_points WHERE scope_key = ?1 AND status = 'active' ORDER BY ordering_index ASC LIMIT 10",
        )?;
        let wp_rows = wp_stmt.query_map(params![normalized], |row| row.get::<_, String>(0))?;
        let mut active_work_points = Vec::new();
        for wp_id in wp_rows.flatten() {
            if let Ok(wp) = load_work_point(&connection, &wp_id) {
                active_work_points.push(wp);
            }
        }

        // Active plans
        let mut plan_stmt = connection.prepare(
            "SELECT id FROM plans WHERE scope_key = ?1 AND status = 'active' ORDER BY created_at DESC LIMIT 10",
        )?;
        let plan_rows = plan_stmt.query_map(params![normalized], |row| row.get::<_, String>(0))?;
        let mut active_plans = Vec::new();
        for plan_id in plan_rows.flatten() {
            if let Ok(plan) = load_plan(&connection, &plan_id) {
                active_plans.push(plan);
            }
        }

        // Next pending todos
        let mut todo_stmt = connection.prepare(
            "SELECT id FROM todos WHERE scope_key = ?1 AND status = 'pending' ORDER BY ordering_index ASC, id ASC LIMIT 10",
        )?;
        let todo_rows = todo_stmt.query_map(params![normalized], |row| row.get::<_, String>(0))?;
        let mut next_pending_todos = Vec::new();
        for todo_id in todo_rows.flatten() {
            if let Ok(todo) = load_todo(&connection, &todo_id) {
                next_pending_todos.push(todo);
            }
        }

        // Open blocking issues (severity high/critical)
        let mut issue_stmt = connection.prepare(
            "SELECT id FROM issues WHERE scope_key = ?1 AND status IN ('open', 'reopened') AND severity IN ('high', 'critical') ORDER BY CASE severity WHEN 'critical' THEN 0 ELSE 1 END, created_at DESC LIMIT 10",
        )?;
        let issue_rows =
            issue_stmt.query_map(params![normalized], |row| row.get::<_, String>(0))?;
        let mut open_blocking_issues = Vec::new();
        for issue_id in issue_rows.flatten() {
            if let Ok(issue) = load_issue(&connection, &issue_id) {
                open_blocking_issues.push(issue);
            }
        }

        // Open blocking review points (severity high/critical)
        let mut rp_stmt = connection.prepare(
            "SELECT id FROM review_points WHERE scope_key = ?1 AND status = 'open' AND severity IN ('high', 'critical') ORDER BY CASE severity WHEN 'critical' THEN 0 ELSE 1 END, created_at DESC LIMIT 10",
        )?;
        let rp_rows = rp_stmt.query_map(params![normalized], |row| row.get::<_, String>(0))?;
        let mut open_blocking_review_points = Vec::new();
        for rp_id in rp_rows.flatten() {
            if let Ok(rp) = load_review_point(&connection, &rp_id) {
                open_blocking_review_points.push(rp);
            }
        }

        // Active discoveries in scope (unresolved, ordered by severity)
        let mut disc_stmt = connection.prepare(
            "SELECT id FROM discovery_nodes WHERE scope_key = ?1 AND status IN ('candidate', 'triaged', 'reopened') ORDER BY CASE severity WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END, created_at DESC LIMIT 20",
        )?;
        let disc_rows = disc_stmt.query_map(params![normalized], |row| row.get::<_, String>(0))?;
        let mut active_discoveries = Vec::new();
        for disc_id in disc_rows.flatten() {
            if let Ok(disc) = load_discovery(&connection, &disc_id) {
                active_discoveries.push(disc);
            }
        }

        // Recommended next action
        let incomplete_todo_count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM todos WHERE scope_key = ?1 AND status IN ('pending', 'in-progress', 'blocked')",
            params![normalized],
            |row| row.get(0),
        )?;

        let recommended_next_action =
            if !active_project_runs.is_empty() && incomplete_todo_count > 0 {
                let run_title = active_project_runs
                    .first()
                    .and_then(|r| {
                        load_work_point(&connection, &r.work_point_id)
                            .ok()
                            .map(|wp| wp.title)
                    })
                    .unwrap_or_default();
                let todo_title = next_pending_todos
                    .first()
                    .map(|t| t.title.clone())
                    .unwrap_or_default();
                Some(format!("continue {}: {}", run_title, todo_title))
            } else if active_project_runs.is_empty() {
                // Check for runnable work points - try first roadmap
                let roadmap_result: Option<String> = connection
                    .query_row(
                        "SELECT id FROM roadmaps WHERE scope_key = ?1 LIMIT 1",
                        params![normalized],
                        |row| row.get(0),
                    )
                    .ok();
                if let Some(rid) = roadmap_result {
                    if let Ok(runnable) = self.find_runnable_work_points(&rid) {
                        runnable
                            .candidates
                            .first()
                            .map(|first| format!("claim {}", first.work_point.title))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

        let recommended_next_action = recommended_next_action.or_else(|| {
            if !open_blocking_issues.is_empty() || !open_blocking_review_points.is_empty() {
                let blocker_title = open_blocking_issues
                    .first()
                    .map(|i| i.title.clone())
                    .or_else(|| {
                        open_blocking_review_points
                            .first()
                            .map(|rp| rp.title.clone())
                    })
                    .unwrap_or_default();
                Some(format!("resolve {}", blocker_title))
            } else {
                Some("review open goals or create a new plan".to_string())
            }
        });

        // Context warnings
        let mut context_warnings = Vec::new();
        if active_project_runs.is_empty() {
            context_warnings
                .push("No active project run. Use project-run claim to start work.".to_string());
        }
        if active_plans.is_empty() {
            context_warnings.push("No active plan. Create a plan with plan create.".to_string());
        }
        let issue_count = open_blocking_issues.len();
        let rp_count = open_blocking_review_points.len();
        if issue_count > 0 || rp_count > 0 {
            context_warnings.push(format!(
                "Blocked: {} unresolved high/critical issue(s) and {} open review point(s).",
                issue_count, rp_count
            ));
        }

        Ok(crate::SessionContextBundle {
            session_id: None,
            correlation_id: Some(correlation_id.to_string()),
            entities_touched,
            insights_recorded,
            validation_summary,
            token_estimate: crate::TokenEstimate {
                entity_tokens,
                related_tokens: 0,
                insight_tokens,
                total_tokens,
            },
            active_project_runs,
            active_work_points,
            active_plans,
            next_pending_todos,
            open_blocking_issues,
            open_blocking_review_points,
            recommended_next_action,
            context_warnings,
            active_discoveries,
        })
    }

    pub fn search_goals(
        &self,
        input: &SearchInput,
    ) -> Result<Vec<crate::SearchResult>, PlanningStoreError> {
        let connection = self.open_connection()?;
        search_entity(&connection, "goals", "goal", input)
    }

    pub fn search_roadmaps(
        &self,
        input: &SearchInput,
    ) -> Result<Vec<crate::SearchResult>, PlanningStoreError> {
        let connection = self.open_connection()?;
        search_entity(&connection, "roadmaps", "roadmap", input)
    }

    pub fn search_plans(
        &self,
        input: &SearchInput,
    ) -> Result<Vec<crate::SearchResult>, PlanningStoreError> {
        let connection = self.open_connection()?;
        search_entity(&connection, "plans", "plan", input)
    }

    pub fn search_todos(
        &self,
        input: &SearchInput,
    ) -> Result<Vec<crate::SearchResult>, PlanningStoreError> {
        let connection = self.open_connection()?;
        search_entity(&connection, "todos", "todo", input)
    }

    pub fn search_issues(
        &self,
        input: &SearchInput,
    ) -> Result<Vec<crate::SearchResult>, PlanningStoreError> {
        let connection = self.open_connection()?;
        search_entity(&connection, "issues", "issue", input)
    }

    pub fn search_all(
        &self,
        input: &SearchInput,
    ) -> Result<Vec<crate::SearchResult>, PlanningStoreError> {
        let mut results = Vec::new();
        results.extend(self.search_goals(input)?);
        results.extend(self.search_roadmaps(input)?);
        results.extend(self.search_plans(input)?);
        results.extend(self.search_todos(input)?);
        results.extend(self.search_issues(input)?);
        results.extend(self.search_insights(input)?);
        Ok(results)
    }

    pub fn create_scope(
        &self,
        input: CreateScopeInput,
    ) -> Result<MutationResult<ScopeRecord>, PlanningStoreError> {
        require_non_empty("scopeKey", &input.scope_key)?;
        let scope_key = normalize_scope_key_value(&input.scope_key);
        let parent_scope_key = input
            .parent_scope_key
            .map(|value| normalize_scope_key_value(&value));
        if let Some(parent) = parent_scope_key.as_ref() {
            if parent == &scope_key {
                return Err(PlanningStoreError::InvalidInput(
                    "parentScopeKey cannot be the same as scopeKey".to_string(),
                ));
            }
        }

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        if let Some(parent) = parent_scope_key.as_ref() {
            ensure_scope_exists(&transaction, parent)?;
        }

        let now = now_string()?;
        let record = ScopeRecord {
            scope_key: scope_key.clone(),
            scope_type: input.scope_type.map(|value| value.trim().to_string()),
            parent_scope_key,
            metadata: input.metadata.unwrap_or_else(|| serde_json::json!({})),
            tags: normalize_string_list(input.tags),
            revision: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        transaction.execute(
            r#"
            INSERT INTO scopes (
                scope_key, scope_type, parent_scope_key, metadata_json, tags_json, revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                record.scope_key,
                record.scope_type,
                record.parent_scope_key,
                record.metadata.to_string(),
                to_json_text(&record.tags)?,
                record.revision,
                record.created_at,
                record.updated_at,
            ],
        )?;

        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::Scope,
                &scope_key,
                EntityType::Scope,
                &scope_key,
                &format!("corr-scope-{scope_key}"),
                input.run_id,
                "scope.created",
                serde_json::to_value(&record)?,
            )?,
        )?;

        transaction.commit()?;
        Ok(MutationResult {
            record,
            validation: ValidationReport::from_findings(Vec::new()),
        })
    }

    pub fn scope(&self, scope_key: &str) -> Result<ScopeRecord, PlanningStoreError> {
        let connection = self.open_connection()?;
        load_scope(&connection, scope_key)
    }

    pub fn list_scopes(&self) -> Result<Vec<ScopeRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT scope_key, scope_type, parent_scope_key, metadata_json, tags_json, revision, created_at, updated_at FROM scopes ORDER BY scope_key ASC",
        )?;
        let rows = statement.query_map([], row_to_scope)?;
        collect_rows(rows)
    }
}

fn allowed_transitions(
    entity_type: EntityType,
    current: &str,
) -> Result<Vec<&'static str>, PlanningStoreError> {
    let allowed: Vec<&str> = match entity_type {
        EntityType::Goal => match current {
            "draft" => vec!["proposed", "abandoned"],
            "proposed" => vec!["active", "abandoned"],
            "active" => vec!["validated", "invalidated", "abandoned"],
            "validated" => vec!["superseded"],
            "invalidated" => vec!["active", "abandoned"],
            "superseded" => vec![],
            "abandoned" => vec!["draft"],
            other => {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "unknown goal status: {other}"
                )))
            }
        },
        EntityType::Roadmap => match current {
            "draft" => vec!["proposed", "cancelled"],
            "proposed" => vec!["active", "cancelled"],
            "active" => vec!["blocked", "completed", "cancelled"],
            "blocked" => vec!["active", "cancelled"],
            "completed" => vec![],
            "cancelled" => vec!["draft"],
            "invalidated" => vec!["draft"],
            other => {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "unknown roadmap status: {other}"
                )))
            }
        },
        EntityType::WorkPoint => match current {
            "draft" => vec!["proposed", "cancelled"],
            "proposed" => vec!["active", "cancelled"],
            "active" => vec!["blocked", "completed", "cancelled"],
            "blocked" => vec!["active", "cancelled"],
            "completed" => vec![],
            "cancelled" => vec!["draft"],
            "invalidated" => vec!["draft"],
            other => {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "unknown work point status: {other}"
                )))
            }
        },
        EntityType::Plan => match current {
            "draft" => vec!["proposed", "cancelled"],
            "proposed" => vec!["active", "cancelled"],
            "active" => vec!["blocked", "completed", "cancelled"],
            "blocked" => vec!["active", "cancelled"],
            "completed" => vec![],
            "cancelled" => vec!["draft"],
            "invalidated" => vec!["draft"],
            other => {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "unknown plan status: {other}"
                )))
            }
        },
        EntityType::Todo => match current {
            "pending" => vec!["in-progress", "cancelled"],
            "in-progress" => vec!["blocked", "completed", "cancelled"],
            "blocked" => vec!["pending", "in-progress", "cancelled"],
            "completed" => vec![],
            "cancelled" => vec!["pending"],
            other => {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "unknown todo status: {other}"
                )))
            }
        },
        EntityType::Issue => match current {
            "open" => vec!["blocked", "resolved"],
            "blocked" => vec!["open", "resolved"],
            "resolved" => vec!["reopened"],
            "reopened" => vec!["open", "blocked", "resolved"],
            other => {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "unknown issue status: {other}"
                )))
            }
        },
        EntityType::ReviewPoint => match current {
            "open" => vec!["resolved", "accepted-risk"],
            "resolved" => vec![],
            "accepted-risk" => vec![],
            other => {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "unknown review point status: {other}"
                )))
            }
        },
        EntityType::Insight => match current {
            "active" => vec!["superseded", "archived"],
            "superseded" => vec!["active", "archived"],
            "archived" => vec!["active"],
            other => {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "unknown insight status: {other}"
                )))
            }
        },
        EntityType::RoadmapSection
        | EntityType::Scope
        | EntityType::DiscoveryNode
        | EntityType::DiscoveryRelationship
        | EntityType::DiscoveryCheckpoint => {
            return Err(PlanningStoreError::InvalidInput(format!(
                "status transitions are not supported for {}",
                entity_type.as_str()
            )));
        }
        other => {
            return Err(PlanningStoreError::InvalidInput(format!(
                "unsupported entity type: {}",
                other.as_str()
            )))
        }
    };
    Ok(allowed)
}

impl PlanningStore {
    pub fn update_status(
        &self,
        input: UpdateStatusInput,
    ) -> Result<serde_json::Value, PlanningStoreError> {
        require_non_empty("entityId", &input.entity_id)?;
        require_non_empty("status", &input.status)?;
        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let now = now_string()?;
        let active_scope_key = normalized_scope_key(input.active_scope_key.clone());

        match input.entity_type {
            EntityType::RoadmapSection
            | EntityType::Scope
            | EntityType::DiscoveryNode
            | EntityType::DiscoveryRelationship
            | EntityType::DiscoveryCheckpoint => {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "status transitions are not supported for {}",
                    input.entity_type.as_str()
                )));
            }
            _ => ensure_entity_in_scope(
                &transaction,
                input.entity_type,
                &input.entity_id,
                &active_scope_key,
            )?,
        };

        let result = match input.entity_type {
            EntityType::Goal => {
                let status = parse_goal_status(input.status.clone())?;
                let old_record = load_goal(&transaction, &input.entity_id)?;
                if !input.override_transition {
                    let allowed =
                        allowed_transitions(EntityType::Goal, old_record.status.as_str())?;
                    if !allowed.contains(&status.as_str()) {
                        return Err(PlanningStoreError::InvalidInput(
                            serde_json::to_string_pretty(&serde_json::json!({
                                "code": "INVALID_STATUS_TRANSITION",
                                "entityType": "goal",
                                "entityId": input.entity_id,
                                "currentStatus": old_record.status.as_str(),
                                "requestedStatus": status.as_str(),
                                "allowedTransitions": allowed,
                            }))
                            .unwrap_or_default(),
                        ));
                    }
                }
                update_status_row(
                    &transaction,
                    "goals",
                    &input.entity_id,
                    status.as_str(),
                    &now,
                )?;
                let record = load_goal(&transaction, &input.entity_id)?;
                let event_type = if input.override_transition {
                    "goal.status-overridden"
                } else {
                    "goal.status-updated"
                };
                let mut payload = serde_json::json!({ "status": record.status.as_str(), "revision": record.revision });
                if input.override_transition {
                    payload["reason"] = serde_json::json!(input.reason.clone().unwrap_or_default());
                    payload["overridden"] = serde_json::json!(true);
                }
                append_event(
                    &transaction,
                    build_event(
                        &transaction,
                        EntityType::Goal,
                        &record.id,
                        EntityType::Goal,
                        &record.id,
                        &record.correlation_id,
                        input.run_id.clone(),
                        event_type,
                        payload,
                    )?,
                )?;
                let validation =
                    refresh_validation_target(&transaction, EntityType::Goal, &record.id)?;
                serde_json::json!({ "record": record, "validation": validation })
            }
            EntityType::Roadmap => {
                let status = parse_roadmap_status(input.status.clone())?;
                let old_record = load_roadmap(&transaction, &input.entity_id)?;
                if !input.override_transition {
                    let allowed =
                        allowed_transitions(EntityType::Roadmap, old_record.status.as_str())?;
                    if !allowed.contains(&status.as_str()) {
                        return Err(PlanningStoreError::InvalidInput(
                            serde_json::to_string_pretty(&serde_json::json!({
                                "code": "INVALID_STATUS_TRANSITION",
                                "entityType": "roadmap",
                                "entityId": input.entity_id,
                                "currentStatus": old_record.status.as_str(),
                                "requestedStatus": status.as_str(),
                                "allowedTransitions": allowed,
                            }))
                            .unwrap_or_default(),
                        ));
                    }
                }
                update_status_row(
                    &transaction,
                    "roadmaps",
                    &input.entity_id,
                    status.as_str(),
                    &now,
                )?;
                let record = load_roadmap(&transaction, &input.entity_id)?;
                let event_type = if input.override_transition {
                    "roadmap.status-overridden"
                } else {
                    "roadmap.status-updated"
                };
                let mut payload = serde_json::json!({ "status": record.status.as_str(), "revision": record.revision });
                if input.override_transition {
                    payload["reason"] = serde_json::json!(input.reason.clone().unwrap_or_default());
                    payload["overridden"] = serde_json::json!(true);
                }
                append_event(
                    &transaction,
                    build_event(
                        &transaction,
                        EntityType::Roadmap,
                        &record.id,
                        EntityType::Roadmap,
                        &record.id,
                        &record.correlation_id,
                        input.run_id.clone(),
                        event_type,
                        payload,
                    )?,
                )?;
                let validation =
                    refresh_validation_target(&transaction, EntityType::Roadmap, &record.id)?;
                serde_json::json!({ "record": record, "validation": validation })
            }
            EntityType::WorkPoint => {
                let status = parse_work_point_status(input.status.clone())?;
                let old_record = load_work_point(&transaction, &input.entity_id)?;
                if !input.override_transition {
                    let allowed =
                        allowed_transitions(EntityType::WorkPoint, old_record.status.as_str())?;
                    if !allowed.contains(&status.as_str()) {
                        return Err(PlanningStoreError::InvalidInput(
                            serde_json::to_string_pretty(&serde_json::json!({
                                "code": "INVALID_STATUS_TRANSITION",
                                "entityType": "work-point",
                                "entityId": input.entity_id,
                                "currentStatus": old_record.status.as_str(),
                                "requestedStatus": status.as_str(),
                                "allowedTransitions": allowed,
                            }))
                            .unwrap_or_default(),
                        ));
                    }
                }
                update_status_row(
                    &transaction,
                    "work_points",
                    &input.entity_id,
                    status.as_str(),
                    &now,
                )?;
                let record = load_work_point(&transaction, &input.entity_id)?;
                let correlation_id = roadmap_correlation_id(&transaction, &record.roadmap_id)?;
                let event_type = if input.override_transition {
                    "work-point.status-overridden"
                } else {
                    "work-point.status-updated"
                };
                let mut payload = serde_json::json!({ "status": record.status.as_str(), "revision": record.revision });
                if input.override_transition {
                    payload["reason"] = serde_json::json!(input.reason.clone().unwrap_or_default());
                    payload["overridden"] = serde_json::json!(true);
                }
                append_event(
                    &transaction,
                    build_event(
                        &transaction,
                        EntityType::WorkPoint,
                        &record.id,
                        EntityType::Roadmap,
                        &record.roadmap_id,
                        &correlation_id,
                        input.run_id.clone(),
                        event_type,
                        payload,
                    )?,
                )?;
                let validation =
                    refresh_validation_target(&transaction, EntityType::WorkPoint, &record.id)?;
                let _ = refresh_validation_target(
                    &transaction,
                    EntityType::Roadmap,
                    &record.roadmap_id,
                )?;
                serde_json::json!({ "record": record, "validation": validation })
            }
            EntityType::Plan => {
                let status = parse_plan_status(input.status.clone())?;
                let old_record = load_plan(&transaction, &input.entity_id)?;
                if !input.override_transition {
                    let allowed =
                        allowed_transitions(EntityType::Plan, old_record.status.as_str())?;
                    if !allowed.contains(&status.as_str()) {
                        return Err(PlanningStoreError::InvalidInput(
                            serde_json::to_string_pretty(&serde_json::json!({
                                "code": "INVALID_STATUS_TRANSITION",
                                "entityType": "plan",
                                "entityId": input.entity_id,
                                "currentStatus": old_record.status.as_str(),
                                "requestedStatus": status.as_str(),
                                "allowedTransitions": allowed,
                            }))
                            .unwrap_or_default(),
                        ));
                    }
                }
                update_status_row(
                    &transaction,
                    "plans",
                    &input.entity_id,
                    status.as_str(),
                    &now,
                )?;
                let record = load_plan(&transaction, &input.entity_id)?;
                let event_type = if input.override_transition {
                    "plan.status-overridden"
                } else {
                    "plan.status-updated"
                };
                let mut payload = serde_json::json!({ "status": record.status.as_str(), "revision": record.revision });
                if input.override_transition {
                    payload["reason"] = serde_json::json!(input.reason.clone().unwrap_or_default());
                    payload["overridden"] = serde_json::json!(true);
                }
                append_event(
                    &transaction,
                    build_event(
                        &transaction,
                        EntityType::Plan,
                        &record.id,
                        EntityType::Plan,
                        &record.id,
                        &record.correlation_id,
                        input.run_id.clone(),
                        event_type,
                        payload,
                    )?,
                )?;
                let validation =
                    refresh_validation_target(&transaction, EntityType::Plan, &record.id)?;
                serde_json::json!({ "record": record, "validation": validation })
            }
            EntityType::Todo => {
                let status = parse_todo_status(input.status.clone())?;
                let old_record = load_todo(&transaction, &input.entity_id)?;
                if !input.override_transition {
                    let allowed =
                        allowed_transitions(EntityType::Todo, old_record.status.as_str())?;
                    if !allowed.contains(&status.as_str()) {
                        return Err(PlanningStoreError::InvalidInput(
                            serde_json::to_string_pretty(&serde_json::json!({
                                "code": "INVALID_STATUS_TRANSITION",
                                "entityType": "todo",
                                "entityId": input.entity_id,
                                "currentStatus": old_record.status.as_str(),
                                "requestedStatus": status.as_str(),
                                "allowedTransitions": allowed,
                            }))
                            .unwrap_or_default(),
                        ));
                    }
                }
                update_todo_status_row(
                    &transaction,
                    &input.entity_id,
                    status.as_str(),
                    input.evidence_refs.unwrap_or_default(),
                    &now,
                )?;
                let record = load_todo(&transaction, &input.entity_id)?;
                let correlation_id = if let Some(plan_id) = &record.plan_id {
                    plan_correlation_id(&transaction, plan_id)?
                } else if let Some(work_point_id) = &record.work_point_id {
                    work_point_correlation_id(&transaction, work_point_id)?
                } else {
                    format!("corr-{}", record.id)
                };
                let event_type = if input.override_transition {
                    "todo.status-overridden"
                } else {
                    "todo.status-updated"
                };
                let mut payload = serde_json::json!({
                    "status": record.status.as_str(),
                    "evidenceRefs": record.evidence_refs,
                    "revision": record.revision
                });
                if input.override_transition {
                    payload["reason"] = serde_json::json!(input.reason.clone().unwrap_or_default());
                    payload["overridden"] = serde_json::json!(true);
                }
                append_event(
                    &transaction,
                    build_event(
                        &transaction,
                        EntityType::Todo,
                        &record.id,
                        EntityType::Todo,
                        &record.id,
                        &correlation_id,
                        input.run_id.clone(),
                        event_type,
                        payload,
                    )?,
                )?;
                let validation =
                    refresh_validation_target(&transaction, EntityType::Todo, &record.id)?;
                if let Some(plan_id) = &record.plan_id {
                    let _ = refresh_validation_target(&transaction, EntityType::Plan, plan_id)?;
                }
                serde_json::json!({ "record": record, "validation": validation })
            }
            EntityType::Issue => {
                let status = parse_issue_status(input.status.clone())?;
                let old_record = load_issue(&transaction, &input.entity_id)?;
                if !input.override_transition {
                    let allowed =
                        allowed_transitions(EntityType::Issue, old_record.status.as_str())?;
                    if !allowed.contains(&status.as_str()) {
                        return Err(PlanningStoreError::InvalidInput(
                            serde_json::to_string_pretty(&serde_json::json!({
                                "code": "INVALID_STATUS_TRANSITION",
                                "entityType": "issue",
                                "entityId": input.entity_id,
                                "currentStatus": old_record.status.as_str(),
                                "requestedStatus": status.as_str(),
                                "allowedTransitions": allowed,
                            }))
                            .unwrap_or_default(),
                        ));
                    }
                }
                update_status_row(
                    &transaction,
                    "issues",
                    &input.entity_id,
                    status.as_str(),
                    &now,
                )?;
                let record = load_issue(&transaction, &input.entity_id)?;
                let event_type = if input.override_transition {
                    "issue.status-overridden"
                } else {
                    "issue.status-updated"
                };
                let mut payload = serde_json::json!({ "status": record.status.as_str(), "revision": record.revision });
                if input.override_transition {
                    payload["reason"] = serde_json::json!(input.reason.clone().unwrap_or_default());
                    payload["overridden"] = serde_json::json!(true);
                }
                append_event(
                    &transaction,
                    build_event(
                        &transaction,
                        EntityType::Issue,
                        &record.id,
                        EntityType::Issue,
                        &record.id,
                        &record.correlation_id,
                        input.run_id.clone(),
                        event_type,
                        payload,
                    )?,
                )?;
                let validation =
                    refresh_validation_target(&transaction, EntityType::Issue, &record.id)?;
                if record.related_entity_type == Some(EntityType::Plan) {
                    if let Some(related_entity_id) = &record.related_entity_id {
                        let _ = refresh_validation_target(
                            &transaction,
                            EntityType::Plan,
                            related_entity_id,
                        )?;
                    }
                }
                serde_json::json!({ "record": record, "validation": validation })
            }
            EntityType::ReviewPoint => {
                let status = parse_review_point_status(input.status.clone())?;
                let old_record = load_review_point(&transaction, &input.entity_id)?;
                if !input.override_transition {
                    let allowed =
                        allowed_transitions(EntityType::ReviewPoint, old_record.status.as_str())?;
                    if !allowed.contains(&status.as_str()) {
                        return Err(PlanningStoreError::InvalidInput(
                            serde_json::to_string_pretty(&serde_json::json!({
                                "code": "INVALID_STATUS_TRANSITION",
                                "entityType": "review-point",
                                "entityId": input.entity_id,
                                "currentStatus": old_record.status.as_str(),
                                "requestedStatus": status.as_str(),
                                "allowedTransitions": allowed,
                            }))
                            .unwrap_or_default(),
                        ));
                    }
                }
                update_status_row(
                    &transaction,
                    "review_points",
                    &input.entity_id,
                    status.as_str(),
                    &now,
                )?;
                let record = load_review_point(&transaction, &input.entity_id)?;
                let correlation_id = attached_entity_correlation_id(
                    &transaction,
                    record.attached_entity_type,
                    &record.attached_entity_id,
                )?;
                let event_type = if input.override_transition {
                    "review-point.status-overridden"
                } else {
                    "review-point.status-updated"
                };
                let mut payload = serde_json::json!({ "status": record.status.as_str(), "revision": record.revision });
                if input.override_transition {
                    payload["reason"] = serde_json::json!(input.reason.clone().unwrap_or_default());
                    payload["overridden"] = serde_json::json!(true);
                }
                append_event(
                    &transaction,
                    build_event(
                        &transaction,
                        EntityType::ReviewPoint,
                        &record.id,
                        record.attached_entity_type,
                        &record.attached_entity_id,
                        &correlation_id,
                        input.run_id.clone(),
                        event_type,
                        payload,
                    )?,
                )?;
                let validation =
                    refresh_validation_target(&transaction, EntityType::ReviewPoint, &record.id)?;
                let _ = refresh_validation_target(
                    &transaction,
                    record.attached_entity_type,
                    &record.attached_entity_id,
                )?;
                serde_json::json!({ "record": record, "validation": validation })
            }
            EntityType::Insight => {
                let status = parse_insight_status(input.status.clone())?;
                let old_record = load_insight(&transaction, &input.entity_id)?;
                if !input.override_transition {
                    let allowed =
                        allowed_transitions(EntityType::Insight, old_record.status.as_str())?;
                    if !allowed.contains(&status.as_str()) {
                        return Err(PlanningStoreError::InvalidInput(
                            serde_json::to_string_pretty(&serde_json::json!({
                                "code": "INVALID_STATUS_TRANSITION",
                                "entityType": "insight",
                                "entityId": input.entity_id,
                                "currentStatus": old_record.status.as_str(),
                                "requestedStatus": status.as_str(),
                                "allowedTransitions": allowed,
                            }))
                            .unwrap_or_default(),
                        ));
                    }
                }
                update_status_row(
                    &transaction,
                    "insights",
                    &input.entity_id,
                    status.as_str(),
                    &now,
                )?;
                let record = load_insight(&transaction, &input.entity_id)?;
                let correlation_id = attached_entity_correlation_id(
                    &transaction,
                    record.parent_entity_type,
                    &record.parent_entity_id,
                )?;
                let event_type = if input.override_transition {
                    "insight.status-overridden"
                } else {
                    "insight.status-updated"
                };
                let mut payload = serde_json::json!({ "status": record.status.as_str(), "revision": record.revision });
                if input.override_transition {
                    payload["reason"] = serde_json::json!(input.reason.clone().unwrap_or_default());
                    payload["overridden"] = serde_json::json!(true);
                }
                append_event(
                    &transaction,
                    build_event(
                        &transaction,
                        EntityType::Insight,
                        &record.id,
                        record.parent_entity_type,
                        &record.parent_entity_id,
                        &correlation_id,
                        input.run_id.clone(),
                        event_type,
                        payload,
                    )?,
                )?;
                let validation =
                    refresh_validation_target(&transaction, EntityType::Insight, &record.id)?;
                let _ = refresh_validation_target(
                    &transaction,
                    record.parent_entity_type,
                    &record.parent_entity_id,
                )?;
                serde_json::json!({ "record": record, "validation": validation })
            }
            EntityType::RoadmapSection
            | EntityType::Scope
            | EntityType::ProjectRun
            | EntityType::GraphNode
            | EntityType::GraphEdge
            | EntityType::DiscoveryNode
            | EntityType::DiscoveryRelationship
            | EntityType::DiscoveryCheckpoint => {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "status transitions are not supported for {}",
                    input.entity_type.as_str()
                )));
            }
        };

        transaction.commit()?;
        Ok(result)
    }

    pub fn revise_plan(
        &self,
        input: RevisePlanInput,
    ) -> Result<MutationResult<PlanRecord>, PlanningStoreError> {
        require_non_empty("planId", &input.plan_id)?;
        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.active_scope_key.clone());
        ensure_entity_in_scope(
            &transaction,
            EntityType::Plan,
            &input.plan_id,
            &active_scope_key,
        )?;
        let existing = load_plan(&transaction, &input.plan_id)?;

        let scope_key = input
            .scope_key
            .map(|value| normalize_scope_key_value(&value))
            .unwrap_or_else(|| existing.scope_key.clone());
        ensure_scope_exists(&transaction, &scope_key)?;

        let targeted_work_point_ids = input
            .targeted_work_point_ids
            .clone()
            .map(normalize_string_list)
            .unwrap_or(existing.targeted_work_point_ids.clone());
        ensure_plan_transfer_compatible(
            &transaction,
            &existing,
            &scope_key,
            &targeted_work_point_ids,
        )?;

        let assumptions = input
            .assumptions
            .map(normalize_string_list)
            .unwrap_or(existing.assumptions.clone());
        let stop_conditions = input
            .stop_conditions
            .map(normalize_string_list)
            .unwrap_or(existing.stop_conditions.clone());
        let validation_steps = input
            .validation_steps
            .map(normalize_string_list)
            .unwrap_or(existing.validation_steps.clone());
        let effort_tier = input.effort_tier.unwrap_or(existing.effort_tier);
        let routing_hint = if input.clear_routing_hint {
            None
        } else {
            input
                .routing_hint
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .or(existing.routing_hint.clone())
        };
        let allow_parallel_overlap = input
            .allow_parallel_overlap
            .unwrap_or(existing.allow_parallel_overlap);
        let file_scopes = if input.clear_file_scopes {
            Vec::new()
        } else {
            input
                .file_scopes
                .map(normalize_file_scopes)
                .unwrap_or(existing.file_scopes.clone())
        };
        let tags = input
            .tags
            .map(normalize_string_list)
            .unwrap_or(existing.tags.clone());
        let now = now_string()?;

        transaction.execute(
            r#"
            UPDATE plans
               SET scope_key = ?2,
                   assumptions_json = ?3,
                   stop_conditions_json = ?4,
                   validation_steps_json = ?5,
                   targeted_work_point_ids_json = ?6,
                   effort_tier = ?7,
                   routing_hint = ?8,
                   allow_parallel_overlap = ?9,
                   tags_json = ?10,
                   revision = revision + 1,
                   updated_at = ?11
             WHERE id = ?1
            "#,
            params![
                input.plan_id,
                scope_key,
                to_json_text(&assumptions)?,
                to_json_text(&stop_conditions)?,
                to_json_text(&validation_steps)?,
                to_json_text(&targeted_work_point_ids)?,
                effort_tier.as_str(),
                routing_hint,
                if allow_parallel_overlap { 1 } else { 0 },
                to_json_text(&tags)?,
                now,
            ],
        )?;

        replace_entity_file_scopes(
            &transaction,
            &scope_key,
            EntityType::Plan,
            &input.plan_id,
            &file_scopes,
            &now,
        )?;

        let record = load_plan(&transaction, &input.plan_id)?;
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::Plan,
                &record.id,
                EntityType::Plan,
                &record.id,
                &record.correlation_id,
                input.run_id,
                "plan.revised",
                serde_json::json!({
                    "assumptions": record.assumptions,
                    "stopConditions": record.stop_conditions,
                    "validationSteps": record.validation_steps,
                    "targetedWorkPointIds": record.targeted_work_point_ids,
                    "effortTier": record.effort_tier,
                    "routingHint": record.routing_hint,
                    "allowParallelOverlap": record.allow_parallel_overlap,
                    "fileScopes": record.file_scopes,
                    "tags": record.tags,
                    "revision": record.revision
                }),
            )?,
        )?;

        let validation = refresh_validation_target(&transaction, EntityType::Plan, &record.id)?;
        rebuild_tag_index_for_entity(&transaction, EntityType::Plan, &record.id, &record.tags)?;
        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn goal(&self, id: &str) -> Result<GoalView, PlanningStoreError> {
        let connection = self.open_connection()?;
        let goal = load_goal(&connection, id)?;
        let roadmaps = list_roadmaps_for_goal_in_scope(&connection, id, &goal.scope_key)?;
        let validation = load_validation_report(&connection, EntityType::Goal, id)?;
        Ok(GoalView {
            goal,
            roadmaps,
            validation,
        })
    }

    pub fn roadmap(&self, id: &str) -> Result<RoadmapView, PlanningStoreError> {
        let connection = self.open_connection()?;
        let roadmap = load_roadmap(&connection, id)?;
        let sections = list_sections_for_roadmap_in_scope(&connection, id, &roadmap.scope_key)?;
        let work_points =
            list_work_points_for_roadmap_in_scope(&connection, id, &roadmap.scope_key)?;
        let validation = load_validation_report(&connection, EntityType::Roadmap, id)?;
        Ok(RoadmapView {
            roadmap,
            sections,
            work_points,
            validation,
        })
    }

    pub fn plan(&self, id: &str) -> Result<PlanView, PlanningStoreError> {
        let connection = self.open_connection()?;
        let plan = load_plan(&connection, id)?;
        let todos = list_todos_for_plan_in_scope(&connection, id, &plan.scope_key)?;
        let review_points = list_review_points_for_entity_in_scope(
            &connection,
            EntityType::Plan,
            id,
            &plan.scope_key,
        )?;
        let validation = load_validation_report(&connection, EntityType::Plan, id)?;
        Ok(PlanView {
            plan,
            todos,
            review_points,
            validation,
        })
    }

    pub fn issue(&self, id: &str) -> Result<IssueView, PlanningStoreError> {
        let connection = self.open_connection()?;
        let issue = load_issue(&connection, id)?;
        let validation = load_validation_report(&connection, EntityType::Issue, id)?;
        Ok(IssueView { issue, validation })
    }

    pub fn work_point(&self, id: &str) -> Result<WorkPointView, PlanningStoreError> {
        let connection = self.open_connection()?;
        let work_point = load_work_point(&connection, id)?;
        let validation = load_validation_report(&connection, EntityType::WorkPoint, id)?;
        Ok(WorkPointView {
            work_point,
            validation,
        })
    }

    pub fn list_goals(&self) -> Result<Vec<GoalRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope_key, correlation_id, title, description, acceptance_criteria_json, rejection_criteria_json, status, tags_json, revision, created_at, updated_at FROM goals ORDER BY updated_at DESC, id ASC",
        )?;
        let rows = statement.query_map([], row_to_goal)?;
        collect_rows(rows)
    }

    pub fn list_goals_in_scope(
        &self,
        scope_key: &str,
    ) -> Result<Vec<GoalRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope_key, correlation_id, title, description, acceptance_criteria_json, rejection_criteria_json, status, tags_json, revision, created_at, updated_at FROM goals WHERE scope_key = ?1 ORDER BY updated_at DESC, id ASC",
        )?;
        let rows =
            statement.query_map(params![normalize_scope_key_value(scope_key)], row_to_goal)?;
        collect_rows(rows)
    }

    pub fn list_roadmaps(&self) -> Result<Vec<RoadmapRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope_key, goal_id, correlation_id, title, summary, status, tags_json, revision, created_at, updated_at FROM roadmaps ORDER BY updated_at DESC, id ASC",
        )?;
        let rows = statement.query_map([], row_to_roadmap)?;
        collect_rows(rows)
    }

    pub fn list_roadmaps_in_scope(
        &self,
        scope_key: &str,
    ) -> Result<Vec<RoadmapRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope_key, goal_id, correlation_id, title, summary, status, tags_json, revision, created_at, updated_at FROM roadmaps WHERE scope_key = ?1 ORDER BY updated_at DESC, id ASC",
        )?;
        let rows = statement.query_map(
            params![normalize_scope_key_value(scope_key)],
            row_to_roadmap,
        )?;
        collect_rows(rows)
    }

    pub fn list_work_points(&self) -> Result<Vec<WorkPointRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope_key, roadmap_id, section_id, title, summary, status, ordering_index, dependency_ids_json, validation_expectations_json, effort_tier, kind, priority, repairs_work_point_ids, supersedes_work_point_ids, blocks_work_point_ids, tags_json, revision, created_at, updated_at FROM work_points ORDER BY updated_at DESC, id ASC",
        )?;
        let rows = statement.query_map([], row_to_work_point)?;
        let mut items = collect_rows(rows)?;
        let ids = items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
        let mut file_scopes_by_id =
            load_file_scopes_for_entities(&connection, EntityType::WorkPoint, &ids)?;
        for item in &mut items {
            item.file_scopes = file_scopes_by_id.remove(&item.id).unwrap_or_default();
        }
        Ok(items)
    }

    pub fn list_work_points_in_scope(
        &self,
        scope_key: &str,
    ) -> Result<Vec<WorkPointRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope_key, roadmap_id, section_id, title, summary, status, ordering_index, dependency_ids_json, validation_expectations_json, effort_tier, kind, priority, repairs_work_point_ids, supersedes_work_point_ids, blocks_work_point_ids, tags_json, revision, created_at, updated_at FROM work_points WHERE scope_key = ?1 ORDER BY updated_at DESC, id ASC",
        )?;
        let rows = statement.query_map(
            params![normalize_scope_key_value(scope_key)],
            row_to_work_point,
        )?;
        let mut items = collect_rows(rows)?;
        let ids = items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
        let mut file_scopes_by_id =
            load_file_scopes_for_entities(&connection, EntityType::WorkPoint, &ids)?;
        for item in &mut items {
            item.file_scopes = file_scopes_by_id.remove(&item.id).unwrap_or_default();
        }
        Ok(items)
    }

    pub fn list_plans(&self) -> Result<Vec<PlanRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope_key, goal_id, roadmap_id, correlation_id, title, summary, scope, assumptions_json, stop_conditions_json, validation_steps_json, targeted_work_point_ids_json, effort_tier, routing_hint, allow_parallel_overlap, status, tags_json, revision, created_at, updated_at FROM plans ORDER BY updated_at DESC, id ASC",
        )?;
        let rows = statement.query_map([], row_to_plan)?;
        let mut items = collect_rows(rows)?;
        let ids = items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
        let mut file_scopes_by_id =
            load_file_scopes_for_entities(&connection, EntityType::Plan, &ids)?;
        for item in &mut items {
            item.file_scopes = file_scopes_by_id.remove(&item.id).unwrap_or_default();
        }
        Ok(items)
    }

    pub fn list_plans_in_scope(
        &self,
        scope_key: &str,
    ) -> Result<Vec<PlanRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope_key, goal_id, roadmap_id, correlation_id, title, summary, scope, assumptions_json, stop_conditions_json, validation_steps_json, targeted_work_point_ids_json, effort_tier, routing_hint, allow_parallel_overlap, status, tags_json, revision, created_at, updated_at FROM plans WHERE scope_key = ?1 ORDER BY updated_at DESC, id ASC",
        )?;
        let rows =
            statement.query_map(params![normalize_scope_key_value(scope_key)], row_to_plan)?;
        let mut items = collect_rows(rows)?;
        let ids = items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
        let mut file_scopes_by_id =
            load_file_scopes_for_entities(&connection, EntityType::Plan, &ids)?;
        for item in &mut items {
            item.file_scopes = file_scopes_by_id.remove(&item.id).unwrap_or_default();
        }
        Ok(items)
    }

    pub fn list_todos(&self) -> Result<Vec<TodoRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope_key, plan_id, work_point_id, title, summary, status, priority, effort_tier, evidence_refs_json, tags_json, ordering_index, revision, created_at, updated_at FROM todos ORDER BY status ASC, ordering_index ASC, id ASC",
        )?;
        let rows = statement.query_map([], row_to_todo)?;
        let mut items = collect_rows(rows)?;
        let ids = items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
        let mut file_scopes_by_id =
            load_file_scopes_for_entities(&connection, EntityType::Todo, &ids)?;
        for item in &mut items {
            item.file_scopes = file_scopes_by_id.remove(&item.id).unwrap_or_default();
        }
        Ok(items)
    }

    pub fn list_todos_in_scope(
        &self,
        scope_key: &str,
    ) -> Result<Vec<TodoRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope_key, plan_id, work_point_id, title, summary, status, priority, effort_tier, evidence_refs_json, tags_json, ordering_index, revision, created_at, updated_at FROM todos WHERE scope_key = ?1 ORDER BY status ASC, ordering_index ASC, id ASC",
        )?;
        let rows =
            statement.query_map(params![normalize_scope_key_value(scope_key)], row_to_todo)?;
        let mut items = collect_rows(rows)?;
        let ids = items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
        let mut file_scopes_by_id =
            load_file_scopes_for_entities(&connection, EntityType::Todo, &ids)?;
        for item in &mut items {
            item.file_scopes = file_scopes_by_id.remove(&item.id).unwrap_or_default();
        }
        Ok(items)
    }

    pub fn list_issues(&self) -> Result<Vec<IssueRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope_key, correlation_id, title, summary, status, severity, related_entity_type, related_entity_id, tags_json, revision, created_at, updated_at FROM issues ORDER BY updated_at DESC, id ASC",
        )?;
        let rows = statement.query_map([], row_to_issue)?;
        collect_rows(rows)
    }

    pub fn list_issues_in_scope(
        &self,
        scope_key: &str,
    ) -> Result<Vec<IssueRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope_key, correlation_id, title, summary, status, severity, related_entity_type, related_entity_id, tags_json, revision, created_at, updated_at FROM issues WHERE scope_key = ?1 ORDER BY updated_at DESC, id ASC",
        )?;
        let rows =
            statement.query_map(params![normalize_scope_key_value(scope_key)], row_to_issue)?;
        collect_rows(rows)
    }

    // ── Discovery CRUD ─────────────────────────────────────────────────────────

    pub fn create_discovery(
        &self,
        input: CreateDiscoveryInput,
    ) -> Result<DiscoveryRecord, PlanningStoreError> {
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("claim", &input.claim)?;

        let connection = self.open_connection()?;
        let scope_key = normalized_scope_key(input.scope_key);
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);

        connection.execute(
            "INSERT INTO discovery_nodes (id, scope_key, correlation_id, classification, verification_state, severity, status, claim, impact, next_action, verification_step, recurrence_key, fingerprint, observed_at_json, occurrence_count, source_lineage_json, review_date, tags_json, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                id, scope_key, input.correlation_id,
                input.classification.as_str(), input.verification_state.as_str(),
                input.severity.as_str(), DiscoveryStatus::Candidate.as_str(),
                input.claim, input.impact, input.next_action, input.verification_step,
                input.recurrence_key, input.fingerprint,
                to_json_text(&input.observed_at)?,
                input.occurrence_count.unwrap_or(1),
                to_json_text(&input.source_lineage)?,
                input.review_date,
                to_json_text(&input.tags)?,
                1, &now, &now,
            ],
        )?;

        self.discovery_by_id(&id)
    }

    pub fn discovery_by_id(
        &self,
        discovery_id: &str,
    ) -> Result<DiscoveryRecord, PlanningStoreError> {
        let connection = self.open_connection()?;
        load_discovery(&connection, discovery_id)
    }

    pub fn list_discoveries(
        &self,
        scope_key: &str,
        status_filter: Option<DiscoveryStatus>,
        classification_filter: Option<DiscoveryClassification>,
    ) -> Result<Vec<DiscoveryRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        let mut sql = String::from(
            "SELECT id, scope_key, correlation_id, classification, verification_state, severity, status, claim, impact, next_action, verification_step, recurrence_key, fingerprint, observed_at_json, occurrence_count, source_lineage_json, review_date, resolved_at, resolution_rationale, promoted_entity_type, promoted_entity_id, tags_json, revision, created_at, updated_at FROM discovery_nodes WHERE scope_key = ?1"
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(normalized)];
        let mut param_index = 2;

        if let Some(ref status) = status_filter {
            sql.push_str(&format!(" AND status = ?{param_index}"));
            param_values.push(Box::new(status.as_str().to_string()));
            param_index += 1;
        }
        if let Some(ref classification) = classification_filter {
            sql.push_str(&format!(" AND classification = ?{param_index}"));
            param_values.push(Box::new(classification.as_str().to_string()));
            param_index += 1;
        }

        let _ = param_index;
        sql.push_str(" ORDER BY created_at DESC, id ASC");

        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(param_values), row_to_discovery)?;
        collect_rows(rows)
    }

    pub fn update_discovery_status(
        &self,
        discovery_id: &str,
        status: DiscoveryStatus,
        scope_key: &str,
    ) -> Result<DiscoveryRecord, PlanningStoreError> {
        let connection = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        let now = now_string()?;

        let affected = connection.execute(
            "UPDATE discovery_nodes SET status = ?1, revision = revision + 1, updated_at = ?2 WHERE id = ?3 AND scope_key = ?4",
            params![status.as_str(), now, discovery_id, normalized],
        )?;
        if affected == 0 {
            return Err(PlanningStoreError::InvalidInput(format!(
                "discovery node `{discovery_id}` not found in scope `{normalized}`"
            )));
        }
        self.discovery_by_id(discovery_id)
    }

    pub fn resolve_discovery(
        &self,
        discovery_id: &str,
        rationale: &str,
        evidence_refs: Vec<String>,
        scope_key: &str,
    ) -> Result<DiscoveryRecord, PlanningStoreError> {
        let connection = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        let now = now_string()?;

        let affected = connection.execute(
            "UPDATE discovery_nodes SET status = ?1, resolution_rationale = ?2, resolved_at = ?3, tags_json = ?4, revision = revision + 1, updated_at = ?5 WHERE id = ?6 AND scope_key = ?7",
            params![
                DiscoveryStatus::Resolved.as_str(),
                rationale,
                &now,
                to_json_text(&evidence_refs)?,
                &now,
                discovery_id,
                normalized,
            ],
        )?;
        if affected == 0 {
            return Err(PlanningStoreError::InvalidInput(format!(
                "discovery node `{discovery_id}` not found in scope `{normalized}`"
            )));
        }
        self.discovery_by_id(discovery_id)
    }

    pub fn reopen_discovery(
        &self,
        discovery_id: &str,
        scope_key: &str,
    ) -> Result<DiscoveryRecord, PlanningStoreError> {
        let connection = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        let now = now_string()?;

        let affected = connection.execute(
            "UPDATE discovery_nodes SET status = ?1, resolved_at = NULL, resolution_rationale = NULL, revision = revision + 1, updated_at = ?2 WHERE id = ?3 AND scope_key = ?4",
            params![DiscoveryStatus::Reopened.as_str(), now, discovery_id, normalized],
        )?;
        if affected == 0 {
            return Err(PlanningStoreError::InvalidInput(format!(
                "discovery node `{discovery_id}` not found in scope `{normalized}`"
            )));
        }
        self.discovery_by_id(discovery_id)
    }

    pub fn promote_discovery(
        &self,
        discovery_id: &str,
        entity_type: EntityType,
        entity_id: &str,
        scope_key: &str,
    ) -> Result<DiscoveryRecord, PlanningStoreError> {
        let connection = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        let now = now_string()?;

        let affected = connection.execute(
            "UPDATE discovery_nodes SET status = ?1, promoted_entity_type = ?2, promoted_entity_id = ?3, revision = revision + 1, updated_at = ?4 WHERE id = ?5 AND scope_key = ?6",
            params![
                DiscoveryStatus::Promoted.as_str(),
                entity_type.as_str(),
                entity_id,
                now,
                discovery_id,
                normalized,
            ],
        )?;
        if affected == 0 {
            return Err(PlanningStoreError::InvalidInput(format!(
                "discovery node `{discovery_id}` not found in scope `{normalized}`"
            )));
        }
        self.discovery_by_id(discovery_id)
    }

    pub fn discovery_view(&self, discovery_id: &str) -> Result<DiscoveryView, PlanningStoreError> {
        let connection = self.open_connection()?;
        let discovery = load_discovery(&connection, discovery_id)?;
        let relationships = list_discovery_relationships_for_source(&connection, discovery_id)?;
        let validation =
            load_validation_report(&connection, EntityType::DiscoveryNode, discovery_id)?;
        Ok(DiscoveryView {
            discovery,
            relationships,
            validation,
        })
    }

    // ── Discovery Relationships ─────────────────────────────────────────────

    pub fn add_discovery_relationship(
        &self,
        input: CreateDiscoveryRelationshipInput,
    ) -> Result<DiscoveryRelationshipRecord, PlanningStoreError> {
        let connection = self.open_connection()?;
        let scope_key = normalized_scope_key(input.scope_key);
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);

        connection.execute(
            "INSERT INTO discovery_relationships (id, scope_key, source_id, target_id, relationship_kind, metadata_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id, scope_key, input.source_id, input.target_id,
                input.relationship_kind.as_str(),
                input.metadata.map(|m| m.to_string()),
                now,
            ],
        )?;

        connection.query_row(
            "SELECT id, scope_key, source_id, target_id, relationship_kind, metadata_json, created_at FROM discovery_relationships WHERE id = ?1",
            params![id],
            row_to_discovery_relationship,
        )
        .map_err(|error| map_not_found(error, EntityType::DiscoveryRelationship, &id))
    }

    pub fn list_discovery_relationships(
        &self,
        discovery_id: &str,
    ) -> Result<Vec<DiscoveryRelationshipRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        list_discovery_relationships_for_source(&connection, discovery_id)
    }

    // ── Discovery Checkpoints ───────────────────────────────────────────────

    pub fn create_discovery_checkpoint(
        &self,
        input: CreateDiscoveryCheckpointInput,
    ) -> Result<DiscoveryCheckpointRecord, PlanningStoreError> {
        require_non_empty("runId", &input.run_id)?;
        require_non_empty("event", &input.event)?;

        let connection = self.open_connection()?;
        let scope_key = normalized_scope_key(input.scope_key);
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);

        connection.execute(
            "INSERT INTO discovery_checkpoints (id, scope_key, run_id, event, snapshot_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id, scope_key, input.run_id, input.event,
                input.snapshot.map(|s| s.to_string()),
                now,
            ],
        )?;

        connection.query_row(
            "SELECT id, scope_key, run_id, event, snapshot_json, created_at FROM discovery_checkpoints WHERE id = ?1",
            params![id],
            row_to_discovery_checkpoint,
        )
        .map_err(|error| map_not_found(error, EntityType::DiscoveryCheckpoint, &id))
    }

    pub fn list_discovery_checkpoints(
        &self,
        scope_key: &str,
    ) -> Result<Vec<DiscoveryCheckpointRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        let mut statement = connection.prepare(
            "SELECT id, scope_key, run_id, event, snapshot_json, created_at FROM discovery_checkpoints WHERE scope_key = ?1 ORDER BY created_at DESC, id ASC",
        )?;
        let rows = statement.query_map(params![normalized], row_to_discovery_checkpoint)?;
        collect_rows(rows)
    }

    pub fn claim_project_run(
        &self,
        input: ClaimProjectRunInput,
    ) -> Result<MutationResult<ProjectRunRecord>, PlanningStoreError> {
        require_non_empty("goalId", &input.goal_id)?;
        require_non_empty("roadmapId", &input.roadmap_id)?;
        require_non_empty("workPointId", &input.work_point_id)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let scope_key = normalized_scope_key(input.scope_key);
        let inherited_scope_key = ensure_referenced_entity_in_scope(
            &transaction,
            EntityType::Goal,
            &input.goal_id,
            "goalId",
            &scope_key,
        )?;
        let _ = ensure_referenced_entity_in_scope(
            &transaction,
            EntityType::Roadmap,
            &input.roadmap_id,
            "roadmapId",
            &scope_key,
        )?;
        let _ = ensure_referenced_entity_in_scope(
            &transaction,
            EntityType::WorkPoint,
            &input.work_point_id,
            "workPointId",
            &scope_key,
        )?;

        // Graph consistency: roadmap must belong to the goal
        let roadmap = load_roadmap(&transaction, &input.roadmap_id)?;
        if roadmap.goal_id != input.goal_id {
            return Err(PlanningStoreError::InvalidInput(format!(
                "{}",
                serde_json::json!({
                    "code": "PROJECT-RUN-GOAL-ROADMAP-MISMATCH",
                    "message": format!("roadmap '{}' belongs to goal '{}', not goal '{}'", input.roadmap_id, roadmap.goal_id, input.goal_id),
                    "roadmapId": input.roadmap_id,
                    "expectedGoalId": input.goal_id,
                    "actualGoalId": roadmap.goal_id,
                })
            )));
        }

        // Graph consistency: work point must belong to the roadmap
        let work_point = load_work_point(&transaction, &input.work_point_id)?;
        if work_point.roadmap_id != input.roadmap_id {
            return Err(PlanningStoreError::InvalidInput(format!(
                "{}",
                serde_json::json!({
                    "code": "PROJECT-RUN-WORK-POINT-ROADMAP-MISMATCH",
                    "message": format!("work point '{}' belongs to roadmap '{}', not roadmap '{}'", input.work_point_id, work_point.roadmap_id, input.roadmap_id),
                    "workPointId": input.work_point_id,
                    "expectedRoadmapId": input.roadmap_id,
                    "actualRoadmapId": work_point.roadmap_id,
                })
            )));
        }

        let now = now_string()?;
        let lease_seconds = normalize_lease_seconds(input.lease_seconds)?;
        let owner_id = input
            .owner_id
            .clone()
            .or_else(|| input.session_id.clone())
            .or_else(|| input.run_id.clone())
            .ok_or_else(|| {
                PlanningStoreError::InvalidInput(
                    serde_json::json!({
                        "code": "PROJECT-RUN-OWNER-REQUIRED",
                        "message": "provide ownerId, sessionId, or a run correlation id",
                    })
                    .to_string(),
                )
            })?;
        if let Some(ref key) = input.idempotency_key {
            require_non_empty("idempotencyKey", key)?;
            let existing_id: Option<String> = transaction
                .query_row(
                    "SELECT id FROM project_runs WHERE scope_key = ?1 AND idempotency_key = ?2",
                    params![inherited_scope_key, key],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(existing_id) = existing_id {
                let existing = load_project_run(&transaction, &existing_id)?;
                if existing.goal_id == input.goal_id
                    && existing.roadmap_id == input.roadmap_id
                    && existing.work_point_id == input.work_point_id
                    && existing.owner_id == owner_id
                    && existing.repo_id == input.repo_id
                    && existing.branch == input.branch
                    && existing.worktree_id == input.worktree_id
                    && existing.session_id == input.session_id
                    && existing.profile_id == input.profile_id
                {
                    let validation =
                        validate_and_store(&transaction, EntityType::ProjectRun, &existing.id)?;
                    transaction.commit()?;
                    return Ok(MutationResult {
                        record: existing,
                        validation,
                    });
                }
                return Err(PlanningStoreError::InvalidInput(
                    serde_json::json!({
                        "code": "PROJECT-RUN-IDEMPOTENCY-CONFLICT",
                        "message": "idempotency key was already used with a different claim payload",
                        "idempotencyKey": key,
                        "existingProjectRunId": existing.id,
                    })
                    .to_string(),
                ));
            }
        }

        let expired_ids = {
            let mut statement = transaction.prepare(
                "SELECT id FROM project_runs WHERE work_point_id = ?1 AND status IN ('claimed', 'active', 'interrupted') AND julianday(lease_expires_at) <= julianday(?2) ORDER BY fencing_token ASC",
            )?;
            let rows = statement.query_map(params![input.work_point_id, now], |row| {
                row.get::<_, String>(0)
            })?;
            collect_rows(rows)?
        };
        transaction.execute(
            "UPDATE project_runs SET status = 'released', revision = revision + 1, updated_at = ?1 WHERE work_point_id = ?2 AND status IN ('claimed', 'active', 'interrupted') AND julianday(lease_expires_at) <= julianday(?1)",
            params![now, input.work_point_id],
        )?;
        for expired_id in expired_ids {
            let expired = load_project_run(&transaction, &expired_id)?;
            let correlation_id = input
                .correlation_id
                .as_deref()
                .unwrap_or("lease-expiration");
            append_event(
                &transaction,
                build_event(
                    &transaction,
                    EntityType::ProjectRun,
                    &expired.id,
                    EntityType::Roadmap,
                    &expired.roadmap_id,
                    correlation_id,
                    input.run_id.clone(),
                    "project-run.expired",
                    serde_json::json!({
                        "fencingToken": expired.fencing_token,
                        "leaseExpiresAt": expired.lease_expires_at,
                        "status": expired.status,
                    }),
                )?,
            )?;
        }
        let active_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM project_runs WHERE work_point_id = ?1 AND status IN ('claimed', 'active', 'interrupted') AND julianday(lease_expires_at) > julianday(?2)",
            params![input.work_point_id, now],
            |row| row.get(0),
        )?;
        if active_count > 0 {
            return Err(PlanningStoreError::ActiveLeaseConflict {
                work_point_id: input.work_point_id.clone(),
            });
        }

        let id = input.id.unwrap_or_else(new_id);
        let fencing_token: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(fencing_token), 0) + 1 FROM project_runs WHERE work_point_id = ?1",
            params![input.work_point_id],
            |row| row.get(0),
        )?;
        let lease_expires_at = lease_deadline(&now, lease_seconds)?;
        let evidence = ProjectRunEvidence::default();
        let record = ProjectRunRecord {
            id: id.clone(),
            scope_key: inherited_scope_key,
            goal_id: input.goal_id,
            roadmap_id: input.roadmap_id,
            work_point_id: input.work_point_id,
            repo_id: input.repo_id,
            branch: input.branch,
            worktree_id: input.worktree_id,
            session_id: input.session_id,
            run_id: input.run_id.clone(),
            profile_id: input.profile_id,
            owner_id,
            idempotency_key: input.idempotency_key,
            fencing_token,
            lease_expires_at,
            heartbeat_at: now.clone(),
            status: ProjectRunStatus::Claimed,
            evidence,
            revision: 1,
            claimed_at: Some(now.clone()),
            completed_at: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        transaction.execute(
            r#"
            INSERT INTO project_runs (
                id, scope_key, goal_id, roadmap_id, work_point_id, repo_id, branch,
                worktree_id, session_id, run_id, profile_id, owner_id, idempotency_key,
                fencing_token, lease_expires_at, heartbeat_at, status, evidence_json,
                revision, claimed_at, completed_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)
            "#,
            params![
                record.id,
                record.scope_key,
                record.goal_id,
                record.roadmap_id,
                record.work_point_id,
                record.repo_id,
                record.branch,
                record.worktree_id,
                record.session_id,
                record.run_id,
                record.profile_id,
                record.owner_id,
                record.idempotency_key,
                record.fencing_token,
                record.lease_expires_at,
                record.heartbeat_at,
                record.status.as_str(),
                to_json_text(&record.evidence)?,
                record.revision,
                record.claimed_at,
                record.completed_at,
                record.created_at,
                record.updated_at,
            ],
        )?;

        let correlation_id = input.correlation_id.unwrap_or_else(|| format!("corr-{id}"));
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::ProjectRun,
                &id,
                EntityType::Roadmap,
                &record.roadmap_id,
                &correlation_id,
                input.run_id,
                "project-run.claimed",
                serde_json::to_value(&record)?,
            )?,
        )?;

        let validation = validate_and_store(&transaction, EntityType::ProjectRun, &id)?;
        let _ =
            refresh_validation_target(&transaction, EntityType::WorkPoint, &record.work_point_id)?;
        transaction.commit()?;
        // Update session with active project run state
        if record.session_id.is_some() {
            let session_state = crate::session::ActiveProjectRunState {
                project_run_id: record.id.clone(),
                goal_id: record.goal_id.clone(),
                roadmap_id: record.roadmap_id.clone(),
                work_point_id: record.work_point_id.clone(),
                status: record.status.as_str().to_string(),
                claimed_at: record.claimed_at.clone().unwrap_or_default(),
                activated_at: None,
                evidence_refs: {
                    let mut refs = Vec::new();
                    refs.extend(record.evidence.implementation_run_refs.iter().cloned());
                    refs.extend(record.evidence.validation_finding_refs.iter().cloned());
                    refs.extend(record.evidence.linked_spec_ids.iter().cloned());
                    refs
                },
            };
            let _ = crate::session::set_active_project_run(session_state);
        }
        Ok(MutationResult { record, validation })
    }

    pub fn activate_project_run(
        &self,
        input: ActivateProjectRunInput,
    ) -> Result<MutationResult<ProjectRunRecord>, PlanningStoreError> {
        require_non_empty("projectRunId", &input.project_run_id)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let active_scope_key = normalized_scope_key(input.active_scope_key);
        ensure_entity_in_scope(
            &transaction,
            EntityType::ProjectRun,
            &input.project_run_id,
            &active_scope_key,
        )?;

        let existing = load_project_run(&transaction, &input.project_run_id)?;
        let now = now_string()?;
        require_current_lease(&existing, input.fencing_token, &now)?;
        if existing.status != ProjectRunStatus::Claimed {
            return Err(PlanningStoreError::ProjectRunStatusMismatch {
                expected: "claimed".to_string(),
                actual: existing.status.to_string(),
            });
        }

        transaction.execute(
            r#"
            UPDATE project_runs SET
                status = ?1,
                revision = revision + 1,
                updated_at = ?2
            WHERE id = ?3
            "#,
            params![ProjectRunStatus::Active.as_str(), now, input.project_run_id],
        )?;

        let record = load_project_run(&transaction, &input.project_run_id)?;
        let correlation_id = roadmap_correlation_id(&transaction, &record.roadmap_id)?;
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::ProjectRun,
                &record.id,
                EntityType::Roadmap,
                &record.roadmap_id,
                &correlation_id,
                input.run_id,
                "project-run.activated",
                serde_json::to_value(&record)?,
            )?,
        )?;

        let validation = validate_and_store(&transaction, EntityType::ProjectRun, &record.id)?;
        let _ =
            refresh_validation_target(&transaction, EntityType::WorkPoint, &record.work_point_id)?;
        transaction.commit()?;
        // Update session with activated state
        if record.session_id.is_some() {
            let now_str = now_string().unwrap_or_default();
            let session_state = crate::session::ActiveProjectRunState {
                project_run_id: record.id.clone(),
                goal_id: record.goal_id.clone(),
                roadmap_id: record.roadmap_id.clone(),
                work_point_id: record.work_point_id.clone(),
                status: "active".to_string(),
                claimed_at: record.claimed_at.clone().unwrap_or_default(),
                activated_at: Some(now_str),
                evidence_refs: {
                    let mut evidence_refs = Vec::new();
                    evidence_refs.extend(record.evidence.implementation_run_refs.iter().cloned());
                    evidence_refs.extend(record.evidence.validation_finding_refs.iter().cloned());
                    evidence_refs.extend(record.evidence.linked_spec_ids.iter().cloned());
                    evidence_refs
                },
            };
            let _ = crate::session::set_active_project_run(session_state);
        }
        Ok(MutationResult { record, validation })
    }

    pub fn heartbeat_project_run(
        &self,
        input: HeartbeatProjectRunInput,
    ) -> Result<MutationResult<ProjectRunRecord>, PlanningStoreError> {
        require_non_empty("projectRunId", &input.project_run_id)?;
        let mut connection = self.open_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let active_scope_key = normalized_scope_key(input.active_scope_key);
        ensure_entity_in_scope(
            &transaction,
            EntityType::ProjectRun,
            &input.project_run_id,
            &active_scope_key,
        )?;
        let existing = load_project_run(&transaction, &input.project_run_id)?;
        let now = now_string()?;
        require_current_lease(&existing, input.fencing_token, &now)?;
        if existing.status != ProjectRunStatus::Claimed
            && existing.status != ProjectRunStatus::Active
        {
            return Err(PlanningStoreError::ProjectRunStatusMismatch {
                expected: "claimed or active".to_string(),
                actual: existing.status.to_string(),
            });
        }
        let lease_seconds = normalize_lease_seconds(input.lease_seconds)?;
        let lease_expires_at = lease_deadline(&now, lease_seconds)?;
        transaction.execute(
            "UPDATE project_runs SET heartbeat_at = ?1, lease_expires_at = ?2, revision = revision + 1, updated_at = ?1 WHERE id = ?3 AND fencing_token = ?4",
            params![now, lease_expires_at, input.project_run_id, existing.fencing_token],
        )?;
        let record = load_project_run(&transaction, &input.project_run_id)?;
        let correlation_id = roadmap_correlation_id(&transaction, &record.roadmap_id)?;
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::ProjectRun,
                &record.id,
                EntityType::Roadmap,
                &record.roadmap_id,
                &correlation_id,
                input.run_id,
                "project-run.heartbeat",
                serde_json::json!({
                    "fencingToken": record.fencing_token,
                    "heartbeatAt": record.heartbeat_at,
                    "leaseExpiresAt": record.lease_expires_at,
                }),
            )?,
        )?;
        let validation = validate_and_store(&transaction, EntityType::ProjectRun, &record.id)?;
        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn release_project_run(
        &self,
        input: ReleaseProjectRunInput,
    ) -> Result<MutationResult<ProjectRunRecord>, PlanningStoreError> {
        require_non_empty("projectRunId", &input.project_run_id)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let active_scope_key = normalized_scope_key(input.active_scope_key);
        ensure_entity_in_scope(
            &transaction,
            EntityType::ProjectRun,
            &input.project_run_id,
            &active_scope_key,
        )?;

        let existing = load_project_run(&transaction, &input.project_run_id)?;
        let now = now_string()?;
        require_current_lease(&existing, input.fencing_token, &now)?;
        if existing.status != ProjectRunStatus::Claimed
            && existing.status != ProjectRunStatus::Active
            && existing.status != ProjectRunStatus::Interrupted
        {
            return Err(PlanningStoreError::ProjectRunStatusMismatch {
                expected: "claimed, active, or interrupted".to_string(),
                actual: existing.status.to_string(),
            });
        }

        let evidence = input.evidence.unwrap_or(existing.evidence);
        let completed_at = if input.status == ProjectRunStatus::Completed {
            Some(now.clone())
        } else {
            None
        };

        transaction.execute(
            r#"
            UPDATE project_runs SET
                status = ?1,
                evidence_json = ?2,
                completed_at = ?3,
                revision = revision + 1,
                updated_at = ?4
            WHERE id = ?5
            "#,
            params![
                input.status.as_str(),
                to_json_text(&evidence)?,
                completed_at,
                now,
                input.project_run_id,
            ],
        )?;

        let record = load_project_run(&transaction, &input.project_run_id)?;
        let correlation_id = roadmap_correlation_id(&transaction, &record.roadmap_id)?;
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::ProjectRun,
                &record.id,
                EntityType::Roadmap,
                &record.roadmap_id,
                &correlation_id,
                input.run_id,
                "project-run.released",
                serde_json::to_value(&record)?,
            )?,
        )?;

        let validation = validate_and_store(&transaction, EntityType::ProjectRun, &record.id)?;
        let _ =
            refresh_validation_target(&transaction, EntityType::WorkPoint, &record.work_point_id)?;
        transaction.commit()?;
        // Clear active project run from session
        let completed_wp_id = if input.status == ProjectRunStatus::Completed {
            Some(record.work_point_id.clone())
        } else {
            None
        };
        let _ = crate::session::clear_active_project_run(completed_wp_id);
        Ok(MutationResult { record, validation })
    }

    pub fn add_project_run_evidence(
        &self,
        input: AddEvidenceInput,
    ) -> Result<MutationResult<ProjectRunRecord>, PlanningStoreError> {
        require_non_empty("projectRunId", &input.project_run_id)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let active_scope_key = normalized_scope_key(input.active_scope_key);
        ensure_entity_in_scope(
            &transaction,
            EntityType::ProjectRun,
            &input.project_run_id,
            &active_scope_key,
        )?;

        let existing = load_project_run(&transaction, &input.project_run_id)?;
        let now = now_string()?;
        require_current_lease(&existing, input.fencing_token, &now)?;
        if existing.status == ProjectRunStatus::Completed
            || existing.status == ProjectRunStatus::Released
        {
            return Err(PlanningStoreError::ProjectRunStatusMismatch {
                expected: "claimed or active".to_string(),
                actual: existing.status.to_string(),
            });
        }
        transaction.execute(
            r#"
            UPDATE project_runs SET
                evidence_json = ?1,
                revision = revision + 1,
                updated_at = ?2
            WHERE id = ?3
            "#,
            params![to_json_text(&input.evidence)?, now, input.project_run_id],
        )?;

        let record = load_project_run(&transaction, &input.project_run_id)?;
        let correlation_id = roadmap_correlation_id(&transaction, &record.roadmap_id)?;
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::ProjectRun,
                &record.id,
                EntityType::Roadmap,
                &record.roadmap_id,
                &correlation_id,
                input.run_id,
                "project-run.evidence-added",
                serde_json::json!({ "evidence": input.evidence }),
            )?,
        )?;

        let validation = validate_and_store(&transaction, EntityType::ProjectRun, &record.id)?;
        transaction.commit()?;
        // Update session evidence refs
        let _ = crate::session::set_active_project_run(crate::session::ActiveProjectRunState {
            project_run_id: record.id.clone(),
            goal_id: record.goal_id.clone(),
            roadmap_id: record.roadmap_id.clone(),
            work_point_id: record.work_point_id.clone(),
            status: record.status.as_str().to_string(),
            claimed_at: record.claimed_at.clone().unwrap_or_default(),
            activated_at: record.completed_at.clone(),
            evidence_refs: {
                let mut evidence_refs = Vec::new();
                evidence_refs.extend(record.evidence.implementation_run_refs.iter().cloned());
                evidence_refs.extend(record.evidence.validation_finding_refs.iter().cloned());
                evidence_refs.extend(record.evidence.linked_spec_ids.iter().cloned());
                evidence_refs
            },
        });
        Ok(MutationResult { record, validation })
    }

    pub fn count_active_runs_for_session(
        &self,
        session_id: &str,
    ) -> Result<i64, PlanningStoreError> {
        let conn = self.open_connection()?;
        let now = now_string()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM project_runs WHERE session_id = ?1 AND status IN ('claimed','active','interrupted') AND julianday(lease_expires_at) > julianday(?2)",
            params![session_id, now],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Attach/register a worktree.
    /// Internal helper: query worktree by ID only (no scope filter).
    fn get_worktree_by_id_only(
        conn: &Connection,
        id: &str,
    ) -> Result<WorktreeRecord, PlanningStoreError> {
        conn.query_row(
            "SELECT id, scope_key, repo_uri, branch, worktree_path, project_run_id, session_id, status, revision, created_at, updated_at FROM worktrees WHERE id = ?1",
            params![id],
            |row| {
                Ok(WorktreeRecord {
                    id: row.get(0)?,
                    scope_key: row.get(1)?,
                    repo_uri: row.get(2)?,
                    branch: row.get(3)?,
                    worktree_path: row.get(4)?,
                    project_run_id: row.get(5)?,
                    session_id: row.get(6)?,
                    status: crate::parse_worktree_status_strict(&row.get::<_, String>(7)?).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(std::io::Error::other(e)))
                    })?,
                    revision: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        )
        .map_err(|e| {
            PlanningStoreError::InvalidInput(format!("worktree not found: {id}: {e}"))
        })
    }

    pub fn attach_worktree(
        &self,
        input: AttachWorktreeInput,
    ) -> Result<WorktreeRecord, PlanningStoreError> {
        let id = input.id.clone().unwrap_or_else(new_id);
        let scope_key = normalized_scope_key(input.scope_key.clone());
        let now = now_string()?;

        // Scope match check: if re-attaching an existing worktree ID, verify scope
        if let Some(ref existing_id) = input.id {
            let conn = self.open_connection()?;
            if let Ok(existing) = Self::get_worktree_by_id_only(&conn, existing_id) {
                if existing.scope_key != scope_key {
                    return Err(PlanningStoreError::InvalidInput(format!(
                        "{}",
                        serde_json::json!({
                            "code": "CROSS_SCOPE_MUTATION",
                            "message": format!("worktree '{}' already exists in scope '{}', cannot re-attach from scope '{}'", existing_id, existing.scope_key, scope_key),
                            "worktreeId": existing_id,
                            "existingScope": existing.scope_key,
                            "requestedScope": scope_key,
                        })
                    )));
                }
            }
        }

        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO worktrees (id, scope_key, repo_uri, branch, worktree_path, project_run_id, session_id, status, revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', 1, ?8, ?8)
             ON CONFLICT(id) DO UPDATE SET
                repo_uri = excluded.repo_uri,
                branch = excluded.branch,
                worktree_path = excluded.worktree_path,
                project_run_id = excluded.project_run_id,
                session_id = excluded.session_id,
                updated_at = excluded.updated_at,
                revision = revision + 1;",
            params![
                id,
                scope_key,
                input.repo_uri,
                input.branch,
                input.worktree_path,
                input.project_run_id,
                input.session_id,
                now,
            ],
        )?;

        self.get_worktree(&id, &scope_key)
    }

    /// Get a worktree by ID, scoped to the given scope.
    pub fn get_worktree(
        &self,
        id: &str,
        scope_key: &str,
    ) -> Result<WorktreeRecord, PlanningStoreError> {
        let conn = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        conn.query_row(
            "SELECT id, scope_key, repo_uri, branch, worktree_path, project_run_id, session_id, status, revision, created_at, updated_at FROM worktrees WHERE id = ?1 AND scope_key = ?2",
            params![id, normalized],
            |row| {
                Ok(WorktreeRecord {
                    id: row.get(0)?,
                    scope_key: row.get(1)?,
                    repo_uri: row.get(2)?,
                    branch: row.get(3)?,
                    worktree_path: row.get(4)?,
                    project_run_id: row.get(5)?,
                    session_id: row.get(6)?,
                    status: crate::parse_worktree_status_strict(&row.get::<_, String>(7)?).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(std::io::Error::other(e)))
                    })?,
                    revision: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        )
        .map_err(|e| {
            PlanningStoreError::InvalidInput(format!("worktree not found: {id}: {e}"))
        })
    }

    /// List worktrees in the given scope, with optional status filter.
    pub fn list_worktrees(
        &self,
        scope_key: &str,
        status_filter: Option<&str>,
    ) -> Result<Vec<WorktreeRecord>, PlanningStoreError> {
        let conn = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        let status_val = status_filter.unwrap_or("active");

        let sql = if status_filter.is_some() {
            "SELECT id, scope_key, repo_uri, branch, worktree_path, project_run_id, session_id, status, revision, created_at, updated_at FROM worktrees WHERE scope_key = ?1 AND status = ?2 ORDER BY created_at DESC".to_string()
        } else {
            "SELECT id, scope_key, repo_uri, branch, worktree_path, project_run_id, session_id, status, revision, created_at, updated_at FROM worktrees WHERE scope_key = ?1 ORDER BY created_at DESC".to_string()
        };

        let mut stmt = conn.prepare(&sql)?;
        let rows = if status_filter.is_some() {
            collect_rows(stmt.query_map(params![normalized, status_val], |row| {
                Ok(WorktreeRecord {
                    id: row.get(0)?,
                    scope_key: row.get(1)?,
                    repo_uri: row.get(2)?,
                    branch: row.get(3)?,
                    worktree_path: row.get(4)?,
                    project_run_id: row.get(5)?,
                    session_id: row.get(6)?,
                    status: crate::parse_worktree_status_strict(&row.get::<_, String>(7)?)
                        .map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                7,
                                rusqlite::types::Type::Text,
                                Box::new(std::io::Error::other(e)),
                            )
                        })?,
                    revision: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?)?
        } else {
            collect_rows(stmt.query_map(params![normalized], |row| {
                Ok(WorktreeRecord {
                    id: row.get(0)?,
                    scope_key: row.get(1)?,
                    repo_uri: row.get(2)?,
                    branch: row.get(3)?,
                    worktree_path: row.get(4)?,
                    project_run_id: row.get(5)?,
                    session_id: row.get(6)?,
                    status: crate::parse_worktree_status_strict(&row.get::<_, String>(7)?)
                        .map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                7,
                                rusqlite::types::Type::Text,
                                Box::new(std::io::Error::other(e)),
                            )
                        })?,
                    revision: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?)?
        };

        Ok(rows)
    }

    /// Update worktree status, scoped to the given scope.
    pub fn update_worktree_status(
        &self,
        id: &str,
        scope_key: &str,
        status: WorktreeStatus,
    ) -> Result<WorktreeRecord, PlanningStoreError> {
        let conn = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        let now = now_string()?;
        let rows_affected = conn.execute(
            "UPDATE worktrees SET status = ?1, updated_at = ?2, revision = revision + 1 WHERE id = ?3 AND scope_key = ?4",
            params![status.to_string(), now, id, normalized],
        )?;
        if rows_affected == 0 {
            return Err(PlanningStoreError::InvalidInput(format!(
                "{}",
                serde_json::json!({
                    "code": "SCOPE_MISMATCH_OR_NOT_FOUND",
                    "message": format!("worktree '{}' not found in scope '{}'", id, scope_key),
                    "worktreeId": id,
                    "scopeKey": scope_key,
                })
            )));
        }
        self.get_worktree(id, scope_key)
    }

    /// List recent sessions from the events table.
    pub fn list_sessions(&self, limit: i64) -> Result<Vec<SessionSummary>, PlanningStoreError> {
        let conn = self.open_connection()?;
        let now = now_string()?;
        let mut stmt = conn.prepare(
            "SELECT
                COALESCE(e.session_id, 'unknown') as sid,
                COUNT(*) as event_count,
                MAX(e.created_at) as last_seen,
                (SELECT COUNT(*) FROM project_runs pr WHERE pr.session_id = e.session_id AND pr.status IN ('claimed','active','interrupted') AND julianday(pr.lease_expires_at) > julianday(?2)) as active_runs
             FROM planning_events e
             WHERE e.session_id IS NOT NULL AND e.session_id != ''
             GROUP BY e.session_id
             ORDER BY last_seen DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit, now], |row| {
            Ok(SessionSummary {
                session_id: row.get(0)?,
                scope: String::new(),
                created_at: None,
                last_seen: row.get::<_, Option<String>>(2)?,
                event_count: row.get(1)?,
                active_project_runs: row.get(3)?,
            })
        })?;

        collect_rows(rows)
    }

    pub fn list_project_runs(&self) -> Result<Vec<ProjectRunRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope_key, goal_id, roadmap_id, work_point_id, repo_id, branch, worktree_id, session_id, run_id, profile_id, owner_id, idempotency_key, fencing_token, lease_expires_at, heartbeat_at, status, evidence_json, revision, claimed_at, completed_at, created_at, updated_at FROM project_runs ORDER BY updated_at DESC, id ASC",
        )?;
        let rows = statement.query_map([], row_to_project_run)?;
        collect_rows(rows)
    }

    pub fn list_project_runs_in_scope(
        &self,
        scope_key: &str,
    ) -> Result<Vec<ProjectRunRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope_key, goal_id, roadmap_id, work_point_id, repo_id, branch, worktree_id, session_id, run_id, profile_id, owner_id, idempotency_key, fencing_token, lease_expires_at, heartbeat_at, status, evidence_json, revision, claimed_at, completed_at, created_at, updated_at FROM project_runs WHERE scope_key = ?1 ORDER BY updated_at DESC, id ASC",
        )?;
        let rows = statement.query_map(
            params![normalize_scope_key_value(scope_key)],
            row_to_project_run,
        )?;
        collect_rows(rows)
    }

    pub fn project_run(&self, id: &str) -> Result<ProjectRunView, PlanningStoreError> {
        let connection = self.open_connection()?;
        let record = load_project_run(&connection, id)?;
        let work_point = load_work_point(&connection, &record.work_point_id).ok();
        let validation = load_validation_report(&connection, EntityType::ProjectRun, id)?;
        Ok(ProjectRunView {
            project_run: record,
            work_point,
            validation,
        })
    }

    pub fn find_runnable_work_points(
        &self,
        roadmap_id: &str,
    ) -> Result<RunnableCandidates, PlanningStoreError> {
        require_non_empty("roadmapId", roadmap_id)?;

        let connection = self.open_connection()?;
        let now = now_string()?;
        let roadmap = load_roadmap(&connection, roadmap_id)?;
        let all_work_points = list_work_points_for_roadmap(&connection, roadmap_id)?;
        let scope_key = &roadmap.scope_key;

        // Pre-load: all open high/critical issues for the scope
        let mut open_blocker_issues: HashMap<String, Vec<String>> = HashMap::new();
        {
            let mut stmt = connection.prepare(
                "SELECT related_entity_id, id FROM issues WHERE scope_key = ?1 AND status = 'open' AND severity IN ('high', 'critical') AND related_entity_id IS NOT NULL",
            )?;
            let rows = stmt.query_map(params![scope_key], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (entity_id, issue_id) = row?;
                open_blocker_issues
                    .entry(entity_id)
                    .or_default()
                    .push(issue_id);
            }
        }

        // Pre-load: all open high/critical review points for the scope
        let mut open_blocker_review_points: HashMap<String, Vec<String>> = HashMap::new();
        {
            let mut stmt = connection.prepare(
                "SELECT attached_entity_id, id FROM review_points WHERE scope_key = ?1 AND status = 'open' AND severity IN ('high', 'critical')",
            )?;
            let rows = stmt.query_map(params![scope_key], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (entity_id, rp_id) = row?;
                open_blocker_review_points
                    .entry(entity_id)
                    .or_default()
                    .push(rp_id);
            }
        }

        // Collect active corrective blockers for this scope
        let mut active_blockers: Vec<(String, String, Vec<String>)> = Vec::new(); // (blocker_id, blocker_title, blocked_ids)
        for wp in &all_work_points {
            if wp.kind != WorkPointKind::Feature
                && !wp.blocks_work_point_ids.is_empty()
                && !matches!(
                    wp.status,
                    WorkPointStatus::Completed
                        | WorkPointStatus::Cancelled
                        | WorkPointStatus::Invalidated
                )
            {
                active_blockers.push((
                    wp.id.clone(),
                    wp.title.clone(),
                    wp.blocks_work_point_ids.clone(),
                ));
            }
        }

        // Build set of all blocked work point IDs
        let mut blocked_set: HashMap<String, (String, String)> = HashMap::new(); // blocked_id -> (blocker_id, blocker_title)
        for (blocker_id, blocker_title, blocked_ids) in &active_blockers {
            for bid in blocked_ids {
                blocked_set.insert(bid.clone(), (blocker_id.clone(), blocker_title.clone()));
            }
        }

        let mut candidates = Vec::new();
        let mut blocked_candidates = Vec::new();

        for wp in &all_work_points {
            // Skip terminal statuses
            if matches!(
                wp.status,
                WorkPointStatus::Completed
                    | WorkPointStatus::Cancelled
                    | WorkPointStatus::Invalidated
                    | WorkPointStatus::Blocked
            ) {
                continue;
            }

            // Check if blocked by active corrective work
            if let Some((blocker_id, blocker_title)) = blocked_set.get(&wp.id) {
                blocked_candidates.push(BlockedCandidate {
                    work_point_id: wp.id.clone(),
                    work_point_title: wp.title.clone(),
                    blocker_id: blocker_id.clone(),
                    blocker_title: blocker_title.clone(),
                    reason: format!("blocked_by:{}", blocker_id),
                });
                continue;
            }

            // Check active leases
            let active_lease_count: i64 = connection.query_row(
                "SELECT COUNT(*) FROM project_runs WHERE work_point_id = ?1 AND status IN ('claimed', 'active', 'interrupted') AND julianday(lease_expires_at) > julianday(?2)",
                params![wp.id, now],
                |row| row.get(0),
            )?;
            if active_lease_count > 0 {
                continue;
            }

            // Check dependencies exist and are completed
            let mut all_deps_exist = true;
            let mut all_deps_completed = true;
            let mut dependency_titles = Vec::new();

            for dep_id in &wp.dependency_ids {
                match load_work_point(&connection, dep_id) {
                    Ok(dep) => {
                        dependency_titles.push(dep.title.clone());
                        if dep.status != WorkPointStatus::Completed {
                            all_deps_completed = false;
                        }
                    }
                    Err(_) => {
                        all_deps_exist = false;
                        dependency_titles.push(format!("<missing: {dep_id}>"));
                    }
                }
            }

            if !all_deps_exist {
                continue;
            }
            if !all_deps_completed {
                continue;
            }

            let mut reasons = Vec::new();
            if wp.status == WorkPointStatus::Draft || wp.status == WorkPointStatus::Proposed {
                reasons.push("status is ready for work".to_string());
            }
            if wp.dependency_ids.is_empty() {
                reasons.push("no dependencies".to_string());
            } else {
                reasons.push("all dependencies completed".to_string());
            }

            // Determine required_reason based on ranking tier
            let required_reason = if wp.kind != WorkPointKind::Feature
                && (wp.priority == Priority::Urgent || wp.priority == Priority::High)
            {
                Some("urgent_fix".to_string())
            } else if wp.repairs_work_point_ids.iter().any(|id| {
                open_blocker_issues.contains_key(id) || open_blocker_review_points.contains_key(id)
            }) || wp.supersedes_work_point_ids.iter().any(|id| {
                open_blocker_issues.contains_key(id) || open_blocker_review_points.contains_key(id)
            }) {
                Some("resolves_blocker".to_string())
            } else {
                Some("ready".to_string())
            };

            candidates.push(RunnableWorkPointCandidate {
                work_point: wp.clone(),
                roadmap_id: roadmap_id.to_string(),
                roadmap_title: roadmap.title.clone(),
                dependency_titles,
                reasons,
                required_reason,
            });
        }

        // Sort by ranking tiers: urgent_fix > resolves_blocker > ready, then by ordering
        fn tier_rank(reason: &Option<String>) -> u8 {
            match reason.as_deref() {
                Some("urgent_fix") => 0,
                Some("resolves_blocker") => 1,
                _ => 2,
            }
        }

        candidates.sort_by(|a, b| {
            tier_rank(&a.required_reason)
                .cmp(&tier_rank(&b.required_reason))
                .then_with(|| a.work_point.ordering.cmp(&b.work_point.ordering))
                .then_with(|| a.work_point.id.cmp(&b.work_point.id))
        });

        Ok(RunnableCandidates {
            roadmap_id: roadmap_id.to_string(),
            candidates,
            blocked: blocked_candidates,
        })
    }

    pub fn build_work_graph(&self, roadmap_id: &str) -> Result<WorkGraph, PlanningStoreError> {
        require_non_empty("roadmapId", roadmap_id)?;

        let connection = self.open_connection()?;
        let now = now_string()?;
        let work_points = list_work_points_for_roadmap(&connection, roadmap_id)?;
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        for wp in &work_points {
            let plan_count: usize = connection
                .query_row(
                    "SELECT COUNT(*) FROM plans WHERE EXISTS (SELECT 1 FROM json_each(plans.targeted_work_point_ids_json) WHERE json_each.value = ?1)",
                    params![wp.id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let has_active_lease: bool = connection
                .query_row(
                    "SELECT COUNT(*) FROM project_runs WHERE work_point_id = ?1 AND status IN ('claimed', 'active', 'interrupted') AND julianday(lease_expires_at) > julianday(?2)",
                    params![wp.id, now],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap_or(0)
                > 0;

            nodes.push(WorkGraphNode {
                work_point: wp.clone(),
                plan_count,
                has_active_lease,
            });

            for dep_id in &wp.dependency_ids {
                edges.push(WorkGraphEdge {
                    source_id: dep_id.clone(),
                    target_id: wp.id.clone(),
                });
            }
        }

        Ok(WorkGraph { nodes, edges })
    }

    pub fn list_events(&self) -> Result<Vec<PlanningEvent>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT event_id, entity_type, entity_id, aggregate_type, aggregate_id, correlation_id, causation_id, run_id, stream_id, sequence, parent_event_id, event_type, timestamp, payload_json FROM planning_events ORDER BY rowid ASC",
        )?;
        let rows = statement.query_map([], row_to_event)?;
        collect_rows(rows)
    }

    pub fn list_events_in_scope(
        &self,
        scope_key: &str,
    ) -> Result<Vec<PlanningEvent>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT event_id, entity_type, entity_id, aggregate_type, aggregate_id, correlation_id, causation_id, run_id, stream_id, sequence, parent_event_id, event_type, timestamp, payload_json FROM planning_events WHERE scope_key = ?1 ORDER BY rowid ASC",
        )?;
        let rows =
            statement.query_map(params![normalize_scope_key_value(scope_key)], row_to_event)?;
        collect_rows(rows)
    }

    pub fn health(&self) -> Result<PlanningHealthReport, PlanningStoreError> {
        let connection = self.open_connection()?;
        Ok(PlanningHealthReport {
            db_path: self.db_path.display().to_string(),
            schema_version: CURRENT_SCHEMA_VERSION.to_string(),
            event_count: count_table(&connection, "planning_events")?,
            active_validation_finding_count: count_table(&connection, "validation_findings")?,
            scope_count: count_table(&connection, "scopes")?,
            goal_count: count_table(&connection, "goals")?,
            roadmap_count: count_table(&connection, "roadmaps")?,
            roadmap_section_count: count_table(&connection, "roadmap_sections")?,
            work_point_count: count_table(&connection, "work_points")?,
            plan_count: count_table(&connection, "plans")?,
            todo_count: count_table(&connection, "todos")?,
            issue_count: count_table(&connection, "issues")?,
            review_point_count: count_table(&connection, "review_points")?,
            insight_count: count_table(&connection, "insights")?,
            project_run_count: count_table(&connection, "project_runs")?,
            graph_node_count: count_table(&connection, "planning_nodes")?,
            graph_edge_count: count_table(&connection, "planning_edges")?,
        })
    }

    pub fn validate_all(&self) -> Result<ValidationRunReport, PlanningStoreError> {
        let connection = self.open_connection()?;
        let entities = collect_entities(&connection)?;
        let mut entity_reports = Vec::with_capacity(entities.len());
        let mut all_findings = Vec::new();

        for (entity_type, entity_id) in entities {
            let findings = validate_entity(&connection, entity_type, &entity_id)?;
            persist_validation_findings(&connection, entity_type, &entity_id, &findings)?;
            let validation = ValidationReport::from_findings(findings.clone());
            all_findings.extend(findings);
            entity_reports.push(crate::EntityValidationView {
                entity_type,
                entity_id,
                validation,
            });
        }

        Ok(ValidationRunReport {
            status: ValidationReport::from_findings(all_findings.clone()).status,
            scope_mode: "all".to_string(),
            scope_key: "all".to_string(),
            findings: all_findings,
            entity_reports,
        })
    }

    pub fn validate_all_in_scope(
        &self,
        scope_key: &str,
    ) -> Result<ValidationRunReport, PlanningStoreError> {
        let connection = self.open_connection()?;
        let entities = collect_entities_in_scope(&connection, scope_key)?;
        let mut entity_reports = Vec::with_capacity(entities.len());
        let mut all_findings = Vec::new();

        for (entity_type, entity_id) in entities {
            let findings = validate_entity(&connection, entity_type, &entity_id)?;
            persist_validation_findings(&connection, entity_type, &entity_id, &findings)?;
            let validation = ValidationReport::from_findings(findings.clone());
            all_findings.extend(findings);
            entity_reports.push(crate::EntityValidationView {
                entity_type,
                entity_id,
                validation,
            });
        }

        Ok(ValidationRunReport {
            status: ValidationReport::from_findings(all_findings.clone()).status,
            scope_mode: "single".to_string(),
            scope_key: scope_key.to_string(),
            findings: all_findings,
            entity_reports,
        })
    }

    pub fn render_projection(
        &self,
        entity_type: EntityType,
        entity_id: &str,
        format: ProjectionFormat,
        output_path: &Path,
    ) -> Result<RenderedProjection, PlanningStoreError> {
        let parent = output_path.parent().ok_or_else(|| {
            PlanningStoreError::ProjectionParentMissing(output_path.to_path_buf())
        })?;
        if !parent.exists() {
            return Err(PlanningStoreError::ProjectionParentMissing(
                parent.to_path_buf(),
            ));
        }

        let rendered = match (entity_type, format) {
            (EntityType::Goal, ProjectionFormat::Markdown) => {
                let view = self.goal(entity_id)?;
                render_goal_markdown(&view)
            }
            (EntityType::Goal, ProjectionFormat::Json) => {
                serde_json::to_string_pretty(&self.goal(entity_id)?)?
            }
            (EntityType::Roadmap, ProjectionFormat::Markdown) => {
                let view = self.roadmap(entity_id)?;
                render_roadmap_markdown(&view)
            }
            (EntityType::Roadmap, ProjectionFormat::Json) => {
                serde_json::to_string_pretty(&self.roadmap(entity_id)?)?
            }
            (EntityType::Plan, ProjectionFormat::Markdown) => {
                let view = self.plan(entity_id)?;
                render_plan_markdown(&view)
            }
            (EntityType::Plan, ProjectionFormat::Json) => {
                serde_json::to_string_pretty(&self.plan(entity_id)?)?
            }
            (EntityType::Issue, ProjectionFormat::Markdown) => {
                let view = self.issue(entity_id)?;
                render_issue_markdown(&view)
            }
            (EntityType::Issue, ProjectionFormat::Json) => {
                serde_json::to_string_pretty(&self.issue(entity_id)?)?
            }
            (EntityType::Insight, ProjectionFormat::Markdown) => {
                let view = self.insight(entity_id)?;
                render_insight_markdown(&view)
            }
            (EntityType::Insight, ProjectionFormat::Json) => {
                serde_json::to_string_pretty(&self.insight(entity_id)?)?
            }
            _ => {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "projection rendering is not implemented for {} as {}",
                    entity_type.as_str(),
                    format.as_str()
                )))
            }
        };

        fs::write(output_path, rendered).map_err(|source| PlanningStoreError::ProjectionWrite {
            path: output_path.to_path_buf(),
            source,
        })?;

        let revision = entity_revision(&self.open_connection()?, entity_type, entity_id)?;
        Ok(RenderedProjection {
            entity_type,
            entity_id: entity_id.to_string(),
            format,
            revision,
            output_path: output_path.display().to_string(),
        })
    }

    pub fn render_projection_in_scope(
        &self,
        scope_key: &str,
        entity_type: EntityType,
        entity_id: &str,
        format: ProjectionFormat,
        output_path: &Path,
    ) -> Result<RenderedProjection, PlanningStoreError> {
        let connection = self.open_connection()?;
        let normalized_scope_key = normalize_scope_key_value(scope_key);
        ensure_entity_in_scope(&connection, entity_type, entity_id, &normalized_scope_key)?;
        self.render_projection(entity_type, entity_id, format, output_path)
    }

    fn open_connection(&self) -> Result<Connection, PlanningStoreError> {
        if let Some(parent) = self.db_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|source| {
                    PlanningStoreError::CreateDirectory {
                        path: parent.to_path_buf(),
                        source,
                    }
                })?;
            }
        }

        let mut connection = Connection::open(&self.db_path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let transaction = connection.transaction()?;
        create_schema(&transaction)?;
        ensure_schema_version(&transaction)?;
        transaction.commit()?;
        Ok(connection)
    }

    // ── Graph node methods ──────────────────────────────────────────────────────

    pub fn create_graph_node(
        &self,
        input: CreateGraphNodeInput,
    ) -> Result<MutationResult<PlanningGraphNode>, PlanningStoreError> {
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("title", &input.title)?;
        require_non_empty("summary", &input.summary)?;
        require_non_empty("status", &input.status)?;
        require_kebab_token("status", &input.status)?;
        if let Some(ref explicit_id) = input.id {
            require_non_empty("id", explicit_id)?;
        }

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let scope_key = normalized_scope_key(input.scope_key);
        ensure_scope_exists(&transaction, &scope_key)?;
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let record = PlanningGraphNode {
            id: id.clone(),
            scope_key: scope_key.clone(),
            kind: input.kind,
            title: input.title.trim().to_string(),
            summary: input.summary.trim().to_string(),
            status: input.status.trim().to_string(),
            payload: input.payload,
            tags: normalize_string_list(input.tags),
            revision: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        transaction.execute(
            r#"
        INSERT INTO planning_nodes (
            id, scope_key, kind, title, summary, status,
            payload_json, tags_json, revision, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
            params![
                record.id,
                record.scope_key,
                record.kind.as_str(),
                record.title,
                record.summary,
                record.status,
                to_json_text(&record.payload)?,
                to_json_text(&record.tags)?,
                record.revision,
                record.created_at,
                record.updated_at,
            ],
        )?;

        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::GraphNode,
                &id,
                EntityType::GraphNode,
                &id,
                &input.correlation_id,
                input.run_id,
                "graph-node.created",
                serde_json::to_value(&record)?,
            )?,
        )?;

        // Phase 3: graph validators run during validate_all but are not called at write time
        let validation = ValidationReport::from_findings(Vec::new());
        rebuild_tag_index_for_entity(&transaction, EntityType::GraphNode, &id, &record.tags)?;
        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn graph_node(&self, id: &str) -> Result<PlanningGraphNode, PlanningStoreError> {
        let connection = self.open_connection()?;
        load_graph_node(&connection, id)
    }

    pub fn graph_node_view(
        &self,
        node_id: &str,
        scope_key: &str,
    ) -> Result<GraphNodeView, PlanningStoreError> {
        let connection = self.open_connection()?;
        let node = load_graph_node(&connection, node_id)?;
        let normalized_scope = normalize_scope_key_value(scope_key);
        if node.scope_key != normalized_scope {
            return Err(PlanningStoreError::InvalidInput(format!(
                "graph node `{}` is in scope `{}`, not `{}`",
                node_id, node.scope_key, normalized_scope
            )));
        }
        let incoming = list_incoming_edges_in_scope(&connection, node_id, &normalized_scope, None)?;
        let outgoing = list_outgoing_edges_in_scope(&connection, node_id, &normalized_scope, None)?;
        let connected = load_connected_graph_node_summaries(&connection, &incoming, &outgoing)?;
        let tags: Vec<String> = parse_json_column(
            connection
                .query_row(
                    "SELECT tags_json FROM planning_nodes WHERE id = ?1",
                    params![node_id],
                    |row| row.get::<_, String>(0),
                )
                .map_err(|e| map_not_found(e, EntityType::GraphNode, node_id))?,
        )
        .map_err(PlanningStoreError::from)?;
        let findings = validate_entity(&connection, EntityType::GraphNode, node_id)?;
        let validation = ValidationReport::from_findings(findings);
        Ok(GraphNodeView {
            node,
            incoming_edges: incoming,
            outgoing_edges: outgoing,
            connected_nodes: connected,
            tags,
            validation,
        })
    }

    pub fn graph_edge_view(
        &self,
        edge_id: &str,
        scope_key: &str,
    ) -> Result<GraphEdgeView, PlanningStoreError> {
        let connection = self.open_connection()?;
        let edge = load_graph_edge(&connection, edge_id)?;
        let normalized_scope = normalize_scope_key_value(scope_key);
        if edge.scope_key != normalized_scope {
            return Err(PlanningStoreError::InvalidInput(format!(
                "graph edge `{}` is in scope `{}`, not `{}`",
                edge_id, edge.scope_key, normalized_scope
            )));
        }
        let source_node = load_graph_node_summary(&connection, &edge.source_node_id)?;
        let target_node = load_graph_node_summary(&connection, &edge.target_node_id)?;
        let findings = validate_entity(&connection, EntityType::GraphEdge, edge_id)?;
        let validation = ValidationReport::from_findings(findings);
        Ok(GraphEdgeView {
            edge,
            source_node,
            target_node,
            validation,
        })
    }

    /// Build a typed acceptance view including requirements, coverage, and evidence paths.
    pub fn acceptance_view(
        &self,
        node_id: &str,
        scope_key: &str,
    ) -> Result<AcceptanceView, PlanningStoreError> {
        let connection = self.open_connection()?;
        let node = load_graph_node(&connection, node_id)?;
        let normalized_scope = normalize_scope_key_value(scope_key);
        if node.scope_key != normalized_scope {
            return Err(PlanningStoreError::InvalidInput(format!(
                "graph node `{}` is in scope `{}`, not `{}`",
                node_id, node.scope_key, normalized_scope
            )));
        }

        if node.kind != PlanningNodeKind::Acceptance {
            return Err(PlanningStoreError::InvalidInput(format!(
                "graph node `{}` is a `{}` node, not an acceptance node",
                node_id,
                node.kind.as_str()
            )));
        }

        // Nodes that require this acceptance (Requires edge: X --requires--> this)
        let incoming = list_incoming_edges_in_scope(&connection, node_id, &normalized_scope, None)?;
        let required_by: Vec<PlanningGraphNode> = incoming
            .iter()
            .filter(|e| e.kind == PlanningEdgeKind::Requires && e.status == "active")
            .filter_map(|e| load_graph_node(&connection, &e.source_node_id).ok())
            .collect();

        // Abstract acceptances this concrete satisfies (Satisfies edge: this --satisfies--> abstract)
        let outgoing = list_outgoing_edges_in_scope(&connection, node_id, &normalized_scope, None)?;
        let satisfied_abstracts: Vec<PlanningGraphNode> = outgoing
            .iter()
            .filter(|e| e.kind == PlanningEdgeKind::Satisfies && e.status == "active")
            .filter_map(|e| load_graph_node(&connection, &e.target_node_id).ok())
            .collect();

        // Concrete acceptances that satisfy this abstract (Satisfies edge: concrete --satisfies--> this)
        let satisfying_concretes: Vec<PlanningGraphNode> = incoming
            .iter()
            .filter(|e| e.kind == PlanningEdgeKind::Satisfies && e.status == "active")
            .filter_map(|e| load_graph_node(&connection, &e.source_node_id).ok())
            .collect();

        // Evidence attached to this acceptance (EvidencedBy edge: this --evidenced-by--> evidence)
        let attached_evidence: Vec<PlanningGraphNode> = outgoing
            .iter()
            .filter(|e| e.kind == PlanningEdgeKind::EvidencedBy && e.status == "active")
            .filter_map(|e| load_graph_node(&connection, &e.target_node_id).ok())
            .collect();

        let findings = validate_entity(&connection, EntityType::GraphNode, node_id)?;
        let validation = ValidationReport::from_findings(findings);

        Ok(AcceptanceView {
            node,
            required_by,
            satisfied_abstracts,
            satisfying_concretes,
            attached_evidence,
            validation,
        })
    }

    /// Build a typed evidence view including linked targets.
    pub fn evidence_view(
        &self,
        node_id: &str,
        scope_key: &str,
    ) -> Result<EvidenceView, PlanningStoreError> {
        let connection = self.open_connection()?;
        let node = load_graph_node(&connection, node_id)?;
        let normalized_scope = normalize_scope_key_value(scope_key);
        if node.scope_key != normalized_scope {
            return Err(PlanningStoreError::InvalidInput(format!(
                "graph node `{}` is in scope `{}`, not `{}`",
                node_id, node.scope_key, normalized_scope
            )));
        }

        if node.kind != PlanningNodeKind::Evidence {
            return Err(PlanningStoreError::InvalidInput(format!(
                "graph node `{}` is a `{}` node, not an evidence node",
                node_id,
                node.kind.as_str()
            )));
        }

        // Targets this evidence is attached to (EvidencedBy edge: target --evidenced-by--> this)
        let incoming = list_incoming_edges_in_scope(&connection, node_id, &normalized_scope, None)?;
        let attached_to: Vec<PlanningGraphNode> = incoming
            .iter()
            .filter(|e| e.kind == PlanningEdgeKind::EvidencedBy && e.status == "active")
            .filter_map(|e| load_graph_node(&connection, &e.source_node_id).ok())
            .collect();

        let findings = validate_entity(&connection, EntityType::GraphNode, node_id)?;
        let validation = ValidationReport::from_findings(findings);

        Ok(EvidenceView {
            node,
            attached_to,
            validation,
        })
    }

    pub fn list_graph_nodes(
        &self,
        scope_key: &str,
        kind: Option<PlanningNodeKind>,
    ) -> Result<Vec<PlanningGraphNode>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        if let Some(k) = kind {
            let mut stmt = connection.prepare(
                "SELECT id, scope_key, kind, title, summary, status, payload_json, tags_json, revision, created_at, updated_at FROM planning_nodes WHERE scope_key = ?1 AND kind = ?2 ORDER BY updated_at DESC, id ASC"
            )?;
            let rows = stmt.query_map(params![normalized, k.as_str()], row_to_graph_node)?;
            collect_rows(rows)
        } else {
            let mut stmt = connection.prepare(
                "SELECT id, scope_key, kind, title, summary, status, payload_json, tags_json, revision, created_at, updated_at FROM planning_nodes WHERE scope_key = ?1 ORDER BY updated_at DESC, id ASC"
            )?;
            let rows = stmt.query_map(params![normalized], row_to_graph_node)?;
            collect_rows(rows)
        }
    }

    pub fn update_graph_node_status(
        &self,
        input: UpdateGraphNodeStatusInput,
    ) -> Result<MutationResult<PlanningGraphNode>, PlanningStoreError> {
        require_non_empty("nodeId", &input.node_id)?;
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("status", &input.status)?;
        require_kebab_token("status", &input.status)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.active_scope_key);
        ensure_entity_in_scope(
            &transaction,
            EntityType::GraphNode,
            &input.node_id,
            &active_scope_key,
        )?;

        let old_record = load_graph_node(&transaction, &input.node_id)?;
        let old_status = old_record.status.clone();
        let now = now_string()?;

        transaction.execute(
            "UPDATE planning_nodes SET status = ?1, revision = revision + 1, updated_at = ?2 WHERE id = ?3",
            params![input.status.trim(), now, input.node_id],
        )?;

        let record = load_graph_node(&transaction, &input.node_id)?;

        let payload = serde_json::json!({
            "status": record.status,
            "previousStatus": old_status,
            "revision": record.revision,
        });

        let correlation_id = &input.correlation_id;
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::GraphNode,
                &record.id,
                EntityType::GraphNode,
                &record.id,
                correlation_id,
                input.run_id,
                "graph-node.status-updated",
                payload,
            )?,
        )?;

        let findings = validate_entity(&transaction, EntityType::GraphNode, &record.id)?;
        persist_validation_findings(&transaction, EntityType::GraphNode, &record.id, &findings)?;
        let validation = ValidationReport::from_findings(findings);

        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn update_graph_edge_status(
        &self,
        input: UpdateGraphEdgeStatusInput,
    ) -> Result<MutationResult<PlanningGraphEdge>, PlanningStoreError> {
        require_non_empty("edgeId", &input.edge_id)?;
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("status", &input.status)?;
        require_kebab_token("status", &input.status)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.active_scope_key);
        ensure_entity_in_scope(
            &transaction,
            EntityType::GraphEdge,
            &input.edge_id,
            &active_scope_key,
        )?;

        let old_record = load_graph_edge(&transaction, &input.edge_id)?;
        let old_status = old_record.status.clone();
        let new_status = input.status.trim().to_string();

        // If transitioning TO "active", recheck invariants (checks on every active transition,
        // including when the edge is already active — the duplicate query excludes the current edge)
        if new_status == "active" {
            // Recheck: no duplicate active edge
            let dup_count: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM planning_edges WHERE scope_key = ?1 AND kind = ?2 AND source_node_id = ?3 AND target_node_id = ?4 AND status = 'active' AND id != ?5",
                params![old_record.scope_key, old_record.kind.as_str(), old_record.source_node_id, old_record.target_node_id, input.edge_id],
                |row| row.get(0),
            )?;
            if dup_count > 0 {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "cannot set edge status to active: duplicate active {} edge from `{}` to `{}` in scope `{}`",
                    old_record.kind.as_str(),
                    old_record.source_node_id,
                    old_record.target_node_id,
                    old_record.scope_key
                )));
            }

            // Recheck: no cycle for acyclic edge kinds
            match old_record.kind {
                PlanningEdgeKind::DecomposesTo | PlanningEdgeKind::DependsOn
                    if would_create_graph_cycle(
                        &transaction,
                        &old_record.source_node_id,
                        &old_record.target_node_id,
                        &old_record.kind,
                    )? =>
                {
                    return Err(PlanningStoreError::InvalidInput(format!(
                        "cannot set edge status to active: {} edge from `{}` to `{}` would create a cycle",
                        old_record.kind.as_str(),
                        old_record.source_node_id,
                        old_record.target_node_id
                    )));
                }
                _ => {}
            }
        }

        let now = now_string()?;
        transaction.execute(
            "UPDATE planning_edges SET status = ?1, revision = revision + 1, updated_at = ?2 WHERE id = ?3",
            params![new_status, now, input.edge_id],
        )?;

        let record = load_graph_edge(&transaction, &input.edge_id)?;

        let payload = serde_json::json!({
            "status": record.status,
            "previousStatus": old_status,
            "revision": record.revision,
        });

        let correlation_id = &input.correlation_id;
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::GraphEdge,
                &record.id,
                EntityType::GraphNode,
                &record.source_node_id,
                correlation_id,
                input.run_id,
                "graph-edge.status-updated",
                payload,
            )?,
        )?;

        let findings = validate_entity(&transaction, EntityType::GraphEdge, &record.id)?;
        persist_validation_findings(&transaction, EntityType::GraphEdge, &record.id, &findings)?;
        let validation = ValidationReport::from_findings(findings);

        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn revise_graph_node(
        &self,
        input: ReviseGraphNodeInput,
    ) -> Result<MutationResult<PlanningGraphNode>, PlanningStoreError> {
        require_non_empty("nodeId", &input.node_id)?;
        require_non_empty("correlationId", &input.correlation_id)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.active_scope_key);
        ensure_entity_in_scope(
            &transaction,
            EntityType::GraphNode,
            &input.node_id,
            &active_scope_key,
        )?;

        let existing = load_graph_node(&transaction, &input.node_id)?;
        let now = now_string()?;

        let title = input
            .title
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or(existing.title.clone());
        let summary = input
            .summary
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or(existing.summary.clone());
        let status = match &input.status {
            Some(s) => {
                require_kebab_token("status", s)?;
                s.trim().to_string()
            }
            None => existing.status.clone(),
        };
        let payload = input.payload.unwrap_or(existing.payload.clone());
        let tags = if input.clear_tags {
            Vec::new()
        } else {
            input
                .tags
                .map(normalize_string_list)
                .unwrap_or(existing.tags.clone())
        };

        transaction.execute(
            "UPDATE planning_nodes SET title = ?1, summary = ?2, status = ?3, payload_json = ?4, tags_json = ?5, revision = revision + 1, updated_at = ?6 WHERE id = ?7",
            params![title, summary, status, to_json_text(&payload)?, to_json_text(&tags)?, now, input.node_id],
        )?;

        let record = load_graph_node(&transaction, &input.node_id)?;

        let event_payload = serde_json::json!({
            "title": record.title,
            "summary": record.summary,
            "status": record.status,
            "tags": record.tags,
            "revision": record.revision,
        });

        let correlation_id = &input.correlation_id;
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::GraphNode,
                &record.id,
                EntityType::GraphNode,
                &record.id,
                correlation_id,
                input.run_id,
                "graph-node.revised",
                event_payload,
            )?,
        )?;

        let findings = validate_entity(&transaction, EntityType::GraphNode, &record.id)?;
        persist_validation_findings(&transaction, EntityType::GraphNode, &record.id, &findings)?;
        let validation = ValidationReport::from_findings(findings);
        rebuild_tag_index_for_entity(
            &transaction,
            EntityType::GraphNode,
            &record.id,
            &record.tags,
        )?;

        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn revise_graph_edge(
        &self,
        input: ReviseGraphEdgeInput,
    ) -> Result<MutationResult<PlanningGraphEdge>, PlanningStoreError> {
        require_non_empty("edgeId", &input.edge_id)?;
        require_non_empty("correlationId", &input.correlation_id)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.active_scope_key);
        ensure_entity_in_scope(
            &transaction,
            EntityType::GraphEdge,
            &input.edge_id,
            &active_scope_key,
        )?;

        let existing = load_graph_edge(&transaction, &input.edge_id)?;
        let now = now_string()?;

        let old_status = existing.status.clone();
        let status = match &input.status {
            Some(s) => {
                require_kebab_token("status", s)?;
                s.trim().to_string()
            }
            None => existing.status.clone(),
        };
        let payload = input.payload.unwrap_or(existing.payload.clone());

        // If status changes TO "active", recheck invariants (checks on every active transition,
        // including when the edge is already active — the duplicate query excludes the current edge)
        if status == "active" {
            let dup_count: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM planning_edges WHERE scope_key = ?1 AND kind = ?2 AND source_node_id = ?3 AND target_node_id = ?4 AND status = 'active' AND id != ?5",
                params![existing.scope_key, existing.kind.as_str(), existing.source_node_id, existing.target_node_id, input.edge_id],
                |row| row.get(0),
            )?;
            if dup_count > 0 {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "cannot revise edge status to active: duplicate active {} edge from `{}` to `{}` in scope `{}`",
                    existing.kind.as_str(),
                    existing.source_node_id,
                    existing.target_node_id,
                    existing.scope_key
                )));
            }
            match existing.kind {
                PlanningEdgeKind::DecomposesTo | PlanningEdgeKind::DependsOn
                    if would_create_graph_cycle(
                        &transaction,
                        &existing.source_node_id,
                        &existing.target_node_id,
                        &existing.kind,
                    )? =>
                {
                    return Err(PlanningStoreError::InvalidInput(format!(
                        "cannot revise edge status to active: {} edge from `{}` to `{}` would create a cycle",
                        existing.kind.as_str(),
                        existing.source_node_id,
                        existing.target_node_id
                    )));
                }
                _ => {}
            }
        }

        transaction.execute(
            "UPDATE planning_edges SET status = ?1, payload_json = ?2, revision = revision + 1, updated_at = ?3 WHERE id = ?4",
            params![status, to_json_text(&payload)?, now, input.edge_id],
        )?;

        let record = load_graph_edge(&transaction, &input.edge_id)?;

        let event_payload = serde_json::json!({
            "status": record.status,
            "previousStatus": old_status,
            "revision": record.revision,
        });

        let correlation_id = &input.correlation_id;
        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::GraphEdge,
                &record.id,
                EntityType::GraphNode,
                &record.source_node_id,
                correlation_id,
                input.run_id,
                "graph-edge.revised",
                event_payload,
            )?,
        )?;

        let findings = validate_entity(&transaction, EntityType::GraphEdge, &record.id)?;
        persist_validation_findings(&transaction, EntityType::GraphEdge, &record.id, &findings)?;
        let validation = ValidationReport::from_findings(findings);

        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    // ── Graph edge methods ──────────────────────────────────────────────────────

    pub fn create_graph_edge(
        &self,
        input: CreateGraphEdgeInput,
    ) -> Result<MutationResult<PlanningGraphEdge>, PlanningStoreError> {
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("source_node_id", &input.source_node_id)?;
        require_non_empty("target_node_id", &input.target_node_id)?;
        require_non_empty("status", &input.status)?;
        require_kebab_token("status", &input.status)?;
        if let Some(ref explicit_id) = input.id {
            require_non_empty("id", explicit_id)?;
        }

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let scope_key = normalized_scope_key(input.scope_key);
        ensure_scope_exists(&transaction, &scope_key)?;

        // Preflight: load source and target nodes
        let source = load_graph_node(&transaction, &input.source_node_id).map_err(|_| {
            PlanningStoreError::InvalidInput(format!(
                "sourceNodeId references missing node `{}`",
                input.source_node_id
            ))
        })?;
        let target = load_graph_node(&transaction, &input.target_node_id).map_err(|_| {
            PlanningStoreError::InvalidInput(format!(
                "targetNodeId references missing node `{}`",
                input.target_node_id
            ))
        })?;

        // Preflight: scopes must match
        if source.scope_key != scope_key || target.scope_key != scope_key {
            return Err(PlanningStoreError::InvalidInput(
                "source and target nodes must belong to the same scope as the edge".to_string(),
            ));
        }

        // Preflight: reject self-loops (edges from a node to itself)
        if input.source_node_id == input.target_node_id {
            return Err(PlanningStoreError::InvalidInput(format!(
                "self-referential {} edge is not allowed: source and target must be different nodes",
                input.kind.as_str(),
            )));
        }

        // Preflight: valid source/target node kinds for this edge kind
        validate_edge_kind_pair(&input.kind, &source.kind, &target.kind)?;

        let final_status = input.status.trim();

        // Preflight: no duplicate active edge (also enforced by UNIQUE partial index)
        // Preflight: cycle detection for acyclic families
        // Only run active-only invariants when the final status is "active"
        if final_status == "active" {
            // Preflight: no duplicate active edge (also enforced by UNIQUE partial index)
            let dup_count: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM planning_edges WHERE scope_key = ?1 AND kind = ?2 AND source_node_id = ?3 AND target_node_id = ?4 AND status = 'active'",
                params![scope_key, input.kind.as_str(), input.source_node_id, input.target_node_id],
                |row| row.get(0),
            )?;
            if dup_count > 0 {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "duplicate active {} edge from `{}` to `{}` in scope `{}`",
                    input.kind.as_str(),
                    input.source_node_id,
                    input.target_node_id,
                    scope_key
                )));
            }

            // Preflight: cycle detection for acyclic families
            match input.kind {
                PlanningEdgeKind::DecomposesTo | PlanningEdgeKind::DependsOn
                    if would_create_graph_cycle(
                        &transaction,
                        &input.source_node_id,
                        &input.target_node_id,
                        &input.kind,
                    )? =>
                {
                    return Err(PlanningStoreError::InvalidInput(format!(
                        "adding this {} edge from `{}` to `{}` would create a cycle",
                        input.kind.as_str(),
                        input.source_node_id,
                        input.target_node_id
                    )));
                }
                _ => {}
            }
        }

        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let record = PlanningGraphEdge {
            id: id.clone(),
            scope_key: scope_key.clone(),
            kind: input.kind,
            source_node_id: input.source_node_id.clone(),
            target_node_id: input.target_node_id.clone(),
            status: input.status.trim().to_string(),
            payload: input.payload,
            revision: 1,
            created_at: now.clone(),
            updated_at: now,
        };

        transaction.execute(
            r#"
        INSERT INTO planning_edges (
            id, scope_key, kind, source_node_id, target_node_id,
            status, payload_json, revision, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
            params![
                record.id,
                record.scope_key,
                record.kind.as_str(),
                record.source_node_id,
                record.target_node_id,
                record.status,
                to_json_text(&record.payload)?,
                record.revision,
                record.created_at,
                record.updated_at,
            ],
        )?;

        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::GraphEdge,
                &id,
                EntityType::GraphNode,
                &input.source_node_id,
                &input.correlation_id,
                input.run_id,
                "graph-edge.created",
                serde_json::to_value(&record)?,
            )?,
        )?;

        // Phase 3: graph edge validators run during validate_all but are not called at write time
        let validation = ValidationReport::from_findings(Vec::new());
        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    /// Create an acceptance node with typed payload.
    pub fn create_acceptance(
        &self,
        input: CreateAcceptanceInput,
    ) -> Result<MutationResult<PlanningGraphNode>, PlanningStoreError> {
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("title", &input.title)?;
        require_non_empty("status", &input.status)?;
        require_kebab_token("status", &input.status)?;

        let payload = serde_json::json!({
            "acceptanceKind": input.acceptance_kind.as_str(),
            "description": input.description,
            "verificationPolicy": input.verification_policy,
            "requiredEvidenceKinds": input.required_evidence_kinds.iter().map(|k| k.as_str()).collect::<Vec<_>>(),
        });
        let payload = if let Some(waiver) = &input.waiver {
            let mut p = payload;
            p["waiver"] = serde_json::json!(waiver);
            p
        } else {
            payload
        };

        self.create_graph_node(CreateGraphNodeInput {
            id: input.id,
            scope_key: input.scope_key,
            correlation_id: input.correlation_id,
            kind: PlanningNodeKind::Acceptance,
            title: input.title,
            summary: input.summary,
            status: input.status,
            payload,
            tags: input.tags,
            run_id: input.run_id,
        })
    }

    /// Create an evidence node with typed payload.
    pub fn create_evidence(
        &self,
        input: CreateEvidenceInput,
    ) -> Result<MutationResult<PlanningGraphNode>, PlanningStoreError> {
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("title", &input.title)?;
        require_non_empty("status", &input.status)?;
        require_kebab_token("status", &input.status)?;

        let payload = serde_json::json!({
            "evidenceKind": input.evidence_kind.as_str(),
            "summary": input.summary,
            "reference": input.reference,
            "content": input.content,
            "capturedAt": input.captured_at,
        });

        self.create_graph_node(CreateGraphNodeInput {
            id: input.id,
            scope_key: input.scope_key,
            correlation_id: input.correlation_id,
            kind: PlanningNodeKind::Evidence,
            title: input.title,
            summary: input.summary,
            status: input.status,
            payload,
            tags: input.tags,
            run_id: input.run_id,
        })
    }

    /// Create a Satisfies edge from a concrete acceptance to an abstract acceptance.
    pub fn satisfy_acceptance(
        &self,
        input: SatisfyAcceptanceInput,
    ) -> Result<MutationResult<PlanningGraphEdge>, PlanningStoreError> {
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("concrete_node_id", &input.concrete_node_id)?;
        require_non_empty("abstract_node_id", &input.abstract_node_id)?;
        require_non_empty("rationale", &input.rationale)?;

        // Preflight: both nodes must be Acceptance kind with correct direction
        let connection = self.open_connection()?;
        let concrete_node =
            load_graph_node(&connection, &input.concrete_node_id).map_err(|_| {
                PlanningStoreError::InvalidInput(format!(
                    "concrete_node_id `{}` references a missing node",
                    input.concrete_node_id
                ))
            })?;
        if concrete_node.kind != PlanningNodeKind::Acceptance {
            return Err(PlanningStoreError::InvalidInput(format!(
                "concrete_node_id `{}` is a `{}` node, not an acceptance node",
                input.concrete_node_id,
                concrete_node.kind.as_str()
            )));
        }
        let concrete_kind = concrete_node
            .payload
            .get("acceptanceKind")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if concrete_kind != "concrete" {
            return Err(PlanningStoreError::InvalidInput(format!(
                "concrete_node_id `{}` has acceptanceKind `{}`, expected `concrete`",
                input.concrete_node_id, concrete_kind
            )));
        }

        let abstract_node =
            load_graph_node(&connection, &input.abstract_node_id).map_err(|_| {
                PlanningStoreError::InvalidInput(format!(
                    "abstract_node_id `{}` references a missing node",
                    input.abstract_node_id
                ))
            })?;
        if abstract_node.kind != PlanningNodeKind::Acceptance {
            return Err(PlanningStoreError::InvalidInput(format!(
                "abstract_node_id `{}` is a `{}` node, not an acceptance node",
                input.abstract_node_id,
                abstract_node.kind.as_str()
            )));
        }
        let abstract_kind = abstract_node
            .payload
            .get("acceptanceKind")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if abstract_kind != "abstract" {
            return Err(PlanningStoreError::InvalidInput(format!(
                "abstract_node_id `{}` has acceptanceKind `{}`, expected `abstract`",
                input.abstract_node_id, abstract_kind
            )));
        }

        let payload = serde_json::json!({
            "rationale": input.rationale,
        });

        self.create_graph_edge(CreateGraphEdgeInput {
            id: input.id,
            scope_key: input.scope_key,
            correlation_id: input.correlation_id,
            kind: PlanningEdgeKind::Satisfies,
            source_node_id: input.concrete_node_id,
            target_node_id: input.abstract_node_id,
            status: "active".to_string(),
            payload,
            run_id: input.run_id,
        })
    }

    /// Create an EvidencedBy edge from a target node to an evidence node.
    pub fn attach_evidence(
        &self,
        input: AttachEvidenceInput,
    ) -> Result<MutationResult<PlanningGraphEdge>, PlanningStoreError> {
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("evidence_node_id", &input.evidence_node_id)?;
        require_non_empty("target_node_id", &input.target_node_id)?;

        let payload = if input.rationale.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::json!({
                "rationale": input.rationale,
            })
        };

        self.create_graph_edge(CreateGraphEdgeInput {
            id: input.id,
            scope_key: input.scope_key,
            correlation_id: input.correlation_id,
            kind: PlanningEdgeKind::EvidencedBy,
            source_node_id: input.target_node_id,
            target_node_id: input.evidence_node_id,
            status: "active".to_string(),
            payload,
            run_id: input.run_id,
        })
    }

    /// Finalize a graph node by transitioning it to a terminal status.
    ///
    /// Rejects finalization when the node or its connected edges have blocking
    /// validation findings. Acceptance/evidence gaps can be waived with
    /// `accepted_risk`; structural graph corruption cannot.
    pub fn finalize_graph_node(
        &self,
        input: FinalizeGraphNodeInput,
    ) -> Result<MutationResult<PlanningGraphNode>, PlanningStoreError> {
        require_non_empty("nodeId", &input.node_id)?;
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("status", &input.status)?;
        require_kebab_token("status", &input.status)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.active_scope_key);
        ensure_entity_in_scope(
            &transaction,
            EntityType::GraphNode,
            &input.node_id,
            &active_scope_key,
        )?;

        let node = load_graph_node(&transaction, &input.node_id)?;

        // Collect all findings from the node itself
        let node_findings = validate_entity(&transaction, EntityType::GraphNode, &node.id)?;

        // Collect findings from connected edges
        let incoming = list_incoming_edges_in_scope(&transaction, &node.id, &node.scope_key, None)?;
        let outgoing = list_outgoing_edges_in_scope(&transaction, &node.id, &node.scope_key, None)?;
        let mut all_edge_findings = Vec::new();
        for edge in incoming.iter().chain(outgoing.iter()) {
            if let Ok(mut ef) = validate_entity(&transaction, EntityType::GraphEdge, &edge.id) {
                all_edge_findings.append(&mut ef);
            }
        }

        // Determine blocking findings — three groups:
        //   1. Structural: always blocked, never waivable
        //   2. Type integrity: always blocked, never waivable (malformed payloads)
        //   3. Waivable gaps: blocked unless accepted_risk is provided
        let structural_codes: &[&str] = &[
            "GRAPH-EDGE-CYCLE",
            "GRAPH-EDGE-DUPLICATE-ACTIVE",
            "GRAPH-EDGE-MISSING-NODE",
            "GRAPH-EDGE-CROSS-SCOPE",
            "GRAPH-EDGE-KIND-MISMATCH",
        ];
        let type_integrity_codes: &[&str] = &["ACCEPTANCE-KIND-INVALID", "EVIDENCE-KIND-INVALID"];
        let waivable_gap_codes: &[&str] =
            &["ACCEPTANCE-COVERAGE-MISSING", "ACCEPTANCE-EVIDENCE-MISSING"];

        let all_findings: Vec<&crate::ValidationFinding> = node_findings
            .iter()
            .chain(all_edge_findings.iter())
            .collect();

        let has_structural = all_findings
            .iter()
            .any(|f| structural_codes.contains(&f.code.as_str()));
        let has_type_integrity = all_findings
            .iter()
            .any(|f| type_integrity_codes.contains(&f.code.as_str()));
        let has_waivable_gap = all_findings
            .iter()
            .any(|f| waivable_gap_codes.contains(&f.code.as_str()));

        // Structural corruption is always blocking
        if has_structural {
            let codes: Vec<_> = all_findings
                .iter()
                .filter(|f| structural_codes.contains(&f.code.as_str()))
                .map(|f| &f.code)
                .collect();
            return Err(PlanningStoreError::InvalidInput(format!(
                "cannot finalize graph node `{}`: blocking structural findings: {:?}",
                input.node_id, codes
            )));
        }

        // Type integrity violations are always blocking — malformed payloads
        if has_type_integrity {
            let codes: Vec<_> = all_findings
                .iter()
                .filter(|f| type_integrity_codes.contains(&f.code.as_str()))
                .map(|f| &f.code)
                .collect();
            return Err(PlanningStoreError::InvalidInput(format!(
                "cannot finalize graph node `{}`: invalid typed payloads: {:?}. Fix the payload before finalizing",
                input.node_id, codes
            )));
        }

        // Coverage/evidence gaps are blocking unless accepted_risk is provided
        if has_waivable_gap {
            match &input.accepted_risk {
                Some(rationale) if !rationale.trim().is_empty() => {
                    // Allow with accepted risk — rationale captured in event
                }
                _ => {
                    let codes: Vec<_> = all_findings
                        .iter()
                        .filter(|f| waivable_gap_codes.contains(&f.code.as_str()))
                        .map(|f| &f.code)
                        .collect();
                    return Err(PlanningStoreError::InvalidInput(format!(
                        "cannot finalize graph node `{}`: acceptance/evidence gaps: {:?}. Use --accepted-risk to waive",
                        input.node_id, codes
                    )));
                }
            }
        }

        // Perform the status update
        let old_status = node.status.clone();
        let now = now_string()?;
        transaction.execute(
            "UPDATE planning_nodes SET status = ?1, revision = revision + 1, updated_at = ?2 WHERE id = ?3",
            params![input.status.trim(), now, input.node_id],
        )?;

        let record = load_graph_node(&transaction, &input.node_id)?;

        // Build event payload
        let mut event_payload = serde_json::json!({
            "status": record.status,
            "previousStatus": old_status,
            "revision": record.revision,
        });
        let event_type = if input
            .accepted_risk
            .as_ref()
            .is_some_and(|r| !r.trim().is_empty())
        {
            event_payload["acceptedRisk"] =
                serde_json::json!(input.accepted_risk.as_deref().unwrap_or(""));
            "graph-node.finalized-with-accepted-risk"
        } else {
            "graph-node.finalized"
        };

        append_event(
            &transaction,
            build_event(
                &transaction,
                EntityType::GraphNode,
                &record.id,
                EntityType::GraphNode,
                &record.id,
                &input.correlation_id,
                input.run_id,
                event_type,
                event_payload,
            )?,
        )?;

        // Re-validate after finalization
        let node_findings = validate_entity(&transaction, EntityType::GraphNode, &record.id)?;
        persist_validation_findings(
            &transaction,
            EntityType::GraphNode,
            &record.id,
            &node_findings,
        )?;
        let validation = ValidationReport::from_findings(node_findings);

        transaction.commit()?;
        Ok(MutationResult { record, validation })
    }

    pub fn graph_edge(&self, id: &str) -> Result<PlanningGraphEdge, PlanningStoreError> {
        let connection = self.open_connection()?;
        connection
            .query_row(
                "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE id = ?1",
                params![id],
                row_to_graph_edge,
            )
            .map_err(|error| {
                if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
                    PlanningStoreError::NotFound {
                        entity_type: "graph-edge".to_string(),
                        entity_id: id.to_string(),
                    }
                } else {
                    PlanningStoreError::Sqlite(error)
                }
            })
    }

    pub fn list_graph_edges(
        &self,
        scope_key: &str,
        kind: Option<PlanningEdgeKind>,
    ) -> Result<Vec<PlanningGraphEdge>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        if let Some(k) = kind {
            let mut stmt = connection.prepare(
                "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE scope_key = ?1 AND kind = ?2 ORDER BY updated_at DESC, id ASC"
            )?;
            let rows = stmt.query_map(params![normalized, k.as_str()], row_to_graph_edge)?;
            collect_rows(rows)
        } else {
            let mut stmt = connection.prepare(
                "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE scope_key = ?1 ORDER BY updated_at DESC, id ASC"
            )?;
            let rows = stmt.query_map(params![normalized], row_to_graph_edge)?;
            collect_rows(rows)
        }
    }

    pub fn list_outgoing_edges(
        &self,
        node_id: &str,
        kind: Option<PlanningEdgeKind>,
    ) -> Result<Vec<PlanningGraphEdge>, PlanningStoreError> {
        let connection = self.open_connection()?;
        if let Some(k) = kind {
            let mut stmt = connection.prepare(
                "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE source_node_id = ?1 AND kind = ?2 ORDER BY updated_at DESC, id ASC"
            )?;
            let rows = stmt.query_map(params![node_id, k.as_str()], row_to_graph_edge)?;
            collect_rows(rows)
        } else {
            let mut stmt = connection.prepare(
                "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE source_node_id = ?1 ORDER BY updated_at DESC, id ASC"
            )?;
            let rows = stmt.query_map(params![node_id], row_to_graph_edge)?;
            collect_rows(rows)
        }
    }

    pub fn list_incoming_edges(
        &self,
        node_id: &str,
        kind: Option<PlanningEdgeKind>,
    ) -> Result<Vec<PlanningGraphEdge>, PlanningStoreError> {
        let connection = self.open_connection()?;
        if let Some(k) = kind {
            let mut stmt = connection.prepare(
                "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE target_node_id = ?1 AND kind = ?2 ORDER BY updated_at DESC, id ASC"
            )?;
            let rows = stmt.query_map(params![node_id, k.as_str()], row_to_graph_edge)?;
            collect_rows(rows)
        } else {
            let mut stmt = connection.prepare(
                "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE target_node_id = ?1 ORDER BY updated_at DESC, id ASC"
            )?;
            let rows = stmt.query_map(params![node_id], row_to_graph_edge)?;
            collect_rows(rows)
        }
    }

    // ─── Manifest / Upsert / Diff ─────────────────────────────────────────

    /// Upsert a graph node: create if not exists, update if exists in same scope.
    pub fn upsert_graph_node(
        &self,
        input: CreateGraphNodeInput,
    ) -> Result<(String, bool), PlanningStoreError> {
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("title", &input.title)?;
        require_non_empty("summary", &input.summary)?;
        require_non_empty("status", &input.status)?;
        require_kebab_token("status", &input.status)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let scope_key = normalized_scope_key(input.scope_key);
        ensure_scope_exists(&transaction, &scope_key)?;
        let now = now_string()?;

        // If user provides an explicit ID, try to find an existing node
        let (id, created) = if let Some(ref explicit_id) = input.id {
            require_non_empty("id", explicit_id)?;
            let existing: Option<PlanningGraphNode> = transaction
                .query_row(
                    "SELECT id, scope_key, kind, title, summary, status, payload_json, tags_json, revision, created_at, updated_at FROM planning_nodes WHERE id = ?1",
                    params![explicit_id],
                    row_to_graph_node,
                )
                .optional()?;

            if let Some(node) = existing {
                // Cross-scope guard: reject if the existing node is in a different scope
                if node.scope_key != scope_key {
                    return Err(PlanningStoreError::InvalidInput(format!(
                        "CROSS_SCOPE_CONFLICT: node `{}` exists in scope `{}`, not `{}`",
                        explicit_id, node.scope_key, scope_key
                    )));
                }
                // Update existing
                let title = input.title.trim().to_string();
                let summary = input.summary.trim().to_string();
                let status = input.status.trim().to_string();
                let tags = normalize_string_list(input.tags);
                let new_revision = node.revision + 1;

                transaction.execute(
                    "UPDATE planning_nodes SET title = ?1, summary = ?2, status = ?3, payload_json = ?4, tags_json = ?5, revision = ?6, updated_at = ?7 WHERE id = ?8",
                    params![
                        title,
                        summary,
                        status,
                        to_json_text(&input.payload)?,
                        to_json_text(&tags)?,
                        new_revision,
                        now,
                        explicit_id,
                    ],
                )?;

                (explicit_id.clone(), false)
            } else {
                // Insert new
                let record = PlanningGraphNode {
                    id: explicit_id.clone(),
                    scope_key: scope_key.clone(),
                    kind: input.kind,
                    title: input.title.trim().to_string(),
                    summary: input.summary.trim().to_string(),
                    status: input.status.trim().to_string(),
                    payload: input.payload,
                    tags: normalize_string_list(input.tags),
                    revision: 1,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                };

                transaction.execute(
                    r#"
                INSERT INTO planning_nodes (
                    id, scope_key, kind, title, summary, status,
                    payload_json, tags_json, revision, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                "#,
                    params![
                        record.id,
                        record.scope_key,
                        record.kind.as_str(),
                        record.title,
                        record.summary,
                        record.status,
                        to_json_text(&record.payload)?,
                        to_json_text(&record.tags)?,
                        record.revision,
                        record.created_at,
                        record.updated_at,
                    ],
                )?;

                append_event(
                    &transaction,
                    build_event(
                        &transaction,
                        EntityType::GraphNode,
                        explicit_id,
                        EntityType::GraphNode,
                        explicit_id,
                        &input.correlation_id,
                        input.run_id,
                        "graph-node.created",
                        serde_json::to_value(&record)?,
                    )?,
                )?;

                (explicit_id.clone(), true)
            }
        } else {
            // No explicit ID — always create
            let id = new_id();
            let record = PlanningGraphNode {
                id: id.clone(),
                scope_key: scope_key.clone(),
                kind: input.kind,
                title: input.title.trim().to_string(),
                summary: input.summary.trim().to_string(),
                status: input.status.trim().to_string(),
                payload: input.payload.clone(),
                tags: normalize_string_list(input.tags),
                revision: 1,
                created_at: now.clone(),
                updated_at: now.clone(),
            };

            transaction.execute(
                r#"
            INSERT INTO planning_nodes (
                id, scope_key, kind, title, summary, status,
                payload_json, tags_json, revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
                params![
                    record.id,
                    record.scope_key,
                    record.kind.as_str(),
                    record.title,
                    record.summary,
                    record.status,
                    to_json_text(&record.payload)?,
                    to_json_text(&record.tags)?,
                    record.revision,
                    record.created_at,
                    record.updated_at,
                ],
            )?;

            append_event(
                &transaction,
                build_event(
                    &transaction,
                    EntityType::GraphNode,
                    &id,
                    EntityType::GraphNode,
                    &id,
                    &input.correlation_id,
                    input.run_id,
                    "graph-node.created",
                    serde_json::to_value(&record)?,
                )?,
            )?;

            (id, true)
        };

        // Rebuild tag index
        let tags: Vec<String> = transaction
            .query_row(
                "SELECT tags_json FROM planning_nodes WHERE id = ?1",
                params![&id],
                |row| {
                    let s: String = row.get(0)?;
                    Ok(s)
                },
            )
            .optional()?
            .map(parse_json_column::<Vec<String>>)
            .unwrap_or_else(|| Ok(Vec::new()))
            .map_err(PlanningStoreError::from)?;
        rebuild_tag_index_for_entity(&transaction, EntityType::GraphNode, &id, &tags)?;

        transaction.commit()?;
        Ok((id, created))
    }

    /// Upsert a graph edge: create if not exists, skip if duplicate active.
    pub fn upsert_graph_edge(
        &self,
        input: CreateGraphEdgeInput,
    ) -> Result<(String, bool), PlanningStoreError> {
        require_non_empty("correlationId", &input.correlation_id)?;
        require_non_empty("source_node_id", &input.source_node_id)?;
        require_non_empty("target_node_id", &input.target_node_id)?;
        require_non_empty("status", &input.status)?;
        require_kebab_token("status", &input.status)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let scope_key = normalized_scope_key(input.scope_key);
        ensure_scope_exists(&transaction, &scope_key)?;

        // Preflight: verify source and target nodes exist in scope
        let _source = load_graph_node(&transaction, &input.source_node_id).map_err(|_| {
            PlanningStoreError::InvalidInput(format!(
                "sourceNodeId references missing node `{}`",
                input.source_node_id
            ))
        })?;
        let _target = load_graph_node(&transaction, &input.target_node_id).map_err(|_| {
            PlanningStoreError::InvalidInput(format!(
                "targetNodeId references missing node `{}`",
                input.target_node_id
            ))
        })?;

        let now = now_string()?;

        // If user provides an explicit ID, try to find existing
        let (id, created) = if let Some(ref explicit_id) = input.id {
            require_non_empty("id", explicit_id)?;
            let existing: Option<PlanningGraphEdge> = transaction
                .query_row(
                    "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE id = ?1",
                    params![explicit_id],
                    row_to_graph_edge,
                )
                .optional()?;

            if let Some(edge) = existing {
                if edge.scope_key != scope_key {
                    return Err(PlanningStoreError::InvalidInput(format!(
                        "CROSS_SCOPE_CONFLICT: edge `{}` exists in scope `{}`, not `{}`",
                        explicit_id, edge.scope_key, scope_key
                    )));
                }
                // Update existing
                let status = input.status.trim().to_string();
                let new_revision = edge.revision + 1;
                transaction.execute(
                    "UPDATE planning_edges SET kind = ?1, source_node_id = ?2, target_node_id = ?3, status = ?4, payload_json = ?5, revision = ?6, updated_at = ?7 WHERE id = ?8",
                    params![
                        input.kind.as_str(),
                        input.source_node_id,
                        input.target_node_id,
                        status,
                        to_json_text(&input.payload)?,
                        new_revision,
                        now,
                        explicit_id,
                    ],
                )?;
                (explicit_id.clone(), false)
            } else {
                let record = PlanningGraphEdge {
                    id: explicit_id.clone(),
                    scope_key: scope_key.clone(),
                    kind: input.kind,
                    source_node_id: input.source_node_id.clone(),
                    target_node_id: input.target_node_id.clone(),
                    status: input.status.trim().to_string(),
                    payload: input.payload,
                    revision: 1,
                    created_at: now.clone(),
                    updated_at: now,
                };
                transaction.execute(
                    r#"INSERT INTO planning_edges (id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
                    params![
                        record.id, record.scope_key, record.kind.as_str(),
                        record.source_node_id, record.target_node_id,
                        record.status, to_json_text(&record.payload)?,
                        record.revision, record.created_at, record.updated_at,
                    ],
                )?;
                (explicit_id.clone(), true)
            }
        } else {
            // Check for existing active edge with same (source, target, kind)
            let existing_id: Option<String> = transaction
                .query_row(
                    "SELECT id FROM planning_edges WHERE scope_key = ?1 AND kind = ?2 AND source_node_id = ?3 AND target_node_id = ?4 AND status = 'active'",
                    params![scope_key, input.kind.as_str(), input.source_node_id, input.target_node_id],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(edge_id) = existing_id {
                // Already exists — return existing ID, no creation
                (edge_id, false)
            } else {
                let id = new_id();
                let record = PlanningGraphEdge {
                    id: id.clone(),
                    scope_key: scope_key.clone(),
                    kind: input.kind,
                    source_node_id: input.source_node_id.clone(),
                    target_node_id: input.target_node_id.clone(),
                    status: input.status.trim().to_string(),
                    payload: input.payload,
                    revision: 1,
                    created_at: now.clone(),
                    updated_at: now,
                };
                transaction.execute(
                    r#"INSERT INTO planning_edges (id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
                    params![
                        record.id, record.scope_key, record.kind.as_str(),
                        record.source_node_id, record.target_node_id,
                        record.status, to_json_text(&record.payload)?,
                        record.revision, record.created_at, record.updated_at,
                    ],
                )?;
                (id, true)
            }
        };

        transaction.commit()?;
        Ok((id, created))
    }

    /// Load all graph nodes in a scope for diff/comparison.
    pub fn load_all_graph_nodes(
        &self,
        scope_key: &str,
    ) -> Result<Vec<PlanningGraphNode>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        let mut stmt = connection.prepare(
            "SELECT id, scope_key, kind, title, summary, status, payload_json, tags_json, revision, created_at, updated_at FROM planning_nodes WHERE scope_key = ?1 ORDER BY id ASC"
        )?;
        let rows = stmt.query_map(params![normalized], row_to_graph_node)?;
        collect_rows(rows)
    }

    /// Load all graph edges in a scope for diff/comparison.
    pub fn load_all_graph_edges(
        &self,
        scope_key: &str,
    ) -> Result<Vec<PlanningGraphEdge>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);
        let mut stmt = connection.prepare(
            "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE scope_key = ?1 ORDER BY id ASC"
        )?;
        let rows = stmt.query_map(params![normalized], row_to_graph_edge)?;
        collect_rows(rows)
    }

    // ─── Graph Runnable ────────────────────────────────────────────────────

    /// Find runnable work nodes in the graph by traversing depends-on and blocks edges.
    pub fn find_runnable_graph_work(
        &self,
        scope_key: &str,
    ) -> Result<crate::GraphRunnableResult, PlanningStoreError> {
        use crate::{BlockedGraphCandidate, GraphRunnableCandidate};

        let connection = self.open_connection()?;
        let normalized = normalize_scope_key_value(scope_key);

        // Load all work nodes in scope with runnable statuses
        let mut stmt = connection.prepare(
            "SELECT id, scope_key, kind, title, summary, status, payload_json, tags_json, revision, created_at, updated_at FROM planning_nodes WHERE scope_key = ?1 AND kind = 'work' AND status IN ('proposed', 'active', 'draft') ORDER BY title ASC"
        )?;
        let work_nodes: Vec<PlanningGraphNode> = stmt
            .query_map(params![&normalized], row_to_graph_node)?
            .collect::<Result<Vec<_>, _>>()?;

        let mut candidates = Vec::new();
        let mut blocked_list = Vec::new();

        for node in &work_nodes {
            // Load outgoing depends-on edges — check if targets are completed
            let outgoing_deps: Vec<PlanningGraphEdge> = list_outgoing_edges_in_scope(
                &connection,
                &node.id,
                &normalized,
                Some(PlanningEdgeKind::DependsOn),
            )?;

            let mut incomplete_deps = Vec::new();
            for dep_edge in &outgoing_deps {
                if dep_edge.status != "active" {
                    continue;
                }
                let target = load_graph_node(&connection, &dep_edge.target_node_id)?;
                if !matches!(target.status.as_str(), "completed" | "validated") {
                    incomplete_deps.push(target.id.clone());
                }
            }

            // Load incoming blocks edges — check if any are active
            let incoming_blocks: Vec<PlanningGraphEdge> = list_incoming_edges_in_scope(
                &connection,
                &node.id,
                &normalized,
                Some(PlanningEdgeKind::Blocks),
            )?;

            let mut active_blockers = Vec::new();
            for block_edge in &incoming_blocks {
                if block_edge.status != "active" {
                    continue;
                }
                let blocker = load_graph_node(&connection, &block_edge.source_node_id)?;
                if !matches!(
                    blocker.status.as_str(),
                    "completed" | "cancelled" | "invalidated" | "archived"
                ) {
                    active_blockers.push(blocker.id.clone());
                }
            }

            if !active_blockers.is_empty() {
                blocked_list.push(BlockedGraphCandidate {
                    node_id: node.id.clone(),
                    title: node.title.clone(),
                    reason: format!("blocked_by:{}", active_blockers.join(",")),
                    blocker_ids: active_blockers,
                });
            } else if incomplete_deps.is_empty() {
                candidates.push(GraphRunnableCandidate {
                    node_id: node.id.clone(),
                    title: node.title.clone(),
                    status: node.status.clone(),
                    reason: "ready".to_string(),
                    incomplete_dependencies: Vec::new(),
                    active_blockers: Vec::new(),
                });
            } else {
                // Not runnable — has incomplete deps but no active blockers
                blocked_list.push(BlockedGraphCandidate {
                    node_id: node.id.clone(),
                    title: node.title.clone(),
                    reason: format!("waiting_on:{}", incomplete_deps.join(",")),
                    blocker_ids: Vec::new(),
                });
            }
        }

        // Sort candidates: work nodes with status=active first, then proposed, then draft
        candidates.sort_by(|a, b| {
            let sa = match a.status.as_str() {
                "active" => 0,
                "proposed" => 1,
                _ => 2,
            };
            let sb = match b.status.as_str() {
                "active" => 0,
                "proposed" => 1,
                _ => 2,
            };
            sa.cmp(&sb).then_with(|| a.title.cmp(&b.title))
        });

        Ok(crate::GraphRunnableResult {
            candidates,
            blocked: blocked_list,
        })
    }

    // ─── Bulk Transition ────────────────────────────────────────────────────

    /// Bulk update status for graph nodes matching IDs or a filter.
    pub fn bulk_update_graph_node_status(
        &self,
        input: &crate::BulkTransitionInput,
    ) -> Result<crate::BulkTransitionResult, PlanningStoreError> {
        use crate::{BulkTransitionRejection, BulkTransitionResult};

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let normalized = normalize_scope_key_value(&input.scope_key);
        let now = now_string()?;

        // Resolve node IDs
        let node_ids: Vec<String> = if let Some(ref ids) = input.node_ids {
            ids.clone()
        } else if let Some(ref filter) = input.filter {
            let (where_clause, filter_params) = parse_graph_node_filter(filter, &normalized);
            let sql = format!(
                "SELECT id FROM planning_nodes WHERE scope_key = ?1 {where_clause} ORDER BY id ASC"
            );
            let mut stmt = transaction.prepare(&sql)?;
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            params.push(Box::new(normalized.clone()));
            for p in filter_params {
                params.push(Box::new(p));
            }
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            let rows = stmt.query_map(param_refs.as_slice(), |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        } else {
            return Err(PlanningStoreError::InvalidInput(
                "either --node-ids or --filter is required".to_string(),
            ));
        };

        if node_ids.is_empty() {
            return Ok(BulkTransitionResult {
                transitioned: Vec::new(),
                rejected: Vec::new(),
                total_matched: 0,
                total_transitioned: 0,
            });
        }

        let total_matched = node_ids.len();
        let mut transitioned = Vec::new();
        let mut rejected = Vec::new();

        // Validate all transitions first
        for node_id in &node_ids {
            let node = match load_graph_node(&transaction, node_id) {
                Ok(n) => n,
                Err(_) => {
                    rejected.push(BulkTransitionRejection {
                        node_id: node_id.clone(),
                        reason: "node not found".to_string(),
                    });
                    continue;
                }
            };
            if node.scope_key != normalized {
                rejected.push(BulkTransitionRejection {
                    node_id: node_id.clone(),
                    reason: format!("node in scope `{}`, not `{}`", node.scope_key, normalized),
                });
                continue;
            }
            // Accept any status transition (no lifecycle enforcement in bulk mode)
        }

        // Commit all valid transitions
        for node_id in &node_ids {
            if rejected.iter().any(|r| &r.node_id == node_id) {
                continue;
            }
            let new_revision: i64 = transaction.query_row(
                "SELECT revision + 1 FROM planning_nodes WHERE id = ?1",
                params![node_id],
                |row| row.get(0),
            )?;
            transaction.execute(
                "UPDATE planning_nodes SET status = ?1, revision = ?2, updated_at = ?3 WHERE id = ?4",
                params![&input.status, new_revision, &now, node_id],
            )?;
            transitioned.push(node_id.clone());
        }

        transaction.commit()?;
        Ok(BulkTransitionResult {
            total_matched,
            total_transitioned: transitioned.len(),
            transitioned,
            rejected,
        })
    }

    /// Apply a full parsed manifest in a single transaction.
    /// Returns counts of created/revised/unchanged entities.
    pub fn apply_manifest(
        &self,
        parsed: &crate::manifest::ParsedManifest,
        dry_run: bool,
    ) -> Result<crate::ManifestApplyResult, PlanningStoreError> {
        use crate::{ManifestApplyResult, ManifestConflict};

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let scope_key = normalize_scope_key_value(&parsed.scope);
        ensure_scope_exists(&transaction, &scope_key)?;

        let mut created_nodes = Vec::new();
        let mut revised_nodes = Vec::new();
        let mut unchanged_nodes = Vec::new();
        let mut created_edges = Vec::new();
        let mut unchanged_edges = Vec::new();
        let mut conflicts = Vec::new();

        // Phase 1: Upsert all nodes
        for node_input in &parsed.nodes {
            match self.upsert_graph_node_in_tx(&transaction, node_input, &scope_key) {
                Ok((id, true)) => created_nodes.push(id),
                Ok((id, false)) => {
                    // Check if anything actually changed
                    let existing = load_graph_node(&transaction, &id)?;
                    if existing.title == node_input.title.trim()
                        && existing.summary == node_input.summary.trim()
                        && existing.status == node_input.status.trim()
                    {
                        unchanged_nodes.push(id);
                    } else {
                        revised_nodes.push(id);
                    }
                }
                Err(e) => {
                    conflicts.push(ManifestConflict {
                        entity_type: "node".to_string(),
                        entity_id: node_input.id.clone().unwrap_or_else(|| "auto".to_string()),
                        reason: e.to_string(),
                    });
                }
            }
        }

        // Phase 2: Upsert all edges
        for edge_input in &parsed.edges {
            match self.upsert_graph_edge_in_tx(&transaction, edge_input, &scope_key) {
                Ok((id, true)) => created_edges.push(id),
                Ok((id, false)) => unchanged_edges.push(id),
                Err(e) => {
                    conflicts.push(ManifestConflict {
                        entity_type: "edge".to_string(),
                        entity_id: edge_input.id.clone().unwrap_or_else(|| "auto".to_string()),
                        reason: e.to_string(),
                    });
                }
            }
        }

        if dry_run {
            transaction.rollback()?;
        } else {
            transaction.commit()?;
        }

        Ok(ManifestApplyResult {
            total_nodes: parsed.nodes.len(),
            total_edges: parsed.edges.len(),
            created_nodes,
            revised_nodes,
            unchanged_nodes,
            created_edges,
            revised_edges: Vec::new(),
            unchanged_edges,
            conflicts,
            validation: None,
        })
    }

    /// Internal: upsert a graph node within an existing transaction.
    fn upsert_graph_node_in_tx(
        &self,
        transaction: &Transaction<'_>,
        input: &CreateGraphNodeInput,
        scope_key: &str,
    ) -> Result<(String, bool), PlanningStoreError> {
        let now = now_string()?;

        if let Some(ref explicit_id) = input.id {
            let existing: Option<PlanningGraphNode> = transaction
                .query_row(
                    "SELECT id, scope_key, kind, title, summary, status, payload_json, tags_json, revision, created_at, updated_at FROM planning_nodes WHERE id = ?1",
                    params![explicit_id],
                    row_to_graph_node,
                )
                .optional()?;

            if let Some(node) = existing {
                if node.scope_key != *scope_key {
                    return Err(PlanningStoreError::InvalidInput(format!(
                        "CROSS_SCOPE_CONFLICT: node `{}` exists in scope `{}`, not `{}`",
                        explicit_id, node.scope_key, scope_key
                    )));
                }
                let new_revision = node.revision + 1;
                transaction.execute(
                    "UPDATE planning_nodes SET title = ?1, summary = ?2, status = ?3, payload_json = ?4, tags_json = ?5, revision = ?6, updated_at = ?7 WHERE id = ?8",
                    params![
                        input.title.trim(),
                        input.summary.trim(),
                        input.status.trim(),
                        to_json_text(&input.payload)?,
                        to_json_text(&normalize_string_list(input.tags.clone()))?,
                        new_revision,
                        now,
                        explicit_id,
                    ],
                )?;
                return Ok((explicit_id.clone(), false));
            }
        }

        // Create new
        let id = input.id.clone().unwrap_or_else(new_id);
        let record = PlanningGraphNode {
            id: id.clone(),
            scope_key: scope_key.to_string(),
            kind: input.kind,
            title: input.title.trim().to_string(),
            summary: input.summary.trim().to_string(),
            status: input.status.trim().to_string(),
            payload: input.payload.clone(),
            tags: normalize_string_list(input.tags.clone()),
            revision: 1,
            created_at: now.clone(),
            updated_at: now,
        };
        transaction.execute(
            r#"INSERT INTO planning_nodes (id, scope_key, kind, title, summary, status, payload_json, tags_json, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
            params![
                record.id, record.scope_key, record.kind.as_str(),
                record.title, record.summary, record.status,
                to_json_text(&record.payload)?, to_json_text(&record.tags)?,
                record.revision, record.created_at, record.updated_at,
            ],
        )?;
        Ok((id, true))
    }

    /// Internal: upsert a graph edge within an existing transaction.
    fn upsert_graph_edge_in_tx(
        &self,
        transaction: &Transaction<'_>,
        input: &CreateGraphEdgeInput,
        scope_key: &str,
    ) -> Result<(String, bool), PlanningStoreError> {
        // Preflight checks (same invariants as create_graph_edge)
        require_non_empty("source_node_id", &input.source_node_id)?;
        require_non_empty("target_node_id", &input.target_node_id)?;
        require_kebab_token("status", &input.status)?;

        if input.source_node_id == input.target_node_id {
            return Err(PlanningStoreError::InvalidInput(format!(
                "self-referential {} edge is not allowed: source and target must be different nodes",
                input.kind.as_str(),
            )));
        }

        let source = load_graph_node(transaction, &input.source_node_id).map_err(|_| {
            PlanningStoreError::InvalidInput(format!(
                "sourceNodeId references missing node `{}`",
                input.source_node_id
            ))
        })?;
        let target = load_graph_node(transaction, &input.target_node_id).map_err(|_| {
            PlanningStoreError::InvalidInput(format!(
                "targetNodeId references missing node `{}`",
                input.target_node_id
            ))
        })?;

        if source.scope_key != *scope_key || target.scope_key != *scope_key {
            return Err(PlanningStoreError::InvalidInput(
                "source and target nodes must belong to the same scope as the edge".to_string(),
            ));
        }

        validate_edge_kind_pair(&input.kind, &source.kind, &target.kind)?;

        if let Some(ref explicit_id) = input.id {
            let existing: Option<PlanningGraphEdge> = transaction
                .query_row(
                    "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE id = ?1",
                    params![explicit_id],
                    row_to_graph_edge,
                )
                .optional()?;

            if let Some(edge) = existing {
                if edge.scope_key != *scope_key {
                    return Err(PlanningStoreError::InvalidInput(format!(
                        "CROSS_SCOPE_CONFLICT: edge `{}` exists in scope `{}`, not `{}`",
                        explicit_id, edge.scope_key, scope_key
                    )));
                }
                let new_revision = edge.revision + 1;
                let now = now_string()?;
                transaction.execute(
                    "UPDATE planning_edges SET kind = ?1, source_node_id = ?2, target_node_id = ?3, status = ?4, payload_json = ?5, revision = ?6, updated_at = ?7 WHERE id = ?8",
                    params![
                        input.kind.as_str(), input.source_node_id, input.target_node_id,
                        input.status.trim(), to_json_text(&input.payload)?,
                        new_revision, now, explicit_id,
                    ],
                )?;
                return Ok((explicit_id.clone(), false));
            }
            // Edge doesn't exist by ID, but check for a matching tuple edge (same kind, source, target)
            let existing_id: Option<String> = transaction
                .query_row(
                    "SELECT id FROM planning_edges WHERE scope_key = ?1 AND kind = ?2 AND source_node_id = ?3 AND target_node_id = ?4 AND status = 'active'",
                    params![scope_key, input.kind.as_str(), input.source_node_id, input.target_node_id],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(edge_id) = existing_id {
                return Ok((edge_id, false));
            }
        } else {
            let existing_id: Option<String> = transaction
                .query_row(
                    "SELECT id FROM planning_edges WHERE scope_key = ?1 AND kind = ?2 AND source_node_id = ?3 AND target_node_id = ?4 AND status = 'active'",
                    params![scope_key, input.kind.as_str(), input.source_node_id, input.target_node_id],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(edge_id) = existing_id {
                return Ok((edge_id, false));
            }
        }

        // Create new — check for cycles and active duplicates
        if input.status.trim() == "active" {
            let dup_count: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM planning_edges WHERE scope_key = ?1 AND kind = ?2 AND source_node_id = ?3 AND target_node_id = ?4 AND status = 'active'",
                params![scope_key, input.kind.as_str(), input.source_node_id, input.target_node_id],
                |row| row.get(0),
            )?;
            if dup_count > 0 {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "duplicate active {} edge from `{}` to `{}` in scope `{}`",
                    input.kind.as_str(),
                    input.source_node_id,
                    input.target_node_id,
                    scope_key
                )));
            }
            match input.kind {
                PlanningEdgeKind::DecomposesTo | PlanningEdgeKind::DependsOn
                    if would_create_graph_cycle(
                        transaction,
                        &input.source_node_id,
                        &input.target_node_id,
                        &input.kind,
                    )? =>
                {
                    return Err(PlanningStoreError::InvalidInput(format!(
                        "adding this {} edge from `{}` to `{}` would create a cycle",
                        input.kind.as_str(),
                        input.source_node_id,
                        input.target_node_id
                    )));
                }
                _ => {}
            }
        }

        let id = input.id.clone().unwrap_or_else(new_id);
        let now = now_string()?;
        let record = PlanningGraphEdge {
            id: id.clone(),
            scope_key: scope_key.to_string(),
            kind: input.kind,
            source_node_id: input.source_node_id.clone(),
            target_node_id: input.target_node_id.clone(),
            status: input.status.trim().to_string(),
            payload: input.payload.clone(),
            revision: 1,
            created_at: now.clone(),
            updated_at: now,
        };
        transaction.execute(
            r#"INSERT INTO planning_edges (id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
            params![
                record.id, record.scope_key, record.kind.as_str(),
                record.source_node_id, record.target_node_id,
                record.status, to_json_text(&record.payload)?,
                record.revision, record.created_at, record.updated_at,
            ],
        )?;
        Ok((id, true))
    }
}

fn create_schema(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS planning_config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS scopes (
            scope_key TEXT PRIMARY KEY,
            scope_type TEXT,
            parent_scope_key TEXT REFERENCES scopes(scope_key) ON DELETE SET NULL,
            metadata_json TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            revision INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_scopes_parent ON scopes(parent_scope_key);

        CREATE TABLE IF NOT EXISTS goals (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            correlation_id TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT NOT NULL,
            acceptance_criteria_json TEXT NOT NULL,
            rejection_criteria_json TEXT NOT NULL,
            status TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            revision INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_goals_correlation ON goals(correlation_id);
        CREATE INDEX IF NOT EXISTS idx_goals_status ON goals(status);

        CREATE TABLE IF NOT EXISTS roadmaps (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            goal_id TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
            correlation_id TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            status TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            revision INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_roadmaps_goal ON roadmaps(goal_id);
        CREATE INDEX IF NOT EXISTS idx_roadmaps_correlation ON roadmaps(correlation_id);

        CREATE TABLE IF NOT EXISTS roadmap_sections (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            roadmap_id TEXT NOT NULL REFERENCES roadmaps(id) ON DELETE CASCADE,
            slug TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            ordering_index INTEGER NOT NULL,
            revision INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(roadmap_id, slug)
        );
        CREATE INDEX IF NOT EXISTS idx_roadmap_sections_roadmap ON roadmap_sections(roadmap_id, ordering_index);

        CREATE TABLE IF NOT EXISTS work_points (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            roadmap_id TEXT NOT NULL REFERENCES roadmaps(id) ON DELETE CASCADE,
            section_id TEXT REFERENCES roadmap_sections(id) ON DELETE SET NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            status TEXT NOT NULL,
            ordering_index INTEGER NOT NULL,
            dependency_ids_json TEXT NOT NULL,
            validation_expectations_json TEXT NOT NULL,
            effort_tier TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            revision INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_work_points_roadmap ON work_points(roadmap_id, ordering_index);

        CREATE TABLE IF NOT EXISTS plans (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            goal_id TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
            roadmap_id TEXT NOT NULL REFERENCES roadmaps(id) ON DELETE CASCADE,
            correlation_id TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            scope TEXT NOT NULL,
            assumptions_json TEXT NOT NULL,
            stop_conditions_json TEXT NOT NULL,
            validation_steps_json TEXT NOT NULL,
            targeted_work_point_ids_json TEXT NOT NULL,
            effort_tier TEXT NOT NULL,
            routing_hint TEXT,
            allow_parallel_overlap INTEGER NOT NULL,
            status TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            revision INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_plans_roadmap ON plans(roadmap_id);
        CREATE INDEX IF NOT EXISTS idx_plans_goal ON plans(goal_id);
        CREATE INDEX IF NOT EXISTS idx_plans_correlation ON plans(correlation_id);

        CREATE TABLE IF NOT EXISTS todos (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            plan_id TEXT REFERENCES plans(id) ON DELETE CASCADE,
            work_point_id TEXT REFERENCES work_points(id) ON DELETE SET NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            status TEXT NOT NULL,
            priority TEXT NOT NULL,
            effort_tier TEXT NOT NULL,
            evidence_refs_json TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            ordering_index INTEGER NOT NULL,
            revision INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_todos_plan ON todos(plan_id, ordering_index);
        CREATE INDEX IF NOT EXISTS idx_todos_work_point ON todos(work_point_id, ordering_index);
        CREATE INDEX IF NOT EXISTS idx_todos_status ON todos(status);

        CREATE TABLE IF NOT EXISTS issues (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            correlation_id TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            status TEXT NOT NULL,
            severity TEXT NOT NULL,
            related_entity_type TEXT,
            related_entity_id TEXT,
            tags_json TEXT NOT NULL,
            revision INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(status);
        CREATE INDEX IF NOT EXISTS idx_issues_related ON issues(related_entity_type, related_entity_id);

        CREATE TABLE IF NOT EXISTS review_points (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            attached_entity_type TEXT NOT NULL,
            attached_entity_id TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            status TEXT NOT NULL,
            severity TEXT NOT NULL,
            revision INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_review_points_entity ON review_points(attached_entity_type, attached_entity_id);

        CREATE TABLE IF NOT EXISTS planning_events (
            event_id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            aggregate_type TEXT NOT NULL,
            aggregate_id TEXT NOT NULL,
            correlation_id TEXT NOT NULL,
            causation_id TEXT,
            run_id TEXT NOT NULL,
            stream_id TEXT NOT NULL,
            sequence INTEGER NOT NULL,
            parent_event_id TEXT,
            event_type TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            payload_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_planning_events_entity ON planning_events(entity_type, entity_id, sequence);
        CREATE INDEX IF NOT EXISTS idx_planning_events_correlation ON planning_events(correlation_id, sequence);

        CREATE TABLE IF NOT EXISTS validation_findings (
            finding_id TEXT PRIMARY KEY,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            severity TEXT NOT NULL,
            code TEXT NOT NULL,
            message TEXT NOT NULL,
            scope_key TEXT NOT NULL DEFAULT '',
            fingerprint TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_validation_findings_entity ON validation_findings(entity_type, entity_id);

        CREATE TABLE IF NOT EXISTS entity_file_scopes (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            owner_entity_type TEXT NOT NULL,
            owner_entity_id TEXT NOT NULL,
            selector_type TEXT NOT NULL,
            selector TEXT NOT NULL,
            intent TEXT NOT NULL,
            ordering_index INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(owner_entity_type, owner_entity_id, selector_type, selector, intent)
        );
        CREATE INDEX IF NOT EXISTS idx_entity_file_scopes_owner ON entity_file_scopes(owner_entity_type, owner_entity_id, ordering_index);
        CREATE INDEX IF NOT EXISTS idx_entity_file_scopes_scope ON entity_file_scopes(scope_key, selector_type, selector);

        CREATE TABLE IF NOT EXISTS insights (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            correlation_id TEXT NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            insight_type TEXT NOT NULL,
            parent_entity_type TEXT NOT NULL,
            parent_entity_id TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            status TEXT NOT NULL,
            revision INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_insights_scope ON insights(scope_key);
        CREATE INDEX IF NOT EXISTS idx_insights_parent ON insights(parent_entity_type, parent_entity_id);
        CREATE INDEX IF NOT EXISTS idx_insights_correlation ON insights(correlation_id);
        CREATE INDEX IF NOT EXISTS idx_insights_status ON insights(status);

        CREATE TABLE IF NOT EXISTS project_runs (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            goal_id TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
            roadmap_id TEXT NOT NULL REFERENCES roadmaps(id) ON DELETE CASCADE,
            work_point_id TEXT NOT NULL REFERENCES work_points(id) ON DELETE CASCADE,
            repo_id TEXT,
            branch TEXT,
            worktree_id TEXT,
            session_id TEXT,
            run_id TEXT,
            profile_id TEXT,
            owner_id TEXT NOT NULL,
            idempotency_key TEXT,
            fencing_token INTEGER NOT NULL,
            lease_expires_at TEXT NOT NULL,
            heartbeat_at TEXT NOT NULL,
            status TEXT NOT NULL,
            evidence_json TEXT NOT NULL,
            revision INTEGER NOT NULL,
            claimed_at TEXT,
            completed_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_project_runs_work_point ON project_runs(work_point_id, status);
        CREATE INDEX IF NOT EXISTS idx_project_runs_roadmap ON project_runs(roadmap_id, status);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_project_runs_idempotency ON project_runs(scope_key, idempotency_key) WHERE idempotency_key IS NOT NULL;

        CREATE TABLE IF NOT EXISTS planning_nodes (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL REFERENCES scopes(scope_key) ON DELETE CASCADE,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            status TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            revision INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_planning_nodes_scope_kind ON planning_nodes(scope_key, kind);

        CREATE TABLE IF NOT EXISTS planning_edges (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL REFERENCES scopes(scope_key) ON DELETE CASCADE,
            kind TEXT NOT NULL,
            source_node_id TEXT NOT NULL REFERENCES planning_nodes(id) ON DELETE CASCADE,
            target_node_id TEXT NOT NULL REFERENCES planning_nodes(id) ON DELETE CASCADE,
            status TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            revision INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_planning_edges_scope_kind ON planning_edges(scope_key, kind);
        CREATE INDEX IF NOT EXISTS idx_planning_edges_source ON planning_edges(source_node_id, kind);
        CREATE INDEX IF NOT EXISTS idx_planning_edges_target ON planning_edges(target_node_id, kind);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_planning_edges_unique_active ON planning_edges(scope_key, kind, source_node_id, target_node_id) WHERE status = 'active';

        CREATE TABLE IF NOT EXISTS tag_index (
            scope_key TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            tag TEXT NOT NULL,
            PRIMARY KEY (scope_key, entity_type, entity_id, tag)
        );
        CREATE INDEX IF NOT EXISTS idx_tag_index_lookup ON tag_index(scope_key, tag);
        CREATE INDEX IF NOT EXISTS idx_tag_index_entity ON tag_index(entity_type, entity_id);
        "#,
    )?;

    connection.execute_batch(
        r#"
        CREATE VIRTUAL TABLE IF NOT EXISTS entities_fts USING fts5(entity_id UNINDEXED, title, content, tokenize='porter');
        CREATE VIRTUAL TABLE IF NOT EXISTS insights_fts USING fts5(entity_id UNINDEXED, title, content, tokenize='porter');
        "#,
    )?;

    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS discovery_nodes (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            correlation_id TEXT NOT NULL,
            classification TEXT NOT NULL,
            verification_state TEXT NOT NULL,
            severity TEXT NOT NULL,
            status TEXT NOT NULL,
            claim TEXT NOT NULL,
            impact TEXT,
            next_action TEXT,
            verification_step TEXT,
            recurrence_key TEXT,
            fingerprint TEXT,
            observed_at_json TEXT NOT NULL DEFAULT '[]',
            occurrence_count INTEGER NOT NULL DEFAULT 1,
            source_lineage_json TEXT NOT NULL DEFAULT '[]',
            review_date TEXT,
            resolved_at TEXT,
            resolution_rationale TEXT,
            promoted_entity_type TEXT,
            promoted_entity_id TEXT,
            tags_json TEXT NOT NULL DEFAULT '[]',
            revision INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_discovery_nodes_status ON discovery_nodes(status);
        CREATE INDEX IF NOT EXISTS idx_discovery_nodes_classification ON discovery_nodes(classification);
        CREATE INDEX IF NOT EXISTS idx_discovery_nodes_recurrence ON discovery_nodes(recurrence_key);
        CREATE INDEX IF NOT EXISTS idx_discovery_nodes_fingerprint ON discovery_nodes(fingerprint);
        CREATE INDEX IF NOT EXISTS idx_discovery_nodes_severity ON discovery_nodes(severity);
        CREATE INDEX IF NOT EXISTS idx_discovery_nodes_scope ON discovery_nodes(scope_key);

        CREATE TABLE IF NOT EXISTS discovery_relationships (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            relationship_kind TEXT NOT NULL,
            metadata_json TEXT,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_discovery_rel_source ON discovery_relationships(source_id);
        CREATE INDEX IF NOT EXISTS idx_discovery_rel_target ON discovery_relationships(target_id);
        CREATE INDEX IF NOT EXISTS idx_discovery_rel_kind ON discovery_relationships(relationship_kind);

        CREATE TABLE IF NOT EXISTS discovery_checkpoints (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            run_id TEXT NOT NULL,
            event TEXT NOT NULL,
            snapshot_json TEXT,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_discovery_checkpoints_run ON discovery_checkpoints(run_id);
        "#,
    )?;

    Ok(())
}

fn ensure_schema_version(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    connection.execute(
        "INSERT OR IGNORE INTO planning_config (key, value) VALUES (?1, ?2)",
        params![SCHEMA_VERSION_KEY, "1"],
    )?;
    let version: Option<String> = connection
        .query_row(
            "SELECT value FROM planning_config WHERE key = ?1",
            params![SCHEMA_VERSION_KEY],
            |row| row.get(0),
        )
        .optional()?;
    match version.as_deref() {
        Some(CURRENT_SCHEMA_VERSION) => {
            ensure_default_scope(connection)?;
            create_scope_indexes(connection)?;
            ensure_event_scope_support(connection)?;
            Ok(())
        }
        Some("1") => {
            migrate_v1_to_v2(connection)?;
            migrate_v2_to_v3(connection)?;
            migrate_v3_to_v4(connection)?;
            migrate_v4_to_v5(connection)?;
            migrate_v5_to_v6(connection)?;
            migrate_v6_to_v7(connection)?;
            migrate_v7_to_v8(connection)?;
            migrate_v8_to_v9(connection)?;
            migrate_v9_to_v10(connection)?;
            migrate_v10_to_v11(connection)
        }
        Some("2") => {
            migrate_v2_to_v3(connection)?;
            migrate_v3_to_v4(connection)?;
            migrate_v4_to_v5(connection)?;
            migrate_v5_to_v6(connection)?;
            migrate_v6_to_v7(connection)?;
            migrate_v7_to_v8(connection)?;
            migrate_v8_to_v9(connection)?;
            migrate_v9_to_v10(connection)?;
            migrate_v10_to_v11(connection)
        }
        Some("3") => {
            migrate_v3_to_v4(connection)?;
            migrate_v4_to_v5(connection)?;
            migrate_v5_to_v6(connection)?;
            migrate_v6_to_v7(connection)?;
            migrate_v7_to_v8(connection)?;
            migrate_v8_to_v9(connection)?;
            migrate_v9_to_v10(connection)?;
            migrate_v10_to_v11(connection)
        }
        Some("4") => {
            migrate_v4_to_v5(connection)?;
            migrate_v5_to_v6(connection)?;
            migrate_v6_to_v7(connection)?;
            migrate_v7_to_v8(connection)?;
            migrate_v8_to_v9(connection)?;
            migrate_v9_to_v10(connection)?;
            migrate_v10_to_v11(connection)
        }
        Some("5") => {
            migrate_v5_to_v6(connection)?;
            migrate_v6_to_v7(connection)?;
            migrate_v7_to_v8(connection)?;
            migrate_v8_to_v9(connection)?;
            migrate_v9_to_v10(connection)?;
            migrate_v10_to_v11(connection)
        }
        Some("6") => {
            migrate_v6_to_v7(connection)?;
            migrate_v7_to_v8(connection)?;
            migrate_v8_to_v9(connection)?;
            migrate_v9_to_v10(connection)?;
            migrate_v10_to_v11(connection)
        }
        Some("7") => {
            migrate_v7_to_v8(connection)?;
            migrate_v8_to_v9(connection)?;
            migrate_v9_to_v10(connection)?;
            migrate_v10_to_v11(connection)
        }
        Some("8") => {
            migrate_v8_to_v9(connection)?;
            migrate_v9_to_v10(connection)?;
            migrate_v10_to_v11(connection)
        }
        Some("9") => {
            migrate_v9_to_v10(connection)?;
            migrate_v10_to_v11(connection)
        }
        Some("10") => migrate_v10_to_v11(connection),
        Some(other) => Err(PlanningStoreError::InvalidInput(format!(
            "unsupported planning schema version {other}; expected {CURRENT_SCHEMA_VERSION}"
        ))),
        None => Err(PlanningStoreError::InvalidInput(
            "planning schema version is missing".to_string(),
        )),
    }
}

fn migrate_v1_to_v2(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    ensure_default_scope(connection)?;
    for (table, column) in [
        ("goals", "scope_key"),
        ("roadmaps", "scope_key"),
        ("roadmap_sections", "scope_key"),
        ("work_points", "scope_key"),
        ("plans", "scope_key"),
        ("todos", "scope_key"),
        ("issues", "scope_key"),
        ("review_points", "scope_key"),
    ] {
        if !table_has_column(connection, table, column)? {
            connection.execute(
                &format!(
                    "ALTER TABLE {table} ADD COLUMN {column} TEXT NOT NULL DEFAULT '{DEFAULT_SCOPE_KEY}'"
                ),
                [],
            )?;
        }
        connection.execute(
            &format!(
                "UPDATE {table} SET {column} = '{DEFAULT_SCOPE_KEY}' WHERE {column} IS NULL OR TRIM({column}) = ''"
            ),
            [],
        )?;
    }

    connection.execute(
        "UPDATE planning_config SET value = ?2 WHERE key = ?1",
        params![SCHEMA_VERSION_KEY, "2"],
    )?;
    create_scope_indexes(connection)?;
    Ok(())
}

fn migrate_v2_to_v3(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    ensure_default_scope(connection)?;
    create_scope_indexes(connection)?;
    ensure_event_scope_support(connection)?;
    connection.execute(
        "UPDATE planning_config SET value = '3' WHERE key = ?1",
        params![SCHEMA_VERSION_KEY],
    )?;
    Ok(())
}

fn migrate_v3_to_v4(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    ensure_default_scope(connection)?;
    create_scope_indexes(connection)?;
    ensure_event_scope_support(connection)?;

    if !table_has_column(connection, "work_points", "effort_tier")? {
        connection.execute(
            "ALTER TABLE work_points ADD COLUMN effort_tier TEXT NOT NULL DEFAULT 'balanced'",
            [],
        )?;
    }
    if !table_has_column(connection, "plans", "effort_tier")? {
        connection.execute(
            "ALTER TABLE plans ADD COLUMN effort_tier TEXT NOT NULL DEFAULT 'balanced'",
            [],
        )?;
    }
    if !table_has_column(connection, "plans", "routing_hint")? {
        connection.execute("ALTER TABLE plans ADD COLUMN routing_hint TEXT", [])?;
    }
    if !table_has_column(connection, "plans", "allow_parallel_overlap")? {
        connection.execute(
            "ALTER TABLE plans ADD COLUMN allow_parallel_overlap INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    if !table_has_column(connection, "todos", "effort_tier")? {
        connection.execute(
            "ALTER TABLE todos ADD COLUMN effort_tier TEXT NOT NULL DEFAULT 'balanced'",
            [],
        )?;
    }

    connection.execute_batch(
        r#"
        UPDATE work_points SET effort_tier = 'balanced' WHERE effort_tier IS NULL OR TRIM(effort_tier) = '';
        UPDATE plans SET effort_tier = 'balanced' WHERE effort_tier IS NULL OR TRIM(effort_tier) = '';
        UPDATE plans SET allow_parallel_overlap = 0 WHERE allow_parallel_overlap IS NULL;
        UPDATE todos SET effort_tier = 'balanced' WHERE effort_tier IS NULL OR TRIM(effort_tier) = '';
        "#,
    )?;

    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS entity_file_scopes (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            owner_entity_type TEXT NOT NULL,
            owner_entity_id TEXT NOT NULL,
            selector_type TEXT NOT NULL,
            selector TEXT NOT NULL,
            intent TEXT NOT NULL,
            ordering_index INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(owner_entity_type, owner_entity_id, selector_type, selector, intent)
        );
        CREATE INDEX IF NOT EXISTS idx_entity_file_scopes_owner ON entity_file_scopes(owner_entity_type, owner_entity_id, ordering_index);
        CREATE INDEX IF NOT EXISTS idx_entity_file_scopes_scope ON entity_file_scopes(scope_key, selector_type, selector);
        "#,
    )?;

    connection.execute(
        "UPDATE planning_config SET value = '4' WHERE key = ?1",
        params![SCHEMA_VERSION_KEY],
    )?;
    Ok(())
}

fn migrate_v4_to_v5(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    ensure_default_scope(connection)?;
    create_scope_indexes(connection)?;
    ensure_event_scope_support(connection)?;

    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS insights (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            correlation_id TEXT NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            insight_type TEXT NOT NULL,
            parent_entity_type TEXT NOT NULL,
            parent_entity_id TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            status TEXT NOT NULL,
            revision INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_insights_scope ON insights(scope_key);
        CREATE INDEX IF NOT EXISTS idx_insights_parent ON insights(parent_entity_type, parent_entity_id);
        CREATE INDEX IF NOT EXISTS idx_insights_correlation ON insights(correlation_id);
        CREATE INDEX IF NOT EXISTS idx_insights_status ON insights(status);

        CREATE TABLE IF NOT EXISTS tag_index (
            scope_key TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            tag TEXT NOT NULL,
            PRIMARY KEY (scope_key, entity_type, entity_id, tag)
        );
        CREATE INDEX IF NOT EXISTS idx_tag_index_lookup ON tag_index(scope_key, tag);
        CREATE INDEX IF NOT EXISTS idx_tag_index_entity ON tag_index(entity_type, entity_id);
        "#,
    )?;

    connection.execute_batch(
        r#"
        CREATE VIRTUAL TABLE IF NOT EXISTS entities_fts USING fts5(entity_id UNINDEXED, title, content, tokenize='porter');
        CREATE VIRTUAL TABLE IF NOT EXISTS insights_fts USING fts5(entity_id UNINDEXED, title, content, tokenize='porter');
        "#,
    )?;

    rebuild_all_tag_indexes(connection)?;

    for (table, id_column, title_column, content_column) in [
        ("goals", "id", "title", "description"),
        ("roadmaps", "id", "title", "summary"),
        ("work_points", "id", "title", "summary"),
        ("plans", "id", "title", "summary"),
        ("todos", "id", "title", "summary"),
        ("issues", "id", "title", "summary"),
    ] {
        let mut statement = connection.prepare(&format!(
            "SELECT {id_column}, {title_column}, {content_column} FROM {table}"
        ))?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (id, title, content) = row?;
            connection.execute(
                "INSERT INTO entities_fts (entity_id, title, content) VALUES (?1, ?2, ?3)",
                params![id, title, content],
            )?;
        }
    }

    {
        let mut statement = connection.prepare("SELECT id, title, content FROM insights")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (id, title, content) = row?;
            connection.execute(
                "INSERT INTO insights_fts (entity_id, title, content) VALUES (?1, ?2, ?3)",
                params![id, title, content],
            )?;
        }
    }

    connection.execute(
        "UPDATE planning_config SET value = '5' WHERE key = ?1",
        params![SCHEMA_VERSION_KEY],
    )?;
    Ok(())
}

fn migrate_v5_to_v6(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS project_runs (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            goal_id TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
            roadmap_id TEXT NOT NULL REFERENCES roadmaps(id) ON DELETE CASCADE,
            work_point_id TEXT NOT NULL REFERENCES work_points(id) ON DELETE CASCADE,
            repo_id TEXT,
            branch TEXT,
            worktree_id TEXT,
            session_id TEXT,
            run_id TEXT,
            profile_id TEXT,
            status TEXT NOT NULL,
            evidence_json TEXT NOT NULL DEFAULT '{}',
            revision INTEGER NOT NULL DEFAULT 1,
            claimed_at TEXT,
            completed_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_project_runs_work_point ON project_runs(work_point_id, status);
        CREATE INDEX IF NOT EXISTS idx_project_runs_roadmap ON project_runs(roadmap_id, status);
        "#,
    )?;

    connection.execute(
        "UPDATE planning_config SET value = '6' WHERE key = ?1",
        params![SCHEMA_VERSION_KEY],
    )?;
    Ok(())
}

fn migrate_v6_to_v7(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS worktrees (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            repo_uri TEXT,
            branch TEXT,
            worktree_path TEXT,
            project_run_id TEXT REFERENCES project_runs(id) ON DELETE SET NULL,
            session_id TEXT,
            status TEXT NOT NULL DEFAULT 'active',
            revision INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_worktrees_project_run ON worktrees(project_run_id);
        CREATE INDEX IF NOT EXISTS idx_worktrees_session ON worktrees(session_id);
        CREATE INDEX IF NOT EXISTS idx_worktrees_status ON worktrees(status);
        "#,
    )?;

    connection.execute(
        "UPDATE planning_config SET value = '7' WHERE key = ?1",
        params![SCHEMA_VERSION_KEY],
    )?;
    Ok(())
}

fn migrate_v7_to_v8(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    for col in [
        "kind",
        "priority",
        "repairs_work_point_ids",
        "supersedes_work_point_ids",
        "blocks_work_point_ids",
    ] {
        if !table_has_column(connection, "work_points", col)? {
            let default = match col {
                "kind" => "feature",
                "priority" => "medium",
                _ => "[]",
            };
            connection.execute(
                &format!(
                    "ALTER TABLE work_points ADD COLUMN {col} TEXT NOT NULL DEFAULT '{default}'"
                ),
                [],
            )?;
        }
    }

    connection.execute(
        "UPDATE planning_config SET value = '8' WHERE key = ?1",
        params![SCHEMA_VERSION_KEY],
    )?;
    Ok(())
}

fn migrate_v8_to_v9(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS planning_nodes (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL REFERENCES scopes(scope_key) ON DELETE CASCADE,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            status TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            revision INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_planning_nodes_scope_kind ON planning_nodes(scope_key, kind);

        CREATE TABLE IF NOT EXISTS planning_edges (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL REFERENCES scopes(scope_key) ON DELETE CASCADE,
            kind TEXT NOT NULL,
            source_node_id TEXT NOT NULL REFERENCES planning_nodes(id) ON DELETE CASCADE,
            target_node_id TEXT NOT NULL REFERENCES planning_nodes(id) ON DELETE CASCADE,
            status TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            revision INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_planning_edges_scope_kind ON planning_edges(scope_key, kind);
        CREATE INDEX IF NOT EXISTS idx_planning_edges_source ON planning_edges(source_node_id, kind);
        CREATE INDEX IF NOT EXISTS idx_planning_edges_target ON planning_edges(target_node_id, kind);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_planning_edges_unique_active ON planning_edges(scope_key, kind, source_node_id, target_node_id) WHERE status = 'active';
        "#,
    )?;

    connection.execute(
        "UPDATE planning_config SET value = '9' WHERE key = ?1",
        params![SCHEMA_VERSION_KEY],
    )?;
    Ok(())
}

fn migrate_v9_to_v10(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    let now = now_string()?;
    let default_expiry = lease_deadline(&now, DEFAULT_LEASE_SECONDS)?;
    for (column, definition) in [
        ("owner_id", "TEXT NOT NULL DEFAULT 'legacy-owner'"),
        ("idempotency_key", "TEXT"),
        ("fencing_token", "INTEGER NOT NULL DEFAULT 1"),
        ("lease_expires_at", "TEXT NOT NULL DEFAULT ''"),
        ("heartbeat_at", "TEXT NOT NULL DEFAULT ''"),
    ] {
        if !table_has_column(connection, "project_runs", column)? {
            connection.execute(
                &format!("ALTER TABLE project_runs ADD COLUMN {column} {definition}"),
                [],
            )?;
        }
    }
    connection.execute(
        "UPDATE project_runs SET owner_id = COALESCE(NULLIF(session_id, ''), NULLIF(run_id, ''), 'legacy-owner') WHERE owner_id = 'legacy-owner'",
        [],
    )?;
    connection.execute(
        "UPDATE project_runs SET heartbeat_at = COALESCE(NULLIF(claimed_at, ''), updated_at, ?1) WHERE heartbeat_at = ''",
        params![now],
    )?;
    connection.execute(
        "UPDATE project_runs SET lease_expires_at = ?1 WHERE lease_expires_at = ''",
        params![default_expiry],
    )?;
    connection.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_project_runs_idempotency ON project_runs(scope_key, idempotency_key) WHERE idempotency_key IS NOT NULL;",
    )?;
    connection.execute(
        "UPDATE planning_config SET value = '10' WHERE key = ?1",
        params![SCHEMA_VERSION_KEY],
    )?;
    Ok(())
}

fn migrate_v10_to_v11(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS discovery_nodes (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            correlation_id TEXT NOT NULL,
            classification TEXT NOT NULL,
            verification_state TEXT NOT NULL,
            severity TEXT NOT NULL,
            status TEXT NOT NULL,
            claim TEXT NOT NULL,
            impact TEXT,
            next_action TEXT,
            verification_step TEXT,
            recurrence_key TEXT,
            fingerprint TEXT,
            observed_at_json TEXT NOT NULL DEFAULT '[]',
            occurrence_count INTEGER NOT NULL DEFAULT 1,
            source_lineage_json TEXT NOT NULL DEFAULT '[]',
            review_date TEXT,
            resolved_at TEXT,
            resolution_rationale TEXT,
            promoted_entity_type TEXT,
            promoted_entity_id TEXT,
            tags_json TEXT NOT NULL DEFAULT '[]',
            revision INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_discovery_nodes_status ON discovery_nodes(status);
        CREATE INDEX IF NOT EXISTS idx_discovery_nodes_classification ON discovery_nodes(classification);
        CREATE INDEX IF NOT EXISTS idx_discovery_nodes_recurrence ON discovery_nodes(recurrence_key);
        CREATE INDEX IF NOT EXISTS idx_discovery_nodes_fingerprint ON discovery_nodes(fingerprint);
        CREATE INDEX IF NOT EXISTS idx_discovery_nodes_severity ON discovery_nodes(severity);

        CREATE TABLE IF NOT EXISTS discovery_relationships (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            relationship_kind TEXT NOT NULL,
            metadata_json TEXT,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_discovery_rel_source ON discovery_relationships(source_id);
        CREATE INDEX IF NOT EXISTS idx_discovery_rel_target ON discovery_relationships(target_id);
        CREATE INDEX IF NOT EXISTS idx_discovery_rel_kind ON discovery_relationships(relationship_kind);

        CREATE TABLE IF NOT EXISTS discovery_checkpoints (
            id TEXT PRIMARY KEY,
            scope_key TEXT NOT NULL,
            run_id TEXT NOT NULL,
            event TEXT NOT NULL,
            snapshot_json TEXT,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_discovery_checkpoints_run ON discovery_checkpoints(run_id);
        "#
    )?;

    connection.execute(
        "UPDATE planning_config SET value = '11' WHERE key = ?1",
        params![SCHEMA_VERSION_KEY],
    )?;
    Ok(())
}

fn rebuild_all_tag_indexes(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    connection.execute("DELETE FROM tag_index", [])?;
    for (table, entity_type) in [
        ("goals", EntityType::Goal),
        ("roadmaps", EntityType::Roadmap),
        ("work_points", EntityType::WorkPoint),
        ("plans", EntityType::Plan),
        ("todos", EntityType::Todo),
        ("issues", EntityType::Issue),
        ("insights", EntityType::Insight),
        ("planning_nodes", EntityType::GraphNode),
        ("discovery_nodes", EntityType::DiscoveryNode),
    ] {
        let mut statement =
            connection.prepare(&format!("SELECT id, scope_key, tags_json FROM {table}"))?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (id, scope_key, tags_json) = row?;
            let tags: Vec<String> = serde_json::from_str(&tags_json).map_err(to_sql_error)?;
            for tag in &tags {
                connection.execute(
                    "INSERT OR IGNORE INTO tag_index (scope_key, entity_type, entity_id, tag) VALUES (?1, ?2, ?3, ?4)",
                    params![scope_key, entity_type.as_str(), id, tag],
                )?;
            }
        }
    }
    Ok(())
}

fn table_has_column(
    connection: &Transaction<'_>,
    table: &str,
    column: &str,
) -> Result<bool, PlanningStoreError> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let columns = collect_rows(rows)?;
    Ok(columns.iter().any(|name| name == column))
}

fn ensure_default_scope(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    let now = now_string()?;
    connection.execute(
        r#"
        INSERT OR IGNORE INTO scopes (
            scope_key, scope_type, parent_scope_key, metadata_json, tags_json, revision, created_at, updated_at
        ) VALUES (?1, ?2, NULL, ?3, ?4, 1, ?5, ?5)
        "#,
        params![
            DEFAULT_SCOPE_KEY,
            "default",
            "{}",
            "[]",
            now,
        ],
    )?;
    Ok(())
}

fn create_scope_indexes(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    connection.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_goals_scope ON goals(scope_key);
        CREATE INDEX IF NOT EXISTS idx_roadmaps_scope ON roadmaps(scope_key);
        CREATE INDEX IF NOT EXISTS idx_roadmap_sections_scope ON roadmap_sections(scope_key);
        CREATE INDEX IF NOT EXISTS idx_work_points_scope ON work_points(scope_key);
        CREATE INDEX IF NOT EXISTS idx_plans_scope ON plans(scope_key);
        CREATE INDEX IF NOT EXISTS idx_todos_scope ON todos(scope_key);
        CREATE INDEX IF NOT EXISTS idx_issues_scope ON issues(scope_key);
        CREATE INDEX IF NOT EXISTS idx_review_points_scope ON review_points(scope_key);
        "#,
    )?;
    Ok(())
}

fn ensure_event_scope_support(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    if !table_has_column(connection, "planning_events", "scope_key")? {
        connection.execute("ALTER TABLE planning_events ADD COLUMN scope_key TEXT", [])?;
    }
    backfill_event_scope_keys(connection)?;
    connection.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_planning_events_scope ON planning_events(scope_key);
        UPDATE planning_events
           SET scope_key = 'default'
         WHERE scope_key IS NULL OR TRIM(scope_key) = ''
        "#,
    )?;
    Ok(())
}

fn backfill_event_scope_keys(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT rowid, entity_type, entity_id, aggregate_type, aggregate_id FROM planning_events WHERE scope_key IS NULL OR TRIM(scope_key) = '' ORDER BY rowid ASC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            parse_entity_type(row.get::<_, String>(1)?)?,
            row.get::<_, String>(2)?,
            parse_entity_type(row.get::<_, String>(3)?)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    for row in rows {
        let (rowid, entity_type, entity_id, aggregate_type, aggregate_id) = row?;
        let scope_key = resolve_event_scope_key(
            connection,
            entity_type,
            &entity_id,
            aggregate_type,
            &aggregate_id,
        )?;
        connection.execute(
            "UPDATE planning_events SET scope_key = ?2 WHERE rowid = ?1",
            params![rowid, scope_key],
        )?;
    }

    Ok(())
}

fn append_event(
    connection: &Transaction<'_>,
    event: PlanningEvent,
) -> Result<(), PlanningStoreError> {
    connection.execute(
        r#"
        INSERT INTO planning_events (
            event_id, scope_key, entity_type, entity_id, aggregate_type, aggregate_id, correlation_id,
            causation_id, run_id, stream_id, sequence, parent_event_id, event_type, timestamp,
            payload_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
        "#,
        params![
            event.event_id,
            resolve_event_scope_key(
                connection,
                event.entity_type,
                &event.entity_id,
                event.aggregate_type,
                &event.aggregate_id,
            )?,
            event.entity_type.as_str(),
            event.entity_id,
            event.aggregate_type.as_str(),
            event.aggregate_id,
            event.correlation_id,
            event.causation_id,
            event.run_id,
            event.stream_id,
            i64::try_from(event.sequence).map_err(|_| {
                PlanningStoreError::InvalidInput("event sequence exceeds i64 range".to_string())
            })?,
            event.parent_event_id,
            event.event_type,
            event.timestamp,
            event.payload.to_string(),
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_event(
    connection: &Transaction<'_>,
    entity_type: EntityType,
    entity_id: &str,
    aggregate_type: EntityType,
    aggregate_id: &str,
    correlation_id: &str,
    run_id: Option<String>,
    event_type: &str,
    payload: Value,
) -> Result<PlanningEvent, PlanningStoreError> {
    let event_id = new_id();
    let timestamp = now_string()?;
    let sequence: i64 = connection.query_row(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM planning_events WHERE stream_id = ?1",
        params![aggregate_id],
        |row| row.get(0),
    )?;
    Ok(PlanningEvent {
        event_id: event_id.clone(),
        entity_type,
        entity_id: entity_id.to_string(),
        aggregate_type,
        aggregate_id: aggregate_id.to_string(),
        correlation_id: correlation_id.to_string(),
        causation_id: None,
        run_id: run_id.unwrap_or_else(|| format!("run-{event_id}")),
        stream_id: aggregate_id.to_string(),
        sequence: u64::try_from(sequence).map_err(|_| {
            PlanningStoreError::InvalidInput("event sequence became negative".to_string())
        })?,
        parent_event_id: None,
        event_type: event_type.to_string(),
        timestamp,
        payload,
    })
}

fn validate_and_store(
    connection: &Transaction<'_>,
    entity_type: EntityType,
    entity_id: &str,
) -> Result<ValidationReport, PlanningStoreError> {
    refresh_validation_target(connection, entity_type, entity_id)
}

fn refresh_validation_target(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
) -> Result<ValidationReport, PlanningStoreError> {
    let findings = validate_entity(connection, entity_type, entity_id)?;
    persist_validation_findings(connection, entity_type, entity_id, &findings)?;
    Ok(ValidationReport::from_findings(findings))
}

fn ensure_entity_exists(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
    field_name: &str,
) -> Result<(), PlanningStoreError> {
    let lookup = match entity_type {
        EntityType::Scope => load_scope(connection, entity_id).map(|_| ()),
        EntityType::Goal => load_goal(connection, entity_id).map(|_| ()),
        EntityType::Roadmap => load_roadmap(connection, entity_id).map(|_| ()),
        EntityType::RoadmapSection => connection
            .query_row(
                "SELECT id FROM roadmap_sections WHERE id = ?1",
                params![entity_id],
                |row| row.get::<_, String>(0),
            )
            .map(|_| ())
            .map_err(|error| map_not_found(error, EntityType::RoadmapSection, entity_id)),
        EntityType::WorkPoint => load_work_point(connection, entity_id).map(|_| ()),
        EntityType::Plan => load_plan(connection, entity_id).map(|_| ()),
        EntityType::Todo => load_todo(connection, entity_id).map(|_| ()),
        EntityType::Issue => load_issue(connection, entity_id).map(|_| ()),
        EntityType::ReviewPoint => connection
            .query_row(
                "SELECT id FROM review_points WHERE id = ?1",
                params![entity_id],
                |row| row.get::<_, String>(0),
            )
            .map(|_| ())
            .map_err(|error| map_not_found(error, EntityType::ReviewPoint, entity_id)),
        EntityType::Insight => load_insight(connection, entity_id).map(|_| ()),
        EntityType::ProjectRun => load_project_run(connection, entity_id).map(|_| ()),
        EntityType::GraphNode => load_graph_node(connection, entity_id).map(|_| ()),
        EntityType::GraphEdge => connection
            .query_row(
                "SELECT id FROM planning_edges WHERE id = ?1",
                params![entity_id],
                |row| row.get::<_, String>(0),
            )
            .map(|_| ())
            .map_err(|error| map_not_found(error, EntityType::GraphEdge, entity_id)),
        EntityType::DiscoveryNode => load_discovery(connection, entity_id).map(|_| ()),
        EntityType::DiscoveryRelationship => connection
            .query_row(
                "SELECT id FROM discovery_relationships WHERE id = ?1",
                params![entity_id],
                |row| row.get::<_, String>(0),
            )
            .map(|_| ())
            .map_err(|error| map_not_found(error, EntityType::DiscoveryRelationship, entity_id)),
        EntityType::DiscoveryCheckpoint => connection
            .query_row(
                "SELECT id FROM discovery_checkpoints WHERE id = ?1",
                params![entity_id],
                |row| row.get::<_, String>(0),
            )
            .map(|_| ())
            .map_err(|error| map_not_found(error, EntityType::DiscoveryCheckpoint, entity_id)),
    };

    match lookup {
        Ok(()) => Ok(()),
        Err(PlanningStoreError::NotFound { .. }) => Err(PlanningStoreError::InvalidInput(format!(
            "{field_name} references missing {} `{}`",
            entity_type.as_str(),
            entity_id
        ))),
        Err(error) => Err(error),
    }
}

fn ensure_referenced_entity_in_scope(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
    field_name: &str,
    active_scope_key: &str,
) -> Result<String, PlanningStoreError> {
    ensure_entity_exists(connection, entity_type, entity_id, field_name)?;
    let scope_key = scope_key_for_entity(connection, entity_type, entity_id)?;
    if scope_key != active_scope_key {
        return Err(scope_reference_mismatch_error(
            field_name,
            entity_type,
            entity_id,
            &scope_key,
            active_scope_key,
        ));
    }
    Ok(scope_key)
}

fn ensure_entity_in_scope(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
    active_scope_key: &str,
) -> Result<String, PlanningStoreError> {
    let scope_key = scope_key_for_entity(connection, entity_type, entity_id)?;
    if scope_key != active_scope_key {
        return Err(scope_entity_mismatch_error(
            entity_type,
            entity_id,
            &scope_key,
            active_scope_key,
        ));
    }
    Ok(scope_key)
}

fn scope_reference_mismatch_error(
    field_name: &str,
    entity_type: EntityType,
    entity_id: &str,
    actual_scope_key: &str,
    active_scope_key: &str,
) -> PlanningStoreError {
    PlanningStoreError::InvalidInput(format!(
        "{field_name} references {} `{entity_id}` in scope `{actual_scope_key}`, not active scope `{active_scope_key}`",
        entity_type.as_str()
    ))
}

fn scope_entity_mismatch_error(
    entity_type: EntityType,
    entity_id: &str,
    actual_scope_key: &str,
    active_scope_key: &str,
) -> PlanningStoreError {
    PlanningStoreError::InvalidInput(format!(
        "{} `{entity_id}` is in scope `{actual_scope_key}`, not active scope `{active_scope_key}`",
        entity_type.as_str()
    ))
}

fn ensure_section_belongs_to_roadmap(
    connection: &Connection,
    section_id: &str,
    roadmap_id: &str,
) -> Result<(), PlanningStoreError> {
    let section_roadmap_id: String = connection
        .query_row(
            "SELECT roadmap_id FROM roadmap_sections WHERE id = ?1",
            params![section_id],
            |row| row.get(0),
        )
        .map_err(|error| map_not_found(error, EntityType::RoadmapSection, section_id))?;

    if section_roadmap_id != roadmap_id {
        return Err(PlanningStoreError::InvalidInput(format!(
            "sectionId `{section_id}` belongs to roadmap `{section_roadmap_id}`, not `{roadmap_id}`"
        )));
    }

    Ok(())
}

fn normalized_scope_key(scope_key: Option<String>) -> String {
    scope_key
        .as_deref()
        .map(normalize_scope_key_value)
        .unwrap_or_else(|| DEFAULT_SCOPE_KEY.to_string())
}

fn normalize_scope_key_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        DEFAULT_SCOPE_KEY.to_string()
    } else {
        trimmed.to_string()
    }
}

fn ensure_scope_exists(connection: &Connection, scope_key: &str) -> Result<(), PlanningStoreError> {
    connection
        .query_row(
            "SELECT scope_key FROM scopes WHERE scope_key = ?1",
            params![scope_key],
            |row| row.get::<_, String>(0),
        )
        .map(|_| ())
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => PlanningStoreError::InvalidInput(format!(
                "scopeKey references missing scope `{scope_key}`"
            )),
            other => PlanningStoreError::Sqlite(other),
        })
}

fn scope_key_for_entity(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
) -> Result<String, PlanningStoreError> {
    let sql = match entity_type {
        EntityType::Scope => "SELECT scope_key FROM scopes WHERE scope_key = ?1",
        EntityType::Goal => "SELECT scope_key FROM goals WHERE id = ?1",
        EntityType::Roadmap => "SELECT scope_key FROM roadmaps WHERE id = ?1",
        EntityType::RoadmapSection => "SELECT scope_key FROM roadmap_sections WHERE id = ?1",
        EntityType::WorkPoint => "SELECT scope_key FROM work_points WHERE id = ?1",
        EntityType::Plan => "SELECT scope_key FROM plans WHERE id = ?1",
        EntityType::Todo => "SELECT scope_key FROM todos WHERE id = ?1",
        EntityType::Issue => "SELECT scope_key FROM issues WHERE id = ?1",
        EntityType::ReviewPoint => "SELECT scope_key FROM review_points WHERE id = ?1",
        EntityType::Insight => "SELECT scope_key FROM insights WHERE id = ?1",
        EntityType::ProjectRun => "SELECT scope_key FROM project_runs WHERE id = ?1",
        EntityType::GraphNode => "SELECT scope_key FROM planning_nodes WHERE id = ?1",
        EntityType::GraphEdge => "SELECT scope_key FROM planning_edges WHERE id = ?1",
        EntityType::DiscoveryNode => "SELECT scope_key FROM discovery_nodes WHERE id = ?1",
        EntityType::DiscoveryRelationship => {
            "SELECT scope_key FROM discovery_relationships WHERE id = ?1"
        }
        EntityType::DiscoveryCheckpoint => {
            "SELECT scope_key FROM discovery_checkpoints WHERE id = ?1"
        }
    };

    connection
        .query_row(sql, params![entity_id], |row| row.get::<_, String>(0))
        .map_err(|error| map_not_found(error, entity_type, entity_id))
}

fn resolve_event_scope_key(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
    aggregate_type: EntityType,
    aggregate_id: &str,
) -> Result<String, PlanningStoreError> {
    scope_key_for_entity(connection, entity_type, entity_id)
        .or_else(|_| scope_key_for_entity(connection, aggregate_type, aggregate_id))
}

fn ensure_plan_transfer_compatible(
    connection: &Connection,
    plan: &PlanRecord,
    target_scope_key: &str,
    targeted_work_point_ids: &[String],
) -> Result<(), PlanningStoreError> {
    if target_scope_key == plan.scope_key {
        return Ok(());
    }

    ensure_plan_transfer_entity_in_scope(
        connection,
        &plan.id,
        EntityType::Goal,
        &plan.goal_id,
        target_scope_key,
    )?;
    ensure_plan_transfer_entity_in_scope(
        connection,
        &plan.id,
        EntityType::Roadmap,
        &plan.roadmap_id,
        target_scope_key,
    )?;

    for work_point_id in targeted_work_point_ids {
        ensure_plan_transfer_entity_in_scope(
            connection,
            &plan.id,
            EntityType::WorkPoint,
            work_point_id,
            target_scope_key,
        )?;
    }

    for todo in list_todos_for_plan(connection, &plan.id)? {
        ensure_plan_transfer_record_in_scope(
            &plan.id,
            EntityType::Todo,
            &todo.id,
            &todo.scope_key,
            target_scope_key,
        )?;
    }

    for review_point in list_review_points_for_entity(connection, EntityType::Plan, &plan.id)? {
        ensure_plan_transfer_record_in_scope(
            &plan.id,
            EntityType::ReviewPoint,
            &review_point.id,
            &review_point.scope_key,
            target_scope_key,
        )?;
    }

    for issue in list_issues_for_related_entity(connection, EntityType::Plan, &plan.id)? {
        ensure_plan_transfer_record_in_scope(
            &plan.id,
            EntityType::Issue,
            &issue.id,
            &issue.scope_key,
            target_scope_key,
        )?;
    }

    Ok(())
}

fn ensure_plan_transfer_entity_in_scope(
    connection: &Connection,
    plan_id: &str,
    entity_type: EntityType,
    entity_id: &str,
    target_scope_key: &str,
) -> Result<(), PlanningStoreError> {
    let actual_scope_key = scope_key_for_entity(connection, entity_type, entity_id)?;
    ensure_plan_transfer_record_in_scope(
        plan_id,
        entity_type,
        entity_id,
        &actual_scope_key,
        target_scope_key,
    )
}

fn ensure_plan_transfer_record_in_scope(
    plan_id: &str,
    entity_type: EntityType,
    entity_id: &str,
    actual_scope_key: &str,
    target_scope_key: &str,
) -> Result<(), PlanningStoreError> {
    if actual_scope_key == target_scope_key {
        return Ok(());
    }

    Err(PlanningStoreError::InvalidInput(format!(
        "plan `{plan_id}` cannot transfer to scope `{target_scope_key}` because linked {} `{entity_id}` remains in scope `{actual_scope_key}`",
        entity_type.as_str()
    )))
}

fn list_work_point_dependents(
    connection: &Connection,
    dependency_id: &str,
) -> Result<Vec<String>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id FROM work_points WHERE EXISTS (SELECT 1 FROM json_each(work_points.dependency_ids_json) WHERE json_each.value = ?1)",
    )?;
    let rows = statement.query_map(params![dependency_id], |row| row.get::<_, String>(0))?;
    collect_rows(rows)
}

fn list_plans_targeting_work_point(
    connection: &Connection,
    work_point_id: &str,
) -> Result<Vec<String>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id FROM plans WHERE EXISTS (SELECT 1 FROM json_each(plans.targeted_work_point_ids_json) WHERE json_each.value = ?1)",
    )?;
    let rows = statement.query_map(params![work_point_id], |row| row.get::<_, String>(0))?;
    collect_rows(rows)
}

pub(crate) fn persist_validation_findings(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
    findings: &[ValidationFinding],
) -> Result<(), PlanningStoreError> {
    connection.execute(
        "DELETE FROM validation_findings WHERE entity_type = ?1 AND entity_id = ?2",
        params![entity_type.as_str(), entity_id],
    )?;
    for finding in findings {
        connection.execute(
            r#"
            INSERT INTO validation_findings (
                finding_id, entity_type, entity_id, severity, code, message, scope_key, fingerprint, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                finding.finding_id,
                finding.entity_type.as_str(),
                finding.entity_id,
                finding.severity.as_str(),
                finding.code,
                finding.message,
                finding.scope_key,
                finding.fingerprint,
                finding.created_at,
            ],
        )?;
    }
    Ok(())
}

pub(crate) fn load_validation_report(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
) -> Result<ValidationReport, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT finding_id, entity_type, entity_id, severity, code, message, scope_key, fingerprint, created_at FROM validation_findings WHERE entity_type = ?1 AND entity_id = ?2 ORDER BY severity ASC, code ASC, created_at ASC",
    )?;
    let rows = statement.query_map(
        params![entity_type.as_str(), entity_id],
        row_to_validation_finding,
    )?;
    Ok(ValidationReport::from_findings(collect_rows(rows)?))
}

pub(crate) fn load_goal(
    connection: &Connection,
    id: &str,
) -> Result<GoalRecord, PlanningStoreError> {
    connection
        .query_row(
            "SELECT id, scope_key, correlation_id, title, description, acceptance_criteria_json, rejection_criteria_json, status, tags_json, revision, created_at, updated_at FROM goals WHERE id = ?1",
            params![id],
            row_to_goal,
        )
        .map_err(|error| map_not_found(error, EntityType::Goal, id))
}

pub(crate) fn load_roadmap(
    connection: &Connection,
    id: &str,
) -> Result<RoadmapRecord, PlanningStoreError> {
    connection
        .query_row(
            "SELECT id, scope_key, goal_id, correlation_id, title, summary, status, tags_json, revision, created_at, updated_at FROM roadmaps WHERE id = ?1",
            params![id],
            row_to_roadmap,
        )
        .map_err(|error| map_not_found(error, EntityType::Roadmap, id))
}

pub(crate) fn load_plan(
    connection: &Connection,
    id: &str,
) -> Result<PlanRecord, PlanningStoreError> {
    let mut record = connection
        .query_row(
            "SELECT id, scope_key, goal_id, roadmap_id, correlation_id, title, summary, scope, assumptions_json, stop_conditions_json, validation_steps_json, targeted_work_point_ids_json, effort_tier, routing_hint, allow_parallel_overlap, status, tags_json, revision, created_at, updated_at FROM plans WHERE id = ?1",
            params![id],
            row_to_plan,
        )
        .map_err(|error| map_not_found(error, EntityType::Plan, id))?;
    record.file_scopes = load_file_scopes_for_entity(connection, EntityType::Plan, id)?;
    Ok(record)
}

pub(crate) fn load_issue(
    connection: &Connection,
    id: &str,
) -> Result<IssueRecord, PlanningStoreError> {
    connection
        .query_row(
            "SELECT id, scope_key, correlation_id, title, summary, status, severity, related_entity_type, related_entity_id, tags_json, revision, created_at, updated_at FROM issues WHERE id = ?1",
            params![id],
            row_to_issue,
        )
        .map_err(|error| map_not_found(error, EntityType::Issue, id))
}

pub(crate) fn load_work_point(
    connection: &Connection,
    id: &str,
) -> Result<WorkPointRecord, PlanningStoreError> {
    let mut record = connection
        .query_row(
            "SELECT id, scope_key, roadmap_id, section_id, title, summary, status, ordering_index, dependency_ids_json, validation_expectations_json, effort_tier, kind, priority, repairs_work_point_ids, supersedes_work_point_ids, blocks_work_point_ids, tags_json, revision, created_at, updated_at FROM work_points WHERE id = ?1",
            params![id],
            row_to_work_point,
        )
        .map_err(|error| map_not_found(error, EntityType::WorkPoint, id))?;
    record.file_scopes = load_file_scopes_for_entity(connection, EntityType::WorkPoint, id)?;
    Ok(record)
}

pub(crate) fn load_todo(
    connection: &Connection,
    id: &str,
) -> Result<TodoRecord, PlanningStoreError> {
    let mut record = connection
        .query_row(
            "SELECT id, scope_key, plan_id, work_point_id, title, summary, status, priority, effort_tier, evidence_refs_json, tags_json, ordering_index, revision, created_at, updated_at FROM todos WHERE id = ?1",
            params![id],
            row_to_todo,
        )
        .map_err(|error| map_not_found(error, EntityType::Todo, id))?;
    record.file_scopes = load_file_scopes_for_entity(connection, EntityType::Todo, id)?;
    Ok(record)
}

pub(crate) fn load_review_point(
    connection: &Connection,
    id: &str,
) -> Result<ReviewPointRecord, PlanningStoreError> {
    connection
        .query_row(
            "SELECT id, scope_key, attached_entity_type, attached_entity_id, title, summary, status, severity, revision, created_at, updated_at FROM review_points WHERE id = ?1",
            params![id],
            row_to_review_point,
        )
        .map_err(|error| map_not_found(error, EntityType::ReviewPoint, id))
}

pub(crate) fn load_scope(
    connection: &Connection,
    scope_key: &str,
) -> Result<ScopeRecord, PlanningStoreError> {
    connection
        .query_row(
            "SELECT scope_key, scope_type, parent_scope_key, metadata_json, tags_json, revision, created_at, updated_at FROM scopes WHERE scope_key = ?1",
            params![normalize_scope_key_value(scope_key)],
            row_to_scope,
        )
        .map_err(|error| map_not_found(error, EntityType::Scope, scope_key))
}

pub(crate) fn list_roadmaps_for_goal_in_scope(
    connection: &Connection,
    goal_id: &str,
    scope_key: &str,
) -> Result<Vec<RoadmapRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, scope_key, goal_id, correlation_id, title, summary, status, tags_json, revision, created_at, updated_at FROM roadmaps WHERE goal_id = ?1 AND scope_key = ?2 ORDER BY updated_at DESC, id ASC",
    )?;
    let rows = statement.query_map(params![goal_id, scope_key], row_to_roadmap)?;
    let items = collect_rows(rows)?;
    Ok(items)
}

pub(crate) fn list_sections_for_roadmap_in_scope(
    connection: &Connection,
    roadmap_id: &str,
    scope_key: &str,
) -> Result<Vec<RoadmapSectionRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, scope_key, roadmap_id, slug, title, summary, ordering_index, revision, created_at, updated_at FROM roadmap_sections WHERE roadmap_id = ?1 AND scope_key = ?2 ORDER BY ordering_index ASC, id ASC",
    )?;
    let rows = statement.query_map(params![roadmap_id, scope_key], row_to_section)?;
    let items = collect_rows(rows)?;
    Ok(items)
}

pub(crate) fn list_work_points_for_roadmap(
    connection: &Connection,
    roadmap_id: &str,
) -> Result<Vec<WorkPointRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, scope_key, roadmap_id, section_id, title, summary, status, ordering_index, dependency_ids_json, validation_expectations_json, effort_tier, kind, priority, repairs_work_point_ids, supersedes_work_point_ids, blocks_work_point_ids, tags_json, revision, created_at, updated_at FROM work_points WHERE roadmap_id = ?1 ORDER BY ordering_index ASC, id ASC",
    )?;
    let rows = statement.query_map(params![roadmap_id], row_to_work_point)?;
    let mut items = collect_rows(rows)?;
    let ids = items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
    let mut file_scopes_by_id =
        load_file_scopes_for_entities(connection, EntityType::WorkPoint, &ids)?;
    for item in &mut items {
        item.file_scopes = file_scopes_by_id.remove(&item.id).unwrap_or_default();
    }
    Ok(items)
}

pub(crate) fn list_work_points_for_roadmap_in_scope(
    connection: &Connection,
    roadmap_id: &str,
    scope_key: &str,
) -> Result<Vec<WorkPointRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, scope_key, roadmap_id, section_id, title, summary, status, ordering_index, dependency_ids_json, validation_expectations_json, effort_tier, kind, priority, repairs_work_point_ids, supersedes_work_point_ids, blocks_work_point_ids, tags_json, revision, created_at, updated_at FROM work_points WHERE roadmap_id = ?1 AND scope_key = ?2 ORDER BY ordering_index ASC, id ASC",
    )?;
    let rows = statement.query_map(params![roadmap_id, scope_key], row_to_work_point)?;
    let mut items = collect_rows(rows)?;
    let ids = items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
    let mut file_scopes_by_id =
        load_file_scopes_for_entities(connection, EntityType::WorkPoint, &ids)?;
    for item in &mut items {
        item.file_scopes = file_scopes_by_id.remove(&item.id).unwrap_or_default();
    }
    Ok(items)
}

pub(crate) fn list_todos_for_plan(
    connection: &Connection,
    plan_id: &str,
) -> Result<Vec<TodoRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, scope_key, plan_id, work_point_id, title, summary, status, priority, effort_tier, evidence_refs_json, tags_json, ordering_index, revision, created_at, updated_at FROM todos WHERE plan_id = ?1 ORDER BY ordering_index ASC, id ASC",
    )?;
    let rows = statement.query_map(params![plan_id], row_to_todo)?;
    let mut items = collect_rows(rows)?;
    let ids = items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
    let mut file_scopes_by_id = load_file_scopes_for_entities(connection, EntityType::Todo, &ids)?;
    for item in &mut items {
        item.file_scopes = file_scopes_by_id.remove(&item.id).unwrap_or_default();
    }
    Ok(items)
}

pub(crate) fn list_todos_for_plan_in_scope(
    connection: &Connection,
    plan_id: &str,
    scope_key: &str,
) -> Result<Vec<TodoRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, scope_key, plan_id, work_point_id, title, summary, status, priority, effort_tier, evidence_refs_json, tags_json, ordering_index, revision, created_at, updated_at FROM todos WHERE plan_id = ?1 AND scope_key = ?2 ORDER BY ordering_index ASC, id ASC",
    )?;
    let rows = statement.query_map(params![plan_id, scope_key], row_to_todo)?;
    let mut items = collect_rows(rows)?;
    let ids = items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
    let mut file_scopes_by_id = load_file_scopes_for_entities(connection, EntityType::Todo, &ids)?;
    for item in &mut items {
        item.file_scopes = file_scopes_by_id.remove(&item.id).unwrap_or_default();
    }
    Ok(items)
}

pub(crate) fn list_review_points_for_entity(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
) -> Result<Vec<ReviewPointRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, scope_key, attached_entity_type, attached_entity_id, title, summary, status, severity, revision, created_at, updated_at FROM review_points WHERE attached_entity_type = ?1 AND attached_entity_id = ?2 ORDER BY created_at ASC, id ASC",
    )?;
    let rows = statement.query_map(
        params![entity_type.as_str(), entity_id],
        row_to_review_point,
    )?;
    let items = collect_rows(rows)?;
    Ok(items)
}

pub(crate) fn list_review_points_for_entity_in_scope(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
    scope_key: &str,
) -> Result<Vec<ReviewPointRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, scope_key, attached_entity_type, attached_entity_id, title, summary, status, severity, revision, created_at, updated_at FROM review_points WHERE attached_entity_type = ?1 AND attached_entity_id = ?2 AND scope_key = ?3 ORDER BY created_at ASC, id ASC",
    )?;
    let rows = statement.query_map(
        params![entity_type.as_str(), entity_id, scope_key],
        row_to_review_point,
    )?;
    let items = collect_rows(rows)?;
    Ok(items)
}

pub(crate) fn list_issues_for_related_entity(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
) -> Result<Vec<IssueRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, scope_key, correlation_id, title, summary, status, severity, related_entity_type, related_entity_id, tags_json, revision, created_at, updated_at FROM issues WHERE related_entity_type = ?1 AND related_entity_id = ?2 ORDER BY created_at ASC, id ASC",
    )?;
    let rows = statement.query_map(params![entity_type.as_str(), entity_id], row_to_issue)?;
    let items = collect_rows(rows)?;
    Ok(items)
}

pub(crate) fn collect_entities(
    connection: &Connection,
) -> Result<Vec<(EntityType, String)>, PlanningStoreError> {
    let mut entities = Vec::new();
    entities.extend(entity_ids(connection, "goals", EntityType::Goal)?);
    entities.extend(entity_ids(connection, "roadmaps", EntityType::Roadmap)?);
    entities.extend(entity_ids(
        connection,
        "roadmap_sections",
        EntityType::RoadmapSection,
    )?);
    entities.extend(entity_ids(
        connection,
        "work_points",
        EntityType::WorkPoint,
    )?);
    entities.extend(entity_ids(connection, "plans", EntityType::Plan)?);
    entities.extend(entity_ids(connection, "todos", EntityType::Todo)?);
    entities.extend(entity_ids(connection, "issues", EntityType::Issue)?);
    entities.extend(entity_ids(
        connection,
        "review_points",
        EntityType::ReviewPoint,
    )?);
    entities.extend(entity_ids(connection, "insights", EntityType::Insight)?);
    entities.extend(entity_ids(
        connection,
        "project_runs",
        EntityType::ProjectRun,
    )?);
    entities.extend(entity_ids(
        connection,
        "planning_nodes",
        EntityType::GraphNode,
    )?);
    entities.extend(entity_ids(
        connection,
        "planning_edges",
        EntityType::GraphEdge,
    )?);
    entities.extend(entity_ids(
        connection,
        "discovery_nodes",
        EntityType::DiscoveryNode,
    )?);
    entities.extend(entity_ids(
        connection,
        "discovery_relationships",
        EntityType::DiscoveryRelationship,
    )?);
    entities.extend(entity_ids(
        connection,
        "discovery_checkpoints",
        EntityType::DiscoveryCheckpoint,
    )?);
    Ok(entities)
}

pub(crate) fn collect_entities_in_scope(
    connection: &Connection,
    scope_key: &str,
) -> Result<Vec<(EntityType, String)>, PlanningStoreError> {
    let mut entities = Vec::new();
    entities.extend(entity_ids_in_scope(
        connection,
        "goals",
        "scope_key",
        EntityType::Goal,
        scope_key,
    )?);
    entities.extend(entity_ids_in_scope(
        connection,
        "roadmaps",
        "scope_key",
        EntityType::Roadmap,
        scope_key,
    )?);
    entities.extend(entity_ids_in_scope(
        connection,
        "roadmap_sections",
        "scope_key",
        EntityType::RoadmapSection,
        scope_key,
    )?);
    entities.extend(entity_ids_in_scope(
        connection,
        "work_points",
        "scope_key",
        EntityType::WorkPoint,
        scope_key,
    )?);
    entities.extend(entity_ids_in_scope(
        connection,
        "plans",
        "scope_key",
        EntityType::Plan,
        scope_key,
    )?);
    entities.extend(entity_ids_in_scope(
        connection,
        "todos",
        "scope_key",
        EntityType::Todo,
        scope_key,
    )?);
    entities.extend(entity_ids_in_scope(
        connection,
        "issues",
        "scope_key",
        EntityType::Issue,
        scope_key,
    )?);
    entities.extend(entity_ids_in_scope(
        connection,
        "review_points",
        "scope_key",
        EntityType::ReviewPoint,
        scope_key,
    )?);
    entities.extend(entity_ids_in_scope(
        connection,
        "insights",
        "scope_key",
        EntityType::Insight,
        scope_key,
    )?);
    entities.extend(entity_ids_in_scope(
        connection,
        "project_runs",
        "scope_key",
        EntityType::ProjectRun,
        scope_key,
    )?);
    entities.extend(entity_ids_in_scope(
        connection,
        "planning_nodes",
        "scope_key",
        EntityType::GraphNode,
        scope_key,
    )?);
    entities.extend(entity_ids_in_scope(
        connection,
        "planning_edges",
        "scope_key",
        EntityType::GraphEdge,
        scope_key,
    )?);
    entities.extend(entity_ids_in_scope(
        connection,
        "discovery_nodes",
        "scope_key",
        EntityType::DiscoveryNode,
        scope_key,
    )?);
    entities.extend(entity_ids_in_scope(
        connection,
        "discovery_relationships",
        "scope_key",
        EntityType::DiscoveryRelationship,
        scope_key,
    )?);
    entities.extend(entity_ids_in_scope(
        connection,
        "discovery_checkpoints",
        "scope_key",
        EntityType::DiscoveryCheckpoint,
        scope_key,
    )?);
    Ok(entities)
}

fn entity_ids_in_scope(
    connection: &Connection,
    table: &str,
    scope_column: &str,
    entity_type: EntityType,
    scope_key: &str,
) -> Result<Vec<(EntityType, String)>, PlanningStoreError> {
    let sql = format!("SELECT id FROM {table} WHERE {scope_column} = ?1 ORDER BY id ASC");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params![scope_key], |row| row.get::<_, String>(0))?;
    let ids = collect_rows(rows)?;
    Ok(ids.into_iter().map(|id| (entity_type, id)).collect())
}

fn entity_ids(
    connection: &Connection,
    table: &str,
    entity_type: EntityType,
) -> Result<Vec<(EntityType, String)>, PlanningStoreError> {
    let mut statement = connection.prepare(&format!("SELECT id FROM {table} ORDER BY id ASC"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    let ids = collect_rows(rows)?;
    Ok(ids.into_iter().map(|id| (entity_type, id)).collect())
}

pub(crate) fn roadmap_correlation_id(
    connection: &Connection,
    roadmap_id: &str,
) -> Result<String, PlanningStoreError> {
    Ok(load_roadmap(connection, roadmap_id)?.correlation_id)
}

pub(crate) fn plan_correlation_id(
    connection: &Connection,
    plan_id: &str,
) -> Result<String, PlanningStoreError> {
    Ok(load_plan(connection, plan_id)?.correlation_id)
}

pub(crate) fn work_point_correlation_id(
    connection: &Connection,
    work_point_id: &str,
) -> Result<String, PlanningStoreError> {
    let work_point = load_work_point(connection, work_point_id)?;
    roadmap_correlation_id(connection, &work_point.roadmap_id)
}

pub(crate) fn attached_entity_correlation_id(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
) -> Result<String, PlanningStoreError> {
    match entity_type {
        EntityType::Scope => Ok(format!("corr-scope-{entity_id}")),
        EntityType::Goal => Ok(load_goal(connection, entity_id)?.correlation_id),
        EntityType::Roadmap => Ok(load_roadmap(connection, entity_id)?.correlation_id),
        EntityType::WorkPoint => work_point_correlation_id(connection, entity_id),
        EntityType::Plan => plan_correlation_id(connection, entity_id),
        EntityType::Todo => {
            let todo = load_todo(connection, entity_id)?;
            if let Some(plan_id) = todo.plan_id {
                plan_correlation_id(connection, &plan_id)
            } else if let Some(work_point_id) = todo.work_point_id {
                work_point_correlation_id(connection, &work_point_id)
            } else {
                Ok(format!("corr-{entity_id}"))
            }
        }
        EntityType::Issue => Ok(load_issue(connection, entity_id)?.correlation_id),
        EntityType::Insight => Ok(load_insight(connection, entity_id)?.correlation_id),
        EntityType::ProjectRun => {
            let run = load_project_run(connection, entity_id)?;
            roadmap_correlation_id(connection, &run.roadmap_id)
        }
        EntityType::RoadmapSection => {
            let section = connection
                .query_row(
                    "SELECT roadmap_id FROM roadmap_sections WHERE id = ?1",
                    params![entity_id],
                    |row| row.get::<_, String>(0),
                )
                .map_err(|error| map_not_found(error, EntityType::RoadmapSection, entity_id))?;
            roadmap_correlation_id(connection, &section)
        }
        EntityType::ReviewPoint => {
            let review = connection
                .query_row(
                    "SELECT attached_entity_type, attached_entity_id FROM review_points WHERE id = ?1",
                    params![entity_id],
                    |row| {
                        Ok((
                            parse_entity_type(row.get::<_, String>(0)?)?,
                            row.get::<_, String>(1)?,
                        ))
                    },
                )
                .map_err(|error| map_not_found(error, EntityType::ReviewPoint, entity_id))?;
            attached_entity_correlation_id(connection, review.0, &review.1)
        }
        EntityType::GraphNode => Ok(format!("corr-graph-node-{entity_id}")),
        EntityType::GraphEdge => Ok(format!("corr-graph-edge-{entity_id}")),
        EntityType::DiscoveryNode => Ok(load_discovery(connection, entity_id)?.correlation_id),
        EntityType::DiscoveryRelationship => Ok(format!("corr-discovery-rel-{entity_id}")),
        EntityType::DiscoveryCheckpoint => Ok(format!("corr-discovery-cp-{entity_id}")),
    }
}

fn next_ordering(
    connection: &Transaction<'_>,
    table: &str,
    group_column: &str,
    group_value: &str,
) -> Result<i64, PlanningStoreError> {
    let sql = format!(
        "SELECT COALESCE(MAX(ordering_index), 0) + 1 FROM {table} WHERE {group_column} = ?1"
    );
    Ok(connection.query_row(&sql, params![group_value], |row| row.get(0))?)
}

fn next_todo_ordering(
    connection: &Transaction<'_>,
    grouping_key: &str,
) -> Result<i64, PlanningStoreError> {
    Ok(connection.query_row(
        r#"
        SELECT COALESCE(MAX(ordering_index), 0) + 1
        FROM todos
        WHERE COALESCE(plan_id, work_point_id, '__global__') = ?1
        "#,
        params![grouping_key],
        |row| row.get(0),
    )?)
}

fn normalize_file_scopes(file_scopes: Vec<FileScopeRecord>) -> Vec<FileScopeRecord> {
    let mut scopes: Vec<FileScopeRecord> = file_scopes
        .into_iter()
        .map(|scope| FileScopeRecord {
            selector_type: scope.selector_type,
            selector: scope.selector.trim().to_string(),
            intent: scope.intent,
        })
        .filter(|scope| !scope.selector.is_empty())
        .collect();
    scopes.sort_by(|left, right| {
        left.selector_type
            .as_str()
            .cmp(right.selector_type.as_str())
            .then_with(|| left.intent.as_str().cmp(right.intent.as_str()))
            .then_with(|| left.selector.cmp(&right.selector))
    });
    scopes.dedup_by(|left, right| {
        left.selector_type == right.selector_type
            && left.intent == right.intent
            && left.selector == right.selector
    });
    scopes
}

fn replace_entity_file_scopes(
    connection: &Transaction<'_>,
    scope_key: &str,
    owner_entity_type: EntityType,
    owner_entity_id: &str,
    file_scopes: &[FileScopeRecord],
    now: &str,
) -> Result<(), PlanningStoreError> {
    connection.execute(
        "DELETE FROM entity_file_scopes WHERE owner_entity_type = ?1 AND owner_entity_id = ?2",
        params![owner_entity_type.as_str(), owner_entity_id],
    )?;

    for (index, file_scope) in file_scopes.iter().enumerate() {
        connection.execute(
            r#"
            INSERT INTO entity_file_scopes (
                id, scope_key, owner_entity_type, owner_entity_id, selector_type, selector,
                intent, ordering_index, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                new_id(),
                scope_key,
                owner_entity_type.as_str(),
                owner_entity_id,
                file_scope.selector_type.as_str(),
                file_scope.selector,
                file_scope.intent.as_str(),
                index as i64,
                now,
                now,
            ],
        )?;
    }

    Ok(())
}

fn load_file_scopes_for_entity(
    connection: &Connection,
    owner_entity_type: EntityType,
    owner_entity_id: &str,
) -> Result<Vec<FileScopeRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT selector_type, selector, intent FROM entity_file_scopes WHERE owner_entity_type = ?1 AND owner_entity_id = ?2 ORDER BY ordering_index ASC, id ASC",
    )?;
    let rows = statement.query_map(
        params![owner_entity_type.as_str(), owner_entity_id],
        |row| {
            Ok(FileScopeRecord {
                selector_type: parse_file_scope_selector_type(row.get::<_, String>(0)?)?,
                selector: row.get(1)?,
                intent: parse_file_scope_intent(row.get::<_, String>(2)?)?,
            })
        },
    )?;
    collect_rows(rows)
}

fn load_file_scopes_for_entities(
    connection: &Connection,
    owner_entity_type: EntityType,
    owner_entity_ids: &[String],
) -> Result<HashMap<String, Vec<FileScopeRecord>>, PlanningStoreError> {
    if owner_entity_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut grouped = HashMap::<String, Vec<FileScopeRecord>>::new();
    let chunk_size = SQLITE_MAX_VARIABLES - FILE_SCOPE_QUERY_FIXED_VARIABLES;

    for chunk in owner_entity_ids.chunks(chunk_size) {
        let mut params_vec = Vec::with_capacity(chunk.len() + 1);
        params_vec.push(owner_entity_type.as_str().to_string());
        params_vec.extend(chunk.iter().cloned());
        let placeholders = (2..(chunk.len() + 2))
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!(
            "SELECT owner_entity_id, selector_type, selector, intent FROM entity_file_scopes WHERE owner_entity_type = ?1 AND owner_entity_id IN ({placeholders}) ORDER BY owner_entity_id ASC, ordering_index ASC, id ASC"
        );

        let mut statement = connection.prepare(&query)?;
        let rows = statement.query_map(params_from_iter(params_vec), |row| {
            Ok((
                row.get::<_, String>(0)?,
                FileScopeRecord {
                    selector_type: parse_file_scope_selector_type(row.get::<_, String>(1)?)?,
                    selector: row.get(2)?,
                    intent: parse_file_scope_intent(row.get::<_, String>(3)?)?,
                },
            ))
        })?;

        for row in rows {
            let (entity_id, scope) = row?;
            grouped.entry(entity_id).or_default().push(scope);
        }
    }

    Ok(grouped)
}

fn update_status_row(
    connection: &Transaction<'_>,
    table: &str,
    entity_id: &str,
    status: &str,
    now: &str,
) -> Result<(), PlanningStoreError> {
    let affected = connection.execute(
        &format!(
            "UPDATE {table} SET status = ?2, revision = revision + 1, updated_at = ?3 WHERE id = ?1"
        ),
        params![entity_id, status, now],
    )?;
    if affected == 0 {
        return Err(PlanningStoreError::NotFound {
            entity_type: table.trim_end_matches('s').to_string(),
            entity_id: entity_id.to_string(),
        });
    }
    Ok(())
}

fn update_todo_status_row(
    connection: &Transaction<'_>,
    entity_id: &str,
    status: &str,
    evidence_refs: Vec<String>,
    now: &str,
) -> Result<(), PlanningStoreError> {
    let affected = connection.execute(
        r#"
        UPDATE todos
           SET status = ?2,
               evidence_refs_json = ?3,
               revision = revision + 1,
               updated_at = ?4
         WHERE id = ?1
        "#,
        params![entity_id, status, to_json_text(&evidence_refs)?, now],
    )?;
    if affected == 0 {
        return Err(PlanningStoreError::NotFound {
            entity_type: EntityType::Todo.as_str().to_string(),
            entity_id: entity_id.to_string(),
        });
    }
    Ok(())
}

fn entity_revision(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
) -> Result<i64, PlanningStoreError> {
    let (table, id_column) = match entity_type {
        EntityType::Scope => ("scopes", "scope_key"),
        EntityType::Goal => ("goals", "id"),
        EntityType::Roadmap => ("roadmaps", "id"),
        EntityType::RoadmapSection => ("roadmap_sections", "id"),
        EntityType::WorkPoint => ("work_points", "id"),
        EntityType::Plan => ("plans", "id"),
        EntityType::Todo => ("todos", "id"),
        EntityType::Issue => ("issues", "id"),
        EntityType::ReviewPoint => ("review_points", "id"),
        EntityType::Insight => ("insights", "id"),
        EntityType::ProjectRun => ("project_runs", "id"),
        EntityType::GraphNode => ("planning_nodes", "id"),
        EntityType::GraphEdge => ("planning_edges", "id"),
        EntityType::DiscoveryNode => ("discovery_nodes", "id"),
        EntityType::DiscoveryRelationship => ("discovery_relationships", "id"),
        EntityType::DiscoveryCheckpoint => ("discovery_checkpoints", "id"),
    };
    let sql = format!("SELECT revision FROM {table} WHERE {id_column} = ?1");
    connection
        .query_row(&sql, params![entity_id], |row| row.get(0))
        .map_err(|error| map_not_found(error, entity_type, entity_id))
}

fn count_table(connection: &Connection, table: &str) -> Result<i64, PlanningStoreError> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    Ok(connection.query_row(&sql, [], |row| row.get(0))?)
}

fn row_to_goal(row: &Row<'_>) -> Result<GoalRecord, rusqlite::Error> {
    Ok(GoalRecord {
        id: row.get(0)?,
        scope_key: row.get(1)?,
        correlation_id: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        acceptance_criteria: parse_json_column(row.get::<_, String>(5)?)?,
        rejection_criteria: parse_json_column(row.get::<_, String>(6)?)?,
        status: parse_goal_status(row.get::<_, String>(7)?)?,
        tags: parse_json_column(row.get::<_, String>(8)?)?,
        revision: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn row_to_roadmap(row: &Row<'_>) -> Result<RoadmapRecord, rusqlite::Error> {
    Ok(RoadmapRecord {
        id: row.get(0)?,
        scope_key: row.get(1)?,
        goal_id: row.get(2)?,
        correlation_id: row.get(3)?,
        title: row.get(4)?,
        summary: row.get(5)?,
        status: parse_roadmap_status(row.get::<_, String>(6)?)?,
        tags: parse_json_column(row.get::<_, String>(7)?)?,
        revision: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn row_to_section(row: &Row<'_>) -> Result<RoadmapSectionRecord, rusqlite::Error> {
    Ok(RoadmapSectionRecord {
        id: row.get(0)?,
        scope_key: row.get(1)?,
        roadmap_id: row.get(2)?,
        slug: row.get(3)?,
        title: row.get(4)?,
        summary: row.get(5)?,
        ordering: row.get(6)?,
        revision: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn row_to_work_point(row: &Row<'_>) -> Result<WorkPointRecord, rusqlite::Error> {
    Ok(WorkPointRecord {
        id: row.get(0)?,
        scope_key: row.get(1)?,
        roadmap_id: row.get(2)?,
        section_id: row.get(3)?,
        title: row.get(4)?,
        summary: row.get(5)?,
        status: parse_work_point_status(row.get::<_, String>(6)?)?,
        ordering: row.get(7)?,
        dependency_ids: parse_json_column(row.get::<_, String>(8)?)?,
        validation_expectations: parse_json_column(row.get::<_, String>(9)?)?,
        effort_tier: parse_effort_tier(row.get::<_, String>(10)?)?,
        kind: parse_work_point_kind(row.get::<_, String>(11)?)?,
        priority: parse_priority(row.get::<_, String>(12)?)?,
        repairs_work_point_ids: parse_json_column(row.get::<_, String>(13)?)?,
        supersedes_work_point_ids: parse_json_column(row.get::<_, String>(14)?)?,
        blocks_work_point_ids: parse_json_column(row.get::<_, String>(15)?)?,
        file_scopes: Vec::new(),
        tags: parse_json_column(row.get::<_, String>(16)?)?,
        revision: row.get(17)?,
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
    })
}

fn row_to_plan(row: &Row<'_>) -> Result<PlanRecord, rusqlite::Error> {
    Ok(PlanRecord {
        id: row.get(0)?,
        scope_key: row.get(1)?,
        goal_id: row.get(2)?,
        roadmap_id: row.get(3)?,
        correlation_id: row.get(4)?,
        title: row.get(5)?,
        summary: row.get(6)?,
        scope: row.get(7)?,
        assumptions: parse_json_column(row.get::<_, String>(8)?)?,
        stop_conditions: parse_json_column(row.get::<_, String>(9)?)?,
        validation_steps: parse_json_column(row.get::<_, String>(10)?)?,
        targeted_work_point_ids: parse_json_column(row.get::<_, String>(11)?)?,
        effort_tier: parse_effort_tier(row.get::<_, String>(12)?)?,
        routing_hint: row.get(13)?,
        allow_parallel_overlap: row.get::<_, i64>(14)? != 0,
        file_scopes: Vec::new(),
        status: parse_plan_status(row.get::<_, String>(15)?)?,
        tags: parse_json_column(row.get::<_, String>(16)?)?,
        revision: row.get(17)?,
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
    })
}

fn row_to_todo(row: &Row<'_>) -> Result<TodoRecord, rusqlite::Error> {
    Ok(TodoRecord {
        id: row.get(0)?,
        scope_key: row.get(1)?,
        plan_id: row.get(2)?,
        work_point_id: row.get(3)?,
        title: row.get(4)?,
        summary: row.get(5)?,
        status: parse_todo_status(row.get::<_, String>(6)?)?,
        priority: parse_priority(row.get::<_, String>(7)?)?,
        effort_tier: parse_effort_tier(row.get::<_, String>(8)?)?,
        file_scopes: Vec::new(),
        evidence_refs: parse_json_column(row.get::<_, String>(9)?)?,
        tags: parse_json_column(row.get::<_, String>(10)?)?,
        ordering: row.get(11)?,
        revision: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn row_to_issue(row: &Row<'_>) -> Result<IssueRecord, rusqlite::Error> {
    Ok(IssueRecord {
        id: row.get(0)?,
        scope_key: row.get(1)?,
        correlation_id: row.get(2)?,
        title: row.get(3)?,
        summary: row.get(4)?,
        status: parse_issue_status(row.get::<_, String>(5)?)?,
        severity: parse_severity(row.get::<_, String>(6)?)?,
        related_entity_type: row
            .get::<_, Option<String>>(7)?
            .map(parse_entity_type)
            .transpose()?,
        related_entity_id: row.get(8)?,
        tags: parse_json_column(row.get::<_, String>(9)?)?,
        revision: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn row_to_review_point(row: &Row<'_>) -> Result<ReviewPointRecord, rusqlite::Error> {
    Ok(ReviewPointRecord {
        id: row.get(0)?,
        scope_key: row.get(1)?,
        attached_entity_type: parse_entity_type(row.get::<_, String>(2)?)?,
        attached_entity_id: row.get(3)?,
        title: row.get(4)?,
        summary: row.get(5)?,
        status: parse_review_point_status(row.get::<_, String>(6)?)?,
        severity: parse_severity(row.get::<_, String>(7)?)?,
        revision: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn row_to_scope(row: &Row<'_>) -> Result<ScopeRecord, rusqlite::Error> {
    Ok(ScopeRecord {
        scope_key: row.get(0)?,
        scope_type: row.get(1)?,
        parent_scope_key: row.get(2)?,
        metadata: serde_json::from_str(&row.get::<_, String>(3)?).map_err(to_sql_error)?,
        tags: parse_json_column(row.get::<_, String>(4)?)?,
        revision: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn row_to_event(row: &Row<'_>) -> Result<PlanningEvent, rusqlite::Error> {
    let sequence: i64 = row.get(9)?;
    Ok(PlanningEvent {
        event_id: row.get(0)?,
        entity_type: parse_entity_type(row.get::<_, String>(1)?)?,
        entity_id: row.get(2)?,
        aggregate_type: parse_entity_type(row.get::<_, String>(3)?)?,
        aggregate_id: row.get(4)?,
        correlation_id: row.get(5)?,
        causation_id: row.get(6)?,
        run_id: row.get(7)?,
        stream_id: row.get(8)?,
        sequence: u64::try_from(sequence).map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                9,
                rusqlite::types::Type::Integer,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "negative event sequence",
                )),
            )
        })?,
        parent_event_id: row.get(10)?,
        event_type: row.get(11)?,
        timestamp: row.get(12)?,
        payload: serde_json::from_str(&row.get::<_, String>(13)?).map_err(to_sql_error)?,
    })
}

fn row_to_validation_finding(row: &Row<'_>) -> Result<ValidationFinding, rusqlite::Error> {
    Ok(ValidationFinding {
        finding_id: row.get(0)?,
        entity_type: parse_entity_type(row.get::<_, String>(1)?)?,
        entity_id: row.get(2)?,
        severity: parse_validation_severity(row.get::<_, String>(3)?)?,
        code: row.get(4)?,
        message: row.get(5)?,
        scope_key: row.get::<_, String>(6).unwrap_or_default(),
        fingerprint: row.get::<_, String>(7).unwrap_or_default(),
        created_at: row.get(8)?,
    })
}

fn render_goal_markdown(view: &GoalView) -> String {
    let mut text = String::new();
    text.push_str(&format!("# {}\n\n", view.goal.title));
    text.push_str(&format!("Goal ID: `{}`\n", view.goal.id));
    text.push_str(&format!("Status: `{}`\n", view.goal.status));
    text.push_str(&format!(
        "Correlation ID: `{}`\n\n",
        view.goal.correlation_id
    ));
    text.push_str("## Description\n\n");
    text.push_str(&view.goal.description);
    text.push_str("\n\n## Acceptance Criteria\n\n");
    append_list(&mut text, &view.goal.acceptance_criteria);
    text.push_str("\n## Rejection Criteria\n\n");
    append_list(&mut text, &view.goal.rejection_criteria);
    text.push_str("\n## Linked Roadmaps\n\n");
    if view.roadmaps.is_empty() {
        text.push_str("No linked roadmaps.\n");
    } else {
        for roadmap in &view.roadmaps {
            text.push_str(&format!(
                "- `{}` {} (`{}`)\n",
                roadmap.id, roadmap.title, roadmap.status
            ));
        }
    }
    text.push_str("\n## Validation\n\n");
    append_validation(&mut text, &view.validation);
    text
}

fn render_roadmap_markdown(view: &RoadmapView) -> String {
    let mut text = String::new();
    text.push_str(&format!("# {}\n\n", view.roadmap.title));
    text.push_str(&format!("Roadmap ID: `{}`\n", view.roadmap.id));
    text.push_str(&format!("Goal ID: `{}`\n", view.roadmap.goal_id));
    text.push_str(&format!("Status: `{}`\n\n", view.roadmap.status));
    text.push_str(&view.roadmap.summary);
    text.push_str("\n\n## Sections\n\n");
    if view.sections.is_empty() {
        text.push_str("No sections yet.\n");
    } else {
        for section in &view.sections {
            text.push_str(&format!("- `{}` {}\n", section.slug, section.title));
        }
    }
    text.push_str("\n## Work Points\n\n");
    if view.work_points.is_empty() {
        text.push_str("No work points yet.\n");
    } else {
        for work_point in &view.work_points {
            text.push_str(&format!(
                "- `{}` {} (`{}`)\n",
                work_point.id, work_point.title, work_point.status
            ));
        }
    }
    text.push_str("\n## Validation\n\n");
    append_validation(&mut text, &view.validation);
    text
}

fn render_plan_markdown(view: &PlanView) -> String {
    let mut text = String::new();
    text.push_str(&format!("# {}\n\n", view.plan.title));
    text.push_str(&format!("Plan ID: `{}`\n", view.plan.id));
    text.push_str(&format!("Goal ID: `{}`\n", view.plan.goal_id));
    text.push_str(&format!("Roadmap ID: `{}`\n", view.plan.roadmap_id));
    text.push_str(&format!("Status: `{}`\n\n", view.plan.status));
    text.push_str(&view.plan.summary);
    text.push_str("\n\n## Scope\n\n");
    text.push_str(&view.plan.scope);
    text.push_str("\n\n## Todos\n\n");
    if view.todos.is_empty() {
        text.push_str("No todos yet.\n");
    } else {
        for todo in &view.todos {
            text.push_str(&format!(
                "- `{}` {} (`{}`)\n",
                todo.id, todo.title, todo.status
            ));
        }
    }
    text.push_str("\n## Review Points\n\n");
    if view.review_points.is_empty() {
        text.push_str("No review points recorded.\n");
    } else {
        for point in &view.review_points {
            text.push_str(&format!(
                "- `{}` {} (`{}` / `{}`)\n",
                point.id, point.title, point.status, point.severity
            ));
        }
    }
    text.push_str("\n## Validation\n\n");
    append_validation(&mut text, &view.validation);
    text
}

fn render_issue_markdown(view: &IssueView) -> String {
    let mut text = String::new();
    text.push_str(&format!("# {}\n\n", view.issue.title));
    text.push_str(&format!("Issue ID: `{}`\n", view.issue.id));
    text.push_str(&format!("Status: `{}`\n", view.issue.status));
    text.push_str(&format!("Severity: `{}`\n\n", view.issue.severity));
    text.push_str(&view.issue.summary);
    text.push_str("\n\n## Validation\n\n");
    append_validation(&mut text, &view.validation);
    text
}

fn append_list(buffer: &mut String, entries: &[String]) {
    if entries.is_empty() {
        buffer.push_str("None recorded.\n");
        return;
    }
    for entry in entries {
        buffer.push_str("- ");
        buffer.push_str(entry);
        buffer.push('\n');
    }
}

fn append_validation(buffer: &mut String, report: &ValidationReport) {
    buffer.push_str(&format!("Status: `{}`\n", report.status));
    if report.findings.is_empty() {
        buffer.push_str("No validation findings.\n");
        return;
    }
    for finding in &report.findings {
        buffer.push_str(&format!(
            "- `{}` `{}` {}\n",
            finding.severity, finding.code, finding.message
        ));
    }
}

fn now_string() -> Result<String, PlanningStoreError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| PlanningStoreError::TimeFormat)
}

fn normalize_lease_seconds(value: Option<i64>) -> Result<i64, PlanningStoreError> {
    let seconds = value.unwrap_or(DEFAULT_LEASE_SECONDS);
    if !(1..=86_400).contains(&seconds) {
        return Err(PlanningStoreError::InvalidInput(
            serde_json::json!({
                "code": "PROJECT-RUN-LEASE-DURATION-INVALID",
                "message": "leaseSeconds must be between 1 and 86400",
                "leaseSeconds": seconds,
            })
            .to_string(),
        ));
    }
    Ok(seconds)
}

fn lease_deadline(now: &str, lease_seconds: i64) -> Result<String, PlanningStoreError> {
    let parsed =
        OffsetDateTime::parse(now, &Rfc3339).map_err(|_| PlanningStoreError::TimeFormat)?;
    (parsed + time::Duration::seconds(lease_seconds))
        .format(&Rfc3339)
        .map_err(|_| PlanningStoreError::TimeFormat)
}

fn require_current_lease(
    record: &ProjectRunRecord,
    fencing_token: Option<i64>,
    now: &str,
) -> Result<(), PlanningStoreError> {
    let supplied = fencing_token.ok_or_else(|| {
        PlanningStoreError::InvalidInput(
            serde_json::json!({
                "code": "PROJECT-RUN-FENCING-TOKEN-REQUIRED",
                "message": "pass the fencingToken returned by project-run claim",
                "projectRunId": record.id,
                "expectedFencingToken": record.fencing_token,
            })
            .to_string(),
        )
    })?;
    if supplied != record.fencing_token {
        return Err(PlanningStoreError::InvalidInput(
            serde_json::json!({
                "code": "PROJECT-RUN-STALE-FENCING-TOKEN",
                "message": "the project run fencing token is stale",
                "projectRunId": record.id,
                "expectedFencingToken": record.fencing_token,
                "actualFencingToken": supplied,
            })
            .to_string(),
        ));
    }
    let lease_expires_at = OffsetDateTime::parse(&record.lease_expires_at, &Rfc3339)
        .map_err(|_| PlanningStoreError::TimeFormat)?;
    let current_time =
        OffsetDateTime::parse(now, &Rfc3339).map_err(|_| PlanningStoreError::TimeFormat)?;
    if lease_expires_at <= current_time {
        return Err(PlanningStoreError::InvalidInput(
            serde_json::json!({
                "code": "PROJECT-RUN-LEASE-EXPIRED",
                "message": "the project run lease has expired",
                "projectRunId": record.id,
                "leaseExpiresAt": record.lease_expires_at,
            })
            .to_string(),
        ));
    }
    Ok(())
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

/// Parse a key=value [AND key=value]* filter string for graph node queries.
/// Returns (WHERE clause fragment, params).
fn parse_graph_node_filter(filter: &str, _scope_key: &str) -> (String, Vec<String>) {
    let mut conditions = Vec::new();
    let mut params = Vec::new();
    for part in filter.split("AND").map(|s| s.trim()) {
        if let Some((key, value)) = part.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "kind" | "status" | "title" => {
                    conditions.push(format!("{key} = ?{}", params.len() + 2));
                    params.push(value.to_string());
                }
                "tag" => {
                    conditions.push(format!("tags_json LIKE ?{}", params.len() + 2));
                    params.push(format!("%\"{value}\"%"));
                }
                _ => {} // ignore unknown keys
            }
        }
    }
    if conditions.is_empty() {
        ("".to_string(), params)
    } else {
        (format!("AND ({})", conditions.join(" AND ")), params)
    }
}

fn require_non_empty(field: &str, value: &str) -> Result<(), PlanningStoreError> {
    if value.trim().is_empty() {
        return Err(PlanningStoreError::InvalidInput(format!(
            "{field} must not be empty"
        )));
    }
    Ok(())
}

fn require_kebab_token(field: &str, value: &str) -> Result<(), PlanningStoreError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(PlanningStoreError::InvalidInput(format!(
            "{field} must not be empty"
        )));
    }
    // Format: [a-z]([a-z0-9]*(-[a-z0-9]+)*)?
    // Must start with a lowercase letter. Rejects empty, uppercase, spaces,
    // leading digits, leading/trailing dashes, and consecutive dashes.
    let is_kebab = trimmed
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && trimmed.starts_with(|c: char| c.is_ascii_lowercase())
        && !trimmed.ends_with('-')
        && !trimmed.contains("--");
    if !is_kebab {
        return Err(PlanningStoreError::InvalidInput(format!(
            "{field} must be a lowercase kebab-case token matching [a-z]([a-z0-9]*(-[a-z0-9]+)*)?, got: '{trimmed}'"
        )));
    }
    Ok(())
}

fn normalize_string_list(values: Vec<String>) -> Vec<String> {
    let mut values: Vec<String> = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    values.sort();
    values.dedup();
    values
}

fn to_json_text<T: Serialize>(value: &T) -> Result<String, PlanningStoreError> {
    serde_json::to_string(value).map_err(PlanningStoreError::from)
}

fn parse_json_column<T: serde::de::DeserializeOwned>(text: String) -> Result<T, rusqlite::Error> {
    serde_json::from_str(&text).map_err(to_sql_error)
}

fn to_sql_error(error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&Row<'_>) -> Result<T, rusqlite::Error>>,
) -> Result<Vec<T>, PlanningStoreError> {
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

fn map_not_found(
    error: rusqlite::Error,
    entity_type: EntityType,
    entity_id: &str,
) -> PlanningStoreError {
    if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
        PlanningStoreError::NotFound {
            entity_type: entity_type.as_str().to_string(),
            entity_id: entity_id.to_string(),
        }
    } else {
        PlanningStoreError::Sqlite(error)
    }
}

fn parse_entity_type(value: String) -> Result<EntityType, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_goal_status(value: String) -> Result<GoalStatus, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_roadmap_status(value: String) -> Result<RoadmapStatus, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_work_point_status(value: String) -> Result<WorkPointStatus, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_work_point_kind(value: String) -> Result<WorkPointKind, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_plan_status(value: String) -> Result<PlanStatus, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_todo_status(value: String) -> Result<TodoStatus, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_issue_status(value: String) -> Result<IssueStatus, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_review_point_status(value: String) -> Result<ReviewPointStatus, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_priority(value: String) -> Result<Priority, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_effort_tier(value: String) -> Result<EffortTier, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_file_scope_selector_type(value: String) -> Result<FileScopeSelectorType, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_file_scope_intent(value: String) -> Result<crate::FileScopeIntent, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_severity(value: String) -> Result<Severity, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_validation_severity(value: String) -> Result<ValidationSeverity, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_planning_node_kind(value: String) -> Result<PlanningNodeKind, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_planning_edge_kind(value: String) -> Result<PlanningEdgeKind, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn text_parse_error(message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )),
    )
}

fn row_to_insight(row: &Row<'_>) -> Result<InsightRecord, rusqlite::Error> {
    Ok(InsightRecord {
        id: row.get(0)?,
        scope_key: row.get(1)?,
        correlation_id: row.get(2)?,
        title: row.get(3)?,
        content: row.get(4)?,
        insight_type: parse_insight_type(row.get::<_, String>(5)?)?,
        parent_entity_type: parse_entity_type(row.get::<_, String>(6)?)?,
        parent_entity_id: row.get(7)?,
        tags: parse_json_column(row.get::<_, String>(8)?)?,
        status: parse_insight_status(row.get::<_, String>(9)?)?,
        revision: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn row_to_project_run(row: &Row<'_>) -> Result<ProjectRunRecord, rusqlite::Error> {
    Ok(ProjectRunRecord {
        id: row.get(0)?,
        scope_key: row.get(1)?,
        goal_id: row.get(2)?,
        roadmap_id: row.get(3)?,
        work_point_id: row.get(4)?,
        repo_id: row.get(5)?,
        branch: row.get(6)?,
        worktree_id: row.get(7)?,
        session_id: row.get(8)?,
        run_id: row.get(9)?,
        profile_id: row.get(10)?,
        owner_id: row.get(11)?,
        idempotency_key: row.get(12)?,
        fencing_token: row.get(13)?,
        lease_expires_at: row.get(14)?,
        heartbeat_at: row.get(15)?,
        status: parse_project_run_status(row.get::<_, String>(16)?)?,
        evidence: parse_json_column(row.get::<_, String>(17)?)?,
        revision: row.get(18)?,
        claimed_at: row.get(19)?,
        completed_at: row.get(20)?,
        created_at: row.get(21)?,
        updated_at: row.get(22)?,
    })
}

fn parse_project_run_status(value: String) -> Result<ProjectRunStatus, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_insight_status(value: String) -> Result<InsightStatus, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_insight_type(value: String) -> Result<InsightType, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_discovery_classification(
    value: String,
) -> Result<DiscoveryClassification, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_verification_state(value: String) -> Result<VerificationState, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_discovery_status(value: String) -> Result<DiscoveryStatus, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn parse_discovery_relationship_kind(
    value: String,
) -> Result<DiscoveryRelationshipKind, rusqlite::Error> {
    value.parse().map_err(text_parse_error)
}

fn row_to_discovery(row: &Row<'_>) -> Result<DiscoveryRecord, rusqlite::Error> {
    let observed_at_json: String = row.get("observed_at_json")?;
    let source_lineage_json: String = row.get("source_lineage_json")?;
    let tags_json: String = row.get("tags_json")?;

    Ok(DiscoveryRecord {
        id: row.get("id")?,
        scope_key: row.get("scope_key")?,
        correlation_id: row.get("correlation_id")?,
        classification: parse_discovery_classification(row.get::<_, String>("classification")?)?,
        verification_state: parse_verification_state(row.get::<_, String>("verification_state")?)?,
        severity: parse_severity(row.get::<_, String>("severity")?)?,
        status: parse_discovery_status(row.get::<_, String>("status")?)?,
        claim: row.get("claim")?,
        impact: row.get("impact")?,
        next_action: row.get("next_action")?,
        verification_step: row.get("verification_step")?,
        recurrence_key: row.get("recurrence_key")?,
        fingerprint: row.get("fingerprint")?,
        observed_at: parse_json_column(observed_at_json)?,
        occurrence_count: row.get("occurrence_count")?,
        source_lineage: parse_json_column(source_lineage_json)?,
        review_date: row.get("review_date")?,
        resolved_at: row.get("resolved_at")?,
        resolution_rationale: row.get("resolution_rationale")?,
        promoted_entity_type: row
            .get::<_, Option<String>>("promoted_entity_type")?
            .map(parse_entity_type)
            .transpose()?,
        promoted_entity_id: row.get("promoted_entity_id")?,
        tags: parse_json_column(tags_json)?,
        revision: row.get("revision")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn row_to_discovery_relationship(
    row: &Row<'_>,
) -> Result<DiscoveryRelationshipRecord, rusqlite::Error> {
    Ok(DiscoveryRelationshipRecord {
        id: row.get("id")?,
        scope_key: row.get("scope_key")?,
        source_id: row.get("source_id")?,
        target_id: row.get("target_id")?,
        relationship_kind: parse_discovery_relationship_kind(
            row.get::<_, String>("relationship_kind")?,
        )?,
        metadata: row
            .get::<_, Option<String>>("metadata_json")?
            .map(|s| serde_json::from_str(&s).map_err(to_sql_error))
            .transpose()?,
        created_at: row.get("created_at")?,
    })
}

fn row_to_discovery_checkpoint(
    row: &Row<'_>,
) -> Result<DiscoveryCheckpointRecord, rusqlite::Error> {
    Ok(DiscoveryCheckpointRecord {
        id: row.get("id")?,
        scope_key: row.get("scope_key")?,
        run_id: row.get("run_id")?,
        event: row.get("event")?,
        snapshot: row
            .get::<_, Option<String>>("snapshot_json")?
            .map(|s| serde_json::from_str(&s).map_err(to_sql_error))
            .transpose()?,
        created_at: row.get("created_at")?,
    })
}

fn list_discovery_relationships_for_source(
    connection: &Connection,
    source_id: &str,
) -> Result<Vec<DiscoveryRelationshipRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, scope_key, source_id, target_id, relationship_kind, metadata_json, created_at FROM discovery_relationships WHERE source_id = ?1 ORDER BY created_at ASC, id ASC",
    )?;
    let rows = statement.query_map(params![source_id], row_to_discovery_relationship)?;
    collect_rows(rows)
}

pub(crate) fn load_discovery(
    connection: &Connection,
    id: &str,
) -> Result<DiscoveryRecord, PlanningStoreError> {
    connection
        .query_row(
            "SELECT id, scope_key, correlation_id, classification, verification_state, severity, status, claim, impact, next_action, verification_step, recurrence_key, fingerprint, observed_at_json, occurrence_count, source_lineage_json, review_date, resolved_at, resolution_rationale, promoted_entity_type, promoted_entity_id, tags_json, revision, created_at, updated_at FROM discovery_nodes WHERE id = ?1",
            params![id],
            row_to_discovery,
        )
        .map_err(|error| map_not_found(error, EntityType::DiscoveryNode, id))
}

pub(crate) fn load_insight(
    connection: &Connection,
    id: &str,
) -> Result<InsightRecord, PlanningStoreError> {
    connection
        .query_row(
            "SELECT id, scope_key, correlation_id, title, content, insight_type, parent_entity_type, parent_entity_id, tags_json, status, revision, created_at, updated_at FROM insights WHERE id = ?1",
            params![id],
            row_to_insight,
        )
        .map_err(|error| map_not_found(error, EntityType::Insight, id))
}

pub(crate) fn load_project_run(
    connection: &Connection,
    id: &str,
) -> Result<ProjectRunRecord, PlanningStoreError> {
    connection
        .query_row(
            "SELECT id, scope_key, goal_id, roadmap_id, work_point_id, repo_id, branch, worktree_id, session_id, run_id, profile_id, owner_id, idempotency_key, fencing_token, lease_expires_at, heartbeat_at, status, evidence_json, revision, claimed_at, completed_at, created_at, updated_at FROM project_runs WHERE id = ?1",
            params![id],
            row_to_project_run,
        )
        .map_err(|error| map_not_found(error, EntityType::ProjectRun, id))
}

pub(crate) fn list_insights_for_entity_in_scope(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
    scope_key: &str,
) -> Result<Vec<InsightRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, scope_key, correlation_id, title, content, insight_type, parent_entity_type, parent_entity_id, tags_json, status, revision, created_at, updated_at FROM insights WHERE parent_entity_type = ?1 AND parent_entity_id = ?2 AND scope_key = ?3 ORDER BY created_at ASC, id ASC",
    )?;
    let rows = statement.query_map(
        params![entity_type.as_str(), entity_id, scope_key],
        row_to_insight,
    )?;
    collect_rows(rows)
}

fn row_to_graph_node(row: &Row<'_>) -> Result<PlanningGraphNode, rusqlite::Error> {
    Ok(PlanningGraphNode {
        id: row.get(0)?,
        scope_key: row.get(1)?,
        kind: parse_planning_node_kind(row.get::<_, String>(2)?)?,
        title: row.get(3)?,
        summary: row.get(4)?,
        status: row.get(5)?,
        payload: serde_json::from_str(&row.get::<_, String>(6)?).map_err(to_sql_error)?,
        tags: parse_json_column(row.get::<_, String>(7)?)?,
        revision: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn row_to_graph_edge(row: &Row<'_>) -> Result<PlanningGraphEdge, rusqlite::Error> {
    Ok(PlanningGraphEdge {
        id: row.get(0)?,
        scope_key: row.get(1)?,
        kind: parse_planning_edge_kind(row.get::<_, String>(2)?)?,
        source_node_id: row.get(3)?,
        target_node_id: row.get(4)?,
        status: row.get(5)?,
        payload: serde_json::from_str(&row.get::<_, String>(6)?).map_err(to_sql_error)?,
        revision: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

pub(crate) fn load_graph_node(
    connection: &Connection,
    id: &str,
) -> Result<PlanningGraphNode, PlanningStoreError> {
    connection
        .query_row(
            "SELECT id, scope_key, kind, title, summary, status, payload_json, tags_json, revision, created_at, updated_at FROM planning_nodes WHERE id = ?1",
            params![id],
            row_to_graph_node,
        )
        .map_err(|error| {
            if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
                PlanningStoreError::NotFound {
                    entity_type: "graph-node".to_string(),
                    entity_id: id.to_string(),
                }
            } else {
                PlanningStoreError::Sqlite(error)
            }
        })
}

pub(crate) fn load_graph_edge(
    connection: &Connection,
    edge_id: &str,
) -> Result<PlanningGraphEdge, PlanningStoreError> {
    connection
        .query_row(
            "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE id = ?1",
            params![edge_id],
            row_to_graph_edge,
        )
        .map_err(|error| map_not_found(error, EntityType::GraphEdge, edge_id))
}

pub(crate) fn list_incoming_edges_for_node(
    connection: &Connection,
    node_id: &str,
    kind: Option<PlanningEdgeKind>,
) -> Result<Vec<PlanningGraphEdge>, PlanningStoreError> {
    if let Some(k) = kind {
        let mut stmt = connection.prepare(
            "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE target_node_id = ?1 AND kind = ?2 ORDER BY updated_at DESC, id ASC"
        )?;
        let rows = stmt.query_map(params![node_id, k.as_str()], row_to_graph_edge)?;
        collect_rows(rows)
    } else {
        let mut stmt = connection.prepare(
            "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE target_node_id = ?1 ORDER BY updated_at DESC, id ASC"
        )?;
        let rows = stmt.query_map(params![node_id], row_to_graph_edge)?;
        collect_rows(rows)
    }
}

pub(crate) fn list_outgoing_edges_for_node(
    connection: &Connection,
    node_id: &str,
    kind: Option<PlanningEdgeKind>,
) -> Result<Vec<PlanningGraphEdge>, PlanningStoreError> {
    if let Some(k) = kind {
        let mut stmt = connection.prepare(
            "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE source_node_id = ?1 AND kind = ?2 ORDER BY updated_at DESC, id ASC"
        )?;
        let rows = stmt.query_map(params![node_id, k.as_str()], row_to_graph_edge)?;
        collect_rows(rows)
    } else {
        let mut stmt = connection.prepare(
            "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE source_node_id = ?1 ORDER BY updated_at DESC, id ASC"
        )?;
        let rows = stmt.query_map(params![node_id], row_to_graph_edge)?;
        collect_rows(rows)
    }
}

pub(crate) fn list_incoming_edges_in_scope(
    connection: &Connection,
    node_id: &str,
    scope_key: &str,
    kind: Option<PlanningEdgeKind>,
) -> Result<Vec<PlanningGraphEdge>, PlanningStoreError> {
    let edges = list_incoming_edges_for_node(connection, node_id, kind)?;
    Ok(edges
        .into_iter()
        .filter(|e| e.scope_key == scope_key)
        .collect())
}

pub(crate) fn list_outgoing_edges_in_scope(
    connection: &Connection,
    node_id: &str,
    scope_key: &str,
    kind: Option<PlanningEdgeKind>,
) -> Result<Vec<PlanningGraphEdge>, PlanningStoreError> {
    let edges = list_outgoing_edges_for_node(connection, node_id, kind)?;
    Ok(edges
        .into_iter()
        .filter(|e| e.scope_key == scope_key)
        .collect())
}

// ── Graph edge preflight helpers ──────────────────────────────────────────

pub(crate) fn validate_edge_kind_pair(
    edge_kind: &PlanningEdgeKind,
    source_kind: &PlanningNodeKind,
    target_kind: &PlanningNodeKind,
) -> Result<(), PlanningStoreError> {
    use PlanningEdgeKind::*;
    use PlanningNodeKind::*;

    match edge_kind {
        DecomposesTo => match (source_kind, target_kind) {
            (Goal | Roadmap | Milestone | Work | Plan | Run, Roadmap | Milestone | Work | Task) => {
                Ok(())
            }
            _ => Err(kind_pair_error(edge_kind, source_kind, target_kind)),
        },
        DependsOn => match (source_kind, target_kind) {
            (Work | Task, Work | Task) => Ok(()),
            _ => Err(kind_pair_error(edge_kind, source_kind, target_kind)),
        },
        Blocks => match (source_kind, target_kind) {
            (Work | Issue | Review, Work | Task | Acceptance) => Ok(()),
            _ => Err(kind_pair_error(edge_kind, source_kind, target_kind)),
        },
        ParallelSafeWith => match (source_kind, target_kind) {
            (Work, Work) => Ok(()),
            _ => Err(kind_pair_error(edge_kind, source_kind, target_kind)),
        },
        PlannedBy => match (source_kind, target_kind) {
            (Work, Plan) => Ok(()),
            _ => Err(kind_pair_error(edge_kind, source_kind, target_kind)),
        },
        ExecutedBy => match (source_kind, target_kind) {
            (Work | Plan, Run) => Ok(()),
            _ => Err(kind_pair_error(edge_kind, source_kind, target_kind)),
        },
        Contains => match (source_kind, target_kind) {
            (Run | Plan, Task | Evidence | Issue | Review | Insight) => Ok(()),
            _ => Err(kind_pair_error(edge_kind, source_kind, target_kind)),
        },
        Requires => match (source_kind, target_kind) {
            (Goal | Roadmap | Milestone | Work | Plan, Acceptance) => Ok(()),
            _ => Err(kind_pair_error(edge_kind, source_kind, target_kind)),
        },
        Satisfies => match (source_kind, target_kind) {
            (Acceptance, Acceptance) => Ok(()),
            _ => Err(kind_pair_error(edge_kind, source_kind, target_kind)),
        },
        EvidencedBy => match (source_kind, target_kind) {
            (Acceptance | Work | Plan | Run | Issue | Review, Evidence) => Ok(()),
            _ => Err(kind_pair_error(edge_kind, source_kind, target_kind)),
        },
        Found => match (source_kind, target_kind) {
            (Run | Work | Plan, Issue | Review) => Ok(()),
            _ => Err(kind_pair_error(edge_kind, source_kind, target_kind)),
        },
        AddressedBy => match (source_kind, target_kind) {
            (Issue | Review, Work | Plan) => Ok(()),
            _ => Err(kind_pair_error(edge_kind, source_kind, target_kind)),
        },
        Repairs => match (source_kind, target_kind) {
            (Work, Work) => Ok(()),
            _ => Err(kind_pair_error(edge_kind, source_kind, target_kind)),
        },
        Supersedes => match (source_kind, target_kind) {
            (Work | Plan | Acceptance, Work | Plan | Acceptance) => Ok(()),
            _ => Err(kind_pair_error(edge_kind, source_kind, target_kind)),
        },
    }
}

fn kind_pair_error(
    edge_kind: &PlanningEdgeKind,
    source_kind: &PlanningNodeKind,
    target_kind: &PlanningNodeKind,
) -> PlanningStoreError {
    PlanningStoreError::InvalidInput(format!(
        "invalid edge: {} edge cannot connect source kind `{}` to target kind `{}`",
        edge_kind.as_str(),
        source_kind.as_str(),
        target_kind.as_str(),
    ))
}

pub(crate) fn would_create_graph_cycle(
    connection: &Connection,
    source_node_id: &str,
    target_node_id: &str,
    edge_kind: &PlanningEdgeKind,
) -> Result<bool, PlanningStoreError> {
    let mut visited = std::collections::HashSet::new();
    walk_graph_edges(
        connection,
        target_node_id,
        source_node_id,
        edge_kind,
        &mut visited,
    )
}

fn walk_graph_edges(
    connection: &Connection,
    current: &str,
    target: &str,
    edge_kind: &PlanningEdgeKind,
    visited: &mut std::collections::HashSet<String>,
) -> Result<bool, PlanningStoreError> {
    if current == target {
        return Ok(true);
    }
    if !visited.insert(current.to_string()) {
        return Ok(false);
    }
    let kind_str = edge_kind.as_str();
    let mut stmt = connection.prepare(
        "SELECT target_node_id FROM planning_edges WHERE source_node_id = ?1 AND kind = ?2 AND status = 'active'"
    )?;
    let rows = stmt.query_map(params![current, kind_str], |row| row.get::<_, String>(0))?;
    for row in rows {
        let next = row?;
        if walk_graph_edges(connection, &next, target, edge_kind, visited)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn rebuild_tag_index_for_entity(
    connection: &Transaction<'_>,
    entity_type: EntityType,
    entity_id: &str,
    tags: &[String],
) -> Result<(), PlanningStoreError> {
    connection.execute(
        "DELETE FROM tag_index WHERE entity_type = ?1 AND entity_id = ?2",
        params![entity_type.as_str(), entity_id],
    )?;
    let scope_key: String = scope_key_for_entity(connection, entity_type, entity_id)?;
    for tag in tags {
        connection.execute(
            "INSERT OR IGNORE INTO tag_index (scope_key, entity_type, entity_id, tag) VALUES (?1, ?2, ?3, ?4)",
            params![scope_key, entity_type.as_str(), entity_id, tag],
        )?;
    }
    Ok(())
}

fn upsert_fts_entry(
    connection: &Transaction<'_>,
    fts_table: &str,
    row_id: &str,
    title: &str,
    content: &str,
) -> Result<(), PlanningStoreError> {
    connection.execute(
        &format!("DELETE FROM {fts_table} WHERE entity_id = ?1"),
        params![row_id],
    )?;
    connection.execute(
        &format!("INSERT INTO {fts_table} (entity_id, title, content) VALUES (?1, ?2, ?3)"),
        params![row_id, title, content],
    )?;
    Ok(())
}

fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

fn resolve_search_result(
    connection: &Connection,
    entity_type_str: &str,
    entity_id: &str,
) -> Result<String, PlanningStoreError> {
    let title = match entity_type_str {
        "goal" => load_goal(connection, entity_id).map(|r| r.title),
        "roadmap" => load_roadmap(connection, entity_id).map(|r| r.title),
        "roadmap-section" => connection
            .query_row(
                "SELECT title FROM roadmap_sections WHERE id = ?1",
                params![entity_id],
                |row| row.get(0),
            )
            .map_err(|e| map_not_found(e, EntityType::RoadmapSection, entity_id)),
        "work-point" => load_work_point(connection, entity_id).map(|r| r.title),
        "plan" => load_plan(connection, entity_id).map(|r| r.title),
        "todo" => load_todo(connection, entity_id).map(|r| r.title),
        "issue" => load_issue(connection, entity_id).map(|r| r.title),
        "review-point" => load_review_point(connection, entity_id).map(|r| r.title),
        "insight" => load_insight(connection, entity_id).map(|r| r.title),
        "scope" => load_scope(connection, entity_id).map(|r| r.scope_key),
        "discovery-node" => load_discovery(connection, entity_id).map(|r| r.claim),
        _ => Ok(String::new()),
    };
    title.or_else(|_| Ok(String::new()))
}

fn search_entity_fts(
    connection: &Connection,
    fts_table: &str,
    query: &str,
) -> Result<Option<Vec<String>>, PlanningStoreError> {
    let fts_query = format!("\"{query}\"");
    let mut statement = connection.prepare(&format!(
        "SELECT entity_id FROM {fts_table} WHERE {fts_table} MATCH ?1"
    ))?;
    let rows = statement.query_map(params![fts_query], |row| row.get::<_, String>(0))?;
    let mut ids = Vec::new();
    for row in rows {
        ids.push(row?);
    }
    Ok(Some(ids))
}

fn search_entity(
    connection: &Connection,
    table: &str,
    entity_type_label: &str,
    input: &SearchInput,
) -> Result<Vec<crate::SearchResult>, PlanningStoreError> {
    let scope_key = normalized_scope_key(input.scope_key.clone());
    let mut sql = format!(
        "SELECT id, title, status, updated_at, created_at FROM {table} WHERE scope_key = ?1"
    );
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(scope_key.clone())];
    let mut param_index = 2;

    if let Some(title) = &input.title {
        sql.push_str(&format!(" AND title LIKE ?{param_index}"));
        param_values.push(Box::new(format!("%{title}%")));
        param_index += 1;
    }
    if let Some(status) = &input.status {
        sql.push_str(&format!(" AND status = ?{param_index}"));
        param_values.push(Box::new(status.clone()));
        param_index += 1;
    }
    if let Some(since) = &input.since {
        sql.push_str(&format!(" AND updated_at >= ?{param_index}"));
        param_values.push(Box::new(since.clone()));
        param_index += 1;
    }
    if let Some(tag) = &input.tag {
        sql.push_str(&format!(
            " AND id IN (SELECT entity_id FROM tag_index WHERE scope_key = ?1 AND tag = ?{param_index})"
        ));
        param_values.push(Box::new(tag.clone()));
        param_index += 1;
    }
    if let Some(fts) = &input.fts {
        let fts_table = format!("{table}_fts");
        if let Some(rowids) = search_entity_fts(connection, &fts_table, fts)? {
            if rowids.is_empty() {
                return Ok(Vec::new());
            }
            let placeholders = rowids
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", param_index + i))
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&format!(" AND id IN ({placeholders})"));
            for rowid in &rowids {
                param_values.push(Box::new(rowid.clone()));
            }
            param_index += rowids.len();
        }
    }

    let _ = param_index;
    sql.push_str(" ORDER BY updated_at DESC, id ASC");
    if let Some(limit) = input.latest {
        sql.push_str(&format!(" LIMIT {limit}"));
    }

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(param_values), |row| {
        Ok(crate::SearchResult {
            entity_type: entity_type_label.to_string(),
            id: row.get(0)?,
            title: row.get(1)?,
            status: row.get(2)?,
            updated_at: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    collect_rows(rows)
}

fn load_entity_json(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
) -> Result<Value, PlanningStoreError> {
    let json_str = match entity_type {
        EntityType::Goal => serde_json::to_string(&load_goal(connection, entity_id)?),
        EntityType::Roadmap => serde_json::to_string(&load_roadmap(connection, entity_id)?),
        EntityType::WorkPoint => serde_json::to_string(&load_work_point(connection, entity_id)?),
        EntityType::Plan => serde_json::to_string(&load_plan(connection, entity_id)?),
        EntityType::Todo => serde_json::to_string(&load_todo(connection, entity_id)?),
        EntityType::Issue => serde_json::to_string(&load_issue(connection, entity_id)?),
        EntityType::ReviewPoint => {
            serde_json::to_string(&load_review_point(connection, entity_id)?)
        }
        EntityType::Insight => serde_json::to_string(&load_insight(connection, entity_id)?),
        EntityType::Scope => serde_json::to_string(&load_scope(connection, entity_id)?),
        EntityType::RoadmapSection => {
            let section = connection
                .query_row(
                    "SELECT id, scope_key, roadmap_id, slug, title, summary, ordering_index, revision, created_at, updated_at FROM roadmap_sections WHERE id = ?1",
                    params![entity_id],
                    row_to_section,
                )
                .map_err(|e| map_not_found(e, EntityType::RoadmapSection, entity_id))?;
            serde_json::to_string(&section)
        }
        EntityType::ProjectRun => serde_json::to_string(&load_project_run(connection, entity_id)?),
        EntityType::GraphNode => serde_json::to_string(&load_graph_node(connection, entity_id)?),
        EntityType::GraphEdge => {
            let edge = connection
                .query_row(
                    "SELECT id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at FROM planning_edges WHERE id = ?1",
                    params![entity_id],
                    row_to_graph_edge,
                )
                .map_err(|e| map_not_found(e, EntityType::GraphEdge, entity_id))?;
            serde_json::to_string(&edge)
        }
        EntityType::DiscoveryNode => serde_json::to_string(&load_discovery(connection, entity_id)?),
        EntityType::DiscoveryRelationship => {
            let rel = connection
                .query_row(
                    "SELECT id, scope_key, source_id, target_id, relationship_kind, metadata_json, created_at FROM discovery_relationships WHERE id = ?1",
                    params![entity_id],
                    row_to_discovery_relationship,
                )
                .map_err(|e| map_not_found(e, EntityType::DiscoveryRelationship, entity_id))?;
            serde_json::to_string(&rel)
        }
        EntityType::DiscoveryCheckpoint => {
            let cp = connection
                .query_row(
                    "SELECT id, scope_key, run_id, event, snapshot_json, created_at FROM discovery_checkpoints WHERE id = ?1",
                    params![entity_id],
                    row_to_discovery_checkpoint,
                )
                .map_err(|e| map_not_found(e, EntityType::DiscoveryCheckpoint, entity_id))?;
            serde_json::to_string(&cp)
        }
    };
    let text = json_str.map_err(PlanningStoreError::from)?;
    serde_json::from_str(&text).map_err(PlanningStoreError::from)
}

fn load_parent_summary(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
) -> Result<Option<Value>, PlanningStoreError> {
    let parent_id = match entity_type {
        EntityType::Roadmap => Some(load_goal(connection, entity_id)?.id),
        EntityType::Plan => Some(load_plan(connection, entity_id)?.goal_id),
        EntityType::Todo => load_todo(connection, entity_id)?.plan_id,
        EntityType::RoadmapSection => {
            let roadmap_id: String = connection
                .query_row(
                    "SELECT roadmap_id FROM roadmap_sections WHERE id = ?1",
                    params![entity_id],
                    |row| row.get(0),
                )
                .map_err(|e| map_not_found(e, EntityType::RoadmapSection, entity_id))?;
            Some(roadmap_id)
        }
        _ => None,
    };
    match parent_id {
        Some(pid) => {
            let parent_type = match entity_type {
                EntityType::Roadmap => EntityType::Goal,
                EntityType::Plan => EntityType::Goal,
                EntityType::Todo => EntityType::Plan,
                EntityType::RoadmapSection => EntityType::Roadmap,
                _ => return Ok(None),
            };
            load_entity_json(connection, parent_type, &pid).map(Some)
        }
        None => Ok(None),
    }
}

fn load_children_json(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
    scope_key: &str,
) -> Result<Vec<Value>, PlanningStoreError> {
    let children = match entity_type {
        EntityType::Goal => {
            let mut statement = connection.prepare(
                "SELECT id, scope_key, goal_id, correlation_id, title, summary, status, tags_json, revision, created_at, updated_at FROM roadmaps WHERE goal_id = ?1 ORDER BY updated_at DESC, id ASC",
            )?;
            let rows = statement.query_map(params![entity_id], row_to_roadmap)?;
            let items: Vec<RoadmapRecord> = collect_rows(rows)?;
            items
                .into_iter()
                .map(|r| serde_json::to_value(&r))
                .collect::<Result<Vec<_>, _>>()?
        }
        EntityType::Roadmap => {
            let items = list_work_points_for_roadmap(connection, entity_id)?;
            items
                .into_iter()
                .map(|r| serde_json::to_value(&r))
                .collect::<Result<Vec<_>, _>>()?
        }
        EntityType::Plan => {
            let items = list_todos_for_plan(connection, entity_id)?;
            items
                .into_iter()
                .map(|r| serde_json::to_value(&r))
                .collect::<Result<Vec<_>, _>>()?
        }
        EntityType::GraphNode => {
            let incoming = list_incoming_edges_in_scope(connection, entity_id, scope_key, None)?;
            let outgoing = list_outgoing_edges_in_scope(connection, entity_id, scope_key, None)?;
            let connected = load_connected_graph_node_summaries(connection, &incoming, &outgoing)?;
            let children: Vec<Value> = vec![
                serde_json::to_value(&incoming)?,
                serde_json::to_value(&outgoing)?,
                serde_json::to_value(&connected)?,
            ];
            children
        }
        EntityType::GraphEdge => {
            let edge = load_graph_edge(connection, entity_id)?;
            let source = load_graph_node_summary(connection, &edge.source_node_id);
            let target = load_graph_node_summary(connection, &edge.target_node_id);
            vec![source.unwrap_or_default(), target.unwrap_or_default()]
        }
        _ => Vec::new(),
    };
    Ok(children.into_iter().filter(|v| !v.is_null()).collect())
}

fn load_graph_node_summary(
    connection: &Connection,
    node_id: &str,
) -> Result<Value, PlanningStoreError> {
    let node = load_graph_node(connection, node_id)?;
    Ok(serde_json::json!({
        "id": node.id,
        "kind": node.kind.as_str(),
        "title": node.title,
        "status": node.status,
        "scope_key": node.scope_key,
    }))
}

fn load_connected_graph_node_summaries(
    connection: &Connection,
    incoming: &[PlanningGraphEdge],
    outgoing: &[PlanningGraphEdge],
) -> Result<Vec<Value>, PlanningStoreError> {
    let mut node_ids = std::collections::HashSet::new();
    for edge in incoming {
        node_ids.insert(edge.source_node_id.clone());
    }
    for edge in outgoing {
        node_ids.insert(edge.target_node_id.clone());
    }
    let mut summaries = Vec::new();
    for node_id in node_ids {
        if let Ok(summary) = load_graph_node_summary(connection, &node_id) {
            summaries.push(summary);
        }
    }
    // Sort by id for deterministic output
    summaries.sort_by(|a, b| {
        a["id"]
            .as_str()
            .unwrap_or("")
            .cmp(b["id"].as_str().unwrap_or(""))
    });
    Ok(summaries)
}

fn load_entity_tags(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
) -> Result<Vec<String>, PlanningStoreError> {
    let table = match entity_type {
        EntityType::Goal => "goals",
        EntityType::Roadmap => "roadmaps",
        EntityType::WorkPoint => "work_points",
        EntityType::Plan => "plans",
        EntityType::Todo => "todos",
        EntityType::Issue => "issues",
        EntityType::Insight => "insights",
        EntityType::Scope => "scopes",
        EntityType::GraphNode => "planning_nodes",
        _ => return Ok(Vec::new()),
    };
    let sql = format!("SELECT tags_json FROM {table} WHERE id = ?1");
    let tags_json: String = connection
        .query_row(&sql, params![entity_id], |row| row.get(0))
        .map_err(|e| map_not_found(e, entity_type, entity_id))?;
    parse_json_column(tags_json).map_err(PlanningStoreError::from)
}

fn render_insight_markdown(view: &InsightView) -> String {
    let mut text = String::new();
    text.push_str(&format!("# {}\n\n", view.insight.title));
    text.push_str(&format!("Insight ID: `{}`\n", view.insight.id));
    text.push_str(&format!("Type: `{}`\n", view.insight.insight_type));
    text.push_str(&format!("Status: `{}`\n", view.insight.status));
    text.push_str(&format!(
        "Parent: {} `{}`\n\n",
        view.insight.parent_entity_type, view.insight.parent_entity_id
    ));
    text.push_str("## Content\n\n");
    text.push_str(&view.insight.content);
    text.push_str("\n\n## Tags\n\n");
    if view.insight.tags.is_empty() {
        text.push_str("No tags.\n");
    } else {
        for tag in &view.insight.tags {
            text.push_str(&format!("- `{tag}`\n"));
        }
    }
    text.push_str("\n## Validation\n\n");
    append_validation(&mut text, &view.validation);
    text
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Barrier},
        thread,
        time::Duration as StdDuration,
    };

    use rusqlite::params;
    use tempfile::tempdir;

    use super::*;
    use crate::ValidationStatus;

    fn create_lease_fixture(store: &PlanningStore) {
        store
            .create_goal(CreateGoalInput {
                id: Some("lease-goal".to_string()),
                scope_key: None,
                correlation_id: "lease-correlation".to_string(),
                title: "Lease goal".to_string(),
                description: "Lease test fixture.".to_string(),
                acceptance_criteria: vec!["lease is exclusive".to_string()],
                rejection_criteria: vec!["duplicate owner".to_string()],
                status: GoalStatus::Active,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create lease goal");
        store
            .create_roadmap(CreateRoadmapInput {
                id: Some("lease-roadmap".to_string()),
                scope_key: None,
                goal_id: "lease-goal".to_string(),
                correlation_id: "lease-correlation".to_string(),
                title: "Lease roadmap".to_string(),
                summary: "Lease test fixture.".to_string(),
                status: RoadmapStatus::Active,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create lease roadmap");
        store
            .add_work_point(AddWorkPointInput {
                id: Some("lease-work-point".to_string()),
                scope_key: None,
                roadmap_id: "lease-roadmap".to_string(),
                section_id: None,
                title: "Lease work point".to_string(),
                summary: "Lease test fixture.".to_string(),
                status: WorkPointStatus::Proposed,
                ordering: Some(1),
                dependency_ids: Vec::new(),
                validation_expectations: vec!["exclusive claim".to_string()],
                effort_tier: EffortTier::Fast,
                kind: Some(WorkPointKind::Feature),
                priority: Some(Priority::Medium),
                repairs_work_point_ids: Vec::new(),
                supersedes_work_point_ids: Vec::new(),
                blocks_work_point_ids: Vec::new(),
                file_scopes: Vec::new(),
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create lease work point");
    }

    fn lease_claim(
        id: &str,
        owner_id: &str,
        idempotency_key: &str,
        lease_seconds: i64,
    ) -> ClaimProjectRunInput {
        ClaimProjectRunInput {
            id: Some(id.to_string()),
            scope_key: None,
            goal_id: "lease-goal".to_string(),
            roadmap_id: "lease-roadmap".to_string(),
            work_point_id: "lease-work-point".to_string(),
            repo_id: Some("lease-repo".to_string()),
            branch: Some("lease-branch".to_string()),
            worktree_id: Some("lease-worktree".to_string()),
            session_id: Some(owner_id.to_string()),
            run_id: Some(format!("run-{id}")),
            profile_id: None,
            correlation_id: Some("lease-correlation".to_string()),
            owner_id: Some(owner_id.to_string()),
            idempotency_key: Some(idempotency_key.to_string()),
            lease_seconds: Some(lease_seconds),
        }
    }

    #[derive(Clone, Debug)]
    struct ScopedFixtureIds {
        goal_id: String,
        roadmap_id: String,
        work_point_id: String,
        plan_id: String,
        todo_id: String,
        issue_id: String,
        review_point_id: String,
    }

    fn ensure_scope(store: &PlanningStore, scope_key: &str) {
        if scope_key == DEFAULT_SCOPE_KEY {
            return;
        }

        store
            .create_scope(CreateScopeInput {
                scope_key: scope_key.to_string(),
                scope_type: Some("workspace".to_string()),
                parent_scope_key: None,
                metadata: None,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create scope");
    }

    fn seed_scoped_fixture(
        store: &PlanningStore,
        scope_key: &str,
        prefix: &str,
    ) -> ScopedFixtureIds {
        let goal_id = format!("{prefix}-goal");
        let roadmap_id = format!("{prefix}-roadmap");
        let work_point_id = format!("{prefix}-work-point");
        let plan_id = format!("{prefix}-plan");
        let todo_id = format!("{prefix}-todo");
        let issue_id = format!("{prefix}-issue");
        let review_point_id = format!("{prefix}-review");

        store
            .create_goal(CreateGoalInput {
                id: Some(goal_id.clone()),
                scope_key: Some(scope_key.to_string()),
                correlation_id: format!("corr-{prefix}"),
                title: format!("{prefix} goal"),
                description: "Scoped goal".to_string(),
                acceptance_criteria: vec!["ok".to_string()],
                rejection_criteria: vec!["no".to_string()],
                status: GoalStatus::Active,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create scoped goal");
        store
            .create_roadmap(CreateRoadmapInput {
                id: Some(roadmap_id.clone()),
                scope_key: Some(scope_key.to_string()),
                goal_id: goal_id.clone(),
                correlation_id: format!("corr-{prefix}"),
                title: format!("{prefix} roadmap"),
                summary: "Scoped roadmap".to_string(),
                status: RoadmapStatus::Active,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create scoped roadmap");
        store
            .add_work_point(AddWorkPointInput {
                id: Some(work_point_id.clone()),
                scope_key: Some(scope_key.to_string()),
                roadmap_id: roadmap_id.clone(),
                section_id: None,
                title: format!("{prefix} work point"),
                summary: "Scoped work point".to_string(),
                status: WorkPointStatus::Active,
                ordering: None,
                dependency_ids: Vec::new(),
                validation_expectations: vec!["proof".to_string()],
                effort_tier: crate::EffortTier::Balanced,
                kind: None,
                priority: None,
                repairs_work_point_ids: Vec::new(),
                supersedes_work_point_ids: Vec::new(),
                blocks_work_point_ids: Vec::new(),
                file_scopes: Vec::new(),
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create scoped work point");
        store
            .create_plan(CreatePlanInput {
                id: Some(plan_id.clone()),
                scope_key: Some(scope_key.to_string()),
                goal_id: goal_id.clone(),
                roadmap_id: roadmap_id.clone(),
                correlation_id: format!("corr-{prefix}"),
                title: format!("{prefix} plan"),
                summary: "Scoped plan".to_string(),
                scope: "implementation".to_string(),
                assumptions: vec!["a1".to_string()],
                stop_conditions: Vec::new(),
                validation_steps: vec!["validate".to_string()],
                targeted_work_point_ids: vec![work_point_id.clone()],
                effort_tier: crate::EffortTier::Balanced,
                routing_hint: None,
                allow_parallel_overlap: false,
                file_scopes: Vec::new(),
                status: PlanStatus::Active,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create scoped plan");
        store
            .create_todo(CreateTodoInput {
                id: Some(todo_id.clone()),
                scope_key: Some(scope_key.to_string()),
                plan_id: Some(plan_id.clone()),
                work_point_id: Some(work_point_id.clone()),
                title: format!("{prefix} todo"),
                summary: "Scoped todo".to_string(),
                status: TodoStatus::InProgress,
                priority: Priority::High,
                effort_tier: crate::EffortTier::Balanced,
                file_scopes: Vec::new(),
                evidence_refs: Vec::new(),
                tags: Vec::new(),
                ordering: None,
                run_id: None,
            })
            .expect("create scoped todo");
        store
            .create_issue(CreateIssueInput {
                id: Some(issue_id.clone()),
                scope_key: Some(scope_key.to_string()),
                correlation_id: format!("corr-{prefix}"),
                title: format!("{prefix} issue"),
                summary: "Scoped issue".to_string(),
                status: IssueStatus::Open,
                severity: Severity::High,
                related_entity_type: Some(EntityType::Plan),
                related_entity_id: Some(plan_id.clone()),
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create scoped issue");
        store
            .create_review_point(CreateReviewPointInput {
                id: Some(review_point_id.clone()),
                scope_key: Some(scope_key.to_string()),
                attached_entity_type: EntityType::Plan,
                attached_entity_id: plan_id.clone(),
                title: format!("{prefix} review"),
                summary: "Scoped review point".to_string(),
                status: ReviewPointStatus::Open,
                severity: Severity::Medium,
                run_id: None,
            })
            .expect("create scoped review point");

        ScopedFixtureIds {
            goal_id,
            roadmap_id,
            work_point_id,
            plan_id,
            todo_id,
            issue_id,
            review_point_id,
        }
    }

    #[test]
    fn store_persists_goal_roadmap_plan_and_validation_findings() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init planning db");

        let goal = store
            .create_goal(CreateGoalInput {
                id: Some("goal-1".to_string()),
                scope_key: None,
                correlation_id: "corr-1".to_string(),
                title: "Ship planning subsystem".to_string(),
                description: "Create a dedicated planning system.".to_string(),
                acceptance_criteria: vec!["CLI exists".to_string()],
                rejection_criteria: vec!["No authority split".to_string()],
                status: GoalStatus::Active,
                tags: vec!["planning".to_string()],
                run_id: Some("run-goal".to_string()),
            })
            .expect("create goal");
        assert_eq!(goal.validation.status, ValidationStatus::Valid);

        let roadmap = store
            .create_roadmap(CreateRoadmapInput {
                id: Some("roadmap-1".to_string()),
                scope_key: None,
                goal_id: "goal-1".to_string(),
                correlation_id: "corr-1".to_string(),
                title: "Planning MVP".to_string(),
                summary: "Build the planning crate.".to_string(),
                status: RoadmapStatus::Active,
                tags: vec!["mvp".to_string()],
                run_id: Some("run-roadmap".to_string()),
            })
            .expect("create roadmap");
        assert_eq!(roadmap.validation.status, ValidationStatus::Warning);
        assert!(roadmap
            .validation
            .findings
            .iter()
            .any(|finding| finding.code == "ROADMAP-NO-WORK-POINTS"));

        store
            .add_work_point(AddWorkPointInput {
                id: Some("wp-1".to_string()),
                scope_key: None,
                roadmap_id: "roadmap-1".to_string(),
                section_id: None,
                title: "Implement store".to_string(),
                summary: "Persist events and projections.".to_string(),
                status: WorkPointStatus::Active,
                ordering: None,
                dependency_ids: Vec::new(),
                validation_expectations: vec!["health command passes".to_string()],
                effort_tier: crate::EffortTier::Balanced,
                kind: None,
                priority: None,
                repairs_work_point_ids: Vec::new(),
                supersedes_work_point_ids: Vec::new(),
                blocks_work_point_ids: Vec::new(),
                file_scopes: Vec::new(),
                tags: Vec::new(),
                run_id: Some("run-wp".to_string()),
            })
            .expect("add work point");

        let roadmap_view = store.roadmap("roadmap-1").expect("roadmap view");
        assert_eq!(roadmap_view.validation.status, ValidationStatus::Valid);

        let plan = store
            .create_plan(CreatePlanInput {
                id: Some("plan-1".to_string()),
                scope_key: None,
                goal_id: "goal-1".to_string(),
                roadmap_id: "roadmap-1".to_string(),
                correlation_id: "corr-1".to_string(),
                title: "Implement MVP".to_string(),
                summary: "Land the first crate version.".to_string(),
                scope: "single implementation pass".to_string(),
                assumptions: Vec::new(),
                stop_conditions: vec!["validation invalid".to_string()],
                validation_steps: vec!["cargo test -p elegy-planning".to_string()],
                targeted_work_point_ids: vec!["wp-1".to_string()],
                effort_tier: crate::EffortTier::Balanced,
                routing_hint: None,
                allow_parallel_overlap: false,
                file_scopes: Vec::new(),
                status: PlanStatus::Active,
                tags: Vec::new(),
                run_id: Some("run-plan".to_string()),
            })
            .expect("create plan");
        assert_eq!(plan.validation.status, ValidationStatus::Warning);
        assert!(plan
            .validation
            .findings
            .iter()
            .any(|finding| finding.code == "PLAN-NO-TODOS"));

        store
            .create_todo(CreateTodoInput {
                id: Some("todo-2".to_string()),
                scope_key: None,
                plan_id: Some("plan-1".to_string()),
                work_point_id: Some("wp-1".to_string()),
                title: "Finish MVP implementation".to_string(),
                summary: "Wire the first shipping slice.".to_string(),
                status: TodoStatus::InProgress,
                priority: Priority::High,
                effort_tier: crate::EffortTier::Balanced,
                file_scopes: Vec::new(),
                evidence_refs: vec!["cargo-test:elegy-planning".to_string()],
                tags: Vec::new(),
                ordering: None,
                run_id: Some("run-todo-plan".to_string()),
            })
            .expect("create plan todo");

        let plan_view = store.plan("plan-1").expect("plan view");
        assert_eq!(plan_view.validation.status, ValidationStatus::Valid);

        let health = store.health().expect("health report");
        assert_eq!(health.goal_count, 1);
        assert_eq!(health.roadmap_count, 1);
        assert_eq!(health.plan_count, 1);
        assert!(health.event_count >= 3);
    }

    #[test]
    fn standalone_todo_is_allowed_but_flagged_with_warning() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init planning db");

        let todo = store
            .create_todo(CreateTodoInput {
                id: Some("todo-1".to_string()),
                scope_key: None,
                plan_id: None,
                work_point_id: None,
                title: "Loose todo".to_string(),
                summary: "Allowed for manual tracking.".to_string(),
                status: TodoStatus::Pending,
                priority: Priority::Medium,
                effort_tier: crate::EffortTier::Balanced,
                file_scopes: Vec::new(),
                evidence_refs: Vec::new(),
                tags: Vec::new(),
                ordering: None,
                run_id: Some("run-todo".to_string()),
            })
            .expect("create todo");

        assert_eq!(todo.validation.status, ValidationStatus::Warning);
        assert!(todo
            .validation
            .findings
            .iter()
            .any(|finding| finding.code == "TODO-STANDALONE"));
    }

    #[test]
    fn validated_goal_refreshes_after_linked_roadmap_is_created() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init planning db");

        let goal = store
            .create_goal(CreateGoalInput {
                id: Some("goal-refresh-1".to_string()),
                scope_key: None,
                correlation_id: "corr-refresh-1".to_string(),
                title: "Validated goal".to_string(),
                description: "Goal starts validated before roadmap exists.".to_string(),
                acceptance_criteria: vec!["criterion".to_string()],
                rejection_criteria: vec!["rejection".to_string()],
                status: GoalStatus::Validated,
                tags: Vec::new(),
                run_id: Some("run-goal-refresh".to_string()),
            })
            .expect("create validated goal");
        assert_eq!(goal.validation.status, ValidationStatus::Warning);
        assert!(goal
            .validation
            .findings
            .iter()
            .any(|finding| finding.code == "GOAL-VALIDATED-WITHOUT-ROADMAP"));

        store
            .create_roadmap(CreateRoadmapInput {
                id: Some("roadmap-refresh-1".to_string()),
                scope_key: None,
                goal_id: "goal-refresh-1".to_string(),
                correlation_id: "corr-refresh-1".to_string(),
                title: "Linked roadmap".to_string(),
                summary: "Now the goal has a roadmap.".to_string(),
                status: RoadmapStatus::Draft,
                tags: Vec::new(),
                run_id: Some("run-roadmap-refresh".to_string()),
            })
            .expect("create roadmap");

        let goal_view = store.goal("goal-refresh-1").expect("goal view");
        assert_eq!(goal_view.validation.status, ValidationStatus::Valid);
    }

    #[test]
    fn plan_refreshes_after_attached_issue_is_recorded() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init planning db");

        store
            .create_goal(CreateGoalInput {
                id: Some("goal-issue-1".to_string()),
                scope_key: None,
                correlation_id: "corr-issue-1".to_string(),
                title: "Goal".to_string(),
                description: "Desc".to_string(),
                acceptance_criteria: vec!["ok".to_string()],
                rejection_criteria: vec!["no".to_string()],
                status: GoalStatus::Active,
                tags: Vec::new(),
                run_id: Some("run-goal-issue".to_string()),
            })
            .expect("create goal");
        store
            .create_roadmap(CreateRoadmapInput {
                id: Some("roadmap-issue-1".to_string()),
                scope_key: None,
                goal_id: "goal-issue-1".to_string(),
                correlation_id: "corr-issue-1".to_string(),
                title: "Roadmap".to_string(),
                summary: "Summary".to_string(),
                status: RoadmapStatus::Draft,
                tags: Vec::new(),
                run_id: Some("run-roadmap-issue".to_string()),
            })
            .expect("create roadmap");
        store
            .add_work_point(AddWorkPointInput {
                id: Some("wp-issue-1".to_string()),
                scope_key: None,
                roadmap_id: "roadmap-issue-1".to_string(),
                section_id: None,
                title: "WP".to_string(),
                summary: "Summary".to_string(),
                status: WorkPointStatus::Draft,
                ordering: None,
                dependency_ids: Vec::new(),
                validation_expectations: vec!["proof".to_string()],
                effort_tier: crate::EffortTier::Balanced,
                kind: None,
                priority: None,
                repairs_work_point_ids: Vec::new(),
                supersedes_work_point_ids: Vec::new(),
                blocks_work_point_ids: Vec::new(),
                file_scopes: Vec::new(),
                tags: Vec::new(),
                run_id: Some("run-wp-issue".to_string()),
            })
            .expect("create work point");
        store
            .create_plan(CreatePlanInput {
                id: Some("plan-issue-1".to_string()),
                scope_key: None,
                goal_id: "goal-issue-1".to_string(),
                roadmap_id: "roadmap-issue-1".to_string(),
                correlation_id: "corr-issue-1".to_string(),
                title: "Plan".to_string(),
                summary: "Summary".to_string(),
                scope: "scope".to_string(),
                assumptions: Vec::new(),
                stop_conditions: Vec::new(),
                validation_steps: vec!["validate".to_string()],
                targeted_work_point_ids: vec!["wp-issue-1".to_string()],
                effort_tier: crate::EffortTier::Balanced,
                routing_hint: None,
                allow_parallel_overlap: false,
                file_scopes: Vec::new(),
                status: PlanStatus::Active,
                tags: Vec::new(),
                run_id: Some("run-plan-issue".to_string()),
            })
            .expect("create plan");
        store
            .create_todo(CreateTodoInput {
                id: Some("todo-issue-1".to_string()),
                scope_key: None,
                plan_id: Some("plan-issue-1".to_string()),
                work_point_id: Some("wp-issue-1".to_string()),
                title: "Todo".to_string(),
                summary: "Summary".to_string(),
                status: TodoStatus::Pending,
                priority: Priority::Medium,
                effort_tier: crate::EffortTier::Balanced,
                file_scopes: Vec::new(),
                evidence_refs: Vec::new(),
                tags: Vec::new(),
                ordering: None,
                run_id: Some("run-todo-issue".to_string()),
            })
            .expect("create todo");

        let plan_before_issue = store.plan("plan-issue-1").expect("plan before issue");
        assert_eq!(plan_before_issue.validation.status, ValidationStatus::Valid);

        store
            .create_issue(CreateIssueInput {
                id: Some("issue-issue-1".to_string()),
                scope_key: None,
                correlation_id: "corr-issue-1".to_string(),
                title: "Blocking issue".to_string(),
                summary: "Blocks plan completion.".to_string(),
                status: IssueStatus::Open,
                severity: Severity::Critical,
                related_entity_type: Some(EntityType::Plan),
                related_entity_id: Some("plan-issue-1".to_string()),
                tags: Vec::new(),
                run_id: Some("run-issue-issue".to_string()),
            })
            .expect("create issue");

        let plan_after_issue = store.plan("plan-issue-1").expect("plan after issue");
        assert_eq!(
            plan_after_issue.validation.status,
            ValidationStatus::Invalid
        );
        assert!(plan_after_issue
            .validation
            .findings
            .iter()
            .any(|finding| finding.code == "PLAN-BLOCKING-ISSUES"));
    }

    #[test]
    fn events_list_uses_append_order_not_stream_local_sequence_order() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init planning db");

        store
            .create_goal(CreateGoalInput {
                id: Some("goal-events-1".to_string()),
                scope_key: None,
                correlation_id: "corr-events-1".to_string(),
                title: "Goal".to_string(),
                description: "Desc".to_string(),
                acceptance_criteria: vec!["ok".to_string()],
                rejection_criteria: vec!["no".to_string()],
                status: GoalStatus::Active,
                tags: Vec::new(),
                run_id: Some("run-events-goal".to_string()),
            })
            .expect("create goal");
        store
            .create_roadmap(CreateRoadmapInput {
                id: Some("roadmap-events-1".to_string()),
                scope_key: None,
                goal_id: "goal-events-1".to_string(),
                correlation_id: "corr-events-1".to_string(),
                title: "Roadmap".to_string(),
                summary: "Summary".to_string(),
                status: RoadmapStatus::Draft,
                tags: Vec::new(),
                run_id: Some("run-events-roadmap".to_string()),
            })
            .expect("create roadmap");
        store
            .create_plan(CreatePlanInput {
                id: Some("plan-events-1".to_string()),
                scope_key: None,
                goal_id: "goal-events-1".to_string(),
                roadmap_id: "roadmap-events-1".to_string(),
                correlation_id: "corr-events-1".to_string(),
                title: "Plan".to_string(),
                summary: "Summary".to_string(),
                scope: "scope".to_string(),
                assumptions: Vec::new(),
                stop_conditions: Vec::new(),
                validation_steps: Vec::new(),
                targeted_work_point_ids: Vec::new(),
                effort_tier: crate::EffortTier::Balanced,
                routing_hint: None,
                allow_parallel_overlap: false,
                file_scopes: Vec::new(),
                status: PlanStatus::Draft,
                tags: Vec::new(),
                run_id: Some("run-events-plan".to_string()),
            })
            .expect("create plan");

        let events = store.list_events().expect("list events");
        let event_types: Vec<&str> = events
            .iter()
            .map(|event| event.event_type.as_str())
            .collect();
        assert_eq!(
            event_types,
            vec!["goal.created", "roadmap.created", "plan.created"]
        );
    }

    #[test]
    fn out_of_scope_update_status_rejects_for_supported_entity_types() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init planning db");
        ensure_scope(&store, "workspace-a");
        let fixture = seed_scoped_fixture(&store, "workspace-a", "scope-status");

        for (entity_type, entity_id, status, evidence_refs) in [
            (EntityType::Goal, fixture.goal_id.clone(), "validated", None),
            (
                EntityType::Roadmap,
                fixture.roadmap_id.clone(),
                "blocked",
                None,
            ),
            (
                EntityType::WorkPoint,
                fixture.work_point_id.clone(),
                "completed",
                None,
            ),
            (EntityType::Plan, fixture.plan_id.clone(), "blocked", None),
            (
                EntityType::Todo,
                fixture.todo_id.clone(),
                "completed",
                Some(vec!["proof://ci".to_string()]),
            ),
            (
                EntityType::Issue,
                fixture.issue_id.clone(),
                "resolved",
                None,
            ),
            (
                EntityType::ReviewPoint,
                fixture.review_point_id.clone(),
                "resolved",
                None,
            ),
        ] {
            let error = store
                .update_status(UpdateStatusInput {
                    entity_type,
                    entity_id: entity_id.clone(),
                    status: status.to_string(),
                    evidence_refs,
                    active_scope_key: Some("default".to_string()),
                    run_id: None,
                    override_transition: false,
                    reason: None,
                })
                .expect_err("out-of-scope update should fail");

            match error {
                PlanningStoreError::InvalidInput(message) => {
                    assert!(message.contains("workspace-a"), "{message}");
                    assert!(message.contains("default"), "{message}");
                    assert!(message.contains(&entity_id), "{message}");
                }
                other => panic!("expected invalid input, got {other:?}"),
            }
        }
    }

    #[test]
    fn revise_plan_rejects_out_of_scope_and_incompatible_scope_transfer() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init planning db");
        ensure_scope(&store, "workspace-a");
        ensure_scope(&store, "workspace-b");
        let fixture = seed_scoped_fixture(&store, "workspace-a", "scope-revise");

        let error = store
            .revise_plan(RevisePlanInput {
                plan_id: fixture.plan_id.clone(),
                active_scope_key: Some("default".to_string()),
                scope_key: None,
                assumptions: None,
                stop_conditions: None,
                validation_steps: None,
                targeted_work_point_ids: None,
                effort_tier: None,
                routing_hint: None,
                clear_routing_hint: false,
                allow_parallel_overlap: None,
                file_scopes: None,
                clear_file_scopes: false,
                tags: None,
                run_id: None,
            })
            .expect_err("out-of-scope revise should fail");
        match error {
            PlanningStoreError::InvalidInput(message) => {
                assert!(message.contains("workspace-a"), "{message}");
                assert!(message.contains("default"), "{message}");
            }
            other => panic!("expected invalid input, got {other:?}"),
        }

        let error = store
            .revise_plan(RevisePlanInput {
                plan_id: fixture.plan_id.clone(),
                active_scope_key: Some("workspace-a".to_string()),
                scope_key: Some("workspace-b".to_string()),
                assumptions: Some(vec!["a1".to_string(), "a2".to_string()]),
                stop_conditions: Some(vec!["stop".to_string()]),
                validation_steps: None,
                targeted_work_point_ids: None,
                effort_tier: None,
                routing_hint: None,
                clear_routing_hint: false,
                allow_parallel_overlap: None,
                file_scopes: None,
                clear_file_scopes: false,
                tags: Some(vec!["transfer".to_string()]),
                run_id: Some("run-transfer".to_string()),
            })
            .expect_err("incompatible scope transfer should fail");

        match error {
            PlanningStoreError::InvalidInput(message) => {
                assert!(
                    message.contains("cannot transfer to scope `workspace-b`"),
                    "{message}"
                );
                assert!(message.contains("workspace-a"), "{message}");
            }
            other => panic!("expected invalid input, got {other:?}"),
        }

        let plan = store
            .plan(&fixture.plan_id)
            .expect("plan remains in source scope");
        assert_eq!(plan.plan.scope_key, "workspace-a");
        let workspace_b_events = store
            .list_events_in_scope("workspace-b")
            .expect("list workspace-b events");
        assert!(!workspace_b_events.iter().any(|event| {
            event.entity_id == fixture.plan_id && event.event_type == "plan.revised"
        }));
    }

    #[test]
    fn revise_plan_can_clear_routing_hint_and_file_scopes() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init planning db");

        store
            .create_goal(CreateGoalInput {
                id: Some("goal-clear".to_string()),
                scope_key: None,
                correlation_id: "corr-clear".to_string(),
                title: "Goal".to_string(),
                description: "Description".to_string(),
                acceptance_criteria: vec!["done".to_string()],
                rejection_criteria: Vec::new(),
                status: GoalStatus::Active,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create goal");

        store
            .create_roadmap(CreateRoadmapInput {
                id: Some("roadmap-clear".to_string()),
                scope_key: None,
                goal_id: "goal-clear".to_string(),
                correlation_id: "corr-clear".to_string(),
                title: "Roadmap".to_string(),
                summary: "Summary".to_string(),
                status: RoadmapStatus::Active,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create roadmap");

        store
            .create_plan(CreatePlanInput {
                id: Some("plan-clear".to_string()),
                scope_key: None,
                goal_id: "goal-clear".to_string(),
                roadmap_id: "roadmap-clear".to_string(),
                correlation_id: "corr-clear".to_string(),
                title: "Plan".to_string(),
                summary: "Summary".to_string(),
                scope: "scope".to_string(),
                assumptions: vec!["a1".to_string()],
                stop_conditions: Vec::new(),
                validation_steps: Vec::new(),
                targeted_work_point_ids: Vec::new(),
                effort_tier: crate::EffortTier::Balanced,
                routing_hint: Some("flash-lane".to_string()),
                allow_parallel_overlap: false,
                file_scopes: vec![crate::FileScopeRecord {
                    selector_type: crate::FileScopeSelectorType::Glob,
                    selector: "plugins/planning/src/**".to_string(),
                    intent: crate::FileScopeIntent::Primary,
                }],
                status: PlanStatus::Active,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create plan");

        let revised = store
            .revise_plan(RevisePlanInput {
                plan_id: "plan-clear".to_string(),
                active_scope_key: None,
                scope_key: None,
                assumptions: None,
                stop_conditions: None,
                validation_steps: None,
                targeted_work_point_ids: None,
                effort_tier: None,
                routing_hint: None,
                clear_routing_hint: true,
                allow_parallel_overlap: None,
                file_scopes: None,
                clear_file_scopes: true,
                tags: None,
                run_id: None,
            })
            .expect("revise plan");

        assert_eq!(revised.record.id, "plan-clear");
        assert!(revised.record.routing_hint.is_none());
        assert!(revised.record.file_scopes.is_empty());
    }

    #[test]
    fn load_file_scopes_for_entities_supports_ids_over_sqlite_variable_limit() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init planning db");

        let connection = store.open_connection().expect("open connection");
        let owner_entity_ids = (0..1200)
            .map(|index| format!("plan-{index}"))
            .collect::<Vec<_>>();

        let grouped =
            load_file_scopes_for_entities(&connection, EntityType::Plan, &owner_entity_ids)
                .expect("load file scopes in chunks");

        assert!(grouped.is_empty());
    }

    #[test]
    fn list_events_in_scope_returns_only_matching_scope_events() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init planning db");
        ensure_scope(&store, "workspace-a");

        store
            .create_goal(CreateGoalInput {
                id: Some("goal-default-events".to_string()),
                scope_key: None,
                correlation_id: "corr-default-events".to_string(),
                title: "Default goal".to_string(),
                description: "Default scope goal".to_string(),
                acceptance_criteria: vec!["ok".to_string()],
                rejection_criteria: vec!["no".to_string()],
                status: GoalStatus::Draft,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create default goal");
        store
            .create_goal(CreateGoalInput {
                id: Some("goal-custom-events".to_string()),
                scope_key: Some("workspace-a".to_string()),
                correlation_id: "corr-custom-events".to_string(),
                title: "Custom goal".to_string(),
                description: "Custom scope goal".to_string(),
                acceptance_criteria: vec!["ok".to_string()],
                rejection_criteria: vec!["no".to_string()],
                status: GoalStatus::Draft,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create custom goal");

        let default_events = store
            .list_events_in_scope("default")
            .expect("list default events");
        let custom_events = store
            .list_events_in_scope("workspace-a")
            .expect("list custom events");

        assert!(default_events
            .iter()
            .any(|event| event.entity_id == "goal-default-events"));
        assert!(!default_events
            .iter()
            .any(|event| event.entity_id == "goal-custom-events"));
        assert!(custom_events
            .iter()
            .any(|event| event.entity_id == "goal-custom-events"));
        assert!(!custom_events
            .iter()
            .any(|event| event.entity_id == "goal-default-events"));
    }

    #[test]
    fn scoped_lists_are_isolated_and_default_scope_is_applied() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init planning db");

        store
            .create_scope(CreateScopeInput {
                scope_key: "workspace-a".to_string(),
                scope_type: Some("workspace".to_string()),
                parent_scope_key: None,
                metadata: None,
                tags: vec!["alpha".to_string()],
                run_id: None,
            })
            .expect("create custom scope");

        store
            .create_goal(CreateGoalInput {
                id: Some("goal-default".to_string()),
                scope_key: None,
                correlation_id: "corr-default".to_string(),
                title: "Default".to_string(),
                description: "Default scope goal".to_string(),
                acceptance_criteria: vec!["ok".to_string()],
                rejection_criteria: vec!["no".to_string()],
                status: GoalStatus::Draft,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create default scope goal");

        store
            .create_goal(CreateGoalInput {
                id: Some("goal-custom".to_string()),
                scope_key: Some("workspace-a".to_string()),
                correlation_id: "corr-custom".to_string(),
                title: "Custom".to_string(),
                description: "Custom scope goal".to_string(),
                acceptance_criteria: vec!["ok".to_string()],
                rejection_criteria: vec!["no".to_string()],
                status: GoalStatus::Draft,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create custom scope goal");

        let default_goals = store
            .list_goals_in_scope("default")
            .expect("list default scope goals");
        let custom_goals = store
            .list_goals_in_scope("workspace-a")
            .expect("list custom scope goals");
        assert_eq!(default_goals.len(), 1);
        assert_eq!(default_goals[0].id, "goal-default");
        assert_eq!(custom_goals.len(), 1);
        assert_eq!(custom_goals[0].id, "goal-custom");
    }

    #[test]
    fn add_work_point_rejects_missing_block_target() {
        let temp = tempdir().expect("temp dir");
        let store = PlanningStore::new(temp.path().join("planning.db"));
        store.init().expect("init store");
        ensure_scope(&store, "workspace-a");
        let fixture = seed_scoped_fixture(&store, "workspace-a", "block-target");

        let error = store
            .add_work_point(AddWorkPointInput {
                id: Some("block-target-blocker".to_string()),
                scope_key: Some("workspace-a".to_string()),
                roadmap_id: fixture.roadmap_id,
                section_id: None,
                title: "Blocker".to_string(),
                summary: "Attempts to block a missing work point".to_string(),
                status: WorkPointStatus::Draft,
                ordering: None,
                dependency_ids: Vec::new(),
                validation_expectations: vec!["missing target rejected".to_string()],
                effort_tier: crate::EffortTier::Balanced,
                kind: Some(WorkPointKind::Corrective),
                priority: Some(Priority::High),
                repairs_work_point_ids: Vec::new(),
                supersedes_work_point_ids: Vec::new(),
                blocks_work_point_ids: vec!["missing-block-target".to_string()],
                file_scopes: Vec::new(),
                tags: Vec::new(),
                run_id: None,
            })
            .expect_err("missing block target should be rejected");

        let message = error.to_string();
        assert!(
            message.contains("blocked work point `missing-block-target` does not exist"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn init_migrates_v1_schema_and_assigns_default_scope() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning-v1.db");

        {
            let connection = rusqlite::Connection::open(&db_path).expect("open sqlite");
            connection
                .execute_batch(
                    r#"
                    CREATE TABLE planning_config (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                    INSERT INTO planning_config (key, value) VALUES ('schema_version', '1');
                    CREATE TABLE goals (
                        id TEXT PRIMARY KEY,
                        correlation_id TEXT NOT NULL,
                        title TEXT NOT NULL,
                        description TEXT NOT NULL,
                        acceptance_criteria_json TEXT NOT NULL,
                        rejection_criteria_json TEXT NOT NULL,
                        status TEXT NOT NULL,
                        tags_json TEXT NOT NULL,
                        revision INTEGER NOT NULL,
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL
                    );
                    "#,
                )
                .expect("create v1 schema");
            connection
                .execute(
                    r#"
                    INSERT INTO goals (
                        id, correlation_id, title, description, acceptance_criteria_json,
                        rejection_criteria_json, status, tags_json, revision, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9)
                    "#,
                    params![
                        "goal-v1",
                        "corr-v1",
                        "Goal v1",
                        "Goal created before scope migration",
                        "[\"ok\"]",
                        "[\"no\"]",
                        "draft",
                        "[]",
                        "2026-05-24T00:00:00Z"
                    ],
                )
                .expect("insert v1 goal");
        }

        let store = PlanningStore::new(&db_path);
        store.init().expect("migrate schema");

        let goals = store.list_goals().expect("list goals");
        assert_eq!(goals.len(), 1);
        assert_eq!(goals[0].scope_key, "default");
        let health = store.health().expect("health");
        assert_eq!(health.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(health.scope_count, 1);
    }

    #[test]
    fn init_migrates_v2_events_and_backfills_scope_keys() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning-v2.db");

        {
            let connection = rusqlite::Connection::open(&db_path).expect("open sqlite");
            connection
                .execute_batch(
                    r#"
                    CREATE TABLE planning_config (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                    INSERT INTO planning_config (key, value) VALUES ('schema_version', '2');
                    CREATE TABLE goals (
                        id TEXT PRIMARY KEY,
                        scope_key TEXT NOT NULL,
                        correlation_id TEXT NOT NULL,
                        title TEXT NOT NULL,
                        description TEXT NOT NULL,
                        acceptance_criteria_json TEXT NOT NULL,
                        rejection_criteria_json TEXT NOT NULL,
                        status TEXT NOT NULL,
                        tags_json TEXT NOT NULL,
                        revision INTEGER NOT NULL,
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL
                    );
                    CREATE TABLE planning_events (
                        event_id TEXT PRIMARY KEY,
                        entity_type TEXT NOT NULL,
                        entity_id TEXT NOT NULL,
                        aggregate_type TEXT NOT NULL,
                        aggregate_id TEXT NOT NULL,
                        correlation_id TEXT NOT NULL,
                        causation_id TEXT,
                        run_id TEXT NOT NULL,
                        stream_id TEXT NOT NULL,
                        sequence INTEGER NOT NULL,
                        parent_event_id TEXT,
                        event_type TEXT NOT NULL,
                        timestamp TEXT NOT NULL,
                        payload_json TEXT NOT NULL
                    );
                    "#,
                )
                .expect("create v2 schema");
            connection
                .execute(
                    r#"
                    INSERT INTO goals (
                        id, scope_key, correlation_id, title, description, acceptance_criteria_json,
                        rejection_criteria_json, status, tags_json, revision, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10, ?10)
                    "#,
                    params![
                        "goal-v2",
                        "default",
                        "corr-v2",
                        "Goal v2",
                        "Goal before event scope migration",
                        "[\"ok\"]",
                        "[\"no\"]",
                        "draft",
                        "[]",
                        "2026-05-24T00:00:00Z"
                    ],
                )
                .expect("insert v2 goal");
            connection
                .execute(
                    r#"
                    INSERT INTO planning_events (
                        event_id, entity_type, entity_id, aggregate_type, aggregate_id,
                        correlation_id, causation_id, run_id, stream_id, sequence,
                        parent_event_id, event_type, timestamp, payload_json
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9, NULL, ?10, ?11, ?12)
                    "#,
                    params![
                        "event-v2-goal",
                        "goal",
                        "goal-v2",
                        "goal",
                        "goal-v2",
                        "corr-v2",
                        "run-v2",
                        "goal-v2",
                        1_i64,
                        "goal.created",
                        "2026-05-24T00:00:00Z",
                        "{\"id\":\"goal-v2\"}"
                    ],
                )
                .expect("insert v2 event");
        }

        let store = PlanningStore::new(&db_path);
        store.init().expect("migrate schema");

        let default_events = store
            .list_events_in_scope("default")
            .expect("list migrated default events");
        assert_eq!(default_events.len(), 1);
        assert_eq!(default_events[0].event_id, "event-v2-goal");

        let connection = rusqlite::Connection::open(&db_path).expect("reopen sqlite");
        let scope_key: String = connection
            .query_row(
                "SELECT scope_key FROM planning_events WHERE event_id = ?1",
                params!["event-v2-goal"],
                |row| row.get(0),
            )
            .expect("load backfilled event scope");
        assert_eq!(scope_key, "default");
    }

    #[test]
    fn init_migrates_v7_work_points_with_readable_v8_defaults() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning-v7.db");

        {
            let connection = rusqlite::Connection::open(&db_path).expect("open sqlite");
            connection
                .execute_batch(
                    r#"
                    CREATE TABLE planning_config (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                    INSERT INTO planning_config (key, value) VALUES ('schema_version', '7');
                    CREATE TABLE work_points (
                        id TEXT PRIMARY KEY,
                        scope_key TEXT NOT NULL,
                        roadmap_id TEXT NOT NULL,
                        section_id TEXT,
                        title TEXT NOT NULL,
                        summary TEXT NOT NULL,
                        status TEXT NOT NULL,
                        ordering_index INTEGER NOT NULL,
                        dependency_ids_json TEXT NOT NULL,
                        validation_expectations_json TEXT NOT NULL,
                        effort_tier TEXT NOT NULL,
                        tags_json TEXT NOT NULL,
                        revision INTEGER NOT NULL,
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL
                    );
                    "#,
                )
                .expect("create v7 schema");
            connection
                .execute(
                    r#"
                    INSERT INTO work_points (
                        id, scope_key, roadmap_id, section_id, title, summary, status,
                        ordering_index, dependency_ids_json, validation_expectations_json,
                        effort_tier, tags_json, revision, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, 1, ?7, ?8, ?9, ?10, 1, ?11, ?11)
                    "#,
                    params![
                        "wp-v7",
                        "default",
                        "roadmap-v7",
                        "Work point before v8",
                        "Existing row should receive readable defaults",
                        "draft",
                        "[]",
                        "[\"proof\"]",
                        "balanced",
                        "[]",
                        "2026-01-01T00:00:00Z",
                    ],
                )
                .expect("insert v7 work point");
        }

        let store = PlanningStore::new(&db_path);
        store.init().expect("migrate v7 store");

        let work_point = store
            .work_point("wp-v7")
            .expect("load migrated work point")
            .work_point;
        assert_eq!(work_point.kind, WorkPointKind::Feature);
        assert_eq!(work_point.priority, Priority::Medium);
        assert!(work_point.repairs_work_point_ids.is_empty());
        assert!(work_point.supersedes_work_point_ids.is_empty());
        assert!(work_point.blocks_work_point_ids.is_empty());
    }

    #[test]
    fn lifecycle_transitions_and_plan_revise_append_events() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning-lifecycle.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init planning db");

        store
            .create_goal(CreateGoalInput {
                id: Some("goal-life".to_string()),
                scope_key: None,
                correlation_id: "corr-life".to_string(),
                title: "Goal".to_string(),
                description: "Desc".to_string(),
                acceptance_criteria: vec!["ok".to_string()],
                rejection_criteria: vec!["no".to_string()],
                status: GoalStatus::Active,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create goal");
        store
            .create_roadmap(CreateRoadmapInput {
                id: Some("roadmap-life".to_string()),
                scope_key: None,
                goal_id: "goal-life".to_string(),
                correlation_id: "corr-life".to_string(),
                title: "Roadmap".to_string(),
                summary: "Summary".to_string(),
                status: RoadmapStatus::Active,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create roadmap");
        store
            .add_work_point(AddWorkPointInput {
                id: Some("wp-life".to_string()),
                scope_key: None,
                roadmap_id: "roadmap-life".to_string(),
                section_id: None,
                title: "WP".to_string(),
                summary: "Summary".to_string(),
                status: WorkPointStatus::Active,
                ordering: None,
                dependency_ids: Vec::new(),
                validation_expectations: vec!["proof".to_string()],
                effort_tier: crate::EffortTier::Balanced,
                kind: None,
                priority: None,
                repairs_work_point_ids: Vec::new(),
                supersedes_work_point_ids: Vec::new(),
                blocks_work_point_ids: Vec::new(),
                file_scopes: Vec::new(),
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create work point");
        store
            .create_plan(CreatePlanInput {
                id: Some("plan-life".to_string()),
                scope_key: None,
                goal_id: "goal-life".to_string(),
                roadmap_id: "roadmap-life".to_string(),
                correlation_id: "corr-life".to_string(),
                title: "Plan".to_string(),
                summary: "Summary".to_string(),
                scope: "implementation".to_string(),
                assumptions: vec!["a1".to_string()],
                stop_conditions: Vec::new(),
                validation_steps: vec!["validate".to_string()],
                targeted_work_point_ids: vec!["wp-life".to_string()],
                effort_tier: crate::EffortTier::Balanced,
                routing_hint: None,
                allow_parallel_overlap: false,
                file_scopes: Vec::new(),
                status: PlanStatus::Active,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create plan");
        store
            .create_todo(CreateTodoInput {
                id: Some("todo-life".to_string()),
                scope_key: None,
                plan_id: Some("plan-life".to_string()),
                work_point_id: Some("wp-life".to_string()),
                title: "Todo".to_string(),
                summary: "Summary".to_string(),
                status: TodoStatus::InProgress,
                priority: Priority::High,
                effort_tier: crate::EffortTier::Balanced,
                file_scopes: Vec::new(),
                evidence_refs: Vec::new(),
                tags: Vec::new(),
                ordering: None,
                run_id: None,
            })
            .expect("create todo");

        store
            .update_status(UpdateStatusInput {
                entity_type: EntityType::Todo,
                entity_id: "todo-life".to_string(),
                status: "completed".to_string(),
                evidence_refs: Some(vec!["proof://ci".to_string()]),
                active_scope_key: None,
                run_id: Some("run-life".to_string()),
                override_transition: false,
                reason: None,
            })
            .expect("complete todo");
        let revise = store
            .revise_plan(RevisePlanInput {
                plan_id: "plan-life".to_string(),
                active_scope_key: None,
                scope_key: None,
                assumptions: Some(vec!["a1".to_string(), "a2".to_string()]),
                stop_conditions: Some(vec!["stop".to_string()]),
                validation_steps: None,
                targeted_work_point_ids: None,
                effort_tier: None,
                routing_hint: None,
                clear_routing_hint: false,
                allow_parallel_overlap: None,
                file_scopes: None,
                clear_file_scopes: false,
                tags: Some(vec!["rev-1".to_string()]),
                run_id: Some("run-life".to_string()),
            })
            .expect("revise plan");

        assert!(revise.record.revision >= 2);
        assert!(revise
            .record
            .assumptions
            .iter()
            .any(|assumption| assumption == "a2"));
        let todo = store.list_todos().expect("list todos");
        assert_eq!(todo[0].status, TodoStatus::Completed);
        assert_eq!(todo[0].evidence_refs, vec!["proof://ci".to_string()]);
        let events = store.list_events().expect("list events");
        assert!(events
            .iter()
            .any(|event| event.event_type == "todo.status-updated"));
        assert!(events
            .iter()
            .any(|event| event.event_type == "plan.revised"));
    }

    #[test]
    fn graph_schema_created_for_new_database() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Verify schema version is 9
        let conn = store.open_connection().expect("open");
        let version: String = conn
            .query_row(
                "SELECT value FROM planning_config WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .expect("get schema version");
        assert_eq!(
            version, "11",
            "fresh database should have schema version 11"
        );

        // Verify graph tables exist and accept inserts
        let node_input = CreateGraphNodeInput {
            id: Some("gn-schema-1".to_string()),
            scope_key: None,
            kind: PlanningNodeKind::Goal,
            title: "Test Goal Node".to_string(),
            summary: "Schema verification".to_string(),
            status: "active".to_string(),
            payload: serde_json::json!({}),
            tags: vec!["test".to_string()],
            correlation_id: "test-correlation".to_string(),
            run_id: None,
        };
        let result = store
            .create_graph_node(node_input)
            .expect("create graph node");
        assert_eq!(result.record.id, "gn-schema-1");
        assert_eq!(result.record.kind, PlanningNodeKind::Goal);
        assert_eq!(result.validation.status, ValidationStatus::Valid);
        assert!(result.validation.findings.is_empty());

        // Verify tables are queryable
        let loaded = store.graph_node("gn-schema-1").expect("load graph node");
        assert_eq!(loaded.title, "Test Goal Node");
    }

    #[test]
    fn graph_migrates_v8_to_v9_without_backfill() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");

        // Step 1: Create a v8-schema database by creating a v9 one,
        // dropping the graph tables, and rolling back the version.
        let v8_goal_id = "goal-v8-fixture";
        {
            let store = PlanningStore::new(&db_path);
            store.init().expect("init");
            let conn = store.open_connection().expect("open");

            // Insert a goal as v8 test data
            conn.execute(
                "INSERT INTO goals (id, scope_key, correlation_id, title, description, acceptance_criteria_json, rejection_criteria_json, status, tags_json, revision, created_at, updated_at) VALUES (?1, 'default', 'corr-v8', 'V8 Goal', 'Pre-migration goal', '[]', '[]', 'active', '[]', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
                params![v8_goal_id],
            )
            .expect("insert v8 goal");

            // Drop graph tables to simulate v8 state
            conn.execute_batch(
                "DROP TABLE IF EXISTS planning_edges; DROP TABLE IF EXISTS planning_nodes;",
            )
            .expect("drop graph tables");

            // Set version back to 8
            conn.execute(
                "UPDATE planning_config SET value = '8' WHERE key = 'schema_version'",
                [],
            )
            .expect("set version to 8");
        }

        // Step 2: Re-open — migration should create graph tables
        let store = PlanningStore::new(&db_path);
        store.init().expect("re-init with migration");

        let conn = store.open_connection().expect("open");

        // Assert version is now 9
        let version: String = conn
            .query_row(
                "SELECT value FROM planning_config WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .expect("get schema version");
        assert_eq!(version, "11", "should have migrated through v11");

        // Assert v8 tables remain readable
        let goal_title: String = conn
            .query_row(
                "SELECT title FROM goals WHERE id = ?1",
                params![v8_goal_id],
                |row| row.get(0),
            )
            .expect("read v8 goal");
        assert_eq!(goal_title, "V8 Goal", "v8 data should survive migration");

        // Assert graph tables exist but are empty
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM planning_nodes", [], |row| row.get(0))
            .expect("count nodes");
        assert_eq!(count, 0, "no nodes should be backfilled");

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM planning_edges", [], |row| row.get(0))
            .expect("count edges");
        assert_eq!(count, 0, "no edges should be backfilled");
    }

    #[test]
    fn graph_node_round_trips() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create a scope first
        store
            .create_scope(CreateScopeInput {
                scope_key: "roundtrip".to_string(),
                scope_type: Some("workspace".to_string()),
                parent_scope_key: None,
                metadata: None,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create scope");

        let payload = serde_json::json!({"key": "value", "nested": {"num": 42}});
        let node_input = CreateGraphNodeInput {
            id: Some("gn-round".to_string()),
            scope_key: Some("roundtrip".to_string()),
            kind: PlanningNodeKind::Work,
            title: "Roundtrip Work".to_string(),
            summary: "Testing roundtrip".to_string(),
            status: "active".to_string(),
            payload: payload.clone(),
            tags: vec!["test".to_string(), "roundtrip".to_string()],
            correlation_id: "test-correlation".to_string(),
            run_id: None,
        };
        let result = store.create_graph_node(node_input).expect("create");
        assert_eq!(result.record.id, "gn-round");
        assert_eq!(result.record.kind, PlanningNodeKind::Work);
        assert_eq!(result.record.status, "active");
        assert_eq!(result.record.payload, payload);
        assert_eq!(result.record.tags.len(), 2);
        assert!(result.record.tags.contains(&"test".to_string()));
        assert!(result.record.tags.contains(&"roundtrip".to_string()));
        assert_eq!(result.record.revision, 1);

        let loaded = store.graph_node("gn-round").expect("load");
        assert_eq!(loaded, result.record, "roundtrip should match");
    }

    #[test]
    fn graph_edge_round_trips() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create two work nodes
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-w1".to_string()),
                scope_key: None,
                kind: PlanningNodeKind::Work,
                title: "Work 1".to_string(),
                summary: "First work node".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("create w1");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-w2".to_string()),
                scope_key: None,
                kind: PlanningNodeKind::Work,
                title: "Work 2".to_string(),
                summary: "Second work node".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("create w2");

        // Create a depends-on edge
        let edge_result = store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-dep".to_string()),
                scope_key: None,
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-w1".to_string(),
                target_node_id: "gn-w2".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({"priority": "high"}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("create edge");
        assert_eq!(edge_result.record.id, "ge-dep");
        assert_eq!(edge_result.record.kind, PlanningEdgeKind::DependsOn);

        // Load by id
        let loaded = store.graph_edge("ge-dep").expect("load edge");
        assert_eq!(loaded.source_node_id, "gn-w1");
        assert_eq!(loaded.target_node_id, "gn-w2");

        // List outgoing from w1
        let outgoing = store
            .list_outgoing_edges("gn-w1", None)
            .expect("list outgoing");
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].target_node_id, "gn-w2");

        // List incoming to w2
        let incoming = store
            .list_incoming_edges("gn-w2", None)
            .expect("list incoming");
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].source_node_id, "gn-w1");

        // W2 should have no outgoing edges
        let outgoing_w2 = store
            .list_outgoing_edges("gn-w2", None)
            .expect("list outgoing w2");
        assert!(outgoing_w2.is_empty());
    }

    #[test]
    fn graph_edge_rejects_missing_node() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create one node
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-missing-1".to_string()),
                scope_key: None,
                kind: PlanningNodeKind::Work,
                title: "Exists".to_string(),
                summary: "Exists".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("create node");

        // Try edge to non-existent target
        let err = store
            .create_graph_edge(CreateGraphEdgeInput {
                id: None,
                scope_key: None,
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-missing-1".to_string(),
                target_node_id: "gn-nonexistent".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect_err("should reject missing target");
        assert!(
            err.to_string().contains("missing node"),
            "error should mention missing node: {err}"
        );

        // Try edge from non-existent source
        let err = store
            .create_graph_edge(CreateGraphEdgeInput {
                id: None,
                scope_key: None,
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-nonexistent".to_string(),
                target_node_id: "gn-missing-1".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect_err("should reject missing source");
        assert!(
            err.to_string().contains("missing node"),
            "error should mention missing node: {err}"
        );
    }

    #[test]
    fn graph_edge_rejects_cross_scope() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create two scopes
        store
            .create_scope(CreateScopeInput {
                scope_key: "scope-a".to_string(),
                scope_type: Some("workspace".to_string()),
                parent_scope_key: None,
                metadata: None,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create scope a");
        store
            .create_scope(CreateScopeInput {
                scope_key: "scope-b".to_string(),
                scope_type: Some("workspace".to_string()),
                parent_scope_key: None,
                metadata: None,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create scope b");

        // Create node in scope A
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-sa".to_string()),
                scope_key: Some("scope-a".to_string()),
                kind: PlanningNodeKind::Work,
                title: "Scope A Node".to_string(),
                summary: "In scope A".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("create node in scope a");
        // Create node in scope B
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-sb".to_string()),
                scope_key: Some("scope-b".to_string()),
                kind: PlanningNodeKind::Work,
                title: "Scope B Node".to_string(),
                summary: "In scope B".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("create node in scope b");

        // Try cross-scope edge
        let err = store
            .create_graph_edge(CreateGraphEdgeInput {
                id: None,
                scope_key: Some("scope-a".to_string()),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-sa".to_string(),
                target_node_id: "gn-sb".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect_err("should reject cross-scope edge");
        assert!(
            err.to_string().contains("same scope"),
            "error should mention scope: {err}"
        );
    }

    #[test]
    fn graph_edge_rejects_invalid_kind_pair() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create a goal node and a plan node
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-goal-invalid".to_string()),
                scope_key: None,
                kind: PlanningNodeKind::Goal,
                title: "A Goal".to_string(),
                summary: "Goal node".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("create goal");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-plan-invalid".to_string()),
                scope_key: None,
                kind: PlanningNodeKind::Plan,
                title: "A Plan".to_string(),
                summary: "Plan node".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("create plan");

        // planned-by requires source Work, not Goal
        let err = store
            .create_graph_edge(CreateGraphEdgeInput {
                id: None,
                scope_key: None,
                kind: PlanningEdgeKind::PlannedBy,
                source_node_id: "gn-goal-invalid".to_string(),
                target_node_id: "gn-plan-invalid".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect_err("should reject invalid kind pair");
        assert!(
            err.to_string().contains("cannot connect"),
            "error should mention invalid edge: {err}"
        );
    }

    #[test]
    fn graph_edge_rejects_dependency_cycle() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create three work nodes: A, B, C
        for id in &["gn-a", "gn-b", "gn-c"] {
            store
                .create_graph_node(CreateGraphNodeInput {
                    id: Some(id.to_string()),
                    scope_key: None,
                    kind: PlanningNodeKind::Work,
                    title: format!("Node {id}"),
                    summary: format!("Node {id}").to_string(),
                    status: "active".to_string(),
                    payload: serde_json::json!({}),
                    tags: vec![],
                    correlation_id: "test-correlation".to_string(),
                    run_id: None,
                })
                .expect("create node");
        }

        // A depends-on B
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-a-b".to_string()),
                scope_key: None,
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-a".to_string(),
                target_node_id: "gn-b".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("A -> B");

        // B depends-on C
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-b-c".to_string()),
                scope_key: None,
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-b".to_string(),
                target_node_id: "gn-c".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("B -> C");

        // C depends-on A should be rejected (cycle: A -> B -> C -> A)
        let err = store
            .create_graph_edge(CreateGraphEdgeInput {
                id: None,
                scope_key: None,
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-c".to_string(),
                target_node_id: "gn-a".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect_err("should reject cycle");
        assert!(
            err.to_string().contains("cycle"),
            "error should mention cycle: {err}"
        );
    }

    #[test]
    fn graph_edge_rejects_decomposition_cycle() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create goal, roadmap, milestone
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-goal-decomp".to_string()),
                scope_key: None,
                kind: PlanningNodeKind::Goal,
                title: "Goal".to_string(),
                summary: "Top-level goal".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("create goal");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-roadmap-decomp".to_string()),
                scope_key: None,
                kind: PlanningNodeKind::Roadmap,
                title: "Roadmap".to_string(),
                summary: "Roadmap".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("create roadmap");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-milestone-decomp".to_string()),
                scope_key: None,
                kind: PlanningNodeKind::Milestone,
                title: "Milestone".to_string(),
                summary: "Milestone".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("create milestone");

        // Goal decomposes-to Roadmap
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-g-r".to_string()),
                scope_key: None,
                kind: PlanningEdgeKind::DecomposesTo,
                source_node_id: "gn-goal-decomp".to_string(),
                target_node_id: "gn-roadmap-decomp".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("Goal -> Roadmap");

        // Roadmap decomposes-to Milestone
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-r-m".to_string()),
                scope_key: None,
                kind: PlanningEdgeKind::DecomposesTo,
                source_node_id: "gn-roadmap-decomp".to_string(),
                target_node_id: "gn-milestone-decomp".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("Roadmap -> Milestone");

        // Milestone decomposes-to Goal should be rejected (cycle)
        let err = store
            .create_graph_edge(CreateGraphEdgeInput {
                id: None,
                scope_key: None,
                kind: PlanningEdgeKind::DecomposesTo,
                source_node_id: "gn-milestone-decomp".to_string(),
                target_node_id: "gn-goal-decomp".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect_err("should reject decomposition cycle");
        assert!(
            err.to_string().contains("invalid edge"),
            "error should mention invalid edge: {err}"
        );
    }

    #[test]
    fn graph_active_duplicate_rejected() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create two work nodes
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-dup-1".to_string()),
                scope_key: None,
                kind: PlanningNodeKind::Work,
                title: "Work 1".to_string(),
                summary: "First".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("create w1");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-dup-2".to_string()),
                scope_key: None,
                kind: PlanningNodeKind::Work,
                title: "Work 2".to_string(),
                summary: "Second".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("create w2");

        // Create first depends-on edge
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-dup-1".to_string()),
                scope_key: None,
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-dup-1".to_string(),
                target_node_id: "gn-dup-2".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("first edge");

        // Try duplicate active edge
        let err = store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-dup-2".to_string()),
                scope_key: None,
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-dup-1".to_string(),
                target_node_id: "gn-dup-2".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect_err("should reject duplicate active edge");
        assert!(
            err.to_string().contains("duplicate"),
            "error should mention duplicate: {err}"
        );
    }

    #[test]
    fn graph_edge_rejects_decomposes_to_cycle_with_valid_kind_pairs() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create three work nodes — DecomposesTo allows Work -> Work
        for id in &["gn-dc-a", "gn-dc-b", "gn-dc-c"] {
            store
                .create_graph_node(CreateGraphNodeInput {
                    id: Some(id.to_string()),
                    scope_key: None,
                    kind: PlanningNodeKind::Work,
                    title: format!("Node {id}"),
                    summary: format!("Decomposition test {id}").to_string(),
                    status: "active".to_string(),
                    payload: serde_json::json!({}),
                    tags: vec![],
                    correlation_id: "test-correlation".to_string(),
                    run_id: None,
                })
                .expect("create node");
        }

        // A decomposes-to B
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-dc-a-b".to_string()),
                scope_key: None,
                kind: PlanningEdgeKind::DecomposesTo,
                source_node_id: "gn-dc-a".to_string(),
                target_node_id: "gn-dc-b".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("A -> B");

        // B decomposes-to C
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-dc-b-c".to_string()),
                scope_key: None,
                kind: PlanningEdgeKind::DecomposesTo,
                source_node_id: "gn-dc-b".to_string(),
                target_node_id: "gn-dc-c".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("B -> C");

        // C decomposes-to A should be rejected (cycle: A -> B -> C -> A)
        let err = store
            .create_graph_edge(CreateGraphEdgeInput {
                id: None,
                scope_key: None,
                kind: PlanningEdgeKind::DecomposesTo,
                source_node_id: "gn-dc-c".to_string(),
                target_node_id: "gn-dc-a".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect_err("should reject DecomposesTo cycle with valid kind pairs");
        assert!(
            err.to_string().contains("cycle"),
            "error should mention cycle: {err}"
        );
    }

    #[test]
    fn graph_edge_rejects_self_loop() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-self".to_string()),
                scope_key: None,
                kind: PlanningNodeKind::Work,
                title: "Self Node".to_string(),
                summary: "Node for self-loop test".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect("create node");

        let err = store
            .create_graph_edge(CreateGraphEdgeInput {
                id: None,
                scope_key: None,
                kind: PlanningEdgeKind::Blocks,
                source_node_id: "gn-self".to_string(),
                target_node_id: "gn-self".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                correlation_id: "test-correlation".to_string(),
                run_id: None,
            })
            .expect_err("should reject self-referential edge");
        assert!(
            err.to_string().contains("self-referential"),
            "error should mention self-referential: {err}"
        );
    }

    #[test]
    fn graph_node_create_appends_event() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        let result = store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-event-test".to_string()),
                scope_key: None,
                correlation_id: "corr-gn-event".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Event Test Node".to_string(),
                summary: "Testing event emission".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({"key": "value"}),
                tags: vec!["event-test".to_string()],
                run_id: Some("run-123".to_string()),
            })
            .expect("create node");
        assert_eq!(result.record.id, "gn-event-test");

        // Verify event was appended
        let events = store.list_events().expect("list events");
        let node_events: Vec<_> = events
            .iter()
            .filter(|e| e.entity_type == EntityType::GraphNode)
            .collect();
        assert_eq!(
            node_events.len(),
            1,
            "should have exactly one graph-node event"
        );
        let event = node_events[0];
        assert_eq!(event.entity_type, EntityType::GraphNode);
        assert_eq!(event.entity_id, "gn-event-test");
        assert_eq!(event.aggregate_type, EntityType::GraphNode);
        assert_eq!(event.aggregate_id, "gn-event-test");
        assert_eq!(event.event_type, "graph-node.created");
        assert_eq!(event.correlation_id, "corr-gn-event");
        assert_eq!(event.run_id, "run-123");
        assert_eq!(event.payload["title"].as_str(), Some("Event Test Node"));
    }

    #[test]
    fn graph_edge_create_appends_event() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create source and target nodes
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-edge-event-src".to_string()),
                scope_key: None,
                correlation_id: "corr-src".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Source".to_string(),
                summary: "Source node".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create source");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-edge-event-tgt".to_string()),
                scope_key: None,
                correlation_id: "corr-tgt".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Target".to_string(),
                summary: "Target node".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create target");

        let result = store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-event-test".to_string()),
                scope_key: None,
                correlation_id: "corr-edge-event".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-edge-event-src".to_string(),
                target_node_id: "gn-edge-event-tgt".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({"priority": "high"}),
                run_id: Some("run-edge-456".to_string()),
            })
            .expect("create edge");
        assert_eq!(result.record.id, "ge-event-test");

        // Verify edge event
        let events = store.list_events().expect("list events");
        let edge_events: Vec<_> = events
            .iter()
            .filter(|e| e.entity_type == EntityType::GraphEdge)
            .collect();
        assert_eq!(
            edge_events.len(),
            1,
            "should have exactly one graph-edge event"
        );
        let event = edge_events[0];
        assert_eq!(event.entity_type, EntityType::GraphEdge);
        assert_eq!(event.entity_id, "ge-event-test");
        assert_eq!(event.aggregate_type, EntityType::GraphNode);
        assert_eq!(event.aggregate_id, "gn-edge-event-src");
        assert_eq!(event.event_type, "graph-edge.created");
        assert_eq!(event.correlation_id, "corr-edge-event");
        assert_eq!(event.run_id, "run-edge-456");
        assert_eq!(event.payload["kind"].as_str(), Some("depends-on"));
    }

    #[test]
    fn graph_node_rejects_empty_id() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        let make_input = |id: Option<String>| CreateGraphNodeInput {
            id,
            scope_key: None,
            correlation_id: "corr-empty-id".to_string(),
            kind: PlanningNodeKind::Work,
            title: "Test".to_string(),
            summary: "Test".to_string(),
            status: "active".to_string(),
            payload: serde_json::json!({}),
            tags: vec![],
            run_id: None,
        };

        // Empty string ID
        let err = store
            .create_graph_node(make_input(Some("".to_string())))
            .expect_err("should reject empty id");
        assert!(
            err.to_string().contains("must not be empty"),
            "error should mention empty: {err}"
        );

        // Whitespace-only ID
        let err = store
            .create_graph_node(make_input(Some("   ".to_string())))
            .expect_err("should reject whitespace id");
        assert!(
            err.to_string().contains("must not be empty"),
            "error should mention empty: {err}"
        );

        // None ID should still work (auto-generated)
        store
            .create_graph_node(make_input(None))
            .expect("None id should auto-generate");
    }

    #[test]
    fn graph_edge_rejects_empty_id() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create a node to use as source/target
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-empty-id".to_string()),
                scope_key: None,
                correlation_id: "corr".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Node".to_string(),
                summary: "Node".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create node");

        let make_input = |id: Option<String>| CreateGraphEdgeInput {
            id,
            scope_key: None,
            correlation_id: "corr-empty-edge-id".to_string(),
            kind: PlanningEdgeKind::DependsOn,
            source_node_id: "gn-empty-id".to_string(),
            target_node_id: "gn-empty-id".to_string(),
            status: "active".to_string(),
            payload: serde_json::json!({}),
            run_id: None,
        };

        // Empty string ID
        let err = store
            .create_graph_edge(make_input(Some("".to_string())))
            .expect_err("should reject empty id");
        assert!(
            err.to_string().contains("must not be empty"),
            "error should mention empty: {err}"
        );

        // Whitespace-only ID
        let err = store
            .create_graph_edge(make_input(Some("   ".to_string())))
            .expect_err("should reject whitespace id");
        assert!(
            err.to_string().contains("must not be empty"),
            "error should mention empty: {err}"
        );
    }

    #[test]
    fn graph_node_rejects_non_kebab_status() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        let make_input = |status: &str| CreateGraphNodeInput {
            id: Some(format!("gn-status-{}", status.replace(' ', "-"))),
            scope_key: None,
            correlation_id: "corr-status".to_string(),
            kind: PlanningNodeKind::Work,
            title: "Test".to_string(),
            summary: "Test".to_string(),
            status: status.to_string(),
            payload: serde_json::json!({}),
            tags: vec![],
            run_id: None,
        };

        // Uppercase rejected
        let err = store
            .create_graph_node(make_input("Active"))
            .expect_err("should reject Active");
        assert!(
            err.to_string().contains("kebab-case"),
            "error should mention kebab-case: {err}"
        );

        // Spaces rejected
        let err = store
            .create_graph_node(make_input("in progress"))
            .expect_err("should reject spaces");
        assert!(
            err.to_string().contains("kebab-case"),
            "error should mention kebab-case: {err}"
        );

        // Trailing dash rejected
        let err = store
            .create_graph_node(make_input("active-"))
            .expect_err("should reject trailing dash");
        assert!(
            err.to_string().contains("kebab-case"),
            "error should mention kebab-case: {err}"
        );

        // Leading digit rejected
        let err = store
            .create_graph_node(make_input("1st"))
            .expect_err("should reject leading digit");
        assert!(
            err.to_string().contains("kebab-case"),
            "error should mention kebab-case: {err}"
        );

        // Valid kebab-case accepted
        for valid in &["active", "in-progress", "completed", "needs-review"] {
            store
                .create_graph_node(make_input(valid))
                .unwrap_or_else(|e| panic!("should accept '{valid}': {e}"));
        }
    }

    #[test]
    fn graph_edge_rejects_non_kebab_status() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create two nodes for edge
        for id in &["gn-edge-status-src", "gn-edge-status-tgt"] {
            store
                .create_graph_node(CreateGraphNodeInput {
                    id: Some(id.to_string()),
                    scope_key: None,
                    correlation_id: "corr".to_string(),
                    kind: PlanningNodeKind::Work,
                    title: format!("Node {id}"),
                    summary: format!("Node {id}"),
                    status: "active".to_string(),
                    payload: serde_json::json!({}),
                    tags: vec![],
                    run_id: None,
                })
                .expect("create node");
        }

        let make_input = |status: &str| CreateGraphEdgeInput {
            id: None,
            scope_key: None,
            correlation_id: "corr-edge-status".to_string(),
            kind: PlanningEdgeKind::DependsOn,
            source_node_id: "gn-edge-status-src".to_string(),
            target_node_id: "gn-edge-status-tgt".to_string(),
            status: status.to_string(),
            payload: serde_json::json!({}),
            run_id: None,
        };

        // Uppercase rejected
        let err = store
            .create_graph_edge(make_input("Active"))
            .expect_err("should reject Active");
        assert!(
            err.to_string().contains("kebab-case"),
            "error should mention kebab-case: {err}"
        );

        // Valid kebab accepted
        store
            .create_graph_edge(make_input("active"))
            .expect("should accept active");
    }

    #[test]
    fn active_status_rejected_before_duplicate_check() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create two nodes
        for id in &["gn-dc-a", "gn-dc-b"] {
            store
                .create_graph_node(CreateGraphNodeInput {
                    id: Some(id.to_string()),
                    scope_key: None,
                    correlation_id: "corr-dc".to_string(),
                    kind: PlanningNodeKind::Work,
                    title: format!("Node {id}"),
                    summary: "DC node".to_string(),
                    status: "active".to_string(),
                    payload: serde_json::json!({}),
                    tags: vec![],
                    run_id: None,
                })
                .expect("create node");
        }

        // Create first edge (valid)
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-dc-1".to_string()),
                scope_key: None,
                correlation_id: "corr-dc-e1".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-dc-a".to_string(),
                target_node_id: "gn-dc-b".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("first edge");

        // Try duplicate with non-kebab status "Active"
        // This should be rejected by status validation BEFORE the duplicate check
        let err = store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-dc-2".to_string()),
                scope_key: None,
                correlation_id: "corr-dc-e2".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-dc-a".to_string(),
                target_node_id: "gn-dc-b".to_string(),
                status: "Active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect_err("should reject non-kebab status");
        // Error must be about kebab-case, NOT about duplicate
        assert!(
            err.to_string().contains("kebab-case"),
            "error should be about kebab-case format, got: {err}"
        );
        assert!(
            !err.to_string().contains("duplicate"),
            "error should NOT mention duplicate (status rejected first): {err}"
        );
    }

    #[test]
    fn health_includes_graph_counts() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create 2 nodes
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gh-1".to_string()),
                scope_key: None,
                correlation_id: "corr-gh".to_string(),
                kind: PlanningNodeKind::Work,
                title: "H1".to_string(),
                summary: "H1".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create node 1");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gh-2".to_string()),
                scope_key: None,
                correlation_id: "corr-gh".to_string(),
                kind: PlanningNodeKind::Work,
                title: "H2".to_string(),
                summary: "H2".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create node 2");

        // Create 1 edge
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("gh-e1".to_string()),
                scope_key: None,
                correlation_id: "corr-gh".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gh-1".to_string(),
                target_node_id: "gh-2".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("create edge");

        let health = store.health().expect("health");
        assert_eq!(health.graph_node_count, 2, "should count 2 graph nodes");
        assert_eq!(health.graph_edge_count, 1, "should count 1 graph edge");
        assert_eq!(health.schema_version, "11", "schema version should be 11");
    }

    #[test]
    fn graph_node_tags_indexed() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-tagged".to_string()),
                scope_key: None,
                correlation_id: "corr-tags".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Tagged Node".to_string(),
                summary: "Has tags".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec!["rust".to_string(), "graph".to_string()],
                run_id: None,
            })
            .expect("create tagged node");

        // Query tags for graph nodes
        let tags = store
            .list_tags("default", Some("graph-node"))
            .expect("list tags");
        assert_eq!(tags.len(), 2, "should find 2 tags for graph node");
        let tag_names: Vec<&str> = tags.iter().map(|t| t.tag.as_str()).collect();
        assert!(tag_names.contains(&"rust"), "should have 'rust' tag");
        assert!(tag_names.contains(&"graph"), "should have 'graph' tag");
    }

    #[test]
    fn graph_validator_detects_invalid_status() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create a valid node
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-val-status".to_string()),
                scope_key: None,
                correlation_id: "corr-val-status".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Status Test".to_string(),
                summary: "Testing status validation".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create node");

        // Corrupt status via raw SQL
        let conn = store.open_connection().expect("open");
        conn.execute(
            "UPDATE planning_nodes SET status = 'Active' WHERE id = 'gn-val-status'",
            [],
        )
        .expect("corrupt status");

        // Run validate_all
        let report = store.validate_all().expect("validate all");
        let findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.code == "GRAPH-STATUS-INVALID")
            .collect();
        assert!(
            !findings.is_empty(),
            "should detect invalid graph node status"
        );
    }

    #[test]
    fn graph_validator_detects_missing_node() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create two nodes and an edge
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-val-src".to_string()),
                scope_key: None,
                correlation_id: "corr-val-missing".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Source".to_string(),
                summary: "Source node".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create source");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-val-tgt".to_string()),
                scope_key: None,
                correlation_id: "corr-val-missing".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Target".to_string(),
                summary: "Target node".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create target");
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-val-missing".to_string()),
                scope_key: None,
                correlation_id: "corr-val-missing".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-val-src".to_string(),
                target_node_id: "gn-val-tgt".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("create edge");

        // Delete target node via raw SQL.
        // Disable foreign keys temporarily to avoid cascading delete of the edge
        // (the planning_edges table uses ON DELETE CASCADE).
        let conn = store.open_connection().expect("open");
        conn.execute("PRAGMA foreign_keys = OFF", [])
            .expect("disable fk");
        conn.execute("DELETE FROM planning_nodes WHERE id = 'gn-val-tgt'", [])
            .expect("delete target node");
        conn.execute("PRAGMA foreign_keys = ON", [])
            .expect("enable fk");

        // Run validate_all
        let report = store.validate_all().expect("validate all");
        let findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.code == "GRAPH-EDGE-MISSING-NODE")
            .collect();
        assert!(
            !findings.is_empty(),
            "should detect missing node referenced by edge"
        );
    }

    #[test]
    fn graph_validator_detects_self_loop() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create a node
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-self-val".to_string()),
                scope_key: None,
                correlation_id: "corr-self-val".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Self Loop".to_string(),
                summary: "Node for self-loop test".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create node");

        // Insert self-loop edge via raw SQL (bypasses preflight check)
        let conn = store.open_connection().expect("open");
        conn.execute(
            "INSERT INTO planning_edges (id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at) VALUES ('ge-self-val', 'default', 'blocks', 'gn-self-val', 'gn-self-val', 'active', '{}', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert self-loop edge");

        // Run validate_all
        let report = store.validate_all().expect("validate all");
        let findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.code == "GRAPH-EDGE-SELF-LOOP")
            .collect();
        assert!(!findings.is_empty(), "should detect self-loop edge");
    }

    #[test]
    fn graph_validator_detects_duplicate_active() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create two nodes via store API
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-dup-val-a".to_string()),
                scope_key: None,
                correlation_id: "corr-dup-val".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Node A".to_string(),
                summary: "Dup test".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create node");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-dup-val-b".to_string()),
                scope_key: None,
                correlation_id: "corr-dup-val".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Node B".to_string(),
                summary: "Dup test".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create node");

        // Create two duplicate active edges using a single raw connection.
        // We drop the UNIQUE index first so the duplicate insert succeeds,
        // then run validation directly on the same connection (avoiding
        // store.open_connection() which would recreate the index and fail).
        let conn = store.open_connection().expect("open");
        conn.execute("DROP INDEX IF EXISTS idx_planning_edges_unique_active", [])
            .expect("drop unique index");
        conn.execute(
            "INSERT INTO planning_edges (id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at) VALUES ('ge-dup-val-1', 'default', 'depends-on', 'gn-dup-val-a', 'gn-dup-val-b', 'active', '{}', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert edge 1");
        conn.execute(
            "INSERT INTO planning_edges (id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at) VALUES ('ge-dup-val-2', 'default', 'depends-on', 'gn-dup-val-a', 'gn-dup-val-b', 'active', '{}', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert edge 2");

        // Run validation directly on the same connection (avoids recreating the index)
        let entities = collect_entities(&conn).expect("collect entities");
        let mut found_duplicate = false;
        for (entity_type, entity_id) in entities {
            if entity_type == EntityType::GraphEdge {
                let findings =
                    validate_entity(&conn, entity_type, &entity_id).expect("validate graph edge");
                if findings
                    .iter()
                    .any(|f| f.code == "GRAPH-EDGE-DUPLICATE-ACTIVE")
                {
                    found_duplicate = true;
                }
                // Persist findings for the store even though we bypassed validate_all
                persist_validation_findings(&conn, entity_type, &entity_id, &findings)
                    .expect("persist findings");
            }
        }
        assert!(found_duplicate, "should detect duplicate active edge");
    }

    #[test]
    fn graph_validator_detects_cycle() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create three nodes
        for id in &["gn-cycle-a", "gn-cycle-b", "gn-cycle-c"] {
            store
                .create_graph_node(CreateGraphNodeInput {
                    id: Some(id.to_string()),
                    scope_key: None,
                    correlation_id: "corr-cycle-val".to_string(),
                    kind: PlanningNodeKind::Work,
                    title: format!("Node {id}"),
                    summary: "Cycle test".to_string(),
                    status: "active".to_string(),
                    payload: serde_json::json!({}),
                    tags: vec![],
                    run_id: None,
                })
                .expect("create node");
        }

        // Create two edges normally: A->B, B->C
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-cycle-ab".to_string()),
                scope_key: None,
                correlation_id: "corr-cycle-val".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-cycle-a".to_string(),
                target_node_id: "gn-cycle-b".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("A->B");
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-cycle-bc".to_string()),
                scope_key: None,
                correlation_id: "corr-cycle-val".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-cycle-b".to_string(),
                target_node_id: "gn-cycle-c".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("B->C");

        // Insert cyclic edge C->A via raw SQL (bypasses preflight)
        let conn = store.open_connection().expect("open");
        conn.execute(
            "INSERT INTO planning_edges (id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at) VALUES ('ge-cycle-ca', 'default', 'depends-on', 'gn-cycle-c', 'gn-cycle-a', 'active', '{}', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .expect("insert cyclic edge");

        // Run validate_all
        let report = store.validate_all().expect("validate all");
        let findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.code == "GRAPH-EDGE-CYCLE")
            .collect();
        assert!(!findings.is_empty(), "should detect graph cycle");
    }

    #[test]
    fn context_graph_node_includes_edges_and_tags() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create two nodes + edge
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-ctx-node".to_string()),
                scope_key: None,
                correlation_id: "corr-ctx".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Context Node".to_string(),
                summary: "Node for context test".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({"ctx": true}),
                tags: vec!["ctx-tag".to_string(), "test".to_string()],
                run_id: None,
            })
            .expect("create node");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-ctx-other".to_string()),
                scope_key: None,
                correlation_id: "corr-ctx".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Other Node".to_string(),
                summary: "Connected node".to_string(),
                status: "completed".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create other node");
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-ctx".to_string()),
                scope_key: None,
                correlation_id: "corr-ctx".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-ctx-node".to_string(),
                target_node_id: "gn-ctx-other".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("create edge");

        let bundle = store
            .context_bundle(EntityType::GraphNode, "gn-ctx-node", "default")
            .expect("context bundle");

        // Tags present
        assert_eq!(bundle.tags.len(), 2);
        assert!(bundle.tags.contains(&"ctx-tag".to_string()));

        // Entity JSON is the full node record
        assert_eq!(bundle.entity["id"], "gn-ctx-node");
        assert_eq!(bundle.entity["title"], "Context Node");

        // Children contains outgoing edges and connected nodes
        assert!(!bundle.children.is_empty(), "should have children");
        // At least one child should contain the edges
        let children_str = serde_json::to_string(&bundle.children).expect("serialize children");
        assert!(
            children_str.contains("gn-ctx-other"),
            "children should reference connected node: {children_str}"
        );

        // Validation report present
        assert!(
            bundle.validation.findings.is_empty(),
            "no validation issues expected"
        );
    }

    #[test]
    fn context_graph_edge_includes_source_target() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create two nodes + edge
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-ectx-src".to_string()),
                scope_key: None,
                correlation_id: "corr-ectx".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Edge Context Source".to_string(),
                summary: "Source for edge context".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create source");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-ectx-tgt".to_string()),
                scope_key: None,
                correlation_id: "corr-ectx".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Edge Context Target".to_string(),
                summary: "Target for edge context".to_string(),
                status: "completed".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create target");
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-ectx".to_string()),
                scope_key: None,
                correlation_id: "corr-ectx".to_string(),
                kind: PlanningEdgeKind::Blocks,
                source_node_id: "gn-ectx-src".to_string(),
                target_node_id: "gn-ectx-tgt".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({"reason": "blocked"}),
                run_id: None,
            })
            .expect("create edge");

        let bundle = store
            .context_bundle(EntityType::GraphEdge, "ge-ectx", "default")
            .expect("context bundle");

        // Entity JSON is the full edge record
        assert_eq!(bundle.entity["id"], "ge-ectx");
        assert_eq!(bundle.entity["kind"], "blocks");

        // Children should contain source and target node summaries
        assert!(!bundle.children.is_empty(), "should have children");
        let children_str = serde_json::to_string(&bundle.children).expect("serialize children");
        assert!(
            children_str.contains("Edge Context Source"),
            "children should reference source node: {children_str}"
        );
        assert!(
            children_str.contains("Edge Context Target"),
            "children should reference target node: {children_str}"
        );
    }

    #[test]
    fn load_entity_tags_returns_graph_node_tags() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-tags-regr".to_string()),
                scope_key: None,
                correlation_id: "corr-tags-regr".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Tag Regression".to_string(),
                summary: "Testing tag regression".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec!["regression".to_string(), "graph".to_string()],
                run_id: None,
            })
            .expect("create tagged node");

        // Verify via context bundle (which uses load_entity_tags internally)
        let bundle = store
            .context_bundle(EntityType::GraphNode, "gn-tags-regr", "default")
            .expect("context bundle");
        assert_eq!(bundle.tags.len(), 2);
        assert!(bundle.tags.contains(&"regression".to_string()));
        assert!(bundle.tags.contains(&"graph".to_string()));
    }

    #[test]
    fn graph_edges_are_collected_in_validate_all() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create a valid node and a valid edge
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-collect-1".to_string()),
                scope_key: None,
                correlation_id: "corr-collect".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Collection Node 1".to_string(),
                summary: "Node for collection test".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create node 1");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-collect-2".to_string()),
                scope_key: None,
                correlation_id: "corr-collect".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Collection Node 2".to_string(),
                summary: "Node for collection test".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create node 2");
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-collect".to_string()),
                scope_key: None,
                correlation_id: "corr-collect".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-collect-1".to_string(),
                target_node_id: "gn-collect-2".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("create edge");

        // Run validate_all — should not panic, should complete
        let report = store.validate_all().expect("validate all");
        // Graph entities should be included: at least 2 nodes + 1 edge = 3+
        assert!(
            report.entity_reports.len() >= 3,
            "should include graph entities in validation reports, got {} reports",
            report.entity_reports.len()
        );
    }

    #[test]
    fn scope_aware_incoming_excludes_cross_scope() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create scope-a
        store
            .create_scope(CreateScopeInput {
                scope_key: "scope-a".to_string(),
                scope_type: Some("workspace".to_string()),
                parent_scope_key: None,
                metadata: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create scope-a");

        // Create a node in default scope and a node in scope-a
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-sa-src".to_string()),
                scope_key: None,
                correlation_id: "corr-sa".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Default Node".to_string(),
                summary: "In default scope".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create default node");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-sa-tgt".to_string()),
                scope_key: Some("scope-a".to_string()),
                correlation_id: "corr-sa".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Scope-A Node".to_string(),
                summary: "In scope-a".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create scope-a node");

        // Insert an edge via SQL that references both nodes (bypassing preflight scope check)
        let conn = store.open_connection().expect("open");
        conn.execute(
            "INSERT INTO planning_edges (id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at) VALUES ('ge-sa-cross', 'scope-a', 'depends-on', 'gn-sa-src', 'gn-sa-tgt', 'active', '{}', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        ).expect("insert cross-scope edge");

        // Scope-aware query from default scope should NOT include the edge (it's in scope-a)
        let edges = list_incoming_edges_in_scope(&conn, "gn-sa-tgt", "default", None)
            .expect("list incoming in default scope");
        assert!(
            edges.is_empty(),
            "cross-scope edge should be excluded from default scope"
        );

        // Scope-aware query from scope-a should include the edge
        let edges = list_incoming_edges_in_scope(&conn, "gn-sa-tgt", "scope-a", None)
            .expect("list incoming in scope-a");
        assert_eq!(edges.len(), 1, "edge should be visible in its own scope");
    }

    #[test]
    fn graph_node_view_includes_validation() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-view-test".to_string()),
                scope_key: None,
                correlation_id: "corr-view".to_string(),
                kind: PlanningNodeKind::Work,
                title: "View Test".to_string(),
                summary: "Testing views".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({"key": "val"}),
                tags: vec!["view-tag".to_string()],
                run_id: None,
            })
            .expect("create node");

        let view = store
            .graph_node_view("gn-view-test", "default")
            .expect("get view");
        assert_eq!(view.node.id, "gn-view-test");
        assert_eq!(view.node.title, "View Test");
        assert_eq!(view.tags, vec!["view-tag"]);
        assert!(
            view.validation.findings.is_empty(),
            "no validation issues expected"
        );
        assert!(view.incoming_edges.is_empty());
        assert!(view.outgoing_edges.is_empty());
        assert!(view.connected_nodes.is_empty());
    }

    #[test]
    fn graph_edge_view_includes_source_target() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-ev-src".to_string()),
                scope_key: None,
                correlation_id: "corr-ev".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Edge View Source".to_string(),
                summary: "Source".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create source");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-ev-tgt".to_string()),
                scope_key: None,
                correlation_id: "corr-ev".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Edge View Target".to_string(),
                summary: "Target".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create target");
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-ev".to_string()),
                scope_key: None,
                correlation_id: "corr-ev".to_string(),
                kind: PlanningEdgeKind::Blocks,
                source_node_id: "gn-ev-src".to_string(),
                target_node_id: "gn-ev-tgt".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({"reason": "blocked"}),
                run_id: None,
            })
            .expect("create edge");

        let view = store.graph_edge_view("ge-ev", "default").expect("get view");
        assert_eq!(view.edge.id, "ge-ev");
        assert_eq!(view.edge.kind, PlanningEdgeKind::Blocks);
        assert_eq!(view.source_node["id"], "gn-ev-src");
        assert_eq!(view.source_node["title"], "Edge View Source");
        assert_eq!(view.target_node["id"], "gn-ev-tgt");
        assert_eq!(view.target_node["title"], "Edge View Target");
        assert!(view.validation.findings.is_empty());
    }

    #[test]
    fn update_graph_node_status_appends_event() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-status-event".to_string()),
                scope_key: None,
                correlation_id: "corr-status-ev".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Status Event".to_string(),
                summary: "Testing status events".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create node");

        let result = store
            .update_graph_node_status(UpdateGraphNodeStatusInput {
                node_id: "gn-status-event".to_string(),
                correlation_id: "test-correlation".to_string(),
                active_scope_key: None,
                status: "completed".to_string(),
                run_id: Some("run-status-1".to_string()),
            })
            .expect("update status");
        assert_eq!(result.record.status, "completed");
        assert_eq!(result.record.revision, 2);

        // Verify event
        let events = store.list_events().expect("list events");
        let status_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "graph-node.status-updated")
            .collect();
        assert_eq!(
            status_events.len(),
            1,
            "should have one status-updated event"
        );
        let event = status_events[0];
        assert_eq!(event.entity_type, EntityType::GraphNode);
        assert_eq!(event.entity_id, "gn-status-event");
        assert_eq!(event.run_id, "run-status-1");
        assert_eq!(event.payload["status"], "completed");
    }

    #[test]
    fn update_graph_edge_status_rejects_duplicate_active() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create nodes and edge (inactive first)
        for id in &["gn-active-1", "gn-active-2"] {
            store
                .create_graph_node(CreateGraphNodeInput {
                    id: Some(id.to_string()),
                    scope_key: None,
                    correlation_id: "corr-active".to_string(),
                    kind: PlanningNodeKind::Work,
                    title: format!("Node {id}"),
                    summary: "Active test".to_string(),
                    status: "active".to_string(),
                    payload: serde_json::json!({}),
                    tags: vec![],
                    run_id: None,
                })
                .expect("create node");
        }
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-active-1".to_string()),
                scope_key: None,
                correlation_id: "corr-active".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-active-1".to_string(),
                target_node_id: "gn-active-2".to_string(),
                status: "inactive".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("create first edge (inactive)");

        // Create second edge (active) — succeeds because first is inactive
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-active-2".to_string()),
                scope_key: None,
                correlation_id: "corr-active".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-active-1".to_string(),
                target_node_id: "gn-active-2".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("create second edge (active)");

        // Try to set first edge to active — should fail (duplicate)
        let err = store
            .update_graph_edge_status(UpdateGraphEdgeStatusInput {
                edge_id: "ge-active-1".to_string(),
                correlation_id: "test-correlation".to_string(),
                active_scope_key: None,
                status: "active".to_string(),
                run_id: None,
            })
            .expect_err("should reject duplicate active");
        assert!(
            err.to_string().contains("duplicate"),
            "error should mention duplicate: {err}"
        );
    }

    #[test]
    fn revise_graph_node_updates_fields() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-revise".to_string()),
                scope_key: None,
                correlation_id: "corr-revise".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Old Title".to_string(),
                summary: "Old Summary".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({"old": true}),
                tags: vec!["old-tag".to_string()],
                run_id: None,
            })
            .expect("create node");

        let result = store
            .revise_graph_node(ReviseGraphNodeInput {
                node_id: "gn-revise".to_string(),
                correlation_id: "test-correlation".to_string(),
                active_scope_key: None,
                title: Some("New Title".to_string()),
                summary: Some("New Summary".to_string()),
                status: Some("completed".to_string()),
                payload: Some(serde_json::json!({"new": true})),
                tags: Some(vec!["new-tag".to_string(), "rust".to_string()]),
                clear_tags: false,
                run_id: Some("run-revise-1".to_string()),
            })
            .expect("revise node");
        assert_eq!(result.record.title, "New Title");
        assert_eq!(result.record.summary, "New Summary");
        assert_eq!(result.record.status, "completed");
        assert_eq!(result.record.payload, serde_json::json!({"new": true}));
        assert_eq!(result.record.tags.len(), 2);
        assert!(result.record.tags.contains(&"new-tag".to_string()));
        assert_eq!(result.record.revision, 2);

        // Verify event
        let events = store.list_events().expect("list events");
        let revise_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "graph-node.revised")
            .collect();
        assert_eq!(revise_events.len(), 1);
        assert_eq!(revise_events[0].run_id, "run-revise-1");
    }

    #[test]
    fn graph_node_context_bundle_excludes_cross_scope_edges() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_scope(CreateScopeInput {
                scope_key: "ws-x".to_string(),
                scope_type: Some("workspace".to_string()),
                parent_scope_key: None,
                metadata: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create scope");

        // Node in default, node in ws-x
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-ctx-def".to_string()),
                scope_key: None,
                correlation_id: "corr-ctx".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Default Ctx Node".to_string(),
                summary: "Default".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create default node");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-ctx-wsx".to_string()),
                scope_key: Some("ws-x".to_string()),
                correlation_id: "corr-ctx".to_string(),
                kind: PlanningNodeKind::Work,
                title: "WS-X Ctx Node".to_string(),
                summary: "WS-X".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create ws-x node");

        // Insert cross-scope edge via SQL
        let conn = store.open_connection().expect("open");
        conn.execute(
            "INSERT INTO planning_edges (id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at) VALUES ('ge-ctx-cross', 'ws-x', 'depends-on', 'gn-ctx-def', 'gn-ctx-wsx', 'active', '{}', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        ).expect("insert cross-scope edge");

        // Context bundle for default scope should NOT include the cross-scope edge
        let bundle = store
            .context_bundle(EntityType::GraphNode, "gn-ctx-def", "default")
            .expect("context bundle");
        let children_str = serde_json::to_string(&bundle.children).expect("serialize children");
        assert!(
            !children_str.contains("ge-ctx-cross"),
            "cross-scope edge should be excluded from children: {children_str}"
        );
    }

    #[test]
    fn graph_node_view_rejects_out_of_scope() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_scope(CreateScopeInput {
                scope_key: "ws-nv".to_string(),
                scope_type: Some("workspace".to_string()),
                parent_scope_key: None,
                metadata: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create scope");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-nv".to_string()),
                scope_key: Some("ws-nv".to_string()),
                correlation_id: "corr-nv".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Scope Test".to_string(),
                summary: "In ws-nv".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create scoped node");

        // Query from wrong scope should fail
        let err = store
            .graph_node_view("gn-nv", "default")
            .expect_err("should reject out-of-scope view");
        assert!(
            err.to_string().contains("not"),
            "should mention scope mismatch: {err}"
        );
    }

    #[test]
    fn graph_edge_view_rejects_out_of_scope() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_scope(CreateScopeInput {
                scope_key: "ws-ev".to_string(),
                scope_type: Some("workspace".to_string()),
                parent_scope_key: None,
                metadata: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create scope");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-ev-s".to_string()),
                scope_key: Some("ws-ev".to_string()),
                correlation_id: "corr-ev".to_string(),
                kind: PlanningNodeKind::Work,
                title: "EV Src".to_string(),
                summary: "Src".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create src");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-ev-t".to_string()),
                scope_key: Some("ws-ev".to_string()),
                correlation_id: "corr-ev".to_string(),
                kind: PlanningNodeKind::Work,
                title: "EV Tgt".to_string(),
                summary: "Tgt".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create tgt");
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-ev".to_string()),
                scope_key: Some("ws-ev".to_string()),
                correlation_id: "corr-ev".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-ev-s".to_string(),
                target_node_id: "gn-ev-t".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("create edge");

        // Query from wrong scope
        let err = store
            .graph_edge_view("ge-ev", "default")
            .expect_err("should reject out-of-scope view");
        assert!(
            err.to_string().contains("not"),
            "should mention scope mismatch: {err}"
        );
    }

    #[test]
    fn graph_node_status_preserves_correlation_id() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-corr".to_string()),
                scope_key: None,
                correlation_id: "corr-create".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Corr Test".to_string(),
                summary: "Testing correlation".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create node");

        store
            .update_graph_node_status(UpdateGraphNodeStatusInput {
                node_id: "gn-corr".to_string(),
                correlation_id: "corr-status-1".to_string(),
                active_scope_key: None,
                status: "completed".to_string(),
                run_id: Some("run-1".to_string()),
            })
            .expect("update status");

        let events = store.list_events().expect("list events");
        let status_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "graph-node.status-updated")
            .collect();
        assert_eq!(status_events.len(), 1);
        assert_eq!(status_events[0].correlation_id, "corr-status-1");
    }

    #[test]
    fn graph_node_revise_preserves_correlation_id() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-rev-corr".to_string()),
                scope_key: None,
                correlation_id: "corr-create".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Revise Corr".to_string(),
                summary: "Testing revise correlation".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create node");

        store
            .revise_graph_node(ReviseGraphNodeInput {
                node_id: "gn-rev-corr".to_string(),
                correlation_id: "corr-revise-1".to_string(),
                active_scope_key: None,
                title: Some("Revised Title".to_string()),
                summary: None,
                status: None,
                payload: None,
                tags: None,
                clear_tags: false,
                run_id: Some("run-1".to_string()),
            })
            .expect("revise node");

        let events = store.list_events().expect("list events");
        let revise_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "graph-node.revised")
            .collect();
        assert_eq!(revise_events.len(), 1);
        assert_eq!(revise_events[0].correlation_id, "corr-revise-1");
    }

    #[test]
    fn graph_edge_status_preserves_correlation_id() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-es-src".to_string()),
                scope_key: None,
                correlation_id: "corr-es".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Edge Status Src".to_string(),
                summary: "Src".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create src");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-es-tgt".to_string()),
                scope_key: None,
                correlation_id: "corr-es".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Edge Status Tgt".to_string(),
                summary: "Tgt".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create tgt");
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-es-corr".to_string()),
                scope_key: None,
                correlation_id: "corr-es".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-es-src".to_string(),
                target_node_id: "gn-es-tgt".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("create edge");

        store
            .update_graph_edge_status(UpdateGraphEdgeStatusInput {
                edge_id: "ge-es-corr".to_string(),
                correlation_id: "corr-status-e1".to_string(),
                active_scope_key: None,
                status: "completed".to_string(),
                run_id: Some("run-e1".to_string()),
            })
            .expect("update edge status");

        let events = store.list_events().expect("list events");
        let status_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "graph-edge.status-updated")
            .collect();
        assert_eq!(status_events.len(), 1);
        assert_eq!(status_events[0].correlation_id, "corr-status-e1");
    }

    #[test]
    fn graph_edge_revise_preserves_correlation_id() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-er-src".to_string()),
                scope_key: None,
                correlation_id: "corr-er".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Edge Revise Src".to_string(),
                summary: "Src".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create src");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-er-tgt".to_string()),
                scope_key: None,
                correlation_id: "corr-er".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Edge Revise Tgt".to_string(),
                summary: "Tgt".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create tgt");
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-er-corr".to_string()),
                scope_key: None,
                correlation_id: "corr-er".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-er-src".to_string(),
                target_node_id: "gn-er-tgt".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("create edge");

        store
            .revise_graph_edge(ReviseGraphEdgeInput {
                edge_id: "ge-er-corr".to_string(),
                correlation_id: "corr-revise-e1".to_string(),
                active_scope_key: None,
                status: None,
                payload: Some(serde_json::json!({"note": "revised"})),
                run_id: Some("run-e1".to_string()),
            })
            .expect("revise edge");

        let events = store.list_events().expect("list events");
        let revise_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "graph-edge.revised")
            .collect();
        assert_eq!(revise_events.len(), 1);
        assert_eq!(revise_events[0].correlation_id, "corr-revise-e1");
    }

    #[test]
    fn create_graph_edge_allows_inactive_duplicates() {
        // Phase 5: inactive graph edges may exist as proposals without
        // enforcing active-only duplicate invariants.
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-id-src".to_string()),
                scope_key: None,
                correlation_id: "corr-id".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Inactive Dup Src".to_string(),
                summary: "Src".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create src");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-id-tgt".to_string()),
                scope_key: None,
                correlation_id: "corr-id".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Inactive Dup Tgt".to_string(),
                summary: "Tgt".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create tgt");

        // Create first inactive edge
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-id-1".to_string()),
                scope_key: None,
                correlation_id: "corr-id".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-id-src".to_string(),
                target_node_id: "gn-id-tgt".to_string(),
                status: "proposed".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("create first inactive edge");

        // Create second inactive edge for same src/target/kind — should succeed
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-id-2".to_string()),
                scope_key: None,
                correlation_id: "corr-id".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-id-src".to_string(),
                target_node_id: "gn-id-tgt".to_string(),
                status: "proposed".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("create second inactive edge — duplicates allowed when inactive");

        // Creating an active edge for same src/target/kind (with no active duplicates) should succeed
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-id-3".to_string()),
                scope_key: None,
                correlation_id: "corr-id".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-id-src".to_string(),
                target_node_id: "gn-id-tgt".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("active edge succeeds when only inactive duplicates exist");

        // Creating another active edge for same src/target/kind should now reject
        let err = store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-id-4".to_string()),
                scope_key: None,
                correlation_id: "corr-id".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-id-src".to_string(),
                target_node_id: "gn-id-tgt".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect_err("active duplicate should be rejected");
        assert!(
            err.to_string().contains("duplicate"),
            "should mention duplicate: {err}"
        );
    }

    #[test]
    fn create_graph_edge_allows_inactive_potential_cycle() {
        // Phase 5: inactive graph edges bypass active-only cycle detection.
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-ic-a".to_string()),
                scope_key: None,
                correlation_id: "corr-ic".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Inactive Cycle A".to_string(),
                summary: "Node A".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create a");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("gn-ic-b".to_string()),
                scope_key: None,
                correlation_id: "corr-ic".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Inactive Cycle B".to_string(),
                summary: "Node B".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create b");

        // Create active edge A → B
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-ic-ab".to_string()),
                scope_key: None,
                correlation_id: "corr-ic".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-ic-a".to_string(),
                target_node_id: "gn-ic-b".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("create a→b");

        // B → A (active) would create a cycle — should reject
        let err = store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-ic-ba-active".to_string()),
                scope_key: None,
                correlation_id: "corr-ic".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-ic-b".to_string(),
                target_node_id: "gn-ic-a".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect_err("active cycle should be rejected");
        assert!(
            err.to_string().contains("cycle"),
            "should mention cycle: {err}"
        );

        // B → A (proposed/inactive) — should succeed since inactive edges bypass cycle check
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-ic-ba-proposed".to_string()),
                scope_key: None,
                correlation_id: "corr-ic".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "gn-ic-b".to_string(),
                target_node_id: "gn-ic-a".to_string(),
                status: "proposed".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("inactive cycle should be allowed as proposal");
    }

    #[test]
    fn acceptance_abstract_round_trip() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        let result = store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("acc-abstract".to_string()),
                scope_key: None,
                correlation_id: "corr-acc".to_string(),
                title: "All tests pass".to_string(),
                summary: "Acceptance criterion for test suite".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Abstract,
                description: "Test suite must pass 100%".to_string(),
                verification_policy: "automated".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create abstract acceptance");

        assert_eq!(result.record.id, "acc-abstract");
        assert_eq!(result.record.kind, PlanningNodeKind::Acceptance);
        assert_eq!(
            result.record.payload["acceptanceKind"]
                .as_str()
                .expect("acceptanceKind is str"),
            "abstract"
        );

        // Verify stored as graph node
        let node = load_graph_node(
            &store.open_connection().expect("open connection"),
            "acc-abstract",
        )
        .expect("load node");
        assert_eq!(node.kind, PlanningNodeKind::Acceptance);
    }

    #[test]
    fn acceptance_concrete_round_trip() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        let result = store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("acc-concrete".to_string()),
                scope_key: None,
                correlation_id: "corr-acc".to_string(),
                title: "Verify login endpoint".to_string(),
                summary: "Concrete check for login".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Concrete,
                description: "POST /login returns 200".to_string(),
                verification_policy: "test-driven".to_string(),
                required_evidence_kinds: vec![EvidenceKind::TestResult],
                waiver: None,
                tags: vec!["security".to_string()],
                run_id: None,
            })
            .expect("create concrete acceptance");

        assert_eq!(result.record.id, "acc-concrete");
        assert_eq!(
            result.record.payload["acceptanceKind"]
                .as_str()
                .expect("acceptanceKind is str"),
            "concrete"
        );
        assert!(result.record.payload["requiredEvidenceKinds"].is_array());
    }

    #[test]
    fn evidence_round_trip() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        let result = store
            .create_evidence(CreateEvidenceInput {
                id: Some("ev-test".to_string()),
                scope_key: None,
                correlation_id: "corr-ev".to_string(),
                title: "Login test results".to_string(),
                summary: "All login tests passed".to_string(),
                status: "active".to_string(),
                evidence_kind: EvidenceKind::TestResult,
                reference: "ci/build-42".to_string(),
                content: "42 passed, 0 failed".to_string(),
                captured_at: "2026-06-01T12:00:00Z".to_string(),
                tags: vec!["ci".to_string()],
                run_id: None,
            })
            .expect("create evidence");

        assert_eq!(result.record.id, "ev-test");
        assert_eq!(result.record.kind, PlanningNodeKind::Evidence);
        assert_eq!(
            result.record.payload["evidenceKind"]
                .as_str()
                .expect("evidenceKind is str"),
            "test-result"
        );
    }

    #[test]
    fn acceptance_satisfy_multiple_abstracts() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("abs-1".to_string()),
                scope_key: None,
                correlation_id: "corr-sat".to_string(),
                title: "Abs 1".to_string(),
                summary: "s".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Abstract,
                description: "d".to_string(),
                verification_policy: "v".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create abstract 1");
        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("abs-2".to_string()),
                scope_key: None,
                correlation_id: "corr-sat".to_string(),
                title: "Abs 2".to_string(),
                summary: "s".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Abstract,
                description: "d".to_string(),
                verification_policy: "v".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create abstract 2");
        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("conc-1".to_string()),
                scope_key: None,
                correlation_id: "corr-sat".to_string(),
                title: "Conc 1".to_string(),
                summary: "s".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Concrete,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create concrete");

        // Satisfy both abstracts
        for (abs_id, rationale) in [
            ("abs-1", "Concrete check covers this requirement"),
            ("abs-2", "Same check also addresses this"),
        ] {
            let edge_id = format!("sat-{abs_id}");
            store
                .satisfy_acceptance(SatisfyAcceptanceInput {
                    id: Some(edge_id),
                    scope_key: None,
                    correlation_id: "corr-sat".to_string(),
                    concrete_node_id: "conc-1".to_string(),
                    abstract_node_id: abs_id.to_string(),
                    rationale: rationale.to_string(),
                    run_id: None,
                })
                .expect("satisfy abstract");
        }

        // View shows both satisfied abstracts
        let view = store.acceptance_view("conc-1", "default").expect("view");
        assert_eq!(view.satisfied_abstracts.len(), 2);
    }

    #[test]
    fn acceptance_view_includes_attached_evidence() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("acc-ea".to_string()),
                scope_key: None,
                correlation_id: "corr-ea".to_string(),
                title: "Acceptance with evidence".to_string(),
                summary: "test summary".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Concrete,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create acceptance");
        store
            .create_evidence(CreateEvidenceInput {
                id: Some("ev-ea".to_string()),
                scope_key: None,
                correlation_id: "corr-ea".to_string(),
                title: "Evidence".to_string(),
                summary: "test summary".to_string(),
                status: "active".to_string(),
                evidence_kind: EvidenceKind::TestResult,
                reference: "".to_string(),
                content: "".to_string(),
                captured_at: "".to_string(),
                tags: vec![],
                run_id: None,
            })
            .expect("create evidence");

        store
            .attach_evidence(AttachEvidenceInput {
                id: Some("edge-ea".to_string()),
                scope_key: None,
                correlation_id: "corr-ea".to_string(),
                evidence_node_id: "ev-ea".to_string(),
                target_node_id: "acc-ea".to_string(),
                rationale: "Test results prove acceptance".to_string(),
                run_id: None,
            })
            .expect("attach evidence");

        let view = store.acceptance_view("acc-ea", "default").expect("view");
        assert_eq!(view.attached_evidence.len(), 1);
        assert_eq!(view.attached_evidence[0].id, "ev-ea");
    }

    #[test]
    fn abstract_acceptance_without_coverage_emits_warning() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("abs-uncov".to_string()),
                scope_key: None,
                correlation_id: "corr-cov".to_string(),
                title: "Uncovered abstract".to_string(),
                summary: "test summary".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Abstract,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create abstract");

        let conn = store.open_connection().expect("open connection");
        let findings =
            validate_entity(&conn, EntityType::GraphNode, "abs-uncov").expect("validate");
        let has_coverage_warning = findings
            .iter()
            .any(|f| f.code == "ACCEPTANCE-COVERAGE-MISSING");
        assert!(
            has_coverage_warning,
            "abstract without coverage should warn"
        );
    }

    #[test]
    fn concrete_acceptance_missing_evidence_emits_warning() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("acc-evmiss".to_string()),
                scope_key: None,
                correlation_id: "corr-em".to_string(),
                title: "Needs evidence".to_string(),
                summary: "test summary".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Concrete,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![EvidenceKind::TestResult],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create concrete");

        let conn = store.open_connection().expect("open connection");
        let findings =
            validate_entity(&conn, EntityType::GraphNode, "acc-evmiss").expect("validate");
        let has_evidence_warning = findings
            .iter()
            .any(|f| f.code == "ACCEPTANCE-EVIDENCE-MISSING");
        assert!(has_evidence_warning, "should warn about missing evidence");

        // Attach evidence of required kind
        store
            .create_evidence(CreateEvidenceInput {
                id: Some("ev-fix".to_string()),
                scope_key: None,
                correlation_id: "corr-em".to_string(),
                title: "Evidence".to_string(),
                summary: "test summary".to_string(),
                status: "active".to_string(),
                evidence_kind: EvidenceKind::TestResult,
                reference: "".to_string(),
                content: "".to_string(),
                captured_at: "".to_string(),
                tags: vec![],
                run_id: None,
            })
            .expect("create evidence");
        store
            .attach_evidence(AttachEvidenceInput {
                id: Some("edge-fix".to_string()),
                scope_key: None,
                correlation_id: "corr-em".to_string(),
                evidence_node_id: "ev-fix".to_string(),
                target_node_id: "acc-evmiss".to_string(),
                rationale: "Coverage".to_string(),
                run_id: None,
            })
            .expect("attach evidence");

        // Re-validate — warning should be gone
        let conn2 = store.open_connection().expect("open connection");
        let findings2 =
            validate_entity(&conn2, EntityType::GraphNode, "acc-evmiss").expect("validate2");
        let still_warns = findings2
            .iter()
            .any(|f| f.code == "ACCEPTANCE-EVIDENCE-MISSING");
        assert!(!still_warns, "warning should clear after evidence attached");
    }

    #[test]
    fn satisfies_edge_without_rationale_emits_warning() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("abs-rat".to_string()),
                scope_key: None,
                correlation_id: "corr-rat".to_string(),
                title: "Abstract".to_string(),
                summary: "test summary".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Abstract,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create abstract");
        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("conc-rat".to_string()),
                scope_key: None,
                correlation_id: "corr-rat".to_string(),
                title: "Concrete".to_string(),
                summary: "test summary".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Concrete,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create concrete");

        // Create satisfies edge with empty rationale via direct graph_edge to bypass typed validation
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-rat".to_string()),
                scope_key: None,
                correlation_id: "corr-rat".to_string(),
                kind: PlanningEdgeKind::Satisfies,
                source_node_id: "conc-rat".to_string(),
                target_node_id: "abs-rat".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({"rationale": ""}),
                run_id: None,
            })
            .expect("create edge with empty rationale");

        let conn = store.open_connection().expect("open connection");
        let findings = validate_entity(&conn, EntityType::GraphEdge, "ge-rat").expect("validate");
        let has_rationale_warning = findings
            .iter()
            .any(|f| f.code == "ACCEPTANCE-RATIONALE-MISSING");
        assert!(has_rationale_warning, "empty rationale should warn");
    }

    #[test]
    fn invalid_acceptance_kind_emits_validation_warning() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("acc-bad".to_string()),
                scope_key: None,
                correlation_id: "corr-bad".to_string(),
                kind: PlanningNodeKind::Acceptance,
                title: "Bad acceptance".to_string(),
                summary: "test summary".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({"acceptanceKind": "invalid-kind"}),
                tags: vec![],
                run_id: None,
            })
            .expect("create bad acceptance node");

        let conn = store.open_connection().expect("open connection");
        let findings = validate_entity(&conn, EntityType::GraphNode, "acc-bad").expect("validate");
        let has_kind_warning = findings.iter().any(|f| f.code == "ACCEPTANCE-KIND-INVALID");
        assert!(has_kind_warning, "invalid acceptanceKind should warn");
    }

    #[test]
    fn invalid_evidence_kind_emits_validation_warning() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("ev-bad".to_string()),
                scope_key: None,
                correlation_id: "corr-bad2".to_string(),
                kind: PlanningNodeKind::Evidence,
                title: "Bad evidence".to_string(),
                summary: "test summary".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({"evidenceKind": "not-a-real-kind"}),
                tags: vec![],
                run_id: None,
            })
            .expect("create bad evidence node");

        let conn = store.open_connection().expect("open connection");
        let findings = validate_entity(&conn, EntityType::GraphNode, "ev-bad").expect("validate");
        let has_kind_warning = findings.iter().any(|f| f.code == "EVIDENCE-KIND-INVALID");
        assert!(has_kind_warning, "invalid evidenceKind should warn");
    }

    #[test]
    fn cross_scope_acceptance_view_rejects() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_scope(CreateScopeInput {
                scope_key: "ws-xs".to_string(),
                scope_type: Some("workspace".to_string()),
                parent_scope_key: None,
                metadata: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create scope");

        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("acc-xs".to_string()),
                scope_key: Some("ws-xs".to_string()),
                correlation_id: "corr-xs".to_string(),
                title: "Scoped acceptance".to_string(),
                summary: "test summary".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Abstract,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create scoped acceptance");

        let err = store
            .acceptance_view("acc-xs", "default")
            .expect_err("should reject cross-scope view");
        assert!(
            err.to_string().contains("not"),
            "should mention scope mismatch: {err}"
        );
    }

    #[test]
    fn evidence_view_shows_attached_targets() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("acc-ev-view".to_string()),
                scope_key: None,
                correlation_id: "corr-ev-v".to_string(),
                title: "Target acceptance".to_string(),
                summary: "test summary".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Concrete,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create acceptance");
        store
            .create_evidence(CreateEvidenceInput {
                id: Some("ev-ev-view".to_string()),
                scope_key: None,
                correlation_id: "corr-ev-v".to_string(),
                title: "View evidence".to_string(),
                summary: "test summary".to_string(),
                status: "active".to_string(),
                evidence_kind: EvidenceKind::Review,
                reference: "".to_string(),
                content: "".to_string(),
                captured_at: "".to_string(),
                tags: vec![],
                run_id: None,
            })
            .expect("create evidence");

        store
            .attach_evidence(AttachEvidenceInput {
                id: Some("edge-ev-view".to_string()),
                scope_key: None,
                correlation_id: "corr-ev-v".to_string(),
                evidence_node_id: "ev-ev-view".to_string(),
                target_node_id: "acc-ev-view".to_string(),
                rationale: "Reviewed".to_string(),
                run_id: None,
            })
            .expect("attach");

        let view = store
            .evidence_view("ev-ev-view", "default")
            .expect("evidence view");
        assert_eq!(view.attached_to.len(), 1);
        assert_eq!(view.attached_to[0].id, "acc-ev-view");
    }

    #[test]
    fn satisfy_acceptance_rejects_abstract_to_abstract() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("abs-a1".to_string()),
                scope_key: None,
                correlation_id: "corr-dir".to_string(),
                title: "Abs A1".to_string(),
                summary: "Abstract acceptance".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Abstract,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create abstract 1");
        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("abs-a2".to_string()),
                scope_key: None,
                correlation_id: "corr-dir".to_string(),
                title: "Abs A2".to_string(),
                summary: "Another abstract".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Abstract,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create abstract 2");

        let err = store
            .satisfy_acceptance(SatisfyAcceptanceInput {
                id: Some("sat-bad-aa".to_string()),
                scope_key: None,
                correlation_id: "corr-dir".to_string(),
                concrete_node_id: "abs-a1".to_string(),
                abstract_node_id: "abs-a2".to_string(),
                rationale: "Should fail".to_string(),
                run_id: None,
            })
            .expect_err("abstract→abstract should be rejected");
        assert!(
            err.to_string().contains("concrete"),
            "should mention concrete: {err}"
        );
    }

    #[test]
    fn satisfy_acceptance_rejects_concrete_to_concrete() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("conc-c1".to_string()),
                scope_key: None,
                correlation_id: "corr-dir2".to_string(),
                title: "Conc C1".to_string(),
                summary: "Concrete acceptance".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Concrete,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create concrete 1");
        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("conc-c2".to_string()),
                scope_key: None,
                correlation_id: "corr-dir2".to_string(),
                title: "Conc C2".to_string(),
                summary: "Another concrete".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Concrete,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create concrete 2");

        let err = store
            .satisfy_acceptance(SatisfyAcceptanceInput {
                id: Some("sat-bad-cc".to_string()),
                scope_key: None,
                correlation_id: "corr-dir2".to_string(),
                concrete_node_id: "conc-c1".to_string(),
                abstract_node_id: "conc-c2".to_string(),
                rationale: "Should fail".to_string(),
                run_id: None,
            })
            .expect_err("concrete→concrete should be rejected");
        assert!(
            err.to_string().contains("abstract"),
            "should mention abstract: {err}"
        );
    }

    #[test]
    fn satisfy_acceptance_rejects_non_acceptance_kind() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("work-nac".to_string()),
                scope_key: None,
                correlation_id: "corr-nac".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Work node".to_string(),
                summary: "Not an acceptance".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create work");
        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("abs-nac".to_string()),
                scope_key: None,
                correlation_id: "corr-nac".to_string(),
                title: "Abstract".to_string(),
                summary: "Acceptance".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Abstract,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create abstract");

        // Work node as "concrete" source
        let err = store
            .satisfy_acceptance(SatisfyAcceptanceInput {
                id: Some("sat-nac-src".to_string()),
                scope_key: None,
                correlation_id: "corr-nac".to_string(),
                concrete_node_id: "work-nac".to_string(),
                abstract_node_id: "abs-nac".to_string(),
                rationale: "Should fail".to_string(),
                run_id: None,
            })
            .expect_err("non-acceptance source should be rejected");
        assert!(
            err.to_string().contains("acceptance"),
            "should mention acceptance: {err}"
        );

        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("conc-nac".to_string()),
                scope_key: None,
                correlation_id: "corr-nac".to_string(),
                title: "Concrete".to_string(),
                summary: "Acceptance".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Concrete,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create concrete");

        // Work node as "abstract" target
        let err2 = store
            .satisfy_acceptance(SatisfyAcceptanceInput {
                id: Some("sat-nac-tgt".to_string()),
                scope_key: None,
                correlation_id: "corr-nac".to_string(),
                concrete_node_id: "conc-nac".to_string(),
                abstract_node_id: "work-nac".to_string(),
                rationale: "Should fail".to_string(),
                run_id: None,
            })
            .expect_err("non-acceptance target should be rejected");
        assert!(
            err2.to_string().contains("acceptance"),
            "should mention acceptance: {err2}"
        );
    }

    #[test]
    fn acceptance_view_rejects_non_acceptance_node() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("work-av".to_string()),
                scope_key: None,
                correlation_id: "corr-av".to_string(),
                kind: PlanningNodeKind::Work,
                title: "A work node".to_string(),
                summary: "Not an acceptance".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create work node");

        let err = store
            .acceptance_view("work-av", "default")
            .expect_err("should reject non-acceptance node in acceptance view");
        assert!(
            err.to_string().contains("acceptance"),
            "should mention acceptance: {err}"
        );
    }

    #[test]
    fn evidence_view_rejects_non_evidence_node() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("acc-ev".to_string()),
                scope_key: None,
                correlation_id: "corr-ev2".to_string(),
                title: "Acceptance".to_string(),
                summary: "Acceptance node".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Concrete,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create acceptance");

        let err = store
            .evidence_view("acc-ev", "default")
            .expect_err("should reject non-evidence node in evidence view");
        assert!(
            err.to_string().contains("evidence"),
            "should mention evidence: {err}"
        );
    }

    #[test]
    fn abstract_coverage_ignores_non_concrete_source() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("abs-cov".to_string()),
                scope_key: None,
                correlation_id: "corr-cov2".to_string(),
                title: "Abstract".to_string(),
                summary: "Needs concrete coverage".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Abstract,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create abstract");

        // Create another abstract acceptance and a Satisfies edge from it to the first abstract
        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("abs-other".to_string()),
                scope_key: None,
                correlation_id: "corr-cov2".to_string(),
                title: "Other Abstract".to_string(),
                summary: "Another abstract acceptance".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Abstract,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create other abstract");

        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-cov-bad".to_string()),
                scope_key: None,
                correlation_id: "corr-cov2".to_string(),
                kind: PlanningEdgeKind::Satisfies,
                source_node_id: "abs-other".to_string(),
                target_node_id: "abs-cov".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({"rationale": "pretend"}),
                run_id: None,
            })
            .expect("create satisfies edge between abstracts");

        // Validate — should still warn ACCEPTANCE-COVERAGE-MISSING because source is abstract, not concrete
        let conn = store.open_connection().expect("open connection");
        let findings = validate_entity(&conn, EntityType::GraphNode, "abs-cov").expect("validate");
        let has_coverage_warning = findings
            .iter()
            .any(|f| f.code == "ACCEPTANCE-COVERAGE-MISSING");
        assert!(
            has_coverage_warning,
            "should still warn: abstract source doesn't count as concrete coverage"
        );
    }

    #[test]
    fn finalize_graph_node_rejects_abstract_without_coverage() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("abs-final".to_string()),
                scope_key: None,
                correlation_id: "corr-fin".to_string(),
                title: "Abstract for finalize".to_string(),
                summary: "No coverage".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Abstract,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create abstract");

        let err = store
            .finalize_graph_node(FinalizeGraphNodeInput {
                node_id: "abs-final".to_string(),
                correlation_id: "corr-fin".to_string(),
                active_scope_key: None,
                status: "validated".to_string(),
                accepted_risk: None,
                run_id: None,
            })
            .expect_err("should reject abstract without coverage");
        assert!(
            err.to_string().contains("ACCEPTANCE-COVERAGE-MISSING"),
            "should mention coverage: {err}"
        );
    }

    #[test]
    fn finalize_graph_node_succeeds_with_coverage() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("abs-cov-final".to_string()),
                scope_key: None,
                correlation_id: "corr-fin2".to_string(),
                title: "Covered abstract".to_string(),
                summary: "Has coverage".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Abstract,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create abstract");
        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("conc-cov-final".to_string()),
                scope_key: None,
                correlation_id: "corr-fin2".to_string(),
                title: "Concrete for abstract".to_string(),
                summary: "Provides coverage".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Concrete,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create concrete");
        store
            .satisfy_acceptance(SatisfyAcceptanceInput {
                id: Some("sat-final".to_string()),
                scope_key: None,
                correlation_id: "corr-fin2".to_string(),
                concrete_node_id: "conc-cov-final".to_string(),
                abstract_node_id: "abs-cov-final".to_string(),
                rationale: "Verified".to_string(),
                run_id: None,
            })
            .expect("satisfy");

        let result = store
            .finalize_graph_node(FinalizeGraphNodeInput {
                node_id: "abs-cov-final".to_string(),
                correlation_id: "corr-fin2".to_string(),
                active_scope_key: None,
                status: "validated".to_string(),
                accepted_risk: None,
                run_id: None,
            })
            .expect("should succeed with coverage");
        assert_eq!(result.record.status, "validated");
    }

    #[test]
    fn finalize_graph_node_accepted_risk_allows_gap() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("abs-risk".to_string()),
                scope_key: None,
                correlation_id: "corr-risk".to_string(),
                title: "Acceptance with risk".to_string(),
                summary: "No coverage but accepted risk".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Abstract,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create abstract");

        let result = store
            .finalize_graph_node(FinalizeGraphNodeInput {
                node_id: "abs-risk".to_string(),
                correlation_id: "corr-risk".to_string(),
                active_scope_key: None,
                status: "validated".to_string(),
                accepted_risk: Some("Accepted by team lead: coverage deferred to Q2".to_string()),
                run_id: Some("run-risk".to_string()),
            })
            .expect("should succeed with accepted risk");
        assert_eq!(result.record.status, "validated");

        // Verify event has accepted risk
        let events = store.list_events().expect("list events");
        let finalize_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "graph-node.finalized-with-accepted-risk")
            .collect();
        assert_eq!(finalize_events.len(), 1);
        assert_eq!(finalize_events[0].correlation_id, "corr-risk");
        assert!(finalize_events[0].payload["acceptedRisk"]
            .as_str()
            .unwrap_or("")
            .contains("Accepted by team lead"));
    }

    #[test]
    fn finalize_graph_node_accepted_risk_does_not_bypass_structural() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        // Create two work nodes
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("work-s1".to_string()),
                scope_key: None,
                correlation_id: "corr-struct".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Structural S1".to_string(),
                summary: "Source".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create s1");
        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("work-s2".to_string()),
                scope_key: None,
                correlation_id: "corr-struct".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Structural S2".to_string(),
                summary: "Target".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create s2");

        // Create a DependsOn edge: s1 --depends-on--> s2
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-cycle1".to_string()),
                scope_key: None,
                correlation_id: "corr-struct".to_string(),
                kind: PlanningEdgeKind::DependsOn,
                source_node_id: "work-s1".to_string(),
                target_node_id: "work-s2".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                run_id: None,
            })
            .expect("create first edge");
        // Create a reverse DependsOn edge: s2 --depends-on--> s1 to form a cycle.
        // We bypass the create-time cycle check via direct SQL.
        let conn = store.open_connection().expect("open connection");
        conn.execute(
            "INSERT INTO planning_edges (id, scope_key, kind, source_node_id, target_node_id, status, payload_json, revision, created_at, updated_at)
             VALUES (?1, 'default', ?2, ?3, ?4, 'active', '{}', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            rusqlite::params!["ge-cycle2", PlanningEdgeKind::DependsOn.as_str(), "work-s2", "work-s1"],
        ).expect("insert second edge via SQL (bypassing cycle check)");

        // Now try to finalize with accepted risk — should still reject because cycle is structural
        let err = store
            .finalize_graph_node(FinalizeGraphNodeInput {
                node_id: "work-s1".to_string(),
                correlation_id: "corr-struct".to_string(),
                active_scope_key: None,
                status: "completed".to_string(),
                accepted_risk: Some("I accept the risks".to_string()),
                run_id: None,
            })
            .expect_err("should reject structural even with accepted risk");
        assert!(
            err.to_string().contains("structural"),
            "should mention structural: {err}"
        );
    }

    #[test]
    fn finalize_graph_node_accepted_risk_does_not_bypass_invalid_acceptance_kind() {
        // Phase 7 Fixup: type integrity is never waivable
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("acc-bad-kind".to_string()),
                scope_key: None,
                correlation_id: "corr-ti".to_string(),
                kind: PlanningNodeKind::Acceptance,
                title: "Bad acceptance kind".to_string(),
                summary: "Malformed payload".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({"acceptanceKind": "not-a-real-kind"}),
                tags: vec![],
                run_id: None,
            })
            .expect("create bad acceptance");

        let err = store
            .finalize_graph_node(FinalizeGraphNodeInput {
                node_id: "acc-bad-kind".to_string(),
                correlation_id: "corr-ti".to_string(),
                active_scope_key: None,
                status: "validated".to_string(),
                accepted_risk: Some("I accept the risks".to_string()),
                run_id: None,
            })
            .expect_err("should reject type integrity even with accepted risk");
        assert!(
            err.to_string().contains("invalid typed payloads"),
            "should mention typed payloads: {err}"
        );
    }

    #[test]
    fn finalize_graph_node_accepted_risk_does_not_bypass_invalid_evidence_kind() {
        // Phase 7 Fixup: type integrity is never waivable
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("ev-bad-kind".to_string()),
                scope_key: None,
                correlation_id: "corr-ti2".to_string(),
                kind: PlanningNodeKind::Evidence,
                title: "Bad evidence kind".to_string(),
                summary: "Malformed payload".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({"evidenceKind": "not-a-real-kind"}),
                tags: vec![],
                run_id: None,
            })
            .expect("create bad evidence");

        let err = store
            .finalize_graph_node(FinalizeGraphNodeInput {
                node_id: "ev-bad-kind".to_string(),
                correlation_id: "corr-ti2".to_string(),
                active_scope_key: None,
                status: "validated".to_string(),
                accepted_risk: Some("I accept the risks".to_string()),
                run_id: None,
            })
            .expect_err("should reject type integrity even with accepted risk");
        assert!(
            err.to_string().contains("invalid typed payloads"),
            "should mention typed payloads: {err}"
        );
    }

    #[test]
    fn finalize_graph_node_appends_event() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_graph_node(CreateGraphNodeInput {
                id: Some("work-final-ev".to_string()),
                scope_key: None,
                correlation_id: "corr-ev-fin".to_string(),
                kind: PlanningNodeKind::Work,
                title: "Finalize event test".to_string(),
                summary: "Testing events".to_string(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
                tags: vec![],
                run_id: None,
            })
            .expect("create work");

        store
            .finalize_graph_node(FinalizeGraphNodeInput {
                node_id: "work-final-ev".to_string(),
                correlation_id: "corr-ev-fin".to_string(),
                active_scope_key: None,
                status: "completed".to_string(),
                accepted_risk: None,
                run_id: Some("run-fin".to_string()),
            })
            .expect("finalize");

        let events = store.list_events().expect("list events");
        let finalize_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "graph-node.finalized")
            .collect();
        assert_eq!(finalize_events.len(), 1);
        assert_eq!(finalize_events[0].correlation_id, "corr-ev-fin");
        assert_eq!(finalize_events[0].run_id, "run-fin");
    }

    #[test]
    fn acceptance_view_excludes_inactive_links() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init");

        store
            .create_evidence(CreateEvidenceInput {
                id: Some("ev-inactive".to_string()),
                scope_key: None,
                correlation_id: "corr-av".to_string(),
                title: "Inactive evidence".to_string(),
                summary: "Should be excluded".to_string(),
                status: "active".to_string(),
                evidence_kind: EvidenceKind::TestResult,
                reference: "".to_string(),
                content: "".to_string(),
                captured_at: "".to_string(),
                tags: vec![],
                run_id: None,
            })
            .expect("create evidence");
        store
            .create_acceptance(CreateAcceptanceInput {
                id: Some("acc-av".to_string()),
                scope_key: None,
                correlation_id: "corr-av".to_string(),
                title: "Acceptance view test".to_string(),
                summary: "With inactive link".to_string(),
                status: "active".to_string(),
                acceptance_kind: AcceptanceKind::Concrete,
                description: "".to_string(),
                verification_policy: "".to_string(),
                required_evidence_kinds: vec![],
                waiver: None,
                tags: vec![],
                run_id: None,
            })
            .expect("create acceptance");

        // Attach evidence with inactive edge (via direct graph edge)
        store
            .create_graph_edge(CreateGraphEdgeInput {
                id: Some("ge-av-inactive".to_string()),
                scope_key: None,
                correlation_id: "corr-av".to_string(),
                kind: PlanningEdgeKind::EvidencedBy,
                source_node_id: "acc-av".to_string(),
                target_node_id: "ev-inactive".to_string(),
                status: "proposed".to_string(),
                payload: serde_json::json!({"rationale": "inactive link"}),
                run_id: None,
            })
            .expect("create inactive evidenced-by edge");

        let view = store.acceptance_view("acc-av", "default").expect("view");
        assert!(
            view.attached_evidence.is_empty(),
            "inactive links should be excluded"
        );

        // Activate the edge
        store
            .update_graph_edge_status(UpdateGraphEdgeStatusInput {
                edge_id: "ge-av-inactive".to_string(),
                correlation_id: "corr-av".to_string(),
                active_scope_key: None,
                status: "active".to_string(),
                run_id: None,
            })
            .expect("activate edge");

        let view2 = store.acceptance_view("acc-av", "default").expect("view2");
        assert_eq!(
            view2.attached_evidence.len(),
            1,
            "active links should be included"
        );
    }

    #[test]
    fn project_run_claim_replays_identical_idempotency_key() {
        let dir = tempdir().expect("tempdir");
        let store = PlanningStore::new(dir.path().join("planning.db"));
        store.init().expect("init");
        create_lease_fixture(&store);

        let first = store
            .claim_project_run(lease_claim("lease-run-1", "owner-a", "claim-key", 30))
            .expect("first claim");
        let replay = store
            .claim_project_run(lease_claim(
                "different-request-id",
                "owner-a",
                "claim-key",
                30,
            ))
            .expect("idempotent replay");

        assert_eq!(first.record.id, replay.record.id);
        assert_eq!(first.record.fencing_token, replay.record.fencing_token);

        let conflict = store
            .claim_project_run(lease_claim("lease-run-2", "owner-b", "claim-key", 30))
            .expect_err("different payload must conflict");
        assert!(conflict
            .to_string()
            .contains("PROJECT-RUN-IDEMPOTENCY-CONFLICT"));
    }

    #[test]
    fn project_run_heartbeat_extends_lease_and_requires_fence() {
        let dir = tempdir().expect("tempdir");
        let store = PlanningStore::new(dir.path().join("planning.db"));
        store.init().expect("init");
        create_lease_fixture(&store);

        let claim = store
            .claim_project_run(lease_claim("lease-run-1", "owner-a", "heartbeat-key", 1))
            .expect("claim");
        thread::sleep(StdDuration::from_millis(500));
        let heartbeat = store
            .heartbeat_project_run(HeartbeatProjectRunInput {
                project_run_id: claim.record.id.clone(),
                active_scope_key: None,
                run_id: None,
                fencing_token: Some(claim.record.fencing_token),
                lease_seconds: Some(3),
            })
            .expect("heartbeat");
        assert!(heartbeat.record.lease_expires_at > claim.record.lease_expires_at);

        thread::sleep(StdDuration::from_millis(700));
        let active = store
            .activate_project_run(ActivateProjectRunInput {
                project_run_id: claim.record.id,
                active_scope_key: None,
                run_id: None,
                fencing_token: Some(claim.record.fencing_token),
            })
            .expect("heartbeat kept lease alive");
        assert_eq!(active.record.status, ProjectRunStatus::Active);
    }

    #[test]
    fn expired_owner_cannot_mutate_after_new_fence() {
        let dir = tempdir().expect("tempdir");
        let store = PlanningStore::new(dir.path().join("planning.db"));
        store.init().expect("init");
        create_lease_fixture(&store);

        let first = store
            .claim_project_run(lease_claim("lease-run-1", "owner-a", "expiry-key-a", 1))
            .expect("first claim");
        thread::sleep(StdDuration::from_millis(1_200));
        let second = store
            .claim_project_run(lease_claim("lease-run-2", "owner-b", "expiry-key-b", 30))
            .expect("claim after expiry");
        assert!(second.record.fencing_token > first.record.fencing_token);
        assert!(store
            .list_events()
            .expect("list events")
            .iter()
            .any(|event| {
                event.event_type == "project-run.expired" && event.entity_id == first.record.id
            }));

        let stale = store
            .activate_project_run(ActivateProjectRunInput {
                project_run_id: second.record.id,
                active_scope_key: None,
                run_id: None,
                fencing_token: Some(first.record.fencing_token),
            })
            .expect_err("stale fence must fail");
        assert!(stale
            .to_string()
            .contains("PROJECT-RUN-STALE-FENCING-TOKEN"));

        let expired = store
            .add_project_run_evidence(AddEvidenceInput {
                project_run_id: first.record.id,
                evidence: ProjectRunEvidence::default(),
                active_scope_key: None,
                run_id: None,
                fencing_token: Some(first.record.fencing_token),
            })
            .expect_err("expired owner must fail");
        assert!(expired.to_string().contains("PROJECT-RUN-LEASE-EXPIRED"));
    }

    #[test]
    fn simultaneous_project_run_claims_have_one_owner() {
        let dir = tempdir().expect("tempdir");
        let store = Arc::new(PlanningStore::new(dir.path().join("planning.db")));
        store.init().expect("init");
        create_lease_fixture(&store);
        let barrier = Arc::new(Barrier::new(2));
        let handles = (0..2)
            .map(|index| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    store.claim_project_run(lease_claim(
                        &format!("lease-race-{index}"),
                        &format!("owner-{index}"),
                        &format!("race-key-{index}"),
                        30,
                    ))
                })
            })
            .collect::<Vec<_>>();
        let outcomes = handles
            .into_iter()
            .map(|handle| handle.join().expect("claim thread"))
            .collect::<Vec<_>>();

        assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            store
                .list_project_runs()
                .expect("list runs")
                .into_iter()
                .filter(|run| {
                    matches!(
                        run.status,
                        ProjectRunStatus::Claimed
                            | ProjectRunStatus::Active
                            | ProjectRunStatus::Interrupted
                    )
                })
                .count(),
            1
        );
    }
}
