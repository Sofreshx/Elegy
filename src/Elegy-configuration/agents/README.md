# Elegy-configuration agents helper lane

This folder is the agents helper lane for the `src/Elegy-configuration` wrapper surface.

It does not become an agent implementation center, orchestration center, or release surface.

Use it to keep wrapper-level handoff guidance aligned with the owned locations:

- `contracts/` and `governance/` for canonical configuration authority.
- `rust/` for reusable deterministic materialization behavior.
- downstream consuming repos for host-specific orchestration, trust, auth, approvals, and runtime registration.
