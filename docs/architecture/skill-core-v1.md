---
title: Skill Core
status: active
owner: elegy-core
doc_kind: reference
---

# Skill Core

Agent Skills are standard SKILL.md files. Plugin-owned skills live under
`plugins/<name>/skills/<skill-id>/SKILL.md`; standalone skill-only packages live at
the repo root (`<skill-id>/SKILL.md`). Each plugin manifest declares its own skill
directory, and a host discovers those skills when that plugin is installed or
projected for the host.

Elegy does not maintain a central skill registry, search index, or cross-plugin
resolver. Those surfaces cannot accurately represent independently installed,
versioned, or host-specific plugin sets. Plugin verification validates package
shape and declared skill paths; the host owns discovery and routing over its
installed skills.

Skills do not project capabilities, MCP tools, invocation templates, or
side-effect metadata. Those contracts remain with the owning plugin and host
projection.
