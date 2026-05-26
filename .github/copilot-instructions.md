# Elegy Project — Copilot Instructions

## Project Overview

Elegy is a modular AI agent infrastructure project written in **Rust**. It combines governed contracts, CLI tools, v2 skill definitions, and an MCP host so agentic harnesses can discover and invoke local capabilities safely.

## Architecture Documentation

Before writing any code, read the relevant architecture docs:

- `docs/agent-integration.md` — Agent-facing discovery, invocation, JSON envelopes, and MCP tool dispatch.
- `rust/crates/elegy-memory/docs/architecture/ARCHITECTURE.md` — Start here. Overview of all systems, design philosophy, and navigation guide.
- `rust/crates/elegy-memory/docs/architecture/memory-model.md` — Memory types, scopes, scoring formulas, and behavioral rules.
- `rust/crates/elegy-memory/docs/architecture/storage-schema.md` — SQLite schema, tables, indexes, and migration strategy.
- `rust/crates/elegy-memory/docs/architecture/traits-and-interfaces.md` — All Rust traits, their contracts, and implementation rules.
- `rust/crates/elegy-memory/docs/architecture/mvp-scope.md` — What is in MVP vs v1 vs v2. **Do not implement features outside the current milestone.**

## Coding Standards

- Language: Rust (latest stable edition).
- Error handling: Use `thiserror` for library errors, `anyhow` for binary/CLI errors. Never `unwrap()` in library code.
- Naming: snake_case for functions/variables, PascalCase for types/traits, SCREAMING_SNAKE_CASE for constants.
- Documentation: Every public type, trait, and function must have a doc comment explaining its purpose and contract.
- Testing: Every public function must have at least one unit test. Integration tests live in `rust/crates/elegy-memory/tests/`.
- Dependencies: Minimize external dependencies. Prefer `rusqlite` (bundled feature), `sqlite-vec`, `serde`, `chrono`, `uuid`, `clap` for CLI.
- No `unsafe` code without explicit justification in a comment.

## Agent Tool Discovery

- Prefer `elegy skills list/search/describe --json` when you need the current capability surface.
- V2 skill definitions in `contracts/fixtures/skill-definition-v2.*.json` are the supported skill contract.
- Do not reintroduce v1 `skill-definition.*.json` files.
- `elegy run` exposes the same v2 registry as MCP tools. Side-effecting tools are blocked by default unless dry-run input is provided or the host was started with `--allow-side-effects`.

## Architecture Rules — Critical

1. **Trait-first design.** All core behaviors are defined as traits (`MemoryStore`, `EmbeddingProvider`, `MemoryConsolidator`). Implementations are separate. Never hardcode a concrete type where a trait would work.
2. **MVP discipline.** Check `rust/crates/elegy-memory/docs/architecture/mvp-scope.md` before implementing anything. If a feature is marked v1 or v2, create the trait/struct/table but leave the implementation as a no-op or todo!().
3. **Write-time gating.** Every memory write passes through the salience gate (novelty check → salience check → provenance check) before being stored. Never bypass the gate.
4. **Scopes are isolated.** Session, Workspace, User, and Agent scopes have separate storage. Never cross-query scopes without explicit API calls.
5. **Embeddings are async-safe.** Embedding computation can fail or be slow. Always handle `embedding_stale` flag. Never block writes on embedding generation.
6. **Provenance is mandatory.** Every memory must have a provenance level. Default is `Imported` (lowest trust).

## Git Workflow

- Promotion chain: `<topic>` -> `roro` -> `dev` -> `main`
- Keep branch ancestry monotonic: `main` must remain an ancestor of `dev`, and `dev` must remain an ancestor of `roro`
- Do feature work on dedicated topic branches, not directly on `roro`, `dev`, or `main`
- Merge `dev` into `main` only after `dev` is clean and validated
- If a hotfix lands on `main`, propagate it back through `dev` and then `roro` before continuing feature work
- If any branch in the chain falls behind its upstream branch, reconcile downstream before starting more feature work
- After a complete promotion cycle, `main`, `dev`, and `roro` may all point to the same commit. This is the correct starting state for the next cycle
- Push promoted branches to `origin` only after the local promotion step is clean. Publish in chain order: `main`, `dev`, then `roro`
- The following `roro` rules apply only when the current branch is `roro`:
- Merge a topic branch into `roro` only after validation is clean
- Merge `roro` into `dev` only after `roro` is clean, validated, and reconciled with newer `main` changes

## File Organization

- `contracts/`, `governance/`, `schemas/`, and `policies/` hold the governed contract and policy surfaces.
- `rust/crates/` is the active Rust workspace and contains multiple first-party crates, including `elegy-memory`, `elegy-memory-mcp`, `elegy-host-mcp`, `elegy-skills`, `elegy-planning`, and related runtime/tooling crates.
- `src/Elegy-*` directories are wrapper or contributor-navigation surfaces, not the canonical Rust implementation roots.
