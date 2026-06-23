# Elegy — Architecture Documentation

> Last updated: 2026-07-25 | Status: MVP complete; v1 and v2 features implemented, with future work focused on knowledge-graph migration and PostgreSQL

## What is Elegy?

Elegy is a modular AI agent infrastructure project. It provides independent systems (Rust crates) that help LLM agents remember, learn, and improve over time. Each system is usable standalone or composed with others.

## Active Systems

| System | Crate | Status | Description |
|--------|-------|--------|-------------|
| **elegy-memory** | `rust/features/elegy-memory` | 🟢 MVP complete + implemented v1/v2 features | Memory engine for LLM agents — storage, retrieval, scoring, decay, consolidation, corrections, learning, sharing, and safety workflows |

## Design Philosophy

1. **Trait-first.** Core behaviors are traits. Implementations are pluggable. Switch SQLite for PostgreSQL, OpenAI embeddings for Ollama, without touching business logic.
2. **Write-time quality.** Filter at write time, not read time. Bad memories never enter the active store. This is structurally superior to read-time filtering (validated by research: write-time gating maintains 100% accuracy at 8:1 distractor ratios where read-time collapses to 0%).
3. **Archive, don't delete.** Memories are deprioritized (dormant), not destroyed. Biological memory works the same way — forgetting is deprioritization, not erasure. Hard deletes only at storage caps.
4. **Provenance is truth.** Every memory carries its origin (user-stated, agent-observed, agent-inferred, consolidated, imported). Provenance determines trust. A user's direct statement always outweighs an agent's inference.
5. **Scopes isolate context.** Session, Workspace, User, and Agent memories live in separate stores. No cross-contamination. Explicit APIs for cross-scope queries.
6. **Grand public, not personal tool.** Elegy targets developers, techniciens, and non-technical professionals. It must be embeddable (SQLite for local), scalable (PostgreSQL for cloud), privacy-compliant (GDPR purge), and ergonomic.

## Architecture Docs

Read these in order for a complete understanding:

### 1. [Memory Model](memory-model.md)
Core concepts: what is a memory, what are scopes, how scoring works, how decay works, the confidence score system, memory types, provenance hierarchy, write-time gating, and the contradiction journal.

### 2. [Storage Schema](storage-schema.md)
SQLite schema with all tables, columns, indexes, FTS5 setup, sqlite-vec virtual tables, and the migration strategy. Also covers the PostgreSQL schema for v1.

### 3. [Traits and Interfaces](traits-and-interfaces.md)
Every Rust trait definition with method signatures, contracts, error types, and implementation notes. This is the API contract between components.

### 4. [MVP Scope](mvp-scope.md)
The feature matrix and current baseline summary. It distinguishes the MVP baseline from v1/v2 maturity labels while also calling out what is already implemented in the current crate. **This is the source of truth for what to build and how the present implementation maps to those milestones.**

## Key Research References

This architecture is informed by peer-reviewed research and state-of-the-art systems:

- **Mem0** (Chhikara et al., 2025) — Two-phase extract/update pipeline, CRUD operations, LOCOMO benchmark
- **A-MEM** (Xu et al., 2025, NeurIPS 2025) — Zettelkasten-inspired dynamic memory organization
- **Letta/MemGPT** (Packer et al., 2023) — OS-inspired memory hierarchy, sleep-time compute for consolidation
- **Write-Time Gating** (arXiv 2603.15994, March 2025) — Salience gate with hierarchical archiving, 100% accuracy vs 13% ungated
- **MaRS** (arXiv 2512.12856, December 2025) — Sensitivity-weighted retention, privacy-aware budgeting
- **Hindsight** (arXiv 2512.12818, January 2026) — Four logical networks separating facts/experiences/opinions/observations
- **Zep/Graphiti** (Rasmussen et al., 2025) — Temporal knowledge graph for agent memory
- **Ebbinghaus Forgetting Curve** — Exponential decay modulated by importance and recall frequency

## Original Contributions

These concepts are novel to Elegy, not found as-is in existing literature:

- **Scope Promotion** — Automatic promotion of memories from session→workspace→user based on cross-scope recurrence
- **Adaptive Decay Rate** — λ adjusts to user activity frequency, preventing premature forgetting for infrequent users
- **Contradiction Journal** — Explicit log of detected contradictions for human or agent resolution
- **Confidence Score Bidirectionnel** — importance (LLM-assigned) × reliability (system-computed from provenance, corroboration, contradiction)
- **Memory Type-Modulated Decay** — Decay rate varies by memory type (facts don't decay; observations do)
- **Memory Portability Format** — `.elegy` export/import format with selective scope inclusion
- **Embedding Staleness Detection** — Flag + batch re-embed on content mutation

