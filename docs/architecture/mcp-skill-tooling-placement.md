---
title: MCP, Skill, and Tooling Placement
status: active
owner: elegy-core
doc_kind: reference
---

# MCP, Skill, and Tooling Placement

Simplified. MCP authoring and descriptor validation remain in `elegy-mcp`. The skill registry (`elegy-skills`) serves list/search/resolve/get/validate against Agent Skills (SKILL.md). MCP-to-skill generation has been removed. Package-backed configuration has been removed.

The placement rule is now:
- `plugins/` — bundled installable plugin packages with co-located governed artifacts
- `tools/` — standalone CLI crates such as `elegy-skills`, `elegy-configuration`, and `elegy-codegraph`
- `hosts/` — host adapters and transport servers such as `elegy-run` and `elegy-memory-mcp`
- `skills/` — standalone skill-only packages
- `marketplace-wrappers/` — public metadata wrappers for external/private plugin archives
- `shared/` — reusable executable behavior and platform libraries
- Consumer repos — host-specific integration
