use clap::{Parser, Subcommand, ValueEnum};
use elegy_core::{
    build_cli_failure_envelope, build_cli_machine_context, build_cli_success_envelope,
    CliFailureKind, CliMachineContext, CliMachineEnvelope,
};
use elegy_skills::{
    validate_skill_directory, validate_skill_file, SkillRegistry, SkillRegistryQuery,
};
use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::OnceLock;

const EXIT_CODE_INVALID_INPUT: u8 = 1;
const EXIT_CODE_RUNTIME_FAILURE: u8 = 2;

static CLI_MACHINE_CONTEXT: OnceLock<MachineContext> = OnceLock::new();

#[derive(Parser, Debug)]
#[command(name = "elegy-skills")]
#[command(about = "Dedicated skill registry CLI for Elegy")]
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

#[derive(Clone, Debug)]
struct MachineContext {
    format: OutputFormat,
    machine: CliMachineContext,
    command: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    List {
        #[arg(long)]
        lifecycle: Option<String>,
        #[arg(long)]
        detail: bool,
    },
    Search {
        #[arg(long)]
        query: String,
        #[arg(long)]
        detail: bool,
    },
    Resolve {
        #[arg(long)]
        query: String,
        #[arg(long)]
        detail: bool,
    },
    Get {
        #[arg(long)]
        skill_id: String,
    },
    Validate {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        dir: Option<PathBuf>,
    },
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
        machine: build_cli_machine_context(cli.non_interactive, cli.correlation_id, "elegy-skills"),
        command: vec![command_name(&cli.command).to_string()],
    };
    let _ = CLI_MACHINE_CONTEXT.set(context.clone());

    match cli.command {
        Command::List { lifecycle, detail } => execute_list(lifecycle, detail, &context),
        Command::Search { query, detail } => execute_search(query, detail, &context),
        Command::Resolve { query, detail } => execute_resolve(query, detail, &context),
        Command::Get { skill_id } => execute_get(skill_id, &context),
        Command::Validate { file, dir } => execute_validate(file, dir, &context),
    }
}

fn execute_list(
    lifecycle: Option<String>,
    detail: bool,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    let registry = SkillRegistry::builtin()
        .map_err(|error| <serde_json::Error as serde::de::Error>::custom(error.to_string()))?;
    let data = registry.list(&SkillRegistryQuery {
        lifecycle,
        include_detail: detail,
    });

    match context.format {
        OutputFormat::Text => {
            if data.is_empty() {
                println!("No skills found matching the given filters.");
            } else {
                println!("{:<16} {:<32} STATE", "ID", "NAME");
                println!("{}", "-".repeat(55));
                for skill in &data {
                    println!(
                        "{:<16} {:<32} {}",
                        skill.summary.id, skill.summary.name, skill.summary.lifecycle_state
                    );
                }
            }
        }
        OutputFormat::Json => print_json(&build_success_envelope(
            context,
            ["list"],
            json!({
                "skills": data,
                "disclosureLevel": if detail { "detail" } else { "index" }
            }),
        ))?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_search(
    query: String,
    detail: bool,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    let registry = SkillRegistry::builtin()
        .map_err(|error| <serde_json::Error as serde::de::Error>::custom(error.to_string()))?;
    let data = registry.search(&query, detail);

    match context.format {
        OutputFormat::Text => {
            if data.is_empty() {
                println!("No skills matched query: \"{query}\"");
            } else {
                println!("Skills matching \"{query}\":");
                println!();
                println!("{:<16} {:<32} {:<6} MATCHED", "ID", "NAME", "SCORE");
                println!("{}", "-".repeat(72));
                for skill in &data {
                    println!(
                        "{:<16} {:<32} {:<6.2} {}",
                        skill.summary.id,
                        skill.summary.name,
                        skill
                            .match_result
                            .as_ref()
                            .map(|match_result| match_result.score)
                            .unwrap_or(0.0),
                        skill
                            .match_result
                            .as_ref()
                            .map(|match_result| match_result.match_reasons.join(", "))
                            .unwrap_or_default()
                    );
                }
            }
        }
        OutputFormat::Json => print_json(&build_success_envelope(
            context,
            ["search"],
            json!({
                "query": query,
                "results": data,
                "disclosureLevel": if detail { "detail" } else { "index" }
            }),
        ))?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_resolve(
    query: String,
    detail: bool,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    let registry = SkillRegistry::builtin()
        .map_err(|error| <serde_json::Error as serde::de::Error>::custom(error.to_string()))?;
    let result = registry.resolve(&query, detail);

    match context.format {
        OutputFormat::Text => {
            if let Some(skill) = &result.top_skill {
                println!("Top skill: {} ({})", skill.summary.name, skill.summary.id);
            } else {
                println!("No matching skills found.");
            }
        }
        OutputFormat::Json => print_json(&build_success_envelope(context, ["resolve"], result))?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_get(skill_id: String, context: &MachineContext) -> Result<ExitCode, serde_json::Error> {
    let registry = SkillRegistry::builtin()
        .map_err(|error| <serde_json::Error as serde::de::Error>::custom(error.to_string()))?;
    let Some(skill) = registry.skill(&skill_id) else {
        return emit_error(
            context,
            format!("skill '{skill_id}' not found"),
            exit_invalid(),
        );
    };

    match context.format {
        OutputFormat::Text => {
            println!("Skill: {} ({})", skill.summary.name, skill.summary.id);
        }
        OutputFormat::Json => print_json(&build_success_envelope(context, ["get"], skill))?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_validate(
    file: Option<PathBuf>,
    dir: Option<PathBuf>,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    let report = match (file, dir) {
        (Some(file), None) => validate_skill_file(&file)
            .map_err(|error| <serde_json::Error as serde::de::Error>::custom(error.to_string()))?,
        (None, Some(dir)) => validate_skill_directory(&dir)
            .map_err(|error| <serde_json::Error as serde::de::Error>::custom(error.to_string()))?,
        _ => {
            return emit_error(
                context,
                "pass exactly one of --file or --dir".to_string(),
                exit_invalid(),
            )
        }
    };

    let status = if report.valid { "ok" } else { "invalid" };
    let code = if report.valid {
        ExitCode::SUCCESS
    } else {
        exit_invalid()
    };

    match context.format {
        OutputFormat::Text => {
            if report.valid {
                println!("skill registry input is valid");
            } else {
                println!("skill registry input is invalid");
                for issue in &report.issues {
                    println!("- {}", issue.message);
                }
            }
        }
        OutputFormat::Json => print_json(&CliMachineEnvelope {
            status: status.to_string(),
            ..build_cli_success_envelope(&context.machine, ["validate"], report)
        })?,
    }

    Ok(code)
}

fn emit_error(
    context: &MachineContext,
    message: String,
    code: ExitCode,
) -> Result<ExitCode, serde_json::Error> {
    match context.format {
        OutputFormat::Text => eprintln!("{message}"),
        OutputFormat::Json => print_json(&build_cli_failure_envelope::<serde_json::Value, _>(
            &context.machine,
            context.command.clone(),
            if code == exit_invalid() {
                CliFailureKind::InvalidInput
            } else {
                CliFailureKind::Runtime
            },
            message,
            None,
        ))?,
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

fn command_name(command: &Command) -> &'static str {
    match command {
        Command::List { .. } => "list",
        Command::Search { .. } => "search",
        Command::Resolve { .. } => "resolve",
        Command::Get { .. } => "get",
        Command::Validate { .. } => "validate",
    }
}

fn build_success_envelope<T, S>(
    context: &MachineContext,
    command: impl IntoIterator<Item = S>,
    data: T,
) -> CliMachineEnvelope<T>
where
    T: Serialize,
    S: Into<String>,
{
    build_cli_success_envelope(&context.machine, command, data)
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
