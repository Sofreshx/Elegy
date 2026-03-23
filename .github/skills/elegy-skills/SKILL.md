---
name: elegy-skills
description: "Repo-local non-authoritative contributor-routing file for Elegy's current dedicated skill-generation surface. Use for governed MCP-to-skill generation through the dedicated elegy-skills CLI."
---

# Elegy Skills

This file is a repo-local, non-authoritative contributor-routing output.

The authority chain is one-way:

1. `contracts/fixtures/skill-definition.elegy-skills.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-skills.json` is the governed discovery projection derived from that definition.
3. `.github/skills/elegy-skills/SKILL.md` is a repo-local contributor-routing file only.

## When to use

- Prefer the dedicated `elegy-skills` binary for the current implemented in-repo MCP-to-skill generation flow.
- Generate governed skill-definition artifacts from an MCP descriptor with `elegy-skills generate --descriptor <path>`.
- Use `--output-dir <path>` when generated skill-definition files should be written to disk.
- Treat `elegy generate skills` as a general-surface compatibility command, not the preferred dedicated path.

## Do not use

- Do not treat this skill as authority for runtime-side skill registration, autonomous authoring, or host-specific orchestration.
- Do not infer that `.github/skills/` markdown is authoritative. Governed fixtures remain the source of truth.
- Do not infer that the overlay under `src/Elegy-skills` is an implementation center or release surface.

## Current commands

```text
elegy-skills generate --descriptor <path> [--output-dir <path>] [--force]
```

`--format json` is available on the CLI when structured output is needed.

## Surface posture

- This CLI is a thin wrapper over the existing governed skill-generation functions in `rust/crates/elegy-tooling`.
- The dedicated surface is intentionally bounded to generation from governed MCP descriptor inputs.
- Governed skill artifacts remain rooted in `contracts/` and versioned through `governance/`.