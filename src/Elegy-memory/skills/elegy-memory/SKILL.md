---
name: elegy-memory
description: Use when an agent needs to add, search, list, inspect, purge, health-check, export, or contradiction-review local memory records through the dedicated elegy-memory CLI over SQLite. The current MVP is keyword-only and SQLite-backed; embedding providers are preview-only.
---

# Elegy-memory Surface Bridge

This file is the surface-local, non-authoritative skill bridge shipped
with the `src/Elegy-memory` wrapper surface and the
`elegy-memory-wrapper-<bundleVersion>.zip` archive. It is a thin
install-and-handoff page; the canonical operational body lives in the
in-tree `skills/elegy-memory/SKILL.md` and is mirrored to
`.agents/skills/elegy-memory/SKILL.md` and
`.github/skills/elegy-memory/SKILL.md`.

Authority stays one-way:

1. `contracts/fixtures/skill.elegy-memory.json` is the governed source
   of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-memory.json` is the
   governed discovery projection.
3. `skills/elegy-memory/SKILL.md` is the canonical operational body.
4. This file mirrors install and CLI handoff needed by wrapper
   consumers.

## Wrapper install

- Run `./install.ps1` from this wrapper root to stage the contracts
  bundle, the `elegy-memory` CLI surface, and this wrapper surface
  together.
- Pass `-LocalArtifactsRoot <path>` when validating against local
  archives instead of GitHub release assets.

## Current commands

```text
elegy-memory add <content> [--db <path>] [--scope <session|workspace|user|agent>] [--type <fact|preference|decision|procedure|observation>] [--importance <0..1>] [--provenance <user-stated|agent-observed|consolidated|imported|agent-inferred>] --format json
elegy-memory search <query> [--db <path>] [--scope <scope>] [--limit <n>] [--include-dormant] --format json
elegy-memory list [--db <path>] [--scope <scope>] [--type <type>] [--state <active|dormant|deleted>] [--limit <n>] --format json
elegy-memory inspect <id> [--db <path>] [--scope <scope>] --format json
elegy-memory purge [--db <path>] [--scope <scope>] --yes
elegy-memory health [--db <path>] [--scope <scope>] --format json
elegy-memory export [--db <path>] [--scope <scope>] [--output <path>] --format json
elegy-memory reembed [--db <path>] [--scope <scope>] [--provider <name>] [--limit <n>] --format json
elegy-memory contradictions [--db <path>] [--scope <scope>] --format json
```

## Current behavior

- Default database path: `~/.elegy/memory.db`.
- Default scope: `workspace`. Always pass `--scope` explicitly to
  avoid cross-scope pollution.
- `search` is keyword-only in the current MVP CLI. Semantic search
  via embedding provider is preview-only.
- `purge` prompts unless `--yes` is passed. In machine mode, always
  pass `--yes`; otherwise the call fails.
- `reembed` is preview-only. The CLI accepts the call and emits a
  preview report but does not call any provider. Treat its output
  as advisory.
- `contradictions` is rule-based and may over-fire on
  near-duplicates. Treat as triage, not as automatic resolution.
- The salience gate is mandatory on every `add`. A `gate: rejected`
  response means the content was too long, too vague, or a
  duplicate. Distill and re-add.

## Agent invocation guidance

- Distill raw observations into one or two short sentences before
  calling `add`. The gate penalizes raw transcripts.
- Always pass `--provenance` and `--scope` explicitly.
- For mutations and reads, pass `--format json` so the host can
  parse the result envelope.
- Confirm `--output` paths with the user before invoking `export`
  or `purge`.
- For the full guardrails, common issues, and worked examples, load
  the canonical body: `../../../skills/elegy-memory/SKILL.md`.
