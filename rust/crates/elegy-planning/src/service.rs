use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::{
    AddEvidenceInput, AddRoadmapSectionInput, AddWorkPointInput, ClaimProjectRunInput,
    CreateGoalInput, CreateInsightInput, CreateIssueInput, CreatePlanInput,
    CreateReviewPointInput, CreateRoadmapInput, CreateTodoInput, EntityType, GoalView, InsightView,
    IssueView, MutationResult, PlanView, PlanningHealthReport, PlanningStore,
    PlanningStoreError, ProjectRunRecord, ProjectRunView, ProjectionFormat,
    ReleaseProjectRunInput, RenderedProjection, RevisePlanInput, RoadmapView, RunnableCandidates,
    ScopeRecord, TagInfo, UpdateStatusInput, ValidationRunReport, WorkGraph, WorkPointRecord,
    WorkPointView,
};

#[derive(Clone, Debug)]
pub struct PlanningServiceConfig {
    pub db_path: PathBuf,
    pub scope_key: String,
    pub scope_type: Option<String>,
}

impl PlanningServiceConfig {
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        Self {
            db_path: db_path.into(),
            scope_key: "default".to_string(),
            scope_type: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PlanningContext {
    pub correlation_id: Option<String>,
    pub run_id: Option<String>,
    pub actor: Option<String>,
    pub host: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PlanningService {
    store: PlanningStore,
    config: PlanningServiceConfig,
}

impl PlanningService {
    pub fn new(config: PlanningServiceConfig) -> Result<Self, PlanningStoreError> {
        let store = PlanningStore::new(config.db_path.clone());
        store.init()?;
        Ok(Self { store, config })
    }

    pub fn db_path(&self) -> &Path {
        self.store.db_path()
    }

    pub fn create_scope(
        &self,
        input: crate::CreateScopeInput,
    ) -> Result<MutationResult<ScopeRecord>, PlanningStoreError> {
        self.store.create_scope(input)
    }

    pub fn list_scopes(&self) -> Result<Vec<ScopeRecord>, PlanningStoreError> {
        self.store.list_scopes()
    }

    pub fn scope(&self, scope_key: &str) -> Result<ScopeRecord, PlanningStoreError> {
        self.store.scope(scope_key)
    }

    pub fn create_goal(
        &self,
        context: &PlanningContext,
        mut input: CreateGoalInput,
    ) -> Result<MutationResult<crate::GoalRecord>, PlanningStoreError> {
        input.scope_key = Some(self.config.scope_key.clone());
        input.run_id = resolve_run_id(context);
        self.store.create_goal(input)
    }

    pub fn create_roadmap(
        &self,
        context: &PlanningContext,
        mut input: CreateRoadmapInput,
    ) -> Result<MutationResult<crate::RoadmapRecord>, PlanningStoreError> {
        input.scope_key = Some(self.config.scope_key.clone());
        input.run_id = resolve_run_id(context);
        self.store.create_roadmap(input)
    }

    pub fn add_roadmap_section(
        &self,
        context: &PlanningContext,
        mut input: AddRoadmapSectionInput,
    ) -> Result<MutationResult<crate::RoadmapSectionRecord>, PlanningStoreError> {
        input.scope_key = Some(self.config.scope_key.clone());
        input.run_id = resolve_run_id(context);
        self.store.add_roadmap_section(input)
    }

    pub fn add_work_point(
        &self,
        context: &PlanningContext,
        mut input: AddWorkPointInput,
    ) -> Result<MutationResult<WorkPointRecord>, PlanningStoreError> {
        input.scope_key = Some(self.config.scope_key.clone());
        input.run_id = resolve_run_id(context);
        self.store.add_work_point(input)
    }

    pub fn create_plan(
        &self,
        context: &PlanningContext,
        mut input: CreatePlanInput,
    ) -> Result<MutationResult<crate::PlanRecord>, PlanningStoreError> {
        input.scope_key = Some(self.config.scope_key.clone());
        input.run_id = resolve_run_id(context);
        self.store.create_plan(input)
    }

    pub fn create_todo(
        &self,
        context: &PlanningContext,
        mut input: CreateTodoInput,
    ) -> Result<MutationResult<crate::TodoRecord>, PlanningStoreError> {
        input.scope_key = Some(self.config.scope_key.clone());
        input.run_id = resolve_run_id(context);
        self.store.create_todo(input)
    }

    pub fn create_issue(
        &self,
        context: &PlanningContext,
        mut input: CreateIssueInput,
    ) -> Result<MutationResult<crate::IssueRecord>, PlanningStoreError> {
        input.scope_key = Some(self.config.scope_key.clone());
        input.run_id = resolve_run_id(context);
        self.store.create_issue(input)
    }

    pub fn create_review_point(
        &self,
        context: &PlanningContext,
        mut input: CreateReviewPointInput,
    ) -> Result<MutationResult<crate::ReviewPointRecord>, PlanningStoreError> {
        input.scope_key = Some(self.config.scope_key.clone());
        input.run_id = resolve_run_id(context);
        self.store.create_review_point(input)
    }

    pub fn goal(&self, id: &str) -> Result<GoalView, PlanningStoreError> {
        let view = self.store.goal(id)?;
        ensure_scope_match("goal", id, &view.goal.scope_key, &self.config.scope_key)?;
        Ok(view)
    }

    pub fn roadmap(&self, id: &str) -> Result<RoadmapView, PlanningStoreError> {
        let view = self.store.roadmap(id)?;
        ensure_scope_match(
            "roadmap",
            id,
            &view.roadmap.scope_key,
            &self.config.scope_key,
        )?;
        Ok(view)
    }

    pub fn work_point(&self, id: &str) -> Result<WorkPointView, PlanningStoreError> {
        let view = self.store.work_point(id)?;
        ensure_scope_match(
            "work point",
            id,
            &view.work_point.scope_key,
            &self.config.scope_key,
        )?;
        Ok(view)
    }

    pub fn plan(&self, id: &str) -> Result<PlanView, PlanningStoreError> {
        let view = self.store.plan(id)?;
        ensure_scope_match("plan", id, &view.plan.scope_key, &self.config.scope_key)?;
        Ok(view)
    }

    pub fn issue(&self, id: &str) -> Result<IssueView, PlanningStoreError> {
        let view = self.store.issue(id)?;
        ensure_scope_match("issue", id, &view.issue.scope_key, &self.config.scope_key)?;
        Ok(view)
    }

    pub fn list_goals(&self) -> Result<Vec<crate::GoalRecord>, PlanningStoreError> {
        self.store.list_goals_in_scope(&self.config.scope_key)
    }

    pub fn list_roadmaps(&self) -> Result<Vec<crate::RoadmapRecord>, PlanningStoreError> {
        self.store.list_roadmaps_in_scope(&self.config.scope_key)
    }

    pub fn list_work_points(&self) -> Result<Vec<WorkPointRecord>, PlanningStoreError> {
        self.store.list_work_points_in_scope(&self.config.scope_key)
    }

    pub fn list_plans(&self) -> Result<Vec<crate::PlanRecord>, PlanningStoreError> {
        self.store.list_plans_in_scope(&self.config.scope_key)
    }

    pub fn list_todos(&self) -> Result<Vec<crate::TodoRecord>, PlanningStoreError> {
        self.store.list_todos_in_scope(&self.config.scope_key)
    }

    pub fn list_issues(&self) -> Result<Vec<crate::IssueRecord>, PlanningStoreError> {
        self.store.list_issues_in_scope(&self.config.scope_key)
    }

    pub fn transition(
        &self,
        context: &PlanningContext,
        mut input: UpdateStatusInput,
    ) -> Result<Value, PlanningStoreError> {
        input.active_scope_key = Some(self.config.scope_key.clone());
        input.run_id = resolve_run_id(context);
        self.store.update_status(input)
    }

    pub fn revise(
        &self,
        context: &PlanningContext,
        mut input: RevisePlanInput,
    ) -> Result<MutationResult<crate::PlanRecord>, PlanningStoreError> {
        input.active_scope_key = Some(self.config.scope_key.clone());
        input.scope_key = input
            .scope_key
            .or_else(|| Some(self.config.scope_key.clone()));
        input.run_id = resolve_run_id(context);
        self.store.revise_plan(input)
    }

    pub fn validate(&self) -> Result<ValidationRunReport, PlanningStoreError> {
        self.store.validate_all()
    }

    pub fn events(&self) -> Result<Vec<crate::PlanningEvent>, PlanningStoreError> {
        self.store.list_events_in_scope(&self.config.scope_key)
    }

    pub fn health(&self) -> Result<PlanningHealthReport, PlanningStoreError> {
        self.store.health()
    }

    pub fn create_insight(
        &self,
        context: &PlanningContext,
        mut input: CreateInsightInput,
    ) -> Result<MutationResult<crate::InsightRecord>, PlanningStoreError> {
        input.scope_key = Some(self.config.scope_key.clone());
        input.run_id = resolve_run_id(context);
        self.store.create_insight(input)
    }

    pub fn insight(&self, id: &str) -> Result<InsightView, PlanningStoreError> {
        let view = self.store.insight(id)?;
        ensure_scope_match(
            "insight",
            id,
            &view.insight.scope_key,
            &self.config.scope_key,
        )?;
        Ok(view)
    }

    pub fn list_insights(
        &self,
        entity_type: EntityType,
        entity_id: &str,
    ) -> Result<Vec<crate::InsightRecord>, PlanningStoreError> {
        self.store
            .list_insights_for_entity(entity_type, entity_id, &self.config.scope_key)
    }

    pub fn list_tags(&self, entity_type: Option<&str>) -> Result<Vec<TagInfo>, PlanningStoreError> {
        self.store.list_tags(&self.config.scope_key, entity_type)
    }

    pub fn context_bundle(
        &self,
        entity_type: EntityType,
        entity_id: &str,
    ) -> Result<crate::EntityContextBundle, PlanningStoreError> {
        self.store
            .context_bundle(entity_type, entity_id, &self.config.scope_key)
    }

    pub fn session_context(
        &self,
        correlation_id: &str,
    ) -> Result<crate::SessionContextBundle, PlanningStoreError> {
        self.store
            .session_context(correlation_id, &self.config.scope_key)
    }

    pub fn render(
        &self,
        entity_type: EntityType,
        entity_id: &str,
        format: ProjectionFormat,
        output_path: &Path,
    ) -> Result<RenderedProjection, PlanningStoreError> {
        self.store.render_projection_in_scope(
            &self.config.scope_key,
            entity_type,
            entity_id,
            format,
            output_path,
        )
    }

    pub fn claim_project_run(
        &self,
        context: &PlanningContext,
        mut input: ClaimProjectRunInput,
    ) -> Result<MutationResult<ProjectRunRecord>, PlanningStoreError> {
        input.scope_key = Some(self.config.scope_key.clone());
        input.run_id = resolve_run_id(context);
        self.store.claim_project_run(input)
    }

    pub fn release_project_run(
        &self,
        context: &PlanningContext,
        mut input: ReleaseProjectRunInput,
    ) -> Result<MutationResult<ProjectRunRecord>, PlanningStoreError> {
        input.active_scope_key = Some(self.config.scope_key.clone());
        input.run_id = resolve_run_id(context);
        self.store.release_project_run(input)
    }

    pub fn add_project_run_evidence(
        &self,
        context: &PlanningContext,
        mut input: AddEvidenceInput,
    ) -> Result<MutationResult<ProjectRunRecord>, PlanningStoreError> {
        input.active_scope_key = Some(self.config.scope_key.clone());
        input.run_id = resolve_run_id(context);
        self.store.add_project_run_evidence(input)
    }

    pub fn list_project_runs(&self) -> Result<Vec<ProjectRunRecord>, PlanningStoreError> {
        self.store.list_project_runs_in_scope(&self.config.scope_key)
    }

    pub fn project_run(&self, id: &str) -> Result<ProjectRunView, PlanningStoreError> {
        let view = self.store.project_run(id)?;
        ensure_scope_match(
            "project run",
            id,
            &view.project_run.scope_key,
            &self.config.scope_key,
        )?;
        Ok(view)
    }

    pub fn find_runnable_work_points(
        &self,
        roadmap_id: &str,
    ) -> Result<RunnableCandidates, PlanningStoreError> {
        let view = self.store.roadmap(roadmap_id)?;
        ensure_scope_match(
            "roadmap",
            roadmap_id,
            &view.roadmap.scope_key,
            &self.config.scope_key,
        )?;
        self.store.find_runnable_work_points(roadmap_id)
    }

    pub fn build_work_graph(
        &self,
        roadmap_id: &str,
    ) -> Result<WorkGraph, PlanningStoreError> {
        let view = self.store.roadmap(roadmap_id)?;
        ensure_scope_match(
            "roadmap",
            roadmap_id,
            &view.roadmap.scope_key,
            &self.config.scope_key,
        )?;
        self.store.build_work_graph(roadmap_id)
    }
}

fn resolve_run_id(context: &PlanningContext) -> Option<String> {
    context
        .run_id
        .clone()
        .or_else(|| context.correlation_id.clone())
}

fn ensure_scope_match(
    entity_label: &str,
    entity_id: &str,
    actual_scope_key: &str,
    expected_scope_key: &str,
) -> Result<(), PlanningStoreError> {
    if actual_scope_key == expected_scope_key {
        return Ok(());
    }

    Err(PlanningStoreError::InvalidInput(format!(
        "{entity_label} `{entity_id}` is in scope `{actual_scope_key}`, not active scope `{expected_scope_key}`"
    )))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::{
        AddWorkPointInput, CreateGoalInput, CreatePlanInput, CreateRoadmapInput, CreateScopeInput,
        CreateTodoInput, GoalStatus, PlanStatus, Priority, ProjectionFormat, TodoStatus,
        WorkPointStatus,
    };

    fn make_service(db_path: &Path, scope_key: &str) -> PlanningService {
        let mut config = PlanningServiceConfig::new(db_path);
        config.scope_key = scope_key.to_string();
        PlanningService::new(config).expect("create planning service")
    }

    fn ensure_scope(store: &PlanningStore, scope_key: &str) {
        if scope_key == "default" {
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

    #[test]
    fn planning_service_enforces_scope_for_show_transition_revise_and_events() {
        let temp = tempdir().expect("temp dir");
        let db_path = temp.path().join("planning.db");
        let store = PlanningStore::new(&db_path);
        store.init().expect("init planning db");
        ensure_scope(&store, "workspace-a");
        ensure_scope(&store, "workspace-b");

        store
            .create_goal(CreateGoalInput {
                id: Some("goal-default-service".to_string()),
                scope_key: None,
                correlation_id: "corr-default-service".to_string(),
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
                id: Some("goal-service-a".to_string()),
                scope_key: Some("workspace-a".to_string()),
                correlation_id: "corr-service-a".to_string(),
                title: "Scoped goal".to_string(),
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
                id: Some("roadmap-service-a".to_string()),
                scope_key: Some("workspace-a".to_string()),
                goal_id: "goal-service-a".to_string(),
                correlation_id: "corr-service-a".to_string(),
                title: "Scoped roadmap".to_string(),
                summary: "Scoped roadmap".to_string(),
                status: crate::RoadmapStatus::Active,
                tags: Vec::new(),
                run_id: None,
            })
            .expect("create scoped roadmap");
        store
            .add_work_point(AddWorkPointInput {
                id: Some("work-point-service-a".to_string()),
                scope_key: Some("workspace-a".to_string()),
                roadmap_id: "roadmap-service-a".to_string(),
                section_id: None,
                title: "Scoped work point".to_string(),
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
                id: Some("plan-service-a".to_string()),
                scope_key: Some("workspace-a".to_string()),
                goal_id: "goal-service-a".to_string(),
                roadmap_id: "roadmap-service-a".to_string(),
                correlation_id: "corr-service-a".to_string(),
                title: "Scoped plan".to_string(),
                summary: "Scoped plan".to_string(),
                scope: "implementation".to_string(),
                assumptions: vec!["a1".to_string()],
                stop_conditions: Vec::new(),
                validation_steps: vec!["validate".to_string()],
                targeted_work_point_ids: vec!["work-point-service-a".to_string()],
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
                id: Some("todo-service-a".to_string()),
                scope_key: Some("workspace-a".to_string()),
                plan_id: Some("plan-service-a".to_string()),
                work_point_id: Some("work-point-service-a".to_string()),
                title: "Scoped todo".to_string(),
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

        let default_service = make_service(&db_path, "default");
        let error = default_service
            .plan("plan-service-a")
            .expect_err("out-of-scope show should fail");
        assert!(error.is_invalid_input());
        assert!(error.to_string().contains("workspace-a"));

        let error = default_service
            .transition(
                &PlanningContext::default(),
                UpdateStatusInput {
                    entity_type: EntityType::Todo,
                    entity_id: "todo-service-a".to_string(),
                    status: "completed".to_string(),
                    evidence_refs: Some(vec!["proof://ci".to_string()]),
                    active_scope_key: None,
                    run_id: None,
                },
            )
            .expect_err("out-of-scope transition should fail");
        assert!(error.is_invalid_input());
        assert!(error.to_string().contains("workspace-a"));

        let error = default_service
            .revise(
                &PlanningContext::default(),
                RevisePlanInput {
                    plan_id: "plan-service-a".to_string(),
                    active_scope_key: None,
                    scope_key: None,
                    assumptions: Some(vec!["a2".to_string()]),
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
                },
            )
            .expect_err("out-of-scope revise should fail");
        assert!(error.is_invalid_input());
        assert!(error.to_string().contains("workspace-a"));

        let default_events = default_service.events().expect("list default events");
        assert!(default_events
            .iter()
            .any(|event| event.entity_id == "goal-default-service"));
        assert!(!default_events
            .iter()
            .any(|event| event.entity_id == "plan-service-a"));

        let workspace_a_service = make_service(&db_path, "workspace-a");
        let view = workspace_a_service
            .plan("plan-service-a")
            .expect("in-scope show succeeds");
        assert_eq!(view.plan.id, "plan-service-a");

        workspace_a_service
            .transition(
                &PlanningContext::default(),
                UpdateStatusInput {
                    entity_type: EntityType::Todo,
                    entity_id: "todo-service-a".to_string(),
                    status: "completed".to_string(),
                    evidence_refs: Some(vec!["proof://ci".to_string()]),
                    active_scope_key: None,
                    run_id: None,
                },
            )
            .expect("in-scope transition succeeds");

        let workspace_a_events = workspace_a_service
            .events()
            .expect("list workspace-a events");
        assert!(workspace_a_events
            .iter()
            .any(|event| event.entity_id == "plan-service-a"));
        assert!(!workspace_a_events
            .iter()
            .any(|event| event.entity_id == "goal-default-service"));

        let error = workspace_a_service
            .revise(
                &PlanningContext::default(),
                RevisePlanInput {
                    plan_id: "plan-service-a".to_string(),
                    active_scope_key: None,
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
                    run_id: None,
                },
            )
            .expect_err("incompatible scope transfer should fail");
        assert!(error.is_invalid_input());
        assert!(error.to_string().contains("workspace-b"));

        let workspace_b_service = make_service(&db_path, "workspace-b");
        let moved_view = workspace_b_service
            .plan("plan-service-a")
            .expect_err("plan should not move into workspace-b");
        assert!(moved_view.is_invalid_input());

        let workspace_b_events = workspace_b_service
            .events()
            .expect("list workspace-b events");
        assert!(!workspace_b_events.iter().any(|event| {
            event.entity_id == "plan-service-a" && event.event_type == "plan.revised"
        }));

        let render_target = temp.path().join("goal.json");
        let render_error = default_service
            .render(
                EntityType::Goal,
                "goal-service-a",
                ProjectionFormat::Json,
                &render_target,
            )
            .expect_err("out-of-scope render should fail");
        assert!(render_error.is_invalid_input());

        workspace_a_service
            .render(
                EntityType::Goal,
                "goal-service-a",
                ProjectionFormat::Json,
                &render_target,
            )
            .expect("in-scope render succeeds");
    }
}
