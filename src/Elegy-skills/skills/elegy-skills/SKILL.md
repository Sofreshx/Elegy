---
name: elegy-skills
description: "Surface-local non-authoritative bridge shipped with the Elegy-skills wrapper surface and wrapper archive."
---

# Elegy-skills Surface Bridge

This file is a surface-local, non-authoritative skill bridge shipped with the `src/Elegy-skills` wrapper surface and the `elegy-skills-wrapper-<bundleVersion>.zip` archive.

External agents outside Elegy can use this associated skill bridge to locate the dedicated `elegy-skills` CLI handoff, then invoke that CLI directly. `src/Elegy-skills` remains a thin wrapper surface, not an implementation center, and this bridge does not imply an in-repo Elegy agent runtime.

Authority stays one-way:

1. `contracts/fixtures/skill-definition.elegy-skills.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-skills.json` is the governed discovery projection.
3. `.github/skills/elegy-skills/SKILL.md` remains the repo-local contributor-routing output.
4. This file mirrors the install and CLI handoff needed by wrapper consumers.

## Wrapper install

- Run `./install.ps1` from this wrapper root to stage the contracts bundle, the `elegy-skills` CLI surface, and this wrapper surface together.
- Pass `-LocalArtifactsRoot <path>` when validating against local archives instead of GitHub release assets.

## Current commands

```text
elegy-skills generate --descriptor <path> [--output-dir <path>] [--force]
```

`--format json` is available when structured output is needed.

Use `elegy-skills` as the preferred dedicated surface. Treat umbrella `elegy generate skills` only as the general/compatibility path when needed.
