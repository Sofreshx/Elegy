# Elegy-skills contributor overlay

This directory is a contributor-navigation overlay for the current dedicated in-repo `elegy-skills` surface.

It is not a repo center, authority layer, implementation center, or release surface.

The overlay itself is not the release surface. Published release archives and install flows are produced from the Rust workspace and repo-root distribution scripts.

Canonical ownership remains outside this overlay:

- `contracts/` and `governance/` remain canonical for skill schemas, fixtures, discovery projections, and policy.
- `rust/` remains the implementation center for reusable skill-generation and runtime behavior.
- `.github/skills/` remains the repo-local non-authoritative contributor-routing surface where specific skills are materialized.
- `docs/architecture/skill-core-v1.md` and `docs/architecture/mcp-skill-tooling-placement.md` remain the canonical documentation entrypoints.

The current executable scope is the dedicated `elegy-skills` CLI for MCP-to-skill generation. The overlay exists only to route contributors back to the owned authority and implementation locations.