# `elegy-configuration` — distribution

## What this binary does

Dedicated CLI for deterministic configuration materialization. Provides
`list | show | apply | verify` flows over governed configuration templates
and profiles. Materializes declared assets (skill mirrors, instruction blocks,
MCP config, hooks, agents, bounded text, JSON, or TOML patches) into target
paths and verifies drift.

This is a post-install or from-source operator lane, not a new distribution
model: `elegy-configuration` reads package-v1 configuration
templates/profiles from a portable plugin package or from a local template
catalog and materializes them deterministically.

**This surface is NOT packaged as an `elegy-plugin/v1` plugin.** It ships as a
standalone CLI binary.

## Binary surface

- **Crate:** `plugins/configuration/`
- **Binary name:** `elegy-configuration`
- **Source:** `plugins/configuration/src/main.rs`

## Distribution shape

- **CLI archive asset family:** `elegy-configuration-<cliVersion>-<target>.zip`
- **Versioning:** follows workspace `version`.

## Install

```bash
# Canonical installer
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy --surface elegy-configuration -Force
```

```powershell
# Native-pwsh entry point: thin shim that forwards all args to bash (requires bash in PATH)
pwsh ./scripts/install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy --surface elegy-configuration -Force
```

## Build from source

```bash
cargo build -p elegy-configuration
cargo run -p elegy-configuration -- --help
```

## Validation

- `cargo test -p elegy-configuration`
- For real materialization, prefer
  `elegy-configuration apply --dry-run --json` or
  `elegy-configuration verify --json` against the smallest target that proves
  the changed materialization path.

## Where to read more

- Configuration contract schemas:
  [`plugins/configuration/schemas/`](./schemas/)
- Crate boundaries: [`AGENTS.md`](./AGENTS.md)
