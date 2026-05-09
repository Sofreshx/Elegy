# CLAUDE.md — Elegy

This file is for Claude Code. For universal instructions, see AGENTS.md.

## Context

Elegy is a Rust workspace building modular AI agent infrastructure. Each crate is a standalone tool that agents can use independently or compose together.

Active crates:
- `rust/crates/elegy-memory/` — standalone memory engine (SQLite + sqlite-vec + FTS5)
- `rust/crates/elegy-memory-mcp/` — remote MCP server exposing elegy-memory (axum, JWT, OAuth 2.1)

Future crates may include: MCP generation tools, user observation/skill crystallization, agent workflow capture, and others. Each follows the same pattern: standalone, trait-first, well-documented.

## Before Coding

1. Identify which crate you're working on
2. Read its `docs/architecture/ARCHITECTURE.md` first
3. Read `mvp-scope.md` to know what's in scope vs deferred
4. Read the specific doc for whatever you're implementing

For elegy-memory specifically:
@rust/crates/elegy-memory/docs/architecture/ARCHITECTURE.md
@rust/crates/elegy-memory/docs/architecture/memory-model.md
@rust/crates/elegy-memory/docs/architecture/traits-and-interfaces.md
@rust/crates/elegy-memory/docs/architecture/storage-schema.md
@rust/crates/elegy-memory/docs/architecture/mvp-scope.md

## Key Constraints

- Rust stable. No nightly.
- Trait-first design. All behaviors behind traits.
- MVP scope is strict. Features marked v1/v2 get trait/struct skeletons with `todo!()`, not implementations.
- No `unwrap()` or `expect()` in library code. Use `thiserror` / `anyhow` / `?`.
- Every crate must have its own architecture docs before implementation starts.

### elegy-memory specific
- Every memory write goes through the salience gate. No exceptions.
- Every memory has mandatory provenance, importance score, and reliability score.
- SQLite only for MVP. PostgreSQL is v1.
- Embeddings are async-compatible. Mark stale on content update. Never block writes.

## Build and Verify

```
cargo test -p <crate-name>
cargo clippy -p <crate-name> -- -D warnings
```

Run both after every significant change. Fix before continuing.

## Structural Invariants — STOP

Never modify without explicit human confirmation:
- DB schema (tables, columns in any `schema.rs`)
- Public traits (any `traits.rs`)
- Serialization / on-disk storage formats
- Public API surface of any crate

If you need to touch these, STOP, explain why, wait for confirmation.

## Flight Recorder

After each completed unit of work, append a summary to `FLIGHT_RECORDER.md`.
Include: date, what changed, test status, decisions made.

## Git

- Working branch: `dev`
- Atomic commits: one logical change = one commit
- Never push to `main`
- Never force-push or rewrite history on `dev`

## Do NOT

- Modify files outside the requested scope
- Add dependencies without asking
- Write tests that depend on execution order
- Create temp files at project root
- Implement features marked as v1/v2 beyond trait skeletons
