# Storage Schema

## Overview

elegy-memory uses SQLite with two extensions:
- **sqlite-vec** — vector similarity search (KNN via virtual tables)
- **FTS5** — full-text keyword search with BM25 ranking

Each non-session scope gets its own `.db` file. Session scope uses in-memory JSON.

## File Layout

```
{elegy_data_dir}/
├── workspaces/
│   └── {workspace_id}.db    # One SQLite per workspace
├── user.db                  # User-scoped memories
└── agent.db                 # Agent-scoped procedural memories
```

For multi-tenant deployments, add `{tenant_id}/` prefix:
```
{elegy_data_dir}/
└── {tenant_id}/
    ├── workspaces/...
    ├── user.db
    └── agent.db
```

## Schema (per .db file)

All `.db` files share the same schema. The `scope` column is redundant per-file but useful for validation and potential future merges.

### Table: memories

```sql
CREATE TABLE memories (
    id                TEXT PRIMARY KEY,           -- UUID v4
    content           TEXT NOT NULL,              -- The memory text
    summary           TEXT,                       -- Optional shorter form
    scope             TEXT NOT NULL,              -- 'workspace' | 'user' | 'agent'
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

The `rowid` of `vec_memories` maps to a separate lookup. We maintain a mapping table:

```sql
CREATE TABLE memory_embeddings (
    memory_id   TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
    vec_rowid   INTEGER NOT NULL UNIQUE  -- Maps to vec_memories rowid
);
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

Combine vector and keyword results with weighted scoring:
```sql
-- Pseudo-query (implemented in Rust, not raw SQL)
final_score = 0.7 * (1.0 - vector_distance) + 0.3 * bm25_score
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

### Table: scope_config

```sql
CREATE TABLE scope_config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Populated at init:
-- 'budget_active_max' → '500'
-- 'storage_cap_mb' → '100'
-- 'decay_lambda_base' → '0.10'
-- 'dedup_threshold' → '0.92'
-- 'salience_threshold' → '0.20'
-- 'embedding_dimensions' → '768'
```

## Migration Strategy

Schema version is tracked in `scope_config` with key `schema_version`. On open, the store checks the version and applies migrations sequentially. Migrations are idempotent SQL scripts in `migrations/` directory.

## PostgreSQL Schema (v1)

The v1 `PgMemoryStore` uses the same logical schema with these adaptations:
- `TEXT` UUIDs become `UUID` type
- `REAL` becomes `DOUBLE PRECISION`
- sqlite-vec becomes `pgvector` extension with `vector(768)` column type
- FTS5 becomes `tsvector` + `GIN` index
- `tenant_id` becomes a required column with row-level security policies
- Connection pooling via `deadpool-postgres` or `bb8`

