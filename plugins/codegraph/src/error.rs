/// Errors for the elegy-codegraph crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Storage error: {0}")]
    Storage(#[source] Box<redb::Error>),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Extraction error: {0}")]
    Extraction(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

/// Crate-level result alias.
pub type Result<T> = std::result::Result<T, Error>;

impl From<redb::Error> for Error {
    fn from(error: redb::Error) -> Self {
        Self::Storage(Box::new(error))
    }
}
