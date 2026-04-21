# CLAUDE.md — Elegy Project

This file is for Claude Code. For universal instructions, see AGENTS.md.

## Context

Elegy is a Rust project building modular AI agent infrastructure: governed contracts, CLI tools, v2 skill definitions, and an MCP host for runtime discovery and tool invocation. `elegy-memory` remains an important MVP subsystem, but current agent-facing work spans the umbrella CLI, contracts, skill generation, and MCP host.

## Before Coding

Read these docs in order:
1. `docs/agent-integration.md` — agent discovery and invocation model
2. The specific doc for whatever you're implementing
3. For memory work, `rust/crates/elegy-memory/docs/architecture/ARCHITECTURE.md`
4. For memory scope decisions, `rust/crates/elegy-memory/docs/architecture/mvp-scope.md`

## Discovery Surface

- Use `elegy skills list/search/describe --json` as the first stop for agent-facing capability discovery.
- V2 skill definitions in `contracts/fixtures/skill-definition-v2.*.json` are authoritative. Do not add v1 skill-definition files.
- `elegy run` serves MCP resources and tool calls from the same built-in v2 registry.
- Side-effecting MCP tool calls require dry-run input or a host started with `--allow-side-effects`.

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

