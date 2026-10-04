---
name: elegy-memory
description: Bounded local non-authoritative memory operations over the Elegy memory CLI surface.
version: "2.0"
---

# Elegy Memory

Bounded local non-authoritative memory operations over the Elegy memory CLI surface.

This retained reference does not establish executable discovery or readiness.
Memory is currently **implemented; not agent-routable**. For source development,
start with [AGENTS.md](../../AGENTS.md) and the
[contributor guide](../../CONTRIBUTING.md). Use the current checkout's CLI help
for supported arguments: `cargo run --locked -p elegy-memory -- --help` from
the repository root. Development experiments use an explicit scratch `--db`
and `--scope` as shown in the contributor guide.

## Selected CLI operations

- `add`: Add a distilled local memory with type, importance, provenance and scope.
- `search`: Search local memories with keyword matching and provider-backed embeddings when configured; updates retrieval tracking.
- `list`: List local memories by type, state, exact scope and limit.
- `inspect`: Inspect a memory and its version and correction history.
- `purge`: Purge the configured memory database using the CLI's confirmation flow.
- `health`: Show health and count summaries for the configured memory scope.
- `export`: Export memories as JSON or a supported portable format.
- `reembed`: Regenerate and persist stale embeddings using a configured provider; this is a write operation, not a preview.
- `contradictions`: List unresolved contradiction records, or use its `resolve` subcommand.

The [interface map](../../docs/architecture/traits-and-interfaces.md) links
these workflows to their Rust owners and tests. This reference is not a full
command inventory; the CLI help includes the other operations.
