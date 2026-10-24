# Memory MCP configuration

## Both binaries

| Variable | Purpose |
|---|---|
| `ELEGY_MCP_LOG_FORMAT` | Optional; `json` (default) or `text`. Both binaries log to `stderr`; an unrecognized value warns and falls back to `json`. |
| `ELEGY_RUNTIME_WORKER_THREADS` | Optional; sizes the shared `elegy-memory` Tokio runtime both binaries and the CLI run on. Clamped to `1..=32`; defaults to `min(available_parallelism, 4)` when unset or unparseable. |

## HTTP binary

Always required:

| Variable | Purpose |
|---|---|
| `ELEGY_MCP_AUTH_MODE` | `local-none` or `external-oauth`. No implicit default. |
| `ELEGY_MCP_DB_PATH` | SQLite memory database path. |
| `ELEGY_MCP_BIND` | Optional IPv4 bind address; defaults to `127.0.0.1`. |
| `ELEGY_MCP_PORT` | Optional port; defaults to `8765`. |
| `ELEGY_MCP_LOG_CONTENT` | Optional boolean; defaults to false. Reserved for any future log path that would otherwise carry memory content — nothing currently logged is gated by it, since content is never logged by default regardless. |

The HTTP binary never constructs an embedding provider, so the embedding
circuit breaker below does not apply to it.

`local-none` refuses a non-loopback bind.

`external-oauth` additionally requires:

| Variable | Purpose |
|---|---|
| `ELEGY_MCP_PUBLIC_URL` | HTTPS public resource-server base URL; normalized with a trailing slash. |
| `ELEGY_MCP_OAUTH_ISSUER` | Exact HTTPS external token issuer with no query or fragment. |
| `ELEGY_MCP_OAUTH_AUDIENCE` | Required access-token audience. |
| `ELEGY_MCP_OAUTH_JWKS_URL` | HTTPS external issuer JWKS endpoint. Keys must be unique ES256 or RS256 public signature keys. |
| `ELEGY_MCP_OAUTH_SCOPES` | Comma-separated scopes required on every MCP request. |

Memory has no admin password, OAuth data directory, client registry, token
store, or signing-key setting.

## Stdio binary

| Variable | Requirement |
|---|---|
| `ELEGY_DB_PATH` | Required SQLite path. |
| `ELEGY_MCP_AGENT_ID` | Fixed agent identity; explicit value required when shared reads are enabled. |
| `ELEGY_MCP_SHARED_SPACE` | Opt-in shared-space label; must be paired with `ELEGY_MCP_SHARED_AGENTS`. Labels do not partition database rows. |
| `ELEGY_MCP_SHARED_AGENTS` | JSON array of distinct, nonblank agent IDs explicitly allowed to share reads. Must include this host's fixed `ELEGY_MCP_AGENT_ID`. Unset means no cross-agent sharing. |
| `ELEGY_MCP_READ_SCOPE` | Optional readable scope range; defaults to `session` and accepts `session`, `workspace`, `user`, or `agent`. Writes remain agent-scoped. |
| `OLLAMA_URL` | Optional; defaults to local Ollama. |
| `ELEGY_EMBEDDING_MODEL` | Optional model name. |
| `ELEGY_EMBEDDING_BOOT_POLICY` | Optional; one of `require`, `prefer` (default), `off`. See below. |
| `ELEGY_ALLOW_NO_EMBEDDINGS` | Deprecated legacy alias for `ELEGY_EMBEDDING_BOOT_POLICY`, read only when the new variable is unset. `true` maps to `off`; `false` maps to `require`. |
| `ELEGY_EMBEDDING_BREAKER_THRESHOLD` | Optional; consecutive embedding-provider failures before the circuit opens and short-circuits further calls instead of paying a full timeout each time. Defaults to 5; `0` disables the breaker. Shared with the CLI. |
| `ELEGY_EMBEDDING_BREAKER_COOLDOWN_SECONDS` | Optional; how long the circuit stays open before a single trial call is allowed through. Defaults to 30. Shared with the CLI. |

Stdio never reads the HTTP authentication variables. Without shared-agent
configuration its read binding also includes memories without an `agent_id`.
Writes require the configured agent's own agent-scoped row.

### Shared agents, distinct authors

Configure each authorized actor's process with its own `ELEGY_MCP_AGENT_ID`,
the same explicit database path, shared-space label and JSON owner allowlist.
Both shared variables must be present together. Invalid configuration stops
before the database opens. The caller must belong to the list. Only listed
agent-scoped rows without tenant/user bindings become shared. The current
actor retains access to its own bound rows; unowned rows
are excluded in this mode. Other owners' wider scopes remain private.

The database path and owner allowlist define the boundary: the space label is
not a per-row project filter. Only enable a database whose listed owners'
agent-scoped contributions are intended for the shared audience. No existing
row is migrated or reattributed. `agentId` in recall/list/search/store results
identifies the original author. Updates/corrections record the acting agent.
Direct mutations, correction merges and consolidation remain owner-only.
Consolidation keeps original authors; its lifecycle transitions and links do
not yet have a separate actor-attributed audit journal.
Search can still update authorized rows' retrieval metadata and promotion.
Owner/access-filtered vector searches scan authorized vectors before selecting
results, so private neighbors cannot exhaust a global KNN candidate pool.
Vector scoring scales with the authorized corpus; SQL filtering also depends
on database size and indexes. This does not establish a latency guarantee.

The [fictional configuration](shared-agent-config.example.json) illustrates
two local hosts with distinct actors and all nine existing tools. One shared
cloud plugin bound to one host still has one fixed actor. Distinct upstream
agents require distinct authenticated host bindings; tool arguments cannot
override the actor. These settings do not authenticate upstream callers,
grant OpenAI permissions, install a service or activate the tunnel.

Before real activation, use SQLite's online backup API and verify restoration
on a separate path with the installed Memory binary. Keep the original data
and previous host configuration for rollback. Stopping the tunnel and revoking
its dedicated key/connection withdraws cloud access without deleting the DB.
`memory_delete` is permanent: exposing the tool does not authorize deleting
real contributions. Keep test fixtures separate from the real database.

### Embedding boot policy

The embedding provider probe is `GET <OLLAMA_URL>/api/tags` with a 5-second
timeout, followed by a check that `ELEGY_EMBEDDING_MODEL` is present in the
response. `ELEGY_EMBEDDING_BOOT_POLICY` controls what happens when that probe
fails:

- **`require`** — single attempt; on failure the binary exits with code `1`
  and prints a remediation message on `stderr`.
- **`prefer`** (default) — up to 3 attempts with a 2-second backoff between
  them. If every attempt fails, the server logs a warning and starts anyway
  with the embedding provider disabled: `memory_search` falls back to
  keyword/FTS ranking, and `memory_store` reports
  `embeddingStatus: "skipped_no_provider"`. The policy never re-probes after
  startup; recovering embeddings requires a restart.
- **`off`** — the probe never runs; the server starts degraded immediately.
