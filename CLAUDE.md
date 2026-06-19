# CLAUDE.md - Elegy

This file is for Claude Code. For universal instructions, see AGENTS.md.

## Context

Elegy is a Rust toolkit for shipping governed local CLI capabilities to AI-agent hosts.
Governed contracts and skill definitions are the durable authority. Rust implements
reusable executable behavior over those artifacts. CLI invocation templates are the
default agent boundary, while MCP is an optional projection for MCP-native clients.

## Before Coding

Read these docs in order:

1. `README.md` for the current public project shape and shipped surfaces
2. `docs/agent-integration.md` before changing host onboarding, discovery, invocation envelopes, profiles, or MCP projection
3. `docs/architecture/mcp-skill-tooling-placement.md` before changing MCP authoring, analysis, skill generation, portable plugin packages, or ownership boundaries
4. The smallest relevant architecture, ADR, or spec under `docs/`
5. For memory work, `rust/features/elegy-memory/docs/architecture/ARCHITECTURE.md` and `rust/features/elegy-memory/docs/architecture/mvp-scope.md`

For elegy-memory specifically:
@rust/features/elegy-memory/docs/architecture/ARCHITECTURE.md
@rust/features/elegy-memory/docs/architecture/memory-model.md
@rust/features/elegy-memory/docs/architecture/traits-and-interfaces.md
@rust/features/elegy-memory/docs/architecture/storage-schema.md
@rust/features/elegy-memory/docs/architecture/mvp-scope.md

## Discovery Surface

- Use `elegy agent check/manifest/discover --json` for host onboarding and profile-filtered progressive discovery.
- Use `elegy-skills list/search/resolve/get/capability/validate --json` or the umbrella `elegy skills ...` compatibility surface when developing or inspecting the governed skill registry.
- skill definitions in `contracts/fixtures/skill.*.json` are authoritative. Do not add v1 skill-definition files.
- `elegy run` is the optional MCP stdio adapter over governed capabilities. Side-effecting MCP calls require explicit dry-run input or a host started with `--allow-side-effects`.
- Profiles are allowlists, not approvals.

## Key Constraints

- Rust stable. No nightly.
- Trait-first design. All behaviors behind traits.
- MVP scope is strict. Features marked v1/v2 get trait/struct skeletons with `todo!()`, not implementations.
- No `unwrap()` or `expect()` in library code. Use `thiserror` / `anyhow` / `?`.
- Do not promote wrapper folders, generated bundles, or MCP projections into authority roots.
- New architecture docs are required when a change introduces a durable cross-crate decision or behavior contract. Do not create boilerplate docs for routine local edits.

### elegy-memory specific
- Every memory write goes through the salience gate. No exceptions.
- Every memory has mandatory provenance, importance score, and reliability score.
- SQLite only for MVP. PostgreSQL is v1.
- Embeddings are async-compatible. Mark stale on content update. Never block writes.

## Build and Verify

```
cd rust
cargo test -p <crate-name>
cargo clippy -p <crate-name> --all-targets --all-features -- -D warnings
```

Use the Rust CLI for governed exports and contract validation:

```bash
cd rust && cargo run -p elegy-cli -- contracts validate --project ..
cd rust && cargo test -p elegy-contracts --test conformance
```

## Structural Invariants — STOP

Never modify without explicit human confirmation:
- DB schema (tables, columns in any `schema.rs`)
- Public traits (any `traits.rs`)
- Serialization / on-disk storage formats
- Public API surface of any crate

If you need to touch these, STOP, explain why, wait for confirmation.

## Do NOT

- Modify files outside the requested scope
- Add dependencies without asking
- Write tests that depend on execution order
- Create temp files at project root
- Implement features marked as v1/v2 beyond trait skeletons
- Treat MCP as the primary integration contract when a CLI invocation template is the governed path
- Store raw transcripts in memory surfaces
