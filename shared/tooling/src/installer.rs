use elegy_plugin_sdk::{validate_elegy_plugin_v1, ElegyPluginV1};
use serde::{Deserialize, Serialize};
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
    pub files: Vec<String>,
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
    let file = fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // Find and validate plugin.json
    let mut manifest: Option<ElegyPluginV1> = None;
    let mut manifest_index = None;
    for i in 0..archive.len() {
        let name = archive.by_index(i)?.name().to_string();
        if name == "plugin.json" || name.ends_with("/plugin.json") {
            let mut content = String::new();
            archive.by_index(i)?.read_to_string(&mut content)?;
            let plugin: ElegyPluginV1 = serde_json::from_str(&content)
                .map_err(|e| InstallError::InvalidManifest(format!("JSON parse: {e}")))?;
            let validation = validate_elegy_plugin_v1(&plugin);
            if !validation.is_valid() {
                return Err(InstallError::InvalidManifest(validation.issues.join("; ")));
            }
            manifest = Some(plugin);
            manifest_index = Some(i);
            break;
        }
    }

    let manifest = manifest.ok_or(InstallError::MissingManifest)?;

    // Determine install directory
    let install_dir = install_root.join(&manifest.name);
    if install_dir.exists() {
        return Err(InstallError::AlreadyInstalled {
            name: manifest.name.clone(),
            path: install_dir,
        });
    }

    // Extract all files
    fs::create_dir_all(&install_dir)?;
    let mut installed_files = Vec::new();

    for i in 0..archive.len() {
        // Skip the manifest entry (already read)
        if Some(i) == manifest_index {
            continue;
        }
        let mut entry = archive.by_index(i)?;
        let entry_name = entry.name().to_string();

        // Skip directories
        if entry_name.ends_with('/') {
            continue;
        }

        let relative_path =
            validate_archive_entry_path(&entry_name).map_err(InstallError::InvalidManifest)?;
        let dest_path = install_dir.join(&relative_path);
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut dest_file = fs::File::create(&dest_path)?;
        io::copy(&mut entry, &mut dest_file)?;
        installed_files.push(relative_path.to_string_lossy().replace('\\', "/"));
    }

    // Write install-receipt.json
    let receipt = InstallReceipt {
        schema_version: "elegy-installer/v1".to_string(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        installed_at: manual_iso8601_timestamp(),
        source: archive_path.display().to_string(),
        install_dir: install_dir.display().to_string(),
        files: installed_files,
    };

    let receipt_path = install_dir.join("install-receipt.json");
    let receipt_json = serde_json::to_string_pretty(&receipt)
        .map_err(|e| InstallError::InvalidManifest(format!("receipt serialize: {e}")))?;
    fs::write(&receipt_path, receipt_json)?;

    Ok(receipt)
}

/// Install a plugin from a URL (download then delegate to install_from_archive).
#[cfg(feature = "reqwest")]
pub fn install_from_url(url: &str, install_root: &Path) -> Result<InstallReceipt, InstallError> {
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

    let tmp = tempfile::NamedTempFile::new()?;
    fs::write(tmp.path(), &bytes)?;

    install_from_archive(tmp.path(), install_root)
}

/// Stub for URL install when reqwest feature is not enabled.
#[cfg(not(feature = "reqwest"))]
pub fn install_from_url(_url: &str, _install_root: &Path) -> Result<InstallReceipt, InstallError> {
    Err(InstallError::DownloadFailed(
        "URL install requires the 'reqwest' feature. Rebuild with --features reqwest.".into(),
    ))
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
    use super::{install_from_archive, InstallError};
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

    #[test]
    fn install_rejects_path_traversal_entries() {
        let temp = tempfile::tempdir().expect("tempdir");
        let archive_path = temp.path().join("bad.plugin.zip");
        write_zip(
            &archive_path,
            &[
                (
                    "plugin.json",
                    r#"{"schemaVersion":"elegy-plugin/v1","name":"safe-plugin","version":"0.1.0","description":"desc","skills":"skills/"}"#,
                ),
                ("../escape.txt", "nope"),
            ],
        );

        let err = install_from_archive(&archive_path, temp.path()).expect_err("must fail");
        assert!(
            matches!(err, InstallError::InvalidManifest(ref message) if message.contains("escapes the install root")),
            "unexpected error: {err}"
        );
    }
}
