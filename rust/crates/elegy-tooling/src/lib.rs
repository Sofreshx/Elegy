mod docs;

pub use docs::*;

use elegy_contracts::{
    validate_elegy_plugin_package, validate_mcp_analysis_result, validate_mcp_server_descriptor,
    validate_skill_definition_v2, ElegyPluginPackage,
    ElegyPluginPackageCapabilityProjectionComponent, ElegyPluginPackagePathComponent,
    McpAnalysisResult, McpServerDescriptor, McpToolDefinition, McpTransportKind, SkillDefinitionV2,
};
use elegy_mcp::{generated_skill_id, McpSkillGenerator, McpToolAnalyzer};
use serde::Serialize;
use serde_json::json;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
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

    if let Err(error) = write_json_file(&manifest_path, &manifest, overwrite) {
        return Err(error);
    }
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
