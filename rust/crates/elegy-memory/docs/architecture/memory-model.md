# Memory Model

## What is a Memory?

A memory is a distilled, atomic piece of information extracted from an agent-user interaction. It is NOT a raw transcript. It is a summary, a fact, a decision, a preference, or a procedure — something the agent should remember.

Every memory has:
- **Content** — the textual information (e.g., "User prefers Rust over Python")
- **Summary** — optional shorter form for context injection
- **Embedding** — vector representation for semantic search (can be stale)
- **Scope** — where it lives (Session, Workspace, User, Agent)
- **Type** — what kind of information it is (Fact, Preference, Decision, Procedure, Observation)
- **Provenance** — where it came from (UserStated, AgentObserved, AgentInferred, Consolidated, Imported)
- **Importance Score** — LLM-assigned salience (0.0 → 1.0)
- **Reliability Score** — system-computed trust (0.0 → 1.0)
- **Sensitivity** — data classification (Low, Medium, High, Critical)
- **State** — lifecycle state (Active, Dormant, Deleted)
- **Metadata** — extensible tags, status, custom key-value pairs
- **Timestamps** — created_at, updated_at, last_accessed_at
- **Access Count** — how often this memory has been retrieved

## Scopes

| Scope | Lifetime | Storage | Purpose |
|-------|----------|---------|---------|
| **Session** | Current session only | JSON in memory / temp file | Working context, immediate task state |
| **Workspace** | As long as workspace exists | SQLite .db per workspace | Project-specific knowledge, decisions, tasks |
| **User** | Indefinite | Global SQLite .db | Cross-workspace preferences, traits, patterns |
| **Agent** | Indefinite | Global SQLite .db (separate namespace) | Procedural knowledge — what tools work, what approaches failed |

Scopes are physically isolated. Each scope has its own storage. No implicit cross-scope queries.

### Scope Promotion (v1)

A memory that recurs in 3+ sessions within a workspace is automatically promoted to workspace scope. A pattern that recurs in 3+ workspaces is promoted to user scope. Promotion creates a new memory in the target scope with provenance `Consolidated` and links back to the source memories.

## Memory Types

| Type | Description | Decay Behavior |
|------|-------------|----------------|
| **Fact** | Objective, verifiable ("the project uses Rust") | No temporal decay. Only invalidated by contradiction. |
| **Preference** | Subjective user preference ("I prefer short responses") | Slow decay. Decays if not corroborated across sessions. |
| **Decision** | Choice made at a point in time ("we chose SQLite") | No temporal decay. Can be superseded by newer decisions. |
| **Procedure** | How to do something ("to deploy, run X then Y") | Very slow decay. Invalidated if environment changes. |
| **Observation** | Agent's inference ("user seems to prefer pragmatic approaches") | Normal decay. Requires corroboration to strengthen. |

## Provenance Hierarchy

Provenance determines the base reliability score. Higher = more trusted.

```
UserStated    → 1.0  (user explicitly said it)
AgentObserved → 0.8  (agent witnessed it happen)
Consolidated  → 0.7  (produced by consolidation/sleep-time)
Imported      → 0.6  (imported from external source)
AgentInferred → 0.5  (agent deduced it)
```

## Confidence Score (Bidirectional)

Each memory carries two independent scores:

### Importance Score (LLM-assigned, 0.0 → 1.0)
The agent evaluates salience at extraction time.
- Critical architecture decision → 0.9
- Cosmetic preference → 0.3
- Passing mention → 0.1

### Reliability Score (system-computed, 0.0 → 1.0)
The system computes trustworthiness:
- **Base:** provenance level (see hierarchy above)
- **Corroboration bonus:** +0.1 per independent corroboration (same fact from different sessions), cap +0.3
- **Contradiction penalty:** -0.3 if contradicted by user, -0.1 if contradicted by another unresolved memory
- **Staleness penalty:** slight decay if never accessed despite relevant retrieval opportunities

### Priority Score
```
priority = importance × reliability
```
A memory with importance=0.9 but reliability=0.3 (uncorroborated inference) → priority 0.27, filtered from normal retrieval. A memory with importance=0.4 but reliability=1.0 (user-stated) → priority 0.40, accessible.

## Retrieval Scoring

When searching for relevant memories, the system combines four signals:

```
score_final = α × similarity + β × recency + γ × log(access_count + 1) + δ × (importance × reliability)
```

Where:
- `similarity` = cosine similarity between query embedding and memory embedding (from sqlite-vec)
- `recency` = `e^(-λ × days_since_last_access)` (Ebbinghaus-inspired decay)
- `access_count` = number of times this memory has been retrieved
- `importance × reliability` = the confidence score
- α, β, γ, δ = configurable weights (defaults: 0.4, 0.25, 0.15, 0.2)

### Context Window Budget

The number of memories injected into an LLM prompt is controlled by the *remaining* context, not the total:

    available_for_memory = model_max_tokens - already_used_tokens - response_reserve
    max_memory_tokens = available_for_memory × memory_context_ratio

Where:
- `model_max_tokens` is the model's total context window (provided by caller)
- `already_used_tokens` is the space already consumed by system prompt, conversation history, and other context (provided by caller, optional — defaults to 0)
- `response_reserve` is space reserved for the model's response (default: 4096 tokens)
- `memory_context_ratio` defaults to 0.10 (10% of remaining context)

This is model-agnostic — the caller provides `model_max_tokens` and optionally `already_used_tokens`.

## Decay Model

Inspired by Ebbinghaus forgetting curve, adapted for agent memory:

```
retention = importance × e^(-λ_adjusted × days_since_last_access) × (1 + 0.2 × access_count)
```

### Adaptive Decay Rate
```
λ_adjusted = λ_base × (average_sessions_per_week / reference_sessions_per_week)
```
Where `reference_sessions_per_week = 3.0` (configurable). A daily user (7 sessions/week) has faster decay. A monthly user (~1/week) has slower decay. This prevents punishing infrequent users.

### Type-Modulated Decay
- Facts and Decisions: λ_base = 0 (no decay)
- Procedures: λ_base = 0.02 (very slow)
- Preferences: λ_base = 0.05 (slow)
- Observations: λ_base = 0.10 (normal)

### High-Importance Decay Protection

Regardless of memory type, any memory with `importance_score > 0.7` has its effective λ_base halved:

    λ_effective = λ_base × (if importance > 0.7 then 0.5 else 1.0)

This prevents the edge case where a highly important Observation (e.g., "user's critical project deadline is March 30") decays too quickly despite its high salience. Facts and Decisions already have λ_base = 0, so this rule only affects Preferences, Procedures, and Observations with high importance.

## Write-Time Salience Gate

Every memory write passes through a 3-step gate:

### Step 1: Novelty Check (Semantic Deduplication)
Compute cosine similarity with existing active memories. If similarity > 0.92 with an existing memory:
- Do NOT create a new memory
- Update the existing memory: increment access_count, update timestamp, enrich content if new info is present
- Mark embedding as stale if content changed

### Step 2: Salience Check
If the LLM-assigned importance_score < 0.2:
- Store directly as Dormant (cold storage), not Active
- This prevents low-value noise from entering the active retrieval pool

### Step 3: Provenance Check
If provenance is `AgentInferred` AND importance < 0.5:
- Store as Dormant by default
- Only user-stated or high-confidence inferences pass to Active

### Gate Output
```
enum GateDecision {
    Accept,             // Store as Active
    Archive,            // Store as Dormant
    Merge(MemoryId),    // Merge with existing memory
    Reject,             // Do not store (e.g., exact duplicate)
}
```

### Gate Safety Rules

The gate is the most critical component. A bad gate decision is harder to reverse than a bad retrieval decision. Therefore:

1. **The gate NEVER hard-rejects useful information.** `Reject` is ONLY for exact duplicates (cosine > 0.99) or empty/malformed content. When in doubt, the gate MUST choose `Archive` (dormant), never `Reject`.
2. **Archive is always preferred over Reject.** A dormant memory costs minimal storage but retains the option value of reactivation. A rejected memory is gone forever.
3. **Merge is reversible.** Every merge creates a version entry in `memory_versions`, preserving the pre-merge content. A bad merge can be rolled back.
4. **The gate is conservative by default.** If the novelty check is borderline (cosine between 0.85 and 0.92), the gate should `Accept` as a new memory rather than `Merge`. False negatives (storing a near-duplicate) are cheap; false positives (merging distinct information) lose data.

These rules ensure that the write-time gate provides quality control without becoming a bottleneck or a source of data loss.

## Contradiction Journal

When the system detects two memories with contradictory content:
1. Both memories are flagged
2. An entry is created in the contradiction journal with: memory_a_id, memory_b_id, detected_at, description, resolution_status
3. The reliability_score of the less trusted memory (by provenance) is reduced by 0.3
4. Resolution can be: automatic (newer user-stated wins), or manual (user resolves)

Detection happens:
- At write time (new memory contradicts existing)
- During consolidation (sleep-time agent reviews and detects)

## Memory Versioning

When a memory is updated (content change, consolidation, contradiction resolution):
- The old content is saved in the `memory_versions` table
- A new version number is assigned
- `changed_by` records who/what made the change
- `change_reason` records why
- Embeddings of old versions are NOT stored (only text)
- The current version's embedding is marked stale for re-computation

## Forgetting Budget

Each scope has a configurable budget of active memories:
- Workspace: 500 (default)
- User: 1000 (default)
- Agent: 200 (default)
- Session: unlimited (ephemeral anyway)

When the budget is exceeded, memories with the lowest `retention` score transition to Dormant. Dormant memories are excluded from default retrieval but can be reactivated if a query has cosine > 0.95 with their embedding.

Hard delete only occurs when the SQLite file exceeds a configurable size cap (default: 100MB per workspace, 500MB for user, 50MB for agent).

## Memory States

```
Active → Dormant (budget exceeded or low retention)
Dormant → Active (high-similarity query reactivation or manual promotion)
Active → Deleted (hard purge at storage cap, or user purge request)
Dormant → Deleted (hard purge at storage cap)
```

## User Correction Feedback Loop (v1)

When an agent uses a memory and the user corrects it:
1. `reliability_score` of the used memory decreases
2. A new memory is created with the corrected info (provenance: UserStated, reliability: 1.0)
3. Contradiction journal entry is created
4. If the corrected memory was AgentInferred, the agent's inference accuracy for that topic is tracked (meta-learning)

