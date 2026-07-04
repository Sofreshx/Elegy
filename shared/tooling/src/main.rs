use clap::{Parser, Subcommand, ValueEnum};
use elegy_plugin_sdk::{
    export_plugin_v1_with_codex_mode_and_binary, inspect_plugin_v1, pack_plugin_v1,
    pack_plugin_v1_with_binary, resolve_plugin_root, scaffold_plugin_v1_repository_with_mode,
    select_marketplace_artifact, validate_elegy_marketplace_v1, validate_elegy_plugin_v1,
    verify_plugin_v1, CodexProjectionMode, ElegyMarketplaceArtifact, ElegyMarketplaceInterface,
    ElegyMarketplacePlugin, ElegyMarketplaceSource, ElegyMarketplaceV1, ElegyPluginV1,
    PluginArchiveBinary, PluginScaffoldMode, ELEGY_MARKETPLACE_V1_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
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
    /// Scaffold a minimal Elegy plugin repository
    Scaffold {
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: String,
        #[arg(long, default_value = "0.1.0")]
        version: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        author: String,
        #[arg(long, default_value = "")]
        license: String,
        #[arg(long, default_value = "")]
        repository: String,
        #[arg(long, value_enum, default_value_t = ScaffoldMode::SkillOnly)]
        mode: ScaffoldMode,
    },
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
    /// Export a local marketplace as a Codex marketplace tree
    ExportCodex {
        #[arg(long, default_value = ".")]
        source: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        target: Option<String>,
        #[arg(long, default_value_t = false)]
        overwrite: bool,
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
    #[serde(default)]
    packaging: Option<String>,
    #[serde(default)]
    plugin_root: Option<String>,
    #[serde(default)]
    artifact_base_url: Option<String>,
    #[serde(default = "default_marketplace_category")]
    marketplace_category: String,
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

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum ScaffoldMode {
    #[default]
    SkillOnly,
    RustCli,
    McpServer,
}

impl From<ScaffoldMode> for PluginScaffoldMode {
    fn from(value: ScaffoldMode) -> Self {
        match value {
            ScaffoldMode::SkillOnly => Self::SkillOnly,
            ScaffoldMode::RustCli => Self::RustCli,
            ScaffoldMode::McpServer => Self::McpServer,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Scaffold {
            name,
            description,
            version,
            output,
            author,
            license,
            repository,
            mode,
        } => match scaffold_plugin_v1_repository_with_mode(
            &name,
            &description,
            &version,
            &output,
            &author,
            &license,
            &repository,
            mode.into(),
        ) {
            Ok(files) => {
                println!("Plugin scaffolded successfully.");
                println!("  mode: {mode:?}  files: {}", files.len());
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("Error: {error}");
                ExitCode::from(2)
            }
        },
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
        MarketplaceCommand::ExportCodex {
            source,
            output,
            target,
            overwrite,
        } => export_codex_marketplace(
            &source,
            &output,
            target.as_deref().unwrap_or(current_release_target()),
            overwrite,
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
        .filter(|surface| surface.packaging.as_deref() == Some("plugin"))
    {
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
        let artifacts = targets
            .iter()
            .map(|target| {
                let file_name = format!("{}-plugin-{target}.zip", surface.name);
                let url = format!("{artifact_base}/{release_tag}/{file_name}");
                ElegyMarketplaceArtifact {
                    target: (*target).to_string(),
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
        if actual != expected {
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
        install_from_url(
            &artifact.url,
            &artifact.checksum_url,
            &root,
            Some(&entry.name),
            Some(&manifest.version),
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
    output: &Path,
    target: &str,
    overwrite: bool,
) -> Result<(), String> {
    let loaded = load_marketplace(source)?;
    let local_root = loaded
        .local_root
        .ok_or_else(|| "Codex export requires a local marketplace source".to_string())?;
    if output.exists() && !overwrite {
        return Err(format!("output already exists: {}", output.display()));
    }
    fs::create_dir_all(output).map_err(|error| format!("create {}: {error}", output.display()))?;
    let artifact_staging = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut codex_entries = Vec::new();
    for (entry, manifest) in &loaded.plugins {
        let plugin_root = local_root.join(entry.source.path.trim_start_matches("./"));
        let plugin_output = output.join("plugins").join(&entry.name);
        let binary =
            materialize_marketplace_binary(entry, manifest, target, artifact_staging.path())?;
        let binary_spec = binary
            .as_ref()
            .map(|(path, archive_path)| PluginArchiveBinary {
                source_path: path.as_path(),
                archive_path: archive_path.clone(),
            });
        export_plugin_v1_with_codex_mode_and_binary(
            &plugin_root,
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
    let index_path = output
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
    println!("Exported Codex marketplace to {}", output.display());
    Ok(())
}

fn materialize_marketplace_binary(
    entry: &ElegyMarketplacePlugin,
    manifest: &ElegyPluginV1,
    target: &str,
    staging_root: &Path,
) -> Result<Option<(PathBuf, String)>, String> {
    if entry.artifacts.is_empty() {
        return Ok(None);
    }
    let artifact = select_marketplace_artifact(entry, target).ok_or_else(|| {
        format!(
            "plugin '{}' has no artifact for Codex export target '{}'",
            entry.name, target
        )
    })?;
    install_from_url(
        &artifact.url,
        &artifact.checksum_url,
        staging_root,
        Some(&entry.name),
        Some(&manifest.version),
    )
    .map_err(|error| error.to_string())?;
    let installed_root = staging_root.join(&entry.name);
    let bin_root = installed_root.join("bin");
    if !bin_root.is_dir() {
        if artifact.target == "any" {
            return Ok(None);
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
    Ok(Some((binary_path, archive_path)))
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
