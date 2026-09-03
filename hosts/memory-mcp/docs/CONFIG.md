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
| `ELEGY_MCP_AGENT_ID` | Optional fixed agent identity. |
| `ELEGY_MCP_READ_SCOPE` | Optional readable scope range; defaults to `session` and accepts `session`, `workspace`, `user`, or `agent`. Writes remain agent-scoped. |
| `OLLAMA_URL` | Optional; defaults to local Ollama. |
| `ELEGY_EMBEDDING_MODEL` | Optional model name. |
| `ELEGY_EMBEDDING_BOOT_POLICY` | Optional; one of `require`, `prefer` (default), `off`. See below. |
| `ELEGY_ALLOW_NO_EMBEDDINGS` | Deprecated legacy alias for `ELEGY_EMBEDDING_BOOT_POLICY`, read only when the new variable is unset. `true` maps to `off`; `false` maps to `require`. |
| `ELEGY_EMBEDDING_BREAKER_THRESHOLD` | Optional; consecutive embedding-provider failures before the circuit opens and short-circuits further calls instead of paying a full timeout each time. Defaults to 5; `0` disables the breaker. Shared with the CLI. |
| `ELEGY_EMBEDDING_BREAKER_COOLDOWN_SECONDS` | Optional; how long the circuit stays open before a single trial call is allowed through. Defaults to 30. Shared with the CLI. |

Stdio never reads the HTTP authentication variables. Its read binding also
includes memories without an `agent_id`; writes remain limited to the
configured agent scope.

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
