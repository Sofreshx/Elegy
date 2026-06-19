use crate::{
    GovernedMemoryRecord, GovernedMemoryRecordImportOptions, LocalMemoryLifecycleState,
    MemoryValidationError, SessionContextScope, SummaryOnlySessionContextEnvelope,
};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const LOCAL_MEMORY_ARTIFACTS_DIR: &str = "artifacts";
pub const LOCAL_MEMORY_STATE_DIR: &str = "state";
pub const LOCAL_MEMORY_EXPORTS_DIR: &str = "exports";
pub const LOCAL_MEMORY_WRITE_LOCK_RELATIVE_PATH: &str = "state/write.lock";
pub const LOCAL_MEMORY_STORE_KIND: &str = "local-non-authoritative-artifact-store";
pub const LOCAL_MEMORY_AUTHORITY_POSTURE: &str = "local non-authoritative artifact management only";
pub const LOCAL_MEMORY_SINGLE_WRITER_POSTURE: &str =
    "single-writer local store; concurrent writers are rejected";
pub const LOCAL_MEMORY_DETERMINISTIC_ORDERING: &str =
    "scopeCapturedAtRecordId asc, lifecycleStateRecordId asc, recordId asc";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalMemoryQueryOptions {
    pub include_superseded: bool,
    pub include_tombstoned: bool,
}

impl LocalMemoryQueryOptions {
    pub fn includes_state(&self, state: LocalMemoryLifecycleState) -> bool {
        match state {
            LocalMemoryLifecycleState::Active => true,
            LocalMemoryLifecycleState::Superseded => self.include_superseded,
            LocalMemoryLifecycleState::Tombstoned => self.include_tombstoned,
        }
    }

    pub fn default_filter_label(&self) -> &'static str {
        if self.include_superseded && self.include_tombstoned {
            "active, superseded, tombstoned"
        } else if self.include_superseded {
            "active, superseded"
        } else if self.include_tombstoned {
            "active, tombstoned"
        } else {
            "active only"
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalMemoryStore {
    root: PathBuf,
}

impl LocalMemoryStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn paths(&self) -> LocalMemoryPaths {
        LocalMemoryPaths {
            root: self.root.clone(),
            artifacts_dir: self.root.join(LOCAL_MEMORY_ARTIFACTS_DIR),
            state_dir: self.root.join(LOCAL_MEMORY_STATE_DIR),
            write_lock_path: self.root.join(LOCAL_MEMORY_WRITE_LOCK_RELATIVE_PATH),
            exports_dir: self.root.join(LOCAL_MEMORY_EXPORTS_DIR),
        }
    }

    pub fn init(&self) -> Result<LocalMemoryStoreInitResult, LocalMemoryStoreError> {
        let paths = self.paths();
        fs::create_dir_all(&paths.root).map_err(|source| LocalMemoryStoreError::Io {
            operation: "create local memory root",
            path: paths.root.clone(),
            source,
        })?;
        fs::create_dir_all(paths.state_dir()).map_err(|source| LocalMemoryStoreError::Io {
            operation: "create local memory state directory",
            path: paths.state_dir().to_path_buf(),
            source,
        })?;

        let _lock = self.acquire_write_lock()?;
        self.ensure_layout_directories()?;

        Ok(LocalMemoryStoreInitResult { paths })
    }

    pub fn import_summary_only_envelope(
        &self,
        envelope: &SummaryOnlySessionContextEnvelope,
        options: GovernedMemoryRecordImportOptions,
    ) -> Result<LocalMemoryStoredRecord, LocalMemoryStoreError> {
        self.ensure_initialized()?;
        let _lock = self.acquire_write_lock()?;

        let record = GovernedMemoryRecord::import_summary_only_envelope(envelope, options)?;
        let artifact_path = self.artifact_path(&record.record_id);
        if artifact_path.is_file() {
            let existing = self.read_record_from_path(&artifact_path)?;
            if existing != record {
                return Err(LocalMemoryStoreError::RecordIdConflict {
                    record_id: record.record_id.clone(),
                });
            }

            return Ok(LocalMemoryStoredRecord {
                artifact_path,
                record: existing,
            });
        }

        self.write_record_to_path(&artifact_path, &record)?;

        Ok(LocalMemoryStoredRecord {
            artifact_path,
            record,
        })
    }

    pub fn list_records(
        &self,
        options: &LocalMemoryQueryOptions,
    ) -> Result<Vec<LocalMemoryCatalogEntry>, LocalMemoryStoreError> {
        self.ensure_initialized()?;

        Ok(self
            .collect_records()?
            .into_iter()
            .filter(|stored| options.includes_state(stored.record.local_lifecycle.state))
            .map(|stored| {
                LocalMemoryCatalogEntry::from_record(
                    &self.root,
                    &stored.artifact_path,
                    &self.export_path(&stored.record.record_id),
                    &stored.record,
                )
            })
            .collect())
    }

    pub fn show_record(
        &self,
        record_id: &str,
        options: &LocalMemoryQueryOptions,
    ) -> Result<LocalMemoryStoredRecord, LocalMemoryStoreError> {
        self.ensure_initialized()?;

        let artifact_path = self.artifact_path(record_id);
        if !artifact_path.is_file() {
            return Err(LocalMemoryStoreError::RecordNotFound {
                record_id: record_id.to_string(),
            });
        }

        let record = self.read_record_from_path(&artifact_path)?;
        if !options.includes_state(record.local_lifecycle.state) {
            return Err(LocalMemoryStoreError::RecordExcludedByLifecycle {
                record_id: record.record_id.clone(),
                state: record.local_lifecycle.state,
            });
        }

        Ok(LocalMemoryStoredRecord {
            artifact_path,
            record,
        })
    }

    pub fn export_summary_only_envelope(
        &self,
        record_id: &str,
        output_path: Option<&Path>,
        options: &LocalMemoryQueryOptions,
    ) -> Result<LocalMemoryExportResult, LocalMemoryStoreError> {
        self.ensure_initialized()?;

        let stored = self.show_record(record_id, options)?;
        let exported_envelope = stored.record.export_summary_only_envelope()?;
        let output_path = output_path
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.export_path(record_id));

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|source| LocalMemoryStoreError::Io {
                operation: "create local export directory",
                path: parent.to_path_buf(),
                source,
            })?;
        }
        self.write_json_to_path(&output_path, &exported_envelope, "write exported artifact")?;

        Ok(LocalMemoryExportResult {
            output_path,
            record: stored.record,
            exported_envelope,
        })
    }

    pub fn supersede_record(
        &self,
        record_id: &str,
        superseded_by_record_id: &str,
    ) -> Result<LocalMemoryStoredRecord, LocalMemoryStoreError> {
        self.ensure_initialized()?;
        if record_id == superseded_by_record_id {
            return Err(LocalMemoryStoreError::SelfSupersede {
                record_id: record_id.to_string(),
            });
        }

        let _lock = self.acquire_write_lock()?;
        let successor_path = self.artifact_path(superseded_by_record_id);
        if !successor_path.is_file() {
            return Err(LocalMemoryStoreError::SuccessorRecordNotFound {
                record_id: superseded_by_record_id.to_string(),
            });
        }

        let artifact_path = self.artifact_path(record_id);
        if !artifact_path.is_file() {
            return Err(LocalMemoryStoreError::RecordNotFound {
                record_id: record_id.to_string(),
            });
        }

        let record = self
            .read_record_from_path(&artifact_path)?
            .supersede(superseded_by_record_id)?;
        self.write_record_to_path(&artifact_path, &record)?;

        Ok(LocalMemoryStoredRecord {
            artifact_path,
            record,
        })
    }

    pub fn tombstone_record(
        &self,
        record_id: &str,
        tombstoned_at_utc: &str,
        reason: &str,
    ) -> Result<LocalMemoryStoredRecord, LocalMemoryStoreError> {
        self.ensure_initialized()?;
        let _lock = self.acquire_write_lock()?;

        let artifact_path = self.artifact_path(record_id);
        if !artifact_path.is_file() {
            return Err(LocalMemoryStoreError::RecordNotFound {
                record_id: record_id.to_string(),
            });
        }

        let record = self
            .read_record_from_path(&artifact_path)?
            .tombstone(tombstoned_at_utc, reason)?;
        self.write_record_to_path(&artifact_path, &record)?;

        Ok(LocalMemoryStoredRecord {
            artifact_path,
            record,
        })
    }

    fn ensure_initialized(&self) -> Result<(), LocalMemoryStoreError> {
        let paths = self.paths();
        if !paths.artifacts_dir.is_dir()
            || !paths.exports_dir.is_dir()
            || !paths.state_dir().is_dir()
        {
            return Err(LocalMemoryStoreError::RootNotInitialized {
                root: self.root.clone(),
            });
        }

        Ok(())
    }

    fn ensure_layout_directories(&self) -> Result<(), LocalMemoryStoreError> {
        let paths = self.paths();
        for (operation, path) in [
            (
                "create local artifacts directory",
                paths.artifacts_dir.clone(),
            ),
            ("create local state directory", paths.state_dir.clone()),
            ("create local exports directory", paths.exports_dir.clone()),
        ] {
            fs::create_dir_all(&path).map_err(|source| LocalMemoryStoreError::Io {
                operation,
                path,
                source,
            })?;
        }

        Ok(())
    }

    fn acquire_write_lock(&self) -> Result<LocalMemoryWriteLockGuard, LocalMemoryStoreError> {
        let paths = self.paths();
        fs::create_dir_all(paths.state_dir()).map_err(|source| LocalMemoryStoreError::Io {
            operation: "create local memory state directory",
            path: paths.state_dir().to_path_buf(),
            source,
        })?;

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&paths.write_lock_path)
        {
            Ok(mut file) => {
                file.write_all(LOCAL_MEMORY_SINGLE_WRITER_POSTURE.as_bytes())
                    .map_err(|source| LocalMemoryStoreError::Io {
                        operation: "write local store lock",
                        path: paths.write_lock_path.clone(),
                        source,
                    })?;
                Ok(LocalMemoryWriteLockGuard {
                    write_lock_path: paths.write_lock_path,
                })
            }
            Err(source) if source.kind() == ErrorKind::AlreadyExists => {
                Err(LocalMemoryStoreError::ConcurrentWriterRejected {
                    root: self.root.clone(),
                })
            }
            Err(source) => Err(LocalMemoryStoreError::Io {
                operation: "create local store lock",
                path: paths.write_lock_path,
                source,
            }),
        }
    }

    fn collect_records(&self) -> Result<Vec<LocalMemoryStoredRecord>, LocalMemoryStoreError> {
        let artifacts_dir = self.paths().artifacts_dir;
        let artifact_paths = collect_artifact_paths(
            fs::read_dir(&artifacts_dir)
                .map_err(|source| LocalMemoryStoreError::Io {
                    operation: "read local artifacts directory",
                    path: artifacts_dir.clone(),
                    source,
                })?
                .map(|entry| entry.map(|entry| entry.path())),
            &artifacts_dir,
        )?;

        let mut stored_records = artifact_paths
            .into_iter()
            .map(|artifact_path| {
                self.read_record_from_path(&artifact_path)
                    .map(|record| LocalMemoryStoredRecord {
                        artifact_path,
                        record,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        stored_records.sort_by(compare_stored_records);
        Ok(stored_records)
    }

    fn read_record_from_path(
        &self,
        path: &Path,
    ) -> Result<GovernedMemoryRecord, LocalMemoryStoreError> {
        let contents = fs::read_to_string(path).map_err(|source| LocalMemoryStoreError::Io {
            operation: "read local artifact",
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_str(&contents).map_err(|source| {
            LocalMemoryStoreError::InvalidArtifactJson {
                path: path.to_path_buf(),
                source,
            }
        })
    }

    fn write_record_to_path(
        &self,
        path: &Path,
        record: &GovernedMemoryRecord,
    ) -> Result<(), LocalMemoryStoreError> {
        self.write_json_to_path(path, record, "write local artifact")
    }

    fn write_json_to_path<T: Serialize>(
        &self,
        path: &Path,
        value: &T,
        operation: &'static str,
    ) -> Result<(), LocalMemoryStoreError> {
        let contents = serde_json::to_string_pretty(value).map_err(|source| {
            LocalMemoryStoreError::InvalidJsonSerialization {
                path: path.to_path_buf(),
                source,
            }
        })?;
        fs::write(path, format!("{contents}\n")).map_err(|source| LocalMemoryStoreError::Io {
            operation,
            path: path.to_path_buf(),
            source,
        })
    }

    fn artifact_path(&self, record_id: &str) -> PathBuf {
        self.paths()
            .artifacts_dir
            .join(artifact_file_name(record_id))
    }

    fn export_path(&self, record_id: &str) -> PathBuf {
        self.paths().exports_dir.join(export_file_name(record_id))
    }

    pub fn default_export_path(&self, record_id: &str) -> PathBuf {
        self.export_path(record_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalMemoryPaths {
    pub root: PathBuf,
    pub artifacts_dir: PathBuf,
    pub state_dir: PathBuf,
    pub write_lock_path: PathBuf,
    pub exports_dir: PathBuf,
}

impl LocalMemoryPaths {
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalMemoryStoreInitResult {
    pub paths: LocalMemoryPaths,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalMemoryStoredRecord {
    pub artifact_path: PathBuf,
    pub record: GovernedMemoryRecord,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalMemoryExportResult {
    pub output_path: PathBuf,
    pub record: GovernedMemoryRecord,
    pub exported_envelope: SummaryOnlySessionContextEnvelope,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalMemoryCatalog {
    pub store_kind: String,
    pub authority_posture: String,
    pub single_writer_posture: String,
    pub deterministic_ordering: String,
    pub records: Vec<LocalMemoryCatalogEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalMemoryCatalogEntry {
    pub record_id: String,
    pub artifact_path: String,
    pub default_export_path: String,
    pub scope: SessionContextScope,
    pub lifecycle_state: LocalMemoryLifecycleState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at_utc: Option<String>,
    pub imported_at_utc: String,
    pub scope_captured_at_record_id: String,
    pub lifecycle_state_record_id: String,
    pub superseded_by_record_id_or_self: String,
}

impl LocalMemoryCatalogEntry {
    fn from_record(
        root: &Path,
        artifact_path: &Path,
        export_path: &Path,
        record: &GovernedMemoryRecord,
    ) -> Self {
        Self {
            record_id: record.record_id.clone(),
            artifact_path: relative_path_string(root, artifact_path),
            default_export_path: relative_path_string(root, export_path),
            scope: record.session_context.scope,
            lifecycle_state: record.local_lifecycle.state,
            captured_at_utc: record.provenance.captured_at_utc.clone(),
            imported_at_utc: record.provenance.imported_at_utc.clone(),
            scope_captured_at_record_id: record
                .deterministic_sort_keys
                .scope_captured_at_record_id
                .clone(),
            lifecycle_state_record_id: record
                .deterministic_sort_keys
                .lifecycle_state_record_id
                .clone(),
            superseded_by_record_id_or_self: record
                .deterministic_sort_keys
                .superseded_by_record_id_or_self
                .clone(),
        }
    }
}

#[derive(Debug, Error)]
pub enum LocalMemoryStoreError {
    #[error("local memory root is not initialized at {root}")]
    RootNotInitialized { root: PathBuf },
    #[error(
        "state/write.lock is already present under {root}; local artifact writes assume a single writer"
    )]
    ConcurrentWriterRejected { root: PathBuf },
    #[error("local record `{record_id}` was not found")]
    RecordNotFound { record_id: String },
    #[error(
        "local record `{record_id}` is hidden by the default active-only filter because it is `{}`",
        state.as_str()
    )]
    RecordExcludedByLifecycle {
        record_id: String,
        state: LocalMemoryLifecycleState,
    },
    #[error("local record `{record_id}` already exists with different artifact contents")]
    RecordIdConflict { record_id: String },
    #[error("local record `{record_id}` cannot be marked as superseded by itself")]
    SelfSupersede { record_id: String },
    #[error("local successor record `{record_id}` was not found")]
    SuccessorRecordNotFound { record_id: String },
    #[error("{operation} failed for {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("artifact JSON at {path} is invalid: {source}")]
    InvalidArtifactJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("could not serialize JSON for {path}: {source}")]
    InvalidJsonSerialization {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error(transparent)]
    MemoryValidation(#[from] MemoryValidationError),
}

struct LocalMemoryWriteLockGuard {
    write_lock_path: PathBuf,
}

impl Drop for LocalMemoryWriteLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.write_lock_path);
    }
}

fn compare_stored_records(
    left: &LocalMemoryStoredRecord,
    right: &LocalMemoryStoredRecord,
) -> Ordering {
    left.record
        .deterministic_sort_keys
        .scope_captured_at_record_id
        .cmp(
            &right
                .record
                .deterministic_sort_keys
                .scope_captured_at_record_id,
        )
        .then_with(|| {
            left.record
                .deterministic_sort_keys
                .lifecycle_state_record_id
                .cmp(
                    &right
                        .record
                        .deterministic_sort_keys
                        .lifecycle_state_record_id,
                )
        })
        .then_with(|| left.record.record_id.cmp(&right.record.record_id))
}

fn collect_artifact_paths<I>(
    entries: I,
    artifacts_dir: &Path,
) -> Result<Vec<PathBuf>, LocalMemoryStoreError>
where
    I: IntoIterator<Item = std::io::Result<PathBuf>>,
{
    let mut artifact_paths = Vec::new();
    for entry in entries {
        let artifact_path = entry.map_err(|source| LocalMemoryStoreError::Io {
            operation: "iterate local artifacts directory",
            path: artifacts_dir.to_path_buf(),
            source,
        })?;

        if artifact_path.extension().and_then(|value| value.to_str()) == Some("json") {
            artifact_paths.push(artifact_path);
        }
    }

    artifact_paths.sort();
    Ok(artifact_paths)
}

fn relative_path_string(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn artifact_file_name(record_id: &str) -> String {
    format!(
        "{}.governed-memory-record.json",
        encode_record_id_for_file_name(record_id)
    )
}

fn export_file_name(record_id: &str) -> String {
    format!(
        "{}.summary-only-session-context-envelope.json",
        encode_record_id_for_file_name(record_id)
    )
}

fn encode_record_id_for_file_name(record_id: &str) -> String {
    let mut encoded = String::with_capacity(record_id.len());
    for byte in record_id.bytes() {
        let ch = byte as char;
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            encoded.push(ch);
        } else {
            encoded.push('~');
            encoded.push(nibble_to_hex(byte >> 4));
            encoded.push(nibble_to_hex(byte & 0x0f));
        }
    }
    encoded
}

fn nibble_to_hex(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + (value - 10)) as char,
        _ => unreachable!("nibble must fit in hex"),
    }
}

#[cfg(test)]
mod tests {
    use super::collect_artifact_paths;
    use crate::LocalMemoryStoreError;
    use std::io::ErrorKind;
    use std::path::PathBuf;

    #[test]
    fn collect_artifact_paths_propagates_directory_entry_errors() {
        let artifacts_dir = PathBuf::from("local-store/artifacts");
        let error = collect_artifact_paths(
            vec![
                Ok(artifacts_dir.join("record-a.governed-memory-record.json")),
                Err(std::io::Error::new(ErrorKind::PermissionDenied, "denied")),
            ],
            &artifacts_dir,
        )
        .expect_err("directory entry failures should be surfaced");

        match error {
            LocalMemoryStoreError::Io {
                operation,
                path,
                source,
            } => {
                assert_eq!(operation, "iterate local artifacts directory");
                assert_eq!(path, artifacts_dir);
                assert_eq!(source.kind(), ErrorKind::PermissionDenied);
            }
            other => panic!("unexpected error: {other}"),
        }
    }
}
