use clap::{Parser, Subcommand, ValueEnum};
use elegy_tooling::{generate_skills_from_descriptor_file, GeneratedSkillArtifacts};
use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "elegy-skills")]
#[command(about = "Dedicated skill-generation CLI for Elegy")]
struct Cli {
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
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, serde_json::Error> {
    let cli = Cli::parse();
    let Command::Generate {
        descriptor,
        output_dir,
        force,
    } = cli.command;

    execute_generate_command(descriptor, output_dir, force, cli.format)
}

fn execute_generate_command(
    descriptor: PathBuf,
    output_dir: Option<PathBuf>,
    force: bool,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    match generate_skills_from_descriptor_file(&descriptor, output_dir.as_deref(), force) {
        Ok(result) => {
            match format {
                OutputFormat::Text => print_generated_skills_text(&result),
                OutputFormat::Json => print_json(&json!({
                    "command": ["generate"],
                    "status": "ok",
                    "data": result,
                }))?,
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_error(format, error.to_string(), ExitCode::from(1)),
    }
}

fn emit_error(
    format: OutputFormat,
    message: String,
    code: ExitCode,
) -> Result<ExitCode, serde_json::Error> {
    match format {
        OutputFormat::Text => eprintln!("{message}"),
        OutputFormat::Json => print_json(&json!({
            "command": ["generate"],
            "status": "error",
            "error": message,
        }))?,
    }

    Ok(code)
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
