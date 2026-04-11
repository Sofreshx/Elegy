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

The struct also carries `corroboration_count`, but automatic corroboration bonuses and broader staleness penalties are not implemented yet.

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

The current code uses a fixed scope-configured decay base (`decay_lambda_base`, default `0.10`) for recency scoring and retention calculations. Adaptive decay, type-specific decay tuning, and high-importance protection are still future refinements, not current behavior.

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

Budget configuration exists and health reporting exposes `budget_usage_ratio`, but automatic budget-driven dormancy and hard-delete policies are not implemented yet.

## Memory States

Current state transitions are simpler than the long-term model:

```
Active  → Dormant  (write-time archive, manual make_dormant, keep-one contradiction resolution)
Dormant → Active   (manual reactivate)
Active  → Deleted  (hard_delete / purge)
Dormant → Deleted  (hard_delete / purge)
```

## Future Work Still Outside Current Code

- corroboration bonuses
- adaptive/type-specific decay
- automatic budget enforcement
- user-correction feedback loops

