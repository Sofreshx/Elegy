---
name: elegy-mcp
description: Use when an agent needs to author a governed MCP server descriptor from a tool list, or analyze an existing governed MCP descriptor for tool-schema viability and trigger extraction. Authoring is lower-level contributor tooling; the daily MCP path is the elegy run host projection.
---

# Elegy-mcp Surface Bridge

This file is the surface-local, non-authoritative skill bridge shipped
with the `src/Elegy-mcp` wrapper surface and the
`elegy-mcp-wrapper-<bundleVersion>.zip` archive. It is a thin
install-and-handoff page; the canonical operational body lives in the
in-tree `skills/elegy-mcp/SKILL.md` and is mirrored to
`.agents/skills/elegy-mcp/SKILL.md` and
`.github/skills/elegy-mcp/SKILL.md`.

Authority stays one-way:

1. `contracts/fixtures/skill.elegy-mcp.json` is the governed source
   of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-mcp.json` is the
   governed discovery projection.
3. `skills/elegy-mcp/SKILL.md` is the canonical operational body.
4. This file mirrors install and CLI handoff needed by wrapper
   consumers.

## Wrapper install

- Run `./install.ps1` from this wrapper root to stage the contracts
  bundle, the `elegy-mcp` CLI surface, and this wrapper surface
  together.
- Pass `-LocalArtifactsRoot <path>` when validating against local
  archives instead of GitHub release assets.

## Current commands

```text
elegy mcp author --server-name <name> --output <path> [--transport stdio|http] [--tool NAME[=DESCRIPTION]] [--force] --json
elegy mcp analyze --descriptor <path> --json
```

`--format json` is available when structured output is needed.

## Surface posture

- This is the **author + analyze** pair for governed MCP descriptors.
  Authoring is lower-level contributor tooling; the daily MCP path
  is the umbrella `elegy run` host projection.
- There is no `elegy-mcp` binary. The commands live on the umbrella
  `elegy` CLI.
- The descriptor is metadata, not runtime config. The MCP server
  uses its own tool definitions, not the descriptor's.

## Agent invocation guidance

- For author: gather tool names and one-line descriptions before
  invoking. Repeat `--tool` once per tool. Format is
  `NAME` or `NAME=DESCRIPTION`.
- For analyze: pass a path to an existing governed descriptor. The
  CLI validates against the current schema before analyzing.
- Always pass `--json` so the host can parse the result envelope.
- For the full guardrails, common issues, and worked examples, load
  the canonical body: `../../../skills/elegy-mcp/SKILL.md`.
