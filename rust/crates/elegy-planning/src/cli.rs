use std::{env, ffi::OsString, path::PathBuf, process::ExitCode, sync::OnceLock};

use clap::{error::ErrorKind, Args, CommandFactory, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::json;
use thiserror::Error;

use crate::{
    AddRoadmapSectionInput, AddWorkPointInput, CreateGoalInput, CreateIssueInput, CreatePlanInput,
    CreateReviewPointInput, CreateRoadmapInput, CreateScopeInput, CreateTodoInput, EntityType,
    GoalStatus, IssueStatus, PlanStatus, PlanningStore, Priority, ProjectionFormat,
    ReviewPointStatus, RevisePlanInput, RoadmapStatus, Severity, TodoStatus, UpdateStatusInput,
    WorkPointStatus,
};

const EXIT_CODE_INVALID_INPUT: u8 = 1;
const EXIT_CODE_RUNTIME_FAILURE: u8 = 2;
const RESULT_SCHEMA_VERSION: &str = "planning-result/v1";

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
    Scope {
        #[command(subcommand)]
        command: ScopeCommand,
    },
    Goal {
        #[command(subcommand)]
        command: GoalCommand,
    },
    Roadmap {
        #[command(subcommand)]
        command: RoadmapCommand,
    },
    WorkPoint {
        #[command(subcommand)]
        command: WorkPointCommand,
    },
    Plan {
        #[command(subcommand)]
        command: PlanCommand,
    },
    Todo {
        #[command(subcommand)]
        command: TodoCommand,
    },
    Issue {
        #[command(subcommand)]
        command: IssueCommand,
    },
    ReviewPoint {
        #[command(subcommand)]
        command: ReviewPointCommand,
    },
    Validate {
        #[command(subcommand)]
        command: ValidateCommand,
    },
    Events,
    Health,
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
}

#[derive(Subcommand, Debug)]
enum GoalCommand {
    Create(GoalCreateArgs),
    UpdateStatus(GoalUpdateStatusArgs),
    List,
    Show(GoalShowArgs),
}

#[derive(Subcommand, Debug)]
enum RoadmapCommand {
    Create(RoadmapCreateArgs),
    UpdateStatus(RoadmapUpdateStatusArgs),
    AddSection(RoadmapAddSectionArgs),
    AddWorkPoint(RoadmapAddWorkPointArgs),
    List,
    Show(RoadmapShowArgs),
}

#[derive(Subcommand, Debug)]
enum WorkPointCommand {
    List,
    Show(WorkPointShowArgs),
    UpdateStatus(WorkPointUpdateStatusArgs),
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
enum PlanCommand {
    Create(PlanCreateArgs),
    Revise(PlanReviseArgs),
    UpdateStatus(PlanUpdateStatusArgs),
    List,
    Show(PlanShowArgs),
}

#[derive(Subcommand, Debug)]
enum TodoCommand {
    Create(TodoCreateArgs),
    UpdateStatus(TodoUpdateStatusArgs),
    List,
}

#[derive(Subcommand, Debug)]
enum IssueCommand {
    Record(IssueRecordArgs),
    UpdateStatus(IssueUpdateStatusArgs),
    List,
    Show(IssueShowArgs),
}

#[derive(Subcommand, Debug)]
enum ReviewPointCommand {
    Record(ReviewPointRecordArgs),
    UpdateStatus(ReviewPointUpdateStatusArgs),
}

#[derive(Subcommand, Debug)]
enum ValidateCommand {
    All,
}

#[derive(Subcommand, Debug)]
enum ProjectCommand {
    Export(ProjectRenderArgs),
    Render(ProjectRenderArgs),
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
    #[arg(long = "tag")]
    tags: Vec<String>,
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
    #[arg(long = "tag")]
    tags: Vec<String>,
}

#[derive(Args, Debug)]
struct PlanUpdateStatusArgs {
    #[arg(long = "plan-id")]
    plan_id: String,
    #[arg(long, value_enum)]
    status: PlanStatus,
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MachineEnvelope<T>
where
    T: Serialize,
{
    schema_version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    correlation_id: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    non_interactive: bool,
    command: Vec<String>,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Clone, Debug)]
struct MachineContext {
    format: OutputFormat,
    non_interactive: bool,
    correlation_id: Option<String>,
    scope_key: String,
    db_path: PathBuf,
    command: Vec<String>,
}

pub fn run_from_env() -> ExitCode {
    match run_from(std::env::args_os()) {
        Ok(code) => code,
        Err(error) => {
            if let Some(context) = CLI_MACHINE_CONTEXT.get() {
                if context.format == OutputFormat::Json
                    && print_json(&MachineEnvelope::<serde_json::Value> {
                        schema_version: RESULT_SCHEMA_VERSION,
                        correlation_id: context.correlation_id.clone(),
                        non_interactive: context.non_interactive,
                        command: context.command.clone(),
                        status: error.status(),
                        data: None,
                        error: Some(error.to_string()),
                    })
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
    };
    let _ = CLI_MACHINE_CONTEXT.set(context.clone());
    let store = PlanningStore::new(&context.db_path);
    store.init()?;

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
        Command::Project { command } => execute_project(command, &store, &context),
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
        print_json(&MachineEnvelope::<serde_json::Value> {
            schema_version: RESULT_SCHEMA_VERSION,
            correlation_id,
            non_interactive,
            command,
            status: "invalid",
            data: None,
            error: Some(error.to_string()),
        })?;
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
        ScopeCommand::Create(args) => emit_success(
            context,
            vec!["scope", "create"],
            store.create_scope(CreateScopeInput {
                scope_key: args.scope_key,
                scope_type: args.scope_type,
                parent_scope_key: args.parent_scope_key,
                metadata: parse_optional_json_object(args.metadata_json)?,
                tags: args.tags,
                run_id: context.correlation_id.clone(),
            })?,
        ),
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
                run_id: context.correlation_id.clone(),
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
                run_id: context.correlation_id.clone(),
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
                run_id: context.correlation_id.clone(),
            })?,
        ),
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
                    status: args.status,
                    tags: args.tags,
                    run_id: context.correlation_id.clone(),
                })?,
            )
        }
        PlanCommand::Revise(args) => emit_success(
            context,
            vec!["plan", "revise"],
            store.revise_plan(RevisePlanInput {
                plan_id: args.plan_id,
                scope_key: args.scope_key,
                assumptions: optional_vec(args.assumptions),
                stop_conditions: optional_vec(args.stop_conditions),
                validation_steps: optional_vec(args.validation_steps),
                targeted_work_point_ids: optional_vec(args.targeted_work_point_ids),
                tags: optional_vec(args.tags),
                run_id: context.correlation_id.clone(),
            })?,
        ),
        PlanCommand::UpdateStatus(args) => emit_success(
            context,
            vec!["plan", "update-status"],
            store.update_status(UpdateStatusInput {
                entity_type: EntityType::Plan,
                entity_id: args.plan_id,
                status: args.status.as_str().to_string(),
                evidence_refs: None,
                run_id: context.correlation_id.clone(),
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
                run_id: context.correlation_id.clone(),
            })?,
        ),
        TodoCommand::List => emit_success(
            context,
            vec!["todo", "list"],
            json!({ "todos": store.list_todos_in_scope(&context.scope_key)? }),
        ),
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
                run_id: context.correlation_id.clone(),
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
                run_id: context.correlation_id.clone(),
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
        ValidateCommand::All => {
            emit_success(context, vec!["validate", "all"], store.validate_all()?)
        }
    }
}

fn execute_events(store: &PlanningStore, context: &MachineContext) -> Result<ExitCode, CliError> {
    emit_success(
        context,
        vec!["events", "list"],
        json!({ "events": store.list_events()? }),
    )
}

fn execute_health(store: &PlanningStore, context: &MachineContext) -> Result<ExitCode, CliError> {
    emit_success(context, vec!["health"], store.health()?)
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
            store.render_projection(
                args.entity_type,
                &args.entity_id,
                args.projection_format,
                &args.output,
            )?,
        ),
        ProjectCommand::Render(args) => emit_success(
            context,
            vec!["project", "render"],
            store.render_projection(
                args.entity_type,
                &args.entity_id,
                args.projection_format,
                &args.output,
            )?,
        ),
    }
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
        OutputFormat::Json => print_json(&MachineEnvelope {
            schema_version: RESULT_SCHEMA_VERSION,
            correlation_id: context.correlation_id.clone(),
            non_interactive: context.non_interactive,
            command: command.iter().map(|item| (*item).to_string()).collect(),
            status: "ok",
            data: Some(data),
            error: None,
        })?,
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
        OutputFormat::Json => print_json(&MachineEnvelope::<serde_json::Value> {
            schema_version: RESULT_SCHEMA_VERSION,
            correlation_id: context.correlation_id.clone(),
            non_interactive: context.non_interactive,
            command: command.iter().map(|item| (*item).to_string()).collect(),
            status: if invalid { "invalid" } else { "error" },
            data: None,
            error: Some(message),
        })?,
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
        Command::Project { command } => vec![
            "project".to_string(),
            project_command_name(command).to_string(),
        ],
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
    }
}

fn work_point_command_name(command: &WorkPointCommand) -> &'static str {
    match command {
        WorkPointCommand::List => "list",
        WorkPointCommand::Show(_) => "show",
        WorkPointCommand::UpdateStatus(_) => "update-status",
    }
}

fn plan_command_name(command: &PlanCommand) -> &'static str {
    match command {
        PlanCommand::Create(_) => "create",
        PlanCommand::Revise(_) => "revise",
        PlanCommand::UpdateStatus(_) => "update-status",
        PlanCommand::List => "list",
        PlanCommand::Show(_) => "show",
    }
}

fn todo_command_name(command: &TodoCommand) -> &'static str {
    match command {
        TodoCommand::Create(_) => "create",
        TodoCommand::UpdateStatus(_) => "update-status",
        TodoCommand::List => "list",
    }
}

fn issue_command_name(command: &IssueCommand) -> &'static str {
    match command {
        IssueCommand::Record(_) => "record",
        IssueCommand::UpdateStatus(_) => "update-status",
        IssueCommand::List => "list",
        IssueCommand::Show(_) => "show",
    }
}

fn review_point_command_name(command: &ReviewPointCommand) -> &'static str {
    match command {
        ReviewPointCommand::Record(_) => "record",
        ReviewPointCommand::UpdateStatus(_) => "update-status",
    }
}

fn validate_command_name(command: &ValidateCommand) -> &'static str {
    match command {
        ValidateCommand::All => "all",
    }
}

fn project_command_name(command: &ProjectCommand) -> &'static str {
    match command {
        ProjectCommand::Export(_) => "export",
        ProjectCommand::Render(_) => "render",
    }
}

fn resolve_correlation_id(
    command_value: Option<String>,
    context: &MachineContext,
) -> Result<String, String> {
    command_value
        .or_else(|| context.correlation_id.clone())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "correlation id is required; pass --correlation-id globally or on the command"
                .to_string()
        })
}

fn optional_vec(values: Vec<String>) -> Option<Vec<String>> {
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
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

fn is_false(value: &bool) -> bool {
    !*value
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
