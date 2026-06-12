mod docs;

pub use docs::*;

use elegy_contracts::{
    validate_elegy_plugin_package, validate_mcp_analysis_result, validate_mcp_server_descriptor,
    validate_skill_definition_v2, ElegyPluginInstallReceiptV1, ElegyPluginPackage,
    ElegyPluginPackageCapabilityProjectionComponent, ElegyPluginPackagePathComponent,
    ElegyPluginReadinessFinding, ElegyPluginReadinessPackageIdentity,
    ElegyPluginReadinessProjectedTool, ElegyPluginReadinessSideEffectSummary,
    ElegyPluginReadinessToolStatus, ElegyPluginReadinessV1, ElegyPluginReadinessVerifiedSkill,
    HostSideEffectClass, McpAnalysisResult, McpServerDescriptor, McpToolDefinition,
    McpTransportKind, SkillDefinitionV2, SkillHostProjection,
};
use elegy_mcp::{generated_skill_id, McpSkillGenerator, McpToolAnalyzer};
use serde::Serialize;
use serde_json::json;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

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

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct GeneratedSkillArtifacts {
    pub source_descriptor: String,
    pub analysis: McpAnalysisResult,
    pub generated_skills: Vec<SkillDefinitionV2>,
    pub skipped_tools: Vec<McpToolDefinition>,
    pub written_files: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedCodexPluginArtifacts {
    pub source_package: String,
    pub plugin_name: String,
    pub plugin_version: String,
    pub emitted_components: GeneratedCodexPluginComponents,
    pub written_files: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedCodexPluginComponents {
    pub plugin_manifest: String,
    pub skills_dir: String,
    pub skills_count: usize,
    pub apps_emitted: bool,
    pub mcp_servers_emitted: bool,
    pub hooks_emitted: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CodexPluginManifest {
    name: String,
    version: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<CodexPluginAuthor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    documentation_uri: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    keywords: Vec<String>,
    skills: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    interface: Option<CodexPluginInterface>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CodexPluginAuthor {
    name: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CodexPluginInterface {
    display_name: String,
    short_description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    developer_name: Option<String>,
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
    let generation = McpSkillGenerator.generate(&analysis);

    validate_generated_skills(&generation.generated_skills)?;

    let written_files = match output_dir {
        Some(output_dir) => write_skill_files(output_dir, &generation.generated_skills, overwrite)?
            .into_iter()
            .map(|path| display_path(&path))
            .collect(),
        None => Vec::new(),
    };

    Ok(GeneratedSkillArtifacts {
        source_descriptor: display_path(descriptor_path),
        analysis,
        generated_skills: generation.generated_skills,
        skipped_tools: generation.skipped_tools,
        written_files,
    })
}

pub fn generate_codex_plugin_from_package_file(
    package_path: &Path,
    output_dir: &Path,
    overwrite: bool,
) -> Result<GeneratedCodexPluginArtifacts, ToolingError> {
    let package = load_plugin_package_file(package_path)?;
    let package_root = package_path.parent().unwrap_or_else(|| Path::new("."));
    let plugin_output_name = package.identity.name.trim();
    let manifest = build_codex_plugin_manifest(&package);
    let skill_documents = collect_codex_skill_documents(&package, package_root)?;

    let plugin_root = output_dir.join(plugin_output_name);
    let manifest_path = plugin_root.join(".codex-plugin").join("plugin.json");
    let skills_root = plugin_root.join("skills");

    let mut target_paths = vec![manifest_path.clone()];
    target_paths.extend(
        skill_documents
            .iter()
            .map(|document| skills_root.join(&document.directory_name).join("SKILL.md")),
    );

    if overwrite {
        clear_plugin_output_root(&plugin_root)?;
    } else {
        preflight_output_paths(&target_paths, overwrite)?;
    }

    let mut written_files = Vec::with_capacity(target_paths.len());

    write_json_file(&manifest_path, &manifest, overwrite)?;
    written_files.push(display_path(&manifest_path));

    for document in &skill_documents {
        let output_path = skills_root.join(&document.directory_name).join("SKILL.md");
        if let Err(error) = write_text_file(&output_path, &document.content, overwrite) {
            cleanup_written_files(&written_files.iter().map(PathBuf::from).collect::<Vec<_>>());
            return Err(error);
        }
        written_files.push(display_path(&output_path));
    }

    Ok(GeneratedCodexPluginArtifacts {
        source_package: display_path(package_path),
        plugin_name: plugin_output_name.to_string(),
        plugin_version: package.identity.version.clone(),
        emitted_components: GeneratedCodexPluginComponents {
            plugin_manifest: display_path(&manifest_path),
            skills_dir: display_path(&skills_root),
            skills_count: skill_documents.len(),
            apps_emitted: false,
            mcp_servers_emitted: false,
            hooks_emitted: false,
        },
        written_files,
    })
}

/// Verify a plugin package against its referenced skill definitions.
/// Cross-validates capability projections, side-effect classes, and subset declarations.
/// Returns a readiness receipt with `ready`, `partial`, or `blocked` status.
pub fn verify_plugin_package(
    package_path: &Path,
    package_root: Option<&Path>,
) -> Result<ElegyPluginReadinessV1, ToolingError> {
    let package = load_plugin_package_file(package_path)?;
    let root = package_root.unwrap_or_else(|| package_path.parent().unwrap_or(Path::new(".")));

    let mut readiness = ElegyPluginReadinessV1 {
        schema_version: "elegy-plugin-readiness/v1".to_string(),
        package_identity: ElegyPluginReadinessPackageIdentity {
            package_id: package.identity.package_id.clone(),
            name: package.identity.name.clone(),
            version: package.identity.version.clone(),
        },
        readiness: "ready".to_string(),
        ..Default::default()
    };

    let mut findings: Vec<ElegyPluginReadinessFinding> = Vec::new();

    // R3: elegyCompatibility check
    // Standard publishable packages must declare an Elegy contract bundle version.
    if package.elegy_compatibility.is_none() {
        findings.push(ElegyPluginReadinessFinding {
            code: "PKG-COMPAT-MISSING".to_string(),
            severity: "error".to_string(),
            message: "publishable plugin package is missing elegyCompatibility. Standard packages must declare contractBundleVersion and schemaLine.".to_string(),
            detail: None,
        });
    }

    // Collect all capabilities from inlined skill definitions (keyed by "namespace.name")
    // and from referenced definition files.
    let mut known_capabilities: std::collections::BTreeMap<
        String,
        std::collections::BTreeSet<String>,
    > = std::collections::BTreeMap::new();
    let mut skill_host_projections: std::collections::BTreeMap<String, SkillHostProjection> =
        std::collections::BTreeMap::new();

    for component in &package.components.skill_definitions {
        let _skill_key = if let Some(def) = &component.definition {
            let key = format!("{}.{}", def.identity.namespace, def.identity.name);
            let mut caps = std::collections::BTreeSet::new();
            for cap in &def.capabilities {
                caps.insert(cap.id.clone());
            }
            known_capabilities.insert(key.clone(), caps);
            if let Some(hp) = &def.host_projection {
                skill_host_projections.insert(key.clone(), hp.clone());
            }
            readiness
                .verified_skills
                .push(ElegyPluginReadinessVerifiedSkill {
                    skill_id: key.clone(),
                    status: "valid".to_string(),
                });
            key
        } else if let Some(definition_ref) = &component.definition_ref {
            // Try to load referenced skill definition from package root
            let skill_path = root.join(definition_ref);
            let key = format!("ref:{}", component.id);
            match load_skill_definition_file(&skill_path) {
                Ok(def) => {
                    let skill_key = format!("{}.{}", def.identity.namespace, def.identity.name);
                    let mut caps = std::collections::BTreeSet::new();
                    for cap in &def.capabilities {
                        caps.insert(cap.id.clone());
                    }
                    known_capabilities.insert(skill_key.clone(), caps);
                    if let Some(hp) = &def.host_projection {
                        skill_host_projections.insert(skill_key.clone(), hp.clone());
                    }
                    readiness
                        .verified_skills
                        .push(ElegyPluginReadinessVerifiedSkill {
                            skill_id: skill_key.clone(),
                            status: "valid".to_string(),
                        });
                }
                Err(_) => {
                    findings.push(ElegyPluginReadinessFinding {
                        code: "PKG-REF-MISSING".to_string(),
                        severity: "error".to_string(),
                        message: format!(
                            "Cannot resolve skill definition reference '{}' for component '{}'",
                            definition_ref, component.id
                        ),
                        detail: Some(format!("Expected file at: {}", skill_path.display())),
                    });
                    readiness
                        .verified_skills
                        .push(ElegyPluginReadinessVerifiedSkill {
                            skill_id: format!("ref:{}", component.id),
                            status: "missing_ref".to_string(),
                        });
                }
            }
            key
        } else {
            continue;
        };
    }

    // Cross-validate capability projections
    for projection in &package.components.capability_projections {
        let cap_ref = format!("{}.{}", projection.skill, projection.capability);
        let known = known_capabilities.get(&projection.skill);

        match known {
            None => {
                findings.push(ElegyPluginReadinessFinding {
                    code: "CAP-SKILL-UNKNOWN".to_string(),
                    severity: "error".to_string(),
                    message: format!(
                        "Projection '{}' references unknown skill '{}'",
                        projection.id, projection.skill
                    ),
                    detail: None,
                });
                readiness.unsupported_capabilities.push(cap_ref.clone());
            }
            Some(caps) => {
                if !caps.contains(&projection.capability) {
                    findings.push(ElegyPluginReadinessFinding {
                        code: "CAP-PHANTOM".to_string(),
                        severity: "error".to_string(),
                        message: format!(
                            "Projection '{}' references capability '{}' not found in skill '{}'",
                            projection.id, projection.capability, projection.skill
                        ),
                        detail: None,
                    });
                    readiness.unsupported_capabilities.push(cap_ref.clone());
                } else {
                    // Projected tool entry
                    let func_name = projection
                        .projection
                        .as_ref()
                        .and_then(|p| p.function_name.clone())
                        .unwrap_or_else(|| {
                            format!(
                                "{}_{}",
                                projection.skill.replace('.', "_"),
                                projection.capability.replace('-', "_")
                            )
                        });
                    readiness
                        .projected_tools
                        .push(ElegyPluginReadinessProjectedTool {
                            tool_name: projection.skill.clone(),
                            function_name: func_name,
                            capability_id: Some(projection.capability.clone()),
                            lane: Some(projection.lane.clone()),
                        });
                }
            }
        }

        // Side-effect loosening check
        if let Some(ref declared_class) = projection.side_effect_class {
            if let Some(skill_hp) = skill_host_projections.get(&projection.skill) {
                let skill_default = skill_hp.default_side_effect_class;
                let skill_per_cap = skill_hp
                    .capability_projections
                    .iter()
                    .find(|cp| cp.capability_id == projection.capability)
                    .and_then(|cp| cp.side_effect_class)
                    .unwrap_or(skill_default);

                let declared_severity = side_effect_severity(declared_class);
                let skill_severity = side_effect_severity_host(&skill_per_cap);

                if declared_severity < skill_severity {
                    findings.push(ElegyPluginReadinessFinding {
                        code: "SIDE-LOOSEN".to_string(),
                        severity: "error".to_string(),
                        message: format!(
                            "Projection '{}' declares sideEffectClass '{}' which is weaker than skill's '{}'",
                            projection.id,
                            declared_class,
                            host_side_effect_to_string(&skill_per_cap)
                        ),
                        detail: None,
                    });
                }
            }
        }
    }

    // Check subset declarations
    let empty_subset = Vec::new();
    let subset_of = package
        .metadata
        .as_ref()
        .map(|m| &m.subset_of)
        .unwrap_or(&empty_subset);
    if !subset_of.is_empty() {
        // All projected capabilities must be in subset_of
        for projection in &package.components.capability_projections {
            if !subset_of.contains(&projection.capability) {
                findings.push(ElegyPluginReadinessFinding {
                    code: "SUBSET-VIOLATION".to_string(),
                    severity: "warning".to_string(),
                    message: format!(
                        "Projection '{}' capability '{}' is not listed in metadata.subsetOf",
                        projection.id, projection.capability
                    ),
                    detail: None,
                });
            }
        }
    } else {
        // Check if subset is implied (not all skill capabilities are projected)
        for (skill_key, caps) in &known_capabilities {
            let projected_caps: std::collections::BTreeSet<String> = package
                .components
                .capability_projections
                .iter()
                .filter(|p| &p.skill == skill_key)
                .map(|p| p.capability.clone())
                .collect();

            if !projected_caps.is_empty() && projected_caps.len() < caps.len() {
                let omitted: Vec<String> = caps.difference(&projected_caps).cloned().collect();
                for cap in &omitted {
                    readiness
                        .omitted_capabilities
                        .push(format!("{}.{}", skill_key, cap));
                }
                if subset_of.is_empty() && !omitted.is_empty() {
                    findings.push(ElegyPluginReadinessFinding {
                        code: "SUBSET-IMPLIED".to_string(),
                        severity: "warning".to_string(),
                        message: format!(
                            "Skill '{}' has {} capabilities but only {} are projected. Consider declaring metadata.subsetOf.",
                            skill_key,
                            caps.len(),
                            projected_caps.len()
                        ),
                        detail: Some(format!("Omitted: {}", omitted.join(", "))),
                    });
                }
            }
        }
    }

    // Compute side-effect summary
    let mut side_effect_summary = ElegyPluginReadinessSideEffectSummary::default();
    for projection in &package.components.capability_projections {
        let se_class = projection
            .side_effect_class
            .as_deref()
            .unwrap_or("read_only");
        match se_class {
            "none" => side_effect_summary.none += 1,
            "read_only" => side_effect_summary.read_only += 1,
            "disk_read" => side_effect_summary.disk_read += 1,
            "disk_write" => side_effect_summary.disk_write += 1,
            "network_outbound" => side_effect_summary.network_outbound += 1,
            "process_spawn" => side_effect_summary.process_spawn += 1,
            "desktop_ui" => side_effect_summary.desktop_ui += 1,
            _ => {}
        }
    }
    readiness.side_effect_summary = Some(side_effect_summary);

    // Determine overall readiness
    let has_errors = findings.iter().any(|f| f.severity == "error");
    let has_warnings = findings.iter().any(|f| f.severity == "warning");

    readiness.readiness = if has_errors {
        "blocked".to_string()
    } else if has_warnings {
        "partial".to_string()
    } else {
        "ready".to_string()
    };
    readiness.findings = findings;

    Ok(readiness)
}

/// Determine severity of a side-effect class string (higher = more restrictive).
fn side_effect_severity(class: &str) -> i32 {
    match class {
        "none" => 0,
        "read_only" => 1,
        "disk_read" => 2,
        "disk_write" => 3,
        "network_outbound" => 4,
        "process_spawn" => 5,
        "desktop_ui" => 6,
        _ => 0,
    }
}

/// Determine severity from HostSideEffectClass enum.
fn side_effect_severity_host(class: &HostSideEffectClass) -> i32 {
    match class {
        HostSideEffectClass::None => 0,
        HostSideEffectClass::ReadOnly => 1,
        HostSideEffectClass::DiskRead => 2,
        HostSideEffectClass::DiskWrite => 3,
        HostSideEffectClass::NetworkOutbound => 4,
        HostSideEffectClass::ProcessSpawn => 5,
        HostSideEffectClass::DesktopUi => 6,
    }
}

fn host_side_effect_to_string(class: &HostSideEffectClass) -> String {
    match class {
        HostSideEffectClass::None => "none".to_string(),
        HostSideEffectClass::ReadOnly => "read_only".to_string(),
        HostSideEffectClass::DiskRead => "disk_read".to_string(),
        HostSideEffectClass::DiskWrite => "disk_write".to_string(),
        HostSideEffectClass::NetworkOutbound => "network_outbound".to_string(),
        HostSideEffectClass::ProcessSpawn => "process_spawn".to_string(),
        HostSideEffectClass::DesktopUi => "desktop_ui".to_string(),
    }
}

/// Load a skill definition from a JSON file.
fn load_skill_definition_file(path: &Path) -> Result<SkillDefinitionV2, ToolingError> {
    let content = fs::read_to_string(path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: path.to_path_buf(),
        source,
    })?;
    let skill: SkillDefinitionV2 =
        serde_json::from_str(&content).map_err(|source| ToolingError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(skill)
}

/// Check whether the tools required by a plugin package are installed and probeable.
/// Reads an install receipt and optionally probes binaries.
pub fn check_plugin_installation(
    package_path: &Path,
    install_receipt_path: &Path,
    bin_dir: Option<&Path>,
    skip_probe: bool,
    package_root: Option<&Path>,
) -> Result<ElegyPluginReadinessV1, ToolingError> {
    let package = load_plugin_package_file(package_path)?;
    let _root = package_root.unwrap_or_else(|| package_path.parent().unwrap_or(Path::new(".")));

    // Load install receipt
    let receipt_content =
        fs::read_to_string(install_receipt_path).map_err(|source| ToolingError::Io {
            operation: "read",
            path: install_receipt_path.to_path_buf(),
            source,
        })?;
    let receipt: ElegyPluginInstallReceiptV1 =
        serde_json::from_str(&receipt_content).map_err(|source| ToolingError::Json {
            path: install_receipt_path.to_path_buf(),
            source,
        })?;

    let mut readiness = ElegyPluginReadinessV1 {
        schema_version: "elegy-plugin-readiness/v1".to_string(),
        package_identity: ElegyPluginReadinessPackageIdentity {
            package_id: package.identity.package_id.clone(),
            name: package.identity.name.clone(),
            version: package.identity.version.clone(),
        },
        readiness: "ready".to_string(),
        ..Default::default()
    };

    let mut findings: Vec<ElegyPluginReadinessFinding> = Vec::new();

    for tool_req in &package.components.tool_requirements {
        let receipt_binary = receipt
            .installed_binaries
            .iter()
            .find(|b| b.tool_name == tool_req.tool_name);

        let resolved_path = resolve_binary_path(
            receipt_binary.map(|rb| rb.binary_path.as_str()),
            bin_dir,
            &tool_req.cli_binary,
        );

        let probe_target = match resolved_path {
            Some(ref path) => path.clone(),
            None => {
                readiness
                    .tool_statuses
                    .push(ElegyPluginReadinessToolStatus {
                        tool_name: tool_req.tool_name.clone(),
                        cli_binary: Some(tool_req.cli_binary.clone()),
                        status: "missing".to_string(),
                        detail: Some(format!(
                            "Binary '{}' not found in install receipt, bin_dir, or PATH",
                            tool_req.cli_binary
                        )),
                        ..Default::default()
                    });
                findings.push(ElegyPluginReadinessFinding {
                    code: "BIN-MISSING".to_string(),
                    severity: "error".to_string(),
                    message: format!(
                        "Required tool '{}' (binary '{}') is not installed",
                        tool_req.tool_name, tool_req.cli_binary
                    ),
                    detail: None,
                });
                continue;
            }
        };

        if skip_probe {
            readiness
                .tool_statuses
                .push(ElegyPluginReadinessToolStatus {
                    tool_name: tool_req.tool_name.clone(),
                    cli_binary: Some(tool_req.cli_binary.clone()),
                    status: "unprobed".to_string(),
                    detail: Some("Probe skipped by --skip-probe flag".to_string()),
                    ..Default::default()
                });
            findings.push(ElegyPluginReadinessFinding {
                code: "READINESS-PROBE-SKIPPED".to_string(),
                severity: "warning".to_string(),
                message: format!(
                    "Tool '{}' was not probed due to --skip-probe",
                    tool_req.tool_name
                ),
                detail: None,
            });
        } else {
            let probe_cmd = tool_req.probe_command.as_deref().unwrap_or("--version");
            let probe_result = probe_binary(&probe_target, probe_cmd);

            match probe_result {
                ProbeResult::Success { output } => {
                    readiness
                        .tool_statuses
                        .push(ElegyPluginReadinessToolStatus {
                            tool_name: tool_req.tool_name.clone(),
                            cli_binary: Some(tool_req.cli_binary.clone()),
                            status: "present".to_string(),
                            probe_output: Some(output),
                            ..Default::default()
                        });
                }
                ProbeResult::Failed { error } => {
                    readiness
                        .tool_statuses
                        .push(ElegyPluginReadinessToolStatus {
                            tool_name: tool_req.tool_name.clone(),
                            cli_binary: Some(tool_req.cli_binary.clone()),
                            status: "broken".to_string(),
                            detail: Some(error.clone()),
                            ..Default::default()
                        });
                    findings.push(ElegyPluginReadinessFinding {
                        code: "BIN-BROKEN".to_string(),
                        severity: "error".to_string(),
                        message: format!(
                            "Tool '{}' found but probe failed: {}",
                            tool_req.tool_name, error
                        ),
                        detail: None,
                    });
                }
                ProbeResult::NotFound => {
                    readiness
                        .tool_statuses
                        .push(ElegyPluginReadinessToolStatus {
                            tool_name: tool_req.tool_name.clone(),
                            cli_binary: Some(tool_req.cli_binary.clone()),
                            status: "missing".to_string(),
                            ..Default::default()
                        });
                    findings.push(ElegyPluginReadinessFinding {
                        code: "BIN-MISSING".to_string(),
                        severity: "error".to_string(),
                        message: format!("Required tool '{}' not found", tool_req.tool_name),
                        detail: None,
                    });
                }
            }
        }
    }

    let has_errors = findings.iter().any(|f| f.severity == "error");
    let has_warnings = findings.iter().any(|f| f.severity == "warning");
    readiness.readiness = if has_errors {
        "blocked"
    } else if has_warnings {
        "partial"
    } else {
        "ready"
    }
    .to_string();
    readiness.findings = findings;

    Ok(readiness)
}

/// Resolve the actual binary path for a tool requirement.
///
/// Resolution order:
/// 1. `install_receipt.binaryPath` when present and the file exists.
/// 2. `--bin-dir/<cliBinary[.exe]>` when `--bin-dir` is supplied and the file exists.
/// 3. `cliBinary` found via `PATH` (returns the name as-is for Command lookup).
fn resolve_binary_path(
    receipt_binary_path: Option<&str>,
    bin_dir: Option<&Path>,
    cli_binary: &str,
) -> Option<PathBuf> {
    // Candidate binary names to check on disk
    let candidates: Vec<String> = if cfg!(windows) {
        vec![
            format!("{}.exe", cli_binary),
            format!("{}.cmd", cli_binary),
            format!("{}.bat", cli_binary),
            cli_binary.to_string(),
        ]
    } else {
        vec![cli_binary.to_string()]
    };

    // 1. Receipt binary path
    if let Some(receipt_path) = receipt_binary_path {
        let path = PathBuf::from(receipt_path);
        if path.is_file() {
            return Some(path);
        }
    }

    // 2. bin_dir binary path
    if let Some(bd) = bin_dir {
        for candidate in &candidates {
            let path = bd.join(candidate);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    // 3. PATH lookup
    if probe_binary_in_path(cli_binary) {
        return Some(PathBuf::from(cli_binary));
    }

    None
}

enum ProbeResult {
    Success { output: String },
    Failed { error: String },
    NotFound,
}

fn probe_binary_in_path(binary_name: &str) -> bool {
    if cfg!(windows) {
        std::process::Command::new("where")
            .arg(binary_name)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    } else {
        std::process::Command::new("which")
            .arg(binary_name)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

fn probe_binary(target: &Path, probe_arg: &str) -> ProbeResult {
    let result = std::process::Command::new(target)
        .arg(probe_arg)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();

    match result {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let combined = if stdout.is_empty() { stderr } else { stdout };
                // Truncate to max 4KB
                let truncated = if combined.len() > 4096 {
                    format!("{}... (truncated)", &combined[..4096])
                } else {
                    combined
                };
                ProbeResult::Success { output: truncated }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                ProbeResult::Failed { error: stderr }
            }
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                ProbeResult::NotFound
            } else {
                ProbeResult::Failed {
                    error: e.to_string(),
                }
            }
        }
    }
}

/// Supported plugin template kinds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginTemplateKind {
    SkillOnly,
    CliTool,
    McpTool,
    Configuration,
    Mixed,
    RustCli,
    RustHarness,
}

impl std::str::FromStr for PluginTemplateKind {
    type Err = ToolingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "skill-only" => Ok(Self::SkillOnly),
            "cli-tool" => Ok(Self::CliTool),
            "mcp-tool" => Ok(Self::McpTool),
            "configuration" => Ok(Self::Configuration),
            "mixed" => Ok(Self::Mixed),
            "rust-cli" => Ok(Self::RustCli),
            "rust-harness" => Ok(Self::RustHarness),
            _ => Err(ToolingError::InvalidPluginPackage {
                path: PathBuf::from(s),
                issues: vec![format!(
                    "Unknown template kind '{}'. Valid options: skill-only, cli-tool, mcp-tool, configuration, mixed, rust-cli, rust-harness",
                    s
                )],
            }),
        }
    }
}

/// Scaffold a new plugin package directory from a template.
pub fn scaffold_plugin_package(
    template: PluginTemplateKind,
    output_dir: &Path,
    package_name: &str,
    package_version: &str,
) -> Result<Vec<String>, ToolingError> {
    if output_dir.exists() {
        return Err(ToolingError::OutputExists {
            path: output_dir.to_path_buf(),
        });
    }

    let mut written_files = Vec::new();
    let package_id = format!("elegy.{}", package_name);

    // Build the plugin.json
    let plugin_json =
        build_scaffold_plugin_json(&template, package_name, package_version, &package_id);
    let plugin_json_path = output_dir.join("plugin.json");
    write_json_file(&plugin_json_path, &plugin_json, false)?;
    written_files.push(display_path(&plugin_json_path));

    // Create template-specific directories
    let dirs_to_create = match template {
        PluginTemplateKind::SkillOnly => vec!["skills", "docs"],
        PluginTemplateKind::CliTool => vec!["skills", "docs", "contracts"],
        PluginTemplateKind::McpTool => vec!["skills", "docs", "contracts", "mcp"],
        PluginTemplateKind::Configuration => vec!["configuration", "docs"],
        PluginTemplateKind::Mixed => {
            vec!["skills", "docs", "contracts", "mcp", "configuration"]
        }
        PluginTemplateKind::RustCli => {
            vec!["skills", "docs", "contracts", "rust", "rust/src"]
        }
        PluginTemplateKind::RustHarness => {
            vec!["skills", "docs", "contracts", "rust", "rust/src"]
        }
    };

    // Common directories created for all template kinds
    let common_dirs = [
        "contracts/fixtures",
        "contracts/schemas",
        ".github/workflows",
    ];

    let all_dirs: Vec<&str> = dirs_to_create
        .iter()
        .chain(common_dirs.iter())
        .copied()
        .collect();

    for dir in &all_dirs {
        let dir_path = output_dir.join(dir);
        fs::create_dir_all(&dir_path).map_err(|source| ToolingError::Io {
            operation: "create directory",
            path: dir_path,
            source,
        })?;
    }

    // Create a README.md
    let readme_description = match template {
        PluginTemplateKind::SkillOnly => "A skill-only plugin package.",
        PluginTemplateKind::CliTool => "A CLI tool plugin package.",
        PluginTemplateKind::McpTool => "An MCP tool plugin package.",
        PluginTemplateKind::Configuration => "A configuration plugin package.",
        PluginTemplateKind::Mixed => {
            "A mixed plugin package with skills, tools, and configurations."
        }
        PluginTemplateKind::RustCli => "A Rust CLI plugin package.",
        PluginTemplateKind::RustHarness => "A Rust harness adapter plugin package.",
    };
    let readme_content = format!(
        "# {}\n\n{}\n\n## Overview\n\nThis is an Elegy plugin package.\n\n## Components\n\n",
        package_name, readme_description,
    );
    let readme_path = output_dir.join("README.md");
    fs::write(&readme_path, &readme_content).map_err(|source| ToolingError::Io {
        operation: "write",
        path: readme_path.clone(),
        source,
    })?;
    written_files.push(display_path(&readme_path));

    // ── Common generated files (all templates) ──────────────────────────

    // elegy-plugin.lock.json
    let lock = serde_json::json!({
        "schemaVersion": "elegy-plugin-lock/v1",
        "lockVersion": 1,
        "elegyCompatibility": {
            "contractBundleVersion": "1.8.0",
            "schemaLine": "1.x"
        },
        "generatedAt": "2026-06-12T00:00:00Z",
        "generatedBy": "elegy-cli",
        "pluginPackageRef": "elegy-plugin-package.json"
    });
    let lock_path = output_dir.join("elegy-plugin.lock.json");
    write_json_file(&lock_path, &lock, false)?;
    written_files.push(display_path(&lock_path));

    // contracts/fixtures/skill.<package-name>.json — minimal skill fixture
    let skill_fixture = serde_json::json!({
        "schemaVersion": "elegy-skill-definition",
        "skillVersion": 2,
        "identity": {
            "namespace": "elegy",
            "name": package_name,
            "version": package_version
        },
        "capabilities": [
            {
                "id": format!("{}.default", package_name),
                "name": "Default capability",
                "description": "Scaffolded default capability for the plugin package."
            }
        ],
        "lifecycleState": "draft"
    });
    let skill_fixture_path =
        output_dir.join(format!("contracts/fixtures/skill.{}.json", package_name));
    write_json_file(&skill_fixture_path, &skill_fixture, false)?;
    written_files.push(display_path(&skill_fixture_path));

    // .github/workflows/plugin-ci.yml
    let ci_workflow = concat!(
        "name: Plugin CI\n",
        "on: [push, pull_request]\n",
        "jobs:\n",
        "  validate:\n",
        "    runs-on: ubuntu-latest\n",
        "    steps:\n",
        "      - uses: actions/checkout@v4\n",
        "      - name: Verify plugin\n",
        "        run: elegy plugin verify --package elegy-plugin-package.json\n",
    )
    .to_string();
    let ci_path = output_dir.join(".github/workflows/plugin-ci.yml");
    fs::write(&ci_path, &ci_workflow).map_err(|source| ToolingError::Io {
        operation: "write",
        path: ci_path.clone(),
        source,
    })?;
    written_files.push(display_path(&ci_path));

    // ── Template-specific generated files ───────────────────────────────

    match template {
        PluginTemplateKind::RustCli => {
            // rust/Cargo.toml
            let cargo_toml = format!(
                concat!(
                    "[package]\n",
                    "name = \"{}-cli\"\n",
                    "version = \"0.1.0\"\n",
                    "edition = \"2021\"\n",
                    "\n",
                    "[dependencies]\n",
                    "clap = {{ version = \"4\", features = [\"derive\"] }}\n",
                    "serde = {{ version = \"1\", features = [\"derive\"] }}\n",
                    "serde_json = \"1\"\n",
                    "\n",
                    "[[bin]]\n",
                    "name = \"{}\"\n",
                    "path = \"src/main.rs\"\n",
                ),
                package_name, package_name,
            );
            let cargo_path = output_dir.join("rust/Cargo.toml");
            fs::write(&cargo_path, &cargo_toml).map_err(|source| ToolingError::Io {
                operation: "write",
                path: cargo_path.clone(),
                source,
            })?;
            written_files.push(display_path(&cargo_path));

            // rust/src/main.rs
            let main_rs = format!(
                "fn main() {{\n    println!(\"{} CLI v0.1.0\");\n}}\n",
                package_name
            );
            let main_path = output_dir.join("rust/src/main.rs");
            fs::write(&main_path, &main_rs).map_err(|source| ToolingError::Io {
                operation: "write",
                path: main_path.clone(),
                source,
            })?;
            written_files.push(display_path(&main_path));
        }
        PluginTemplateKind::RustHarness => {
            // rust/Cargo.toml
            let cargo_toml = format!(
                concat!(
                    "[package]\n",
                    "name = \"{}-adapter\"\n",
                    "version = \"0.1.0\"\n",
                    "edition = \"2021\"\n",
                    "\n",
                    "[dependencies]\n",
                    "serde = {{ version = \"1\", features = [\"derive\"] }}\n",
                    "serde_json = \"1\"\n",
                    "\n",
                    "[lib]\n",
                    "name = \"{}_adapter\"\n",
                    "path = \"src/lib.rs\"\n",
                ),
                package_name, package_name,
            );
            let cargo_path = output_dir.join("rust/Cargo.toml");
            fs::write(&cargo_path, &cargo_toml).map_err(|source| ToolingError::Io {
                operation: "write",
                path: cargo_path.clone(),
                source,
            })?;
            written_files.push(display_path(&cargo_path));

            // rust/src/lib.rs
            let lib_rs = concat!(
                "/// Register tool adapters for the host harness.\n",
                "/// Called by the host during harness initialization.\n",
                "pub fn register_tools() -> Vec<ToolAdapter> {{\n",
                "    vec![]\n",
                "}}\n",
                "\n",
                "/// A tool adapter registered by this plugin.\n",
                "pub struct ToolAdapter {{\n",
                "    pub name: String,\n",
                "    pub handler: String,\n",
                "}}\n",
            )
            .to_string();
            let lib_path = output_dir.join("rust/src/lib.rs");
            fs::write(&lib_path, &lib_rs).map_err(|source| ToolingError::Io {
                operation: "write",
                path: lib_path.clone(),
                source,
            })?;
            written_files.push(display_path(&lib_path));
        }
        _ => {}
    }

    Ok(written_files)
}

fn build_scaffold_plugin_json(
    template: &PluginTemplateKind,
    package_name: &str,
    package_version: &str,
    package_id: &str,
) -> serde_json::Value {
    let mut components = serde_json::json!({});

    match template {
        PluginTemplateKind::SkillOnly | PluginTemplateKind::CliTool | PluginTemplateKind::Mixed => {
            components["skillDefinitions"] = serde_json::json!([{
                "id": format!("{}-skill", package_name),
                "definition": {
                    "skillFormat": "elegy-skill-definition",
                    "skillVersion": 2,
                    "identity": {
                        "namespace": "elegy",
                        "name": package_name,
                        "version": package_version
                    },
                    "capabilities": [],
                    "lifecycleState": "draft"
                }
            }]);
        }
        PluginTemplateKind::McpTool => {
            components["skillDefinitions"] = serde_json::json!([{
                "id": format!("{}-skill", package_name),
                "definition": {
                    "skillFormat": "elegy-skill-definition",
                    "skillVersion": 2,
                    "identity": {
                        "namespace": "elegy",
                        "name": package_name,
                        "version": package_version
                    },
                    "capabilities": [],
                    "lifecycleState": "draft"
                }
            }]);
            components["mcpProjections"] = serde_json::json!([{
                "id": format!("{}-mcp", package_name),
                "serverName": package_name
            }]);
        }
        PluginTemplateKind::Configuration => {
            components["configurationTemplates"] = serde_json::json!([{
                "id": format!("{}-template", package_name),
                "path": format!("configuration/{}-template.json", package_name),
                "description": format!("Default configuration template for {}", package_name)
            }]);
        }
        PluginTemplateKind::RustCli => {
            components["skillDefinitions"] = serde_json::json!([{
                "id": format!("{}-skill", package_name),
                "definition": {
                    "skillFormat": "elegy-skill-definition",
                    "skillVersion": 2,
                    "identity": {
                        "namespace": "elegy",
                        "name": package_name,
                        "version": package_version
                    },
                    "capabilities": [],
                    "lifecycleState": "draft"
                }
            }]);
            components["cliHelpers"] = serde_json::json!([{
                "id": format!("{}-cli", package_name),
                "description": format!("{} CLI helper", package_name),
                "binary": package_name
            }]);
            components["capabilityProjections"] = serde_json::json!([{
                "id": format!("{}-cli-projection", package_name),
                "capabilityRef": format!("{}.default", package_name),
                "lane": "subprocess",
                "help": format!("{}-cli", package_name)
            }]);
        }
        PluginTemplateKind::RustHarness => {
            components["skillDefinitions"] = serde_json::json!([{
                "id": format!("{}-skill", package_name),
                "definition": {
                    "skillFormat": "elegy-skill-definition",
                    "skillVersion": 2,
                    "identity": {
                        "namespace": "elegy",
                        "name": package_name,
                        "version": package_version
                    },
                    "capabilities": [],
                    "lifecycleState": "draft"
                }
            }]);
            components["rustToolAdapters"] = serde_json::json!([{
                "id": format!("{}-adapter", package_name),
                "crateName": format!("{}-adapter", package_name),
                "adapterPath": "rust/src/lib.rs",
                "registerFn": "register_tools"
            }]);
            components["capabilityProjections"] = serde_json::json!([{
                "id": format!("{}-rust-projection", package_name),
                "capabilityRef": format!("{}.default", package_name),
                "lane": "rust",
                "adapterId": format!("{}-adapter", package_name)
            }]);
        }
    }

    // Add docs
    components["docs"] = serde_json::json!([{
        "id": "readme",
        "path": "README.md"
    }]);

    serde_json::json!({
        "schemaVersion": "elegy-plugin-package/v1",
        "identity": {
            "packageId": package_id,
            "name": package_name,
            "version": package_version,
            "displayName": format!("Elegy {} Plugin", package_name)
        },
        "metadata": {
            "description": format!("A {} Elegy plugin package.", match template {
                PluginTemplateKind::SkillOnly => "skill-only",
                PluginTemplateKind::CliTool => "CLI tool",
                PluginTemplateKind::McpTool => "MCP tool",
                PluginTemplateKind::Configuration => "configuration",
                PluginTemplateKind::Mixed => "mixed",
                PluginTemplateKind::RustCli => "Rust CLI",
                PluginTemplateKind::RustHarness => "Rust harness",
            }),
            "license": "MIT"
        },
        "elegyCompatibility": {
            "contractBundleVersion": "1.8.0",
            "schemaLine": "1.x"
        },
        "components": components
    })
}

/// Inspect a plugin package and return its metadata as a JSON value.
pub fn inspect_plugin_package(package_path: &Path) -> Result<serde_json::Value, ToolingError> {
    let package = load_plugin_package_file(package_path)?;

    let skill_count = package.components.skill_definitions.len();
    let projection_count = package.components.capability_projections.len();
    let mcp_count = package.components.mcp_projections.len();
    let config_template_count = package.components.configuration_templates.len();
    let config_profile_count = package.components.configuration_profiles.len();
    let tool_req_count = package.components.tool_requirements.len();
    let instruction_count = package.components.instruction_skills.len();
    let doc_count = package.components.docs.len();

    Ok(serde_json::json!({
        "schemaVersion": package.schema_version,
        "identity": {
            "packageId": package.identity.package_id,
            "name": package.identity.name,
            "version": package.identity.version,
            "displayName": package.identity.display_name
        },
        "summary": {
            "skillCount": skill_count,
            "capabilityProjectionCount": projection_count,
            "mcpProjectionCount": mcp_count,
            "configurationTemplateCount": config_template_count,
            "configurationProfileCount": config_profile_count,
            "toolRequirementCount": tool_req_count,
            "instructionSkillCount": instruction_count,
            "docCount": doc_count
        }
    }))
}

/// Pack a plugin package source directory into a portable zip archive.
pub fn pack_plugin_package(source_dir: &Path, output_zip: &Path) -> Result<String, ToolingError> {
    // Verify plugin.json exists
    let plugin_json_path = source_dir.join("plugin.json");
    if !plugin_json_path.exists() {
        return Err(ToolingError::Io {
            operation: "find plugin.json",
            path: plugin_json_path,
            source: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "plugin.json not found in source directory",
            ),
        });
    }

    // Validate the package before packing
    let _package = load_plugin_package_file(&plugin_json_path)?;

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
    let entries = walk_dir_for_pack(source_dir)?;

    for entry_path in &entries {
        let relative = entry_path.strip_prefix(source_dir).unwrap_or(entry_path);
        let relative_str = relative.to_string_lossy().replace('\\', "/");

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

fn walk_dir_for_pack(dir: &Path) -> Result<Vec<PathBuf>, ToolingError> {
    let mut entries = Vec::new();
    walk_dir_recursive(dir, dir, &mut entries)?;
    Ok(entries)
}

#[allow(clippy::only_used_in_recursion)]
fn walk_dir_recursive(
    base: &Path,
    dir: &Path,
    entries: &mut Vec<PathBuf>,
) -> Result<(), ToolingError> {
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
        entries.push(path.clone());
        if path.is_dir() {
            walk_dir_recursive(base, &path, entries)?;
        }
    }
    Ok(())
}

/// Project a plugin package for a specific host target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostTarget {
    Codex,
    OpenCode,
    Generic,
}

impl std::str::FromStr for HostTarget {
    type Err = ToolingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "codex" => Ok(Self::Codex),
            "opencode" => Ok(Self::OpenCode),
            "generic" => Ok(Self::Generic),
            _ => Err(ToolingError::InvalidPluginPackage {
                path: PathBuf::from(s),
                issues: vec![format!(
                    "Unknown host target '{}'. Valid options: codex, opencode, generic",
                    s
                )],
            }),
        }
    }
}

/// Project a plugin package for a specific host, emitting host-specific files.
pub fn project_plugin_for_host(
    package_path: &Path,
    host: HostTarget,
    output_dir: &Path,
    overwrite: bool,
    package_root: Option<&Path>,
) -> Result<GeneratedCodexPluginArtifacts, ToolingError> {
    match host {
        HostTarget::Codex => {
            generate_codex_plugin_from_package_file(package_path, output_dir, overwrite)
        }
        HostTarget::OpenCode | HostTarget::Generic => {
            project_generic_host_plugin(package_path, host, output_dir, overwrite, package_root)
        }
    }
}

fn project_generic_host_plugin(
    package_path: &Path,
    host: HostTarget,
    output_dir: &Path,
    overwrite: bool,
    package_root: Option<&Path>,
) -> Result<GeneratedCodexPluginArtifacts, ToolingError> {
    let package = load_plugin_package_file(package_path)?;
    let _root = package_root.unwrap_or_else(|| package_path.parent().unwrap_or(Path::new(".")));
    let plugin_output_name = package.identity.name.trim();

    let host_name = match host {
        HostTarget::OpenCode => "opencode",
        HostTarget::Generic => "generic",
        _ => "generic",
    };

    let plugin_root = output_dir.join(plugin_output_name);
    let manifest_dir = plugin_root.join(format!(".elegy-host-{}", host_name));
    let manifest_path = manifest_dir.join("plugin.json");

    let target_paths = vec![manifest_path.clone()];

    if overwrite {
        if plugin_root.exists() {
            fs::remove_dir_all(&plugin_root).map_err(|source| ToolingError::Io {
                operation: "remove",
                path: plugin_root.clone(),
                source,
            })?;
        }
    } else {
        preflight_output_paths(&target_paths, overwrite)?;
    }

    // Build a generic host manifest
    let host_manifest = serde_json::json!({
        "schemaVersion": "elegy-host-projection/v1",
        "host": host_name,
        "package": {
            "packageId": package.identity.package_id,
            "name": package.identity.name,
            "version": package.identity.version,
            "displayName": package.identity.display_name
        },
        "skills": package.components.skill_definitions.iter().map(|sd| {
            serde_json::json!({
                "id": sd.id,
                "hasDefinition": sd.definition.is_some(),
                "hasDefinitionRef": sd.definition_ref.is_some()
            })
        }).collect::<Vec<_>>(),
        "capabilityProjections": package.components.capability_projections.iter().map(|cp| {
            serde_json::json!({
                "id": cp.id,
                "skill": cp.skill,
                "capability": cp.capability,
                "lane": cp.lane,
                "functionName": cp.projection.as_ref().and_then(|p| p.function_name.clone())
            })
        }).collect::<Vec<_>>(),
        "toolRequirements": package.components.tool_requirements.iter().map(|tr| {
            serde_json::json!({
                "toolName": tr.tool_name,
                "cliBinary": tr.cli_binary
            })
        }).collect::<Vec<_>>()
    });

    write_json_file(&manifest_path, &host_manifest, overwrite)?;

    let written_files = vec![display_path(&manifest_path)];

    Ok(GeneratedCodexPluginArtifacts {
        source_package: display_path(package_path),
        plugin_name: plugin_output_name.to_string(),
        plugin_version: package.identity.version.clone(),
        emitted_components: GeneratedCodexPluginComponents {
            plugin_manifest: display_path(&manifest_path),
            skills_dir: String::new(),
            skills_count: 0,
            apps_emitted: false,
            mcp_servers_emitted: false,
            hooks_emitted: false,
        },
        written_files,
    })
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

fn load_plugin_package_file(path: &Path) -> Result<ElegyPluginPackage, ToolingError> {
    let content = fs::read_to_string(path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: path.to_path_buf(),
        source,
    })?;

    let package = serde_json::from_str::<ElegyPluginPackage>(&content).map_err(|source| {
        ToolingError::Json {
            path: path.to_path_buf(),
            source,
        }
    })?;

    let validation = validate_elegy_plugin_package(&package);
    if !validation.is_valid() {
        return Err(ToolingError::InvalidPluginPackage {
            path: path.to_path_buf(),
            issues: validation.issues,
        });
    }

    Ok(package)
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

fn build_codex_plugin_manifest(package: &ElegyPluginPackage) -> CodexPluginManifest {
    let description = package
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.description.clone())
        .unwrap_or_else(|| {
            format!(
                "Derived Codex plugin projection for the portable Elegy package '{}'.",
                package.identity.name
            )
        });

    let display_name = package
        .identity
        .display_name
        .clone()
        .unwrap_or_else(|| package.identity.name.clone());

    let developer_name = package
        .components
        .skill_definitions
        .iter()
        .filter_map(|component| component.definition.as_ref())
        .find_map(|definition| {
            definition
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.author.clone())
        });

    CodexPluginManifest {
        name: package.identity.name.clone(),
        version: package.identity.version.clone(),
        description: description.clone(),
        author: developer_name
            .clone()
            .map(|name| CodexPluginAuthor { name }),
        homepage: package
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.homepage.clone()),
        license: package
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.license.clone()),
        documentation_uri: package
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.documentation_uri.clone()),
        keywords: package
            .metadata
            .as_ref()
            .map(|metadata| metadata.tags.clone())
            .unwrap_or_default(),
        skills: "./skills/".to_string(),
        interface: Some(CodexPluginInterface {
            display_name,
            short_description: description,
            developer_name,
        }),
    }
}

fn collect_codex_skill_documents(
    package: &ElegyPluginPackage,
    package_root: &Path,
) -> Result<Vec<CodexSkillDocument>, ToolingError> {
    let mut documents = Vec::new();
    let mut seen_names = BTreeSet::new();

    for component in &package.components.skill_definitions {
        let Some(definition) = load_package_skill_definition(component, package_root)? else {
            continue;
        };

        let document =
            render_codex_skill_document(&definition, &package.components.capability_projections);
        let normalized = document.directory_name.to_ascii_lowercase();
        if !seen_names.insert(normalized) {
            return Err(ToolingError::DuplicateSkillId {
                skill_id: document.directory_name,
            });
        }
        documents.push(document);
    }

    for instruction in &package.components.instruction_skills {
        let document = render_instruction_skill_document(instruction, package_root)?;
        let normalized = document.directory_name.to_ascii_lowercase();
        if !seen_names.insert(normalized) {
            return Err(ToolingError::DuplicateSkillId {
                skill_id: document.directory_name,
            });
        }
        documents.push(document);
    }

    Ok(documents)
}

fn load_package_skill_definition(
    component: &elegy_contracts::ElegyPluginPackageSkillDefinitionComponent,
    package_root: &Path,
) -> Result<Option<SkillDefinitionV2>, ToolingError> {
    if let Some(definition) = &component.definition {
        return Ok(Some(definition.clone()));
    }

    let Some(definition_ref) = component.definition_ref.as_ref() else {
        return Ok(None);
    };

    let path = package_root.join(Path::new(definition_ref));
    let content = fs::read_to_string(&path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: path.clone(),
        source,
    })?;

    let definition = serde_json::from_str::<SkillDefinitionV2>(&content).map_err(|source| {
        ToolingError::Json {
            path: path.clone(),
            source,
        }
    })?;

    if let Err(error) = validate_skill_definition_v2(&definition) {
        return Err(ToolingError::InvalidSkillDefinition {
            skill_id: component.id.clone(),
            issues: vec![error.to_string()],
        });
    }

    Ok(Some(definition))
}

fn render_codex_skill_document(
    definition: &SkillDefinitionV2,
    capability_projections: &[ElegyPluginPackageCapabilityProjectionComponent],
) -> CodexSkillDocument {
    let name = definition.identity.name.trim().to_string();
    let title = definition
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.display_name.as_deref())
        .or(definition.identity.display_name.as_deref())
        .unwrap_or(name.as_str())
        .to_string();
    let description = skill_description(definition);

    let skill_ref = format!(
        "{}.{}",
        definition.identity.namespace, definition.identity.name
    );
    let plugin_capabilities = capability_projections
        .iter()
        .filter(|projection| projection.skill == skill_ref)
        .collect::<Vec<_>>();

    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&format!("name: {}\n", yaml_scalar(&name)));
    content.push_str(&format!("description: {}\n", yaml_quoted(&description)));
    content.push_str("---\n\n");
    content.push_str(&format!("# {}\n\n", title));
    content.push_str("This file is a derived Codex skill projection generated from governed Elegy package metadata.\n\n");
    content.push_str("## When to use\n\n");
    content.push_str(&format!("- {}\n", description));
    if let Some(category) = definition
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.category.as_deref())
    {
        content.push_str(&format!("- Category: `{category}`.\n"));
    }
    if !definition.identity.aliases.is_empty() {
        content.push_str(&format!(
            "- Aliases: {}.\n",
            definition
                .identity
                .aliases
                .iter()
                .map(|alias| format!("`{alias}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if !definition.capabilities.is_empty() {
        content.push_str("\n## Capabilities\n\n");
        for capability in &definition.capabilities {
            content.push_str(&format!(
                "- `{}`: {}\n",
                capability.id, capability.description
            ));
        }
    }

    if !plugin_capabilities.is_empty() {
        content.push_str("\n## Projection Hints\n\n");
        for projection in plugin_capabilities {
            let mut details = vec![format!("lane `{}`", projection.lane)];
            if let Some(projection_metadata) = &projection.projection {
                if let Some(function_name) = &projection_metadata.function_name {
                    details.push(format!("function `{function_name}`"));
                }
                if let Some(mcp_tool_name) = &projection_metadata.mcp_tool_name {
                    details.push(format!("mcp tool `{mcp_tool_name}`"));
                }
            }
            content.push_str(&format!(
                "- `{}` projects as {}.\n",
                projection.capability,
                details.join(", ")
            ));
        }
    }

    if let Some(doc_uri) = definition
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.documentation_uri.as_deref())
    {
        content.push_str("\n## References\n\n");
        content.push_str(&format!("- Documentation: `{doc_uri}`\n"));
    }

    content.push_str("\n## Boundary\n\n");
    content.push_str("- This Codex skill file is derived output only. Governed skill definitions and package metadata remain authoritative.\n");
    content.push_str("- Host install, auth, trust, approvals, hooks, and connector state remain outside this generated skill file.\n");

    CodexSkillDocument {
        directory_name: projected_governed_skill_directory_name(definition),
        content,
    }
}

fn render_instruction_skill_document(
    component: &ElegyPluginPackagePathComponent,
    package_root: &Path,
) -> Result<CodexSkillDocument, ToolingError> {
    let path = Path::new(&component.path);
    let directory_name = projected_instruction_skill_directory_name(component);

    let source_path = package_root.join(path);
    if source_path.is_file() {
        let content = fs::read_to_string(&source_path).map_err(|source| ToolingError::Io {
            operation: "read",
            path: source_path,
            source,
        })?;

        return Ok(CodexSkillDocument {
            directory_name,
            content,
        });
    }

    let description = component.description.clone().unwrap_or_else(|| {
        format!(
            "Derived instruction skill projection for '{}'.",
            component.id
        )
    });

    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&format!("name: {}\n", yaml_scalar(&directory_name)));
    content.push_str(&format!("description: {}\n", yaml_quoted(&description)));
    content.push_str("---\n\n");
    content.push_str(&format!("# {}\n\n", component.id));
    content.push_str("This file is a derived Codex instruction-skill placeholder generated from portable Elegy package metadata.\n\n");
    content.push_str("## Current status\n\n");
    content.push_str(&format!(
        "- The package declares an instruction skill at `{}`.\n",
        component.path
    ));
    content.push_str("- The source package does not embed the original markdown body, so this projection preserves metadata only.\n");
    content.push_str("- Treat the portable package and any host-local packaged files as the authority for the real instruction content.\n");

    Ok(CodexSkillDocument {
        directory_name,
        content,
    })
}

fn skill_description(definition: &SkillDefinitionV2) -> String {
    definition
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.description.clone())
        .or_else(|| {
            definition.discovery.as_ref().and_then(|discovery| {
                discovery
                    .triggers
                    .first()
                    .and_then(|trigger| trigger.description.clone())
            })
        })
        .unwrap_or_else(|| {
            format!(
                "Use when work needs the '{}' skill capability surface.",
                definition.identity.name
            )
        })
}

fn projected_governed_skill_directory_name(definition: &SkillDefinitionV2) -> String {
    build_projected_skill_directory_name(
        "skill",
        &format!(
            "{}.{}",
            definition.identity.namespace, definition.identity.name
        ),
    )
}

fn projected_instruction_skill_directory_name(
    component: &ElegyPluginPackagePathComponent,
) -> String {
    let normalized_path = component.path.replace('\\', "/");
    let without_skill_file = normalized_path
        .strip_suffix("/SKILL.md")
        .unwrap_or(&normalized_path);
    let without_prefix = without_skill_file
        .strip_prefix("skills/")
        .unwrap_or(without_skill_file);
    let key = if without_prefix.is_empty() {
        component.id.as_str()
    } else {
        without_prefix
    };

    build_projected_skill_directory_name("instruction", key)
}

fn build_projected_skill_directory_name(prefix: &str, key: &str) -> String {
    let encoded_key = encode_case_safe_directory_key(key);
    if encoded_key.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}-{encoded_key}")
    }
}

fn encode_case_safe_directory_key(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.as_bytes() {
        let ch = char::from(*byte);
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' {
            encoded.push(ch);
        } else {
            encoded.push('_');
            encoded.push_str(&format!("{:02x}", byte));
        }
    }

    encoded
}

fn yaml_scalar(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        value.to_string()
    } else {
        yaml_quoted(value)
    }
}

fn yaml_quoted(value: &str) -> String {
    json!(value).to_string()
}

fn descriptor_validation_issues(descriptor: &McpServerDescriptor) -> Vec<String> {
    let mut issues = validate_mcp_server_descriptor(descriptor).issues;
    issues.extend(generator_collision_issues(descriptor));
    issues
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

fn generator_collision_issues(descriptor: &McpServerDescriptor) -> Vec<String> {
    let mut distinct_ids = BTreeSet::new();
    let mut issues = Vec::new();

    for tool in &descriptor.tools {
        let Some(schema) = tool.input_schema.as_ref() else {
            continue;
        };

        if !is_supported_input_schema(schema) {
            continue;
        }

        let skill_id = generated_skill_id(&descriptor.server_name, &tool.name);
        let normalized_skill_id = skill_id.to_ascii_lowercase();
        if !distinct_ids.insert(normalized_skill_id) {
            issues.push(format!(
                "MCP descriptor tools must not collapse to the same generated skill ID; {skill_id} is duplicated."
            ));
        }
    }

    issues
}

fn validate_generated_skills(skills: &[SkillDefinitionV2]) -> Result<(), ToolingError> {
    let mut distinct_ids = BTreeSet::new();

    for skill in skills {
        let skill_id = skill.identity.name.trim();
        let normalized_skill_id = skill_id.to_ascii_lowercase();
        if !distinct_ids.insert(normalized_skill_id) {
            return Err(ToolingError::DuplicateSkillId {
                skill_id: skill_id.to_string(),
            });
        }

        if let Err(error) = validate_skill_definition_v2(skill) {
            return Err(ToolingError::InvalidSkillDefinition {
                skill_id: skill.identity.name.clone(),
                issues: vec![error.to_string()],
            });
        }
    }

    Ok(())
}

fn preflight_output_paths(paths: &[PathBuf], overwrite: bool) -> Result<(), ToolingError> {
    if overwrite {
        return Ok(());
    }

    for path in paths {
        if path.exists() {
            return Err(ToolingError::OutputExists { path: path.clone() });
        }
    }

    Ok(())
}

fn clear_plugin_output_root(path: &Path) -> Result<(), ToolingError> {
    if !path.exists() {
        return Ok(());
    }

    let metadata = fs::metadata(path).map_err(|source| ToolingError::Io {
        operation: "inspect",
        path: path.to_path_buf(),
        source,
    })?;

    if metadata.is_dir() {
        fs::remove_dir_all(path).map_err(|source| ToolingError::Io {
            operation: "remove directory",
            path: path.to_path_buf(),
            source,
        })?;
    } else {
        fs::remove_file(path).map_err(|source| ToolingError::Io {
            operation: "remove file",
            path: path.to_path_buf(),
            source,
        })?;
    }

    Ok(())
}

fn write_skill_files(
    output_dir: &Path,
    skills: &[SkillDefinitionV2],
    overwrite: bool,
) -> Result<Vec<PathBuf>, ToolingError> {
    fs::create_dir_all(output_dir).map_err(|source| ToolingError::Io {
        operation: "create directory",
        path: output_dir.to_path_buf(),
        source,
    })?;

    let target_paths = skills
        .iter()
        .map(|skill| output_dir.join(format!("{}.json", skill.identity.name)))
        .collect::<Vec<_>>();

    preflight_output_paths(&target_paths, overwrite)?;

    let mut written_files = Vec::with_capacity(skills.len());
    for (skill, file_path) in skills.iter().zip(target_paths.iter()) {
        if let Err(error) = write_json_file(file_path, skill, overwrite) {
            if !overwrite {
                cleanup_written_files(&written_files);
            }
            return Err(error);
        }

        written_files.push(file_path.clone());
    }

    Ok(written_files)
}

fn write_text_file(output_path: &Path, content: &str, overwrite: bool) -> Result<(), ToolingError> {
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct CodexSkillDocument {
    directory_name: String,
    content: String,
}

fn write_json_file<T: Serialize>(
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

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

fn cleanup_written_files(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        analyze_mcp_descriptor_file, author_mcp_descriptor_to_path,
        generate_codex_plugin_from_package_file, generate_skills_from_descriptor_file,
        AuthorMcpDescriptorRequest, AuthorMcpToolRequest, ToolingError,
    };
    use elegy_contracts::{validate_mcp_server_descriptor, McpTransportKind};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

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
            generated.generated_skills[0].identity.name,
            "mcp-weather-server-get-weather"
        );
        assert_eq!(generated.skipped_tools.len(), 1);
        assert_eq!(generated.written_files.len(), 1);
        assert!(output_dir
            .join("mcp-weather-server-get-weather.json")
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
        fs::write(
            output_dir.join("mcp-weather-server-list-alerts.json"),
            "{}\n",
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
                .join("mcp-weather-server-get-weather.json")
                .exists(),
            "preflight should block all writes when a collision is detected"
        );
    }

    #[test]
    fn analyze_rejects_generator_id_collisions_for_valid_schema_tools() {
        let temp_dir = unique_temp_dir("elegy-tooling-collision");
        let descriptor_path = temp_dir.join("weather-mcp.json");

        fs::write(
            &descriptor_path,
            r#"{
          "serverName": "weather-server",
          "transport": "stdio",
          "tools": [
            {
              "name": "get-user",
              "description": "Get a user",
              "inputSchema": { "type": "object" }
            },
            {
              "name": "get_user",
              "description": "Get a user through another alias",
              "inputSchema": { "type": "object" }
            }
          ]
        }
        "#,
        )
        .expect("write descriptor fixture");

        let error = analyze_mcp_descriptor_file(&descriptor_path)
            .expect_err("colliding generated skill IDs should be rejected during analysis");

        match error {
            ToolingError::InvalidMcpDescriptor { issues, .. } => {
                assert!(issues
                    .iter()
                    .any(|issue| issue.contains("generated skill ID")));
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn codex_plugin_generation_writes_plugin_manifest_and_skills() {
        let temp_dir = unique_temp_dir("elegy-tooling-codex-plugin");
        let package_path = temp_dir.join("plugin-package.json");
        let output_dir = temp_dir.join("codex-output");

        fs::write(
            &package_path,
            r#"{
  "schemaVersion": "elegy-plugin-package/v1",
  "identity": {
    "packageId": "elegy.demo-plugin",
    "name": "demo-plugin",
    "version": "0.1.0",
    "displayName": "Elegy Demo Plugin"
  },
  "metadata": {
    "description": "Portable package fixture for a governed skill definition and optional MCP projection metadata.",
    "tags": ["plugin", "demo"],
    "license": "MIT",
    "homepage": "https://example.com/demo-plugin"
  },
  "components": {
    "skillDefinitions": [
      {
        "id": "demo-skill",
        "definition": {
          "skillFormat": "elegy-skill-definition",
          "skillVersion": 2,
          "identity": {
            "namespace": "elegy",
            "name": "demo-plugin",
            "version": "0.1.0",
            "displayName": "Demo Plugin Skill"
          },
          "metadata": {
            "displayName": "Demo Plugin Skill",
            "description": "Demonstrates portable plugin package capability metadata.",
            "category": "demo",
            "author": "Elegy",
            "tags": ["plugin", "demo"],
            "documentationUri": "docs/architecture/codex-plugin-projection.md"
          },
          "capabilities": [
            {
              "id": "demo-search",
              "name": "Demo Search",
              "description": "Search demo package data.",
              "implementation": {
                "executionType": "mcp",
                "executableName": "elegy-demo-mcp",
                "arguments": ["search", "--query", "${query}", "--json"]
              },
              "input": {
                "parameters": [
                  {
                    "name": "query",
                    "type": "string",
                    "description": "Search query.",
                    "required": true
                  }
                ]
              },
              "execution": {
                "mode": "requestResponse",
                "isDeterministic": true,
                "hasSideEffects": false,
                "timeoutSeconds": 30
              }
            }
          ],
          "governance": {
            "riskLevel": "low",
            "approvalRequirement": "none",
            "policyRefs": []
          },
          "origin": {
            "materializationKind": "declared",
            "sourceKind": "manual",
            "sourceRef": "contracts/fixtures/elegy-plugin-package-v1.minimal.json"
          },
          "lifecycleState": "active"
        }
      }
    ],
    "instructionSkills": [
      {
        "id": "demo-instructions",
        "path": "skills/demo/SKILL.md",
        "description": "Optional instruction surface derived from the governed skill definition."
      }
    ],
    "mcpProjections": [
      {
        "id": "demo-mcp",
        "serverName": "elegy-demo-mcp",
        "capabilityRefs": [
          {
            "skill": "elegy.demo-plugin",
            "capability": "demo-search"
          }
        ]
      }
    ],
    "capabilityProjections": [
      {
        "id": "demo-search-mcp",
        "skill": "elegy.demo-plugin",
        "capability": "demo-search",
        "lane": "mcp",
        "supportsDryRun": true,
        "sideEffectClass": "none",
        "projection": {
          "projections": ["function_calling", "mcp"],
          "functionName": "demo_search",
          "mcpToolName": "demo.search"
        }
      }
    ]
  }
}
"#,
        )
        .expect("write package fixture");

        let generated = generate_codex_plugin_from_package_file(&package_path, &output_dir, false)
            .expect("codex plugin generation should succeed");

        let plugin_root = output_dir.join("demo-plugin");
        let manifest_path = plugin_root.join(".codex-plugin").join("plugin.json");
        let governed_skill_path = plugin_root
            .join("skills")
            .join("skill-elegy_2edemo-plugin")
            .join("SKILL.md");
        let instruction_skill_path = plugin_root
            .join("skills")
            .join("instruction-demo")
            .join("SKILL.md");

        assert!(manifest_path.is_file());
        assert!(governed_skill_path.is_file());
        assert!(instruction_skill_path.is_file());
        assert!(!plugin_root.join(".mcp.json").exists());
        assert_eq!(generated.emitted_components.skills_count, 2);
        assert!(!generated.emitted_components.mcp_servers_emitted);

        let manifest = fs::read_to_string(manifest_path).expect("read generated plugin manifest");
        assert!(manifest.contains("\"name\": \"demo-plugin\""));
        assert!(manifest.contains("\"skills\": \"./skills/\""));

        let governed_skill =
            fs::read_to_string(governed_skill_path).expect("read generated governed skill bridge");
        assert!(governed_skill.contains("name: demo-plugin"));
        assert!(
            governed_skill.contains("Demonstrates portable plugin package capability metadata.")
        );
        assert!(governed_skill.contains("`demo-search`: Search demo package data."));

        let instruction_skill = fs::read_to_string(instruction_skill_path)
            .expect("read generated instruction skill bridge");
        assert!(instruction_skill.contains("name: instruction-demo"));
        assert!(
            instruction_skill.contains("source package does not embed the original markdown body")
        );
    }

    #[test]
    fn codex_plugin_generation_force_clears_stale_outputs() {
        let temp_dir = unique_temp_dir("elegy-tooling-codex-plugin-force");
        let package_path = temp_dir.join("demo-plugin-package.json");
        let output_dir = temp_dir.join("codex-output");

        fs::write(
            &package_path,
            r#"{
  "schemaVersion": "elegy-plugin-package/v1",
  "identity": {
    "packageId": "elegy.demo-plugin",
    "name": "demo-plugin",
    "version": "0.1.0"
  },
  "components": {
    "skillDefinitions": [
      {
        "id": "old-skill",
        "definition": {
          "skillFormat": "elegy-skill-definition",
          "skillVersion": 2,
          "identity": {
            "namespace": "elegy",
            "name": "old-skill",
            "version": "0.1.0"
          },
          "capabilities": [
            {
              "id": "old-cap",
              "name": "Old Cap",
              "description": "Old capability",
              "implementation": {
                "executionType": "subprocess",
                "executableName": "demo",
                "arguments": []
              }
            }
          ],
          "lifecycleState": "active"
        }
      }
    ]
  }
}
"#,
        )
        .expect("write initial package fixture");

        generate_codex_plugin_from_package_file(&package_path, &output_dir, false)
            .expect("initial codex generation should succeed");

        let plugin_root = output_dir.join("demo-plugin");
        let stale_skill_path = plugin_root
            .join("skills")
            .join("skill-elegy_2eold-skill")
            .join("SKILL.md");
        assert!(stale_skill_path.is_file());

        fs::write(
            &package_path,
            r#"{
  "schemaVersion": "elegy-plugin-package/v1",
  "identity": {
    "packageId": "elegy.demo-plugin",
    "name": "demo-plugin",
    "version": "0.2.0"
  },
  "components": {
    "skillDefinitions": [
      {
        "id": "new-skill",
        "definition": {
          "skillFormat": "elegy-skill-definition",
          "skillVersion": 2,
          "identity": {
            "namespace": "elegy",
            "name": "new-skill",
            "version": "0.2.0"
          },
          "capabilities": [
            {
              "id": "new-cap",
              "name": "New Cap",
              "description": "New capability",
              "implementation": {
                "executionType": "subprocess",
                "executableName": "demo",
                "arguments": []
              }
            }
          ],
          "lifecycleState": "active"
        }
      }
    ]
  }
}
"#,
        )
        .expect("write updated package fixture");

        let generated = generate_codex_plugin_from_package_file(&package_path, &output_dir, true)
            .expect("forced codex regeneration should succeed");

        let fresh_skill_path = plugin_root
            .join("skills")
            .join("skill-elegy_2enew-skill")
            .join("SKILL.md");
        assert!(!stale_skill_path.exists());
        assert!(fresh_skill_path.is_file());
        assert_eq!(generated.plugin_version, "0.2.0");
    }

    #[test]
    fn codex_plugin_generation_uses_non_lossy_skill_directory_names() {
        let temp_dir = unique_temp_dir("elegy-tooling-codex-plugin-non-lossy");
        let package_path = temp_dir.join("demo-plugin-package.json");
        let output_dir = temp_dir.join("codex-output");

        fs::create_dir_all(temp_dir.join("skills/app/demo")).expect("create app instruction dir");
        fs::create_dir_all(temp_dir.join("skills/lib/demo")).expect("create lib instruction dir");
        fs::write(
            temp_dir.join("skills/app/demo/SKILL.md"),
            "---\nname: app-demo\ndescription: App demo instructions.\n---\n\nApp demo.\n",
        )
        .expect("write app instruction skill");
        fs::write(
            temp_dir.join("skills/lib/demo/SKILL.md"),
            "---\nname: lib-demo\ndescription: Lib demo instructions.\n---\n\nLib demo.\n",
        )
        .expect("write lib instruction skill");

        fs::write(
            &package_path,
            r#"{
  "schemaVersion": "elegy-plugin-package/v1",
  "identity": {
    "packageId": "elegy.demo-plugin",
    "name": "demo-plugin",
    "version": "0.1.0"
  },
  "components": {
    "skillDefinitions": [
      {
        "id": "acme-search",
        "definition": {
          "skillFormat": "elegy-skill-definition",
          "skillVersion": 2,
          "identity": {
            "namespace": "acme",
            "name": "search",
            "version": "0.1.0"
          },
          "capabilities": [
            {
              "id": "acme-search-cap",
              "name": "Search",
              "description": "Search acme data.",
              "implementation": {
                "executionType": "subprocess",
                "executableName": "demo",
                "arguments": []
              }
            }
          ],
          "lifecycleState": "active"
        }
      },
      {
        "id": "contoso-search",
        "definition": {
          "skillFormat": "elegy-skill-definition",
          "skillVersion": 2,
          "identity": {
            "namespace": "contoso",
            "name": "search",
            "version": "0.1.0"
          },
          "capabilities": [
            {
              "id": "contoso-search-cap",
              "name": "Search",
              "description": "Search contoso data.",
              "implementation": {
                "executionType": "subprocess",
                "executableName": "demo",
                "arguments": []
              }
            }
          ],
          "lifecycleState": "active"
        }
      }
    ],
    "instructionSkills": [
      {
        "id": "app-demo",
        "path": "skills/app/demo/SKILL.md"
      },
      {
        "id": "lib-demo",
        "path": "skills/lib/demo/SKILL.md"
      }
    ]
  }
}
"#,
        )
        .expect("write non-lossy package fixture");

        let generated = generate_codex_plugin_from_package_file(&package_path, &output_dir, false)
            .expect("non-lossy codex generation should succeed");

        let plugin_root = output_dir.join("demo-plugin");
        assert!(plugin_root
            .join("skills")
            .join("skill-acme_2esearch")
            .join("SKILL.md")
            .is_file());
        assert!(plugin_root
            .join("skills")
            .join("skill-contoso_2esearch")
            .join("SKILL.md")
            .is_file());
        assert!(plugin_root
            .join("skills")
            .join("instruction-app_2fdemo")
            .join("SKILL.md")
            .is_file());
        assert!(plugin_root
            .join("skills")
            .join("instruction-lib_2fdemo")
            .join("SKILL.md")
            .is_file());
        assert_eq!(generated.emitted_components.skills_count, 4);
    }

    #[test]
    fn plugin_install_check_receipt_binary_path_succeeds_when_not_on_path() {
        use super::check_plugin_installation;
        use std::process::Command;

        let temp_dir = unique_temp_dir("elegy-tooling-install-check-receipt");

        // Find a real binary path that exists on disk
        let cargo_path = if cfg!(windows) {
            let output = Command::new("where")
                .arg("cargo")
                .output()
                .expect("run where cargo");
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.lines().next().unwrap_or("cargo").trim().to_string()
        } else {
            let output = Command::new("which")
                .arg("cargo")
                .output()
                .expect("run which cargo");
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        };

        let package_path = temp_dir.join("plugin.json");
        fs::write(
            &package_path,
            r#"{
  "schemaVersion": "elegy-plugin-package/v1",
  "identity": {
    "packageId": "elegy.test-plugin",
    "name": "test-plugin",
    "version": "0.1.0"
  },
  "components": {
    "toolRequirements": [
      {
        "toolName": "nonexistent-tool",
        "cliBinary": "nonexistent-binary-xyz-never-on-path",
        "probeCommand": "--version"
      }
    ]
  }
}
"#,
        )
        .expect("write package fixture");

        let receipt_path = temp_dir.join("install-receipt.json");
        fs::write(
            &receipt_path,
            format!(
                r#"{{
  "packageId": "elegy.test-plugin",
  "installPath": "/tmp/test",
  "installedBinaries": [
    {{
      "toolName": "nonexistent-tool",
      "binaryPath": "{}"
    }}
  ]
}}
"#,
                cargo_path.replace('\\', "\\\\")
            ),
        )
        .expect("write receipt fixture");

        let result = check_plugin_installation(&package_path, &receipt_path, None, false, None)
            .expect("install check should succeed");

        assert_eq!(result.readiness, "ready");
        assert_eq!(result.tool_statuses.len(), 1);
        assert_eq!(result.tool_statuses[0].status, "present");
        assert!(result.findings.is_empty());
    }

    #[test]
    fn plugin_install_check_bin_dir_binary_succeeds_when_no_receipt_binary() {
        use super::check_plugin_installation;

        let temp_dir = unique_temp_dir("elegy-tooling-install-check-bindir");
        let bin_dir = temp_dir.join("bin");
        fs::create_dir_all(&bin_dir).expect("create bin dir");

        // Create a stub script/batch that responds to --version
        if cfg!(windows) {
            let binary_path = bin_dir.join("stub-tool.cmd");
            fs::write(&binary_path, "@echo off\necho stub 1.0\n").expect("write stub cmd");
        } else {
            let binary_path = bin_dir.join("stub-tool");
            fs::write(&binary_path, "#!/bin/sh\necho stub 1.0\n").expect("write stub script");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&binary_path, fs::Permissions::from_mode(0o755))
                    .expect("set executable");
            }
        }

        let cli_binary_name = "stub-tool";

        let package_path = temp_dir.join("plugin.json");
        fs::write(
            &package_path,
            format!(
                r#"{{
  "schemaVersion": "elegy-plugin-package/v1",
  "identity": {{
    "packageId": "elegy.test-plugin",
    "name": "test-plugin",
    "version": "0.1.0"
  }},
  "components": {{
    "toolRequirements": [
      {{
        "toolName": "stub-tool",
        "cliBinary": "{}",
        "probeCommand": "--version"
      }}
    ]
  }}
}}
"#,
                cli_binary_name
            ),
        )
        .expect("write package fixture");

        let receipt_path = temp_dir.join("install-receipt.json");
        fs::write(
            &receipt_path,
            r#"{
  "packageId": "elegy.test-plugin",
  "installPath": "/tmp/test",
  "installedBinaries": []
}
"#,
        )
        .expect("write receipt fixture");

        let result =
            check_plugin_installation(&package_path, &receipt_path, Some(&bin_dir), false, None)
                .expect("install check should succeed");

        assert_eq!(result.readiness, "ready", "should find binary via bin_dir");
        assert_eq!(result.tool_statuses.len(), 1);
        assert_eq!(result.tool_statuses[0].status, "present");
    }

    #[test]
    fn plugin_install_check_missing_binary_remains_blocked() {
        use super::check_plugin_installation;

        let temp_dir = unique_temp_dir("elegy-tooling-install-check-missing");

        let package_path = temp_dir.join("plugin.json");
        fs::write(
            &package_path,
            r#"{
  "schemaVersion": "elegy-plugin-package/v1",
  "identity": {
    "packageId": "elegy.test-plugin",
    "name": "test-plugin",
    "version": "0.1.0"
  },
  "components": {
    "toolRequirements": [
      {
        "toolName": "ghost-tool",
        "cliBinary": "ghost-binary-xyz-never-exists"
      }
    ]
  }
}
"#,
        )
        .expect("write package fixture");

        let receipt_path = temp_dir.join("install-receipt.json");
        fs::write(
            &receipt_path,
            r#"{
  "packageId": "elegy.test-plugin",
  "installPath": "/tmp/test",
  "installedBinaries": []
}
"#,
        )
        .expect("write receipt fixture");

        let result = check_plugin_installation(&package_path, &receipt_path, None, false, None)
            .expect("install check should succeed even when binary is missing");

        assert_eq!(result.readiness, "blocked");
        assert_eq!(result.tool_statuses.len(), 1);
        assert_eq!(result.tool_statuses[0].status, "missing");
        assert!(result.findings.iter().any(|f| f.code == "BIN-MISSING"));
    }

    #[test]
    fn plugin_install_check_skip_probe_yields_partial() {
        use super::check_plugin_installation;

        let temp_dir = unique_temp_dir("elegy-tooling-install-check-skip-probe");

        // Find a real binary
        let cargo_path = if cfg!(windows) {
            let output = std::process::Command::new("where")
                .arg("cargo")
                .output()
                .expect("run where cargo");
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.lines().next().unwrap_or("cargo").trim().to_string()
        } else {
            let output = std::process::Command::new("which")
                .arg("cargo")
                .output()
                .expect("run which cargo");
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        };

        let package_path = temp_dir.join("plugin.json");
        fs::write(
            &package_path,
            r#"{
  "schemaVersion": "elegy-plugin-package/v1",
  "identity": {
    "packageId": "elegy.test-plugin",
    "name": "test-plugin",
    "version": "0.1.0"
  },
  "components": {
    "toolRequirements": [
      {
        "toolName": "cargo",
        "cliBinary": "cargo"
      }
    ]
  }
}
"#,
        )
        .expect("write package fixture");

        let receipt_path = temp_dir.join("install-receipt.json");
        fs::write(
            &receipt_path,
            format!(
                r#"{{
  "packageId": "elegy.test-plugin",
  "installPath": "/tmp/test",
  "installedBinaries": [
    {{
      "toolName": "cargo",
      "binaryPath": "{}"
    }}
  ]
}}
"#,
                cargo_path.replace('\\', "\\\\")
            ),
        )
        .expect("write receipt fixture");

        let result = check_plugin_installation(&package_path, &receipt_path, None, true, None)
            .expect("install check should succeed");

        assert_eq!(result.readiness, "partial");
        assert_eq!(result.tool_statuses.len(), 1);
        assert_eq!(result.tool_statuses[0].status, "unprobed");
        assert!(result
            .findings
            .iter()
            .any(|f| f.code == "READINESS-PROBE-SKIPPED"));
    }

    #[test]
    fn readiness_json_side_effect_summary_uses_snake_case_keys() {
        use elegy_contracts::ElegyPluginReadinessSideEffectSummary;

        let summary = ElegyPluginReadinessSideEffectSummary {
            none: 1,
            read_only: 2,
            disk_read: 3,
            disk_write: 4,
            network_outbound: 5,
            process_spawn: 6,
            desktop_ui: 7,
        };

        let json = serde_json::to_string(&summary).expect("serialize summary");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse summary JSON");

        assert_eq!(parsed["none"], 1);
        assert_eq!(parsed["read_only"], 2);
        assert_eq!(parsed["disk_read"], 3);
        assert_eq!(parsed["disk_write"], 4);
        assert_eq!(parsed["network_outbound"], 5);
        assert_eq!(parsed["process_spawn"], 6);
        assert_eq!(parsed["desktop_ui"], 7);

        // Verify camelCase variants are NOT present
        assert!(parsed.get("readOnly").is_none());
        assert!(parsed.get("diskRead").is_none());
        assert!(parsed.get("diskWrite").is_none());
        assert!(parsed.get("networkOutbound").is_none());
        assert!(parsed.get("processSpawn").is_none());
        assert!(parsed.get("desktopUi").is_none());
    }

    #[test]
    fn codex_plugin_generation_rejects_traversal_plugin_output_name_before_writing() {
        let temp_dir = unique_temp_dir("elegy-tooling-codex-plugin-invalid-name");
        let package_path = temp_dir.join("demo-plugin-package.json");
        let output_dir = temp_dir.join("codex-output");
        let escaped_dir = temp_dir.join("target");

        fs::create_dir_all(&escaped_dir).expect("create escaped target dir");
        fs::write(escaped_dir.join("sentinel.txt"), "keep").expect("write sentinel file");
        fs::write(
            &package_path,
            r#"{
  "schemaVersion": "elegy-plugin-package/v1",
  "identity": {
    "packageId": "elegy.demo-plugin",
    "name": "../target",
    "version": "0.1.0"
  },
  "components": {
    "skillDefinitions": [
      {
        "id": "demo-skill",
        "definition": {
          "skillFormat": "elegy-skill-definition",
          "skillVersion": 2,
          "identity": {
            "namespace": "elegy",
            "name": "demo-plugin",
            "version": "0.1.0"
          },
          "capabilities": [
            {
              "id": "demo-cap",
              "name": "Demo Cap",
              "description": "Demo capability",
              "implementation": {
                "executionType": "subprocess",
                "executableName": "demo",
                "arguments": []
              }
            }
          ],
          "lifecycleState": "active"
        }
      }
    ]
  }
}
"#,
        )
        .expect("write invalid package fixture");

        let error = generate_codex_plugin_from_package_file(&package_path, &output_dir, true)
            .expect_err("invalid plugin output name should be rejected");

        match error {
            ToolingError::InvalidPluginPackage { issues, .. } => {
                assert!(issues
                    .iter()
                    .any(|issue| issue.contains("identity.name must be a Codex plugin slug")));
            }
            other => panic!("unexpected error: {other}"),
        }

        assert!(escaped_dir.join("sentinel.txt").is_file());
        assert!(!output_dir.exists());
    }
}
