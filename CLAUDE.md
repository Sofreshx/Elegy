# CLAUDE.md — Elegy Project

This file is for Claude Code. For universal instructions, see AGENTS.md.

## Context

Elegy is a Rust project building modular AI agent infrastructure. The active system is `elegy-memory`, a standalone memory engine for LLM agents. It uses SQLite + sqlite-vec + FTS5 for storage and semantic search.

## Before Coding

Read these docs in order:
1. `rust/crates/elegy-memory/docs/architecture/ARCHITECTURE.md` — system overview
2. `rust/crates/elegy-memory/docs/architecture/mvp-scope.md` — what to build now vs later
3. The specific doc for whatever you're implementing

## Key Constraints

- Rust, latest stable. No nightly features.
- Trait-first design. All behaviors behind traits (`MemoryStore`, `EmbeddingProvider`, `SalienceGate`, `MemoryConsolidator`).
- MVP scope is strict. Features marked v1/v2 in `rust/crates/elegy-memory/docs/architecture/mvp-scope.md` get trait/struct skeletons with `todo!()`, not implementations.
- Every memory write goes through the salience gate. No exceptions.
- Every memory has mandatory provenance, importance score, and reliability score.
- SQLite is the only storage backend for MVP. PostgreSQL is v1.
- Embeddings are async-compatible. Mark stale on content update. Never block writes.
- No `unwrap()` in library code. Use `thiserror` for errors.

## Project Layout

The architecture is documented in `rust/crates/elegy-memory/docs/architecture/`. Start with `rust/crates/elegy-memory/docs/architecture/ARCHITECTURE.md`. The current crate lives in `rust/crates/elegy-memory/` and currently exposes `src/lib.rs`, `src/main.rs`, `src/cli.rs`, and `src/local_store.rs` plus integration tests in `rust/crates/elegy-memory/tests/`.

## Testing

- `cargo test` must pass at all times.
- Unit tests in the same file as the code (`#[cfg(test)] mod tests`).
- Integration tests in `rust/crates/elegy-memory/tests/`.
- Use temp directories for SQLite test databases.

