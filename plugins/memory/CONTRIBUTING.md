---
title: Developing Elegy Memory
status: active
owner: Elegy Memory
doc_kind: guide
---

# Developing Elegy Memory

Use this guide to change `elegy-memory` without reading the whole Elegy
workspace. It is shared by human contributors and coding agents. The short
[agent entrypoint](AGENTS.md) routes here; behavior and acceptance remain in
the linked architecture docs and specs.

## Set up once

Work from the repository root, two levels above this directory. Install Rust
through rustup; [`rust-toolchain.toml`](../../rust-toolchain.toml) selects the
toolchain, rustfmt and Clippy. Building bundled SQLite and sqlite-vec also
requires a native C toolchain (MSVC Build Tools and Windows SDK on Windows,
or the platform C compiler on Linux/macOS). The first Cargo build downloads
locked dependencies. No running database server, API key, Ollama installation,
or MCP host is required for the default Memory tests.

```text
cargo run --locked -p elegy-memory -- --help
cargo test --locked -p elegy-memory
```

Use `cargo run --locked -p elegy-memory -- …` for source development. A binary
on PATH may be an older installation. Cargo builds this checkout, including
the source fingerprint used by qualification. For faster repeated invocations
after building, use `target/debug/elegy-memory` (`.exe` on Windows).

## Try the CLI safely

Store commands default to a persistent home-directory database. Use a fresh
temporary directory for experiments and supply `--db` on **every** store
command. This example uses synthetic content and no remote providers.

PowerShell:

```powershell
$memoryScratch = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $memoryScratch | Out-Null
$memoryDb = Join-Path $memoryScratch 'memory.db'
cargo run --locked -p elegy-memory -- add --db $memoryDb --scope workspace "The demo project uses SQLite." --provenance user-stated --json
cargo run --locked -p elegy-memory -- search --db $memoryDb --scope workspace "SQLite" --json
cargo run --locked -p elegy-memory -- list --db $memoryDb --scope workspace --json
```

POSIX shell:

```sh
memory_scratch=$(mktemp -d)
memory_db="$memory_scratch/memory.db"
cargo run --locked -p elegy-memory -- add --db "$memory_db" --scope workspace "The demo project uses SQLite." --provenance user-stated --json
cargo run --locked -p elegy-memory -- search --db "$memory_db" --scope workspace "SQLite" --json
cargo run --locked -p elegy-memory -- list --db "$memory_db" --scope workspace --json
```

The scratch directory is yours to inspect and remove afterwards. For detailed
arguments use `cargo run --locked -p elegy-memory -- <command> --help`.
`list` inventories an exact scope; `search` sees its documented broader scopes
and updates retrieval counters. Contextual recall uses a separate read-only
source path and separate journal; see the [architecture map](docs/architecture/ARCHITECTURE.md).

## Find the change owner

Read the relevant row, then its contract and implementation. Paths below are
relative to this directory. Test names in the last column are Cargo integration
targets (`--test NAME`) or library filters (`--lib FILTER`).

| Change | Read first | Implementation | Focused tests |
| --- | --- | --- | --- |
| CLI arguments, dispatch, JSON/text output | [machine-envelope conformance tests](tests/conformance.rs) | [cli.rs](src/cli.rs), [main.rs](src/main.rs) | `--test cli`, `--test conformance` |
| Stored fields, traits, metadata | [memory model](docs/architecture/memory-model.md), [interface map](docs/architecture/traits-and-interfaces.md) | [types.rs](src/types.rs), [traits.rs](src/traits.rs), [error.rs](src/error.rs) | `--test integration`, `--lib storage::sqlite_store::tests` |
| Search, ranking, scope visibility, feedback weights | [memory model](docs/architecture/memory-model.md), [eval spec](../../docs/specs/eval-harness-v1/spec.md) | [sqlite_store.rs](src/storage/sqlite_store.rs), [decay.rs](src/decay.rs), [similarity.rs](src/similarity.rs) | `--lib storage::sqlite_store::tests`, `--test eval` |
| Write gate, contradiction, correction | [memory model](docs/architecture/memory-model.md) | [gate.rs](src/gate.rs), [sqlite_store.rs](src/storage/sqlite_store.rs), [cli.rs](src/cli.rs) | `--lib gate::tests`, `--test cli`, `--test eval` |
| Schema or migrations | [storage schema](docs/architecture/storage-schema.md), [migration framework](docs/architecture/migration-framework.md) | [schema.rs](src/storage/schema.rs) | `--lib storage::schema::tests`, `--test integration` |
| Budget, forgetting, promotion, consolidation | [feature matrix](docs/architecture/mvp-scope.md), [interface map](docs/architecture/traits-and-interfaces.md) | [forgetting.rs](src/forgetting.rs), [promotion.rs](src/promotion.rs), [consolidator.rs](src/consolidator.rs), [sqlite_store.rs](src/storage/sqlite_store.rs) | `--lib`, `--test integration`, `--test qualification` |
| Embedding or LLM adapter | [interface map](docs/architecture/traits-and-interfaces.md) | [embedding](src/embedding/mod.rs), [llm](src/llm/mod.rs), [runtime.rs](src/runtime.rs) | `--lib embedding`, `--lib llm`, `--test cli` |
| Portable summary-only JSON and governed lifecycle | [contract versioning ADR](../../docs/adr/2026-07-08-memory-contract-versioning.md) | [artifacts.rs](src/artifacts.rs), [local_store.rs](src/local_store.rs), [schema](schemas/summary-only-session-context-envelope.schema.json), [fixture](fixtures/summary-only-session-context-envelope.minimal.json) | `--test governed_memory`, `--test local_store`, `--test conformance` |
| Contextual recall or event-bound feedback | [recall spec](../../docs/specs/memory-contextual-recall-v1/spec.md) | [recall.rs](src/recall.rs), [recall_store.rs](src/storage/recall_store.rs), [config schema](schemas/contextual-recall-config.schema.json) | `--test contextual_recall`, `--test qualification` |
| Evaluation corpus, thresholds, qualification receipts | [eval spec](../../docs/specs/eval-harness-v1/spec.md), [qualification guide](docs/qualification.md) | [eval/mod.rs](src/eval/mod.rs), [qualification.rs](src/eval/qualification.rs), [fingerprint.rs](src/eval/fingerprint.rs), [thresholds](eval-harness-v1.json) | `--test eval`, `--test qualification` |
| Codex recall hook | [adapter guide](integrations/codex/README.md) | [recall_hook.py](integrations/codex/recall_hook.py) | Python command below |

The SQLite engine and the governed JSON artifact store are separate paths.
Do not put retrieval changes in `local_store.rs`. `src/lib.rs` preserves their
public re-exports. Large unit-test modules live next to their implementation
in `cli/tests.rs`, `gate/tests.rs`, `storage/schema/tests.rs` and
`storage/sqlite_store/tests.rs`; the test names retain the same module filters.

MCP transport, authentication and host policy belong to
[`hosts/memory-mcp`](../../hosts/memory-mcp/AGENTS.md). Only enter that crate
when the change crosses its boundary. Its validation is
`cargo test --locked -p elegy-memory-mcp`; Memory itself does not require it to run.

## Validate the change

Run the smallest relevant tests while editing, then the complete Memory suite
for a change to Rust behavior or module organization:

```text
cargo test --locked -p elegy-memory --lib storage::schema::tests
cargo test --locked -p elegy-memory --test contextual_recall
cargo test --locked -p elegy-memory
cargo fmt -p elegy-memory -- --check
cargo clippy --locked -p elegy-memory --all-targets -- -D warnings
```

The first two commands are examples; select the targets from the table.
Integration tests use the just-built executable or crate and synthetic,
temporary data. Provider tests use stubs/local servers. They do not establish
live-provider or installed-host readiness. Regression evaluation is covered by
`--test eval`; for its readable standalone report run:

```text
cargo run --locked -p elegy-memory -- eval run --ci
```

For hook edits, Python 3 with its standard library is sufficient:

```text
python -m unittest discover -s plugins/memory/integrations/codex -p "test_*.py"
```

For governed schema/fixture changes, also run the repository contract check:

```text
cargo run --locked -p elegy-core --bin elegy-contracts -- --project . contracts validate
```

For documentation changes, run the repository documentation checker and verify
the changed local Markdown links (including source files and heading anchors):

```text
cargo run --locked -p elegy-documentation -- check --project . --json
```

The shared checker may report pre-existing findings outside Memory. Report
those separately; they do not authorize expanding a Memory-only task.

## Common development traps

- **Eval latency failure:** the regression harness includes wall-clock limits.
  Preserve the failing metrics. To check contention between benchmark tests,
  rerun `cargo test --locked -p elegy-memory --test eval -- --test-threads=1`;
  report that result separately from the default run. The dedicated CI eval
  job runs in release mode; compare with
  `cargo run --locked --release -p elegy-memory -- eval run --ci`.
  Do not lower thresholds to hide a timing failure.
- **Stale qualification after a refactor:** source paths and bytes participate
  in the fingerprint. Rebuild, then use `eval status --json` / `eval next --json`.
  Retain prior receipts, including failures; a refactor does not requalify them.
- **Wrong store:** CLI `--db` selects SQLite; `LocalMemoryStore` manages governed
  JSON records. Recall's journal is a third, separate database with its own
  application ID and version.
- **Missing vectors:** writes can succeed when embedding fails. Preserve the
  stale-embedding and keyword-fallback behavior; use provider fixtures before
  involving an actual service.
- **API vs host policy:** keep retrieval and persistence here. Host approvals,
  authorization and freshness decisions belong to the caller/host contracts.
- **Parallel builds:** let an active Cargo process finish rather than starting
  duplicate builds against the same target directory.

For resumable experiments use the [qualification guide](docs/qualification.md).
Its evidence directory is persistent by default; use `--evidence-dir` with a
scratch directory for disposable experiments. Readiness remains governed by
[`readiness.json`](readiness.json) and the [generated repository view](../../docs/readiness.md).
