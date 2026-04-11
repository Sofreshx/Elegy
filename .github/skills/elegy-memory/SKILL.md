---
name: elegy-memory
description: "Repo-local non-authoritative contributor-routing file for Elegy's implemented in-repo memory V1 surface. Use for summary-only session-context inspection or validation and local non-authoritative artifact import, list, show, export, supersede, and tombstone flows."
---

# Elegy Memory

This file is a repo-local, non-authoritative contributor-routing output for external-agent integration with the dedicated `elegy-memory` surface.

External agents outside Elegy should load this skill as routing guidance, then invoke the dedicated `elegy-memory` CLI directly. Elegy itself does not orchestrate or call agents internally through this file.

The authority chain is one-way:

1. `contracts/fixtures/skill-definition.elegy-memory.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-memory.json` is the governed discovery projection derived from that definition.
3. `.github/skills/elegy-memory/SKILL.md` is a repo-local contributor-routing file only.

## When to use

- If you are operating as an external agent outside Elegy, load this skill and invoke the dedicated `elegy-memory` binary directly for the bounded memory surface.
- Inspect the governed summary-only session-context contract with `elegy-memory inspect`.
- Validate a summary-only session-context artifact with `elegy-memory validate --input <path>`.
- Initialize or inspect the local memory store with `elegy-memory init`, `elegy-memory list`, and `elegy-memory show`.
- Import or export governed summary-only session-context artifacts with `elegy-memory import` and `elegy-memory export`.
- Record local lineage or local withdrawal with `elegy-memory supersede` and `elegy-memory tombstone`.
- Treat `elegy` memory commands as a temporary legacy compatibility bridge, not the preferred path.

## Do not use

- Do not treat this skill as authority for currentness, approval, freshness, retrieval ranking, runtime validation, or promotion.
- Do not infer runtime invalidation from local supersede or tombstone actions.
- Do not use this skill to describe raw transcript storage or broad host-owned persistence behavior.

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

`--format json` is available on the CLI when structured output is needed.

## Consumption posture

- This skill is a routing bridge for external agents and contributors, not an internal agent runtime lane inside Elegy.
- Prefer `elegy-memory` directly when you need the dedicated memory surface.
- Treat `src/Elegy-memory` as a thin wrapper and packaging surface, not as the implementation center.
- Treat `elegy` memory commands as the general/compatibility path only.

## Local store semantics

- Default root: `.elegy-local-memory` when `--root` is omitted.
- Local import accepts governed `summary-only-session-context-envelope` artifacts only.
- The local layout keeps `artifacts/`, `state/`, `state/write.lock`, and `exports/`.
- Durable local state is derived by scanning artifact files; `state/catalog.json` is not persisted local state.
- Default active-only views hide superseded and tombstoned records unless `--include-superseded` or `--include-tombstoned` is passed.
- Local writes are single-writer; concurrent writers are rejected.
- Listing and reporting follow deterministic ordering from the current local store surface.

## Retention and removal semantics

- Use `supersede` when a newer local artifact copy replaces another for local lineage purposes.
- Use `tombstone` when a local artifact copy should be withdrawn from normal local visibility and carry an explicit removal reason.
- Both actions remain local and non-authoritative. They do not decide what `SAASTools` treats as current, approved, fresh, valid, or promotable.
