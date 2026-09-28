# Elegy Memory

**Readiness: implemented; not agent-routable.** The scoped SQLite store,
retrieval, correction, provenance, and optional embedding paths work in source
tests. Cross-host installed value and provider-backed behavior are not proven.
Build with `cargo build -p elegy-memory`; invoke `elegy-memory --help` with an
explicit local database scope. This is a stateful product/tool; MCP transports
are separate host adapters.

Opt-in [contextual recall](../../docs/specs/memory-contextual-recall-v1/spec.md)
adds automatic, bounded historical context without touching source memories.
It preserves legacy search/feedback learning and records event-bound feedback
in a separate local journal. Start with the disabled binding fixture and the
[Codex adapter guide](integrations/codex/README.md); installation and live-host
usability remain unproven until explicitly exercised on the target host.

