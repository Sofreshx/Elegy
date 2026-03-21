use clap::{Parser, Subcommand, ValueEnum};
use elegy_core::{
    compose_runtime, validate_descriptor_set, Catalog, ConfigInspection, CoreError, Diagnostic,
    McpAnalysisResult, McpTransportKind, ProjectLocator, ResourceFamily, Severity,
    CLI_SCHEMA_VERSION,
};
use elegy_host_mcp::{serve_stdio, HostError};
use elegy_tooling::{
    analyze_mcp_descriptor_file, author_mcp_descriptor_to_path,
    generate_skills_from_descriptor_file, AuthorMcpDescriptorRequest, AuthorMcpToolRequest,
    AuthoredMcpDescriptor, GeneratedSkillArtifacts, ToolingError,
};
use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;
use std::process::ExitCode;

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
    Run {
        #[arg(long)]
        dry_run: bool,
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
enum ValidateCommand {
    Config,
    Runtime,
}

#[derive(Subcommand, Debug)]
enum InspectCommand {
    Resources,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CliTransport {
    Stdio,
    Http,
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
        Command::Inspect {
            command: InspectCommand::Resources,
        } => execute_runtime_command(locator, cli.format, vec!["inspect", "resources"]),
        Command::Run { dry_run } => execute_run_command(locator, dry_run, cli.format).await,
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
        Err(error) => {
            emit_tooling_error(error, format, vec!["generate", "skills"], json!({}))
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

fn print_authored_mcp_text(result: &AuthoredMcpDescriptor) {
    println!("authored MCP descriptor");
    println!("server: {}", result.descriptor.server_name);
    println!("transport: {}", format_transport(result.descriptor.transport));
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

fn print_json<T: Serialize>(value: &T) -> Result<(), serde_json::Error> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

impl From<CliTransport> for McpTransportKind {
    fn from(value: CliTransport) -> Self {
        match value {
            CliTransport::Stdio => Self::Stdio,
            CliTransport::Http => Self::Http,
        }
    }
}
