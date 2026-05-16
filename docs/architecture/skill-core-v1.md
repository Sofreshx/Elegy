# Skill Core V1

## Purpose

This document retains its earlier title for continuity, but its content reflects the current post-legacy repo shape.

The goal is to keep skill authority in neutral governed artifacts while keeping reusable executable behavior in the Rust workspace.

## Authority decision

The canonical skill model lives in the governed skill artifact family exported from `contracts/` and versioned through `governance/`.

That means:

- the stable skill shape and compatibility expectations belong to governed schemas, fixtures, manifests, and support metadata
- discovery index entries are projections of the authoritative governed contract
- `SKILL.md` materialization is an output format, not the source of truth
- Rust crates and downstream consumers should consume or emit the governed skill contract rather than silently inventing parallel shapes

The implemented `elegy-memory` surface follows that rule directly: `contracts/fixtures/skill-definition-v2.elegy-memory.json` is authoritative, `contracts/fixtures/skill-discovery-index.elegy-memory.json` is the governed projection derived from it, and `.github/skills/elegy-memory/SKILL.md` is a repo-local non-authoritative contributor-routing output only.

The contributor-navigation overlays under `src/Elegy-memory` and `src/Elegy-skills` do not change that authority split. They are pointer shells only, not skill authority surfaces, implementation centers, or release surfaces.

## Current executable ownership

The Rust workspace owns the reusable executable path around those governed skill artifacts.

- `rust/crates/elegy-tooling` owns reusable authoring, analysis, and MCP-to-skill generation behavior over governed descriptors and skill projections
- `rust/crates/elegy-cli` exposes the general/compatibility skills commands: `skills list/search/resolve/get/capability/validate`
- `rust/crates/elegy-skills` owns the reusable registry API plus the dedicated `elegy-skills` registry CLI surface
- `rust/crates/elegy-mcp` exposes the current dedicated MCP descriptor authoring and analysis flow
- `rust/crates/elegy-mcp` and related runtime crates may interpret governed MCP and skill artifacts, but they do not redefine skill authority
- consuming applications keep host-specific registration, auth, persistence, and orchestration local

## Current registry scope

What the repo proves today:

- governed built-in skill registry loading with strict validation
- dedicated `elegy-skills` search, resolve, inspect, and validation commands over the governed registry
- direct Rust skill-registry access for downstream Rust hosts that should avoid CLI subprocess overhead
- MCP and umbrella CLI reuse over the same registry metadata instead of re-parsing unrelated ad hoc shapes
- skill generation from analyzed MCP descriptor inputs through the general CLI/tooling path when contributor tooling still needs it
- deterministic export of governed skill artifacts for downstream consumers
- a clean split between neutral artifact authority and Rust executable ownership
- the current operator CLI surfaces remain `elegy`, `elegy-memory`, `elegy-mcp`, and `elegy-skills`

What the repo does not yet prove as a finished product surface:

- a built-in remote skill package or install ecosystem
- runtime-owned autonomous registration or hosted skill orchestration
- license to describe all future skill-hosting ideas as already implemented

## Current skill tools

The high-level skills model is now:

- `elegy-skills` is the dedicated skill registry tool
- `elegy skills ...` is the umbrella compatibility surface for the same registry features
- `elegy agent discover` is the host/profile-filtered router over that same registry
- built-in validation is part of the registry surface, not an afterthought
- MCP-to-skill generation remains lower-level contributor tooling, not the main skills product story

## Verification posture

Current confidence comes from the surviving validation and export flows:

- `scripts/export-contracts.ps1` and `scripts/validate-canonical-outputs.ps1`
- `scripts/validate-package-boundaries.ps1`
- Rust CI for formatting, linting, and tests in `.github/workflows/rust-ci.yml`

Future work should build on this split rather than reopening skill authority. New reusable execution logic belongs in Rust, and new durable skill truth belongs in governed artifacts.
