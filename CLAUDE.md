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
5. For memory work, `rust/crates/elegy-memory/docs/architecture/ARCHITECTURE.md` and `rust/crates/elegy-memory/docs/architecture/mvp-scope.md`

For elegy-memory specifically:
@rust/crates/elegy-memory/docs/architecture/ARCHITECTURE.md
@rust/crates/elegy-memory/docs/architecture/memory-model.md
@rust/crates/elegy-memory/docs/architecture/traits-and-interfaces.md
@rust/crates/elegy-memory/docs/architecture/storage-schema.md
@rust/crates/elegy-memory/docs/architecture/mvp-scope.md

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
- Do not promote `.agents/skills/**`, `.github/skills/**`, wrapper folders, generated bundles, or MCP projections into authority roots.
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

Use repo-root scripts for governed exports or package boundaries:

```powershell
pwsh ./scripts/validate-package-boundaries.ps1
pwsh ./scripts/export-contracts.ps1
pwsh ./scripts/validate-canonical-outputs.ps1 -RequireGeneratedOutputs
```

## Structural Invariants — STOP

Never modify without explicit human confirmation:
- DB schema (tables, columns in any `schema.rs`)
- Public traits (any `traits.rs`)
- Serialization / on-disk storage formats
- Public API surface of any crate

If you need to touch these, STOP, explain why, wait for confirmation.

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
- Treat MCP as the primary integration contract when a CLI invocation template is the governed path
- Store raw transcripts in memory surfaces
