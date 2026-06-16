use crate::ToolingError;
use elegy_contracts::{
    validate_skill_definition_v2, ElegyPluginPackage,
    ElegyPluginPackageCapabilityProjectionComponent, ElegyPluginPackagePathComponent,
    SkillDefinitionV2,
};
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::{GeneratedHostProjection, GeneratedHostProjectionComponents};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexPluginManifest {
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

#[derive(Clone, Debug)]
pub(crate) struct CodexSkillDocument {
    pub directory_name: String,
    pub content: String,
}

pub fn generate_codex_plugin_from_package_file(
    package_path: &Path,
    output_dir: &Path,
    overwrite: bool,
) -> Result<GeneratedHostProjection, ToolingError> {
    let package = crate::load_plugin_package_file(package_path)?;
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
        crate::clear_plugin_output_root(&plugin_root)?;
    } else {
        crate::preflight_output_paths(&target_paths, overwrite)?;
    }

    let mut written_files = Vec::with_capacity(target_paths.len());

    crate::write_json_file(&manifest_path, &manifest, overwrite)?;
    written_files.push(crate::display_path(&manifest_path));

    for document in &skill_documents {
        let output_path = skills_root.join(&document.directory_name).join("SKILL.md");
        if let Err(error) = crate::write_text_file(&output_path, &document.content, overwrite) {
            crate::cleanup_written_files(
                &written_files
                    .iter()
                    .map(std::path::PathBuf::from)
                    .collect::<Vec<_>>(),
            );
            return Err(error);
        }
        written_files.push(crate::display_path(&output_path));
    }

    Ok(GeneratedHostProjection {
        source_package: crate::display_path(package_path),
        plugin_name: plugin_output_name.to_string(),
        plugin_version: package.identity.version.clone(),
        emitted_components: GeneratedHostProjectionComponents {
            plugin_manifest: crate::display_path(&manifest_path),
            skills_dir: crate::display_path(&skills_root),
            skills_count: skill_documents.len(),
            apps_emitted: false,
            mcp_servers_emitted: false,
            hooks_emitted: false,
        },
        written_files,
    })
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

pub(crate) fn load_package_skill_definition(
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
    let path = std::path::Path::new(&component.path);
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

pub(crate) fn skill_description(definition: &SkillDefinitionV2) -> String {
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

pub(crate) fn projected_governed_skill_directory_name(definition: &SkillDefinitionV2) -> String {
    build_projected_skill_directory_name(
        "skill",
        &format!(
            "{}.{}",
            definition.identity.namespace, definition.identity.name
        ),
    )
}

pub(crate) fn projected_instruction_skill_directory_name(
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

pub(crate) fn yaml_scalar(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        value.to_string()
    } else {
        yaml_quoted(value)
    }
}

pub(crate) fn yaml_quoted(value: &str) -> String {
    serde_json::json!(value).to_string()
}
