//! Host-neutral capability package and agent lock contracts.
//!
//! These contracts deliberately do not contain Codex, MCP, or Holon-specific
//! fields. Host projections consume this authority and may add only fields
//! native to their target host.

use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;
use url::Url;

pub const ELEGY_PACKAGE_V1_SCHEMA_VERSION: &str = "elegy-package/v1";
pub const ELEGY_LOCK_V1_SCHEMA_VERSION: &str = "elegy-lock/v1";
pub const ELEGY_SBOM_V1_SCHEMA_VERSION: &str = "elegy-sbom/v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPackagePublisherV1 {
    pub name: String,
    pub repository: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_identity: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ElegyPackageEntrypointKind {
    Cli,
    McpStdio,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPackageEntrypointV1 {
    pub id: String,
    pub kind: ElegyPackageEntrypointKind,
    pub executable: String,
    #[serde(default)]
    pub command: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPackageProvenanceV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_workflow: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builder: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPackageFileV1 {
    pub path: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// The canonical package manifest. Host-specific manifests are generated from
/// this document and are not an independent source of truth.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPackageV1 {
    pub schema_version: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub publisher: ElegyPackagePublisherV1,
    pub license: String,
    pub targets: Vec<String>,
    pub capability_catalog: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<String>,
    pub entrypoints: Vec<ElegyPackageEntrypointV1>,
    pub files: Vec<ElegyPackageFileV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<ElegyPackageProvenanceV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegySbomFileV1 {
    pub path: String,
    pub role: String,
    pub sha256: String,
}

/// A deterministic software bill of materials for one packaged artifact.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegySbomV1 {
    pub schema_version: String,
    pub package: String,
    pub version: String,
    pub publisher: String,
    pub archive_sha256: String,
    pub files: Vec<ElegySbomFileV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<ElegyPackageProvenanceV1>,
}

/// One exact package selection in an agent's reviewed tool lock.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyCapabilityReferenceV1 {
    pub name: String,
    pub version: String,
    pub target: String,
    pub source: String,
    pub archive_sha256: String,
    pub manifest_sha256: String,
    pub capability_catalog_sha256: String,
    pub executable_digests: BTreeMap<String, String>,
    pub publisher: String,
    pub allowed_capabilities: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyLockV1 {
    pub schema_version: String,
    pub agent_id: String,
    pub packages: Vec<ElegyCapabilityReferenceV1>,
}

#[derive(Debug, Error)]
pub enum CapabilityPackageError {
    #[error("I/O error while {operation} {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("archive error: {0}")]
    Archive(#[from] zip::result::ZipError),
    #[error("invalid capability package: {0}")]
    Invalid(String),
}

pub fn verify_elegy_package_v1(root: &Path) -> Result<ElegyPackageV1, CapabilityPackageError> {
    let manifest_path = root.join("elegy-package.json");
    let raw = fs::read(&manifest_path).map_err(|source| CapabilityPackageError::Io {
        operation: "read",
        path: manifest_path.clone(),
        source,
    })?;
    let package: ElegyPackageV1 =
        serde_json::from_slice(&raw).map_err(|source| CapabilityPackageError::Json {
            path: manifest_path.clone(),
            source,
        })?;
    let issues = validate_elegy_package_v1(&package);
    if !issues.is_empty() {
        return Err(CapabilityPackageError::Invalid(issues.join("; ")));
    }
    for file in &package.files {
        let path = package_path(root, &file.path)?;
        let metadata =
            fs::symlink_metadata(&path).map_err(|source| CapabilityPackageError::Io {
                operation: "inspect",
                path: path.clone(),
                source,
            })?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(CapabilityPackageError::Invalid(format!(
                "declared file '{}' must be a regular file",
                file.path
            )));
        }
        if let Some(expected) = &file.sha256 {
            let bytes = fs::read(&path).map_err(|source| CapabilityPackageError::Io {
                operation: "read",
                path: path.clone(),
                source,
            })?;
            let actual = format!("{:x}", Sha256::digest(bytes));
            if actual != expected.to_ascii_lowercase() {
                return Err(CapabilityPackageError::Invalid(format!(
                    "declared sha256 for '{}' does not match file contents",
                    file.path
                )));
            }
        }
    }
    Ok(package)
}

pub fn pack_elegy_package_v1(root: &Path, output: &Path) -> Result<String, CapabilityPackageError> {
    let package = verify_elegy_package_v1(root)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|source| CapabilityPackageError::Io {
            operation: "create",
            path: parent.to_path_buf(),
            source,
        })?;
    }
    reject_package_output_collision(root, &package, output)?;

    let file = fs::File::create(output).map_err(|source| CapabilityPackageError::Io {
        operation: "create",
        path: output.to_path_buf(),
        source,
    })?;
    let mut archive = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    let manifest_bytes =
        serde_json::to_vec_pretty(&package).map_err(|source| CapabilityPackageError::Json {
            path: root.join("elegy-package.json"),
            source,
        })?;
    archive.start_file("elegy-package.json", options)?;
    archive
        .write_all(&manifest_bytes)
        .map_err(|source| CapabilityPackageError::Io {
            operation: "write",
            path: output.to_path_buf(),
            source,
        })?;

    let mut paths = package
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    paths.sort();
    for relative in paths {
        let source = package_path(root, &relative)?;
        let archive_path = normalize_package_path(&relative)?;
        let bytes = fs::read(&source).map_err(|error| CapabilityPackageError::Io {
            operation: "read",
            path: source.clone(),
            source: error,
        })?;
        archive.start_file(archive_path, options)?;
        archive
            .write_all(&bytes)
            .map_err(|error| CapabilityPackageError::Io {
                operation: "write",
                path: output.to_path_buf(),
                source: error,
            })?;
    }
    archive
        .finish()?
        .sync_all()
        .map_err(|source| CapabilityPackageError::Io {
            operation: "sync",
            path: output.to_path_buf(),
            source,
        })?;

    let bytes = fs::read(output).map_err(|source| CapabilityPackageError::Io {
        operation: "read",
        path: output.to_path_buf(),
        source,
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn reject_package_output_collision(
    root: &Path,
    package: &ElegyPackageV1,
    output: &Path,
) -> Result<(), CapabilityPackageError> {
    let output_absolute = resolve_path_for_collision(output)?;
    let manifest_path = root.join("elegy-package.json");
    if same_path(
        &output_absolute,
        &resolve_path_for_collision(&manifest_path)?,
    ) {
        return Err(CapabilityPackageError::Invalid(
            "archive output must not overwrite elegy-package.json".to_string(),
        ));
    }
    for file in &package.files {
        let source = package_path(root, &file.path)?;
        if same_path(&output_absolute, &resolve_path_for_collision(&source)?) {
            return Err(CapabilityPackageError::Invalid(format!(
                "archive output must not overwrite declared package file '{}'",
                file.path
            )));
        }
    }
    Ok(())
}

fn resolve_path_for_collision(path: &Path) -> Result<PathBuf, CapabilityPackageError> {
    if path.exists() {
        return fs::canonicalize(path).map_err(|source| CapabilityPackageError::Io {
            operation: "resolve",
            path: path.to_path_buf(),
            source,
        });
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().ok_or_else(|| {
        CapabilityPackageError::Invalid(format!(
            "archive output path '{}' is not a file path",
            path.display()
        ))
    })?;
    let parent = fs::canonicalize(parent).map_err(|source| CapabilityPackageError::Io {
        operation: "resolve",
        path: parent.to_path_buf(),
        source,
    })?;
    Ok(parent.join(file_name))
}

fn same_path(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

pub fn validate_elegy_package_v1(package: &ElegyPackageV1) -> Vec<String> {
    let mut issues = Vec::new();

    if package.schema_version != ELEGY_PACKAGE_V1_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{}'.",
            ELEGY_PACKAGE_V1_SCHEMA_VERSION
        ));
    }
    validate_name(&package.name, "name", &mut issues);
    validate_version(&package.version, "version", &mut issues);
    require_non_empty(&package.description, "description", &mut issues);
    require_non_empty(&package.license, "license", &mut issues);
    require_non_empty(&package.publisher.name, "publisher.name", &mut issues);
    validate_url(
        &package.publisher.repository,
        "publisher.repository",
        &mut issues,
    );
    if package
        .publisher
        .workflow_identity
        .as_deref()
        .is_some_and(str::is_empty)
    {
        issues.push("publisher.workflowIdentity must not be blank when provided.".to_string());
    }

    if package.targets.is_empty() {
        issues.push("targets must contain at least one target.".to_string());
    }
    for (index, target) in package.targets.iter().enumerate() {
        require_non_empty(target, &format!("targets[{index}]"), &mut issues);
    }

    validate_safe_path(
        &package.capability_catalog,
        "capabilityCatalog",
        &mut issues,
    );
    if let Some(readiness) = &package.readiness {
        validate_safe_path(readiness, "readiness", &mut issues);
    }

    if package.entrypoints.is_empty() {
        issues.push("entrypoints must contain at least one entrypoint.".to_string());
    }
    let mut entrypoint_ids = BTreeSet::new();
    for (index, entrypoint) in package.entrypoints.iter().enumerate() {
        validate_name(
            &entrypoint.id,
            &format!("entrypoints[{index}].id"),
            &mut issues,
        );
        if !entrypoint_ids.insert(entrypoint.id.clone()) {
            issues.push(format!("duplicate entrypoint id '{}'.", entrypoint.id));
        }
        validate_safe_path(
            &entrypoint.executable,
            &format!("entrypoints[{index}].executable"),
            &mut issues,
        );
        if entrypoint.command.iter().any(|part| part.trim().is_empty()) {
            issues.push(format!(
                "entrypoints[{index}].command must not contain blank segments."
            ));
        }
    }

    if package.files.is_empty() {
        issues.push("files must contain at least one declared file.".to_string());
    }
    let mut file_paths = BTreeSet::new();
    for (index, file) in package.files.iter().enumerate() {
        validate_safe_path(&file.path, &format!("files[{index}].path"), &mut issues);
        if file.path == "./elegy-package.json" {
            issues.push(
                "elegy-package.json is the manifest and must not be redeclared in files."
                    .to_string(),
            );
        }
        require_non_empty(&file.role, &format!("files[{index}].role"), &mut issues);
        if let Some(digest) = &file.sha256 {
            validate_digest(digest, &format!("files[{index}].sha256"), &mut issues);
        }
        if !file_paths.insert(file.path.clone()) {
            issues.push(format!("duplicate declared file '{}'.", file.path));
        }
    }

    let declared_paths = file_paths;
    require_declared_file(
        &declared_paths,
        &package.capability_catalog,
        "capabilityCatalog",
        &mut issues,
    );
    if let Some(readiness) = &package.readiness {
        require_declared_file(&declared_paths, readiness, "readiness", &mut issues);
    }
    for skill in &package.skills {
        validate_safe_path(skill, "skills", &mut issues);
        require_declared_file(&declared_paths, skill, "skill", &mut issues);
    }
    for entrypoint in &package.entrypoints {
        require_declared_file(
            &declared_paths,
            &entrypoint.executable,
            "entrypoint executable",
            &mut issues,
        );
    }

    if let Some(provenance) = &package.provenance {
        if let Some(commit) = &provenance.source_commit {
            if commit.len() < 7
                || !commit
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
            {
                issues.push(
                    "provenance.sourceCommit must be at least 7 hexadecimal characters."
                        .to_string(),
                );
            }
        }
        if let Some(workflow) = &provenance.build_workflow {
            require_non_empty(workflow, "provenance.buildWorkflow", &mut issues);
        }
        if let Some(builder) = &provenance.builder {
            require_non_empty(builder, "provenance.builder", &mut issues);
        }
    }

    issues
}

pub fn validate_elegy_lock_v1(lock: &ElegyLockV1) -> Vec<String> {
    let mut issues = Vec::new();

    if lock.schema_version != ELEGY_LOCK_V1_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{}'.",
            ELEGY_LOCK_V1_SCHEMA_VERSION
        ));
    }
    validate_name(&lock.agent_id, "agentId", &mut issues);
    if lock.packages.is_empty() {
        issues.push("packages must contain at least one package.".to_string());
    }

    let mut package_names = BTreeSet::new();
    for (index, package) in lock.packages.iter().enumerate() {
        validate_name(
            &package.name,
            &format!("packages[{index}].name"),
            &mut issues,
        );
        validate_version(
            &package.version,
            &format!("packages[{index}].version"),
            &mut issues,
        );
        require_non_empty(
            &package.target,
            &format!("packages[{index}].target"),
            &mut issues,
        );
        validate_url(
            &package.source,
            &format!("packages[{index}].source"),
            &mut issues,
        );
        validate_url(
            &package.publisher,
            &format!("packages[{index}].publisher"),
            &mut issues,
        );
        validate_digest(
            &package.archive_sha256,
            &format!("packages[{index}].archiveSha256"),
            &mut issues,
        );
        validate_digest(
            &package.manifest_sha256,
            &format!("packages[{index}].manifestSha256"),
            &mut issues,
        );
        validate_digest(
            &package.capability_catalog_sha256,
            &format!("packages[{index}].capabilityCatalogSha256"),
            &mut issues,
        );
        if package.executable_digests.is_empty() {
            issues.push(format!(
                "packages[{index}].executableDigests must contain at least one executable."
            ));
        }
        for (path, digest) in &package.executable_digests {
            validate_safe_path(
                path,
                &format!("packages[{index}].executableDigests path"),
                &mut issues,
            );
            validate_digest(
                digest,
                &format!("packages[{index}].executableDigests[{path}]"),
                &mut issues,
            );
        }
        if !package_names.insert(package.name.clone()) {
            issues.push(format!("duplicate locked package '{}'.", package.name));
        }
        if package.allowed_capabilities.is_empty() {
            issues.push(format!(
                "packages[{index}].allowedCapabilities must contain at least one capability."
            ));
        }
        let mut capabilities = BTreeSet::new();
        for (capability_index, capability) in package.allowed_capabilities.iter().enumerate() {
            require_non_empty(
                capability,
                &format!("packages[{index}].allowedCapabilities[{capability_index}]"),
                &mut issues,
            );
            if !capabilities.insert(capability.clone()) {
                issues.push(format!("duplicate allowed capability '{capability}'."));
            }
        }
    }

    issues
}

pub fn validate_elegy_sbom_v1(sbom: &ElegySbomV1) -> Vec<String> {
    let mut issues = Vec::new();
    if sbom.schema_version != ELEGY_SBOM_V1_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{}'.",
            ELEGY_SBOM_V1_SCHEMA_VERSION
        ));
    }
    validate_name(&sbom.package, "package", &mut issues);
    validate_version(&sbom.version, "version", &mut issues);
    validate_url(&sbom.publisher, "publisher", &mut issues);
    validate_digest(&sbom.archive_sha256, "archiveSha256", &mut issues);
    if sbom.files.is_empty() {
        issues.push("files must contain at least one entry.".to_string());
    }
    let mut paths = BTreeSet::new();
    for (index, file) in sbom.files.iter().enumerate() {
        if !is_safe_package_relative_path(&file.path) && file.path != "elegy-package.json" {
            issues.push(format!(
                "files[{index}].path must be a safe package-relative path."
            ));
        }
        require_non_empty(&file.role, &format!("files[{index}].role"), &mut issues);
        validate_digest(&file.sha256, &format!("files[{index}].sha256"), &mut issues);
        if !paths.insert(file.path.clone()) {
            issues.push(format!("duplicate SBOM file '{}'.", file.path));
        }
    }
    issues
}

pub fn canonical_json_sha256<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn validate_name(value: &str, field: &str, issues: &mut Vec<String>) {
    if value.is_empty()
        || value.len() > 96
        || value.starts_with('-')
        || value.ends_with('-')
        || value.chars().any(|character| {
            !(character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-')
        })
    {
        issues.push(format!(
            "{field} must be lowercase kebab-case and at most 96 characters."
        ));
    }
}

fn validate_version(value: &str, field: &str, issues: &mut Vec<String>) {
    if Version::parse(value).is_err() {
        issues.push(format!("{field} must be valid SemVer."));
    }
}

fn validate_url(value: &str, field: &str, issues: &mut Vec<String>) {
    if Url::parse(value).is_err() {
        issues.push(format!("{field} must be an absolute URL."));
    }
}

fn validate_safe_path(value: &str, field: &str, issues: &mut Vec<String>) {
    if !is_safe_package_relative_path(value) {
        issues.push(format!("{field} must be a safe package-relative path."));
    }
}

fn is_safe_package_relative_path(value: &str) -> bool {
    value.starts_with("./")
        && !value.contains('\\')
        && !value.is_empty()
        && !value.starts_with('/')
        && !value.starts_with("../")
        && value != ".."
        && !value.split('/').any(|part| part == ".." || part.is_empty())
}

fn package_path(root: &Path, relative: &str) -> Result<PathBuf, CapabilityPackageError> {
    let normalized = normalize_package_path(relative)?;
    Ok(root.join(normalized))
}

fn normalize_package_path(relative: &str) -> Result<String, CapabilityPackageError> {
    if !is_safe_package_relative_path(relative) {
        return Err(CapabilityPackageError::Invalid(format!(
            "path '{relative}' is not a safe package-relative path"
        )));
    }
    Ok(relative.trim_start_matches("./").to_string())
}

fn require_declared_file(
    declared_paths: &BTreeSet<String>,
    path: &str,
    field: &str,
    issues: &mut Vec<String>,
) {
    if !declared_paths.contains(path) {
        issues.push(format!("{field} '{path}' must be listed in files."));
    }
}

fn require_non_empty(value: &str, field: &str, issues: &mut Vec<String>) {
    if value.trim().is_empty() {
        issues.push(format!("{field} must not be empty."));
    }
}

fn validate_digest(value: &str, field: &str, issues: &mut Vec<String>) {
    if value.len() != 64 || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        issues.push(format!("{field} must be a 64-character SHA-256 digest."));
    }
}
