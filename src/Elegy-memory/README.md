# Elegy-memory wrapper surface

This directory is a thin wrapper and integration entrypoint for the implemented in-repo `elegy-memory` surface.

It is a real contributor-facing surface for wrapper metadata and contributor routing for external-agent integration, but it is not the authority source, implementation center, or release orchestration surface.

External agents outside Elegy should use this wrapper only to find the associated skill guidance and the dedicated `elegy-memory` CLI handoff. `src/Elegy-memory` remains a thin wrapper surface, not an implementation center, and this wrapper does not mean Elegy runs an internal agent orchestration lane.

The wrapper contract for this root lives in `wrapper-entrypoint.json`.

Delegation stays one-way:

- `contracts/` and `governance/` remain canonical for governed memory-family artifacts, discovery projections, and release/version policy.
- `rust/crates/elegy-memory` remains the implementation center for the in-repo `elegy-memory` operator surface.
- `.github/skills/elegy-memory/SKILL.md` remains the repo-local non-authoritative contributor-routing output for this surface.
- `docs/architecture/elegy-memory-v1.md` and `docs/migration/reusable-memory-boundary.md` remain the canonical documentation entrypoints.

This wrapper surface organizes its helper lanes like this:

- `docs/` maps this surface to its canonical documentation entrypoints.
- `agents/` captures wrapper-level external-agent integration and contributor-routing guidance for the bounded memory surface; it is not an in-repo runtime lane.
- `skills/` explains how this surface delegates repo-local skill routing output and ships a surface-local bridge in `skills/elegy-memory/SKILL.md` for external-agent and wrapper-archive handoff.

This root now also carries `install.ps1` as a thin installer entrypoint for the `elegy-memory` CLI surface plus the platform-neutral `elegy-memory-wrapper-<bundleVersion>.zip` wrapper archive.
