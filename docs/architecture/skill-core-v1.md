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

## Current executable ownership

The Rust workspace owns the reusable executable path around those governed skill artifacts.

- `rust/crates/elegy-tooling` owns reusable authoring, analysis, and skill-generation behavior over governed descriptors and skill projections
- `rust/crates/elegy-cli` exposes the current contributor-facing commands: `author mcp`, `analyze mcp`, and `generate skills`
- `rust/crates/elegy-mcp` and related runtime crates may interpret governed MCP and skill artifacts, but they do not redefine skill authority
- consuming applications keep host-specific registration, auth, persistence, and orchestration local

## Current self-authoring scope

What the repo proves today:

- skill generation from analyzed MCP descriptor inputs through the Rust CLI and tooling path
- deterministic export of governed skill artifacts for downstream consumers
- a clean split between neutral artifact authority and Rust executable ownership

What the repo does not yet prove as a finished product surface:

- a built-in MCP-native self-authoring loop
- a skill-driven autonomous authoring surface baked into the runtime by default
- license to describe all future skill-hosting ideas as already implemented

## Verification posture

Current confidence comes from the surviving validation and export flows:

- `scripts/export-contracts.ps1` and `scripts/validate-canonical-outputs.ps1`
- `scripts/validate-package-boundaries.ps1`
- Rust CI for formatting, linting, and tests in `.github/workflows/rust-ci.yml`

Future work should build on this split rather than reopening skill authority. New reusable execution logic belongs in Rust, and new durable skill truth belongs in governed artifacts.