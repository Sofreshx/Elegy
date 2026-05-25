use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlanningStoreError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("failed to create planning database directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("entity not found: {entity_type} {entity_id}")]
    NotFound {
        entity_type: String,
        entity_id: String,
    },
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("projection target parent does not exist: {0}")]
    ProjectionParentMissing(PathBuf),
    #[error("failed to write projection {path}: {source}")]
    ProjectionWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("time formatting failed")]
    TimeFormat,
}

impl PlanningStoreError {
    pub fn is_invalid_input(&self) -> bool {
        matches!(
            self,
            Self::InvalidInput(_) | Self::NotFound { .. } | Self::ProjectionParentMissing(_)
        )
    }
}
