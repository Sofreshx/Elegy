# Elegy-memory contributor overlay

This directory is a contributor-navigation overlay for the implemented in-repo `elegy-memory` surface.

It is not a repo center, authority layer, implementation center, or release surface.

Canonical ownership remains outside this overlay:

- `contracts/` and `governance/` remain canonical for governed memory-family artifacts, discovery projections, and release/version policy.
- `rust/crates/elegy-memory` remains the implementation center for the in-repo `elegy-memory` operator surface.
- `.github/skills/elegy-memory/SKILL.md` remains a repo-local non-authoritative contributor-routing output only, not the authority source.
- `docs/architecture/elegy-memory-v1.md` and `docs/migration/reusable-memory-boundary.md` remain the canonical documentation entrypoints.

Current operator CLI surfaces remain `elegy`, `elegy-memory`, `elegy-mcp`, and `elegy-skills`. This overlay exists only to route contributors back to those owned locations.

- `docs/` points to the canonical docs.
- `agents/` is a pointer shell, not an agent implementation center.
- `skills/` is a pointer shell, not a skill authority surface.