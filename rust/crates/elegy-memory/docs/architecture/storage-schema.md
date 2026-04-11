# Storage Schema

## Overview

elegy-memory uses SQLite with two extensions:
- **sqlite-vec** — vector similarity search (KNN via virtual tables)
- **FTS5** — full-text keyword search with BM25 ranking

The current crate uses one SQLite database file per configured CLI/store target. All logical scopes (`session`, `workspace`, `user`, `agent`) coexist inside that shared file, and the `scope` column is the active isolation / visibility key. Session scope is persisted in the same SQLite backend today.

## File Layout

```
{chosen_db_path}             # One SQLite database file
└── memories(scope=...)      # session / workspace / user / agent rows share the file
```

The CLI defaults to `~/.elegy/memory.db`, but `--db <path>` can point at any SQLite file.

## Schema (per .db file)

Each configured `.db` file uses the same schema. The `scope` column is not redundant in the current implementation because one file can contain all four scopes and search visibility cascades across them.

### Table: memories

```sql
CREATE TABLE memories (
    id                TEXT PRIMARY KEY,           -- UUID v4
    content           TEXT NOT NULL,              -- The memory text
    summary           TEXT,                       -- Optional shorter form
    scope             TEXT NOT NULL,              -- 'session' | 'workspace' | 'user' | 'agent'
    memory_type       TEXT NOT NULL DEFAULT 'fact', -- 'fact' | 'preference' | 'decision' | 'procedure' | 'observation'
    provenance        TEXT NOT NULL DEFAULT 'imported', -- 'user_stated' | 'agent_observed' | 'agent_inferred' | 'consolidated' | 'imported'
    importance_score  REAL NOT NULL DEFAULT 0.5,  -- LLM-assigned, 0.0-1.0
    reliability_score REAL NOT NULL DEFAULT 0.5,  -- System-computed, 0.0-1.0
    sensitivity       TEXT NOT NULL DEFAULT 'low', -- 'low' | 'medium' | 'high' | 'critical'
    state             TEXT NOT NULL DEFAULT 'active', -- 'active' | 'dormant' | 'deleted'
    
    -- Metadata (JSON for extensibility)
    tags              TEXT DEFAULT '[]',           -- JSON array of strings
    status            TEXT,                        -- Optional workflow status ('planned', 'in_progress', 'completed', etc.)
    custom_metadata   TEXT DEFAULT '{}',           -- JSON object for extensible key-value pairs
    
    -- Tracking
    access_count      INTEGER NOT NULL DEFAULT 0,
    corroboration_count INTEGER NOT NULL DEFAULT 0,
    embedding_stale   INTEGER NOT NULL DEFAULT 1,  -- Boolean: 1 = needs re-embedding
    
    -- Timestamps
    created_at        TEXT NOT NULL,               -- ISO 8601 / RFC 3339
    updated_at        TEXT NOT NULL,               -- ISO 8601
    last_accessed_at  TEXT,                        -- ISO 8601, NULL if never accessed
    
    -- Multi-tenant (NULL for single-user mode)
    tenant_id         TEXT,
    user_id           TEXT,
    agent_id          TEXT
);

-- Indexes
CREATE INDEX idx_memories_state ON memories(state);
CREATE INDEX idx_memories_scope ON memories(scope);
CREATE INDEX idx_memories_type ON memories(memory_type);
CREATE INDEX idx_memories_provenance ON memories(provenance);
CREATE INDEX idx_memories_tenant ON memories(tenant_id) WHERE tenant_id IS NOT NULL;
CREATE INDEX idx_memories_updated ON memories(updated_at);
CREATE INDEX idx_memories_importance ON memories(importance_score);
CREATE INDEX idx_memories_stale ON memories(embedding_stale) WHERE embedding_stale = 1;
```

### Virtual Table: vec_memories (sqlite-vec)

```sql
CREATE VIRTUAL TABLE vec_memories USING vec0(
    embedding float[768]    -- Dimension depends on embedding model. 768 for all-MiniLM-L6-v2, 1536 for OpenAI ada-002
);
```

When the `vec0` module is not available at runtime (e.g., sqlite-vec extension not loaded), the schema initialization falls back to a regular table:

```sql
CREATE TABLE vec_memories (
    embedding BLOB NOT NULL
);
```

This keeps the `rowid`-based mapping intact so that the rest of the schema can reference `vec_memories` uniformly.

The `rowid` of `vec_memories` maps to a separate lookup. We maintain a mapping table:

```sql
CREATE TABLE memory_embeddings (
    memory_id      TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
    vec_rowid      INTEGER NOT NULL UNIQUE,  -- Maps to vec_memories rowid
    content_sha256 TEXT                       -- Content hash for embedding-cache deduplication
);

CREATE INDEX idx_memory_embeddings_content_sha256
    ON memory_embeddings(content_sha256)
    WHERE content_sha256 IS NOT NULL;
```

**KNN Query Pattern:**
```sql
SELECT m.*, v.distance
FROM vec_memories v
JOIN memory_embeddings me ON me.vec_rowid = v.rowid
JOIN memories m ON m.id = me.memory_id
WHERE v.embedding MATCH ?query_embedding
  AND m.state = 'active'
ORDER BY v.distance
LIMIT ?k;
```

### Virtual Table: memories_fts (FTS5)

```sql
CREATE VIRTUAL TABLE memories_fts USING fts5(
    content,
    summary,
    tags,
    content=memories,
    content_rowid=rowid
);
```

**Keyword Query Pattern:**
```sql
SELECT m.*, bm25(memories_fts) as text_score
FROM memories_fts fts
JOIN memories m ON m.rowid = fts.rowid
WHERE memories_fts MATCH ?query
  AND m.state = 'active'
ORDER BY text_score
LIMIT ?k;
```

### Hybrid Search

Combine vector and keyword results into a blended similarity signal (see [Memory Model → Retrieval Scoring](memory-model.md#retrieval-scoring) for the full formula):
```sql
-- Pseudo-query (implemented in Rust, not raw SQL)
blended_similarity = 0.7 * (1.0 - vector_distance) + 0.3 * bm25_score
```

### Table: memory_links (Proto-Graph)

```sql
CREATE TABLE memory_links (
    id            TEXT PRIMARY KEY,
    source_id     TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    target_id     TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL,    -- 'related' | 'supersedes' | 'contradicts' | 'corroborates' | 'promotes_from'
    weight        REAL DEFAULT 1.0,
    created_at    TEXT NOT NULL,
    
    UNIQUE(source_id, target_id, relation_type)
);

CREATE INDEX idx_links_source ON memory_links(source_id);
CREATE INDEX idx_links_target ON memory_links(target_id);
CREATE INDEX idx_links_type ON memory_links(relation_type);
```

### Table: memory_versions

```sql
CREATE TABLE memory_versions (
    id              TEXT PRIMARY KEY,
    memory_id       TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    version_number  INTEGER NOT NULL,
    content         TEXT NOT NULL,         -- Old content (text only, no embedding)
    changed_at      TEXT NOT NULL,
    changed_by      TEXT NOT NULL,         -- 'user' | 'agent:{agent_id}' | 'system:consolidation' | 'system:contradiction_resolution'
    change_reason   TEXT,
    
    UNIQUE(memory_id, version_number)
);

CREATE INDEX idx_versions_memory ON memory_versions(memory_id);
```

### Table: memory_promotions

```sql
CREATE TABLE memory_promotions (
    id                 TEXT PRIMARY KEY,
    memory_id          TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    from_scope         TEXT NOT NULL,
    to_scope           TEXT NOT NULL,
    reason             TEXT NOT NULL,
    trigger_session_id TEXT,
    promoted_at        TEXT NOT NULL
);

CREATE INDEX idx_memory_promotions_memory
    ON memory_promotions(memory_id);
CREATE INDEX idx_memory_promotions_promoted_at
    ON memory_promotions(promoted_at);
```

### Table: memory_session_accesses

```sql
CREATE TABLE memory_session_accesses (
    memory_id         TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    session_id        TEXT NOT NULL,
    first_accessed_at TEXT NOT NULL,
    last_accessed_at  TEXT NOT NULL,
    PRIMARY KEY(memory_id, session_id)
);

CREATE INDEX idx_memory_session_accesses_memory
    ON memory_session_accesses(memory_id);
CREATE INDEX idx_memory_session_accesses_session
    ON memory_session_accesses(session_id);
```

### Table: contradictions

```sql
CREATE TABLE contradictions (
    id                TEXT PRIMARY KEY,
    memory_a_id       TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    memory_b_id       TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    detected_at       TEXT NOT NULL,
    description       TEXT NOT NULL,
    resolution_status TEXT NOT NULL DEFAULT 'unresolved', -- 'unresolved' | 'resolved_by_user' | 'resolved_by_system' | 'dismissed'
    resolved_at       TEXT,
    resolution_note   TEXT
);

CREATE INDEX idx_contradictions_status ON contradictions(resolution_status);
```

Tier 2 LLM consolidation does **not** add new persistence tables. When the model returns
`CONTRADICTION: ...`, the implementation records that result in this same contradiction journal.

### Table: scope_config

```sql
CREATE TABLE scope_config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Populated at init (key examples):
-- 'schema_version' → current schema version
-- 'budget_active_max' → '500'
-- 'storage_cap_mb' → '100'
-- 'embedding_dimensions' → '768'
-- 'decay_lambda_base' → '0.10'
-- 'similarity_weight' → '0.40'
-- 'recency_weight' → '0.25'
-- 'access_weight' → '0.15'
-- 'priority_weight' → '0.20'
-- 'memory_context_ratio' → '0.10'
-- 'response_reserve' → '4096'
-- 'salience_threshold' → '0.20'
-- 'novelty_doubt_threshold' → '0.80'
-- 'merge_similarity_threshold' → '0.85'
-- 'duplicate_similarity_threshold' → '0.99'
-- 'agent_inferred_importance_threshold' → '0.50'

The key `dedup_threshold` also exists in databases created before the threshold rename and is maintained by the schema migration path, but it is not loaded by current application code.
```

## Migration Strategy

Schema version is tracked in `scope_config` with key `schema_version`. The current implementation still uses additive, idempotent `CREATE TABLE IF NOT EXISTS` / `ALTER TABLE` initialization at open time rather than an external migration runner.

## PostgreSQL Schema (v1)

The v1 `PgMemoryStore` uses the same logical schema with these adaptations:
- `TEXT` UUIDs become `UUID` type
- `REAL` becomes `DOUBLE PRECISION`
- sqlite-vec becomes `pgvector` extension with `vector(768)` column type
- FTS5 becomes `tsvector` + `GIN` index
- `tenant_id` becomes a required column with row-level security policies
- Connection pooling via `deadpool-postgres` or `bb8`

