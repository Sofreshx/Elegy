# Elegy-mcp skills overlay

This folder is a contributor pointer shell only.

It is not a skill authority surface, implementation center, or release surface.

Use these locations instead:

- `contracts/` and `governance/` for governed MCP and skill artifacts.
- `rust/crates/elegy-mcp` for the current dedicated in-repo MCP CLI surface.
- `rust/crates/elegy-tooling` and related Rust crates for MCP-to-skill executable behavior.
- `.github/skills/elegy-mcp/SKILL.md` for the repo-local non-authoritative contributor-routing output.
- `docs/architecture/skill-core-v1.md` and `docs/architecture/mcp-skill-tooling-placement.md` for the canonical documentation.

The overlay itself is not the current operator surface. The dedicated executable lives under `rust/crates/elegy-mcp` and is published through the repo-root distribution path.