//! SQLite schema creation, migrations, and protected persistence boundaries.

use std::{
    fs,
    path::Path,
    sync::OnceLock,
    time::{Duration, Instant},
};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::{MemoryScope, StoreError};

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

const DEFAULT_SCOPE_CONFIG: [(&str, &str); 28] = [
    ("budget_active_max", "500"),
    ("storage_cap_mb", "100"),
    ("forgetting_policy", "importance-reliability"),
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
/// Guards `elegy_sqlite_vec_init::register()` to a single call per process:
/// `sqlite3_auto_extension` registers against every connection opened
/// *afterward*, so this must run before the first `Connection::open` below,
/// but only needs to run once regardless of how many stores get opened.
pub(super) static VEC_EXTENSION_REGISTERED: OnceLock<()> = OnceLock::new();

pub fn init_database(path: &Path) -> Result<Connection, StoreError> {
    VEC_EXTENSION_REGISTERED.get_or_init(elegy_sqlite_vec_init::register);

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
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;

    let transaction = connection.transaction()?;
    create_schema(&transaction)?;
    initialize_scope_config(&transaction)?;
    run_migrations(
        &transaction,
        &[&SchemaAdditiveMigration, &ScopeConfigSemanticMigration],
    )?;
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

        CREATE TABLE IF NOT EXISTS migration_runs (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            migration_name  TEXT NOT NULL UNIQUE,
            applied_at      TEXT NOT NULL,
            checksum        TEXT,
            duration_ms     INTEGER NOT NULL DEFAULT 0,
            status          TEXT NOT NULL DEFAULT 'committed'
        );

        CREATE TABLE IF NOT EXISTS reembed_staging (
            memory_id       TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
            content_sha256  TEXT NOT NULL,
            embedding       BLOB NOT NULL,
            staged_at       TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS reembed_pending_retry (
            memory_id       TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
            retry_count     INTEGER NOT NULL DEFAULT 0,
            last_error      TEXT,
            next_retry_at   TEXT NOT NULL
        );
        "#,
    )?;

    ensure_vec_memories_object(connection)?;

    Ok(())
}

fn ensure_vec_memories_object(connection: &Connection) -> Result<(), StoreError> {
    if schema_object_exists(connection, "vec_memories")? {
        // A database created before elegy_sqlite_vec_init::register() existed
        // (i.e. every database created before this change shipped) has the
        // plain-table fallback below, not a real vec0 table, and stays on the
        // fallback forever — ensure_vec_memories_object never re-runs its DDL
        // once the object exists. Migrating those in place (copy every
        // (rowid, embedding) row into a freshly created vec0 table) is real,
        // tracked follow-up work; a fresh database always gets vec0 today.
        return Ok(());
    }

    // `distance_metric=cosine` makes `v.distance` report cosine distance
    // (1 - cosine_similarity, confirmed empirically: an identical vector
    // yields distance 0, an orthogonal vector yields distance 1) rather than
    // vec0's default L2 distance, matching the similarity semantics this
    // crate's scoring model (similarity_weight * similarity + ...) expects.
    match connection.execute_batch(&format!(
        "CREATE VIRTUAL TABLE vec_memories USING vec0(embedding float[{EMBEDDING_DIMENSIONS}] distance_metric=cosine);"
    )) {
        Ok(()) => Ok(()),
        Err(error) if is_missing_module_error(&error, SQLITE_VEC_MODULE_NAME) => {
            // Should not happen: elegy_sqlite_vec_init::register() is called
            // unconditionally before any connection is opened (see
            // init_database). Kept as a defensive fallback, not the expected
            // path, so it's worth surfacing loudly if it ever fires.
            tracing::warn!(
                "sqlite-vec's vec0 module was unavailable despite registration; \
                 falling back to a plain vec_memories table, so vector search \
                 will fall back to a full-table Rust-side scan"
            );
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

/// Whether `vec_memories` is a real `vec0` virtual table (accelerated KNN via
/// `MATCH`/`ORDER BY distance`) or the plain-table fallback (requires a
/// Rust-side scan). Checked via `sqlite_master.sql` rather than cached, since
/// it only runs once per `search()`/`find_similar()` call and a schema object
/// never changes type without `ensure_vec_memories_object` running again.
pub(crate) fn vec_memories_uses_native_module(connection: &Connection) -> Result<bool, StoreError> {
    let sql: Option<String> = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name = 'vec_memories'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(sql.is_some_and(|sql| sql.contains(SQLITE_VEC_MODULE_NAME)))
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

fn scope_to_db(scope: MemoryScope) -> &'static str {
    match scope {
        MemoryScope::Session => "session",
        MemoryScope::Workspace => "workspace",
        MemoryScope::User => "user",
        MemoryScope::Agent => "agent",
    }
}

// ---------------------------------------------------------------------------
// Phase B migration framework — runner, capability-split, triggers, verify()
// ---------------------------------------------------------------------------

/// Columns of the `memories` table protected from accidental modification during
/// schema migrations.  These are the source-of-truth columns that must remain
/// byte-identical across any migration that does not explicitly declare write
/// capability for the relevant table.
const PROTECTED_MEMORY_COLUMNS: &[&str] = &[
    "content",
    "summary",
    "scope",
    "state",
    "provenance",
    "memory_type",
    "tags",
    "custom_metadata",
    "status",
    "tenant_id",
    "user_id",
    "agent_id",
    "importance_score",
    "reliability_score",
    "sensitivity",
];

/// Capabilities granted to a migration, controlling which tables (and at which
/// granularity) the migration is allowed to modify.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MigrationCapability {
    /// Read/write access to the `scope_config` table.
    ScopeConfigWrite,
    /// `ALTER TABLE ADD COLUMN` on derived tables (never on `memories` itself).
    SchemaAdditive,
    /// Recompute embeddings (staging tables, vec_memories, memory_embeddings,
    /// embedding_stale flag).
    Reembed,
}

/// A single versioned, idempotent, transaction-safe schema migration.
///
/// Each migration declares its intended capability via
/// [`capabilities()`](Migration::capabilities). The runner protects the source-row
/// boundary with SQLite column-level triggers created before the first pending
/// migration and dropped after the last one, and calls [`verify()`](Migration::verify)
/// for each migration before recording it as committed. Capabilities describe
/// migration intent; they are not a general per-table SQL sandbox.
#[allow(dead_code)]
pub trait Migration {
    /// Unique name used as the idempotency key in `migration_runs`.
    fn name(&self) -> &'static str;

    /// The set of capabilities this migration requires.
    fn capabilities(&self) -> &[MigrationCapability];

    /// Verify invariants after [`run()`](Migration::run) completed, before the
    /// run is recorded as committed.  An `Err` result causes a full rollback of
    /// the outer `init_database()` transaction.
    fn verify(&self, connection: &Connection) -> Result<(), StoreError>;

    /// Apply the migration.  The caller guarantees that protective triggers are
    /// active and that the outer transaction is still open.
    fn run(&self, connection: &Connection) -> Result<(), StoreError>;
}

/// Create column-level triggers that block accidental modification of
/// `memories` protected columns during migrations.
///
/// Triggers are created in the current transaction (SQLite DDL is
/// transactional) so any crash or rollback cleans them up automatically.
fn create_protective_triggers(connection: &Connection) -> Result<(), StoreError> {
    for column in PROTECTED_MEMORY_COLUMNS {
        connection.execute(
            &format!(
                "CREATE TRIGGER IF NOT EXISTS [protect_memories_col_{column}] \
                 BEFORE UPDATE OF [{column}] ON memories \
                 BEGIN \
                     SELECT RAISE(ABORT, 'Migration capability violation: \
                     column \"{column}\" is protected'); \
                 END;"
            ),
            [],
        )?;
    }
    connection.execute_batch(
        "CREATE TRIGGER IF NOT EXISTS [protect_memories_delete] \
         BEFORE DELETE ON memories \
         BEGIN \
             SELECT RAISE(ABORT, 'Migration capability violation: \
             DELETE on memories is not allowed'); \
         END;",
    )?;
    Ok(())
}

/// Drop the column-level protective triggers created by
/// [`create_protective_triggers`].
fn drop_protective_triggers(connection: &Connection) -> Result<(), StoreError> {
    for column in PROTECTED_MEMORY_COLUMNS {
        connection.execute(
            &format!("DROP TRIGGER IF EXISTS [protect_memories_col_{column}]"),
            [],
        )?;
    }
    connection.execute("DROP TRIGGER IF EXISTS [protect_memories_delete]", [])?;
    Ok(())
}

/// Execute all registered migrations that have not yet been applied.
///
/// Protective triggers are created before the first pending migration and
/// dropped after the last pending migration.  Each pending migration is
/// verified after [`Migration::run()`] and before its `migration_runs` row is
/// inserted.  If [`Migration::verify()`] returns an error the whole
/// initialisation transaction rolls back.
pub fn run_migrations(
    connection: &Connection,
    migrations: &[&dyn Migration],
) -> Result<(), StoreError> {
    let has_pending = migrations.iter().any(|m| {
        connection
            .query_row(
                "SELECT 1 FROM migration_runs WHERE migration_name = ?1",
                [m.name()],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .unwrap_or(None)
            .is_none()
    });

    if !has_pending {
        return Ok(());
    }

    create_protective_triggers(connection)?;

    for migration in migrations {
        let already_run: bool = connection
            .query_row(
                "SELECT 1 FROM migration_runs WHERE migration_name = ?1",
                [migration.name()],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some();

        if already_run {
            continue;
        }

        let start = Instant::now();
        migration.run(connection)?;
        migration.verify(connection)?;
        let duration_ms = start.elapsed().as_millis() as i64;

        connection.execute(
            "INSERT INTO migration_runs(migration_name, applied_at, duration_ms, status) \
             VALUES (?1, ?2, ?3, 'committed')",
            (migration.name(), Utc::now().to_rfc3339(), duration_ms),
        )?;
    }

    drop_protective_triggers(connection)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// ReembedMigration — Section B.4 staging + cutover
// ---------------------------------------------------------------------------

fn compute_content_sha256(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

fn encode_f32_vec(vec: &[f32]) -> Vec<u8> {
    vec.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Migration that adds columns to derived tables (`memory_embeddings`,
/// `memory_corrections`) via `ALTER TABLE ADD COLUMN`.
///
/// Never touches `memories` itself — only SchemaAdditive-Capability tables.
#[allow(dead_code)]
pub struct SchemaAdditiveMigration;

#[allow(dead_code)]
impl Migration for SchemaAdditiveMigration {
    fn name(&self) -> &'static str {
        "ensure_memory_derived_columns"
    }

    fn capabilities(&self) -> &[MigrationCapability] {
        &[MigrationCapability::SchemaAdditive]
    }

    fn run(&self, connection: &Connection) -> Result<(), StoreError> {
        ensure_memory_embeddings_columns(connection)?;
        ensure_memory_corrections_columns(connection)?;
        Ok(())
    }

    fn verify(&self, connection: &Connection) -> Result<(), StoreError> {
        if !table_column_exists(connection, "memory_embeddings", "content_sha256")? {
            return Err(StoreError::Migration(
                "SchemaAdditive verify: content_sha256 column missing in memory_embeddings".into(),
            ));
        }
        if !table_column_exists(connection, "memory_corrections", "disposition")? {
            return Err(StoreError::Migration(
                "SchemaAdditive verify: disposition column missing in memory_corrections".into(),
            ));
        }
        if !table_column_exists(connection, "memory_corrections", "related_memory_id")? {
            return Err(StoreError::Migration(
                "SchemaAdditive verify: related_memory_id column missing in memory_corrections"
                    .into(),
            ));
        }
        Ok(())
    }
}

/// Migration that re-clamps retrieval scoring weights to safe ceilings and
/// records the current `retrieval_scoring_version`.
///
/// This is a `ScopeConfigWrite` operation — it only touches the derived
/// `scope_config` table, never `memories`.
#[allow(dead_code)]
pub struct ScopeConfigSemanticMigration;

#[allow(dead_code)]
impl Migration for ScopeConfigSemanticMigration {
    fn name(&self) -> &'static str {
        "scope_config_retrieval_scoring_v2"
    }

    fn capabilities(&self) -> &[MigrationCapability] {
        &[MigrationCapability::ScopeConfigWrite]
    }

    fn run(&self, connection: &Connection) -> Result<(), StoreError> {
        migrate_retrieval_scoring_config(connection)
    }

    fn verify(&self, connection: &Connection) -> Result<(), StoreError> {
        let existing_version: Option<String> = connection
            .query_row(
                "SELECT value FROM scope_config WHERE key = 'retrieval_scoring_version'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        match existing_version.as_deref() {
            Some(CURRENT_RETRIEVAL_SCORING_VERSION) => Ok(()),
            Some(other) => Err(StoreError::Migration(format!(
                "ScopeConfigSemantic verify: expected retrieval_scoring_version '{}', got '{other}'",
                CURRENT_RETRIEVAL_SCORING_VERSION,
            ))),
            None => Err(StoreError::Migration(
                "ScopeConfigSemantic verify: retrieval_scoring_version key not found".into(),
            )),
        }
    }
}

/// Migration that recomputes embeddings for all active memories with
/// `embedding_stale = 1`.
///
/// Workflow (all within the outer initialisation transaction):
///
/// 1. **Staging** — compute `content_sha256`, generate embeddings via the
///    provider callback, store results in `reembed_staging`.  Provider failures
///    add entries to `reembed_pending_retry`.
/// 2. **Verify (pre-cutover)** — confirm every active stale memory has a
///    staging entry or is pending retry, and that no source memories were
///    deleted.
/// 3. **Cutover** — for each staging entry, compare the current content hash
///    with the staged hash:
///    - **match** → upsert the vector into `memory_embeddings` / `vec_memories`,
///      set `embedding_stale = 0`.
///    - **mismatch** → content was edited between staging and cutover
///      (course concurrente) → set `embedding_stale = 1` for recovery pass.
pub struct ReembedMigration {
    /// Provider callback: `(embedding_vector, dimensions)` for a text slice.
    #[allow(clippy::type_complexity)]
    generator: Box<dyn Fn(&str) -> Result<(Vec<f32>, usize), StoreError>>,
    /// Opaque profile identifier for orphan guard (B.4.5).  If the profile
    /// changes, incomplete staging is discarded.
    profile_id: String,
    /// Memories per inner batch.
    batch_size: usize,
    /// Max automatic retries per memory before escalation.
    #[allow(dead_code)]
    retry_limit: u32,
    /// Optional scope filter.  When `Some`, only stale memories in that
    /// scope are re-embedded.  When `None`, all scopes are processed.
    scope_filter: Option<MemoryScope>,
}

impl ReembedMigration {
    #[allow(clippy::type_complexity)]
    pub fn new(
        generator: Box<dyn Fn(&str) -> Result<(Vec<f32>, usize), StoreError>>,
        profile_id: impl Into<String>,
    ) -> Self {
        Self {
            generator,
            profile_id: profile_id.into(),
            batch_size: 50,
            retry_limit: 3,
            scope_filter: None,
        }
    }

    /// Attach an optional scope filter so only stale memories in the given
    /// scope are re-embedded.
    pub fn with_scope(mut self, scope: MemoryScope) -> Self {
        self.scope_filter = Some(scope);
        self
    }

    /// ── Provider health check (Phase 2-bis) ─────────────────────────
    ///
    /// Called before [`run_staging()`] to fail fast when the provider is
    /// unreachable.  A single test embedding is attempted; on failure the
    /// whole migration aborts without writing any staging rows.
    fn check_provider_health(&self) -> Result<(), StoreError> {
        match (self.generator)("test") {
            Ok(_) => Ok(()),
            Err(e) => Err(StoreError::Migration(format!(
                "reembed provider unavailable at start: {e}"
            ))),
        }
    }

    /// ── Staging phase (B.4.1) ───────────────────────────────────────
    fn run_staging(&self, connection: &Connection) -> Result<(), StoreError> {
        // ── Phase 3: orphan staging cleanup at run start ─────────────
        connection.execute(
            "DELETE FROM reembed_staging WHERE memory_id NOT IN (SELECT id FROM memories)",
            [],
        )?;
        connection.execute(
            "DELETE FROM reembed_pending_retry WHERE memory_id NOT IN (SELECT id FROM memories)",
            [],
        )?;

        let stored_profile: Option<String> = connection
            .query_row(
                "SELECT value FROM scope_config WHERE key = 'reembed_profile_id'",
                [],
                |row| row.get(0),
            )
            .optional()?;

        match stored_profile {
            Some(ref p) if p == &self.profile_id => {
                let staged: i64 =
                    connection
                        .query_row("SELECT COUNT(*) FROM reembed_staging", [], |row| row.get(0))?;
                let (where_clause, _) = self.stale_where_clause();
                let active_sql = format!("SELECT COUNT(*) FROM memories WHERE {where_clause}");
                let active: i64 = self.query_stale(connection, &active_sql, |row| row.get(0))?;
                if staged >= active {
                    return Ok(());
                }
                connection.execute("DELETE FROM reembed_staging", [])?;
            }
            _ => {
                connection.execute("DELETE FROM reembed_staging", [])?;
                connection.execute(
                    "INSERT OR REPLACE INTO scope_config(key, value) VALUES ('reembed_profile_id', ?1)",
                    [&self.profile_id],
                )?;
            }
        }

        let (where_clause, scope_val) = self.stale_where_clause();
        let select_sql = format!("SELECT id, content FROM memories WHERE {where_clause}");
        let mut stmt = connection.prepare(&select_sql)?;
        let scope_params: Vec<rusqlite::types::Value> = scope_val
            .into_iter()
            .map(rusqlite::types::Value::from)
            .collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(scope_params.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut batch: Vec<(String, String)> = Vec::new();
        for row in rows {
            let (id, content) = row?;
            batch.push((id, content));
            if batch.len() >= self.batch_size {
                self.process_batch(connection, &batch)?;
                batch.clear();
            }
        }
        if !batch.is_empty() {
            self.process_batch(connection, &batch)?;
        }
        Ok(())
    }

    fn process_batch(
        &self,
        connection: &Connection,
        batch: &[(String, String)],
    ) -> Result<(), StoreError> {
        let now = Utc::now().to_rfc3339();
        for (id, content) in batch {
            let hash = compute_content_sha256(content);
            match (self.generator)(content) {
                Ok((embedding, _)) => {
                    let blob = encode_f32_vec(&embedding);
                    connection.execute(
                        "INSERT OR REPLACE INTO reembed_staging(memory_id, content_sha256, embedding, staged_at) \
                         VALUES (?1, ?2, ?3, ?4)",
                        rusqlite::params![id, hash, blob, now],
                    )?;
                    connection.execute(
                        "DELETE FROM reembed_pending_retry WHERE memory_id = ?1",
                        [id],
                    )?;
                }
                Err(e) => {
                    connection.execute(
                        "INSERT OR REPLACE INTO reembed_pending_retry(memory_id, retry_count, last_error, next_retry_at) \
                         VALUES (?1, COALESCE((SELECT retry_count FROM reembed_pending_retry WHERE memory_id = ?1), 0) + 1, \
                         ?2, ?3)",
                        rusqlite::params![id, e.to_string(), now],
                    )?;
                }
            }
        }
        Ok(())
    }

    fn stale_where_clause(&self) -> (&str, Option<String>) {
        match self.scope_filter {
            Some(ref s) => (
                "state = 'active' AND embedding_stale = 1 AND scope = ?1",
                Some(scope_to_db(*s).to_string()),
            ),
            None => ("state = 'active' AND embedding_stale = 1", None),
        }
    }

    fn query_stale<F, T>(&self, connection: &Connection, sql: &str, f: F) -> Result<T, StoreError>
    where
        F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        let (_, scope_val) = self.stale_where_clause();
        let scope_params: Vec<rusqlite::types::Value> = scope_val
            .into_iter()
            .map(rusqlite::types::Value::from)
            .collect();
        Ok(connection.query_row(sql, rusqlite::params_from_iter(scope_params.iter()), f)?)
    }

    /// ── Pre-cutover verify (B.4.3) ──────────────────────────────────
    fn verify_staging(&self, connection: &Connection) -> Result<(), StoreError> {
        let staged: i64 =
            connection.query_row("SELECT COUNT(*) FROM reembed_staging", [], |row| row.get(0))?;
        let retrying: i64 =
            connection.query_row("SELECT COUNT(*) FROM reembed_pending_retry", [], |row| {
                row.get(0)
            })?;
        let (where_clause, _) = self.stale_where_clause();
        let active_sql = format!("SELECT COUNT(*) FROM memories WHERE {where_clause}");
        let active: i64 = self.query_stale(connection, &active_sql, |row| row.get(0))?;
        if staged + retrying != active {
            return Err(StoreError::Migration(format!(
                "reembed verify: staged ({staged}) + retry ({retrying}) != active stale ({active})",
            )));
        }
        let orphaned: i64 = connection.query_row(
            "SELECT COUNT(*) FROM reembed_staging s \
             WHERE NOT EXISTS (SELECT 1 FROM memories m WHERE m.id = s.memory_id)",
            [],
            |row| row.get(0),
        )?;
        if orphaned > 0 {
            return Err(StoreError::Migration(format!(
                "reembed verify: {orphaned} staging entries reference deleted memories",
            )));
        }
        Ok(())
    }

    /// ── Cutover phase (B.4.2) ───────────────────────────────────────
    fn run_cutover(&self, connection: &Connection) -> Result<(), StoreError> {
        let mut stmt = connection
            .prepare("SELECT memory_id, content_sha256, embedding FROM reembed_staging")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        })?;

        for row in rows {
            let (id, staged_hash, blob) = row?;

            let current: Option<String> = connection
                .query_row("SELECT content FROM memories WHERE id = ?1", [&id], |row| {
                    row.get(0)
                })
                .optional()?;

            match current {
                None => {}
                Some(content) => {
                    let current_hash = compute_content_sha256(&content);
                    if current_hash == staged_hash {
                        Self::upsert_cutover_embedding(connection, &id, &blob, &staged_hash)?;
                    } else {
                        connection.execute(
                            "UPDATE memories SET embedding_stale = 1 WHERE id = ?1",
                            [&id],
                        )?;
                    }
                }
            }
        }

        connection.execute("DELETE FROM reembed_staging", [])?;
        Ok(())
    }

    fn upsert_cutover_embedding(
        connection: &Connection,
        id: &str,
        blob: &[u8],
        content_sha256: &str,
    ) -> Result<(), StoreError> {
        let existing: Option<i64> = connection
            .query_row(
                "SELECT vec_rowid FROM memory_embeddings WHERE memory_id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?;

        match existing {
            Some(vec_rowid) => {
                connection.execute(
                    "UPDATE vec_memories SET embedding = ?1 WHERE rowid = ?2",
                    rusqlite::params![blob, vec_rowid],
                )?;
                connection.execute(
                    "UPDATE memory_embeddings SET content_sha256 = ?1 WHERE memory_id = ?2",
                    rusqlite::params![content_sha256, id],
                )?;
            }
            None => {
                connection.execute(
                    "INSERT INTO vec_memories(embedding) VALUES (?1)",
                    rusqlite::params![blob],
                )?;
                let vec_rowid = connection.last_insert_rowid();
                connection.execute(
                    "INSERT INTO memory_embeddings(memory_id, vec_rowid, content_sha256) VALUES (?1, ?2, ?3)",
                    rusqlite::params![id, vec_rowid, content_sha256],
                )?;
            }
        }
        connection.execute(
            "UPDATE memories SET embedding_stale = 0 WHERE id = ?1",
            [id],
        )?;
        Ok(())
    }
}

impl Migration for ReembedMigration {
    fn name(&self) -> &'static str {
        "reembed"
    }

    fn capabilities(&self) -> &[MigrationCapability] {
        &[MigrationCapability::Reembed]
    }

    fn run(&self, connection: &Connection) -> Result<(), StoreError> {
        self.check_provider_health()?;
        self.run_staging(connection)?;
        self.verify_staging(connection)?;
        self.run_cutover(connection)?;
        Ok(())
    }

    fn verify(&self, connection: &Connection) -> Result<(), StoreError> {
        let (where_clause, scope_val) = self.stale_where_clause();
        let stale_sql = format!(
            "SELECT COUNT(*) FROM memories WHERE {where_clause} \
             AND id NOT IN (SELECT memory_id FROM reembed_pending_retry)",
        );
        let scope_params: Vec<rusqlite::types::Value> = scope_val
            .into_iter()
            .map(rusqlite::types::Value::from)
            .collect();
        let stale_remaining: i64 = connection.query_row(
            &stale_sql,
            rusqlite::params_from_iter(scope_params.iter()),
            |row| row.get(0),
        )?;
        let staging_left: i64 =
            connection.query_row("SELECT COUNT(*) FROM reembed_staging", [], |row| row.get(0))?;
        if staging_left > 0 {
            return Err(StoreError::Migration(format!(
                "reembed verify: {staging_left} staging entries remain after cutover",
            )));
        }
        if stale_remaining > 0 {
            return Err(StoreError::Migration(format!(
                "reembed verify: {stale_remaining} memories still stale and not pending retry",
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
