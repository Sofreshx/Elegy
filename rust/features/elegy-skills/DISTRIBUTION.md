# `elegy-skills` — distribution

## What this binary does

Dedicated CLI for the governed skill registry. Provides search, resolve,
inspect, profile filtering, projection, and validation over governed skill
artifacts under `contracts/fixtures/skill.*.json`. Reusable executable
behavior over governed artifacts — the registry's authority stays in
`contracts/`.

## Binary surface

- **Crate:** `rust/features/elegy-skills/`
- **Binary name:** `elegy-skills`
- **Source:** `rust/features/elegy-skills/src/main.rs`
- **Library consumers:** `rust/bin/elegy-cli` (umbrella `elegy skills` subcommands),
  `rust/bin/elegy-host-mcp` (host transport uses the registry for capability
  discovery).

## Distribution shape

- **CLI archive asset family:** `elegy-skills-<cliVersion>-<target>.zip`
- **Wrapper archive:** `elegy-skills-wrapper-<bundleVersion>.zip`
- **Release catalog entry:** `distribution/surfaces.json` (name: `elegy-skills`)
- **Skill fixture:** `contracts/fixtures/skill.elegy-skills.json`
- **Versioning:** follows workspace `version`.

## Install

```bash
# Canonical installer (recommended)
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-skills -Force
```

```powershell
# Native-pwsh entry point: thin shim that forwards all args to bash (requires bash in PATH)
pwsh ./scripts/install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-skills -Force
```

## Build from source

```bash
cd rust
cargo build -p elegy-skills
cargo run -p elegy-skills -- --help
```

## Validation

- `cargo test -p elegy-skills` (runs the full test suite under `rust/features/elegy-skills/tests/`)

## Where to read more

- Skill core v1 (current skill authority split):
  [`docs/architecture/skill-core-v1.md`](../../../../docs/architecture/skill-core-v1.md)
- Crate boundaries: [`rust/features/elegy-skills/AGENTS.md`](./AGENTS.md)
