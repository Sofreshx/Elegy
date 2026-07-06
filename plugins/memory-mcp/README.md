# elegy-memory-mcp

`elegy-memory-mcp` now ships two binaries that expose the same 8 MCP tools over different transports.

## Status

The crate currently supports one remote HTTP/OAuth server and one local stdio server.

| Binary | Transport | Primary use | Auth |
|---|---|---|---|
| `elegy-memory-mcp-http` | MCP Streamable HTTP | Remote Claude connectors | OAuth 2.1 + bearer JWT on `/mcp` |
| `elegy-memory-mcp-stdio` | MCP stdio | Local Claude Desktop and future Holon subprocess hosting | None; local process boundary only |

Shared behavior:

- All 8 tools are exposed:
  - Read: `memory_search`, `memory_recall`, `memory_list`, `memory_stats`
  - Write: `memory_store`, `memory_update`, `memory_correct`, `memory_delete`
- Tool requests stay pinned to `MemoryScope::Agent`.
- Successful write-tool audits log `tool`, `id`, `scope`, and `timestamp`, never memory content.

Tracked follow-up:

- [Review B pending note](docs/PENDING.md): scope-isolation review before any return to public HTTP/OAuth emphasis.

## Transports

Use the HTTP binary for public remote connectors, and the stdio binary for local desktop subprocess consumers.

### `elegy-memory-mcp-http`

The HTTP binary keeps the existing remote shape.

- Serves `/mcp` as MCP Streamable HTTP.
- Protects only `/mcp` with `Authorization: Bearer <jwt>`.
- Leaves OAuth, DCR, and `/.well-known/*` endpoints public.
- Persists auth state under `ELEGY_MCP_DATA_DIR` (`signing-key`, `clients.json`, `refresh-tokens.json`).

### `elegy-memory-mcp-stdio`

The stdio binary is the local transport for desktop MCP clients.

- Expects `ELEGY_DB_PATH`.
- Uses `ELEGY_MCP_AGENT_ID` for the fixed agent scope.
- Warns and falls back to `default-agent` when `ELEGY_MCP_AGENT_ID` is unset.
- Fails fast at boot unless Ollama is reachable and the configured embedding model is available.
- Supports explicit degraded mode via `ELEGY_ALLOW_NO_EMBEDDINGS=true`; in that mode semantic search is unavailable and `memory_store` responses report `embeddingStatus: "skipped_no_provider"`.
- Logs to `stderr` so `stdout` stays reserved for the MCP protocol.

## Prerequisites

For the stdio binary in normal mode, Ollama must be running and the embedding model must be present before startup.

```powershell
ollama list | findstr nomic-embed-text
ollama pull nomic-embed-text
```

When the embedding model is `nomic-embed-text`, Elegy prefixes embedding inputs with the model's task markers:

- stored memory content is embedded as `search_document: <content>`
- search queries are embedded as `search_query: <query>`

Existing databases whose embeddings were generated before this change should be re-embedded before you trust semantic ranking again. Fresh stores are unaffected; older rows should be purged and re-stored or marked stale and re-embedded.

## Environment variables

The binaries intentionally do not use the same env surface.

| Variable | HTTP binary | stdio binary | Default / notes |
|---|---|---|---|
| `ELEGY_MCP_ADMIN_PASSWORD` | Required | Not used | Cleartext consent password; startup derives an Argon2 verifier in memory. |
| `ELEGY_MCP_DB_PATH` | Required | Not used | SQLite path for the HTTP/OAuth server. |
| `ELEGY_MCP_PUBLIC_URL` | Required | Not used | Public base URL used by OAuth metadata and auth flows. |
| `ELEGY_MCP_PORT` | Optional | Not used | Defaults to `8765`. |
| `ELEGY_MCP_LOG_CONTENT` | Optional | Not used | Defaults to `0`. |
| `ELEGY_MCP_DATA_DIR` | Optional | Not used | Persists signing key, DCR clients, and refresh tokens. |
| `ELEGY_DB_PATH` | Not used | Required | SQLite path for the local stdio server. |
| `ELEGY_MCP_AGENT_ID` | Not used | Optional | Warns and falls back to `default-agent` when unset. |
| `OLLAMA_URL` | Not used | Optional | Defaults to `http://localhost:11434`. |
| `ELEGY_EMBEDDING_MODEL` | Not used | Optional | Defaults to `nomic-embed-text`. Boot verifies that the model exists in `OLLAMA_URL/api/tags`. |
| `ELEGY_ALLOW_NO_EMBEDDINGS` | Not used | Optional | Defaults to `false`. Set to `true` to start in degraded mode without an embedding provider. |
| `RUST_LOG` | Optional | Optional | Typical local default is `info`. |

## Consumer configs

Use the provided examples as copy-and-edit starting points.

- [Claude Desktop stdio config example](docs/claude-desktop-config.example.json)
- [Holon stdio config example](docs/holon-mcp-config.example.json)

## Docs

- [Configuration](docs/CONFIG.md)
- [Auth](docs/AUTH.md)
- [Transport](docs/TRANSPORT.md)
- [Deployment](docs/DEPLOYMENT.md)
- [Pending follow-ups](docs/PENDING.md)
- [Architecture overview](docs/architecture/overview.md)

## Troubleshooting

| Symptom | Meaning | Fix |
|---|---|---|
| stdio binary exits immediately with `Ollama not reachable ...` | Ollama was unavailable during boot. | Start Ollama (`ollama serve` or Ollama Desktop), then restart the MCP binary. |
| stdio binary exits immediately with `Model <name> not pulled` | The configured embedding model is missing. | Run `ollama pull <name>` and restart. |
| `memory_store` returns `"embeddingStatus": "failed"` | The memory stored, but embedding generation or indexing did not complete, so semantic recall may miss it. | Check Ollama health/logs and retry or re-embed later. |
| `memory_store` returns `"embeddingStatus": "skipped_no_provider"` | The server is running without an embedding provider. | Disable degraded mode or restore Ollama/model availability if semantic search is required. |
| Semantic ranking looks noisy after upgrading `nomic-embed-text` handling | The database still contains embeddings produced before Elegy started sending `search_document:` / `search_query:` task prefixes. | Re-embed existing rows (or rebuild the test DB) so all vectors are generated with the same task-prefix contract. |
