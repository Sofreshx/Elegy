# Memory MCP transports

The binaries share one Memory tool layer.

| Transport | Runtime boundary | Auth behavior |
|---|---|---|
| stdio | Host-spawned local child process | No OAuth, HTTP, JWT, or discovery behavior. |
| Streamable HTTP | Loopback or remote network resource | Loopback may explicitly select `local-none`; remote operation requires external OAuth validation. |

The HTTP production routes are `/mcp` and, in external OAuth mode,
`/.well-known/oauth-protected-resource`. Authorization-server routes do not
exist.

Both transports pin calls to `MemoryScope::Agent`, reject request-level scope
overrides, and expose the same eight tools. Stdio keeps protocol output on
stdout and diagnostics on stderr. HTTP write audits may record tool, id, scope,
timestamp, and externally supplied token id; they never record memory content.
