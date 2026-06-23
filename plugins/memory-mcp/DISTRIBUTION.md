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

## Binary surface

- **Crate:** `rust/features/elegy-memory-mcp/`
- **Binary names:**
  - `elegy-memory-mcp-stdio` (`rust/features/elegy-memory-mcp/src/stdio_main.rs`)
  - `elegy-memory-mcp-http` (`rust/features/elegy-memory-mcp/src/main.rs`)
- **Library consumers:** none — both binaries are leaf entrypoints.

## Distribution shape

- **CLI archive asset families:**
  - `elegy-memory-mcp-stdio-<cliVersion>-<target>.zip`
  - `elegy-memory-mcp-http-<cliVersion>-<target>.zip`
- **Plugin package:** none — these are transport adapters over an existing
  feature, not portable package surfaces.

## Install

```bash
# Canonical installer (recommended)
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-memory-mcp-stdio -Force
```

```powershell
# Native-pwsh entry point: thin shim that forwards all args to bash (requires bash in PATH)
pwsh ./scripts/install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-memory-mcp-stdio -Force
```

## Build from source

```bash
cd rust
cargo build -p elegy-memory-mcp
cargo run -p elegy-memory-mcp-stdio -- --help
cargo run -p elegy-memory-mcp-http -- --help
```

## Validation

- `cargo test -p elegy-memory-mcp` (runs the full test suite under
  `rust/features/elegy-memory-mcp/tests/`)
- For HTTP/OAuth behavior, also read `rust/features/elegy-memory-mcp/docs/AUTH.md`
  and `rust/features/elegy-memory-mcp/docs/TRANSPORT.md`.

## Where to read more

- Crate boundaries: [`rust/features/elegy-memory-mcp/AGENTS.md`](./AGENTS.md)
- Memory operator that this transport exposes:
  [`../elegy-memory/DISTRIBUTION.md`](../elegy-memory/DISTRIBUTION.md)
