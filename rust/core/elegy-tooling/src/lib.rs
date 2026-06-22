mod docs;

pub use docs::*;

use elegy_contracts::{
    validate_elegy_plugin_v1, validate_kebab_case_name, validate_mcp_analysis_result,
    validate_mcp_server_descriptor, validate_semver, ElegyPluginV1, McpAnalysisResult,
    McpServerDescriptor, McpToolDefinition, McpTransportKind,
};
use elegy_mcp::McpToolAnalyzer;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

fn generated_skill_id(server_name: &str, tool_name: &str) -> String {
    let slug = build_slug(server_name, tool_name);
    format!("mcp-{slug}")
}

fn build_slug(server_name: &str, tool_name: &str) -> String {
    let combined = format!("{server_name}-{tool_name}");
    let mut slug = String::new();
    for character in combined.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if matches!(character, '-' | '_') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorMcpDescriptorRequest {
    pub server_name: String,
    pub transport: McpTransportKind,
    pub tools: Vec<AuthorMcpToolRequest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorMcpToolRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct AuthoredMcpDescriptor {
    pub output_path: String,
    pub descriptor: McpServerDescriptor,
}

/// Lightweight skill info for generated MCP skills.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct GeneratedSkillInfo {
    pub skill_name: String,
    pub display_name: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct GeneratedSkillArtifacts {
    pub source_descriptor: String,
    pub analysis: McpAnalysisResult,
    pub generated_skills: Vec<GeneratedSkillInfo>,
    pub skipped_tools: Vec<McpToolDefinition>,
    pub written_files: Vec<String>,
}

/// Shared return type for all host exports.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedHostExport {
    pub source_package: String,
    pub plugin_name: String,
    pub plugin_version: String,
    pub emitted_components: GeneratedHostExportComponents,
    pub written_files: Vec<String>,
}

/// Component summary for a host export.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedHostExportComponents {
    pub plugin_manifest: String,
    pub skills_dir: String,
    pub skills_count: usize,
    pub apps_emitted: bool,
    pub mcp_servers_emitted: bool,
    pub hooks_emitted: bool,
}

#[derive(Debug, Error)]
pub enum ToolingError {
    #[error("failed to {operation} {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to parse YAML in {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("invalid MCP descriptor in {path}")]
    InvalidMcpDescriptor { path: PathBuf, issues: Vec<String> },
    #[error("invalid MCP analysis result for {path}")]
    InvalidMcpAnalysis { path: PathBuf, issues: Vec<String> },
    #[error("generated skill definition {skill_id} is invalid")]
    InvalidSkillDefinition {
        skill_id: String,
        issues: Vec<String>,
    },
    #[error("invalid Elegy plugin package in {path}")]
    InvalidPluginPackage { path: PathBuf, issues: Vec<String> },
    #[error("invalid docs config in {path}")]
    InvalidDocsConfig { path: PathBuf, issues: Vec<String> },
    #[error("invalid docs request")]
    InvalidDocsRequest { issues: Vec<String> },
    #[error("duplicate generated skill ID: {skill_id}")]
    DuplicateSkillId { skill_id: String },
    #[error("output file already exists: {path}")]
    OutputExists { path: PathBuf },
    #[error("unsupported host target: {host}")]
    UnsupportedHostTarget { host: String },
}

/// Resolve a plugin path to canonical (repo_root, manifest_path).
///
/// Accepts three forms:
/// - `<repo_root>` — directory containing `.elegy-plugin/plugin.json`
/// - `<repo_root>/.elegy-plugin` — the .elegy-plugin directory itself
/// - `<repo_root>/.elegy-plugin/plugin.json` — the manifest file
///
/// Returns `(repo_root, manifest_path)` on success.
pub fn resolve_plugin_root(plugin_path: &Path) -> Result<(PathBuf, PathBuf), ToolingError> {
    let path = plugin_path;
    if path.is_file() && path.file_name().is_some_and(|n| n == "plugin.json") {
        // Direct path to plugin.json
        let manifest = path.to_path_buf();
        let repo_root = path
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(Path::new("."));
        return Ok((repo_root.to_path_buf(), manifest));
    }
    if path.is_dir() && path.file_name().is_some_and(|n| n == ".elegy-plugin") {
        // .elegy-plugin directory
        let manifest = path.join("plugin.json");
        if !manifest.exists() {
            return Err(ToolingError::Io {
                operation: "resolve plugin manifest",
                path: manifest.clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "plugin.json not found in .elegy-plugin directory",
                ),
            });
        }
        let repo_root = path.parent().unwrap_or(Path::new("."));
        return Ok((repo_root.to_path_buf(), manifest));
    }
    if path.is_dir() {
        // Repo root — look for .elegy-plugin/plugin.json
        let manifest = path.join(".elegy-plugin").join("plugin.json");
        if manifest.exists() {
            return Ok((path.to_path_buf(), manifest));
        }
        Err(ToolingError::Io {
            operation: "resolve plugin root",
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No .elegy-plugin/plugin.json found in directory",
            ),
        })
    } else {
        Err(ToolingError::Io {
            operation: "resolve plugin path",
            path: path.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "Path does not exist"),
        })
    }
}

/// Resolve plugin root and load the ElegyPluginV1 manifest.
pub fn resolve_and_load_plugin_v1(
    plugin_path: &Path,
) -> Result<(PathBuf, ElegyPluginV1), ToolingError> {
    let (repo_root, manifest_path) = resolve_plugin_root(plugin_path)?;
    let raw = fs::read_to_string(&manifest_path).map_err(|e| ToolingError::Io {
        operation: "read",
        path: manifest_path.clone(),
        source: e,
    })?;
    let plugin: ElegyPluginV1 = serde_json::from_str(&raw).map_err(|e| ToolingError::Json {
        path: manifest_path,
        source: e,
    })?;
    Ok((repo_root, plugin))
}

pub fn author_mcp_descriptor_to_path(
    request: AuthorMcpDescriptorRequest,
    output_path: &Path,
    overwrite: bool,
) -> Result<AuthoredMcpDescriptor, ToolingError> {
    let descriptor = build_mcp_descriptor(request)?;
    write_json_file(output_path, &descriptor, overwrite)?;

    Ok(AuthoredMcpDescriptor {
        output_path: display_path(output_path),
        descriptor,
    })
}

pub fn analyze_mcp_descriptor_file(path: &Path) -> Result<McpAnalysisResult, ToolingError> {
    let descriptor = load_mcp_descriptor_file(path)?;
    let analysis = analyze_descriptor(&descriptor);
    let validation = validate_mcp_analysis_result(&analysis);

    if !validation.is_valid() {
        return Err(ToolingError::InvalidMcpAnalysis {
            path: path.to_path_buf(),
            issues: validation.issues,
        });
    }

    Ok(analysis)
}

pub fn generate_skills_from_descriptor_file(
    descriptor_path: &Path,
    output_dir: Option<&Path>,
    overwrite: bool,
) -> Result<GeneratedSkillArtifacts, ToolingError> {
    let analysis = analyze_mcp_descriptor_file(descriptor_path)?;
    let _descriptor = load_mcp_descriptor_file(descriptor_path)?;

    let mut generated_skills = Vec::new();
    let mut skipped_tools = Vec::new();
    let mut written_files = Vec::new();

    if let Some(output_dir) = output_dir.filter(|_| !overwrite) {
        for tool_analysis in &analysis.analyses {
            if !tool_analysis.has_valid_schema {
                continue;
            }
            let skill_name = generated_skill_id(&analysis.server_name, &tool_analysis.tool.name);
            let skill_path = output_dir.join(skill_name).join("SKILL.md");
            if skill_path.exists() {
                return Err(ToolingError::OutputExists { path: skill_path });
            }
        }
    }

    // For each tool with a valid schema, generate a SKILL.md file
    for tool_analysis in &analysis.analyses {
        if !tool_analysis.has_valid_schema {
            skipped_tools.push(tool_analysis.tool.clone());
            continue;
        }

        let skill_name = generated_skill_id(&analysis.server_name, &tool_analysis.tool.name);
        let display_name = tool_analysis.tool.name.clone();
        let description = tool_analysis
            .tool
            .description
            .clone()
            .unwrap_or_else(|| format!("Call MCP tool '{}'.", tool_analysis.tool.name));

        generated_skills.push(GeneratedSkillInfo {
            skill_name: skill_name.clone(),
            display_name: display_name.clone(),
            description: description.clone(),
        });

        if let Some(output_dir) = output_dir {
            let skill_dir = output_dir.join(&skill_name);
            let skill_path = skill_dir.join("SKILL.md");

            if skill_path.exists() && !overwrite {
                return Err(ToolingError::OutputExists { path: skill_path });
            }

            fs::create_dir_all(&skill_dir).map_err(|e| ToolingError::Io {
                operation: "create directory",
                path: skill_dir.clone(),
                source: e,
            })?;

            let skill_md = format!(
                r#"---
name: {name}
description: {description}
version: "1.0"
---

# {display_name}

{description}

## Capabilities

- `{name}`: {description}

## Details

Generated from MCP server `{server}`.
"#,
                name = skill_name,
                description = description,
                display_name = display_name,
                server = analysis.server_name,
            );

            fs::write(&skill_path, &skill_md).map_err(|e| ToolingError::Io {
                operation: "write",
                path: skill_path.clone(),
                source: e,
            })?;

            written_files.push(display_path(&skill_path));
        }
    }

    Ok(GeneratedSkillArtifacts {
        source_descriptor: display_path(descriptor_path),
        analysis,
        generated_skills,
        skipped_tools,
        written_files,
    })
}

// ── (Old verify, doctor, install-check, and probe functions removed in Phase 3 cleanup) ──

// ── (PluginTemplateKind, scaffold, doctor, and related types/functions removed in Phase 3 cleanup) ──

/// Scaffold a complete v1-format Elegy plugin repository.
///
/// Generates a standalone repository with the elegy-plugin/v1 layout:
/// `.elegy-plugin/plugin.json`, `skills/<name>/SKILL.md`,
/// `Cargo.toml`, `src/main.rs`, CI workflows, README, etc.
/// # Arguments
///
/// * `name` - Plugin name (lowercase kebab-case)
/// * `description` - Plugin description (non-empty)
/// * `version` - Plugin version (valid SemVer)
/// * `output_dir` - Output directory for generated repository
/// * `author_name` - Author name
/// * `license` - SPDX license identifier (empty string to omit)
/// * `repository_url` - Repository URL (empty string to omit)
pub fn scaffold_plugin_v1_repository(
    name: &str,
    description: &str,
    version: &str,
    output_dir: &Path,
    author_name: &str,
    license: &str,
    repository_url: &str,
) -> Result<Vec<String>, ToolingError> {
    // ── 0. Validate inputs before writing ──
    if !validate_kebab_case_name(name) {
        return Err(ToolingError::InvalidPluginPackage {
            path: output_dir.to_path_buf(),
            issues: vec![format!(
                "name '{}' is not valid lowercase kebab-case (must start with a letter, contain only a-z, 0-9, hyphens).",
                name
            )],
        });
    }
    if !validate_semver(version) {
        return Err(ToolingError::InvalidPluginPackage {
            path: output_dir.to_path_buf(),
            issues: vec![format!("version '{}' is not valid SemVer.", version)],
        });
    }
    if description.trim().is_empty() {
        return Err(ToolingError::InvalidPluginPackage {
            path: output_dir.to_path_buf(),
            issues: vec!["description must not be empty.".into()],
        });
    }

    // Reject existing non-empty destination
    if output_dir.exists() {
        let mut has_files = false;
        if let Ok(entries) = fs::read_dir(output_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                // Skip common empty markers
                if name_str == ".git" || name_str == ".gitkeep" || name_str == ".gitignore" {
                    continue;
                }
                has_files = true;
                break;
            }
        }
        if has_files {
            return Err(ToolingError::OutputExists {
                path: output_dir.to_path_buf(),
            });
        }
    }

    let mut written = Vec::new();
    let description = description.trim();

    // Create output directory
    fs::create_dir_all(output_dir).map_err(|e| ToolingError::Io {
        operation: "create directory",
        path: output_dir.to_path_buf(),
        source: e,
    })?;

    // 1. Create .elegy-plugin/plugin.json
    let plugin_dir = output_dir.join(".elegy-plugin");
    fs::create_dir_all(&plugin_dir).map_err(|e| ToolingError::Io {
        operation: "create directory",
        path: plugin_dir.clone(),
        source: e,
    })?;

    let mut plugin_map = serde_json::Map::new();
    plugin_map.insert(
        "schemaVersion".into(),
        serde_json::Value::String("elegy-plugin/v1".into()),
    );
    plugin_map.insert("name".into(), serde_json::Value::String(name.into()));
    plugin_map.insert("version".into(), serde_json::Value::String(version.into()));
    plugin_map.insert(
        "description".into(),
        serde_json::Value::String(description.into()),
    );
    plugin_map.insert("author".into(), serde_json::json!({"name": author_name}));
    // Omit license if empty
    if !license.is_empty() {
        plugin_map.insert("license".into(), serde_json::Value::String(license.into()));
    }
    // Omit repository if empty
    if !repository_url.is_empty() {
        plugin_map.insert(
            "repository".into(),
            serde_json::Value::String(repository_url.into()),
        );
    }
    plugin_map.insert(
        "skills".into(),
        serde_json::Value::String("./skills".into()),
    );
    plugin_map.insert("mcpServers".into(), serde_json::Value::Null);
    plugin_map.insert("extensions".into(), serde_json::json!({}));
    let plugin_json = serde_json::Value::Object(plugin_map);

    let plugin_path = plugin_dir.join("plugin.json");
    let content = serde_json::to_string_pretty(&plugin_json).map_err(|e| ToolingError::Json {
        path: plugin_path.clone(),
        source: e,
    })?;
    fs::write(&plugin_path, &content).map_err(|e| ToolingError::Io {
        operation: "write",
        path: plugin_path.clone(),
        source: e,
    })?;
    written.push(display_path(&plugin_path));

    // 2. Create skills/<name>/SKILL.md (Agent Skills standard)
    let skills_dir = output_dir.join("skills").join(name);
    fs::create_dir_all(&skills_dir).map_err(|e| ToolingError::Io {
        operation: "create directory",
        path: skills_dir.clone(),
        source: e,
    })?;

    let display_name = name.replace('-', " ");
    let skill_md = format!(
        r#"---
name: {name}
description: {description}
---

# {display_name}

{description}

## Usage

This skill provides agent instructions for {name}.

## Capabilities

Describe what this skill enables agents to do.
"#,
        name = name,
        display_name = display_name,
        description = description,
    );
    let skill_path = skills_dir.join("SKILL.md");
    fs::write(&skill_path, &skill_md).map_err(|e| ToolingError::Io {
        operation: "write",
        path: skill_path.clone(),
        source: e,
    })?;
    written.push(display_path(&skill_path));

    // 3. Create Cargo.toml (Rust binary)
    let mut cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "{version}"
edition = "2021"
description = "{description}"
"#,
        name = name,
        version = version,
        description = description,
    );
    if !license.is_empty() {
        cargo_toml.push_str(&format!("license = \"{license}\"\n", license = license));
    }
    if !repository_url.is_empty() {
        cargo_toml.push_str(&format!(
            "repository = \"{repository_url}\"\n",
            repository_url = repository_url
        ));
    }
    cargo_toml.push_str(
        r#"
[[bin]]
name = "{name}"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
"#,
    );
    let cargo_toml = cargo_toml.replace("{name}", name);
    let cargo_path = output_dir.join("Cargo.toml");
    fs::write(&cargo_path, &cargo_toml).map_err(|e| ToolingError::Io {
        operation: "write",
        path: cargo_path.clone(),
        source: e,
    })?;
    written.push(display_path(&cargo_path));

    // 5. Create src/main.rs
    let src_dir = output_dir.join("src");
    fs::create_dir_all(&src_dir).map_err(|e| ToolingError::Io {
        operation: "create directory",
        path: src_dir.clone(),
        source: e,
    })?;

    let main_rs = format!(
        r#"use clap::{{Parser, Subcommand}};
use serde::Serialize;

#[derive(Parser)]
#[command(name = "{name}", version = "{version}")]
struct Cli {{
    #[command(subcommand)]
    command: Command,
}}

#[derive(Subcommand)]
enum Command {{
    /// Print plugin status as JSON
    Status,
}}

#[derive(Serialize)]
struct StatusOutput {{
    status: String,
    version: String,
}}

fn main() {{
    let cli = Cli::parse();
    match cli.command {{
        Command::Status => {{
            let output = StatusOutput {{
                status: "ok".to_string(),
                version: "{version}".to_string(),
            }};
            println!("{{}}", serde_json::to_string_pretty(&output).unwrap());
        }}
    }}
}}
"#,
        name = name,
        version = version,
    );
    let main_path = src_dir.join("main.rs");
    fs::write(&main_path, &main_rs).map_err(|e| ToolingError::Io {
        operation: "write",
        path: main_path.clone(),
        source: e,
    })?;
    written.push(display_path(&main_path));

    // 5b. Create rust-toolchain.toml (pinned Rust toolchain)
    let toolchain_toml = "[toolchain]\nchannel = \"stable\"\n";
    let toolchain_path = output_dir.join("rust-toolchain.toml");
    fs::write(&toolchain_path, toolchain_toml).map_err(|e| ToolingError::Io {
        operation: "write",
        path: toolchain_path.clone(),
        source: e,
    })?;
    written.push(display_path(&toolchain_path));

    // 5c. Create tests/ directory with integration test
    let tests_dir = output_dir.join("tests");
    fs::create_dir_all(&tests_dir).map_err(|e| ToolingError::Io {
        operation: "create directory",
        path: tests_dir.clone(),
        source: e,
    })?;
    let test_rs = format!(
        r#"// Integration test for {name} plugin.
// Replace with actual tests as needed.
#[test]
fn test_plugin_compiles() {{
    assert!(true);
}}
"#,
        name = name,
    );
    let test_path = tests_dir.join("integration_test.rs");
    fs::write(&test_path, &test_rs).map_err(|e| ToolingError::Io {
        operation: "write",
        path: test_path.clone(),
        source: e,
    })?;
    written.push(display_path(&test_path));

    // 6. Create README.md
    let readme = format!(
        r#"# {display_name}

{description}

## Plugin Layout

```
.elegy-plugin/plugin.json   — Plugin manifest (elegy-plugin/v1)
skills/{name}/SKILL.md      — Agent skill instructions
src/main.rs                 — Tool implementations
```

## Verify

```bash
elegy plugin verify --plugin .
```

## Build

```bash
cargo build --release
```
"#,
        display_name = display_name,
        name = name,
        description = description,
    );
    let readme_path = output_dir.join("README.md");
    fs::write(&readme_path, &readme).map_err(|e| ToolingError::Io {
        operation: "write",
        path: readme_path.clone(),
        source: e,
    })?;
    written.push(display_path(&readme_path));

    // 7. Create AGENTS.md
    let agents_md = format!(
        r#"# {display_name}

This repository is an Elegy plugin.

## Layout

- `.elegy-plugin/plugin.json` — Plugin manifest (elegy-plugin/v1)
- `skills/{name}/SKILL.md` — Agent skill instructions
- `src/main.rs` — CLI implementation
- `Cargo.toml` — Rust project

## Commands

- `elegy plugin verify --plugin .` — Verify the plugin
- `elegy plugin doctor --plugin .` — Diagnose the plugin
- `cargo build` — Build the plugin
"#,
        display_name = display_name,
        name = name,
    );
    let agents_path = output_dir.join("AGENTS.md");
    fs::write(&agents_path, &agents_md).map_err(|e| ToolingError::Io {
        operation: "write",
        path: agents_path.clone(),
        source: e,
    })?;
    written.push(display_path(&agents_path));

    // 8. Create .gitignore
    let gitignore = "target/\n**/*.rs.bk\n.DS_Store\n";
    let gitignore_path = output_dir.join(".gitignore");
    fs::write(&gitignore_path, gitignore).map_err(|e| ToolingError::Io {
        operation: "write",
        path: gitignore_path.clone(),
        source: e,
    })?;
    written.push(display_path(&gitignore_path));

    // 9. Create .github/workflows/ci.yml
    let ci_dir = output_dir.join(".github").join("workflows");
    fs::create_dir_all(&ci_dir).map_err(|e| ToolingError::Io {
        operation: "create directory",
        path: ci_dir.clone(),
        source: e,
    })?;

    let ci_yml = r#"name: CI
on: [push, pull_request]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
      - run: cargo test
      - name: Install Elegy CLI
        run: cargo install --git https://github.com/elegy/elegy.git elegy
      - name: Verify plugin
        run: elegy plugin verify --plugin .
"#
    .to_string();
    let ci_path = ci_dir.join("ci.yml");
    fs::write(&ci_path, ci_yml).map_err(|e| ToolingError::Io {
        operation: "write",
        path: ci_path.clone(),
        source: e,
    })?;
    written.push(display_path(&ci_path));

    // 10. Verify the generated plugin
    let verify_result = verify_plugin_v1(&plugin_dir)?;
    if !verify_result.valid {
        return Err(ToolingError::InvalidPluginPackage {
            path: output_dir.to_path_buf(),
            issues: verify_result.issues,
        });
    }

    Ok(written)
}

// ── V1 plugin verification, inspection, and export ────────────────────────

/// Simple verification result for a v1 plugin.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginV1VerifyResult {
    pub valid: bool,
    pub plugin_name: String,
    pub plugin_version: String,
    pub has_skills: bool,
    pub skill_count: usize,
    pub has_mcp: bool,
    pub mcp_server_count: usize,
    pub issues: Vec<String>,
}

/// Verify a v1-format plugin manifest.
///
/// Loads `.elegy-plugin/plugin.json`, validates it structurally,
/// and checks that referenced component directories exist and contain
/// well-formed entries.
pub fn verify_plugin_v1(package_dir: &Path) -> Result<PluginV1VerifyResult, ToolingError> {
    let plugin_path = package_dir.join("plugin.json");

    // Load the plugin manifest
    let raw = fs::read_to_string(&plugin_path).map_err(|e| ToolingError::Io {
        operation: "read",
        path: plugin_path.clone(),
        source: e,
    })?;

    let plugin: ElegyPluginV1 = serde_json::from_str(&raw).map_err(|e| ToolingError::Json {
        path: plugin_path.clone(),
        source: e,
    })?;

    // Component paths are package-relative (relative to repo root,
    // which is the parent of .elegy-plugin/).
    let package_root = package_dir.parent().unwrap_or(Path::new("."));

    let validation = validate_elegy_plugin_v1(&plugin);
    let manifest_valid = validation.is_valid();
    let mut issues = validation.issues.clone();

    // Check skills directory
    let (has_skills, skill_count) = if let Some(ref skills_path) = plugin.skills {
        let skills_dir = if let Some(stripped) = skills_path.strip_prefix("./") {
            package_root.join(stripped)
        } else {
            package_root.join(skills_path)
        };
        if skills_dir.exists() && skills_dir.is_dir() {
            let mut count = 0;
            if let Ok(entries) = fs::read_dir(&skills_dir) {
                for entry in entries.flatten() {
                    let skill_dir = entry.path();
                    if skill_dir.is_dir() {
                        let skill_md = skill_dir.join("SKILL.md");
                        if skill_md.exists() {
                            count += 1;
                        }
                    }
                }
            }
            (true, count)
        } else {
            issues.push(format!(
                "skills directory '{}' does not exist.",
                skills_path
            ));
            (false, 0)
        }
    } else {
        (false, 0)
    };

    // Check MCP servers directory
    let (has_mcp, mcp_server_count) = if let Some(ref mcp_path) = plugin.mcp_servers {
        let mcp_dir = if let Some(stripped) = mcp_path.strip_prefix("./") {
            package_root.join(stripped)
        } else {
            package_root.join(mcp_path)
        };
        if mcp_dir.exists() && mcp_dir.is_dir() {
            let mut count = 0;
            if let Ok(entries) = fs::read_dir(&mcp_dir) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.extension().is_some_and(|e| e == "json") {
                        // Basic existence check; full MCP descriptor validation deferred
                        count += 1;
                    }
                }
            }
            (true, count)
        } else {
            issues.push(format!(
                "mcpServers directory '{}' does not exist.",
                mcp_path
            ));
            (false, 0)
        }
    } else {
        (false, 0)
    };

    Ok(PluginV1VerifyResult {
        valid: manifest_valid && issues.is_empty(),
        plugin_name: plugin.name,
        plugin_version: plugin.version,
        has_skills,
        skill_count,
        has_mcp,
        mcp_server_count,
        issues,
    })
}

/// Inspect a v1-format plugin and return a JSON summary.
pub fn inspect_plugin_v1(package_dir: &Path) -> Result<serde_json::Value, ToolingError> {
    let plugin_path = package_dir.join("plugin.json");
    let raw = fs::read_to_string(&plugin_path).map_err(|e| ToolingError::Io {
        operation: "read",
        path: plugin_path.clone(),
        source: e,
    })?;
    let plugin: ElegyPluginV1 = serde_json::from_str(&raw).map_err(|e| ToolingError::Json {
        path: plugin_path,
        source: e,
    })?;

    Ok(serde_json::json!({
        "schemaVersion": plugin.schema_version,
        "name": plugin.name,
        "version": plugin.version,
        "description": plugin.description,
        "author": plugin.author.map(|a| serde_json::json!({
            "name": a.name,
            "email": a.email,
            "url": a.url,
        })),
        "license": plugin.license,
        "repository": plugin.repository,
        "hasSkills": plugin.skills.is_some(),
        "hasMcpServers": plugin.mcp_servers.is_some(),
        "extensionKeys": plugin.extensions.as_ref().map(|e| e.keys().collect::<Vec<_>>()),
    }))
}

/// Export v1 plugin skills for a host target.
///
/// Accepts any of the three path forms supported by `resolve_plugin_root`.
/// Copies the ENTIRE skill directory contents (not just SKILL.md).
pub fn export_plugin_v1(
    plugin_path: &Path,
    host: &str, // "codex", "opencode", "claude"
    output_dir: &Path,
    overwrite: bool,
) -> Result<GeneratedHostExport, ToolingError> {
    let (package_root, manifest_path) = resolve_plugin_root(plugin_path)?;

    let raw = fs::read_to_string(&manifest_path).map_err(|e| ToolingError::Io {
        operation: "read",
        path: manifest_path.clone(),
        source: e,
    })?;
    let plugin: ElegyPluginV1 = serde_json::from_str(&raw).map_err(|e| ToolingError::Json {
        path: manifest_path,
        source: e,
    })?;

    let mut written_files = Vec::new();
    let mut skills_count = 0usize;
    let mut mcp_servers_emitted = false;

    // Determine host-specific output layout
    let (host_skills_dir, needs_codex_manifest, needs_claude_manifest) = match host {
        "codex" => (output_dir.join("skills"), true, false),
        "opencode" => (output_dir.join("skills"), false, false),
        "claude" => (output_dir.join("skills"), false, true),
        _ => {
            return Err(ToolingError::UnsupportedHostTarget {
                host: host.to_string(),
            });
        }
    };

    // Create output directory if needed
    fs::create_dir_all(&host_skills_dir).map_err(|e| ToolingError::Io {
        operation: "create directory",
        path: host_skills_dir.clone(),
        source: e,
    })?;

    // Export skills — copy entire skill directories
    if let Some(ref skills_path) = plugin.skills {
        let skills_src = if let Some(stripped) = skills_path.strip_prefix("./") {
            package_root.join(stripped)
        } else {
            package_root.join(skills_path)
        };

        if skills_src.exists() && skills_src.is_dir() {
            if let Ok(entries) = fs::read_dir(&skills_src) {
                for entry in entries.flatten() {
                    let skill_dir = entry.path();
                    if !skill_dir.is_dir() {
                        continue;
                    }
                    let skill_name = skill_dir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");

                    let dest_dir = host_skills_dir.join(skill_name);

                    // Copy the entire skill directory
                    if dest_dir.exists() && !overwrite {
                        return Err(ToolingError::OutputExists { path: dest_dir });
                    }
                    copy_dir_all(&skill_dir, &dest_dir)?;

                    // Track written files
                    if let Ok(walked) = walk_dir_files(&dest_dir) {
                        for f in walked {
                            written_files.push(display_path(&f));
                        }
                    }
                    skills_count += 1;
                }
            }
        }
    }

    // Export MCP server descriptors for claude export
    if host == "claude" {
        if let Some(ref mcp_path) = plugin.mcp_servers {
            let mcp_src = if let Some(stripped) = mcp_path.strip_prefix("./") {
                package_root.join(stripped)
            } else {
                package_root.join(mcp_path)
            };

            if mcp_src.exists() && mcp_src.is_dir() {
                let mcp_dest = output_dir.join("mcp");
                if mcp_dest.exists() && !overwrite {
                    return Err(ToolingError::OutputExists { path: mcp_dest });
                }
                copy_dir_all(&mcp_src, &mcp_dest)?;
                if let Ok(walked) = walk_dir_files(&mcp_dest) {
                    for f in walked {
                        written_files.push(display_path(&f));
                    }
                }
                mcp_servers_emitted = true;
            }
        }
    }

    // Write host-specific plugin manifest if applicable
    if needs_codex_manifest {
        let manifest_dir = output_dir.join(".codex-plugin");
        fs::create_dir_all(&manifest_dir).map_err(|e| ToolingError::Io {
            operation: "create directory",
            path: manifest_dir.clone(),
            source: e,
        })?;
        let codex_manifest = serde_json::json!({
            "name": plugin.name,
            "version": plugin.version,
            "description": plugin.description,
            "author": plugin.author.as_ref().map(|a| serde_json::json!({"name": a.name})),
            "license": plugin.license,
            "repository": plugin.repository,
            "skills": "./skills",
        });
        let manifest_path = manifest_dir.join("plugin.json");
        write_json_file(&manifest_path, &codex_manifest, overwrite)?;
        written_files.push(display_path(&manifest_path));
    }

    if needs_claude_manifest {
        let manifest_dir = output_dir.join(".claude-plugin");
        fs::create_dir_all(&manifest_dir).map_err(|e| ToolingError::Io {
            operation: "create directory",
            path: manifest_dir.clone(),
            source: e,
        })?;
        let claude_manifest = serde_json::json!({
            "name": plugin.name,
            "version": plugin.version,
            "description": plugin.description,
            "author": plugin.author.as_ref().map(|a| serde_json::json!({"name": a.name})),
            "skills": "./skills",
        });
        let manifest_path = manifest_dir.join("plugin.json");
        write_json_file(&manifest_path, &claude_manifest, overwrite)?;
        written_files.push(display_path(&manifest_path));
    }

    Ok(GeneratedHostExport {
        source_package: format!("{}-v{}", plugin.name, plugin.version),
        plugin_name: plugin.name,
        plugin_version: plugin.version,
        emitted_components: GeneratedHostExportComponents {
            plugin_manifest: match host {
                "codex" => ".codex-plugin/plugin.json".to_string(),
                "claude" => ".claude-plugin/plugin.json".to_string(),
                _ => String::new(),
            },
            skills_dir: host.to_string(),
            skills_count,
            apps_emitted: false,
            mcp_servers_emitted,
            hooks_emitted: false,
        },
        written_files,
    })
}

/// Recursively copy a directory.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), ToolingError> {
    fs::create_dir_all(dst).map_err(|e| ToolingError::Io {
        operation: "create directory",
        path: dst.to_path_buf(),
        source: e,
    })?;
    for entry in fs::read_dir(src).map_err(|e| ToolingError::Io {
        operation: "read directory",
        path: src.to_path_buf(),
        source: e,
    })? {
        let entry = entry.map_err(|e| ToolingError::Io {
            operation: "read directory entry",
            path: src.to_path_buf(),
            source: e,
        })?;
        let ty = entry.file_type().map_err(|e| ToolingError::Io {
            operation: "read file type",
            path: entry.path(),
            source: e,
        })?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else if ty.is_file() {
            fs::copy(entry.path(), dst.join(entry.file_name())).map_err(|e| ToolingError::Io {
                operation: "copy",
                path: entry.path(),
                source: e,
            })?;
        }
    }
    Ok(())
}

/// Walk a directory tree and return all file paths.
fn walk_dir_files(dir: &Path) -> Result<Vec<PathBuf>, ToolingError> {
    let mut files = Vec::new();
    walk_dir_files_recursive(dir, &mut files)?;
    Ok(files)
}

fn walk_dir_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), ToolingError> {
    for entry in fs::read_dir(dir).map_err(|e| ToolingError::Io {
        operation: "read directory",
        path: dir.to_path_buf(),
        source: e,
    })? {
        let entry = entry.map_err(|e| ToolingError::Io {
            operation: "read directory entry",
            path: dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if path.is_dir() {
            walk_dir_files_recursive(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

// ── (build_scaffold_plugin_json removed in Phase 3 cleanup) ──

/// Pack a v1-format plugin into a portable zip archive.
///
/// Accepts the three path forms supported by `resolve_plugin_root`.
/// The manifest entry is placed at the archive root as `plugin.json`.
/// Only declared component directories are included.
pub fn pack_plugin_v1(plugin_path: &Path, output_zip: &Path) -> Result<String, ToolingError> {
    let (repo_root, _manifest_path) = resolve_plugin_root(plugin_path)?;
    let plugin_dir = repo_root.join(".elegy-plugin");
    let manifest_path = plugin_dir.join("plugin.json");

    // Verify the plugin before packing
    let verify_result = verify_plugin_v1(&plugin_dir)?;
    if !verify_result.valid {
        return Err(ToolingError::InvalidPluginPackage {
            path: manifest_path,
            issues: verify_result.issues,
        });
    }

    // Load the plugin manifest to find component directories
    let raw = fs::read_to_string(&manifest_path).map_err(|e| ToolingError::Io {
        operation: "read",
        path: manifest_path.clone(),
        source: e,
    })?;
    let plugin: ElegyPluginV1 = serde_json::from_str(&raw).map_err(|e| ToolingError::Json {
        path: manifest_path.clone(),
        source: e,
    })?;

    // Collect all files to include
    let mut entries: Vec<PathBuf> = Vec::new();

    // Include the manifest file (will be renamed to plugin.json at root)
    entries.push(manifest_path.clone());

    // Include declared component directories
    let component_roots: Vec<&str> = vec![plugin.skills.as_deref(), plugin.mcp_servers.as_deref()]
        .into_iter()
        .flatten()
        .collect();

    for root_str in &component_roots {
        let root_path = if let Some(stripped) = root_str.strip_prefix("./") {
            repo_root.join(stripped)
        } else {
            repo_root.join(root_str)
        };
        if root_path.exists() && root_path.is_dir() {
            collect_files_recursive(&root_path, &mut entries)?;
        }
    }

    // Sort for deterministic archives
    entries.sort();

    // Create the zip archive
    let file = fs::File::create(output_zip).map_err(|source| ToolingError::Io {
        operation: "create",
        path: output_zip.to_path_buf(),
        source,
    })?;

    let mut zip_writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let mut buffer = Vec::new();

    for entry_path in &entries {
        // Determine the relative path in the archive
        let relative_str = if entry_path == &manifest_path {
            // Manifest entry goes to archive root as plugin.json
            "plugin.json".to_string()
        } else if let Ok(rel) = entry_path.strip_prefix(&repo_root) {
            rel.to_string_lossy().replace('\\', "/")
        } else {
            entry_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        // Skip excluded patterns
        if should_exclude_from_pack(&relative_str) {
            continue;
        }

        zip_writer
            .start_file(relative_str.clone(), options)
            .map_err(|source| ToolingError::Io {
                operation: "write zip entry",
                path: PathBuf::from(&relative_str),
                source: source.into(),
            })?;

        if entry_path.is_file() {
            buffer.clear();
            let mut f = fs::File::open(entry_path).map_err(|source| ToolingError::Io {
                operation: "read",
                path: entry_path.clone(),
                source,
            })?;
            f.read_to_end(&mut buffer)
                .map_err(|source| ToolingError::Io {
                    operation: "read",
                    path: entry_path.clone(),
                    source,
                })?;
            zip_writer
                .write_all(&buffer)
                .map_err(|source| ToolingError::Io {
                    operation: "write zip content",
                    path: entry_path.clone(),
                    source,
                })?;
        }
    }

    zip_writer.finish().map_err(|source| ToolingError::Io {
        operation: "finalize zip",
        path: output_zip.to_path_buf(),
        source: source.into(),
    })?;

    Ok(display_path(output_zip))
}

fn collect_files_recursive(dir: &Path, entries: &mut Vec<PathBuf>) -> Result<(), ToolingError> {
    for entry in fs::read_dir(dir).map_err(|source| ToolingError::Io {
        operation: "read directory",
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ToolingError::Io {
            operation: "read directory entry",
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, entries)?;
        } else if path.is_file() {
            entries.push(path);
        }
    }
    Ok(())
}

/// Check if a relative path should be excluded from the plugin archive.
fn should_exclude_from_pack(relative_str: &str) -> bool {
    let parts: Vec<&str> = relative_str.split('/').collect();
    for part in &parts {
        if *part == ".git" || *part == "target" {
            return true;
        }
    }
    // Exclude temporary files
    if relative_str.ends_with(".tmp")
        || relative_str.ends_with(".swp")
        || relative_str.ends_with('~')
    {
        return true;
    }
    false
}

fn build_mcp_descriptor(
    request: AuthorMcpDescriptorRequest,
) -> Result<McpServerDescriptor, ToolingError> {
    let descriptor = McpServerDescriptor {
        server_name: request.server_name,
        transport: request.transport,
        tools: request
            .tools
            .into_iter()
            .map(|tool| McpToolDefinition {
                name: tool.name,
                description: tool.description,
                input_schema: None,
            })
            .collect(),
    };

    let issues = descriptor_validation_issues(&descriptor);
    if !issues.is_empty() {
        return Err(ToolingError::InvalidMcpDescriptor {
            path: PathBuf::from("<in-memory>"),
            issues,
        });
    }

    Ok(descriptor)
}

fn load_mcp_descriptor_file(path: &Path) -> Result<McpServerDescriptor, ToolingError> {
    let content = fs::read_to_string(path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: path.to_path_buf(),
        source,
    })?;

    let descriptor = serde_json::from_str::<McpServerDescriptor>(&content).map_err(|source| {
        ToolingError::Json {
            path: path.to_path_buf(),
            source,
        }
    })?;

    let issues = descriptor_validation_issues(&descriptor);
    if !issues.is_empty() {
        return Err(ToolingError::InvalidMcpDescriptor {
            path: path.to_path_buf(),
            issues,
        });
    }

    Ok(descriptor)
}

fn descriptor_validation_issues(descriptor: &McpServerDescriptor) -> Vec<String> {
    validate_mcp_server_descriptor(descriptor).issues
}

fn analyze_descriptor(descriptor: &McpServerDescriptor) -> McpAnalysisResult {
    let mut analysis = McpToolAnalyzer.analyze(descriptor);
    for tool_analysis in &mut analysis.analyses {
        tool_analysis.has_valid_schema = tool_analysis
            .tool
            .input_schema
            .as_ref()
            .is_some_and(is_supported_input_schema);
    }

    analysis
}

fn is_supported_input_schema(value: &Value) -> bool {
    matches!(value, Value::Object(_))
}

pub(crate) fn write_text_file(
    output_path: &Path,
    content: &str,
    overwrite: bool,
) -> Result<(), ToolingError> {
    if output_path.exists() && !overwrite {
        return Err(ToolingError::OutputExists {
            path: output_path.to_path_buf(),
        });
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| ToolingError::Io {
            operation: "create directory",
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(output_path, content).map_err(|source| ToolingError::Io {
        operation: "write",
        path: output_path.to_path_buf(),
        source,
    })
}

pub(crate) fn write_json_file<T: Serialize>(
    output_path: &Path,
    value: &T,
    overwrite: bool,
) -> Result<(), ToolingError> {
    if output_path.exists() && !overwrite {
        return Err(ToolingError::OutputExists {
            path: output_path.to_path_buf(),
        });
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| ToolingError::Io {
            operation: "create directory",
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let mut content = serde_json::to_string_pretty(value).map_err(|source| ToolingError::Json {
        path: output_path.to_path_buf(),
        source,
    })?;
    content.push('\n');

    fs::write(output_path, content).map_err(|source| ToolingError::Io {
        operation: "write",
        path: output_path.to_path_buf(),
        source,
    })
}

pub(crate) fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        analyze_mcp_descriptor_file, author_mcp_descriptor_to_path, export_plugin_v1,
        generate_skills_from_descriptor_file, inspect_plugin_v1, scaffold_plugin_v1_repository,
        verify_plugin_v1, AuthorMcpDescriptorRequest, AuthorMcpToolRequest, ToolingError,
    };
    use elegy_contracts::{validate_mcp_server_descriptor, McpTransportKind};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tempfile::TempDir;

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&dir).expect("create temp directory");
        dir
    }

    #[test]
    fn author_mcp_descriptor_writes_valid_json() {
        let temp_dir = unique_temp_dir("elegy-tooling-author");
        let output_path = temp_dir.join("weather-mcp.json");

        let result = author_mcp_descriptor_to_path(
            AuthorMcpDescriptorRequest {
                server_name: "weather-server".to_string(),
                transport: McpTransportKind::Stdio,
                tools: vec![
                    AuthorMcpToolRequest {
                        name: "get-weather".to_string(),
                        description: Some("Look up a weather report".to_string()),
                    },
                    AuthorMcpToolRequest {
                        name: "list-alerts".to_string(),
                        description: None,
                    },
                ],
            },
            &output_path,
            false,
        )
        .expect("authoring should succeed");

        assert_eq!(result.descriptor.server_name, "weather-server");
        assert_eq!(result.descriptor.tools.len(), 2);
        assert!(output_path.is_file());

        let persisted = fs::read_to_string(&output_path).expect("read descriptor file");
        let parsed = serde_json::from_str(&persisted).expect("parse descriptor file");
        let validation = validate_mcp_server_descriptor(&parsed);
        assert!(
            validation.is_valid(),
            "unexpected issues: {:?}",
            validation.issues
        );
        assert!(
            parsed.tools.iter().all(|tool| tool.input_schema.is_none()),
            "authored MCP descriptors should not fabricate tool schemas"
        );
    }

    #[test]
    fn analyze_and_generate_skills_from_descriptor_file() {
        let temp_dir = unique_temp_dir("elegy-tooling-generate");
        let descriptor_path = temp_dir.join("weather-mcp.json");
        let output_dir = temp_dir.join("generated-skills");

        fs::write(
            &descriptor_path,
            r#"{
    "serverName": "weather-server",
    "transport": "stdio",
    "tools": [
        {
            "name": "get-weather",
            "description": "Look up a weather report",
            "inputSchema": { "type": "object" }
        },
        {
            "name": "list-alerts",
            "description": "List active weather alerts"
        }
    ]
}
"#,
        )
        .expect("write descriptor fixture");

        let analysis = analyze_mcp_descriptor_file(&descriptor_path)
            .expect("analysis should succeed for valid descriptor");
        assert_eq!(analysis.server_name, "weather-server");
        assert_eq!(analysis.analyses.len(), 2);

        let generated =
            generate_skills_from_descriptor_file(&descriptor_path, Some(&output_dir), false)
                .expect("skill generation should succeed");
        assert_eq!(generated.generated_skills.len(), 1);
        assert_eq!(
            generated.generated_skills[0].skill_name,
            "mcp-weather-server-get-weather"
        );
        assert_eq!(generated.skipped_tools.len(), 1);
        assert_eq!(generated.written_files.len(), 1);
        assert!(output_dir
            .join("mcp-weather-server-get-weather")
            .join("SKILL.md")
            .is_file());
    }

    #[test]
    fn analyze_and_generate_skip_present_but_invalid_schemas() {
        let temp_dir = unique_temp_dir("elegy-tooling-invalid-schema");
        let descriptor_path = temp_dir.join("weather-mcp.json");

        fs::write(
            &descriptor_path,
            r#"{
  "serverName": "weather-server",
  "transport": "stdio",
  "tools": [
    {
      "name": "get-weather",
      "description": "Look up a weather report",
      "inputSchema": "not-a-schema-object"
    }
  ]
}
"#,
        )
        .expect("write descriptor fixture");

        let analysis = analyze_mcp_descriptor_file(&descriptor_path)
            .expect("analysis should still succeed for structurally invalid tool schema values");
        assert_eq!(analysis.analyses.len(), 1);
        assert!(
            !analysis.analyses[0].has_valid_schema,
            "non-object schemas should not be treated as valid for skill generation"
        );

        let generated = generate_skills_from_descriptor_file(&descriptor_path, None, false)
            .expect("generation should succeed while skipping invalid-schema tools");
        assert!(generated.generated_skills.is_empty());
        assert_eq!(generated.skipped_tools.len(), 1);
        assert_eq!(generated.skipped_tools[0].name, "get-weather");
    }

    #[test]
    fn authoring_refuses_to_overwrite_existing_file_without_force() {
        let temp_dir = unique_temp_dir("elegy-tooling-overwrite");
        let output_path = temp_dir.join("weather-mcp.json");
        fs::write(&output_path, "{}\n").expect("seed existing file");

        let error = author_mcp_descriptor_to_path(
            AuthorMcpDescriptorRequest {
                server_name: "weather-server".to_string(),
                transport: McpTransportKind::Stdio,
                tools: Vec::new(),
            },
            &output_path,
            false,
        )
        .expect_err("existing file should be rejected without force");

        assert!(matches!(error, ToolingError::OutputExists { .. }));
    }

    #[test]
    fn generation_preflights_existing_outputs_before_writing_any_files() {
        let temp_dir = unique_temp_dir("elegy-tooling-preflight");
        let descriptor_path = temp_dir.join("weather-mcp.json");
        let output_dir = temp_dir.join("generated-skills");
        fs::create_dir_all(&output_dir).expect("create output directory");
        fs::create_dir_all(output_dir.join("mcp-weather-server-list-alerts"))
            .expect("create skill dir");
        fs::write(
            output_dir
                .join("mcp-weather-server-list-alerts")
                .join("SKILL.md"),
            "---\nname: test\ndescription: test\n---\n",
        )
        .expect("seed colliding output file");

        fs::write(
            &descriptor_path,
            r#"{
    "serverName": "weather-server",
    "transport": "stdio",
    "tools": [
        {
            "name": "get-weather",
            "description": "Look up a weather report",
            "inputSchema": { "type": "object" }
        },
        {
            "name": "list-alerts",
            "description": "List active weather alerts",
            "inputSchema": { "type": "object" }
        }
    ]
}
"#,
        )
        .expect("write descriptor fixture");

        let error =
            generate_skills_from_descriptor_file(&descriptor_path, Some(&output_dir), false)
                .expect_err("colliding output should fail before any write occurs");

        assert!(matches!(error, ToolingError::OutputExists { .. }));
        assert!(
            !output_dir
                .join("mcp-weather-server-get-weather")
                .join("SKILL.md")
                .exists(),
            "preflight should block all writes when a collision is detected"
        );
    }

    // ── V1 plugin scaffold/verify/inspect/export tests ─────────────────

    #[test]
    fn scaffold_v1_then_verify_passes() {
        let tmp = TempDir::new().expect("create temp dir");
        let repo_dir = tmp.path().join("my-plugin");

        let written = scaffold_plugin_v1_repository(
            "my-plugin",
            "A test plugin.",
            "0.1.0",
            &repo_dir,
            "Test Author",
            "MIT",
            "https://github.com/test/my-plugin",
        )
        .expect("scaffold plugin");

        assert!(!written.is_empty(), "should have written files");

        // Verify the plugin
        let plugin_dir = repo_dir.join(".elegy-plugin");
        let result = verify_plugin_v1(&plugin_dir).expect("verify plugin");
        assert!(
            result.valid,
            "scaffolded plugin should be valid, but got issues: {:?}",
            result.issues
        );
        assert_eq!(result.plugin_name, "my-plugin");
        assert_eq!(result.plugin_version, "0.1.0");
        assert!(result.has_skills, "should have skills");
        assert!(result.skill_count >= 1, "should have at least 1 skill");
        assert!(!result.has_mcp, "should not have MCP servers");
    }

    #[test]
    fn scaffold_v1_minimal_verify_passes() {
        let tmp = TempDir::new().expect("create temp dir");
        let repo_dir = tmp.path().join("minimal-plugin");

        scaffold_plugin_v1_repository(
            "minimal-plugin",
            "A minimal plugin.",
            "0.1.0",
            &repo_dir,
            "Test Author",
            "MIT",
            "https://github.com/test/minimal-plugin",
        )
        .expect("scaffold minimal plugin");

        let plugin_dir = repo_dir.join(".elegy-plugin");
        let result = verify_plugin_v1(&plugin_dir).expect("verify plugin");
        assert!(
            result.valid,
            "minimal plugin should be valid, but got issues: {:?}",
            result.issues
        );
        assert!(result.has_skills);
    }

    #[test]
    fn inspect_v1_returns_expected_json() {
        let tmp = TempDir::new().expect("create temp dir");
        let repo_dir = tmp.path().join("inspect-plugin");

        scaffold_plugin_v1_repository(
            "inspect-plugin",
            "A plugin for inspection testing.",
            "0.2.0",
            &repo_dir,
            "Test Author",
            "Apache-2.0",
            "https://github.com/test/inspect-plugin",
        )
        .expect("scaffold plugin");

        let plugin_dir = repo_dir.join(".elegy-plugin");
        let summary = inspect_plugin_v1(&plugin_dir).expect("inspect plugin");

        assert_eq!(summary["name"], "inspect-plugin");
        assert_eq!(summary["version"], "0.2.0");
        assert_eq!(summary["license"], "Apache-2.0");
        assert_eq!(summary["hasSkills"], serde_json::Value::Bool(true));
    }

    #[test]
    fn export_v1_codex_creates_expected_files() {
        let tmp = TempDir::new().expect("create temp dir");
        let repo_dir = tmp.path().join("export-plugin");
        let export_dir = tmp.path().join("export-output");

        scaffold_plugin_v1_repository(
            "export-plugin",
            "A plugin for export testing.",
            "0.3.0",
            &repo_dir,
            "Test Author",
            "MIT",
            "https://github.com/test/export-plugin",
        )
        .expect("scaffold plugin");

        let plugin_dir = repo_dir.join(".elegy-plugin");
        let result =
            export_plugin_v1(&plugin_dir, "codex", &export_dir, true).expect("export codex plugin");

        assert!(result.emitted_components.skills_count >= 1);
        assert!(export_dir.join("skills").exists());
        assert!(export_dir
            .join(".codex-plugin")
            .join("plugin.json")
            .exists());
    }
}
