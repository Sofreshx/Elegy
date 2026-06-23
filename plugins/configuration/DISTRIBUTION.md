# `elegy-configuration` — distribution

## What this binary does

Dedicated CLI for deterministic configuration materialization. Provides
`list | show | apply | verify` flows over governed configuration templates
and profiles under `contracts/configuration/`. Materializes declared assets
(skill mirrors, instruction blocks, MCP config, hooks, agents, bounded text,
JSON, or TOML patches) into target paths and verifies drift.

This is a post-install or from-source operator lane, not a new distribution
model: `elegy-configuration` reads package-v1 configuration
templates/profiles from a portable plugin package or from a local template
catalog and materializes them deterministically.

## Binary surface

- **Crate:** `rust/features/elegy-configuration/`
- **Binary name:** `elegy-configuration`
- **Source:** `rust/features/elegy-configuration/src/main.rs`
- **Library consumers:** `rust/bin/elegy-cli` (umbrella `elegy configuration`
  subcommands).

## Distribution shape

- **CLI archive asset family:** `elegy-configuration-<cliVersion>-<target>.zip`
- **Wrapper archive:** `elegy-configuration-wrapper-<bundleVersion>.zip`
- **Versioning:** follows workspace `version`.

## Install

```bash
# Canonical installer (recommended)
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-configuration -Force
```

```powershell
# Native-pwsh entry point: thin shim that forwards all args to bash (requires bash in PATH)
pwsh ./scripts/install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-configuration -Force
```

## Build from source

```bash
cd rust
cargo build -p elegy-configuration
cargo run -p elegy-configuration -- --help
```

## Validation

- `cargo test -p elegy-configuration` (runs the full test suite under
  `rust/features/elegy-configuration/tests/`)
- For real materialization, prefer
  `elegy-configuration apply --dry-run --json` or
  `elegy-configuration verify --json` against the smallest target that proves
  the changed materialization path.

## Where to read more

- Configuration contract schemas:
  [`contracts/schemas/elegy-configuration-{template,profile,receipt}-v1.schema.json`](../../../../contracts/schemas/)
- Configuration catalog: [`contracts/configuration/`](../../../../contracts/configuration/)
- Crate boundaries: [`rust/features/elegy-configuration/docs/architecture/v1.md`](./docs/architecture/v1.md)
