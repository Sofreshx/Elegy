---
name: elegy-memory
description: "Surface-local non-authoritative bridge shipped with the Elegy-memory wrapper surface and wrapper archive for the implemented memory MVP CLI."
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
elegy-memory add <content> [--db <path>] [--scope <session|workspace|user|agent>] [--type <fact|preference|decision|procedure|observation>] [--importance <0..1>] [--provenance <user-stated|agent-observed|consolidated|imported|agent-inferred>]
elegy-memory search <query> [--db <path>] [--scope <session|workspace|user|agent>] [--limit <n>] [--include-dormant]
elegy-memory list [--db <path>] [--scope <session|workspace|user|agent>] [--type <fact|preference|decision|procedure|observation>] [--state <active|dormant|deleted>] [--limit <n>]
elegy-memory inspect <id> [--db <path>] [--scope <session|workspace|user|agent>]
elegy-memory purge [--db <path>] [--scope <session|workspace|user|agent>] [--yes]
elegy-memory health [--db <path>] [--scope <session|workspace|user|agent>]
elegy-memory export [--db <path>] [--scope <session|workspace|user|agent>] [--output <path>]
elegy-memory reembed [--db <path>] [--scope <session|workspace|user|agent>] [--provider <name>] [--limit <n>]
elegy-memory contradictions [--db <path>] [--scope <session|workspace|user|agent>]
elegy-memory --format json <command> ...
```

## Current behavior

- Default database path: `~/.elegy/memory.db`.
- Default scope: `workspace`.
- `search` is keyword-only in the current MVP CLI.
- `purge` prompts unless `--yes` is passed.
- `reembed` is present as a preview command surface but is not wired to a provider yet.

This wrapper ships the current CLI handoff. It should not be described as the older local artifact import and supersede/tombstone flow.