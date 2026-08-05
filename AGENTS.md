# Elegy Guidance

## What Elegy Is

Rust toolkit for shipping governed local CLI capabilities to AI-agent hosts.
Durable contracts and discovery metadata live in repo-visible artifacts. CLI
invocation templates are the default integration contract; MCP is an optional
adapter.

Current agent-facing readiness is generated in `docs/readiness.md`. Treat
`concept` and `implemented` surfaces as non-routable. Passing tests, schemas,
fixtures, archives, and generated projections do not establish usability.

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
- An Elegy plugin must adapt a reusable data source, database, platform, API,
  local-system, or executable/CLI boundary. Put business/domain logic in a
  library, application, Automation Pack, or product-local command.
- Only an active `surfaceClass: adapter-plugin` may use Elegy plugin packaging,
  and it must declare a typed capability catalog. A skill or historical
  `plugins/` directory is not executable discovery authority.
- Do not conform an implementation to an incubating Elegy contract or add an
  Elegy dependency solely because a schema, catalog entry, or wrapper exists.
- Do not generalize a contract or adapter without two independent working
  consumers and a documented smallest shared boundary.
- Default agent discovery may route only to `usable` and `production`
  readiness. Incubating overrides are for explicit maintainer inspection.
- Profiles are allowlists, not approvals. Side-effecting MCP tools stay blocked unless the host is started with `--allow-side-effects`.
- CLI invocation templates are the default contract. Use MCP only when the host specifically needs an MCP protocol boundary.
- Obsidian is a non-authoritative vault bridge. Not a source of truth for plans, roadmaps, or review state.

## Documentation Rules

- Read `docs/plans/automation-portability-handoff.md` before changing plugin,
  host-projection, or automation-pack terminology.
- Keep portfolio automation strategy and lifecycle truth in the canonical
  Overseer strategy; Elegy owns only its plugin/capability boundary. Target-
  native solution repositories own workflow and delivery behavior.

- Update an existing ADR or spec when extending the same decision slice.
- Use `elegy-documentation inspect/map/check --project . --json` for objective docs validation. Regenerate `docs/readiness.md` through `elegy-documentation export readiness`.
- Keep harness files thin. Root `AGENTS.md` is the repo authority; other harness files should point back here.

## Validation

Run from repo root: `cargo test -p <crate>`, `cargo run -p elegy-core --bin elegy-contracts -- --project . contracts validate`. When capability behavior changes, verify both the Rust implementation and the governed fixture/projection.

## Rust Style

- `snake_case` functions/variables, `PascalCase` types/traits, `SCREAMING_SNAKE_CASE` constants.
- `thiserror` for library errors, `anyhow` for CLI errors.
- No `unwrap()` in library code. Group imports: std, external crates, internal modules.
