---
name: elegy-skills
description: "Repo-local non-authoritative contributor-routing file for Elegy's current dedicated skill-generation surface. Use for governed MCP-to-skill generation through the dedicated elegy-skills CLI."
---

# Elegy Skills

This file is a repo-local, non-authoritative contributor-routing output for external-agent integration with the dedicated `elegy-skills` surface.

External agents outside Elegy should load this skill as routing guidance, then invoke the dedicated `elegy-skills` CLI directly. Elegy itself does not orchestrate or call agents internally through this file.

The authority chain is one-way:

1. `contracts/fixtures/skill-definition.elegy-skills.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-skills.json` is the governed discovery projection derived from that definition.
3. `.github/skills/elegy-skills/SKILL.md` is a repo-local contributor-routing file only.

## When to use

- If you are operating as an external agent outside Elegy, load this skill and invoke the dedicated `elegy-skills` binary directly for MCP-to-skill generation.
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

- This skill routes external agents and contributors to the dedicated `elegy-skills` CLI surface rather than to an internal Elegy orchestration lane.
- The dedicated CLI surface lives in `rust/crates/elegy-skills`.
- Shared crates such as `rust/crates/elegy-tooling` provide lower-level helper and compatibility infrastructure where needed; they are not the dedicated `elegy-skills` implementation center.
- Treat `src/Elegy-skills` as a thin wrapper and packaging surface, not as the implementation center.
- The dedicated surface is intentionally bounded to generation from governed MCP descriptor inputs.
- Treat `elegy generate skills` as the umbrella general/compatibility path, not the preferred dedicated path.
- Governed skill artifacts remain rooted in `contracts/` and versioned through `governance/`.
