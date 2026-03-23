# Elegy-mcp contributor overlay

This directory is a contributor-navigation overlay for the current dedicated in-repo `elegy-mcp` surface.

It is not a repo center, authority layer, implementation center, or release surface.

The overlay itself is not the release surface. Published release archives and install flows are produced from the Rust workspace and repo-root distribution scripts.

Canonical ownership remains outside this overlay:

- `contracts/` and `governance/` remain canonical for governed MCP descriptors, analysis-result artifacts, compatibility metadata, and policy.
- `rust/` remains the implementation center for reusable MCP behavior, including `rust/crates/elegy-mcp` and related tooling/runtime crates.
- `.github/skills/elegy-mcp/SKILL.md` remains a repo-local non-authoritative contributor-routing output only, not the authority source.
- `docs/architecture/mcp-skill-tooling-placement.md`, `docs/architecture/ecosystem-topology.md`, and `docs/spec-baseline.md` remain the canonical documentation entrypoints.

The current executable scope is the dedicated `elegy-mcp` CLI for descriptor authoring and descriptor analysis. The longer-range target narrative remains REST/OpenAPI definition ingestion, governed operation-catalog projection, and dynamic MCP generation from API specs through governed artifacts plus Rust tooling.