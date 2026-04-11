# Elegy-memory wrapper-level skill-routing lane

This folder is the wrapper-level skill-routing lane for the `src/Elegy-memory` wrapper surface.

It does not become a skill authority surface, implementation center, or release surface.

Use it to explain how the thin wrapper delegates skill-facing guidance and contributor routing:

- `contracts/` for the governed `elegy-memory` skill definition and discovery projection artifacts.
- `governance/` for version and release policy.
- `.github/skills/elegy-memory/SKILL.md` for the repo-local non-authoritative contributor-routing output.
- `elegy-memory/SKILL.md` for the shipped surface-local bridge used by wrapper archive consumers.
- `docs/architecture/skill-core-v1.md` and `docs/architecture/elegy-memory-v1.md` for the canonical documentation.

External agents outside Elegy should load the associated skill guidance from this lane or `.github/skills/`, then use the associated bridge to reach `elegy-memory` directly. `src/Elegy-memory` remains a thin wrapper surface, not an implementation center.
