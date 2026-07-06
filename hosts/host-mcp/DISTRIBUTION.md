# `elegy-run` — distribution

## What this crate is

`hosts/host-mcp/` ships the `elegy-run` binary. It is the MCP host adapter
surface for MCP-native clients.

The crate also exposes the host-side transport library used by that binary.

## Binary Surface

- **Crate:** `hosts/host-mcp/`
- **Binary:** `elegy-run`
- **Library:** `elegy-host-mcp`
- **Source files:** `src/main.rs`, `src/lib.rs`, `host.rs`, `error.rs`

## Distribution shape

- **Standalone binary asset:** `elegy-run-<target>[.exe]`
- **Versioning:** follows workspace `version`.
- **Plugin package:** none — the host is a transport adapter library, not a
  portable package surface.

## Install

Install the standalone binary asset with the generic installer.

```bash
bash ./scripts/install-distribution.sh --tag vX.Y.Z --destination ./tools/elegy --surface elegy-run --force
```

## Build from source

```bash
cargo build -p elegy-host-mcp
cargo run -p elegy-host-mcp --bin elegy-run -- --help
```

## Validation

- `cargo test -p elegy-host-mcp`
- `cargo run -p elegy-host-mcp --bin elegy-run -- --help`

## Where to read more

- MCP architecture and ownership rules:
  [`docs/architecture/mcp-skill-tooling-placement.md`](../../../../docs/architecture/mcp-skill-tooling-placement.md)
- Distribution index:
  [`../../docs/distribution.md`](../../docs/distribution.md)
