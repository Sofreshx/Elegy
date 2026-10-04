# Elegy Memory

**Readiness: implemented; not agent-routable.** The scoped SQLite store,
retrieval, correction, provenance, and optional embedding paths work in source
tests. Cross-host installed value and provider-backed behavior are not proven.
This is a stateful product/tool; MCP transports are separate host adapters.

## Develop Memory

Start with the [contributor guide](CONTRIBUTING.md): setup, a safe first run,
the source map, and tests selected by the change you are making. Coding agents
start with [AGENTS.md](AGENTS.md), which routes to the same guide and contracts.
Both paths use the same documentation; there is no second copy of the design.

Run from the repository root with the toolchain in
[`rust-toolchain.toml`](../../rust-toolchain.toml):

```text
cargo run --locked -p elegy-memory -- --help
cargo test --locked -p elegy-memory
```

Cargo runs the current checkout without relying on a separately installed
binary. For commands that open a store, always pass an explicit scratch
`--db` and `--scope`; see the [first-run example](CONTRIBUTING.md#try-the-cli-safely).

The [architecture map](docs/architecture/ARCHITECTURE.md) explains the SQLite
engine, portable JSON artifacts, and contextual-recall journal. Read the
[feature matrix](docs/architecture/mvp-scope.md) for implemented and deferred
behavior, and the [qualification guide](docs/qualification.md) for evidence.

## Contextual recall

Opt-in [contextual recall](../../docs/specs/memory-contextual-recall-v1/spec.md)
adds automatic, bounded historical context without touching source memories.
It preserves legacy search/feedback learning and records event-bound feedback
in a separate local journal. Start with the disabled binding fixture and the
[Codex adapter guide](integrations/codex/README.md); installation and live-host
usability remain unproven until explicitly exercised on the target host.

## Qualification

For resumable feature evidence, start with
`cargo run --locked -p elegy-memory -- eval next --json`
from the Elegy source repository. It returns the exact unresolved claim,
protocol, current evidence and next invocation. See the
[qualification guide](docs/qualification.md) for source experiments, installed
MCP observations, paired answer evaluation and durable receipt history.

