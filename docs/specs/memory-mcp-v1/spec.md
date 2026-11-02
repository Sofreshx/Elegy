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

### Explicit shared-agent reads (opt-in)

The stdio host may name a shared space and an explicit JSON array of agent IDs.
The configured caller must occur in that array; empty, duplicate or blank IDs
are invalid. Without this configuration, existing read bindings stay private.
This is a local trusted-host configuration, not caller-supplied authorization.
Each distinct actor requires its own stdio host binding. One cloud plugin bound
to one process identifies that process's actor, not every upstream human/agent.

The database is the data boundary. The space name is a display label, not an
additional row partition. Only agent-scoped rows owned by listed agents, with
no tenant or user binding, become shared. The current actor retains its prior
access to its own bound rows. Unlisted owners, foreign user/tenant-bound
rows and other owners' wider scopes remain inaccessible. Search authorization
must precede access tracking, promotion, result truncation and score logging.

Content and explicit lifecycle mutations remain restricted to the current actor's agent-scoped rows. This
includes indirect merge, correction and consolidation targets. Creation and
responses retain the original `agentId`; version/correction records identify
the acting agent. A shared reader cannot delete or rewrite another author's
contribution. Permanent deletion remains an explicit host/user decision; tool
availability alone does not authorize deleting real data.
Consolidation preserves row authorship; lifecycle transitions and links do not
yet have a separate actor-attributed audit journal. This slice does not claim
complete per-operation audit coverage.
Search may update retrieval metadata for authorized shared rows; it never
reattributes their authorship. This is distinct from content mutation rights.

The same authorization-before-ranking rule applies to the legacy private
binding that includes unowned rows. SQL must select the configured actor's
rows or unowned rows before a vector candidate cap or result limit. A large
number of foreign vectors cannot hide authorized results, and retrieval
metadata is updated only for the authorized page actually returned. Existing
private read scopes and bound-row visibility are preserved; including unowned
rows does not enable shared-agent configuration.

Acceptance uses fictional databases only: two allowed actors can read each
other while retaining attribution; an excluded actor, wider scope and
foreign tenant/user bindings remain hidden; search cannot touch hidden rows; direct
and indirect cross-author mutations fail or remain confined to owned rows;
invalid configuration fails closed; default behavior remains private. No
migration, real-data activation or readiness promotion follows from these tests.

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
