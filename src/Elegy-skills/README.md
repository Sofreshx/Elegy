# Elegy-skills wrapper surface

This directory is a thin wrapper and integration entrypoint for the current dedicated in-repo `elegy-skills` surface.

It is a real contributor-facing surface for wrapper metadata and guided handoff, but it is not the authority source, implementation center, or release orchestration surface.

The wrapper contract for this root lives in `wrapper-entrypoint.json`.

Delegation stays one-way:

- `contracts/` and `governance/` remain canonical for skill schemas, fixtures, discovery projections, and policy.
- `rust/crates/elegy-skills` remains the implementation center for the dedicated skill-registry CLI and reusable Rust registry API.
- `.agents/skills/elegy-skills/SKILL.md` and `.github/skills/elegy-skills/SKILL.md` remain repo-local non-authoritative contributor-routing outputs for this surface.
- `docs/architecture/skill-core-v1.md` and `docs/architecture/mcp-skill-tooling-placement.md` remain the canonical documentation entrypoints.

This wrapper surface organizes its helper lanes like this:

- `docs/` maps this surface to its canonical documentation entrypoints.
- `agents/` captures wrapper-level agent handoff guidance for the dedicated registry surface.
- `skills/` explains how this surface delegates repo-local registry routing output and ships a surface-local bridge in `skills/elegy-skills/SKILL.md`.

Published release archives and install flows remain produced from the Rust workspace and repo-root distribution scripts, including the platform-neutral `elegy-skills-wrapper-<bundleVersion>.zip` wrapper archive.

This root now also carries `install.ps1` as a thin installer entrypoint for the `elegy-skills` CLI surface.
