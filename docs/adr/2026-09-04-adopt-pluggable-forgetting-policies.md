---
title: Adopt pluggable forgetting policies
status: accepted
date: 2026-09-04
owner: Elegy Memory
---

# Adopt pluggable forgetting policies

## Context

`SqliteMemoryStore::enforce_budget()` demotes active memories to dormant when
a scope exceeds `budget_active_max`, and hard-deletes dormant memories when
the scope exceeds `storage_cap_mb`. Both phases ranked eviction candidates
with a single hardcoded formula, `importance_score * reliability_score`, with
no way to select a different ranking and zero test coverage pinning the
behavior. `docs/specs/eval-harness-v1/spec.md` scopes a "MaRS-inspired local
forgetting-policy suite" (arXiv 2512.12856) as Phase C of the eval harness
work; this ADR covers the feature half of that phase — the eval-scenario
half that scores retention quality across policies lands separately.

Investigation surfaced two facts that shaped this decision:

- `decay::adaptive_retention` — a finished, 11-test-covered implementation of
  MaRS-style priority decay (type, store activity, importance, and access
  frequency) — existed with no production caller.
- `enforce_budget`'s Phase 2 storage-cap loop re-checked `pragma_page_count`
  after each deletion to decide when to stop. SQLite does not shrink
  `page_count` on `DELETE` without `VACUUM`, so that check never observed
  progress: once a scope was over its storage cap, deletion ran to the end of
  the dormant set instead of stopping at the target. This was found and fixed
  as part of this change; see Decision below.

## Decision

Eviction ranking is now pluggable via a `ForgettingPolicy` trait
(`plugins/memory/src/traits.rs`), a sync (non-`async_trait`) strategy trait —
unlike `SalienceGate`/`MemoryConsolidator`/`EmbeddingProvider`, a policy is a
pure function over an already-loaded `Memory` with no I/O, following the
precedent of `MemoryObservability`. Five policies ship in
`plugins/memory/src/forgetting.rs`:

| Policy | Score | Notes |
|---|---|---|
| `ImportanceReliability` | `importance_score * reliability_score` | Default — reproduces prior behavior exactly |
| `Fifo` | `created_at` | Oldest evicted first |
| `Lru` | `last_accessed_at.unwrap_or(updated_at)` | Matches `decay`'s own reference-time convention |
| `PriorityDecay` | `decay::adaptive_retention(...)` | Direct reuse of the previously-unused function |
| `RandomDrop` | stable hash of `memory.id` (SHA-256, not RNG) | Deterministic and reproducible across runs |

Policy selection reads a new `forgetting_policy` scope-config key (default
`importance-reliability`), following the same string-keyed pattern as
`reembed_profile_id` and `schema_version`; an unrecognized value fails loudly
(`StoreError::Serialization`) rather than silently falling back. The CLI's
`budget --policy <name>` flag overrides the configured default for that
invocation only via `enforce_budget_with_policy`.

The budget itself stays count-based (`budget_active_max`) plus a byte cap
(`storage_cap_mb`); only the ranking within each phase became pluggable. MaRS
budgets in per-memory token cost (`Σwᵢ ≤ B`), which `Memory` has no field for
today — introducing it would require a schema change unrelated to ranking.

Sensitivity (`SensitivityLevel`) was deliberately kept out of retention
scoring in every policy above. In the current codebase it is documented "for
privacy and purge handling" and its only behavioral use is filtering
`export_for_sharing`; it appears in no ranking or eviction path today, and
importance is a separate axis by design (`memory-model.md`). Making it
load-bearing for data destruction is a decision on its own, deferred to a
future Hybrid policy where sensitivity-weighted retention is the point.

**Phase 2 bug fix.** The storage-cap loop now measures live bytes as
`(page_count - freelist_count) * page_size` (via `pragma_freelist_count`,
which does grow as rows are deleted) instead of raw `page_count * page_size`,
applied consistently to both the entry condition and the per-iteration break
check. `MemoryObservability::health_report`'s `total_storage_bytes`
deliberately keeps reporting raw file size — the eval harness's
`storage_efficiency_bytes_per_memory` gate is calibrated against exactly
that — so the two measurements now intentionally diverge for different
purposes: health reports true file size, budget enforcement measures
reclaimable-adjusted live size.

## Consequences

1. Choosing a non-default policy (or `PriorityDecay` in particular) changes
   which memories survive budget enforcement; the default policy's behavior
   is unchanged from before this ADR.
2. The Phase 2 fix changes real behavior: a scope that exceeds
   `storage_cap_mb` no longer has its entire dormant set deleted once over
   cap — deletion now stops once live bytes drop under the cap, which is a
   deliberate, tested change from the prior (buggy) behavior.
3. `enforce_budget` (previously untested) now has characterization and
   policy-selection test coverage in `plugins/memory/src/storage/sqlite_store.rs`.

## Not Yet Implemented

The following are explicitly out of scope for this pass, recorded here
rather than silently dropped:

- **Reflection-Summary policy** — needs the consolidator and an LLM call;
  deferred until that integration is designed.
- **Hybrid policy** — needs both cost-weighted budgeting (below) and the
  sensitivity decision this ADR deferred.
- **Cost-weighted budgeting** (`Σwᵢ ≤ B` per MaRS) — would require a schema
  change (`Memory` has no cost/weight field) and is the natural prerequisite
  for Hybrid.
- **Sensitivity-weighted retention** — deliberately excluded from all five
  policies above; see Decision.
- **Dead per-scope budget fallbacks** — `DEFAULT_USER_BUDGET` (1000),
  `DEFAULT_AGENT_BUDGET` (200), and Session's implicit 0 are unreachable in
  practice: `initialize_scope_config` unconditionally `INSERT OR IGNORE`s
  `budget_active_max = "500"` into every database, so every scope gets 500
  regardless of these constants. Fixing that changes real retention behavior
  for three scopes and belongs in its own change, not bundled here.
- **General `config set` CLI surface** — there is no CLI command to persist a
  `scope_config` value; tests set values via raw SQL. `budget --policy` is
  therefore a per-run override only, and the persisted `forgetting_policy`
  default is effectively read-only from the CLI until a config-setter exists.
- **Comparative eval scenario suite** — running a corpus through each policy
  and scoring retention quality (the other half of eval-harness-v1 Phase C)
  lands separately from this feature work.
