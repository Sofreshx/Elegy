# MCP transport (WU3)

- Transport: `rmcp` `1.2.0` retained from the workspace, with Streamable HTTP server support enabled for this crate.
- Endpoint: `POST /mcp` on `127.0.0.1:<ELEGY_MCP_PORT>` using axum + rmcp Streamable HTTP.
- WU5 protects `/mcp` with bearer validation only on that route. OAuth endpoints and `/.well-known/*` remain public.
- Accepted `/mcp` tokens must be HS256 JWTs with a valid signature, unexpired `exp`, and scope exactly `claude-ai-remote`.
- WU7 exposes all 8 MCP tools on `/mcp`:
  - Read: `memory_search`, `memory_recall`, `memory_list`, `memory_stats`
  - Write: `memory_store`, `memory_update`, `memory_correct`, `memory_delete`
- Tool schemas do not expose any scope field. Any incoming `scope`, `scopes`, `namespace`, or alias override is rejected with MCP `-32602`.
- Memory isolation is enforced in the MCP layer by pinning every request to `MemoryScope::Agent` with fixed `agent_id = "claude-ai-remote"`.
- Successful write tools emit INFO audit logs with `tool`, `id`, `scope`, `timestamp`, and bearer `jti`, and never log memory content.
