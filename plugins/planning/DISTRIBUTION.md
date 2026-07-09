# `elegy-planning` — distribution

## What this binary does

Dedicated CLI for durable planning state: goals, roadmaps, sections, work
points, plans, todos, issues, review points, insights, validation, project-run
leases, and the planning graph (work-point graph, run-trace context, acceptance
evidence).

SQLite via `elegy-planning` is the planning MVP authority. Markdown and JSON
files are projections only.

This binary is packaged as an `elegy-plugin/v1` plugin. Release configuration is in
`distribution/surfaces.json`.

## Binary surface

- **Crate:** `plugins/planning/`
- **Binary name:** `elegy-planning`
- **Source:** `plugins/planning/src/main.rs`
- **Plugin manifest:** `.elegy-plugin/plugin.json`
- **Plugin skills:** `plugins/planning/skills/elegy-planning/`

## Distribution shape

- **Release plugin archive:** `elegy-planning-plugin-<target>.zip` (primary GitHub release and marketplace contract)
- **Local pack default:** `elegy-planning-v<version>.plugin.zip` (ad hoc output when `pack --output` is omitted)
- **Codex export** (derived host projection): `.codex-plugin/plugin.json` + `skills/` directory
- **Versioning:** follows workspace `version`.

## Install

```bash
# Install as a plugin package (primary lane)
elegy-plugin-packaging install --archive elegy-planning-plugin-<target>.zip

# Check or update the installed marketplace plugin
elegy-plugin-packaging marketplace status --source . --plugin elegy-planning --json
elegy-plugin-packaging marketplace update elegy-planning --source . --json

# Export for Codex host (derived lane)
elegy-plugin-packaging export --plugin plugins/planning --host codex --output ./export
```

## Build from source

```bash
cargo build -p elegy-planning
cargo run -p elegy-planning -- --help
cargo run -p elegy-planning -- --json version
```

## Validation

- `cargo test -p elegy-planning`
- Plugin verify: `cargo run -p elegy-tooling --bin elegy-plugin-packaging -- verify --plugin plugins/planning`

## Where to read more

- Plugin manifest authority: `shared/plugin-sdk/src/lib.rs`
- Crate boundaries: [`AGENTS.md`](./AGENTS.md)
