# `elegy-memory-mcp` — distribution

## What this binary does

Transport adapters that expose the `elegy-memory` library over MCP. Two
binaries are published:

- `elegy-memory-mcp-stdio` — local subprocess transport, no OAuth, no network.
- `elegy-memory-mcp-http` — remote OAuth 2.1 + bearer JWT transport over
  streamable HTTP.

This crate adapts `elegy-memory` to MCP transports. It does not define new
memory authority, salience, correction, or scope behavior — those stay in
`elegy-memory`.

**This surface is NOT packaged as an `elegy-plugin/v1` plugin.** It ships as a
standalone CLI binary.

## Binary surface

- **Crate:** `plugins/memory-mcp/`
- **Binary names:**
  - `elegy-memory-mcp-stdio` (`plugins/memory-mcp/src/stdio_main.rs`)
  - `elegy-memory-mcp-http` (`plugins/memory-mcp/src/main.rs`)
- **Library consumers:** none — both binaries are leaf entrypoints.

## Distribution shape

- **CLI archive asset families:**
  - `elegy-memory-mcp-stdio-<cliVersion>-<target>.zip`
  - `elegy-memory-mcp-http-<cliVersion>-<target>.zip`
- **Plugin package:** none — these are transport adapters over an existing
  plugin, not portable package surfaces.

## Install

```bash
# Canonical installer
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy --surface elegy-memory-mcp-stdio -Force
```

```powershell
# Native-pwsh entry point: thin shim that forwards all args to bash (requires bash in PATH)
pwsh ./scripts/install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy --surface elegy-memory-mcp-stdio -Force
```

## Build from source

```bash
cargo build -p elegy-memory-mcp
cargo run -p elegy-memory-mcp-stdio -- --help
cargo run -p elegy-memory-mcp-http -- --help
```

## Validation

- `cargo test -p elegy-memory-mcp`

## Where to read more

- Crate boundaries: [`AGENTS.md`](./AGENTS.md)
- Memory operator that this transport exposes:
  [`../memory/DISTRIBUTION.md`](../memory/DISTRIBUTION.md)
