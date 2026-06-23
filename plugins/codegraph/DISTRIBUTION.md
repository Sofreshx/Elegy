# `elegy-codegraph` — distribution

## What this binary does

Portable codebase graph extraction and query CLI for TypeScript and Rust.
Extracts an entity/edge graph (symbols, files, types, edges), persists it in
a redb store, and answers symbol/edge/impact/summary queries.

This binary is **not** a plugin package and has no `distribution/surfaces.json`
entry for release as a plugin. The portable contract for its graph IR is
`contracts/schemas/elegy-codegraph.graph.v0.json`.

## Binary surface

- **Crate:** `rust/features/elegy-codegraph/`
- **Binary name:** `elegy-codegraph` (auto-discovered from `src/main.rs`)
- **Source:** `rust/features/elegy-codegraph/src/main.rs`
- **Library consumers:** the umbrella `elegy` CLI does not currently dispatch
  codegraph commands; this binary stands alone.
- **Publishability:** the crate's `Cargo.toml` does not set `publish = false`
  because `elegy-codegraph` is intended to be publishable to crates.io.
  This is the only feature crate with that posture; see
  `docs/distribution.md` for the workspace policy note.

## Distribution shape

- **CLI archive asset family:** `elegy-codegraph-<cliVersion>-<target>.zip`
- **Versioning:** follows workspace `version`.
- **Plugin package:** none — codegraph ships as a standalone graph tool, not
  a plugin.

## Install

```bash
# Canonical installer (recommended)
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-codegraph -Force
```

```powershell
# Native-pwsh entry point: thin shim that forwards all args to bash (requires bash in PATH)
pwsh ./scripts/install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-codegraph -Force
```

## Build from source

```bash
cd rust
cargo build -p elegy-codegraph
cargo run -p elegy-codegraph -- --help
```

## Validation

- `cargo test -p elegy-codegraph` (covers `integration_tests.rs`)

## Where to read more

- Crate architecture and design decisions:
  [`rust/features/elegy-codegraph/AGENTS.md`](./AGENTS.md)
- Deferred subcommand specs: [`docs/specs/diff-slice.md`](./docs/specs/diff-slice.md),
  [`docs/specs/plugin-research.md`](./docs/specs/plugin-research.md)
- Graph IR contract: [`contracts/schemas/elegy-codegraph.graph.v0.json`](../../../../contracts/schemas/elegy-codegraph.graph.v0.json)
