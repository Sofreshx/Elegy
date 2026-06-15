# Elegy-memory V1

## Purpose

`elegy-memory` is the shipped local memory surface in this repo, but the current implementation should be described as an MVP or preview CLI rather than as the older planned artifact-management flow.

`rust/crates/elegy-memory` owns the dedicated `elegy-memory` binary for that surface. `rust/crates/elegy-cli` keeps only a temporary compatibility bridge for legacy memory commands.

Alongside the existing `elegy` CLI surface, this in-repo `elegy-memory` surface is one of the current shipped operator surfaces. The contributor-navigation overlay under `src/Elegy-memory` is a pointer shell only, not a repo center, authority layer, implementation center, or separate release surface.

It currently covers a bounded local SQLite memory operator surface:

- adding memories through the salience gate
- keyword search over the current scope
- filtered listing and single-record inspection
- health reporting, JSON export, full purge, and contradiction listing
- preview re-embed handling that is visible in the CLI but not yet wired to a provider

It does not move runtime authority out of `SAASTools`.

## Authority chain

The authority chain is explicit and one-way:

1. `contracts/fixtures/skill.elegy-memory.json` is the governed skill definition source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-memory.json` is the governed discovery projection derived from that definition.
3. `.agents/skills/elegy-memory/SKILL.md` and `.github/skills/elegy-memory/SKILL.md` are rendered local outputs and are not authoritative.

Contributors should update the governed definition first, then the governed projection, and only then the rendered markdown output when the materialized skill text needs to change.

## Shipped CLI surface

The implemented CLI surface in `rust/crates/elegy-memory/src/cli.rs` is:

- `elegy-memory add <content>`
- `elegy-memory search <query>`
- `elegy-memory list`
- `elegy-memory inspect <id>`
- `elegy-memory purge`
- `elegy-memory health`
- `elegy-memory export`
- `elegy-memory import`
- `elegy-memory reembed`
- `elegy-memory contradictions`

The shared `elegy` CLI remains only as a temporary compatibility bridge for legacy memory command paths.

## Current behavior

The current MVP CLI behavior is intentionally narrow:

- the default database path is `~/.elegy/memory.db`
- the default scope is `workspace`
- `search` is keyword-only in the current CLI MVP
- `add` runs the salience gate and may accept, merge, or archive a memory as dormant
- `list` supports type and state filters with a simple limit
- `inspect` returns the current record plus version history
- `export` writes the current scope as JSON, either to stdout or to a file
- `import` restores JSON file or stdin inputs; full export-shape imports preserve exported memory state while simplified imports keep the current gate-first behavior unless `--force` is used
- `purge` deletes the configured database contents after confirmation unless `--yes` is passed
- `contradictions` lists unresolved contradiction records for the current scope
- `reembed` currently reports queued stale records and exits because provider-backed re-embedding is not wired yet

This surface is usable now for local preview workflows, but the docs should not describe it as a completed artifact import, supersede, or tombstone system.

## What stays in SAASTools

`SAASTools` retains authority for:

- currentness
- approval
- freshness policy
- retrieval ranking
- runtime validation
- promotion and invalidation decisions

`Elegy` does not become the runtime or policy owner for those decisions.

## Related

- [Skill Core V1](skill-core-v1.md)
- [Reusable Memory and Context Boundary](../migration/reusable-memory-boundary.md)
- [Distribution and downstream consumption](../distribution.md)
- [Research note: memory retention and removal guidance](../research/elegy-memory-retention-removal.md)
