# elegy-memory-mcp

**Readiness: implemented; not agent-routable.** Source tests cover both
transports and the HTTP authentication guards. No clean-installed real MCP
consumer has proved either surface. See [stdio readiness](readiness-stdio.json)
and [HTTP readiness](readiness-http.json).

This crate provides host adapters for the `elegy-memory` library. Memory is a
tool/library, not an Elegy adapter plugin.

| Binary | Boundary | Authentication |
|---|---|---|
| `elegy-memory-mcp-stdio` | Local subprocess MCP | No additional OAuth; the local process boundary applies. |
| `elegy-memory-mcp-http` | Streamable HTTP MCP | Explicit loopback-only no-auth, or external OAuth resource-server validation. |

The HTTP binary does not implement login, consent, DCR, token issuance,
refresh, or an OAuth client registry. In remote mode the MCP host and external
identity provider own those behaviors; Memory validates the resulting bearer
token.

Both transports expose `memory_search`, `memory_recall`, `memory_list`,
`memory_stats`, `memory_store`, `memory_update`, `memory_correct`, and
`memory_delete`. Requests cannot override the configured agent namespace.

## Stdio read visibility

The local stdio binding can read the scopes visible from `ELEGY_MCP_READ_SCOPE`
(default `session`, the widest range). It also includes memories without an
`agent_id`, which is how the `elegy-memory` CLI writes shared local knowledge.
Reads may be widened, but writes remain limited to the configured agent scope;
a merely readable memory cannot be updated, corrected, or deleted through this
surface.

Build and test:

```powershell
cargo build -p elegy-memory-mcp
cargo test -p elegy-memory-mcp
```

Read [configuration](docs/CONFIG.md), [authentication](docs/AUTH.md),
[transport](docs/TRANSPORT.md), and [deployment](docs/DEPLOYMENT.md) before
invocation.
