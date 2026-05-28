# Elegy-mcp external-agent integration and contributor-routing lane

This folder holds wrapper-level guidance for external agents outside Elegy that integrate with the `src/Elegy-mcp` wrapper surface and for contributors routing that integration correctly.

It does not become an in-repo agent center, host implementation center, orchestration lane, or release surface.

Use it to keep wrapper-level external-agent integration and contributor-routing guidance aligned with the owned locations:

- `rust/crates/elegy-mcp` for the dedicated MCP descriptor authoring and analysis CLI implementation.
- `docs/architecture/mcp-skill-tooling-placement.md` for placement rules.
- downstream consuming repos for host-specific orchestration, transport, auth, and product policy.

External agents should load the associated skill guidance and invoke `elegy-mcp` directly. `src/Elegy-mcp` remains a thin wrapper surface, not an implementation center. Agent wrappers and host flows remain consumer-local unless a stronger shared boundary is later proven.
