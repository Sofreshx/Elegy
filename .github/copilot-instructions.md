# Elegy Project - Copilot Instructions

## Project Overview

Elegy is a Rust toolkit for shipping governed local CLI capabilities to AI-agent hosts.
Governed contracts, v2 skill definitions, compatibility data, and policy artifacts are
the durable authority. Rust implements reusable executable behavior over those artifacts.
CLI invocation templates are the default agent boundary; MCP is an optional projection.

## Architecture Documentation

Before writing any code, read the relevant architecture docs:

- `README.md` - current public project shape and shipped surfaces.
- `docs/agent-integration.md` - host onboarding, discovery, invocation templates, profiles, JSON envelopes, and MCP projection.
- `docs/architecture/mcp-skill-tooling-placement.md` - MCP, skill generation, portable package, and ownership boundaries.
- `docs/architecture/documentation-practices.md` and `docs/specs/documentation-practices-skill-and-cli.md` - ADR/spec doctrine and docs validation posture.
- `docs/spec-baseline.md` - current MCP protocol baseline.
- `rust/crates/elegy-memory/docs/architecture/ARCHITECTURE.md` and `rust/crates/elegy-memory/docs/architecture/mvp-scope.md` - memory architecture and MVP scope.

## Coding Standards

- Language: Rust (latest stable edition).
- Error handling: Use `thiserror` for library errors, `anyhow` for binary/CLI errors. Never `unwrap()` in library code.
- Naming: snake_case for functions/variables, PascalCase for types/traits, SCREAMING_SNAKE_CASE for constants.
- Documentation: Every public type, trait, and function must have a doc comment explaining its purpose and contract.
- Testing: Every public function must have at least one unit test. Integration tests live in `rust/crates/elegy-memory/tests/`.
- Dependencies: Minimize external dependencies, especially in crates that feed CLI, MCP, host, or contract surfaces.
- No `unsafe` code without explicit justification in a comment.

## Agent Tool Discovery

- Prefer `elegy agent check/manifest/discover --json` for host onboarding and profile-filtered progressive discovery.
- Use `elegy-skills list/search/resolve/get/capability/validate --json` or the umbrella `elegy skills ...` compatibility surface when developing or inspecting the governed skill registry.
- V2 skill definitions in `contracts/fixtures/skill-definition-v2.*.json` are the supported skill contract.
- Do not reintroduce v1 `skill-definition.*.json` files.
- `elegy run` exposes an optional MCP stdio adapter over governed capabilities. Side-effecting tools are blocked by default unless dry-run input is provided or the host was started with `--allow-side-effects`.
- Profiles are allowlists, not approvals.

## Architecture Rules — Critical

1. **Trait-first design.** All core behaviors are defined as traits (`MemoryStore`, `EmbeddingProvider`, `MemoryConsolidator`). Implementations are separate. Never hardcode a concrete type where a trait would work.
2. **MVP discipline.** Check `rust/crates/elegy-memory/docs/architecture/mvp-scope.md` before implementing anything. If a feature is marked v1 or v2, create the trait/struct/table but leave the implementation as a no-op or todo!().
3. **Write-time gating.** Every memory write passes through the salience gate (novelty check → salience check → provenance check) before being stored. Never bypass the gate.
4. **Scopes are isolated.** Session, Workspace, User, and Agent scopes have separate storage. Never cross-query scopes without explicit API calls.
5. **Embeddings are async-safe.** Embedding computation can fail or be slow. Always handle `embedding_stale` flag. Never block writes on embedding generation.
6. **Provenance is mandatory.** Every memory must have a provenance level. Default is `Imported` (lowest trust).
7. **Derived surfaces are adapters.** Do not promote `.agents/skills/**`, `.github/skills/**`, wrapper folders, generated bundles, Codex plugin projections, or MCP projections into authority roots.

## Git Workflow

- Promotion chain: `<topic>` -> `roro` -> `dev` -> `main`
- Keep branch ancestry monotonic: `main` must remain an ancestor of `dev`, and `dev` must remain an ancestor of `roro`
- Do feature work on dedicated topic branches, not directly on `roro`, `dev`, or `main`
- Merge `dev` into `main` only after `dev` is clean and validated
- If a hotfix lands on `main`, propagate it back through `dev` and then `roro` before continuing feature work
- If any branch in the chain falls behind its upstream branch, reconcile downstream before starting more feature work
- After a complete promotion cycle, `main`, `dev`, and `roro` may all point to the same commit. This is the correct starting state for the next cycle
- After a clean local promotion cycle, push `main`, `dev`, and `roro` to `origin` immediately so the remote stays aligned with the validated local state. Prefer a single atomic push when available
- The following `roro` rules apply only when the current branch is `roro`:
- Merge a topic branch into `roro` only after validation is clean
- Merge `roro` into `dev` only after `roro` is clean, validated, and reconciled with newer `main` changes

## File Organization

- `contracts/`, `governance/`, `schemas/`, and `policies/` hold the governed contract and policy surfaces.
- `rust/crates/` is the active Rust workspace and contains first-party crates for the umbrella CLI, dedicated CLIs, runtime, adapters, policy, contracts, memory, MCP, skills, planning, configuration, documentation, observe, desktop, data, web, notify, and related tooling.
- `src/Elegy-*` directories are wrapper or contributor-navigation surfaces, not the canonical Rust implementation roots.
- `docs/adr/` and `docs/specs/` are the configured current documentation authority roots for durable decisions and implementation-facing specs.
