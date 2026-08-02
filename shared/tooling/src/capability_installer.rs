use elegy_plugin_sdk::capability_package::{
    canonical_json_sha256, validate_elegy_lock_v1, validate_elegy_package_v1,
    ElegyCapabilityReferenceV1, ElegyLockV1, ElegyPackageV1,
};
use elegy_plugin_sdk::{
    validate_elegy_capability_catalog_v2, ElegyCapabilityCatalogV2, ElegyCapabilityV2,
    ELEGY_CAPABILITY_CATALOG_V2_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use zip::ZipArchive;

const RECEIPT_SCHEMA_VERSION: &str = "elegy-capability-installer/v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityInstallReceipt {
    pub schema_version: String,
    pub name: String,
    pub version: String,
    pub target: String,
    pub publisher: String,
    pub archive_sha256: String,
    pub manifest_sha256: String,
    pub capability_catalog_sha256: String,
    pub install_dir: String,
    pub files: BTreeMap<String, String>,
}

#[derive(Debug, Error)]
pub enum CapabilityInstallError {
    #[error("I/O error while {operation} {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("archive error: {0}")]
    Archive(#[from] zip::result::ZipError),
    #[error("invalid capability package: {0}")]
    Invalid(String),
    #[error("locked package identity mismatch: {0}")]
    IdentityMismatch(String),
    #[error("archive digest mismatch: expected {expected}, found {actual}")]
    ArchiveDigestMismatch { expected: String, actual: String },
    #[error("installed file digest mismatch for '{path}': expected {expected}, found {actual}")]
    FileDigestMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("package '{name}' is already installed at {path}")]
    AlreadyInstalled { name: String, path: PathBuf },
}

pub fn install_elegy_package_from_archive(
    archive_path: &Path,
    install_root: &Path,
    lock: &ElegyLockV1,
    target: &str,
) -> Result<CapabilityInstallReceipt, CapabilityInstallError> {
    install_elegy_package_from_archive_inner(archive_path, install_root, lock, target, false)
}

/// Replace an existing exact installation after verifying the new archive.
///
/// The previous installation is moved into the staging directory before the
/// new one is published. If publication fails, the previous directory is
/// moved back into place before the error is returned.
pub fn update_elegy_package_from_archive(
    archive_path: &Path,
    install_root: &Path,
    lock: &ElegyLockV1,
    target: &str,
) -> Result<CapabilityInstallReceipt, CapabilityInstallError> {
    install_elegy_package_from_archive_inner(archive_path, install_root, lock, target, true)
}

fn install_elegy_package_from_archive_inner(
    archive_path: &Path,
    install_root: &Path,
    lock: &ElegyLockV1,
    target: &str,
    replace_existing: bool,
) -> Result<CapabilityInstallReceipt, CapabilityInstallError> {
    validate_lock(lock)?;
    let archive_bytes = read_file(archive_path, "read archive")?;
    let archive_sha256 = digest_bytes(&archive_bytes);
    let (package, files) = read_archive(&archive_bytes)?;
    let reference = lock_reference(lock, &package.name)?;
    validate_locked_identity(&package, &files, reference, target, &archive_sha256)?;
    let (catalog, catalog_value) = read_and_validate_catalog(&package, &files)?;
    let manifest_sha256 = canonical_json_sha256(&package)
        .map_err(|error| CapabilityInstallError::Invalid(format!("manifest digest: {error}")))?;
    let capability_catalog_sha256 = canonical_json_sha256(&catalog_value)
        .map_err(|error| CapabilityInstallError::Invalid(format!("catalog digest: {error}")))?;
    if manifest_sha256 != reference.manifest_sha256.to_ascii_lowercase() {
        return Err(CapabilityInstallError::IdentityMismatch(
            "manifest digest does not match the exact lock".to_string(),
        ));
    }
    if capability_catalog_sha256 != reference.capability_catalog_sha256.to_ascii_lowercase() {
        return Err(CapabilityInstallError::IdentityMismatch(
            "capability catalog digest does not match the exact lock".to_string(),
        ));
    }
    validate_allowed_capabilities(reference, &catalog)?;

    fs::create_dir_all(install_root).map_err(|source| CapabilityInstallError::Io {
        operation: "create install root",
        path: install_root.to_path_buf(),
        source,
    })?;
    let install_dir = install_root.join(&package.name);
    if install_dir.exists() && !replace_existing {
        return Err(CapabilityInstallError::AlreadyInstalled {
            name: package.name,
            path: install_dir,
        });
    }
    if install_dir.exists() && replace_existing {
        verify_installation_file_integrity(&install_dir)?;
    }
    let staging = tempfile::Builder::new()
        .prefix(".elegy-capability-install-")
        .tempdir_in(install_root)
        .map_err(|source| CapabilityInstallError::Io {
            operation: "create staging directory",
            path: install_root.to_path_buf(),
            source,
        })?;
    let staged_dir = staging.path().join(&package.name);
    fs::create_dir_all(&staged_dir).map_err(|source| CapabilityInstallError::Io {
        operation: "create staged package directory",
        path: staged_dir.clone(),
        source,
    })?;

    let manifest_bytes = serde_json::to_vec_pretty(&package)
        .map_err(|error| CapabilityInstallError::Invalid(format!("manifest serialize: {error}")))?;
    write_staged_file(&staged_dir, "elegy-package.json", &manifest_bytes, false)?;
    let mut hashes = BTreeMap::new();
    hashes.insert(
        "elegy-package.json".to_string(),
        digest_bytes(&manifest_bytes),
    );
    let executable_paths = entrypoint_executable_paths(&package)?;
    for (relative, bytes) in &files {
        write_staged_file(
            &staged_dir,
            relative,
            bytes,
            executable_paths.contains(relative),
        )?;
        hashes.insert(relative.clone(), digest_bytes(bytes));
    }

    let receipt = CapabilityInstallReceipt {
        schema_version: RECEIPT_SCHEMA_VERSION.to_string(),
        name: package.name.clone(),
        version: package.version.clone(),
        target: target.to_string(),
        publisher: package.publisher.repository.clone(),
        archive_sha256,
        manifest_sha256,
        capability_catalog_sha256,
        install_dir: install_dir.display().to_string(),
        files: hashes,
    };
    let receipt_bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| CapabilityInstallError::Invalid(format!("receipt serialize: {error}")))?;
    write_staged_file(
        &staged_dir,
        "capability-install-receipt.json",
        &receipt_bytes,
        false,
    )?;
    if install_dir.exists() {
        let previous_dir = staging.path().join("previous");
        fs::rename(&install_dir, &previous_dir).map_err(|source| CapabilityInstallError::Io {
            operation: "stage previous package for update",
            path: install_dir.clone(),
            source,
        })?;
        if let Err(source) = fs::rename(&staged_dir, &install_dir) {
            let rollback = fs::rename(&previous_dir, &install_dir);
            return match rollback {
                Ok(()) => Err(CapabilityInstallError::Io {
                    operation: "publish staged package",
                    path: install_dir.clone(),
                    source,
                }),
                Err(rollback_source) => Err(CapabilityInstallError::Invalid(format!(
                    "publish staged package failed: {source}; rollback also failed: {rollback_source}"
                ))),
            };
        }
        let _ = fs::remove_dir_all(&previous_dir);
    } else {
        fs::rename(&staged_dir, &install_dir).map_err(|source| CapabilityInstallError::Io {
            operation: "publish staged package",
            path: install_dir.clone(),
            source,
        })?;
    }
    Ok(receipt)
}

/// Verify an exact installation and remove only that package directory.
pub fn uninstall_elegy_package(
    install_dir: &Path,
    lock: &ElegyLockV1,
    target: &str,
) -> Result<CapabilityInstallReceipt, CapabilityInstallError> {
    let receipt = verify_elegy_installation(install_dir, lock, target)?;
    fs::remove_dir_all(install_dir).map_err(|source| CapabilityInstallError::Io {
        operation: "uninstall verified package",
        path: install_dir.to_path_buf(),
        source,
    })?;
    Ok(receipt)
}

pub fn verify_elegy_installation(
    install_dir: &Path,
    lock: &ElegyLockV1,
    target: &str,
) -> Result<CapabilityInstallReceipt, CapabilityInstallError> {
    validate_lock(lock)?;
    let receipt_path = install_dir.join("capability-install-receipt.json");
    let receipt_bytes = read_file(&receipt_path, "read install receipt")?;
    let receipt: CapabilityInstallReceipt = serde_json::from_slice(&receipt_bytes)
        .map_err(|error| CapabilityInstallError::Invalid(format!("receipt JSON: {error}")))?;
    if receipt.schema_version != RECEIPT_SCHEMA_VERSION {
        return Err(CapabilityInstallError::Invalid(format!(
            "receipt schemaVersion must be '{RECEIPT_SCHEMA_VERSION}'"
        )));
    }
    let manifest_path = install_dir.join("elegy-package.json");
    let manifest_bytes = read_file(&manifest_path, "read installed package manifest")?;
    let package: ElegyPackageV1 = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| CapabilityInstallError::Invalid(format!("manifest JSON: {error}")))?;
    let issues = validate_elegy_package_v1(&package);
    if !issues.is_empty() {
        return Err(CapabilityInstallError::Invalid(issues.join("; ")));
    }
    let reference = lock_reference(lock, &package.name)?;
    if receipt.name != package.name
        || receipt.version != package.version
        || receipt.target != target
        || receipt.publisher != package.publisher.repository
    {
        return Err(CapabilityInstallError::IdentityMismatch(
            "install receipt does not match the installed package or requested target".to_string(),
        ));
    }
    let manifest_sha256 = canonical_json_sha256(&package)
        .map_err(|error| CapabilityInstallError::Invalid(format!("manifest digest: {error}")))?;
    if manifest_sha256 != reference.manifest_sha256.to_ascii_lowercase()
        || receipt.manifest_sha256 != manifest_sha256
    {
        return Err(CapabilityInstallError::IdentityMismatch(
            "installed manifest digest does not match the exact lock".to_string(),
        ));
    }
    let catalog_path = install_dir.join(normalize_package_path(&package.capability_catalog)?);
    let catalog_bytes = read_file(&catalog_path, "read installed capability catalog")?;
    let (catalog, catalog_value) = parse_catalog(&catalog_bytes)?;
    if catalog.plugin != package.name || catalog.plugin_version != package.version {
        return Err(CapabilityInstallError::IdentityMismatch(
            "installed capability catalog identity does not match package".to_string(),
        ));
    }
    let capability_catalog_sha256 = canonical_json_sha256(&catalog_value)
        .map_err(|error| CapabilityInstallError::Invalid(format!("catalog digest: {error}")))?;
    if capability_catalog_sha256 != reference.capability_catalog_sha256.to_ascii_lowercase()
        || receipt.capability_catalog_sha256 != capability_catalog_sha256
    {
        return Err(CapabilityInstallError::IdentityMismatch(
            "installed capability catalog digest does not match the exact lock".to_string(),
        ));
    }
    validate_allowed_capabilities(reference, &catalog)?;
    verify_installed_executable_digests(install_dir, &package, reference)?;
    let mut required_files = BTreeSet::from(["elegy-package.json".to_string()]);
    required_files.extend(
        package
            .files
            .iter()
            .map(|file| file.path.trim_start_matches("./").to_string()),
    );
    for required in required_files {
        if !receipt.files.contains_key(&required) {
            return Err(CapabilityInstallError::Invalid(format!(
                "install receipt is missing digest for '{required}'"
            )));
        }
    }
    for (relative, expected) in &receipt.files {
        let normalized = normalize_receipt_path(relative)?;
        if normalized != *relative {
            return Err(CapabilityInstallError::Invalid(format!(
                "install receipt path '{relative}' is not normalized"
            )));
        }
        let path = install_dir.join(&normalized);
        let metadata =
            fs::symlink_metadata(&path).map_err(|source| CapabilityInstallError::Io {
                operation: "inspect installed file",
                path: path.clone(),
                source,
            })?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(CapabilityInstallError::Invalid(format!(
                "installed file '{normalized}' is not a regular file"
            )));
        }
        let bytes = read_file(&path, "read installed file")?;
        let actual = digest_bytes(&bytes);
        if actual != expected.to_ascii_lowercase() {
            return Err(CapabilityInstallError::FileDigestMismatch {
                path: normalized,
                expected: expected.clone(),
                actual,
            });
        }
        if let Some(declared) = package
            .files
            .iter()
            .find(|file| file.path.trim_start_matches("./") == relative)
            .and_then(|file| file.sha256.as_ref())
        {
            if actual != declared.to_ascii_lowercase() {
                return Err(CapabilityInstallError::FileDigestMismatch {
                    path: relative.clone(),
                    expected: declared.clone(),
                    actual,
                });
            }
        }
    }
    Ok(receipt)
}

fn read_archive(
    archive_bytes: &[u8],
) -> Result<(ElegyPackageV1, BTreeMap<String, Vec<u8>>), CapabilityInstallError> {
    let mut archive = ZipArchive::new(Cursor::new(archive_bytes))?;
    let mut manifest_bytes = None;
    let mut entries = BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_string();
        if name.ends_with('/') {
            continue;
        }
        if name == "elegy-package.json" {
            if manifest_bytes.is_some() {
                return Err(CapabilityInstallError::Invalid(
                    "archive contains duplicate elegy-package.json entries".to_string(),
                ));
            }
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .map_err(|source| CapabilityInstallError::Io {
                    operation: "read package manifest from archive",
                    path: PathBuf::from("elegy-package.json"),
                    source,
                })?;
            manifest_bytes = Some(bytes);
            continue;
        }
        let normalized = normalize_archive_entry(&name)?;
        if entries.contains_key(&normalized) {
            return Err(CapabilityInstallError::Invalid(format!(
                "archive contains duplicate entry '{normalized}'"
            )));
        }
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|source| CapabilityInstallError::Io {
                operation: "read package file from archive",
                path: PathBuf::from(&normalized),
                source,
            })?;
        entries.insert(normalized, bytes);
    }
    let manifest_bytes = manifest_bytes.ok_or_else(|| {
        CapabilityInstallError::Invalid("archive is missing elegy-package.json".to_string())
    })?;
    let package: ElegyPackageV1 = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| CapabilityInstallError::Invalid(format!("manifest JSON: {error}")))?;
    let issues = validate_elegy_package_v1(&package);
    if !issues.is_empty() {
        return Err(CapabilityInstallError::Invalid(issues.join("; ")));
    }
    let declared = package
        .files
        .iter()
        .map(|file| normalize_package_path(&file.path))
        .collect::<Result<BTreeSet<_>, _>>()?;
    for path in entries.keys() {
        if !declared.contains(path) {
            return Err(CapabilityInstallError::Invalid(format!(
                "archive entry '{path}' is not declared in package files"
            )));
        }
    }
    for path in &declared {
        if !entries.contains_key(path) {
            return Err(CapabilityInstallError::Invalid(format!(
                "declared package file '{path}' is missing from archive"
            )));
        }
    }
    for file in &package.files {
        if let Some(expected) = &file.sha256 {
            let path = normalize_package_path(&file.path)?;
            let bytes = entries.get(&path).ok_or_else(|| {
                CapabilityInstallError::Invalid(format!(
                    "declared package file '{path}' is missing"
                ))
            })?;
            let actual = digest_bytes(bytes);
            if actual != expected.to_ascii_lowercase() {
                return Err(CapabilityInstallError::IdentityMismatch(format!(
                    "archive file '{path}' does not match its declared sha256"
                )));
            }
        }
    }
    Ok((package, entries))
}

fn read_and_validate_catalog(
    package: &ElegyPackageV1,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<(ElegyCapabilityCatalogV2, serde_json::Value), CapabilityInstallError> {
    let path = normalize_package_path(&package.capability_catalog)?;
    let bytes = files.get(&path).ok_or_else(|| {
        CapabilityInstallError::Invalid(format!("capability catalog '{path}' is missing"))
    })?;
    let (catalog, value) = parse_catalog(bytes)?;
    let declared = package
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<BTreeSet<_>>();
    for capability in &catalog.capabilities {
        if let ElegyCapabilityV2::Cli { common, invocation } = capability {
            if !declared.contains(invocation.executable.as_str()) {
                return Err(CapabilityInstallError::Invalid(format!(
                    "capability '{}' executable '{}' is not declared in package files",
                    common.id, invocation.executable
                )));
            }
            if !invocation
                .input_schema
                .as_ref()
                .is_some_and(serde_json::Value::is_object)
                || !invocation
                    .output_schema
                    .as_ref()
                    .is_some_and(serde_json::Value::is_object)
            {
                return Err(CapabilityInstallError::Invalid(format!(
                    "capability '{}' CLI inputSchema and outputSchema must be JSON objects",
                    common.id
                )));
            }
        }
    }
    Ok((catalog, value))
}

fn parse_catalog(
    bytes: &[u8],
) -> Result<(ElegyCapabilityCatalogV2, serde_json::Value), CapabilityInstallError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| CapabilityInstallError::Invalid(format!("catalog JSON: {error}")))?;
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
        != Some(ELEGY_CAPABILITY_CATALOG_V2_SCHEMA_VERSION)
    {
        return Err(CapabilityInstallError::Invalid(
            "capability packages require capability-catalog/v2".to_string(),
        ));
    }
    let catalog: ElegyCapabilityCatalogV2 = serde_json::from_value(value.clone())
        .map_err(|error| CapabilityInstallError::Invalid(format!("catalog JSON: {error}")))?;
    let validation = validate_elegy_capability_catalog_v2(&catalog);
    if !validation.is_valid() {
        return Err(CapabilityInstallError::Invalid(
            validation.issues.join("; "),
        ));
    }
    Ok((catalog, value))
}

fn validate_locked_identity(
    package: &ElegyPackageV1,
    files: &BTreeMap<String, Vec<u8>>,
    reference: &ElegyCapabilityReferenceV1,
    target: &str,
    archive_sha256: &str,
) -> Result<(), CapabilityInstallError> {
    if reference.version != package.version {
        return Err(CapabilityInstallError::IdentityMismatch(format!(
            "lock pins version {}, archive contains {}",
            reference.version, package.version
        )));
    }
    if reference.target != target {
        return Err(CapabilityInstallError::IdentityMismatch(format!(
            "lock target is {}, requested target is {target}",
            reference.target
        )));
    }
    if !package
        .targets
        .iter()
        .any(|value| value == target || value == "any")
    {
        return Err(CapabilityInstallError::IdentityMismatch(format!(
            "package does not support target {target}"
        )));
    }
    if reference.publisher != package.publisher.repository {
        return Err(CapabilityInstallError::IdentityMismatch(
            "locked publisher does not match package publisher repository".to_string(),
        ));
    }
    let expected = reference.archive_sha256.to_ascii_lowercase();
    if expected != archive_sha256 {
        return Err(CapabilityInstallError::ArchiveDigestMismatch {
            expected,
            actual: archive_sha256.to_string(),
        });
    }
    let actual_executable_digests = package
        .entrypoints
        .iter()
        .map(|entrypoint| {
            let path = normalize_package_path(&entrypoint.executable)?;
            let bytes = files.get(&path).ok_or_else(|| {
                CapabilityInstallError::Invalid(format!(
                    "entrypoint executable '{}' is missing from the archive",
                    entrypoint.executable
                ))
            })?;
            Ok((entrypoint.executable.clone(), digest_bytes(bytes)))
        })
        .collect::<Result<BTreeMap<_, _>, CapabilityInstallError>>()?;
    let expected_executable_digests = reference
        .executable_digests
        .iter()
        .map(|(path, digest)| (path.clone(), digest.to_ascii_lowercase()))
        .collect::<BTreeMap<_, _>>();
    if actual_executable_digests != expected_executable_digests {
        return Err(CapabilityInstallError::IdentityMismatch(
            "executable digests do not match the exact lock".to_string(),
        ));
    }
    Ok(())
}

fn verify_installed_executable_digests(
    install_dir: &Path,
    package: &ElegyPackageV1,
    reference: &ElegyCapabilityReferenceV1,
) -> Result<(), CapabilityInstallError> {
    let actual = package
        .entrypoints
        .iter()
        .map(|entrypoint| {
            let relative = normalize_package_path(&entrypoint.executable)?;
            let path = install_dir.join(&relative);
            let metadata =
                fs::symlink_metadata(&path).map_err(|source| CapabilityInstallError::Io {
                    operation: "inspect installed executable",
                    path: path.clone(),
                    source,
                })?;
            if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
                return Err(CapabilityInstallError::Invalid(format!(
                    "installed executable '{}' is not a regular file",
                    entrypoint.executable
                )));
            }
            let digest = digest_bytes(&read_file(&path, "read installed executable")?);
            Ok((entrypoint.executable.clone(), digest))
        })
        .collect::<Result<BTreeMap<_, _>, CapabilityInstallError>>()?;
    let expected = reference
        .executable_digests
        .iter()
        .map(|(path, digest)| (path.clone(), digest.to_ascii_lowercase()))
        .collect::<BTreeMap<_, _>>();
    if actual != expected {
        return Err(CapabilityInstallError::IdentityMismatch(
            "installed executable digests do not match the exact lock".to_string(),
        ));
    }
    Ok(())
}

fn entrypoint_executable_paths(
    package: &ElegyPackageV1,
) -> Result<BTreeSet<String>, CapabilityInstallError> {
    package
        .entrypoints
        .iter()
        .map(|entrypoint| normalize_package_path(&entrypoint.executable))
        .collect()
}

fn validate_allowed_capabilities(
    reference: &ElegyCapabilityReferenceV1,
    catalog: &ElegyCapabilityCatalogV2,
) -> Result<(), CapabilityInstallError> {
    for requested in &reference.allowed_capabilities {
        let Some(capability) = catalog
            .capabilities
            .iter()
            .find(|capability| capability.common().id == *requested)
        else {
            return Err(CapabilityInstallError::IdentityMismatch(format!(
                "lock enables undeclared capability '{requested}'"
            )));
        };
        if !capability.common().readiness.is_agent_routable() {
            return Err(CapabilityInstallError::IdentityMismatch(format!(
                "lock enables non-routable capability '{requested}'"
            )));
        }
    }
    Ok(())
}

fn verify_installation_file_integrity(install_dir: &Path) -> Result<(), CapabilityInstallError> {
    let receipt_path = install_dir.join("capability-install-receipt.json");
    let receipt_bytes = read_file(&receipt_path, "read existing install receipt")?;
    let receipt: CapabilityInstallReceipt =
        serde_json::from_slice(&receipt_bytes).map_err(|error| {
            CapabilityInstallError::Invalid(format!("existing receipt JSON: {error}"))
        })?;
    if receipt.schema_version != RECEIPT_SCHEMA_VERSION {
        return Err(CapabilityInstallError::Invalid(
            "existing install receipt has an unsupported schema".to_string(),
        ));
    }
    if !receipt.files.contains_key("elegy-package.json") {
        return Err(CapabilityInstallError::Invalid(
            "existing install receipt is missing the package manifest digest".to_string(),
        ));
    }
    for (relative, expected) in &receipt.files {
        let normalized = normalize_receipt_path(relative)?;
        let path = install_dir.join(&normalized);
        let metadata =
            fs::symlink_metadata(&path).map_err(|source| CapabilityInstallError::Io {
                operation: "inspect existing installed file",
                path: path.clone(),
                source,
            })?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(CapabilityInstallError::Invalid(format!(
                "existing installed file '{normalized}' is not a regular file"
            )));
        }
        let actual = digest_bytes(&read_file(&path, "read existing installed file")?);
        if actual != expected.to_ascii_lowercase() {
            return Err(CapabilityInstallError::FileDigestMismatch {
                path: normalized,
                expected: expected.clone(),
                actual,
            });
        }
    }
    Ok(())
}

fn validate_lock(lock: &ElegyLockV1) -> Result<(), CapabilityInstallError> {
    let issues = validate_elegy_lock_v1(lock);
    if issues.is_empty() {
        Ok(())
    } else {
        Err(CapabilityInstallError::Invalid(issues.join("; ")))
    }
}

fn lock_reference<'a>(
    lock: &'a ElegyLockV1,
    name: &str,
) -> Result<&'a ElegyCapabilityReferenceV1, CapabilityInstallError> {
    lock.packages
        .iter()
        .find(|package| package.name == name)
        .ok_or_else(|| {
            CapabilityInstallError::IdentityMismatch(format!(
                "package '{name}' is not present in the exact lock"
            ))
        })
}

fn write_staged_file(
    root: &Path,
    relative: &str,
    bytes: &[u8],
    executable: bool,
) -> Result<(), CapabilityInstallError> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CapabilityInstallError::Io {
            operation: "create staged file directory",
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(&path, bytes).map_err(|source| CapabilityInstallError::Io {
        operation: "write staged file",
        path: path.clone(),
        source,
    })?;
    if executable {
        make_executable(&path)?;
    }
    Ok(())
}

fn make_executable(path: &Path) -> Result<(), CapabilityInstallError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|source| CapabilityInstallError::Io {
                operation: "inspect staged executable",
                path: path.to_path_buf(),
                source,
            })?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(|source| CapabilityInstallError::Io {
            operation: "mark staged executable",
            path: path.to_path_buf(),
            source,
        })?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn read_file(path: &Path, operation: &'static str) -> Result<Vec<u8>, CapabilityInstallError> {
    fs::read(path).map_err(|source| CapabilityInstallError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    })
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn normalize_package_path(relative: &str) -> Result<String, CapabilityInstallError> {
    if !relative.starts_with("./") {
        return Err(CapabilityInstallError::Invalid(format!(
            "package path '{relative}' must start with './'"
        )));
    }
    let normalized = relative.trim_start_matches("./");
    if normalized.is_empty()
        || normalized
            .split('/')
            .any(|part| part.is_empty() || part == "..")
    {
        return Err(CapabilityInstallError::Invalid(format!(
            "package path '{relative}' is unsafe"
        )));
    }
    Ok(normalized.to_string())
}

fn normalize_archive_entry(name: &str) -> Result<String, CapabilityInstallError> {
    let path = Path::new(name);
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(CapabilityInstallError::Invalid(format!(
                    "archive entry '{name}' escapes the install root"
                )));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(CapabilityInstallError::Invalid(format!(
                    "archive entry '{name}' is absolute"
                )));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(CapabilityInstallError::Invalid(format!(
            "archive entry '{name}' is empty"
        )));
    }
    Ok(normalized.to_string_lossy().replace('\\', "/"))
}

fn normalize_receipt_path(relative: &str) -> Result<String, CapabilityInstallError> {
    let normalized = relative.replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.contains(':')
        || normalized
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(CapabilityInstallError::Invalid(format!(
            "install receipt path '{relative}' is unsafe"
        )));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_permissions_follow_entrypoint_paths_not_file_roles() {
        let package: ElegyPackageV1 = serde_json::from_value(serde_json::json!({
            "schemaVersion": "elegy-package/v1",
            "name": "portable-tool",
            "version": "1.0.0",
            "description": "Test package",
            "publisher": {
                "name": "Test Publisher",
                "repository": "https://github.com/example/portable-tool"
            },
            "license": "Apache-2.0",
            "targets": ["any"],
            "capabilityCatalog": "./capability-catalog.json",
            "entrypoints": [{
                "id": "portable-tool-cli",
                "kind": "cli",
                "executable": "./bin/tool",
                "command": []
            }],
            "files": [{
                "path": "./bin/tool",
                "role": "binary"
            }]
        }))
        .expect("parse package fixture");

        let paths = entrypoint_executable_paths(&package).expect("normalize executable paths");

        assert_eq!(paths, BTreeSet::from(["bin/tool".to_string()]));
    }
}
