# Elegy-skills skills helper lane

This folder is the skills helper lane for the `src/Elegy-skills` wrapper surface.

It does not become a skill authority surface, implementation center, or release surface.

Use it to explain how the wrapper delegates skill-facing guidance:

- `contracts/` for governed skill schemas, fixtures, and discovery projections.
- `governance/` for version and release policy.
- `rust/crates/elegy-skills` for the current dedicated in-repo skill-generation CLI surface.
- `rust/crates/elegy-tooling` and the existing `elegy` CLI surface for the current executable generation path.
- `.github/skills/elegy-skills/SKILL.md` for the repo-local non-authoritative contributor-routing output.
- `elegy-skills/SKILL.md` for the shipped surface-local bridge used by wrapper archive consumers.
- `.github/skills/` for repo-local non-authoritative contributor-routing outputs.

The current operator CLI surfaces remain `elegy`, `elegy-memory`, `elegy-mcp`, and `elegy-skills`.