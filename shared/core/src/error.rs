use serde_json;
use std::path::PathBuf;
use thiserror::Error;
use zip;

/// Error type for contract loading, validation, and archive operations.
#[derive(Debug, Error)]
pub enum ContractsError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to write archive {path}: {source}")]
    Archive {
        path: PathBuf,
        #[source]
        source: zip::result::ZipError,
    },
    #[error("compatibility manifest is missing schema '{0}'")]
    MissingSchema(String),
    #[error("{0}")]
    Compatibility(String),
}
