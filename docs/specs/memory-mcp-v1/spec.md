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
- ✅ **Tokio runtime** — `plugins/memory/src/runtime.rs` exposes a single lazily-built, process-wide shared runtime (`ELEGY_RUNTIME_WORKER_THREADS` to size it). The CLI's `run_async`, the re-embed migration closure, and `SqliteMemoryStore::run_store_future` all delegate to it instead of building their own `Builder::new_current_thread()` per call; both MCP binaries call `elegy_memory::runtime::block_on` from `fn main()` instead of `#[tokio::main]`.

### 2. Observability

`hosts/memory-mcp/src/observability.rs` is the shared subscriber/instrumentation module for both binaries. Production requirements:

- ✅ Spans on every tool call (tool name, duration, result) — `instrumented_tool()`, wrapped around all 9 tool methods in `server.rs`
- ✅ Log memory IDs and gate decisions; never log memory content by default (matches AGENTS.md boundary) — the existing `WriteAuditor` path logs tool/id/scope/agent only; `content_logging_enabled()` backs `ELEGY_MCP_LOG_CONTENT` for any future content-bearing log path, off by default
- ✅ Structured JSON logging to stderr (stdio transport: stdout is protocol) — `ELEGY_MCP_LOG_FORMAT=json` (default) or `=text`; the HTTP binary also moved off stdout onto stderr

### 3. Error handling

Errors are mapped via `map_store_error()` (`hosts/memory-mcp/src/memory_tools.rs`, line number drifts with the file — search rather than hardcode it). Production requirements:

- ✅ Distinguish validation errors (`invalid_params`) from internal errors (`internal_error`)
- ✅ Return meaningful error messages without leaking internal paths or API keys — `map_store_error` no longer forwards raw `StoreError` text; `Sqlite`/`Serialization`/`Migration` become generic category messages (the `Serialization` case could otherwise have echoed stored memory content), `Validation` messages go through `hosts/memory-mcp/src/memory_tools/redaction.rs` (URLs, Windows/Unix paths, API-key-shaped tokens). Full detail is still logged server-side via `tracing::warn!`.
- ✅ Circuit breaker for embedding provider failures (shared with CLI) — `elegy_memory::embedding::CircuitBreakerEmbeddingProvider`, wrapping both Ollama/OpenAI providers in the CLI and the stdio binary; `ELEGY_EMBEDDING_BREAKER_THRESHOLD` / `ELEGY_EMBEDDING_BREAKER_COOLDOWN_SECONDS` tune it.

### 4. Testing

Tests cover: store with embedding, store without provider, semantic search, agent isolation, embedding boot policy (require/prefer/off), consolidation, plus the following. Production requirements:

- ✅ Concurrency test: multiple simultaneous MCP clients against same DB — `hosts/memory-mcp/tests/concurrency.rs`. Writing it surfaced and fixed a real pre-existing bug: `SqliteMemoryStore::store()` could commit a memory row and then fail the whole call from a later, unrelated embedding-cache/embedding-persist step, so a client-visible error did not always mean nothing was written. Both steps now degrade the same way `generate_embedding` already did (log a warning, leave the embedding stale) instead of failing the write.
- ✅ Error path: corrupt DB, permission denied (provider-offline is already covered by the boot-policy tests) — `hosts/memory-mcp/tests/error_paths.rs` (permission-denied case is Unix-only; see the file for why a Windows equivalent isn't attempted)
- ✅ Signal handling: graceful shutdown on SIGTERM/SIGINT — `hosts/memory-mcp/tests/shutdown.rs` (Unix-only for the same reason); implemented in `stdio_main.rs`'s `run_until_shutdown`/`wait_for_os_signal` and `main.rs`'s `shutdown_signal`
- ✅ Large corpus: performance benchmark at 10k/100k memories — `hosts/memory-mcp/tests/large_corpus_bench.rs`, opt-in via `ELEGY_MEMORY_BENCH_SCALE` (self-skips otherwise, so it never affects normal `cargo test`)

**Known follow-up, deliberately not addressed here:** `SqliteMemoryStore` serializes writes through one `Arc<Mutex<Connection>>` per store instance and leans on SQLite's WAL `busy_timeout` for contention *between* separate connections (as multiple concurrent MCP clients would be). That timeout does not fully absorb a genuinely concurrent multi-connection write burst — a `"database is locked"` error can surface almost immediately rather than after the configured wait. The concurrency test above tolerates that specific, narrow error class rather than treating it as a failure; the write itself is never lost or duplicated when it happens (verified by the test), but write availability under heavy concurrent load from independent connections is bounded by this architecture. A connection-pooled or single-writer-queue redesign would be the fix; out of scope for this spec.

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
