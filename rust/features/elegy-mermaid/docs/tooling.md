# Mermaid Tooling

## Purpose

This document describes the current Mermaid tooling slice exposed through the umbrella `elegy` CLI.

The Mermaid surface is intentionally projection-oriented:

- governed canonical workflow artifacts remain the authority source
- Mermaid output is derived
- reverse and narrative outputs are derived reports over workflow-graph semantics
- no dedicated Mermaid binary, wrapper root, or distribution lane is introduced in this slice

## Authority posture

Mermaid is not a canonical workflow authority surface.

The durable authority remains in governed canonical artifacts such as:

- `contracts/fixtures/canonical-workflow.minimal.json`
- `contracts/fixtures/canonical-workflow-graph.minimal.json`
- the corresponding governed schemas and manifests exported from `contracts/`

Mermaid render output, Mermaid reverse output, and Mermaid narrative output are all projections of that authority. They are useful for visualization, explanation, and bounded machine-readable reporting, but they do not replace canonical workflow truth.

## Current commands

```text
elegy mermaid render [--input <path>] [--format text|json]
elegy mermaid reverse [--input <path>] [--format text|json]
elegy mermaid narrate [--input <path>] [--format text|json]
```

When `--input` is omitted, each command reads from stdin.

### Render

`render` accepts governed canonical workflow JSON and produces deterministic Mermaid `flowchart TD` output.

Supported canonical inputs:

- `canonical-workflow`
- `canonical-workflow-graph`

`--format json` wraps the Mermaid string in the standard CLI envelope for machine use.

### Reverse

`reverse` accepts a bounded Mermaid `flowchart TD` subset compatible with the current renderer output and emits a stable workflow-graph-semantics projection.

The reverse projection reports:

- derived Mermaid node ids
- node labels and trigger/activity roles
- activation edges and transition edges
- derived entry node ids from activation edges, or graph roots when no activation edges are present
- optional source metadata when it can be inferred safely from renderer-style node ids

`reverse` does not attempt full canonical workflow reconstruction. It does not recover canonical layout, conflict policy, blueprint metadata, or other canonical-only fields.

### Narrate

`narrate` accepts either:

- governed canonical workflow JSON
- Mermaid `flowchart TD` input compatible with the current renderer subset

The command emits a concise derived narrative rooted in the same shared workflow-graph projection model used by `reverse`.

`--format json` includes both the narrative and the underlying projection so machine consumers do not need to parse the text body.

## Reverse subset boundaries

The current reverse parser is intentionally narrow.

Supported input shape:

- directive line exactly `flowchart TD`
- activity nodes emitted as `nodeId["Label"]`
- trigger nodes emitted as `nodeId(("Label"))`
- edges emitted as `from --> to` or `from -->|label| to`

Anything outside that subset is rejected rather than guessed.

## Governed skill surface

The Mermaid contributor-routing skill follows the same authority split as the other current skill surfaces:

1. `contracts/fixtures/skill.elegy-mermaid.json` is authoritative.
2. `contracts/fixtures/skill-discovery-index.elegy-mermaid.json` is the governed discovery projection.
3. SKILL.md mirrors are generated via `elegy plugin export` and are repo-local non-authoritative rendered outputs.

This skill surface exists to help AI-capable tooling discover the Mermaid commands through governed metadata while keeping Mermaid explicitly non-authoritative.
