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
- Defaults `OLLAMA_URL` to `http://localhost:11434`.
- Logs to `stderr` so `stdout` stays reserved for the MCP protocol.

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
