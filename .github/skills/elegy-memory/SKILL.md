---
name: elegy-memory
description: "Repo-local non-authoritative contributor-routing file for Elegy's implemented in-repo memory MVP CLI surface. Use for local memory add, search, list, inspect, health, export, purge, contradiction review, and preview reembed flows."
---

# Elegy Memory

This file is a repo-local, non-authoritative contributor-routing output.

The authority chain is one-way:

1. `contracts/fixtures/skill-definition.elegy-memory.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-memory.json` is the governed discovery projection derived from that definition.
3. `.github/skills/elegy-memory/SKILL.md` is a repo-local contributor-routing file only.

## When to use

- Prefer the dedicated `elegy-memory` binary for the implemented in-repo memory MVP surface.
- Add a memory with `elegy-memory add <content>`.
- Search the current scope with `elegy-memory search <query>`.
- Review stored memories with `elegy-memory list`, `elegy-memory inspect <id>`, `elegy-memory health`, and `elegy-memory contradictions`.
- Export the current scope as JSON with `elegy-memory export`.
- Clear a disposable or test database with `elegy-memory purge --yes`.
- Treat `elegy-memory reembed` as a preview command surface only; provider-backed re-embedding is not wired in the MVP CLI yet.
- Treat `elegy` memory commands as a temporary legacy compatibility bridge, not the preferred path.

## Do not use

- Do not treat this skill as authority for currentness, approval, freshness, retrieval ranking, runtime validation, or promotion.
- Do not describe the current CLI as the older `init` / `import` / `show` / `supersede` / `tombstone` artifact-management flow.
- Do not use this skill to describe raw transcript storage, broad host-owned persistence behavior, or a fully wired embedding pipeline.

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

- Default database path: `~/.elegy/memory.db` when `--db` is omitted.
- Default scope: `workspace`.
- `add` runs the salience gate and may accept, merge, or archive a memory as dormant.
- `search` is keyword-only in the MVP CLI and can optionally include dormant records.
- `export` writes JSON to stdout by default or to a file when `--output` is supplied.
- `purge` prompts for confirmation unless `--yes` is passed.
- `reembed` currently reports the queued stale-memory count and exits because provider wiring is not implemented yet.

This surface remains bounded and non-authoritative: it is a local memory operator CLI, not the owner of downstream runtime policy.