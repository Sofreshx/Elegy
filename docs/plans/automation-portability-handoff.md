---
title: Automation Portability Handoff
status: active
owner: elegy-core
doc_kind: planning
---

# Automation Portability Handoff

## Current status

The canonical terminology boundary landed on 2026-07-15. Automation Pack
delivery contracts continue to incubate in `elegy-automation-forge`; no Pack
schema, native workflow authority, target adapter, or client deployment state
has moved into Elegy. The cross-repository boundary is now recorded by the
[Automation Program ecosystem governance ADR](https://github.com/Sofreshx/elegy-automation-program/blob/main/docs/adr/2026-07-21-automation-ecosystem-governance.md).

## Goal

Clarify the boundary between portable Elegy capability plugins and separately
owned automation packs without implementing an automation engine in Elegy.

## Accepted boundary

- [Canonical terminology](../architecture/terminology.md) defines portable
  plugin core, host projection, capability binding, Automation Pack, target
  adapter, agent-runner binding, automation deployment, and Automation Forge.
- An Elegy plugin is an optional capability dependency, not the root of every
  Automation Pack.
- Keep native workflow graphs and client operation above the Elegy substrate.
- Require Elegy + current-compatible Codex; require explicit conformance for
  other host and target claims.
- Automation Forge owns delivery and adapter contracts outside
  Elegy, including a separable installer protocol, while Elegy remains the
  plugin and capability authority.

```mermaid
flowchart LR
    Core["Portable Elegy plugin core"] --> Projection["Host projection"]
    Projection --> Host["Compatible agent host"]
    Core -->|"optional capability binding"| Pack["Signed Automation Pack"]
    Pack --> Adapter["Target adapter"]
    Adapter --> Target["Automation runtime"]
    Pack -->|"optional agent-runner binding"| Host
    Host --> Deployment["Client-local automation deployment"]
    Target --> Deployment
```

## Remaining adoption work

- Record an Elegy-local ADR only when Forge Pack contracts become eligible for
  core promotion after two unrelated conforming Packs; the Program ADR already
  owns the current cross-repository boundary.
- Add a governed fixture proving isolated host extensions remain projections.
- Update compatibility specifications only when a public Pack-to-capability
  binding contract is ready for Elegy ownership.

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
- Architecture, ADR, specification, planning, roadmap, research, and generated
  documentation roots are explicitly classified.
- Existing plugin SDK/tooling tests and documentation validation pass.
- No current plugin loses compatibility.
