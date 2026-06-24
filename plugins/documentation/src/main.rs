use clap::{Parser, Subcommand, ValueEnum};
use elegy_core::{
    build_cli_failure_envelope, build_cli_machine_context, build_cli_success_envelope,
    CliFailureKind, CliMachineContext,
};
use elegy_documentation::{
    documentation_check, documentation_export_bundle, documentation_export_llms,
    documentation_init, documentation_inspect, documentation_map, DocumentationCheckResult,
    DocumentationError, DocumentationExportResult, DocumentationInitResult, DocumentationMapResult,
};
use serde::Serialize;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::OnceLock;

const EXIT_CODE_INVALID_INPUT: u8 = 1;
const EXIT_CODE_RUNTIME_FAILURE: u8 = 2;

static CLI_MACHINE_CONTEXT: OnceLock<MachineContext> = OnceLock::new();

#[derive(Parser, Debug)]
#[command(name = "elegy-documentation")]
#[command(about = "Dedicated deterministic documentation inspection CLI for Elegy")]
struct Cli {
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    format: OutputFormat,
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    non_interactive: bool,
    #[arg(long, global = true)]
    correlation_id: Option<String>,
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
    Init {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    Inspect {
        #[arg(long)]
        project: PathBuf,
    },
    Map {
        #[arg(long)]
        project: PathBuf,
    },
    Check {
        #[arg(long)]
        project: PathBuf,
    },
    Export {
        #[command(subcommand)]
        command: ExportCommand,
    },
}

#[derive(Subcommand, Debug)]
enum ExportCommand {
    Llms {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Bundle {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Clone, Debug)]
struct MachineContext {
    format: OutputFormat,
    machine: CliMachineContext,
    command: Vec<String>,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            if let Some(context) = CLI_MACHINE_CONTEXT.get() {
                if context.format == OutputFormat::Json
                    && print_json(&build_cli_failure_envelope::<serde_json::Value, _>(
                        &context.machine,
                        context.command.clone(),
                        CliFailureKind::Runtime,
                        error.to_string(),
                        None,
                    ))
                    .is_ok()
                {
                    return exit_runtime();
                }
            }
            eprintln!("unexpected CLI failure: {error}");
            exit_runtime()
        }
    }
}

fn run() -> Result<ExitCode, serde_json::Error> {
    let cli = Cli::parse();
    let context = MachineContext {
        format: resolve_output_format(cli.json, cli.format),
        machine: build_cli_machine_context(
            cli.non_interactive,
            cli.correlation_id,
            "elegy-documentation",
        ),
        command: vec![command_name(&cli.command).to_string()],
    };
    let _ = CLI_MACHINE_CONTEXT.set(context.clone());

    match cli.command {
        Command::Init { project, dry_run } => execute_init(project, dry_run, &context),
        Command::Inspect { project } => execute_inspect(project, &context),
        Command::Map { project } => execute_map(project, &context),
        Command::Check { project } => execute_check(project, &context),
        Command::Export { command } => match command {
            ExportCommand::Llms { project, output } => {
                execute_export_llms(project, output, &context)
            }
            ExportCommand::Bundle { project, output } => {
                execute_export_bundle(project, output, &context)
            }
        },
    }
}

fn execute_init(
    project: PathBuf,
    dry_run: bool,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    match documentation_init(&project, dry_run) {
        Ok(result) => emit_result(vec!["init"], result, context),
        Err(error) => emit_error(vec!["init"], error, context),
    }
}

fn execute_inspect(
    project: PathBuf,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    match documentation_inspect(&project) {
        Ok(result) => emit_map_result(vec!["inspect"], result, context),
        Err(error) => emit_error(vec!["inspect"], error, context),
    }
}

fn execute_map(project: PathBuf, context: &MachineContext) -> Result<ExitCode, serde_json::Error> {
    match documentation_map(&project) {
        Ok(result) => emit_map_result(vec!["map"], result, context),
        Err(error) => emit_error(vec!["map"], error, context),
    }
}

fn execute_check(
    project: PathBuf,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    match documentation_check(&project) {
        Ok(result) => emit_check_result(vec!["check"], result, context),
        Err(error) => emit_error(vec!["check"], error, context),
    }
}

fn execute_export_llms(
    project: PathBuf,
    output: PathBuf,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    match documentation_export_llms(&project, &output) {
        Ok(result) => emit_export_result(vec!["export", "llms"], result, context),
        Err(error) => emit_error(vec!["export", "llms"], error, context),
    }
}

fn execute_export_bundle(
    project: PathBuf,
    output: PathBuf,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    match documentation_export_bundle(&project, &output) {
        Ok(result) => emit_export_result(vec!["export", "bundle"], result, context),
        Err(error) => emit_error(vec!["export", "bundle"], error, context),
    }
}

fn emit_result(
    command: Vec<&str>,
    result: DocumentationInitResult,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    match context.format {
        OutputFormat::Text => {
            println!("documentation init");
            println!("config: {}", result.config_path);
            println!("created: {}", result.created.len());
            Ok(ExitCode::SUCCESS)
        }
        OutputFormat::Json => {
            print_json(&build_cli_success_envelope(
                &context.machine,
                command,
                result,
            ))?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn emit_map_result(
    command: Vec<&str>,
    result: DocumentationMapResult,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    match context.format {
        OutputFormat::Text => {
            println!("documentation {}", command[0]);
            println!("documents: {}", result.documents.len());
            println!("entrypoints: {}", result.entrypoints.len());
            Ok(ExitCode::SUCCESS)
        }
        OutputFormat::Json => {
            print_json(&build_cli_success_envelope(
                &context.machine,
                command,
                result,
            ))?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn emit_check_result(
    command: Vec<&str>,
    result: DocumentationCheckResult,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    match context.format {
        OutputFormat::Text => {
            println!("documentation check");
            println!("valid: {}", result.valid);
            println!("documents: {}", result.document_count);
            println!("issues: {}", result.issues.len());
        }
        OutputFormat::Json => {
            print_json(&build_cli_success_envelope(
                &context.machine,
                command,
                &result,
            ))?;
        }
    }

    Ok(if result.valid {
        ExitCode::SUCCESS
    } else {
        exit_invalid()
    })
}

fn emit_export_result(
    command: Vec<&str>,
    result: DocumentationExportResult,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    match context.format {
        OutputFormat::Text => {
            println!("documentation export {}", result.export_kind);
            println!("output: {}", result.output_path);
            println!("documents: {}", result.document_count);
            Ok(ExitCode::SUCCESS)
        }
        OutputFormat::Json => {
            print_json(&build_cli_success_envelope(
                &context.machine,
                command,
                result,
            ))?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn emit_error(
    command: Vec<&'static str>,
    error: DocumentationError,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    let kind = match error {
        DocumentationError::InvalidConfig { .. } | DocumentationError::InvalidRequest { .. } => {
            CliFailureKind::InvalidInput
        }
        DocumentationError::Io { .. }
        | DocumentationError::Yaml { .. }
        | DocumentationError::Json { .. } => CliFailureKind::Runtime,
    };
    let exit_code = match kind {
        CliFailureKind::InvalidInput => exit_invalid(),
        _ => exit_runtime(),
    };

    match context.format {
        OutputFormat::Text => eprintln!("Error: {error}"),
        OutputFormat::Json => {
            print_json(&build_cli_failure_envelope::<serde_json::Value, _>(
                &context.machine,
                command,
                kind,
                error.to_string(),
                None,
            ))?;
        }
    }

    Ok(exit_code)
}

fn print_json<T: Serialize>(value: &T) -> Result<(), serde_json::Error> {
    let output = serde_json::to_string_pretty(value)?;
    println!("{output}");
    Ok(())
}

fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Init { .. } => "init",
        Command::Inspect { .. } => "inspect",
        Command::Map { .. } => "map",
        Command::Check { .. } => "check",
        Command::Export { .. } => "export",
    }
}

fn resolve_output_format(json: bool, format: OutputFormat) -> OutputFormat {
    if json {
        OutputFormat::Json
    } else {
        format
    }
}

fn exit_invalid() -> ExitCode {
    ExitCode::from(EXIT_CODE_INVALID_INPUT)
}

fn exit_runtime() -> ExitCode {
    ExitCode::from(EXIT_CODE_RUNTIME_FAILURE)
}
