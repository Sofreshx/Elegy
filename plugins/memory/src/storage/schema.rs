use std::{fs, path::Path, time::Duration};

use rusqlite::{Connection, OptionalExtension};

use crate::StoreError;

pub const CURRENT_SCHEMA_VERSION: &str = "1";
const EMBEDDING_DIMENSIONS: usize = 768;
const SCHEMA_VERSION_KEY: &str = "schema_version";
const SQLITE_VEC_MODULE_NAME: &str = "vec0";

const DEFAULT_SCOPE_CONFIG: [(&str, &str); 27] = [
    ("budget_active_max", "500"),
    ("storage_cap_mb", "100"),
    ("decay_lambda_base", "0.10"),
    ("salience_threshold", "0.20"),
    ("novelty_doubt_threshold", "0.80"),
    ("embedding_dimensions", "768"),
    ("similarity_weight", "0.4"),
    ("recency_weight", "0.25"),
    ("access_weight", "0.15"),
    ("priority_weight", "0.2"),
    ("memory_context_ratio", "0.10"),
    ("response_reserve", "4096"),
    ("merge_similarity_threshold", "0.85"),
    ("duplicate_similarity_threshold", "0.99"),
    ("agent_inferred_importance_threshold", "0.50"),
    ("poison_frequency_hourly_threshold", "50"),
    ("poison_frequency_scope_ratio", "0.30"),
    ("poison_frequency_burst_ratio", "0.25"),
    ("poison_frequency_burst_min_hourly", "12"),
    ("poison_trust_mismatch_importance_threshold", "0.80"),
    ("poison_trust_mismatch_count_threshold", "5"),
    ("poison_trust_mismatch_scope_ratio", "0.10"),
    ("poison_bulk_overwrite_count_threshold", "20"),
    ("poison_bulk_overwrite_scope_ratio", "0.15"),
    ("poison_mass_contradiction_per_memory_threshold", "3"),
    ("poison_mass_contradiction_scope_ratio", "0.05"),
    ("poison_remediation_reliability_ceiling", "0.60"),
];

/// Open or create a SQLite-backed memory store database and ensure the MVP schema exists.
pub fn init_database(path: &Path) -> Result<Connection, StoreError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|source| {
                StoreError::Migration(format!(
                    "failed to create database directory {}: {source}",
                    parent.display()
                ))
            })?;
        }
    }

    let mut connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", "ON")?;

    let transaction = connection.transaction()?;
    create_schema(&transaction)?;
    initialize_scope_config(&transaction)?;
    verify_schema_version(&transaction)?;
    transaction.commit()?;

    Ok(connection)
}

fn create_schema(connection: &Connection) -> Result<(), StoreError> {
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS memories (
            id                  TEXT PRIMARY KEY,
            content             TEXT NOT NULL,
            summary             TEXT,
            scope               TEXT NOT NULL,
            memory_type         TEXT NOT NULL DEFAULT 'fact',
            provenance          TEXT NOT NULL DEFAULT 'imported',
            importance_score    REAL NOT NULL DEFAULT 0.5,
            reliability_score   REAL NOT NULL DEFAULT 0.5,
            sensitivity         TEXT NOT NULL DEFAULT 'low',
            state               TEXT NOT NULL DEFAULT 'active',
            tags                TEXT DEFAULT '[]',
            status              TEXT,
            custom_metadata     TEXT DEFAULT '{}',
            access_count        INTEGER NOT NULL DEFAULT 0,
            corroboration_count INTEGER NOT NULL DEFAULT 0,
            embedding_stale     INTEGER NOT NULL DEFAULT 1,
            created_at          TEXT NOT NULL,
            updated_at          TEXT NOT NULL,
            last_accessed_at    TEXT,
            tenant_id           TEXT,
            user_id             TEXT,
            agent_id            TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_memories_state
            ON memories(state);
        CREATE INDEX IF NOT EXISTS idx_memories_scope
            ON memories(scope);
        CREATE INDEX IF NOT EXISTS idx_memories_type
            ON memories(memory_type);
        CREATE INDEX IF NOT EXISTS idx_memories_provenance
            ON memories(provenance);
        CREATE INDEX IF NOT EXISTS idx_memories_tenant
            ON memories(tenant_id)
            WHERE tenant_id IS NOT NULL;
        CREATE INDEX IF NOT EXISTS idx_memories_updated
            ON memories(updated_at);
        CREATE INDEX IF NOT EXISTS idx_memories_importance
            ON memories(importance_score);
        CREATE INDEX IF NOT EXISTS idx_memories_stale
            ON memories(embedding_stale)
            WHERE embedding_stale = 1;

        CREATE TABLE IF NOT EXISTS memory_embeddings (
            memory_id TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
            vec_rowid INTEGER NOT NULL UNIQUE,
            content_sha256 TEXT
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
            content,
            summary,
            tags,
            content=memories,
            content_rowid=rowid
        );

        CREATE TABLE IF NOT EXISTS memory_links (
            id            TEXT PRIMARY KEY,
            source_id     TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
            target_id     TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
            relation_type TEXT NOT NULL,
            weight        REAL DEFAULT 1.0,
            created_at    TEXT NOT NULL,
            UNIQUE(source_id, target_id, relation_type)
        );

        CREATE INDEX IF NOT EXISTS idx_links_source
            ON memory_links(source_id);
        CREATE INDEX IF NOT EXISTS idx_links_target
            ON memory_links(target_id);
        CREATE INDEX IF NOT EXISTS idx_links_type
            ON memory_links(relation_type);

        CREATE TABLE IF NOT EXISTS memory_versions (
            id             TEXT PRIMARY KEY,
            memory_id      TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
            version_number INTEGER NOT NULL,
            content        TEXT NOT NULL,
            changed_at     TEXT NOT NULL,
            changed_by     TEXT NOT NULL,
            change_reason  TEXT,
            UNIQUE(memory_id, version_number)
        );

        CREATE INDEX IF NOT EXISTS idx_versions_memory
            ON memory_versions(memory_id);

        CREATE TABLE IF NOT EXISTS memory_promotions (
            id                 TEXT PRIMARY KEY,
            memory_id          TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
            from_scope         TEXT NOT NULL,
            to_scope           TEXT NOT NULL,
            reason             TEXT NOT NULL,
            trigger_session_id TEXT,
            promoted_at        TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_memory_promotions_memory
            ON memory_promotions(memory_id);
        CREATE INDEX IF NOT EXISTS idx_memory_promotions_promoted_at
            ON memory_promotions(promoted_at);

        CREATE TABLE IF NOT EXISTS memory_session_accesses (
            memory_id          TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
            session_id         TEXT NOT NULL,
            first_accessed_at  TEXT NOT NULL,
            last_accessed_at   TEXT NOT NULL,
            PRIMARY KEY(memory_id, session_id)
        );

        CREATE INDEX IF NOT EXISTS idx_memory_session_accesses_memory
            ON memory_session_accesses(memory_id);
        CREATE INDEX IF NOT EXISTS idx_memory_session_accesses_session
            ON memory_session_accesses(session_id);

        CREATE TABLE IF NOT EXISTS contradictions (
            id                TEXT PRIMARY KEY,
            memory_a_id       TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
            memory_b_id       TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
            detected_at       TEXT NOT NULL,
            description       TEXT NOT NULL,
            resolution_status TEXT NOT NULL DEFAULT 'unresolved',
            resolved_at       TEXT,
            resolution_note   TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_contradictions_status
            ON contradictions(resolution_status);

        CREATE TABLE IF NOT EXISTS memory_corrections (
            id               TEXT PRIMARY KEY,
            memory_id        TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
            previous_content TEXT NOT NULL,
            corrected_content TEXT NOT NULL,
            corrected_by     TEXT NOT NULL,
            reason           TEXT NOT NULL,
            disposition      TEXT NOT NULL DEFAULT 'applied',
            related_memory_id TEXT,
            corrected_at     TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_corrections_memory
            ON memory_corrections(memory_id);

        CREATE TABLE IF NOT EXISTS retrieval_feedback (
            id          TEXT PRIMARY KEY,
            memory_id   TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
            relevant    INTEGER NOT NULL,
            query_text  TEXT,
            recorded_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_feedback_memory
            ON retrieval_feedback(memory_id);

        CREATE TABLE IF NOT EXISTS scope_config (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        "#,
    )?;

    ensure_vec_memories_object(connection)?;
    ensure_memory_embeddings_columns(connection)?;
    ensure_memory_corrections_columns(connection)?;

    Ok(())
}

fn ensure_vec_memories_object(connection: &Connection) -> Result<(), StoreError> {
    if schema_object_exists(connection, "vec_memories")? {
        return Ok(());
    }

    match connection.execute_batch(&format!(
        "CREATE VIRTUAL TABLE vec_memories USING vec0(embedding float[{EMBEDDING_DIMENSIONS}]);"
    )) {
        Ok(()) => Ok(()),
        Err(error) if is_missing_module_error(&error, SQLITE_VEC_MODULE_NAME) => {
            // TODO(WU4/sqlite-vec): replace this rowid-compatible fallback with runtime sqlite-vec
            // extension loading once the backend integration path is finalized for the workspace.
            connection.execute_batch(
                r#"
                CREATE TABLE vec_memories (
                    embedding BLOB NOT NULL
                );
                "#,
            )?;
            Ok(())
        }
        Err(error) => Err(StoreError::from(error)),
    }
}

fn ensure_memory_embeddings_columns(connection: &Connection) -> Result<(), StoreError> {
    if !table_column_exists(connection, "memory_embeddings", "content_sha256")? {
        connection.execute(
            "ALTER TABLE memory_embeddings ADD COLUMN content_sha256 TEXT",
            [],
        )?;
    }

    connection.execute(
        r#"
        CREATE INDEX IF NOT EXISTS idx_memory_embeddings_content_sha256
            ON memory_embeddings(content_sha256)
            WHERE content_sha256 IS NOT NULL
        "#,
        [],
    )?;

    Ok(())
}

fn ensure_memory_corrections_columns(connection: &Connection) -> Result<(), StoreError> {
    if !table_column_exists(connection, "memory_corrections", "disposition")? {
        connection.execute(
            "ALTER TABLE memory_corrections ADD COLUMN disposition TEXT NOT NULL DEFAULT 'applied'",
            [],
        )?;
    }

    if !table_column_exists(connection, "memory_corrections", "related_memory_id")? {
        connection.execute(
            "ALTER TABLE memory_corrections ADD COLUMN related_memory_id TEXT",
            [],
        )?;
    }

    Ok(())
}

fn initialize_scope_config(connection: &Connection) -> Result<(), StoreError> {
    for (key, value) in DEFAULT_SCOPE_CONFIG {
        connection.execute(
            "INSERT OR IGNORE INTO scope_config(key, value) VALUES (?1, ?2)",
            (key, value),
        )?;
    }

    for (key, legacy_default, replacement_default) in [
        ("dedup_threshold", "0.92", "0.85"),
        ("novelty_doubt_threshold", "0.85", "0.80"),
        ("merge_similarity_threshold", "0.92", "0.85"),
    ] {
        connection.execute(
            "UPDATE scope_config SET value = ?3 WHERE key = ?1 AND value = ?2",
            (key, legacy_default, replacement_default),
        )?;
    }

    let existing_schema_version: Option<String> = connection
        .query_row(
            "SELECT value FROM scope_config WHERE key = ?1",
            [SCHEMA_VERSION_KEY],
            |row| row.get(0),
        )
        .optional()?;

    if existing_schema_version.is_none() {
        connection.execute(
            "INSERT INTO scope_config(key, value) VALUES (?1, ?2)",
            (SCHEMA_VERSION_KEY, CURRENT_SCHEMA_VERSION),
        )?;
    }

    Ok(())
}

fn verify_schema_version(connection: &Connection) -> Result<(), StoreError> {
    let version: String = connection.query_row(
        "SELECT value FROM scope_config WHERE key = ?1",
        [SCHEMA_VERSION_KEY],
        |row| row.get(0),
    )?;

    if version == CURRENT_SCHEMA_VERSION {
        return Ok(());
    }

    Err(StoreError::Migration(format!(
        "unsupported schema version {version}; expected {CURRENT_SCHEMA_VERSION}"
    )))
}

fn schema_object_exists(connection: &Connection, name: &str) -> Result<bool, StoreError> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE name = ?1 LIMIT 1",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;

    Ok(exists.is_some())
}

fn table_column_exists(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
) -> Result<bool, StoreError> {
    let pragma = format!("PRAGMA table_info({table_name})");
    let mut statement = connection.prepare(&pragma)?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;

    for row in rows {
        if row? == column_name {
            return Ok(true);
        }
    }

    Ok(false)
}

fn is_missing_module_error(error: &rusqlite::Error, module_name: &str) -> bool {
    error.to_string().contains("no such module") && error.to_string().contains(module_name)
}

#[cfg(test)]
mod tests {
    use std::{env, fs, io::ErrorKind};

    use uuid::Uuid;

    use rusqlite::Connection;

    use super::{init_database, table_column_exists, CURRENT_SCHEMA_VERSION};

    #[test]
    fn init_database_creates_expected_schema_objects_idempotently() {
        let database_path =
            env::temp_dir().join(format!("elegy-memory-schema-{}.sqlite3", Uuid::new_v4()));

        let first_connection = must(
            init_database(&database_path),
            "initialize first temporary schema database",
        );
        let expected_objects = [
            "contradictions",
            "memories",
            "memories_fts",
            "memory_corrections",
            "memory_embeddings",
            "memory_links",
            "memory_promotions",
            "memory_session_accesses",
            "memory_versions",
            "retrieval_feedback",
            "scope_config",
            "vec_memories",
        ];

        let first_objects = must(
            load_object_names(&first_connection),
            "load sqlite schema objects after first init",
        );
        for object_name in expected_objects {
            assert!(
                first_objects.iter().any(|existing| existing == object_name),
                "expected schema object `{object_name}` to exist, found {first_objects:?}",
            );
        }

        let schema_version = must(
            first_connection.query_row(
                "SELECT value FROM scope_config WHERE key = 'schema_version'",
                [],
                |row| row.get::<_, String>(0),
            ),
            "read initialized schema_version",
        );
        assert_eq!(schema_version, CURRENT_SCHEMA_VERSION);
        drop(first_connection);

        let second_connection = must(
            init_database(&database_path),
            "re-initialize the same schema database",
        );
        let second_objects = must(
            load_object_names(&second_connection),
            "load sqlite schema objects after second init",
        );
        for object_name in expected_objects {
            assert!(
                second_objects.iter().any(|existing| existing == object_name),
                "expected schema object `{object_name}` after second init, found {second_objects:?}",
            );
        }
        drop(second_connection);

        if let Err(error) = fs::remove_file(&database_path) {
            assert_eq!(
                error.kind(),
                ErrorKind::NotFound,
                "failed to remove temporary database {}: {error}",
                database_path.display()
            );
        }
    }

    #[test]
    fn init_database_adds_embedding_content_hash_column_for_existing_databases() {
        let database_path =
            env::temp_dir().join(format!("elegy-memory-schema-{}.sqlite3", Uuid::new_v4()));

        let legacy_connection = must(
            Connection::open(&database_path),
            "create legacy temporary schema database",
        );
        must(
            legacy_connection.execute_batch(
                r#"
                CREATE TABLE memory_embeddings (
                    memory_id TEXT PRIMARY KEY,
                    vec_rowid INTEGER NOT NULL UNIQUE
                );
                "#,
            ),
            "create legacy memory_embeddings table",
        );
        drop(legacy_connection);

        let upgraded_connection = must(
            init_database(&database_path),
            "upgrade legacy temporary schema database",
        );
        assert!(
            must(
                table_column_exists(&upgraded_connection, "memory_embeddings", "content_sha256"),
                "load upgraded memory_embeddings columns",
            ),
            "expected init_database to add content_sha256 to memory_embeddings",
        );
        drop(upgraded_connection);

        if let Err(error) = fs::remove_file(&database_path) {
            assert_eq!(
                error.kind(),
                ErrorKind::NotFound,
                "failed to remove temporary database {}: {error}",
                database_path.display()
            );
        }
    }

    #[test]
    fn init_database_updates_legacy_threshold_defaults_without_overriding_custom_values() {
        let database_path =
            env::temp_dir().join(format!("elegy-memory-schema-{}.sqlite3", Uuid::new_v4()));

        let connection = must(
            Connection::open(&database_path),
            "create legacy temporary scope config database",
        );
        must(
            connection.execute_batch(
                r#"
                CREATE TABLE scope_config (
                    key   TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                INSERT INTO scope_config(key, value) VALUES
                    ('dedup_threshold', '0.92'),
                    ('novelty_doubt_threshold', '0.85'),
                    ('merge_similarity_threshold', '0.87'),
                    ('schema_version', '1');
                "#,
            ),
            "create legacy scope_config table",
        );
        drop(connection);

        let upgraded_connection = must(
            init_database(&database_path),
            "upgrade legacy temporary scope config database",
        );
        let dedup_threshold = must(
            upgraded_connection.query_row(
                "SELECT value FROM scope_config WHERE key = 'dedup_threshold'",
                [],
                |row| row.get::<_, String>(0),
            ),
            "read updated dedup_threshold",
        );
        let novelty_doubt_threshold = must(
            upgraded_connection.query_row(
                "SELECT value FROM scope_config WHERE key = 'novelty_doubt_threshold'",
                [],
                |row| row.get::<_, String>(0),
            ),
            "read updated novelty_doubt_threshold",
        );
        let merge_similarity_threshold = must(
            upgraded_connection.query_row(
                "SELECT value FROM scope_config WHERE key = 'merge_similarity_threshold'",
                [],
                |row| row.get::<_, String>(0),
            ),
            "read preserved merge_similarity_threshold",
        );

        assert_eq!(dedup_threshold, "0.85");
        assert_eq!(novelty_doubt_threshold, "0.80");
        assert_eq!(merge_similarity_threshold, "0.87");
        drop(upgraded_connection);

        if let Err(error) = fs::remove_file(&database_path) {
            assert_eq!(
                error.kind(),
                ErrorKind::NotFound,
                "failed to remove temporary database {}: {error}",
                database_path.display()
            );
        }
    }

    fn load_object_names(
        connection: &rusqlite::Connection,
    ) -> Result<Vec<String>, rusqlite::Error> {
        let mut statement = connection.prepare(
            r#"
            SELECT name
            FROM sqlite_master
            WHERE name IN (
                'contradictions',
                'memories',
                'memories_fts',
                'memory_corrections',
                'memory_embeddings',
                'memory_links',
                'memory_promotions',
                'memory_session_accesses',
                'memory_versions',
                'retrieval_feedback',
                'scope_config',
                'vec_memories'
            )
            ORDER BY name
            "#,
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;

        let mut names = Vec::new();
        for row in rows {
            names.push(row?);
        }

        Ok(names)
    }

    fn must<T, E>(result: Result<T, E>, context: &str) -> T
    where
        E: std::fmt::Display,
    {
        match result {
            Ok(value) => value,
            Err(error) => panic!("{context}: {error}"),
        }
    }
}
