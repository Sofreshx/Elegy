use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};
use serde::Serialize;
use serde_json::Value;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

use crate::{
    validation::validate_entity, EntityType, GoalRecord, GoalStatus, GoalView, IssueRecord,
    IssueStatus, IssueView, MutationResult, PlanRecord, PlanStatus, PlanView, PlanningEvent,
    PlanningHealthReport, PlanningStoreError, Priority, ProjectionFormat, RenderedProjection,
    ReviewPointRecord, ReviewPointStatus, RoadmapRecord, RoadmapSectionRecord, RoadmapStatus,
    RoadmapView, Severity, TodoRecord, TodoStatus, ValidationFinding, ValidationReport,
    ValidationRunReport, ValidationSeverity, WorkPointRecord, WorkPointStatus,
};

pub const CURRENT_SCHEMA_VERSION: &str = "1";
const SCHEMA_VERSION_KEY: &str = "schema_version";

#[derive(Clone, Debug)]
pub struct PlanningStore {
    db_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct CreateGoalInput {
    pub id: Option<String>,
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
    pub roadmap_id: String,
    pub section_id: Option<String>,
    pub title: String,
    pub summary: String,
    pub status: WorkPointStatus,
    pub ordering: Option<i64>,
    pub dependency_ids: Vec<String>,
    pub validation_expectations: Vec<String>,
    pub tags: Vec<String>,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreatePlanInput {
    pub id: Option<String>,
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
    pub status: PlanStatus,
    pub tags: Vec<String>,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreateTodoInput {
    pub id: Option<String>,
    pub plan_id: Option<String>,
    pub work_point_id: Option<String>,
    pub title: String,
    pub summary: String,
    pub status: TodoStatus,
    pub priority: Priority,
    pub evidence_refs: Vec<String>,
    pub tags: Vec<String>,
    pub ordering: Option<i64>,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreateIssueInput {
    pub id: Option<String>,
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
    pub attached_entity_type: EntityType,
    pub attached_entity_id: String,
    pub title: String,
    pub summary: String,
    pub status: ReviewPointStatus,
    pub severity: Severity,
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
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let record = GoalRecord {
            id: id.clone(),
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
                id, correlation_id, title, description, acceptance_criteria_json,
                rejection_criteria_json, status, tags_json, revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                record.id,
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
        ensure_entity_exists(&transaction, EntityType::Goal, &input.goal_id, "goalId")?;
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let record = RoadmapRecord {
            id: id.clone(),
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
                id, goal_id, correlation_id, title, summary, status, tags_json, revision,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                record.id,
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
        ensure_entity_exists(
            &transaction,
            EntityType::Roadmap,
            &input.roadmap_id,
            "roadmapId",
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
                id, roadmap_id, slug, title, summary, ordering_index, revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                record.id,
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
        ensure_entity_exists(
            &transaction,
            EntityType::Roadmap,
            &input.roadmap_id,
            "roadmapId",
        )?;
        if let Some(section_id) = &input.section_id {
            ensure_entity_exists(
                &transaction,
                EntityType::RoadmapSection,
                section_id,
                "sectionId",
            )?;
            ensure_section_belongs_to_roadmap(&transaction, section_id, &input.roadmap_id)?;
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
            roadmap_id: input.roadmap_id,
            section_id: input.section_id,
            title: input.title.trim().to_string(),
            summary: input.summary.trim().to_string(),
            status: input.status,
            ordering,
            dependency_ids: normalize_string_list(input.dependency_ids),
            validation_expectations: normalize_string_list(input.validation_expectations),
            tags: normalize_string_list(input.tags),
            revision: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        transaction.execute(
            r#"
            INSERT INTO work_points (
                id, roadmap_id, section_id, title, summary, status, ordering_index,
                dependency_ids_json, validation_expectations_json, tags_json, revision,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
                record.id,
                record.roadmap_id,
                record.section_id,
                record.title,
                record.summary,
                record.status.as_str(),
                record.ordering,
                to_json_text(&record.dependency_ids)?,
                to_json_text(&record.validation_expectations)?,
                to_json_text(&record.tags)?,
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
        ensure_entity_exists(&transaction, EntityType::Goal, &input.goal_id, "goalId")?;
        ensure_entity_exists(
            &transaction,
            EntityType::Roadmap,
            &input.roadmap_id,
            "roadmapId",
        )?;
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let record = PlanRecord {
            id: id.clone(),
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
            status: input.status,
            tags: normalize_string_list(input.tags),
            revision: 1,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        transaction.execute(
            r#"
            INSERT INTO plans (
                id, goal_id, roadmap_id, correlation_id, title, summary, scope,
                assumptions_json, stop_conditions_json, validation_steps_json,
                targeted_work_point_ids_json, status, tags_json, revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            "#,
            params![
                record.id,
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
        if let Some(plan_id) = &input.plan_id {
            ensure_entity_exists(&transaction, EntityType::Plan, plan_id, "planId")?;
        }
        if let Some(work_point_id) = &input.work_point_id {
            ensure_entity_exists(
                &transaction,
                EntityType::WorkPoint,
                work_point_id,
                "workPointId",
            )?;
        }
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
            plan_id: input.plan_id,
            work_point_id: input.work_point_id,
            title: input.title.trim().to_string(),
            summary: input.summary.trim().to_string(),
            status: input.status,
            priority: input.priority,
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
                id, plan_id, work_point_id, title, summary, status, priority,
                evidence_refs_json, tags_json, ordering_index, revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
                record.id,
                record.plan_id,
                record.work_point_id,
                record.title,
                record.summary,
                record.status.as_str(),
                record.priority.as_str(),
                to_json_text(&record.evidence_refs)?,
                to_json_text(&record.tags)?,
                record.ordering,
                record.revision,
                record.created_at,
                record.updated_at,
            ],
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
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let record = IssueRecord {
            id: id.clone(),
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
                id, correlation_id, title, summary, status, severity,
                related_entity_type, related_entity_id, tags_json, revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
            params![
                record.id,
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
        ensure_entity_exists(
            &transaction,
            input.attached_entity_type,
            &input.attached_entity_id,
            "attachedEntityId",
        )?;
        let now = now_string()?;
        let id = input.id.unwrap_or_else(new_id);
        let record = ReviewPointRecord {
            id: id.clone(),
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
                id, attached_entity_type, attached_entity_id, title, summary, status, severity,
                revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                record.id,
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

    pub fn goal(&self, id: &str) -> Result<GoalView, PlanningStoreError> {
        let connection = self.open_connection()?;
        let goal = load_goal(&connection, id)?;
        let roadmaps = list_roadmaps_for_goal(&connection, id)?;
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
        let sections = list_sections_for_roadmap(&connection, id)?;
        let work_points = list_work_points_for_roadmap(&connection, id)?;
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
        let todos = list_todos_for_plan(&connection, id)?;
        let review_points = list_review_points_for_entity(&connection, EntityType::Plan, id)?;
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

    pub fn list_goals(&self) -> Result<Vec<GoalRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, correlation_id, title, description, acceptance_criteria_json, rejection_criteria_json, status, tags_json, revision, created_at, updated_at FROM goals ORDER BY updated_at DESC, id ASC",
        )?;
        let rows = statement.query_map([], row_to_goal)?;
        collect_rows(rows)
    }

    pub fn list_roadmaps(&self) -> Result<Vec<RoadmapRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, goal_id, correlation_id, title, summary, status, tags_json, revision, created_at, updated_at FROM roadmaps ORDER BY updated_at DESC, id ASC",
        )?;
        let rows = statement.query_map([], row_to_roadmap)?;
        collect_rows(rows)
    }

    pub fn list_plans(&self) -> Result<Vec<PlanRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, goal_id, roadmap_id, correlation_id, title, summary, scope, assumptions_json, stop_conditions_json, validation_steps_json, targeted_work_point_ids_json, status, tags_json, revision, created_at, updated_at FROM plans ORDER BY updated_at DESC, id ASC",
        )?;
        let rows = statement.query_map([], row_to_plan)?;
        collect_rows(rows)
    }

    pub fn list_todos(&self) -> Result<Vec<TodoRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, plan_id, work_point_id, title, summary, status, priority, evidence_refs_json, tags_json, ordering_index, revision, created_at, updated_at FROM todos ORDER BY status ASC, ordering_index ASC, id ASC",
        )?;
        let rows = statement.query_map([], row_to_todo)?;
        collect_rows(rows)
    }

    pub fn list_issues(&self) -> Result<Vec<IssueRecord>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT id, correlation_id, title, summary, status, severity, related_entity_type, related_entity_id, tags_json, revision, created_at, updated_at FROM issues ORDER BY updated_at DESC, id ASC",
        )?;
        let rows = statement.query_map([], row_to_issue)?;
        collect_rows(rows)
    }

    pub fn list_events(&self) -> Result<Vec<PlanningEvent>, PlanningStoreError> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare(
            "SELECT event_id, entity_type, entity_id, aggregate_type, aggregate_id, correlation_id, causation_id, run_id, stream_id, sequence, parent_event_id, event_type, timestamp, payload_json FROM planning_events ORDER BY rowid ASC",
        )?;
        let rows = statement.query_map([], row_to_event)?;
        collect_rows(rows)
    }

    pub fn health(&self) -> Result<PlanningHealthReport, PlanningStoreError> {
        let connection = self.open_connection()?;
        Ok(PlanningHealthReport {
            db_path: self.db_path.display().to_string(),
            event_count: count_table(&connection, "planning_events")?,
            active_validation_finding_count: count_table(&connection, "validation_findings")?,
            goal_count: count_table(&connection, "goals")?,
            roadmap_count: count_table(&connection, "roadmaps")?,
            roadmap_section_count: count_table(&connection, "roadmap_sections")?,
            work_point_count: count_table(&connection, "work_points")?,
            plan_count: count_table(&connection, "plans")?,
            todo_count: count_table(&connection, "todos")?,
            issue_count: count_table(&connection, "issues")?,
            review_point_count: count_table(&connection, "review_points")?,
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

        CREATE TABLE IF NOT EXISTS goals (
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
        CREATE INDEX IF NOT EXISTS idx_goals_correlation ON goals(correlation_id);
        CREATE INDEX IF NOT EXISTS idx_goals_status ON goals(status);

        CREATE TABLE IF NOT EXISTS roadmaps (
            id TEXT PRIMARY KEY,
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
            roadmap_id TEXT NOT NULL REFERENCES roadmaps(id) ON DELETE CASCADE,
            section_id TEXT REFERENCES roadmap_sections(id) ON DELETE SET NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            status TEXT NOT NULL,
            ordering_index INTEGER NOT NULL,
            dependency_ids_json TEXT NOT NULL,
            validation_expectations_json TEXT NOT NULL,
            tags_json TEXT NOT NULL,
            revision INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_work_points_roadmap ON work_points(roadmap_id, ordering_index);

        CREATE TABLE IF NOT EXISTS plans (
            id TEXT PRIMARY KEY,
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
            plan_id TEXT REFERENCES plans(id) ON DELETE CASCADE,
            work_point_id TEXT REFERENCES work_points(id) ON DELETE SET NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            status TEXT NOT NULL,
            priority TEXT NOT NULL,
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
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_validation_findings_entity ON validation_findings(entity_type, entity_id);
        "#,
    )?;
    Ok(())
}

fn ensure_schema_version(connection: &Transaction<'_>) -> Result<(), PlanningStoreError> {
    connection.execute(
        "INSERT OR IGNORE INTO planning_config (key, value) VALUES (?1, ?2)",
        params![SCHEMA_VERSION_KEY, CURRENT_SCHEMA_VERSION],
    )?;
    let version: Option<String> = connection
        .query_row(
            "SELECT value FROM planning_config WHERE key = ?1",
            params![SCHEMA_VERSION_KEY],
            |row| row.get(0),
        )
        .optional()?;
    match version.as_deref() {
        Some(CURRENT_SCHEMA_VERSION) => Ok(()),
        Some(other) => Err(PlanningStoreError::InvalidInput(format!(
            "unsupported planning schema version {other}; expected {CURRENT_SCHEMA_VERSION}"
        ))),
        None => Err(PlanningStoreError::InvalidInput(
            "planning schema version is missing".to_string(),
        )),
    }
}

fn append_event(
    connection: &Transaction<'_>,
    event: PlanningEvent,
) -> Result<(), PlanningStoreError> {
    connection.execute(
        r#"
        INSERT INTO planning_events (
            event_id, entity_type, entity_id, aggregate_type, aggregate_id, correlation_id,
            causation_id, run_id, stream_id, sequence, parent_event_id, event_type, timestamp,
            payload_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        "#,
        params![
            event.event_id,
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

fn list_work_point_dependents(
    connection: &Connection,
    dependency_id: &str,
) -> Result<Vec<String>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, roadmap_id, section_id, title, summary, status, ordering_index, dependency_ids_json, validation_expectations_json, tags_json, revision, created_at, updated_at FROM work_points ORDER BY id ASC",
    )?;
    let rows = statement.query_map([], row_to_work_point)?;
    let work_points = collect_rows(rows)?;
    Ok(work_points
        .into_iter()
        .filter(|work_point| {
            work_point
                .dependency_ids
                .iter()
                .any(|entry| entry == dependency_id)
        })
        .map(|work_point| work_point.id)
        .collect())
}

fn list_plans_targeting_work_point(
    connection: &Connection,
    work_point_id: &str,
) -> Result<Vec<String>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, goal_id, roadmap_id, correlation_id, title, summary, scope, assumptions_json, stop_conditions_json, validation_steps_json, targeted_work_point_ids_json, status, tags_json, revision, created_at, updated_at FROM plans ORDER BY id ASC",
    )?;
    let rows = statement.query_map([], row_to_plan)?;
    let plans = collect_rows(rows)?;
    Ok(plans
        .into_iter()
        .filter(|plan| {
            plan.targeted_work_point_ids
                .iter()
                .any(|entry| entry == work_point_id)
        })
        .map(|plan| plan.id)
        .collect())
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
                finding_id, entity_type, entity_id, severity, code, message, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                finding.finding_id,
                finding.entity_type.as_str(),
                finding.entity_id,
                finding.severity.as_str(),
                finding.code,
                finding.message,
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
        "SELECT finding_id, entity_type, entity_id, severity, code, message, created_at FROM validation_findings WHERE entity_type = ?1 AND entity_id = ?2 ORDER BY severity ASC, code ASC, created_at ASC",
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
            "SELECT id, correlation_id, title, description, acceptance_criteria_json, rejection_criteria_json, status, tags_json, revision, created_at, updated_at FROM goals WHERE id = ?1",
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
            "SELECT id, goal_id, correlation_id, title, summary, status, tags_json, revision, created_at, updated_at FROM roadmaps WHERE id = ?1",
            params![id],
            row_to_roadmap,
        )
        .map_err(|error| map_not_found(error, EntityType::Roadmap, id))
}

pub(crate) fn load_plan(
    connection: &Connection,
    id: &str,
) -> Result<PlanRecord, PlanningStoreError> {
    connection
        .query_row(
            "SELECT id, goal_id, roadmap_id, correlation_id, title, summary, scope, assumptions_json, stop_conditions_json, validation_steps_json, targeted_work_point_ids_json, status, tags_json, revision, created_at, updated_at FROM plans WHERE id = ?1",
            params![id],
            row_to_plan,
        )
        .map_err(|error| map_not_found(error, EntityType::Plan, id))
}

pub(crate) fn load_issue(
    connection: &Connection,
    id: &str,
) -> Result<IssueRecord, PlanningStoreError> {
    connection
        .query_row(
            "SELECT id, correlation_id, title, summary, status, severity, related_entity_type, related_entity_id, tags_json, revision, created_at, updated_at FROM issues WHERE id = ?1",
            params![id],
            row_to_issue,
        )
        .map_err(|error| map_not_found(error, EntityType::Issue, id))
}

pub(crate) fn load_work_point(
    connection: &Connection,
    id: &str,
) -> Result<WorkPointRecord, PlanningStoreError> {
    connection
        .query_row(
            "SELECT id, roadmap_id, section_id, title, summary, status, ordering_index, dependency_ids_json, validation_expectations_json, tags_json, revision, created_at, updated_at FROM work_points WHERE id = ?1",
            params![id],
            row_to_work_point,
        )
        .map_err(|error| map_not_found(error, EntityType::WorkPoint, id))
}

pub(crate) fn load_todo(
    connection: &Connection,
    id: &str,
) -> Result<TodoRecord, PlanningStoreError> {
    connection
        .query_row(
            "SELECT id, plan_id, work_point_id, title, summary, status, priority, evidence_refs_json, tags_json, ordering_index, revision, created_at, updated_at FROM todos WHERE id = ?1",
            params![id],
            row_to_todo,
        )
        .map_err(|error| map_not_found(error, EntityType::Todo, id))
}

pub(crate) fn list_roadmaps_for_goal(
    connection: &Connection,
    goal_id: &str,
) -> Result<Vec<RoadmapRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, goal_id, correlation_id, title, summary, status, tags_json, revision, created_at, updated_at FROM roadmaps WHERE goal_id = ?1 ORDER BY updated_at DESC, id ASC",
    )?;
    let rows = statement.query_map(params![goal_id], row_to_roadmap)?;
    let items = collect_rows(rows)?;
    Ok(items)
}

pub(crate) fn list_sections_for_roadmap(
    connection: &Connection,
    roadmap_id: &str,
) -> Result<Vec<RoadmapSectionRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, roadmap_id, slug, title, summary, ordering_index, revision, created_at, updated_at FROM roadmap_sections WHERE roadmap_id = ?1 ORDER BY ordering_index ASC, id ASC",
    )?;
    let rows = statement.query_map(params![roadmap_id], row_to_section)?;
    let items = collect_rows(rows)?;
    Ok(items)
}

pub(crate) fn list_work_points_for_roadmap(
    connection: &Connection,
    roadmap_id: &str,
) -> Result<Vec<WorkPointRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, roadmap_id, section_id, title, summary, status, ordering_index, dependency_ids_json, validation_expectations_json, tags_json, revision, created_at, updated_at FROM work_points WHERE roadmap_id = ?1 ORDER BY ordering_index ASC, id ASC",
    )?;
    let rows = statement.query_map(params![roadmap_id], row_to_work_point)?;
    let items = collect_rows(rows)?;
    Ok(items)
}

pub(crate) fn list_todos_for_plan(
    connection: &Connection,
    plan_id: &str,
) -> Result<Vec<TodoRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, plan_id, work_point_id, title, summary, status, priority, evidence_refs_json, tags_json, ordering_index, revision, created_at, updated_at FROM todos WHERE plan_id = ?1 ORDER BY ordering_index ASC, id ASC",
    )?;
    let rows = statement.query_map(params![plan_id], row_to_todo)?;
    let items = collect_rows(rows)?;
    Ok(items)
}

pub(crate) fn list_review_points_for_entity(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
) -> Result<Vec<ReviewPointRecord>, PlanningStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, attached_entity_type, attached_entity_id, title, summary, status, severity, revision, created_at, updated_at FROM review_points WHERE attached_entity_type = ?1 AND attached_entity_id = ?2 ORDER BY created_at ASC, id ASC",
    )?;
    let rows = statement.query_map(
        params![entity_type.as_str(), entity_id],
        row_to_review_point,
    )?;
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
    Ok(entities)
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

fn entity_revision(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
) -> Result<i64, PlanningStoreError> {
    let (table, id_column) = match entity_type {
        EntityType::Goal => ("goals", "id"),
        EntityType::Roadmap => ("roadmaps", "id"),
        EntityType::RoadmapSection => ("roadmap_sections", "id"),
        EntityType::WorkPoint => ("work_points", "id"),
        EntityType::Plan => ("plans", "id"),
        EntityType::Todo => ("todos", "id"),
        EntityType::Issue => ("issues", "id"),
        EntityType::ReviewPoint => ("review_points", "id"),
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
        correlation_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        acceptance_criteria: parse_json_column(row.get::<_, String>(4)?)?,
        rejection_criteria: parse_json_column(row.get::<_, String>(5)?)?,
        status: parse_goal_status(row.get::<_, String>(6)?)?,
        tags: parse_json_column(row.get::<_, String>(7)?)?,
        revision: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn row_to_roadmap(row: &Row<'_>) -> Result<RoadmapRecord, rusqlite::Error> {
    Ok(RoadmapRecord {
        id: row.get(0)?,
        goal_id: row.get(1)?,
        correlation_id: row.get(2)?,
        title: row.get(3)?,
        summary: row.get(4)?,
        status: parse_roadmap_status(row.get::<_, String>(5)?)?,
        tags: parse_json_column(row.get::<_, String>(6)?)?,
        revision: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn row_to_section(row: &Row<'_>) -> Result<RoadmapSectionRecord, rusqlite::Error> {
    Ok(RoadmapSectionRecord {
        id: row.get(0)?,
        roadmap_id: row.get(1)?,
        slug: row.get(2)?,
        title: row.get(3)?,
        summary: row.get(4)?,
        ordering: row.get(5)?,
        revision: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn row_to_work_point(row: &Row<'_>) -> Result<WorkPointRecord, rusqlite::Error> {
    Ok(WorkPointRecord {
        id: row.get(0)?,
        roadmap_id: row.get(1)?,
        section_id: row.get(2)?,
        title: row.get(3)?,
        summary: row.get(4)?,
        status: parse_work_point_status(row.get::<_, String>(5)?)?,
        ordering: row.get(6)?,
        dependency_ids: parse_json_column(row.get::<_, String>(7)?)?,
        validation_expectations: parse_json_column(row.get::<_, String>(8)?)?,
        tags: parse_json_column(row.get::<_, String>(9)?)?,
        revision: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn row_to_plan(row: &Row<'_>) -> Result<PlanRecord, rusqlite::Error> {
    Ok(PlanRecord {
        id: row.get(0)?,
        goal_id: row.get(1)?,
        roadmap_id: row.get(2)?,
        correlation_id: row.get(3)?,
        title: row.get(4)?,
        summary: row.get(5)?,
        scope: row.get(6)?,
        assumptions: parse_json_column(row.get::<_, String>(7)?)?,
        stop_conditions: parse_json_column(row.get::<_, String>(8)?)?,
        validation_steps: parse_json_column(row.get::<_, String>(9)?)?,
        targeted_work_point_ids: parse_json_column(row.get::<_, String>(10)?)?,
        status: parse_plan_status(row.get::<_, String>(11)?)?,
        tags: parse_json_column(row.get::<_, String>(12)?)?,
        revision: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

fn row_to_todo(row: &Row<'_>) -> Result<TodoRecord, rusqlite::Error> {
    Ok(TodoRecord {
        id: row.get(0)?,
        plan_id: row.get(1)?,
        work_point_id: row.get(2)?,
        title: row.get(3)?,
        summary: row.get(4)?,
        status: parse_todo_status(row.get::<_, String>(5)?)?,
        priority: parse_priority(row.get::<_, String>(6)?)?,
        evidence_refs: parse_json_column(row.get::<_, String>(7)?)?,
        tags: parse_json_column(row.get::<_, String>(8)?)?,
        ordering: row.get(9)?,
        revision: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn row_to_issue(row: &Row<'_>) -> Result<IssueRecord, rusqlite::Error> {
    Ok(IssueRecord {
        id: row.get(0)?,
        correlation_id: row.get(1)?,
        title: row.get(2)?,
        summary: row.get(3)?,
        status: parse_issue_status(row.get::<_, String>(4)?)?,
        severity: parse_severity(row.get::<_, String>(5)?)?,
        related_entity_type: row
            .get::<_, Option<String>>(6)?
            .map(parse_entity_type)
            .transpose()?,
        related_entity_id: row.get(7)?,
        tags: parse_json_column(row.get::<_, String>(8)?)?,
        revision: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn row_to_review_point(row: &Row<'_>) -> Result<ReviewPointRecord, rusqlite::Error> {
    Ok(ReviewPointRecord {
        id: row.get(0)?,
        attached_entity_type: parse_entity_type(row.get::<_, String>(1)?)?,
        attached_entity_id: row.get(2)?,
        title: row.get(3)?,
        summary: row.get(4)?,
        status: parse_review_point_status(row.get::<_, String>(5)?)?,
        severity: parse_severity(row.get::<_, String>(6)?)?,
        revision: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
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
        created_at: row.get(6)?,
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

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::ValidationStatus;

    #[test]
    fn store_persists_goal_roadmap_plan_and_validation_findings() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init planning db");

        let goal = store
            .create_goal(CreateGoalInput {
                id: Some("goal-1".to_string()),
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
                roadmap_id: "roadmap-1".to_string(),
                section_id: None,
                title: "Implement store".to_string(),
                summary: "Persist events and projections.".to_string(),
                status: WorkPointStatus::Active,
                ordering: None,
                dependency_ids: Vec::new(),
                validation_expectations: vec!["health command passes".to_string()],
                tags: Vec::new(),
                run_id: Some("run-wp".to_string()),
            })
            .expect("add work point");

        let roadmap_view = store.roadmap("roadmap-1").expect("roadmap view");
        assert_eq!(roadmap_view.validation.status, ValidationStatus::Valid);

        let plan = store
            .create_plan(CreatePlanInput {
                id: Some("plan-1".to_string()),
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
                plan_id: Some("plan-1".to_string()),
                work_point_id: Some("wp-1".to_string()),
                title: "Finish MVP implementation".to_string(),
                summary: "Wire the first shipping slice.".to_string(),
                status: TodoStatus::InProgress,
                priority: Priority::High,
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
                plan_id: None,
                work_point_id: None,
                title: "Loose todo".to_string(),
                summary: "Allowed for manual tracking.".to_string(),
                status: TodoStatus::Pending,
                priority: Priority::Medium,
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
                roadmap_id: "roadmap-issue-1".to_string(),
                section_id: None,
                title: "WP".to_string(),
                summary: "Summary".to_string(),
                status: WorkPointStatus::Draft,
                ordering: None,
                dependency_ids: Vec::new(),
                validation_expectations: vec!["proof".to_string()],
                tags: Vec::new(),
                run_id: Some("run-wp-issue".to_string()),
            })
            .expect("create work point");
        store
            .create_plan(CreatePlanInput {
                id: Some("plan-issue-1".to_string()),
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
                status: PlanStatus::Active,
                tags: Vec::new(),
                run_id: Some("run-plan-issue".to_string()),
            })
            .expect("create plan");
        store
            .create_todo(CreateTodoInput {
                id: Some("todo-issue-1".to_string()),
                plan_id: Some("plan-issue-1".to_string()),
                work_point_id: Some("wp-issue-1".to_string()),
                title: "Todo".to_string(),
                summary: "Summary".to_string(),
                status: TodoStatus::Pending,
                priority: Priority::Medium,
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
}
