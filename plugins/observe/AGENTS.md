# Elegy Observe

## Start Here

- Read `../docs/cli.md` before changing command model, output contracts, or capture lifecycle.
- Inspect `../fixtures/skill.elegy-observe.json` when changing agent-visible discovery semantics.

## Boundaries

- This crate owns read-only local observation behavior. Do not introduce desktop automation, approval flows, or host orchestration here.
- Governed observation schemas under `../schemas/` and fixtures under `../fixtures/` remain the authority for emitted observation artifacts.
- Keep bounded capture as the model. `observe record` is a one-shot artifact-producing lane, not a daemon or start/stop lifecycle.
- Preserve explicit `Unsupported` behavior on platforms that do not implement a lane. Do not fake parity or silently degrade.
- Keep OS-specific or unsafe details in leaf crates such as `elegy-observe-win32`; this crate should remain the safe cross-platform surface.
