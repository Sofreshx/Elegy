pub mod codex;
pub mod opencode;

use crate::ToolingError;
use serde::Serialize;
use std::path::Path;

/// Supported host export targets for a plugin package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostTarget {
    Codex,
    OpenCode,
}

impl std::str::FromStr for HostTarget {
    type Err = ToolingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "codex" => Ok(Self::Codex),
            "opencode" => Ok(Self::OpenCode),
            _ => Err(ToolingError::InvalidPluginPackage {
                path: std::path::PathBuf::from(s),
                issues: vec![format!(
                    "Unknown host target '{}'. Valid options: codex, opencode",
                    s
                )],
            }),
        }
    }
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

/// Export a plugin package for a specific host, emitting host-specific files.
pub fn project_plugin_for_host(
    package_path: &Path,
    host: HostTarget,
    output_dir: &Path,
    overwrite: bool,
    package_root: Option<&Path>,
) -> Result<GeneratedHostExport, ToolingError> {
    match host {
        HostTarget::Codex => {
            codex::generate_codex_plugin_from_package_file(package_path, output_dir, overwrite)
        }
        HostTarget::OpenCode => opencode::generate_opencode_plugin_from_package_file(
            package_path,
            output_dir,
            overwrite,
            package_root,
        ),
    }
}
