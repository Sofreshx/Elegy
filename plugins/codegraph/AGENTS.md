# Elegy Codegraph

Portable codebase graph extraction and query CLI for TypeScript and Rust.

## Build

```bash
cargo build -p elegy-codegraph
```

## Test

```bash
cargo test -p elegy-codegraph
```

## Lint

```bash
cargo clippy -p elegy-codegraph -- -D warnings
```

## Architecture

- `ir.rs` — Graph IR types mirroring `contracts/schemas/elegy-codegraph.graph.v0.json`
- `store.rs` — redb-backed persistence (entities, edges, multimap neighbor tables)
- `extractor/ts.rs` — TypeScript Compiler API extraction
- `extractor/rust_lang.rs` — tree-sitter-rust + cargo metadata (always-on)
- `extractor/rust_scip.rs` — SCIP from rust-analyzer (opt-in, augments rust_lang)
- `query.rs` — symbol lookup, neighbors, impact, summary queries
- `main.rs` — clap CLI with `extract` and `query` subcommands

## Design decisions

- **Storage:** redb 4.1 (ACID/MVCC, stable on-disk format) over sled 0.34 (2021 beta, breaking on-disk format). See design doc in parent spec.
- **Rust semantics:** SCIP from rust-analyzer (one-shot artifact, no LSP lifecycle) over full LSP client.
- **Deferred:** `diff`, `review`, `validate` commands — see `./docs/specs/diff-slice.md`.

## Deferred command stubs

The following subcommands are deferred to a follow-up spec (`./docs/specs/diff-slice.md`):
- `diff` — structural diff between two graph snapshots
- `review` — rule-pack-based code review against the graph
- `validate` — graph freshness, schema compliance, provenance health
