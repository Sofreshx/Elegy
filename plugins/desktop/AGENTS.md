# Elegy Desktop

## Start Here

- Read `../../docs/agent-integration.md` for host policy and side-effect boundaries.
- Inspect `../fixtures/skill.elegy-desktop.json` before changing agent-visible desktop capability semantics.

## Boundaries

- This crate is the safe automation surface over platform leaves such as `elegy-desktop-win32`.
- Dry-run is part of the safety model, not optional convenience behavior.
- Evidence capture belongs with action execution so callers can inspect what happened without inventing parallel heuristics.
- Keep window targeting strict and explicit; prefer no match or ambiguous match over guessing.
- Approval policy, orchestration, and side-effect authorization stay outside this crate.
- Preserve explicit `Unsupported` behavior on unimplemented platforms rather than silently degrading.
