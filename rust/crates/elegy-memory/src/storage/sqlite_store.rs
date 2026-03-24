use std::{
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
        ContradictionEntry, Memory, MemoryHealthReport, MemoryId, MemoryScope, MemoryState,
        MemoryType, ProvenanceLevel, PurgeReport, ResolutionStatus, ScoredMemory, SearchQuery,
        SensitivityLevel,
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

    async fn search(&self, _query: SearchQuery) -> Result<Vec<ScoredMemory>, StoreError> {
        Err(StoreError::Validation(
            "hybrid search is scheduled for WU5; WU4 intentionally leaves search unimplemented"
                .to_string(),
        ))
    }

    async fn find_similar(
        &self,
        _embedding: &[f32],
        _threshold: f32,
        _limit: usize,
    ) -> Result<Vec<ScoredMemory>, StoreError> {
        Err(StoreError::Validation(
            "vector similarity search is scheduled for WU5; WU4 intentionally leaves find_similar unimplemented"
                .to_string(),
        ))
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

fn encode_embedding(embedding: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(embedding.len() * std::mem::size_of::<f32>());
    for component in embedding {
        bytes.extend_from_slice(&component.to_le_bytes());
    }
    bytes
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
        OptionalFieldUpdate, ProvenanceLevel, ResolutionStatus, SensitivityLevel,
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
