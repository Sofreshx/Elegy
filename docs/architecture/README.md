# Architecture Docs

This directory contains the current architectural guidance for the repo that exists today: neutral governed artifact roots under `contracts/`, plus the first-party Rust workspace under `rust/`.

Some page titles were retained from earlier cleanup phases for continuity. Unless a page marks itself as historical, treat it as current guidance for the post-legacy repo shape.

## Current references

- [Ecosystem topology](ecosystem-topology.md) - current repo centers, dependency direction, and contributor-facing scope
- [Rust consolidation](rust-consolidation.md) - current cleanup baseline for governed roots, exports, and Rust executable ownership
- [Substrate governance (historical)](substrate-governance.md) - active artifact/runtime boundary and validation rules
- [Skill Core V1](skill-core-v1.md) - current skill authority split between governed artifacts and Rust executable behavior
- [Agent skill bridge mirrors](agent-skill-bridge-mirrors.md) - OBSOLETE: former repo-local SKILL.md mirror policy
- [Codex plugin projection](codex-plugin-projection.md) - optional Codex projection slice (derived adapter surface, not the primary plugin path)
- [Piloting Moved To Holon](piloting-moved-to-holon.md) - migration note: piloting authority, protocol, and execution have moved to the Holon Rust runtime
- [Elegy Plugin Package Model](elegy-plugin-package-model.md) - primary plugin package model: shape, setup flow, authority chain, and boundaries
- [Authoring lane (deferred)](../issues/unresolved-goals.md#goal-20260616-01) - the polished host-driven plugin authoring lane (`elegy plugin author`, `definitionRef` resolution) is deferred; current authoring is hand-edited via `elegy plugin new` plus manual verify-iterate
- [Elegy Plugin Readiness](elegy-plugin-readiness.md) - host-neutral package metadata and publishing posture for LLM agent hosts
- [MCP, skill, and tooling placement](mcp-skill-tooling-placement.md) - placement rules for governed MCP and skill artifacts versus Rust tooling
- [Documentation practices](documentation-practices.md) - central ADR/spec doctrine, placement rules, and the lean `elegy docs` validation posture
- [Terminology](terminology.md) - neutral vocabulary for artifact authority, projections, and runtime ownership

## Feature mirrors

These pages are mirror summaries of the active feature surfaces. The
authoritative implementation lives next to each mirror inside the matching
`rust/features/<feature>/` tree.

- [Elegy-configuration V1](../../rust/features/elegy-configuration/docs/architecture/v1.md) - deterministic template and profile materialization boundary between installer, reusable runtime, and consumer bootstrap
- [Mermaid tooling](../../rust/features/elegy-mermaid/docs/tooling.md) - current Mermaid render, reverse, and narrative projection slice under the umbrella `elegy` CLI
- [Observe CLI](../../rust/features/elegy-observe/docs/cli.md) - shipped read-only observation commands plus the bounded `elegy observe record` MVP contract
- [Elegy-memory V1](../../rust/features/elegy-memory/docs/architecture/v1.md) - shipped local memory surface under `elegy-memory`, authority chain, and retention/removal semantics
- [Elegy-planning V1](../../rust/features/elegy-planning/docs/architecture/v1.md) - durable planning authority surface, work-point graph, and project-run lease state

## Companion docs

- [MCP spec baseline](../spec-baseline.md)
- [Migration / extraction matrix](../migration/extraction-matrix.md)
- [Distribution and downstream consumption](../distribution.md)
- [Repository README](../../README.md)
- [Contributing](../../CONTRIBUTING.md)
- [Security policy](../../SECURITY.md)
