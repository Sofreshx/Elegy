# Memory MCP configuration

## HTTP binary

Always required:

| Variable | Purpose |
|---|---|
| `ELEGY_MCP_AUTH_MODE` | `local-none` or `external-oauth`. No implicit default. |
| `ELEGY_MCP_DB_PATH` | SQLite memory database path. |
| `ELEGY_MCP_BIND` | Optional IPv4 bind address; defaults to `127.0.0.1`. |
| `ELEGY_MCP_PORT` | Optional port; defaults to `8765`. |
| `ELEGY_MCP_LOG_CONTENT` | Optional boolean; defaults to false. |

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
| `OLLAMA_URL` | Optional; defaults to local Ollama. |
| `ELEGY_EMBEDDING_MODEL` | Optional model name. |
| `ELEGY_ALLOW_NO_EMBEDDINGS` | Explicit degraded mode when true. |

Stdio never reads the HTTP authentication variables.
