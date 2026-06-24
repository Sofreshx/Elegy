# MCP, Skill, and Tooling Placement

Simplified. MCP authoring and descriptor validation remain in `elegy-mcp`. The skill registry (`elegy-skills`) serves list/search/resolve/get/validate against Agent Skills (SKILL.md). MCP-to-skill generation has been removed. Package-backed configuration has been removed.

The placement rule is now:
- `plugins/` and `shared/` — governed schemas, fixtures, and reusable executable behavior co-located with each plugin
- `hosts/` — thin binary entrypoints that consume plugin and shared behavior
- Consumer repos — host-specific integration
