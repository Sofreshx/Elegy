# AGENTS.md — Elegy Project

> Universal instructions for any AI agent working on this codebase (GitHub Copilot, Claude Code, OpenAI Codex, Cursor, Windsurf, etc.)

## Project Identity

- **Name:** Elegy
- **Language:** Rust (latest stable)
- **Type:** Modular AI agent infrastructure — a collection of independent crates
- **Active system:** `elegy-memory` (memory engine for LLM agents)

## First Steps for Any Agent

1. Read `rust/crates/elegy-memory/docs/architecture/ARCHITECTURE.md` for the full system overview.
2. Read `rust/crates/elegy-memory/docs/architecture/mvp-scope.md` to know what to implement and what to leave as stubs.
3. Read the specific architecture doc for the system you're working on before writing code.

## Architecture Docs Index

| File | Purpose |
|------|---------|
| `rust/crates/elegy-memory/docs/architecture/ARCHITECTURE.md` | Start here. Overview, philosophy, system map. |
| `rust/crates/elegy-memory/docs/architecture/memory-model.md` | Memory types, scopes, scoring, decay, confidence scores. |
| `rust/crates/elegy-memory/docs/architecture/storage-schema.md` | SQLite schema, tables, indexes, FTS5, sqlite-vec setup. |
| `rust/crates/elegy-memory/docs/architecture/traits-and-interfaces.md` | All Rust traits, their contracts, method signatures. |
| `rust/crates/elegy-memory/docs/architecture/mvp-scope.md` | MVP vs v1 vs v2 feature matrix. The source of truth for scope. |

## Non-Negotiable Rules

1. **Trait-first.** Define behavior as traits. Implementations are pluggable. Never hardcode concrete types in function signatures where a trait bound works.
2. **MVP discipline.** If `mvp-scope.md` says a feature is v1 or v2, write the type/trait skeleton with `todo!()` or no-op. Do not implement it.
3. **No raw transcripts.** Memories are distilled (summaries, facts, decisions). Never store full conversation text.
4. **Provenance is mandatory.** Every memory has a `ProvenanceLevel`. No memory without provenance.
5. **Write-time gate.** Every memory passes through the salience gate before storage. No bypass.
6. **Embeddings can fail.** Handle gracefully. Mark as stale. Never block writes on embedding computation.
7. **Scopes are isolated.** Session ≠ Workspace ≠ User ≠ Agent. No implicit cross-scope queries.
8. **Test everything public.** Every public function needs at least one test.
9. **Document everything public.** Doc comments on all public items.
10. **Minimize dependencies.** Core deps: rusqlite (bundled), sqlite-vec, serde, chrono, uuid, clap.

## Code Style

- `snake_case` functions/variables, `PascalCase` types/traits, `SCREAMING_SNAKE_CASE` constants.
- `thiserror` for library errors, `anyhow` for CLI errors.
- No `unwrap()` in library code. Use `?` or explicit error handling.
- Prefer returning `Result<T, E>` over panicking.
- Group imports: std → external crates → internal modules, separated by blank lines.

## Git Workflow

- Keep `dev` clean and fast-forwardable to `origin/dev`.
- Do day-to-day work on a personal branch (for example `roro` or `roro/<topic>`), not directly on `dev`.
- Merge to `dev` only after the relevant validation passes and the work is ready.
- Never force-push or rewrite history on `dev`.

## Crate Structure

```
rust/crates/elegy-memory/
├── Cargo.toml
├── docs/
│   └── architecture/
│       ├── ARCHITECTURE.md
│       ├── memory-model.md
│       ├── storage-schema.md
│       ├── traits-and-interfaces.md
│       └── mvp-scope.md
├── src/
│   ├── lib.rs              # Public API re-exports and core memory record types
│   ├── main.rs             # CLI entrypoint
│   ├── cli.rs              # CLI commands and output formatting
│   └── local_store.rs      # Local artifact-backed memory store implementation
└── tests/
    ├── cli.rs
    ├── governed_memory.rs
    └── local_store.rs
```
