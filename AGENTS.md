# Elegy Codex Guidance

## Repo Centers

- `contracts/`, `governance/`, `schemas/`, and `policies/` are the authority roots for canonical truth.
- `rust/` is the first-party runtime family for reusable executable behavior over those governed artifacts.
- `src/Elegy-memory`, `src/Elegy-mcp`, and `src/Elegy-skills` are contributor-navigation wrapper overlays only. They are not authority roots, implementation centers, or release surfaces.

## Start Here

- Read `README.md`, then the smallest relevant doc under `docs/`.
- Use `docs/agent-integration.md` before changing host onboarding, discovery, invocation envelopes, MCP projection, or other agent-facing JSON.
- Use `docs/architecture/mcp-skill-tooling-placement.md` before changing MCP authoring, analysis, skill-generation, or ownership boundaries.
- Treat `contracts/fixtures/skill-definition-v2.*.json` as governed discovery authority. Do not reintroduce v1 `skill-definition.*.json`.

## Boundary Rules

- `contracts/schemas/**` define durable contract truth. Governed fixtures and compatibility data under `contracts/**` define the stable agent-facing artifact family.
- Discovery indexes, generated bundles, `SKILL.md`, wrapper surfaces, and MCP projections are derived outputs or adapters, not independent authority.
- MCP is an optional adapter over governed capabilities and CLI behavior, not the primary Elegy authority surface.
- Profiles are allowlists, not approvals. Side effects still require host policy, and side-effecting MCP tools stay blocked unless the call is an explicit dry run or the host is started with `--allow-side-effects`.
- Mermaid reverse projection is bounded analysis; do not describe it as canonical workflow reconstruction.

## Review Discipline

- For non-trivial changes, state the smallest safe plan before editing.
- When capability behavior changes, verify both the Rust implementation and the governed fixture/projection that exposes it to agents.
- Before handoff, challenge the change for contract drift, stale docs, and accidental promotion of wrapper or projection surfaces into authority.

## Validation

- Prefer the narrowest validation that proves the changed boundary.
- Run validation from `rust/` for Rust behavior and use repo-root scripts for governed/export boundaries.
- If docs or fixtures changed without code, inspect emitted JSON or generated contract output instead of only proofreading Markdown.

## elegy-memory Guardrails

1. **Trait-first.** Define behavior as traits. Implementations are pluggable. Never hardcode concrete types in function signatures where a trait bound works.
2. **MVP discipline.** If `mvp-scope.md` says a feature is v1 or v2, write the type/trait skeleton with `todo!()` or no-op. Do not implement it.
3. **No raw transcripts.** Memories are distilled (summaries, facts, decisions). Never store full conversation text.
4. **Provenance is mandatory.** Every memory has a `ProvenanceLevel`. No memory without provenance.
5. **Write-time gate.** Every memory passes through the salience gate before storage. No bypass.
6. **Embeddings can fail.** Handle gracefully. Mark as stale. Never block writes on embedding computation.
7. **Scopes are isolated.** Session != Workspace != User != Agent. No implicit cross-scope queries.
8. **Test everything public.** Every public function needs at least one test.
9. **Document everything public.** Doc comments on all public items.
10. **Minimize dependencies.** Core deps: rusqlite (bundled), sqlite-vec, serde, chrono, uuid, clap.

## Code Style

- `snake_case` functions/variables, `PascalCase` types/traits, `SCREAMING_SNAKE_CASE` constants.
- `thiserror` for library errors, `anyhow` for CLI errors.
- No `unwrap()` in library code. Use `?` or explicit error handling.
- Prefer returning `Result<T, E>` over panicking.
- Group imports: std -> external crates -> internal modules, separated by blank lines.

## Git Workflow

- Promotion chain: `<topic>` -> `roro` -> `dev` -> `main`.
- Keep branch ancestry monotonic: `main` must remain an ancestor of `dev`, and `dev` must remain an ancestor of `roro`.
- Do feature work on dedicated topic branches rather than directly on `roro`, `dev`, or `main`.
- Merge `dev` into `main` only after `dev` is clean and validated.
- If a hotfix lands on `main`, propagate it back through `dev` and then `roro` before continuing feature work.
- If any branch in the chain falls behind its upstream branch, reconcile downstream before starting more feature work.
- After a complete promotion cycle, `main`, `dev`, and `roro` may all point to the same commit. This is the correct starting state for the next cycle.
- Push promoted branches to `origin` only after the local promotion step is clean. Publish in chain order: `main`, `dev`, then `roro`.
- The following `roro` rules apply only when the current branch is `roro`:
- Merge a topic branch into `roro` only after the relevant validation passes and the branch is ready to promote.
- Merge `roro` into `dev` only after `roro` is clean, validated, and reconciled with newer `main` changes.
- Never force-push or rewrite history on `main` or `dev`.
