use std::{collections::BTreeMap, env, fs, io::ErrorKind, path::Path};

use uuid::Uuid;

use rusqlite::{params, Connection, OptionalExtension};

use super::{
    compute_content_sha256, create_protective_triggers, create_schema, drop_protective_triggers,
    init_database, migrate_retrieval_scoring_config, run_migrations, table_column_exists,
    Migration, MigrationCapability, ReembedMigration, CURRENT_RETRIEVAL_SCORING_VERSION,
    CURRENT_SCHEMA_VERSION, DEFAULT_SCOPE_CONFIG, SCHEMA_VERSION_KEY,
};
use crate::StoreError;

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
            second_objects
                .iter()
                .any(|existing| existing == object_name),
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

fn load_object_names(connection: &rusqlite::Connection) -> Result<Vec<String>, rusqlite::Error> {
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

// -- Phase B migration framework tests --

struct TestScopeConfigMigration;

impl Migration for TestScopeConfigMigration {
    fn name(&self) -> &'static str {
        "test_scope_config"
    }
    fn capabilities(&self) -> &[MigrationCapability] {
        &[MigrationCapability::ScopeConfigWrite]
    }
    fn verify(&self, connection: &Connection) -> Result<(), crate::StoreError> {
        let _count: i64 =
            connection.query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))?;
        Ok(())
    }
    fn run(&self, connection: &Connection) -> Result<(), crate::StoreError> {
        connection.execute(
            "INSERT OR IGNORE INTO scope_config(key, value) VALUES ('test_migration_applied', 'true')",
            [],
        )?;
        Ok(())
    }
}

struct BadMemoryWriterMigration;

impl Migration for BadMemoryWriterMigration {
    fn name(&self) -> &'static str {
        "bad_memory_writer"
    }
    fn capabilities(&self) -> &[MigrationCapability] {
        &[MigrationCapability::ScopeConfigWrite]
    }
    fn verify(&self, _connection: &Connection) -> Result<(), crate::StoreError> {
        Ok(())
    }
    fn run(&self, connection: &Connection) -> Result<(), crate::StoreError> {
        connection.execute("UPDATE memories SET content = 'hacked'", [])?;
        Ok(())
    }
}

#[test]
fn runner_preserves_memory_rows() {
    let database_path =
        env::temp_dir().join(format!("elegy-memory-runner-{}.sqlite3", Uuid::new_v4()));

    let mut connection = must(init_database(&database_path), "create test database");
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, created_at, updated_at) VALUES \
             ('test-id', 'hello world', 'session', 'user', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert memory row",
    );

    let before_count: i64 = must(
        connection.query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0)),
        "count memories before runner",
    );

    let transaction = must(connection.transaction(), "begin runner test transaction");
    must(
        run_migrations(&transaction, &[&TestScopeConfigMigration]),
        "run test migration",
    );
    must(transaction.commit(), "commit runner test transaction");

    let after_count: i64 = must(
        connection.query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0)),
        "count memories after runner",
    );
    assert_eq!(
        before_count, after_count,
        "memory row count must not change after runner migrations",
    );

    let scope_val: Option<String> = must(
        connection
            .query_row(
                "SELECT value FROM scope_config WHERE key = 'test_migration_applied'",
                [],
                |row| row.get(0),
            )
            .optional(),
        "read test migration scope_config value",
    );
    assert_eq!(
        scope_val.as_deref(),
        Some("true"),
        "test migration must have written scope_config value",
    );

    let run_exists: bool = must(
        connection
            .query_row(
                "SELECT 1 FROM migration_runs WHERE migration_name = 'test_scope_config'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional(),
        "check migration_runs entry",
    )
    .is_some();
    assert!(
        run_exists,
        "migration_runs must contain an entry for the test migration",
    );
    drop(connection);

    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

#[test]
fn runner_rolls_back_atomically() {
    let database_path =
        env::temp_dir().join(format!("elegy-memory-rollback-{}.sqlite3", Uuid::new_v4()));

    let mut connection = must(
        init_database(&database_path),
        "create rollback test database",
    );
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, created_at, updated_at) VALUES \
             ('rollback-id', 'rollback content', 'session', 'user', \
              '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert memory row for rollback test",
    );

    let scope_config_before: std::collections::BTreeMap<String, String> = must(
        {
            let mut stmt = must(
                connection.prepare("SELECT key, value FROM scope_config ORDER BY key"),
                "prepare scope_config read",
            );
            let rows = must(
                stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                }),
                "query scope_config rows",
            );
            let mut map = std::collections::BTreeMap::new();
            for row in rows {
                let (k, v) = must(row, "read scope_config row");
                map.insert(k, v);
            }
            Ok::<_, crate::StoreError>(map)
        },
        "snapshot scope_config before rollback",
    );

    {
        let transaction = must(connection.transaction(), "begin rollback transaction");
        must(
            run_migrations(&transaction, &[&TestScopeConfigMigration]),
            "run migration inside rollback transaction",
        );
        must(transaction.rollback(), "rollback transaction");
    }

    let scope_config_after: std::collections::BTreeMap<String, String> = must(
        {
            let mut stmt = must(
                connection.prepare("SELECT key, value FROM scope_config ORDER BY key"),
                "prepare scope_config read",
            );
            let rows = must(
                stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                }),
                "query scope_config rows",
            );
            let mut map = std::collections::BTreeMap::new();
            for row in rows {
                let (k, v) = must(row, "read scope_config row");
                map.insert(k, v);
            }
            Ok::<_, crate::StoreError>(map)
        },
        "snapshot scope_config after rollback",
    );
    assert_eq!(
        scope_config_after, scope_config_before,
        "scope_config must be fully restored after rollback of runner transaction",
    );

    let run_exists: bool = must(
        connection
            .query_row(
                "SELECT 1 FROM migration_runs WHERE migration_name = 'test_scope_config'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional(),
        "check migration_runs after rollback",
    )
    .is_some();
    assert!(
        !run_exists,
        "migration_runs must not contain rolled back entry",
    );
    drop(connection);

    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

#[test]
fn protective_triggers_block_unauthorized_memory_write() {
    let database_path =
        env::temp_dir().join(format!("elegy-memory-trigger-{}.sqlite3", Uuid::new_v4()));

    let mut connection = must(
        init_database(&database_path),
        "create trigger test database",
    );
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, created_at, updated_at) VALUES \
             ('trigger-test-id', 'protected content', 'session', 'user', \
              '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert memory row for trigger test",
    );

    let transaction = must(connection.transaction(), "begin trigger test transaction");
    must(
        create_protective_triggers(&transaction),
        "create protective triggers",
    );

    let result = transaction.execute(
        "UPDATE memories SET content = 'hacked' WHERE id = 'trigger-test-id'",
        [],
    );
    assert!(
        result.is_err(),
        "expected protective triggers to block UPDATE on protected column content"
    );
    let error_text = result.unwrap_err().to_string();
    assert!(
        error_text.contains("capability violation"),
        "trigger error must mention 'capability violation', got: {error_text}",
    );

    must(
        drop_protective_triggers(&transaction),
        "drop protective triggers",
    );
    drop(transaction);

    drop(connection);

    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

#[test]
fn runner_rejects_capability_violation() {
    let database_path =
        env::temp_dir().join(format!("elegy-memory-capviol-{}.sqlite3", Uuid::new_v4()));

    let mut connection = must(
        init_database(&database_path),
        "create cap violation database",
    );
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, created_at, updated_at) VALUES \
             ('cap-viol-id', 'protected content', 'session', 'user', \
              '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert memory row",
    );

    let mem_before: i64 = must(
        connection.query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0)),
        "count memories before cap violation",
    );

    let result = {
        let transaction = must(connection.transaction(), "begin cap violation transaction");
        let res = run_migrations(&transaction, &[&BadMemoryWriterMigration]);
        // Transaction drops here → auto-rollback
        res
    };

    assert!(
        result.is_err(),
        "runner must return Err when a migration writes to protected columns",
    );
    let error_text = result.unwrap_err().to_string();
    assert!(
        error_text.contains("capability violation"),
        "error must mention capability violation, got: {error_text}",
    );

    let mem_after: i64 = must(
        connection.query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0)),
        "count memories after cap violation rollback",
    );
    assert_eq!(
        mem_after, mem_before,
        "memory rows must be preserved after cap violation rollback",
    );

    drop(connection);

    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

// ── Phase 2 Reembed tests ───────────────────────────────────────

#[test]
fn reembed_preserves_memory_rows() {
    let database_path = env::temp_dir().join(format!(
        "elegy-memory-reembed-int-{}.sqlite3",
        Uuid::new_v4()
    ));
    let mut connection = must(
        init_database(&database_path),
        "create reembed test database",
    );
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('m1', 'hello', 'session', 'user', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'); \
             INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('m2', 'world', 'session', 'user', 0, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert test memories",
    );

    let before_rows: Vec<String> = must(
        {
            let mut stmt = must(
                connection.prepare(
                    "SELECT id, content, scope, provenance, state FROM memories ORDER BY id",
                ),
                "prepare snap",
            );
            let rows = must(
                stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                }),
                "query snap",
            );
            let mut out = Vec::new();
            for r in rows {
                let (id, c, s, p, st) = must(r, "row");
                out.push(format!("{id}|{c}|{s}|{p}|{st}"));
            }
            Ok::<_, crate::StoreError>(out)
        },
        "snapshot before",
    );

    let migration =
        ReembedMigration::new(Box::new(|_| Ok((vec![1.0f32; 768], 768))), "test-profile");
    let txn = must(connection.transaction(), "begin txn");
    must(migration.run(&txn), "reembed run");
    must(migration.verify(&txn), "reembed verify");
    must(txn.commit(), "commit");

    let after_rows: Vec<String> = must(
        {
            let mut stmt = must(
                connection.prepare(
                    "SELECT id, content, scope, provenance, state FROM memories ORDER BY id",
                ),
                "prepare snap",
            );
            let rows = must(
                stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                }),
                "query snap",
            );
            let mut out = Vec::new();
            for r in rows {
                let (id, c, s, p, st) = must(r, "row");
                out.push(format!("{id}|{c}|{s}|{p}|{st}"));
            }
            Ok::<_, crate::StoreError>(out)
        },
        "snapshot after",
    );
    assert_eq!(
        before_rows, after_rows,
        "memories source columns must be byte-identical after reembed",
    );

    let m1_stale: i64 = must(
        connection.query_row(
            "SELECT embedding_stale FROM memories WHERE id = 'm1'",
            [],
            |row| row.get(0),
        ),
        "m1 embedding_stale",
    );
    assert_eq!(m1_stale, 0, "m1 must be cleared after reembed");

    let m2_stale: i64 = must(
        connection.query_row(
            "SELECT embedding_stale FROM memories WHERE id = 'm2'",
            [],
            |row| row.get(0),
        ),
        "m2 embedding_stale",
    );
    assert_eq!(m2_stale, 0, "m2 was already 0, must remain 0");

    drop(connection);
    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

#[test]
fn reembed_rolls_back_atomically() {
    let database_path = env::temp_dir().join(format!(
        "elegy-memory-reembed-rb-{}.sqlite3",
        Uuid::new_v4()
    ));
    let mut connection = must(init_database(&database_path), "create rb test database");
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('rb1', 'rollback test', 'session', 'user', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert rollback memory",
    );

    let before_count: i64 = must(
        connection.query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0)),
        "count before",
    );

    let staging_before: i64 = must(
        connection.query_row("SELECT COUNT(*) FROM reembed_staging", [], |row| row.get(0)),
        "staging before",
    );
    assert_eq!(staging_before, 0);

    {
        let txn = must(connection.transaction(), "begin rollback txn");
        let migration =
            ReembedMigration::new(Box::new(|_| Ok((vec![2.0f32; 768], 768))), "rb-profile");
        must(migration.run(&txn), "reembed run inside rollback");
        must(
            txn.query_row("SELECT COUNT(*) FROM reembed_staging", [], |row| {
                row.get::<_, i64>(0)
            }),
            "staging should be cleared after cutover",
        );
        must(txn.rollback(), "rollback");
    }

    let after_count: i64 = must(
        connection.query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0)),
        "count after rollback",
    );
    assert_eq!(
        after_count, before_count,
        "memory count unchanged after rollback"
    );

    let staging_after: i64 = must(
        connection.query_row("SELECT COUNT(*) FROM reembed_staging", [], |row| row.get(0)),
        "staging after rollback",
    );
    assert_eq!(
        staging_after, 0,
        "staging table must be empty after rollback"
    );

    let still_stale: i64 = must(
        connection.query_row(
            "SELECT COUNT(*) FROM memories WHERE embedding_stale = 1",
            [],
            |row| row.get(0),
        ),
        "still stale after rollback",
    );
    assert_eq!(still_stale, 1, "memory must remain stale after rollback");

    drop(connection);
    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

#[test]
fn reembed_detects_concurrent_edit() {
    let database_path = env::temp_dir().join(format!(
        "elegy-memory-reembed-course-{}.sqlite3",
        Uuid::new_v4()
    ));
    let mut connection = must(init_database(&database_path), "create course test database");
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('course1', 'original', 'session', 'user', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert memory for course test",
    );

    let _original_hash = compute_content_sha256("original");

    // Stage manually, then simulate concurrent edit, then cutover
    let migration =
        ReembedMigration::new(Box::new(|_| Ok((vec![3.0f32; 768], 768))), "course-profile");

    let txn = must(connection.transaction(), "begin course txn");
    must(migration.run_staging(&txn), "staging phase");
    must(migration.verify_staging(&txn), "verify staging");

    // Simulate concurrent edit: change content between staging and cutover
    must(
        txn.execute(
            "UPDATE memories SET content = 'modified' WHERE id = 'course1'",
            [],
        ),
        "concurrent edit",
    );

    must(migration.run_cutover(&txn), "cutover phase");

    let still_stale: i64 = must(
        txn.query_row(
            "SELECT embedding_stale FROM memories WHERE id = 'course1'",
            [],
            |row| row.get(0),
        ),
        "embedding_stale after concurrent edit",
    );
    assert_eq!(
        still_stale, 1,
        "memory edited during reembed must remain stale (no stale vector applied)",
    );

    let hash_in_staging: Option<String> = must(
        txn.query_row(
            "SELECT content_sha256 FROM reembed_staging WHERE memory_id = 'course1'",
            [],
            |row| row.get(0),
        )
        .optional(),
        "staging hash",
    );
    assert!(
        hash_in_staging.is_none(),
        "staging must be cleared after cutover",
    );

    must(txn.rollback(), "rollback course transaction");

    drop(connection);
    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

#[test]
fn reembed_orphan_staging_on_profile_change() {
    let database_path = env::temp_dir().join(format!(
        "elegy-memory-reembed-orphan-{}.sqlite3",
        Uuid::new_v4()
    ));
    let mut connection = must(init_database(&database_path), "create orphan test database");
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('orph1', 'orphan test', 'session', 'user', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert memory for orphan test",
    );

    // First run with profile "v1" — populate staging
    let migration_v1 =
        ReembedMigration::new(Box::new(|_| Ok((vec![4.0f32; 768], 768))), "profile-v1");
    let txn = must(connection.transaction(), "begin v1 txn");
    must(migration_v1.run(&txn), "reembed v1");
    must(txn.rollback(), "rollback v1 (simulate incomplete staging)");

    // Manually inject staging data to simulate an incomplete run
    must(
        connection.execute(
            "INSERT OR REPLACE INTO scope_config(key, value) VALUES ('reembed_profile_id', 'profile-v1')",
            [],
        ),
        "set profile-v1",
    );
    must(
        connection.execute(
            "INSERT OR REPLACE INTO reembed_staging(memory_id, content_sha256, embedding, staged_at) \
             VALUES ('orph1', 'deadbeef', X'01020304', '2024-01-01T00:00:00Z')",
            [],
        ),
        "inject stale staging entry",
    );

    let staging_before: i64 = must(
        connection.query_row("SELECT COUNT(*) FROM reembed_staging", [], |row| row.get(0)),
        "staging before profile change",
    );
    assert_eq!(
        staging_before, 1,
        "staging should have 1 entry before profile change"
    );

    // Second run with profile "v2" — should detect orphan and clear it
    let migration_v2 =
        ReembedMigration::new(Box::new(|_| Ok((vec![5.0f32; 768], 768))), "profile-v2");
    let txn = must(connection.transaction(), "begin v2 txn");
    must(migration_v2.run_staging(&txn), "run_staging with v2");

    let staging_after: i64 = must(
        txn.query_row("SELECT COUNT(*) FROM reembed_staging", [], |row| row.get(0)),
        "staging after profile change",
    );
    assert_eq!(
        staging_after, 1,
        "staging must be re-populated for v2 (orphan cleared + new entry)",
    );

    let profile_in_db: String = must(
        txn.query_row(
            "SELECT value FROM scope_config WHERE key = 'reembed_profile_id'",
            [],
            |row| row.get(0),
        ),
        "profile after v2",
    );
    assert_eq!(
        profile_in_db, "profile-v2",
        "profile_id must be updated to v2",
    );

    must(txn.rollback(), "rollback v2");
    drop(connection);
    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

#[test]
fn reembed_handles_provider_failure() {
    let database_path = env::temp_dir().join(format!(
        "elegy-memory-reembed-down-{}.sqlite3",
        Uuid::new_v4()
    ));
    let mut connection = must(init_database(&database_path), "create down test database");
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('down1', 'fail me', 'session', 'user', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'); \
             INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('down2', 'fail me too', 'session', 'user', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert memories for provider down test",
    );

    let migration = ReembedMigration::new(
        Box::new(|_| {
            Err(StoreError::Migration(
                "simulated Ollama provider unavailable".into(),
            ))
        }),
        "down-profile",
    );

    let txn = must(connection.transaction(), "begin down txn");
    // Staging should succeed (failures go to pending_retry)
    must(
        migration.run_staging(&txn),
        "run_staging with provider down",
    );

    let staging_count: i64 = must(
        txn.query_row("SELECT COUNT(*) FROM reembed_staging", [], |row| row.get(0)),
        "staging entries",
    );
    assert_eq!(staging_count, 0, "no staging entries when provider is down");

    let retry_count: i64 = must(
        txn.query_row("SELECT COUNT(*) FROM reembed_pending_retry", [], |row| {
            row.get(0)
        }),
        "pending retry entries",
    );
    assert_eq!(retry_count, 2, "all memories must be in pending_retry");

    // verify_staging should pass (staged 0 + retry 2 == active stale 2)
    must(
        migration.verify_staging(&txn),
        "verify_staging with provider down",
    );

    // Cutover: nothing to cut over (empty staging)
    must(migration.run_cutover(&txn), "cutover with empty staging");

    // Memories must still be stale (no embedding generated)
    let still_stale: i64 = must(
        txn.query_row(
            "SELECT COUNT(*) FROM memories WHERE embedding_stale = 1",
            [],
            |row| row.get(0),
        ),
        "still stale after failed reembed",
    );
    assert_eq!(
        still_stale, 2,
        "memories must remain stale when provider is down",
    );

    must(txn.rollback(), "rollback");
    drop(connection);
    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

// ── Phase B (suite) new tests ───────────────────────────────────

#[test]
fn reembed_migration_via_runner_succeeds() {
    let database_path = env::temp_dir().join(format!(
        "elegy-memory-ree-runner-{}.sqlite3",
        Uuid::new_v4()
    ));
    let mut connection = must(init_database(&database_path), "create runner test database");
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('r1', 'hello runner', 'session', 'user', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert stale memory",
    );

    let migration =
        ReembedMigration::new(Box::new(|_| Ok((vec![1.0f32; 768], 768))), "runner-profile");

    let txn = must(connection.transaction(), "begin runner txn");
    must(
        run_migrations(&txn, &[&migration]),
        "run reembed via runner",
    );
    must(txn.commit(), "commit");

    let stale: i64 = must(
        connection.query_row(
            "SELECT COUNT(*) FROM memories WHERE embedding_stale = 1",
            [],
            |row| row.get(0),
        ),
        "count stale after runner",
    );
    assert_eq!(stale, 0, "stale must be cleared after runner reembed");

    // migration_runs must record the run
    let run_exists: bool = must(
        connection
            .query_row(
                "SELECT 1 FROM migration_runs WHERE migration_name = 'reembed'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional(),
        "check migration_runs",
    )
    .is_some();
    assert!(run_exists, "migration_runs must contain reembed entry");

    // Idempotent: second run should be skipped
    let txn2 = must(connection.transaction(), "begin second txn");
    must(
        run_migrations(&txn2, &[&migration]),
        "second run must be no-op (already recorded)",
    );
    must(txn2.commit(), "commit second");

    drop(connection);
    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

#[test]
fn reembed_fails_fast_when_provider_unavailable_at_start() {
    let database_path =
        env::temp_dir().join(format!("elegy-memory-ree-ff-{}.sqlite3", Uuid::new_v4()));
    let mut connection = must(init_database(&database_path), "create ff test database");
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('ff1', 'fail fast', 'session', 'user', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert stale memory",
    );

    let migration = ReembedMigration::new(
        Box::new(|_| Err(StoreError::Migration("provider is down".into()))),
        "ff-profile",
    );

    let txn = must(connection.transaction(), "begin ff txn");
    let result = run_migrations(&txn, &[&migration]);
    assert!(result.is_err(), "must fail when provider is down at start");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("provider unavailable at start"),
        "error must mention provider unavailable, got: {err_msg}"
    );

    // No staging entries should exist (health check failed before staging)
    let staging_count: i64 = must(
        txn.query_row("SELECT COUNT(*) FROM reembed_staging", [], |row| row.get(0)),
        "staging after fail-fast",
    );
    assert_eq!(staging_count, 0, "no staging entries on fail-fast");

    // Memory must still be stale
    let stale: i64 = must(
        txn.query_row(
            "SELECT COUNT(*) FROM memories WHERE embedding_stale = 1 AND id = 'ff1'",
            [],
            |row| row.get(0),
        ),
        "stale after fail-fast",
    );
    assert_eq!(stale, 1, "memory must remain stale after fail-fast");

    must(txn.rollback(), "rollback ff");
    drop(connection);
    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

#[test]
fn reembed_health_check_succeeds_with_non_empty_probe() {
    // Regression test: The health probe used to pass an empty string "",
    // which fails with real embedding providers (OpenAI/Ollama) that
    // reject empty input. The probe must use a non-empty string so that
    // the health check passes for a working provider.
    let database_path =
        env::temp_dir().join(format!("elegy-memory-ree-ne-{}.sqlite3", Uuid::new_v4()));
    let mut connection = must(init_database(&database_path), "create ne test database");
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('ne1', 'non empty content', 'session', 'user', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert stale memory",
    );

    // Mock provider that rejects empty input (like real OpenAI/Ollama)
    // but works with non-empty input.
    let migration = ReembedMigration::new(
        Box::new(|content: &str| {
            if content.trim().is_empty() {
                Err(StoreError::Migration(
                    "embedding input must not be empty".into(),
                ))
            } else {
                Ok((vec![1.0f32; 768], 768))
            }
        }),
        "ne-profile",
    );

    // Should succeed: health probe is "test" (non-empty), not ""
    let txn = must(connection.transaction(), "begin ne txn");
    must(
        migration.run(&txn),
        "reembed with non-empty-probe-aware provider must succeed",
    );
    must(txn.commit(), "commit ne");

    // Verify the memory was re-embedded
    let stale: i64 = must(
        connection.query_row(
            "SELECT embedding_stale FROM memories WHERE id = 'ne1'",
            [],
            |row| row.get(0),
        ),
        "ne1 stale",
    );
    assert_eq!(stale, 0, "memory must be re-embedded");

    drop(connection);
    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }
}

#[test]
fn reembed_cleans_orphan_staging_at_start_of_run() {
    let database_path = env::temp_dir().join(format!(
        "elegy-memory-ree-orphan2-{}.sqlite3",
        Uuid::new_v4()
    ));
    let mut connection = must(init_database(&database_path), "create orphan test database");
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('orph2', 'orphan test 2', 'session', 'user', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert memory",
    );

    // Inject orphan staging by temporarily disabling FK
    must(
        connection.execute_batch("PRAGMA foreign_keys = OFF;"),
        "disable FK",
    );
    must(
        connection.execute(
            "INSERT INTO reembed_staging(memory_id, content_sha256, embedding, staged_at) \
             VALUES ('nonexistent-id', 'deadbeef', X'01020304', '2024-01-01T00:00:00Z')",
            [],
        ),
        "inject orphan staging entry",
    );
    must(
        connection.execute(
            "INSERT INTO reembed_pending_retry(memory_id, retry_count, last_error, next_retry_at) \
             VALUES ('also-nonexistent', 1, 'error', '2024-01-01T00:00:00Z')",
            [],
        ),
        "inject orphan pending_retry entry",
    );
    must(
        connection.execute_batch("PRAGMA foreign_keys = ON;"),
        "re-enable FK",
    );

    let staging_before: i64 = must(
        connection.query_row("SELECT COUNT(*) FROM reembed_staging", [], |row| row.get(0)),
        "staging before",
    );
    assert_eq!(staging_before, 1, "orphan staging must exist before run");

    let migration =
        ReembedMigration::new(Box::new(|_| Ok((vec![1.0f32; 768], 768))), "orphan-profile");
    let txn = must(connection.transaction(), "begin orphan txn");
    must(migration.run_staging(&txn), "run_staging with orphans");

    // Orphan entries must be cleaned
    let staging_after: i64 = must(
        txn.query_row("SELECT COUNT(*) FROM reembed_staging", [], |row| row.get(0)),
        "staging after",
    );
    assert_eq!(staging_after, 1, "only the real memory must be staged");

    let retry_after: i64 = must(
        txn.query_row("SELECT COUNT(*) FROM reembed_pending_retry", [], |row| {
            row.get(0)
        }),
        "pending_retry after",
    );
    assert_eq!(retry_after, 0, "orphan pending_retry must be cleaned");

    must(txn.rollback(), "rollback orphan");
    drop(connection);
    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

#[test]
fn reembed_resumes_idempotently_after_partial_staging() {
    let database_path = env::temp_dir().join(format!(
        "elegy-memory-ree-resume-{}.sqlite3",
        Uuid::new_v4()
    ));
    let mut connection = must(init_database(&database_path), "create resume test database");
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('res1', 'resume me', 'session', 'user', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'); \
             INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('res2', 'resume me too', 'session', 'user', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert stale memories",
    );

    // Simulate partial staging: only one of two memories is staged
    must(
        connection.execute(
            "INSERT INTO reembed_staging(memory_id, content_sha256, embedding, staged_at) \
             VALUES ('res1', X'deadbeef', X'01020304', '2024-01-01T00:00:00Z')",
            [],
        ),
        "inject partial staging",
    );

    let staging_before: i64 = must(
        connection.query_row("SELECT COUNT(*) FROM reembed_staging", [], |row| row.get(0)),
        "staging before",
    );
    assert_eq!(staging_before, 1, "one memory should be partially staged");

    let migration =
        ReembedMigration::new(Box::new(|_| Ok((vec![1.0f32; 768], 768))), "resume-profile");

    // Full run — should clear stale staging and re-stage both
    let txn = must(connection.transaction(), "begin resume txn");
    must(migration.run(&txn), "full reembed run over partial staging");
    must(txn.commit(), "commit");

    let stale: i64 = must(
        connection.query_row(
            "SELECT COUNT(*) FROM memories WHERE embedding_stale = 1",
            [],
            |row| row.get(0),
        ),
        "stale after resume",
    );
    assert_eq!(stale, 0, "both memories must be re-embedded after resume");

    let staging_after: i64 = must(
        connection.query_row("SELECT COUNT(*) FROM reembed_staging", [], |row| row.get(0)),
        "staging after",
    );
    assert_eq!(staging_after, 0, "staging must be cleared after cutover");

    drop(connection);
    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

#[test]
fn reembed_mid_run_provider_failure_staging_not_promoted() {
    let database_path =
        env::temp_dir().join(format!("elegy-memory-ree-mid-{}.sqlite3", Uuid::new_v4()));
    let mut connection = must(init_database(&database_path), "create mid test database");
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('mid1', 'first ok', 'session', 'user', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z'); \
             INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('mid2', 'second fails', 'session', 'user', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert stale memories",
    );

    // Snapshot content and provenance before migration
    let content_before: Vec<(String, String)> = must(
        {
            let mut stmt = must(
                connection.prepare("SELECT id, content FROM memories ORDER BY id"),
                "snapshot before",
            );
            let rows = must(
                stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                }),
                "query",
            );
            let mut out = Vec::new();
            for r in rows {
                out.push(must(r, "row"));
            }
            Ok::<_, crate::StoreError>(out)
        },
        "snapshot",
    );

    // Provider succeeds for first, fails for second
    let migration = ReembedMigration::new(
        Box::new(|content: &str| {
            if content.contains("second fails") {
                Err(StoreError::Migration(
                    "simulated mid-run provider failure".into(),
                ))
            } else {
                Ok((vec![1.0f32; 768], 768))
            }
        }),
        "mid-profile",
    );

    // run() calls check_provider_health first ("test" → succeeds)
    // staging: mid1 succeeds (staged), mid2 fails (pending_retry)
    // verify: staged(1) + retry(1) == active(2) → passes
    // cutover: mid1 gets promoted (embedding_stale=0), mid2 stays stale
    let txn = must(connection.transaction(), "begin mid txn");
    must(
        migration.run(&txn),
        "reembed with mid-run failure must succeed",
    );
    must(txn.commit(), "commit");

    // (a) Staging partiel NON promu
    let mid1_stale: i64 = must(
        connection.query_row(
            "SELECT embedding_stale FROM memories WHERE id = 'mid1'",
            [],
            |row| row.get(0),
        ),
        "mid1 stale",
    );
    assert_eq!(mid1_stale, 0, "mid1 must be re-embedded");

    let mid2_stale: i64 = must(
        connection.query_row(
            "SELECT embedding_stale FROM memories WHERE id = 'mid2'",
            [],
            |row| row.get(0),
        ),
        "mid2 stale",
    );
    assert_eq!(
        mid2_stale, 1,
        "mid2 must remain stale after provider failure"
    );

    let staging_left: i64 = must(
        connection.query_row("SELECT COUNT(*) FROM reembed_staging", [], |row| row.get(0)),
        "staging after",
    );
    assert_eq!(staging_left, 0, "staging must be cleared after cutover");

    // (b) Zone active intacte — content byte-identical
    let content_after: Vec<(String, String)> = must(
        {
            let mut stmt = must(
                connection.prepare("SELECT id, content FROM memories ORDER BY id"),
                "snapshot after",
            );
            let rows = must(
                stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                }),
                "query",
            );
            let mut out = Vec::new();
            for r in rows {
                out.push(must(r, "row"));
            }
            Ok::<_, crate::StoreError>(out)
        },
        "snapshot",
    );
    assert_eq!(
        content_before, content_after,
        "memories content must be byte-identical after reembed (no-loss invariant)"
    );

    // (c) Reprise OK au run suivant
    let migration_ok =
        ReembedMigration::new(Box::new(|_| Ok((vec![2.0f32; 768], 768))), "mid-profile-ok");
    let txn2 = must(connection.transaction(), "begin recovery txn");
    must(migration_ok.run(&txn2), "recovery reembed succeeds");
    must(txn2.commit(), "commit recovery");

    let stale_after: i64 = must(
        connection.query_row(
            "SELECT COUNT(*) FROM memories WHERE embedding_stale = 1",
            [],
            |row| row.get(0),
        ),
        "stale after recovery",
    );
    assert_eq!(stale_after, 0, "all memories re-embedded after recovery");

    drop(connection);
    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

#[test]
fn reembed_two_consecutive_runs_both_execute() {
    // Proves that reembed is not gated by migration_runs — it is an
    // explicit operator action, not a one-shot schema migration.
    let database_path =
        env::temp_dir().join(format!("elegy-memory-ree-twice-{}.sqlite3", Uuid::new_v4()));
    let mut connection = must(init_database(&database_path), "create twice test database");
    must(
        connection.execute_batch(
            "INSERT INTO memories(id, content, scope, provenance, embedding_stale, created_at, updated_at) VALUES \
             ('t1', 'first model', 'session', 'user', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z');",
        ),
        "insert stale memory",
    );

    // First reembed: model A
    let migration_a = ReembedMigration::new(
        Box::new(|_| Ok((vec![1.0f32; 768], 768))),
        "profile-model-a",
    );
    let txn = must(connection.transaction(), "begin model-a txn");
    must(migration_a.run(&txn), "model-a reembed");
    must(migration_a.verify(&txn), "model-a verify");
    must(txn.commit(), "commit model-a");

    let stale_after_a: i64 = must(
        connection.query_row(
            "SELECT COUNT(*) FROM memories WHERE embedding_stale = 1",
            [],
            |row| row.get(0),
        ),
        "stale after model-a",
    );
    assert_eq!(stale_after_a, 0, "model-a must clear all stale");

    // Simulate model change: re-mark all memories as stale
    must(
        connection.execute("UPDATE memories SET embedding_stale = 1", []),
        "re-mark stale for model B",
    );

    // Second reembed: model B — must EXECUTE (not silently skipped)
    let migration_b = ReembedMigration::new(
        Box::new(|_| Ok((vec![2.0f32; 768], 768))),
        "profile-model-b",
    );
    let txn = must(connection.transaction(), "begin model-b txn");
    must(migration_b.run(&txn), "model-b reembed");
    must(migration_b.verify(&txn), "model-b verify");
    must(txn.commit(), "commit model-b");

    let stale_after_b: i64 = must(
        connection.query_row(
            "SELECT COUNT(*) FROM memories WHERE embedding_stale = 1",
            [],
            |row| row.get(0),
        ),
        "stale after model-b",
    );
    assert_eq!(
        stale_after_b, 0,
        "model-b must execute and clear stale (not silently skipped)"
    );

    // Verify correct embedding was written for model B
    let vec_exists: bool = must(
        connection
            .query_row(
                "SELECT 1 FROM vec_memories WHERE rowid = \
                 (SELECT vec_rowid FROM memory_embeddings WHERE memory_id = 't1')",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional(),
        "check vec",
    )
    .is_some();
    assert!(vec_exists, "model-b embedding must be persisted");

    drop(connection);
    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

// ── Phase 3 — SchemaAdditiveMigration ───────────────────────────

#[test]
fn schema_additive_migration_adds_columns_via_runner() {
    let database_path = env::temp_dir().join(format!("elegy-memory-sa-{}.sqlite3", Uuid::new_v4()));
    let legacy = must(Connection::open(&database_path), "create sa test database");
    must(
        legacy.execute_batch(
            r#"
            CREATE TABLE memory_embeddings (
                memory_id TEXT PRIMARY KEY,
                vec_rowid INTEGER NOT NULL UNIQUE
            );
            CREATE TABLE memory_corrections (
                id               TEXT PRIMARY KEY,
                memory_id        TEXT NOT NULL,
                previous_content TEXT NOT NULL,
                corrected_content TEXT NOT NULL,
                corrected_by     TEXT NOT NULL,
                reason           TEXT NOT NULL,
                corrected_at     TEXT NOT NULL
            );
            CREATE TABLE scope_config (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        ),
        "create legacy tables",
    );

    // Insert a scope_config schema_version so init_database doesn't fail
    must(
        legacy.execute(
            "INSERT INTO scope_config(key, value) VALUES ('schema_version', '1')",
            [],
        ),
        "seed schema_version",
    );
    drop(legacy);

    let upgraded = must(init_database(&database_path), "init_database");
    assert!(
        must(
            table_column_exists(&upgraded, "memory_embeddings", "content_sha256"),
            "check content_sha256 column",
        ),
        "content_sha256 must be added by SchemaAdditiveMigration",
    );
    assert!(
        must(
            table_column_exists(&upgraded, "memory_corrections", "disposition"),
            "check disposition column",
        ),
        "disposition must be added by SchemaAdditiveMigration",
    );
    assert!(
        must(
            table_column_exists(&upgraded, "memory_corrections", "related_memory_id"),
            "check related_memory_id column",
        ),
        "related_memory_id must be added by SchemaAdditiveMigration",
    );

    drop(upgraded);
    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

// ── Phase 3 — ScopeConfigSemanticMigration ──────────────────────

#[test]
fn scope_config_semantic_migration_records_version_via_runner() {
    let database_path =
        env::temp_dir().join(format!("elegy-memory-scs-{}.sqlite3", Uuid::new_v4()));
    let connection = must(init_database(&database_path), "init_database");

    let version: String = must(
        connection.query_row(
            "SELECT value FROM scope_config WHERE key = 'retrieval_scoring_version'",
            [],
            |row| row.get(0),
        ),
        "read retrieval_scoring_version",
    );
    assert_eq!(
        &version, CURRENT_RETRIEVAL_SCORING_VERSION,
        "retrieval_scoring_version must match CURRENT_RETRIEVAL_SCORING_VERSION",
    );

    drop(connection);
    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}

#[test]
fn scope_config_semantic_migration_idempotent_on_rerun() {
    let database_path =
        env::temp_dir().join(format!("elegy-memory-scs2-{}.sqlite3", Uuid::new_v4()));
    let connection = must(init_database(&database_path), "first init");

    let version_before: String = must(
        connection.query_row(
            "SELECT value FROM scope_config WHERE key = 'retrieval_scoring_version'",
            [],
            |row| row.get(0),
        ),
        "version before",
    );
    assert_eq!(&version_before, CURRENT_RETRIEVAL_SCORING_VERSION);

    // Close and re-open — init_database runs again; migration should be skipped
    // (already recorded in migration_runs)
    drop(connection);
    let reopened = must(init_database(&database_path), "second init");

    let version_after: String = must(
        reopened.query_row(
            "SELECT value FROM scope_config WHERE key = 'retrieval_scoring_version'",
            [],
            |row| row.get(0),
        ),
        "version after",
    );
    assert_eq!(
        version_after, version_before,
        "version must be unchanged on rerun"
    );

    drop(reopened);
    if let Err(error) = fs::remove_file(&database_path) {
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }
}
