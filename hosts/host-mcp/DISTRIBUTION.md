# `elegy-host-mcp` — distribution

## What this crate is

`hosts/host-mcp/` is a **library** crate, not a binary. It is the
host-side MCP transport adapter library used by the umbrella `elegy` CLI.

It re-exports `serve_stdio`, `serve_stdio_with_options`, `ElegyMcpHost`, and
`HostOptions` so the umbrella `elegy mcp host` subcommand can wire up MCP
hosting without rebuilding the transport here.

## Library surface

- **Crate:** `hosts/host-mcp/`
- **Library name:** `elegy-host-mcp`
- **Source files:** `hosts/host-mcp/src/lib.rs` (re-exports),
  `host.rs` (transport), `error.rs` (typed errors)
- **Library consumers:**
  - the umbrella `elegy` binary for the
    `elegy mcp host` subcommand
- **Binary consumers:** none — this crate has no `[[bin]]` and no
  `src/main.rs`. It compiles only to a library.

## Distribution shape

- **No standalone archive.** This crate ships as part of the umbrella
  `elegy-cli-<cliVersion>-<target>.zip` archive.
- **Versioning:** follows workspace `version`.
- **Plugin package:** none — the host is a transport adapter library, not a
  portable package surface.

## Install

There is no separate install. The umbrella `elegy-cli` archive carries the
host transport.

```bash
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy --surface elegy-cli -Force
```

## Build from source

```bash
cargo build -p elegy-host-mcp
```

The library is then linked into `cargo build -p elegy-cli`.

## Validation

- `cargo test -p elegy-host-mcp`
- For the umbrella integration: `cargo run -p elegy-cli -- mcp host --help`

## Where to read more

- MCP architecture and ownership rules:
  [`docs/architecture/mcp-skill-tooling-placement.md`](../../../../docs/architecture/mcp-skill-tooling-placement.md)
- Umbrella CLI that uses this library:
  [`../elegy-cli/DISTRIBUTION.md`](../elegy-cli/DISTRIBUTION.md)
