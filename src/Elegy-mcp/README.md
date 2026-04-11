# Elegy-mcp wrapper surface

This directory is a thin wrapper and integration entrypoint for the current dedicated in-repo `elegy-mcp` surface.

It is a real contributor-facing surface for wrapper metadata and contributor routing for external-agent integration, but it is not the authority source, implementation center, or release orchestration surface.

External agents outside Elegy should use this wrapper only to find the associated skill guidance and the dedicated `elegy-mcp` CLI handoff. `src/Elegy-mcp` remains a thin wrapper surface, not an implementation center, and this wrapper does not mean Elegy runs an internal agent orchestration lane.

The wrapper contract for this root lives in `wrapper-entrypoint.json`.

Delegation stays one-way:

- `contracts/` and `governance/` remain canonical for governed MCP descriptors, analysis-result artifacts, compatibility metadata, and policy.
- `rust/crates/elegy-mcp` remains the implementation center for descriptor authoring and analysis behavior.
- `.github/skills/elegy-mcp/SKILL.md` remains the repo-local non-authoritative contributor-routing output for this surface.
- `docs/architecture/mcp-skill-tooling-placement.md`, `docs/architecture/ecosystem-topology.md`, and `docs/spec-baseline.md` remain the canonical documentation entrypoints.

This wrapper surface organizes its helper lanes like this:

- `docs/` maps this surface to its canonical documentation entrypoints.
- `agents/` captures wrapper-level external-agent integration and contributor-routing guidance for descriptor authoring and analysis; it is not an in-repo runtime lane.
- `skills/` explains how this surface delegates repo-local skill routing output and ships a surface-local bridge in `skills/elegy-mcp/SKILL.md` for external-agent and wrapper-archive handoff.

Published release archives and install flows remain produced from the Rust workspace and repo-root distribution scripts, including the platform-neutral `elegy-mcp-wrapper-<bundleVersion>.zip` wrapper archive.

This root now also carries `install.ps1` as a thin installer entrypoint for the `elegy-mcp` CLI surface.
