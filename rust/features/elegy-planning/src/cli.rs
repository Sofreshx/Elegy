use std::{env, ffi::OsString, path::PathBuf, process::ExitCode, sync::OnceLock};

use clap::{error::ErrorKind, Args, CommandFactory, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::json;
use thiserror::Error;

use crate::envelope::{MachineEnvelope, MachineStatus};
use crate::storage::CURRENT_SCHEMA_VERSION;
use crate::{
    manifest, AcceptanceKind, ActivateProjectRunInput, AddEvidenceInput, AddRoadmapSectionInput,
    AddWorkPointInput, AttachEvidenceInput, AttachWorktreeInput, ClaimProjectRunInput,
    CompactGraphEdge, CompactGraphNode, CreateAcceptanceInput, CreateEvidenceInput,
    CreateGoalInput, CreateGraphEdgeInput, CreateGraphNodeInput, CreateInsightInput,
    CreateIssueInput, CreatePlanInput, CreateReviewPointInput, CreateRoadmapInput,
    CreateScopeInput, CreateTodoInput, EffortTier, EntityType, EvidenceKind, FieldDiff,
    FileScopeIntent, FileScopeRecord, FileScopeSelectorType, FinalizeGraphNodeInput, GoalStatus,
    InsightStatus, InsightType, IssueStatus, ManifestDiffEntry, ManifestDiffResult, PlanStatus,
    PlanningEdgeKind, PlanningGraphEdge, PlanningGraphNode, PlanningNodeKind, PlanningStore,
    PlanningStoreError, Priority, ProjectRunEvidence, ProjectRunStatus, ProjectionFormat,
    ReleaseProjectRunInput, ReviewPointStatus, ReviseGraphEdgeInput, ReviseGraphNodeInput,
    RevisePlanInput, ReviseWorkPointInput, RoadmapStatus, SatisfyAcceptanceInput, SearchInput,
    Severity, TodoStatus, UpdateGraphEdgeStatusInput, UpdateGraphNodeStatusInput,
    UpdateStatusInput, WorkPointKind, WorkPointStatus, WorktreeStatus,
};

const EXIT_CODE_INVALID_INPUT: u8 = 1;
const EXIT_CODE_RUNTIME_FAILURE: u8 = 2;
pub(crate) const RESULT_SCHEMA_VERSION: &str = "planning-result/v1";

static CLI_MACHINE_CONTEXT: OnceLock<MachineContext> = OnceLock::new();

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Store(#[from] crate::PlanningStoreError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Parser, Debug)]
#[command(name = "elegy-planning")]
#[command(
    about = "Dedicated planning authority CLI for durable goals, roadmaps, plans, todos, and issues"
)]
struct Cli {
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    format: OutputFormat,
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    non_interactive: bool,
    #[arg(long, global = true)]
    correlation_id: Option<String>,
    #[arg(long, global = true, default_value = "default")]
    scope: String,
    #[arg(long, global = true)]
    db: Option<PathBuf>,
    #[arg(long, global = true)]
    compact: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Subcommand, Debug)]
enum Command {
    #[command(about = "Manage planning scopes")]
    Scope {
        #[command(subcommand)]
        command: ScopeCommand,
    },
    #[command(about = "Create and manage goals with acceptance criteria")]
    Goal {
        #[command(subcommand)]
        command: GoalCommand,
    },
    #[command(about = "Manage roadmaps linked to goals")]
    Roadmap {
        #[command(subcommand)]
        command: RoadmapCommand,
    },
    #[command(about = "Manage work points within plans")]
    WorkPoint {
        #[command(subcommand)]
        command: WorkPointCommand,
    },
    #[command(about = "Create and manage plans with scope and roadmap references")]
    Plan {
        #[command(subcommand)]
        command: PlanCommand,
    },
    #[command(about = "Manage actionable todo items")]
    Todo {
        #[command(subcommand)]
        command: TodoCommand,
    },
    #[command(about = "Track and manage issues")]
    Issue {
        #[command(subcommand)]
        command: IssueCommand,
    },
    #[command(about = "Manage review points for quality gates")]
    ReviewPoint {
        #[command(subcommand)]
        command: ReviewPointCommand,
    },
    #[command(about = "Run validation checks across planning entities")]
    Validate {
        #[command(subcommand)]
        command: ValidateCommand,
    },
    #[command(about = "View and manage event history")]
    Events,
    #[command(about = "Check planning database health")]
    Health,
    #[command(about = "Report machine-readable CLI compatibility metadata")]
    Capabilities,
    #[command(about = "Manage project-level configuration")]
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    #[command(about = "Manage operational sessions")]
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    #[command(about = "Search across planning entities")]
    Search(SearchArgs),
    #[command(about = "Manage retrospective insights")]
    Insight {
        #[command(subcommand)]
        command: InsightCommand,
    },
    #[command(about = "Manage contextual information")]
    Context(ContextArgs),
    #[command(about = "Manage tagging across entities")]
    Tags(TagsArgs),
    #[command(about = "Manage project run records")]
    ProjectRun {
        #[command(subcommand)]
        command: ProjectRunCommand,
    },
    #[command(about = "Manage registered worktrees")]
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommand,
    },
    #[command(about = "Inspect and manage the planning graph")]
    Graph {
        #[command(subcommand)]
        command: GraphCommand,
    },
    #[command(
        about = "Apply a planning manifest (YAML/JSON) to create or update a complete graph"
    )]
    Manifest(ManifestArgs),
    #[command(about = "Diff a planning manifest against current database state")]
    Diff(DiffArgs),
    #[command(about = "List or render planning manifest templates")]
    Template {
        #[command(subcommand)]
        command: TemplateCommand,
    },
    #[command(about = "Expand a planning intent document into a manifest")]
    Intent(IntentExpandArgs),
}

#[derive(Subcommand, Debug)]
enum TemplateCommand {
    List(TemplateListArgs),
    Render(TemplateRenderArgs),
}

#[derive(Subcommand, Debug)]
enum GoalCommand {
    Create(GoalCreateArgs),
    UpdateStatus(GoalUpdateStatusArgs),
    List,
    Show(GoalShowArgs),
    Search(EntitySearchArgs),
}

#[derive(Subcommand, Debug)]
enum RoadmapCommand {
    Create(RoadmapCreateArgs),
    UpdateStatus(RoadmapUpdateStatusArgs),
    AddSection(RoadmapAddSectionArgs),
    AddWorkPoint(RoadmapAddWorkPointArgs),
    List,
    Show(RoadmapShowArgs),
    Search(EntitySearchArgs),
}

#[derive(Subcommand, Debug)]
enum WorkPointCommand {
    List,
    Show(WorkPointShowArgs),
    UpdateStatus(WorkPointUpdateStatusArgs),
    NextRunnable(WorkPointNextRunnableArgs),
    WorkGraph(WorkPointWorkGraphArgs),
    Revise(WorkPointReviseArgs),
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
enum PlanCommand {
    Create(PlanCreateArgs),
    Revise(PlanReviseArgs),
    UpdateStatus(PlanUpdateStatusArgs),
    List,
    Show(PlanShowArgs),
    Search(EntitySearchArgs),
}

#[derive(Subcommand, Debug)]
enum TodoCommand {
    Create(TodoCreateArgs),
    UpdateStatus(TodoUpdateStatusArgs),
    List,
    Search(EntitySearchArgs),
}

#[derive(Subcommand, Debug)]
enum IssueCommand {
    Record(IssueRecordArgs),
    UpdateStatus(IssueUpdateStatusArgs),
    List,
    Show(IssueShowArgs),
    Search(EntitySearchArgs),
}

#[derive(Subcommand, Debug)]
enum ReviewPointCommand {
    Record(ReviewPointRecordArgs),
    UpdateStatus(ReviewPointUpdateStatusArgs),
}

#[derive(Subcommand, Debug)]
enum ValidateCommand {
    All(ValidateAllArgs),
}

#[derive(Args, Debug)]
struct ValidateAllArgs {
    #[arg(long = "all-scopes")]
    all_scopes: bool,
}

#[derive(Subcommand, Debug)]
enum ProjectCommand {
    Export(ProjectRenderArgs),
    Render(ProjectRenderArgs),
}

#[derive(Subcommand, Debug)]
enum SessionCommand {
    Init(SessionInitArgs),
    Use(SessionUseArgs),
    Show,
    Resume(SessionResumeArgs),
    List(SessionListArgs),
}

#[derive(Args, Debug)]
struct SessionInitArgs {
    #[arg(long, default_value = "default")]
    scope: String,
}

#[derive(Args, Debug)]
struct SessionUseArgs {
    #[arg(long = "session-id")]
    session_id: String,
}

#[derive(Args, Debug)]
struct SessionResumeArgs {
    #[arg(long)]
    session_id: Option<String>,
}

#[derive(Args, Debug)]
struct SessionListArgs {
    #[arg(long, default_value = "10")]
    limit: i64,
}

#[derive(Args, Debug)]
struct EntitySearchArgs {
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    since: Option<String>,
    #[arg(long)]
    latest: Option<usize>,
    #[arg(long)]
    tag: Option<String>,
    #[arg(long)]
    fts: Option<String>,
}

#[derive(Args, Debug)]
struct SearchArgs {
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    since: Option<String>,
    #[arg(long)]
    latest: Option<usize>,
    #[arg(long)]
    tag: Option<String>,
    #[arg(long)]
    fts: Option<String>,
}

#[derive(Subcommand, Debug)]
enum InsightCommand {
    Record(InsightRecordArgs),
    List(InsightListArgs),
    Show(InsightShowArgs),
    Search(InsightSearchArgs),
    UpdateStatus(InsightUpdateStatusArgs),
}

#[derive(Args, Debug)]
struct InsightRecordArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long)]
    title: String,
    #[arg(long)]
    content: String,
    #[arg(long, value_enum)]
    insight_type: InsightType,
    #[arg(long = "parent-type", value_enum)]
    parent_entity_type: EntityType,
    #[arg(long = "parent-id")]
    parent_entity_id: String,
    #[arg(long = "tag")]
    tags: Vec<String>,
    #[arg(long, value_enum, default_value_t = InsightStatus::Active)]
    status: InsightStatus,
}

#[derive(Args, Debug)]
struct InsightListArgs {
    #[arg(long = "all")]
    all: bool,
    #[arg(long = "parent-type", value_enum)]
    parent_entity_type: Option<EntityType>,
    #[arg(long = "parent-id")]
    parent_entity_id: Option<String>,
}

#[derive(Args, Debug)]
struct InsightShowArgs {
    #[arg(long = "insight-id")]
    insight_id: String,
}

#[derive(Args, Debug)]
struct InsightSearchArgs {
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    since: Option<String>,
    #[arg(long)]
    latest: Option<usize>,
    #[arg(long)]
    tag: Option<String>,
    #[arg(long)]
    fts: Option<String>,
}

#[derive(Args, Debug)]
struct InsightUpdateStatusArgs {
    #[arg(long = "insight-id")]
    insight_id: String,
    #[arg(long, value_enum)]
    status: InsightStatus,
    #[arg(long)]
    override_transition: bool,
    #[arg(long)]
    reason: Option<String>,
}

#[derive(Args, Debug)]
struct ContextArgs {
    #[arg(long = "entity-type", value_enum)]
    entity_type: Option<EntityType>,
    #[arg(long = "entity-id")]
    entity_id: Option<String>,
    #[arg(long)]
    session: bool,
}

#[derive(Args, Debug)]
struct TagsArgs {
    #[arg(long = "entity-type")]
    entity_type: Option<String>,
}

#[derive(Subcommand, Debug)]
enum ScopeCommand {
    Create(ScopeCreateArgs),
    List,
    Show(ScopeShowArgs),
}

#[derive(Args, Debug)]
struct ScopeCreateArgs {
    #[arg(long = "scope-key")]
    scope_key: String,
    #[arg(long = "scope-type")]
    scope_type: Option<String>,
    #[arg(long = "parent-scope-key")]
    parent_scope_key: Option<String>,
    #[arg(long = "metadata-json")]
    metadata_json: Option<String>,
    #[arg(long = "metadata-file")]
    metadata_file: Option<PathBuf>,
    #[arg(long = "tag")]
    tags: Vec<String>,
}

#[derive(Args, Debug)]
struct ScopeShowArgs {
    #[arg(long = "scope-key")]
    scope_key: String,
}

#[derive(Args, Debug)]
struct GoalCreateArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long)]
    title: String,
    #[arg(long)]
    description: String,
    #[arg(long = "acceptance")]
    acceptance_criteria: Vec<String>,
    #[arg(long = "rejection")]
    rejection_criteria: Vec<String>,
    #[arg(long, value_enum, default_value_t = GoalStatus::Draft)]
    status: GoalStatus,
    #[arg(long = "tag")]
    tags: Vec<String>,
}

#[derive(Args, Debug)]
struct GoalUpdateStatusArgs {
    #[arg(long = "goal-id")]
    goal_id: String,
    #[arg(long, value_enum)]
    status: GoalStatus,
    #[arg(long)]
    override_transition: bool,
    #[arg(long)]
    reason: Option<String>,
}

#[derive(Args, Debug)]
struct GoalShowArgs {
    #[arg(long = "goal-id")]
    goal_id: String,
}

#[derive(Args, Debug)]
struct RoadmapCreateArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long = "goal-id")]
    goal_id: String,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long)]
    title: String,
    #[arg(long)]
    summary: String,
    #[arg(long, value_enum, default_value_t = RoadmapStatus::Draft)]
    status: RoadmapStatus,
    #[arg(long = "tag")]
    tags: Vec<String>,
}

#[derive(Args, Debug)]
struct RoadmapAddSectionArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long = "roadmap-id")]
    roadmap_id: String,
    #[arg(long)]
    slug: String,
    #[arg(long)]
    title: String,
    #[arg(long, default_value = "")]
    summary: String,
    #[arg(long)]
    ordering: Option<i64>,
}

#[derive(Args, Debug)]
struct RoadmapAddWorkPointArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long = "roadmap-id")]
    roadmap_id: String,
    #[arg(long = "section-id")]
    section_id: Option<String>,
    #[arg(long)]
    title: String,
    #[arg(long)]
    summary: String,
    #[arg(long, value_enum, default_value_t = WorkPointStatus::Draft)]
    status: WorkPointStatus,
    #[arg(long)]
    ordering: Option<i64>,
    #[arg(long = "dependency-id")]
    dependency_ids: Vec<String>,
    #[arg(long = "validation")]
    validation_expectations: Vec<String>,
    #[arg(long, value_enum, default_value_t = EffortTier::Balanced)]
    effort_tier: EffortTier,
    #[arg(long = "file-scope")]
    file_scopes: Vec<String>,
    #[arg(long = "tag")]
    tags: Vec<String>,
    #[arg(long, value_enum)]
    kind: Option<WorkPointKind>,
    #[arg(long, value_enum)]
    priority: Option<Priority>,
    #[arg(long = "repairs-work-point-id")]
    repairs_work_point_ids: Vec<String>,
    #[arg(long = "supersedes-work-point-id")]
    supersedes_work_point_ids: Vec<String>,
    #[arg(long = "blocks-work-point-id")]
    blocks_work_point_ids: Vec<String>,
}

#[derive(Args, Debug)]
struct RoadmapShowArgs {
    #[arg(long = "roadmap-id")]
    roadmap_id: String,
}

#[derive(Args, Debug)]
struct RoadmapUpdateStatusArgs {
    #[arg(long = "roadmap-id")]
    roadmap_id: String,
    #[arg(long, value_enum)]
    status: RoadmapStatus,
    #[arg(long)]
    override_transition: bool,
    #[arg(long)]
    reason: Option<String>,
}

#[derive(Args, Debug)]
struct PlanCreateArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long = "goal-id")]
    goal_id: String,
    #[arg(long = "roadmap-id")]
    roadmap_id: String,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long)]
    title: String,
    #[arg(long)]
    summary: String,
    #[arg(long = "plan-scope")]
    plan_scope: String,
    #[arg(long = "assumption")]
    assumptions: Vec<String>,
    #[arg(long = "stop-condition")]
    stop_conditions: Vec<String>,
    #[arg(long = "validation-step")]
    validation_steps: Vec<String>,
    #[arg(long = "target-work-point-id")]
    targeted_work_point_ids: Vec<String>,
    #[arg(long, value_enum, default_value_t = EffortTier::Balanced)]
    effort_tier: EffortTier,
    #[arg(long = "routing-hint")]
    routing_hint: Option<String>,
    #[arg(long, default_value_t = false)]
    allow_parallel_overlap: bool,
    #[arg(long = "file-scope")]
    file_scopes: Vec<String>,
    #[arg(long, value_enum, default_value_t = PlanStatus::Draft)]
    status: PlanStatus,
    #[arg(long = "tag")]
    tags: Vec<String>,
}

#[derive(Args, Debug)]
struct PlanShowArgs {
    #[arg(long = "plan-id")]
    plan_id: String,
}

#[derive(Args, Debug)]
struct PlanReviseArgs {
    #[arg(long = "plan-id")]
    plan_id: String,
    #[arg(long = "scope-key")]
    scope_key: Option<String>,
    #[arg(long = "assumption")]
    assumptions: Vec<String>,
    #[arg(long = "stop-condition")]
    stop_conditions: Vec<String>,
    #[arg(long = "validation-step")]
    validation_steps: Vec<String>,
    #[arg(long = "target-work-point-id")]
    targeted_work_point_ids: Vec<String>,
    #[arg(long, value_enum)]
    effort_tier: Option<EffortTier>,
    #[arg(long = "routing-hint")]
    routing_hint: Option<String>,
    #[arg(long, default_value_t = false)]
    clear_routing_hint: bool,
    #[arg(long)]
    allow_parallel_overlap: Option<bool>,
    #[arg(long = "file-scope")]
    file_scopes: Vec<String>,
    #[arg(long, default_value_t = false)]
    clear_file_scopes: bool,
    #[arg(long = "tag")]
    tags: Vec<String>,
}

#[derive(Args, Debug)]
struct PlanUpdateStatusArgs {
    #[arg(long = "plan-id")]
    plan_id: String,
    #[arg(long, value_enum)]
    status: PlanStatus,
    #[arg(long)]
    override_transition: bool,
    #[arg(long)]
    reason: Option<String>,
}

#[derive(Args, Debug)]
struct TodoCreateArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long = "plan-id")]
    plan_id: Option<String>,
    #[arg(long = "work-point-id")]
    work_point_id: Option<String>,
    #[arg(long)]
    title: String,
    #[arg(long, default_value = "")]
    summary: String,
    #[arg(long, value_enum, default_value_t = TodoStatus::Pending)]
    status: TodoStatus,
    #[arg(long, value_enum, default_value_t = Priority::Medium)]
    priority: Priority,
    #[arg(long, value_enum, default_value_t = EffortTier::Balanced)]
    effort_tier: EffortTier,
    #[arg(long = "file-scope")]
    file_scopes: Vec<String>,
    #[arg(long = "evidence-ref")]
    evidence_refs: Vec<String>,
    #[arg(long = "tag")]
    tags: Vec<String>,
    #[arg(long)]
    ordering: Option<i64>,
}

#[derive(Args, Debug)]
struct TodoUpdateStatusArgs {
    #[arg(long = "todo-id")]
    todo_id: String,
    #[arg(long, value_enum)]
    status: TodoStatus,
    #[arg(long = "evidence-ref")]
    evidence_refs: Vec<String>,
    #[arg(long)]
    override_transition: bool,
    #[arg(long)]
    reason: Option<String>,
}

#[derive(Args, Debug)]
struct IssueRecordArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long)]
    title: String,
    #[arg(long)]
    summary: String,
    #[arg(long, value_enum, default_value_t = IssueStatus::Open)]
    status: IssueStatus,
    #[arg(long, value_enum, default_value_t = Severity::Medium)]
    severity: Severity,
    #[arg(long = "related-entity-type", value_enum)]
    related_entity_type: Option<EntityType>,
    #[arg(long = "related-entity-id")]
    related_entity_id: Option<String>,
    #[arg(long = "tag")]
    tags: Vec<String>,
}

#[derive(Args, Debug)]
struct IssueShowArgs {
    #[arg(long = "issue-id")]
    issue_id: String,
}

#[derive(Args, Debug)]
struct IssueUpdateStatusArgs {
    #[arg(long = "issue-id")]
    issue_id: String,
    #[arg(long, value_enum)]
    status: IssueStatus,
    #[arg(long)]
    override_transition: bool,
    #[arg(long)]
    reason: Option<String>,
}

#[derive(Args, Debug)]
struct ReviewPointRecordArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long = "entity-type", value_enum)]
    attached_entity_type: EntityType,
    #[arg(long = "entity-id")]
    attached_entity_id: String,
    #[arg(long)]
    title: String,
    #[arg(long)]
    summary: String,
    #[arg(long, value_enum, default_value_t = ReviewPointStatus::Open)]
    status: ReviewPointStatus,
    #[arg(long, value_enum, default_value_t = Severity::Medium)]
    severity: Severity,
}

#[derive(Args, Debug)]
struct ReviewPointUpdateStatusArgs {
    #[arg(long = "review-point-id")]
    review_point_id: String,
    #[arg(long, value_enum)]
    status: ReviewPointStatus,
    #[arg(long)]
    override_transition: bool,
    #[arg(long)]
    reason: Option<String>,
}

#[derive(Args, Debug)]
struct WorkPointShowArgs {
    #[arg(long = "work-point-id")]
    work_point_id: String,
}

#[derive(Args, Debug)]
struct WorkPointUpdateStatusArgs {
    #[arg(long = "work-point-id")]
    work_point_id: String,
    #[arg(long, value_enum)]
    status: WorkPointStatus,
    #[arg(long)]
    override_transition: bool,
    #[arg(long)]
    reason: Option<String>,
}

#[derive(Args, Debug)]
struct WorkPointNextRunnableArgs {
    #[arg(long = "roadmap-id")]
    roadmap_id: String,
}

#[derive(Args, Debug)]
struct WorkPointWorkGraphArgs {
    #[arg(long = "roadmap-id")]
    roadmap_id: String,
}

#[derive(Args, Debug)]
struct WorkPointReviseArgs {
    #[arg(long = "work-point-id")]
    work_point_id: String,
    #[arg(long = "dependency-id")]
    dependency_ids: Vec<String>,
    #[arg(long = "clear-dependencies")]
    clear_dependencies: bool,
    #[arg(long = "blocks-work-point-id")]
    blocks_work_point_ids: Vec<String>,
    #[arg(long = "clear-blocks")]
    clear_blocks: bool,
}

#[derive(Subcommand, Debug)]
enum ProjectRunCommand {
    Claim(Box<ProjectRunClaimArgs>),
    Activate(ProjectRunActivateArgs),
    Heartbeat(ProjectRunHeartbeatArgs),
    Release(ProjectRunReleaseArgs),
    AddEvidence(ProjectRunAddEvidenceArgs),
    List,
    Show(ProjectRunShowArgs),
}

#[derive(Args, Debug)]
struct ProjectRunClaimArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long = "goal-id")]
    goal_id: String,
    #[arg(long = "roadmap-id")]
    roadmap_id: String,
    #[arg(long = "work-point-id")]
    work_point_id: String,
    #[arg(long = "repo-id")]
    repo_id: Option<String>,
    #[arg(long)]
    branch: Option<String>,
    #[arg(long = "worktree-id")]
    worktree_id: Option<String>,
    #[arg(long = "session-id")]
    session_id: Option<String>,
    #[arg(long = "profile-id")]
    profile_id: Option<String>,
    #[arg(long = "correlation-id")]
    correlation_id: Option<String>,
    #[arg(long = "owner-id")]
    owner_id: Option<String>,
    #[arg(long = "idempotency-key")]
    idempotency_key: Option<String>,
    #[arg(long = "lease-seconds", default_value_t = 900)]
    lease_seconds: i64,
}

#[derive(Args, Debug)]
struct ProjectRunActivateArgs {
    #[arg(long = "project-run-id")]
    project_run_id: String,
    #[arg(long = "fencing-token")]
    fencing_token: Option<i64>,
}

#[derive(Args, Debug)]
struct ProjectRunHeartbeatArgs {
    #[arg(long = "project-run-id")]
    project_run_id: String,
    #[arg(long = "fencing-token")]
    fencing_token: Option<i64>,
    #[arg(long = "lease-seconds", default_value_t = 900)]
    lease_seconds: i64,
}

#[derive(Args, Debug)]
struct ProjectRunReleaseArgs {
    #[arg(long = "project-run-id")]
    project_run_id: String,
    #[arg(long, value_enum)]
    status: ProjectRunStatus,
    #[arg(long = "evidence-json")]
    evidence_json: Option<String>,
    #[arg(long = "fencing-token")]
    fencing_token: Option<i64>,
}

#[derive(Args, Debug)]
struct ProjectRunAddEvidenceArgs {
    #[arg(long = "project-run-id")]
    project_run_id: String,
    #[arg(long = "evidence-json")]
    evidence_json: String,
    #[arg(long = "fencing-token")]
    fencing_token: Option<i64>,
}

#[derive(Args, Debug)]
struct ProjectRunShowArgs {
    #[arg(long = "project-run-id")]
    project_run_id: String,
}

#[derive(Subcommand, Debug)]
enum WorktreeCommand {
    List(WorktreeListArgs),
    Show(WorktreeShowArgs),
    Attach(WorktreeAttachArgs),
    Archive(WorktreeArchiveArgs),
    CleanupIntent(WorktreeCleanupIntentArgs),
}

#[derive(Args, Debug)]
struct WorktreeListArgs {
    #[arg(long)]
    status: Option<String>,
}

#[derive(Args, Debug)]
struct WorktreeShowArgs {
    #[arg(long)]
    id: String,
}

#[derive(Args, Debug)]
struct WorktreeAttachArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long = "repo-uri")]
    repo_uri: Option<String>,
    #[arg(long)]
    branch: Option<String>,
    #[arg(long = "worktree-path")]
    worktree_path: Option<String>,
    #[arg(long = "project-run-id")]
    project_run_id: Option<String>,
    #[arg(long = "session-id")]
    session_id: Option<String>,
    #[arg(long = "correlation-id")]
    correlation_id: Option<String>,
}

#[derive(Args, Debug)]
struct WorktreeArchiveArgs {
    #[arg(long)]
    id: String,
}

#[derive(Args, Debug)]
struct WorktreeCleanupIntentArgs {
    #[arg(long)]
    id: String,
}

#[derive(Subcommand, Debug)]
enum GraphCommand {
    #[command(about = "Manage graph nodes (goals, work items, milestones, etc.)")]
    Node {
        #[command(subcommand)]
        command: GraphNodeCommand,
    },
    #[command(about = "Manage graph edges (dependencies, decompositions, etc.)")]
    Edge {
        #[command(subcommand)]
        command: GraphEdgeCommand,
    },
    #[command(about = "Manage acceptance criteria (abstract and concrete requirements)")]
    Acceptance {
        #[command(subcommand)]
        command: AcceptanceCommand,
    },
    #[command(about = "Manage evidence (test results, artifacts, reviews, etc.)")]
    Evidence {
        #[command(subcommand)]
        command: EvidenceCommand,
    },
    #[command(about = "Find runnable work nodes in the graph")]
    Runnable(GraphRunnableArgs),
    #[command(about = "Bulk update status for graph nodes")]
    Bulk(BulkTransitionArgs),
}

#[derive(Subcommand, Debug)]
enum GraphNodeCommand {
    Create(GraphNodeCreateArgs),
    Show(GraphNodeShowArgs),
    List(GraphNodeListArgs),
    Status(GraphNodeStatusArgs),
    Revise(GraphNodeReviseArgs),
    Finalize(GraphNodeFinalizeArgs),
}

#[derive(Subcommand, Debug)]
enum GraphEdgeCommand {
    Create(GraphEdgeCreateArgs),
    Show(GraphEdgeShowArgs),
    List(GraphEdgeListArgs),
    Incoming(GraphEdgeIncomingArgs),
    Outgoing(GraphEdgeOutgoingArgs),
    Status(GraphEdgeStatusArgs),
    Revise(GraphEdgeReviseArgs),
}

#[derive(Subcommand, Debug)]
enum AcceptanceCommand {
    Create(AcceptanceCreateArgs),
    Show(AcceptanceShowArgs),
    List(AcceptanceListArgs),
    Satisfy(AcceptanceSatisfyArgs),
}

#[derive(Subcommand, Debug)]
enum EvidenceCommand {
    Create(EvidenceCreateArgs),
    Show(EvidenceShowArgs),
    List(EvidenceListArgs),
    Attach(EvidenceAttachArgs),
}

#[derive(Args, Debug)]
struct AcceptanceCreateArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long, value_enum)]
    acceptance_kind: AcceptanceKind,
    #[arg(long)]
    title: String,
    #[arg(long)]
    summary: String,
    #[arg(long, default_value = "active")]
    status: String,
    #[arg(long)]
    description: String,
    #[arg(long, default_value = "manual-review")]
    verification_policy: String,
    #[arg(long = "required-evidence-kind", value_enum)]
    required_evidence_kinds: Vec<EvidenceKind>,
    #[arg(long)]
    waiver: Option<String>,
    #[arg(long = "tag")]
    tags: Vec<String>,
}

#[derive(Args, Debug)]
struct AcceptanceShowArgs {
    #[arg(long = "node-id")]
    node_id: String,
}

#[derive(Args, Debug)]
struct AcceptanceListArgs {
    #[arg(long, value_enum)]
    kind: Option<AcceptanceKind>,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Args, Debug)]
struct AcceptanceSatisfyArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long = "concrete-id")]
    concrete_node_id: String,
    #[arg(long = "abstract-id")]
    abstract_node_id: String,
    #[arg(long)]
    rationale: String,
}

#[derive(Args, Debug)]
struct EvidenceCreateArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long, value_enum)]
    evidence_kind: EvidenceKind,
    #[arg(long)]
    title: String,
    #[arg(long)]
    summary: String,
    #[arg(long, default_value = "active")]
    status: String,
    #[arg(long)]
    reference: String,
    #[arg(long)]
    content: String,
    #[arg(long)]
    captured_at: String,
    #[arg(long = "tag")]
    tags: Vec<String>,
}

#[derive(Args, Debug)]
struct EvidenceShowArgs {
    #[arg(long = "node-id")]
    node_id: String,
}

#[derive(Args, Debug)]
struct EvidenceListArgs {
    #[arg(long, value_enum)]
    kind: Option<EvidenceKind>,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Args, Debug)]
struct EvidenceAttachArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long = "evidence-id")]
    evidence_node_id: String,
    #[arg(long = "target-id")]
    target_node_id: String,
    #[arg(long)]
    rationale: String,
}

#[derive(Args, Debug)]
struct GraphNodeCreateArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long, value_enum)]
    kind: PlanningNodeKind,
    #[arg(long)]
    title: String,
    #[arg(long)]
    summary: String,
    #[arg(long)]
    status: String,
    #[arg(long = "payload-json")]
    payload_json: Option<String>,
    #[arg(long = "payload-file")]
    payload_file: Option<PathBuf>,
    #[arg(long = "tag")]
    tags: Vec<String>,
}

#[derive(Args, Debug)]
struct GraphNodeShowArgs {
    #[arg(long = "node-id")]
    node_id: String,
}

#[derive(Args, Debug)]
struct GraphNodeListArgs {
    #[arg(long, value_enum)]
    kind: Option<PlanningNodeKind>,
    #[arg(long)]
    limit: Option<usize>,
}

#[derive(Args, Debug)]
struct GraphEdgeCreateArgs {
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long, value_enum)]
    kind: PlanningEdgeKind,
    #[arg(long = "source-node-id")]
    source_node_id: String,
    #[arg(long = "target-node-id")]
    target_node_id: String,
    #[arg(long)]
    status: String,
    #[arg(long = "payload-json")]
    payload_json: Option<String>,
    #[arg(long = "payload-file")]
    payload_file: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct GraphEdgeShowArgs {
    #[arg(long = "edge-id")]
    edge_id: String,
}

#[derive(Args, Debug)]
struct GraphEdgeListArgs {
    #[arg(long, value_enum)]
    kind: Option<PlanningEdgeKind>,
    #[arg(long)]
    limit: Option<usize>,
}

#[derive(Args, Debug)]
struct GraphEdgeIncomingArgs {
    #[arg(long = "node-id")]
    node_id: String,
    #[arg(long, value_enum)]
    kind: Option<PlanningEdgeKind>,
}

#[derive(Args, Debug)]
struct GraphEdgeOutgoingArgs {
    #[arg(long = "node-id")]
    node_id: String,
    #[arg(long, value_enum)]
    kind: Option<PlanningEdgeKind>,
}

#[derive(Args, Debug)]
struct GraphNodeStatusArgs {
    #[arg(long = "node-id")]
    node_id: String,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long)]
    status: String,
}

#[derive(Args, Debug)]
struct GraphNodeReviseArgs {
    #[arg(long = "node-id")]
    node_id: String,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    summary: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long = "payload-json")]
    payload_json: Option<String>,
    #[arg(long = "payload-file")]
    payload_file: Option<PathBuf>,
    #[arg(long = "tag")]
    tags: Vec<String>,
    #[arg(long, default_value_t = false)]
    clear_tags: bool,
}

#[derive(Args, Debug)]
struct GraphNodeFinalizeArgs {
    #[arg(long = "node-id")]
    node_id: String,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long)]
    status: String,
    #[arg(long = "accepted-risk")]
    accepted_risk: Option<String>,
}

#[derive(Args, Debug)]
struct GraphEdgeStatusArgs {
    #[arg(long = "edge-id")]
    edge_id: String,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long)]
    status: String,
}

#[derive(Args, Debug)]
struct GraphEdgeReviseArgs {
    #[arg(long = "edge-id")]
    edge_id: String,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long = "payload-json")]
    payload_json: Option<String>,
    #[arg(long = "payload-file")]
    payload_file: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct GraphRunnableArgs {
    #[arg(long)]
    limit: Option<usize>,
}

#[derive(Args, Debug)]
struct BulkTransitionArgs {
    #[arg(long = "node-ids", value_delimiter = ',')]
    node_ids: Option<Vec<String>>,
    #[arg(long)]
    filter: Option<String>,
    #[arg(long)]
    correlation_id: Option<String>,
    #[arg(long)]
    status: String,
}

#[derive(Args, Debug)]
struct TemplateListArgs {}

#[derive(Args, Debug)]
struct TemplateRenderArgs {
    #[arg(long)]
    template: String,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct IntentExpandArgs {
    #[arg(long)]
    file: PathBuf,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct ProjectRenderArgs {
    #[arg(long = "entity-type", value_enum)]
    entity_type: EntityType,
    #[arg(long = "entity-id")]
    entity_id: String,
    #[arg(long = "projection-format", value_enum, default_value_t = ProjectionFormat::Markdown)]
    projection_format: ProjectionFormat,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Clone, Debug)]
struct MachineContext {
    format: OutputFormat,
    non_interactive: bool,
    correlation_id: Option<String>,
    scope_key: String,
    db_path: PathBuf,
    command: Vec<String>,
    compact: bool,
}

pub fn run_from_env() -> ExitCode {
    match run_from(std::env::args_os()) {
        Ok(code) => code,
        Err(error) => {
            if let Some(context) = CLI_MACHINE_CONTEXT.get() {
                if context.format == OutputFormat::Json
                    && print_json(&MachineEnvelope::<serde_json::Value>::error(
                        context.correlation_id.clone(),
                        context.non_interactive,
                        context.command.clone(),
                        match error.status() {
                            "invalid" => MachineStatus::Invalid,
                            _ => MachineStatus::Error,
                        },
                        error.to_string(),
                    ))
                    .is_ok()
                {
                    return error.exit_code();
                }
            }

            eprintln!("{error}");
            error.exit_code()
        }
    }
}

fn run_from<I, T>(args: I) -> Result<ExitCode, CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let raw_args = args.into_iter().map(Into::into).collect::<Vec<OsString>>();
    let cli = match Cli::try_parse_from(raw_args.clone()) {
        Ok(cli) => cli,
        Err(error) => return handle_parse_error(error, &raw_args),
    };
    let context = MachineContext {
        format: resolve_output_format(cli.json, cli.format),
        non_interactive: cli.non_interactive,
        correlation_id: cli.correlation_id,
        scope_key: cli.scope,
        db_path: cli.db.unwrap_or_else(default_db_path),
        command: command_path(&cli.command),
        compact: cli.compact,
    };
    let _ = CLI_MACHINE_CONTEXT.set(context.clone());
    if matches!(&cli.command, Command::Capabilities) {
        return execute_capabilities(&context);
    }
    let store = PlanningStore::new(&context.db_path);
    store.init()?;

    // Phase 3: Scope gate for machine-mode mutations
    if context.format == OutputFormat::Json
        && context.non_interactive
        && !is_command_exempt_from_scope_gate(&cli.command)
        && is_command_mutation(&cli.command)
    {
        let scope = &context.scope_key;
        let has_active_session = crate::session::read_session_file()
            .unwrap_or(None)
            .is_some();
        if scope == "default" && !has_active_session {
            let error = serde_json::json!({
                "status": "invalid",
                "code": "SCOPE_REQUIRED",
                "message": "Machine-mode mutations require an explicit --scope or an active session. Use `--scope <key>` or run `elegy-planning session init --scope <key>` first.",
                "scope": scope,
                "hasActiveSession": false,
            });
            return Err(CliError::Store(PlanningStoreError::InvalidInput(
                serde_json::to_string_pretty(&error).unwrap_or_else(|_| {
                    "Machine-mode mutations require an explicit --scope or an active session. Use `--scope <key>` or run `elegy-planning session init --scope <key>` first.".to_string()
                }),
            )));
        }
    }

    match cli.command {
        Command::Scope { command } => execute_scope(command, &store, &context),
        Command::Goal { command } => execute_goal(command, &store, &context),
        Command::Roadmap { command } => execute_roadmap(command, &store, &context),
        Command::WorkPoint { command } => execute_work_point(command, &store, &context),
        Command::Plan { command } => execute_plan(command, &store, &context),
        Command::Todo { command } => execute_todo(command, &store, &context),
        Command::Issue { command } => execute_issue(command, &store, &context),
        Command::ReviewPoint { command } => execute_review_point(command, &store, &context),
        Command::Validate { command } => execute_validate(command, &store, &context),
        Command::Events => execute_events(&store, &context),
        Command::Health => execute_health(&store, &context),
        Command::Capabilities => {
            unreachable!("capabilities returns before database initialization")
        }
        Command::Project { command } => execute_project(command, &store, &context),
        Command::Session { command } => execute_session(command, &store, &context),
        Command::Search(args) => execute_search(args, &store, &context),
        Command::Insight { command } => execute_insight(command, &store, &context),
        Command::Context(args) => execute_context(args, &store, &context),
        Command::Tags(args) => execute_tags(args, &store, &context),
        Command::ProjectRun { command } => execute_project_run(command, &store, &context),
        Command::Worktree { command } => execute_worktree(command, &store, &context),
        Command::Graph { command } => execute_graph(command, &store, &context),
        Command::Manifest(args) => execute_manifest(args, &store, &context),
        Command::Diff(args) => execute_diff(args, &store, &context),
        Command::Template { command } => execute_template(command, &store, &context),
        Command::Intent(args) => execute_intent(args, &store, &context),
    }
}

fn execute_project_run(
    command: ProjectRunCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        ProjectRunCommand::Claim(args) => emit_success(
            context,
            vec!["project-run", "claim"],
            store.claim_project_run(ClaimProjectRunInput {
                id: args.id,
                scope_key: Some(context.scope_key.clone()),
                goal_id: args.goal_id,
                roadmap_id: args.roadmap_id,
                work_point_id: args.work_point_id,
                repo_id: args.repo_id,
                branch: args.branch,
                worktree_id: args.worktree_id,
                session_id: args.session_id,
                run_id: context.correlation_id.clone(),
                profile_id: args.profile_id,
                correlation_id: args.correlation_id,
                owner_id: args.owner_id,
                idempotency_key: args.idempotency_key,
                lease_seconds: Some(args.lease_seconds),
            })?,
        ),
        ProjectRunCommand::Activate(args) => emit_success(
            context,
            vec!["project-run", "activate"],
            store.activate_project_run(ActivateProjectRunInput {
                project_run_id: args.project_run_id,
                active_scope_key: Some(context.scope_key.clone()),
                run_id: context.correlation_id.clone(),
                fencing_token: args.fencing_token,
            })?,
        ),
        ProjectRunCommand::Heartbeat(args) => emit_success(
            context,
            vec!["project-run", "heartbeat"],
            store.heartbeat_project_run(crate::HeartbeatProjectRunInput {
                project_run_id: args.project_run_id,
                active_scope_key: Some(context.scope_key.clone()),
                run_id: context.correlation_id.clone(),
                fencing_token: args.fencing_token,
                lease_seconds: Some(args.lease_seconds),
            })?,
        ),
        ProjectRunCommand::Release(args) => {
            let evidence = match args.evidence_json {
                Some(json_str) => Some(serde_json::from_str::<ProjectRunEvidence>(&json_str)?),
                None => None,
            };
            emit_success(
                context,
                vec!["project-run", "release"],
                store.release_project_run(ReleaseProjectRunInput {
                    project_run_id: args.project_run_id,
                    status: args.status,
                    evidence,
                    active_scope_key: Some(context.scope_key.clone()),
                    run_id: context.correlation_id.clone(),
                    fencing_token: args.fencing_token,
                })?,
            )
        }
        ProjectRunCommand::AddEvidence(args) => {
            let evidence: ProjectRunEvidence = serde_json::from_str(&args.evidence_json)?;
            emit_success(
                context,
                vec!["project-run", "add-evidence"],
                store.add_project_run_evidence(AddEvidenceInput {
                    project_run_id: args.project_run_id,
                    evidence,
                    active_scope_key: Some(context.scope_key.clone()),
                    run_id: context.correlation_id.clone(),
                    fencing_token: args.fencing_token,
                })?,
            )
        }
        ProjectRunCommand::List => emit_success(
            context,
            vec!["project-run", "list"],
            json!({ "projectRuns": store.list_project_runs_in_scope(&context.scope_key)? }),
        ),
        ProjectRunCommand::Show(args) => {
            let view = store.project_run(&args.project_run_id)?;
            if view.project_run.scope_key != context.scope_key {
                return emit_error(
                    context,
                    vec!["project-run", "show"],
                    format!(
                        "project run `{}` is in scope `{}`, not `{}`",
                        args.project_run_id, view.project_run.scope_key, context.scope_key
                    ),
                    true,
                );
            }
            emit_success(context, vec!["project-run", "show"], view)
        }
    }
}

fn handle_parse_error(error: clap::Error, raw_args: &[OsString]) -> Result<ExitCode, CliError> {
    let format = resolve_parse_error_format(raw_args);
    let non_interactive = parse_flag_value(raw_args, "--non-interactive");
    let correlation_id = parse_flag_argument_value(raw_args, "--correlation-id");
    let command = parse_command_path(raw_args);

    if format == OutputFormat::Json
        && !matches!(
            error.kind(),
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
        )
    {
        print_json(&MachineEnvelope::<serde_json::Value>::error(
            correlation_id,
            non_interactive,
            command,
            MachineStatus::Invalid,
            error.to_string(),
        ))?;
        return Ok(exit_invalid());
    }

    error.print().map_err(serde_json::Error::io)?;
    Ok(match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => ExitCode::SUCCESS,
        _ => exit_invalid(),
    })
}

impl CliError {
    fn status(&self) -> &'static str {
        match self {
            CliError::Store(error) if error.is_invalid_input() => "invalid",
            CliError::Store(_) | CliError::Json(_) => "error",
        }
    }

    fn exit_code(&self) -> ExitCode {
        match self {
            CliError::Store(error) if error.is_invalid_input() => exit_invalid(),
            CliError::Store(_) | CliError::Json(_) => exit_runtime(),
        }
    }
}

fn execute_scope(
    command: ScopeCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        ScopeCommand::Create(args) => {
            if args.metadata_json.is_some() && args.metadata_file.is_some() {
                return emit_error(
                    context,
                    vec!["scope", "create"],
                    "--metadata-json and --metadata-file are mutually exclusive".to_string(),
                    true,
                );
            }

            let metadata = if let Some(ref path) = args.metadata_file {
                let content = match std::fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(e) => {
                        return emit_error(
                            context,
                            vec!["scope", "create"],
                            format!("failed to read metadata file `{}`: {e}", path.display()),
                            true,
                        );
                    }
                };
                let parsed: serde_json::Value = match serde_json::from_str(&content) {
                    Ok(v) => v,
                    Err(e) => {
                        return emit_error(
                            context,
                            vec!["scope", "create"],
                            format!("invalid JSON in metadata file `{}`: {e}", path.display()),
                            true,
                        );
                    }
                };
                if !parsed.is_object() {
                    return emit_error(
                        context,
                        vec!["scope", "create"],
                        format!(
                            "metadata file `{}` must contain a JSON object",
                            path.display()
                        ),
                        true,
                    );
                }
                Some(parsed)
            } else {
                parse_optional_json_object(args.metadata_json)?
            };

            emit_success(
                context,
                vec!["scope", "create"],
                store.create_scope(CreateScopeInput {
                    scope_key: args.scope_key,
                    scope_type: args.scope_type,
                    parent_scope_key: args.parent_scope_key,
                    metadata,
                    tags: args.tags,
                    run_id: context.correlation_id.clone(),
                })?,
            )
        }
        ScopeCommand::List => emit_success(
            context,
            vec!["scope", "list"],
            json!({ "scopes": store.list_scopes()? }),
        ),
        ScopeCommand::Show(args) => emit_success(
            context,
            vec!["scope", "show"],
            json!({ "scope": store.scope(&args.scope_key)? }),
        ),
    }
}

fn execute_goal(
    command: GoalCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        GoalCommand::Create(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => return emit_error(context, vec!["goal", "create"], message, true),
            };
            let result = store.create_goal(CreateGoalInput {
                id: args.id,
                scope_key: Some(context.scope_key.clone()),
                correlation_id,
                title: args.title,
                description: args.description,
                acceptance_criteria: args.acceptance_criteria,
                rejection_criteria: args.rejection_criteria,
                status: args.status,
                tags: args.tags,
                run_id: context.correlation_id.clone(),
            })?;
            emit_success(context, vec!["goal", "create"], result)
        }
        GoalCommand::UpdateStatus(args) => emit_success(
            context,
            vec!["goal", "update-status"],
            store.update_status(UpdateStatusInput {
                entity_type: EntityType::Goal,
                entity_id: args.goal_id,
                status: args.status.as_str().to_string(),
                evidence_refs: None,
                active_scope_key: Some(context.scope_key.clone()),
                run_id: context.correlation_id.clone(),
                override_transition: args.override_transition,
                reason: args.reason,
            })?,
        ),
        GoalCommand::List => emit_success(
            context,
            vec!["goal", "list"],
            json!({ "goals": store.list_goals_in_scope(&context.scope_key)? }),
        ),
        GoalCommand::Show(args) => {
            let view = store.goal(&args.goal_id)?;
            if view.goal.scope_key != context.scope_key {
                return emit_error(
                    context,
                    vec!["goal", "show"],
                    format!(
                        "goal `{}` is in scope `{}`, not `{}`",
                        args.goal_id, view.goal.scope_key, context.scope_key
                    ),
                    true,
                );
            }
            emit_success(context, vec!["goal", "show"], view)
        }
        GoalCommand::Search(args) => execute_entity_search(args, store, context, "goal"),
    }
}

fn execute_roadmap(
    command: RoadmapCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        RoadmapCommand::Create(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => {
                    return emit_error(context, vec!["roadmap", "create"], message, true)
                }
            };
            emit_success(
                context,
                vec!["roadmap", "create"],
                store.create_roadmap(CreateRoadmapInput {
                    id: args.id,
                    scope_key: Some(context.scope_key.clone()),
                    goal_id: args.goal_id,
                    correlation_id,
                    title: args.title,
                    summary: args.summary,
                    status: args.status,
                    tags: args.tags,
                    run_id: context.correlation_id.clone(),
                })?,
            )
        }
        RoadmapCommand::UpdateStatus(args) => emit_success(
            context,
            vec!["roadmap", "update-status"],
            store.update_status(UpdateStatusInput {
                entity_type: EntityType::Roadmap,
                entity_id: args.roadmap_id,
                status: args.status.as_str().to_string(),
                evidence_refs: None,
                active_scope_key: Some(context.scope_key.clone()),
                run_id: context.correlation_id.clone(),
                override_transition: args.override_transition,
                reason: args.reason,
            })?,
        ),
        RoadmapCommand::AddSection(args) => emit_success(
            context,
            vec!["roadmap", "add-section"],
            store.add_roadmap_section(AddRoadmapSectionInput {
                id: args.id,
                scope_key: Some(context.scope_key.clone()),
                roadmap_id: args.roadmap_id,
                slug: args.slug,
                title: args.title,
                summary: args.summary,
                ordering: args.ordering,
                run_id: context.correlation_id.clone(),
            })?,
        ),
        RoadmapCommand::AddWorkPoint(args) => emit_success(
            context,
            vec!["roadmap", "add-work-point"],
            store.add_work_point(AddWorkPointInput {
                id: args.id,
                scope_key: Some(context.scope_key.clone()),
                roadmap_id: args.roadmap_id,
                section_id: args.section_id,
                title: args.title,
                summary: args.summary,
                status: args.status,
                ordering: args.ordering,
                dependency_ids: args.dependency_ids,
                validation_expectations: args.validation_expectations,
                effort_tier: args.effort_tier,
                kind: args.kind,
                priority: args.priority,
                repairs_work_point_ids: args.repairs_work_point_ids,
                supersedes_work_point_ids: args.supersedes_work_point_ids,
                blocks_work_point_ids: args.blocks_work_point_ids,
                file_scopes: parse_file_scopes(args.file_scopes)?,
                tags: args.tags,
                run_id: context.correlation_id.clone(),
            })?,
        ),
        RoadmapCommand::List => emit_success(
            context,
            vec!["roadmap", "list"],
            json!({ "roadmaps": store.list_roadmaps_in_scope(&context.scope_key)? }),
        ),
        RoadmapCommand::Show(args) => {
            let view = store.roadmap(&args.roadmap_id)?;
            if view.roadmap.scope_key != context.scope_key {
                return emit_error(
                    context,
                    vec!["roadmap", "show"],
                    format!(
                        "roadmap `{}` is in scope `{}`, not `{}`",
                        args.roadmap_id, view.roadmap.scope_key, context.scope_key
                    ),
                    true,
                );
            }
            emit_success(context, vec!["roadmap", "show"], view)
        }
        RoadmapCommand::Search(args) => execute_entity_search(args, store, context, "roadmap"),
    }
}

fn execute_work_point(
    command: WorkPointCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        WorkPointCommand::List => emit_success(
            context,
            vec!["work-point", "list"],
            json!({ "workPoints": store.list_work_points_in_scope(&context.scope_key)? }),
        ),
        WorkPointCommand::Show(args) => {
            let view = store.work_point(&args.work_point_id)?;
            if view.work_point.scope_key != context.scope_key {
                return emit_error(
                    context,
                    vec!["work-point", "show"],
                    format!(
                        "work point `{}` is in scope `{}`, not `{}`",
                        args.work_point_id, view.work_point.scope_key, context.scope_key
                    ),
                    true,
                );
            }
            emit_success(context, vec!["work-point", "show"], view)
        }
        WorkPointCommand::UpdateStatus(args) => emit_success(
            context,
            vec!["work-point", "update-status"],
            store.update_status(UpdateStatusInput {
                entity_type: EntityType::WorkPoint,
                entity_id: args.work_point_id,
                status: args.status.as_str().to_string(),
                evidence_refs: None,
                active_scope_key: Some(context.scope_key.clone()),
                run_id: context.correlation_id.clone(),
                override_transition: args.override_transition,
                reason: args.reason,
            })?,
        ),
        WorkPointCommand::NextRunnable(args) => {
            let _ = store.validate_all()?;
            emit_success(
                context,
                vec!["work-point", "next-runnable"],
                store.find_runnable_work_points(&args.roadmap_id)?,
            )
        }
        WorkPointCommand::WorkGraph(args) => {
            let _ = store.validate_all()?;
            emit_success(
                context,
                vec!["work-point", "work-graph"],
                store.build_work_graph(&args.roadmap_id)?,
            )
        }
        WorkPointCommand::Revise(args) => {
            if args.clear_dependencies && !args.dependency_ids.is_empty() {
                return emit_error(
                    context,
                    vec!["work-point", "revise"],
                    "--clear-dependencies cannot be combined with --dependency-id".to_string(),
                    true,
                );
            }
            emit_success(
                context,
                vec!["work-point", "revise"],
                store.revise_work_point(ReviseWorkPointInput {
                    work_point_id: args.work_point_id,
                    active_scope_key: Some(context.scope_key.clone()),
                    dependency_ids: if !args.dependency_ids.is_empty() {
                        Some(args.dependency_ids)
                    } else {
                        None
                    },
                    clear_dependencies: args.clear_dependencies,
                    blocks_work_point_ids: if !args.blocks_work_point_ids.is_empty() {
                        Some(args.blocks_work_point_ids)
                    } else {
                        None
                    },
                    clear_blocks: args.clear_blocks,
                    run_id: context.correlation_id.clone(),
                })?,
            )
        }
    }
}

fn execute_plan(
    command: PlanCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        PlanCommand::Create(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => return emit_error(context, vec!["plan", "create"], message, true),
            };
            emit_success(
                context,
                vec!["plan", "create"],
                store.create_plan(CreatePlanInput {
                    id: args.id,
                    scope_key: Some(context.scope_key.clone()),
                    goal_id: args.goal_id,
                    roadmap_id: args.roadmap_id,
                    correlation_id,
                    title: args.title,
                    summary: args.summary,
                    scope: args.plan_scope,
                    assumptions: args.assumptions,
                    stop_conditions: args.stop_conditions,
                    validation_steps: args.validation_steps,
                    targeted_work_point_ids: args.targeted_work_point_ids,
                    effort_tier: args.effort_tier,
                    routing_hint: args.routing_hint,
                    allow_parallel_overlap: args.allow_parallel_overlap,
                    file_scopes: parse_file_scopes(args.file_scopes)?,
                    status: args.status,
                    tags: args.tags,
                    run_id: context.correlation_id.clone(),
                })?,
            )
        }
        PlanCommand::Revise(args) => {
            if args.clear_routing_hint && args.routing_hint.is_some() {
                return emit_error(
                    context,
                    vec!["plan", "revise"],
                    "--clear-routing-hint cannot be combined with --routing-hint".to_string(),
                    true,
                );
            }
            if args.clear_file_scopes && !args.file_scopes.is_empty() {
                return emit_error(
                    context,
                    vec!["plan", "revise"],
                    "--clear-file-scopes cannot be combined with --file-scope".to_string(),
                    true,
                );
            }
            emit_success(
                context,
                vec!["plan", "revise"],
                store.revise_plan(RevisePlanInput {
                    plan_id: args.plan_id,
                    active_scope_key: Some(context.scope_key.clone()),
                    scope_key: args.scope_key,
                    assumptions: optional_vec(args.assumptions),
                    stop_conditions: optional_vec(args.stop_conditions),
                    validation_steps: optional_vec(args.validation_steps),
                    targeted_work_point_ids: optional_vec(args.targeted_work_point_ids),
                    effort_tier: args.effort_tier,
                    routing_hint: args.routing_hint,
                    clear_routing_hint: args.clear_routing_hint,
                    allow_parallel_overlap: args.allow_parallel_overlap,
                    file_scopes: optional_file_scopes(args.file_scopes)?,
                    clear_file_scopes: args.clear_file_scopes,
                    tags: optional_vec(args.tags),
                    run_id: context.correlation_id.clone(),
                })?,
            )
        }
        PlanCommand::UpdateStatus(args) => emit_success(
            context,
            vec!["plan", "update-status"],
            store.update_status(UpdateStatusInput {
                entity_type: EntityType::Plan,
                entity_id: args.plan_id,
                status: args.status.as_str().to_string(),
                evidence_refs: None,
                active_scope_key: Some(context.scope_key.clone()),
                run_id: context.correlation_id.clone(),
                override_transition: args.override_transition,
                reason: args.reason,
            })?,
        ),
        PlanCommand::List => emit_success(
            context,
            vec!["plan", "list"],
            json!({ "plans": store.list_plans_in_scope(&context.scope_key)? }),
        ),
        PlanCommand::Show(args) => {
            let view = store.plan(&args.plan_id)?;
            if view.plan.scope_key != context.scope_key {
                return emit_error(
                    context,
                    vec!["plan", "show"],
                    format!(
                        "plan `{}` is in scope `{}`, not `{}`",
                        args.plan_id, view.plan.scope_key, context.scope_key
                    ),
                    true,
                );
            }
            emit_success(context, vec!["plan", "show"], view)
        }
        PlanCommand::Search(args) => execute_entity_search(args, store, context, "plan"),
    }
}

fn execute_todo(
    command: TodoCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        TodoCommand::Create(args) => emit_success(
            context,
            vec!["todo", "create"],
            store.create_todo(CreateTodoInput {
                id: args.id,
                scope_key: Some(context.scope_key.clone()),
                plan_id: args.plan_id,
                work_point_id: args.work_point_id,
                title: args.title,
                summary: args.summary,
                status: args.status,
                priority: args.priority,
                effort_tier: args.effort_tier,
                file_scopes: parse_file_scopes(args.file_scopes)?,
                evidence_refs: args.evidence_refs,
                tags: args.tags,
                ordering: args.ordering,
                run_id: context.correlation_id.clone(),
            })?,
        ),
        TodoCommand::UpdateStatus(args) => emit_success(
            context,
            vec!["todo", "update-status"],
            store.update_status(UpdateStatusInput {
                entity_type: EntityType::Todo,
                entity_id: args.todo_id,
                status: args.status.as_str().to_string(),
                evidence_refs: optional_vec(args.evidence_refs),
                active_scope_key: Some(context.scope_key.clone()),
                run_id: context.correlation_id.clone(),
                override_transition: args.override_transition,
                reason: args.reason,
            })?,
        ),
        TodoCommand::List => emit_success(
            context,
            vec!["todo", "list"],
            json!({ "todos": store.list_todos_in_scope(&context.scope_key)? }),
        ),
        TodoCommand::Search(args) => execute_entity_search(args, store, context, "todo"),
    }
}

fn execute_issue(
    command: IssueCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        IssueCommand::Record(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => return emit_error(context, vec!["issue", "record"], message, true),
            };
            emit_success(
                context,
                vec!["issue", "record"],
                store.create_issue(CreateIssueInput {
                    id: args.id,
                    scope_key: Some(context.scope_key.clone()),
                    correlation_id,
                    title: args.title,
                    summary: args.summary,
                    status: args.status,
                    severity: args.severity,
                    related_entity_type: args.related_entity_type,
                    related_entity_id: args.related_entity_id,
                    tags: args.tags,
                    run_id: context.correlation_id.clone(),
                })?,
            )
        }
        IssueCommand::UpdateStatus(args) => emit_success(
            context,
            vec!["issue", "update-status"],
            store.update_status(UpdateStatusInput {
                entity_type: EntityType::Issue,
                entity_id: args.issue_id,
                status: args.status.as_str().to_string(),
                evidence_refs: None,
                active_scope_key: Some(context.scope_key.clone()),
                run_id: context.correlation_id.clone(),
                override_transition: args.override_transition,
                reason: args.reason,
            })?,
        ),
        IssueCommand::List => emit_success(
            context,
            vec!["issue", "list"],
            json!({ "issues": store.list_issues_in_scope(&context.scope_key)? }),
        ),
        IssueCommand::Show(args) => {
            let view = store.issue(&args.issue_id)?;
            if view.issue.scope_key != context.scope_key {
                return emit_error(
                    context,
                    vec!["issue", "show"],
                    format!(
                        "issue `{}` is in scope `{}`, not `{}`",
                        args.issue_id, view.issue.scope_key, context.scope_key
                    ),
                    true,
                );
            }
            emit_success(context, vec!["issue", "show"], view)
        }
        IssueCommand::Search(args) => execute_entity_search(args, store, context, "issue"),
    }
}

fn execute_review_point(
    command: ReviewPointCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        ReviewPointCommand::Record(args) => emit_success(
            context,
            vec!["review-point", "record"],
            store.create_review_point(CreateReviewPointInput {
                id: args.id,
                scope_key: Some(context.scope_key.clone()),
                attached_entity_type: args.attached_entity_type,
                attached_entity_id: args.attached_entity_id,
                title: args.title,
                summary: args.summary,
                status: args.status,
                severity: args.severity,
                run_id: context.correlation_id.clone(),
            })?,
        ),
        ReviewPointCommand::UpdateStatus(args) => emit_success(
            context,
            vec!["review-point", "update-status"],
            store.update_status(UpdateStatusInput {
                entity_type: EntityType::ReviewPoint,
                entity_id: args.review_point_id,
                status: args.status.as_str().to_string(),
                evidence_refs: None,
                active_scope_key: Some(context.scope_key.clone()),
                run_id: context.correlation_id.clone(),
                override_transition: args.override_transition,
                reason: args.reason,
            })?,
        ),
    }
}

fn execute_validate(
    command: ValidateCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        ValidateCommand::All(args) => {
            if args.all_scopes {
                emit_success(context, vec!["validate", "all"], store.validate_all()?)
            } else {
                emit_success(
                    context,
                    vec!["validate", "all"],
                    store.validate_all_in_scope(&context.scope_key)?,
                )
            }
        }
    }
}

fn execute_events(store: &PlanningStore, context: &MachineContext) -> Result<ExitCode, CliError> {
    emit_success(
        context,
        vec!["events", "list"],
        json!({ "events": store.list_events_in_scope(&context.scope_key)? }),
    )
}

fn execute_health(store: &PlanningStore, context: &MachineContext) -> Result<ExitCode, CliError> {
    emit_success(context, vec!["health"], store.health()?)
}

fn execute_capabilities(context: &MachineContext) -> Result<ExitCode, CliError> {
    emit_success(
        context,
        vec!["capabilities"],
        json!({
            "cliVersion": env!("CARGO_PKG_VERSION"),
            "resultSchemaVersion": RESULT_SCHEMA_VERSION,
            "planningSchemaVersion": CURRENT_SCHEMA_VERSION,
            "capabilities": [
                "project-run.claim.v2",
                "project-run.activate.fenced.v1",
                "project-run.heartbeat.v1",
                "project-run.release.fenced.v1",
                "project-run.add-evidence.fenced.v1"
            ]
        }),
    )
}

fn execute_project(
    command: ProjectCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        ProjectCommand::Export(args) => emit_success(
            context,
            vec!["project", "export"],
            store.render_projection_in_scope(
                &context.scope_key,
                args.entity_type,
                &args.entity_id,
                args.projection_format,
                &args.output,
            )?,
        ),
        ProjectCommand::Render(args) => emit_success(
            context,
            vec!["project", "render"],
            store.render_projection_in_scope(
                &context.scope_key,
                args.entity_type,
                &args.entity_id,
                args.projection_format,
                &args.output,
            )?,
        ),
    }
}

fn execute_session(
    command: SessionCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        SessionCommand::Init(args) => {
            let session = crate::session::init_session(&args.scope)?;
            emit_success(context, vec!["session", "init"], session)
        }
        SessionCommand::Use(args) => {
            let session = crate::session::use_session(&args.session_id)?;
            emit_success(context, vec!["session", "use"], session)
        }
        SessionCommand::Show => {
            let session = crate::session::show_session()?;
            emit_success(context, vec!["session", "show"], session)
        }
        SessionCommand::Resume(args) => {
            if let Some(ref sid) = args.session_id {
                let session = crate::session::update_session_file(sid, &context.scope_key)?;
                let summary = serde_json::json!({
                    "sessionId": session.session_id,
                    "scope": session.scope,
                    "action": "resumed-specific",
                    "message": format!("Resumed session {}", session.session_id)
                });
                emit_success(context, vec!["session", "resume"], summary)
            } else {
                match crate::session::read_session_file()? {
                    Some(session) => {
                        let active_runs =
                            store.count_active_runs_for_session(&session.session_id)?;
                        let summary = serde_json::json!({
                            "sessionId": session.session_id,
                            "scope": session.scope,
                            "action": "resumed-current",
                            "message": format!(
                                "Current session: {} (created: {}, active project runs: {})",
                                session.session_id, session.created_at, active_runs
                            )
                        });
                        emit_success(context, vec!["session", "resume"], summary)
                    }
                    None => emit_error(
                        context,
                        vec!["session", "resume"],
                        "No active session found. Use 'session init' to create one.".to_string(),
                        true,
                    ),
                }
            }
        }
        SessionCommand::List(args) => {
            let sessions = store.list_sessions(args.limit)?;
            emit_success(
                context,
                vec!["session", "list"],
                serde_json::json!({ "sessions": sessions }),
            )
        }
    }
}

fn execute_worktree(
    command: WorktreeCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        WorktreeCommand::List(args) => {
            let status = args.status.as_deref();
            let worktrees = store.list_worktrees(&context.scope_key, status)?;
            emit_success(
                context,
                vec!["worktree", "list"],
                serde_json::json!({ "worktrees": worktrees }),
            )
        }
        WorktreeCommand::Show(args) => {
            let worktree = store.get_worktree(&args.id, &context.scope_key)?;
            emit_success(context, vec!["worktree", "show"], worktree)
        }
        WorktreeCommand::Attach(args) => {
            let input = AttachWorktreeInput {
                id: args.id,
                scope_key: Some(context.scope_key.clone()),
                repo_uri: args.repo_uri,
                branch: args.branch,
                worktree_path: args.worktree_path,
                project_run_id: args.project_run_id,
                session_id: args.session_id,
                correlation_id: args.correlation_id,
            };
            let worktree = store.attach_worktree(input)?;
            emit_success(context, vec!["worktree", "attach"], worktree)
        }
        WorktreeCommand::Archive(args) => {
            let worktree = store.update_worktree_status(
                &args.id,
                &context.scope_key,
                WorktreeStatus::Archived,
            )?;
            emit_success(context, vec!["worktree", "archive"], worktree)
        }
        WorktreeCommand::CleanupIntent(args) => {
            let worktree = store.update_worktree_status(
                &args.id,
                &context.scope_key,
                WorktreeStatus::CleanupIntent,
            )?;
            emit_success(context, vec!["worktree", "cleanup-intent"], worktree)
        }
    }
}

fn execute_graph(
    command: GraphCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        GraphCommand::Node { command } => execute_graph_node(command, store, context),
        GraphCommand::Edge { command } => execute_graph_edge(command, store, context),
        GraphCommand::Acceptance { command } => execute_graph_acceptance(command, store, context),
        GraphCommand::Evidence { command } => execute_graph_evidence(command, store, context),
        GraphCommand::Runnable(args) => execute_graph_runnable(args, store, context),
        GraphCommand::Bulk(args) => execute_graph_bulk(args, store, context),
    }
}

fn execute_graph_node(
    command: GraphNodeCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        GraphNodeCommand::Create(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => {
                    return emit_error(context, vec!["graph", "node", "create"], message, true)
                }
            };
            let payload = parse_graph_payload(&args.payload_json, &args.payload_file)?;
            let result = store.create_graph_node(CreateGraphNodeInput {
                id: args.id,
                scope_key: Some(context.scope_key.clone()),
                correlation_id,
                kind: args.kind,
                title: args.title,
                summary: args.summary,
                status: args.status,
                payload,
                tags: args.tags,
                run_id: context.correlation_id.clone(),
            })?;
            emit_success(context, vec!["graph", "node", "create"], result)
        }
        GraphNodeCommand::Show(args) => {
            // Pre-check scope before building the full view
            let node = store.graph_node(&args.node_id)?;
            if node.scope_key != context.scope_key {
                return emit_error(
                    context,
                    vec!["graph", "node", "show"],
                    format!(
                        "graph node `{}` is in scope `{}`, not `{}`",
                        args.node_id, node.scope_key, context.scope_key
                    ),
                    true,
                );
            }
            if context.compact {
                let compact = CompactGraphNode {
                    id: node.id,
                    kind: node.kind,
                    title: node.title,
                    status: node.status,
                };
                emit_success(context, vec!["graph", "node", "show"], compact)
            } else {
                let view = store.graph_node_view(&args.node_id, &context.scope_key)?;
                emit_success(context, vec!["graph", "node", "show"], view)
            }
        }
        GraphNodeCommand::List(args) => {
            let nodes = store.list_graph_nodes(&context.scope_key, args.kind)?;
            let nodes = if let Some(limit) = args.limit {
                nodes.into_iter().take(limit).collect::<Vec<_>>()
            } else {
                nodes
            };
            if context.compact {
                let compact: Vec<CompactGraphNode> = nodes
                    .into_iter()
                    .map(|n| CompactGraphNode {
                        id: n.id,
                        kind: n.kind,
                        title: n.title,
                        status: n.status,
                    })
                    .collect();
                emit_success(
                    context,
                    vec!["graph", "node", "list"],
                    json!({ "nodes": compact }),
                )
            } else {
                emit_success(
                    context,
                    vec!["graph", "node", "list"],
                    json!({ "nodes": nodes }),
                )
            }
        }
        GraphNodeCommand::Status(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => {
                    return emit_error(context, vec!["graph", "node", "status"], message, true)
                }
            };
            let result = store.update_graph_node_status(UpdateGraphNodeStatusInput {
                node_id: args.node_id,
                correlation_id,
                active_scope_key: Some(context.scope_key.clone()),
                status: args.status,
                run_id: context.correlation_id.clone(),
            })?;
            emit_success(context, vec!["graph", "node", "status"], result)
        }
        GraphNodeCommand::Revise(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => {
                    return emit_error(context, vec!["graph", "node", "revise"], message, true)
                }
            };
            let payload = parse_graph_payload(&args.payload_json, &args.payload_file)?;
            let result = store.revise_graph_node(ReviseGraphNodeInput {
                node_id: args.node_id,
                correlation_id,
                active_scope_key: Some(context.scope_key.clone()),
                title: args.title,
                summary: args.summary,
                status: args.status,
                payload: if args.payload_json.is_some() || args.payload_file.is_some() {
                    Some(payload)
                } else {
                    None
                },
                tags: if args.tags.is_empty() && !args.clear_tags {
                    None
                } else {
                    Some(args.tags)
                },
                clear_tags: args.clear_tags,
                run_id: context.correlation_id.clone(),
            })?;
            emit_success(context, vec!["graph", "node", "revise"], result)
        }
        GraphNodeCommand::Finalize(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => {
                    return emit_error(context, vec!["graph", "node", "finalize"], message, true)
                }
            };
            let result = store.finalize_graph_node(FinalizeGraphNodeInput {
                node_id: args.node_id,
                correlation_id,
                active_scope_key: Some(context.scope_key.clone()),
                status: args.status,
                accepted_risk: args.accepted_risk,
                run_id: context.correlation_id.clone(),
            })?;
            emit_success(context, vec!["graph", "node", "finalize"], result)
        }
    }
}

fn execute_graph_edge(
    command: GraphEdgeCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        GraphEdgeCommand::Create(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => {
                    return emit_error(context, vec!["graph", "edge", "create"], message, true)
                }
            };
            let payload = parse_graph_payload(&args.payload_json, &args.payload_file)?;
            let result = store.create_graph_edge(CreateGraphEdgeInput {
                id: args.id,
                scope_key: Some(context.scope_key.clone()),
                correlation_id,
                kind: args.kind,
                source_node_id: args.source_node_id,
                target_node_id: args.target_node_id,
                status: args.status,
                payload,
                run_id: context.correlation_id.clone(),
            })?;
            emit_success(context, vec!["graph", "edge", "create"], result)
        }
        GraphEdgeCommand::Show(args) => {
            // Pre-check scope before building the full view
            let edge = store.graph_edge(&args.edge_id)?;
            if edge.scope_key != context.scope_key {
                return emit_error(
                    context,
                    vec!["graph", "edge", "show"],
                    format!(
                        "graph edge `{}` is in scope `{}`, not `{}`",
                        args.edge_id, edge.scope_key, context.scope_key
                    ),
                    true,
                );
            }
            if context.compact {
                let compact = CompactGraphEdge {
                    id: edge.id,
                    kind: edge.kind,
                    source_node_id: edge.source_node_id,
                    target_node_id: edge.target_node_id,
                    status: edge.status,
                };
                emit_success(context, vec!["graph", "edge", "show"], compact)
            } else {
                let view = store.graph_edge_view(&args.edge_id, &context.scope_key)?;
                emit_success(context, vec!["graph", "edge", "show"], view)
            }
        }
        GraphEdgeCommand::List(args) => {
            let edges = store.list_graph_edges(&context.scope_key, args.kind)?;
            let edges = if let Some(limit) = args.limit {
                edges.into_iter().take(limit).collect::<Vec<_>>()
            } else {
                edges
            };
            if context.compact {
                let compact: Vec<CompactGraphEdge> = edges
                    .into_iter()
                    .map(|e| CompactGraphEdge {
                        id: e.id,
                        kind: e.kind,
                        source_node_id: e.source_node_id,
                        target_node_id: e.target_node_id,
                        status: e.status,
                    })
                    .collect();
                emit_success(
                    context,
                    vec!["graph", "edge", "list"],
                    json!({ "edges": compact }),
                )
            } else {
                emit_success(
                    context,
                    vec!["graph", "edge", "list"],
                    json!({ "edges": edges }),
                )
            }
        }
        GraphEdgeCommand::Incoming(args) => {
            // Scope gate: verify the referenced node belongs to the active scope
            let node = store.graph_node(&args.node_id)?;
            if node.scope_key != context.scope_key {
                return emit_error(
                    context,
                    vec!["graph", "edge", "incoming"],
                    format!(
                        "graph node `{}` is in scope `{}`, not `{}`",
                        args.node_id, node.scope_key, context.scope_key
                    ),
                    true,
                );
            }
            // Query all edges then filter by active scope (defense-in-depth against SQL corruption)
            let edges: Vec<_> = store
                .list_incoming_edges(&args.node_id, args.kind)?
                .into_iter()
                .filter(|e| e.scope_key == context.scope_key)
                .collect();
            if context.compact {
                let compact: Vec<CompactGraphEdge> = edges
                    .into_iter()
                    .map(|e| CompactGraphEdge {
                        id: e.id,
                        kind: e.kind,
                        source_node_id: e.source_node_id,
                        target_node_id: e.target_node_id,
                        status: e.status,
                    })
                    .collect();
                emit_success(
                    context,
                    vec!["graph", "edge", "incoming"],
                    json!({ "edges": compact }),
                )
            } else {
                emit_success(
                    context,
                    vec!["graph", "edge", "incoming"],
                    json!({ "edges": edges }),
                )
            }
        }
        GraphEdgeCommand::Outgoing(args) => {
            // Scope gate: verify the referenced node belongs to the active scope
            let node = store.graph_node(&args.node_id)?;
            if node.scope_key != context.scope_key {
                return emit_error(
                    context,
                    vec!["graph", "edge", "outgoing"],
                    format!(
                        "graph node `{}` is in scope `{}`, not `{}`",
                        args.node_id, node.scope_key, context.scope_key
                    ),
                    true,
                );
            }
            // Query all edges then filter by active scope (defense-in-depth against SQL corruption)
            let edges: Vec<_> = store
                .list_outgoing_edges(&args.node_id, args.kind)?
                .into_iter()
                .filter(|e| e.scope_key == context.scope_key)
                .collect();
            if context.compact {
                let compact: Vec<CompactGraphEdge> = edges
                    .into_iter()
                    .map(|e| CompactGraphEdge {
                        id: e.id,
                        kind: e.kind,
                        source_node_id: e.source_node_id,
                        target_node_id: e.target_node_id,
                        status: e.status,
                    })
                    .collect();
                emit_success(
                    context,
                    vec!["graph", "edge", "outgoing"],
                    json!({ "edges": compact }),
                )
            } else {
                emit_success(
                    context,
                    vec!["graph", "edge", "outgoing"],
                    json!({ "edges": edges }),
                )
            }
        }
        GraphEdgeCommand::Status(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => {
                    return emit_error(context, vec!["graph", "edge", "status"], message, true)
                }
            };
            let result = store.update_graph_edge_status(UpdateGraphEdgeStatusInput {
                edge_id: args.edge_id,
                correlation_id,
                active_scope_key: Some(context.scope_key.clone()),
                status: args.status,
                run_id: context.correlation_id.clone(),
            })?;
            emit_success(context, vec!["graph", "edge", "status"], result)
        }
        GraphEdgeCommand::Revise(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => {
                    return emit_error(context, vec!["graph", "edge", "revise"], message, true)
                }
            };
            let payload = parse_graph_payload(&args.payload_json, &args.payload_file)?;
            let result = store.revise_graph_edge(ReviseGraphEdgeInput {
                edge_id: args.edge_id,
                correlation_id,
                active_scope_key: Some(context.scope_key.clone()),
                status: args.status,
                payload: if args.payload_json.is_some() || args.payload_file.is_some() {
                    Some(payload)
                } else {
                    None
                },
                run_id: context.correlation_id.clone(),
            })?;
            emit_success(context, vec!["graph", "edge", "revise"], result)
        }
    }
}

fn execute_graph_acceptance(
    command: AcceptanceCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        AcceptanceCommand::Create(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => {
                    return emit_error(
                        context,
                        vec!["graph", "acceptance", "create"],
                        message,
                        true,
                    )
                }
            };
            let result = store.create_acceptance(CreateAcceptanceInput {
                id: args.id,
                scope_key: Some(context.scope_key.clone()),
                correlation_id,
                title: args.title,
                summary: args.summary,
                status: args.status,
                acceptance_kind: args.acceptance_kind,
                description: args.description,
                verification_policy: args.verification_policy,
                required_evidence_kinds: args.required_evidence_kinds,
                waiver: args.waiver,
                tags: args.tags,
                run_id: context.correlation_id.clone(),
            })?;
            emit_success(context, vec!["graph", "acceptance", "create"], result)
        }
        AcceptanceCommand::Show(args) => {
            if context.compact {
                let node = store.graph_node(&args.node_id)?;
                let compact = CompactGraphNode {
                    id: node.id,
                    kind: node.kind,
                    title: node.title,
                    status: node.status,
                };
                emit_success(context, vec!["graph", "acceptance", "show"], compact)
            } else {
                let view = store.acceptance_view(&args.node_id, &context.scope_key)?;
                emit_success(context, vec!["graph", "acceptance", "show"], view)
            }
        }
        AcceptanceCommand::List(args) => {
            let kind_filter = args.kind;
            let mut nodes =
                store.list_graph_nodes(&context.scope_key, Some(PlanningNodeKind::Acceptance))?;
            if let Some(filter_kind) = &kind_filter {
                nodes.retain(|n| {
                    n.payload
                        .get("acceptanceKind")
                        .and_then(|v| v.as_str())
                        .map(|k| k == filter_kind.as_str())
                        .unwrap_or(false)
                });
            }
            nodes.truncate(args.limit);
            emit_success(
                context,
                vec!["graph", "acceptance", "list"],
                serde_json::json!({ "nodes": nodes }),
            )
        }
        AcceptanceCommand::Satisfy(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => {
                    return emit_error(
                        context,
                        vec!["graph", "acceptance", "satisfy"],
                        message,
                        true,
                    )
                }
            };
            let result = store.satisfy_acceptance(SatisfyAcceptanceInput {
                id: args.id,
                scope_key: Some(context.scope_key.clone()),
                correlation_id,
                concrete_node_id: args.concrete_node_id,
                abstract_node_id: args.abstract_node_id,
                rationale: args.rationale,
                run_id: context.correlation_id.clone(),
            })?;
            emit_success(context, vec!["graph", "acceptance", "satisfy"], result)
        }
    }
}

fn execute_graph_evidence(
    command: EvidenceCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        EvidenceCommand::Create(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => {
                    return emit_error(context, vec!["graph", "evidence", "create"], message, true)
                }
            };
            let result = store.create_evidence(CreateEvidenceInput {
                id: args.id,
                scope_key: Some(context.scope_key.clone()),
                correlation_id,
                title: args.title,
                summary: args.summary,
                status: args.status,
                evidence_kind: args.evidence_kind,
                reference: args.reference,
                content: args.content,
                captured_at: args.captured_at,
                tags: args.tags,
                run_id: context.correlation_id.clone(),
            })?;
            emit_success(context, vec!["graph", "evidence", "create"], result)
        }
        EvidenceCommand::Show(args) => {
            if context.compact {
                let node = store.graph_node(&args.node_id)?;
                let compact = CompactGraphNode {
                    id: node.id,
                    kind: node.kind,
                    title: node.title,
                    status: node.status,
                };
                emit_success(context, vec!["graph", "evidence", "show"], compact)
            } else {
                let view = store.evidence_view(&args.node_id, &context.scope_key)?;
                emit_success(context, vec!["graph", "evidence", "show"], view)
            }
        }
        EvidenceCommand::List(args) => {
            let kind_filter = args.kind;
            let mut nodes =
                store.list_graph_nodes(&context.scope_key, Some(PlanningNodeKind::Evidence))?;
            if let Some(filter_kind) = &kind_filter {
                nodes.retain(|n| {
                    n.payload
                        .get("evidenceKind")
                        .and_then(|v| v.as_str())
                        .map(|k| k == filter_kind.as_str())
                        .unwrap_or(false)
                });
            }
            nodes.truncate(args.limit);
            emit_success(
                context,
                vec!["graph", "evidence", "list"],
                serde_json::json!({ "nodes": nodes }),
            )
        }
        EvidenceCommand::Attach(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => {
                    return emit_error(context, vec!["graph", "evidence", "attach"], message, true)
                }
            };
            let result = store.attach_evidence(AttachEvidenceInput {
                id: args.id,
                scope_key: Some(context.scope_key.clone()),
                correlation_id,
                evidence_node_id: args.evidence_node_id,
                target_node_id: args.target_node_id,
                rationale: args.rationale,
                run_id: context.correlation_id.clone(),
            })?;
            emit_success(context, vec!["graph", "evidence", "attach"], result)
        }
    }
}

fn execute_graph_runnable(
    args: GraphRunnableArgs,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    let mut result = store.find_runnable_graph_work(&context.scope_key)?;
    if let Some(limit) = args.limit {
        result.candidates.truncate(limit);
        result.blocked.truncate(limit);
    }
    emit_success(context, vec!["graph", "runnable"], result)
}

fn execute_graph_bulk(
    args: BulkTransitionArgs,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    let correlation_id = args.correlation_id.unwrap_or_else(|| {
        context
            .correlation_id
            .clone()
            .unwrap_or_else(|| "bulk".to_string())
    });
    let input = crate::BulkTransitionInput {
        scope_key: context.scope_key.clone(),
        node_ids: args.node_ids,
        filter: args.filter,
        status: args.status,
        correlation_id,
        run_id: context.correlation_id.clone(),
    };
    let result = store.bulk_update_graph_node_status(&input)?;
    emit_success(context, vec!["graph", "bulk"], result)
}

fn execute_template(
    command: TemplateCommand,
    _store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        TemplateCommand::List(_) => {
            let templates = crate::template::list_templates()?;
            emit_success(
                context,
                vec!["template", "list"],
                serde_json::json!({ "templates": templates }),
            )
        }
        TemplateCommand::Render(args) => {
            let content = crate::template::render_template(&args.template)?;
            if let Some(output) = &args.output {
                std::fs::write(output, &content).map_err(|e| {
                    CliError::Store(PlanningStoreError::InvalidInput(format!(
                        "failed to write template to {}: {e}",
                        output.display()
                    )))
                })?;
                let result = serde_json::json!({
                    "template": args.template,
                    "output": output.to_string_lossy(),
                });
                emit_success(context, vec!["template", "render"], result)
            } else {
                println!("{content}");
                Ok(ExitCode::SUCCESS)
            }
        }
    }
}

fn execute_intent(
    args: IntentExpandArgs,
    _store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    let content = crate::intent::expand_intent_file(&args.file)?;
    if let Some(output) = &args.output {
        std::fs::write(output, &content).map_err(|e| {
            CliError::Store(PlanningStoreError::InvalidInput(format!(
                "failed to write manifest to {}: {e}",
                output.display()
            )))
        })?;
        let result = serde_json::json!({
            "output": output.to_string_lossy(),
        });
        emit_success(context, vec!["intent", "expand"], result)
    } else {
        println!("{content}");
        Ok(ExitCode::SUCCESS)
    }
}

fn execute_manifest(
    args: ManifestArgs,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    let correlation_id = context
        .correlation_id
        .clone()
        .unwrap_or_else(|| "manifest-apply".to_string());
    let raw = manifest::parse_manifest_file(&args.file)?;
    let parsed = manifest::expand_manifest(raw, &correlation_id);

    let result = store.apply_manifest(&parsed, args.dry_run)?;
    emit_success(context, vec!["manifest", "apply"], result)
}

fn execute_diff(
    args: DiffArgs,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    let raw = manifest::parse_manifest_file(&args.manifest)?;
    let correlation_id = "diff".to_string();
    let parsed = manifest::expand_manifest(raw, &correlation_id);

    let db_nodes = store.load_all_graph_nodes(&context.scope_key)?;
    let db_edges = store.load_all_graph_edges(&context.scope_key)?;

    // Build maps from the database
    let db_node_map: std::collections::HashMap<&str, &PlanningGraphNode> =
        db_nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let _db_edge_map: std::collections::HashMap<&str, &PlanningGraphEdge> =
        db_edges.iter().map(|e| (e.id.as_str(), e)).collect();

    let manifest_node_ids: std::collections::HashSet<&str> = parsed
        .nodes
        .iter()
        .filter_map(|n| n.id.as_deref())
        .collect();
    let manifest_edge_ids: std::collections::HashSet<&str> = parsed
        .edges
        .iter()
        .filter_map(|e| e.id.as_deref())
        .collect();

    let db_node_ids: std::collections::HashSet<&str> =
        db_nodes.iter().map(|n| n.id.as_str()).collect();
    let db_edge_ids: std::collections::HashSet<&str> =
        db_edges.iter().map(|e| e.id.as_str()).collect();

    // Added nodes: in manifest but not in DB
    let added_nodes: Vec<String> = manifest_node_ids
        .difference(&db_node_ids)
        .map(|s| s.to_string())
        .collect();

    // Removed nodes: in DB but not in manifest
    let removed_nodes: Vec<String> = db_node_ids
        .difference(&manifest_node_ids)
        .map(|s| s.to_string())
        .collect();

    // Changed nodes: in both, but fields differ
    let mut changed_nodes = Vec::new();
    let mut unchanged_nodes = Vec::new();
    for manifest_node in &parsed.nodes {
        if let Some(ref id) = manifest_node.id {
            if let Some(db_node) = db_node_map.get(id.as_str()) {
                let diffs = compute_node_diffs(manifest_node, db_node);
                if diffs.is_empty() {
                    unchanged_nodes.push(id.clone());
                } else {
                    changed_nodes.push(ManifestDiffEntry {
                        entity_id: id.clone(),
                        diffs,
                    });
                }
            }
        }
    }

    // Added edges: in manifest but not in DB
    let added_edges: Vec<String> = manifest_edge_ids
        .difference(&db_edge_ids)
        .map(|s| s.to_string())
        .collect();
    let removed_edges: Vec<String> = db_edge_ids
        .difference(&manifest_edge_ids)
        .map(|s| s.to_string())
        .collect();

    // Changed edges: in both, but fields differ
    let mut changed_edges = Vec::new();
    let mut unchanged_edges = Vec::new();
    let db_edge_map: std::collections::HashMap<&str, &PlanningGraphEdge> =
        db_edges.iter().map(|e| (e.id.as_str(), e)).collect();
    for manifest_edge in &parsed.edges {
        if let Some(ref id) = manifest_edge.id {
            if let Some(db_edge) = db_edge_map.get(id.as_str()) {
                let diffs = compute_edge_diffs(manifest_edge, db_edge);
                if diffs.is_empty() {
                    unchanged_edges.push(id.clone());
                } else {
                    changed_edges.push(ManifestDiffEntry {
                        entity_id: id.clone(),
                        diffs,
                    });
                }
            }
        }
    }

    let diff = ManifestDiffResult {
        added_nodes,
        removed_nodes,
        changed_nodes,
        unchanged_nodes,
        added_edges,
        removed_edges,
        changed_edges,
        unchanged_edges,
    };

    emit_success(context, vec!["diff"], diff)
}

fn compute_node_diffs(input: &CreateGraphNodeInput, db: &PlanningGraphNode) -> Vec<FieldDiff> {
    let mut diffs = Vec::new();
    if input.title.trim() != db.title {
        diffs.push(FieldDiff {
            field: "title".to_string(),
            manifest_value: serde_json::json!(input.title.trim()),
            db_value: serde_json::json!(db.title),
        });
    }
    if input.summary.trim() != db.summary {
        diffs.push(FieldDiff {
            field: "summary".to_string(),
            manifest_value: serde_json::json!(input.summary.trim()),
            db_value: serde_json::json!(db.summary),
        });
    }
    if input.status.trim() != db.status {
        diffs.push(FieldDiff {
            field: "status".to_string(),
            manifest_value: serde_json::json!(input.status.trim()),
            db_value: serde_json::json!(db.status),
        });
    }
    diffs
}

fn compute_edge_diffs(input: &CreateGraphEdgeInput, db: &PlanningGraphEdge) -> Vec<FieldDiff> {
    let mut diffs = Vec::new();
    if input.status.trim() != db.status {
        diffs.push(FieldDiff {
            field: "status".to_string(),
            manifest_value: serde_json::json!(input.status.trim()),
            db_value: serde_json::json!(db.status),
        });
    }
    if input.kind.as_str() != db.kind.as_str() {
        diffs.push(FieldDiff {
            field: "kind".to_string(),
            manifest_value: serde_json::json!(input.kind.as_str()),
            db_value: serde_json::json!(db.kind.as_str()),
        });
    }
    if input.source_node_id != db.source_node_id {
        diffs.push(FieldDiff {
            field: "sourceNodeId".to_string(),
            manifest_value: serde_json::json!(input.source_node_id),
            db_value: serde_json::json!(db.source_node_id),
        });
    }
    if input.target_node_id != db.target_node_id {
        diffs.push(FieldDiff {
            field: "targetNodeId".to_string(),
            manifest_value: serde_json::json!(input.target_node_id),
            db_value: serde_json::json!(db.target_node_id),
        });
    }
    diffs
}

#[derive(Args, Debug)]
struct ManifestArgs {
    #[arg(long)]
    file: PathBuf,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args, Debug)]
struct DiffArgs {
    #[arg(long)]
    manifest: PathBuf,
}

fn parse_graph_payload(
    payload_json: &Option<String>,
    payload_file: &Option<PathBuf>,
) -> Result<serde_json::Value, CliError> {
    match (payload_json, payload_file) {
        (Some(json_str), None) => Ok(serde_json::from_str(json_str)?),
        (None, Some(path)) => {
            let content = std::fs::read_to_string(path).map_err(|e| {
                CliError::Store(PlanningStoreError::InvalidInput(format!(
                    "failed to read payload file: {e}"
                )))
            })?;
            Ok(serde_json::from_str(&content)?)
        }
        (None, None) => Ok(serde_json::json!({})),
        (Some(_), Some(_)) => Err(CliError::Store(PlanningStoreError::InvalidInput(
            "cannot specify both --payload-json and --payload-file".to_string(),
        ))),
    }
}

fn execute_search(
    args: SearchArgs,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    let input = SearchInput {
        scope_key: Some(context.scope_key.clone()),
        title: args.title,
        status: args.status,
        since: args.since,
        latest: args.latest,
        tag: args.tag,
        fts: args.fts,
    };
    let results = store.search_all(&input)?;
    emit_success(context, vec!["search"], json!({ "results": results }))
}

fn execute_entity_search(
    args: EntitySearchArgs,
    store: &PlanningStore,
    context: &MachineContext,
    entity_type: &str,
) -> Result<ExitCode, CliError> {
    let input = SearchInput {
        scope_key: Some(context.scope_key.clone()),
        title: args.title,
        status: args.status,
        since: args.since,
        latest: args.latest,
        tag: args.tag,
        fts: args.fts,
    };
    let results = match entity_type {
        "goal" => store.search_goals(&input)?,
        "roadmap" => store.search_roadmaps(&input)?,
        "plan" => store.search_plans(&input)?,
        "todo" => store.search_todos(&input)?,
        "issue" => store.search_issues(&input)?,
        "insight" => store.search_insights(&input)?,
        _ => Vec::new(),
    };
    emit_success(
        context,
        vec![entity_type, "search"],
        json!({ "results": results }),
    )
}

fn execute_insight(
    command: InsightCommand,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    match command {
        InsightCommand::Record(args) => {
            let correlation_id = match resolve_correlation_id(args.correlation_id, context) {
                Ok(value) => value,
                Err(message) => {
                    return emit_error(context, vec!["insight", "record"], message, true)
                }
            };
            emit_success(
                context,
                vec!["insight", "record"],
                store.create_insight(CreateInsightInput {
                    id: args.id,
                    scope_key: Some(context.scope_key.clone()),
                    correlation_id,
                    title: args.title,
                    content: args.content,
                    insight_type: args.insight_type,
                    parent_entity_type: args.parent_entity_type,
                    parent_entity_id: args.parent_entity_id,
                    tags: args.tags,
                    status: args.status,
                    run_id: context.correlation_id.clone(),
                })?,
            )
        }
        InsightCommand::List(args) => {
            if args.all {
                emit_success(
                    context,
                    vec!["insight", "list"],
                    json!({ "insights": store.list_insights_in_scope(&context.scope_key)? }),
                )
            } else {
                let parent_type = args.parent_entity_type.ok_or_else(|| {
                    CliError::Store(crate::PlanningStoreError::InvalidInput(
                        "either --all or --parent-type and --parent-id are required for insight list"
                            .to_string(),
                    ))
                })?;
                let parent_id = args.parent_entity_id.ok_or_else(|| {
                    CliError::Store(crate::PlanningStoreError::InvalidInput(
                        "either --all or --parent-type and --parent-id are required for insight list"
                            .to_string(),
                    ))
                })?;
                emit_success(
                    context,
                    vec!["insight", "list"],
                    json!({ "insights": store
                        .list_insights_for_entity(parent_type, &parent_id, &context.scope_key)? }),
                )
            }
        }
        InsightCommand::Show(args) => {
            let view = store.insight(&args.insight_id)?;
            if view.insight.scope_key != context.scope_key {
                return emit_error(
                    context,
                    vec!["insight", "show"],
                    format!(
                        "insight `{}` is in scope `{}`, not `{}`",
                        args.insight_id, view.insight.scope_key, context.scope_key
                    ),
                    true,
                );
            }
            emit_success(context, vec!["insight", "show"], view)
        }
        InsightCommand::Search(args) => {
            let input = SearchInput {
                scope_key: Some(context.scope_key.clone()),
                title: args.title,
                status: args.status,
                since: args.since,
                latest: args.latest,
                tag: args.tag,
                fts: args.fts,
            };
            let results = store.search_insights(&input)?;
            emit_success(
                context,
                vec!["insight", "search"],
                json!({ "results": results }),
            )
        }
        InsightCommand::UpdateStatus(args) => emit_success(
            context,
            vec!["insight", "update-status"],
            store.update_status(UpdateStatusInput {
                entity_type: EntityType::Insight,
                entity_id: args.insight_id,
                status: args.status.as_str().to_string(),
                evidence_refs: None,
                active_scope_key: Some(context.scope_key.clone()),
                run_id: context.correlation_id.clone(),
                override_transition: args.override_transition,
                reason: args.reason,
            })?,
        ),
    }
}

fn execute_context(
    args: ContextArgs,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    if args.session {
        let correlation_id = context.correlation_id.clone().unwrap_or_default();
        if correlation_id.is_empty() {
            return emit_error(
                context,
                vec!["context"],
                "session context requires --correlation-id or an active session".to_string(),
                true,
            );
        }
        let bundle = store.session_context(&correlation_id, &context.scope_key)?;
        return emit_success(context, vec!["context"], bundle);
    }

    match (args.entity_type, args.entity_id) {
        (Some(entity_type), Some(entity_id)) => {
            let bundle = store.context_bundle(entity_type, &entity_id, &context.scope_key)?;
            emit_success(context, vec!["context"], bundle)
        }
        _ => emit_error(
            context,
            vec!["context"],
            "context requires --entity-type and --entity-id, or --session".to_string(),
            true,
        ),
    }
}

fn execute_tags(
    args: TagsArgs,
    store: &PlanningStore,
    context: &MachineContext,
) -> Result<ExitCode, CliError> {
    let tags = store.list_tags(&context.scope_key, args.entity_type.as_deref())?;
    emit_success(context, vec!["tags", "list"], json!({ "tags": tags }))
}

fn emit_success<T>(
    context: &MachineContext,
    command: Vec<&str>,
    data: T,
) -> Result<ExitCode, CliError>
where
    T: Serialize,
{
    match context.format {
        OutputFormat::Text => {
            let text = serde_json::to_string_pretty(&data)?;
            println!("{text}");
        }
        OutputFormat::Json => print_json(&MachineEnvelope::ok(
            context.correlation_id.clone(),
            context.non_interactive,
            command.iter().map(|item| (*item).to_string()).collect(),
            data,
        ))?,
    }
    Ok(ExitCode::SUCCESS)
}

fn emit_error(
    context: &MachineContext,
    command: Vec<&str>,
    message: String,
    invalid: bool,
) -> Result<ExitCode, CliError> {
    match context.format {
        OutputFormat::Text => eprintln!("{message}"),
        OutputFormat::Json => print_json(&MachineEnvelope::<serde_json::Value>::error(
            context.correlation_id.clone(),
            context.non_interactive,
            command.iter().map(|item| (*item).to_string()).collect(),
            if invalid {
                MachineStatus::Invalid
            } else {
                MachineStatus::Error
            },
            message,
        ))?,
    }
    Ok(if invalid {
        exit_invalid()
    } else {
        exit_runtime()
    })
}

fn resolve_output_format(json: bool, format: OutputFormat) -> OutputFormat {
    if json {
        OutputFormat::Json
    } else {
        format
    }
}

fn resolve_parse_error_format(raw_args: &[OsString]) -> OutputFormat {
    if parse_flag_value(raw_args, "--json")
        || parse_flag_argument_value(raw_args, "--format").as_deref() == Some("json")
    {
        OutputFormat::Json
    } else {
        OutputFormat::Text
    }
}

fn parse_flag_value(raw_args: &[OsString], flag: &str) -> bool {
    raw_args.iter().skip(1).any(|value| {
        let value = value.to_string_lossy();
        value == flag || value.starts_with(&format!("{flag}="))
    })
}

fn parse_flag_argument_value(raw_args: &[OsString], flag: &str) -> Option<String> {
    let values = raw_args
        .iter()
        .skip(1)
        .map(|value| value.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    for (index, value) in values.iter().enumerate() {
        if value == flag {
            if let Some(next) = values.get(index + 1) {
                let next = next.trim();
                if !next.is_empty() {
                    return Some(next.to_string());
                }
            }
        }

        if let Some(inline) = value.strip_prefix(&format!("{flag}=")) {
            let inline = inline.trim();
            if !inline.is_empty() {
                return Some(inline.to_string());
            }
        }
    }

    None
}

fn parse_command_path(raw_args: &[OsString]) -> Vec<String> {
    let values = raw_args
        .iter()
        .skip(1)
        .map(|value| value.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let command_names = command_names();
    let mut path = Vec::new();
    let value_flags = ["--db", "--correlation-id", "--format", "--scope"];

    let mut index = 0;
    while index < values.len() {
        let normalized = values[index].trim();

        if normalized.starts_with('-') {
            if value_flags.contains(&normalized) {
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }

        if path.is_empty() {
            if command_names
                .iter()
                .any(|candidate| candidate == normalized)
            {
                path.push(normalized.to_string());
            }
            index += 1;
            break;
        }
        index += 1;
    }

    while index < values.len() && path.len() < 2 {
        let normalized = values[index].trim();
        if normalized.starts_with('-') {
            break;
        }
        path.push(normalized.to_string());
        index += 1;
    }

    path
}

fn command_names() -> Vec<String> {
    Cli::command()
        .get_subcommands()
        .map(|command| command.get_name().to_string())
        .collect()
}

fn command_path(command: &Command) -> Vec<String> {
    match command {
        Command::Scope { command } => {
            vec!["scope".to_string(), scope_command_name(command).to_string()]
        }
        Command::Goal { command } => {
            vec!["goal".to_string(), goal_command_name(command).to_string()]
        }
        Command::Roadmap { command } => {
            vec![
                "roadmap".to_string(),
                roadmap_command_name(command).to_string(),
            ]
        }
        Command::WorkPoint { command } => vec![
            "work-point".to_string(),
            work_point_command_name(command).to_string(),
        ],
        Command::Plan { command } => {
            vec!["plan".to_string(), plan_command_name(command).to_string()]
        }
        Command::Todo { command } => {
            vec!["todo".to_string(), todo_command_name(command).to_string()]
        }
        Command::Issue { command } => {
            vec!["issue".to_string(), issue_command_name(command).to_string()]
        }
        Command::ReviewPoint { command } => vec![
            "review-point".to_string(),
            review_point_command_name(command).to_string(),
        ],
        Command::Validate { command } => vec![
            "validate".to_string(),
            validate_command_name(command).to_string(),
        ],
        Command::Events => vec!["events".to_string(), "list".to_string()],
        Command::Health => vec!["health".to_string()],
        Command::Capabilities => vec!["capabilities".to_string()],
        Command::Project { command } => vec![
            "project".to_string(),
            project_command_name(command).to_string(),
        ],
        Command::Session { command } => vec![
            "session".to_string(),
            session_command_name(command).to_string(),
        ],
        Command::Search(_) => vec!["search".to_string()],
        Command::Insight { command } => vec![
            "insight".to_string(),
            insight_command_name(command).to_string(),
        ],
        Command::Context(_) => vec!["context".to_string()],
        Command::Tags(_) => vec!["tags".to_string()],
        Command::ProjectRun { command } => vec![
            "project-run".to_string(),
            project_run_command_name(command).to_string(),
        ],
        Command::Worktree { command } => vec![
            "worktree".to_string(),
            worktree_command_name(command).to_string(),
        ],
        Command::Graph { command } => match command {
            GraphCommand::Node { command } => vec![
                "graph".to_string(),
                "node".to_string(),
                graph_node_command_name(command).to_string(),
            ],
            GraphCommand::Edge { command } => vec![
                "graph".to_string(),
                "edge".to_string(),
                graph_edge_command_name(command).to_string(),
            ],
            GraphCommand::Acceptance { command } => vec![
                "graph".to_string(),
                "acceptance".to_string(),
                acceptance_command_name(command).to_string(),
            ],
            GraphCommand::Evidence { command } => vec![
                "graph".to_string(),
                "evidence".to_string(),
                evidence_command_name(command).to_string(),
            ],
            GraphCommand::Runnable(_) => vec!["graph".to_string(), "runnable".to_string()],
            GraphCommand::Bulk(_) => vec!["graph".to_string(), "bulk".to_string()],
        },
        Command::Manifest(_) => vec!["manifest".to_string(), "apply".to_string()],
        Command::Diff(_) => vec!["diff".to_string()],
        Command::Template { command } => match command {
            TemplateCommand::List(_) => vec!["template".to_string(), "list".to_string()],
            TemplateCommand::Render(_) => vec!["template".to_string(), "render".to_string()],
        },
        Command::Intent(_) => vec!["intent".to_string(), "expand".to_string()],
    }
}

fn scope_command_name(command: &ScopeCommand) -> &'static str {
    match command {
        ScopeCommand::Create(_) => "create",
        ScopeCommand::List => "list",
        ScopeCommand::Show(_) => "show",
    }
}

fn goal_command_name(command: &GoalCommand) -> &'static str {
    match command {
        GoalCommand::Create(_) => "create",
        GoalCommand::UpdateStatus(_) => "update-status",
        GoalCommand::List => "list",
        GoalCommand::Show(_) => "show",
        GoalCommand::Search(_) => "search",
    }
}

fn roadmap_command_name(command: &RoadmapCommand) -> &'static str {
    match command {
        RoadmapCommand::Create(_) => "create",
        RoadmapCommand::UpdateStatus(_) => "update-status",
        RoadmapCommand::AddSection(_) => "add-section",
        RoadmapCommand::AddWorkPoint(_) => "add-work-point",
        RoadmapCommand::List => "list",
        RoadmapCommand::Show(_) => "show",
        RoadmapCommand::Search(_) => "search",
    }
}

fn work_point_command_name(command: &WorkPointCommand) -> &'static str {
    match command {
        WorkPointCommand::List => "list",
        WorkPointCommand::Show(_) => "show",
        WorkPointCommand::UpdateStatus(_) => "update-status",
        WorkPointCommand::NextRunnable(_) => "next-runnable",
        WorkPointCommand::WorkGraph(_) => "work-graph",
        WorkPointCommand::Revise(_) => "revise",
    }
}

fn project_run_command_name(command: &ProjectRunCommand) -> &'static str {
    match command {
        ProjectRunCommand::Claim(_) => "claim",
        ProjectRunCommand::Activate(_) => "activate",
        ProjectRunCommand::Heartbeat(_) => "heartbeat",
        ProjectRunCommand::Release(_) => "release",
        ProjectRunCommand::AddEvidence(_) => "add-evidence",
        ProjectRunCommand::List => "list",
        ProjectRunCommand::Show(_) => "show",
    }
}

fn worktree_command_name(command: &WorktreeCommand) -> &'static str {
    match command {
        WorktreeCommand::List(_) => "list",
        WorktreeCommand::Show(_) => "show",
        WorktreeCommand::Attach(_) => "attach",
        WorktreeCommand::Archive(_) => "archive",
        WorktreeCommand::CleanupIntent(_) => "cleanup-intent",
    }
}

fn graph_node_command_name(command: &GraphNodeCommand) -> &'static str {
    match command {
        GraphNodeCommand::Create(_) => "create",
        GraphNodeCommand::Show(_) => "show",
        GraphNodeCommand::List(_) => "list",
        GraphNodeCommand::Status(_) => "status",
        GraphNodeCommand::Revise(_) => "revise",
        GraphNodeCommand::Finalize(_) => "finalize",
    }
}

fn graph_edge_command_name(command: &GraphEdgeCommand) -> &'static str {
    match command {
        GraphEdgeCommand::Create(_) => "create",
        GraphEdgeCommand::Show(_) => "show",
        GraphEdgeCommand::List(_) => "list",
        GraphEdgeCommand::Incoming(_) => "incoming",
        GraphEdgeCommand::Outgoing(_) => "outgoing",
        GraphEdgeCommand::Status(_) => "status",
        GraphEdgeCommand::Revise(_) => "revise",
    }
}

fn acceptance_command_name(command: &AcceptanceCommand) -> &'static str {
    match command {
        AcceptanceCommand::Create(_) => "create",
        AcceptanceCommand::Show(_) => "show",
        AcceptanceCommand::List(_) => "list",
        AcceptanceCommand::Satisfy(_) => "satisfy",
    }
}

fn evidence_command_name(command: &EvidenceCommand) -> &'static str {
    match command {
        EvidenceCommand::Create(_) => "create",
        EvidenceCommand::Show(_) => "show",
        EvidenceCommand::List(_) => "list",
        EvidenceCommand::Attach(_) => "attach",
    }
}

fn plan_command_name(command: &PlanCommand) -> &'static str {
    match command {
        PlanCommand::Create(_) => "create",
        PlanCommand::Revise(_) => "revise",
        PlanCommand::UpdateStatus(_) => "update-status",
        PlanCommand::List => "list",
        PlanCommand::Show(_) => "show",
        PlanCommand::Search(_) => "search",
    }
}

fn todo_command_name(command: &TodoCommand) -> &'static str {
    match command {
        TodoCommand::Create(_) => "create",
        TodoCommand::UpdateStatus(_) => "update-status",
        TodoCommand::List => "list",
        TodoCommand::Search(_) => "search",
    }
}

fn issue_command_name(command: &IssueCommand) -> &'static str {
    match command {
        IssueCommand::Record(_) => "record",
        IssueCommand::UpdateStatus(_) => "update-status",
        IssueCommand::List => "list",
        IssueCommand::Show(_) => "show",
        IssueCommand::Search(_) => "search",
    }
}

fn review_point_command_name(command: &ReviewPointCommand) -> &'static str {
    match command {
        ReviewPointCommand::Record(_) => "record",
        ReviewPointCommand::UpdateStatus(_) => "update-status",
    }
}

fn insight_command_name(command: &InsightCommand) -> &'static str {
    match command {
        InsightCommand::Record(_) => "record",
        InsightCommand::List(_) => "list",
        InsightCommand::Show(_) => "show",
        InsightCommand::Search(_) => "search",
        InsightCommand::UpdateStatus(_) => "update-status",
    }
}

fn validate_command_name(command: &ValidateCommand) -> &'static str {
    match command {
        ValidateCommand::All(_) => "all",
    }
}

fn project_command_name(command: &ProjectCommand) -> &'static str {
    match command {
        ProjectCommand::Export(_) => "export",
        ProjectCommand::Render(_) => "render",
    }
}

fn session_command_name(command: &SessionCommand) -> &'static str {
    match command {
        SessionCommand::Init(_) => "init",
        SessionCommand::Use(_) => "use",
        SessionCommand::Show => "show",
        SessionCommand::Resume(_) => "resume",
        SessionCommand::List(_) => "list",
    }
}

fn resolve_correlation_id(
    command_value: Option<String>,
    context: &MachineContext,
) -> Result<String, String> {
    if let Some(value) = command_value {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    if let Some(value) = &context.correlation_id {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    if let Ok(Some(session_id)) = crate::session::resolve_session_correlation_id() {
        if !session_id.is_empty() {
            return Ok(session_id);
        }
    }
    Err("correlation id is required; pass --correlation-id globally, on the command, or run `elegy-planning session init` first".to_string())
}

fn optional_vec(values: Vec<String>) -> Option<Vec<String>> {
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn optional_file_scopes(values: Vec<String>) -> Result<Option<Vec<FileScopeRecord>>, CliError> {
    if values.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parse_file_scopes(values)?))
    }
}

fn parse_file_scopes(values: Vec<String>) -> Result<Vec<FileScopeRecord>, CliError> {
    let mut scopes = Vec::new();
    for raw in values {
        let mut segments = raw.splitn(3, ':');
        let selector_type = segments.next().unwrap_or_default().trim();
        let intent = segments.next().unwrap_or_default().trim();
        let selector = segments.next().unwrap_or_default().trim();
        if selector_type.is_empty() || intent.is_empty() || selector.is_empty() {
            return Err(CliError::Store(crate::PlanningStoreError::InvalidInput(
                "file scope must match '<selector-type>:<intent>:<selector>'".to_string(),
            )));
        }
        let selector_type = selector_type
            .parse::<FileScopeSelectorType>()
            .map_err(|error| {
                CliError::Store(crate::PlanningStoreError::InvalidInput(error.to_string()))
            })?;
        let intent = intent.parse::<FileScopeIntent>().map_err(|error| {
            CliError::Store(crate::PlanningStoreError::InvalidInput(error.to_string()))
        })?;
        scopes.push(FileScopeRecord {
            selector_type,
            selector: selector.to_string(),
            intent,
        });
    }
    Ok(scopes)
}

fn parse_optional_json_object(
    value: Option<String>,
) -> Result<Option<serde_json::Value>, CliError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let parsed: serde_json::Value = serde_json::from_str(value.trim())?;
    if !parsed.is_object() {
        return Err(CliError::Store(crate::PlanningStoreError::InvalidInput(
            "metadataJson must be a JSON object".to_string(),
        )));
    }
    Ok(Some(parsed))
}

fn default_db_path() -> PathBuf {
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".elegy").join("planning.db")
}

/// Returns true if the command is a mutation (write operation) that requires scope in machine mode.
fn is_command_mutation(command: &Command) -> bool {
    match command {
        Command::Scope { command } => matches!(command, ScopeCommand::Create(_)),
        Command::Goal { command } => matches!(
            command,
            GoalCommand::Create(_) | GoalCommand::UpdateStatus(_)
        ),
        Command::Roadmap { command } => matches!(
            command,
            RoadmapCommand::Create(_)
                | RoadmapCommand::UpdateStatus(_)
                | RoadmapCommand::AddSection(_)
                | RoadmapCommand::AddWorkPoint(_)
        ),
        Command::WorkPoint { command } => matches!(
            command,
            WorkPointCommand::UpdateStatus(_) | WorkPointCommand::Revise(_)
        ),
        Command::Plan { command } => matches!(
            command,
            PlanCommand::Create(_) | PlanCommand::Revise(_) | PlanCommand::UpdateStatus(_)
        ),
        Command::Todo { command } => matches!(
            command,
            TodoCommand::Create(_) | TodoCommand::UpdateStatus(_)
        ),
        Command::Issue { command } => matches!(
            command,
            IssueCommand::Record(_) | IssueCommand::UpdateStatus(_)
        ),
        Command::ReviewPoint { command } => matches!(
            command,
            ReviewPointCommand::Record(_) | ReviewPointCommand::UpdateStatus(_)
        ),
        Command::Insight { command } => matches!(
            command,
            InsightCommand::Record(_) | InsightCommand::UpdateStatus(_)
        ),
        Command::ProjectRun { command } => matches!(
            command,
            ProjectRunCommand::Claim(_)
                | ProjectRunCommand::Activate(_)
                | ProjectRunCommand::Heartbeat(_)
                | ProjectRunCommand::Release(_)
                | ProjectRunCommand::AddEvidence(_)
        ),
        Command::Worktree { command } => matches!(
            command,
            WorktreeCommand::Attach(_)
                | WorktreeCommand::Archive(_)
                | WorktreeCommand::CleanupIntent(_)
        ),
        Command::Graph { command } => matches!(
            command,
            GraphCommand::Node {
                command: GraphNodeCommand::Create(_)
                    | GraphNodeCommand::Status(_)
                    | GraphNodeCommand::Revise(_)
                    | GraphNodeCommand::Finalize(_)
            } | GraphCommand::Edge {
                command: GraphEdgeCommand::Create(_)
                    | GraphEdgeCommand::Status(_)
                    | GraphEdgeCommand::Revise(_)
            } | GraphCommand::Acceptance {
                command: AcceptanceCommand::Create(_) | AcceptanceCommand::Satisfy(_)
            } | GraphCommand::Evidence {
                command: EvidenceCommand::Create(_) | EvidenceCommand::Attach(_)
            }
        ),
        Command::Manifest(_) => true,
        Command::Diff(_) => false,
        Command::Template { .. } => false,
        Command::Intent(_) => false,
        // Read-only commands
        Command::Validate { .. }
        | Command::Events
        | Command::Health
        | Command::Capabilities
        | Command::Project { .. }
        | Command::Search(_)
        | Command::Context(_)
        | Command::Tags(_) => false,
        // Session commands have their own exemption check (session init is exempt)
        Command::Session { .. } => false,
    }
}

/// Returns true if the command is exempt from the scope gate check.
/// Currently only `session init` is exempt — it needs to run without scope to create a session.
fn is_command_exempt_from_scope_gate(command: &Command) -> bool {
    matches!(command, Command::Session { command } if matches!(command, SessionCommand::Init(_)))
}

fn exit_invalid() -> ExitCode {
    ExitCode::from(EXIT_CODE_INVALID_INPUT)
}

fn exit_runtime() -> ExitCode {
    ExitCode::from(EXIT_CODE_RUNTIME_FAILURE)
}

fn print_json<T>(value: &T) -> Result<(), serde_json::Error>
where
    T: Serialize,
{
    let text = serde_json::to_string_pretty(value)?;
    println!("{text}");
    Ok(())
}
