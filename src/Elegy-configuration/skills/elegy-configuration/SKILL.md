---
name: elegy-configuration
description: Use when an agent needs to list, inspect, apply, or verify deterministic configuration templates and profiles through the dedicated elegy-configuration CLI or the umbrella elegy configuration compatibility surface.
---

# Elegy-configuration Surface Bridge

This file is the surface-local, non-authoritative skill bridge shipped
with the `src/Elegy-configuration` wrapper surface.

Authority stays one-way:

1. `contracts/configuration/` plus the governed configuration schemas
   and fixtures remain the source of truth.
2. `skills/elegy-configuration/SKILL.md` is the canonical operational
   body.
3. This file mirrors install and CLI handoff needed by wrapper
   consumers.

## Wrapper install

- Run `./install.ps1` from this wrapper root.
- Pass `-LocalArtifactsRoot <path>` for local archives.

## Current commands

```text
elegy-configuration list --format json
elegy-configuration show [--package <path>] [--template-id <id> | --template-path <path>] --format json
elegy-configuration apply --target <dir> [--dry-run] [--package <path>] [--template-id <id> | --template-path <path> | --profile-id <id> | --profile-path <path>] [--binding KEY=VALUE] [--force] --format json
elegy-configuration verify --target <dir> [--package <path>] [--template-id <id> | --template-path <path> | --profile-id <id> | --profile-path <path>] [--binding KEY=VALUE] --format json
```

## Surface posture

- Deterministic materialization and drift verification only.
- The same behavior is available on the umbrella
  `elegy configuration ...` commands.
- For the full guardrails, common issues, and examples, load the
  canonical body: `../../../skills/elegy-configuration/SKILL.md`.
