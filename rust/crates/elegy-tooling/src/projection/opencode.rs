use crate::ToolingError;
use elegy_contracts::ElegyPluginPackage;
use std::fs;
use std::path::Path;

use super::codex::{
    load_package_skill_definition, projected_governed_skill_directory_name,
    projected_instruction_skill_directory_name, skill_description, yaml_quoted, yaml_scalar,
};
use super::{GeneratedHostProjection, GeneratedHostProjectionComponents};

struct OpenCodeSkillDocument {
    directory_name: String,
    content: String,
}

pub fn generate_opencode_plugin_from_package_file(
    package_path: &Path,
    output_dir: &Path,
    overwrite: bool,
    package_root: Option<&Path>,
) -> Result<GeneratedHostProjection, ToolingError> {
    let package = crate::load_plugin_package_file(package_path)?;
    let root = package_root.unwrap_or_else(|| package_path.parent().unwrap_or(Path::new(".")));
    let plugin_output_name = package.identity.name.trim();
    let skill_documents = collect_opencode_skill_documents(&package, root)?;

    let plugin_root = output_dir.join(plugin_output_name);
    let skills_root = plugin_root.join(".opencode").join("skills");

    let mut target_paths: Vec<std::path::PathBuf> = skill_documents
        .iter()
        .map(|document| skills_root.join(&document.directory_name).join("SKILL.md"))
        .collect();

    if target_paths.is_empty() {
        target_paths.push(plugin_root.join(".opencode").join("_placeholder"));
    }

    if overwrite {
        if plugin_root.exists() {
            fs::remove_dir_all(&plugin_root).map_err(|source| ToolingError::Io {
                operation: "remove",
                path: plugin_root.clone(),
                source,
            })?;
        }
    } else {
        crate::preflight_output_paths(&target_paths, overwrite)?;
    }

    let mut written_files = Vec::with_capacity(target_paths.len());

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
            plugin_manifest: String::new(),
            skills_dir: crate::display_path(&skills_root),
            skills_count: skill_documents.len(),
            apps_emitted: false,
            mcp_servers_emitted: false,
            hooks_emitted: false,
        },
        written_files,
    })
}

fn collect_opencode_skill_documents(
    package: &ElegyPluginPackage,
    package_root: &Path,
) -> Result<Vec<OpenCodeSkillDocument>, ToolingError> {
    let mut documents = Vec::new();
    let mut seen_names = std::collections::BTreeSet::new();

    for component in &package.components.skill_definitions {
        let Some(definition) = load_package_skill_definition(component, package_root)? else {
            continue;
        };

        let document = render_opencode_skill_document(&definition);
        let normalized = document.directory_name.to_ascii_lowercase();
        if !seen_names.insert(normalized) {
            return Err(ToolingError::DuplicateSkillId {
                skill_id: document.directory_name,
            });
        }
        documents.push(document);
    }

    for instruction in &package.components.instruction_skills {
        let document = render_opencode_instruction_skill_document(instruction, package_root)?;
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

fn render_opencode_skill_document(
    definition: &elegy_contracts::SkillDefinitionV2,
) -> OpenCodeSkillDocument {
    let name = definition.identity.name.trim().to_string();
    let description = skill_description(definition);

    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&format!("name: {}\n", yaml_scalar(&name)));
    content.push_str(&format!("description: {}\n", yaml_quoted(&description)));
    content.push_str("---\n\n");

    let title = definition
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.display_name.as_deref())
        .or(definition.identity.display_name.as_deref())
        .unwrap_or(name.as_str());
    content.push_str(&format!("# {}\n\n", title));

    content.push_str(&format!("{}\n\n", description));

    if let Some(category) = definition
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.category.as_deref())
    {
        content.push_str(&format!("Category: `{category}`\n\n"));
    }

    if !definition.identity.aliases.is_empty() {
        content.push_str(&format!(
            "Aliases: {}\n\n",
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
        content.push_str("## Capabilities\n\n");
        for capability in &definition.capabilities {
            content.push_str(&format!(
                "- `{}`: {}\n",
                capability.id, capability.description
            ));
        }
        content.push('\n');
    }

    if let Some(doc_uri) = definition
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.documentation_uri.as_deref())
    {
        content.push_str(&format!("- Documentation: `{doc_uri}`\n"));
    }

    content.push_str("\n---\n");
    content.push_str("This skill file is derived from governed Elegy package metadata.\n");

    OpenCodeSkillDocument {
        directory_name: projected_governed_skill_directory_name(definition),
        content,
    }
}

fn render_opencode_instruction_skill_document(
    component: &elegy_contracts::ElegyPluginPackagePathComponent,
    package_root: &Path,
) -> Result<OpenCodeSkillDocument, ToolingError> {
    let path = std::path::Path::new(&component.path);
    let directory_name = projected_instruction_skill_directory_name(component);

    let source_path = package_root.join(path);
    if source_path.is_file() {
        let content = fs::read_to_string(&source_path).map_err(|source| ToolingError::Io {
            operation: "read",
            path: source_path,
            source,
        })?;
        return Ok(OpenCodeSkillDocument {
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
    content.push_str("This is a derived instruction-skill placeholder.\n\n");
    content.push_str(&format!(
        "The package declares an instruction skill at `{}`.\n",
        component.path
    ));
    content.push_str("The source package does not embed the original markdown body.\n");

    Ok(OpenCodeSkillDocument {
        directory_name,
        content,
    })
}
