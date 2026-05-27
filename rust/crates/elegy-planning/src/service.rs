use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::{
    AddRoadmapSectionInput, AddWorkPointInput, CreateGoalInput, CreateIssueInput, CreatePlanInput,
    CreateReviewPointInput, CreateRoadmapInput, CreateTodoInput, EntityType, GoalView, IssueView,
    MutationResult, PlanView, PlanningHealthReport, PlanningStore, PlanningStoreError,
    ProjectionFormat, RenderedProjection, RevisePlanInput, RoadmapView, ScopeRecord,
    UpdateStatusInput, ValidationRunReport, WorkPointRecord, WorkPointView,
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
        self.store.goal(id)
    }

    pub fn roadmap(&self, id: &str) -> Result<RoadmapView, PlanningStoreError> {
        self.store.roadmap(id)
    }

    pub fn work_point(&self, id: &str) -> Result<WorkPointView, PlanningStoreError> {
        self.store.work_point(id)
    }

    pub fn plan(&self, id: &str) -> Result<PlanView, PlanningStoreError> {
        self.store.plan(id)
    }

    pub fn issue(&self, id: &str) -> Result<IssueView, PlanningStoreError> {
        self.store.issue(id)
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
        input.run_id = resolve_run_id(context);
        self.store.update_status(input)
    }

    pub fn revise(
        &self,
        context: &PlanningContext,
        mut input: RevisePlanInput,
    ) -> Result<MutationResult<crate::PlanRecord>, PlanningStoreError> {
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
        self.store.list_events()
    }

    pub fn health(&self) -> Result<PlanningHealthReport, PlanningStoreError> {
        self.store.health()
    }

    pub fn render(
        &self,
        entity_type: EntityType,
        entity_id: &str,
        format: ProjectionFormat,
        output_path: &Path,
    ) -> Result<RenderedProjection, PlanningStoreError> {
        self.store
            .render_projection(entity_type, entity_id, format, output_path)
    }
}

fn resolve_run_id(context: &PlanningContext) -> Option<String> {
    context
        .run_id
        .clone()
        .or_else(|| context.correlation_id.clone())
}
