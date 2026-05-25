# Elegy-planning wrapper surface

This directory is a thin wrapper and integration entrypoint for the implemented in-repo `elegy-planning` surface.

It is a contributor-facing surface for wrapper metadata and integration handoff, but it is not the authority source, implementation center, or release orchestration surface.

The wrapper contract for this root lives in `wrapper-entrypoint.json`.

Delegation stays one-way:

- `contracts/` and `governance/` remain canonical for governed planning artifacts, discovery projections, and release/version policy.
- `rust/crates/elegy-planning` remains the implementation center for the in-repo `elegy-planning` operator surface.
- `.github/skills/elegy-planning/SKILL.md` remains the repo-local non-authoritative contributor-routing output for this surface.
- `rust/crates/elegy-planning/docs/architecture/ARCHITECTURE.md` remains the canonical planning architecture entrypoint.

This wrapper surface organizes helper lanes in `docs/`, `agents/`, and `skills/`, and includes `install.ps1` as a thin installer entrypoint for the `elegy-planning` CLI plus the platform-neutral `elegy-planning-wrapper-<bundleVersion>.zip` archive.
