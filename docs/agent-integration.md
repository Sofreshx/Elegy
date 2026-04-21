---
title: Agent Integration Guide
description: How to discover and invoke Elegy capabilities from an LLM agent or automation harness.
created: 2025-07-15
updated: 2026-04-21
category: architecture
status: active
doc_kind: guide
---

# Agent Integration Guide

Elegy is designed for harnesses that can run local subprocesses or connect to an MCP stdio server. The primary integration rule is: discover first, expand only the capability you need, then invoke through the advertised template.

## Quick Start

```bash
elegy --version --json
elegy skills list --json
elegy skills search --query "diagram" --json
elegy skills describe --skill-id diagram --json
```

`skills list` returns compact summaries. `skills search` returns scored matches. `skills describe` returns the full v2 skill definition, including exact invocation templates.

Aliases are supported for skill lookup, so `--skill-id elegy-memory` resolves to the `memory` skill.

## V2 Skill Definitions

Elegy only supports v2 skill definitions during this early development phase. They live in `contracts/fixtures/skill-definition-v2.*.json` and are embedded in the binaries through the shared contracts registry.

Each capability includes:

- `implementation.executionType`: currently `subprocess`, `library`, or `mcp`.
- `implementation.executableName`: binary or host name.
- `implementation.arguments`: ordered arguments with `${name}` placeholders.
- `input.parameters`: typed parameters with required/default metadata.
- `input.stdinFormat`: optional stdin format, usually `json` or `text`.
- `output`: result description and optional schema reference.
- `execution`: determinism, side-effect, and timeout metadata.
- `governance`: skill-level risk and approval posture.

Example:

```json
{
  "id": "diagram-patch",
  "implementation": {
    "executionType": "subprocess",
    "executableName": "elegy",
    "arguments": ["diagram", "patch", "--input", "${inputPath}", "--patch-stdin", "--json"]
  },
  "input": {
    "parameters": [{ "name": "inputPath", "type": "path", "required": true }],
    "stdinFormat": "json"
  },
  "execution": {
    "mode": "requestResponse",
    "isDeterministic": true,
    "hasSideEffects": true
  }
}
```

## Subprocess Invocation

Replace placeholders with argument values. For booleans, pass the flag only when the value is true. For array parameters, repeat the flag once per value.

```bash
elegy diagram create --diagram-type architecture --json
```

Prefer stdin-capable commands when available:

```bash
echo '{"addNodes":[{"id":"cache","label":"Cache Layer"}]}' \
  | elegy diagram patch --input diagram.json --patch-stdin --output diagram.json --json
```

Diagram and Mermaid read commands accept stdin when `--input` is omitted:

```bash
cat diagram.json | elegy diagram narrate --json
cat workflow.json | elegy mermaid render --json
```

## MCP Invocation

Start the host:

```bash
elegy run
```

The host serves:

- `resources/list`
- `resources/read`
- `tools/list`
- `tools/call`

`tools/list` is generated from the same built-in v2 skill registry as `elegy skills`. `tools/call` dispatches subprocess-backed capabilities through their implementation templates.

Side-effect policy:

- By default, side-effecting tools are blocked unless the call passes `dryRun=true` or `dry_run=true`.
- Start with `--allow-side-effects` only when the harness has its own approval gate.
- Use `--tool-timeout-seconds` and `--max-tool-output-bytes` to bound tool execution.

```bash
elegy run --allow-side-effects --tool-timeout-seconds 30 --max-tool-output-bytes 1048576
```

## JSON Envelope

Most `elegy` commands emit this envelope when called with `--json`:

```json
{
  "schema_version": "1",
  "correlationId": "optional-id",
  "command": ["diagram", "create"],
  "status": "ok",
  "summary": { "errors": 0, "warnings": 0 },
  "dataSchema": "elegy://schemas/canonical-diagram",
  "data": {},
  "diagnostics": []
}
```

Important fields:

| Field | Meaning |
| --- | --- |
| `status` | `ok`, `error`, `invalid`, or `not_found`. |
| `diagnostics` | Structured errors and warnings with codes and optional hints. |
| `dataSchema` | Optional schema URI for the payload type. |
| `data` | Command-specific payload. |
| `correlationId` | Optional value from `--correlation-id`. |

Dedicated binaries may use narrower envelopes, but they still support JSON output for automation.

## Practical Workflow

1. Run `elegy skills search --query <task> --json`.
2. Select the most relevant result.
3. Run `elegy skills describe --skill-id <id> --json`.
4. Read the target capability's `input.parameters`, `implementation.arguments`, and `execution.hasSideEffects`.
5. Invoke through subprocess or MCP.
6. Parse `status` and `diagnostics` before trusting `data`.

This pattern works for Claude Code, Codex, Copilot, ADK, MCP-native clients, and custom local agent harnesses.
