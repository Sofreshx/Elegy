# Elegy-memory agents helper lane

This folder is the agents helper lane for the `src/Elegy-memory` wrapper surface.

It does not become an agent implementation center, runtime authority layer, or release surface.

Use it to keep wrapper-level handoff guidance aligned with the owned locations:

- `rust/crates/elegy-memory` for the shipped local operator implementation.
- `docs/architecture/elegy-memory-v1.md` for the owned surface description.
- `contracts/` and `governance/` for governed artifacts and policy.

Host-owned runtime decisions such as currentness, approval, freshness, retrieval ranking, and promotion remain outside this wrapper lane.