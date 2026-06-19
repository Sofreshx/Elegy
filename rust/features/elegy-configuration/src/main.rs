use clap::{Parser, Subcommand, ValueEnum};
use elegy_configuration::{
    apply_configuration, list_builtin_configuration_catalog, show_configuration_template,
    verify_configuration, ApplyConfigurationRequest, ConfigurationError,
    VerifyConfigurationRequest,
};
use elegy_contracts::{
    build_cli_failure_envelope, build_cli_machine_context, build_cli_success_envelope,
    CliFailureKind, CliMachineContext, CliMachineEnvelope,
};
use serde::Serialize;
use std::path::PathBuf;
use std::process::ExitCode;

const EXIT_CODE_INVALID_INPUT: u8 = 1;
const EXIT_CODE_RUNTIME_FAILURE: u8 = 2;

#[derive(Parser, Debug)]
#[command(name = "elegy-configuration")]
#[command(about = "Dedicated deterministic configuration CLI for Elegy")]
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
    List,
    Show {
        #[arg(long)]
        package: Option<PathBuf>,
        #[arg(long)]
        template_id: Option<String>,
        #[arg(long)]
        template_path: Option<PathBuf>,
    },
    Apply {
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        package: Option<PathBuf>,
        #[arg(long)]
        template_id: Option<String>,
        #[arg(long)]
        template_path: Option<PathBuf>,
        #[arg(long)]
        profile_id: Option<String>,
        #[arg(long)]
        profile_path: Option<PathBuf>,
        #[arg(long = "binding", value_name = "KEY=VALUE")]
        bindings: Vec<String>,
        #[arg(long)]
        force: bool,
    },
    Verify {
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        package: Option<PathBuf>,
        #[arg(long)]
        template_id: Option<String>,
        #[arg(long)]
        template_path: Option<PathBuf>,
        #[arg(long)]
        profile_id: Option<String>,
        #[arg(long)]
        profile_path: Option<PathBuf>,
        #[arg(long = "binding", value_name = "KEY=VALUE")]
        bindings: Vec<String>,
    },
}

#[derive(Clone, Debug)]
struct MachineContext {
    format: OutputFormat,
    machine: CliMachineContext,
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
        machine: build_cli_machine_context(
            cli.non_interactive,
            cli.correlation_id,
            "elegy-configuration",
        ),
    };

    match cli.command {
        Command::List => execute_list_command(&context),
        Command::Show {
            package,
            template_id,
            template_path,
        } => execute_show_command(package, template_id, template_path, &context),
        Command::Apply {
            target,
            dry_run,
            package,
            template_id,
            template_path,
            profile_id,
            profile_path,
            bindings,
            force,
        } => execute_apply_command(
            target,
            dry_run,
            package,
            template_id,
            template_path,
            profile_id,
            profile_path,
            bindings,
            force,
            &context,
        ),
        Command::Verify {
            target,
            package,
            template_id,
            template_path,
            profile_id,
            profile_path,
            bindings,
        } => execute_verify_command(
            target,
            package,
            template_id,
            template_path,
            profile_id,
            profile_path,
            bindings,
            &context,
        ),
    }
}

fn execute_list_command(context: &MachineContext) -> Result<ExitCode, serde_json::Error> {
    match list_builtin_configuration_catalog() {
        Ok(report) => {
            match context.format {
                OutputFormat::Text => {
                    println!("elegy-configuration catalog");
                    println!("templates: {}", report.template_count);
                    println!("profiles: {}", report.profile_count);
                }
                OutputFormat::Json => {
                    print_json(&build_success_envelope(context, ["list"], report))?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_error(context, ["list"], error.to_string(), exit_invalid()),
    }
}

fn execute_show_command(
    package: Option<PathBuf>,
    template_id: Option<String>,
    template_path: Option<PathBuf>,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    match show_configuration_template(
        package.as_deref(),
        template_id.as_deref(),
        template_path.as_deref(),
    ) {
        Ok(report) => {
            match context.format {
                OutputFormat::Text => {
                    println!("configuration template: {}", report.template.template_id);
                    println!("source: {}", report.source_ref);
                }
                OutputFormat::Json => {
                    print_json(&build_success_envelope(context, ["show"], report))?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_error(context, ["show"], error.to_string(), exit_invalid()),
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_apply_command(
    target: PathBuf,
    dry_run: bool,
    package: Option<PathBuf>,
    template_id: Option<String>,
    template_path: Option<PathBuf>,
    profile_id: Option<String>,
    profile_path: Option<PathBuf>,
    bindings: Vec<String>,
    force: bool,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    let bindings = match parse_bindings(bindings) {
        Ok(bindings) => bindings,
        Err(error) => return emit_error(context, ["apply"], error.to_string(), exit_invalid()),
    };

    match apply_configuration(ApplyConfigurationRequest {
        target_root: target,
        dry_run,
        force,
        bindings,
        package_path: package,
        template_id,
        template_path,
        profile_id,
        profile_path,
    }) {
        Ok(report) => {
            match context.format {
                OutputFormat::Text => {
                    println!(
                        "configuration apply{}",
                        if dry_run { " (dry-run)" } else { "" }
                    );
                    println!("subject: {}", report.subject_id);
                    println!("verified: {}", report.verified);
                }
                OutputFormat::Json => {
                    print_json(&build_success_envelope(context, ["apply"], report.clone()))?;
                }
            }
            Ok(if report.verified {
                ExitCode::SUCCESS
            } else {
                exit_invalid()
            })
        }
        Err(error) => emit_error(context, ["apply"], error.to_string(), exit_invalid()),
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_verify_command(
    target: PathBuf,
    package: Option<PathBuf>,
    template_id: Option<String>,
    template_path: Option<PathBuf>,
    profile_id: Option<String>,
    profile_path: Option<PathBuf>,
    bindings: Vec<String>,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    let bindings = match parse_bindings(bindings) {
        Ok(bindings) => bindings,
        Err(error) => return emit_error(context, ["verify"], error.to_string(), exit_invalid()),
    };

    match verify_configuration(VerifyConfigurationRequest {
        target_root: target,
        bindings,
        package_path: package,
        template_id,
        template_path,
        profile_id,
        profile_path,
    }) {
        Ok(report) => {
            match context.format {
                OutputFormat::Text => {
                    println!("configuration verify");
                    println!("subject: {}", report.subject_id);
                    println!("verified: {}", report.verified);
                }
                OutputFormat::Json => {
                    print_json(&build_success_envelope(context, ["verify"], report.clone()))?;
                }
            }
            Ok(if report.verified {
                ExitCode::SUCCESS
            } else {
                exit_invalid()
            })
        }
        Err(error) => emit_error(context, ["verify"], error.to_string(), exit_invalid()),
    }
}

fn parse_bindings(
    values: Vec<String>,
) -> Result<std::collections::BTreeMap<String, String>, ConfigurationError> {
    let mut bindings = std::collections::BTreeMap::new();
    for value in values {
        let Some((key, binding_value)) = value.split_once('=') else {
            return Err(ConfigurationError::Contracts(format!(
                "binding must use KEY=VALUE syntax: {value}"
            )));
        };
        if key.trim().is_empty() {
            return Err(ConfigurationError::Contracts(
                "binding keys must not be empty".to_string(),
            ));
        }
        bindings.insert(key.trim().to_string(), binding_value.to_string());
    }
    Ok(bindings)
}

fn emit_error<S>(
    context: &MachineContext,
    command: impl IntoIterator<Item = S>,
    message: String,
    code: ExitCode,
) -> Result<ExitCode, serde_json::Error>
where
    S: Into<String>,
{
    match context.format {
        OutputFormat::Text => eprintln!("{message}"),
        OutputFormat::Json => print_json(&build_cli_failure_envelope::<serde_json::Value, _>(
            &context.machine,
            command,
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

fn print_json<T: Serialize>(value: &T) -> Result<(), serde_json::Error> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn exit_invalid() -> ExitCode {
    ExitCode::from(EXIT_CODE_INVALID_INPUT)
}

fn exit_runtime() -> ExitCode {
    ExitCode::from(EXIT_CODE_RUNTIME_FAILURE)
}
