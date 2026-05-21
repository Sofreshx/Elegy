---
name: elegy-mcp
description: "Derived repo-local skill bridge mirror for Elegy's current dedicated MCP surface. Use for governed descriptor authoring and descriptor analysis through the dedicated elegy-mcp CLI."
---

# Elegy MCP

This file is a repo-local, non-authoritative rendered skill bridge mirror.

The authority chain is one-way:

1. `contracts/fixtures/skill-definition-v2.elegy-mcp.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-mcp.json` is the governed discovery projection derived from that definition.
3. `.agents/skills/elegy-mcp/SKILL.md` and `.github/skills/elegy-mcp/SKILL.md` are repo-local rendered mirrors only.

## When to use

- Prefer the dedicated `elegy-mcp` binary for the current implemented in-repo MCP descriptor authoring and analysis flows.
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

- This CLI is a thin wrapper over the existing governed descriptor authoring and analysis functions in `rust/crates/elegy-mcp`.
- The dedicated surface is intentionally bounded to descriptor authoring and analysis.
- Governed MCP artifacts remain rooted in `contracts/` and versioned through `governance/`.
