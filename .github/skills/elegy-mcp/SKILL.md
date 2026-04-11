---
name: elegy-mcp
description: "Repo-local non-authoritative contributor-routing file for Elegy's current dedicated MCP surface. Use for governed descriptor authoring and descriptor analysis through the dedicated elegy-mcp CLI."
---

# Elegy MCP

This file is a repo-local, non-authoritative contributor-routing output for external-agent integration with the dedicated `elegy-mcp` surface.

External agents outside Elegy should load this skill as routing guidance, then invoke the dedicated `elegy-mcp` CLI directly. Elegy itself does not orchestrate or call agents internally through this file.

The authority chain is one-way:

1. `contracts/fixtures/skill-definition.elegy-mcp.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-mcp.json` is the governed discovery projection derived from that definition.
3. `.github/skills/elegy-mcp/SKILL.md` is a repo-local contributor-routing file only.

## When to use

- If you are operating as an external agent outside Elegy, load this skill and invoke the dedicated `elegy-mcp` binary directly for MCP descriptor authoring and analysis.
- Author a governed MCP descriptor with `elegy-mcp author` when you have explicit server and tool inputs.
- Analyze a governed MCP descriptor with `elegy-mcp analyze --descriptor <path>`.
- Treat `elegy author mcp` and `elegy analyze mcp` as general-surface compatibility commands, not the preferred dedicated path.

## Do not use

- Do not treat this skill as authority for MCP runtime execution, hosted server behavior, or product-specific orchestration.
- Do not claim OpenAPI ingestion, REST execution, or operation-catalog projection as available command behavior if the underlying path is not implemented.
- Do not infer that the overlay under `src/Elegy-mcp` is an implementation center or release surface.

## Current commands

```text
elegy-mcp author --server-name <name> --output <path> [--transport stdio|http] [--tool NAME[=DESCRIPTION]] [--force]
elegy-mcp analyze --descriptor <path>
```

`--format json` is available on the CLI when structured output is needed.

## Surface posture

- This skill routes external agents and contributors to the dedicated `elegy-mcp` CLI surface rather than to an internal Elegy orchestration lane.
- This CLI is a thin wrapper over the existing governed descriptor authoring and analysis functions in `rust/crates/elegy-mcp`.
- Treat `src/Elegy-mcp` as a thin wrapper and packaging surface, not as the implementation center.
- The dedicated surface is intentionally bounded to descriptor authoring and analysis.
- Treat `elegy` author/analyze commands as the umbrella general/compatibility path, not the preferred dedicated path.
- Governed MCP artifacts remain rooted in `contracts/` and versioned through `governance/`.
