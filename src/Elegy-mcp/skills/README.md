# Elegy-mcp skills helper lane

This folder is the skills helper lane for the `src/Elegy-mcp` wrapper surface.

It does not become a skill authority surface, implementation center, or release surface.

Use it to explain how the wrapper delegates skill-facing guidance:

- `contracts/` and `governance/` for governed MCP and skill artifacts.
- `rust/crates/elegy-mcp` for the current dedicated in-repo MCP CLI surface.
- `rust/crates/elegy-tooling` and related Rust crates for MCP-to-skill executable behavior.
- `.github/skills/elegy-mcp/SKILL.md` for the repo-local non-authoritative contributor-routing output.
- `elegy-mcp/SKILL.md` for the shipped surface-local bridge used by wrapper archive consumers.
- `docs/architecture/skill-core-v1.md` and `docs/architecture/mcp-skill-tooling-placement.md` for the canonical documentation.

The dedicated executable still lives under `rust/crates/elegy-mcp` and is published through the repo-root distribution path.