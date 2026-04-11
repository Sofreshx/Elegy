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
| Adaptive Decay Rate | **v1** | Still future |
| Memory Type-Modulated Decay | **v1** | Still future |
| Context window budget (ratio) | **MVP** | Implemented via `MemoryContextConfig` |

### Confidence Score

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Importance score (stored) | **MVP** | Provided at extraction/import time |
| Reliability score (seeded from provenance) | **MVP** | Stored on write from `base_reliability()` |
| Corroboration bonus | **v1** | Counter exists, bonus logic still future |
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
| Rollback to previous version | **v1** | Still future |

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
| Export (SQLite / `.elegy`) | **v1** | Still future |
| Import | **v1** | Implemented now for JSON export-shape and simplified JSON inputs |
| Selective scope export | **v1** | Implemented now as exact-scope export by default plus `export --all-scopes` for aggregate export |

### Purge & Privacy

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `purge_all()` | **MVP** | Implemented |
| `purge_user(user_id)` | **v1** | Still an explicit stub |

### CLI

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `add` | **MVP** | Gate-aware memory creation |
| `search` | **MVP** | Hybrid search |
| `list` | **MVP** | Filtered listing |
| `inspect` | **MVP** | Memory + version history |
| `purge` | **MVP** | Confirmation flow |
| `health` | **MVP** | Base report plus average importance, stale previews, contradiction previews, oldest age, most-accessed memory, human-readable DB size |
| `export` | **MVP** | JSON to stdout or file |
| `import` | **v1** | Implemented now for JSON file/stdin inputs; full export-shape imports preserve exported state and `--force` still bypasses the gate |
| `reembed` | **MVP** | Batch re-embedding |
| `contradictions` list | **MVP** | Shows unresolved contradictions |
| `contradictions resolve` | **v1** | Implemented now; keep-one makes the losing memory dormant, keep-both leaves both active |
| `consolidate` | **v1** | Implemented now with simple dedup; optional `--cross-scope` lifts results to the highest scope in the pair |
| `promote` | **v1** | Implemented now for automatic and manual scope promotion |

### Memory Links

| Feature | Milestone | Notes |
|---------|-----------|-------|
| `memory_links` table | **MVP** | Table created |
| `supersedes` links on update | **MVP** | Still the intended versioning relation |
| Manual link creation | **v1** | Still future |
| Graph traversal queries | **v2** | Still future |

### Forgetting Budget

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Budget config per scope | **MVP** | Present in configuration / health usage ratio |
| Automatic dormant transition at budget | **v1** | Still future |
| Hard delete at storage cap | **v1** | Still future |

### Security

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Input validation on writes | **MVP** | Implemented on candidates, store writes, import, and CLI args |
| Row-level security (multi-tenant) | **v1** | Still future / PostgreSQL-oriented |
| Memory poisoning detection | **v2** | Still future |

### Advanced (v2)

| Feature | Milestone | Notes |
|---------|-----------|-------|
| Memory Portability Format (.elegy) | **v2** | Still future |
| Cross-Agent Memory Sharing Protocol | **v2** | Still future |
| User Correction Feedback Loop | **v2** | Still future |
| Parameter Learning | **v2** | Still future |
| Knowledge Graph migration | **v2** | Still future |

## Current Baseline Summary

The codebase still has an MVP core: SQLite storage, hybrid search, working gate, versioning, export, re-embedding, and CLI flows. It now also includes several v1-tier features that were not present in the original MVP baseline:

- OpenAI and Ollama embedding providers
- OpenAI and Ollama LLM providers
- JSON import
- heuristic contradiction detection
- optional LLM contradiction classification
- manual contradiction resolution
- richer health reporting
- optional LLM-backed consolidation
- automatic and manual scope promotion
- exact-scope export plus `--all-scopes`

These are implemented now, but they should still be read as first-pass v1 behavior rather than fully expanded end-state platform features.

### Note on Thresholds

Current default gate thresholds are stored in `scope_config` and default to:

- likely-duplicate warning floor: `0.80`
- merge threshold: `0.85`
- duplicate threshold constant: `0.99`
- salience threshold: `0.20`
- agent-inferred archive threshold: `0.50`

Retrieval scoring weights remain configurable (`0.40 / 0.25 / 0.15 / 0.20` by default). Thresholds are architecture defaults, not a claim that all higher-order tuning work is finished.

