---
name: elegy-configuration
description: "Surface-local non-authoritative bridge shipped with the Elegy-configuration wrapper surface and wrapper archive."
---

# Elegy-configuration Surface Bridge

This file is a surface-local, non-authoritative skill bridge shipped with the `src/Elegy-configuration` wrapper surface and the `elegy-configuration-wrapper-<bundleVersion>.zip` archive.

Authority stays one-way:

1. `contracts/configuration/` plus the governed configuration schemas and fixtures remain the source of truth.
2. `.github/skills/elegy-configuration/SKILL.md` remains the repo-local contributor-routing output.
3. This file mirrors the install and CLI handoff needed by wrapper consumers.

## Wrapper install

- Run `./install.ps1` from this wrapper root to stage the contracts bundle, the `elegy-configuration` CLI surface, and this wrapper surface together.
- Pass `-LocalArtifactsRoot <path>` when validating against local archives instead of GitHub release assets.

## Current commands

```text
elegy-configuration list
elegy-configuration show [--package <path>] [--template-id <id> | --template-path <path>]
elegy-configuration apply --target <dir> [--dry-run] [--package <path>] [--template-id <id> | --template-path <path> | --profile-id <id> | --profile-path <path>] [--binding KEY=VALUE] [--force]
elegy-configuration verify --target <dir> [--package <path>] [--template-id <id> | --template-path <path> | --profile-id <id> | --profile-path <path>] [--binding KEY=VALUE]
```

`--format json` is available when structured output is needed.

## Surface posture

- This dedicated surface owns deterministic materialization and drift verification only.
- The same behavior is also available on the umbrella `elegy configuration ...` commands.
- Portable package loading currently supports local package files that carry governed configuration components; host install state, trust, auth, approvals, and runtime registration remain outside this surface.
