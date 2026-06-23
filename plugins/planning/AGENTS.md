# Elegy Planning

## Start Here

- Read `README.md` for the current command surface and authority posture.
- Read `docs/architecture/ARCHITECTURE.md` before changing entities, event flow, or validation behavior.
- Read `docs/architecture/mvp-scope.md` before expanding scope.

## Boundaries

- This crate owns durable planning state, not memory retention.
- SQLite is the MVP authority; markdown and JSON files are projections only.
- Every roadmap must link to a goal.
- Issues are first-class aggregates; review points stay attached to other entities.
- Validation findings should steer authoring without unnecessarily blocking writes.

## Validation

- Prefer crate-local validation first: `cargo test -p elegy-planning`.
- When changing CLI machine output, verify `--json --non-interactive --correlation-id` behavior explicitly.
