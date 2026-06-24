# Architecture Docs

This directory contains the current architectural guidance for the Elegy repo. The Rust workspace lives at the repo root with three crate trees: `hosts/` (CLI entrypoints and thin host wrappers), `plugins/` (agent-capability plugins), and `shared/` (cross-cutting library crates). Each plugin co-locates its own schemas, fixtures, templates, and discovery metadata — there is no separate `contracts/` root.

## Current references

- [Ecosystem topology](ecosystem-topology.md) — current repo centers, dependency direction, and contributor-facing scope
- [Rust consolidation](rust-consolidation.md) — current cleanup baseline for governed roots, exports, and Rust executable ownership
- [Substrate governance](substrate-governance.md) — active artifact/runtime boundary and validation rules
- [Skill Core V1](skill-core-v1.md) — current skill authority split between governed artifacts and Rust executable behavior
- [Codex plugin projection](codex-plugin-projection.md) — optional Codex projection slice (derived adapter surface, not the primary plugin path)
- [MCP, skill, and tooling placement](mcp-skill-tooling-placement.md) — placement rules for governed MCP and skill artifacts versus Rust tooling
- [Documentation practices](documentation-practices.md) — central ADR/spec doctrine, placement rules, and the lean `elegy docs` validation posture
- [Terminology](terminology.md) — neutral vocabulary for artifact authority, projections, and runtime ownership

## Companion docs

- [MCP spec baseline](../spec-baseline.md)
- [Distribution and downstream consumption](../distribution.md)
- [Repository README](../../README.md)
- [Contributing](../../CONTRIBUTING.md)
- [Security policy](../../SECURITY.md)
