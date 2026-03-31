use clap::{Parser, Subcommand, ValueEnum};
use elegy_tooling::{generate_skills_from_descriptor_file, GeneratedSkillArtifacts};
use serde::Serialize;
use std::path::PathBuf;
use std::process::ExitCode;

const EXIT_CODE_INVALID_INPUT: u8 = 1;
const EXIT_CODE_RUNTIME_FAILURE: u8 = 2;

#[derive(Parser, Debug)]
#[command(name = "elegy-skills")]
#[command(about = "Dedicated skill-generation CLI for Elegy")]
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MachineEnvelope<T>
where
    T: Serialize,
{
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
}

#[derive(Subcommand, Debug)]
enum Command {
    Generate {
        #[arg(long)]
        descriptor: PathBuf,
        #[arg(long)]
        output_dir: Option<PathBuf>,
        #[arg(long)]
        force: bool,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("unexpected CLI failure: {error}");
            exit_runtime()
        }
    }
}

fn run() -> Result<ExitCode, serde_json::Error> {
    let cli = Cli::parse();
    let context = MachineContext {
        format: resolve_output_format(cli.json, cli.format),
        non_interactive: cli.non_interactive,
        correlation_id: cli.correlation_id,
    };
    let Command::Generate {
        descriptor,
        output_dir,
        force,
    } = cli.command;

    execute_generate_command(descriptor, output_dir, force, &context)
}

fn execute_generate_command(
    descriptor: PathBuf,
    output_dir: Option<PathBuf>,
    force: bool,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    match generate_skills_from_descriptor_file(&descriptor, output_dir.as_deref(), force) {
        Ok(result) => {
            match context.format {
                OutputFormat::Text => print_generated_skills_text(&result),
                OutputFormat::Json => {
                    print_json(&build_success_envelope(context, ["generate"], result))?
                }
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_error(context, error.to_string(), exit_invalid()),
    }
}

fn emit_error(
    context: &MachineContext,
    message: String,
    code: ExitCode,
) -> Result<ExitCode, serde_json::Error> {
    match context.format {
        OutputFormat::Text => eprintln!("{message}"),
        OutputFormat::Json => print_json(&MachineEnvelope::<serde_json::Value> {
            correlation_id: context.correlation_id.clone(),
            non_interactive: context.non_interactive,
            command: vec!["generate".to_string()],
            status: "error",
            data: None,
            error: Some(message),
        })?,
    }

    Ok(code)
}

fn resolve_output_format(json: bool, format: OutputFormat) -> OutputFormat {
    if json {
        OutputFormat::Json
    } else {
        format
    }
}

fn build_success_envelope<T, S>(
    context: &MachineContext,
    command: impl IntoIterator<Item = S>,
    data: T,
) -> MachineEnvelope<T>
where
    T: Serialize,
    S: Into<String>,
{
    MachineEnvelope {
        correlation_id: context.correlation_id.clone(),
        non_interactive: context.non_interactive,
        command: command.into_iter().map(Into::into).collect(),
        status: "ok",
        data: Some(data),
        error: None,
    }
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
        println!("written: {path}");
    }
}

fn print_json<T>(value: &T) -> Result<(), serde_json::Error>
where
    T: Serialize,
{
    let text = serde_json::to_string_pretty(value)?;
    println!("{text}");
    Ok(())
}
