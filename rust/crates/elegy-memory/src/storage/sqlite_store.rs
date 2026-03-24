use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, types::Type, Connection, OptionalExtension, Row};
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

use super::schema::init_database;
use crate::{
    traits::MemoryStore,
    types::{
        ContradictionEntry, Memory, MemoryContextConfig, MemoryHealthReport, MemoryId,
        MemoryScope, MemoryState, MemoryType, ProvenanceLevel, PurgeReport, ResolutionStatus,
        ScopeConfig, ScoredMemory, SearchQuery, SensitivityLevel,
    },
    MemoryFilter, MetadataUpdate, OptionalFieldUpdate, StoreError,
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
const DEFAULT_DECAY_LAMBDA_BASE: f32 = 0.10;
const VECTOR_SIMILARITY_BLEND_WEIGHT: f32 = 0.7;
const KEYWORD_SIMILARITY_BLEND_WEIGHT: f32 = 0.3;
const ESTIMATED_CHARS_PER_TOKEN: usize = 4;
const BASE_MEMORY_TOKEN_OVERHEAD: u32 = 16;

/// SQLite-backed [`MemoryStore`] implementation for the MVP memory schema.
#[derive(Clone)]
pub struct SqliteMemoryStore {
    connection: Arc<Mutex<Connection>>,
    scope: MemoryScope,
}

impl SqliteMemoryStore {
    /// Open or create a SQLite-backed store at `path` for a single logical scope.
    pub fn new(path: impl AsRef<Path>, scope: MemoryScope) -> Result<Self, StoreError> {
        let connection = init_database(path.as_ref())?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            scope,
        })
    }

    /// Returns the scope this store instance is responsible for.
    #[must_use]
    pub const fn scope(&self) -> MemoryScope {
        self.scope
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
}

#[async_trait]
impl MemoryStore for SqliteMemoryStore {
    async fn store(&self, memory: Memory) -> Result<MemoryId, StoreError> {
        validate_memory_for_store(&memory, self.scope)?;

        self.with_connection(|connection| {
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
        })
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
            let mut memory =
                require_memory(&transaction, id)?.ok_or_else(|| StoreError::NotFound(*id))?;
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
            let mut memory =
                require_memory(&transaction, id)?.ok_or_else(|| StoreError::NotFound(*id))?;
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
                .ok_or_else(|| StoreError::Validation("access_count overflow".to_string()))?;
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

        if let Some(scope) = filter.scope {
            if scope != self.scope {
                return Ok(Vec::new());
            }
        }

        self.with_connection(|connection| {
            let mut sql = format!("SELECT {MEMORY_SELECT_COLUMNS} FROM memories WHERE scope = ?1");
            let mut params: Vec<rusqlite::types::Value> = vec![rusqlite::types::Value::from(
                scope_to_db(self.scope).to_string(),
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
        if query.scope != self.scope {
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

        self.with_connection(|connection| {
            let scope_config = load_scope_config(connection)?;
            let query_embedding = match query.embedding.as_deref() {
                Some(embedding) => {
                    let expected_dimensions = load_embedding_dimensions(connection)?;
                    validate_query_embedding(embedding, expected_dimensions)?;
                    Some(embedding)
                }
                None => None,
            };

            let mut keyword_scores = if trimmed_text.is_empty() {
                HashMap::new()
            } else {
                load_keyword_scores(
                    connection,
                    self.scope,
                    requested_state,
                    query.type_filter.as_deref(),
                    &trimmed_text,
                )?
            };

            let mut vector_scores = match query_embedding {
                Some(embedding) => load_vector_similarity_scores(
                    connection,
                    self.scope,
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
                self.scope,
                requested_state,
                query.type_filter.as_deref(),
            )?;
            let candidate_memories_by_id: HashMap<MemoryId, Memory> = candidate_memories
                .into_iter()
                .filter(|memory| candidate_ids.contains(&memory.id))
                .map(|memory| (memory.id, memory))
                .collect();

            let decay_lambda = load_decay_lambda(connection)?;
            let mut results = Vec::with_capacity(candidate_memories_by_id.len());
            for id in candidate_ids {
                let Some(memory) = candidate_memories_by_id.get(&id).cloned() else {
                    continue;
                };

                let keyword_similarity = keyword_scores.remove(&id);
                let vector_similarity = vector_scores.remove(&id);
                let similarity = combine_similarity_signals(vector_similarity, keyword_similarity);
                let score = compute_retrieval_score(&memory, similarity, &scope_config, decay_lambda);
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
            let mut results =
                trim_results_to_context_budget(results, query.context_config.as_ref(), &scope_config);
            touch_scored_memories(connection, &mut results)?;
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

            let similarity_scores = load_vector_similarity_scores(
                connection,
                self.scope,
                MemoryState::Active,
                None,
                embedding,
                threshold,
            )?;
            if similarity_scores.is_empty() {
                return Ok(Vec::new());
            }

            let memories_by_id: HashMap<MemoryId, Memory> = load_search_memories(
                connection,
                self.scope,
                MemoryState::Active,
                None,
            )?
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
            if require_memory(&transaction, id)?.is_none() {
                return Err(StoreError::NotFound(*id));
            }

            let expected_dimensions = load_embedding_dimensions(&transaction)?;
            if embedding.len() != expected_dimensions {
                return Err(StoreError::Validation(format!(
                    "embedding dimension mismatch: expected {expected_dimensions}, got {}",
                    embedding.len()
                )));
            }

            let encoded_embedding = encode_embedding(embedding);
            let existing_vec_rowid: Option<i64> = transaction
                .query_row(
                    "SELECT vec_rowid FROM memory_embeddings WHERE memory_id = ?1",
                    [id.to_string()],
                    |row| row.get(0),
                )
                .optional()?;

            match existing_vec_rowid {
                Some(vec_rowid) => {
                    transaction.execute(
                        "UPDATE vec_memories SET embedding = ?1 WHERE rowid = ?2",
                        params![encoded_embedding, vec_rowid],
                    )?;
                }
                None => {
                    transaction.execute(
                        "INSERT INTO vec_memories(embedding) VALUES (?1)",
                        params![encoded_embedding],
                    )?;
                    let vec_rowid = transaction.last_insert_rowid();
                    transaction.execute(
                        "INSERT INTO memory_embeddings(memory_id, vec_rowid) VALUES (?1, ?2)",
                        params![id.to_string(), vec_rowid],
                    )?;
                }
            }

            transaction.execute(
                "UPDATE memories SET embedding_stale = 0 WHERE id = ?1",
                [id.to_string()],
            )?;
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
            let memory =
                require_memory(&transaction, id)?.ok_or_else(|| StoreError::NotFound(*id))?;
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
        Err(StoreError::Validation(
            "purge_all is reserved for a later work unit and is intentionally left as an explicit stub in WU4"
                .to_string(),
        ))
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
                "SELECT id, memory_a_id, memory_b_id, detected_at, description, resolution_status, resolved_at, resolution_note FROM contradictions",
            );
            let mut params = Vec::new();
            if let Some(status) = status {
                sql.push_str(" WHERE resolution_status = ?1");
                params.push(rusqlite::types::Value::from(
                    resolution_status_to_db(status).to_string(),
                ));
            }
            sql.push_str(" ORDER BY detected_at DESC, id ASC");

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
                require_memory(&transaction, a_id)?.ok_or_else(|| StoreError::NotFound(*a_id))?;
            let memory_b =
                require_memory(&transaction, b_id)?.ok_or_else(|| StoreError::NotFound(*b_id))?;
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
}

async fn transition_state(
    store: &SqliteMemoryStore,
    id: &MemoryId,
    target_state: MemoryState,
) -> Result<(), StoreError> {
    store.with_connection(|connection| {
        let transaction = connection.transaction()?;
        let mut memory =
            require_memory(&transaction, id)?.ok_or_else(|| StoreError::NotFound(*id))?;

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
    connection.execute(
        "INSERT INTO memories_fts(rowid, content, summary, tags) VALUES (?1, ?2, ?3, ?4)",
        params![
            row_id,
            &memory.content,
            memory.summary.as_deref(),
            indexed_tags(memory)
        ],
    )?;
    Ok(())
}

fn delete_fts_entry(connection: &Connection, row_id: i64, memory: &Memory) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO memories_fts(memories_fts, rowid, content, summary, tags) VALUES ('delete', ?1, ?2, ?3, ?4)",
        params![
            row_id,
            &memory.content,
            memory.summary.as_deref(),
            indexed_tags(memory)
        ],
    )?;
    Ok(())
}

fn indexed_tags(memory: &Memory) -> String {
    memory.tags.join(" ")
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
    })
}

fn load_decay_lambda(connection: &Connection) -> Result<f32, StoreError> {
    load_f32_config(connection, "decay_lambda_base", DEFAULT_DECAY_LAMBDA_BASE)
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
    let mut bytes = Vec::with_capacity(embedding.len() * std::mem::size_of::<f32>());
    for component in embedding {
        bytes.extend_from_slice(&component.to_le_bytes());
    }
    bytes
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

fn validate_query_embedding(embedding: &[f32], expected_dimensions: usize) -> Result<(), StoreError> {
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
        "similarity threshold must be a finite value in the inclusive range 0.0..=1.0"
            .to_string(),
    ))
}

fn load_search_memories(
    connection: &Connection,
    scope: MemoryScope,
    state: MemoryState,
    type_filter: Option<&[MemoryType]>,
) -> Result<Vec<Memory>, StoreError> {
    let mut sql = format!(
        "SELECT {MEMORY_SELECT_COLUMNS} FROM memories WHERE scope = ?1 AND state = ?2"
    );
    let mut params: Vec<rusqlite::types::Value> = vec![
        rusqlite::types::Value::from(scope_to_db(scope).to_string()),
        rusqlite::types::Value::from(state_to_db(state).to_string()),
    ];

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
    scope: MemoryScope,
    state: MemoryState,
    type_filter: Option<&[MemoryType]>,
    text: &str,
) -> Result<HashMap<MemoryId, f32>, StoreError> {
    let Some(fts_query) = build_fts_query(text) else {
        return Ok(HashMap::new());
    };

    let mut sql = String::from(
        r#"
        SELECT m.id, bm25(memories_fts) AS bm25_score
        FROM memories_fts
        JOIN memories m ON m.rowid = memories_fts.rowid
        WHERE memories_fts MATCH ?1
          AND m.scope = ?2
          AND m.state = ?3
        "#,
    );
    let mut params: Vec<rusqlite::types::Value> = vec![
        rusqlite::types::Value::from(fts_query),
        rusqlite::types::Value::from(scope_to_db(scope).to_string()),
        rusqlite::types::Value::from(state_to_db(state).to_string()),
    ];

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
    scope: MemoryScope,
    state: MemoryState,
    type_filter: Option<&[MemoryType]>,
    query_embedding: &[f32],
    threshold: f32,
) -> Result<HashMap<MemoryId, f32>, StoreError> {
    let expected_dimensions = load_embedding_dimensions(connection)?;
    validate_query_embedding(query_embedding, expected_dimensions)?;

    let mut sql = String::from(
        r#"
        SELECT m.id, v.embedding
        FROM memories m
        JOIN memory_embeddings me ON me.memory_id = m.id
        JOIN vec_memories v ON v.rowid = me.vec_rowid
        WHERE m.scope = ?1
          AND m.state = ?2
        "#,
    );
    let mut params: Vec<rusqlite::types::Value> = vec![
        rusqlite::types::Value::from(scope_to_db(scope).to_string()),
        rusqlite::types::Value::from(state_to_db(state).to_string()),
    ];

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

fn cosine_similarity(left: &[f32], right: &[f32]) -> Result<f32, StoreError> {
    if left.len() != right.len() {
        return Err(StoreError::Validation(format!(
            "cosine similarity requires equal vector dimensions, got {} and {}",
            left.len(),
            right.len()
        )));
    }
    if left.is_empty() {
        return Err(StoreError::Validation(
            "cosine similarity requires non-empty vectors".to_string(),
        ));
    }

    let mut dot_product = 0.0_f32;
    let mut left_norm = 0.0_f32;
    let mut right_norm = 0.0_f32;
    for (left_component, right_component) in left.iter().zip(right.iter()) {
        dot_product += left_component * right_component;
        left_norm += left_component * left_component;
        right_norm += right_component * right_component;
    }

    if left_norm <= f32::EPSILON || right_norm <= f32::EPSILON {
        return Ok(0.0);
    }

    Ok((dot_product / (left_norm.sqrt() * right_norm.sqrt())).clamp(0.0, 1.0))
}

fn combine_similarity_signals(vector_similarity: Option<f32>, keyword_similarity: Option<f32>) -> f32 {
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
    decay_lambda: f32,
) -> f32 {
    let reference_time = memory.last_accessed_at.unwrap_or(memory.updated_at);
    let recency = compute_recency_score(reference_time, decay_lambda);
    let access = ((memory.access_count as f32) + 1.0).ln();
    let priority = memory.importance_score * memory.reliability_score;

    (scope_config.similarity_weight * similarity)
        + (scope_config.recency_weight * recency)
        + (scope_config.access_weight * access)
        + (scope_config.priority_weight * priority)
}

fn compute_recency_score(reference_time: DateTime<Utc>, decay_lambda: f32) -> f32 {
    let days_since_reference =
        ((Utc::now() - reference_time).num_seconds().max(0) as f32) / 86_400.0;
    (-decay_lambda.max(0.0) * days_since_reference).exp()
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
        if retained.is_empty() || used_tokens.saturating_add(estimated_tokens) <= max_memory_tokens {
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
        result.memory.access_count =
            result.memory.access_count.checked_add(1).ok_or_else(|| {
                StoreError::Validation("access_count overflow".to_string())
            })?;
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
    use std::{collections::HashMap, env, fs, path::PathBuf};

    use chrono::Utc;
    use tokio;
    use uuid::Uuid;

    use super::SqliteMemoryStore;
    use crate::{
        Memory, MemoryFilter, MemoryScope, MemoryState, MemoryStore, MemoryType, MetadataUpdate,
        OptionalFieldUpdate, ProvenanceLevel, ResolutionStatus, SearchQuery, SensitivityLevel,
    };

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

        let mut semantic_priority = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
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
                    [keyword_only_id.to_string(), super::format_timestamp(Utc::now())],
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
    async fn search_respects_type_and_state_filters() {
        let fixture = test_fixture();

        let mut active_decision = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        active_decision.content = "migration decision for apollo workspace".to_string();
        active_decision.memory_type = MemoryType::Decision;
        let active_decision_id = active_decision.id;

        let mut active_fact = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        active_fact.content = "apollo workspace fact sheet".to_string();
        active_fact.memory_type = MemoryType::Fact;

        let mut dormant_decision = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
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
            })
            .await
            .expect("run dormant decision search");
        assert_eq!(dormant_results.len(), 1);
        assert_eq!(dormant_results[0].memory.id, dormant_decision_id);
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
}
