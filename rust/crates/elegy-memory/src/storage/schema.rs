use std::{fs, path::Path, time::Duration};

use rusqlite::{Connection, OptionalExtension};

use crate::StoreError;

pub const CURRENT_SCHEMA_VERSION: &str = "1";
const EMBEDDING_DIMENSIONS: usize = 768;
const SCHEMA_VERSION_KEY: &str = "schema_version";
const RETRIEVAL_SCORING_VERSION_KEY: &str = "retrieval_scoring_version";
const CURRENT_RETRIEVAL_SCORING_VERSION: &str = "2";
const SQLITE_VEC_MODULE_NAME: &str = "vec0";
const SAFE_SIMILARITY_WEIGHT_CEILING: f64 = 0.70;
const SAFE_RECENCY_WEIGHT_CEILING: f64 = 0.45;
const SAFE_ACCESS_WEIGHT_CEILING: f64 = 0.05;
const SAFE_PRIORITY_WEIGHT_CEILING: f64 = 0.45;

const DEFAULT_SCOPE_CONFIG: [(&str, &str); 27] = [
    ("budget_active_max", "500"),
    ("storage_cap_mb", "100"),
    ("decay_lambda_base", "0.10"),
    ("salience_threshold", "0.20"),
    ("novelty_doubt_threshold", "0.80"),
    ("embedding_dimensions", "768"),
    ("similarity_weight", "0.4"),
    ("recency_weight", "0.25"),
    ("access_weight", "0.05"),
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
///
/// All schema and config migrations run inside a single SQLite transaction so source-of-truth
/// memory rows remain preserve-only across upgrades. The only intentional mutations during
/// initialization target derived, recalculable config entries such as bounded retrieval weights.
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

    migrate_retrieval_scoring_config(connection)?;

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

fn migrate_retrieval_scoring_config(connection: &Connection) -> Result<(), StoreError> {
    let existing_version: Option<String> = connection
        .query_row(
            "SELECT value FROM scope_config WHERE key = ?1",
            [RETRIEVAL_SCORING_VERSION_KEY],
            |row| row.get(0),
        )
        .optional()?;
    if existing_version.as_deref() == Some(CURRENT_RETRIEVAL_SCORING_VERSION) {
        return Ok(());
    }

    // This migration is preserve-only for memory source-of-truth rows: it only re-clamps
    // persisted derived retrieval weights and records the retrieval scoring config version.
    for (key, ceiling_value, ceiling_text) in [
        ("similarity_weight", SAFE_SIMILARITY_WEIGHT_CEILING, "0.70"),
        ("recency_weight", SAFE_RECENCY_WEIGHT_CEILING, "0.45"),
        ("access_weight", SAFE_ACCESS_WEIGHT_CEILING, "0.05"),
        ("priority_weight", SAFE_PRIORITY_WEIGHT_CEILING, "0.45"),
    ] {
        connection.execute(
            "UPDATE scope_config SET value = ?2 WHERE key = ?1 AND CAST(value AS REAL) > ?3",
            (key, ceiling_text, ceiling_value),
        )?;
    }

    connection.execute(
        "INSERT INTO scope_config(key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (
            RETRIEVAL_SCORING_VERSION_KEY,
            CURRENT_RETRIEVAL_SCORING_VERSION,
        ),
    )?;

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
    use std::{collections::BTreeMap, env, fs, io::ErrorKind, path::Path};

    use uuid::Uuid;

    use rusqlite::{params, Connection};

    use super::{
        create_schema, init_database, migrate_retrieval_scoring_config, table_column_exists,
        CURRENT_RETRIEVAL_SCORING_VERSION, CURRENT_SCHEMA_VERSION, DEFAULT_SCOPE_CONFIG,
        SCHEMA_VERSION_KEY,
    };

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
        let retrieval_scoring_version = must(
            first_connection.query_row(
                "SELECT value FROM scope_config WHERE key = 'retrieval_scoring_version'",
                [],
                |row| row.get::<_, String>(0),
            ),
            "read initialized retrieval_scoring_version",
        );
        assert_eq!(retrieval_scoring_version, CURRENT_RETRIEVAL_SCORING_VERSION);
        let access_weight = must(
            first_connection.query_row(
                "SELECT value FROM scope_config WHERE key = 'access_weight'",
                [],
                |row| row.get::<_, String>(0),
            ),
            "read initialized access_weight",
        );
        assert_eq!(access_weight, "0.05");
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

    #[test]
    fn init_database_migrates_retrieval_scoring_weights_to_safe_bounds() {
        let database_path =
            env::temp_dir().join(format!("elegy-memory-schema-{}.sqlite3", Uuid::new_v4()));

        let connection = must(
            Connection::open(&database_path),
            "create legacy temporary retrieval scoring database",
        );
        must(
            connection.execute_batch(
                r#"
                CREATE TABLE scope_config (
                    key   TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                INSERT INTO scope_config(key, value) VALUES
                    ('similarity_weight', '0.95'),
                    ('recency_weight', '0.40'),
                    ('access_weight', '0.15'),
                    ('priority_weight', '0.60'),
                    ('schema_version', '1');
                "#,
            ),
            "create legacy retrieval scoring scope_config table",
        );
        drop(connection);

        let upgraded_connection = must(
            init_database(&database_path),
            "upgrade legacy retrieval scoring database",
        );

        let similarity_weight = must(
            upgraded_connection.query_row(
                "SELECT value FROM scope_config WHERE key = 'similarity_weight'",
                [],
                |row| row.get::<_, String>(0),
            ),
            "read migrated similarity_weight",
        );
        let recency_weight = must(
            upgraded_connection.query_row(
                "SELECT value FROM scope_config WHERE key = 'recency_weight'",
                [],
                |row| row.get::<_, String>(0),
            ),
            "read migrated recency_weight",
        );
        let access_weight = must(
            upgraded_connection.query_row(
                "SELECT value FROM scope_config WHERE key = 'access_weight'",
                [],
                |row| row.get::<_, String>(0),
            ),
            "read migrated access_weight",
        );
        let priority_weight = must(
            upgraded_connection.query_row(
                "SELECT value FROM scope_config WHERE key = 'priority_weight'",
                [],
                |row| row.get::<_, String>(0),
            ),
            "read migrated priority_weight",
        );
        let retrieval_scoring_version = must(
            upgraded_connection.query_row(
                "SELECT value FROM scope_config WHERE key = 'retrieval_scoring_version'",
                [],
                |row| row.get::<_, String>(0),
            ),
            "read migrated retrieval_scoring_version",
        );

        assert_eq!(similarity_weight, "0.70");
        assert_eq!(recency_weight, "0.40");
        assert_eq!(access_weight, "0.05");
        assert_eq!(priority_weight, "0.45");
        assert_eq!(retrieval_scoring_version, CURRENT_RETRIEVAL_SCORING_VERSION);
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
    fn init_database_preserves_memory_rows_while_migrating_retrieval_weights() {
        let database_path =
            env::temp_dir().join(format!("elegy-memory-schema-{}.sqlite3", Uuid::new_v4()));

        let fixture_connection = must(
            build_v1_retrieval_fixture_database(&database_path),
            "create v1 retrieval fixture database",
        );
        let memory_rows_before = must(
            load_memory_row_snapshots(&fixture_connection),
            "snapshot memory rows before retrieval migration",
        );
        let scope_config_before = must(
            load_scope_config_map(&fixture_connection),
            "snapshot scope config before retrieval migration",
        );
        drop(fixture_connection);

        let upgraded_connection = must(
            init_database(&database_path),
            "upgrade v1 retrieval fixture database",
        );
        let memory_rows_after = must(
            load_memory_row_snapshots(&upgraded_connection),
            "snapshot memory rows after retrieval migration",
        );
        let scope_config_after = must(
            load_scope_config_map(&upgraded_connection),
            "snapshot scope config after retrieval migration",
        );

        assert_eq!(
            memory_rows_before.len(),
            memory_rows_after.len(),
            "memory count must stay identical across retrieval config migration",
        );
        assert_eq!(
            memory_rows_before, memory_rows_after,
            "retrieval config migration must preserve every memory row byte-for-byte and metadata-for-metadata",
        );

        let changed_scope_entries = scope_config_after
            .iter()
            .filter_map(|(key, after_value)| {
                let before_value = scope_config_before.get(key);
                if before_value == Some(after_value) {
                    None
                } else {
                    Some((key.clone(), (before_value.cloned(), after_value.clone())))
                }
            })
            .collect::<BTreeMap<_, _>>();

        let expected_changed_entries = BTreeMap::from([
            (
                "access_weight".to_string(),
                (Some("0.15".to_string()), "0.05".to_string()),
            ),
            (
                "priority_weight".to_string(),
                (Some("0.60".to_string()), "0.45".to_string()),
            ),
            (
                "retrieval_scoring_version".to_string(),
                (None, CURRENT_RETRIEVAL_SCORING_VERSION.to_string()),
            ),
            (
                "similarity_weight".to_string(),
                (Some("0.95".to_string()), "0.70".to_string()),
            ),
        ]);

        assert_eq!(
            changed_scope_entries, expected_changed_entries,
            "only retrieval weights above the safe ceilings may change during migration",
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
    fn retrieval_scoring_migration_rolls_back_atomically() {
        let database_path =
            env::temp_dir().join(format!("elegy-memory-schema-{}.sqlite3", Uuid::new_v4()));

        let mut connection = must(
            build_v1_retrieval_fixture_database(&database_path),
            "create rollback fixture database",
        );
        let scope_config_before = must(
            load_scope_config_map(&connection),
            "snapshot scope config before rollback test",
        );

        {
            let transaction = must(
                connection.transaction(),
                "open retrieval migration rollback transaction",
            );
            must(
                migrate_retrieval_scoring_config(&transaction),
                "apply retrieval migration inside rollback transaction",
            );

            let scope_config_during = must(
                load_scope_config_map(&transaction),
                "snapshot scope config inside rollback transaction",
            );
            assert_eq!(
                scope_config_during
                    .get("retrieval_scoring_version")
                    .map(String::as_str),
                Some(CURRENT_RETRIEVAL_SCORING_VERSION),
                "version bump must occur in the same transaction as weight clamps",
            );
            assert_eq!(
                scope_config_during.get("access_weight").map(String::as_str),
                Some("0.05"),
                "clamped weights must be visible before commit inside the transaction",
            );

            must(
                transaction.rollback(),
                "roll back retrieval migration transaction",
            );
        }

        let scope_config_after = must(
            load_scope_config_map(&connection),
            "snapshot scope config after rollback",
        );
        assert_eq!(
            scope_config_after, scope_config_before,
            "rolling back the outer transaction must leave the database fully coherent",
        );
        drop(connection);

        if let Err(error) = fs::remove_file(&database_path) {
            assert_eq!(
                error.kind(),
                ErrorKind::NotFound,
                "failed to remove temporary database {}: {error}",
                database_path.display()
            );
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    struct MemoryRowSnapshot {
        id: String,
        content: String,
        summary: Option<String>,
        scope: String,
        memory_type: String,
        provenance: String,
        importance_score: String,
        reliability_score: String,
        sensitivity: String,
        state: String,
        tags: String,
        status: Option<String>,
        custom_metadata: String,
        access_count: i64,
        corroboration_count: i64,
        embedding_stale: i64,
        created_at: String,
        updated_at: String,
        last_accessed_at: Option<String>,
        tenant_id: Option<String>,
        user_id: Option<String>,
        agent_id: Option<String>,
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

    fn build_v1_retrieval_fixture_database(path: &Path) -> Result<Connection, crate::StoreError> {
        let connection = Connection::open(path)?;
        create_schema(&connection)?;
        seed_scope_config_v1(&connection)?;
        insert_memory_fixture_rows(&connection)?;
        Ok(connection)
    }

    fn seed_scope_config_v1(connection: &Connection) -> Result<(), crate::StoreError> {
        for (key, value) in DEFAULT_SCOPE_CONFIG {
            connection.execute(
                "INSERT INTO scope_config(key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
        }
        connection.execute(
            "INSERT INTO scope_config(key, value) VALUES (?1, ?2)",
            params![SCHEMA_VERSION_KEY, CURRENT_SCHEMA_VERSION],
        )?;
        connection.execute(
            "UPDATE scope_config SET value = '0.95' WHERE key = 'similarity_weight'",
            [],
        )?;
        connection.execute(
            "UPDATE scope_config SET value = '0.40' WHERE key = 'recency_weight'",
            [],
        )?;
        connection.execute(
            "UPDATE scope_config SET value = '0.15' WHERE key = 'access_weight'",
            [],
        )?;
        connection.execute(
            "UPDATE scope_config SET value = '0.60' WHERE key = 'priority_weight'",
            [],
        )?;
        Ok(())
    }

    fn insert_memory_fixture_rows(connection: &Connection) -> Result<(), crate::StoreError> {
        let long_content =
            "long-preserve-only-fixture-".repeat(512) + "terminal-sentinel-for-byte-identity";
        connection.execute(
            r#"
            INSERT INTO memories (
                id, content, summary, scope, memory_type, provenance,
                importance_score, reliability_score, sensitivity, state, tags, status,
                custom_metadata, access_count, corroboration_count, embedding_stale,
                created_at, updated_at, last_accessed_at, tenant_id, user_id, agent_id
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16,
                ?17, ?18, ?19, ?20, ?21, ?22
            )
            "#,
            params![
                "fixture-memory-long",
                long_content,
                "Long fixture summary",
                "workspace",
                "fact",
                "user_stated",
                0.91_f64,
                0.88_f64,
                "medium",
                "active",
                r#"["alpha","beta","gamma"]"#,
                "pinned",
                r#"{"source":"migration-test","lang":"fr","version":1}"#,
                42_i64,
                3_i64,
                0_i64,
                "2026-05-01T08:00:00Z",
                "2026-05-02T09:30:00Z",
                "2026-05-03T10:45:00Z",
                "tenant-fixture",
                "user-fixture",
                "agent-fixture",
            ],
        )?;
        connection.execute(
            r#"
            INSERT INTO memories (
                id, content, summary, scope, memory_type, provenance,
                importance_score, reliability_score, sensitivity, state, tags, status,
                custom_metadata, access_count, corroboration_count, embedding_stale,
                created_at, updated_at, last_accessed_at, tenant_id, user_id, agent_id
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16,
                ?17, ?18, ?19, ?20, ?21, ?22
            )
            "#,
            params![
                "fixture-memory-nullable",
                "Short fixture content with explicit nullables preserved.",
                Option::<String>::None,
                "session",
                "observation",
                "imported",
                0.33_f64,
                0.61_f64,
                "low",
                "dormant",
                r#"["delta"]"#,
                Option::<String>::None,
                r#"{"source":"migration-test","nullable":true}"#,
                0_i64,
                0_i64,
                1_i64,
                "2026-04-11T06:15:00Z",
                "2026-04-11T06:15:00Z",
                Option::<String>::None,
                Option::<String>::None,
                Option::<String>::None,
                Option::<String>::None,
            ],
        )?;
        Ok(())
    }

    fn load_memory_row_snapshots(
        connection: &rusqlite::Connection,
    ) -> Result<Vec<MemoryRowSnapshot>, rusqlite::Error> {
        let mut statement = connection.prepare(
            r#"
            SELECT
                id,
                content,
                summary,
                scope,
                memory_type,
                provenance,
                CAST(importance_score AS TEXT),
                CAST(reliability_score AS TEXT),
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
            FROM memories
            ORDER BY id
            "#,
        )?;
        let rows = statement.query_map([], |row| {
            Ok(MemoryRowSnapshot {
                id: row.get(0)?,
                content: row.get(1)?,
                summary: row.get(2)?,
                scope: row.get(3)?,
                memory_type: row.get(4)?,
                provenance: row.get(5)?,
                importance_score: row.get(6)?,
                reliability_score: row.get(7)?,
                sensitivity: row.get(8)?,
                state: row.get(9)?,
                tags: row.get(10)?,
                status: row.get(11)?,
                custom_metadata: row.get(12)?,
                access_count: row.get(13)?,
                corroboration_count: row.get(14)?,
                embedding_stale: row.get(15)?,
                created_at: row.get(16)?,
                updated_at: row.get(17)?,
                last_accessed_at: row.get(18)?,
                tenant_id: row.get(19)?,
                user_id: row.get(20)?,
                agent_id: row.get(21)?,
            })
        })?;

        let mut snapshots = Vec::new();
        for row in rows {
            snapshots.push(row?);
        }
        Ok(snapshots)
    }

    fn load_scope_config_map(
        connection: &rusqlite::Connection,
    ) -> Result<BTreeMap<String, String>, rusqlite::Error> {
        let mut statement =
            connection.prepare("SELECT key, value FROM scope_config ORDER BY key ASC")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut config = BTreeMap::new();
        for row in rows {
            let (key, value) = row?;
            config.insert(key, value);
        }
        Ok(config)
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
