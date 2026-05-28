# Elegy-memory external-agent integration and contributor-routing lane

This folder holds wrapper-level guidance for external agents outside Elegy that integrate with the `src/Elegy-memory` wrapper surface and for contributors routing that integration correctly.

It does not become an in-repo agent implementation center, runtime authority layer, orchestration lane, or release surface.

Use it to keep wrapper-level external-agent integration and contributor-routing guidance aligned with the owned locations:

- `rust/crates/elegy-memory` for the shipped local operator implementation.
- `docs/architecture/elegy-memory-v1.md` for the owned surface description.
- `contracts/` and `governance/` for governed artifacts and policy.

External agents should load the associated skill guidance and invoke `elegy-memory` directly. `src/Elegy-memory` remains a thin wrapper surface, not an implementation center. Host-owned runtime decisions such as currentness, approval, freshness, retrieval ranking, and promotion remain outside this wrapper lane.
