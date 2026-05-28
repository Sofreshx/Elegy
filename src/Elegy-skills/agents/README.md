# Elegy-skills external-agent integration and contributor-routing lane

This folder holds wrapper-level guidance for external agents outside Elegy that integrate with the `src/Elegy-skills` wrapper surface and for contributors routing that integration correctly.

It does not become an in-repo agent implementation center, orchestration center, or release surface.

Use it to keep wrapper-level external-agent integration and contributor-routing guidance aligned with the owned locations:

- `contracts/` and `governance/` for canonical skill authority.
- `rust/crates/elegy-skills` for the dedicated MCP-to-skill generation CLI implementation.
- downstream consuming repos for host-specific agent registration, orchestration, auth, and runtime policy.

External agents should load the associated skill guidance and invoke `elegy-skills` directly. `src/Elegy-skills` remains a thin wrapper surface, not an implementation center. This wrapper lane does not reopen a shared in-repo agent package-family story.
