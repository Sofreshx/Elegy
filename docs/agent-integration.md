---
title: Agent Integration Guide
description: How to use Elegy from an LLM agent or automation system.
created: 2025-07-15
updated: 2025-07-15
category: architecture
status: active
doc_kind: guide
---

# Agent Integration Guide

Elegy provides a CLI-first tool surface designed for consumption by LLM agents, orchestrators, and automation systems. Every command emits structured output, accepts JSON input, and is discoverable at runtime.

## Quick start

### 1. Discover available capabilities

```bash
elegy skills list --json
```

Returns a JSON envelope listing all registered skill definitions with their capability counts and lifecycle states.

### 2. Inspect a specific skill

```bash
elegy skills describe --skill-id diagram --json
```

Returns the full v2 skill definition including per-capability implementation details, input parameters, and execution characteristics.

### 3. Search by keyword

```bash
elegy skills search --query "render" --json
```

Matches against keywords, triggers, capability names, and descriptions.

## Invocation patterns

### Subprocess invocation

Every capability in a v2 skill definition includes an `implementation` block:

```json
{
  "implementation": {
    "executionType": "subprocess",
    "executableName": "elegy",
    "arguments": ["diagram", "create", "--diagram-type", "${type}", "--json"]
  }
}
```

Replace `${var}` placeholders with actual values and invoke as a subprocess.

### JSON stdin for mutations

For mutation commands, prefer `--patch-stdin` over positional arguments:

```bash
echo '{"addNodes":[{"id":"cache","label":"Cache Layer"}]}' | elegy diagram patch --input diagram.json --patch-stdin --json
```

This avoids escaping issues and provides structured input.

### Stdin for read commands

Diagram and Mermaid commands accept input from stdin when `--input` is omitted:

```bash
cat diagram.json | elegy diagram narrate --json
cat diagram.json | elegy diagram render --json
```

### File write-back

Use `--output` to write results to a file:

```bash
echo '{"addNodes":[{"id":"db","label":"Database"}]}' | elegy diagram patch --input diagram.json --patch-stdin --output diagram.json --json
```

## Output contract

All commands emit a governed JSON envelope when invoked with `--json`:

```json
{
  "schema_version": "1",
  "correlation_id": "optional-tracking-id",
  "command": ["diagram", "patch"],
  "status": "ok",
  "summary": {},
  "data": { "..." : "..." },
  "diagnostics": []
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | string | Envelope schema version (currently `"1"`). |
| `correlation_id` | string | Optional tracking ID passed via `--correlation-id`. |
| `command` | string[] | The command path that was invoked. |
| `status` | string | `"ok"`, `"error"`, `"invalid"`, or `"not_found"`. |
| `summary` | object | Command-specific summary metadata. |
| `data` | any | The primary response payload. Type varies by command. |
| `diagnostics` | array | Structured diagnostic messages (errors, warnings). |

### Diagnostics

Each diagnostic has:

```json
{
  "severity": "error",
  "code": "CLI-DIAGRAM-001",
  "message": "failed to read diagram file: No such file",
  "path": "missing.json"
}
```

### Status codes

| Status | Meaning | Exit code |
|--------|---------|-----------|
| `ok` | Success | 0 |
| `error` | Runtime failure | 2 |
| `invalid` | Invalid input | 1 |
| `not_found` | Resource not found | 1 |

## Skill definition reference (v2)

V2 skill definitions live in `contracts/fixtures/skill-definition-v2.*.json` and are embedded in the binary at compile time.

### Structure

```json
{
  "skillFormat": "elegy-skill-definition",
  "skillVersion": 2,
  "identity": { "namespace": "elegy", "name": "diagram", "version": "0.1.0" },
  "metadata": { "displayName": "...", "description": "...", "category": "..." },
  "capabilities": [
    {
      "id": "diagram-patch",
      "name": "Patch Diagram",
      "description": "...",
      "implementation": {
        "executionType": "subprocess",
        "executableName": "elegy",
        "arguments": ["diagram", "patch", "--input", "${inputPath}", "--patch-stdin", "--json"]
      },
      "input": {
        "parameters": [
          { "name": "inputPath", "type": "path", "required": true }
        ],
        "stdinFormat": "json"
      },
      "output": { "description": "The patched CanonicalDiagram JSON object." },
      "execution": { "mode": "requestResponse", "hasSideEffects": true },
      "composesWell": {
        "typicalNext": ["diagram-render", "diagram-narrate"],
        "pipeableTo": ["diagram-render", "diagram-narrate"]
      }
    }
  ],
  "constraints": [{ "constraintId": "...", "description": "...", "required": true }],
  "governance": { "riskLevel": "low", "approvalRequirement": "none" },
  "discovery": { "keywords": ["..."], "triggers": [{ "pattern": "...", "description": "..." }] },
  "lifecycleState": "active"
}
```

### Key sections for agents

- **`capabilities[].implementation`** — Exact invocation command. Replace `${var}` placeholders.
- **`capabilities[].input.parameters`** — Typed parameters with names, types, defaults, and required flags.
- **`capabilities[].input.stdinFormat`** — When present, the command accepts structured input on stdin.
- **`capabilities[].execution`** — Whether the command has side effects, is deterministic, and timeout hints.
- **`capabilities[].composesWell`** — Hints for chaining commands together.
- **`governance`** — Risk level and approval requirements — useful for safety-conscious agents.

## Example agent workflows

### Workflow 1: Create and render a diagram

```bash
# Step 1: Create a new diagram
elegy diagram create --diagram-type architecture --json > arch.json

# Step 2: Add nodes via patch
echo '{"addNodes":[{"id":"api","label":"API Gateway"},{"id":"db","label":"Database"}],"addEdges":[{"id":"e1","sourceId":"api","targetId":"db","label":"queries"}]}' \
  | elegy diagram patch --input arch.json --patch-stdin --output arch.json --json

# Step 3: Render to Mermaid
elegy diagram render --input arch.json --json
```

### Workflow 2: Discover and invoke

```bash
# Step 1: Find relevant skills
elegy skills search --query "diagram" --json

# Step 2: Read the implementation details
elegy skills describe --skill-id diagram --json

# Step 3: Invoke the capability using the implementation block
elegy diagram create --diagram-type concept --json
```

### Workflow 3: Pipe-based composition

```bash
# Create → Narrate in a pipeline
elegy diagram create --diagram-type concept --json \
  | jq '.data' \
  | elegy diagram narrate --json
```

## Version detection

Agents can detect capabilities before invocation:

```bash
elegy --version --json
```

Returns:

```json
{
  "data": {
    "version": "0.1.0",
    "cliSchemaVersion": "1",
    "availableCommands": ["author", "analyze", "generate", "validate", "inspect", "local", "mermaid", "diagram", "run", "contracts", "skills", "observe", "desktop", "repo", "web", "data", "notify"],
    "skillDefinitionFormat": 2,
    "mcpHostCapable": true
  }
}
```

## Available command families

| Family | Purpose | Side effects |
|--------|---------|--------------|
| `diagram create` | Create empty diagrams | No |
| `diagram patch` | Add/remove nodes and edges | Yes (with `--output`) |
| `diagram narrate` | Generate narrative summaries | No |
| `diagram render` | Render to Mermaid or other formats | No |
| `mermaid render` | Render canonical workflows to Mermaid | No |
| `mermaid reverse` | Reverse Mermaid to workflow graph | No |
| `mermaid narrate` | Narrate workflows or Mermaid | No |
| `skills list` | List all skill definitions | No |
| `skills describe` | Show full skill detail | No |
| `skills search` | Search skills by keyword | No |
| `repo status` | Return structured repository status | No |
| `repo diff` | Return bounded repository diff summaries | No |
| `repo branches` | List local branches and upstream refs | No |
| `repo log` | Return bounded structured commit history | No |
| `web fetch` | Perform bounded HTTP fetches with optional extraction | Yes |
| `web ping` | Perform bounded HTTP reachability checks | No |
| `data convert` | Convert between JSON, YAML, TOML, and CSV | No |
| `data extract` | Extract JSON values by pointer or dotted path | No |
| `data validate` | Validate JSON against a JSON Schema | No |
| `notify toast` | Deliver a local toast notification | Yes |
| `notify webhook` | Send an outbound webhook POST | Yes |

## MCP integration

Elegy's MCP host (`elegy-host-mcp`) serves both resources and tools over stdio. Each embedded skill capability is exposed as an MCP tool callable via `tools/list` and `tools/call`.
