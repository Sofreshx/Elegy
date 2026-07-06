---
name: elegy-memory
description: Bounded local non-authoritative memory operations over the Elegy memory CLI surface.
version: "2.0"
---

# Elegy Memory

Bounded local non-authoritative memory operations over the Elegy memory CLI surface.

## Capabilities

- `memory-add`: Add a distilled local memory with explicit type, importance, provenance, scope, and optional database path.
- `memory-search`: Search local memories with keyword matching and provider-backed embeddings when configured.
- `memory-list`: List local memories by type, state, scope, and limit.
- `memory-inspect`: Inspect a memory and its version history.
- `memory-purge`: Purge the configured memory database after explicit confirmation.
- `memory-health`: Show health and count summaries for the configured memory scope.
- `memory-export`: Export memories as JSON to stdout or a file.
- `memory-reembed`: Preview re-embedding of stale memories when a provider is configured.
- `memory-contradictions`: List unresolved contradiction records for the configured memory scope.
