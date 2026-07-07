# `elegy-codegraph` — distribution

## What this binary does

Portable codebase graph extraction and query CLI for TypeScript and Rust.
Extracts an entity/edge graph (symbols, files, types, edges), persists it in
a redb store, and answers symbol/edge/impact/summary queries.

**This surface is NOT packaged as an `elegy-plugin/v1` plugin.** It ships as a
standalone CLI binary. Its `distribution/surfaces.json` entry uses
`kind: "cli"` and no plugin packaging. The portable contract for its graph IR
is defined in-code at `tools/codegraph/src/ir.rs`.

## Binary surface

- **Crate:** `tools/codegraph/`
- **Binary name:** `elegy-codegraph`
- **Source:** `tools/codegraph/src/main.rs`

## Distribution shape

- **CLI asset family:** `elegy-codegraph-<target>[.exe]`
- **Versioning:** follows workspace `version`.
- **Plugin package:** none — codegraph ships as a standalone graph tool, not a plugin.

## Install

```bash
# Canonical installer
bash ./scripts/install-distribution.sh --tag vX.Y.Z --destination ./tools/elegy --surface elegy-codegraph --force
```

```powershell
# Native-pwsh entry point: thin shim that forwards all args to bash (requires bash in PATH)
pwsh ./scripts/install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy -Surface elegy-codegraph -Force
```

## Build from source

```bash
cargo build -p elegy-codegraph
cargo run -p elegy-codegraph -- --help
```

## Validation

- `cargo test -p elegy-codegraph`

## Where to read more

- Crate boundaries: [`AGENTS.md`](./AGENTS.md)
- Graph IR contract: `tools/codegraph/src/ir.rs`
