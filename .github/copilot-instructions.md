# Elegy Project - Copilot Instructions

## Project Overview

Elegy is a Rust toolkit for shipping governed local CLI capabilities to AI-agent hosts.
Governed contracts, skill definitions, compatibility data, and policy artifacts are
the durable authority. Rust implements reusable executable behavior over those artifacts.
CLI invocation templates are the default agent boundary; MCP is an optional projection.

## Architecture Documentation

Before writing any code, read the relevant architecture docs:

- `README.md` - current public project shape and shipped surfaces.
- `docs/agent-integration.md` - host onboarding, discovery, invocation templates, profiles, JSON envelopes, and MCP projection.
- `docs/architecture/mcp-skill-tooling-placement.md` - MCP, skill registry, and ownership boundaries.
- `docs/architecture/documentation-practices.md` and `docs/specs/documentation-practices-skill-and-cli.md` - ADR/spec doctrine and docs validation posture.
- `docs/spec-baseline.md` - current MCP protocol baseline.
- `rust/features/elegy-memory/docs/architecture/ARCHITECTURE.md` and `rust/features/elegy-memory/docs/architecture/mvp-scope.md` - memory architecture and MVP scope.

## Coding Standards

- Language: Rust (latest stable edition).
- Error handling: Use `thiserror` for library errors, `anyhow` for binary/CLI errors. Never `unwrap()` in library code.
- Naming: snake_case for functions/variables, PascalCase for types/traits, SCREAMING_SNAKE_CASE for constants.
- Documentation: Every public type, trait, and function must have a doc comment explaining its purpose and contract.
- Testing: Every public function must have at least one unit test. Integration tests live in crate-local `tests/` directories.
- Dependencies: Minimize external dependencies, especially in crates that feed CLI, MCP, host, or contract surfaces.
- No `unsafe` code without explicit justification in a comment.

## Agent Tool Discovery

- Prefer `elegy agent check/manifest/discover --json` for host onboarding and profile-filtered progressive discovery.
- Use `elegy-skills list/search/resolve/get/validate --json` or the umbrella `elegy skills ...` compatibility surface when developing or inspecting the governed skill registry.
- skill definitions in `contracts/fixtures/skill.*.json` are the supported skill contract.
- Do not reintroduce v1 `skill-definition.*.json` files.
- `elegy run` exposes an optional MCP stdio adapter over governed capabilities. Side-effecting tools are blocked by default unless dry-run input is provided or the host was started with `--allow-side-effects`.
- Profiles are allowlists, not approvals.

## Architecture Rules — Critical

1. **Trait-first design.** All core behaviors are defined as traits (`MemoryStore`, `EmbeddingProvider`, `MemoryConsolidator`). Implementations are separate. Never hardcode a concrete type where a trait would work.
2. **MVP discipline.** Check `rust/features/elegy-memory/docs/architecture/mvp-scope.md` before implementing anything. If a feature is marked v1 or v2, create the trait/struct/table but leave the implementation as a no-op or todo!().
3. **Write-time gating.** Every memory write passes through the salience gate (novelty check → salience check → provenance check) before being stored. Never bypass the gate.
4. **Scopes are isolated.** Session, Workspace, User, and Agent scopes have separate storage. Never cross-query scopes without explicit API calls.
5. **Embeddings are async-safe.** Embedding computation can fail or be slow. Always handle `embedding_stale` flag. Never block writes on embedding generation.
6. **Provenance is mandatory.** Every memory must have a provenance level. Default is `Imported` (lowest trust).
7. **Derived surfaces are adapters.** Do not promote wrapper folders, generated bundles, Codex plugin exports, or MCP projections into authority roots.

## File Organization

- `contracts/` holds the governed contract and policy surfaces. Operational policy lives at `docs/governance/`.
- `rust/{core,features,bin}/` is the active Rust workspace. `core/` holds the shared kernel crates used by multiple features, `features/` holds one tree per agent capability, and `bin/` holds the thin host and umbrella CLI entrypoints.
- `docs/adr/` and `docs/specs/` are the configured current documentation authority roots for durable decisions and implementation-facing specs.
