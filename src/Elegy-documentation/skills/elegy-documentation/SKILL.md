---
name: elegy-documentation
description: Use when an agent needs to initialize, inspect, map, objectively check, or export repo-local documentation configuration through the dedicated elegy-documentation CLI.
---

# Elegy-documentation Surface Bridge

This file is the surface-local, non-authoritative skill bridge shipped
with the `src/Elegy-documentation` wrapper surface and the
`elegy-documentation-wrapper-<bundleVersion>.zip` archive.

Authority stays one-way:

1. `contracts/fixtures/skill.elegy-documentation.json` is the governed
   source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-documentation.json`
   is the governed discovery projection.
3. `skills/elegy-documentation/SKILL.md` is the canonical operational
   body.
4. This file mirrors install and CLI handoff needed by wrapper
   consumers.

## Wrapper install

- Run `./install.ps1` from this wrapper root.
- Pass `-LocalArtifactsRoot <path>` for local archives.

## Current commands

```text
elegy-documentation init --project <path> [--dry-run] --json
elegy-documentation inspect --project <path> --json
elegy-documentation map --project <path> --json
elegy-documentation check --project <path> --json
elegy-documentation export llms --project <path> --output <path> --json
elegy-documentation export bundle --project <path> --output <path> --json
```

## Surface posture

- Source documents remain authoritative; generated llms and bundle
  outputs remain derived.
- `skills/elegy-doc-practices/` remains the doctrine layer; this
  dedicated surface is the tool lane.
- For the full guardrails, common issues, and examples, load the
  canonical body: `../../../skills/elegy-documentation/SKILL.md`.
