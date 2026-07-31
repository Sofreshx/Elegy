---
title: Skill placement
status: active
owner: elegy-core
doc_kind: reference
---

# Skill placement

Agent skills are standard `SKILL.md` instruction packages.

- An active adapter may bundle optional skills under
  `plugins/<name>/skills/<skill-id>/SKILL.md`.
- Standalone Elegy-authored guidance lives under
  `skills/<skill-id>/SKILL.md`.
- A standalone skill is installed through the target host's native skill
  mechanism. It is not an Elegy marketplace plugin.

Elegy does not maintain a central skill registry or cross-host resolver. A
skill can describe a repeatable workflow and call tools the host already has;
it does not prove that a runtime, connector, authentication flow, typed
capability, or provider operation exists.

Adapter verification checks bundled skill shape only as one optional component
of the package. Executable and protocol discovery authority remains the
adapter's typed capability catalog. Each entry names one concrete interface
(`cli`, `mcp-resource`, or `mcp-tool`); MCP resources and tools are first-class
interfaces, while host plugin layouts and skills are projections or guidance.
