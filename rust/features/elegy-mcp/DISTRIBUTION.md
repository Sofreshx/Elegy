# `elegy-mcp` — distribution

## What this binary does

Dedicated CLI for MCP descriptor authoring and analysis. Wraps the reusable
author/analyze behavior in `rust/core/elegy-tooling` over governed MCP
descriptors under `contracts/schemas/mcp-*.schema.json` and
`contracts/fixtures/mcp-*.json`.

Lower-level MCP-to-skill generation is contributor tooling exposed on the
umbrella `elegy` CLI via `elegy generate skills` /
`generate_skills_from_descriptor_file`; this binary does not cover that
lane.

## Binary surface

- **Crate:** `rust/features/elegy-mcp/`
- **Binary name:** `elegy-mcp`
- **Source:** `rust/features/elegy-mcp/src/main.rs`
- **Library consumers:** `rust/bin/elegy-cli` (umbrella `elegy mcp` subcommands).

## Distribution shape

- **CLI archive asset family:** `elegy-mcp-<cliVersion>-<target>.zip`
- **Wrapper archive:** `elegy-mcp-wrapper-<bundleVersion>.zip`
- **Versioning:** follows workspace `version`.

## Install

```bash
# Canonical installer (recommended)
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-mcp -Force
```

```powershell
# Native-pwsh entry point: thin shim that forwards all args to bash (requires bash in PATH)
pwsh ./scripts/install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-mcp -Force
```

## Build from source

```bash
cd rust
cargo build -p elegy-mcp
cargo run -p elegy-mcp -- --help
```

## Validation

- `cargo test -p elegy-mcp` (runs the full test suite under `rust/features/elegy-mcp/tests/`)

## Where to read more

- MCP ownership rules and authoring flow:
  [`docs/architecture/mcp-skill-tooling-placement.md`](../../../../docs/architecture/mcp-skill-tooling-placement.md)
- Crate boundaries: [`rust/features/elegy-mcp/AGENTS.md`](./AGENTS.md)
