# Elegy-memory V1

## Purpose

`elegy-memory` V1 is the shipped bounded memory surface in this repo.

`rust/crates/elegy-memory` owns the dedicated `elegy-memory` binary for that surface. `rust/crates/elegy-cli` keeps only a temporary compatibility bridge for legacy memory commands.

It covers two things only:

- governed reusable-memory artifacts rooted in `contracts/` and versioned through `governance/`
- the local non-authoritative CLI and store behavior for inspecting, validating, importing, listing, showing, exporting, superseding, and tombstoning governed summary-only session-context artifacts

It does not move runtime authority out of `SAASTools`.

## Authority chain

The authority chain is explicit and one-way:

1. `contracts/fixtures/skill-definition.elegy-memory.json` is the governed skill definition source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-memory.json` is the governed discovery projection derived from that definition.
3. `.github/skills/elegy-memory/SKILL.md` is a rendered local output for contributor routing and is not authoritative.

Contributors should update the governed definition first, then the governed projection, and only then the rendered markdown output when the materialized skill text needs to change.

## Shipped local surface

The shipped local surface is the `elegy-memory` CLI behavior around summary-only session-context artifacts and the local artifact store.

Inspection and validation:

- `elegy-memory inspect`
- `elegy-memory validate --input <path>`

Local artifact management:

- `elegy-memory init`
- `elegy-memory import`
- `elegy-memory list`
- `elegy-memory show`
- `elegy-memory export`
- `elegy-memory supersede`
- `elegy-memory tombstone`

The shared `elegy` CLI remains only as a temporary compatibility bridge for those legacy memory command paths.

Current guarantees of that local surface:

- import accepts governed `summary-only-session-context-envelope` artifacts only
- omitted `--root` defaults to `.elegy-local-memory`
- the local layout keeps `artifacts/`, `state/`, `state/write.lock`, and `exports/`
- durable local state is derived by scanning artifact files rather than persisting `state/catalog.json`
- active-only visibility is the default, so superseded and tombstoned records stay hidden unless explicitly requested
- local writes are single-writer
- local list and reporting order is deterministic

## Retention and removal semantics

The shipped semantics are intentionally local and bounded:

- `supersede` records local lineage that one artifact copy has been replaced by another local artifact copy
- `tombstone` records local withdrawal plus a local reason for that withdrawal

Neither action decides runtime currentness, approval, freshness, retrieval ranking, runtime validity, or promotion.

Those remain host-owned in `SAASTools`.

## What stays in SAASTools

`SAASTools` retains authority for:

- currentness
- approval
- freshness policy
- retrieval ranking
- runtime validation
- promotion and invalidation decisions

`Elegy` may describe local lineage and local artifact state. It does not become the runtime or policy owner for those decisions.

## Related

- [Skill Core V1](skill-core-v1.md)
- [Reusable Memory and Context Boundary](../migration/reusable-memory-boundary.md)
- [Distribution and downstream consumption](../distribution.md)
- [Research note: memory retention and removal guidance](../research/elegy-memory-retention-removal.md)