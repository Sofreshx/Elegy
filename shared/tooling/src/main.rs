use clap::{Parser, Subcommand};
use elegy_plugin_sdk::{
    export_plugin_v1_with_codex_mode_and_binary, inspect_plugin_v1, pack_plugin_v1,
    pack_plugin_v1_with_binary, resolve_plugin_root, select_marketplace_artifact,
    validate_elegy_marketplace_v1, validate_elegy_plugin_v1, verify_plugin_v1, CodexProjectionMode,
    ElegyMarketplaceArtifact, ElegyMarketplaceInterface, ElegyMarketplacePlugin,
    ElegyMarketplaceSource, ElegyMarketplaceV1, ElegyPluginV1, PluginArchiveBinary,
    ELEGY_MARKETPLACE_V1_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

mod installer;
use installer::{
    install_from_archive, install_from_archive_with_identity, install_from_url,
    install_from_url_with_metadata, InstallReceipt, InstallReceiptMetadata,
};

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
        /// Emit documented experimental Codex manifest fields
        #[arg(long, default_value_t = false)]
        experimental_codex: bool,
        /// Optional compiled binary to include in the host export.
        #[arg(long)]
        binary: Option<PathBuf>,
        /// Host-relative path for the compiled binary (default: bin/<filename>).
        #[arg(long)]
        binary_name: Option<String>,
    },
    /// Install a plugin from a local archive or URL
    Install {
        /// Path to plugin archive (.zip)
        #[arg(long, conflicts_with = "url")]
        archive: Option<PathBuf>,
        /// URL to download plugin archive from
        #[arg(long, conflicts_with = "archive")]
        url: Option<String>,
        /// URL containing the archive SHA-256 digest
        #[arg(long, requires = "url")]
        checksum_url: Option<String>,
        /// Install root directory (default: ~/.elegy/plugins)
        #[arg(long)]
        install_root: Option<PathBuf>,
    },
    /// Validate, browse, install, and project a static Elegy marketplace
    Marketplace {
        #[command(subcommand)]
        command: MarketplaceCommand,
    },
}

#[derive(Subcommand)]
enum MarketplaceCommand {
    /// Generate the marketplace index from distribution/surfaces.json
    Generate {
        #[arg(long, default_value = ".")]
        project: PathBuf,
        #[arg(long, default_value = ".elegy/marketplace.json")]
        output: PathBuf,
        #[arg(
            long,
            default_value = "https://github.com/Sofreshx/Elegy/releases/download"
        )]
        release_base_url: String,
        #[arg(long, default_value = "main-snapshot")]
        release_tag: String,
        #[arg(long, default_value_t = false)]
        check: bool,
    },
    /// Validate a marketplace and its plugin manifests
    Validate {
        #[arg(long, default_value = ".")]
        source: String,
    },
    /// List marketplace plugins
    List {
        #[arg(long, default_value = ".")]
        source: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Search marketplace names, descriptions, and categories
    Search {
        query: String,
        #[arg(long, default_value = ".")]
        source: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Install a marketplace plugin for the current or selected target
    Install {
        plugin: String,
        #[arg(long, default_value = ".")]
        source: String,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        install_root: Option<PathBuf>,
    },
    /// Report whether installed plugins match the selected marketplace artifacts
    Status {
        #[arg(long, default_value = ".")]
        source: String,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        plugin: Option<String>,
        #[arg(long)]
        install_root: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Update one marketplace plugin after checksum and identity verification
    Update {
        plugin: String,
        #[arg(long, default_value = ".")]
        source: String,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        install_root: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Poll marketplace freshness and emit JSON lines
    Monitor {
        #[arg(long, default_value = ".")]
        source: String,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        plugin: Option<String>,
        #[arg(long)]
        install_root: Option<PathBuf>,
        #[arg(long, default_value_t = 300)]
        interval_seconds: u64,
        #[arg(long, default_value_t = false)]
        jsonl: bool,
    },
    /// Export a local marketplace as a Codex marketplace tree
    ExportCodex {
        #[arg(long, default_value = ".")]
        source: String,
        /// Export only one marketplace plugin by name.
        #[arg(long)]
        plugin: Option<String>,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        target: Option<String>,
        #[arg(long, default_value_t = false)]
        overwrite: bool,
        #[arg(long, default_value_t = false)]
        check: bool,
        /// Resolve marketplace artifacts from this local release asset directory
        #[arg(long)]
        artifact_dir: Option<PathBuf>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DistributionCatalog {
    surfaces: Vec<DistributionSurface>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DistributionSurface {
    name: String,
    kind: String,
    #[serde(default)]
    packaging: Option<String>,
    #[serde(default)]
    plugin_root: Option<String>,
    #[serde(default)]
    artifact_base_url: Option<String>,
    #[serde(default = "default_marketplace_published")]
    marketplace_published: bool,
    #[serde(default = "default_marketplace_category")]
    marketplace_category: String,
    #[serde(default)]
    targets: Vec<String>,
}

#[derive(Debug)]
struct LoadedMarketplace {
    marketplace: ElegyMarketplaceV1,
    plugins: Vec<(ElegyMarketplacePlugin, ElegyPluginV1)>,
    local_root: Option<PathBuf>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MarketplacePluginSummary<'a> {
    name: &'a str,
    version: &'a str,
    description: &'a str,
    category: &'a str,
    targets: Vec<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MarketplaceListOutput<'a> {
    schema_version: &'static str,
    marketplace: &'a str,
    plugins: Vec<MarketplacePluginSummary<'a>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum MarketplaceFreshnessStatus {
    NotInstalled,
    Current,
    Stale,
    MissingArtifact,
    ChecksumUnavailable,
    IdentityMismatch,
    UnsupportedTarget,
    Unknown,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct MarketplaceStatusRecord {
    plugin: String,
    target: String,
    marketplace_version: String,
    installed_version: Option<String>,
    status: MarketplaceFreshnessStatus,
    artifact_sha256: Option<String>,
    installed_sha256: Option<String>,
    capability_digest: Option<String>,
    source: String,
    install_dir: String,
    recommended_command: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MarketplaceStatusOutput {
    schema_version: &'static str,
    marketplace: String,
    records: Vec<MarketplaceStatusRecord>,
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
                (Some(binary_path), None) => binary_path
                    .file_name()
                    .map(|name| format!("bin/{}", name.to_string_lossy())),
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
                Some(binary_spec) => {
                    pack_plugin_v1_with_binary(&plugin, &output_path, Some(binary_spec))
                }
                None => {
                    if plugin_requires_binary(&plugin) {
                        eprintln!("Error: plugin archive for a CLI surface must include --binary.");
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
            experimental_codex,
            binary,
            binary_name,
        } => {
            let default_binary_name = match (&binary, &binary_name) {
                (Some(binary_path), None) => binary_path
                    .file_name()
                    .map(|name| format!("bin/{}", name.to_string_lossy())),
                _ => None,
            };
            let binary_archive_name = binary_name.or(default_binary_name);
            let binary_spec = match (&binary, &binary_archive_name) {
                (Some(binary_path), Some(archive_path)) => Some(PluginArchiveBinary {
                    source_path: binary_path.as_path(),
                    archive_path: archive_path.clone(),
                }),
                (Some(_), None) => {
                    eprintln!(
                        "Error: --binary requires --binary-name when no filename can be inferred."
                    );
                    return ExitCode::from(2);
                }
                (None, Some(_)) => {
                    eprintln!("Error: --binary-name requires --binary.");
                    return ExitCode::from(2);
                }
                (None, None) => None,
            };
            match export_plugin_v1_with_codex_mode_and_binary(
                &plugin,
                &host,
                &output,
                overwrite,
                if experimental_codex {
                    CodexProjectionMode::Experimental
                } else {
                    CodexProjectionMode::Current
                },
                binary_spec,
            ) {
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
            }
        }
        Command::Install {
            archive,
            url,
            checksum_url,
            install_root,
        } => {
            let root = install_root.unwrap_or_else(|| {
                let home = dirs_or_manual_home();
                home.join(".elegy").join("plugins")
            });
            match (archive, url, checksum_url) {
                (Some(archive), None, None) => match install_from_archive(&archive, &root) {
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
                (None, Some(url), Some(checksum_url)) => {
                    match install_from_url(&url, &checksum_url, &root, None, None) {
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
                    }
                }
                _ => {
                    eprintln!("Specify --archive or --url with --checksum-url");
                    ExitCode::from(1)
                }
            }
        }
        Command::Marketplace { command } => run_marketplace_command(command),
    }
}

fn run_marketplace_command(command: MarketplaceCommand) -> ExitCode {
    let result = match command {
        MarketplaceCommand::Generate {
            project,
            output,
            release_base_url,
            release_tag,
            check,
        } => generate_marketplace(&project, &output, &release_base_url, &release_tag, check),
        MarketplaceCommand::Validate { source } => load_marketplace(&source).map(|loaded| {
            println!(
                "Marketplace '{}' is valid ({} plugins).",
                loaded.marketplace.name,
                loaded.plugins.len()
            );
        }),
        MarketplaceCommand::List { source, json } => load_marketplace(&source)
            .and_then(|loaded| print_marketplace_plugins(&loaded, None, json)),
        MarketplaceCommand::Search {
            query,
            source,
            json,
        } => load_marketplace(&source)
            .and_then(|loaded| print_marketplace_plugins(&loaded, Some(&query), json)),
        MarketplaceCommand::Install {
            plugin,
            source,
            target,
            install_root,
        } => install_marketplace_plugin(
            &source,
            &plugin,
            target.as_deref().unwrap_or(current_release_target()),
            install_root.as_deref(),
        ),
        MarketplaceCommand::Status {
            source,
            target,
            plugin,
            install_root,
            json,
        } => print_marketplace_status(
            &source,
            plugin.as_deref(),
            target.as_deref().unwrap_or(current_release_target()),
            install_root.as_deref(),
            json,
        ),
        MarketplaceCommand::Update {
            plugin,
            source,
            target,
            install_root,
            json,
        } => update_marketplace_plugin(
            &source,
            &plugin,
            target.as_deref().unwrap_or(current_release_target()),
            install_root.as_deref(),
            json,
        ),
        MarketplaceCommand::Monitor {
            source,
            target,
            plugin,
            install_root,
            interval_seconds,
            jsonl,
        } => monitor_marketplace_status(
            &source,
            plugin.as_deref(),
            target.as_deref().unwrap_or(current_release_target()),
            install_root.as_deref(),
            interval_seconds,
            jsonl,
        ),
        MarketplaceCommand::ExportCodex {
            source,
            plugin,
            output,
            target,
            overwrite,
            check,
            artifact_dir,
        } => export_codex_marketplace(
            &source,
            plugin.as_deref(),
            &output,
            target.as_deref().unwrap_or(current_release_target()),
            overwrite,
            check,
            artifact_dir.as_deref(),
        ),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("Error: {message}");
            ExitCode::from(2)
        }
    }
}

fn default_marketplace_category() -> String {
    "Developer Tools".to_string()
}

fn default_marketplace_published() -> bool {
    true
}

fn generate_marketplace(
    project: &Path,
    output: &Path,
    release_base_url: &str,
    release_tag: &str,
    check: bool,
) -> Result<(), String> {
    if release_tag.trim().is_empty() || release_tag.contains('/') || release_tag.contains('\\') {
        return Err("release tag must be a non-empty single path segment".to_string());
    }
    let catalog_path = project.join("distribution").join("surfaces.json");
    let catalog_raw = fs::read_to_string(&catalog_path)
        .map_err(|error| format!("read {}: {error}", catalog_path.display()))?;
    let catalog: DistributionCatalog = serde_json::from_str(&catalog_raw)
        .map_err(|error| format!("parse {}: {error}", catalog_path.display()))?;
    let base = release_base_url.trim_end_matches('/');
    let targets = [
        "x86_64-pc-windows-msvc",
        "x86_64-unknown-linux-gnu",
        "aarch64-apple-darwin",
    ];
    let mut plugins = Vec::new();

    for surface in catalog
        .surfaces
        .into_iter()
        .filter(is_plugin_packaged_surface)
    {
        if !surface.marketplace_published {
            continue;
        }
        let plugin_root = surface
            .plugin_root
            .ok_or_else(|| format!("surface '{}' is missing pluginRoot", surface.name))?;
        let manifest_path = project
            .join(&plugin_root)
            .join(".elegy-plugin")
            .join("plugin.json");
        let manifest_raw = fs::read_to_string(&manifest_path)
            .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
        let manifest: ElegyPluginV1 = serde_json::from_str(&manifest_raw)
            .map_err(|error| format!("parse {}: {error}", manifest_path.display()))?;
        let validation = validate_elegy_plugin_v1(&manifest);
        if !validation.is_valid() {
            return Err(format!(
                "invalid {}: {}",
                manifest_path.display(),
                validation.issues.join("; ")
            ));
        }
        if manifest.name != surface.name {
            return Err(format!(
                "surface '{}' does not match plugin manifest name '{}'",
                surface.name, manifest.name
            ));
        }
        let artifact_base = surface
            .artifact_base_url
            .as_deref()
            .unwrap_or(base)
            .trim_end_matches('/');
        let artifact_targets = if surface.kind == "skill-package" {
            vec!["any"]
        } else if !surface.targets.is_empty() {
            surface.targets.iter().map(String::as_str).collect()
        } else {
            targets.to_vec()
        };
        let artifacts = artifact_targets
            .into_iter()
            .map(|target| {
                let file_name = format!("{}-plugin-{target}.zip", surface.name);
                let url = format!("{artifact_base}/{release_tag}/{file_name}");
                ElegyMarketplaceArtifact {
                    target: target.to_string(),
                    checksum_url: format!("{url}.sha256"),
                    url,
                }
            })
            .collect();
        plugins.push(ElegyMarketplacePlugin {
            name: surface.name,
            source: ElegyMarketplaceSource {
                source: "local".to_string(),
                path: format!("./{plugin_root}").replace('\\', "/"),
            },
            category: surface.marketplace_category,
            artifacts,
        });
    }

    let marketplace = ElegyMarketplaceV1 {
        schema_version: ELEGY_MARKETPLACE_V1_SCHEMA_VERSION.to_string(),
        name: "elegy".to_string(),
        interface: Some(ElegyMarketplaceInterface {
            display_name: Some("Elegy".to_string()),
        }),
        plugins,
    };
    let validation = validate_elegy_marketplace_v1(&marketplace);
    if !validation.is_valid() {
        return Err(validation.issues.join("; "));
    }
    let mut expected =
        serde_json::to_string_pretty(&marketplace).map_err(|error| error.to_string())?;
    expected.push('\n');
    let output_path = if output.is_absolute() {
        output.to_path_buf()
    } else {
        project.join(output)
    };
    if check {
        let actual = fs::read_to_string(&output_path)
            .map_err(|error| format!("read {}: {error}", output_path.display()))?;
        if !generated_content_matches(&actual, &expected) {
            return Err(format!(
                "{} is stale; run marketplace generate",
                output_path.display()
            ));
        }
        println!("Marketplace index is current.");
        return Ok(());
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(&output_path, expected)
        .map_err(|error| format!("write {}: {error}", output_path.display()))?;
    println!("Generated {}", output_path.display());
    Ok(())
}

fn is_plugin_packaged_surface(surface: &DistributionSurface) -> bool {
    if surface.packaging.as_deref() != Some("plugin") {
        return false;
    }

    matches!(
        surface.kind.as_str(),
        "bundled-plugin" | "skill-package" | "external-plugin-wrapper" | "cli"
    )
}

fn generated_content_matches(actual: &str, expected: &str) -> bool {
    actual.replace("\r\n", "\n") == expected
}

fn load_marketplace(source: &str) -> Result<LoadedMarketplace, String> {
    if source.starts_with("https://") {
        return load_remote_marketplace(source);
    }
    let root = fs::canonicalize(source)
        .map_err(|error| format!("resolve marketplace root '{source}': {error}"))?;
    let index_path = root.join(".elegy").join("marketplace.json");
    let raw = fs::read_to_string(&index_path)
        .map_err(|error| format!("read {}: {error}", index_path.display()))?;
    let marketplace = parse_marketplace(&raw, &index_path.display().to_string())?;
    let mut plugins = Vec::new();
    for entry in &marketplace.plugins {
        let relative = entry.source.path.trim_start_matches("./");
        let manifest_path = root
            .join(relative)
            .join(".elegy-plugin")
            .join("plugin.json");
        let manifest_path = fs::canonicalize(&manifest_path)
            .map_err(|error| format!("resolve {}: {error}", manifest_path.display()))?;
        if !manifest_path.starts_with(&root) {
            return Err(format!(
                "plugin '{}' source escapes the marketplace root",
                entry.name
            ));
        }
        let manifest_raw = fs::read_to_string(&manifest_path)
            .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
        let manifest = parse_marketplace_plugin_manifest(&entry.name, &manifest_raw)?;
        plugins.push((entry.clone(), manifest));
    }
    Ok(LoadedMarketplace {
        marketplace,
        plugins,
        local_root: Some(root),
    })
}

fn load_remote_marketplace(source: &str) -> Result<LoadedMarketplace, String> {
    let base = format!("{}/", source.trim_end_matches('/'));
    let index_url = format!("{base}.elegy/marketplace.json");
    let raw = get_text(&index_url)?;
    let marketplace = parse_marketplace(&raw, &index_url)?;
    let mut plugins = Vec::new();
    for entry in &marketplace.plugins {
        let relative = entry.source.path.trim_start_matches("./");
        let manifest_url = format!("{base}{relative}/.elegy-plugin/plugin.json");
        let manifest_raw = get_text(&manifest_url)?;
        let manifest = parse_marketplace_plugin_manifest(&entry.name, &manifest_raw)?;
        plugins.push((entry.clone(), manifest));
    }
    Ok(LoadedMarketplace {
        marketplace,
        plugins,
        local_root: None,
    })
}

fn parse_marketplace(raw: &str, source: &str) -> Result<ElegyMarketplaceV1, String> {
    let marketplace: ElegyMarketplaceV1 =
        serde_json::from_str(raw).map_err(|error| format!("parse {source}: {error}"))?;
    let validation = validate_elegy_marketplace_v1(&marketplace);
    if !validation.is_valid() {
        return Err(format!(
            "invalid marketplace {source}: {}",
            validation.issues.join("; ")
        ));
    }
    Ok(marketplace)
}

fn parse_marketplace_plugin_manifest(
    expected_name: &str,
    raw: &str,
) -> Result<ElegyPluginV1, String> {
    let manifest: ElegyPluginV1 = serde_json::from_str(raw)
        .map_err(|error| format!("parse plugin '{expected_name}': {error}"))?;
    let validation = validate_elegy_plugin_v1(&manifest);
    if !validation.is_valid() {
        return Err(format!(
            "invalid plugin '{}': {}",
            expected_name,
            validation.issues.join("; ")
        ));
    }
    if manifest.name != expected_name {
        return Err(format!(
            "marketplace entry '{expected_name}' points to plugin '{}'",
            manifest.name
        ));
    }
    Ok(manifest)
}

fn get_text(url: &str) -> Result<String, String> {
    let response = reqwest::blocking::get(url).map_err(|error| format!("GET {url}: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("GET {url}: HTTP {}", response.status()));
    }
    response
        .text()
        .map_err(|error| format!("read {url}: {error}"))
}

fn print_marketplace_plugins(
    loaded: &LoadedMarketplace,
    query: Option<&str>,
    as_json: bool,
) -> Result<(), String> {
    let query = query.map(str::to_ascii_lowercase);
    let summaries = loaded
        .plugins
        .iter()
        .filter(|(entry, manifest)| {
            query.as_ref().is_none_or(|query| {
                entry.name.to_ascii_lowercase().contains(query)
                    || entry.category.to_ascii_lowercase().contains(query)
                    || manifest.description.to_ascii_lowercase().contains(query)
            })
        })
        .map(|(entry, manifest)| MarketplacePluginSummary {
            name: &entry.name,
            version: &manifest.version,
            description: &manifest.description,
            category: &entry.category,
            targets: entry
                .artifacts
                .iter()
                .map(|artifact| artifact.target.as_str())
                .collect(),
        })
        .collect::<Vec<_>>();
    if as_json {
        let output = MarketplaceListOutput {
            schema_version: "elegy-marketplace-list/v1",
            marketplace: &loaded.marketplace.name,
            plugins: summaries,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&output).map_err(|error| error.to_string())?
        );
    } else {
        for summary in summaries {
            println!(
                "{} v{} [{}] - {}",
                summary.name, summary.version, summary.category, summary.description
            );
        }
    }
    Ok(())
}

fn print_marketplace_status(
    source: &str,
    plugin_name: Option<&str>,
    target: &str,
    install_root: Option<&Path>,
    as_json: bool,
) -> Result<(), String> {
    let loaded = load_marketplace(source)?;
    let root = install_root
        .map(Path::to_path_buf)
        .unwrap_or_else(default_plugin_install_root);
    let records =
        build_marketplace_status_records(&loaded, source, plugin_name, target, &root, &|url| {
            get_text(url)
        })?;
    if as_json {
        let output = MarketplaceStatusOutput {
            schema_version: "elegy-marketplace-status/v1",
            marketplace: loaded.marketplace.name,
            records,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&output).map_err(|error| error.to_string())?
        );
    } else {
        for record in records {
            println!(
                "{} {:?} installed={:?} marketplace={} target={}",
                record.plugin,
                record.status,
                record.installed_version,
                record.marketplace_version,
                record.target
            );
            if let Some(command) = record.recommended_command {
                println!("  fix: {command}");
            }
        }
    }
    Ok(())
}

fn build_marketplace_status_records(
    loaded: &LoadedMarketplace,
    source: &str,
    plugin_name: Option<&str>,
    target: &str,
    install_root: &Path,
    checksum_reader: &dyn Fn(&str) -> Result<String, String>,
) -> Result<Vec<MarketplaceStatusRecord>, String> {
    let selected = loaded
        .plugins
        .iter()
        .filter(|(entry, _)| plugin_name.is_none_or(|plugin_name| entry.name == plugin_name))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(match plugin_name {
            Some(plugin_name) => format!(
                "plugin '{plugin_name}' is not in marketplace '{}'",
                loaded.marketplace.name
            ),
            None => format!(
                "marketplace '{}' contains no plugins",
                loaded.marketplace.name
            ),
        });
    }

    let mut records = Vec::new();
    for (entry, manifest) in selected {
        records.push(build_marketplace_status_record(
            source,
            entry,
            manifest,
            target,
            install_root,
            checksum_reader,
        )?);
    }
    Ok(records)
}

fn build_marketplace_status_record(
    source: &str,
    entry: &ElegyMarketplacePlugin,
    manifest: &ElegyPluginV1,
    target: &str,
    install_root: &Path,
    checksum_reader: &dyn Fn(&str) -> Result<String, String>,
) -> Result<MarketplaceStatusRecord, String> {
    let install_dir = install_root.join(&entry.name);
    let recommended_command = Some(format!(
        "elegy-plugin-packaging marketplace update {} --source {} --target {} --install-root {} --json",
        entry.name,
        source,
        target,
        install_root.display()
    ));

    if !is_supported_marketplace_target(target) {
        return Ok(MarketplaceStatusRecord {
            plugin: entry.name.clone(),
            target: target.to_string(),
            marketplace_version: manifest.version.clone(),
            installed_version: None,
            status: MarketplaceFreshnessStatus::UnsupportedTarget,
            artifact_sha256: None,
            installed_sha256: None,
            capability_digest: None,
            source: source.to_string(),
            install_dir: install_dir.display().to_string(),
            recommended_command,
        });
    }

    let Some(artifact) = select_marketplace_artifact(entry, target) else {
        return Ok(MarketplaceStatusRecord {
            plugin: entry.name.clone(),
            target: target.to_string(),
            marketplace_version: manifest.version.clone(),
            installed_version: installed_plugin_version(&install_dir),
            status: MarketplaceFreshnessStatus::MissingArtifact,
            artifact_sha256: None,
            installed_sha256: installed_receipt(&install_dir)
                .and_then(|receipt| receipt.artifact_sha256),
            capability_digest: installed_capability_digest(&install_dir),
            source: source.to_string(),
            install_dir: install_dir.display().to_string(),
            recommended_command,
        });
    };

    let artifact_sha256 = match checksum_reader(&artifact.checksum_url) {
        Ok(raw) => parse_checksum_text(&raw),
        Err(_) => None,
    };
    if artifact_sha256.is_none() {
        return Ok(MarketplaceStatusRecord {
            plugin: entry.name.clone(),
            target: artifact.target.clone(),
            marketplace_version: manifest.version.clone(),
            installed_version: installed_plugin_version(&install_dir),
            status: MarketplaceFreshnessStatus::ChecksumUnavailable,
            artifact_sha256: None,
            installed_sha256: installed_receipt(&install_dir)
                .and_then(|receipt| receipt.artifact_sha256),
            capability_digest: installed_capability_digest(&install_dir),
            source: source.to_string(),
            install_dir: install_dir.display().to_string(),
            recommended_command,
        });
    }
    let artifact_sha256 = artifact_sha256.expect("checked above");

    let installed_manifest_path = install_dir.join(".elegy-plugin").join("plugin.json");
    if !installed_manifest_path.is_file() {
        return Ok(MarketplaceStatusRecord {
            plugin: entry.name.clone(),
            target: artifact.target.clone(),
            marketplace_version: manifest.version.clone(),
            installed_version: None,
            status: MarketplaceFreshnessStatus::NotInstalled,
            artifact_sha256: Some(artifact_sha256),
            installed_sha256: installed_receipt(&install_dir)
                .and_then(|receipt| receipt.artifact_sha256),
            capability_digest: installed_capability_digest(&install_dir),
            source: source.to_string(),
            install_dir: install_dir.display().to_string(),
            recommended_command,
        });
    }

    let installed_raw = fs::read_to_string(&installed_manifest_path)
        .map_err(|error| format!("read {}: {error}", installed_manifest_path.display()))?;
    let installed_manifest: ElegyPluginV1 = serde_json::from_str(&installed_raw)
        .map_err(|error| format!("parse {}: {error}", installed_manifest_path.display()))?;
    if installed_manifest.name != entry.name {
        return Ok(MarketplaceStatusRecord {
            plugin: entry.name.clone(),
            target: artifact.target.clone(),
            marketplace_version: manifest.version.clone(),
            installed_version: Some(installed_manifest.version),
            status: MarketplaceFreshnessStatus::IdentityMismatch,
            artifact_sha256: Some(artifact_sha256),
            installed_sha256: installed_receipt(&install_dir)
                .and_then(|receipt| receipt.artifact_sha256),
            capability_digest: installed_capability_digest(&install_dir),
            source: source.to_string(),
            install_dir: install_dir.display().to_string(),
            recommended_command,
        });
    }

    let receipt = installed_receipt(&install_dir);
    let installed_sha256 = receipt
        .as_ref()
        .and_then(|receipt| receipt.artifact_sha256.clone());
    let status = if installed_manifest.version == manifest.version
        && installed_sha256.as_deref() == Some(artifact_sha256.as_str())
    {
        MarketplaceFreshnessStatus::Current
    } else {
        MarketplaceFreshnessStatus::Stale
    };

    Ok(MarketplaceStatusRecord {
        plugin: entry.name.clone(),
        target: artifact.target.clone(),
        marketplace_version: manifest.version.clone(),
        installed_version: Some(installed_manifest.version),
        status,
        artifact_sha256: Some(artifact_sha256),
        installed_sha256,
        capability_digest: receipt
            .and_then(|receipt| receipt.capability_digest)
            .or_else(|| installed_capability_digest(&install_dir)),
        source: source.to_string(),
        install_dir: install_dir.display().to_string(),
        recommended_command,
    })
}

fn update_marketplace_plugin(
    source: &str,
    plugin_name: &str,
    target: &str,
    install_root: Option<&Path>,
    as_json: bool,
) -> Result<(), String> {
    let loaded = load_marketplace(source)?;
    let (entry, manifest) = loaded
        .plugins
        .iter()
        .find(|(entry, _)| entry.name == plugin_name)
        .ok_or_else(|| {
            format!(
                "plugin '{plugin_name}' is not in marketplace '{}'",
                loaded.marketplace.name
            )
        })?;
    if !is_supported_marketplace_target(target) {
        return Err(format!("unsupported marketplace target '{target}'"));
    }
    let artifact = select_marketplace_artifact(entry, target).ok_or_else(|| {
        format!(
            "plugin '{}' has no artifact for target '{}'",
            entry.name, target
        )
    })?;
    let root = install_root
        .map(Path::to_path_buf)
        .unwrap_or_else(default_plugin_install_root);
    let staging = tempfile::tempdir().map_err(|error| error.to_string())?;
    let metadata = InstallReceiptMetadata {
        target: Some(artifact.target.clone()),
        marketplace_name: Some(loaded.marketplace.name.clone()),
        marketplace_source: Some(source.to_string()),
        artifact_url: Some(artifact.url.clone()),
        checksum_url: Some(artifact.checksum_url.clone()),
        manifest_version: Some(manifest.version.clone()),
        capability_digest: manifest_capability_digest(&loaded, entry, manifest),
        ..Default::default()
    };
    let mut receipt = install_from_url_with_metadata(
        &artifact.url,
        &artifact.checksum_url,
        staging.path(),
        Some(&entry.name),
        Some(&manifest.version),
        metadata,
    )
    .map_err(|error| error.to_string())?;
    let staged_dir = staging.path().join(&entry.name);
    let final_dir = root.join(&entry.name);
    receipt.install_dir = final_dir.display().to_string();
    write_install_receipt(&staged_dir, &receipt)?;
    publish_staged_plugin(&staged_dir, &final_dir)?;

    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&receipt).map_err(|error| error.to_string())?
        );
    } else {
        println!(
            "Updated {} v{} at {}",
            receipt.name, receipt.version, receipt.install_dir
        );
    }
    Ok(())
}

fn monitor_marketplace_status(
    source: &str,
    plugin_name: Option<&str>,
    target: &str,
    install_root: Option<&Path>,
    interval_seconds: u64,
    jsonl: bool,
) -> Result<(), String> {
    let interval = Duration::from_secs(interval_seconds.max(1));
    loop {
        let loaded = load_marketplace(source)?;
        let root = install_root
            .map(Path::to_path_buf)
            .unwrap_or_else(default_plugin_install_root);
        let records = build_marketplace_status_records(
            &loaded,
            source,
            plugin_name,
            target,
            &root,
            &|url| get_text(url),
        )?;
        if jsonl {
            for record in records {
                println!(
                    "{}",
                    serde_json::to_string(&record).map_err(|error| error.to_string())?
                );
            }
        } else {
            for record in records {
                println!("{} {:?}", record.plugin, record.status);
            }
        }
        thread::sleep(interval);
    }
}

fn is_supported_marketplace_target(target: &str) -> bool {
    matches!(
        target,
        "any" | "x86_64-pc-windows-msvc" | "x86_64-unknown-linux-gnu" | "aarch64-apple-darwin"
    )
}

fn parse_checksum_text(raw: &str) -> Option<String> {
    raw.split_whitespace()
        .next()
        .filter(|value| value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
}

fn installed_receipt(install_dir: &Path) -> Option<InstallReceipt> {
    let raw = fs::read_to_string(install_dir.join("install-receipt.json")).ok()?;
    serde_json::from_str(&raw).ok()
}

fn installed_plugin_version(install_dir: &Path) -> Option<String> {
    installed_receipt(install_dir)
        .map(|receipt| receipt.version)
        .or_else(|| {
            let raw =
                fs::read_to_string(install_dir.join(".elegy-plugin").join("plugin.json")).ok()?;
            serde_json::from_str::<ElegyPluginV1>(&raw)
                .ok()
                .map(|manifest| manifest.version)
        })
}

fn installed_capability_digest(install_dir: &Path) -> Option<String> {
    let raw = fs::read_to_string(install_dir.join(".elegy-plugin").join("plugin.json")).ok()?;
    let manifest: ElegyPluginV1 = serde_json::from_str(&raw).ok()?;
    let catalog_path = manifest
        .capability_catalog
        .as_ref()?
        .path
        .trim_start_matches("./");
    let catalog_raw = fs::read_to_string(install_dir.join(catalog_path)).ok()?;
    serde_json::from_str::<Value>(&catalog_raw)
        .ok()?
        .get("digest")?
        .as_str()
        .map(str::to_string)
}

fn manifest_capability_digest(
    loaded: &LoadedMarketplace,
    entry: &ElegyMarketplacePlugin,
    manifest: &ElegyPluginV1,
) -> Option<String> {
    let catalog = manifest.capability_catalog.as_ref()?;
    let local_root = loaded.local_root.as_ref()?;
    let plugin_root = local_root.join(entry.source.path.trim_start_matches("./"));
    let catalog_raw =
        fs::read_to_string(plugin_root.join(catalog.path.trim_start_matches("./"))).ok()?;
    serde_json::from_str::<Value>(&catalog_raw)
        .ok()?
        .get("digest")?
        .as_str()
        .map(str::to_string)
}

fn write_install_receipt(install_dir: &Path, receipt: &InstallReceipt) -> Result<(), String> {
    let receipt_path = install_dir.join("install-receipt.json");
    let content = serde_json::to_string_pretty(receipt).map_err(|error| error.to_string())?;
    fs::write(&receipt_path, format!("{content}\n"))
        .map_err(|error| format!("write {}: {error}", receipt_path.display()))
}

fn publish_staged_plugin(staged_dir: &Path, final_dir: &Path) -> Result<(), String> {
    if let Some(parent) = final_dir.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let backup_dir = final_dir.with_extension("elegy-update-backup");
    if backup_dir.exists() {
        fs::remove_dir_all(&backup_dir)
            .map_err(|error| format!("remove stale backup {}: {error}", backup_dir.display()))?;
    }
    if final_dir.exists() {
        fs::rename(final_dir, &backup_dir).map_err(|error| {
            format!(
                "move existing install {} to backup {}: {error}",
                final_dir.display(),
                backup_dir.display()
            )
        })?;
    }
    if let Err(error) = fs::rename(staged_dir, final_dir) {
        if backup_dir.exists() {
            let _ = fs::rename(&backup_dir, final_dir);
        }
        return Err(format!(
            "publish staged install {}: {error}",
            final_dir.display()
        ));
    }
    if backup_dir.exists() {
        fs::remove_dir_all(&backup_dir)
            .map_err(|error| format!("remove backup {}: {error}", backup_dir.display()))?;
    }
    Ok(())
}

fn install_marketplace_plugin(
    source: &str,
    plugin_name: &str,
    target: &str,
    install_root: Option<&Path>,
) -> Result<(), String> {
    let loaded = load_marketplace(source)?;
    let (entry, manifest) = loaded
        .plugins
        .iter()
        .find(|(entry, _)| entry.name == plugin_name)
        .ok_or_else(|| {
            format!(
                "plugin '{plugin_name}' is not in marketplace '{}'",
                loaded.marketplace.name
            )
        })?;
    let root = install_root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| dirs_or_manual_home().join(".elegy").join("plugins"));

    let receipt = if let Some(artifact) = select_marketplace_artifact(entry, target) {
        let metadata = InstallReceiptMetadata {
            target: Some(artifact.target.clone()),
            marketplace_name: Some(loaded.marketplace.name.clone()),
            marketplace_source: Some(source.to_string()),
            artifact_url: Some(artifact.url.clone()),
            checksum_url: Some(artifact.checksum_url.clone()),
            manifest_version: Some(manifest.version.clone()),
            capability_digest: manifest_capability_digest(&loaded, entry, manifest),
            ..Default::default()
        };
        install_from_url_with_metadata(
            &artifact.url,
            &artifact.checksum_url,
            &root,
            Some(&entry.name),
            Some(&manifest.version),
            metadata,
        )
        .map_err(|error| error.to_string())?
    } else if !entry.artifacts.is_empty() {
        return Err(format!(
            "plugin '{}' has no artifact for target '{}'",
            entry.name, target
        ));
    } else if let Some(local_root) = loaded.local_root {
        let plugin_root = local_root.join(entry.source.path.trim_start_matches("./"));
        let temp = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
        pack_plugin_v1(&plugin_root, temp.path()).map_err(|error| error.to_string())?;
        install_from_archive(temp.path(), &root).map_err(|error| error.to_string())?
    } else {
        return Err(format!(
            "remote plugin '{}' has no installable artifact",
            entry.name
        ));
    };
    println!(
        "Installed {} v{} to {}",
        receipt.name, receipt.version, receipt.install_dir
    );
    Ok(())
}

fn export_codex_marketplace(
    source: &str,
    plugin_name: Option<&str>,
    output: &Path,
    target: &str,
    overwrite: bool,
    check: bool,
    artifact_dir: Option<&Path>,
) -> Result<(), String> {
    let loaded = load_marketplace(source)?;
    let local_root = loaded
        .local_root
        .ok_or_else(|| "Codex export requires a local marketplace source".to_string())?;
    if output.exists() && !overwrite && !check {
        return Err(format!("output already exists: {}", output.display()));
    }
    let check_staging = if check {
        Some(tempfile::tempdir().map_err(|error| error.to_string())?)
    } else {
        None
    };
    let generation_output = check_staging
        .as_ref()
        .map(|temp| temp.path())
        .unwrap_or(output);
    fs::create_dir_all(generation_output)
        .map_err(|error| format!("create {}: {error}", generation_output.display()))?;
    let artifact_staging = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut codex_entries = Vec::new();
    let selected_plugins = loaded
        .plugins
        .iter()
        .filter(|(entry, _)| plugin_name.is_none_or(|plugin_name| entry.name == plugin_name))
        .collect::<Vec<_>>();
    if selected_plugins.is_empty() {
        return Err(match plugin_name {
            Some(plugin_name) => format!(
                "plugin '{plugin_name}' is not in marketplace '{}'",
                loaded.marketplace.name
            ),
            None => format!(
                "marketplace '{}' contains no plugins",
                loaded.marketplace.name
            ),
        });
    }
    for (entry, manifest) in selected_plugins {
        if !entry.artifacts.is_empty() && select_marketplace_artifact(entry, target).is_none() {
            if plugin_name.is_some() {
                return Err(format!(
                    "plugin '{}' has no artifact for Codex export target '{}'",
                    entry.name, target
                ));
            }
            continue;
        }
        let wrapper_root = local_root.join(entry.source.path.trim_start_matches("./"));
        let plugin_output = generation_output.join("plugins").join(&entry.name);
        let materialized = materialize_marketplace_artifact(
            entry,
            manifest,
            target,
            artifact_staging.path(),
            artifact_dir,
        )?;
        let plugin_root = materialized
            .as_ref()
            .map(|artifact| artifact.plugin_root.as_path())
            .unwrap_or(wrapper_root.as_path());
        let binary_spec = materialized
            .as_ref()
            .and_then(|artifact| artifact.binary.as_ref())
            .map(|binary| PluginArchiveBinary {
                source_path: binary.source_path.as_path(),
                archive_path: binary.archive_path.clone(),
            });
        export_plugin_v1_with_codex_mode_and_binary(
            plugin_root,
            "codex",
            &plugin_output,
            overwrite,
            CodexProjectionMode::Current,
            binary_spec,
        )
        .map_err(|error| error.to_string())?;
        codex_entries.push(json!({
            "name": entry.name,
            "source": {"source": "local", "path": format!("./plugins/{}", entry.name)},
            "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"},
            "category": entry.category
        }));
    }
    let codex_index = json!({
        "name": loaded.marketplace.name,
        "interface": {
            "displayName": loaded.marketplace.interface
                .and_then(|interface| interface.display_name)
                .unwrap_or_else(|| "Elegy".to_string())
        },
        "plugins": codex_entries
    });
    let index_path = generation_output
        .join(".agents")
        .join("plugins")
        .join("marketplace.json");
    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let mut content =
        serde_json::to_string_pretty(&codex_index).map_err(|error| error.to_string())?;
    content.push('\n');
    fs::write(&index_path, content)
        .map_err(|error| format!("write {}: {error}", index_path.display()))?;
    if check {
        compare_directory_trees(generation_output, output)?;
        println!("Codex marketplace projection is current.");
    } else {
        println!("Exported Codex marketplace to {}", output.display());
    }
    Ok(())
}

fn compare_directory_trees(expected_root: &Path, actual_root: &Path) -> Result<(), String> {
    if !actual_root.is_dir() {
        return Err(format!(
            "{} is missing; run marketplace export-codex",
            actual_root.display()
        ));
    }
    let expected = collect_tree_files(expected_root)?;
    let actual = collect_tree_files(actual_root)?;
    if expected != actual {
        return Err(format!(
            "{} is stale; run marketplace export-codex",
            actual_root.display()
        ));
    }
    for relative in expected {
        let expected_bytes = fs::read(expected_root.join(&relative)).map_err(|error| {
            format!("read {}: {error}", expected_root.join(&relative).display())
        })?;
        let actual_bytes = fs::read(actual_root.join(&relative))
            .map_err(|error| format!("read {}: {error}", actual_root.join(&relative).display()))?;
        if expected_bytes != actual_bytes {
            return Err(format!(
                "{} differs; run marketplace export-codex",
                actual_root.join(&relative).display()
            ));
        }
    }
    Ok(())
}

fn collect_tree_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_tree_files_recursive(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_tree_files_recursive(
    root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    for entry in
        fs::read_dir(directory).map_err(|error| format!("read {}: {error}", directory.display()))?
    {
        let entry =
            entry.map_err(|error| format!("read entry in {}: {error}", directory.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("inspect {}: {error}", path.display()))?;
        if file_type.is_dir() {
            collect_tree_files_recursive(root, &path, files)?;
        } else if file_type.is_file() {
            files.push(
                path.strip_prefix(root)
                    .map_err(|_| format!("{} escaped {}", path.display(), root.display()))?
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

struct MaterializedMarketplaceArtifact {
    plugin_root: PathBuf,
    binary: Option<MaterializedMarketplaceBinary>,
}

struct MaterializedMarketplaceBinary {
    source_path: PathBuf,
    archive_path: String,
}

fn install_from_local_marketplace_artifact(
    entry: &ElegyMarketplacePlugin,
    manifest: &ElegyPluginV1,
    artifact: &ElegyMarketplaceArtifact,
    artifact_dir: &Path,
    staging_root: &Path,
) -> Result<(), String> {
    let artifact_name = artifact_url_file_name(&artifact.url).ok_or_else(|| {
        format!(
            "plugin '{}' artifact URL has no file name: {}",
            entry.name, artifact.url
        )
    })?;
    let checksum_name = artifact_url_file_name(&artifact.checksum_url).ok_or_else(|| {
        format!(
            "plugin '{}' checksum URL has no file name: {}",
            entry.name, artifact.checksum_url
        )
    })?;
    let artifact_path = artifact_dir.join(artifact_name);
    let checksum_path = artifact_dir.join(checksum_name);
    let expected_sha = fs::read_to_string(&checksum_path)
        .map_err(|error| format!("read {}: {error}", checksum_path.display()))
        .and_then(|raw| {
            parse_checksum_text(&raw).ok_or_else(|| {
                format!(
                    "{} does not contain a SHA-256 digest",
                    checksum_path.display()
                )
            })
        })?;
    let actual_sha = file_sha256_hex(&artifact_path)?;
    if actual_sha != expected_sha {
        return Err(format!(
            "plugin '{}' local artifact checksum mismatch for target '{}': expected {}, found {}",
            entry.name, artifact.target, expected_sha, actual_sha
        ));
    }
    install_from_archive_with_identity(
        &artifact_path,
        staging_root,
        Some(&entry.name),
        Some(&manifest.version),
    )
    .map_err(|error| {
        format!(
            "plugin '{}' local artifact materialization failed for target '{}': {error}",
            entry.name, artifact.target
        )
    })?;
    Ok(())
}

fn artifact_url_file_name(url: &str) -> Option<&str> {
    url.rsplit('/').next().filter(|name| !name.is_empty())
}

fn file_sha256_hex(path: &Path) -> Result<String, String> {
    let mut file =
        fs::File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn apply_external_wrapper_codex_projection(
    manifest: &ElegyPluginV1,
    installed_root: &Path,
    target: &str,
) -> Result<(), String> {
    let Some(wrapper) = manifest
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.get("elegy.marketplace-wrapper/v1"))
        .and_then(Value::as_object)
    else {
        return Ok(());
    };
    apply_external_wrapper_manifest_metadata(manifest, installed_root)?;
    if !target.contains("windows") {
        return Ok(());
    }
    let Some(windows_binary_name) = wrapper
        .get("windowsBinaryName")
        .and_then(Value::as_str)
        .filter(|name| name.ends_with(".exe") && !name.contains('/') && !name.contains('\\'))
    else {
        return Ok(());
    };

    let bin_root = installed_root.join("bin");
    let target_binary = bin_root.join(windows_binary_name);
    if !target_binary.exists() {
        let source_binary_name = windows_binary_name.trim_end_matches(".exe");
        let source_binary = bin_root.join(source_binary_name);
        if !source_binary.is_file() {
            return Err(format!(
                "plugin '{}' wrapper declares windowsBinaryName '{}', but {} is missing",
                manifest.name,
                windows_binary_name,
                source_binary.display()
            ));
        }
        fs::rename(&source_binary, &target_binary).map_err(|error| {
            format!(
                "rename {} to {}: {error}",
                source_binary.display(),
                target_binary.display()
            )
        })?;
    }

    let Some(rewrites) = wrapper
        .get("windowsMcpCommandRewrites")
        .and_then(Value::as_object)
    else {
        return Ok(());
    };
    let mcp_path = installed_root.join(".mcp.json");
    if !mcp_path.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&mcp_path)
        .map_err(|error| format!("read {}: {error}", mcp_path.display()))?;
    let mut value: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("parse {}: {error}", mcp_path.display()))?;
    if let Some(servers) = value.get_mut("mcpServers").and_then(Value::as_object_mut) {
        for config in servers.values_mut() {
            let Some(command) = config.get("command").and_then(Value::as_str) else {
                continue;
            };
            let Some(next) = rewrites.get(command).and_then(Value::as_str) else {
                continue;
            };
            config["command"] = Value::String(next.to_string());
        }
    }
    let mut rewritten = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
    rewritten.push('\n');
    fs::write(&mcp_path, rewritten)
        .map_err(|error| format!("write {}: {error}", mcp_path.display()))?;
    Ok(())
}

fn apply_external_wrapper_manifest_metadata(
    wrapper_manifest: &ElegyPluginV1,
    installed_root: &Path,
) -> Result<(), String> {
    let manifest_path = installed_root.join(".elegy-plugin").join("plugin.json");
    let raw = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
    let mut installed: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("parse {}: {error}", manifest_path.display()))?;
    let wrapper_value =
        serde_json::to_value(wrapper_manifest).map_err(|error| error.to_string())?;

    for key in ["description", "author", "license", "repository"] {
        if let Some(value) = wrapper_value.get(key) {
            installed[key] = value.clone();
        }
    }
    if let Some(wrapper_extensions) = wrapper_value.get("extensions").and_then(Value::as_object) {
        let installed_extensions = installed
            .as_object_mut()
            .ok_or_else(|| format!("{} must contain a JSON object", manifest_path.display()))?
            .entry("extensions")
            .or_insert_with(|| json!({}));
        let installed_extensions = installed_extensions
            .as_object_mut()
            .ok_or_else(|| format!("{} extensions must be an object", manifest_path.display()))?;
        for (extension_name, wrapper_extension) in wrapper_extensions {
            if extension_name == "codex.plugin/v1" {
                let target_extension = installed_extensions
                    .entry(extension_name.clone())
                    .or_insert_with(|| json!({}));
                let target_extension = target_extension.as_object_mut().ok_or_else(|| {
                    format!(
                        "{} extension '{}' must be an object",
                        manifest_path.display(),
                        extension_name
                    )
                })?;
                if let Some(wrapper_object) = wrapper_extension.as_object() {
                    for (key, value) in wrapper_object {
                        target_extension.insert(key.clone(), value.clone());
                    }
                }
            } else {
                installed_extensions.insert(extension_name.clone(), wrapper_extension.clone());
            }
        }
    }
    let mut serialized =
        serde_json::to_string_pretty(&installed).map_err(|error| error.to_string())?;
    serialized.push('\n');
    fs::write(&manifest_path, serialized)
        .map_err(|error| format!("write {}: {error}", manifest_path.display()))?;
    Ok(())
}

fn materialize_marketplace_artifact(
    entry: &ElegyMarketplacePlugin,
    manifest: &ElegyPluginV1,
    target: &str,
    staging_root: &Path,
    artifact_dir: Option<&Path>,
) -> Result<Option<MaterializedMarketplaceArtifact>, String> {
    if entry.artifacts.is_empty() {
        return Ok(None);
    }
    let artifact = select_marketplace_artifact(entry, target).ok_or_else(|| {
        format!(
            "plugin '{}' has no artifact for Codex export target '{}'",
            entry.name, target
        )
    })?;
    if let Some(artifact_dir) = artifact_dir {
        install_from_local_marketplace_artifact(
            entry,
            manifest,
            artifact,
            artifact_dir,
            staging_root,
        )?;
    } else {
        install_from_url(
            &artifact.url,
            &artifact.checksum_url,
            staging_root,
            Some(&entry.name),
            Some(&manifest.version),
        )
        .map_err(|error| {
            let message = error.to_string();
            if message.contains("checksum HTTP 404") {
                format!(
                    "plugin '{}' is missing the public checksum artifact for target '{}': {}",
                    entry.name, artifact.target, artifact.checksum_url
                )
            } else if message.contains("checksum HTTP") {
                format!(
                    "plugin '{}' checksum download failed for target '{}': {} ({message})",
                    entry.name, artifact.target, artifact.checksum_url
                )
            } else if message.contains("Download failed: HTTP 404") {
                format!(
                    "plugin '{}' is missing the public plugin artifact for target '{}': {}",
                    entry.name, artifact.target, artifact.url
                )
            } else if message.contains("Download failed: HTTP") {
                format!(
                    "plugin '{}' artifact download failed for target '{}': {} ({message})",
                    entry.name, artifact.target, artifact.url
                )
            } else if message.contains("Checksum mismatch") {
                format!(
                    "plugin '{}' checksum verification failed for target '{}': {message}",
                    entry.name, artifact.target
                )
            } else {
                format!(
                    "plugin '{}' artifact materialization failed for target '{}': {message}",
                    entry.name, artifact.target
                )
            }
        })?;
    }
    let installed_root = staging_root.join(&entry.name);
    apply_external_wrapper_codex_projection(manifest, &installed_root, target)?;
    validate_target_mcp_commands(&installed_root, target)?;
    let bin_root = installed_root.join("bin");
    if !bin_root.is_dir() {
        if artifact.target == "any" {
            return Ok(Some(MaterializedMarketplaceArtifact {
                plugin_root: installed_root,
                binary: None,
            }));
        }
        return Err(format!(
            "plugin '{}' artifact for target '{}' contains no bin directory",
            entry.name, target
        ));
    }
    let mut binaries = Vec::new();
    collect_regular_files(&bin_root, &mut binaries)?;
    if binaries.len() != 1 {
        return Err(format!(
            "plugin '{}' artifact for target '{}' must contain exactly one binary, found {}",
            entry.name,
            target,
            binaries.len()
        ));
    }
    let binary_path = binaries.remove(0);
    let archive_path = binary_path
        .strip_prefix(&installed_root)
        .map_err(|_| format!("plugin '{}' binary escaped its install root", entry.name))?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(Some(MaterializedMarketplaceArtifact {
        plugin_root: installed_root,
        binary: Some(MaterializedMarketplaceBinary {
            source_path: binary_path,
            archive_path,
        }),
    }))
}

fn validate_target_mcp_commands(plugin_root: &Path, target: &str) -> Result<(), String> {
    let mcp_path = plugin_root.join(".mcp.json");
    if !mcp_path.is_file() {
        return Ok(());
    }

    let raw = fs::read_to_string(&mcp_path)
        .map_err(|error| format!("read {}: {error}", mcp_path.display()))?;
    let value: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("parse {}: {error}", mcp_path.display()))?;
    let Some(servers_value) = value.get("mcpServers") else {
        return Ok(());
    };
    let servers = servers_value
        .as_object()
        .ok_or_else(|| format!("{} mcpServers must be an object", mcp_path.display()))?;

    for (server_name, config) in servers {
        let Some(command) = config.get("command").and_then(Value::as_str) else {
            continue;
        };
        let Some(relative_command) = command
            .strip_prefix("./")
            .or_else(|| command.strip_prefix(".\\"))
        else {
            continue;
        };
        let normalized_command = relative_command.replace('\\', "/");
        if !normalized_command.starts_with("bin/") {
            continue;
        }

        if target.contains("windows") {
            let lowercase = normalized_command.to_ascii_lowercase();
            let runnable = [".exe", ".cmd", ".bat", ".ps1"]
                .iter()
                .any(|extension| lowercase.ends_with(extension));
            if !runnable {
                return Err(format!(
                    "{} MCP server '{}' command '{}' is not Windows-runnable; use a .exe/.cmd/.bat/.ps1 launcher or a host executable",
                    mcp_path.display(),
                    server_name,
                    command
                ));
            }
        }

        let resolved_command = package_path(plugin_root, &normalized_command);
        if !resolved_command.is_file() {
            return Err(format!(
                "{} MCP server '{}' command '{}' points to missing file {}",
                mcp_path.display(),
                server_name,
                command,
                resolved_command.display()
            ));
        }
    }

    Ok(())
}

fn package_path(root: &Path, path: &str) -> PathBuf {
    let mut resolved = root.to_path_buf();
    for part in path.split('/') {
        resolved.push(part);
    }
    resolved
}

fn collect_regular_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in
        fs::read_dir(directory).map_err(|error| format!("read {}: {error}", directory.display()))?
    {
        let entry =
            entry.map_err(|error| format!("read entry in {}: {error}", directory.display()))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("inspect {}: {error}", entry.path().display()))?;
        if file_type.is_dir() {
            collect_regular_files(&entry.path(), files)?;
        } else if file_type.is_file() {
            files.push(entry.path());
        }
    }
    Ok(())
}

fn current_release_target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        _ => "unsupported",
    }
}

// Helper for home directory
fn dirs_or_manual_home() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn default_plugin_install_root() -> PathBuf {
    dirs_or_manual_home().join(".elegy").join("plugins")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("elegy-tooling-{name}-{nonce}"))
    }

    fn marketplace_fixture(version: &str) -> LoadedMarketplace {
        let entry = ElegyMarketplacePlugin {
            name: "demo-plugin".to_string(),
            source: ElegyMarketplaceSource {
                source: "local".to_string(),
                path: "./plugins/demo-plugin".to_string(),
            },
            category: "Developer Tools".to_string(),
            artifacts: vec![ElegyMarketplaceArtifact {
                target: "x86_64-pc-windows-msvc".to_string(),
                url: "https://example.com/demo-plugin.zip".to_string(),
                checksum_url: "https://example.com/demo-plugin.zip.sha256".to_string(),
            }],
        };
        let manifest = ElegyPluginV1 {
            schema_version: "elegy-plugin/v1".to_string(),
            name: "demo-plugin".to_string(),
            version: version.to_string(),
            description: "Demo plugin".to_string(),
            ..Default::default()
        };
        LoadedMarketplace {
            marketplace: ElegyMarketplaceV1 {
                schema_version: ELEGY_MARKETPLACE_V1_SCHEMA_VERSION.to_string(),
                name: "test-market".to_string(),
                interface: None,
                plugins: vec![entry.clone()],
            },
            plugins: vec![(entry, manifest)],
            local_root: None,
        }
    }

    fn write_installed_plugin(root: &Path, version: &str, artifact_sha256: &str) {
        let install_dir = root.join("demo-plugin");
        fs::create_dir_all(install_dir.join(".elegy-plugin"))
            .expect("create installed manifest dir");
        fs::write(
            install_dir.join(".elegy-plugin").join("plugin.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion": "elegy-plugin/v1",
                "name": "demo-plugin",
                "version": version,
                "description": "Demo plugin"
            }))
            .expect("serialize manifest"),
        )
        .expect("write installed manifest");
        fs::write(
            install_dir.join("install-receipt.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion": "elegy-installer/v1",
                "name": "demo-plugin",
                "version": version,
                "installedAt": "2026-07-09T00:00:00Z",
                "source": "https://example.com/demo-plugin.zip",
                "installDir": install_dir.display().to_string(),
                "artifactSha256": artifact_sha256,
                "files": [".elegy-plugin/plugin.json"]
            }))
            .expect("serialize receipt"),
        )
        .expect("write receipt");
    }

    #[test]
    fn old_install_receipts_parse_without_new_metadata() {
        let raw = r#"{
            "schemaVersion": "elegy-installer/v1",
            "name": "legacy-plugin",
            "version": "0.1.0",
            "installedAt": "2026-07-09T00:00:00Z",
            "source": "legacy.zip",
            "installDir": "/tmp/legacy-plugin",
            "files": [".elegy-plugin/plugin.json"]
        }"#;
        let receipt: InstallReceipt = serde_json::from_str(raw).expect("parse old receipt");
        assert_eq!(receipt.name, "legacy-plugin");
        assert_eq!(receipt.target, None);
        assert_eq!(receipt.artifact_sha256, None);
    }

    #[test]
    fn marketplace_status_reports_not_installed_current_and_stale() {
        let checksum = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let loaded = marketplace_fixture("0.1.0");
        let missing_root = temp_dir("status-not-installed");
        fs::create_dir_all(&missing_root).expect("create root");
        let records = build_marketplace_status_records(
            &loaded,
            ".",
            Some("demo-plugin"),
            "x86_64-pc-windows-msvc",
            &missing_root,
            &|_| Ok(checksum.to_string()),
        )
        .expect("status records");
        assert_eq!(records[0].status, MarketplaceFreshnessStatus::NotInstalled);

        let current_root = temp_dir("status-current");
        fs::create_dir_all(&current_root).expect("create current root");
        write_installed_plugin(&current_root, "0.1.0", checksum);
        let records = build_marketplace_status_records(
            &loaded,
            ".",
            Some("demo-plugin"),
            "x86_64-pc-windows-msvc",
            &current_root,
            &|_| Ok(checksum.to_string()),
        )
        .expect("current status records");
        assert_eq!(records[0].status, MarketplaceFreshnessStatus::Current);

        let stale_root = temp_dir("status-stale");
        fs::create_dir_all(&stale_root).expect("create stale root");
        write_installed_plugin(
            &stale_root,
            "0.1.0",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        );
        let records = build_marketplace_status_records(
            &loaded,
            ".",
            Some("demo-plugin"),
            "x86_64-pc-windows-msvc",
            &stale_root,
            &|_| Ok(checksum.to_string()),
        )
        .expect("stale status records");
        assert_eq!(records[0].status, MarketplaceFreshnessStatus::Stale);

        let _ = fs::remove_dir_all(missing_root);
        let _ = fs::remove_dir_all(current_root);
        let _ = fs::remove_dir_all(stale_root);
    }

    #[test]
    fn marketplace_target_selection_rejects_unsupported_platform() {
        let loaded = marketplace_fixture("0.1.0");
        let entry = &loaded.plugins[0].0;
        assert!(select_marketplace_artifact(entry, "x86_64-pc-windows-msvc").is_some());
        assert!(select_marketplace_artifact(entry, "x86_64-unknown-linux-gnu").is_none());
    }

    #[test]
    fn target_mcp_commands_reject_extensionless_windows_bin_command() {
        let root = temp_dir("windows-extensionless-mcp");
        fs::create_dir_all(root.join("bin")).expect("create bin dir");
        fs::write(
            root.join("bin").join("elegy-opencode-workers.exe"),
            b"binary",
        )
        .expect("write binary");
        fs::write(
            root.join(".mcp.json"),
            r#"{"mcpServers":{"elegy-opencode-workers":{"command":"./bin/elegy-opencode-workers","args":["mcp","serve"]}}}"#,
        )
        .expect("write mcp config");

        let err = validate_target_mcp_commands(&root, "x86_64-pc-windows-msvc").unwrap_err();

        assert!(err.contains("Windows-runnable"), "unexpected error: {err}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn target_mcp_commands_accept_windows_exe_command() {
        let root = temp_dir("windows-exe-mcp");
        fs::create_dir_all(root.join("bin")).expect("create bin dir");
        fs::write(
            root.join("bin").join("elegy-opencode-workers.exe"),
            b"binary",
        )
        .expect("write binary");
        fs::write(
            root.join(".mcp.json"),
            r#"{"mcpServers":{"elegy-opencode-workers":{"command":"./bin/elegy-opencode-workers.exe","args":["mcp","serve"]}}}"#,
        )
        .expect("write mcp config");

        validate_target_mcp_commands(&root, "x86_64-pc-windows-msvc").expect("valid mcp config");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn external_wrapper_projection_declares_windows_binary_and_mcp_rewrite() {
        let root = temp_dir("wrapper-windows-projection");
        fs::create_dir_all(root.join(".elegy-plugin")).expect("create manifest dir");
        fs::create_dir_all(root.join("bin")).expect("create bin dir");
        fs::write(root.join("bin").join("demo-plugin"), b"binary").expect("write binary");
        fs::write(
            root.join(".mcp.json"),
            r#"{"mcpServers":{"demo-plugin":{"command":"./bin/demo-plugin","args":["mcp","serve"]}}}"#,
        )
        .expect("write mcp config");
        fs::write(
            root.join(".elegy-plugin").join("plugin.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion": "elegy-plugin/v1",
                "name": "demo-plugin",
                "version": "0.1.0",
                "description": "Runtime manifest",
                "extensions": {
                    "codex.plugin/v1": {
                        "schemaVersion": "codex.plugin/v1",
                        "mcpServers": "./.mcp.json"
                    }
                }
            }))
            .expect("serialize runtime manifest"),
        )
        .expect("write runtime manifest");
        let mut wrapper = ElegyPluginV1 {
            schema_version: "elegy-plugin/v1".to_string(),
            name: "demo-plugin".to_string(),
            version: "0.1.0".to_string(),
            description: "Wrapper manifest".to_string(),
            ..Default::default()
        };
        wrapper.extensions = Some(
            serde_json::from_value(json!({
                "elegy.marketplace-wrapper/v1": {
                    "schemaVersion": "elegy.marketplace-wrapper/v1",
                    "windowsBinaryName": "demo-plugin.exe",
                    "windowsMcpCommandRewrites": {
                        "./bin/demo-plugin": "./bin/demo-plugin.exe"
                    }
                },
                "codex.plugin/v1": {
                    "schemaVersion": "codex.plugin/v1",
                    "interface": {
                        "displayName": "Demo",
                        "shortDescription": "Demo",
                        "longDescription": "Demo plugin",
                        "developerName": "Elegy Contributors",
                        "category": "Developer Tools",
                        "capabilities": ["Read"],
                        "defaultPrompt": ["Use demo."]
                    }
                }
            }))
            .expect("extensions value"),
        );

        apply_external_wrapper_codex_projection(&wrapper, &root, "x86_64-pc-windows-msvc")
            .expect("apply projection");
        validate_target_mcp_commands(&root, "x86_64-pc-windows-msvc")
            .expect("rewritten mcp config is valid");

        assert!(root.join("bin").join("demo-plugin.exe").is_file());
        let mcp_raw = fs::read_to_string(root.join(".mcp.json")).expect("read mcp");
        assert!(mcp_raw.contains("./bin/demo-plugin.exe"));
        let manifest_raw = fs::read_to_string(root.join(".elegy-plugin").join("plugin.json"))
            .expect("read manifest");
        assert!(manifest_raw.contains("Wrapper manifest"));
        assert!(manifest_raw.contains("mcpServers"));
        let _ = fs::remove_dir_all(root);
    }
}
