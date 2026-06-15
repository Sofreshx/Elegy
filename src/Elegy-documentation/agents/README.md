# Elegy-documentation external-agent integration and contributor-routing lane

This folder holds wrapper-level guidance for external agents outside Elegy that integrate with the `src/Elegy-documentation` wrapper surface and for contributors routing that integration correctly.

It does not become an in-repo agent implementation center, orchestration center, or release surface.

Use it to keep wrapper-level external-agent integration and contributor-routing guidance aligned with the owned locations:

- `contracts/` and `governance/` for canonical documentation config, result contracts, and skill authority.
- `rust/crates/elegy-documentation` for the dedicated documentation CLI implementation.
- downstream consuming repos for host-specific documentation workflows, approvals, and runtime policy.

External agents should load the associated skill guidance and invoke `elegy-documentation` directly. `src/Elegy-documentation` remains a thin wrapper surface, not an implementation center.
