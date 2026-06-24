use clap::{Parser, Subcommand};
use elegy_core::*;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "elegy-contracts", about = "Elegy contract and configuration tools")]
struct Cli {
    /// Path to the project root or elegy.toml file
    #[arg(long, default_value = ".")]
    project: PathBuf,

    /// Output machine-readable JSON envelopes
    #[arg(long, default_value_t = false)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Contract bundle operations
    #[command(subcommand)]
    Contracts(ContractsAction),
    /// Validate configurations and runtime
    #[command(subcommand)]
    Validate(ValidateTarget),
    /// Inspect runtime entities
    #[command(subcommand)]
    Inspect(InspectTarget),
}

#[derive(Subcommand)]
enum ContractsAction {
    /// Export the contract bundle to the output directory
    Export {
        /// Output directory for exported contracts (default: artifacts/contracts)
        #[arg(long)]
        output_dir: Option<PathBuf>,
        /// Create a zip archive of the bundle
        #[arg(long, default_value_t = false)]
        archive: bool,
        /// Path for the archive output (default: artifacts/distribution/elegy-contracts-<version>.zip)
        #[arg(long)]
        archive_output: Option<PathBuf>,
    },
    /// Validate contract bundle export and compatibility
    Validate,
}

#[derive(Subcommand)]
enum ValidateTarget {
    /// Validate the descriptor set configuration from elegy.toml
    Config,
    /// Validate runtime composition (load and compose all resources)
    Runtime,
}

#[derive(Subcommand)]
enum InspectTarget {
    /// Inspect loaded resources from the project descriptor set
    Resources,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let ctx = build_cli_machine_context(cli.json, None, "elegy-contracts");
    let cmd_args: Vec<String> = std::env::args().collect();

    let result: Result<Value, (CliFailureKind, String)> = run_command(&cli);

    match result {
        Ok(data) => {
            if cli.json {
                let envelope = build_cli_success_envelope(&ctx, cmd_args, &data);
                println!("{}", serde_json::to_string_pretty(&envelope)?);
            } else {
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
            Ok(())
        }
        Err((kind, msg)) => {
            if cli.json {
                let envelope: CliMachineEnvelope<Value> =
                    build_cli_failure_envelope(&ctx, cmd_args, kind, msg, None);
                println!("{}", serde_json::to_string_pretty(&envelope)?);
            } else {
                eprintln!("Error: {}", msg);
            }
            std::process::exit(1);
        }
    }
}

fn run_command(cli: &Cli) -> Result<Value, (CliFailureKind, String)> {
    let locator = ProjectLocator::Path(cli.project.clone());

    match &cli.command {
        Command::Contracts(action) => match action {
            ContractsAction::Export {
                output_dir,
                archive,
                archive_output,
            } => {
                let export = export_contract_bundle(
                    output_dir.as_deref(),
                    *archive,
                    archive_output.as_deref(),
                )
                .map_err(|e| (CliFailureKind::Runtime, e.to_string()))?;
                serde_json::to_value(&export)
                    .map_err(|e| (CliFailureKind::Runtime, e.to_string()))
            }
            ContractsAction::Validate => {
                // Export to a temp dir to validate the bundle process
                let export = export_contract_bundle(None, false, None)
                    .map_err(|e| (CliFailureKind::Runtime, e.to_string()))?;

                // Try loading the compatibility manifest from the output
                let compat = load_compatibility_manifest_from_dir(&export.output_path);
                let compat_info = match compat {
                    Ok(manifest) => serde_json::json!({
                        "status": "valid",
                        "package": {
                            "name": manifest.package.name,
                            "version": manifest.package.version,
                        },
                        "schemaCount": manifest.schemas.len(),
                        "supplementalFixtures": manifest.supplemental_fixtures.len(),
                    }),
                    Err(e) => serde_json::json!({
                        "status": "warning",
                        "message": format!("compatibility manifest not found: {e}"),
                    }),
                };

                Ok(serde_json::json!({
                    "export": export,
                    "compatibility": compat_info,
                }))
            }
        },
        Command::Validate(target) => match target {
            ValidateTarget::Config => {
                let inspection = validate_descriptor_set(locator)
                    .map_err(|e| (CliFailureKind::Runtime, e.to_string()))?;
                serde_json::to_value(&inspection)
                    .map_err(|e| (CliFailureKind::Runtime, e.to_string()))
            }
            ValidateTarget::Runtime => {
                let catalog = compose_runtime(locator)
                    .map_err(|e| (CliFailureKind::Runtime, e.to_string()))?;
                serde_json::to_value(&catalog)
                    .map_err(|e| (CliFailureKind::Runtime, e.to_string()))
            }
        },
        Command::Inspect(target) => match target {
            InspectTarget::Resources => {
                let loaded = load_descriptor_set(locator)
                    .map_err(|e| (CliFailureKind::Runtime, e.to_string()))?;

                let resource_values: Vec<Value> = loaded
                    .resources
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "id": r.id(),
                            "family": format!("{:?}", r.family()),
                        })
                    })
                    .collect();

                Ok(serde_json::json!({
                    "projectName": loaded.config.project_name,
                    "resourceCount": loaded.resources.len(),
                    "descriptorCount": loaded.descriptors.len(),
                    "resources": resource_values,
                }))
            }
        },
    }
}
