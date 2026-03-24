use thiserror::Error;

use crate::types::MemoryId;

/// Errors produced by [`crate::traits::MemoryStore`] implementations.
#[derive(Debug, Error)]
pub enum StoreError {
    /// The requested memory record does not exist.
    #[error("memory not found: {0}")]
    NotFound(MemoryId),
    /// The underlying SQLite backend returned an error.
    #[error("SQLite error: {0}")]
    Sqlite(String),
    /// Serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(String),
    /// Schema initialization or migration failed.
    #[error("schema migration failed: {0}")]
    Migration(String),
    /// The caller supplied invalid input for the requested operation.
    #[error("validation error: {0}")]
    Validation(String),
}

/// Errors produced by [`crate::traits::EmbeddingProvider`] implementations.
#[derive(Debug, Error)]
pub enum EmbeddingError {
    /// The provider failed to generate an embedding.
    #[error("embedding provider error: {0}")]
    Provider(String),
    /// The produced embedding does not match the expected dimensionality.
    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
}

/// Errors produced by [`crate::traits::SalienceGate`] implementations.
#[derive(Debug, Error)]
pub enum GateError {
    /// The salience gate could not query or mutate the backing store as needed.
    #[error("store error during gate evaluation: {0}")]
    Store(#[from] StoreError),
    /// The salience gate could not generate or compare embeddings.
    #[error("embedding error during novelty check: {0}")]
    Embedding(#[from] EmbeddingError),
    /// The candidate could not be evaluated because it violated gate invariants.
    #[error("invalid candidate: {0}")]
    InvalidCandidate(String),
}

/// Errors produced by [`crate::traits::MemoryConsolidator`] implementations.
#[derive(Debug, Error)]
pub enum ConsolidationError {
    /// Consolidation failed while reading from or writing to the store.
    #[error("store error during consolidation: {0}")]
    Store(#[from] StoreError),
    /// The requested consolidation operation is not supported by this implementation.
    #[error("unsupported consolidation operation: {0}")]
    Unsupported(String),
}

/// Errors produced by [`crate::traits::MemoryObservability`] implementations.
#[derive(Debug, Error)]
pub enum ObservabilityError {
    /// Observability failed because the underlying store operation failed.
    #[error("store error during observability operation: {0}")]
    Store(#[from] StoreError),
    /// Export or reporting failed for a non-store reason.
    #[error("observability operation failed: {0}")]
    Operation(String),
}
