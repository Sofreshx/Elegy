---
name: elegy-mcp
description: "Surface-local non-authoritative bridge shipped with the Elegy-mcp wrapper surface and wrapper archive."
---

# Elegy-mcp Surface Bridge

This file is a surface-local, non-authoritative skill bridge shipped with the `src/Elegy-mcp` wrapper surface and the `elegy-mcp-wrapper-<bundleVersion>.zip` archive.

Authority stays one-way:

1. `contracts/fixtures/skill-definition.elegy-mcp.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-mcp.json` is the governed discovery projection.
3. `.github/skills/elegy-mcp/SKILL.md` remains the repo-local contributor-routing output.
4. This file mirrors the install and CLI handoff needed by wrapper consumers.

## Wrapper install

- Run `./install.ps1` from this wrapper root to stage the contracts bundle, the `elegy-mcp` CLI surface, and this wrapper surface together.
- Pass `-LocalArtifactsRoot <path>` when validating against local archives instead of GitHub release assets.

## Current commands

```text
elegy-mcp author --server-name <name> --output <path> [--transport stdio|http] [--tool NAME[=DESCRIPTION]] [--force]
elegy-mcp analyze --descriptor <path>
```

`--format json` is available when structured output is needed.