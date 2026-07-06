use std::{
    fs,
    path::Path,
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
/// Each migration declares the capabilities it requires via
/// [`capabilities()`](Migration::capabilities).  The runner enforces the invite
/// boundary at two levels:
///
/// 1. **Capability-split** — the runner checks declared capabilities before
///    invoking [`run()`](Migration::run) and rejects migrations that attempt
///    undeclared access patterns.
/// 2. **SQLite column-level triggers** — created before the first pending
///    migration and dropped after the last one.  Any `UPDATE` or `DELETE` that
///    touches protected columns of `memories` during a migration raises an
///    immediate ABORT.
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
        match (self.generator)("") {
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
mod tests {
    use std::{collections::BTreeMap, env, fs, io::ErrorKind, path::Path};

    use uuid::Uuid;

    use rusqlite::{params, Connection, OptionalExtension};

    use super::{
        compute_content_sha256, create_protective_triggers, create_schema,
        drop_protective_triggers, init_database, migrate_retrieval_scoring_config, run_migrations,
        table_column_exists, Migration, MigrationCapability, ReembedMigration,
        CURRENT_RETRIEVAL_SCORING_VERSION, CURRENT_SCHEMA_VERSION, DEFAULT_SCOPE_CONFIG,
        SCHEMA_VERSION_KEY,
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
            ReembedMigration::new(Box::new(|_| Ok((vec![1.0f32; 4], 4))), "test-profile");
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
                ReembedMigration::new(Box::new(|_| Ok((vec![2.0f32; 4], 4))), "rb-profile");
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
            ReembedMigration::new(Box::new(|_| Ok((vec![3.0f32; 4], 4))), "course-profile");

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
            ReembedMigration::new(Box::new(|_| Ok((vec![4.0f32; 4], 4))), "profile-v1");
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
            ReembedMigration::new(Box::new(|_| Ok((vec![5.0f32; 4], 4))), "profile-v2");
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
            ReembedMigration::new(Box::new(|_| Ok((vec![1.0f32; 4], 4))), "runner-profile");

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
            ReembedMigration::new(Box::new(|_| Ok((vec![1.0f32; 4], 4))), "orphan-profile");
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
            ReembedMigration::new(Box::new(|_| Ok((vec![1.0f32; 4], 4))), "resume-profile");

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
                    Ok((vec![1.0f32; 4], 4))
                }
            }),
            "mid-profile",
        );

        // run() calls check_provider_health first ("" → succeeds)
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
            ReembedMigration::new(Box::new(|_| Ok((vec![2.0f32; 4], 4))), "mid-profile-ok");
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
        let migration_a =
            ReembedMigration::new(Box::new(|_| Ok((vec![1.0f32; 4], 4))), "profile-model-a");
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
        let migration_b =
            ReembedMigration::new(Box::new(|_| Ok((vec![2.0f32; 4], 4))), "profile-model-b");
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
        let database_path =
            env::temp_dir().join(format!("elegy-memory-sa-{}.sqlite3", Uuid::new_v4()));
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
}
