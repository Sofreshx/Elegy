# Elegy-mcp wrapper-level skill-routing lane

This folder is the wrapper-level skill-routing lane for the `src/Elegy-mcp` wrapper surface.

It does not become a skill authority surface, implementation center, or release surface.

Use it to explain how the thin wrapper delegates skill-facing guidance and contributor routing:

- `contracts/` and `governance/` for governed MCP and skill artifacts.
- `rust/crates/elegy-mcp` for the current dedicated in-repo MCP CLI surface.
- `rust/crates/elegy-skills` for the dedicated MCP-to-skill generation surface when generation work is needed.
- `rust/crates/elegy-tooling` and related Rust crates as shared helper and compatibility infrastructure for descriptor and skill workflows.
- `.github/skills/elegy-mcp/SKILL.md` for the repo-local non-authoritative contributor-routing output.
- `elegy-mcp/SKILL.md` for the shipped surface-local bridge used by wrapper archive consumers.
- `docs/architecture/skill-core-v1.md` and `docs/architecture/mcp-skill-tooling-placement.md` for the canonical documentation.

External agents outside Elegy should load the associated skill guidance from this lane or `.github/skills/`, then use the associated bridge to reach `elegy-mcp` directly. `src/Elegy-mcp` remains a thin wrapper surface, not an implementation center. The dedicated executable still lives under `rust/crates/elegy-mcp` and is published through the repo-root distribution path.
