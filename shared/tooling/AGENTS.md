# Elegy Tooling

## Start Here

- Read `../../docs/architecture/mcp-skill-tooling-placement.md` before changing crate ownership or generation boundaries.
- Inspect governed MCP and skill artifacts under `../../shared/core/fixtures/` when changing emitted file shapes.

## Boundaries

- This crate owns reusable executable author/analyze/generate logic over governed artifacts.
- It is not an authority root: contract truth stays in governed artifacts, while CLI UX and dedicated operator surfaces stay in top-level binaries or thin wrapper crates.
- Keep outputs deterministic and reusable so multiple surfaces can rely on the same behavior.
- Keep downstream registration, transport, auth, and product orchestration outside this crate.
