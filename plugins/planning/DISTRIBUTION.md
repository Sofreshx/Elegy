# `elegy-planning` — distribution

## What this binary does

Dedicated CLI for durable planning state: goals, roadmaps, sections, work
points, plans, todos, issues, review points, insights, validation, project-run
leases, and the planning graph (work-point graph, run-trace context,
acceptance evidence).

SQLite via `elegy-planning` is the planning MVP authority. Markdown and JSON
files are projections only.

## Binary surface

- **Crate:** `rust/features/elegy-planning/`
- **Binary name:** `elegy-planning`
- **Source:** `rust/features/elegy-planning/src/main.rs`
- **Library consumers:** `rust/bin/elegy-cli` (umbrella `elegy planning` subcommands),
  `rust/features/elegy-skills` (the planning skill fixture references the
  planning binary via capability projection).

## Distribution shape

- **CLI archive asset family:** `elegy-planning-<cliVersion>-<target>.zip`
- **Wrapper archive:** `elegy-planning-wrapper-<bundleVersion>.zip`
- **Release catalog entry:** `distribution/surfaces.json` (name: `elegy-planning`)
- **Skill fixture:** `contracts/fixtures/skill.elegy-planning.json`
- **Versioning:** follows workspace `version`.

## Install

```bash
# Canonical installer (recommended)
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-planning -Force
```

```powershell
# Native-pwsh entry point: thin shim that forwards all args to bash (requires bash in PATH)
pwsh ./scripts/install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-planning -Force
```

## Build from source

```bash
cd rust
cargo build -p elegy-planning
cargo run -p elegy-planning -- --help
```

## Validation

- `cargo test -p elegy-planning` (covers `storage.rs` test suite, `machine_posture.rs` integration tests)
- For machine output changes, verify `--json --non-interactive --correlation-id` behavior explicitly

## Where to read more

- Planning architecture:
  [`docs/architecture/ARCHITECTURE.md`](./docs/architecture/ARCHITECTURE.md)
- Planning MVP scope:
  [`docs/architecture/mvp-scope.md`](./docs/architecture/mvp-scope.md)
- Planning v1: [`docs/architecture/v1.md`](./docs/architecture/v1.md)
- Crate boundaries: [`rust/features/elegy-planning/AGENTS.md`](./AGENTS.md)
- Planning specs (graph, state machine, run-trace, acceptance):
  [`docs/specs/`](./docs/specs/)
