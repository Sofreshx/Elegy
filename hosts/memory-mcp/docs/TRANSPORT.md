# MCP transports

`elegy-memory-mcp` now exposes the same tool surface over HTTP and stdio.

## Shape

The transport split is binary-level, while the tool layer stays shared.

```text
Remote connector        Local desktop host
       |                        |
       v                        v
elegy-memory-mcp-http   elegy-memory-mcp-stdio
       |                        |
       +-----------+------------+
                   |
                   v
            shared MCP tools
                   |
                   v
         elegy-memory SQLite store
```

## HTTP transport

The HTTP binary keeps the remote Streamable HTTP server.

- Transport: `rmcp` Streamable HTTP over axum.
- Endpoint: `POST /mcp` on `127.0.0.1:<ELEGY_MCP_PORT>`.
- OAuth endpoints and `/.well-known/*` stay public.
- `/mcp` alone requires bearer validation.
- Accepted `/mcp` tokens must be HS256 JWTs with a valid signature, unexpired `exp`, and scope exactly `claude-ai-remote`.
- The shared tool layer is pinned to `MemoryScope::Agent` with the HTTP-side fixed agent namespace.

## stdio transport

The stdio binary is the local subprocess transport for Claude Desktop and future Holon hosting.

- Transport: `rmcp` stdio.
- Process model: client spawns `elegy-memory-mcp-stdio` as a child process.
- Auth: none in-process; no OAuth, DCR, JWT, or discovery endpoints.
- Scope binding: every request is pinned to `MemoryScope::Agent` with `agent_id` from `ELEGY_MCP_AGENT_ID`.
- Fallback behavior: if `ELEGY_MCP_AGENT_ID` is unset, startup warns and uses `default-agent`.
- Logging: all logs go to `stderr`; `stdout` is reserved for MCP protocol traffic.

## Shared tool contract

Both transports expose the same 8 tools and reject caller-provided scope overrides.

- Read: `memory_search`, `memory_recall`, `memory_list`, `memory_stats`
- Write: `memory_store`, `memory_update`, `memory_correct`, `memory_delete`
- Rejected override fields: `scope`, `scopes`, `namespace`, and aliases
- Successful write tools emit minimal audit logs without memory content

## Follow-up

Before the crate returns to public HTTP/OAuth emphasis, complete the pending scope-isolation review tracked in [PENDING.md](PENDING.md).
