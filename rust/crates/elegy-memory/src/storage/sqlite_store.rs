use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, types::Type, Connection, OptionalExtension, Row};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::schema::init_database;
use crate::{
    decay,
    similarity::cosine_similarity,
    traits::{EmbeddingProvider, MemoryStore},
    types::{
        ContradictionEntry, Memory, MemoryContextConfig, MemoryHealthReport, MemoryId, MemoryScope,
        MemoryState, MemoryType, MemoryVersion, ProvenanceLevel, PurgeReport, ResolutionStatus,
        ScopeConfig, ScoredMemory, SearchQuery, SensitivityLevel,
    },
    EmbeddingError, MemoryFilter, MetadataUpdate, OptionalFieldUpdate, StoreError,
};

const MEMORY_SELECT_COLUMNS: &str = r#"
    id,
    content,
    summary,
    scope,
    memory_type,
    provenance,
    importance_score,
    reliability_score,
    sensitivity,
    state,
    tags,
    status,
    custom_metadata,
    access_count,
    corroboration_count,
    embedding_stale,
    created_at,
    updated_at,
    last_accessed_at,
    tenant_id,
    user_id,
    agent_id
"#;

const DEFAULT_EMBEDDING_DIMENSIONS: usize = 768;
const DEFAULT_WORKSPACE_BUDGET: f32 = 500.0;
const DEFAULT_USER_BUDGET: f32 = 1_000.0;
const DEFAULT_AGENT_BUDGET: f32 = 200.0;
const VECTOR_SIMILARITY_BLEND_WEIGHT: f32 = 0.7;
const KEYWORD_SIMILARITY_BLEND_WEIGHT: f32 = 0.3;
const ESTIMATED_CHARS_PER_TOKEN: usize = 4;
const BASE_MEMORY_TOKEN_OVERHEAD: u32 = 16;

/// SQLite-backed [`MemoryStore`] implementation for the MVP memory schema.
#[derive(Clone)]
pub struct SqliteMemoryStore {
    connection: Arc<Mutex<Connection>>,
    scope: MemoryScope,
    embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
}

impl std::fmt::Debug for SqliteMemoryStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteMemoryStore")
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

impl SqliteMemoryStore {
    /// Open or create a SQLite-backed store at `path` for a single logical scope.
    pub fn new(path: impl AsRef<Path>, scope: MemoryScope) -> Result<Self, StoreError> {
        Self::new_with_optional_embedding_provider(path, scope, None)
    }

    /// Open or create a SQLite-backed store with an embedding provider for automatic embedding flows.
    pub fn new_with_embedding_provider(
        path: impl AsRef<Path>,
        scope: MemoryScope,
        embedding_provider: Arc<dyn EmbeddingProvider>,
    ) -> Result<Self, StoreError> {
        Self::new_with_optional_embedding_provider(path, scope, Some(embedding_provider))
    }

    /// Open or create a SQLite-backed store with an optional embedding provider.
    pub fn new_with_optional_embedding_provider(
        path: impl AsRef<Path>,
        scope: MemoryScope,
        embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
    ) -> Result<Self, StoreError> {
        let connection = init_database(path.as_ref())?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            scope,
            embedding_provider,
        })
    }

    /// Returns the scope this store instance is responsible for.
    #[must_use]
    pub const fn scope(&self) -> MemoryScope {
        self.scope
    }

    /// Promote a memory to a broader scope and record promotion provenance.
    pub fn promote_memory_to(
        &self,
        id: &MemoryId,
        to_scope: MemoryScope,
        changed_by: &str,
        reason: &str,
        trigger_session_id: Option<&str>,
    ) -> Result<Option<Memory>, StoreError> {
        self.with_connection(|connection| {
            let scope_config = load_scope_config(connection)?;
            let transaction = connection.transaction()?;
            let Some(mut memory) = require_memory(&transaction, id)? else {
                return Ok(None);
            };

            if memory.scope == to_scope {
                return Ok(Some(memory));
            }
            if !memory.scope.can_promote_to(to_scope) {
                return Err(StoreError::Validation(format!(
                    "cannot promote memory {} from {} to {}",
                    memory.id,
                    scope_to_db(memory.scope),
                    scope_to_db(to_scope)
                )));
            }

            record_promotion(
                &transaction,
                &mut memory,
                to_scope,
                reason,
                changed_by,
                trigger_session_id,
                &scope_config,
            )?;
            transaction.commit()?;
            Ok(Some(memory))
        })
    }

    /// Evaluate automatic promotion criteria and apply promotions for the visible scopes.
    pub fn run_promotion_pass(
        &self,
        limit: Option<usize>,
        trigger_session_id: Option<&str>,
    ) -> Result<Vec<Memory>, StoreError> {
        self.with_connection(|connection| {
            let scope_config = load_scope_config(connection)?;
            let mut memories = load_search_memories(
                connection,
                self.scope.visible_scopes(),
                MemoryState::Active,
                None,
            )?;
            memories.sort_by(|left, right| {
                right
                    .updated_at
                    .cmp(&left.updated_at)
                    .then_with(|| right.id.cmp(&left.id))
            });
            if let Some(limit) = limit {
                memories.truncate(limit);
            }

            let mut promoted = Vec::new();
            for memory in memories {
                if let Some(to_scope) =
                    promotion_target(connection, &memory, &scope_config, trigger_session_id)?
                {
                    let transaction = connection.transaction()?;
                    let Some(mut latest) = require_memory(&transaction, &memory.id)? else {
                        continue;
                    };
                    record_promotion(
                        &transaction,
                        &mut latest,
                        to_scope,
                        "automatic promotion pass",
                        "system:promotion",
                        trigger_session_id,
                        &scope_config,
                    )?;
                    transaction.commit()?;
                    promoted.push(latest);
                }
            }

            Ok(promoted)
        })
    }

    /// Load active memories plus their stored embeddings for consolidation.
    pub fn list_consolidation_candidates(
        &self,
        scopes: &[MemoryScope],
        limit: Option<usize>,
    ) -> Result<Vec<crate::ConsolidationCandidate>, StoreError> {
        self.with_connection(|connection| {
            let mut memories = load_search_memories(connection, scopes, MemoryState::Active, None)?;
            memories.sort_by(|left, right| {
                right
                    .updated_at
                    .cmp(&left.updated_at)
                    .then_with(|| right.id.cmp(&left.id))
            });
            if let Some(limit) = limit {
                memories.truncate(limit);
            }

            let expected_dimensions = load_embedding_dimensions(connection)?;
            let mut candidates = Vec::with_capacity(memories.len());
            for memory in memories {
                let embedding = load_stored_embedding(connection, &memory.id, expected_dimensions)?;
                candidates.push(crate::ConsolidationCandidate { memory, embedding });
            }
            Ok(candidates)
        })
    }

    /// Record the timestamp of the latest consolidation pass.
    pub fn mark_consolidation_run(&self) -> Result<(), StoreError> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO scope_config(key, value) VALUES ('last_consolidation_at', ?1) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [format_timestamp(Utc::now())],
            )?;
            Ok(())
        })
    }

    async fn generate_embedding(&self, text: &str) -> Result<Option<Vec<f32>>, EmbeddingError> {
        let trimmed_text = text.trim();
        if trimmed_text.is_empty() {
            return Ok(None);
        }

        let Some(embedding_provider) = self.embedding_provider.as_ref() else {
            return Ok(None);
        };

        embedding_provider.embed(trimmed_text).await.map(Some)
    }

    async fn reuse_cached_embedding(
        &self,
        id: &MemoryId,
        content_sha256: &str,
    ) -> Result<bool, StoreError> {
        let Some(encoded_embedding) = self.with_connection(|connection| {
            load_cached_embedding_blob(connection, self.scope, content_sha256)
        })?
        else {
            return Ok(false);
        };

        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            if require_memory(&transaction, id)?.is_none() {
                return Err(StoreError::NotFound(*id));
            }

            let expected_dimensions = load_embedding_dimensions(&transaction)?;
            decode_embedding(&encoded_embedding, expected_dimensions)?;
            upsert_encoded_embedding(&transaction, id, &encoded_embedding, content_sha256)?;
            transaction.commit()?;

            Ok(true)
        })
    }

    fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| StoreError::Sqlite("sqlite connection lock poisoned".to_string()))?;
        operation(&mut connection)
    }

    /// Load the effective scope configuration used by this store.
    pub fn scope_config(&self) -> Result<ScopeConfig, StoreError> {
        self.with_connection(|connection| load_scope_config(connection))
    }

    /// Load version-history rows for a single memory without mutating access tracking.
    pub fn list_versions(&self, id: &MemoryId) -> Result<Vec<MemoryVersion>, StoreError> {
        self.with_connection(|connection| {
            if require_memory(connection, id)?.is_none() {
                return Err(StoreError::NotFound(*id));
            }

            let mut statement = connection.prepare(
                r#"
                SELECT id, memory_id, version_number, content, changed_by, change_reason, changed_at
                FROM memory_versions
                WHERE memory_id = ?1
                ORDER BY version_number DESC, changed_at DESC, id DESC
                "#,
            )?;
            let rows = statement.query_map([id.to_string()], map_memory_version_row)?;
            let mut versions = Vec::new();
            for row in rows {
                versions.push(row?);
            }
            Ok(versions)
        })
    }
}

#[async_trait]
impl MemoryStore for SqliteMemoryStore {
    fn scope(&self) -> MemoryScope {
        self.scope
    }

    async fn store(&self, memory: Memory) -> Result<MemoryId, StoreError> {
        let should_attempt_embedding =
            self.embedding_provider.is_some() && !memory.content.trim().is_empty();
        let content_sha256 = should_attempt_embedding.then(|| content_sha256(&memory.content));
        let mut memory = memory;
        if should_attempt_embedding {
            memory.embedding_stale = true;
        }

        validate_memory_for_store(&memory, self.scope)?;

        let id = self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            transaction.execute(
                r#"
                INSERT INTO memories(
                    id,
                    content,
                    summary,
                    scope,
                    memory_type,
                    provenance,
                    importance_score,
                    reliability_score,
                    sensitivity,
                    state,
                    tags,
                    status,
                    custom_metadata,
                    access_count,
                    corroboration_count,
                    embedding_stale,
                    created_at,
                    updated_at,
                    last_accessed_at,
                    tenant_id,
                    user_id,
                    agent_id
                )
                VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                    ?18, ?19, ?20, ?21, ?22
                )
                "#,
                rusqlite::params_from_iter(memory_insert_params(&memory)?),
            )?;

            let row_id = require_memory_rowid(&transaction, &memory.id)?;
            sync_fts_entry(&transaction, row_id, None, &memory)?;
            transaction.commit()?;

            Ok(memory.id)
        })?;

        if !should_attempt_embedding {
            return Ok(id);
        }

        if let Some(content_sha256) = content_sha256.as_deref() {
            if self.reuse_cached_embedding(&id, content_sha256).await? {
                return Ok(id);
            }
        }

        let embedding: Vec<f32> = match self.generate_embedding(&memory.content).await {
            Ok(Some(embedding)) => embedding,
            Ok(None) => return Ok(id),
            Err(error) => {
                if let Some(warning) = embedding_degradation_warning(&error) {
                    eprintln!("warning: {warning}");
                }
                return Ok(id);
            }
        };

        match self.store_embedding(&id, &embedding).await {
            Ok(()) | Err(StoreError::Validation(_)) => Ok(id),
            Err(error) => Err(error),
        }
    }

    async fn update_content(
        &self,
        id: &MemoryId,
        new_content: &str,
        changed_by: &str,
        reason: &str,
    ) -> Result<(), StoreError> {
        let trimmed_content = new_content.trim();
        if trimmed_content.is_empty() {
            return Err(StoreError::Validation(
                "memory content must not be empty".to_string(),
            ));
        }
        if changed_by.trim().is_empty() {
            return Err(StoreError::Validation(
                "changed_by must not be empty".to_string(),
            ));
        }

        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let mut memory = require_memory(&transaction, id)?.ok_or(StoreError::NotFound(*id))?;
            let previous_memory = memory.clone();

            if memory.content == trimmed_content {
                return Ok(());
            }

            let row_id = require_memory_rowid(&transaction, id)?;
            let next_version_number = load_next_version_number(&transaction, id)?;
            let changed_at = Utc::now();

            transaction.execute(
                r#"
                INSERT INTO memory_versions(
                    id,
                    memory_id,
                    version_number,
                    content,
                    changed_at,
                    changed_by,
                    change_reason
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    Uuid::new_v4().to_string(),
                    id.to_string(),
                    i64::from(next_version_number),
                    memory.content,
                    format_timestamp(changed_at),
                    changed_by.trim(),
                    reason,
                ],
            )?;

            memory.content = trimmed_content.to_string();
            memory.embedding_stale = true;
            memory.updated_at = changed_at;

            persist_memory(&transaction, &memory)?;
            sync_fts_entry(&transaction, row_id, Some(&previous_memory), &memory)?;
            transaction.commit()?;

            Ok(())
        })
    }

    async fn update_metadata(
        &self,
        id: &MemoryId,
        updates: MetadataUpdate,
    ) -> Result<(), StoreError> {
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let mut memory = require_memory(&transaction, id)?.ok_or(StoreError::NotFound(*id))?;
            let row_id = require_memory_rowid(&transaction, id)?;
            let previous_memory = memory.clone();
            let mut changed = false;

            if let Some(tags) = updates.tags {
                memory.tags = tags;
                changed = true;
            }

            if let Some(status) = updates.status {
                memory.status = match status {
                    OptionalFieldUpdate::Set(value) => Some(value),
                    OptionalFieldUpdate::Clear => None,
                };
                changed = true;
            }

            if let Some(custom_metadata) = updates.custom_metadata {
                memory.custom_metadata = custom_metadata;
                changed = true;
            }

            if let Some(importance_score) = updates.importance_score {
                validate_unit_interval("importance_score", importance_score)?;
                memory.importance_score = importance_score;
                changed = true;
            }

            if let Some(reliability_score) = updates.reliability_score {
                validate_unit_interval("reliability_score", reliability_score)?;
                memory.reliability_score = reliability_score;
                changed = true;
            }

            if let Some(state) = updates.state {
                memory.state = state;
                changed = true;
            }

            if !changed {
                return Ok(());
            }

            memory.updated_at = Utc::now();
            persist_memory(&transaction, &memory)?;
            sync_fts_entry(&transaction, row_id, Some(&previous_memory), &memory)?;
            transaction.commit()?;

            Ok(())
        })
    }

    async fn get(&self, id: &MemoryId) -> Result<Option<Memory>, StoreError> {
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let memory = require_memory(&transaction, id)?;

            let Some(mut memory) = memory else {
                return Ok(None);
            };

            memory.access_count = memory
                .access_count
                .checked_add(1)
                .ok_or(StoreError::Validation("access_count overflow".to_string()))?;
            memory.last_accessed_at = Some(Utc::now());

            transaction.execute(
                r#"
                UPDATE memories
                SET access_count = ?2,
                    last_accessed_at = ?3
                WHERE id = ?1
                "#,
                params![
                    id.to_string(),
                    i64::from(memory.access_count),
                    memory.last_accessed_at.map(format_timestamp),
                ],
            )?;

            transaction.commit()?;
            Ok(Some(memory))
        })
    }

    async fn get_raw(&self, id: &MemoryId) -> Result<Option<Memory>, StoreError> {
        self.with_connection(|connection| require_memory(connection, id))
    }

    async fn list(&self, filter: MemoryFilter) -> Result<Vec<Memory>, StoreError> {
        if matches!(filter.limit, Some(0)) {
            return Ok(Vec::new());
        }
        let list_scope = filter.scope.unwrap_or(self.scope);

        self.with_connection(|connection| {
            let mut sql = format!("SELECT {MEMORY_SELECT_COLUMNS} FROM memories WHERE scope = ?1");
            let mut params: Vec<rusqlite::types::Value> = vec![rusqlite::types::Value::from(
                scope_to_db(list_scope).to_string(),
            )];

            if let Some(state) = filter.state {
                sql.push_str(" AND state = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(rusqlite::types::Value::from(state_to_db(state).to_string()));
            }

            if let Some(status) = &filter.status {
                sql.push_str(" AND status = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(rusqlite::types::Value::from(status.clone()));
            }

            if let Some(tenant_id) = &filter.tenant_id {
                sql.push_str(" AND tenant_id = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(rusqlite::types::Value::from(tenant_id.clone()));
            }

            if let Some(user_id) = &filter.user_id {
                sql.push_str(" AND user_id = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(rusqlite::types::Value::from(user_id.clone()));
            }

            if let Some(agent_id) = &filter.agent_id {
                sql.push_str(" AND agent_id = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(rusqlite::types::Value::from(agent_id.clone()));
            }

            sql.push_str(" ORDER BY created_at ASC");

            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(rusqlite::params_from_iter(params), map_memory_row)?;

            let mut memories = Vec::new();
            for row in rows {
                let memory = row?;
                if !matches_filter(&memory, &filter) {
                    continue;
                }
                memories.push(memory);
                if filter.limit.is_some_and(|limit| memories.len() >= limit) {
                    break;
                }
            }

            Ok(memories)
        })
    }

    async fn search(&self, query: SearchQuery) -> Result<Vec<ScoredMemory>, StoreError> {
        let trimmed_text = query.text.trim().to_string();
        if query.max_results == 0 {
            return Ok(Vec::new());
        }
        if trimmed_text.is_empty() && query.embedding.is_none() {
            return Err(StoreError::Validation(
                "search requires non-empty text or a query embedding".to_string(),
            ));
        }

        let requested_state = query.state_filter.unwrap_or(MemoryState::Active);
        if requested_state == MemoryState::Deleted {
            return Ok(Vec::new());
        }

        let derived_query_embedding = if query.embedding.is_none() {
            self.generate_embedding(&trimmed_text)
                .await
                .unwrap_or_default()
        } else {
            None
        };

        self.with_connection(|connection| {
            let scope_config = load_scope_config(connection)?;
            let visible_scopes = query.scope.visible_scopes();
            let query_embedding = match query.embedding.as_deref() {
                Some(embedding) => {
                    let expected_dimensions = load_embedding_dimensions(connection)?;
                    validate_query_embedding(embedding, expected_dimensions)?;
                    Some(embedding)
                }
                None => match derived_query_embedding.as_deref() {
                    Some(embedding) => {
                        let expected_dimensions = load_embedding_dimensions(connection)?;
                        match validate_query_embedding(embedding, expected_dimensions) {
                            Ok(()) => Some(embedding),
                            Err(StoreError::Validation(_)) => None,
                            Err(error) => return Err(error),
                        }
                    }
                    None => None,
                },
            };

            let mut keyword_scores = if trimmed_text.is_empty() {
                HashMap::new()
            } else {
                load_keyword_scores(
                    connection,
                    visible_scopes,
                    requested_state,
                    query.type_filter.as_deref(),
                    &trimmed_text,
                )?
            };

            let mut vector_scores = match query_embedding {
                Some(embedding) => load_vector_similarity_scores(
                    connection,
                    visible_scopes,
                    requested_state,
                    query.type_filter.as_deref(),
                    embedding,
                    0.0,
                )?,
                None => HashMap::new(),
            };

            let candidate_ids: HashSet<MemoryId> = keyword_scores
                .keys()
                .copied()
                .chain(vector_scores.keys().copied())
                .collect();
            if candidate_ids.is_empty() {
                return Ok(Vec::new());
            }

            let candidate_memories = load_search_memories(
                connection,
                visible_scopes,
                requested_state,
                query.type_filter.as_deref(),
            )?;
            let candidate_memories_by_id: HashMap<MemoryId, Memory> = candidate_memories
                .into_iter()
                .filter(|memory| candidate_ids.contains(&memory.id))
                .map(|memory| (memory.id, memory))
                .collect();

            let scoring_now = Utc::now();
            let mut results = Vec::with_capacity(candidate_memories_by_id.len());
            for id in candidate_ids {
                let Some(memory) = candidate_memories_by_id.get(&id).cloned() else {
                    continue;
                };

                let keyword_similarity = keyword_scores.remove(&id);
                let vector_similarity = vector_scores.remove(&id);
                let similarity = combine_similarity_signals(vector_similarity, keyword_similarity);
                let score =
                    compute_retrieval_score(&memory, similarity, &scope_config, scoring_now);
                results.push(ScoredMemory {
                    memory,
                    score,
                    similarity,
                });
            }

            results.sort_by(|left, right| {
                right
                    .score
                    .total_cmp(&left.score)
                    .then_with(|| right.similarity.total_cmp(&left.similarity))
                    .then_with(|| right.memory.updated_at.cmp(&left.memory.updated_at))
                    .then_with(|| right.memory.id.cmp(&left.memory.id))
            });
            results.truncate(query.max_results);
            let mut results = trim_results_to_context_budget(
                results,
                query.context_config.as_ref(),
                &scope_config,
            );
            touch_scored_memories(connection, &mut results)?;
            auto_promote_scored_memories(
                connection,
                &mut results,
                query.session_id.as_deref(),
                &scope_config,
            )?;
            Ok(results)
        })
    }

    async fn find_similar(
        &self,
        embedding: &[f32],
        threshold: f32,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>, StoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        validate_similarity_threshold(threshold)?;

        self.with_connection(|connection| {
            let expected_dimensions = load_embedding_dimensions(connection)?;
            validate_query_embedding(embedding, expected_dimensions)?;
            let visible_scopes = self.scope.visible_scopes();

            let similarity_scores = load_vector_similarity_scores(
                connection,
                visible_scopes,
                MemoryState::Active,
                None,
                embedding,
                threshold,
            )?;
            if similarity_scores.is_empty() {
                return Ok(Vec::new());
            }

            let memories_by_id: HashMap<MemoryId, Memory> =
                load_search_memories(connection, visible_scopes, MemoryState::Active, None)?
                    .into_iter()
                    .filter_map(|memory| {
                        similarity_scores
                            .get(&memory.id)
                            .copied()
                            .map(|_| (memory.id, memory))
                    })
                    .collect();

            let mut results = similarity_scores
                .into_iter()
                .filter_map(|(id, similarity)| {
                    memories_by_id.get(&id).cloned().map(|memory| ScoredMemory {
                        memory,
                        score: similarity,
                        similarity,
                    })
                })
                .collect::<Vec<_>>();

            results.sort_by(|left, right| {
                right
                    .similarity
                    .total_cmp(&left.similarity)
                    .then_with(|| right.memory.updated_at.cmp(&left.memory.updated_at))
                    .then_with(|| right.memory.id.cmp(&left.memory.id))
            });
            results.truncate(limit);
            Ok(results)
        })
    }

    async fn store_embedding(&self, id: &MemoryId, embedding: &[f32]) -> Result<(), StoreError> {
        if embedding.is_empty() {
            return Err(StoreError::Validation(
                "embedding vector must not be empty".to_string(),
            ));
        }

        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let memory = require_memory(&transaction, id)?.ok_or(StoreError::NotFound(*id))?;

            let expected_dimensions = load_embedding_dimensions(&transaction)?;
            if embedding.len() != expected_dimensions {
                return Err(StoreError::Validation(format!(
                    "embedding dimension mismatch: expected {expected_dimensions}, got {}",
                    embedding.len()
                )));
            }

            let content_sha256 = content_sha256(&memory.content);
            let encoded_embedding = encode_embedding(embedding);
            upsert_encoded_embedding(&transaction, id, &encoded_embedding, &content_sha256)?;
            transaction.commit()?;

            Ok(())
        })
    }

    async fn get_stale_embeddings(&self, limit: usize) -> Result<Vec<MemoryId>, StoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                r#"
                SELECT id
                FROM memories
                WHERE scope = ?1
                  AND embedding_stale = 1
                ORDER BY updated_at ASC, created_at ASC
                LIMIT ?2
                "#,
            )?;
            let rows = statement.query_map(
                params![
                    scope_to_db(self.scope),
                    i64::try_from(limit).unwrap_or(i64::MAX)
                ],
                |row| row.get::<_, String>(0),
            )?;

            let mut ids = Vec::new();
            for row in rows {
                let raw_id = row?;
                ids.push(parse_uuid(&raw_id)?);
            }

            Ok(ids)
        })
    }

    async fn make_dormant(&self, id: &MemoryId) -> Result<(), StoreError> {
        transition_state(self, id, MemoryState::Dormant).await
    }

    async fn reactivate(&self, id: &MemoryId) -> Result<(), StoreError> {
        transition_state(self, id, MemoryState::Active).await
    }

    async fn hard_delete(&self, id: &MemoryId) -> Result<(), StoreError> {
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let memory = require_memory(&transaction, id)?.ok_or(StoreError::NotFound(*id))?;
            let row_id = require_memory_rowid(&transaction, id)?;
            let vec_rowid: Option<i64> = transaction
                .query_row(
                    "SELECT vec_rowid FROM memory_embeddings WHERE memory_id = ?1",
                    [id.to_string()],
                    |row| row.get(0),
                )
                .optional()?;

            delete_fts_entry(&transaction, row_id, &memory)?;

            if let Some(vec_rowid) = vec_rowid {
                transaction.execute("DELETE FROM vec_memories WHERE rowid = ?1", [vec_rowid])?;
            }

            let deleted_rows = transaction.execute(
                "DELETE FROM memories WHERE id = ?1",
                [memory.id.to_string()],
            )?;
            if deleted_rows == 0 {
                return Err(StoreError::NotFound(*id));
            }

            transaction.commit()?;
            Ok(())
        })
    }

    async fn purge_user(&self, _user_id: &str) -> Result<PurgeReport, StoreError> {
        Err(StoreError::Validation(
            "purge_user is reserved for a later work unit and is intentionally left as an explicit stub in WU4"
                .to_string(),
        ))
    }

    async fn purge_all(&self) -> Result<PurgeReport, StoreError> {
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let memories_deleted = count_table_rows(&transaction, "memories")?;
            let versions_deleted = count_table_rows(&transaction, "memory_versions")?;
            let links_deleted = count_table_rows(&transaction, "memory_links")?;
            let contradictions_deleted = count_table_rows(&transaction, "contradictions")?;
            let embeddings_deleted = count_table_rows(&transaction, "memory_embeddings")?;

            transaction.execute("DELETE FROM contradictions", [])?;
            transaction.execute("DELETE FROM memory_links", [])?;
            transaction.execute("DELETE FROM memory_versions", [])?;
            transaction.execute("DELETE FROM memory_embeddings", [])?;
            transaction.execute("DELETE FROM vec_memories", [])?;
            transaction.execute("DELETE FROM memories", [])?;
            transaction.execute(
                "INSERT INTO memories_fts(memories_fts) VALUES('rebuild')",
                [],
            )?;
            transaction.commit()?;

            Ok(PurgeReport {
                memories_deleted: i64_to_u64(memories_deleted, "memories_deleted")?,
                versions_deleted: i64_to_u64(versions_deleted, "versions_deleted")?,
                links_deleted: i64_to_u64(links_deleted, "links_deleted")?,
                contradictions_deleted: i64_to_u64(
                    contradictions_deleted,
                    "contradictions_deleted",
                )?,
                embeddings_deleted: i64_to_u64(embeddings_deleted, "embeddings_deleted")?,
            })
        })
    }

    async fn health_report(&self) -> Result<MemoryHealthReport, StoreError> {
        self.with_connection(|connection| {
            let active_count =
                count_memories_by_state(connection, self.scope, MemoryState::Active)?;
            let dormant_count =
                count_memories_by_state(connection, self.scope, MemoryState::Dormant)?;
            let stale_embeddings_count = connection.query_row(
                "SELECT COUNT(*) FROM memories WHERE scope = ?1 AND embedding_stale = 1",
                [scope_to_db(self.scope)],
                |row| row.get::<_, i64>(0),
            )?;
            let unresolved_contradictions = connection.query_row(
                "SELECT COUNT(*) FROM contradictions WHERE resolution_status = 'unresolved'",
                [],
                |row| row.get::<_, i64>(0),
            )?;
            let page_count =
                connection.query_row("PRAGMA page_count", [], |row| row.get::<_, i64>(0))?;
            let page_size =
                connection.query_row("PRAGMA page_size", [], |row| row.get::<_, i64>(0))?;
            let oldest_active_memory = connection
                .query_row(
                    "SELECT MIN(created_at) FROM memories WHERE scope = ?1 AND state = 'active'",
                    [scope_to_db(self.scope)],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()?
                .flatten()
                .map(|value| parse_datetime(&value))
                .transpose()?;
            let newest_memory = connection
                .query_row(
                    "SELECT MAX(created_at) FROM memories WHERE scope = ?1",
                    [scope_to_db(self.scope)],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()?
                .flatten()
                .map(|value| parse_datetime(&value))
                .transpose()?;
            let last_consolidation = connection
                .query_row(
                    "SELECT value FROM scope_config WHERE key = 'last_consolidation_at'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .map(|value| parse_datetime(&value))
                .transpose()?;

            Ok(MemoryHealthReport {
                scope: self.scope,
                active_count: i64_to_u64(active_count, "active_count")?,
                dormant_count: i64_to_u64(dormant_count, "dormant_count")?,
                total_storage_bytes: i64_to_u64(page_count, "page_count")?
                    .saturating_mul(i64_to_u64(page_size, "page_size")?),
                budget_usage_ratio: compute_budget_usage_ratio(
                    connection,
                    self.scope,
                    active_count,
                )?,
                unresolved_contradictions: i64_to_u64(
                    unresolved_contradictions,
                    "unresolved_contradictions",
                )?,
                stale_embeddings_count: i64_to_u64(
                    stale_embeddings_count,
                    "stale_embeddings_count",
                )?,
                last_consolidation,
                oldest_active_memory,
                newest_memory,
            })
        })
    }

    async fn list_contradictions(
        &self,
        status: Option<ResolutionStatus>,
    ) -> Result<Vec<ContradictionEntry>, StoreError> {
        self.with_connection(|connection| {
            let mut sql = String::from(
                "SELECT c.id, c.memory_a_id, c.memory_b_id, c.detected_at, c.description, c.resolution_status, c.resolved_at, c.resolution_note \
                 FROM contradictions c \
                 JOIN memories a ON a.id = c.memory_a_id \
                 JOIN memories b ON b.id = c.memory_b_id \
                 WHERE ",
            );
            let visible_scopes = self.scope.visible_scopes();
            let mut params: Vec<rusqlite::types::Value> = Vec::new();
            sql.push('(');
            sql.push_str(&scope_in_clause("a.scope", visible_scopes, &mut params));
            sql.push_str(") AND (");
            sql.push_str(&scope_in_clause("b.scope", visible_scopes, &mut params));
            sql.push(')');
            if let Some(status) = status {
                sql.push_str(" AND c.resolution_status = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(rusqlite::types::Value::from(
                    resolution_status_to_db(status).to_string(),
                ));
            }
            sql.push_str(" ORDER BY c.detected_at DESC, c.id ASC");

            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(rusqlite::params_from_iter(params), map_contradiction_row)?;
            let mut contradictions = Vec::new();
            for row in rows {
                contradictions.push(row?);
            }
            Ok(contradictions)
        })
    }

    async fn record_contradiction(
        &self,
        a_id: &MemoryId,
        b_id: &MemoryId,
        description: &str,
    ) -> Result<(), StoreError> {
        if a_id == b_id {
            return Err(StoreError::Validation(
                "a contradiction requires two distinct memory ids".to_string(),
            ));
        }
        if description.trim().is_empty() {
            return Err(StoreError::Validation(
                "contradiction description must not be empty".to_string(),
            ));
        }

        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let memory_a =
                require_memory(&transaction, a_id)?.ok_or(StoreError::NotFound(*a_id))?;
            let memory_b =
                require_memory(&transaction, b_id)?.ok_or(StoreError::NotFound(*b_id))?;
            let now = Utc::now();

            transaction.execute(
                r#"
                INSERT INTO contradictions(
                    id,
                    memory_a_id,
                    memory_b_id,
                    detected_at,
                    description,
                    resolution_status,
                    resolved_at,
                    resolution_note
                )
                VALUES (?1, ?2, ?3, ?4, ?5, 'unresolved', NULL, NULL)
                "#,
                params![
                    Uuid::new_v4().to_string(),
                    a_id.to_string(),
                    b_id.to_string(),
                    format_timestamp(now),
                    description.trim(),
                ],
            )?;

            if memory_a.provenance.base_reliability() > memory_b.provenance.base_reliability() {
                lower_reliability(&transaction, &memory_b, now)?;
            } else if memory_b.provenance.base_reliability()
                > memory_a.provenance.base_reliability()
            {
                lower_reliability(&transaction, &memory_a, now)?;
            }

            transaction.commit()?;
            Ok(())
        })
    }

    async fn update_contradiction_status(
        &self,
        contradiction_id: &str,
        status: ResolutionStatus,
        note: Option<&str>,
    ) -> Result<(), StoreError> {
        let trimmed_id = contradiction_id.trim();
        if trimmed_id.is_empty() {
            return Err(StoreError::Validation(
                "contradiction id must not be empty".to_string(),
            ));
        }

        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let normalized_note = note.and_then(|value| {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then_some(trimmed)
            });
            let resolved_at = if status == ResolutionStatus::Unresolved {
                None
            } else {
                Some(format_timestamp(Utc::now()))
            };
            let updated_rows = transaction.execute(
                r#"
                UPDATE contradictions
                SET resolution_status = ?2,
                    resolved_at = ?3,
                    resolution_note = ?4
                WHERE id = ?1
                "#,
                params![
                    trimmed_id,
                    resolution_status_to_db(status),
                    resolved_at,
                    normalized_note,
                ],
            )?;
            if updated_rows == 0 {
                return Err(StoreError::Validation(format!(
                    "contradiction not found: {trimmed_id}"
                )));
            }

            transaction.commit()?;
            Ok(())
        })
    }
}

fn embedding_degradation_warning(error: &EmbeddingError) -> Option<String> {
    let EmbeddingError::Provider(message) = error else {
        return None;
    };

    provider_not_reachable_warning(message, "ollama not reachable at ", "Ollama")
        .or_else(|| provider_not_reachable_warning(message, "openai not reachable at ", "OpenAI"))
        .or_else(|| openai_degradation_warning(message))
}

fn provider_not_reachable_warning(
    message: &str,
    prefix: &str,
    display_name: &str,
) -> Option<String> {
    message
        .strip_prefix(prefix)
        .and_then(|remainder| remainder.split_once(": ").map(|(url, _)| url.trim()))
        .map(|url| {
            format!(
                "{display_name} not reachable at {url}, storing without embeddings. Run reembed later.",
            )
        })
}

fn openai_degradation_warning(message: &str) -> Option<String> {
    if let Some(remainder) = message.strip_prefix("openai returned ") {
        let (status, detail) = remainder
            .split_once(": ")
            .map_or((remainder.trim(), None), |(status, detail)| {
                (status.trim(), summarize_openai_error_detail(detail))
            });
        let context = detail.map_or_else(
            || status.to_string(),
            |detail| format!("{status}: {detail}"),
        );
        return Some(format!(
            "OpenAI embeddings unavailable ({context}), storing without embeddings. Run reembed later.",
        ));
    }

    if let Some(remainder) = message.strip_prefix("openai embeddings request returned ") {
        let status = remainder
            .split_once(": ")
            .map_or(remainder.trim(), |(status, _)| status.trim());
        return Some(format!(
            "OpenAI embeddings unavailable ({status}), storing without embeddings. Run reembed later.",
        ));
    }

    None
}

fn summarize_openai_error_detail(detail: &str) -> Option<&str> {
    let detail = detail.trim();
    if detail.is_empty() {
        return None;
    }

    let summary = detail.split(" (").next().unwrap_or(detail).trim();
    if summary.is_empty() {
        None
    } else {
        Some(summary)
    }
}

async fn transition_state(
    store: &SqliteMemoryStore,
    id: &MemoryId,
    target_state: MemoryState,
) -> Result<(), StoreError> {
    store.with_connection(|connection| {
        let transaction = connection.transaction()?;
        let mut memory = require_memory(&transaction, id)?.ok_or(StoreError::NotFound(*id))?;

        if memory.state == MemoryState::Deleted {
            return Err(StoreError::Validation(format!(
                "memory {id} is logically deleted and cannot transition to {}",
                state_to_db(target_state)
            )));
        }

        if memory.state == target_state {
            return Ok(());
        }

        memory.state = target_state;
        memory.updated_at = Utc::now();
        persist_memory(&transaction, &memory)?;
        transaction.commit()?;

        Ok(())
    })
}

fn validate_memory_for_store(
    memory: &Memory,
    expected_scope: MemoryScope,
) -> Result<(), StoreError> {
    if memory.scope != expected_scope {
        return Err(StoreError::Validation(format!(
            "memory scope {} does not match store scope {}",
            scope_to_db(memory.scope),
            scope_to_db(expected_scope)
        )));
    }
    if memory.content.trim().is_empty() {
        return Err(StoreError::Validation(
            "memory content must not be empty".to_string(),
        ));
    }
    validate_unit_interval("importance_score", memory.importance_score)?;
    validate_unit_interval("reliability_score", memory.reliability_score)?;
    Ok(())
}

fn validate_unit_interval(field: &str, value: f32) -> Result<(), StoreError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        return Ok(());
    }

    Err(StoreError::Validation(format!(
        "{field} must be a finite value in the inclusive range 0.0..=1.0"
    )))
}

fn memory_insert_params(memory: &Memory) -> Result<Vec<rusqlite::types::Value>, StoreError> {
    Ok(vec![
        rusqlite::types::Value::from(memory.id.to_string()),
        rusqlite::types::Value::from(memory.content.clone()),
        optional_string_value(memory.summary.clone()),
        rusqlite::types::Value::from(scope_to_db(memory.scope).to_string()),
        rusqlite::types::Value::from(memory_type_to_db(memory.memory_type).to_string()),
        rusqlite::types::Value::from(provenance_to_db(memory.provenance).to_string()),
        rusqlite::types::Value::from(f64::from(memory.importance_score)),
        rusqlite::types::Value::from(f64::from(memory.reliability_score)),
        rusqlite::types::Value::from(sensitivity_to_db(memory.sensitivity).to_string()),
        rusqlite::types::Value::from(state_to_db(memory.state).to_string()),
        rusqlite::types::Value::from(serialize_json(&memory.tags)?),
        optional_string_value(memory.status.clone()),
        rusqlite::types::Value::from(serialize_json(&memory.custom_metadata)?),
        rusqlite::types::Value::from(i64::from(memory.access_count)),
        rusqlite::types::Value::from(i64::from(memory.corroboration_count)),
        rusqlite::types::Value::from(i64::from(memory.embedding_stale as u8)),
        rusqlite::types::Value::from(format_timestamp(memory.created_at)),
        rusqlite::types::Value::from(format_timestamp(memory.updated_at)),
        optional_string_value(memory.last_accessed_at.map(format_timestamp)),
        optional_string_value(memory.tenant_id.clone()),
        optional_string_value(memory.user_id.clone()),
        optional_string_value(memory.agent_id.clone()),
    ])
}

fn optional_string_value(value: Option<String>) -> rusqlite::types::Value {
    match value {
        Some(value) => rusqlite::types::Value::from(value),
        None => rusqlite::types::Value::Null,
    }
}

fn persist_memory(connection: &Connection, memory: &Memory) -> Result<(), StoreError> {
    connection.execute(
        r#"
        UPDATE memories
        SET content = ?2,
            summary = ?3,
            scope = ?4,
            memory_type = ?5,
            provenance = ?6,
            importance_score = ?7,
            reliability_score = ?8,
            sensitivity = ?9,
            state = ?10,
            tags = ?11,
            status = ?12,
            custom_metadata = ?13,
            access_count = ?14,
            corroboration_count = ?15,
            embedding_stale = ?16,
            created_at = ?17,
            updated_at = ?18,
            last_accessed_at = ?19,
            tenant_id = ?20,
            user_id = ?21,
            agent_id = ?22
        WHERE id = ?1
        "#,
        rusqlite::params_from_iter(memory_insert_params(memory)?),
    )?;
    Ok(())
}

fn sync_fts_entry(
    connection: &Connection,
    row_id: i64,
    previous_memory: Option<&Memory>,
    memory: &Memory,
) -> Result<(), StoreError> {
    if let Some(previous_memory) = previous_memory {
        delete_fts_entry(connection, row_id, previous_memory)?;
    }
    let indexed_fields = indexed_fts_fields(memory);
    connection.execute(
        "INSERT INTO memories_fts(rowid, content, summary, tags) VALUES (?1, ?2, ?3, ?4)",
        params![
            row_id,
            indexed_fields.content,
            indexed_fields.summary.as_deref(),
            indexed_fields.tags
        ],
    )?;
    Ok(())
}

fn delete_fts_entry(
    connection: &Connection,
    row_id: i64,
    memory: &Memory,
) -> Result<(), StoreError> {
    let indexed_fields = indexed_fts_fields(memory);
    connection.execute(
        "INSERT INTO memories_fts(memories_fts, rowid, content, summary, tags) VALUES ('delete', ?1, ?2, ?3, ?4)",
        params![
            row_id,
            indexed_fields.content,
            indexed_fields.summary.as_deref(),
            indexed_fields.tags
        ],
    )?;
    Ok(())
}

struct IndexedFtsFields {
    content: String,
    summary: Option<String>,
    tags: String,
}

fn indexed_fts_fields(memory: &Memory) -> IndexedFtsFields {
    IndexedFtsFields {
        content: expand_compound_words(&memory.content),
        summary: memory.summary.as_deref().map(expand_compound_words),
        tags: indexed_tags(memory),
    }
}

fn indexed_tags(memory: &Memory) -> String {
    expand_compound_words(&memory.tags.join(" "))
}

fn expand_compound_words(text: &str) -> String {
    let mut expansions = Vec::new();
    let mut seen_expansions = HashSet::new();
    let mut token = String::new();

    for character in text.chars() {
        if character.is_alphanumeric() || character == '_' {
            token.push(character);
        } else {
            collect_compound_word_expansion(&token, &mut expansions, &mut seen_expansions);
            token.clear();
        }
    }

    collect_compound_word_expansion(&token, &mut expansions, &mut seen_expansions);

    if expansions.is_empty() {
        return text.to_string();
    }

    let expansion_length = expansions.iter().map(String::len).sum::<usize>();
    let mut expanded = String::with_capacity(text.len() + expansion_length + expansions.len());
    expanded.push_str(text);

    for expansion in expansions {
        expanded.push(' ');
        expanded.push_str(&expansion);
    }

    expanded
}

fn collect_compound_word_expansion(
    token: &str,
    expansions: &mut Vec<String>,
    seen_expansions: &mut HashSet<String>,
) {
    let Some(expansion) = split_compound_word(token) else {
        return;
    };

    if seen_expansions.insert(expansion.clone()) {
        expansions.push(expansion);
    }
}

fn split_compound_word(token: &str) -> Option<String> {
    let characters = token.chars().collect::<Vec<_>>();
    if characters.len() < 2 {
        return None;
    }

    let mut parts = Vec::new();
    let mut current_part = String::new();

    for (index, character) in characters.iter().copied().enumerate() {
        if index > 0 {
            let previous = characters[index - 1];
            let next = characters.get(index + 1).copied();
            let has_boundary = (previous.is_lowercase() && character.is_uppercase())
                || (previous.is_uppercase()
                    && character.is_uppercase()
                    && next.is_some_and(|next_character| next_character.is_lowercase()))
                || (previous.is_ascii_digit() && character.is_alphabetic())
                || (previous.is_alphabetic() && character.is_ascii_digit());

            if has_boundary && !current_part.is_empty() {
                parts.push(std::mem::take(&mut current_part));
            }
        }

        current_part.push(character);
    }

    if !current_part.is_empty() {
        parts.push(current_part);
    }

    if parts.len() > 1 {
        Some(parts.join(" "))
    } else {
        None
    }
}

fn require_memory_rowid(connection: &Connection, id: &MemoryId) -> Result<i64, StoreError> {
    connection
        .query_row(
            "SELECT rowid FROM memories WHERE id = ?1",
            [id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .ok_or(StoreError::NotFound(*id))
}

fn require_memory(connection: &Connection, id: &MemoryId) -> Result<Option<Memory>, StoreError> {
    connection
        .query_row(
            &format!("SELECT {MEMORY_SELECT_COLUMNS} FROM memories WHERE id = ?1"),
            [id.to_string()],
            map_memory_row,
        )
        .optional()
        .map_err(StoreError::from)
}

fn map_memory_row(row: &Row<'_>) -> rusqlite::Result<Memory> {
    let raw_id: String = row.get(0)?;
    let raw_scope: String = row.get(3)?;
    let raw_memory_type: String = row.get(4)?;
    let raw_provenance: String = row.get(5)?;
    let raw_sensitivity: String = row.get(8)?;
    let raw_state: String = row.get(9)?;
    let raw_tags: Option<String> = row.get(10)?;
    let raw_custom_metadata: Option<String> = row.get(12)?;
    let raw_access_count: i64 = row.get(13)?;
    let raw_corroboration_count: i64 = row.get(14)?;
    let raw_embedding_stale: i64 = row.get(15)?;
    let raw_created_at: String = row.get(16)?;
    let raw_updated_at: String = row.get(17)?;
    let raw_last_accessed_at: Option<String> = row.get(18)?;

    Ok(Memory {
        id: parse_uuid_for_sqlite(&raw_id)?,
        content: row.get(1)?,
        summary: row.get(2)?,
        scope: parse_scope_for_sqlite(&raw_scope)?,
        memory_type: parse_memory_type_for_sqlite(&raw_memory_type)?,
        provenance: parse_provenance_for_sqlite(&raw_provenance)?,
        importance_score: row.get(6)?,
        reliability_score: row.get(7)?,
        sensitivity: parse_sensitivity_for_sqlite(&raw_sensitivity)?,
        state: parse_state_for_sqlite(&raw_state)?,
        tags: parse_json_for_sqlite(raw_tags.as_deref().unwrap_or("[]"))?,
        status: row.get(11)?,
        custom_metadata: parse_json_for_sqlite(raw_custom_metadata.as_deref().unwrap_or("{}"))?,
        access_count: i64_to_u32_for_sqlite(raw_access_count, "access_count")?,
        corroboration_count: i64_to_u32_for_sqlite(raw_corroboration_count, "corroboration_count")?,
        embedding_stale: raw_embedding_stale != 0,
        created_at: parse_datetime_for_sqlite(&raw_created_at)?,
        updated_at: parse_datetime_for_sqlite(&raw_updated_at)?,
        last_accessed_at: raw_last_accessed_at
            .as_deref()
            .map(parse_datetime_for_sqlite)
            .transpose()?,
        tenant_id: row.get(19)?,
        user_id: row.get(20)?,
        agent_id: row.get(21)?,
    })
}

fn map_contradiction_row(row: &Row<'_>) -> rusqlite::Result<ContradictionEntry> {
    let raw_id: String = row.get(0)?;
    let raw_memory_a_id: String = row.get(1)?;
    let raw_memory_b_id: String = row.get(2)?;
    let raw_detected_at: String = row.get(3)?;
    let raw_resolution_status: String = row.get(5)?;
    let raw_resolved_at: Option<String> = row.get(6)?;

    Ok(ContradictionEntry {
        id: raw_id,
        memory_a_id: parse_uuid_for_sqlite(&raw_memory_a_id)?,
        memory_b_id: parse_uuid_for_sqlite(&raw_memory_b_id)?,
        detected_at: parse_datetime_for_sqlite(&raw_detected_at)?,
        description: row.get(4)?,
        resolution_status: parse_resolution_status_for_sqlite(&raw_resolution_status)?,
        resolved_at: raw_resolved_at
            .as_deref()
            .map(parse_datetime_for_sqlite)
            .transpose()?,
        resolution_note: row.get(7)?,
    })
}

fn map_memory_version_row(row: &Row<'_>) -> rusqlite::Result<MemoryVersion> {
    let raw_memory_id: String = row.get(1)?;
    let raw_changed_at: String = row.get(6)?;

    Ok(MemoryVersion {
        id: row.get(0)?,
        memory_id: parse_uuid_for_sqlite(&raw_memory_id)?,
        version_number: i64_to_u32_for_sqlite(row.get(2)?, "version_number")?,
        content: row.get(3)?,
        changed_by: row.get(4)?,
        change_reason: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
        changed_at: parse_datetime_for_sqlite(&raw_changed_at)?,
    })
}

fn parse_json<T>(raw: &str) -> Result<T, StoreError>
where
    T: DeserializeOwned,
{
    serde_json::from_str(raw).map_err(|error| {
        StoreError::Serialization(format!("failed to decode JSON `{raw}`: {error}"))
    })
}

fn serialize_json<T>(value: &T) -> Result<String, StoreError>
where
    T: Serialize,
{
    serde_json::to_string(value)
        .map_err(|error| StoreError::Serialization(format!("failed to encode JSON: {error}")))
}

fn parse_json_for_sqlite<T>(raw: &str) -> rusqlite::Result<T>
where
    T: DeserializeOwned,
{
    parse_json(raw).map_err(sqlite_conversion_error)
}

fn count_table_rows(connection: &Connection, table: &str) -> Result<i64, StoreError> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    connection
        .query_row(&sql, [], |row| row.get::<_, i64>(0))
        .map_err(StoreError::from)
}

fn parse_uuid(raw: &str) -> Result<MemoryId, StoreError> {
    Uuid::parse_str(raw)
        .map_err(|error| StoreError::Serialization(format!("invalid memory id `{raw}`: {error}")))
}

fn parse_uuid_for_sqlite(raw: &str) -> rusqlite::Result<MemoryId> {
    parse_uuid(raw).map_err(sqlite_conversion_error)
}

fn parse_datetime(raw: &str) -> Result<DateTime<Utc>, StoreError> {
    DateTime::parse_from_rfc3339(raw)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| {
            StoreError::Serialization(format!("invalid RFC3339 timestamp `{raw}`: {error}"))
        })
}

fn parse_datetime_for_sqlite(raw: &str) -> rusqlite::Result<DateTime<Utc>> {
    parse_datetime(raw).map_err(sqlite_conversion_error)
}

fn i64_to_u32_for_sqlite(value: i64, field: &str) -> rusqlite::Result<u32> {
    u32::try_from(value).map_err(|_| {
        sqlite_conversion_error(StoreError::Serialization(format!(
            "{field} value `{value}` does not fit into u32"
        )))
    })
}

fn i64_to_u64(value: i64, field: &str) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| {
        StoreError::Serialization(format!("{field} value `{value}` does not fit into u64"))
    })
}

fn sqlite_conversion_error(error: StoreError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error))
}

fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339()
}

fn scope_to_db(scope: MemoryScope) -> &'static str {
    match scope {
        MemoryScope::Session => "session",
        MemoryScope::Workspace => "workspace",
        MemoryScope::User => "user",
        MemoryScope::Agent => "agent",
    }
}

fn parse_scope(raw: &str) -> Result<MemoryScope, StoreError> {
    match raw {
        "session" => Ok(MemoryScope::Session),
        "workspace" => Ok(MemoryScope::Workspace),
        "user" => Ok(MemoryScope::User),
        "agent" => Ok(MemoryScope::Agent),
        _ => Err(StoreError::Serialization(format!(
            "unknown memory scope `{raw}`"
        ))),
    }
}

fn parse_scope_for_sqlite(raw: &str) -> rusqlite::Result<MemoryScope> {
    parse_scope(raw).map_err(sqlite_conversion_error)
}

fn memory_type_to_db(memory_type: MemoryType) -> &'static str {
    match memory_type {
        MemoryType::Fact => "fact",
        MemoryType::Preference => "preference",
        MemoryType::Decision => "decision",
        MemoryType::Procedure => "procedure",
        MemoryType::Observation => "observation",
    }
}

fn parse_memory_type(raw: &str) -> Result<MemoryType, StoreError> {
    match raw {
        "fact" => Ok(MemoryType::Fact),
        "preference" => Ok(MemoryType::Preference),
        "decision" => Ok(MemoryType::Decision),
        "procedure" => Ok(MemoryType::Procedure),
        "observation" => Ok(MemoryType::Observation),
        _ => Err(StoreError::Serialization(format!(
            "unknown memory type `{raw}`"
        ))),
    }
}

fn parse_memory_type_for_sqlite(raw: &str) -> rusqlite::Result<MemoryType> {
    parse_memory_type(raw).map_err(sqlite_conversion_error)
}

fn provenance_to_db(provenance: ProvenanceLevel) -> &'static str {
    match provenance {
        ProvenanceLevel::UserStated => "user_stated",
        ProvenanceLevel::AgentObserved => "agent_observed",
        ProvenanceLevel::Consolidated => "consolidated",
        ProvenanceLevel::Imported => "imported",
        ProvenanceLevel::AgentInferred => "agent_inferred",
    }
}

fn parse_provenance(raw: &str) -> Result<ProvenanceLevel, StoreError> {
    match raw {
        "user_stated" => Ok(ProvenanceLevel::UserStated),
        "agent_observed" => Ok(ProvenanceLevel::AgentObserved),
        "consolidated" => Ok(ProvenanceLevel::Consolidated),
        "imported" => Ok(ProvenanceLevel::Imported),
        "agent_inferred" => Ok(ProvenanceLevel::AgentInferred),
        _ => Err(StoreError::Serialization(format!(
            "unknown provenance level `{raw}`"
        ))),
    }
}

fn parse_provenance_for_sqlite(raw: &str) -> rusqlite::Result<ProvenanceLevel> {
    parse_provenance(raw).map_err(sqlite_conversion_error)
}

fn sensitivity_to_db(sensitivity: SensitivityLevel) -> &'static str {
    match sensitivity {
        SensitivityLevel::Low => "low",
        SensitivityLevel::Medium => "medium",
        SensitivityLevel::High => "high",
        SensitivityLevel::Critical => "critical",
    }
}

fn parse_sensitivity(raw: &str) -> Result<SensitivityLevel, StoreError> {
    match raw {
        "low" => Ok(SensitivityLevel::Low),
        "medium" => Ok(SensitivityLevel::Medium),
        "high" => Ok(SensitivityLevel::High),
        "critical" => Ok(SensitivityLevel::Critical),
        _ => Err(StoreError::Serialization(format!(
            "unknown sensitivity level `{raw}`"
        ))),
    }
}

fn parse_sensitivity_for_sqlite(raw: &str) -> rusqlite::Result<SensitivityLevel> {
    parse_sensitivity(raw).map_err(sqlite_conversion_error)
}

fn state_to_db(state: MemoryState) -> &'static str {
    match state {
        MemoryState::Active => "active",
        MemoryState::Dormant => "dormant",
        MemoryState::Deleted => "deleted",
    }
}

fn parse_state(raw: &str) -> Result<MemoryState, StoreError> {
    match raw {
        "active" => Ok(MemoryState::Active),
        "dormant" => Ok(MemoryState::Dormant),
        "deleted" => Ok(MemoryState::Deleted),
        _ => Err(StoreError::Serialization(format!(
            "unknown memory state `{raw}`"
        ))),
    }
}

fn parse_state_for_sqlite(raw: &str) -> rusqlite::Result<MemoryState> {
    parse_state(raw).map_err(sqlite_conversion_error)
}

fn resolution_status_to_db(status: ResolutionStatus) -> &'static str {
    match status {
        ResolutionStatus::Unresolved => "unresolved",
        ResolutionStatus::ResolvedByUser => "resolved_by_user",
        ResolutionStatus::ResolvedBySystem => "resolved_by_system",
        ResolutionStatus::Dismissed => "dismissed",
    }
}

fn parse_resolution_status(raw: &str) -> Result<ResolutionStatus, StoreError> {
    match raw {
        "unresolved" => Ok(ResolutionStatus::Unresolved),
        "resolved_by_user" => Ok(ResolutionStatus::ResolvedByUser),
        "resolved_by_system" => Ok(ResolutionStatus::ResolvedBySystem),
        "dismissed" => Ok(ResolutionStatus::Dismissed),
        _ => Err(StoreError::Serialization(format!(
            "unknown resolution status `{raw}`"
        ))),
    }
}

fn parse_resolution_status_for_sqlite(raw: &str) -> rusqlite::Result<ResolutionStatus> {
    parse_resolution_status(raw).map_err(sqlite_conversion_error)
}

fn scope_in_clause(
    column_name: &str,
    scopes: &[MemoryScope],
    params: &mut Vec<rusqlite::types::Value>,
) -> String {
    let mut clause = String::new();
    clause.push_str(column_name);
    clause.push_str(" IN (");
    for (index, scope) in scopes.iter().enumerate() {
        if index > 0 {
            clause.push_str(", ");
        }
        clause.push('?');
        clause.push_str(&(params.len() + 1).to_string());
        params.push(rusqlite::types::Value::from(
            scope_to_db(*scope).to_string(),
        ));
    }
    clause.push(')');
    clause
}

fn auto_promote_scored_memories(
    connection: &mut Connection,
    results: &mut [ScoredMemory],
    session_id: Option<&str>,
    scope_config: &ScopeConfig,
) -> Result<(), StoreError> {
    if results.is_empty() {
        return Ok(());
    }

    if let Some(session_id) = session_id {
        validate_session_id(session_id)?;
        record_session_accesses(
            connection,
            results.iter().map(|result| result.memory.id),
            session_id,
        )?;
    }

    for result in results {
        let Some(to_scope) =
            promotion_target(connection, &result.memory, scope_config, session_id)?
        else {
            continue;
        };
        let transaction = connection.transaction()?;
        let Some(mut latest) = require_memory(&transaction, &result.memory.id)? else {
            continue;
        };
        record_promotion(
            &transaction,
            &mut latest,
            to_scope,
            "automatic promotion during search",
            "system:promotion",
            session_id,
            scope_config,
        )?;
        transaction.commit()?;
        result.memory = latest;
    }

    Ok(())
}

fn validate_session_id(session_id: &str) -> Result<(), StoreError> {
    Uuid::parse_str(session_id).map(|_| ()).map_err(|error| {
        StoreError::Validation(format!("invalid session_id `{session_id}`: {error}"))
    })
}

fn record_session_accesses(
    connection: &Connection,
    memory_ids: impl IntoIterator<Item = MemoryId>,
    session_id: &str,
) -> Result<(), StoreError> {
    let now = format_timestamp(Utc::now());
    for memory_id in memory_ids {
        connection.execute(
            r#"
            INSERT INTO memory_session_accesses(memory_id, session_id, first_accessed_at, last_accessed_at)
            VALUES (?1, ?2, ?3, ?3)
            ON CONFLICT(memory_id, session_id) DO UPDATE
            SET last_accessed_at = excluded.last_accessed_at
            "#,
            params![memory_id.to_string(), session_id, now],
        )?;
    }
    Ok(())
}

fn promotion_target(
    connection: &Connection,
    memory: &Memory,
    scope_config: &ScopeConfig,
    _trigger_session_id: Option<&str>,
) -> Result<Option<MemoryScope>, StoreError> {
    let Some(next_scope) = memory.scope.next() else {
        return Ok(None);
    };

    if memory.scope == MemoryScope::Session {
        let distinct_sessions = connection.query_row(
            "SELECT COUNT(DISTINCT session_id) FROM memory_session_accesses WHERE memory_id = ?1",
            [memory.id.to_string()],
            |row| row.get::<_, i64>(0),
        )?;
        if distinct_sessions >= 3 {
            return Ok(Some(MemoryScope::Workspace));
        }
    }

    if memory.corroboration_count >= 2 {
        return Ok(Some(next_scope));
    }

    let age_days = (Utc::now() - memory.updated_at).num_days();
    if age_days >= 7
        && (memory.importance_score as f64) * decay::retention(memory, Utc::now(), scope_config)
            >= 0.4
    {
        return Ok(Some(next_scope));
    }

    Ok(None)
}

fn record_promotion(
    connection: &Connection,
    memory: &mut Memory,
    to_scope: MemoryScope,
    reason: &str,
    changed_by: &str,
    trigger_session_id: Option<&str>,
    _scope_config: &ScopeConfig,
) -> Result<(), StoreError> {
    let from_scope = memory.scope;
    if from_scope == to_scope {
        return Ok(());
    }

    let previous_memory = memory.clone();
    let now = Utc::now();
    let next_version = load_next_version_number(connection, &memory.id)?;
    connection.execute(
        r#"
        INSERT INTO memory_versions(id, memory_id, version_number, content, changed_at, changed_by, change_reason)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        params![
            Uuid::new_v4().to_string(),
            memory.id.to_string(),
            i64::from(next_version),
            memory.content.clone(),
            format_timestamp(now),
            changed_by,
            format!("scope promotion: {} -> {} ({reason})", scope_to_db(from_scope), scope_to_db(to_scope)),
        ],
    )?;
    connection.execute(
        r#"
        INSERT INTO memory_promotions(id, memory_id, from_scope, to_scope, reason, trigger_session_id, promoted_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        params![
            Uuid::new_v4().to_string(),
            memory.id.to_string(),
            scope_to_db(from_scope),
            scope_to_db(to_scope),
            reason,
            trigger_session_id,
            format_timestamp(now),
        ],
    )?;

    memory.scope = to_scope;
    memory.updated_at = now;
    persist_memory(connection, memory)?;
    let row_id = require_memory_rowid(connection, &memory.id)?;
    sync_fts_entry(connection, row_id, Some(&previous_memory), memory)?;
    Ok(())
}

fn matches_filter(memory: &Memory, filter: &MemoryFilter) -> bool {
    if let Some(memory_types) = &filter.memory_types {
        if !memory_types.contains(&memory.memory_type) {
            return false;
        }
    }

    if let Some(provenance_levels) = &filter.provenance_levels {
        if !provenance_levels.contains(&memory.provenance) {
            return false;
        }
    }

    if let Some(tags) = &filter.tags {
        if !tags
            .iter()
            .all(|tag| memory.tags.iter().any(|existing| existing == tag))
        {
            return false;
        }
    }

    true
}

fn load_next_version_number(connection: &Connection, id: &MemoryId) -> Result<u32, StoreError> {
    let max_version: Option<i64> = connection.query_row(
        "SELECT MAX(version_number) FROM memory_versions WHERE memory_id = ?1",
        [id.to_string()],
        |row| row.get(0),
    )?;

    let next = max_version.unwrap_or(0) + 1;
    u32::try_from(next).map_err(|_| {
        StoreError::Serialization(format!(
            "next version number `{next}` does not fit into u32"
        ))
    })
}

fn load_embedding_dimensions(connection: &Connection) -> Result<usize, StoreError> {
    let configured_value = connection
        .query_row(
            "SELECT value FROM scope_config WHERE key = 'embedding_dimensions'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    match configured_value {
        Some(value) => value.parse::<usize>().map_err(|error| {
            StoreError::Serialization(format!(
                "invalid embedding_dimensions config `{value}`: {error}"
            ))
        }),
        None => Ok(DEFAULT_EMBEDDING_DIMENSIONS),
    }
}

fn load_cached_embedding_blob(
    connection: &Connection,
    scope: MemoryScope,
    content_sha256: &str,
) -> Result<Option<Vec<u8>>, StoreError> {
    connection
        .query_row(
            r#"
            SELECT v.embedding
            FROM memory_embeddings me
            JOIN memories m ON m.id = me.memory_id
            JOIN vec_memories v ON v.rowid = me.vec_rowid
            WHERE me.content_sha256 = ?1
              AND m.scope = ?2
              AND m.embedding_stale = 0
            ORDER BY m.updated_at DESC, m.created_at DESC
            LIMIT 1
            "#,
            params![content_sha256, scope_to_db(scope)],
            |row| row.get(0),
        )
        .optional()
        .map_err(StoreError::from)
}

fn load_stored_embedding(
    connection: &Connection,
    id: &MemoryId,
    expected_dimensions: usize,
) -> Result<Option<Vec<f32>>, StoreError> {
    let encoded: Option<Vec<u8>> = connection
        .query_row(
            r#"
            SELECT v.embedding
            FROM memory_embeddings me
            JOIN vec_memories v ON v.rowid = me.vec_rowid
            WHERE me.memory_id = ?1
            "#,
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    encoded
        .map(|bytes| decode_embedding(&bytes, expected_dimensions))
        .transpose()
}

fn load_scope_config(connection: &Connection) -> Result<ScopeConfig, StoreError> {
    let defaults = ScopeConfig::default();
    Ok(ScopeConfig {
        similarity_weight: load_f32_config(
            connection,
            "similarity_weight",
            defaults.similarity_weight,
        )?,
        recency_weight: load_f32_config(connection, "recency_weight", defaults.recency_weight)?,
        access_weight: load_f32_config(connection, "access_weight", defaults.access_weight)?,
        priority_weight: load_f32_config(connection, "priority_weight", defaults.priority_weight)?,
        memory_context_ratio: load_f32_config(
            connection,
            "memory_context_ratio",
            defaults.memory_context_ratio,
        )?,
        decay_lambda_base: load_f32_config(
            connection,
            "decay_lambda_base",
            defaults.decay_lambda_base,
        )?,
        response_reserve: load_u32_config(
            connection,
            "response_reserve",
            defaults.response_reserve,
        )?,
        salience_threshold: load_f32_config(
            connection,
            "salience_threshold",
            defaults.salience_threshold,
        )?,
        novelty_doubt_threshold: load_f32_config(
            connection,
            "novelty_doubt_threshold",
            defaults.novelty_doubt_threshold,
        )?,
        merge_similarity_threshold: load_f32_config(
            connection,
            "merge_similarity_threshold",
            defaults.merge_similarity_threshold,
        )?,
        duplicate_similarity_threshold: load_f32_config(
            connection,
            "duplicate_similarity_threshold",
            defaults.duplicate_similarity_threshold,
        )?,
        agent_inferred_importance_threshold: load_f32_config(
            connection,
            "agent_inferred_importance_threshold",
            defaults.agent_inferred_importance_threshold,
        )?,
    })
}

fn load_f32_config(connection: &Connection, key: &str, default: f32) -> Result<f32, StoreError> {
    let raw_value = load_config_value(connection, key)?;
    match raw_value {
        Some(raw_value) => raw_value.parse::<f32>().map_err(|error| {
            StoreError::Serialization(format!("invalid {key} config `{raw_value}`: {error}"))
        }),
        None => Ok(default),
    }
}

fn load_u32_config(connection: &Connection, key: &str, default: u32) -> Result<u32, StoreError> {
    let raw_value = load_config_value(connection, key)?;
    match raw_value {
        Some(raw_value) => raw_value.parse::<u32>().map_err(|error| {
            StoreError::Serialization(format!("invalid {key} config `{raw_value}`: {error}"))
        }),
        None => Ok(default),
    }
}

fn load_config_value(connection: &Connection, key: &str) -> Result<Option<String>, StoreError> {
    connection
        .query_row(
            "SELECT value FROM scope_config WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(StoreError::from)
}

fn encode_embedding(embedding: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(embedding));
    for component in embedding {
        bytes.extend_from_slice(&component.to_le_bytes());
    }
    bytes
}

fn content_sha256(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";

    for byte in digest {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }

    encoded
}

fn upsert_encoded_embedding(
    connection: &Connection,
    id: &MemoryId,
    encoded_embedding: &[u8],
    content_sha256: &str,
) -> Result<(), StoreError> {
    let existing_vec_rowid: Option<i64> = connection
        .query_row(
            "SELECT vec_rowid FROM memory_embeddings WHERE memory_id = ?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()?;

    match existing_vec_rowid {
        Some(vec_rowid) => {
            connection.execute(
                "UPDATE vec_memories SET embedding = ?1 WHERE rowid = ?2",
                params![encoded_embedding, vec_rowid],
            )?;
            connection.execute(
                "UPDATE memory_embeddings SET content_sha256 = ?1 WHERE memory_id = ?2",
                params![content_sha256, id.to_string()],
            )?;
        }
        None => {
            connection.execute(
                "INSERT INTO vec_memories(embedding) VALUES (?1)",
                params![encoded_embedding],
            )?;
            let vec_rowid = connection.last_insert_rowid();
            connection.execute(
                "INSERT INTO memory_embeddings(memory_id, vec_rowid, content_sha256) VALUES (?1, ?2, ?3)",
                params![id.to_string(), vec_rowid, content_sha256],
            )?;
        }
    }

    connection.execute(
        "UPDATE memories SET embedding_stale = 0 WHERE id = ?1",
        [id.to_string()],
    )?;

    Ok(())
}

fn decode_embedding(bytes: &[u8], expected_dimensions: usize) -> Result<Vec<f32>, StoreError> {
    if bytes.len() % std::mem::size_of::<f32>() != 0 {
        return Err(StoreError::Serialization(format!(
            "embedding blob length {} is not aligned to f32 components",
            bytes.len()
        )));
    }

    let actual_dimensions = bytes.len() / std::mem::size_of::<f32>();
    if actual_dimensions != expected_dimensions {
        return Err(StoreError::Serialization(format!(
            "stored embedding dimension mismatch: expected {expected_dimensions}, got {actual_dimensions}"
        )));
    }

    let mut embedding = Vec::with_capacity(actual_dimensions);
    for chunk in bytes.chunks_exact(std::mem::size_of::<f32>()) {
        embedding.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }

    Ok(embedding)
}

fn validate_query_embedding(
    embedding: &[f32],
    expected_dimensions: usize,
) -> Result<(), StoreError> {
    if embedding.is_empty() {
        return Err(StoreError::Validation(
            "embedding vector must not be empty".to_string(),
        ));
    }

    if embedding.len() != expected_dimensions {
        return Err(StoreError::Validation(format!(
            "embedding dimension mismatch: expected {expected_dimensions}, got {}",
            embedding.len()
        )));
    }

    Ok(())
}

fn validate_similarity_threshold(threshold: f32) -> Result<(), StoreError> {
    if threshold.is_finite() && (0.0..=1.0).contains(&threshold) {
        return Ok(());
    }

    Err(StoreError::Validation(
        "similarity threshold must be a finite value in the inclusive range 0.0..=1.0".to_string(),
    ))
}

fn load_search_memories(
    connection: &Connection,
    scopes: &[MemoryScope],
    state: MemoryState,
    type_filter: Option<&[MemoryType]>,
) -> Result<Vec<Memory>, StoreError> {
    if scopes.is_empty() {
        return Ok(Vec::new());
    }
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    let scope_clause = scope_in_clause("scope", scopes, &mut params);
    let mut sql = format!(
        "SELECT {MEMORY_SELECT_COLUMNS} FROM memories WHERE {scope_clause} AND state = ?{}",
        params.len() + 1
    );
    params.push(rusqlite::types::Value::from(state_to_db(state).to_string()));

    if let Some(type_filter) = type_filter {
        if type_filter.is_empty() {
            return Ok(Vec::new());
        }

        sql.push_str(" AND memory_type IN (");
        for (index, memory_type) in type_filter.iter().enumerate() {
            if index > 0 {
                sql.push_str(", ");
            }
            sql.push('?');
            sql.push_str(&(params.len() + 1).to_string());
            params.push(rusqlite::types::Value::from(
                memory_type_to_db(*memory_type).to_string(),
            ));
        }
        sql.push(')');
    }

    sql.push_str(" ORDER BY updated_at DESC, rowid DESC");

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(params), map_memory_row)?;
    let mut memories = Vec::new();
    for row in rows {
        memories.push(row?);
    }
    Ok(memories)
}

fn load_keyword_scores(
    connection: &Connection,
    scopes: &[MemoryScope],
    state: MemoryState,
    type_filter: Option<&[MemoryType]>,
    text: &str,
) -> Result<HashMap<MemoryId, f32>, StoreError> {
    let Some(fts_query) = build_fts_query(text) else {
        return Ok(HashMap::new());
    };
    if scopes.is_empty() {
        return Ok(HashMap::new());
    }

    let mut params: Vec<rusqlite::types::Value> = vec![rusqlite::types::Value::from(fts_query)];
    let scope_clause = scope_in_clause("m.scope", scopes, &mut params);
    let mut sql = format!(
        r#"
        SELECT m.id, bm25(memories_fts) AS bm25_score
        FROM memories_fts
        JOIN memories m ON m.rowid = memories_fts.rowid
        WHERE memories_fts MATCH ?1
          AND {scope_clause}
          AND m.state = ?{}
        "#,
        params.len() + 1
    );
    params.push(rusqlite::types::Value::from(state_to_db(state).to_string()));

    if let Some(type_filter) = type_filter {
        if type_filter.is_empty() {
            return Ok(HashMap::new());
        }

        sql.push_str(" AND m.memory_type IN (");
        for (index, memory_type) in type_filter.iter().enumerate() {
            if index > 0 {
                sql.push_str(", ");
            }
            sql.push('?');
            sql.push_str(&(params.len() + 1).to_string());
            params.push(rusqlite::types::Value::from(
                memory_type_to_db(*memory_type).to_string(),
            ));
        }
        sql.push(')');
    }

    sql.push_str(" ORDER BY bm25_score ASC, m.updated_at DESC");

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(params), |row| {
        let raw_id: String = row.get(0)?;
        Ok((parse_uuid_for_sqlite(&raw_id)?, row.get::<_, f64>(1)?))
    })?;

    let mut raw_matches = Vec::new();
    for row in rows {
        raw_matches.push(row?);
    }
    if raw_matches.is_empty() {
        return Ok(HashMap::new());
    }

    let best_score = raw_matches
        .iter()
        .map(|(_, score)| *score)
        .fold(f64::INFINITY, f64::min);
    let worst_score = raw_matches
        .iter()
        .map(|(_, score)| *score)
        .fold(f64::NEG_INFINITY, f64::max);

    let mut normalized_scores = HashMap::with_capacity(raw_matches.len());
    for (id, raw_score) in raw_matches {
        let normalized = if (worst_score - best_score).abs() < f64::EPSILON {
            1.0
        } else {
            ((worst_score - raw_score) / (worst_score - best_score)).clamp(0.0, 1.0)
        } as f32;
        normalized_scores.insert(id, normalized);
    }

    Ok(normalized_scores)
}

fn build_fts_query(text: &str) -> Option<String> {
    let terms = text
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>();

    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" AND "))
    }
}

fn load_vector_similarity_scores(
    connection: &Connection,
    scopes: &[MemoryScope],
    state: MemoryState,
    type_filter: Option<&[MemoryType]>,
    query_embedding: &[f32],
    threshold: f32,
) -> Result<HashMap<MemoryId, f32>, StoreError> {
    let expected_dimensions = load_embedding_dimensions(connection)?;
    validate_query_embedding(query_embedding, expected_dimensions)?;
    if scopes.is_empty() {
        return Ok(HashMap::new());
    }

    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    let scope_clause = scope_in_clause("m.scope", scopes, &mut params);
    let mut sql = format!(
        r#"
        SELECT m.id, v.embedding
        FROM memories m
        JOIN memory_embeddings me ON me.memory_id = m.id
        JOIN vec_memories v ON v.rowid = me.vec_rowid
        WHERE {scope_clause}
          AND m.state = ?{}
        "#,
        params.len() + 1
    );
    params.push(rusqlite::types::Value::from(state_to_db(state).to_string()));

    if let Some(type_filter) = type_filter {
        if type_filter.is_empty() {
            return Ok(HashMap::new());
        }

        sql.push_str(" AND m.memory_type IN (");
        for (index, memory_type) in type_filter.iter().enumerate() {
            if index > 0 {
                sql.push_str(", ");
            }
            sql.push('?');
            sql.push_str(&(params.len() + 1).to_string());
            params.push(rusqlite::types::Value::from(
                memory_type_to_db(*memory_type).to_string(),
            ));
        }
        sql.push(')');
    }

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(params), |row| {
        let raw_id: String = row.get(0)?;
        Ok((parse_uuid_for_sqlite(&raw_id)?, row.get::<_, Vec<u8>>(1)?))
    })?;

    let mut similarity_scores = HashMap::new();
    for row in rows {
        let (id, encoded_embedding) = row?;
        let stored_embedding = decode_embedding(&encoded_embedding, expected_dimensions)?;
        let similarity = cosine_similarity(query_embedding, &stored_embedding)?;
        if similarity >= threshold && similarity > 0.0 {
            similarity_scores.insert(id, similarity);
        }
    }

    Ok(similarity_scores)
}

fn combine_similarity_signals(
    vector_similarity: Option<f32>,
    keyword_similarity: Option<f32>,
) -> f32 {
    match (vector_similarity, keyword_similarity) {
        (Some(vector_similarity), Some(keyword_similarity)) => {
            (VECTOR_SIMILARITY_BLEND_WEIGHT * vector_similarity)
                + (KEYWORD_SIMILARITY_BLEND_WEIGHT * keyword_similarity)
        }
        (Some(vector_similarity), None) => vector_similarity,
        (None, Some(keyword_similarity)) => keyword_similarity,
        (None, None) => 0.0,
    }
}

fn compute_retrieval_score(
    memory: &Memory,
    similarity: f32,
    scope_config: &ScopeConfig,
    now: DateTime<Utc>,
) -> f32 {
    let recency = decay::retention(memory, now, scope_config) as f32;
    let access = ((memory.access_count as f32) + 1.0).ln();
    let priority = memory.importance_score * memory.reliability_score;
    let similarity_gated_priority = similarity * priority;

    (scope_config.similarity_weight * similarity)
        + (scope_config.recency_weight * recency)
        + (scope_config.access_weight * access)
        + (scope_config.priority_weight * similarity_gated_priority)
}

fn trim_results_to_context_budget(
    results: Vec<ScoredMemory>,
    context_config: Option<&MemoryContextConfig>,
    scope_config: &ScopeConfig,
) -> Vec<ScoredMemory> {
    let Some(context_config) = context_config else {
        return results;
    };

    let memory_context_ratio = if context_config.memory_context_ratio.is_finite() {
        context_config.memory_context_ratio.clamp(0.0, 1.0)
    } else {
        scope_config.memory_context_ratio
    };
    let response_reserve = context_config
        .response_reserve
        .unwrap_or(scope_config.response_reserve);
    let available_for_memory = context_config.model_max_tokens.saturating_sub(
        context_config
            .effective_already_used_tokens()
            .saturating_add(response_reserve),
    );
    let max_memory_tokens = ((available_for_memory as f32) * memory_context_ratio).floor() as u32;

    if max_memory_tokens == 0 {
        return Vec::new();
    }

    let mut retained = Vec::new();
    let mut used_tokens = 0_u32;
    for result in results {
        let estimated_tokens = estimate_memory_tokens(&result.memory);
        if retained.is_empty() || used_tokens.saturating_add(estimated_tokens) <= max_memory_tokens
        {
            used_tokens = used_tokens.saturating_add(estimated_tokens);
            retained.push(result);
        } else {
            break;
        }
    }

    retained
}

fn estimate_memory_tokens(memory: &Memory) -> u32 {
    let tags_length = if memory.tags.is_empty() {
        0
    } else {
        memory.tags.iter().map(String::len).sum::<usize>() + memory.tags.len().saturating_sub(1)
    };
    let raw_characters = memory.content.len()
        + memory.summary.as_ref().map_or(0, String::len)
        + memory.status.as_ref().map_or(0, String::len)
        + tags_length;

    let content_tokens = raw_characters.div_ceil(ESTIMATED_CHARS_PER_TOKEN) as u32;
    content_tokens.saturating_add(BASE_MEMORY_TOKEN_OVERHEAD)
}

fn touch_scored_memories(
    connection: &mut Connection,
    results: &mut [ScoredMemory],
) -> Result<(), StoreError> {
    if results.is_empty() {
        return Ok(());
    }

    let now = Utc::now();
    let transaction = connection.transaction()?;
    for result in results {
        result.memory.access_count = result
            .memory
            .access_count
            .checked_add(1)
            .ok_or(StoreError::Validation("access_count overflow".to_string()))?;
        result.memory.last_accessed_at = Some(now);

        transaction.execute(
            r#"
            UPDATE memories
            SET access_count = ?2,
                last_accessed_at = ?3
            WHERE id = ?1
            "#,
            params![
                result.memory.id.to_string(),
                i64::from(result.memory.access_count),
                format_timestamp(now),
            ],
        )?;
    }
    transaction.commit()?;

    Ok(())
}

fn count_memories_by_state(
    connection: &Connection,
    scope: MemoryScope,
    state: MemoryState,
) -> Result<i64, StoreError> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM memories WHERE scope = ?1 AND state = ?2",
            params![scope_to_db(scope), state_to_db(state)],
            |row| row.get::<_, i64>(0),
        )
        .map_err(StoreError::from)
}

fn compute_budget_usage_ratio(
    connection: &Connection,
    scope: MemoryScope,
    active_count: i64,
) -> Result<f32, StoreError> {
    let configured_budget = connection
        .query_row(
            "SELECT value FROM scope_config WHERE key = 'budget_active_max'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    let budget = match configured_budget {
        Some(value) => value.parse::<f32>().map_err(|error| {
            StoreError::Serialization(format!(
                "invalid budget_active_max config `{value}`: {error}"
            ))
        })?,
        None => match scope {
            MemoryScope::Workspace => DEFAULT_WORKSPACE_BUDGET,
            MemoryScope::User => DEFAULT_USER_BUDGET,
            MemoryScope::Agent => DEFAULT_AGENT_BUDGET,
            MemoryScope::Session => 0.0,
        },
    };

    if budget <= 0.0 {
        return Ok(0.0);
    }

    Ok((active_count as f32) / budget)
}

fn lower_reliability(
    connection: &Connection,
    memory: &Memory,
    now: DateTime<Utc>,
) -> Result<(), StoreError> {
    let adjusted_reliability = (memory.reliability_score - 0.3).max(0.0);
    connection.execute(
        "UPDATE memories SET reliability_score = ?2, updated_at = ?3 WHERE id = ?1",
        params![
            memory.id.to_string(),
            f64::from(adjusted_reliability),
            format_timestamp(now),
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        env, fs,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use chrono::Utc;
    use rusqlite::params;
    use tokio;
    use uuid::Uuid;

    use super::{expand_compound_words, split_compound_word, SqliteMemoryStore};
    use crate::{
        EmbeddingError, EmbeddingProvider, Memory, MemoryFilter, MemoryScope, MemoryState,
        MemoryStore, MemoryType, MetadataUpdate, OptionalFieldUpdate, ProvenanceLevel,
        ResolutionStatus, SearchQuery, SensitivityLevel,
    };

    #[derive(Debug, Clone)]
    enum StubEmbeddingResponse {
        Embedding(Vec<f32>),
        Failure(String),
    }

    #[derive(Debug)]
    struct StubEmbeddingProvider {
        responses: HashMap<String, StubEmbeddingResponse>,
        calls: Mutex<Vec<String>>,
    }

    impl StubEmbeddingProvider {
        fn new<I, S>(responses: I) -> Self
        where
            I: IntoIterator<Item = (S, StubEmbeddingResponse)>,
            S: Into<String>,
        {
            Self {
                responses: responses
                    .into_iter()
                    .map(|(text, response)| (text.into(), response))
                    .collect(),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("stub provider calls lock").clone()
        }

        fn call_count(&self) -> usize {
            self.calls.lock().expect("stub provider calls lock").len()
        }
    }

    #[async_trait]
    impl EmbeddingProvider for StubEmbeddingProvider {
        async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
            let trimmed = text.trim().to_string();
            self.calls
                .lock()
                .expect("stub provider calls lock")
                .push(trimmed.clone());

            match self.responses.get(&trimmed) {
                Some(StubEmbeddingResponse::Embedding(embedding)) => Ok(embedding.clone()),
                Some(StubEmbeddingResponse::Failure(message)) => {
                    Err(EmbeddingError::Provider(message.clone()))
                }
                None => Err(EmbeddingError::Provider(format!(
                    "missing stub embedding for `{trimmed}`"
                ))),
            }
        }

        fn dimensions(&self) -> usize {
            768
        }

        fn model_id(&self) -> &str {
            "stub-embedding-provider"
        }
    }

    #[test]
    fn split_compound_word_handles_camel_case_and_acronym_boundaries() {
        assert_eq!(
            split_compound_word("JavaScript").as_deref(),
            Some("Java Script")
        );
        assert_eq!(
            split_compound_word("ProtonVPN").as_deref(),
            Some("Proton VPN")
        );
        assert_eq!(
            split_compound_word("XMLParser").as_deref(),
            Some("XML Parser")
        );
        assert_eq!(split_compound_word("VPN"), None);
    }

    #[test]
    fn expand_compound_words_preserves_original_text_and_appends_split_forms() {
        assert_eq!(
            expand_compound_words("ProtonVPN avec WireGuard et JavaScript"),
            "ProtonVPN avec WireGuard et JavaScript Proton VPN Wire Guard Java Script"
        );
    }

    #[tokio::test]
    async fn store_and_get_round_trip_updates_access_tracking() {
        let fixture = test_fixture();
        let memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let id = memory.id;

        fixture
            .store
            .store(memory.clone())
            .await
            .expect("store memory");

        let raw = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get raw memory")
            .expect("memory exists");
        assert_eq!(raw.content, memory.content);
        assert_eq!(raw.access_count, 0);
        assert!(raw.last_accessed_at.is_none());

        let fetched = fixture
            .store
            .get(&id)
            .await
            .expect("get memory")
            .expect("memory exists");
        assert_eq!(fetched.access_count, 1);
        assert!(fetched.last_accessed_at.is_some());

        let persisted = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get raw persisted memory")
            .expect("memory exists");
        assert_eq!(persisted.access_count, 1);
        assert!(persisted.last_accessed_at.is_some());
    }

    #[tokio::test]
    async fn update_content_creates_version_and_marks_embedding_stale() {
        let fixture = test_fixture();
        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        memory.embedding_stale = false;
        let id = memory.id;
        let original_updated_at = memory.updated_at;

        fixture
            .store
            .store(memory.clone())
            .await
            .expect("store memory");

        fixture
            .store
            .update_content(&id, "updated content", "agent:test", "manual enrichment")
            .await
            .expect("update content");

        let updated = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get updated memory")
            .expect("memory exists");
        assert_eq!(updated.content, "updated content");
        assert!(updated.embedding_stale);
        assert!(updated.updated_at >= original_updated_at);

        let version_row = fixture
            .store
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT version_number, content, changed_by, change_reason FROM memory_versions WHERE memory_id = ?1",
                    [id.to_string()],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, Option<String>>(3)?,
                        ))
                    },
                )
                .map_err(crate::StoreError::from)
            })
            .expect("load version row");

        assert_eq!(version_row.0, 1);
        assert_eq!(version_row.1, memory.content);
        assert_eq!(version_row.2, "agent:test");
        assert_eq!(version_row.3.as_deref(), Some("manual enrichment"));
    }

    #[tokio::test]
    async fn lifecycle_and_hard_delete_remove_related_rows() {
        let fixture = test_fixture();
        let memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let id = memory.id;

        fixture.store.store(memory).await.expect("store memory");
        fixture
            .store
            .store_embedding(&id, &[0.5; 768])
            .await
            .expect("store embedding");

        fixture.store.make_dormant(&id).await.expect("make dormant");
        let dormant = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get dormant memory")
            .expect("memory exists");
        assert_eq!(dormant.state, MemoryState::Dormant);

        fixture.store.reactivate(&id).await.expect("reactivate");
        let active = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get active memory")
            .expect("memory exists");
        assert_eq!(active.state, MemoryState::Active);

        fixture.store.hard_delete(&id).await.expect("hard delete");
        assert!(fixture
            .store
            .get_raw(&id)
            .await
            .expect("get deleted memory")
            .is_none());

        let (embedding_rows, vector_rows) = fixture
            .store
            .with_connection(|connection| {
                let embedding_rows = connection.query_row(
                    "SELECT COUNT(*) FROM memory_embeddings WHERE memory_id = ?1",
                    [id.to_string()],
                    |row| row.get::<_, i64>(0),
                )?;
                let vector_rows =
                    connection.query_row("SELECT COUNT(*) FROM vec_memories", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                Ok((embedding_rows, vector_rows))
            })
            .expect("load cascade counts");

        assert_eq!(embedding_rows, 0);
        assert_eq!(vector_rows, 0);
    }

    #[tokio::test]
    async fn list_applies_filters_and_metadata_updates() {
        let fixture = test_fixture();
        let memory_a = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let id_a = memory_a.id;
        let mut memory_b = sample_memory(MemoryScope::Workspace, ProvenanceLevel::Imported);
        memory_b.memory_type = MemoryType::Decision;
        memory_b.tags = vec!["project".to_string(), "shipping".to_string()];
        let id_b = memory_b.id;

        fixture.store.store(memory_a).await.expect("store memory a");
        fixture.store.store(memory_b).await.expect("store memory b");

        fixture
            .store
            .update_metadata(
                &id_a,
                MetadataUpdate {
                    tags: Some(vec!["project".to_string(), "important".to_string()]),
                    status: Some(OptionalFieldUpdate::Set("open".to_string())),
                    custom_metadata: Some(HashMap::from([(
                        "source".to_string(),
                        "notes".to_string(),
                    )])),
                    importance_score: Some(0.9),
                    reliability_score: Some(0.95),
                    state: Some(MemoryState::Dormant),
                },
            )
            .await
            .expect("update metadata");

        let dormant = fixture
            .store
            .list(MemoryFilter {
                state: Some(MemoryState::Dormant),
                tags: Some(vec!["project".to_string(), "important".to_string()]),
                status: Some("open".to_string()),
                limit: Some(5),
                ..MemoryFilter::default()
            })
            .await
            .expect("list dormant memories");
        assert_eq!(dormant.len(), 1);
        assert_eq!(dormant[0].id, id_a);
        assert_eq!(
            dormant[0].custom_metadata.get("source").map(String::as_str),
            Some("notes")
        );

        let active_decisions = fixture
            .store
            .list(MemoryFilter {
                state: Some(MemoryState::Active),
                memory_types: Some(vec![MemoryType::Decision]),
                tags: Some(vec!["project".to_string()]),
                limit: Some(5),
                ..MemoryFilter::default()
            })
            .await
            .expect("list active decision memories");
        assert_eq!(active_decisions.len(), 1);
        assert_eq!(active_decisions[0].id, id_b);
    }

    #[tokio::test]
    async fn health_report_and_contradictions_reflect_store_state() {
        let fixture = test_fixture();
        let trusted = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let trusted_id = trusted.id;
        let mut less_trusted =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::AgentInferred);
        less_trusted.reliability_score = 0.7;
        less_trusted.embedding_stale = true;
        let less_trusted_id = less_trusted.id;

        fixture
            .store
            .store(trusted)
            .await
            .expect("store trusted memory");
        fixture
            .store
            .store(less_trusted)
            .await
            .expect("store less trusted memory");
        fixture
            .store
            .store_embedding(&trusted_id, &[0.25; 768])
            .await
            .expect("store trusted embedding");
        fixture
            .store
            .make_dormant(&less_trusted_id)
            .await
            .expect("make less trusted memory dormant");
        fixture
            .store
            .record_contradiction(&trusted_id, &less_trusted_id, "conflicting delivery date")
            .await
            .expect("record contradiction");

        let report = fixture.store.health_report().await.expect("health report");
        assert_eq!(report.scope, MemoryScope::Workspace);
        assert_eq!(report.active_count, 1);
        assert_eq!(report.dormant_count, 1);
        assert_eq!(report.unresolved_contradictions, 1);
        assert_eq!(report.stale_embeddings_count, 1);
        assert!(report.total_storage_bytes > 0);
        assert!(report.budget_usage_ratio > 0.0);
        assert!(report.newest_memory.is_some());

        let contradictions = fixture
            .store
            .list_contradictions(Some(ResolutionStatus::Unresolved))
            .await
            .expect("list contradictions");
        assert_eq!(contradictions.len(), 1);
        assert_eq!(contradictions[0].memory_a_id, trusted_id);
        assert_eq!(contradictions[0].memory_b_id, less_trusted_id);

        let downgraded = fixture
            .store
            .get_raw(&less_trusted_id)
            .await
            .expect("get downgraded memory")
            .expect("memory exists");
        assert!((downgraded.reliability_score - 0.4).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn search_uses_keyword_fts_and_updates_access_tracking() {
        let fixture = test_fixture();

        let mut active_match = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        active_match.content = "Apollo launch checklist and mission notes".to_string();
        active_match.tags = vec!["apollo".to_string(), "launch".to_string()];
        let active_match_id = active_match.id;

        let mut dormant_match = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        dormant_match.content = "Apollo archive notes".to_string();
        dormant_match.state = MemoryState::Dormant;

        let mut active_miss = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        active_miss.content = "Garden irrigation instructions".to_string();

        fixture
            .store
            .store(active_match)
            .await
            .expect("store active keyword match");
        fixture
            .store
            .store(dormant_match)
            .await
            .expect("store dormant keyword match");
        fixture
            .store
            .store(active_miss)
            .await
            .expect("store active non-match");

        let results = fixture
            .store
            .search(SearchQuery {
                text: "apollo launch".to_string(),
                embedding: None,
                scope: MemoryScope::Workspace,
                state_filter: None,
                type_filter: None,
                max_results: 5,
                context_config: None,
                session_id: None,
            })
            .await
            .expect("run keyword search");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memory.id, active_match_id);
        assert!(results[0].similarity > 0.0);
        assert_eq!(results[0].memory.access_count, 1);
        assert!(results[0].memory.last_accessed_at.is_some());
    }

    #[tokio::test]
    async fn find_similar_returns_active_embedding_matches_without_touching_access() {
        let fixture = test_fixture();

        let active_match = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let active_match_id = active_match.id;
        let mut dormant_match = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        dormant_match.state = MemoryState::Dormant;
        let dormant_match_id = dormant_match.id;
        let active_far = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let active_far_id = active_far.id;

        fixture
            .store
            .store(active_match)
            .await
            .expect("store active vector match");
        fixture
            .store
            .store(dormant_match)
            .await
            .expect("store dormant vector match");
        fixture
            .store
            .store(active_far)
            .await
            .expect("store active far vector");

        fixture
            .store
            .store_embedding(&active_match_id, &[1.0; 768])
            .await
            .expect("store active embedding");
        fixture
            .store
            .store_embedding(&dormant_match_id, &[1.0; 768])
            .await
            .expect("store dormant embedding");
        fixture
            .store
            .store_embedding(&active_far_id, &[0.0; 768])
            .await
            .expect("store far embedding");

        let results = fixture
            .store
            .find_similar(&[1.0; 768], 0.95, 5)
            .await
            .expect("find similar");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memory.id, active_match_id);
        assert!((results[0].similarity - 1.0).abs() < f32::EPSILON);

        let persisted = fixture
            .store
            .get_raw(&active_match_id)
            .await
            .expect("reload active match")
            .expect("active match exists");
        assert_eq!(persisted.access_count, 0);
        assert!(persisted.last_accessed_at.is_none());
    }

    #[tokio::test]
    async fn search_combines_semantic_and_priority_signals_for_ordering() {
        let fixture = test_fixture();

        let mut keyword_only = sample_memory(MemoryScope::Workspace, ProvenanceLevel::Imported);
        keyword_only.content = "release checklist for apollo deployment".to_string();
        keyword_only.importance_score = 0.2;
        keyword_only.reliability_score = 0.2;
        let keyword_only_id = keyword_only.id;

        let mut semantic_priority =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        semantic_priority.content = "deployment runbook for launch window".to_string();
        semantic_priority.importance_score = 1.0;
        semantic_priority.reliability_score = 1.0;
        semantic_priority.access_count = 5;
        let semantic_priority_id = semantic_priority.id;

        fixture
            .store
            .store(keyword_only)
            .await
            .expect("store keyword-only memory");
        fixture
            .store
            .store(semantic_priority)
            .await
            .expect("store semantic-priority memory");

        let mut weak_embedding = vec![1.0_f32; 384];
        weak_embedding.extend(vec![-1.0_f32; 384]);
        fixture
            .store
            .store_embedding(&keyword_only_id, &weak_embedding)
            .await
            .expect("store weak semantic embedding");
        fixture
            .store
            .store_embedding(&semantic_priority_id, &[1.0; 768])
            .await
            .expect("store strong semantic embedding");

        fixture
            .store
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE memories SET access_count = 5, last_accessed_at = ?2 WHERE id = ?1",
                    [
                        semantic_priority_id.to_string(),
                        super::format_timestamp(Utc::now()),
                    ],
                )?;
                connection.execute(
                    "UPDATE memories SET access_count = 0, last_accessed_at = ?2 WHERE id = ?1",
                    [
                        keyword_only_id.to_string(),
                        super::format_timestamp(Utc::now()),
                    ],
                )?;
                Ok(())
            })
            .expect("seed deterministic access counts");

        let results = fixture
            .store
            .search(SearchQuery {
                text: "apollo deployment".to_string(),
                embedding: Some(vec![1.0; 768]),
                scope: MemoryScope::Workspace,
                state_filter: None,
                type_filter: None,
                max_results: 5,
                context_config: None,
                session_id: None,
            })
            .await
            .expect("run hybrid search");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].memory.id, semantic_priority_id);
        assert_eq!(results[1].memory.id, keyword_only_id);
        assert!(results[0].score > results[1].score);
        assert!(results[0].similarity >= results[1].similarity);
    }

    #[tokio::test]
    async fn search_prefers_higher_similarity_over_higher_importance() {
        let fixture = test_fixture();

        let mut higher_similarity =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        higher_similarity.content = "higher semantic match".to_string();
        higher_similarity.importance_score = 0.5;
        higher_similarity.reliability_score = 1.0;
        let higher_similarity_id = higher_similarity.id;

        let mut higher_importance =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        higher_importance.content = "lower semantic match".to_string();
        higher_importance.importance_score = 0.8;
        higher_importance.reliability_score = 1.0;
        let higher_importance_id = higher_importance.id;

        fixture
            .store
            .store(higher_similarity)
            .await
            .expect("store higher-similarity memory");
        fixture
            .store
            .store(higher_importance)
            .await
            .expect("store higher-importance memory");

        fixture
            .store
            .store_embedding(&higher_similarity_id, &embedding_with_similarity(0.9))
            .await
            .expect("store higher-similarity embedding");
        fixture
            .store
            .store_embedding(&higher_importance_id, &embedding_with_similarity(0.5))
            .await
            .expect("store higher-importance embedding");

        fixture
            .store
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE scope_config SET value = '0.0' WHERE key IN ('recency_weight', 'access_weight')",
                    [],
                )?;
                connection.execute(
                    "UPDATE memories SET access_count = 0, last_accessed_at = NULL WHERE id IN (?1, ?2)",
                    [higher_similarity_id.to_string(), higher_importance_id.to_string()],
                )?;
                Ok(())
            })
            .expect("isolate similarity-vs-priority scoring");

        let results = fixture
            .store
            .search(SearchQuery {
                text: String::new(),
                embedding: Some(query_embedding()),
                scope: MemoryScope::Workspace,
                state_filter: None,
                type_filter: None,
                max_results: 5,
                context_config: None,
                session_id: None,
            })
            .await
            .expect("run similarity-priority ordering search");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].memory.id, higher_similarity_id);
        assert_eq!(results[1].memory.id, higher_importance_id);
        assert!((results[0].similarity - 0.9).abs() < 1e-5);
        assert!((results[1].similarity - 0.5).abs() < 1e-5);
        assert!(results[0].score > results[1].score);
    }

    #[tokio::test]
    async fn search_uses_scope_configured_fixed_decay_for_recency_ordering() {
        let fixture = test_fixture();

        let mut recent = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        recent.content = "apollo recency note".to_string();
        let recent_id = recent.id;

        let mut stale = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        stale.content = "apollo recency note".to_string();
        let stale_id = stale.id;

        fixture
            .store
            .store(recent)
            .await
            .expect("store recent memory");
        fixture
            .store
            .store(stale)
            .await
            .expect("store stale memory");

        fixture
            .store
            .with_connection(|connection| {
                let now = Utc::now();
                let stale_time = now - chrono::Duration::days(10);

                connection.execute(
                    "UPDATE scope_config SET value = '0.0' WHERE key IN ('similarity_weight', 'access_weight', 'priority_weight')",
                    [],
                )?;
                connection.execute(
                    "UPDATE scope_config SET value = '1.0' WHERE key = 'recency_weight'",
                    [],
                )?;
                connection.execute(
                    "UPDATE scope_config SET value = '0.5' WHERE key = 'decay_lambda_base'",
                    [],
                )?;
                connection.execute(
                    "UPDATE memories SET last_accessed_at = ?2, updated_at = ?2, access_count = 0 WHERE id = ?1",
                    [recent_id.to_string(), super::format_timestamp(now)],
                )?;
                connection.execute(
                    "UPDATE memories SET last_accessed_at = ?2, updated_at = ?2, access_count = 0 WHERE id = ?1",
                    [stale_id.to_string(), super::format_timestamp(stale_time)],
                )?;
                Ok(())
            })
            .expect("seed recency ordering fixture");

        let results = fixture
            .store
            .search(SearchQuery {
                text: "apollo recency".to_string(),
                embedding: None,
                scope: MemoryScope::Workspace,
                state_filter: None,
                type_filter: None,
                max_results: 5,
                context_config: None,
                session_id: None,
            })
            .await
            .expect("run recency-focused search");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].memory.id, recent_id);
        assert_eq!(results[1].memory.id, stale_id);
        assert!(results[0].score > results[1].score);
    }

    #[tokio::test]
    async fn search_respects_type_and_state_filters() {
        let fixture = test_fixture();

        let mut active_decision =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        active_decision.content = "migration decision for apollo workspace".to_string();
        active_decision.memory_type = MemoryType::Decision;
        let active_decision_id = active_decision.id;

        let mut active_fact = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        active_fact.content = "apollo workspace fact sheet".to_string();
        active_fact.memory_type = MemoryType::Fact;

        let mut dormant_decision =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        dormant_decision.content = "old apollo decision archive".to_string();
        dormant_decision.memory_type = MemoryType::Decision;
        dormant_decision.state = MemoryState::Dormant;
        let dormant_decision_id = dormant_decision.id;

        fixture
            .store
            .store(active_decision)
            .await
            .expect("store active decision");
        fixture
            .store
            .store(active_fact)
            .await
            .expect("store active fact");
        fixture
            .store
            .store(dormant_decision)
            .await
            .expect("store dormant decision");

        let active_results = fixture
            .store
            .search(SearchQuery {
                text: "apollo decision".to_string(),
                embedding: None,
                scope: MemoryScope::Workspace,
                state_filter: None,
                type_filter: Some(vec![MemoryType::Decision]),
                max_results: 5,
                context_config: None,
                session_id: None,
            })
            .await
            .expect("run active decision search");
        assert_eq!(active_results.len(), 1);
        assert_eq!(active_results[0].memory.id, active_decision_id);

        let dormant_results = fixture
            .store
            .search(SearchQuery {
                text: "apollo decision".to_string(),
                embedding: None,
                scope: MemoryScope::Workspace,
                state_filter: Some(MemoryState::Dormant),
                type_filter: Some(vec![MemoryType::Decision]),
                max_results: 5,
                context_config: None,
                session_id: None,
            })
            .await
            .expect("run dormant decision search");
        assert_eq!(dormant_results.len(), 1);
        assert_eq!(dormant_results[0].memory.id, dormant_decision_id);
    }

    #[tokio::test]
    async fn store_with_embedding_provider_persists_embedding_automatically() {
        let memory_content = "provider-backed storage memory";
        let provider = Arc::new(StubEmbeddingProvider::new([(
            memory_content,
            StubEmbeddingResponse::Embedding(vec![1.0; 768]),
        )]));
        let fixture = test_fixture_with_provider(provider.clone());

        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        memory.content = memory_content.to_string();
        let id = memory.id;

        fixture
            .store
            .store(memory)
            .await
            .expect("store memory with automatic embedding");

        let persisted = fixture
            .store
            .get_raw(&id)
            .await
            .expect("reload stored memory")
            .expect("memory exists");
        assert!(!persisted.embedding_stale);

        let (embedding_rows, vector_rows) = fixture
            .store
            .with_connection(|connection| {
                let embedding_rows = connection.query_row(
                    "SELECT COUNT(*) FROM memory_embeddings WHERE memory_id = ?1",
                    [id.to_string()],
                    |row| row.get::<_, i64>(0),
                )?;
                let vector_rows =
                    connection.query_row("SELECT COUNT(*) FROM vec_memories", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                Ok((embedding_rows, vector_rows))
            })
            .expect("load automatic embedding counts");

        assert_eq!(embedding_rows, 1);
        assert_eq!(vector_rows, 1);
        assert_eq!(provider.calls(), vec![memory_content.to_string()]);
    }

    #[tokio::test]
    async fn store_with_duplicate_content_reuses_cached_embedding_without_reembedding() {
        let memory_content = "provider-backed cached storage memory";
        let provider = Arc::new(StubEmbeddingProvider::new([(
            memory_content,
            StubEmbeddingResponse::Embedding(vec![1.0; 768]),
        )]));
        let fixture = test_fixture_with_provider(provider.clone());

        let mut first = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        first.content = memory_content.to_string();
        let first_id = first.id;

        fixture
            .store
            .store(first)
            .await
            .expect("store first cached memory");

        let mut second = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        second.content = memory_content.to_string();
        let second_id = second.id;

        fixture
            .store
            .store(second)
            .await
            .expect("store second cached memory");

        let (embedding_rows, vector_rows, cached_hashes) = fixture
            .store
            .with_connection(|connection| {
                let embedding_rows =
                    connection.query_row("SELECT COUNT(*) FROM memory_embeddings", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                let vector_rows =
                    connection.query_row("SELECT COUNT(*) FROM vec_memories", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                let mut statement = connection.prepare(
                    "SELECT content_sha256 FROM memory_embeddings WHERE memory_id IN (?1, ?2) ORDER BY memory_id",
                )?;
                let hashes = statement
                    .query_map(params![first_id.to_string(), second_id.to_string()], |row| {
                        row.get::<_, Option<String>>(0)
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((embedding_rows, vector_rows, hashes))
            })
            .expect("load duplicate cached embedding state");

        assert_eq!(provider.call_count(), 1);
        assert_eq!(provider.calls(), vec![memory_content.to_string()]);
        assert_eq!(embedding_rows, 2);
        assert_eq!(vector_rows, 2);
        assert_eq!(cached_hashes.len(), 2);
        assert_eq!(cached_hashes[0], cached_hashes[1]);
        assert!(cached_hashes[0].is_some());
        assert!(
            !fixture
                .store
                .get_raw(&second_id)
                .await
                .expect("reload cached memory")
                .expect("cached memory exists")
                .embedding_stale
        );
    }

    #[tokio::test]
    async fn store_ignores_stale_cached_embeddings_for_changed_content() {
        let original_content = "cached content before update";
        let updated_content = "changed content after update";
        let provider = Arc::new(StubEmbeddingProvider::new([
            (
                original_content,
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
            (
                updated_content,
                StubEmbeddingResponse::Embedding(vec![0.5; 768]),
            ),
        ]));
        let fixture = test_fixture_with_provider(provider.clone());

        let mut first = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        first.content = original_content.to_string();
        let first_id = first.id;

        fixture
            .store
            .store(first)
            .await
            .expect("store original memory");
        fixture
            .store
            .update_content(&first_id, updated_content, "editor", "content changed")
            .await
            .expect("update original memory content");

        let mut second = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        second.content = original_content.to_string();

        fixture
            .store
            .store(second)
            .await
            .expect("store second memory with original content");

        assert_eq!(
            provider.calls(),
            vec![original_content.to_string(), original_content.to_string(),]
        );
    }

    #[tokio::test]
    async fn search_with_provider_auto_generates_query_embedding_when_missing() {
        let semantic_query = "semantic probe";
        let semantic_match_content = "release readiness checklist";
        let non_match_content = "garden watering schedule";
        let mut weak_embedding = vec![1.0_f32; 384];
        weak_embedding.extend(vec![-1.0_f32; 384]);
        let provider = Arc::new(StubEmbeddingProvider::new([
            (
                semantic_match_content,
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
            (
                non_match_content,
                StubEmbeddingResponse::Embedding(weak_embedding),
            ),
            (
                semantic_query,
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
        ]));
        let fixture = test_fixture_with_provider(provider.clone());

        let mut semantic_match = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        semantic_match.content = semantic_match_content.to_string();
        let semantic_match_id = semantic_match.id;

        let mut non_match = sample_memory(MemoryScope::Workspace, ProvenanceLevel::Imported);
        non_match.content = non_match_content.to_string();

        fixture
            .store
            .store(semantic_match)
            .await
            .expect("store semantic match");
        fixture
            .store
            .store(non_match)
            .await
            .expect("store semantic non-match");

        let results = fixture
            .store
            .search(SearchQuery {
                text: semantic_query.to_string(),
                embedding: None,
                scope: MemoryScope::Workspace,
                state_filter: None,
                type_filter: None,
                max_results: 5,
                context_config: None,
                session_id: None,
            })
            .await
            .expect("run provider-backed semantic search");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memory.id, semantic_match_id);
        assert!(results[0].similarity > 0.0);
        assert_eq!(
            provider.calls(),
            vec![
                semantic_match_content.to_string(),
                non_match_content.to_string(),
                semantic_query.to_string()
            ]
        );
    }

    #[tokio::test]
    async fn ollama_offline_store_keeps_memory_and_preserves_keyword_search_fallback() {
        let fallback_content = "apollo keyword fallback";
        let provider = Arc::new(StubEmbeddingProvider::new([(
            fallback_content,
            StubEmbeddingResponse::Failure(
                "ollama not reachable at http://127.0.0.1:11434: connection failed".to_string(),
            ),
        )]));
        let fixture = test_fixture_with_provider(provider.clone());

        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        memory.content = fallback_content.to_string();
        let id = memory.id;

        fixture
            .store
            .store(memory)
            .await
            .expect("store memory despite provider failure");

        let persisted = fixture
            .store
            .get_raw(&id)
            .await
            .expect("reload stored fallback memory")
            .expect("memory exists");
        assert!(persisted.embedding_stale);

        let (embedding_rows, vector_rows) = fixture
            .store
            .with_connection(|connection| {
                let embedding_rows = connection.query_row(
                    "SELECT COUNT(*) FROM memory_embeddings WHERE memory_id = ?1",
                    [id.to_string()],
                    |row| row.get::<_, i64>(0),
                )?;
                let vector_rows =
                    connection.query_row("SELECT COUNT(*) FROM vec_memories", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                Ok((embedding_rows, vector_rows))
            })
            .expect("load failed automatic embedding counts");

        assert_eq!(embedding_rows, 0);
        assert_eq!(vector_rows, 0);

        let results = fixture
            .store
            .search(SearchQuery {
                text: fallback_content.to_string(),
                embedding: None,
                scope: MemoryScope::Workspace,
                state_filter: None,
                type_filter: None,
                max_results: 5,
                context_config: None,
                session_id: None,
            })
            .await
            .expect("run keyword fallback search");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memory.id, id);
        assert!(results[0].similarity > 0.0);
        assert_eq!(
            provider.calls(),
            vec![fallback_content.to_string(), fallback_content.to_string()]
        );
    }

    #[test]
    fn ollama_offline_errors_map_to_user_facing_degradation_warning() {
        let warning = super::embedding_degradation_warning(&EmbeddingError::Provider(
            "ollama not reachable at http://127.0.0.1:11434: request timed out after 30s"
                .to_string(),
        ))
        .expect("offline provider errors should produce a degradation warning");

        assert_eq!(
            warning,
            "Ollama not reachable at http://127.0.0.1:11434, storing without embeddings. Run reembed later."
        );

        let non_offline_warning = super::embedding_degradation_warning(&EmbeddingError::Provider(
            "ollama embeddings request returned 500 Internal Server Error: boom".to_string(),
        ));
        assert!(non_offline_warning.is_none());
    }

    #[test]
    fn openai_offline_errors_map_to_user_facing_degradation_warning() {
        let warning = super::embedding_degradation_warning(&EmbeddingError::Provider(
            "openai not reachable at https://api.openai.com: request timed out after 30s"
                .to_string(),
        ))
        .expect("offline openai errors should produce a degradation warning");

        assert_eq!(
            warning,
            "OpenAI not reachable at https://api.openai.com, storing without embeddings. Run reembed later."
        );

        let invalid_api_key_warning =
            super::embedding_degradation_warning(&EmbeddingError::Provider(
                "openai returned 401 Unauthorized: invalid API key (...)".to_string(),
            ))
            .expect("openai auth errors should produce a degradation warning");

        assert_eq!(
            invalid_api_key_warning,
            "OpenAI embeddings unavailable (401 Unauthorized: invalid API key), storing without embeddings. Run reembed later."
        );
    }

    #[test]
    fn openai_http_errors_map_to_user_facing_degradation_warning() {
        let rate_limit_warning = super::embedding_degradation_warning(&EmbeddingError::Provider(
            "openai returned 429 Too Many Requests: rate limited, try again later (...)"
                .to_string(),
        ))
        .expect("rate limit errors should produce a degradation warning");

        assert_eq!(
            rate_limit_warning,
            "OpenAI embeddings unavailable (429 Too Many Requests: rate limited, try again later), storing without embeddings. Run reembed later."
        );

        let server_error_warning = super::embedding_degradation_warning(&EmbeddingError::Provider(
            "openai embeddings request returned 500 Internal Server Error: boom".to_string(),
        ))
        .expect("openai http status errors should produce a degradation warning");

        assert_eq!(
            server_error_warning,
            "OpenAI embeddings unavailable (500 Internal Server Error), storing without embeddings. Run reembed later."
        );
    }

    #[tokio::test]
    async fn search_cascades_upward_while_list_stays_exact_scope() {
        let fixture = test_fixture();
        let session_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Session).expect("session store");
        let workspace_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Workspace).expect("workspace store");
        let user_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::User).expect("user store");
        let agent_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Agent).expect("agent store");

        let mut session = sample_memory(MemoryScope::Session, ProvenanceLevel::UserStated);
        session.content = "shared scope note session".to_string();
        let session_id = session.id;
        let mut workspace = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        workspace.content = "shared scope note workspace".to_string();
        let workspace_id = workspace.id;
        let mut user = sample_memory(MemoryScope::User, ProvenanceLevel::UserStated);
        user.content = "shared scope note user".to_string();
        let user_id = user.id;
        let mut agent = sample_memory(MemoryScope::Agent, ProvenanceLevel::UserStated);
        agent.content = "shared scope note agent".to_string();
        let agent_id = agent.id;

        session_store
            .store(session)
            .await
            .expect("store session memory");
        workspace_store
            .store(workspace)
            .await
            .expect("store workspace memory");
        user_store.store(user).await.expect("store user memory");
        agent_store.store(agent).await.expect("store agent memory");

        let search_results = session_store
            .search(SearchQuery {
                text: "shared scope note".to_string(),
                embedding: None,
                scope: MemoryScope::Session,
                state_filter: None,
                type_filter: None,
                max_results: 10,
                context_config: None,
                session_id: None,
            })
            .await
            .expect("search visible scopes");
        let ids = search_results
            .iter()
            .map(|result| result.memory.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&session_id));
        assert!(ids.contains(&workspace_id));
        assert!(ids.contains(&user_id));
        assert!(ids.contains(&agent_id));

        let listed = session_store
            .list(MemoryFilter {
                scope: Some(MemoryScope::Session),
                ..MemoryFilter::default()
            })
            .await
            .expect("list exact session scope");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, session_id);
    }

    #[tokio::test]
    async fn find_similar_cascades_to_higher_visible_scopes() {
        let fixture = test_fixture();
        let session_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Session).expect("session store");
        let workspace_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Workspace).expect("workspace store");

        let workspace = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let workspace_id = workspace.id;
        workspace_store
            .store(workspace)
            .await
            .expect("store workspace memory");
        workspace_store
            .store_embedding(&workspace_id, &[1.0; 768])
            .await
            .expect("store workspace embedding");

        let matches = session_store
            .find_similar(&[1.0; 768], 0.95, 5)
            .await
            .expect("find visible similar memories");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].memory.id, workspace_id);
        assert_eq!(matches[0].memory.scope, MemoryScope::Workspace);
    }

    #[tokio::test]
    async fn search_promotes_session_memory_after_three_distinct_sessions_and_records_provenance() {
        let fixture = test_fixture();
        let session_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Session).expect("session store");
        let workspace_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Workspace).expect("workspace store");

        let mut memory = sample_memory(MemoryScope::Session, ProvenanceLevel::UserStated);
        memory.content = "promotion candidate memory".to_string();
        let id = memory.id;
        session_store
            .store(memory)
            .await
            .expect("store session memory");

        for session_id in [
            "00000000-0000-0000-0000-000000000001",
            "00000000-0000-0000-0000-000000000002",
            "00000000-0000-0000-0000-000000000003",
        ] {
            let _ = session_store
                .search(SearchQuery {
                    text: "promotion candidate".to_string(),
                    embedding: None,
                    scope: MemoryScope::Session,
                    state_filter: None,
                    type_filter: None,
                    max_results: 5,
                    context_config: None,
                    session_id: Some(session_id.to_string()),
                })
                .await
                .expect("search session memory");
        }

        let promoted = workspace_store
            .get_raw(&id)
            .await
            .expect("reload promoted memory")
            .expect("promoted memory exists");
        assert_eq!(promoted.scope, MemoryScope::Workspace);

        let (promotion_rows, version_rows) = workspace_store
            .with_connection(|connection| {
                let promotion_rows = connection.query_row(
                    "SELECT COUNT(*) FROM memory_promotions WHERE memory_id = ?1",
                    [id.to_string()],
                    |row| row.get::<_, i64>(0),
                )?;
                let version_rows = connection.query_row(
                    "SELECT COUNT(*) FROM memory_versions WHERE memory_id = ?1",
                    [id.to_string()],
                    |row| row.get::<_, i64>(0),
                )?;
                Ok((promotion_rows, version_rows))
            })
            .expect("load promotion provenance rows");
        assert_eq!(promotion_rows, 1);
        assert_eq!(version_rows, 1);
    }

    #[tokio::test]
    async fn promotion_pass_advances_corroborated_and_durable_memories_one_scope_only() {
        let fixture = test_fixture();
        let workspace_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Workspace).expect("workspace store");
        let user_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::User).expect("user store");
        let agent_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Agent).expect("agent store");

        let mut corroborated = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        corroborated.content = "corroborated promotion candidate".to_string();
        corroborated.corroboration_count = 2;
        let corroborated_id = corroborated.id;

        let mut durable = sample_memory(MemoryScope::User, ProvenanceLevel::UserStated);
        durable.content = "durable promotion candidate".to_string();
        durable.importance_score = 0.9;
        durable.updated_at = Utc::now() - chrono::Duration::days(8);
        durable.last_accessed_at = Some(Utc::now() - chrono::Duration::days(8));
        let durable_id = durable.id;

        let mut top_scope = sample_memory(MemoryScope::Agent, ProvenanceLevel::UserStated);
        top_scope.content = "top scope remains agent".to_string();
        top_scope.corroboration_count = 5;
        let top_scope_id = top_scope.id;

        workspace_store
            .store(corroborated)
            .await
            .expect("store corroborated memory");
        user_store
            .store(durable)
            .await
            .expect("store durable memory");
        agent_store
            .store(top_scope)
            .await
            .expect("store top-scope memory");

        let promoted = workspace_store
            .run_promotion_pass(None, None)
            .expect("run promotion pass");
        let promoted_ids = promoted.iter().map(|memory| memory.id).collect::<Vec<_>>();
        assert!(promoted_ids.contains(&corroborated_id));
        assert!(promoted_ids.contains(&durable_id));
        assert!(!promoted_ids.contains(&top_scope_id));

        let corroborated_promoted = user_store
            .get_raw(&corroborated_id)
            .await
            .expect("load corroborated promoted memory")
            .expect("corroborated memory exists");
        assert_eq!(corroborated_promoted.scope, MemoryScope::User);

        let durable_promoted = agent_store
            .get_raw(&durable_id)
            .await
            .expect("load durable promoted memory")
            .expect("durable memory exists");
        assert_eq!(durable_promoted.scope, MemoryScope::Agent);
    }

    struct TestFixture {
        store: SqliteMemoryStore,
        path: PathBuf,
    }

    impl Drop for TestFixture {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    fn test_fixture() -> TestFixture {
        let path = env::temp_dir().join(format!("elegy-memory-store-{}.sqlite3", Uuid::new_v4()));
        let store =
            SqliteMemoryStore::new(&path, MemoryScope::Workspace).expect("create sqlite store");
        TestFixture { store, path }
    }

    fn test_fixture_with_provider(provider: Arc<dyn EmbeddingProvider>) -> TestFixture {
        let path = env::temp_dir().join(format!("elegy-memory-store-{}.sqlite3", Uuid::new_v4()));
        let store =
            SqliteMemoryStore::new_with_embedding_provider(&path, MemoryScope::Workspace, provider)
                .expect("create sqlite store with embedding provider");
        TestFixture { store, path }
    }

    fn sample_memory(scope: MemoryScope, provenance: ProvenanceLevel) -> Memory {
        let now = Utc::now();
        Memory {
            id: Uuid::new_v4(),
            content: format!("memory {}", Uuid::new_v4()),
            summary: Some("summary".to_string()),
            scope,
            memory_type: MemoryType::Fact,
            provenance,
            importance_score: 0.8,
            reliability_score: provenance.base_reliability(),
            sensitivity: SensitivityLevel::Low,
            state: MemoryState::Active,
            tags: vec!["baseline".to_string()],
            status: None,
            custom_metadata: HashMap::new(),
            access_count: 0,
            corroboration_count: 0,
            embedding_stale: true,
            created_at: now,
            updated_at: now,
            last_accessed_at: None,
            tenant_id: None,
            user_id: Some("user-1".to_string()),
            agent_id: Some("agent-1".to_string()),
        }
    }

    fn query_embedding() -> Vec<f32> {
        embedding_with_similarity(1.0)
    }

    fn embedding_with_similarity(similarity: f32) -> Vec<f32> {
        let clamped_similarity = similarity.clamp(0.0, 1.0);
        let orthogonal_component = (1.0 - (clamped_similarity * clamped_similarity)).sqrt();
        let mut embedding = vec![0.0; 768];
        embedding[0] = clamped_similarity;
        embedding[1] = orthogonal_component;
        embedding
    }
}
