# Elegy-documentation skills helper lane

This folder is the skills helper lane for the `src/Elegy-documentation` wrapper surface.

It does not become a skill authority surface, implementation center, or release surface.

Use it to explain how the wrapper delegates skill-facing guidance:

- `contracts/` for governed documentation skill definitions, discovery projections, config schema, and result schemas.
- `governance/` for version and release policy.
- `rust/crates/elegy-documentation` for the dedicated deterministic documentation CLI surface.
- `skills/elegy-doc-practices/` for reusable documentation doctrine and templates.
- `.github/skills/elegy-documentation/SKILL.md` for the repo-local non-authoritative contributor-routing output.
- `elegy-documentation/SKILL.md` for the shipped surface-local bridge used by wrapper archive consumers.

The current operator CLI surfaces include the dedicated `elegy-documentation` binary for this lane.
