# `elegy-host-mcp` — distribution

## What this crate is

`hosts/host-mcp/` ships host-side adapter binaries:

- `elegy-run` is the MCP host adapter surface for MCP-native clients.

The crate also exposes the host-side transport library used by that binary.

## Binary Surface

- **Crate:** `hosts/host-mcp/`
- **Binaries:** `elegy-run`
- **Library:** `elegy-host-mcp`
- **Source files:** `src/main.rs`, `src/lib.rs`, `host.rs`, `error.rs`

## Distribution shape

- **Standalone binary assets:** `elegy-run-<target>[.exe]`
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

## Workflow ownership

Workflow preparation and result writeback live in `elegy-planning`. Codex and
Holon hosts own native worker/session execution and call the portable
`orchestrator-dispatch/v1` and `orchestrator-worker-result/v1` contracts. This
host crate does not ship a generic subprocess workflow runner.

## Validation

- `cargo test -p elegy-host-mcp`
- `cargo run -p elegy-host-mcp --bin elegy-run -- --help`

## Where to read more

- MCP architecture and ownership rules:
  [`docs/architecture/mcp-skill-tooling-placement.md`](../../docs/architecture/mcp-skill-tooling-placement.md)
- Distribution index:
  [`../../docs/distribution.md`](../../docs/distribution.md)
