# Elegy-mcp agents overlay

This folder is a contributor pointer shell only.

It is not an agent center, host implementation center, or release surface.

Use these locations instead:

- `rust/` for reusable MCP runtime and tooling implementation.
- `docs/architecture/mcp-skill-tooling-placement.md` for placement rules.
- downstream consuming repos for host-specific orchestration, transport, auth, and product policy.

Agent wrappers and host flows remain consumer-local unless a stronger shared boundary is later proven.