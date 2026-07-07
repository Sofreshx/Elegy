---
title: Architecture Docs
status: active
owner: elegy-core
doc_kind: index
---

# Architecture Docs

This directory contains the current architectural guidance for the Elegy repo.
The Rust workspace lives at the repo root. Surface roles are separated by
directory: `plugins/` for bundled installable plugin packages, `tools/` for
standalone CLI crates, `hosts/` for host adapters and transport servers,
`skills/` for standalone skill packages, `marketplace-wrappers/` for external
plugin metadata wrappers, and `shared/` for reusable libraries.

## Current references

- [Ecosystem topology](ecosystem-topology.md) — current repo centers, dependency direction, and contributor-facing scope
- [Substrate governance](substrate-governance.md) — active artifact/runtime boundary and validation rules
- [Skill Core V1](skill-core-v1.md) — current skill authority split between governed artifacts and Rust executable behavior
- [Codex plugin projection](codex-plugin-projection.md) — optional Codex projection slice (derived adapter surface, not the primary plugin path)
- [Static plugin marketplace ADR](../adr/2026-07-01-adopt-static-plugin-marketplace.md) — host-neutral marketplace authority and closed-source binary boundary
- [Plugin marketplace v1](../specs/plugin-marketplace-v1.md) — index, artifact, install, and projection contract
- [MCP, skill, and tooling placement](mcp-skill-tooling-placement.md) — placement rules for governed MCP and skill artifacts versus Rust tooling
- [Shared crate boundaries](shared-crate-boundaries.md) — keep/merge criteria for shared Rust crates
- [Documentation practices](documentation-practices.md) — central ADR/spec doctrine, placement rules, and the lean `elegy docs` validation posture
- [Terminology](terminology.md) — neutral vocabulary for artifact authority, projections, and runtime ownership
- [Repository layout](../repo-layout.md) — surface taxonomy, directory contracts, and repo-shape validation
- [Repo surface taxonomy ADR](../adr/2026-07-07-adopt-repo-surface-taxonomy.md) — decision record for separating plugins, tools, hosts, skills, wrappers, and shared crates

## Companion docs

- [MCP spec baseline](../spec-baseline.md)
- [Distribution and downstream consumption](../distribution.md)
- [Repository README](../../README.md)
- [Contributing](../../CONTRIBUTING.md)
- [Security policy](../../SECURITY.md)
