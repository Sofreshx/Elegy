---
title: Automation Portability Handoff
status: planned
owner: elegy-core
doc_kind: planning
---

# Automation Portability Handoff

## Goal

Clarify the boundary between portable Elegy capability plugins and separately
owned automation packs without implementing an automation engine in Elegy.

## Required changes

- Add canonical terms for portable plugin core, host projection, capability
  binding, automation pack, target adapter, agent-runner binding, and
  automation deployment. Clarify that an Elegy plugin is an optional capability
  dependency rather than the root of every Automation Pack.
- Keep native workflow graphs and client operation above the Elegy substrate.
- Record an ADR: Pack v0 incubates in `elegy-automation-forge` and is eligible
  for core promotion only after two unrelated conforming packs.
- Require Elegy + current-compatible Codex; require explicit conformance for
  other host and target claims.
- Add a governed fixture proving isolated host extensions remain projections.
- Record that Automation Forge owns the delivery and adapter contracts outside
  Elegy, including a separable installer protocol, while Elegy remains the
  plugin and capability authority.

## Non-goals

- n8n workflow schemas or execution.
- Forge implementation.
- Target and installation adapter protocols or installer execution.
- Client deployment, credentials, approvals, monitoring, or UI state.
- A universal workflow graph.
- Requiring every plugin to support every harness.

## Acceptance

- Terminology, topology, capability-catalog, Codex projection, and compatibility
  specs remain mutually consistent.
- Existing plugin SDK/tooling tests and documentation validation pass.
- No current plugin loses compatibility.
