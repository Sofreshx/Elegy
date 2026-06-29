use clap::{Parser, Subcommand};
use elegy_plugin_sdk::{
    export_plugin_v1, inspect_plugin_v1, pack_plugin_v1, pack_plugin_v1_with_binary,
    resolve_plugin_root, verify_plugin_v1, PluginArchiveBinary,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod installer;
use installer::{install_from_archive, install_from_url};

#[derive(Parser)]
#[command(name = "elegy-plugin-packaging")]
#[command(about = "Verify, pack, and export Elegy plugin v1 packages")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Verify a plugin package (validates manifest and skills)
    Verify {
        /// Path to plugin directory, .elegy-plugin dir, or plugin.json
        #[arg(long)]
        plugin: PathBuf,
    },
    /// Pack a plugin into a portable zip archive
    Pack {
        /// Path to plugin directory, .elegy-plugin dir, or plugin.json
        #[arg(long)]
        plugin: PathBuf,
        /// Output zip path (default: <plugin-name>-v<version>.plugin.zip)
        #[arg(long)]
        output: Option<PathBuf>,
        /// Optional compiled binary to include in the archive.
        #[arg(long)]
        binary: Option<PathBuf>,
        /// Archive path for the compiled binary (default: bin/<plugin-name>[.exe])
        #[arg(long)]
        binary_name: Option<String>,
    },
    /// Export a plugin for a target host (codex, opencode, claude)
    Export {
        /// Path to plugin directory, .elegy-plugin dir, or plugin.json
        #[arg(long)]
        plugin: PathBuf,
        /// Target host: codex, opencode, or claude
        #[arg(long)]
        host: String,
        /// Output directory
        #[arg(long)]
        output: PathBuf,
        /// Overwrite existing output
        #[arg(long, default_value_t = false)]
        overwrite: bool,
    },
    /// Install a plugin from a local archive or URL
    Install {
        /// Path to plugin archive (.zip)
        #[arg(long, conflicts_with = "url")]
        archive: Option<PathBuf>,
        /// URL to download plugin archive from
        #[arg(long, conflicts_with = "archive")]
        url: Option<String>,
        /// Install root directory (default: ~/.elegy/plugins)
        #[arg(long)]
        install_root: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Verify { plugin } => {
            // verify_plugin_v1 expects the .elegy-plugin directory directly.
            let (repo_root, _manifest) = match resolve_plugin_root(&plugin) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Error: {e}");
                    return ExitCode::from(2);
                }
            };
            let package_dir = repo_root.join(".elegy-plugin");

            match verify_plugin_v1(&package_dir) {
                Ok(result) => {
                    if result.valid {
                        println!("Plugin verified successfully.");
                        println!(
                            "  name: {}  version: {}  skills: {}  mcp: {}",
                            result.plugin_name,
                            result.plugin_version,
                            result.skill_count,
                            result.mcp_server_count,
                        );
                        ExitCode::SUCCESS
                    } else {
                        eprintln!("Plugin verification failed:");
                        for issue in &result.issues {
                            eprintln!("  - {issue}");
                        }
                        ExitCode::from(1)
                    }
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    ExitCode::from(2)
                }
            }
        }
        Command::Pack {
            plugin,
            output,
            binary,
            binary_name,
        } => {
            let output_path = match output {
                Some(p) => p,
                None => {
                    // Resolve root and inspect manifest to build default filename.
                    let (repo_root, _manifest) = match resolve_plugin_root(&plugin) {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("Error: {e}");
                            return ExitCode::from(2);
                        }
                    };
                    let plugin_dir = repo_root.join(".elegy-plugin");
                    let summary = match inspect_plugin_v1(&plugin_dir) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("Error inspecting plugin: {e}");
                            return ExitCode::from(2);
                        }
                    };
                    let name = summary["name"].as_str().unwrap_or("plugin");
                    let version = summary["version"].as_str().unwrap_or("0.0.0");
                    PathBuf::from(format!("{name}-v{version}.plugin.zip"))
                }
            };

            let default_binary_name = match (&binary, &binary_name) {
                (Some(binary_path), None) => binary_path.file_name().map(|name| {
                    format!("bin/{}", name.to_string_lossy())
                }),
                _ => None,
            };
            let binary_archive_name = binary_name.or(default_binary_name);
            let binary_spec = match (&binary, &binary_archive_name) {
                (Some(binary_path), Some(archive_path)) => Some(PluginArchiveBinary {
                    source_path: binary_path.as_path(),
                    archive_path: archive_path.clone(),
                }),
                (Some(_), None) => {
                    eprintln!("Error: --binary requires an archive path or a filename that can be inferred.");
                    return ExitCode::from(2);
                }
                (None, Some(_)) => {
                    eprintln!("Error: --binary-name requires --binary.");
                    return ExitCode::from(2);
                }
                (None, None) => None,
            };

            let pack_result = match binary_spec {
                Some(binary_spec) => pack_plugin_v1_with_binary(&plugin, &output_path, Some(binary_spec)),
                None => {
                    if plugin_requires_binary(&plugin) {
                        eprintln!(
                            "Error: plugin archive for a CLI surface must include --binary."
                        );
                        return ExitCode::from(2);
                    }
                    pack_plugin_v1(&plugin, &output_path)
                }
            };

            match pack_result {
                Ok(archive_path) => {
                    println!("Plugin packed: {archive_path}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    ExitCode::from(2)
                }
            }
        }
        Command::Export {
            plugin,
            host,
            output,
            overwrite,
        } => match export_plugin_v1(&plugin, &host, &output, overwrite) {
            Ok(result) => {
                println!(
                    "Exported {} to {}: {} files",
                    result.plugin_name,
                    host,
                    result.written_files.len()
                );
                for file in &result.written_files {
                    println!("  {file}");
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Error: {e}");
                ExitCode::from(2)
            }
        },
        Command::Install {
            archive,
            url,
            install_root,
        } => {
            let root = install_root.unwrap_or_else(|| {
                let home = dirs_or_manual_home();
                home.join(".elegy").join("plugins")
            });
            match (archive, url) {
                (Some(archive), None) => match install_from_archive(&archive, &root) {
                    Ok(receipt) => {
                        println!(
                            "Installed {} v{} to {}",
                            receipt.name, receipt.version, receipt.install_dir
                        );
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("Error: {e}");
                        ExitCode::from(2)
                    }
                },
                (None, Some(url)) => match install_from_url(&url, &root) {
                    Ok(receipt) => {
                        println!(
                            "Installed {} v{} to {}",
                            receipt.name, receipt.version, receipt.install_dir
                        );
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("Error: {e}");
                        ExitCode::from(2)
                    }
                },
                _ => {
                    eprintln!("Specify --archive or --url");
                    ExitCode::from(1)
                }
            }
        }
    }
}

// Helper for home directory
fn dirs_or_manual_home() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn plugin_requires_binary(plugin: &Path) -> bool {
    let Ok((repo_root, manifest_path)) = resolve_plugin_root(plugin) else {
        return false;
    };

    if !repo_root.join("Cargo.toml").exists() {
        return false;
    }

    let Ok(raw) = std::fs::read_to_string(manifest_path) else {
        return false;
    };
    let Ok(manifest) = serde_json::from_str::<Value>(&raw) else {
        return false;
    };
    matches!(
        manifest.get("name").and_then(Value::as_str),
        Some(name) if name.starts_with("elegy-")
    )
}
