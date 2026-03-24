# Elegy-mcp agents helper lane

This folder is the agents helper lane for the `src/Elegy-mcp` wrapper surface.

It does not become an agent center, host implementation center, or release surface.

Use it to keep wrapper-level handoff guidance aligned with the owned locations:

- `rust/` for reusable MCP runtime and tooling implementation.
- `docs/architecture/mcp-skill-tooling-placement.md` for placement rules.
- downstream consuming repos for host-specific orchestration, transport, auth, and product policy.

Agent wrappers and host flows remain consumer-local unless a stronger shared boundary is later proven.