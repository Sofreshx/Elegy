---
title: Memory MCP v1
status: active
owner: Elegy Memory MCP
---

# Memory MCP v1

## Contract

MCP transport adapters exposing `elegy-memory` capabilities. Two binaries exist (`elegy-memory-mcp-stdio` and `elegy-memory-mcp-http`) and this spec documents the production-ready surface. The MCP layer adapts — it does not define new memory authority, salience, correction, or scope behavior.

## Existing Surface

The `hosts/memory-mcp/` crate already implements:

| Tool | MCP operation | Status |
|---|---|---|
| `memory_store` | `tools/call` | ✅ Implemented (gate-aware, merge/contradiction/archive/accept/reject) |
| `memory_search` | `tools/call` | ✅ Implemented (hybrid search, agent-isolated) |
| `memory_recall` | `tools/call` | ✅ Implemented (get by ID) |
| `memory_list` | `tools/call` | ✅ Implemented (filtered listing) |
| `memory_stats` | `tools/call` | ✅ Implemented (counts, contradictions, type distribution) |
| `memory_update` | `tools/call` | ✅ Implemented (content update via update_content) |
| `memory_correct` | `tools/call` | ✅ Implemented (gate-aware correction) |
| `memory_delete` | `tools/call` | ✅ Implemented (hard delete) |
| `memory_consolidate` | `tools/call` | ✅ Implemented (simple consolidator, agent-scoped) |
| Agent isolation | MemoryBinding | ✅ Implemented (namespace + agent_id, scope override rejection at the JSON-parsing boundary) |
| Embedding status | EmbeddingStatus return | ✅ Implemented (Ready/Failed/SkippedNoProvider) |

## Productionization Scope

### 1. Shared runtime contract

Both CLI (`elegy-memory`) and MCP (`elegy-memory-mcp-*`) share the same `elegy-memory` library crate as dependency. They must also share:

- ✅ **Provider implementations** — both use `elegy_memory::embedding::*` and `elegy_memory::llm::*`
- ✅ **Store initialization** — both call `SqliteMemoryStore::new()`
- ✅ **Salience gate** — both use `DefaultSalienceGate`
- ⬜ **Tokio runtime** — after Phase 1.1 hardening, both must use a single shared runtime rather than per-call `Builder::new_current_thread()`

### 2. Observability

The MCP crate already depends on `tracing` and `tracing-subscriber` (`hosts/memory-mcp/Cargo.toml:32-33`). Production requirements:

- ⬜ Spans on every tool call (tool name, duration, result)
- ⬜ Log memory IDs and gate decisions; never log memory content by default (matches AGENTS.md boundary)
- ⬜ Structured JSON logging to stderr (stdio transport: stdout is protocol)

### 3. Error handling

Current errors are mapped via `map_store_error()` (`hosts/memory-mcp/src/memory_tools.rs`, line number drifts with the file — search rather than hardcode it). Production requirements:

- ✅ Distinguish validation errors (`invalid_params`) from internal errors (`internal_error`)
- ⬜ Return meaningful error messages without leaking internal paths or API keys
- ⬜ Circuit breaker for embedding provider failures (shared with CLI from Phase 1.4)

### 4. Testing

Current tests cover: store with embedding, store without provider, semantic search, agent isolation, embedding boot policy (require/prefer/off), consolidation. Production requirements:

- ⬜ Concurrency test: multiple simultaneous MCP clients against same DB
- ⬜ Error path: corrupt DB, permission denied (provider-offline is already covered by the boot-policy tests)
- ⬜ Signal handling: graceful shutdown on SIGTERM/SIGINT
- ⬜ Large corpus: performance benchmark at 10k/100k memories

### 5. Embedding availability

- ✅ **Boot policy** — `ELEGY_EMBEDDING_BOOT_POLICY` (`require` / `prefer` / `off`) controls whether the stdio binary fails fast, retries then degrades, or starts degraded immediately. See [Memory MCP configuration](../../../hosts/memory-mcp/docs/CONFIG.md).

## Transport Boundaries

| Feature | stdio | HTTP |
|---|---|---|
| Authentication | None (local subprocess) | OAuth 2.1 + Bearer JWT, or explicit loopback-only no-auth |
| Network | None | Streamable HTTP |
| OAuth/DCR | MUST NOT exist | OAuth metadata + DCR + consent |
| Scope override rejection | Same as HTTP | Same |
| Agent isolation | Same as HTTP | Same |

## Acceptance Criteria

- Every tool listed under Existing Surface is registered on both `elegy-memory-mcp-stdio` and `elegy-memory-mcp-http` and covered by at least one passing test in `hosts/memory-mcp`.
- `cargo test -p elegy-memory-mcp` passes with zero failures.
- A raw JSON tool-call argument containing any key matching `scope`, `namespace`, `agent`, or `agentId` (case-insensitively, ignoring non-alphanumeric characters) is rejected before it reaches a typed args struct, for every tool.
- Items marked ⬜ above do not block `status: active`; they are the acceptance bar for this spec to move toward a stricter status (e.g. a future `status: stable`), not preconditions for the current implemented surface.

## Validation

```bash
cargo test -p elegy-memory-mcp
cargo run -p elegy-memory-mcp-stdio -- --help
cargo run -p elegy-memory-mcp-http -- --help
```

When `elegy-memory` library behavior changes, run `cargo test -p elegy-memory` as upstream validation.
