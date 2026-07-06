# elegy-memory-mcp architecture overview

This crate now has two binaries over one shared MCP tool layer.

## Component map

The shared tool layer now sits behind either the HTTP/OAuth binary or the local stdio binary.

```text
Claude.ai remote             Claude Desktop / Holon local
        |                                 |
        v                                 v
https://...holon.it.com         stdio child process spawn
        |                                 |
        v                                 v
   cloudflared tunnel          elegy-memory-mcp-stdio
        |                                 |
        v                                 |
elegy-memory-mcp-http                     |
   |                    \                 /
   | public OAuth        \               /
   v                      v             v
OAuth + DCR          shared MCP tool/service layer
   |                                |
   +------------------------------->|
                                    v
                             elegy-memory SQLite store
```

## Config and bootstrap

Startup now branches at the binary entrypoint, then reuses the shared memory/tool bootstrap.

| Binary | Required env | Optional env |
|---|---|---|
| `elegy-memory-mcp-http` | `ELEGY_MCP_ADMIN_PASSWORD`, `ELEGY_MCP_DB_PATH`, `ELEGY_MCP_PUBLIC_URL` | `ELEGY_MCP_PORT`, `ELEGY_MCP_LOG_CONTENT`, `ELEGY_MCP_DATA_DIR`, `RUST_LOG` |
| `elegy-memory-mcp-stdio` | `ELEGY_DB_PATH` | `ELEGY_MCP_AGENT_ID`, `OLLAMA_URL`, `RUST_LOG` |

Persisted auth state remains HTTP-only under `ELEGY_MCP_DATA_DIR`:

- `signing-key`
- `clients.json`
- `refresh-tokens.json`

## HTTP/OAuth binary

`elegy-memory-mcp-http` serves its own OAuth metadata, DCR, consent, and token endpoints.

- Fixed scope: `claude-ai-remote`
- Public clients only (`token_endpoint_auth_method = "none"`)
- PKCE authorization code flow with refresh-token rotation
- OAuth and `/.well-known/*` routes remain public

## stdio binary

`elegy-memory-mcp-stdio` is a local subprocess server with no HTTP surface.

- Transport: `rmcp` stdio
- No OAuth, DCR, JWT, or discovery endpoints
- Fixed scope mapping: `MemoryScope::Agent` + `agent_id = ELEGY_MCP_AGENT_ID`
- Unset `ELEGY_MCP_AGENT_ID` warns and falls back to `default-agent`
- All logs go to `stderr`

## Shared MCP surface

The MCP tool surface is shared across both binaries.

- Exposed tools: `memory_search`, `memory_recall`, `memory_list`, `memory_stats`, `memory_store`, `memory_update`, `memory_correct`, `memory_delete`

## Shared repository binding layer

The shared MCP repository layer binds every tool call to one caller-selected agent namespace instead of accepting per-request scope input.

- Backing scope mapping: `MemoryScope::Agent` + configured `agent_id`
- Scope override fields such as `scope`, `scopes`, and `namespace` are rejected before tool handling
- Write paths reuse the underlying salience-gate and correction behavior instead of bypassing it

Current bindings:

- HTTP/OAuth binary: fixed remote namespace for the bearer-protected connector flow
- stdio binary: `MemoryScope::Agent` + caller-configured `ELEGY_MCP_AGENT_ID`

## Audit logging

Successful writes emit minimal audit records without logging memory content.

- Logged fields: `tool`, `id`, `scope`, `timestamp`, `jti`
- Not logged: memory content

## Related docs

- [Configuration](../CONFIG.md)
- [Auth](../AUTH.md)
- [Transport](../TRANSPORT.md)
- [Deployment](../DEPLOYMENT.md)
- [Pending follow-ups](../PENDING.md)
