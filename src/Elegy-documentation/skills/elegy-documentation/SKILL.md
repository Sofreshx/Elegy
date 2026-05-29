---
name: elegy-documentation
description: "Surface-local non-authoritative bridge shipped with the Elegy-documentation wrapper surface and wrapper archive."
---

# Elegy-documentation Surface Bridge

This file is a surface-local, non-authoritative skill bridge shipped with the `src/Elegy-documentation` wrapper surface and the `elegy-documentation-wrapper-<bundleVersion>.zip` archive.

Authority stays one-way:

1. `contracts/fixtures/skill-definition-v2.elegy-documentation.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-documentation.json` is the governed discovery projection.
3. `.github/skills/elegy-documentation/SKILL.md` remains the repo-local contributor-routing output.
4. This file mirrors the install and CLI handoff needed by wrapper consumers.

## Wrapper install

- Run `./install.ps1` from this wrapper root to stage the contracts bundle, the `elegy-documentation` CLI surface, and this wrapper surface together.
- Pass `-LocalArtifactsRoot <path>` when validating against local archives instead of GitHub release assets.

## Current commands

```text
elegy-documentation init --project <path> [--dry-run]
elegy-documentation inspect --project <path>
elegy-documentation map --project <path>
elegy-documentation check --project <path>
elegy-documentation export llms --project <path> --output <path>
elegy-documentation export bundle --project <path> --output <path>
```

`--json` is available when structured output is needed.

## Surface posture

- This dedicated surface is documentation-authority aware and deterministic.
- Source documents remain authoritative; generated llms and bundle outputs remain derived.
- `skills/elegy-doc-practices/` remains the doctrine layer; this dedicated surface is the tool lane.
