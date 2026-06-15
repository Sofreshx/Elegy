# Elegy-documentation wrapper surface

This directory is a thin wrapper and integration entrypoint for the current dedicated in-repo `elegy-documentation` surface.

It is a real contributor-facing surface for wrapper metadata and guided handoff, but it is not the authority source, implementation center, or release orchestration surface.

The wrapper contract for this root lives in `wrapper-entrypoint.json`.

Delegation stays one-way:

- `contracts/` and `governance/` remain canonical for documentation config/result schemas, governed skill fixtures, discovery projections, and release policy.
- `rust/crates/elegy-documentation` remains the implementation center for the dedicated deterministic documentation CLI and reusable Rust API.
- `.github/skills/elegy-documentation/SKILL.md` remains the repo-local non-authoritative contributor-routing output for this surface.
- `docs/architecture/documentation-practices.md` remains the canonical documentation entrypoint.

This wrapper surface organizes its helper lanes like this:

- `docs/` maps this surface to its canonical documentation entrypoints.
- `agents/` captures wrapper-level handoff guidance for the dedicated documentation surface.
- `skills/` explains how this surface delegates repo-local routing output and ships a surface-local bridge in `skills/elegy-documentation/SKILL.md`.

Published release archives and install flows remain produced from the Rust workspace and repo-root distribution scripts, including the platform-neutral `elegy-documentation-wrapper-<bundleVersion>.zip` wrapper archive.

This root also carries `install.ps1` as a thin installer entrypoint for the `elegy-documentation` CLI surface.
