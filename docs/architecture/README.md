# Architecture Docs

This directory contains the current architectural guidance for the repo that exists today: neutral governed artifact roots under `contracts/`, `governance/`, `schemas/`, and `policies/`, plus the first-party Rust workspace under `rust/`.

Contributor-navigation overlays under `src/Elegy-memory`, `src/Elegy-mcp`, and `src/Elegy-skills` are pointer shells only. They do not replace these docs, the governed roots, or `rust/` as canonical centers.

Some page titles were retained from earlier cleanup phases for continuity. Unless a page marks itself as historical, treat it as current guidance for the post-legacy repo shape.

## Current references

- [Ecosystem topology](ecosystem-topology.md) - current repo centers, dependency direction, and contributor-facing scope
- [Rust consolidation](rust-consolidation.md) - current cleanup baseline for governed roots, exports, and Rust executable ownership
- [Substrate governance](substrate-governance.md) - active artifact/runtime boundary and validation rules
- [Skill Core V1](skill-core-v1.md) - current skill authority split between governed artifacts and Rust executable behavior
- [Agent skill bridge mirrors](agent-skill-bridge-mirrors.md) - current repo-local `.agents/skills` and `.github/skills` derived mirror policy
- [Codex plugin projection](codex-plugin-projection.md) - current conservative Codex plugin projection slice and its boundaries
- [Mermaid tooling](mermaid-tooling.md) - current Mermaid render, reverse, and narrative projection slice under the umbrella `elegy` CLI
- [Observe CLI](observe-cli.md) - shipped read-only observation commands plus the bounded `elegy observe record` MVP contract
- [Elegy-memory V1](elegy-memory-v1.md) - shipped local memory surface under `elegy-memory`, authority chain, and retention/removal semantics
- [MCP, skill, and tooling placement](mcp-skill-tooling-placement.md) - placement rules for governed MCP and skill artifacts versus Rust tooling
- [Terminology](terminology.md) - neutral vocabulary for artifact authority, projections, and runtime ownership

## Companion docs

- [MCP spec baseline](../spec-baseline.md)
- [Migration / extraction matrix](../migration/extraction-matrix.md)
- [Distribution and downstream consumption](../distribution.md)
- [Repository README](../../README.md)
- [Contributing](../../CONTRIBUTING.md)
- [Security policy](../../SECURITY.md)
