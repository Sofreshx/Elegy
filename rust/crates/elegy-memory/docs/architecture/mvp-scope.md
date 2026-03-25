# MVP Scope

> **This document is the source of truth for what to implement.** If it's not marked MVP, don't implement it. Create the type/trait skeleton with `todo!()` instead.

## Milestone Definitions

- **MVP** — Implement fully. Working code with tests. Used by the two co-founders daily.
- **v1** — Trait/struct/table skeleton exists. Implementation is `todo!()` or no-op. Implement when MVP is stable and in daily use.
- **v2** — Documented in architecture docs. No code at all. Implement when v1 features are validated.

## Feature Matrix

### Storage

| Feature | Milestone | Notes |
|---------|-----------|-------|
| SQLite + rusqlite (bundled) | **MVP** | Single backend, all tables created |
| sqlite-vec virtual table | **MVP** | KNN search working |
| FTS5 virtual table | **MVP** | Keyword search working |
| Hybrid search (vector + FTS5) | **MVP** | Weighted combination |
| `MemoryStore` trait definition | **MVP** | Full trait with all methods |
| `SqliteMemoryStore` implementation | **MVP** | All CRUD + search + purge |
| `PgMemoryStore` trait skeleton | **v1** | Trait exists, impl = `todo!()` |
| Schema migrations | **MVP** | Version-based, idempotent |
| Multi-tenant schema (tenant_id) | **v1** | Column exists in MVP, unused |

### Scopes

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Single global scope | **MVP** | One .db file, scope column = 'global' |
| Session scope (JSON) | **v1** | In-memory JSON, purgeable |
| Workspace scope (per-workspace .db) | **v1** | Separate files per workspace |
| User scope | **v1** | Separate user.db |
| Agent scope | **v1** | Separate agent.db |
| Scope Promotion | **v2** | Auto-promote recurring memories cross-scope |

### Memory Model

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `Memory` struct with all fields | **MVP** | All fields present, even if some unused |
| `MemoryType` enum | **MVP** | All variants defined |
| `ProvenanceLevel` enum + base_reliability() | **MVP** | All variants and scoring |
| `SensitivityLevel` enum | **MVP** | Field exists, always Low in MVP |
| `MemoryState` enum (Active/Dormant/Deleted) | **MVP** | States exist, Dormant transition manual only |
| `MemoryMetadata` (tags, status, custom) | **MVP** | Stored as JSON in SQLite |

### Scoring & Retrieval

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Cosine similarity via sqlite-vec | **MVP** | Core retrieval mechanism |
| BM25 via FTS5 | **MVP** | Keyword fallback |
| Combined scoring formula | **MVP** | α × similarity + β × recency + γ × log(access + 1) + δ × priority |
| Configurable weights (α, β, γ, δ) | **MVP** | In `scope_config` table |
| Recency decay (Ebbinghaus) | **MVP** | Fixed λ, not adaptive |
| Adaptive Decay Rate | **v1** | λ adjusts to user frequency |
| Memory Type-Modulated Decay | **v1** | Different λ_base per type |
| Context window budget (ratio) | **MVP** | `memory_context_ratio` config |

### Confidence Score

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Importance score (stored) | **MVP** | LLM provides at extraction |
| Reliability score (computed from provenance) | **MVP** | Base = provenance level |
| Corroboration bonus | **v1** | +0.1 per corroboration |
| Contradiction penalty | **v1** | -0.3 on contradiction |
| Priority = importance × reliability | **MVP** | Used in retrieval scoring |

### Write-Time Gate

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `SalienceGate` trait | **MVP** | Full trait defined |
| Novelty check (semantic dedup, cosine > 0.92) | **MVP** | Critical for anti-bloat |
| Salience check (importance > 0.2 → active) | **MVP** | Low-importance → dormant |
| Provenance check (agent-inferred + low importance → dormant) | **MVP** | Prevents inference pollution |
| Configurable thresholds | **MVP** | In `scope_config` |

### Contradiction Journal

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `contradictions` table | **MVP** | Table created |
| Manual contradiction recording | **MVP** | API to record a contradiction |
| Automatic contradiction detection at write time | **v1** | LLM-based or heuristic |
| Contradiction resolution workflow | **v1** | User-facing resolution |

### Memory Versioning

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `memory_versions` table | **MVP** | Table created |
| Auto-versioning on content update | **MVP** | Every `update_content()` creates a version |
| Version history query | **MVP** | List versions by memory_id |
| Rollback to previous version | **v1** | Restore old content |

### Embedding

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `EmbeddingProvider` trait | **MVP** | Full trait |
| Single provider implementation | **MVP** | OpenAI OR Ollama, whichever you use |
| `embedding_stale` flag | **MVP** | Set on content update |
| Batch re-embedding of stale memories | **MVP** | CLI command or function |
| Multiple provider implementations | **v1** | OpenAI + Ollama + Voyage |

### Consolidation

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `MemoryConsolidator` trait | **MVP** | Trait defined |
| Simple dedup consolidator | **MVP** | Merge memories with cosine > 0.92 |
| LLM-based consolidation (sleep-time) | **v1** | LLM reviews and summarizes |

### Observability

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `MemoryHealthReport` struct | **MVP** | All fields |
| `health_report()` implementation | **MVP** | Counts, sizes, stale count |
| Contradiction listing | **MVP** | `list_contradictions()` |
| Export (JSON) | **MVP** | Full export of scope |
| Export (SQLite) | **v1** | `.elegy` portable format |
| Import | **v1** | Import from `.elegy` file |
| Selective scope export | **v1** | Choose which scopes to include |

### Purge & Privacy

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `purge_all()` | **MVP** | Delete everything |
| `purge_user(user_id)` | **v1** | GDPR-compliant per-user purge |

### CLI

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `add` — add a memory | **MVP** | With all required fields |
| `search` — hybrid search | **MVP** | Query string, returns scored results |
| `list` — list with filters | **MVP** | By state, type, provenance |
| `inspect` — show a memory + versions | **MVP** | Full detail view |
| `purge` — purge all or by filter | **MVP** | With confirmation prompt |
| `health` — show health report | **MVP** | Stats output |
| `export` — export to JSON | **MVP** | Stdout or file |
| `import` — import from file | **v1** | From JSON or .elegy |
| `consolidate` — run consolidation | **v1** | Trigger manual consolidation |
| `reembed` — re-embed stale memories | **MVP** | Batch re-embedding |
| `contradictions` — list contradictions | **MVP** | Show unresolved |

### Memory Links

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `memory_links` table | **MVP** | Table created |
| `supersedes` links on update | **MVP** | Created automatically by versioning |
| Manual link creation | **v1** | API to link memories |
| Graph traversal queries | **v2** | Multi-hop reasoning |

### Forgetting Budget

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Budget config per scope | **MVP** | In `scope_config`, unused in MVP |
| Automatic dormant transition at budget | **v1** | Lowest retention → dormant |
| Hard delete at storage cap | **v1** | Oldest dormant → deleted |

### Security

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Input validation on all writes | **MVP** | Content length, field bounds |
| Row-level security (multi-tenant) | **v1** | PostgreSQL only |
| Memory poisoning detection | **v2** | Anomaly detection on writes |

### Advanced (v2)

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Memory Portability Format (.elegy) | **v2** | Standardized export/import |
| Cross-Agent Memory Sharing Protocol | **v2** | Read-shared, write-isolated |
| User Correction Feedback Loop | **v2** | Auto-learn from corrections |
| Parameter Learning (regression on usage logs) | **v2** | Use access_count + retrieval-but-not-used signals to auto-tune α β γ δ and gate thresholds |
| Knowledge Graph migration (Neo4j/FalkorDB) | **v2** | If memory_links aren't enough |

## MVP Summary

The MVP is a **single SQLite file with all tables**, a **single embedding provider**, a **working write-time gate**, **hybrid search**, **basic scoring**, **versioning on update**, and a **CLI**. Two people use it daily. It's simple, it works, and it doesn't bloat.

Everything else is a skeleton that compiles but doesn't run.

### Note on Thresholds

All thresholds in this architecture (cosine similarity 0.92, salience minimum 0.2, scoring weights α=0.4 β=0.25 γ=0.15 δ=0.2, decay λ values, forgetting budget sizes) are conservative initial values stored in the `scope_config` table. They are NOT hardcoded constants. A calibration cycle based on real usage data is planned for v1 — specifically, logged retrieval data (which memories are actually used by the agent after retrieval vs ignored) will be used to adjust scoring weights via regression analysis. This is documented as a v2 feature: "Parameter Learning".

