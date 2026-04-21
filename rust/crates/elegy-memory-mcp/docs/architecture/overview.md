# elegy-memory-mcp architecture overview

This crate combines OAuth, MCP transport, fixed-namespace memory access, and audit logging in one binary.

## Component map

The public hostname terminates at Cloudflare Tunnel, then the local axum server splits public OAuth routes from the protected MCP route.

```text
Claude / Claude Desktop
        |
        v
https://...holon.it.com
        |
        v
   cloudflared tunnel
        |
        v
 axum router on localhost:8765
   |                    |
   | public             | bearer on /mcp
   v                    v
OAuth + DCR        rmcp Streamable HTTP
   |                    |
   +---------> fixed claude-ai-remote repository
                         |
                         v
                  elegy-memory SQLite store
```

## Config and bootstrap

Startup loads env-backed config, derives the admin password verifier in memory, and initializes local persistence.

- Required: `ELEGY_MCP_ADMIN_PASSWORD`, `ELEGY_MCP_DB_PATH`, `ELEGY_MCP_PUBLIC_URL`
- Optional: `ELEGY_MCP_PORT` (`8765`), `ELEGY_MCP_LOG_CONTENT`, `ELEGY_MCP_DATA_DIR`
- Persisted auth state under `DATA_DIR`:
  - `signing-key`
  - `clients.json`
  - `refresh-tokens.json`

## OAuth server

The binary serves its own OAuth metadata, DCR, consent, and token endpoints.

- Fixed scope: `claude-ai-remote`
- Public clients only (`token_endpoint_auth_method = "none"`)
- PKCE authorization code flow with refresh-token rotation
- OAuth and `/.well-known/*` routes remain public

## `/mcp` transport

The MCP surface is a Streamable HTTP endpoint protected only on `/mcp`.

- Transport: `rmcp` Streamable HTTP over axum
- Route: `POST /mcp`
- Bearer validation checks signature, expiry, and exact `claude-ai-remote` scope
- Exposed tools: `memory_search`, `memory_recall`, `memory_list`, `memory_stats`, `memory_store`, `memory_update`, `memory_correct`, `memory_delete`

## Fixed namespace repository layer

The MCP layer hardwires every tool call to one namespace instead of accepting caller-selected scope input.

- Namespace: `claude-ai-remote`
- Backing scope mapping: `MemoryScope::Agent` + `agent_id = "claude-ai-remote"`
- Scope override fields such as `scope`, `scopes`, and `namespace` are rejected before tool handling
- Write paths reuse the underlying salience-gate and correction behavior instead of bypassing it

## Audit logging

Successful writes emit minimal audit records without logging memory content.

- Logged fields: `tool`, `id`, `scope`, `timestamp`, `jti`
- Not logged: memory content

## Related docs

- [Configuration](../CONFIG.md)
- [Auth](../AUTH.md)
- [Transport](../TRANSPORT.md)
- [Deployment](../DEPLOYMENT.md)
