# Elegy Guidance

## What Elegy Is

Rust toolkit for shipping governed local CLI capabilities to AI-agent hosts.
Durable contracts and discovery metadata live in repo-visible artifacts. CLI
invocation templates are the default integration contract; MCP is an optional
adapter.

## Authority Hierarchy

| Priority | Source |
|---|---|
| 1 | Explicit user instruction |
| 2 | `docs/architecture/README.md` — repo topology, governance, skill placement, terminology |
| 3 | `docs/adr/` — durable architecture decisions |
| 4 | `docs/specs/` — implementation-facing behavior and acceptance criteria |
| 5 | `plugins/<name>/AGENTS.md` — plugin-local guidance (e.g. `plugins/memory/AGENTS.md`) |
| 6 | Repeated implementation patterns in the workspace |

## Boundary Rules

- Discovery indexes, generated bundles, SKILL.md mirrors, and MCP projections are derived outputs, not independent authority.
- Profiles are allowlists, not approvals. Side-effecting MCP tools stay blocked unless the host is started with `--allow-side-effects`.
- CLI invocation templates are the default contract. Use MCP only when the host specifically needs an MCP protocol boundary.
- Obsidian is a non-authoritative vault bridge. Not a source of truth for plans, roadmaps, or review state.

## Documentation Rules

- Read `docs/plans/automation-portability-handoff.md` before changing plugin,
  host-projection, or automation-pack terminology.

- Update an existing ADR or spec when extending the same decision slice.
- Use `elegy-documentation inspect/map/check --project . --json` for objective docs validation.
- Keep harness files thin. Root `AGENTS.md` is the repo authority; other harness files should point back here.

## Validation

Run from repo root: `cargo test -p <crate>`, `cargo run -p elegy-core --bin elegy-contracts -- --project . contracts validate`. When capability behavior changes, verify both the Rust implementation and the governed fixture/projection.

## Rust Style

- `snake_case` functions/variables, `PascalCase` types/traits, `SCREAMING_SNAKE_CASE` constants.
- `thiserror` for library errors, `anyhow` for CLI errors.
- No `unwrap()` in library code. Group imports: std, external crates, internal modules.
