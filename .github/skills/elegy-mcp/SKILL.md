---
name: elegy-mcp
description: Use when an agent needs to author a governed MCP server descriptor from a tool list, or analyze an existing governed MCP descriptor for tool-schema viability and trigger extraction. Authoring is lower-level contributor tooling; the daily MCP path is the elegy run host projection.
---

# Elegy MCP

> Use when an agent needs to author a governed MCP server descriptor from a tool list, or analyze an existing governed MCP descriptor for tool-schema viability and trigger extraction. Authoring is lower-level contributor tooling; the daily MCP path is the `elegy run` host projection.

This skill is the **author + analyze** pair for governed MCP
descriptors. The umbrella command is `elegy mcp author` and
`elegy mcp analyze`. There is no `elegy-mcp` binary in the current
distribution; the commands live on the umbrella `elegy` CLI.

## Quick start

1. Decide whether you need to author or analyze.
   - Use `mcp-author-descriptor` when starting from scratch and you
     have a tool list with names and descriptions.
   - Use `mcp-analyze-descriptor` when you already have a governed
     descriptor file and need a viability report.
2. Author from a tool list:
   `elegy author mcp --server-name <name> --output <path> --transport stdio --tool <NAME> --tool <NAME>=<description> --json`.
   Repeat `--tool` once per tool.
3. Analyze an existing descriptor:
   `elegy analyze mcp --descriptor <path> --json`. The result reports
   tool schema viability plus extracted triggers.
4. Inspect the result envelope for `data.tools[].schemaOk` and
   `data.triggers[]`. Both are advisory; a `schemaOk: false` does not
   fail the call, it flags a tool for review.
5. If authoring, confirm `--output` does not exist or pass `--force`
   to overwrite.

## Tool-call guardrails

### Author (`mcp-author-descriptor`)

- `--server-name` is required. It must be unique within the
  descriptors directory and use only `[a-z0-9-]`. Uniqueness is
  enforced against the descriptors directory, not just the output
  file.
- `--output` is required and must end in `.json`. The CLI writes a
  governed descriptor envelope to that path.
- `--transport` is `stdio` (default) or `http`. The transport is
  recorded in the descriptor but not validated against a real MCP
  server; the descriptor is metadata.
- `--tool` is repeated. Format is `NAME` or `NAME=DESCRIPTION`. Do
  not use `NAME: DESCRIPTION` or `NAME|DESCRIPTION` — only `=`.
  Descriptions are optional but improve downstream trigger
  extraction.
- `--force` is required to overwrite an existing file at
  `--output`. Without `--force`, the CLI fails rather than
  overwriting.
- Do not pass raw conversation transcripts as `--tool DESCRIPTION`.
  Distill the tool's purpose into one short sentence.
- Side-effect class: `disk_write` (creates a file at `--output`).
- Approval posture: `advisory` for new files; `required` with
  `--force`.

### Analyze (`mcp-analyze-descriptor`)

- `--descriptor` is the path to an existing governed descriptor. The
  CLI validates the descriptor against the schema before analyzing;
  an invalid descriptor fails the call with a validation report.
- The result is a viability report plus trigger extraction. Neither
  is authoritative for runtime behavior; the descriptor itself is
  metadata.
- The call is read-only. It does not modify the descriptor.
- Side-effect class: `read_only`.
- Approval posture: `none`.

## Workflow

1. Decide between author and analyze.
   - Author when you have a tool list and need a descriptor.
   - Analyze when you have a descriptor and need to understand it
     or audit it.
2. Gather inputs.
   - For author: collect tool names and one-line descriptions from
     the user or from the upstream MCP server. Do not guess tool
     names; missing or wrong names are the most common failure
     mode.
   - For analyze: confirm the descriptor path. A descriptor from
     another team may have a different schema; the analyzer only
     accepts descriptors conforming to the current governed schema.
3. Invoke.
   - Pass `--json` so the host can parse the result envelope. Do
     not omit it.
4. Read the result.
   - For author: confirm `data.descriptorPath` matches `--output`
     and that `data.tools` contains every tool the user named.
     Missing tools are a fail-stop.
   - For analyze: read `data.tools[].schemaOk` and surface
     `false` values to the user. Triggers in `data.triggers` are
     extracted heuristically; do not treat them as the only
     triggers.
5. Hand off.
   - The descriptor is metadata, not runtime. The next step for an
     authored descriptor is to register it with the host's MCP
     integration (out of scope for this skill).

## Capability index

| id | side-effect | purpose |
| -- | -- | -- |
| `mcp-author-descriptor` | disk_write | Create a governed MCP server descriptor from a tool list |
| `mcp-analyze-descriptor` | read-only | Validate and analyze an existing governed descriptor |

## Output envelope

- Envelope: `mcp-descriptor-result/v1` for both commands (declared in
  `contracts/schemas/mcp-descriptor-result.schema.json`).
- `status`: `ok` or `error`. Author can succeed with
  `data.tools[].schemaOk: false` flags without `status: error`.
- `data.descriptorPath`: written path on author; null on analyze.
- `data.tools`: per-tool records. For author, the tool list as
  written. For analyze, the tool list as parsed plus
  `schemaOk` and `issues[]` per tool.
- `data.triggers`: extracted trigger strings. Heuristic; advisory.
- `error`: machine-readable error code plus human message on
  failure.

## Common issues

| Symptom | Cause | Solution |
| -- | -- | -- |
| `mcp author` rejects with "server-name already in use". | Another descriptor in the directory uses the same server-name. | Pick a different `--server-name` or remove the conflicting descriptor. |
| Authored descriptor is missing a tool the user named. | The agent passed `--tool` once with a comma-joined list, which the CLI parses as one tool. | Repeat `--tool` once per tool. `NAME=DESCRIPTION` per occurrence. |
| `--tool NAME=DESCRIPTION` parses the description as the tool's id. | The `=` separator is the only one supported; the CLI split on the first `=`. | Confirm the name is on the left of the first `=`. The CLI does not warn on extra `=` in the description. |
| `mcp analyze` fails with "invalid descriptor" before producing a report. | The descriptor does not conform to the current schema, or it is a v1 descriptor from a previous format. | Re-author with the current schema, or update the descriptor with the `elegy mcp author --force` flow after reconciling the field set. |
| `data.tools[].schemaOk: false` on a tool the user expects to be valid. | The tool's name contains disallowed characters, or its description is empty. | Re-author with `[a-z0-9-]` for the name and a non-empty description. |
| `--output` already exists and the call fails without overwriting. | The CLI refuses to overwrite without `--force`. | Confirm the user wants to overwrite, then re-run with `--force`. |
| `data.triggers` is empty on a tool with a clear purpose. | The trigger extractor is heuristic. Empty triggers is a known false-negative, not a tool error. | Surface empty triggers as advisory. The next session's `agent discover` will still surface the tool via registry lookup. |
| The descriptor passes analysis but the MCP server fails to start with it. | The descriptor is metadata, not runtime config. The server uses its own tool definitions, not the descriptor's. | Do not assume authored descriptors drive server behavior. Pair the descriptor with the server's own tool manifest. |

## Version compatibility

- Minimum supported `elegy` umbrella version: `0.1.0` (the version
  that introduced the governed MCP descriptor schema).
- The descriptor schema version is tracked in
  `contracts/schemas/mcp-server-descriptor.schema.json`. Use a
  descriptor at the current schema version with the analyzer.
- The transport field is metadata; older MCP runtimes may not
  honor `http` even when the descriptor declares it.

## Examples

### Example 1 — author a small stdio descriptor

```text
elegy author mcp \
  --server-name demo \
  --output ./demo.descriptor.json \
  --transport stdio \
  --tool echo="Echoes a message back to the caller" \
  --tool reverse="Reverses a string"
```

Expected: `status: "ok"`, `data.descriptorPath: "./demo.descriptor.json"`,
`data.tools` contains `echo` and `reverse` with the supplied
descriptions.

### Example 2 — analyze the descriptor

```text
elegy analyze mcp --descriptor ./demo.descriptor.json --json
```

Expected: `status: "ok"`, `data.tools[].schemaOk: true` for both
tools, `data.triggers` populated from the descriptions.

## Boundaries

- This skill owns: governed MCP descriptor authoring and analysis.
- This skill does not own: MCP server runtime behavior, host
  registration of MCP servers, or tool-call invocation. Those live
  in the host's MCP integration.
- This skill does not own: registry discovery for MCP servers. The
  registry is for skills, not MCP descriptors.
- Companion skills:
  - `elegy-skills` — for registry-first discovery of skills. MCP
    descriptors and skills are distinct surfaces.
  - `elegy-skill-authoring` — for SKILL.md audit and review; not
    applicable to MCP descriptors.
  - `elegy-mermaid` — for diagram rendering when the MCP
    descriptor analysis includes a flow diagram.

## References

- Governed source: `contracts/fixtures/skill.elegy-mcp.json`.
- Discovery projection:
  `contracts/fixtures/skill-discovery-index.elegy-mcp.json`.
- Architecture: `docs/architecture/mcp-skill-tooling-placement.md`.
- Result envelope schema:
  `contracts/schemas/mcp-descriptor-result.schema.json`.
- Descriptor schema:
  `contracts/schemas/mcp-server-descriptor.schema.json`.
- Analysis schema:
  `contracts/schemas/mcp-analysis-result.schema.json`.
- Parity expectation fixture:
  `contracts/fixtures/mcp-parity-expected.json`.
