use elegy_plugin_sdk::{is_safe_package_relative_path, validate_elegy_plugin_v1, ElegyPluginV1};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

/// Metadata written after successful install.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstallReceipt {
    pub schema_version: String,
    pub name: String,
    pub version: String,
    pub installed_at: String,
    pub source: String,
    pub install_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marketplace_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marketplace_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_digest: Option<String>,
    pub files: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct InstallReceiptMetadata {
    pub target: Option<String>,
    pub marketplace_name: Option<String>,
    pub marketplace_source: Option<String>,
    pub artifact_url: Option<String>,
    pub checksum_url: Option<String>,
    pub artifact_sha256: Option<String>,
    pub manifest_version: Option<String>,
    pub capability_digest: Option<String>,
}

/// Error type for installation failures.
#[derive(Debug)]
pub enum InstallError {
    Io(std::io::Error),
    Zip(zip::result::ZipError),
    InvalidManifest(String),
    MissingManifest,
    AlreadyInstalled { name: String, path: PathBuf },
    DownloadFailed(String),
    ChecksumMismatch { expected: String, actual: String },
    IdentityMismatch(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Zip(e) => write!(f, "Zip error: {e}"),
            Self::InvalidManifest(msg) => write!(f, "Invalid plugin manifest: {msg}"),
            Self::MissingManifest => write!(f, "Plugin archive missing plugin.json"),
            Self::AlreadyInstalled { name, path } => {
                write!(
                    f,
                    "Plugin '{name}' is already installed at {}",
                    path.display()
                )
            }
            Self::DownloadFailed(msg) => write!(f, "Download failed: {msg}"),
            Self::ChecksumMismatch { expected, actual } => {
                write!(f, "Checksum mismatch: expected {expected}, found {actual}")
            }
            Self::IdentityMismatch(message) => write!(f, "Plugin identity mismatch: {message}"),
        }
    }
}

impl std::error::Error for InstallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Zip(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<zip::result::ZipError> for InstallError {
    fn from(e: zip::result::ZipError) -> Self {
        Self::Zip(e)
    }
}

/// Install a plugin from a local archive file.
pub fn install_from_archive(
    archive_path: &Path,
    install_root: &Path,
) -> Result<InstallReceipt, InstallError> {
    install_from_archive_with_identity(archive_path, install_root, None, None)
}

pub fn install_from_archive_with_identity(
    archive_path: &Path,
    install_root: &Path,
    expected_name: Option<&str>,
    expected_version: Option<&str>,
) -> Result<InstallReceipt, InstallError> {
    install_from_archive_with_identity_and_source(
        archive_path,
        install_root,
        expected_name,
        expected_version,
        None,
        InstallReceiptMetadata::default(),
    )
}

fn install_from_archive_with_identity_and_source(
    archive_path: &Path,
    install_root: &Path,
    expected_name: Option<&str>,
    expected_version: Option<&str>,
    source_override: Option<&str>,
    metadata: InstallReceiptMetadata,
) -> Result<InstallReceipt, InstallError> {
    let file = fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // Find and validate plugin.json
    let mut manifest: Option<ElegyPluginV1> = None;
    let mut manifest_index = None;
    for i in 0..archive.len() {
        let name = archive.by_index(i)?.name().to_string();
        if name == "plugin.json" {
            if manifest.is_some() {
                return Err(InstallError::InvalidManifest(
                    "archive contains duplicate root plugin.json entries".to_string(),
                ));
            }
            let mut content = String::new();
            archive.by_index(i)?.read_to_string(&mut content)?;
            let mut plugin: ElegyPluginV1 = serde_json::from_str(&content)
                .map_err(|e| InstallError::InvalidManifest(format!("JSON parse: {e}")))?;
            normalize_legacy_component_path(&mut plugin.skills);
            normalize_legacy_component_path(&mut plugin.mcp_servers);
            let validation = validate_elegy_plugin_v1(&plugin);
            if !validation.is_valid() {
                return Err(InstallError::InvalidManifest(validation.issues.join("; ")));
            }
            manifest = Some(plugin);
            manifest_index = Some(i);
        }
    }

    let manifest = manifest.ok_or(InstallError::MissingManifest)?;
    let manifest_index = manifest_index.ok_or(InstallError::MissingManifest)?;
    if let Some(expected_name) = expected_name {
        if manifest.name != expected_name {
            return Err(InstallError::IdentityMismatch(format!(
                "expected name '{expected_name}', found '{}'",
                manifest.name
            )));
        }
    }
    if let Some(expected_version) = expected_version {
        if manifest.version != expected_version {
            return Err(InstallError::IdentityMismatch(format!(
                "expected version '{expected_version}', found '{}'",
                manifest.version
            )));
        }
    }

    let mut archive_entries = Vec::new();
    let mut seen_paths = BTreeSet::new();
    for i in 0..archive.len() {
        if i == manifest_index {
            continue;
        }
        let entry = archive.by_index(i)?;
        let entry_name = entry.name().to_string();
        if entry_name.ends_with('/') {
            continue;
        }
        let relative_path =
            validate_archive_entry_path(&entry_name).map_err(InstallError::InvalidManifest)?;
        let normalized = relative_path.to_string_lossy().replace('\\', "/");
        if !seen_paths.insert(normalized.clone()) {
            return Err(InstallError::InvalidManifest(format!(
                "archive contains duplicate entry '{normalized}'"
            )));
        }
        archive_entries.push((i, relative_path));
    }

    // Determine install directory
    let install_dir = install_root.join(&manifest.name);
    if install_dir.exists() {
        return Err(InstallError::AlreadyInstalled {
            name: manifest.name.clone(),
            path: install_dir,
        });
    }

    // Extract into a same-volume staging directory, then publish atomically.
    fs::create_dir_all(install_root)?;
    let staging = tempfile::Builder::new()
        .prefix(".elegy-install-")
        .tempdir_in(install_root)?;
    let staged_install_dir = staging.path().join(&manifest.name);
    fs::create_dir_all(&staged_install_dir)?;
    let mut installed_files = Vec::new();

    for (i, relative_path) in archive_entries {
        let mut entry = archive.by_index(i)?;
        let dest_path = staged_install_dir.join(&relative_path);
        let normalized = relative_path.to_string_lossy().replace('\\', "/");
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut dest_file = fs::File::create(&dest_path)?;
        io::copy(&mut entry, &mut dest_file)?;
        drop(dest_file);
        make_binary_executable(&dest_path, &normalized)?;
        installed_files.push(normalized);
    }

    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| InstallError::InvalidManifest(format!("manifest serialize: {e}")))?;
    let installed_manifest_dir = staged_install_dir.join(".elegy-plugin");
    fs::create_dir_all(&installed_manifest_dir)?;
    fs::write(installed_manifest_dir.join("plugin.json"), manifest_json)?;
    installed_files.retain(|path| path != ".elegy-plugin/plugin.json");
    installed_files.push(".elegy-plugin/plugin.json".to_string());

    // Write install-receipt.json
    let receipt = InstallReceipt {
        schema_version: "elegy-installer/v1".to_string(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        installed_at: manual_iso8601_timestamp(),
        source: source_override
            .map(str::to_string)
            .unwrap_or_else(|| archive_path.display().to_string()),
        install_dir: install_dir.display().to_string(),
        target: metadata.target,
        marketplace_name: metadata.marketplace_name,
        marketplace_source: metadata.marketplace_source,
        artifact_url: metadata.artifact_url,
        checksum_url: metadata.checksum_url,
        artifact_sha256: metadata.artifact_sha256,
        manifest_version: metadata
            .manifest_version
            .or_else(|| Some(manifest.version.clone())),
        capability_digest: metadata.capability_digest,
        files: installed_files,
    };

    let receipt_path = staged_install_dir.join("install-receipt.json");
    let receipt_json = serde_json::to_string_pretty(&receipt)
        .map_err(|e| InstallError::InvalidManifest(format!("receipt serialize: {e}")))?;
    fs::write(&receipt_path, receipt_json)?;
    fs::rename(&staged_install_dir, &install_dir)?;

    Ok(receipt)
}

/// Install a plugin from a URL after verifying its SHA-256 sidecar.
pub fn install_from_url(
    url: &str,
    checksum_url: &str,
    install_root: &Path,
    expected_name: Option<&str>,
    expected_version: Option<&str>,
) -> Result<InstallReceipt, InstallError> {
    install_from_url_with_metadata(
        url,
        checksum_url,
        install_root,
        expected_name,
        expected_version,
        InstallReceiptMetadata::default(),
    )
}

pub fn install_from_url_with_metadata(
    url: &str,
    checksum_url: &str,
    install_root: &Path,
    expected_name: Option<&str>,
    expected_version: Option<&str>,
    mut metadata: InstallReceiptMetadata,
) -> Result<InstallReceipt, InstallError> {
    let checksum_response = reqwest::blocking::get(checksum_url)
        .map_err(|e| InstallError::DownloadFailed(e.to_string()))?;
    if !checksum_response.status().is_success() {
        return Err(InstallError::DownloadFailed(format!(
            "checksum HTTP {}",
            checksum_response.status()
        )));
    }
    let checksum_text = checksum_response
        .text()
        .map_err(|e| InstallError::DownloadFailed(e.to_string()))?;
    let expected_checksum = checksum_text
        .split_whitespace()
        .next()
        .filter(|value| value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(|| {
            InstallError::DownloadFailed("checksum response is not a SHA-256 digest".to_string())
        })?
        .to_ascii_lowercase();
    if metadata.artifact_sha256.is_none() {
        metadata.artifact_sha256 = Some(expected_checksum.clone());
    }
    if metadata.artifact_url.is_none() {
        metadata.artifact_url = Some(url.to_string());
    }
    if metadata.checksum_url.is_none() {
        metadata.checksum_url = Some(checksum_url.to_string());
    }

    let response =
        reqwest::blocking::get(url).map_err(|e| InstallError::DownloadFailed(e.to_string()))?;
    if !response.status().is_success() {
        return Err(InstallError::DownloadFailed(format!(
            "HTTP {}",
            response.status()
        )));
    }
    let bytes = response
        .bytes()
        .map_err(|e| InstallError::DownloadFailed(e.to_string()))?;
    verify_sha256(&bytes, &expected_checksum)?;

    let tmp = tempfile::NamedTempFile::new()?;
    fs::write(tmp.path(), &bytes)?;

    install_from_archive_with_identity_and_source(
        tmp.path(),
        install_root,
        expected_name,
        expected_version,
        Some(url),
        metadata,
    )
}

fn verify_sha256(bytes: &[u8], expected_checksum: &str) -> Result<(), InstallError> {
    let actual_checksum = format!("{:x}", Sha256::digest(bytes));
    if actual_checksum != expected_checksum {
        return Err(InstallError::ChecksumMismatch {
            expected: expected_checksum.to_string(),
            actual: actual_checksum,
        });
    }
    Ok(())
}

#[cfg(unix)]
fn make_binary_executable(path: &Path, archive_path: &str) -> Result<(), InstallError> {
    use std::os::unix::fs::PermissionsExt;

    if archive_path.starts_with("bin/") {
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn make_binary_executable(_path: &Path, _archive_path: &str) -> Result<(), InstallError> {
    Ok(())
}

/// Generate a manual ISO 8601 timestamp without pulling in chrono.
fn manual_iso8601_timestamp() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Calculate year/month/day from days since epoch
    let mut year = 1970i64;
    let mut remaining_days = days_since_epoch as i64;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }
    let days_in_months = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1;
    for &dim in &days_in_months {
        if remaining_days < dim {
            break;
        }
        remaining_days -= dim;
        month += 1;
    }
    let day = remaining_days + 1;
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

fn normalize_legacy_component_path(path: &mut Option<String>) {
    let Some(value) = path.as_ref() else {
        return;
    };
    if value.starts_with("./") {
        return;
    }
    let candidate = format!("./{value}");
    if is_safe_package_relative_path(&candidate) {
        *path = Some(candidate);
    }
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn validate_archive_entry_path(entry_name: &str) -> Result<PathBuf, String> {
    let path = Path::new(entry_name);
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(format!(
                    "archive entry '{entry_name}' escapes the install root"
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("archive entry '{entry_name}' is absolute"));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(format!("archive entry '{entry_name}' is empty"));
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::{
        install_from_archive, install_from_archive_with_identity, verify_sha256, InstallError,
    };
    use elegy_plugin_sdk::{pack_plugin_v1, verify_plugin_v1};
    use std::fs;
    use std::io::Write;

    fn write_zip(path: &std::path::Path, entries: &[(&str, &str)]) {
        let file = fs::File::create(path).expect("create zip");
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for (name, contents) in entries {
            zip.start_file(name, options).expect("start entry");
            zip.write_all(contents.as_bytes()).expect("write entry");
        }

        zip.finish().expect("finish zip");
    }

    fn write_plugin_fixture(root: &std::path::Path, name: &str, description: &str) {
        fs::create_dir_all(root.join(".elegy-plugin")).expect("create manifest dir");
        fs::create_dir_all(root.join("skills").join(name)).expect("create skill dir");
        fs::write(
            root.join(".elegy-plugin").join("plugin.json"),
            format!(
                r#"{{
  "schemaVersion": "elegy-plugin/v1",
  "name": "{name}",
  "version": "0.1.0",
  "description": "{description}",
  "author": {{"name": "Elegy"}},
  "license": "Apache-2.0",
  "skills": "./skills/"
}}"#
            ),
        )
        .expect("write manifest");
        fs::write(
            root.join("skills").join(name).join("SKILL.md"),
            format!(
                "---\nname: {name}\ndescription: {description}\n---\n\n# {name}\n\nUse this test fixture skill.\n"
            ),
        )
        .expect("write skill");
    }

    #[test]
    fn install_rejects_path_traversal_entries() {
        let temp = tempfile::tempdir().expect("tempdir");
        let archive_path = temp.path().join("bad.plugin.zip");
        write_zip(
            &archive_path,
            &[
                (
                    "plugin.json",
                    r#"{"schemaVersion":"elegy-plugin/v1","name":"safe-plugin","version":"0.1.0","description":"desc","skills":"./skills/"}"#,
                ),
                ("../escape.txt", "nope"),
            ],
        );

        let err = install_from_archive(&archive_path, temp.path()).expect_err("must fail");
        assert!(
            matches!(err, InstallError::InvalidManifest(ref message) if message.contains("escapes the install root")),
            "unexpected error: {err}"
        );
        assert!(!temp.path().join("safe-plugin").exists());
    }

    #[test]
    fn install_rejects_duplicate_entries_before_writing() {
        let temp = tempfile::tempdir().expect("tempdir");
        let archive_path = temp.path().join("duplicate.plugin.zip");
        write_zip(
            &archive_path,
            &[
                (
                    "plugin.json",
                    r#"{"schemaVersion":"elegy-plugin/v1","name":"safe-plugin","version":"0.1.0","description":"desc","skills":"./skills/"}"#,
                ),
                ("skills/example/SKILL.md", "one"),
                ("skills/example/./SKILL.md", "two"),
            ],
        );

        let err = install_from_archive(&archive_path, temp.path()).expect_err("must fail");
        assert!(
            matches!(err, InstallError::InvalidManifest(ref message) if message.contains("duplicate entry")),
            "unexpected error: {err}"
        );
        assert!(!temp.path().join("safe-plugin").exists());
    }

    #[test]
    fn packed_plugin_installs_with_receipt() {
        let temp = tempfile::tempdir().expect("tempdir");
        let plugin_root = temp.path().join("source");
        write_plugin_fixture(&plugin_root, "roundtrip-plugin", "Round-trip fixture");
        let archive_path = temp.path().join("roundtrip.plugin.zip");
        pack_plugin_v1(&plugin_root, &archive_path).expect("pack");
        let install_root = temp.path().join("installed");

        let receipt = install_from_archive(&archive_path, &install_root).expect("install");

        assert_eq!(receipt.name, "roundtrip-plugin");
        let installed = install_root.join("roundtrip-plugin");
        assert!(installed.join("install-receipt.json").is_file());
        assert!(installed
            .join(".elegy-plugin")
            .join("plugin.json")
            .is_file());
        assert!(installed
            .join("skills")
            .join("roundtrip-plugin")
            .join("SKILL.md")
            .is_file());
        assert!(
            verify_plugin_v1(&installed.join(".elegy-plugin"))
                .expect("verify installed plugin")
                .valid
        );
        assert!(matches!(
            install_from_archive(&archive_path, &install_root),
            Err(InstallError::AlreadyInstalled { .. })
        ));
    }

    #[test]
    fn install_rejects_unexpected_marketplace_identity_before_writing() {
        let temp = tempfile::tempdir().expect("tempdir");
        let archive_path = temp.path().join("plugin.zip");
        write_zip(
            &archive_path,
            &[
                (
                    "plugin.json",
                    r#"{"schemaVersion":"elegy-plugin/v1","name":"actual-plugin","version":"1.0.0","description":"desc","skills":"./skills/"}"#,
                ),
                ("skills/example/SKILL.md", "fixture"),
            ],
        );
        let install_root = temp.path().join("installed");

        let error = install_from_archive_with_identity(
            &archive_path,
            &install_root,
            Some("expected-plugin"),
            Some("1.0.0"),
        )
        .expect_err("identity mismatch must fail");

        assert!(matches!(error, InstallError::IdentityMismatch(_)));
        assert!(!install_root.join("actual-plugin").exists());
    }

    #[test]
    fn install_normalizes_legacy_component_paths_and_writes_manifest() {
        let temp = tempfile::tempdir().expect("tempdir");
        let archive_path = temp.path().join("legacy.zip");
        write_zip(
            &archive_path,
            &[
                (
                    "plugin.json",
                    r#"{"schemaVersion":"elegy-plugin/v1","name":"legacy-plugin","version":"1.0.0","description":"desc","skills":"skills/"}"#,
                ),
                ("skills/example/SKILL.md", "fixture"),
            ],
        );
        let install_root = temp.path().join("installed");

        let receipt = install_from_archive(&archive_path, &install_root).expect("install");
        let installed_manifest = fs::read_to_string(
            install_root
                .join("legacy-plugin")
                .join(".elegy-plugin")
                .join("plugin.json"),
        )
        .expect("read installed manifest");
        let installed: serde_json::Value =
            serde_json::from_str(&installed_manifest).expect("parse installed manifest");

        assert_eq!(installed["skills"], "./skills/");
        assert!(receipt
            .files
            .contains(&".elegy-plugin/plugin.json".to_string()));
    }

    #[test]
    fn checksum_mismatch_fails_closed() {
        let error = verify_sha256(b"archive", &"0".repeat(64)).expect_err("checksum must fail");
        assert!(matches!(error, InstallError::ChecksumMismatch { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn install_marks_bin_entries_executable() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let archive_path = temp.path().join("unix.zip");
        write_zip(
            &archive_path,
            &[
                (
                    "plugin.json",
                    r#"{"schemaVersion":"elegy-plugin/v1","name":"unix-plugin","version":"1.0.0","description":"desc","skills":"./skills/"}"#,
                ),
                ("skills/example/SKILL.md", "fixture"),
                ("bin/unix-plugin", "binary"),
            ],
        );
        let install_root = temp.path().join("installed");

        install_from_archive(&archive_path, &install_root).expect("install");
        let mode = fs::metadata(install_root.join("unix-plugin/bin/unix-plugin"))
            .expect("binary metadata")
            .permissions()
            .mode();

        assert_eq!(mode & 0o111, 0o111);
    }
}
