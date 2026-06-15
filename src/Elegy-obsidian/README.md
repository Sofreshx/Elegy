# Elegy-obsidian wrapper surface

This directory is a thin wrapper and integration entrypoint for the `elegy-obsidian` surface, which teaches agents to drive the **official Obsidian v1.12+ CLI** in a governed, capability-shaped way.

It is a contributor-facing surface for wrapper metadata and integration handoff, but it is not the authority source, the implementation center, or the release orchestration surface.

The wrapper contract for this root lives in `wrapper-entrypoint.json`.

Delegation stays one-way:

- `contracts/` and `governance/` remain canonical for the governed skill definition, the discovery projection, and release/version policy.
- The official `obsidian` CLI (shipped with the user's installed Obsidian Desktop) is the runtime. This surface is **not** a Rust crate; it is a non-authoritative vault bridge.
- `skills/elegy-obsidian/SKILL.md` remains the repo-local non-authoritative contributor-routing output for this surface.
- `docs/specs/obsidian-skill-and-cli.md` is the canonical design entrypoint for the foundation, including the extension point for the future `elegy-planning obsidian mirror/attach/resolve/list` commands.
- `docs/research/obsidian-figma-and-vision-models-for-elegy.md` is the research note that motivated this skill and the future planning-mirror work.

This wrapper surface organizes helper lanes in `docs/`, `agents/`, and `skills/`, and includes `install.ps1` as a thin installer entrypoint for the `elegy-obsidian-wrapper-<bundleVersion>.zip` archive. The archive contains the contracts bundle and this wrapper surface only; the underlying `obsidian` executable is provided by the user's Obsidian Desktop installation.
