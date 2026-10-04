---
title: Memory migration framework
status: active
owner: Elegy Memory
doc_kind: guide
---

# Memory migration framework

[Storage schema](storage-schema.md) describes the data. The executable migration
contract is `Migration`, `MigrationCapability`, `run_migrations` and the concrete
implementations in [schema.rs](../../src/storage/schema.rs). Regression examples
live in [schema/tests.rs](../../src/storage/schema/tests.rs). Use symbol names
rather than historical line numbers to find them.

## Initialization and preservation

`init_database` opens a transaction, creates missing schema/config entries,
runs pending named migrations, verifies the schema version and commits.
Failures propagate out of that transaction. Source content and protected
metadata in `memories` must be preserved; recalculable configuration and
embeddings have separate update paths. Re-embedding may update the derived
`embedding_stale` flag, so "no field in memories ever changes" is not the contract.

The runner installs SQLite triggers for `PROTECTED_MEMORY_COLUMNS` and blocks
`DELETE` on `memories` while pending migrations execute. Each migration runs
`verify` before a successful record is written to `migration_runs`; successful
completion removes those triggers. `capabilities()` declares intended changes.
It is not a general SQL permission sandbox: the runner does not inspect every
statement or enforce a per-table capability allowlist.

`run_migrations` accepts a connection and relies on the **caller** to provide
the transaction; it does not start one itself. Both initialization and the CLI
re-embedding path own that transaction. New callers must preserve this boundary.
A migration's name is its idempotency key: a recorded name is skipped.

## Implemented migrations

| Type | Intent | Implementation to inspect |
| --- | --- | --- |
| `SchemaAdditiveMigration` | Add missing columns on derived embedding/correction tables | `ensure_memory_embeddings_columns`, `ensure_memory_corrections_columns` |
| `ScopeConfigSemanticMigration` | Re-clamp derived retrieval weights and record the scoring version | `migrate_retrieval_scoring_config`, `CURRENT_RETRIEVAL_SCORING_VERSION` |
| `ReembedMigration` | Generate vectors in staging, verify coverage, then cut over | `run_staging`, `verify_staging`, `run_cutover` |

`initialize_scope_config` inserts missing defaults and handles selected legacy
thresholds; this bootstrap is distinct from the named scoring migration.
The current scoring migration preserves customized values below its ceilings.
Rescaling/resetting learned weights and a general derived-index rebuild are
not implemented migration kinds. `MigrationCapability` currently has only
`ScopeConfigWrite`, `SchemaAdditive` and `Reembed`.

## Re-embedding and cutover

The CLI `reembed` command creates a `ReembedMigration` and directly calls
`run` and `verify` inside its own transaction in [cli.rs](../../src/cli.rs).
It does not use `run_migrations`, its protective triggers or its name-based
ledger: re-embedding can be repeated. Inspect this caller and its tests for
the concrete scope/preservation behavior before adding another caller.

- A provider health check precedes staging.
- Staging records a vector and source-content hash; failed rows enter
  `reembed_pending_retry` rather than losing their source memories.
- Profile changes and incomplete staging cause staging to be rebuilt;
  orphan staging/retry entries are removed.
- Verification requires coverage by staged or retry entries before cutover.
- Cutover compares the current source-content hash with the staged hash.
  Matching entries update derived vectors; mismatches remain stale.
- The transaction supplies atomicity. Keep provider failure, rollback,
  source-row preservation and scope isolation covered when changing this path.

The fresh vector-table dimensions and legacy fallback behavior are described
in [storage schema](storage-schema.md). Configuring a different provider does
not by itself migrate that table's shape.

## Add or change a migration

1. Identify the source/derived data boundary and the smallest mutation needed.
   Read the existing migration and its verification before extending it.
2. Use a new unique name when a new migration must execute on databases that
   already recorded the old one. Version constants alone do not change the
   runner's name-based skip rule.
3. Implement the intended capability declaration, `run`, and `verify`. Register
   initialization migrations in `init_database`; keep explicit re-embedding
   under its provider-configured caller.
4. Add focused cases for an existing database, a repeated run, protected source
   rows and rollback. For weight changes also preserve values below the ceiling;
   for re-embedding cover failure/retry, hash mismatch and scope filtering.
5. Run from the repository root:

   ```text
   cargo test --locked -p elegy-memory --lib storage::schema::tests
   cargo test --locked -p elegy-memory --test cli
   cargo test --locked -p elegy-memory
   ```

Useful existing tests include
`init_database_preserves_memory_rows_while_migrating_retrieval_weights`,
`retrieval_scoring_migration_rolls_back_atomically` and
`scope_config_semantic_migration_idempotent_on_rerun`.
Qualification fingerprints change with source/test changes; rebuild before
running the [qualification workflow](../qualification.md).
