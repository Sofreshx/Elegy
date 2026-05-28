# CLAUDE.md — Elegy

This file is for Claude Code. For universal instructions, see AGENTS.md.

## Context

Elegy is a Rust project building modular AI agent infrastructure: governed contracts, CLI tools, v2 skill definitions, and an MCP host for runtime discovery and tool invocation. `elegy-memory` remains an important MVP subsystem, and `elegy-memory-mcp` remains an important integration surface, but current agent-facing work spans the umbrella CLI, contracts, skill generation, and MCP host.

## Before Coding

Read these docs in order:
1. `docs/agent-integration.md` — agent discovery and invocation model
2. The specific doc for whatever you're implementing
3. For memory work, `rust/crates/elegy-memory/docs/architecture/ARCHITECTURE.md`
4. For memory scope decisions, `rust/crates/elegy-memory/docs/architecture/mvp-scope.md`

For elegy-memory specifically:
@rust/crates/elegy-memory/docs/architecture/ARCHITECTURE.md
@rust/crates/elegy-memory/docs/architecture/memory-model.md
@rust/crates/elegy-memory/docs/architecture/traits-and-interfaces.md
@rust/crates/elegy-memory/docs/architecture/storage-schema.md
@rust/crates/elegy-memory/docs/architecture/mvp-scope.md

## Discovery Surface

- Use `elegy skills list/search/describe --json` as the first stop for agent-facing capability discovery.
- V2 skill definitions in `contracts/fixtures/skill-definition-v2.*.json` are authoritative. Do not add v1 skill-definition files.
- `elegy run` serves MCP resources and tool calls from the same built-in v2 registry.
- Side-effecting MCP tool calls require dry-run input or a host started with `--allow-side-effects`.

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

- Promotion chain: `<topic>` -> `roro` -> `dev` -> `main`
- Keep branch ancestry monotonic: `main` must remain an ancestor of `dev`, and `dev` must remain an ancestor of `roro`
- Do normal feature work on dedicated topic branches, not directly on `roro`, `dev`, or `main`
- Merge `dev` into `main` only when `dev` is clean and validated
- If a hotfix lands on `main`, propagate it back through `dev`, then `roro`, before continuing feature work
- If any branch in the chain falls behind its upstream branch, reconcile downstream before starting more feature work
- After a complete promotion cycle, `main`, `dev`, and `roro` may all point to the same commit. This is the correct starting state for the next cycle
- After a clean local promotion cycle, push `main`, `dev`, and `roro` to `origin` immediately so the remote stays aligned with the validated local state. Prefer a single atomic push when available
- The following `roro` rules apply only when the current branch is `roro`:
- Merge a topic branch into `roro` only when the work and validation are clean
- Merge `roro` into `dev` only when `roro` is clean, validated, and reconciled with newer `main` changes
- Atomic commits: one logical change = one commit
- Never force-push or rewrite history on `main` or `dev`

## Do NOT

- Modify files outside the requested scope
- Add dependencies without asking
- Write tests that depend on execution order
- Create temp files at project root
- Implement features marked as v1/v2 beyond trait skeletons
