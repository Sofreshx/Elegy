//! Memory contracts and implementations for governed local memory.
//!
//! SQLite modules own durable storage mechanics; the private artifacts module owns governed JSON envelopes, records, projections, and lifecycle validation.

mod artifacts;
pub mod cli;
pub mod consolidator;
pub mod decay;
pub mod embedding;
pub mod error;
mod eval;
pub mod forgetting;
pub mod gate;
pub mod llm;
mod local_store;
pub mod promotion;
pub mod recall;
pub mod runtime;
mod similarity;
pub mod storage;
pub mod traits;
pub mod types;

pub use artifacts::{
    GovernedMemoryRecord, GovernedMemoryRecordImportOptions, GovernedMemoryRecordProjection,
    LocalMemoryLifecycle, LocalMemoryLifecycleState, MemoryArtifactKind, MemoryRecordProvenance,
    MemoryRecordSortKeys, MemoryTombstoneMetadata, MemoryValidationError,
    SessionContextRepresentation, SessionContextScope, SummaryOnlySessionContext,
    SummaryOnlySessionContextEnvelope, GOVERNED_MEMORY_RECORD_ARTIFACT_KIND,
    GOVERNED_MEMORY_RECORD_PROJECTION_ARTIFACT_KIND, MAX_CONTEXT_ITEM_LENGTH,
    MAX_INSTRUCTION_CONTEXT_ITEMS, MAX_MEMORY_RECORD_ID_LENGTH, MAX_MEMORY_SORT_KEY_LENGTH,
    MAX_SALIENT_FACTS, MAX_SUMMARY_LENGTH, MEMORY_PROJECTION_RULES_VERSION,
    SUMMARY_ONLY_REPRESENTATION, SUMMARY_ONLY_SESSION_CONTEXT_ARTIFACT_KIND,
};
pub use consolidator::{LlmConsolidator, SimpleConsolidator};
pub use decay::{adaptive_retention, retention, retention_with_lambda, type_decay_multiplier};
pub use embedding::{
    OllamaEmbeddingProvider, OpenAiEmbeddingProvider, DEFAULT_OLLAMA_BASE_URL,
    DEFAULT_OLLAMA_CONNECT_TIMEOUT, DEFAULT_OLLAMA_DIMENSIONS, DEFAULT_OLLAMA_MODEL,
    DEFAULT_OLLAMA_REQUEST_TIMEOUT, DEFAULT_OPENAI_BASE_URL, DEFAULT_OPENAI_CONNECT_TIMEOUT,
    DEFAULT_OPENAI_DIMENSIONS, DEFAULT_OPENAI_MODEL, DEFAULT_OPENAI_REQUEST_TIMEOUT,
};
pub use error::{
    ConsolidationError, EmbeddingError, GateError, LlmError, ObservabilityError, StoreError,
};
pub use forgetting::{Fifo, ImportanceReliability, Lru, PriorityDecay, RandomDrop};
pub use gate::DefaultSalienceGate;
pub use llm::{
    OllamaLlmProvider, OpenAiLlmProvider, DEFAULT_OLLAMA_LLM_BASE_URL,
    DEFAULT_OLLAMA_LLM_CONNECT_TIMEOUT, DEFAULT_OLLAMA_LLM_MODEL,
    DEFAULT_OLLAMA_LLM_REQUEST_TIMEOUT, DEFAULT_OPENAI_LLM_BASE_URL,
    DEFAULT_OPENAI_LLM_CONNECT_TIMEOUT, DEFAULT_OPENAI_LLM_MODEL,
    DEFAULT_OPENAI_LLM_REQUEST_TIMEOUT,
};
pub use local_store::{
    LocalMemoryCatalog, LocalMemoryCatalogEntry, LocalMemoryExportResult, LocalMemoryPaths,
    LocalMemoryQueryOptions, LocalMemoryStore, LocalMemoryStoreError, LocalMemoryStoreInitResult,
    LocalMemoryStoredRecord, LOCAL_MEMORY_ARTIFACTS_DIR, LOCAL_MEMORY_AUTHORITY_POSTURE,
    LOCAL_MEMORY_DETERMINISTIC_ORDERING, LOCAL_MEMORY_EXPORTS_DIR,
    LOCAL_MEMORY_SINGLE_WRITER_POSTURE, LOCAL_MEMORY_STATE_DIR, LOCAL_MEMORY_STORE_KIND,
    LOCAL_MEMORY_WRITE_LOCK_RELATIVE_PATH,
};
pub use promotion::PromotionEngine;
pub use storage::{init_database, SqliteMemoryStore, CURRENT_SCHEMA_VERSION};
pub use traits::{
    ConsolidationAction, EmbeddingProvider, ForgettingPolicy, GateDecision, LlmProvider,
    MemoryConsolidator, MemoryFilter, MemoryObservability, MemoryStore, MetadataUpdate,
    OptionalFieldUpdate, RetentionContext, SalienceGate,
};
pub use types::{
    ConsolidationCandidate, ContradictionEntry, ContradictionRecord, CorrectionDisposition,
    CorrectionRecord, ElegyArchive, ExportFormat, GraphNode, GraphTraversalResult, Memory,
    MemoryCandidate, MemoryContextConfig, MemoryHealthReport, MemoryId, MemoryLink, MemoryScope,
    MemorySearchQuery, MemorySearchResult, MemoryState, MemoryType, MemoryVersion, PoisoningAlert,
    PoisoningAlertType, ProvenanceLevel, PurgeReport, ResolutionStatus, RetrievalFeedback,
    ScopeConfig, ScoredMemory, SearchQuery, SensitivityLevel, ShareConfig,
};
