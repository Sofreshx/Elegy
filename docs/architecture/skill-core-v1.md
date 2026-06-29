# Skill Core

Agent Skills are standard SKILL.md files. Plugin-owned skills live under
`plugins/<name>/skills/<skill-id>/SKILL.md`; standalone skill-only packages live at
the repo root (`<skill-id>/SKILL.md`). The `elegy-skills` registry discovers skills
from plugin manifests first, then standalone root skills, and fails on duplicate IDs.

The skill registry does not project capabilities, MCP tools, invocation templates, or side-effect metadata. It validates YAML frontmatter and serves skill metadata for agent discovery.
