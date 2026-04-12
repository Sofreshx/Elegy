# MVP Scope

> Milestone labels describe target product maturity, not just whether code exists. Session 4 landed several first-pass v1 features in the codebase; those rows now call out current implementation status explicitly.

## Milestone Definitions

- **MVP** — required baseline behavior; implemented and expected to work in the current codebase.
- **v1** — beyond the original MVP bar. A feature may already have a first implementation, but it is still treated as v1-grade behavior rather than finished platform baseline.
- **v2** — documented future direction; not part of the current implementation baseline.

## Feature Matrix

### Storage

| Feature | Milestone | Notes |
|---------|-----------|-------|
| SQLite + rusqlite (bundled) | **MVP** | Single backend, all core tables created |
| sqlite-vec virtual table | **MVP** | Vector storage/search path working |
| FTS5 virtual table | **MVP** | Keyword search working |
| Hybrid search (vector + FTS5) | **MVP** | Vector similarity is blended ahead of final scoring |
| `MemoryStore` trait definition | **MVP** | Full async CRUD/search/health contract |
| `SqliteMemoryStore` implementation | **MVP** | CRUD, search, embeddings, contradictions, export support, purge_all |
| `PgMemoryStore` trait skeleton | **v1** | Still absent |
| Schema migrations | **MVP** | Version-based initialization |
| Multi-tenant schema (tenant_id) | **v1** | Fields exist, not enforced end-to-end |

### Scopes

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Selected scope per store instance | **MVP** | Store and CLI operate against one explicit `MemoryScope` at a time |
| Upward visibility search model | **v1** | Search / duplicate checks cascade to broader visible scopes |
| Session scope backend | **v1** | Session rows persist in the shared SQLite backend today, not a dedicated JSON backend |
| Workspace scope | **v1** | Implemented as a scope value and visible from `session` |
| User scope | **v1** | Implemented as a scope value and visible from `session` / `workspace` |
| Agent scope | **v1** | Implemented as a scope value and visible from all lower scopes |
| Scope Promotion | **v1** | Automatic and manual upward promotion are now implemented |

### Memory Model

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `Memory` struct with full persisted fields | **MVP** | Includes tags/status/custom metadata, counters, embedding freshness, ids |
| `MemoryType` enum | **MVP** | All variants defined |
| `ProvenanceLevel` enum + base reliability | **MVP** | `base_reliability()` is implemented |
| `SensitivityLevel` enum | **MVP** | Fully modeled |
| `MemoryState` enum | **MVP** | `Dormant` is now used by archive flows and contradiction keep-one resolution |

### Scoring & Retrieval

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Cosine similarity via stored embeddings | **MVP** | Core retrieval mechanism |
| BM25 via FTS5 | **MVP** | Keyword fallback / blend |
| Combined scoring formula | **MVP** | `α × similarity + β × recency + γ × ln(access + 1) + δ × (similarity × priority)` |
| Configurable weights (α, β, γ, δ) | **MVP** | Loaded from `scope_config` |
| Recency decay (fixed λ) | **MVP** | Fixed base lambda, not adaptive |
| Adaptive Decay Rate | **v1** | Implemented now with activity-rate scaling of lambda via `adaptive_retention()` |
| Memory Type-Modulated Decay | **v1** | Implemented now with per-type multipliers via `type_decay_multiplier()` (Procedure 0.7×, Fact 0.8×, Decision 0.85×, Preference 0.9×, Observation 1.2×) |
| Context window budget (ratio) | **MVP** | Implemented via `MemoryContextConfig` |

### Confidence Score

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Importance score (stored) | **MVP** | Provided at extraction/import time |
| Reliability score (seeded from provenance) | **MVP** | Stored on write from `base_reliability()` |
| Corroboration bonus | **v1** | Implemented now; +0.05 reliability per corroboration, capped at base_reliability + 0.2 |
| Contradiction penalty | **v1** | Implemented now when contradiction records are created |
| Priority = importance × reliability | **MVP** | Used in retrieval scoring |

### Write-Time Gate

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `SalienceGate` trait | **MVP** | Async contract |
| Novelty check with conservative warning band | **MVP** | Accepts as new below `0.85`; `0.80–0.85` returns a `similar_to` warning |
| Merge threshold | **MVP** | Current default is `0.85` |
| Exact-duplicate threshold constant | **MVP** | Config default is `0.99`; current gate still prefers conservative merge handling over aggressive reject |
| Salience check | **MVP** | `importance < 0.20` archives |
| Provenance check | **MVP** | `AgentInferred` with `importance < 0.50` archives |
| Configurable thresholds | **MVP** | Loaded from `scope_config` |
| LLM-assisted contradiction classification | **v1** | Implemented now as an optional pre-heuristic check with graceful fallback to the heuristic gate |

### Contradiction Journal

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `contradictions` table | **MVP** | Active and queried |
| Manual contradiction recording | **MVP** | Store API implemented |
| Automatic contradiction detection at write time | **v1** | Implemented now with conservative heuristics in the high-similarity merge branch |
| LLM contradiction classification at write time | **v1** | Implemented now as an optional enhancement before the heuristic branch |
| Contradiction resolution workflow | **v1** | Implemented now in CLI: list unresolved, resolve with keep-one or keep-both |

### Memory Versioning

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `memory_versions` table | **MVP** | Table created |
| Auto-versioning on content update | **MVP** | Every `update_content()` creates a version |
| Version history query | **MVP** | Exposed by `SqliteMemoryStore::list_versions()` |
| Rollback to previous version | **v1** | Implemented now via `rollback_to_version()` and CLI `rollback` command |

### Embedding

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `EmbeddingProvider` trait | **MVP** | Full async trait |
| Provider-backed embeddings | **MVP** | Working search/reembed flows |
| `LlmProvider` trait | **v1** | Implemented now for Ollama and OpenAI text-generation calls |
| `embedding_stale` flag | **MVP** | Set on content update and used by re-embed flows |
| Batch re-embedding of stale memories | **MVP** | CLI command implemented |
| Multiple provider implementations | **v1** | Implemented now with both Ollama and OpenAI; more providers remain future |

### Consolidation

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `MemoryConsolidator` trait | **MVP** | Trait defined |
| Simple dedup consolidator | **MVP** | Uses the configured merge threshold (default `0.85`) |
| LLM-based consolidation | **v1** | Implemented now as an optional CLI/runtime path with graceful fallback to the simple consolidator |

### Observability

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `MemoryHealthReport` struct | **MVP** | Base store health snapshot |
| `health_report()` implementation | **MVP** | Core counts/sizes plus richer CLI-derived health output |
| Contradiction listing | **MVP** | `list_contradictions()` implemented |
| Export (JSON) | **MVP** | Full scope export |
| Export (SQLite / `.elegy`) | **v1** | Implemented now; SQLite portable DB export and `.elegy` JSON archive with links + versions |
| Import | **v1** | Implemented now for JSON export-shape and simplified JSON inputs |
| Selective scope export | **v1** | Implemented now as exact-scope export by default plus `export --all-scopes` for aggregate export |

### Purge & Privacy

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `purge_all()` | **MVP** | Implemented |
| `purge_user(user_id)` | **v1** | Implemented now; deletes all memories, versions, links, corrections, and feedback for a user |

### CLI

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `add` | **MVP** | Gate-aware memory creation |
| `search` | **MVP** | Hybrid search |
| `list` | **MVP** | Filtered listing |
| `inspect` | **MVP** | Memory + version history |
| `purge` | **MVP** | Confirmation flow |
| `health` | **MVP** | Base report plus average importance, stale previews, contradiction previews, oldest age, most-accessed memory, human-readable DB size |
| `export` | **MVP** | JSON to stdout or file; `--export-format sqlite|elegy` for portable exports |
| `import` | **v1** | Implemented now for JSON file/stdin inputs; full export-shape imports preserve exported state and `--force` still bypasses the gate |
| `reembed` | **MVP** | Batch re-embedding |
| `contradictions` list | **MVP** | Shows unresolved contradictions |
| `contradictions resolve` | **v1** | Implemented now; keep-one makes the losing memory dormant, keep-both leaves both active |
| `consolidate` | **v1** | Implemented now with simple dedup; optional `--cross-scope` lifts results to the highest scope in the pair |
| `promote` | **v1** | Implemented now for automatic and manual scope promotion |
| `rollback` | **v1** | Implemented now; restores a memory to a specific version |
| `corroborate` | **v1** | Implemented now; records corroboration and boosts reliability |
| `budget` | **v1** | Implemented now; enforces active/dormant budget limits |
| `correct` | **v2** | Implemented now; user correction with version tracking and reliability boost |
| `feedback` | **v2** | Implemented now; records retrieval relevance feedback |
| `weights` | **v2** | Implemented now; computes learned scoring weights from feedback data |
| `traverse` | **v2** | Implemented now; BFS graph traversal with depth limit and relation filter |
| `detect-poisoning` | **v2** | Implemented now; runs 4 heuristic poisoning checks |
| `delete-link` | **v1** | Implemented now; removes a link by ID |
| `share-export` | **v2** | Implemented now; exports memories filtered by sensitivity and reliability for cross-agent sharing |
| `share-import` | **v2** | Implemented now; imports shared memories with fresh IDs and Imported provenance |

### Memory Links

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `memory_links` table | **MVP** | Table created |
| `supersedes` links on update | **MVP** | Still the intended versioning relation |
| Manual link creation | **v1** | Implemented now via `record_link()` and CLI `link create` |
| Link deletion | **v1** | Implemented now via `delete_link()` and CLI `delete-link` |
| Graph traversal queries | **v2** | Implemented now with BFS traversal, depth limit, and optional relation filter |

### Forgetting Budget

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Budget config per scope | **MVP** | Present in configuration / health usage ratio |
| Automatic dormant transition at budget | **v1** | Implemented now via `enforce_budget()` — lowest-scoring active memories transition to dormant |
| Hard delete at storage cap | **v1** | Implemented now via `enforce_budget()` — lowest-scoring dormant memories hard-deleted when over cap |

### Security

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Input validation on writes | **MVP** | Implemented on candidates, store writes, import, and CLI args |
| Row-level security (multi-tenant) | **v1** | Still future / PostgreSQL-oriented |
| Memory poisoning detection | **v2** | Implemented now with 4 heuristics: frequency analysis, trust mismatch, bulk overwrite, mass contradiction |

### Advanced (v2)

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Memory Portability Format (.elegy) | **v2** | Implemented now as JSON archive with memories, links, and version history |
| Cross-Agent Memory Sharing Protocol | **v2** | Implemented now with `export_for_sharing()` and `import_shared()`, sensitivity/reliability filtering |
| User Correction Feedback Loop | **v2** | Implemented now with `correct_memory()`, version tracking, and +0.1 reliability bump |
| Parameter Learning | **v2** | Implemented now with `record_feedback()` / `compute_learned_weights()` from retrieval relevance data |
| Knowledge Graph migration | **v2** | Still future; proto-graph links and BFS traversal provide the foundation |

## Current Baseline Summary

The codebase has a complete MVP core plus the full v1 and v2 feature set. Only Knowledge Graph migration and PostgreSQL backend remain as future work. Current implementation includes:

**MVP baseline:**
- SQLite storage, hybrid search (vector + FTS5), working gate, versioning, export, re-embedding, and CLI flows

**v1 features (all implemented):**
- OpenAI and Ollama embedding providers
- OpenAI and Ollama LLM providers
- JSON import with gate bypass
- Heuristic and optional LLM contradiction detection
- Manual contradiction resolution (keep-one, keep-both)
- Richer health reporting
- Optional LLM-backed consolidation
- Automatic and manual scope promotion
- Exact-scope export plus `--all-scopes`
- Adaptive decay rate and type-modulated decay
- Corroboration bonus (+0.05 reliability, capped)
- Version rollback
- Budget enforcement (active→dormant, dormant→delete)
- `purge_user` full implementation
- SQLite and `.elegy` portable export formats
- Manual link creation and deletion

**v2 features (all implemented except Knowledge Graph migration):**
- Graph traversal (BFS with depth limit and relation filter)
- Memory poisoning detection (4 heuristics)
- User correction feedback loop with version tracking
- Parameter learning from retrieval relevance feedback
- Cross-agent memory sharing (export/import with sensitivity filtering)
- 11 new CLI commands: rollback, corroborate, budget, correct, feedback, weights, traverse, detect-poisoning, delete-link, share-export, share-import

### Note on Thresholds

Current default gate thresholds are stored in `scope_config` and default to:

- likely-duplicate warning floor: `0.80`
- merge threshold: `0.85`
- duplicate threshold constant: `0.99`
- salience threshold: `0.20`
- agent-inferred archive threshold: `0.50`

Retrieval scoring weights remain configurable (`0.40 / 0.25 / 0.15 / 0.20` by default). Thresholds are architecture defaults, not a claim that all higher-order tuning work is finished.

