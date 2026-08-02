---
title: MCP, Skill, and Tooling Placement
status: active
owner: elegy-core
doc_kind: reference
---

# MCP, Skill, and Tooling Placement

Capability packages are the host-neutral authority. MCP authoring and
descriptor validation remain in `elegy-mcp`; the generic CLI-to-MCP bridge is a
host transport under `hosts/`. Agent Skills are optional guidance owned and
distributed by the package that declares them; installed hosts discover them
through their native skill lane. MCP-to-skill generation has been removed.
Host-specific manifests and registrations are derived projections, not package
authority or authorization.

The placement rule is now:
- `plugins/` — bundled installable plugin packages with co-located governed artifacts
- `tools/` — standalone CLI crates such as `elegy-configuration` and `elegy-codegraph`
- `hosts/` — host adapters and transport servers such as `elegy-run` and `elegy-memory-mcp`
- `skills/` — standalone skill-only packages
- `marketplace-wrappers/` — public metadata wrappers for external/private plugin archives
- `shared/` — reusable executable behavior and platform libraries
- Consumer repos — host-specific integration
