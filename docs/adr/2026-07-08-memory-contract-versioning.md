---
title: Memory contract versioning
status: accepted
date: 2026-07-08
owner: Elegy Memory
---

# Memory contract versioning

## Context

`elegy-memory` persists to a SQLite file, computes retrieval scores with a
formula that has already changed once in production, and exports a portable
`.elegy` / JSON archive format consumed outside the crate. None of the three
carried an explicit version before this decision, so an older database file,
an archive from a prior release, or a documented scoring formula could
silently drift from what the running engine actually does. This ADR fixes
that by giving each of the three a tracked version number and a bump rule.

An alternative considered was a single crate-wide version covering all three
concerns. Rejected: the three change independently — a scoring formula
change does not require a schema migration, and an archive format change
does not require either. Coupling them would force unrelated version bumps.

## Decision

The `elegy-memory` storage format, retrieval contract, and portable archive format carry explicit semantic versions. Three version dimensions are independently tracked.

## Version Dimensions

### 1. Storage schema (`schema_version`)

| Mechanism | Current implementation |
|---|---|
| `scope_config` key | `"schema_version"` — string, currently `"1"` (`CURRENT_SCHEMA_VERSION`, `plugins/memory/src/storage/schema.rs`) |
| `migration_runs` table | Tracks which named migrations have been applied |

**Bump rule**: MAJOR on any change that makes an older DB file unreadable (e.g., table drop, column removal, `vec0` dimension change). MINOR on additive changes (new table, new column with default).

### 2. Retrieval scoring (`retrieval_scoring_version`)

| Mechanism | Current implementation |
|---|---|
| `scope_config` key | `"retrieval_scoring_version"` — string, currently `"2"` (`CURRENT_RETRIEVAL_SCORING_VERSION`, `plugins/memory/src/storage/schema.rs`) |
| Migration re-clamp | Older versions are clamped to safe ceilings on init |

**Bump rule**: MAJOR on any change that changes the scoring formula (new term, removed term, weight redefinition). MINOR on ceiling/safety threshold changes.

### 3. Portable archive (`.elegy` / JSON export)

The `.elegy` export is `elegy_memory::types::ElegyArchive`, serialized with `#[serde(rename_all = "camelCase")]`. It includes a `formatVersion` field at the top level, currently written as `"1"`:

```json
{
  "formatVersion": "1",
  "exportedAt": "...",
  "scope": "workspace",
  "memories": [...],
  "links": [...],
  "versions": [...]
}
```

**Bump rule**: MAJOR = breaking change to export schema (field removal, type change). MINOR = additive field. PATCH = bugfix, documentation-only change.

## Semantic Versioning Contract

The workspace `version` (from `Cargo.toml`) follows these conventions for the memory engine:

| Version component | Meaning |
|---|---|
| MAJOR | Storage schema change requiring re-ingest OR retrieval scoring formula change OR `.elegy` MAJOR version change |
| MINOR | New capability (new table, new CLI command, new memory type, new export field) that doesn't break existing queries |
| PATCH | Bugfix, threshold recalibration, non-breaking improvement, dependency update |

## Not Yet Implemented

The following were part of the original design intent but do not exist in the codebase as of this writing. They are recorded here as the target shape for a future change, not as current behavior:

- A `PRAGMA user_version` mirror of `schema_version`, so external tools could check compatibility without querying `scope_config`.
- A `compatibility_check()` function on `MemoryStore` and a `CompatibilityReport` type:

  ```rust
  pub struct CompatibilityReport {
      pub db_compatible: bool,
      pub db_schema_version: u32,
      pub engine_schema_version: u32,
      pub retrieval_scoring_version: u32,
      pub requires_reembed: bool,  // true when embedding model/dim changed
      pub requires_reingest: bool, // true when schema MAJOR mismatch
  }
  ```

Today, compatibility is enforced only by `verify_schema_version()` failing closed with an error when a DB's stored `schema_version` does not match `CURRENT_SCHEMA_VERSION` — there is no separate query surface a caller can use to check compatibility without opening the store.

## Migration path

1. Existing DB files without `formatVersion` in `.elegy` exports predate this ADR and should be treated as unversioned.
2. Re-embed migration (`ReembedMigration`) bumps `schema_version` when the `vec0` dimension changes.
3. Scope config semantic migrations (`ScopeConfigSemantic`) bump `retrieval_scoring_version` and clamp older values to safe ceilings on init.

## Consequences

Contextual recall v1 adds a separate, disposable event journal with its own
SQLite `application_id` and `user_version`, as specified in
[Contextual recall v1](../specs/memory-contextual-recall-v1/spec.md).
It does not migrate the source memory database, change the legacy scoring
formula or extend `.elegy` archives. Unknown journal versions fail closed;
future journal evolution must version and migrate that journal independently.

1. `.elegy` files include self-describing version info, enabling cross-version import validation.
2. A DB whose stored `schema_version` does not match the running engine's `CURRENT_SCHEMA_VERSION` fails to open with a clear error, rather than silently misreading rows.
3. The scoring version lets a migration distinguish "older formula, needs re-clamping" from "current formula" without re-deriving history.
4. The `PRAGMA user_version` mirror and `compatibility_check()` API remain future work; a caller cannot currently check compatibility without attempting to open the store.
