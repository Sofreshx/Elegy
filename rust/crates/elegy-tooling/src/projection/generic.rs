use crate::ToolingError;
use std::path::Path;

use super::{GeneratedHostProjection, GeneratedHostProjectionComponents};

pub fn generate_generic_host_projection(
    package_path: &Path,
    output_dir: &Path,
    overwrite: bool,
    _package_root: Option<&Path>,
) -> Result<GeneratedHostProjection, ToolingError> {
    let package = crate::load_plugin_package_file(package_path)?;
    let plugin_output_name = package.identity.name.trim();

    let plugin_root = output_dir.join(plugin_output_name);
    let manifest_dir = plugin_root.join(".elegy-host-generic");
    let manifest_path = manifest_dir.join("plugin.json");

    let target_paths = vec![manifest_path.clone()];

    if overwrite {
        if plugin_root.exists() {
            std::fs::remove_dir_all(&plugin_root).map_err(|source| ToolingError::Io {
                operation: "remove",
                path: plugin_root.clone(),
                source,
            })?;
        }
    } else {
        crate::preflight_output_paths(&target_paths, overwrite)?;
    }

    let host_manifest = serde_json::json!({
        "schemaVersion": "elegy-host-projection/v1",
        "host": "generic",
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

    crate::write_json_file(&manifest_path, &host_manifest, overwrite)?;

    let written_files = vec![crate::display_path(&manifest_path)];

    Ok(GeneratedHostProjection {
        source_package: crate::display_path(package_path),
        plugin_name: plugin_output_name.to_string(),
        plugin_version: package.identity.version.clone(),
        emitted_components: GeneratedHostProjectionComponents {
            plugin_manifest: crate::display_path(&manifest_path),
            skills_dir: String::new(),
            skills_count: 0,
            apps_emitted: false,
            mcp_servers_emitted: false,
            hooks_emitted: false,
        },
        written_files,
    })
}
