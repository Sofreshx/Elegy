use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row, Transaction};
use serde::Serialize;
use serde_json::Value;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

use crate::{
    validation::validate_entity, AttachWorktreeInput, EffortTier, EntityType, FileScopeRecord,
    FileScopeSelectorType, GoalRecord, GoalStatus, GoalView, InsightRecord, InsightStatus,
    InsightType, InsightView, IssueRecord, IssueStatus, IssueView, MutationResult, PlanRecord,
    PlanStatus, PlanView, PlanningEvent, PlanningHealthReport, PlanningStoreError, Priority,
    ProjectRunEvidence, ProjectRunRecord, ProjectRunStatus, ProjectRunView, ProjectionFormat,
    RenderedProjection, ReviewPointRecord, ReviewPointStatus, RoadmapRecord, RoadmapSectionRecord,
    RoadmapStatus, RoadmapView, RunnableCandidates, RunnableWorkPointCandidate, ScopeRecord,
    SessionSummary, Severity, TagInfo, TodoRecord, TodoStatus, ValidationFinding, ValidationReport,
    ValidationRunReport, ValidationSeverity, WorkGraph, WorkGraphEdge, WorkGraphNode,
    WorkPointRecord, WorkPointStatus, WorkPointView, WorktreeRecord, WorktreeStatus,
};

pub const CURRENT_SCHEMA_VERSION: &str = "7";
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
}

#[derive(Clone, Debug)]
pub struct ActivateProjectRunInput {
    pub project_run_id: String,
    pub active_scope_key: Option<String>,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ReleaseProjectRunInput {
    pub project_run_id: String,
    pub status: ProjectRunStatus,
    pub evidence: Option<ProjectRunEvidence>,
    pub active_scope_key: Option<String>,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AddEvidenceInput {
    pub project_run_id: String,
    pub evidence: ProjectRunEvidence,
    pub active_scope_key: Option<String>,
    pub run_id: Option<String>,
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
            file_scopes: normalize_file_scopes(input.file_scopes),
            tags: normalize_string_list(input.tags),
            revision: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        transaction.execute(
            r#"
            INSERT INTO work_points (
                id, scope_key, roadmap_id, section_id, title, summary, status, ordering_index,
                dependency_ids_json, validation_expectations_json, effort_tier, tags_json,
                revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
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
                "--clear-dependencies cannot be combined with providing new dependency IDs".to_string(),
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

        let now = now_string()?;

        transaction.execute(
            "UPDATE work_points SET dependency_ids_json = ?1, revision = revision + 1, updated_at = ?2 WHERE id = ?3",
            params![to_json_text(&new_deps)?, now, record.id],
        )?;

        record.dependency_ids = new_deps;
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
            let _ =
                refresh_validation_target(&transaction, EntityType::WorkPoint, &dependent_id)?;
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
        let children = load_children_json(&connection, entity_type, entity_id)?;
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
            EntityType::RoadmapSection | EntityType::Scope => {
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
                update_status_row(
                    &transaction,
                    "goals",
                    &input.entity_id,
                    status.as_str(),
                    &now,
                )?;
                let record = load_goal(&transaction, &input.entity_id)?;
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
                        "goal.status-updated",
                        serde_json::json!({ "status": record.status.as_str(), "revision": record.revision }),
                    )?,
                )?;
                let validation =
                    refresh_validation_target(&transaction, EntityType::Goal, &record.id)?;
                serde_json::json!({ "record": record, "validation": validation })
            }
            EntityType::Roadmap => {
                let status = parse_roadmap_status(input.status.clone())?;
                update_status_row(
                    &transaction,
                    "roadmaps",
                    &input.entity_id,
                    status.as_str(),
                    &now,
                )?;
                let record = load_roadmap(&transaction, &input.entity_id)?;
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
                        "roadmap.status-updated",
                        serde_json::json!({ "status": record.status.as_str(), "revision": record.revision }),
                    )?,
                )?;
                let validation =
                    refresh_validation_target(&transaction, EntityType::Roadmap, &record.id)?;
                serde_json::json!({ "record": record, "validation": validation })
            }
            EntityType::WorkPoint => {
                let status = parse_work_point_status(input.status.clone())?;
                update_status_row(
                    &transaction,
                    "work_points",
                    &input.entity_id,
                    status.as_str(),
                    &now,
                )?;
                let record = load_work_point(&transaction, &input.entity_id)?;
                let correlation_id = roadmap_correlation_id(&transaction, &record.roadmap_id)?;
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
                        "work-point.status-updated",
                        serde_json::json!({ "status": record.status.as_str(), "revision": record.revision }),
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
                update_status_row(
                    &transaction,
                    "plans",
                    &input.entity_id,
                    status.as_str(),
                    &now,
                )?;
                let record = load_plan(&transaction, &input.entity_id)?;
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
                        "plan.status-updated",
                        serde_json::json!({ "status": record.status.as_str(), "revision": record.revision }),
                    )?,
                )?;
                let validation =
                    refresh_validation_target(&transaction, EntityType::Plan, &record.id)?;
                serde_json::json!({ "record": record, "validation": validation })
            }
            EntityType::Todo => {
                let status = parse_todo_status(input.status.clone())?;
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
                        "todo.status-updated",
                        serde_json::json!({
                            "status": record.status.as_str(),
                            "evidenceRefs": record.evidence_refs,
                            "revision": record.revision
                        }),
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
                update_status_row(
                    &transaction,
                    "issues",
                    &input.entity_id,
                    status.as_str(),
                    &now,
                )?;
                let record = load_issue(&transaction, &input.entity_id)?;
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
                        "issue.status-updated",
                        serde_json::json!({ "status": record.status.as_str(), "revision": record.revision }),
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
                        "review-point.status-updated",
                        serde_json::json!({ "status": record.status.as_str(), "revision": record.revision }),
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
                        "insight.status-updated",
                        serde_json::json!({ "status": record.status.as_str(), "revision": record.revision }),
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
            EntityType::RoadmapSection | EntityType::Scope | EntityType::ProjectRun => {
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
            "SELECT id, scope_key, roadmap_id, section_id, title, summary, status, ordering_index, dependency_ids_json, validation_expectations_json, effort_tier, tags_json, revision, created_at, updated_at FROM work_points ORDER BY updated_at DESC, id ASC",
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
            "SELECT id, scope_key, roadmap_id, section_id, title, summary, status, ordering_index, dependency_ids_json, validation_expectations_json, effort_tier, tags_json, revision, created_at, updated_at FROM work_points WHERE scope_key = ?1 ORDER BY updated_at DESC, id ASC",
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

    pub fn claim_project_run(
        &self,
        input: ClaimProjectRunInput,
    ) -> Result<MutationResult<ProjectRunRecord>, PlanningStoreError> {
        require_non_empty("goalId", &input.goal_id)?;
        require_non_empty("roadmapId", &input.roadmap_id)?;
        require_non_empty("workPointId", &input.work_point_id)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
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

        let active_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM project_runs WHERE work_point_id = ?1 AND status IN ('claimed', 'active', 'interrupted')",
            params![input.work_point_id],
            |row| row.get(0),
        )?;
        if active_count > 0 {
            return Err(PlanningStoreError::ActiveLeaseConflict {
                work_point_id: input.work_point_id.clone(),
            });
        }

        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
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
                worktree_id, session_id, run_id, profile_id, status, evidence_json,
                revision, claimed_at, completed_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
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
        Ok(MutationResult { record, validation })
    }

    pub fn activate_project_run(
        &self,
        input: ActivateProjectRunInput,
    ) -> Result<MutationResult<ProjectRunRecord>, PlanningStoreError> {
        require_non_empty("projectRunId", &input.project_run_id)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.active_scope_key);
        ensure_entity_in_scope(
            &transaction,
            EntityType::ProjectRun,
            &input.project_run_id,
            &active_scope_key,
        )?;

        let existing = load_project_run(&transaction, &input.project_run_id)?;
        if existing.status != ProjectRunStatus::Claimed {
            return Err(PlanningStoreError::ProjectRunStatusMismatch {
                expected: "claimed".to_string(),
                actual: existing.status.to_string(),
            });
        }

        let now = now_string()?;

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
        Ok(MutationResult { record, validation })
    }

    pub fn release_project_run(
        &self,
        input: ReleaseProjectRunInput,
    ) -> Result<MutationResult<ProjectRunRecord>, PlanningStoreError> {
        require_non_empty("projectRunId", &input.project_run_id)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.active_scope_key);
        ensure_entity_in_scope(
            &transaction,
            EntityType::ProjectRun,
            &input.project_run_id,
            &active_scope_key,
        )?;

        let existing = load_project_run(&transaction, &input.project_run_id)?;
        if existing.status != ProjectRunStatus::Claimed
            && existing.status != ProjectRunStatus::Active
            && existing.status != ProjectRunStatus::Interrupted
        {
            return Err(PlanningStoreError::ProjectRunStatusMismatch {
                expected: "claimed, active, or interrupted".to_string(),
                actual: existing.status.to_string(),
            });
        }

        let now = now_string()?;
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
        Ok(MutationResult { record, validation })
    }

    pub fn add_project_run_evidence(
        &self,
        input: AddEvidenceInput,
    ) -> Result<MutationResult<ProjectRunRecord>, PlanningStoreError> {
        require_non_empty("projectRunId", &input.project_run_id)?;

        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        let active_scope_key = normalized_scope_key(input.active_scope_key);
        ensure_entity_in_scope(
            &transaction,
            EntityType::ProjectRun,
            &input.project_run_id,
            &active_scope_key,
        )?;

        let existing = load_project_run(&transaction, &input.project_run_id)?;
        if existing.status == ProjectRunStatus::Completed
            || existing.status == ProjectRunStatus::Released
        {
            return Err(PlanningStoreError::ProjectRunStatusMismatch {
                expected: "claimed or active".to_string(),
                actual: existing.status.to_string(),
            });
        }
        let now = now_string()?;

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
        Ok(MutationResult { record, validation })
    }

    pub fn count_active_runs_for_session(
        &self,
        session_id: &str,
    ) -> Result<i64, PlanningStoreError> {
        let conn = self.open_connection()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM project_runs WHERE session_id = ?1 AND status IN ('claimed','active','interrupted')",
            params![session_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Attach/register a worktree.
    pub fn attach_worktree(
        &self,
        input: AttachWorktreeInput,
    ) -> Result<WorktreeRecord, PlanningStoreError> {
        let id = input.id.unwrap_or_else(new_id);
        let scope_key = normalized_scope_key(input.scope_key.clone());
        let now = now_string()?;

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

        self.get_worktree(&id)
    }

    /// Get a worktree by ID.
    pub fn get_worktree(&self, id: &str) -> Result<WorktreeRecord, PlanningStoreError> {
        let conn = self.open_connection()?;
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
                    status: crate::parse_worktree_status(&row.get::<_, String>(7)?),
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

    /// List worktrees in the current scope, with optional status filter.
    pub fn list_worktrees(
        &self,
        status_filter: Option<&str>,
    ) -> Result<Vec<WorktreeRecord>, PlanningStoreError> {
        let conn = self.open_connection()?;
        let scope_key = normalized_scope_key(None);
        let status_val = status_filter.unwrap_or("active");

        let sql = if status_filter.is_some() {
            "SELECT id, scope_key, repo_uri, branch, worktree_path, project_run_id, session_id, status, revision, created_at, updated_at FROM worktrees WHERE scope_key = ?1 AND status = ?2 ORDER BY created_at DESC".to_string()
        } else {
            "SELECT id, scope_key, repo_uri, branch, worktree_path, project_run_id, session_id, status, revision, created_at, updated_at FROM worktrees WHERE scope_key = ?1 ORDER BY created_at DESC".to_string()
        };

        let mut stmt = conn.prepare(&sql)?;
        let rows: Vec<WorktreeRecord> = if status_filter.is_some() {
            stmt.query_map(params![scope_key, status_val], |row| {
                Ok(WorktreeRecord {
                    id: row.get(0)?, scope_key: row.get(1)?, repo_uri: row.get(2)?,
                    branch: row.get(3)?, worktree_path: row.get(4)?, project_run_id: row.get(5)?,
                    session_id: row.get(6)?, status: crate::parse_worktree_status(&row.get::<_, String>(7)?),
                    revision: row.get(8)?, created_at: row.get(9)?, updated_at: row.get(10)?,
                })
            })?.filter_map(|r| r.ok()).collect()
        } else {
            stmt.query_map(params![scope_key], |row| {
                Ok(WorktreeRecord {
                    id: row.get(0)?, scope_key: row.get(1)?, repo_uri: row.get(2)?,
                    branch: row.get(3)?, worktree_path: row.get(4)?, project_run_id: row.get(5)?,
                    session_id: row.get(6)?, status: crate::parse_worktree_status(&row.get::<_, String>(7)?),
                    revision: row.get(8)?, created_at: row.get(9)?, updated_at: row.get(10)?,
                })
            })?.filter_map(|r| r.ok()).collect()
        };

        Ok(rows)
    }

    /// Update worktree status.
    pub fn update_worktree_status(
        &self,
        id: &str,
        status: WorktreeStatus,
    ) -> Result<WorktreeRecord, PlanningStoreError> {
        let conn = self.open_connection()?;
        let now = now_string()?;
        conn.execute(
            "UPDATE worktrees SET status = ?1, updated_at = ?2, revision = revision + 1 WHERE id = ?3",
            params![status.to_string(), now, id],
        )?;
        self.get_worktree(id)
    }

    /// List recent sessions from the events table.
    pub fn list_sessions(&self, limit: i64) -> Result<Vec<SessionSummary>, PlanningStoreError> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                COALESCE(e.session_id, 'unknown') as sid,
                COUNT(*) as event_count,
                MAX(e.created_at) as last_seen,
                (SELECT COUNT(*) FROM project_runs pr WHERE pr.session_id = e.session_id AND pr.status IN ('claimed','active','interrupted')) as active_runs
             FROM planning_events e
             WHERE e.session_id IS NOT NULL AND e.session_id != ''
             GROUP BY e.session_id
             ORDER BY last_seen DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit], |row| {
            Ok(SessionSummary {
                session_id: row.get(0)?,
                scope: String::new(),
                created_at: None,
                last_seen: row.get::<_, Option<String>>(2)?,
                event_count: row.get(1)?,
                active_project_runs: row.get(3)?,
            })
        })?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_project_runs(&self) -> Result<Vec<ProjectRunRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope_key, goal_id, roadmap_id, work_point_id, repo_id, branch, worktree_id, session_id, run_id, profile_id, status, evidence_json, revision, claimed_at, completed_at, created_at, updated_at FROM project_runs ORDER BY updated_at DESC, id ASC",
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
            "SELECT id, scope_key, goal_id, roadmap_id, work_point_id, repo_id, branch, worktree_id, session_id, run_id, profile_id, status, evidence_json, revision, claimed_at, completed_at, created_at, updated_at FROM project_runs WHERE scope_key = ?1 ORDER BY updated_at DESC, id ASC",
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
        let roadmap = load_roadmap(&connection, roadmap_id)?;
        let all_work_points = list_work_points_for_roadmap(&connection, roadmap_id)?;
        let mut candidates = Vec::new();

        for wp in &all_work_points {
            if matches!(
                wp.status,
                WorkPointStatus::Completed
                    | WorkPointStatus::Cancelled
                    | WorkPointStatus::Invalidated
                    | WorkPointStatus::Blocked
            ) {
                continue;
            }

            let active_lease_count: i64 = connection.query_row(
                "SELECT COUNT(*) FROM project_runs WHERE work_point_id = ?1 AND status IN ('claimed', 'active', 'interrupted')",
                params![wp.id],
                |row| row.get(0),
            )?;
            if active_lease_count > 0 {
                continue;
            }

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

            candidates.push(RunnableWorkPointCandidate {
                work_point: wp.clone(),
                roadmap_id: roadmap_id.to_string(),
                roadmap_title: roadmap.title.clone(),
                dependency_titles,
                reasons,
            });
        }

        candidates.sort_by(|a, b| {
            a.work_point
                .ordering
                .cmp(&b.work_point.ordering)
                .then_with(|| a.work_point.id.cmp(&b.work_point.id))
        });

        Ok(RunnableCandidates {
            roadmap_id: roadmap_id.to_string(),
            candidates,
        })
    }

    pub fn build_work_graph(&self, roadmap_id: &str) -> Result<WorkGraph, PlanningStoreError> {
        require_non_empty("roadmapId", roadmap_id)?;

        let connection = self.open_connection()?;
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
                    "SELECT COUNT(*) FROM project_runs WHERE work_point_id = ?1 AND status IN ('claimed', 'active', 'interrupted')",
                    params![wp.id],
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
            migrate_v5_to_v6(connection)
        }
        Some("2") => {
            migrate_v2_to_v3(connection)?;
            migrate_v3_to_v4(connection)?;
            migrate_v4_to_v5(connection)?;
            migrate_v5_to_v6(connection)
        }
        Some("3") => {
            migrate_v3_to_v4(connection)?;
            migrate_v4_to_v5(connection)?;
            migrate_v5_to_v6(connection)
        }
        Some("4") => {
            migrate_v4_to_v5(connection)?;
            migrate_v5_to_v6(connection)
        }
        Some("5") => migrate_v5_to_v6(connection),
        Some("6") => migrate_v6_to_v7(connection),
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
        "UPDATE planning_config SET value = ?2 WHERE key = ?1",
        params![SCHEMA_VERSION_KEY, CURRENT_SCHEMA_VERSION],
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
        "UPDATE planning_config SET value = ?2 WHERE key = ?1",
        params![SCHEMA_VERSION_KEY, CURRENT_SCHEMA_VERSION],
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
        "UPDATE planning_config SET value = ?2 WHERE key = ?1",
        params![SCHEMA_VERSION_KEY, CURRENT_SCHEMA_VERSION],
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
        "UPDATE planning_config SET value = ?2 WHERE key = ?1",
        params![SCHEMA_VERSION_KEY, CURRENT_SCHEMA_VERSION],
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
        "UPDATE planning_config SET value = ?2 WHERE key = ?1",
        params![SCHEMA_VERSION_KEY, CURRENT_SCHEMA_VERSION],
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
            "SELECT id, scope_key, roadmap_id, section_id, title, summary, status, ordering_index, dependency_ids_json, validation_expectations_json, effort_tier, tags_json, revision, created_at, updated_at FROM work_points WHERE id = ?1",
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
        "SELECT id, scope_key, roadmap_id, section_id, title, summary, status, ordering_index, dependency_ids_json, validation_expectations_json, effort_tier, tags_json, revision, created_at, updated_at FROM work_points WHERE roadmap_id = ?1 ORDER BY ordering_index ASC, id ASC",
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
        "SELECT id, scope_key, roadmap_id, section_id, title, summary, status, ordering_index, dependency_ids_json, validation_expectations_json, effort_tier, tags_json, revision, created_at, updated_at FROM work_points WHERE roadmap_id = ?1 AND scope_key = ?2 ORDER BY ordering_index ASC, id ASC",
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
    Ok(entities)
}

pub(crate) fn collect_entities_in_scope(
    connection: &Connection,
    scope_key: &str,
) -> Result<Vec<(EntityType, String)>, PlanningStoreError> {
    let mut entities = Vec::new();
    entities.extend(entity_ids_in_scope(connection, "goals", "scope_key", EntityType::Goal, scope_key)?);
    entities.extend(entity_ids_in_scope(connection, "roadmaps", "scope_key", EntityType::Roadmap, scope_key)?);
    entities.extend(entity_ids_in_scope(connection, "roadmap_sections", "scope_key", EntityType::RoadmapSection, scope_key)?);
    entities.extend(entity_ids_in_scope(connection, "work_points", "scope_key", EntityType::WorkPoint, scope_key)?);
    entities.extend(entity_ids_in_scope(connection, "plans", "scope_key", EntityType::Plan, scope_key)?);
    entities.extend(entity_ids_in_scope(connection, "todos", "scope_key", EntityType::Todo, scope_key)?);
    entities.extend(entity_ids_in_scope(connection, "issues", "scope_key", EntityType::Issue, scope_key)?);
    entities.extend(entity_ids_in_scope(connection, "review_points", "scope_key", EntityType::ReviewPoint, scope_key)?);
    entities.extend(entity_ids_in_scope(connection, "insights", "scope_key", EntityType::Insight, scope_key)?);
    entities.extend(entity_ids_in_scope(connection, "project_runs", "scope_key", EntityType::ProjectRun, scope_key)?);
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
        file_scopes: Vec::new(),
        tags: parse_json_column(row.get::<_, String>(11)?)?,
        revision: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
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

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

fn require_non_empty(field: &str, value: &str) -> Result<(), PlanningStoreError> {
    if value.trim().is_empty() {
        return Err(PlanningStoreError::InvalidInput(format!(
            "{field} must not be empty"
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
        status: parse_project_run_status(row.get::<_, String>(11)?)?,
        evidence: parse_json_column(row.get::<_, String>(12)?)?,
        revision: row.get(13)?,
        claimed_at: row.get(14)?,
        completed_at: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
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
            "SELECT id, scope_key, goal_id, roadmap_id, work_point_id, repo_id, branch, worktree_id, session_id, run_id, profile_id, status, evidence_json, revision, claimed_at, completed_at, created_at, updated_at FROM project_runs WHERE id = ?1",
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
        _ => Vec::new(),
    };
    Ok(children.into_iter().filter(|v| !v.is_null()).collect())
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
    use rusqlite::params;
    use tempfile::tempdir;

    use super::*;
    use crate::ValidationStatus;

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
                    selector: "rust/crates/elegy-planning/src/**".to_string(),
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
}
