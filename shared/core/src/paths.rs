// ── Path resolution, contract bundle export, and archive helpers ─────────

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

use super::error::ContractsError;
use super::machine_types::{CompatibilityManifest, ConsumerSupportManifest};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContractsBundleExport {
    pub output_path: PathBuf,
    pub archive_path: Option<PathBuf>,
    pub package_version: String,
    pub schema_version: String,
    pub files: Vec<PathBuf>,
}

// ── Path resolution ──────────────────────────────────────────────────────

pub fn resolve_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Returns the repo root (since the central contracts dir is dissolved).
pub fn resolve_plugin_root() -> PathBuf {
    resolve_repo_root()
}

/// Legacy alias for modules still expecting the old name.
pub fn resolve_contracts_source_dir() -> PathBuf {
    resolve_repo_root()
}

pub fn default_contracts_output_dir() -> PathBuf {
    resolve_repo_root().join("artifacts").join("contracts")
}

pub fn resolve_upstream_contracts_dir() -> PathBuf {
    if let Some(path) = env::var_os("ELEGY_CONTRACTS_DIR") {
        return PathBuf::from(path);
    }

    resolve_repo_root().join("shared").join("core")
}

pub fn default_support_manifest_path() -> PathBuf {
    resolve_repo_root()
        .join("support")
        .join("elegy-rust-support.json")
}

fn resolve_contracts_source_path(root: &Path, relative_path: &Path) -> PathBuf {
    let direct_path = root.join(relative_path);
    if direct_path.is_file() {
        return direct_path;
    }

    let schema_path = root.join("schemas").join(relative_path);
    if schema_path.is_file() {
        return schema_path;
    }

    root.join("manifests").join(relative_path)
}

fn default_contracts_archive_path(repo_root: &Path, bundle_version: &str) -> PathBuf {
    repo_root
        .join("artifacts")
        .join("distribution")
        .join(format!("elegy-contracts-{bundle_version}.zip"))
}

// ── Compatibility manifest loading / validation ──────────────────────────

pub fn load_compatibility_manifest_from_dir(
    dir: &Path,
) -> Result<CompatibilityManifest, ContractsError> {
    let bundled_manifest = dir.join("compatibility-manifest.json");
    if bundled_manifest.is_file() {
        return load_json_file(&bundled_manifest);
    }

    load_json_file(&dir.join("manifests").join("compatibility-manifest.json"))
}

pub fn load_consumer_support_manifest(
    path: &Path,
) -> Result<ConsumerSupportManifest, ContractsError> {
    load_json_file(path)
}

pub fn validate_support_manifest_against_upstream(
    support: &ConsumerSupportManifest,
    upstream: &CompatibilityManifest,
) -> Result<(), ContractsError> {
    if support.upstream_package.name != upstream.package.name {
        return Err(ContractsError::Compatibility(format!(
            "support manifest expects upstream package '{}', but bundle package is '{}'",
            support.upstream_package.name, upstream.package.name
        )));
    }

    if support.upstream_package.version != upstream.package.version {
        return Err(ContractsError::Compatibility(format!(
            "support manifest expects upstream package version '{}', but bundle version is '{}'",
            support.upstream_package.version, upstream.package.version
        )));
    }

    for (schema_name, expected_version) in &support.schemas {
        let entry = upstream
            .schemas
            .iter()
            .find(|candidate| candidate.name == *schema_name)
            .ok_or_else(|| ContractsError::MissingSchema(schema_name.clone()))?;

        if entry.schema_version != *expected_version {
            return Err(ContractsError::Compatibility(format!(
                "support manifest expects schema '{}' at version '{}', but bundle provides '{}'",
                schema_name, expected_version, entry.schema_version
            )));
        }
    }

    Ok(())
}

// ── Contract bundle export ────────────────────────────────────────────────

pub fn export_contract_bundle(
    output_dir: Option<&Path>,
    create_archive: bool,
    archive_output_path: Option<&Path>,
) -> Result<ContractsBundleExport, ContractsError> {
    let repo_root = resolve_repo_root();

    let schema_version = "1.0.0".to_string();
    let package_version = "1.0.0".to_string();

    let mut relative_files = BTreeSet::new();

    // Walk plugins/*/schemas/
    let plugins_dir = repo_root.join("plugins");
    if plugins_dir.is_dir() {
        if let Ok(plugin_entries) = fs::read_dir(&plugins_dir) {
            for plugin_entry in plugin_entries.flatten() {
                let plugin_path = plugin_entry.path();
                if plugin_path.is_dir() {
                    // Collect schemas from each plugin
                    let plugin_schemas = plugin_path.join("schemas");
                    if plugin_schemas.is_dir() {
                        if let Ok(schema_entries) = fs::read_dir(&plugin_schemas) {
                            for entry in schema_entries.flatten() {
                                let path = entry.path();
                                if path.extension().and_then(std::ffi::OsStr::to_str)
                                    == Some("json")
                                {
                                    if let Ok(relative) =
                                        path.strip_prefix(&repo_root)
                                    {
                                        relative_files.insert(relative.to_path_buf());
                                    }
                                }
                            }
                        }
                    }

                    // Collect fixtures from each plugin
                    let plugin_fixtures = plugin_path.join("fixtures");
                    if plugin_fixtures.is_dir() {
                        collect_fixture_files_with_prefix(
                            &plugin_fixtures,
                            &repo_root,
                            &mut relative_files,
                        )?;
                    }

                    // Collect contracts from each plugin
                    let plugin_contracts = plugin_path.join("contracts");
                    if plugin_contracts.is_dir() {
                        collect_fixture_files_with_prefix(
                            &plugin_contracts,
                            &repo_root,
                            &mut relative_files,
                        )?;
                    }
                }
            }
        }
    }

    // Walk shared/core/fixtures/
    let core_fixtures = repo_root.join("shared").join("core").join("fixtures");
    if core_fixtures.is_dir() {
        collect_fixture_files_with_prefix(&core_fixtures, &repo_root, &mut relative_files)?;
    }

    let output_path = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(default_contracts_output_dir);

    if output_path.exists() {
        fs::remove_dir_all(&output_path).map_err(|source| ContractsError::Io {
            path: output_path.clone(),
            source,
        })?;
    }

    fs::create_dir_all(&output_path).map_err(|source| ContractsError::Io {
        path: output_path.clone(),
        source,
    })?;

    let mut exported_files = Vec::new();
    for relative_path in &relative_files {
        let source_path = resolve_contracts_source_path(&repo_root, relative_path);
        let destination_path = output_path.join(relative_path);

        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent).map_err(|source| ContractsError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        fs::copy(&source_path, &destination_path).map_err(|source| ContractsError::Io {
            path: destination_path.clone(),
            source,
        })?;
        exported_files.push(destination_path);
    }
    exported_files.sort();

    let archive_path = if create_archive || archive_output_path.is_some() {
        let resolved_archive_path = archive_output_path
            .map(Path::to_path_buf)
            .unwrap_or_else(|| default_contracts_archive_path(&repo_root, &package_version));
        write_contract_archive(&resolved_archive_path, &output_path, &relative_files)?;
        Some(resolved_archive_path)
    } else {
        None
    };

    Ok(ContractsBundleExport {
        output_path,
        archive_path,
        package_version,
        schema_version,
        files: exported_files,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn collect_fixture_files_with_prefix(
    dir: &Path,
    repo_root: &Path,
    relative_files: &mut BTreeSet<PathBuf>,
) -> Result<(), ContractsError> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_fixture_files_with_prefix(&path, repo_root, relative_files)?;
            } else if path.extension().and_then(std::ffi::OsStr::to_str) == Some("json") {
                if let Ok(relative) = path.strip_prefix(repo_root) {
                    relative_files.insert(relative.to_path_buf());
                }
            }
        }
    }
    Ok(())
}

fn write_contract_archive(
    archive_path: &Path,
    output_path: &Path,
    relative_files: &BTreeSet<PathBuf>,
) -> Result<(), ContractsError> {
    if let Some(parent) = archive_path.parent() {
        fs::create_dir_all(parent).map_err(|source| ContractsError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let archive_file = fs::File::create(archive_path).map_err(|source| ContractsError::Io {
        path: archive_path.to_path_buf(),
        source,
    })?;
    let mut archive = zip::ZipWriter::new(archive_file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    for relative_path in relative_files {
        let bundle_path = output_path.join(relative_path);
        let archive_name = relative_path.to_string_lossy().replace('\\', "/");
        archive
            .start_file(&archive_name, options)
            .map_err(|source| ContractsError::Archive {
                path: archive_path.to_path_buf(),
                source,
            })?;
        let file_bytes = fs::read(&bundle_path).map_err(|source| ContractsError::Io {
            path: bundle_path,
            source,
        })?;
        archive
            .write_all(&file_bytes)
            .map_err(|source| ContractsError::Io {
                path: archive_path.to_path_buf(),
                source,
            })?;
    }

    archive.finish().map_err(|source| ContractsError::Archive {
        path: archive_path.to_path_buf(),
        source,
    })?;

    Ok(())
}

fn load_json_file<T>(path: &Path) -> Result<T, ContractsError>
where
    T: for<'de> Deserialize<'de>,
{
    let content = fs::read_to_string(path).map_err(|source| ContractsError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    serde_json::from_str(&content).map_err(|source| ContractsError::Json {
        path: path.to_path_buf(),
        source,
    })
}
