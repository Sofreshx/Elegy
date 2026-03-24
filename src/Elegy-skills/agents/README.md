# Elegy-skills agents helper lane

This folder is the agents helper lane for the `src/Elegy-skills` wrapper surface.

It does not become an agent implementation center, orchestration center, or release surface.

Use it to keep wrapper-level handoff guidance aligned with the owned locations:

- `contracts/` and `governance/` for canonical skill authority.
- `rust/` for reusable executable behavior.
- downstream consuming repos for host-specific agent registration, orchestration, auth, and runtime policy.

This wrapper lane does not reopen a shared in-repo agent package-family story.