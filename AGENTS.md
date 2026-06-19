# Elegy Codex Guidance

## What Elegy Is

Elegy is a Rust toolkit for shipping governed local CLI capabilities to AI-agent hosts.
It keeps durable contracts and discovery metadata in repo-visible artifacts, exposes
installable binaries through releases, and treats CLI invocation templates as the
default execution boundary. MCP is supported as an optional adapter, not the primary
authority surface.

## Authority Centers

- `contracts/` is the neutral authority root for contracts, schemas, fixtures, compatibility, schema-line metadata, and policy. Workflow and operational governance policy lives at `docs/governance/`.
- `contracts/fixtures/skill.*.json` is the governed discovery authority for built-in skills. Do not add or revive v1 `skill-definition.*.json` files.
- `rust/` is the first-party runtime family for reusable executable behavior over governed artifacts.
- `docs/adr/` and `docs/specs/` hold current durable documentation decisions and implementation-facing specs, configured by `.elegy/docs.yaml`.
- The `Elegy-obsidian` surface delegates to the official Obsidian Desktop CLI. Do not describe it as a Rust binary or as durable planning authority.

## Start Here

- Read `README.md`, then `docs/architecture/ecosystem-topology.md` or `docs/architecture/substrate-governance.md` when changing repo ownership, authority boundaries, or public positioning.
- Read the smallest relevant doc under `docs/architecture/`, `docs/adr/`, or `docs/specs/` before changing behavior in that lane.
- Use `docs/agent-integration.md` before changing host onboarding, discovery, invocation envelopes, profiles, MCP projection, side-effect posture, or other agent-facing JSON.
- Use `docs/architecture/mcp-skill-tooling-placement.md` before changing MCP authoring, analysis, skill generation, portable plugin packages, or ownership boundaries.
- Use `docs/architecture/documentation-practices.md` and `docs/specs/documentation-practices-skill-and-cli.md` before changing ADR/spec doctrine, docs config, or docs validation behavior.
- Use `docs/specs/obsidian-skill-and-cli.md` before changing the Obsidian skill, wrapper, result envelope, or vault-boundary guidance.
- Use `docs/spec-baseline.md` before changing MCP protocol baseline language.

## Boundary Rules

- `contracts/schemas/**` define durable contract truth. Governed fixtures and compatibility data under `contracts/**` define the stable agent-facing artifact family.
- Discovery indexes, generated bundles, `SKILL.md` mirrors, wrapper surfaces, Codex plugin exports, and MCP projections are derived outputs or adapters, not independent authority.
- Profiles are allowlists, not approvals. Side effects still require host policy, and side-effecting MCP tools stay blocked unless the call is an explicit dry run or the host is started with `--allow-side-effects`.
- CLI invocation templates are the default integration contract. Use MCP only when the host specifically needs an MCP protocol boundary.
- Mermaid reverse projection is bounded analysis. Do not describe it as canonical workflow reconstruction.
- Portable plugin packages are metadata envelopes, not runtime sessions, approval records, secret stores, or host policy containers.
- Obsidian is a non-authoritative vault bridge. Do not make it a source of truth for plans, roadmaps, goals, todos, or review state.

## Documentation Rules

- Prefer updating an existing ADR or spec when extending the same decision or behavior slice.
- Create or update specs for implementation-facing behavior, acceptance criteria, and validation evidence.
- Use `elegy-documentation inspect/map/check --project . --json` for authority-aware documentation inspection and objective validation.
- Use umbrella `elegy docs ...` only for the current compatibility path for ADR/spec scaffolding and docs index behavior.
- Objective docs validation does not prove prose quality or architecture correctness; still inspect reasoning and authority boundaries manually.

## Validation

- Prefer the narrowest validation that proves the changed boundary.
- Run validation from `rust/` for Rust behavior and use repo-root scripts for governed/export boundaries.
- If docs or fixtures changed without code, inspect emitted JSON or generated contract output instead of only proofreading Markdown.
- When capability behavior changes, verify both the Rust implementation and the governed fixture/projection that exposes it to agents.
- When wrapper roots or generated projections change, verify the canonical-output or package-boundary path that covers that generated surface.

## Rust Style

- `snake_case` functions/variables, `PascalCase` types/traits, `SCREAMING_SNAKE_CASE` constants.
- `thiserror` for library errors, `anyhow` for CLI errors.
- No `unwrap()` in library code. Use `?` or explicit error handling.
- Prefer returning `Result<T, E>` over panicking.
- Group imports: std, external crates, internal modules, separated by blank lines.
- Minimize dependencies, especially in crates that feed CLI, MCP, host, or contract surfaces.

## elegy-memory Guardrails

1. Trait-first. Define behavior as traits. Implementations are pluggable. Never hardcode concrete types in function signatures where a trait bound works.
2. MVP discipline. If `mvp-scope.md` says a feature is v1 or v2, write the type/trait skeleton with `todo!()` or no-op. Do not implement it.
3. No raw transcripts. Memories are distilled summaries, facts, decisions, procedures, or observations.
4. Provenance is mandatory. Every memory has a `ProvenanceLevel`. No memory without provenance.
5. Write-time gate. Every memory passes through the salience gate before storage. No bypass.
6. Embeddings can fail. Handle gracefully, mark stale when needed, and never block writes on provider-backed embedding computation.
7. Scopes are isolated. Session, workspace, user, and agent scopes must not cross-query implicitly.
8. SQLite is the MVP storage authority. PostgreSQL and broader provider surfaces stay later-scope unless the current memory docs say otherwise.

