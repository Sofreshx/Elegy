# elegy-memory-mcp configuration

The crate has separate env surfaces for the HTTP binary and the stdio binary.

## Transport split

Use the transport-specific variables below instead of assuming one shared startup contract.

| Variable | `elegy-memory-mcp-http` | `elegy-memory-mcp-stdio` | Notes |
|---|---|---|---|
| `ELEGY_MCP_ADMIN_PASSWORD` | Required | Not used | Consent password for the HTTP OAuth flow. |
| `ELEGY_MCP_DB_PATH` | Required | Not used | SQLite path used by the HTTP binary. |
| `ELEGY_MCP_PUBLIC_URL` | Required | Not used | Public base URL for OAuth metadata and auth headers. |
| `ELEGY_MCP_PORT` | Optional | Not used | Defaults to `8765`. |
| `ELEGY_MCP_LOG_CONTENT` | Optional | Not used | Defaults to `0`. |
| `ELEGY_MCP_DATA_DIR` | Optional | Not used | Persists signing key, DCR clients, and refresh tokens. |
| `ELEGY_DB_PATH` | Not used | Required | SQLite path used by the stdio binary. |
| `ELEGY_MCP_AGENT_ID` | Not used | Optional | Warns and falls back to `default-agent` when unset. |
| `OLLAMA_URL` | Not used | Optional | Defaults to `http://localhost:11434`. |
| `ELEGY_EMBEDDING_MODEL` | Not used | Optional | Defaults to `nomic-embed-text`. Verified at boot through `OLLAMA_URL/api/tags`. |
| `ELEGY_ALLOW_NO_EMBEDDINGS` | Not used | Optional | Defaults to `false`. Set to `true` to force degraded mode without an embedding provider. |
| `RUST_LOG` | Optional | Optional | Common local default is `info`. |

## HTTP binary

The HTTP binary refuses to start when any required HTTP variable is missing or empty.

### Required

| Variable | Purpose |
|---|---|
| `ELEGY_MCP_ADMIN_PASSWORD` | Admin password checked by the OAuth consent flow. Provide the password itself; startup derives an Argon2 verifier in memory and rejects pre-hashed Argon2 strings. |
| `ELEGY_MCP_DB_PATH` | Path to the SQLite database used by `elegy-memory`. |
| `ELEGY_MCP_PUBLIC_URL` | Public URL used by OAuth metadata, bearer headers, and connector flows. |

### Optional

| Variable | Default | Notes |
|---|---|---|
| `ELEGY_MCP_PORT` | `8765` | Must be an integer from `1` to `65535`. |
| `ELEGY_MCP_LOG_CONTENT` | `0` | Boolean parser accepts `0/1`, `true/false`, `yes/no`, `on/off`. |
| `ELEGY_MCP_DATA_DIR` | `directories::ProjectDirs::from("com", "holon", "elegy-mcp").data_local_dir()` | On Windows this resolves under `%LOCALAPPDATA%\\holon\\elegy-mcp`. |

## stdio binary

The stdio binary is local-only and does not read the HTTP/OAuth variables.

### Required

| Variable | Purpose |
|---|---|
| `ELEGY_DB_PATH` | Path to the SQLite database used by `elegy-memory`. |

### Optional

| Variable | Default | Notes |
|---|---|---|
| `ELEGY_MCP_AGENT_ID` | `default-agent` | Startup warns on `stderr` if unset before using the fallback. |
| `OLLAMA_URL` | `http://localhost:11434` | Local Ollama base URL used for boot verification, write-time embeddings, and semantic search. |
| `ELEGY_EMBEDDING_MODEL` | `nomic-embed-text` | Boot fails if this model is absent from `OLLAMA_URL/api/tags` unless degraded mode is enabled. |
| `ELEGY_ALLOW_NO_EMBEDDINGS` | `false` | When `true`, startup skips Ollama checks, semantic search is disabled, and `memory_store` returns `embeddingStatus: "skipped_no_provider"`. |
| `RUST_LOG` | implementation default | Use `info` for normal local startup and increase only for debugging. |

### Boot behavior

Normal mode validates the embedding provider before the MCP transport starts:

1. `GET <OLLAMA_URL>/api/tags` with a 5-second timeout
2. verify that `ELEGY_EMBEDDING_MODEL` is present
3. start normally only when both checks pass

If either check fails, the stdio binary exits with code `1` and prints a remediation message on `stderr`.

## Logging

Logging behavior depends on the binary.

- `elegy-memory-mcp-http` keeps its structured server logging.
- `elegy-memory-mcp-stdio` logs to `stderr` only so `stdout` remains clean for MCP JSON-RPC traffic.
- Passwords, derived verifiers, tokens, codes, signing keys, and memory content are not logged.
