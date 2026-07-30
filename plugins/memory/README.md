# Elegy Memory

**Readiness: implemented; not agent-routable.** The scoped SQLite store,
retrieval, correction, provenance, and optional embedding paths work in source
tests. Cross-host installed value and provider-backed behavior are not proven.
Build with `cargo build -p elegy-memory`; invoke `elegy-memory --help` with an
explicit local database scope. This is a stateful product/tool; MCP transports
are separate host adapters.

