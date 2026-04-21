# elegy-memory-mcp

Remote MCP server for exposing `elegy-memory` to Claude clients through a fixed `claude-ai-remote` namespace.

## Status

WU1-WU9 closeout is documented.

- `/mcp` is served as MCP Streamable HTTP.
- Only `/mcp` requires `Authorization: Bearer <jwt>`.
- OAuth, DCR, and `/.well-known/*` endpoints stay public.
- All 8 tools are exposed:
  - Read: `memory_search`, `memory_recall`, `memory_list`, `memory_stats`
  - Write: `memory_store`, `memory_update`, `memory_correct`, `memory_delete`
- Successful write-tool audits log `tool`, `id`, `scope`, `timestamp`, and `jti`, never memory content.
- Auth state persists under `ELEGY_MCP_DATA_DIR` (`signing-key`, `clients.json`, `refresh-tokens.json`).

WU8 validation passed with:

- `cargo test -p elegy-memory-mcp`
- `cargo clippy -p elegy-memory-mcp --all-targets -- -D warnings`

WU9 is the deployment/docs closeout.

## Configuration

Required environment variables:

- `ELEGY_MCP_ADMIN_PASSWORD`
- `ELEGY_MCP_DB_PATH`
- `ELEGY_MCP_PUBLIC_URL`

Optional environment variables:

- `ELEGY_MCP_PORT` (default `8765`)
- `ELEGY_MCP_LOG_CONTENT`
- `ELEGY_MCP_DATA_DIR`

`ELEGY_MCP_ADMIN_PASSWORD` is the cleartext consent password. Startup derives an Argon2 verifier in memory and rejects pre-hashed Argon2 strings.

## Docs

- [Configuration](docs/CONFIG.md)
- [Auth](docs/AUTH.md)
- [Transport](docs/TRANSPORT.md)
- [Deployment](docs/DEPLOYMENT.md)
- [Architecture overview](docs/architecture/overview.md)
