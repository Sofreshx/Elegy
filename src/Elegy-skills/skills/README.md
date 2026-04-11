# Elegy-skills wrapper-level skill-routing lane

This folder is the wrapper-level skill-routing lane for the `src/Elegy-skills` wrapper surface.

It does not become a skill authority surface, implementation center, or release surface.

Use it to explain how the thin wrapper delegates skill-facing guidance and contributor routing:

- `contracts/` for governed skill schemas, fixtures, and discovery projections.
- `governance/` for version and release policy.
- `rust/crates/elegy-skills` for the current dedicated in-repo skill-generation CLI surface.
- `rust/crates/elegy-tooling` as shared helper and compatibility infrastructure for descriptor and skill workflows, not as the dedicated implementation center.
- `rust/crates/elegy-cli` for the umbrella `elegy` general/compatibility surface.
- `.github/skills/elegy-skills/SKILL.md` for the repo-local non-authoritative contributor-routing output.
- `elegy-skills/SKILL.md` for the shipped surface-local bridge used by wrapper archive consumers.
- `.github/skills/` for repo-local non-authoritative contributor-routing outputs.

External agents outside Elegy should load the associated skill guidance from this lane or `.github/skills/`, then use the associated bridge to reach `elegy-skills` directly. `src/Elegy-skills` remains a thin wrapper surface, not an implementation center. The current operator CLI surfaces remain `elegy`, `elegy-memory`, `elegy-mcp`, and `elegy-skills`.
