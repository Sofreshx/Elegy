# Elegy-skills skills helper lane

This folder is the skills helper lane for the `src/Elegy-skills` wrapper surface.

It does not become a skill authority surface, implementation center, or release surface.

Use it to explain how the wrapper delegates skill-facing guidance:

- `contracts/` for governed skill schemas, fixtures, and discovery projections.
- `governance/` for version and release policy.
- `rust/crates/elegy-skills` for the current dedicated in-repo skill-registry CLI and shared registry API surface.
- `rust/crates/elegy-tooling` and the existing `elegy` CLI surface for lower-level MCP-to-skill generation when contributor tooling still needs it.
- `.agents/skills/elegy-skills/SKILL.md` and `.github/skills/elegy-skills/SKILL.md` for repo-local non-authoritative contributor-routing outputs.
- `elegy-skills/SKILL.md` for the shipped surface-local bridge used by wrapper archive consumers.
- `.github/skills/` for repo-local non-authoritative contributor-routing outputs.

The current operator CLI surfaces include `elegy` plus dedicated bounded binaries such as `elegy-memory`, `elegy-mcp`, `elegy-planning`, `elegy-skills`, `elegy-configuration`, and `elegy-documentation`.
