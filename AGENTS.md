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
