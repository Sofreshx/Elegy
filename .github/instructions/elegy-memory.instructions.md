---
applyTo: "rust/crates/elegy-memory/**"
---

# elegy-memory — System-Specific Instructions

## What This System Does

elegy-memory is a **standalone memory engine for LLM agents**. It stores, retrieves, scores, and manages memories across sessions with semantic search, write-time filtering, and configurable decay. It is NOT a chatbot, NOT an agent framework, NOT a UI. It is a library (crate) + CLI.

> Current repo layout note: the crate currently lives at `rust/crates/elegy-memory/` and exposes `src/lib.rs`, `src/main.rs`, `src/cli.rs`, and `src/local_store.rs`. Treat the architecture docs under `rust/crates/elegy-memory/docs/architecture/` as the source of truth for planned types and traits referenced below.

## Key Design Decisions

- **Storage:** SQLite + sqlite-vec (vector search) + FTS5 (keyword search). Single file per scope.
- **Scopes:** Session (JSON, ephemeral), Workspace (SQLite per workspace), User (global SQLite), Agent (procedural SQLite).
- **Scoring:** `score = α × similarity + β × recency + γ × log(access_count + 1) + δ × importance`. Weights are configurable.
- **Confidence Score:** Each memory has `importance_score` (LLM-assigned, 0-1) AND `reliability_score` (system-computed, 0-1). Priority = importance × reliability.
- **Write-Time Gate:** 3-step filter before any write: (1) Novelty/dedup check cosine > 0.92, (2) Salience check importance > 0.2, (3) Provenance check — agent-inferred + low importance → dormant.
- **Decay:** Ebbinghaus-inspired. `retention = importance × e^(-λ × days) × (1 + 0.2 × access_count)`. λ adapts to user activity rate.
- **States:** Active (normal retrieval), Dormant (excluded from default retrieval, reactivatable), Deleted (hard purge only at storage cap).

## Types to Know (see `rust/crates/elegy-memory/docs/architecture/memory-model.md` and `rust/crates/elegy-memory/src/lib.rs`)

- `Memory` — core struct with all fields
- `MemoryScope` — enum: Session, Workspace, User, Agent
- `MemoryType` — enum: Fact, Preference, Decision, Procedure, Observation
- `ProvenanceLevel` — enum: UserStated(1.0), AgentObserved(0.8), AgentInferred(0.5), Consolidated(0.7), Imported(0.6)
- `SensitivityLevel` — enum: Low, Medium, High, Critical
- `MemoryState` — enum: Active, Dormant, Deleted
- `MemoryMetadata` — custom tags, status, extensible HashMap

## Traits to Know (see `rust/crates/elegy-memory/docs/architecture/traits-and-interfaces.md`)

- `MemoryStore` — CRUD + search + health report + purge. Architecture target implementations: `SqliteMemoryStore` (MVP), `PgMemoryStore` (v1). The current repository implementation centers on `rust/crates/elegy-memory/src/local_store.rs`.
- `EmbeddingProvider` — `embed(text) -> Vec<f32>`. Implementations: OpenAI (MVP), Ollama (v1).
- `MemoryConsolidator` — `consolidate(memories) -> consolidated`. MVP: dedup by similarity. v1: LLM-based.
- `SalienceGate` — `evaluate(candidate) -> GateDecision` (Accept/Archive/Merge).
- `MemoryObservability` — health reports, contradiction listing, export, purge.

## Common Pitfalls

- Do NOT store raw conversation transcripts. Only distilled memories (summaries, facts, decisions).
- Do NOT skip the write-time gate. Every `store()` call must go through `SalienceGate::evaluate()` first.
- Do NOT delete memories on temporal criteria alone. Use dormant state. Hard delete only at storage cap.
- Do NOT assume a single embedding provider. Always go through the `EmbeddingProvider` trait.
- Do NOT forget to set `embedding_stale = true` when updating memory content.
- Do NOT mix scopes in queries. Each scope is a separate storage unit.

