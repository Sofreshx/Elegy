use clap::{Args, Parser, Subcommand, ValueEnum};
use elegy_contracts::{export_contract_bundle, ContractsBundleExport, ContractsError};
use elegy_core::{
    compose_runtime, validate_descriptor_set, Catalog, ConfigInspection, CoreError, Diagnostic,
    McpAnalysisResult, McpTransportKind, ProjectLocator, ResourceFamily, Severity,
    CLI_SCHEMA_VERSION,
};
use elegy_host_mcp::{serve_stdio, HostError};
use elegy_mermaid::{
    narrate_from_json_str, narrate_from_mermaid_str, render_from_json_str,
    reverse_from_mermaid_str, MermaidNarrative, MermaidProjectionEdgeRelation,
    MermaidProjectionNodeRole, MermaidProjectionSourceKind, MermaidToolError,
    MermaidWorkflowProjection,
};
use elegy_memory::{
    cli as memory_cli, GovernedMemoryRecord, LocalMemoryCatalogEntry, LocalMemoryExportResult,
    LocalMemoryLifecycleState, LocalMemoryPaths, LocalMemoryQueryOptions, LocalMemoryStore,
    LocalMemoryStoreError, LocalMemoryStoredRecord, SessionContextScope,
    SummaryOnlySessionContextEnvelope, LOCAL_MEMORY_AUTHORITY_POSTURE,
    LOCAL_MEMORY_DETERMINISTIC_ORDERING, LOCAL_MEMORY_SINGLE_WRITER_POSTURE,
    SUMMARY_ONLY_REPRESENTATION, SUMMARY_ONLY_SESSION_CONTEXT_ARTIFACT_KIND,
};
use elegy_tooling::{
    analyze_mcp_descriptor_file, author_mcp_descriptor_to_path,
    generate_skills_from_descriptor_file, AuthorMcpDescriptorRequest, AuthorMcpToolRequest,
    AuthoredMcpDescriptor, GeneratedSkillArtifacts, ToolingError,
};
use serde::Serialize;
use serde_json::json;
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

const SESSION_CONTEXT_PREVIEW_LIMIT: usize = 160;
const LOCAL_DEFAULT_ROOT_DIR: &str = ".elegy-local-memory";
const LOCAL_DEFAULT_VISIBILITY_POSTURE: &str =
    "default active-only queries exclude superseded and tombstoned local records";
const SESSION_CONTEXT_NEUTRAL_VALIDATION_SCOPE: &str =
    "artifact-shape validation over the governed summary-only envelope only";
const SESSION_CONTEXT_AUTHORITY_POSTURE: &str =
    "non-authoritative CLI surface; host-owned authority remains in SAASTools";
const SESSION_CONTEXT_ADAPTER_POSTURE: &str =
    "mirror-or-inspect-only; adapters cannot promote, invalidate, or override host-owned truth";
const SESSION_CONTEXT_HOST_OWNER: &str = "SAASTools";

#[derive(Parser, Debug)]
#[command(name = "elegy")]
#[command(about = "Bootstrap CLI for Elegy runtime and MCP authoring")]
struct Cli {
    #[arg(long)]
    project: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    format: OutputFormat,
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
    Author {
        #[command(subcommand)]
        command: AuthorCommand,
    },
    Analyze {
        #[command(subcommand)]
        command: AnalyzeCommand,
    },
    Generate {
        #[command(subcommand)]
        command: GenerateCommand,
    },
    Validate {
        #[command(subcommand)]
        command: ValidateCommand,
    },
    Inspect {
        #[command(subcommand)]
        command: InspectCommand,
    },
    Local {
        #[command(subcommand)]
        command: LocalCommand,
    },
    Mermaid {
        #[command(subcommand)]
        command: MermaidCommand,
    },
    Run {
        #[arg(long)]
        dry_run: bool,
    },
    Contracts {
        #[command(subcommand)]
        command: ContractsCommand,
    },
}

#[derive(Subcommand, Debug)]
enum AuthorCommand {
    Mcp {
        #[arg(long)]
        server_name: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, value_enum, default_value_t = CliTransport::Stdio)]
        transport: CliTransport,
        #[arg(long = "tool", value_name = "NAME[=DESCRIPTION]")]
        tools: Vec<String>,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand, Debug)]
enum AnalyzeCommand {
    Mcp {
        #[arg(long)]
        descriptor: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum GenerateCommand {
    Skills {
        #[arg(long)]
        descriptor: PathBuf,
        #[arg(long)]
        output_dir: Option<PathBuf>,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand, Debug)]
enum LocalCommand {
    Init {
        #[command(flatten)]
        root: LocalRootArgs,
    },
    Import {
        #[command(flatten)]
        root: LocalRootArgs,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        record_id: String,
        #[arg(long)]
        imported_at_utc: String,
    },
    List {
        #[command(flatten)]
        root: LocalRootArgs,
        #[command(flatten)]
        visibility: LocalVisibilityArgs,
    },
    Show {
        #[command(flatten)]
        root: LocalRootArgs,
        #[arg(long)]
        record_id: String,
        #[command(flatten)]
        visibility: LocalVisibilityArgs,
    },
    Export {
        #[command(flatten)]
        root: LocalRootArgs,
        #[arg(long)]
        record_id: String,
        #[arg(long)]
        output_path: Option<PathBuf>,
        #[command(flatten)]
        visibility: LocalVisibilityArgs,
    },
    Supersede {
        #[command(flatten)]
        root: LocalRootArgs,
        #[arg(long)]
        record_id: String,
        #[arg(long)]
        superseded_by_record_id: String,
    },
    Tombstone {
        #[command(flatten)]
        root: LocalRootArgs,
        #[arg(long)]
        record_id: String,
        #[arg(long)]
        tombstoned_at_utc: String,
        #[arg(long)]
        reason: String,
    },
}

#[derive(Subcommand, Debug)]
enum ValidateCommand {
    Config,
    Runtime,
    #[command(name = "session-context", alias = "memory")]
    SessionContext {
        #[arg(long)]
        input: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum InspectCommand {
    Resources,
    #[command(name = "session-context", alias = "memory")]
    SessionContext,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CliTransport {
    Stdio,
    Http,
}

#[derive(Subcommand, Debug)]
enum ContractsCommand {
    Export {
        #[arg(long)]
        output_path: Option<PathBuf>,
        #[arg(long)]
        create_archive: bool,
        #[arg(long)]
        archive_output_path: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum MermaidCommand {
    Render {
        #[arg(long)]
        input: Option<PathBuf>,
    },
    Reverse {
        #[arg(long)]
        input: Option<PathBuf>,
    },
    Narrate {
        #[arg(long)]
        input: Option<PathBuf>,
    },
}

#[derive(Args, Clone, Debug)]
struct LocalRootArgs {
    #[arg(long)]
    root: Option<PathBuf>,
}

#[derive(Args, Clone, Debug, Default)]
struct LocalVisibilityArgs {
    #[arg(long)]
    include_superseded: bool,
    #[arg(long)]
    include_tombstoned: bool,
}

impl LocalVisibilityArgs {
    fn query_options(&self) -> LocalMemoryQueryOptions {
        LocalMemoryQueryOptions {
            include_superseded: self.include_superseded,
            include_tombstoned: self.include_tombstoned,
        }
    }
}

#[derive(Serialize)]
struct Envelope<T>
where
    T: Serialize,
{
    schema_version: &'static str,
    command: Vec<String>,
    status: &'static str,
    summary: Summary,
    data: T,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Default, Serialize)]
struct Summary {
    errors: usize,
    warnings: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionContextInspection {
    capability: &'static str,
    contract_field: &'static str,
    schema_file: &'static str,
    representation: &'static str,
    supported_scopes: Vec<&'static str>,
    intended_consumers: Vec<&'static str>,
    bounded_fields: Vec<&'static str>,
    raw_transcript_persisted: bool,
    transcript_bodies_allowed_in_artifact: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionContextValidationReport {
    input_path: String,
    artifact_kind: &'static str,
    representation: &'static str,
    scope: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    captured_at_utc: Option<String>,
    summary_length: usize,
    summary_preview: String,
    salient_facts_count: usize,
    instruction_context_count: usize,
    raw_transcript_persisted: bool,
    read_only: bool,
    neutral_validation_scope: &'static str,
    authority_posture: &'static str,
    host_validation_owner: &'static str,
    host_promotion_owner: &'static str,
    host_invalidation_owner: &'static str,
    adapter_posture: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalInitReport {
    root_path: String,
    artifacts_path: String,
    state_path: String,
    write_lock_path: String,
    exports_path: String,
    authority_posture: &'static str,
    single_writer_posture: &'static str,
    deterministic_ordering: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalListReport {
    root_path: String,
    authority_posture: &'static str,
    default_visibility: String,
    deterministic_ordering: &'static str,
    records: Vec<LocalMemoryCatalogEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalRecordReport {
    root_path: String,
    artifact_path: String,
    default_export_path: String,
    authority_posture: &'static str,
    default_visibility: String,
    deterministic_ordering: &'static str,
    record: GovernedMemoryRecord,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalExportReport {
    root_path: String,
    output_path: String,
    authority_posture: &'static str,
    default_visibility: String,
    deterministic_ordering: &'static str,
    record: GovernedMemoryRecord,
    exported_envelope: SummaryOnlySessionContextEnvelope,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MermaidRenderReport {
    mermaid: String,
    input_source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_path: Option<String>,
}

#[derive(Serialize)]
struct MermaidReverseReport {
    projection: MermaidWorkflowProjection,
    input_source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_path: Option<String>,
}

#[derive(Serialize)]
struct MermaidNarrateReport {
    narrative: MermaidNarrative,
    projection: MermaidWorkflowProjection,
    input_source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_path: Option<String>,
}

enum MermaidInputSource {
    File(PathBuf),
    Stdin,
}

enum MermaidInputLoadError {
    File { path: PathBuf, source: io::Error },
    Stdin { source: io::Error },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MermaidNarrateInputKind {
    CanonicalJson,
    MermaidFlowchart,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("unexpected CLI failure: {error}");
            ExitCode::from(2)
        }
    }
}

async fn run() -> Result<ExitCode, serde_json::Error> {
    let cli = Cli::parse();
    let locator = cli
        .project
        .map_or(ProjectLocator::Auto, ProjectLocator::Path);

    match cli.command {
        Command::Author {
            command:
                AuthorCommand::Mcp {
                    server_name,
                    output,
                    transport,
                    tools,
                    force,
                },
        } => execute_author_mcp_command(server_name, output, transport, tools, force, cli.format),
        Command::Analyze {
            command: AnalyzeCommand::Mcp { descriptor },
        } => execute_analyze_mcp_command(descriptor, cli.format),
        Command::Generate {
            command:
                GenerateCommand::Skills {
                    descriptor,
                    output_dir,
                    force,
                },
        } => execute_generate_skills_command(descriptor, output_dir, force, cli.format),
        Command::Validate {
            command: ValidateCommand::Config,
        } => execute_config_command(locator, cli.format, vec!["validate", "config"]),
        Command::Validate {
            command: ValidateCommand::Runtime,
        } => execute_runtime_command(locator, cli.format, vec!["validate", "runtime"]),
        Command::Validate {
            command: ValidateCommand::SessionContext { input },
        } => execute_validate_session_context_command(input, cli.format),
        Command::Inspect {
            command: InspectCommand::Resources,
        } => execute_runtime_command(locator, cli.format, vec!["inspect", "resources"]),
        Command::Inspect {
            command: InspectCommand::SessionContext,
        } => execute_session_context_command(cli.format),
        Command::Local {
            command: LocalCommand::Init { root },
        } => execute_local_init_command(root.root, cli.format),
        Command::Local {
            command:
                LocalCommand::Import {
                    root,
                    input,
                    record_id,
                    imported_at_utc,
                },
        } => execute_local_import_command(root.root, input, record_id, imported_at_utc, cli.format),
        Command::Local {
            command: LocalCommand::List { root, visibility },
        } => execute_local_list_command(root.root, visibility.query_options(), cli.format),
        Command::Local {
            command:
                LocalCommand::Show {
                    root,
                    record_id,
                    visibility,
                },
        } => {
            execute_local_show_command(root.root, record_id, visibility.query_options(), cli.format)
        }
        Command::Local {
            command:
                LocalCommand::Export {
                    root,
                    record_id,
                    output_path,
                    visibility,
                },
        } => execute_local_export_command(
            root.root,
            record_id,
            output_path,
            visibility.query_options(),
            cli.format,
        ),
        Command::Local {
            command:
                LocalCommand::Supersede {
                    root,
                    record_id,
                    superseded_by_record_id,
                },
        } => execute_local_supersede_command(
            root.root,
            record_id,
            superseded_by_record_id,
            cli.format,
        ),
        Command::Local {
            command:
                LocalCommand::Tombstone {
                    root,
                    record_id,
                    tombstoned_at_utc,
                    reason,
                },
        } => execute_local_tombstone_command(
            root.root,
            record_id,
            tombstoned_at_utc,
            reason,
            cli.format,
        ),
        Command::Mermaid {
            command: MermaidCommand::Render { input },
        } => execute_mermaid_render_command(input, cli.format),
        Command::Mermaid {
            command: MermaidCommand::Reverse { input },
        } => execute_mermaid_reverse_command(input, cli.format),
        Command::Mermaid {
            command: MermaidCommand::Narrate { input },
        } => execute_mermaid_narrate_command(input, cli.format),
        Command::Run { dry_run } => execute_run_command(locator, dry_run, cli.format).await,
        Command::Contracts {
            command:
                ContractsCommand::Export {
                    output_path,
                    create_archive,
                    archive_output_path,
                },
        } => execute_contracts_export_command(
            output_path,
            create_archive,
            archive_output_path,
            cli.format,
        ),
    }
}

fn execute_session_context_command(format: OutputFormat) -> Result<ExitCode, serde_json::Error> {
    memory_cli::execute_inspect_command(map_memory_output_format(format))
}

fn execute_validate_session_context_command(
    input: PathBuf,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    memory_cli::execute_validate_command(input, map_memory_output_format(format))
}

fn execute_local_init_command(
    root: Option<PathBuf>,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    memory_cli::execute_init_command(root, map_memory_output_format(format))
}

fn execute_local_import_command(
    root: Option<PathBuf>,
    input: PathBuf,
    record_id: String,
    imported_at_utc: String,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    memory_cli::execute_import_command(
        root,
        input,
        record_id,
        imported_at_utc,
        map_memory_output_format(format),
    )
}

fn execute_local_list_command(
    root: Option<PathBuf>,
    options: LocalMemoryQueryOptions,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    memory_cli::execute_list_command(root, options, map_memory_output_format(format))
}

fn execute_local_show_command(
    root: Option<PathBuf>,
    record_id: String,
    options: LocalMemoryQueryOptions,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    memory_cli::execute_show_command(root, record_id, options, map_memory_output_format(format))
}

fn execute_local_export_command(
    root: Option<PathBuf>,
    record_id: String,
    output_path: Option<PathBuf>,
    options: LocalMemoryQueryOptions,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    memory_cli::execute_export_command(
        root,
        record_id,
        output_path,
        options,
        map_memory_output_format(format),
    )
}

fn execute_local_supersede_command(
    root: Option<PathBuf>,
    record_id: String,
    superseded_by_record_id: String,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    memory_cli::execute_supersede_command(
        root,
        record_id,
        superseded_by_record_id,
        map_memory_output_format(format),
    )
}

fn execute_local_tombstone_command(
    root: Option<PathBuf>,
    record_id: String,
    tombstoned_at_utc: String,
    reason: String,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    memory_cli::execute_tombstone_command(
        root,
        record_id,
        tombstoned_at_utc,
        reason,
        map_memory_output_format(format),
    )
}

fn map_memory_output_format(format: OutputFormat) -> memory_cli::OutputFormat {
    match format {
        OutputFormat::Text => memory_cli::OutputFormat::Text,
        OutputFormat::Json => memory_cli::OutputFormat::Json,
    }
}

fn execute_author_mcp_command(
    server_name: String,
    output: PathBuf,
    transport: CliTransport,
    tools: Vec<String>,
    force: bool,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let tools = match parse_tool_specs(&tools) {
        Ok(tools) => tools,
        Err(message) => {
            return emit_diagnostics(
                format,
                vec!["author", "mcp"],
                vec![Diagnostic::error("CLI-AUTHOR-001", message)],
                json!({}),
                "invalid",
                ExitCode::from(1),
            )
        }
    };

    match author_mcp_descriptor_to_path(
        AuthorMcpDescriptorRequest {
            server_name,
            transport: transport.into(),
            tools,
        },
        &output,
        force,
    ) {
        Ok(result) => {
            match format {
                OutputFormat::Text => print_authored_mcp_text(&result),
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: CLI_SCHEMA_VERSION,
                    command: vec!["author".to_string(), "mcp".to_string()],
                    status: "ok",
                    summary: Summary::default(),
                    data: result,
                    diagnostics: Vec::new(),
                })?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_tooling_error(error, format, vec!["author", "mcp"], json!({})),
    }
}

fn execute_analyze_mcp_command(
    descriptor: PathBuf,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    match analyze_mcp_descriptor_file(&descriptor) {
        Ok(analysis) => {
            match format {
                OutputFormat::Text => print_mcp_analysis_text(&analysis),
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: CLI_SCHEMA_VERSION,
                    command: vec!["analyze".to_string(), "mcp".to_string()],
                    status: "ok",
                    summary: Summary::default(),
                    data: analysis,
                    diagnostics: Vec::new(),
                })?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_tooling_error(error, format, vec!["analyze", "mcp"], json!({})),
    }
}

fn execute_generate_skills_command(
    descriptor: PathBuf,
    output_dir: Option<PathBuf>,
    force: bool,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    match generate_skills_from_descriptor_file(&descriptor, output_dir.as_deref(), force) {
        Ok(result) => {
            match format {
                OutputFormat::Text => print_generated_skills_text(&result),
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: CLI_SCHEMA_VERSION,
                    command: vec!["generate".to_string(), "skills".to_string()],
                    status: "ok",
                    summary: Summary::default(),
                    data: result,
                    diagnostics: Vec::new(),
                })?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_tooling_error(error, format, vec!["generate", "skills"], json!({})),
    }
}

fn execute_mermaid_render_command(
    input: Option<PathBuf>,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let command = vec!["mermaid", "render"];

    let (canonical_json, input_source) = match load_mermaid_input(input) {
        Ok(result) => result,
        Err(error) => {
            return emit_diagnostics(
                format,
                command,
                mermaid_input_load_diagnostics(error),
                json!({}),
                "error",
                ExitCode::from(1),
            )
        }
    };

    match render_from_json_str(&canonical_json) {
        Ok(mermaid) => {
            match format {
                OutputFormat::Text => println!("{mermaid}"),
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: CLI_SCHEMA_VERSION,
                    command: vec!["mermaid".to_string(), "render".to_string()],
                    status: "ok",
                    summary: Summary::default(),
                    data: build_mermaid_render_report(mermaid, &input_source),
                    diagnostics: Vec::new(),
                })?,
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_diagnostics(
            format,
            command,
            mermaid_render_diagnostics(error, &input_source),
            json!({}),
            "invalid",
            ExitCode::from(1),
        ),
    }
}

fn execute_mermaid_reverse_command(
    input: Option<PathBuf>,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let command = vec!["mermaid", "reverse"];

    let (mermaid, input_source) = match load_mermaid_input(input) {
        Ok(result) => result,
        Err(error) => {
            return emit_diagnostics(
                format,
                command,
                mermaid_input_load_diagnostics(error),
                json!({}),
                "error",
                ExitCode::from(1),
            )
        }
    };

    match reverse_from_mermaid_str(&mermaid) {
        Ok(projection) => {
            match format {
                OutputFormat::Text => print_mermaid_projection_text(&projection),
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: CLI_SCHEMA_VERSION,
                    command: vec!["mermaid".to_string(), "reverse".to_string()],
                    status: "ok",
                    summary: Summary::default(),
                    data: build_mermaid_reverse_report(projection, &input_source),
                    diagnostics: Vec::new(),
                })?,
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_diagnostics(
            format,
            command,
            mermaid_reverse_diagnostics(error, &input_source),
            json!({}),
            "invalid",
            ExitCode::from(1),
        ),
    }
}

fn execute_mermaid_narrate_command(
    input: Option<PathBuf>,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let command = vec!["mermaid", "narrate"];

    let (contents, input_source) = match load_mermaid_input(input) {
        Ok(result) => result,
        Err(error) => {
            return emit_diagnostics(
                format,
                command,
                mermaid_input_load_diagnostics(error),
                json!({}),
                "error",
                ExitCode::from(1),
            )
        }
    };

    let input_kind = detect_mermaid_narrate_input_kind(&contents);
    let result = match input_kind {
        MermaidNarrateInputKind::CanonicalJson => narrate_from_json_str(&contents),
        MermaidNarrateInputKind::MermaidFlowchart => narrate_from_mermaid_str(&contents),
    };

    match result {
        Ok((narrative, projection)) => {
            match format {
                OutputFormat::Text => print_mermaid_narrative_text(&narrative),
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: CLI_SCHEMA_VERSION,
                    command: vec!["mermaid".to_string(), "narrate".to_string()],
                    status: "ok",
                    summary: Summary::default(),
                    data: build_mermaid_narrate_report(narrative, projection, &input_source),
                    diagnostics: Vec::new(),
                })?,
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            let diagnostics = match input_kind {
                MermaidNarrateInputKind::CanonicalJson => {
                    mermaid_narrate_canonical_diagnostics(error, &input_source)
                }
                MermaidNarrateInputKind::MermaidFlowchart => {
                    mermaid_narrate_mermaid_diagnostics(error, &input_source)
                }
            };

            emit_diagnostics(
                format,
                command,
                diagnostics,
                json!({}),
                "invalid",
                ExitCode::from(1),
            )
        }
    }
}

fn execute_config_command(
    locator: ProjectLocator,
    format: OutputFormat,
    command: Vec<&str>,
) -> Result<ExitCode, serde_json::Error> {
    match validate_descriptor_set(locator) {
        Ok(inspection) => {
            let summary = Summary::default();
            match format {
                OutputFormat::Text => print_config_text(&inspection),
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: CLI_SCHEMA_VERSION,
                    command: command.into_iter().map(str::to_string).collect(),
                    status: "ok",
                    summary,
                    data: inspection,
                    diagnostics: Vec::new(),
                })?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_error(error, format, command, json!({})),
    }
}

fn execute_runtime_command(
    locator: ProjectLocator,
    format: OutputFormat,
    command: Vec<&str>,
) -> Result<ExitCode, serde_json::Error> {
    match compose_runtime(locator) {
        Ok(catalog) => {
            let summary = Summary::default();
            match format {
                OutputFormat::Text => print_catalog_text(&catalog),
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: CLI_SCHEMA_VERSION,
                    command: command.into_iter().map(str::to_string).collect(),
                    status: "ok",
                    summary,
                    data: catalog,
                    diagnostics: Vec::new(),
                })?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_error(error, format, command, json!({})),
    }
}

async fn execute_run_command(
    locator: ProjectLocator,
    dry_run: bool,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    if !dry_run {
        if format != OutputFormat::Text {
            let diagnostic = Diagnostic::error(
                "CLI-RUN-002",
                "live stdio host mode does not support `--format json`",
            );
            return emit_diagnostics(
                format,
                vec!["run"],
                vec![diagnostic],
                json!({}),
                "error",
                ExitCode::from(2),
            );
        }

        return match serve_stdio(locator).await {
            Ok(()) => Ok(ExitCode::SUCCESS),
            Err(HostError::Core(error)) => emit_error(error, format, vec!["run"], json!({})),
            Err(error) => emit_diagnostics(
                format,
                vec!["run"],
                vec![Diagnostic::error("CLI-RUN-003", error.to_string())],
                json!({}),
                "error",
                ExitCode::from(2),
            ),
        };
    }

    execute_runtime_command(locator, format, vec!["run", "dry-run"])
}

fn execute_contracts_export_command(
    output_path: Option<PathBuf>,
    create_archive: bool,
    archive_output_path: Option<PathBuf>,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    match export_contract_bundle(
        output_path.as_deref(),
        create_archive,
        archive_output_path.as_deref(),
    ) {
        Ok(result) => {
            let summary = Summary::default();
            match format {
                OutputFormat::Text => print_contracts_export_text(&result),
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: CLI_SCHEMA_VERSION,
                    command: vec!["contracts".to_string(), "export".to_string()],
                    status: "ok",
                    summary,
                    data: result,
                    diagnostics: Vec::new(),
                })?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_contracts_error(error, format, vec!["contracts", "export"]),
    }
}

fn load_mermaid_input(
    input: Option<PathBuf>,
) -> Result<(String, MermaidInputSource), MermaidInputLoadError> {
    match input {
        Some(path) => fs::read_to_string(&path)
            .map(|contents| (contents, MermaidInputSource::File(path.clone())))
            .map_err(|source| MermaidInputLoadError::File { path, source }),
        None => {
            let mut contents = String::new();
            io::stdin()
                .read_to_string(&mut contents)
                .map_err(|source| MermaidInputLoadError::Stdin { source })?;
            Ok((contents, MermaidInputSource::Stdin))
        }
    }
}

fn build_mermaid_render_report(
    mermaid: String,
    input_source: &MermaidInputSource,
) -> MermaidRenderReport {
    MermaidRenderReport {
        mermaid,
        input_source: input_source.kind(),
        input_path: input_source.input_path(),
    }
}

fn build_mermaid_reverse_report(
    projection: MermaidWorkflowProjection,
    input_source: &MermaidInputSource,
) -> MermaidReverseReport {
    MermaidReverseReport {
        projection,
        input_source: input_source.kind(),
        input_path: input_source.input_path(),
    }
}

fn build_mermaid_narrate_report(
    narrative: MermaidNarrative,
    projection: MermaidWorkflowProjection,
    input_source: &MermaidInputSource,
) -> MermaidNarrateReport {
    MermaidNarrateReport {
        narrative,
        projection,
        input_source: input_source.kind(),
        input_path: input_source.input_path(),
    }
}

fn mermaid_input_load_diagnostics(error: MermaidInputLoadError) -> Vec<Diagnostic> {
    match error {
        MermaidInputLoadError::File { path, source } => vec![Diagnostic::error(
            "CLI-MERMAID-001",
            format!("failed to read Mermaid render input {}: {source}", path.display()),
        )
        .with_path(path.display().to_string())],
        MermaidInputLoadError::Stdin { source } => vec![Diagnostic::error(
            "CLI-MERMAID-001",
            format!("failed to read Mermaid render input from stdin: {source}"),
        )
        .with_path("<stdin>".to_string())],
    }
}

fn mermaid_render_diagnostics(
    error: MermaidToolError,
    input_source: &MermaidInputSource,
) -> Vec<Diagnostic> {
    mermaid_canonical_diagnostics(
        "render",
        error,
        input_source,
        "supply governed canonical-workflow or canonical-workflow-graph JSON to `elegy mermaid render`",
    )
}

fn mermaid_narrate_canonical_diagnostics(
    error: MermaidToolError,
    input_source: &MermaidInputSource,
) -> Vec<Diagnostic> {
    mermaid_canonical_diagnostics(
        "narrate",
        error,
        input_source,
        "supply governed canonical-workflow or canonical-workflow-graph JSON, or Mermaid `flowchart TD` content, to `elegy mermaid narrate`",
    )
}

fn mermaid_canonical_diagnostics(
    command_name: &str,
    error: MermaidToolError,
    input_source: &MermaidInputSource,
    unsupported_hint: &'static str,
) -> Vec<Diagnostic> {
    let input_location = input_source.display();

    match error {
        MermaidToolError::Json { source } => vec![Diagnostic::error(
            "CLI-MERMAID-002",
            format!(
                "failed to parse Mermaid {command_name} input JSON {}: {source}",
                input_location,
            ),
        )
        .with_path(input_location)],
        MermaidToolError::UnsupportedCanonicalDocument => vec![Diagnostic::error(
            "CLI-MERMAID-003",
            format!(
                "unsupported Mermaid {command_name} input {}; expected canonical workflow or canonical workflow graph JSON",
                input_location,
            ),
        )
        .with_path(input_source.display())
        .with_hint(unsupported_hint)],
        error @ (MermaidToolError::InvalidCanonicalWorkflowGraphReference { .. }
        | MermaidToolError::DuplicateCanonicalWorkflowGraphId { .. }) => {
            vec![Diagnostic::error(
                "CLI-MERMAID-004",
                format!(
                    "canonical workflow graph input is invalid for Mermaid {command_name} {}: {error}",
                    input_location,
                ),
            )
            .with_path(input_source.display())
            .with_hint("ensure the input matches the governed canonical workflow graph contract")]
        }
        MermaidToolError::CanonicalWorkflowGraph { source } => vec![Diagnostic::error(
            "CLI-MERMAID-004",
            format!(
                "canonical workflow graph input is invalid for Mermaid {command_name} {}: {source}",
                input_location,
            ),
        )
        .with_path(input_source.display())
        .with_hint("ensure the input matches the governed canonical workflow graph contract")],
        error @ (MermaidToolError::InvalidCanonicalWorkflowReference { .. }
        | MermaidToolError::DuplicateCanonicalWorkflowId { .. }) => vec![
            Diagnostic::error(
                "CLI-MERMAID-005",
                format!(
                    "canonical workflow input is invalid for Mermaid {command_name} {}: {error}",
                    input_location,
                ),
            )
            .with_path(input_source.display())
            .with_hint("ensure the input matches the governed canonical workflow contract"),
        ],
        MermaidToolError::CanonicalWorkflow { source } => vec![Diagnostic::error(
            "CLI-MERMAID-005",
            format!(
                "canonical workflow input is invalid for Mermaid {command_name} {}: {source}",
                input_location,
            ),
        )
        .with_path(input_source.display())
        .with_hint("ensure the input matches the governed canonical workflow contract")],
        other => vec![Diagnostic::error(
            "CLI-MERMAID-005",
            format!(
                "Mermaid {command_name} input is invalid {}: {other}",
                input_location
            ),
        )
        .with_path(input_source.display())],
    }
}

fn mermaid_reverse_diagnostics(
    error: MermaidToolError,
    input_source: &MermaidInputSource,
) -> Vec<Diagnostic> {
    mermaid_subset_diagnostics(
        "reverse",
        error,
        input_source,
        "supply Mermaid `flowchart TD` content compatible with `elegy mermaid render`; reverse output is a bounded workflow-graph projection, not canonical reconstruction",
    )
}

fn mermaid_narrate_mermaid_diagnostics(
    error: MermaidToolError,
    input_source: &MermaidInputSource,
) -> Vec<Diagnostic> {
    mermaid_subset_diagnostics(
        "narrate",
        error,
        input_source,
        "supply Mermaid `flowchart TD` content compatible with `elegy mermaid render`, or governed canonical workflow JSON, to `elegy mermaid narrate`",
    )
}

fn mermaid_subset_diagnostics(
    command_name: &str,
    error: MermaidToolError,
    input_source: &MermaidInputSource,
    hint: &'static str,
) -> Vec<Diagnostic> {
    let input_location = input_source.display();

    vec![Diagnostic::error(
        "CLI-MERMAID-006",
        format!(
            "Mermaid {command_name} input is invalid {}: {error}",
            input_location
        ),
    )
    .with_path(input_source.display())
    .with_hint(hint)]
}

fn detect_mermaid_narrate_input_kind(input: &str) -> MermaidNarrateInputKind {
    let trimmed = input.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        MermaidNarrateInputKind::CanonicalJson
    } else {
        MermaidNarrateInputKind::MermaidFlowchart
    }
}

fn emit_error<T: Serialize>(
    error: CoreError,
    format: OutputFormat,
    command: Vec<&str>,
    data: T,
) -> Result<ExitCode, serde_json::Error> {
    emit_diagnostics(
        format,
        command,
        error.diagnostics().to_vec(),
        data,
        "invalid",
        ExitCode::from(1),
    )
}

fn emit_contracts_error(
    error: ContractsError,
    format: OutputFormat,
    command: Vec<&str>,
) -> Result<ExitCode, serde_json::Error> {
    emit_diagnostics(
        format,
        command,
        vec![Diagnostic::error("CLI-CONTRACTS-001", error.to_string())],
        json!({}),
        "error",
        ExitCode::from(1),
    )
}

fn emit_tooling_error<T: Serialize>(
    error: ToolingError,
    format: OutputFormat,
    command: Vec<&str>,
    data: T,
) -> Result<ExitCode, serde_json::Error> {
    emit_diagnostics(
        format,
        command,
        tooling_error_diagnostics(error),
        data,
        "invalid",
        ExitCode::from(1),
    )
}

fn emit_diagnostics<T: Serialize>(
    format: OutputFormat,
    command: Vec<&str>,
    diagnostics: Vec<Diagnostic>,
    data: T,
    status: &'static str,
    exit_code: ExitCode,
) -> Result<ExitCode, serde_json::Error> {
    let summary = summarize(&diagnostics);
    match format {
        OutputFormat::Text => print_diagnostics_text(&diagnostics),
        OutputFormat::Json => print_json(&Envelope {
            schema_version: CLI_SCHEMA_VERSION,
            command: command.into_iter().map(str::to_string).collect(),
            status,
            summary,
            data,
            diagnostics,
        })?,
    }
    Ok(exit_code)
}

fn summarize(diagnostics: &[Diagnostic]) -> Summary {
    Summary {
        errors: diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .count(),
        warnings: diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Warning)
            .count(),
    }
}

fn print_config_text(inspection: &ConfigInspection) {
    println!("configuration is valid");
    println!("project: {}", inspection.project_name);
    println!("root config: {}", inspection.root_config);
    println!("descriptor files: {}", inspection.descriptor_files.len());
    println!("resources: {}", inspection.resource_count);
}

fn print_catalog_text(catalog: &Catalog) {
    println!("runtime is valid");
    println!("project: {}", catalog.project_name);
    println!("resources: {}", catalog.resource_count);
    for resource in &catalog.resources {
        println!(
            "- {} [{}] {}",
            resource.uri,
            format_family(resource.family),
            resource.id
        );
    }
}

fn print_session_context_text(inspection: &SessionContextInspection) {
    println!("summary-only session context artifact");
    println!("capability: {}", inspection.capability);
    println!("contract field: {}", inspection.contract_field);
    println!("schema: {}", inspection.schema_file);
    println!("representation: {}", inspection.representation);
    println!(
        "supported scopes: {}",
        inspection.supported_scopes.join(", ")
    );
    println!("consumers: {}", inspection.intended_consumers.join(", "));
    println!("bounded fields: {}", inspection.bounded_fields.join(", "));
    println!(
        "raw transcript persisted: {}",
        inspection.raw_transcript_persisted
    );
    println!(
        "transcript bodies allowed in artifact: {}",
        inspection.transcript_bodies_allowed_in_artifact
    );
}

fn print_validated_session_context_text(report: &SessionContextValidationReport) {
    println!("summary-only session context artifact is valid");
    println!("input: {}", report.input_path);
    println!("artifact kind: {}", report.artifact_kind);
    println!("representation: {}", report.representation);
    println!("scope: {}", report.scope);
    if let Some(request_id) = &report.request_id {
        println!("request id: {request_id}");
    }
    if let Some(run_id) = &report.run_id {
        println!("run id: {run_id}");
    }
    if let Some(captured_at_utc) = &report.captured_at_utc {
        println!("captured at utc: {captured_at_utc}");
    }
    println!("summary length: {}", report.summary_length);
    println!("summary preview: {}", report.summary_preview);
    println!("salient facts: {}", report.salient_facts_count);
    println!(
        "instruction context items: {}",
        report.instruction_context_count
    );
    println!(
        "raw transcript persisted: {}",
        report.raw_transcript_persisted
    );
    println!("read only: {}", report.read_only);
    println!(
        "neutral validation scope: {}",
        report.neutral_validation_scope
    );
    println!("authority posture: {}", report.authority_posture);
    println!("host validation owner: {}", report.host_validation_owner);
    println!("host promotion owner: {}", report.host_promotion_owner);
    println!(
        "host invalidation owner: {}",
        report.host_invalidation_owner
    );
    println!("adapter posture: {}", report.adapter_posture);
}

fn print_local_init_text(report: &LocalInitReport) {
    println!("initialized local non-authoritative artifact root");
    println!("root: {}", report.root_path);
    println!("artifacts: {}", report.artifacts_path);
    println!("state: {}", report.state_path);
    println!("write lock: {}", report.write_lock_path);
    println!("exports: {}", report.exports_path);
    println!("authority posture: {}", report.authority_posture);
    println!("single writer posture: {}", report.single_writer_posture);
    println!("deterministic ordering: {}", report.deterministic_ordering);
}

fn print_local_list_text(report: &LocalListReport) {
    println!("local non-authoritative artifact list");
    println!("root: {}", report.root_path);
    println!("authority posture: {}", report.authority_posture);
    println!("default visibility: {}", report.default_visibility);
    println!("deterministic ordering: {}", report.deterministic_ordering);
    println!("records: {}", report.records.len());
    for record in &report.records {
        println!(
            "- {} [{}] {} | {}",
            record.record_id,
            format_local_lifecycle_state(record.lifecycle_state),
            format_scope(record.scope),
            record.scope_captured_at_record_id
        );
    }
}

fn print_local_record_text(header: &str, report: &LocalRecordReport) {
    println!("{header}");
    println!("root: {}", report.root_path);
    println!("record id: {}", report.record.record_id);
    println!(
        "lifecycle state: {}",
        format_local_lifecycle_state(report.record.local_lifecycle.state)
    );
    println!(
        "scope: {}",
        format_scope(report.record.session_context.scope)
    );
    if let Some(captured_at_utc) = &report.record.provenance.captured_at_utc {
        println!("captured at utc: {captured_at_utc}");
    }
    println!(
        "imported at utc: {}",
        report.record.provenance.imported_at_utc
    );
    println!("artifact: {}", report.artifact_path);
    println!("default export: {}", report.default_export_path);
    println!(
        "summary preview: {}",
        truncate_for_preview(&report.record.session_context.summary)
    );
    if let Some(superseded_by_record_id) = &report.record.local_lifecycle.superseded_by_record_id {
        println!("superseded by local record: {superseded_by_record_id}");
    }
    if let Some(tombstone) = &report.record.local_lifecycle.tombstone {
        println!("tombstoned at utc: {}", tombstone.tombstoned_at_utc);
        println!("tombstone reason: {}", tombstone.reason);
    }
    println!("authority posture: {}", report.authority_posture);
    println!("default visibility: {}", report.default_visibility);
    println!("deterministic ordering: {}", report.deterministic_ordering);
}

fn print_local_export_text(report: &LocalExportReport) {
    println!("exported local non-authoritative summary-only artifact");
    println!("root: {}", report.root_path);
    println!("record id: {}", report.record.record_id);
    println!("output: {}", report.output_path);
    println!(
        "lifecycle state: {}",
        format_local_lifecycle_state(report.record.local_lifecycle.state)
    );
    println!(
        "scope: {}",
        format_scope(report.record.session_context.scope)
    );
    println!("authority posture: {}", report.authority_posture);
    println!("default visibility: {}", report.default_visibility);
    println!("deterministic ordering: {}", report.deterministic_ordering);
}

fn session_context_inspection() -> SessionContextInspection {
    SessionContextInspection {
        capability: "summary-only-session-context-envelope",
        contract_field: "summary-only-session-context-envelope.sessionContext",
        schema_file: "contracts/schemas/summary-only-session-context-envelope.schema.json",
        representation: "summary-only",
        supported_scopes: vec!["run", "session", "workspace"],
        intended_consumers: vec!["instruction-engine", "workspace-bootstrap", "agent-runtime"],
        bounded_fields: vec![
            "summary",
            "salientFacts",
            "instructionContext",
            "rawTranscriptPersisted",
        ],
        raw_transcript_persisted: false,
        transcript_bodies_allowed_in_artifact: false,
    }
}

fn build_session_context_validation_report(
    input: &Path,
    artifact: &SummaryOnlySessionContextEnvelope,
) -> SessionContextValidationReport {
    SessionContextValidationReport {
        input_path: input.display().to_string(),
        artifact_kind: SUMMARY_ONLY_SESSION_CONTEXT_ARTIFACT_KIND,
        representation: SUMMARY_ONLY_REPRESENTATION,
        scope: format_scope(artifact.session_context.scope),
        request_id: artifact.request_id.clone(),
        run_id: artifact.run_id.clone(),
        captured_at_utc: artifact.captured_at_utc.clone(),
        summary_length: artifact.session_context.summary.chars().count(),
        summary_preview: truncate_for_preview(&artifact.session_context.summary),
        salient_facts_count: artifact.session_context.salient_facts.len(),
        instruction_context_count: artifact.session_context.instruction_context.len(),
        raw_transcript_persisted: artifact.session_context.raw_transcript_persisted,
        read_only: true,
        neutral_validation_scope: SESSION_CONTEXT_NEUTRAL_VALIDATION_SCOPE,
        authority_posture: SESSION_CONTEXT_AUTHORITY_POSTURE,
        host_validation_owner: SESSION_CONTEXT_HOST_OWNER,
        host_promotion_owner: SESSION_CONTEXT_HOST_OWNER,
        host_invalidation_owner: SESSION_CONTEXT_HOST_OWNER,
        adapter_posture: SESSION_CONTEXT_ADAPTER_POSTURE,
    }
}

fn build_local_init_report(paths: &LocalMemoryPaths) -> LocalInitReport {
    LocalInitReport {
        root_path: paths.root.display().to_string(),
        artifacts_path: paths.artifacts_dir.display().to_string(),
        state_path: paths.state_dir.display().to_string(),
        write_lock_path: paths.write_lock_path.display().to_string(),
        exports_path: paths.exports_dir.display().to_string(),
        authority_posture: LOCAL_MEMORY_AUTHORITY_POSTURE,
        single_writer_posture: LOCAL_MEMORY_SINGLE_WRITER_POSTURE,
        deterministic_ordering: LOCAL_MEMORY_DETERMINISTIC_ORDERING,
    }
}

fn build_local_list_report(
    root: &Path,
    options: &LocalMemoryQueryOptions,
    records: Vec<LocalMemoryCatalogEntry>,
) -> LocalListReport {
    LocalListReport {
        root_path: root.display().to_string(),
        authority_posture: LOCAL_MEMORY_AUTHORITY_POSTURE,
        default_visibility: format!(
            "{}; {}",
            options.default_filter_label(),
            LOCAL_DEFAULT_VISIBILITY_POSTURE
        ),
        deterministic_ordering: LOCAL_MEMORY_DETERMINISTIC_ORDERING,
        records,
    }
}

fn build_local_record_report(
    root: &Path,
    stored: &LocalMemoryStoredRecord,
    options: &LocalMemoryQueryOptions,
) -> LocalRecordReport {
    let store = LocalMemoryStore::new(root);
    LocalRecordReport {
        root_path: root.display().to_string(),
        artifact_path: stored.artifact_path.display().to_string(),
        default_export_path: store
            .paths()
            .exports_dir
            .join(format!(
                "{}.summary-only-session-context-envelope.json",
                sanitize_record_id_for_cli_path(&stored.record.record_id)
            ))
            .display()
            .to_string(),
        authority_posture: LOCAL_MEMORY_AUTHORITY_POSTURE,
        default_visibility: format!(
            "{}; {}",
            options.default_filter_label(),
            LOCAL_DEFAULT_VISIBILITY_POSTURE
        ),
        deterministic_ordering: LOCAL_MEMORY_DETERMINISTIC_ORDERING,
        record: stored.record.clone(),
    }
}

fn build_local_export_report(
    root: &Path,
    result: &LocalMemoryExportResult,
    options: &LocalMemoryQueryOptions,
) -> LocalExportReport {
    LocalExportReport {
        root_path: root.display().to_string(),
        output_path: result.output_path.display().to_string(),
        authority_posture: LOCAL_MEMORY_AUTHORITY_POSTURE,
        default_visibility: format!(
            "{}; {}",
            options.default_filter_label(),
            LOCAL_DEFAULT_VISIBILITY_POSTURE
        ),
        deterministic_ordering: LOCAL_MEMORY_DETERMINISTIC_ORDERING,
        record: result.record.clone(),
        exported_envelope: result.exported_envelope.clone(),
    }
}

fn resolve_local_root(root: Option<PathBuf>) -> PathBuf {
    root.unwrap_or_else(|| PathBuf::from(LOCAL_DEFAULT_ROOT_DIR))
}

fn format_scope(scope: SessionContextScope) -> &'static str {
    match scope {
        SessionContextScope::Run => "run",
        SessionContextScope::Session => "session",
        SessionContextScope::Workspace => "workspace",
    }
}

fn format_local_lifecycle_state(state: LocalMemoryLifecycleState) -> &'static str {
    match state {
        LocalMemoryLifecycleState::Active => "active",
        LocalMemoryLifecycleState::Superseded => "superseded",
        LocalMemoryLifecycleState::Tombstoned => "tombstoned",
    }
}

fn truncate_for_preview(value: &str) -> String {
    let char_count = value.chars().count();
    if char_count <= SESSION_CONTEXT_PREVIEW_LIMIT {
        return value.to_string();
    }

    let preview: String = value.chars().take(SESSION_CONTEXT_PREVIEW_LIMIT).collect();
    format!("{preview}...")
}

fn print_authored_mcp_text(result: &AuthoredMcpDescriptor) {
    println!("authored MCP descriptor");
    println!("server: {}", result.descriptor.server_name);
    println!(
        "transport: {}",
        format_transport(result.descriptor.transport)
    );
    println!("tools: {}", result.descriptor.tools.len());
    println!("output: {}", result.output_path);
}

fn print_mcp_analysis_text(analysis: &McpAnalysisResult) {
    println!("analyzed MCP descriptor");
    println!("server: {}", analysis.server_name);
    println!("tools: {}", analysis.analyses.len());
    for tool in &analysis.analyses {
        let schema_status = if tool.has_valid_schema {
            "valid_schema"
        } else {
            "missing_schema"
        };
        println!("- {} [{}]", tool.tool.name, schema_status);
    }
}

fn print_generated_skills_text(result: &GeneratedSkillArtifacts) {
    println!("generated skills from MCP descriptor");
    println!("source: {}", result.source_descriptor);
    println!("server: {}", result.analysis.server_name);
    println!("generated skills: {}", result.generated_skills.len());
    println!("skipped tools: {}", result.skipped_tools.len());
    for skill in &result.generated_skills {
        println!("- {} ({})", skill.effective_id(), skill.effective_name());
    }
    for path in &result.written_files {
        println!("written: {}", path);
    }
}

fn print_contracts_export_text(result: &ContractsBundleExport) {
    println!("contracts bundle exported");
    println!("output: {}", result.output_path.display());
    println!("package version: {}", result.package_version);
    println!("schema version: {}", result.schema_version);
    println!("files: {}", result.files.len());
    if let Some(archive_path) = &result.archive_path {
        println!("archive: {}", archive_path.display());
    }
}

fn print_mermaid_projection_text(projection: &MermaidWorkflowProjection) {
    println!("derived Mermaid workflow projection");
    println!("source: {}", format_mermaid_source_kind(projection.source_kind));
    println!("direction: {}", projection.direction);
    println!("entry nodes: {}", projection.entry_node_ids.len());
    println!("nodes: {}", projection.nodes.len());
    println!("edges: {}", projection.edges.len());
    for node in &projection.nodes {
        println!(
            "- {} [{}] {}",
            node.node_id,
            format_mermaid_node_role(node.node_role),
            node.label
        );
    }
    for edge in &projection.edges {
        let edge_label = edge
            .label
            .as_deref()
            .filter(|label| !label.trim().is_empty())
            .map(|label| format!(" ({label})"))
            .unwrap_or_default();
        println!(
            "- {}: {} -> {}{}",
            format_mermaid_relation(edge.relation),
            edge.from_node_id,
            edge.to_node_id,
            edge_label
        );
    }
}

fn print_mermaid_narrative_text(narrative: &MermaidNarrative) {
    println!("{}", narrative.text);
}

fn print_diagnostics_text(diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        let mut location = String::new();
        if let Some(path) = &diagnostic.location.path {
            location.push_str(path);
        }
        if let Some(field) = &diagnostic.location.field {
            if !location.is_empty() {
                location.push('#');
            }
            location.push_str(field);
        }

        if location.is_empty() {
            eprintln!(
                "{}[{}]: {}",
                severity_label(diagnostic),
                diagnostic.code,
                diagnostic.message
            );
        } else {
            eprintln!(
                "{}[{}] {}: {}",
                severity_label(diagnostic),
                diagnostic.code,
                location,
                diagnostic.message
            );
        }

        if let Some(hint) = &diagnostic.hint {
            eprintln!("  hint: {hint}");
        }
    }
}

fn severity_label(diagnostic: &Diagnostic) -> &'static str {
    match diagnostic.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

fn format_family(family: ResourceFamily) -> &'static str {
    match family {
        ResourceFamily::Static => "static",
        ResourceFamily::Filesystem => "filesystem",
        ResourceFamily::Http => "http",
        ResourceFamily::OpenApi => "open_api",
    }
}

fn format_transport(transport: McpTransportKind) -> &'static str {
    match transport {
        McpTransportKind::Stdio => "stdio",
        McpTransportKind::Http => "http",
    }
}

fn format_mermaid_source_kind(kind: MermaidProjectionSourceKind) -> &'static str {
    match kind {
        MermaidProjectionSourceKind::CanonicalWorkflow => "canonical-workflow",
        MermaidProjectionSourceKind::CanonicalWorkflowGraph => "canonical-workflow-graph",
        MermaidProjectionSourceKind::MermaidFlowchartTd => "mermaid-flowchart-td",
    }
}

fn format_mermaid_node_role(role: MermaidProjectionNodeRole) -> &'static str {
    match role {
        MermaidProjectionNodeRole::Activity => "activity",
        MermaidProjectionNodeRole::Trigger => "trigger",
    }
}

fn format_mermaid_relation(relation: MermaidProjectionEdgeRelation) -> &'static str {
    match relation {
        MermaidProjectionEdgeRelation::Activates => "activates",
        MermaidProjectionEdgeRelation::TransitionsTo => "transitions_to",
    }
}

fn parse_tool_specs(values: &[String]) -> Result<Vec<AuthorMcpToolRequest>, String> {
    values
        .iter()
        .map(|value| {
            let (name, description) = match value.split_once('=') {
                Some((name, description)) => (name.trim(), Some(description.trim())),
                None => (value.trim(), None),
            };

            if name.is_empty() {
                return Err(format!(
                    "tool specification `{value}` is invalid; expected NAME or NAME=DESCRIPTION"
                ));
            }

            Ok(AuthorMcpToolRequest {
                name: name.to_string(),
                description: description
                    .filter(|description| !description.is_empty())
                    .map(str::to_string),
            })
        })
        .collect()
}

fn tooling_error_diagnostics(error: ToolingError) -> Vec<Diagnostic> {
    match error {
        ToolingError::Io {
            operation,
            path,
            source,
        } => vec![Diagnostic::error(
            "CLI-TOOLING-001",
            format!("failed to {operation} {}: {source}", path.display()),
        )
        .with_path(path.display().to_string())],
        ToolingError::Json { path, source } => vec![Diagnostic::error(
            "CLI-TOOLING-002",
            format!("failed to parse JSON in {}: {source}", path.display()),
        )
        .with_path(path.display().to_string())],
        ToolingError::InvalidMcpDescriptor { path, issues } => issues
            .into_iter()
            .map(|issue| {
                Diagnostic::error("CLI-MCP-001", issue)
                    .with_path(path.display().to_string())
                    .with_hint(
                        "author or supply a descriptor that matches the governed MCP contract",
                    )
            })
            .collect(),
        ToolingError::InvalidMcpAnalysis { path, issues } => issues
            .into_iter()
            .map(|issue| {
                Diagnostic::error("CLI-MCP-002", issue)
                    .with_path(path.display().to_string())
                    .with_hint(
                        "ensure the analyzed descriptor produces a governed MCP analysis result",
                    )
            })
            .collect(),
        ToolingError::InvalidSkillDefinition { skill_id, issues } => issues
            .into_iter()
            .map(|issue| {
                Diagnostic::error("CLI-SKILL-001", issue)
                    .with_field(skill_id.clone())
                    .with_hint("generated skill definitions must remain valid governed artifacts")
            })
            .collect(),
        ToolingError::DuplicateSkillId { skill_id } => vec![Diagnostic::error(
            "CLI-SKILL-002",
            format!("duplicate generated skill ID detected: {skill_id}"),
        )],
        ToolingError::OutputExists { path } => vec![Diagnostic::error(
            "CLI-OUTPUT-001",
            format!("output already exists: {}", path.display()),
        )
        .with_path(path.display().to_string())
        .with_hint("pass --force to overwrite generated output")],
    }
}

fn emit_local_store_error<T: Serialize>(
    error: LocalMemoryStoreError,
    format: OutputFormat,
    command: Vec<&str>,
    data: T,
) -> Result<ExitCode, serde_json::Error> {
    emit_diagnostics(
        format,
        command,
        local_store_error_diagnostics(error),
        data,
        "invalid",
        ExitCode::from(1),
    )
}

fn local_store_error_diagnostics(error: LocalMemoryStoreError) -> Vec<Diagnostic> {
    match error {
        LocalMemoryStoreError::RootNotInitialized { root } => vec![Diagnostic::error(
            "CLI-LOCAL-003",
            format!("local artifact root is not initialized: {}", root.display()),
        )
        .with_path(root.display().to_string())
        .with_hint("run `elegy local init --root <path>` before using local artifact commands")],
        LocalMemoryStoreError::ConcurrentWriterRejected { root } => vec![Diagnostic::error(
            "CLI-LOCAL-004",
            format!(
                "concurrent local writer rejected for {}; state/write.lock already exists",
                root.display()
            ),
        )
        .with_path(root.display().to_string())
        .with_hint(
            "local artifact writes assume a single writer; retry after the current writer exits or remove a stale lock intentionally",
        )],
        LocalMemoryStoreError::RecordNotFound { record_id } => vec![Diagnostic::error(
            "CLI-LOCAL-005",
            format!("local record was not found: {record_id}"),
        )
        .with_field(record_id)],
        LocalMemoryStoreError::RecordExcludedByLifecycle { record_id, state } => {
            vec![Diagnostic::error(
                "CLI-LOCAL-006",
                format!(
                    "local record `{record_id}` is `{}` and is hidden by the default active-only filter",
                    format_local_lifecycle_state(state)
                ),
            )
            .with_field(record_id)
            .with_hint(
                "pass --include-superseded or --include-tombstoned when you explicitly want non-active local records",
            )]
        }
        LocalMemoryStoreError::RecordIdConflict { record_id } => vec![Diagnostic::error(
            "CLI-LOCAL-007",
            format!("local record ID collision detected for {record_id}"),
        )
        .with_field(record_id)
        .with_hint(
            "reuse the same record ID only for the same governed artifact contents; choose a different local record ID otherwise",
        )],
        LocalMemoryStoreError::SelfSupersede { record_id } => vec![Diagnostic::error(
            "CLI-LOCAL-008",
            format!("local record `{record_id}` cannot supersede itself"),
        )
        .with_field(record_id)],
        LocalMemoryStoreError::SuccessorRecordNotFound { record_id } => {
            vec![Diagnostic::error(
                "CLI-LOCAL-009",
                format!("successor local record was not found: {record_id}"),
            )
            .with_field(record_id)
            .with_hint("import the successor local record before linking another record to it")]
        }
        LocalMemoryStoreError::Io {
            operation,
            path,
            source,
        } => vec![Diagnostic::error(
            "CLI-LOCAL-010",
            format!("failed to {operation} {}: {source}", path.display()),
        )
        .with_path(path.display().to_string())],
        LocalMemoryStoreError::InvalidArtifactJson { path, source } => vec![Diagnostic::error(
            "CLI-LOCAL-011",
            format!("invalid local governed-memory artifact JSON {}: {source}", path.display()),
        )
        .with_path(path.display().to_string())],
        LocalMemoryStoreError::InvalidJsonSerialization { path, source } => {
            vec![Diagnostic::error(
                "CLI-LOCAL-012",
                format!("failed to serialize local JSON {}: {source}", path.display()),
            )
            .with_path(path.display().to_string())]
        }
        LocalMemoryStoreError::MemoryValidation(error) => vec![Diagnostic::error(
            "CLI-LOCAL-013",
            error.to_string(),
        )
        .with_hint(
            "local artifact management accepts only governed memory shapes and bounded local lifecycle metadata",
        )],
    }
}

fn sanitize_record_id_for_cli_path(record_id: &str) -> String {
    let mut encoded = String::with_capacity(record_id.len());
    for ch in record_id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            encoded.push(ch);
        } else {
            encoded.push('_');
        }
    }
    encoded
}

fn print_json<T: Serialize>(value: &T) -> Result<(), serde_json::Error> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

impl MermaidInputSource {
    fn kind(&self) -> &'static str {
        match self {
            Self::File(_) => "file",
            Self::Stdin => "stdin",
        }
    }

    fn input_path(&self) -> Option<String> {
        match self {
            Self::File(path) => Some(path.display().to_string()),
            Self::Stdin => None,
        }
    }

    fn display(&self) -> String {
        self.input_path().unwrap_or_else(|| "<stdin>".to_string())
    }
}

impl From<CliTransport> for McpTransportKind {
    fn from(value: CliTransport) -> Self {
        match value {
            CliTransport::Stdio => Self::Stdio,
            CliTransport::Http => Self::Http,
        }
    }
}
