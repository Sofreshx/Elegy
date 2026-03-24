---
name: elegy-memory
description: "Surface-local non-authoritative bridge shipped with the Elegy-memory wrapper surface and wrapper archive."
---

# Elegy-memory Surface Bridge

This file is a surface-local, non-authoritative skill bridge shipped with the `src/Elegy-memory` wrapper surface and the `elegy-memory-wrapper-<bundleVersion>.zip` archive.

Authority stays one-way:

1. `contracts/fixtures/skill-definition.elegy-memory.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-memory.json` is the governed discovery projection.
3. `.github/skills/elegy-memory/SKILL.md` remains the repo-local contributor-routing output.
4. This file mirrors the install and CLI handoff needed by wrapper consumers.

## Wrapper install

- Run `./install.ps1` from this wrapper root to stage the contracts bundle, the `elegy-memory` CLI surface, and this wrapper surface together.
- Pass `-LocalArtifactsRoot <path>` when validating against local archives instead of GitHub release assets.

## Current commands

```text
elegy-memory inspect
elegy-memory validate --input <path>
elegy-memory init [--root <path>]
elegy-memory import --input <path> --record-id <record-id> --imported-at-utc <utc> [--root <path>]
elegy-memory list [--root <path>] [--include-superseded] [--include-tombstoned]
elegy-memory show --record-id <record-id> [--root <path>] [--include-superseded] [--include-tombstoned]
elegy-memory export --record-id <record-id> [--output-path <path>] [--root <path>] [--include-superseded] [--include-tombstoned]
elegy-memory supersede --record-id <record-id> --superseded-by-record-id <record-id> [--root <path>]
elegy-memory tombstone --record-id <record-id> --tombstoned-at-utc <utc> --reason <text> [--root <path>]
```

`--format json` is available when structured output is needed.