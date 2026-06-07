---
name: elegy-mermaid
description: Use when an agent needs to render governed canonical workflow JSON into Mermaid flowchart text, reverse Mermaid text into a bounded workflow-graph projection, or produce a concise Mermaid + canonical narrative output through the umbrella elegy CLI.
---

# Elegy Mermaid

> Use when an agent needs to render governed canonical workflow JSON into Mermaid flowchart text, reverse Mermaid text into a bounded workflow-graph projection, or produce a concise narrative output through the umbrella `elegy mermaid` commands.

Mermaid is a projection surface. Governed canonical workflow artifacts
remain the authority. All three commands (render, reverse, narrate) are
projections derived from those artifacts, not canonical replacements.

## Quick start

1. Render canonical workflow JSON to Mermaid:
   `elegy mermaid render --input <path> --format json` or pipe
   canonical JSON on stdin: `cat workflow.json | elegy mermaid render --format json`.
2. Reverse a Mermaid flowchart back to a bounded graph report:
   `elegy mermaid reverse --input <path> --format json` or pipe on
   stdin.
3. Narrate a Mermaid flowchart in natural language:
   `elegy mermaid narrate --input <path> --format json` or pipe on
   stdin.

## Tool-call guardrails

### Render (`mermaid-render`)

- Accepts governed canonical workflow JSON (formats:
  `canonical-workflow`, `canonical-workflow-graph`). Rejects
  arbitrary JSON — if the input is not a recognized canonical
  shape, the call fails with `status: "error"`.
- `--input` is optional. When omitted, reads from stdin.
  `stdinFormat: "json"` is declared; do not pipe Mermaid text to
  render.
- The output is a deterministic `flowchart TD` Mermaid string.
  Node ids are generated from the canonical graph and are stable
  for the same input.
- Duplicate step ids in the input are rejected. The renderer
  enforces unique ids; do not attempt to render a graph with
  collisions.
- Undeclared step references (edges pointing to missing nodes) are
  also rejected.
- Side-effect class: `read_only`.
- Approval posture: `none`.

### Reverse (`mermaid-reverse`)

- Accepts a bounded Mermaid `flowchart TD` subset compatible with
  the renderer output. Arbitrary Mermaid beyond this subset is
  rejected.
- Reports derived node ids, node labels, trigger/activity roles,
  activation edges, transition edges, and entry node ids.
- Does **not** perform full canonical workflow reconstruction.
  Do not describe reverse output as canonical or claim it recovers
  the original governed workflow.
- Side-effect class: `read_only`.
- Approval posture: `none`.

### Narrate (`mermaid-narrate`)

- Accepts Mermaid `flowchart TD` text and produces a concise
  natural-language narrative of the flow: entry points, step
  order, decisions, terminal states.
- The narrative is a projection of the Mermaid text, not of the
  original canonical workflow.
- Side-effect class: `read_only`.
- Approval posture: `none`.

## Workflow

1. Decide which direction you need.
   - Render: you have a canonical workflow JSON and need a Mermaid
     diagram.
   - Reverse: you have a Mermaid diagram and need a bounded
     workflow-graph semantic report.
   - Narrate: you have a Mermaid diagram and need a short
     natural-language description.
2. Invoke with `--format json` for machine parsing.
3. Read the result envelope. Render returns `data.mermaid` (the
   Mermaid string). Reverse returns `data.nodes[]` and
   `data.edges[]`. Narrate returns `data.narrative` (a plain text
   paragraph).
4. Do not treat any output as canonical. For downstream
   consumers, the canonical source is the original governed
   workflow artifact, not the Mermaid projection.

## Capability index

| id | side-effect | purpose |
| -- | -- | -- |
| `mermaid-render` | read-only | Render canonical workflow JSON to Mermaid flowchart text |
| `mermaid-reverse` | read-only | Reverse bounded Mermaid to workflow-graph projection |
| `mermaid-narrate` | read-only | Produce concise narrative from Mermaid text |

## Output envelope

- Envelope: `mermaid-result/v1` (declared in
  `contracts/schemas/mermaid-result.schema.json`) for all three
  commands.
- `render`: `data.mermaid` is the Mermaid string. `data.format` is
  `"text"` or `"json"` matching the requested format.
- `reverse`: `data.nodes[]` (id, label, role), `data.edges[]`
  (source, target, type), `data.entryNodeIds[]`.
- `narrate`: `data.narrative` is a concise natural-language
  paragraph. `data.stepCount` is the number of steps covered.
- All three: `status: "ok" | "error"`. `error` on `render` for
  invalid input shape, duplicate ids, or undeclared references.

## Common issues

| Symptom | Cause | Solution |
| -- | -- | -- |
| `render` rejects valid-looking JSON. | The input JSON is not one of the two supported canonical shapes (`canonical-workflow`, `canonical-workflow-graph`). | Validate the input against the canonical workflow schema first, or re-export from the governed fixture that authored it. |
| `render` rejects with "duplicate id" on a graph you edited by hand. | Step ids must be unique across the whole graph. A copy-paste retained a duplicate. | Deduplicate ids and re-render. |
| `render` rejects with "undeclared reference" for an edge target. | The edge points to a node that was not declared in the graph. | Add the missing node or remove the dangling edge. |
| `reverse` rejects Mermaid that `render` produced. | The Mermaid text was edited by hand after rendering and the edit introduced unsupported syntax. | Re-render from the canonical fixture to get valid reverse input. |
| `reverse` produces fewer nodes than the Mermaid diagram shows. | The reverse parser is bounded to the `flowchart TD` subset the renderer emits. Compound node syntax or custom shapes are not supported. | Simplify the Mermaid to the renderer-compatible subset. |
| `narrate` produces a generic description that misses key decision points. | The Mermaid input uses abbreviations or non-descriptive labels. | Use full-word labels in the Mermaid text. The narrative engine is label-driven. |
| stdin input is read as an empty string. | The subprocess was not configured to pipe stdin, or the input file path was passed as `--input` *and* stdin was empty. | Pick one: `--input <path>` or stdin. The CLI reads the first non-empty source and ignores the other. |

## Version compatibility

- Minimum supported `elegy` umbrella version: `0.1.0`. The
  `mermaid` subcommands live on the umbrella CLI.
- The canonical workflow schema version is declared in
  `contracts/schemas/canonical-workflow.schema.json`. Confirm the
  input matches this version before rendering.
- Semver rule: the CLI rejects input at an incompatible schema
  version; re-export the fixture at the current version.

## Examples

### Example 1 — render a canonical workflow to Mermaid

```text
elegy mermaid render --input contracts/fixtures/canonical-workflow.minimal.json --format json
```

Expected:

```json
{
  "status": "ok",
  "data": {
    "mermaid": "flowchart TD\n  A[Start] --> B[Decision]\n  B -->|yes| C[Action]\n  B -->|no| D[End]",
    "format": "json"
  }
}
```

### Example 2 — reverse a Mermaid diagram

```text
echo "flowchart TD\n  A[Start] --> B[End]" | elegy mermaid reverse --format json
```

Expected:

```json
{
  "status": "ok",
  "data": {
    "nodes": [
      { "id": "A", "label": "Start", "role": "entry" },
      { "id": "B", "label": "End", "role": "terminal" }
    ],
    "edges": [
      { "source": "A", "target": "B", "type": "transition" }
    ],
    "entryNodeIds": ["A"]
  }
}
```

## Boundaries

- This skill owns: Mermaid projection from canonical workflows,
  bounded Mermaid reverse analysis, and narrative output.
- This skill does not own: canonical workflow authority (the
  governed `canonical-workflow` artifacts are authority),
  workflow execution, or workflow editing.
- This skill does not own: random Mermaid diagrams the user pastes
  from external sources. Reverse and narrate accept only the
  bounded subset produced by render.
- Companion skills:
  - `elegy-documentation` — for exporting flowchart docs.
  - `elegy-planning` — for tracking workflow changes.
  - `elegy-skill-authoring` — for other doctine surfaces.

## References

- Governed source: `contracts/fixtures/skill.elegy-mermaid.json`.
- Discovery projection:
  `contracts/fixtures/skill-discovery-index.elegy-mermaid.json`.
- Architecture: `docs/architecture/mermaid-tooling.md`.
- Result envelope: `contracts/schemas/mermaid-result.schema.json`.
- Canonical workflow tools: `contracts/fixtures/canonical-workflow.minimal.json`,
  `contracts/fixtures/canonical-workflow-graph.minimal.json`.
