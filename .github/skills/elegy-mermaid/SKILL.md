---
name: elegy-mermaid
description: "Repo-local non-authoritative contributor-routing file for Elegy's Mermaid tooling surface. Use for canonical Mermaid render, bounded Mermaid reverse projection, and concise Mermaid/canonical narrative output through the umbrella elegy CLI."
---

# Elegy Mermaid

This file is a repo-local, non-authoritative contributor-routing output.

The authority chain is one-way:

1. `contracts/fixtures/skill-definition.elegy-mermaid.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-mermaid.json` is the governed discovery projection derived from that definition.
3. `.github/skills/elegy-mermaid/SKILL.md` is a repo-local contributor-routing file only.

## When to use

- Use `elegy mermaid render` to project governed `canonical-workflow` or `canonical-workflow-graph` JSON into Mermaid `flowchart TD` output.
- Use `elegy mermaid reverse` to project Mermaid `flowchart TD` content compatible with the current renderer into a bounded workflow-graph-semantics report.
- Use `elegy mermaid narrate` to produce a concise derived narrative from either governed canonical JSON or Mermaid `flowchart TD` input.
- Prefer `--format json` when a machine-readable projection or narrative report is needed.

## Do not use

- Do not treat Mermaid as canonical workflow authority.
- Do not describe reverse output as full canonical workflow reconstruction.
- Do not infer a dedicated Mermaid binary, wrapper root, or release lane from this surface.

## Current commands

```text
elegy mermaid render [--input <path>] [--format text|json]
elegy mermaid reverse [--input <path>] [--format text|json]
elegy mermaid narrate [--input <path>] [--format text|json]
```

When `--input` is omitted, the commands read from stdin.

## Surface posture

- This tooling lives in `rust/crates/elegy-mermaid` and is exposed through the umbrella `elegy` CLI only.
- `reverse` emits a bounded workflow-graph-semantics projection rooted in Mermaid node ids, labels, entry nodes, and activation/transition edges.
- `narrate` emits a derived explanation over the same projection model; it does not upgrade Mermaid into an authority surface.