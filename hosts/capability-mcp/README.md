# Elegy capability MCP bridge — readiness: implemented, not agent-routable

`hosts/capability-mcp/` provides `elegy-capability-mcp`, a generic MCP stdio
adapter for deterministic JSON CLI capabilities. It reads the canonical
package and catalog, exposes only routable and allowed capabilities, invokes
the executable directly, and validates both sides of the JSON contract.

## Build and invoke

```bash
cargo build -p elegy-capability-mcp
cargo run -p elegy-capability-mcp --bin elegy-capability-mcp -- \
  --package ./installed-package --lock ./agent.elegy.lock.json
```

The bridge is not a native MCP implementation for a domain tool. Use a native
server only when state, sessions, subscriptions, or streaming require it. No
clean installed MCP-client task receipt has been reviewed yet; see
[readiness.json](readiness.json).
