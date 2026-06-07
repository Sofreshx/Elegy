# Elegy Codex Guidance

## What Elegy Is

Elegy is a Rust toolkit for shipping governed local CLI capabilities to AI-agent hosts.
It keeps durable contracts and discovery metadata in repo-visible artifacts, exposes
installable binaries through releases, and treats CLI invocation templates as the
default execution boundary. MCP is supported as an optional adapter, not the primary
authority surface.

## Authority Centers

- `contracts/`, `governance/`, `schemas/`, and `policies/` are the neutral authority roots for contracts, compatibility, schemas, and policy.
- `contracts/fixtures/skill.*.json` is the governed discovery authority for built-in skills. Do not add or revive v1 `skill-definition.*.json` files.
- `rust/` is the first-party runtime family for reusable executable behavior over governed artifacts.
- `docs/adr/` and `docs/specs/` hold current durable documentation decisions and implementation-facing specs configured by `.elegy/docs.yaml`.
- `src/Elegy-memory`, `src/Elegy-mcp`, `src/Elegy-skills`, `src/Elegy-planning`, `src/Elegy-configuration`, `src/Elegy-documentation`, and `src/Elegy-mermaid` are contributor-navigation wrapper overlays only. They are not authority roots, implementation centers, or release surfaces.

## Start Here

- Read `README.md`, then the smallest relevant doc under `docs/architecture/`, `docs/adr/`, or `docs/specs/`.
- Use `docs/agent-integration.md` before changing host onboarding, discovery, invocation envelopes, profiles, MCP projection, side-effect posture, or other agent-facing JSON.
- Use `docs/architecture/mcp-skill-tooling-placement.md` before changing MCP authoring, analysis, skill generation, portable plugin packages, or ownership boundaries.
- Use `docs/architecture/documentation-practices.md` and `docs/specs/documentation-practices-skill-and-cli.md` before changing ADR/spec doctrine, docs config, or docs validation behavior.
- Use `docs/spec-baseline.md` before changing MCP protocol baseline language.

## Boundary Rules

- `contracts/schemas/**` define durable contract truth. Governed fixtures and compatibility data under `contracts/**` define the stable agent-facing artifact family.
- Discovery indexes, generated bundles, `.agents/skills/**`, `.github/skills/**`, `SKILL.md` mirrors, wrapper surfaces, Codex plugin projections, and MCP projections are derived outputs or adapters, not independent authority.
- Profiles are allowlists, not approvals. Side effects still require host policy, and side-effecting MCP tools stay blocked unless the call is an explicit dry run or the host is started with `--allow-side-effects`.
- CLI invocation templates are the default integration contract. Use MCP only when the host specifically needs an MCP protocol boundary.
- Mermaid reverse projection is bounded analysis. Do not describe it as canonical workflow reconstruction.
- Portable plugin packages are metadata envelopes, not runtime sessions, approval records, secret stores, or host policy containers.

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

## Git Workflow

- Promotion chain: `<topic>` -> `roro` -> `dev` -> `main`.
- Keep branch ancestry monotonic: `main` must remain an ancestor of `dev`, and `dev` must remain an ancestor of `roro`.
- Do feature work on dedicated topic branches rather than directly on `roro`, `dev`, or `main`.
- Merge `dev` into `main` only after `dev` is clean and validated.
- If a hotfix lands on `main`, propagate it back through `dev` and then `roro` before continuing feature work.
- If any branch in the chain falls behind its upstream branch, reconcile downstream before starting more feature work.
- After a complete promotion cycle, `main`, `dev`, and `roro` may all point to the same commit. This is the correct starting state for the next cycle.
- After a clean local promotion cycle, push `main`, `dev`, and `roro` to `origin` immediately so the remote stays aligned with the validated local state. Prefer a single atomic push when available.
- The following `roro` rules apply only when the current branch is `roro`:
- Merge a topic branch into `roro` only after the relevant validation passes and the branch is ready to promote.
- Merge `roro` into `dev` only after `roro` is clean, validated, and reconciled with newer `main` changes.
- Never force-push or rewrite history on `main` or `dev`.
