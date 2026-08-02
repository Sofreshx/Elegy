---
title: Architecture Docs
status: active
owner: elegy-core
doc_kind: index
---

# Architecture Docs

This directory contains the current architectural guidance for the Elegy repo.
The Rust workspace lives at the repo root. The surface catalog owns product
classification; directory names do not. Historical runtimes remain under
`plugins/`, standalone CLIs also live under `tools/`, protocol adapters under
`hosts/`, guidance under `skills/`, and reusable libraries under `shared/`.

## Current references

- [Ecosystem topology](ecosystem-topology.md) — current repo centers, dependency direction, and contributor-facing scope
- [Substrate governance](substrate-governance.md) — active artifact/runtime boundary and validation rules
- [Skill Core V1](skill-core-v1.md) — current skill authority split between governed artifacts and Rust executable behavior
- [Retire the central skill registry ADR](../adr/2026-07-20-retire-central-skill-registry.md) — host-owned skill discovery and the removal of `elegy-skills`
- [Codex plugin projection](codex-plugin-projection.md) — optional Codex projection slice (derived adapter surface, not the primary plugin path)
- [Capability package authority ADR](../adr/2026-08-02-adopt-capability-packages-as-authority.md) — host-neutral package and exact-lock decision
- [Capability package v1](../specs/capability-package-v1.md) — package manifest and authoring contract
- [Elegy lock v1](../specs/elegy-lock-v1.md) — exact agent selection and digest contract
- [Elegy SBOM v1](../specs/elegy-sbom-v1.md) — deterministic archive inventory and provenance sidecar
- [Codex-parity and delegated-authentication ADR](../adr/2026-07-30-adopt-codex-parity-and-delegated-authentication.md) — current package-envelope, authentication, and marketplace decision
- [Historical static marketplace ADR](../adr/2026-07-01-adopt-static-plugin-marketplace.md) — superseded v1 context
- [Plugin marketplace v2](../specs/plugin-marketplace-v2.md) — current index, policy, source-descriptor, artifact, install, and projection contract
- [Capability catalog v2](../specs/capability-catalog-v2.md) — current authority for one-kind CLI, MCP-resource, and MCP-tool entries
- [Capability catalog v1 compatibility](../specs/capability-catalog-v1.md) — legacy wire shape and migration notes
- [Plugin connections v1](../specs/plugin-connections-v1.md) — explicit authentication requirements, host bindings, and credential-free connection control
- [Evidence-backed readiness ADR](../adr/2026-07-30-adopt-evidence-backed-readiness-and-plugin-boundary.md) — proof stages, default routing gate, plugin eligibility, and the two-consumer extraction rule
- [Readiness v1](../specs/readiness-v1.md) — manifest/artifact shape, evidence requirements, promotion, and documentation enforcement
- [Generated ecosystem readiness](../readiness.md) — current evidence-backed surface matrix
- [Deprecations and reclassification](../deprecations.md) — removed plugin labels and retained replacement surfaces
- [Capability-kind taxonomy ADR](../adr/2026-07-08-adopt-capability-kind-taxonomy.md) — decision record for concrete interface kinds and legacy app-binding handling
- [MCP, skill, and tooling placement](mcp-skill-tooling-placement.md) — placement rules for governed MCP and skill artifacts versus Rust tooling
- [Shared crate boundaries](shared-crate-boundaries.md) — keep/merge criteria for shared Rust crates
- [Documentation practices](documentation-practices.md) — central ADR/spec doctrine, placement rules, and the lean `elegy docs` validation posture
- [Terminology](terminology.md) — canonical vocabulary for plugins, Automation Packs, projections, bindings, deployments, and runtime ownership
- [Repository layout](../repo-layout.md) — surface taxonomy, directory contracts, and repo-shape validation
- [Repo surface taxonomy ADR](../adr/2026-07-07-adopt-repo-surface-taxonomy.md) — decision record for separating plugins, tools, hosts, skills, wrappers, and shared crates

## Companion docs

- [MCP spec baseline](../spec-baseline.md)
- [Distribution and downstream consumption](../distribution.md)
- [Repository README](../../README.md)
- [Contributing](../../CONTRIBUTING.md)
- [Security policy](../../SECURITY.md)
