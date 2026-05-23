# Memory Model

## What is a Memory?

A memory is a distilled, atomic record extracted from interaction context. It is not a raw transcript. In code, the core `Memory` struct stores:

- **Content** and optional **Summary**
- **Scope** (`Session`, `Workspace`, `User`, `Agent`)
- **Type** (`Fact`, `Preference`, `Decision`, `Procedure`, `Observation`)
- **Provenance** (`UserStated`, `AgentObserved`, `Consolidated`, `Imported`, `AgentInferred`)
- **Importance Score** and **Reliability Score**
- **Sensitivity** and **State** (`Active`, `Dormant`, `Deleted`)
- **Tags**, optional **status**, and free-form **custom_metadata**
- **Access / corroboration counters**
- **Embedding freshness**
- **Timestamps** plus optional tenant / user / agent ids

## Scopes

| Scope | Current Role | Current Storage Reality |
|-------|--------------|-------------------------|
| **Session** | Ephemeral task-local memory | Variant exists in the model and store APIs |
| **Workspace** | Project knowledge and decisions | Current CLI default |
| **User** | Cross-workspace user preferences and traits | Variant exists in the model and store APIs |
| **Agent** | Procedural knowledge about what works | Variant exists in the model and store APIs |

All four scopes exist in the type system and SQLite schema. The current implementation still binds each `SqliteMemoryStore` instance to one explicit **write** scope, but **retrieval visibility now cascades upward**:

- `session` searches / duplicate checks can see `session + workspace + user + agent`
- `workspace` can see `workspace + user + agent`
- `user` can see `user + agent`
- `agent` can see `agent`

`list` remains inventory-only for the exact requested scope.

## Scope Promotion

Automatic promotion is now implemented as a lightweight SQLite-backed pass:

- `session -> workspace` after access in **3 distinct session ids**
- any non-top scope with `corroboration_count >= 2` promotes **one level up**
- any non-top scope with `importance × retention >= 0.4` after **7+ days** promotes **one level up**
- manual CLI override can promote directly to any broader scope

Promotions never move downward. Each promotion records:

- a `memory_versions` row with `changed_by = system:promotion` (or the CLI actor)
- a `memory_promotions` provenance row with `from_scope`, `to_scope`, reason, and timestamp

## Memory Types

| Type | Description |
|------|-------------|
| **Fact** | Objective or verifiable information |
| **Preference** | Subjective preference |
| **Decision** | Intentional choice captured at a point in time |
| **Procedure** | How-to knowledge |
| **Observation** | Agent observation or inference candidate |

## Provenance Hierarchy

`ProvenanceLevel::base_reliability()` seeds the stored reliability score:

```
UserStated    → 1.0
AgentObserved → 0.8
Consolidated  → 0.7
Imported      → 0.6
AgentInferred → 0.5
```

## Confidence and Priority

The model keeps **importance** and **reliability** as separate values.

- **Importance** is assigned at extraction time.
- **Reliability** is seeded from provenance when the memory is created.
- Recording a contradiction lowers the less-trusted side by `0.3`.

The struct also carries `corroboration_count`, and corroboration is now operational:

- `SqliteMemoryStore::corroborate()` increments `corroboration_count`
- reliability increases by `+0.05` per corroboration
- the bonus is capped at `base_reliability + 0.2`
- the corroborating relationship is also recorded as a `corroborates` memory link

Priority used in retrieval is:

```
priority = importance × reliability
```

## Retrieval Scoring

Search uses a hybrid similarity signal plus recency/access/priority scoring:

```
score_final =
    α × similarity
  + β × recency
  + γ × ln(access_count + 1)
  + δ × (similarity × importance × reliability)
```

Default weights remain:

- `α = 0.40`
- `β = 0.25`
- `γ = 0.15`
- `δ = 0.20`

Vector and keyword similarity are blended before this score, with vector similarity intentionally dominant in the current store.

### Feedback-Driven Weight Learning

Retrieval feedback now closes the loop into live ranking instead of acting as a report-only diagnostic.

- `feedback` stores a `retrieval_feedback` row and immediately recomputes the effective scoring weights
- the recomputed weights are written back into the live `scope_config` keys used by search:
  - `similarity_weight`
  - `recency_weight`
  - `access_weight`
  - `priority_weight`
- `search()` reloads `scope_config` on each query, so newly learned weights affect the next retrieval without any extra migration or restart step

Current learner behavior:

- waits for at least **12** total feedback rows
- also requires at least **3 relevant** and **3 irrelevant** judgments
- derives four signals from the feedback corpus:
  - query-to-memory lexical similarity proxy
  - recency at feedback time
  - access-frequency signal
  - similarity-gated priority (`similarity × importance × reliability`)
- measures how strongly each signal separates relevant from irrelevant outcomes
- blends the learned profile back toward the defaults until enough balanced evidence exists, so weights move gradually instead of thrashing on small samples

The `weights` CLI now reports whether the store is still using defaults or has switched into learned mode, along with sample counts, confidence, and the current effective live values.

### Context Window Budget

Prompt injection still uses remaining-context budgeting:

```
available_for_memory = model_max_tokens - already_used_tokens - response_reserve
max_memory_tokens = available_for_memory × memory_context_ratio
```

Defaults:

- `memory_context_ratio = 0.10`
- `response_reserve = 4096`

## Decay Model

The current code uses a scope-configured decay base (`decay_lambda_base`, default `0.10`) and then modulates it with implemented retention refinements:

- **adaptive decay** adjusts effective retention by observed activity rate via `adaptive_retention()`
- **type-specific decay** applies `type_decay_multiplier()` by memory type
  - `Procedure = 0.7×`
  - `Fact = 0.8×`
  - `Decision = 0.85×`
  - `Preference = 0.9×`
  - `Observation = 1.2×`

The configured base lambda still anchors the model, but decay is no longer fixed-only.

## Write-Time Salience Gate

`DefaultSalienceGate` is intentionally conservative. It prefers storing a new memory over collapsing distinct information too early.

### Step 1: Novelty / Similarity

The current default thresholds are:

- **Likely-duplicate warning floor:** `0.80`
- **Merge threshold:** `0.85`
- **Duplicate threshold constant:** `0.99`
- **Salience threshold:** `0.20`
- **Agent-inferred archive threshold:** `0.50`

Current behavior:

- **Similarity < 0.80** → accept as a new memory
- **0.80 ≤ similarity < 0.85** → accept as a new memory, but surface a `similar_to` warning
- **similarity ≥ 0.85** → enter the merge branch

The `0.99` duplicate threshold exists in `ScopeConfig`, but the current default gate does not aggressively auto-reject near-duplicates; it stays conservative and routes high-similarity cases through merge-or-contradiction handling first.

### Step 2: Contradiction Check Inside the Merge Branch

Before merging a high-similarity candidate, the gate now prefers an optional LLM verdict when configured:

- prompt result `AGREE` → merge
- prompt result `CONTRADICT: ...` → contradiction record
- prompt result `UNRELATED` → accept as a new memory
- provider failure / timeout / unusable response → visible warning plus fallback to the conservative heuristic path

Without an LLM provider, or after fallback, the gate runs conservative write-time contradiction heuristics:

- **technology / category swaps** for the same subject
- **numeric-value swaps** for the same subject

If the change looks additive or like a rephrasing, the candidate still merges. If it looks like a real conflict, the gate returns a contradiction decision instead of merging.

### Scope-Aware Duplicate Policy

The salience gate now evaluates near-duplicates across the store's full visible scope set.

- **higher-scope near-duplicate** → reject the write
- **same-scope near-duplicate** → merge as before
- **lower-scope near-duplicate** → return a merge decision plus a promotion target for the merged result

### Step 3: Archive Checks

- `importance_score < 0.20` → store as `Dormant`
- `provenance == AgentInferred` and `importance_score < 0.50` → store as `Dormant`

### Gate Output

```rust
enum GateDecision {
    Accept {
        similar_to: Option<MemoryId>,
        similarity: Option<f32>,
    },
    Archive,
    Merge {
        target_id: MemoryId,
        enriched_content: String,
        promote_to: Option<MemoryScope>,
    },
    Contradiction {
        conflicting_id: MemoryId,
        description: String,
    },
    Reject {
        reason: String,
    },
}
```

### Merge Semantics

When the gate decides to merge:

- at very high similarity (`>= 0.95`), the newer content replaces the old body
- if the candidate is clearly more detailed, it can also replace the old body
- otherwise the existing body is kept

`update_content()` versions the old text before replacement and marks embeddings stale for re-embedding.

For `nomic-embed-text`, Elegy now applies task-aware prefixes at embedding time:

- document / stored-memory embeddings use `search_document: <content>`
- search-query embeddings use `search_query: <query>`

This keeps the query/document spaces aligned for semantic retrieval. Rows embedded before this rule changed are not directly comparable with freshly generated vectors; mark them stale and re-embed (or rebuild the database) before relying on mixed old/new semantic rankings.

## Contradiction Journal

Contradiction auto-detection is now implemented at write time, and the same journal is also reused by LLM-backed consolidation when the model flags a contradictory pair instead of returning merged text.

Current workflow:

1. The new memory is stored separately
2. A contradiction record is created with both memory ids and a human-readable description
3. The less-trusted side may receive a `-0.3` reliability penalty
4. Operators list unresolved contradictions
5. They resolve each one with either:
   - **keep-one**: keep one memory active and make the losing memory `Dormant`
   - **keep-both**: mark the contradiction resolved without changing either state

There is no automatic contradiction resolution in the current codepath.

## User Correction Feedback Loop

`correct_memory()` is no longer just a versioning convenience. A user correction now runs back through the same gate-aware backend lane as new writes and records a durable correction-history row.

Current correction outcomes:

- **applied** — the corrected content replaces the memory in place
- **archived** — the correction is accepted, but the gate moves the corrected memory to `Dormant`
- **merged** — the corrected content is folded into another memory, and the corrected source memory becomes `Dormant`
- **contradiction** — the corrected content is applied in place and a contradiction record is journaled against the related memory

Each correction records:

- the previous and corrected content
- the actor and free-form reason
- the final `disposition`
- an optional `related_memory_id` for merge / contradiction / near-duplicate outcomes
- the correction timestamp

When correction changes content, embeddings are refreshed when possible. Until a corrected row is re-embedded, stale vectors are excluded from similarity search so retrieval does not keep ranking pre-correction embeddings.

## Poisoning Detection and Remediation

`detect-poisoning` now stays scoped to the store's configured scope for **all four heuristics** and loads its thresholds from internal `scope_config` keys instead of hardcoded constants.

Current checks:

- **frequency anomaly** — recent write volume versus scope-sized thresholds
- **trust mismatch** — imported / inferred active memories with unusually high importance
- **bulk overwrite** — many distinct active memories in the same scope updated in a short window
- **mass contradiction** — scoped active memories accumulating repeated unresolved contradictions

The CLI now prints operator-actionable poisoning records in both text and JSON output, including:

- stable per-alert `id`
- `detected_at` timestamp
- implicated `memory_ids`
- heuristic severity scores in the inclusive `0.0..=1.0` range
- remediation actions showing which rows were quarantined or skipped and why

When operators pass `detect-poisoning --quarantine` (the older `--remediate` spelling is still accepted as an alias), the store now applies a concrete containment step:

- only **low-trust active memories** implicated by the alerts are targeted
- those memories are moved to `Dormant`
- they are marked as quarantined through metadata (`poisoning_quarantined_at`, `poisoning_alert_types`, `poisoning_alert_ids`, `poisoning_remediation`) so the store is protected without hard-deleting evidence

Trusted user-stated memories are not auto-dormanted by this remediation pass.

## Cross-Agent Sharing Safety

`share-export` still filters by sensitivity and reliability, but `share-import` is now intentionally conservative on the receiving side.

Imported shared memories now:

- are evaluated through the same salience gate used for normal writes
- still run an exact-text duplicate sweep across visible scopes even when no embedding provider is configured
- **never auto-merge into an existing memory**
- land as `Dormant` review entries even when accepted as novel
- are escalated to a quarantined dormant entry when they look like a near-duplicate, merge candidate, or contradiction
- are skipped entirely when an exact duplicate already exists in a higher visible scope
- may record a contradiction journal entry when the imported content conflicts with an existing memory
- surface per-item import dispositions (`review`, `quarantine`, `skip`) plus reasons and any referenced canonical memory in CLI output

This keeps cross-agent content available for operator review without letting it poison the active canonical store by default.

## Memory Versioning

`update_content()` writes a `memory_versions` row before replacing content. This is used by normal edits and merge updates. The version row stores:

- previous content
- version number
- `changed_by`
- `change_reason`
- timestamp

## Consolidation

Tier 2 consolidation now has two runtime modes:

- **SimpleConsolidator** — embedding-only dedup with the configured merge threshold
- **LlmConsolidator** — same candidate selection, but each qualifying pair is sent to an LLM for a constrained merge decision

CLI behavior:

- `consolidate` without `--llm-provider` uses `SimpleConsolidator`
- `consolidate --llm-provider ...` uses `LlmConsolidator`
- `--consolidate-limit` caps qualifying pair processing, not raw row loading
- `--dry-run` reports planned merges / contradictions without mutating the store

LLM consolidation degrades explicitly:

- `CONTRADICTION: ...` creates a contradiction record and keeps both memories
- empty / garbled LLM responses fall back to the simple merge strategy
- provider failures / timeouts fall back to the simple merge strategy with a visible warning

## Forgetting Budget

Budget configuration exists and health reporting exposes `budget_usage_ratio`, and automatic enforcement is now implemented through `SqliteMemoryStore::enforce_budget()`:

- when active memories exceed `budget_active_max`, the lowest-scoring active rows transition to `Dormant`
- when storage exceeds the cap, the lowest-scoring dormant rows are hard-deleted
- the CLI `budget` command surfaces the resulting dormant / deleted counts

## Memory States

Current state transitions are simpler than the long-term model:

```
Active  → Dormant  (write-time archive, manual make_dormant, keep-one contradiction resolution)
Dormant → Active   (manual reactivate)
Active  → Deleted  (hard_delete / purge)
Dormant → Deleted  (hard_delete / purge)
```

## Future Work Still Outside Current Code

- knowledge-graph-native storage and retrieval beyond today's link graph + BFS traversal
- PostgreSQL backend / multi-tenant runtime hardening beyond the current SQLite implementation
