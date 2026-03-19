# Skill Core V1

## Purpose

This document records the Skill Core V1 authority decision for Phase 2 and the downstream convergence that followed it.

The goal was to make the skills package the authoritative skill contract and then force discovery, forge, MCP, and dynamic-skill flows to adapt to that contract instead of continuing to evolve around older thin shapes.

## Authority decision

The canonical skill model lives in `Elegy.Formalization.Skills`.

That means:

- `SkillDefinition` is the authoritative skill contract
- discovery index entries are projections of the authoritative contract
- `SKILL.md` materialization is an output format, not the source of truth
- downstream generators and bridges should consume or emit the canonical skill contract rather than silently inventing parallel shapes

## Phase 2 implementation scope

Phase 2 established and then propagated explicit contract areas for:

- identity
- metadata
- input shape
- output shape
- execution expectations
- governance metadata
- discovery hints
- origin and materialization posture

It also adds semantic validation so the skills package can define the difference between a merely present object and a minimally valid skill contract.

## Converged downstream surfaces

The downstream consumers that previously drifted from the canonical contract are now aligned.

- `Elegy.Formalization.Skills.Discovery` treats discovery index entries as projections of the canonical contract.
- governed `skill-definition` and `skill-discovery-index` artifacts now match the authoritative .NET model.
- `Elegy.Formalization.DynamicSkills` creates and validates canonical skill definitions with explicit dynamic origin semantics.
- `Elegy.Formalization.SkillForge` consumes canonical skill inputs, preserves governance and I/O metadata, and materializes `SKILL.md` as a projection.
- `Elegy.Formalization.Mcp` maps MCP analysis results into canonical skills as a strict adapter rather than introducing a parallel skill shape.
- first-party Rust runtime crates inside the main Elegy repo must consume the same canonical skill contract and may replace behavior-heavy MCP logic only if they preserve the same projection semantics and conformance expectations.

## Verification posture

Phase 2 completion is anchored by focused downstream tests, governed artifact export, package-boundary validation, and a full solution test pass.

The next phase should build on this contract baseline rather than reopening skill-core authority or downstream shape decisions. In particular, any Rust replacement of MCP analyzer, generator, or discovery behavior must treat this document as the authority source instead of inventing a Rust-local contract model.