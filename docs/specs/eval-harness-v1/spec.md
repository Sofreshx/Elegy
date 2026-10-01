---
title: Eval harness v1
status: active
owner: Elegy Memory
---

# Eval Harness v1

## Contract

### Resumable qualification (approved 2026-09-30)

`eval status`, `eval next`, and `eval history` expose an exact, deterministic
inventory of Memory claims. `eval run --claim ID` executes an allowlisted,
offline experiment and persists a receipt. Existing `eval run --ci` remains
the metric regression harness. `eval record --claim ID --observation FILE`
records externally performed host experiments with explicit reported provenance.
See the [implementation plan](../../plans/memory-qualification.md) for the current delivery checkpoints.

The initial claims are `recall.contract`, `forgetting.retention`,
`host.cross-session`, `host.agent-isolation`, and `recall.answer-quality`.
Each has a bounded promise, falsifier, protocol, acceptance criteria, and
next action. Host claims require actual client/session observations; answer
quality requires paired with/without-recall cases and predeclared scoring.
The first two execute against disposable synthetic stores; the other three
provide protocols and accept observations, never masquerading as automated proof.

The canonical inventory is the typed `Claim` table in
`plugins/memory/src/eval/qualification.rs`, exposed as
`memory-claim-inventory/v1`. Each protocol SHA-256 covers the inventory version
and complete serialized claim. Its named prerequisite checks must succeed
before a failed hypothesis check can count as a refutation. `status`/`next`
return exact check names, protocol steps, stale reasons, receipt paths, and
process argument arrays preserving `--project` and `--evidence-dir`.
These options belong to qualification subcommands; project defaults to cwd,
relative evidence directories resolve against project, and input observation
paths resolve against cwd. Source qualification requires the Elegy checkout.

Receipts are append-only JSON files under an explicit evidence directory
(default `plugins/memory/evidence/qualification` in `--project`). They include
claim/protocol identity, UTC timestamp, source and executable SHA-256, build
source fingerprint, Git revision and dirty flag, OS/architecture, duration,
checks, outcome and provenance. Inputs and evidence files are content-addressed;
raw memory content, prompts, transcripts, credentials and personal filesystem
paths are not automatically captured. Reports contain synthetic measurements
or operator-reviewed external observations only. External evidence files must
be regular files beneath the evidence directory and are verified on each read.
Receipts use create-new writes; historical receipts are never overwritten.
A create-new writer lock serializes appends. A same-directory atomic hard link
publishes each complete UUID receipt; atomic replacement updates `ledger.json`.
The ledger's monotonic sequence orders history and detects missing receipts.
A crash between those operations fails closed on the next inventory read.
Readers reject malformed evidence across the entire inventory; lock recovery
requires confirming the previous writer has stopped and preserving its files.
Missing, malformed, modified or incompatible receipts fail closed rather than
silently producing a successful or empty status. Digests detect accidental
drift; these local files are not a tamper-proof attestation system.

Outcomes are `satisfied`, `refuted`, and `inconclusive`; status also represents
`unverified` and `stale`. External status uses `reported-satisfied`,
`reported-refuted`, and `reported-inconclusive` plus `reviewRequired`.
A protocol/runtime failure is inconclusive; failed
acceptance checks refute only the stated bounded claim. External observations
are always labelled `external-observation`, including a reported success.
No outcome automatically promotes ecosystem readiness. Status compares the
current source, protocol, executable and environment to the last receipt;
`next` deterministically prioritizes refuted, stale, inconclusive and unverified
claims. Older conclusive outcomes remain visible in history. A source/binary
build mismatch prevents new automated qualification. Unrelated Git commits
alone do not invalidate unchanged source fingerprints.

The shared build/runtime fingerprint algorithm lives in
`plugins/memory/src/eval/fingerprint.rs`: sorted relative paths and exact file
bytes are length-prefixed into SHA-256 over its explicit `INPUTS` list and
source-like extensions. This includes Memory source, tests, fixtures, schemas,
integration, thresholds, manifests/lock, shared code and the relevant specs;
it excludes generated binaries and evidence. Source changes during a run make
that attempt inconclusive. Executable SHA-256 binds actual build options;
OS/architecture are compared separately. No compiler-to-artifact attestation
or multi-user tamper resistance is claimed.

External observations use the strict
`plugins/memory/schemas/qualification-observation.schema.json` contract.
The recorder context is separate from the tested subject's artifact/configuration
digests and host/version. At least two distinct non-personal session/client
pseudonyms, chronological UTC timestamps, exact named checks and reviewed evidence
digests are required. The runtime additionally validates digests, evidence paths,
finite metrics, unique paired-case IDs and protocol prerequisites. The CLI cannot
automatically recheck the external subject; it always exposes that limitation.
It derives paired answer improvement from normalized with/without scores rather
than accepting a caller's verdict. Files containing raw private material must
be reviewed/redacted before sharing; no transcript is loaded automatically.

Memory recall may carry a short pointer to a claim and its evidence directory;
the authoritative open-claim inventory does not depend on semantic search,
consolidation, learning, salience or forgetting. Scheduling and ownership stay
with the host/Planning. This slice adds no MCP execution tools or scheduler.

Acceptance for resumable qualification:

- Exact lookup and deterministic next-work selection survive a fresh CLI process.
  → verify: `cargo test -p elegy-memory --test qualification`.
- Receipt corruption, missing evidence, source/binary drift and changed protocols cannot retain a current successful status.
  → verify: qualification integration tests and fingerprint unit tests.
- Real recall boundaries and all five forgetting policies execute against isolated stores; a failed check and a runner error yield different outcomes.
  → verify: scenario unit tests and the two `eval run --claim` commands.
- External sessions and paired answer assessments retain their provenance and evidence; synthetic success cannot certify a host claim.
  → verify: external observation validation tests.
- Existing metric CLI behavior remains compatible, readiness artifacts remain unchanged, and documentation links resolve.
  → verify: `cargo test -p elegy-memory --test eval` and `elegy-documentation check --project . --json`.

An evaluation harness for regression-testing elegy-memory's write→store→retrieve pipeline. Anchored on a hand-authored embedded corpus plus a synthetic distractor corpus, with the three academic benchmarks reachable as advisory, non-gating extensions (see Corpus). Metrics gate production readiness of threshold changes, decay parameter changes, and scoring weight changes.

**Implementation status (Phase A, complete)**: the full CI gate exists and runs with no network access: `plugins/memory/src/eval/` (corpus schema, deterministic embeddings, metrics, gate thresholds, the synthetic distractor generator, and the runner), the `eval run` / `eval list-corpora` / `eval sweep-threshold` / `eval export-labels` CLI subcommand, `plugins/memory/fixtures/eval/*.json` corpora, `plugins/memory/tests/eval.rs`, `plugins/memory/eval-harness-v1.json`, and the `memory-eval` CI job. `cargo run -p elegy-memory -- eval run --ci` exits non-zero on a gate miss; `cargo test -p elegy-memory --test eval` requires no network and no external download.

**Implementation status (Phase C)**: the pluggable `ForgettingPolicy` trait and five policies (`ImportanceReliability`/default, `Fifo`, `Lru`, `PriorityDecay`, `RandomDrop`) are implemented — `plugins/memory/src/traits.rs`, `plugins/memory/src/forgetting.rs`, wired into `SqliteMemoryStore::enforce_budget()`, selectable via the `forgetting_policy` scope-config key or CLI `budget --policy`. See `docs/adr/2026-09-04-adopt-pluggable-forgetting-policies.md`. A bounded six-memory, two-slot comparative scenario is available through `eval run --claim forgetting.retention`; it records retained IDs and labelled precision/recall for every policy. The broader FiFA-style evaluation suite remains follow-up work; the local scenario does not claim benchmark parity.

**Remaining follow-up (Phase B and broader Phase C evaluation)**: fetching the LoCoMo/LongMemEval corpora, importing their formats, and expanding the bounded retention scenario into a MaRS-inspired scenario suite. See Corpus below for the limits of the current implementation.

Five deliberate deviations from the design below, each with a stated reason, are called out inline: the embedded-vs-fetched corpus split, the local FiFA/MaRS adaptation, `plugins/memory/src/eval/` replacing `test_harness/`, `plugins/memory/eval-harness-v1.json` replacing a repo-root file, and three gate thresholds calibrated to a measured baseline rather than the aspirational targets below.

## Corpus

Contextual recall has a complementary deterministic selection corpus at
`plugins/memory/fixtures/eval/contextual-recall-v1.json`, executed by
`cargo test -p elegy-memory --test contextual_recall`. It covers explicit and
indirect prompts, current-context duplication, topic changes, empty input and
negative queries. The same test target covers source immutability, scope and
owner restrictions, semantic retrieval, event feedback and opt-in learning.
This corpus supplements the existing `eval run --ci` gates; it does not claim
to measure improvements to model answers. With/without-recall answer evaluation
and installed-host latency remain separate qualification work before rollout.

| Source | Use | Reference | Status |
|---|---|---|---|
| Golden retrieval corpus | recall/precision/NDCG/hallucination | Hand-authored, `plugins/memory/fixtures/eval/golden-v1.json` | **Implemented, embedded, CI-gated** |
| Gate-decision corpus | Accept/Archive/Merge/Contradiction/Reject coverage | Hand-authored, `plugins/memory/fixtures/eval/gate-decisions-v1.json` | **Implemented, embedded, CI-gated** (`gate_accuracy_full`) |
| Synthetic distractors | Write-time gating accuracy at 1:1, 4:1, 8:1 ratios | Locally adapted from the Write-Time Gating paper (arXiv 2603.15994) §4 | **Implemented, generated at runtime, CI-gated at 8:1** |
| LoCoMo | Multi-session long-conversation recall | Hindsight paper (arXiv 2512.12818) | Not implemented (Phase B) — see deviation below |
| LongMemEval | Factual recall after distraction | Hindsight paper (arXiv 2512.12818) | Not implemented (Phase B) — see deviation below |
| FiFA (MaRS) | Forgetting policy quality | MaRS paper (arXiv 2512.12856) | Five policies and a bounded local retention scenario implemented; broader scenario suite deferred — see deviation below |

### Deviation: corpus acquisition (fetch upstream, sha256-pinned; no CI-storage mirror)

The original design assumed "CI storage + `--download-corpus`" — a pipeline that does not exist and cannot be built as specified:

- **LoCoMo** (`data/locomo10.json`, `snap-research/locomo` on GitHub) is released under **CC BY-NC 4.0**. Elegy is Apache-2.0, and `deny.toml`'s license allowlist is permissive-only. Mirroring LoCoMo into Elegy's own CI storage would be redistribution under a license this repo cannot grant. LoCoMo can only ever be an advisory, non-gating extension for this repo, fetched from its own upstream URL on demand, never vendored.
- **LongMemEval** (`xiaowu0162/longmemeval-cleaned` on Hugging Face, `longmemeval_oracle.json`) is MIT-licensed and redistributable, but its oracle split is ~15 MB — too large to vendor as a fixture.
- Both remain reachable via a future `eval fetch-corpus` (Phase B): pull from the original upstream URL into a gitignored cache, verified against a checked-in sha256 manifest. No such downloader exists yet; **Phase A ships no downloader and no import adapter for either format.**

### Deviation: FiFA/MaRS has no public artifact — reimplement locally, don't import (Phase C)

arXiv 2512.12856 ("Forgetful but Faithful") has no GitHub repository, no Hugging Face dataset, and no supplementary-material link as of this writing. Its FiFA benchmark is a generative-agent simulation, not a downloadable annotated retrieval corpus — there is nothing to import. Phase C reimplements the paper's six forgetting policies as local code rather than an imported benchmark, explicitly **not** claiming benchmark parity with the paper.

Five of the six policies are implemented as the **feature half** (complete): `ImportanceReliability` (default), `Fifo`, `Lru`, `PriorityDecay` (reusing `decay::adaptive_retention`), and `RandomDrop` (deterministic hash, not RNG) — `plugins/memory/src/forgetting.rs`. Reflection-Summary is deferred (needs the consolidator and an LLM call) and Hybrid is deferred (needs cost-weighted budgeting and a sensitivity-retention decision neither of which this pass makes); both are recorded in `docs/adr/2026-09-04-adopt-pluggable-forgetting-policies.md`.

The **eval-scenario half** now has one bounded qualification protocol in `plugins/memory/src/eval/scenarios.rs`: six labelled memories, a two-memory active budget, actual budget enforcement under all five policies, and recorded retention precision/recall. It supplements the unit and characterization tests in `plugins/memory/src/forgetting.rs` and `plugins/memory/src/storage/sqlite_store.rs`. Multiple scenario families and benchmark-scale gates remain deferred; this single fixture establishes only its declared claim.

### Synthetic Distractor Corpus (implemented, adapted)

`plugins/memory/src/eval/synthetic.rs` generates this at runtime — it is not a checked-in fixture. Adapted from §4 of the Write-Time Gating paper to this crate's `DefaultSalienceGate`, which has no reputation-scoring concept: "high-reputation correct fact" maps to a `UserStated`, high-importance candidate (expected `Accept` on first write); "low-reputation distractor" maps to an `AgentInferred`, low-importance candidate (expected `Archive`).

```
For each target memory T:
  Generate D distractors, each embedded at a distinct cosine similarity to T,
  spread deterministically across [0.0, 0.75) — see the note on the 0.75 ceiling below
  Assert T is Accepted when written to an empty store
  Assert every distractor is Archived when written against a store containing only T
```

Ratios: 1:1, 4:1, 8:1 distractors per target, generated with `SYNTHETIC_TARGET_COUNT = 20` targets per ratio (not the 100 minimum below) — see the CI-runtime-budget deviation below. Only the 8:1 ratio is wired into a CI gate (`gate_accuracy_8to1`); 1:1 and 4:1 are generated and reachable via `eval list-corpora` but not separately gated.

**Deviation — deterministic spread ceiling of 0.75, not the full [0.0, 1.0) range**: `DefaultSalienceGate`'s `novelty_doubt_threshold` (default 0.80) and `merge_similarity_threshold` (default 0.85) mean that above ~0.80 cosine similarity, the gate correctly prioritizes near-duplicate merging over salience filtering — a different, equally correct code path, not a gate-accuracy failure. Spreading distractors up to 1.0 would non-deterministically mix two different expected outcomes (`Archive` below the novelty floor, `Merge` above the merge threshold) into a single fixed `Archive` expectation. Capping the spread at 0.75 isolates exactly the property under test: that low-value candidates get archived regardless of topical similarity, up to the point where near-duplicate detection legitimately takes over.

**Deviation — 20 targets per ratio, not the specified minimum of 100**: `eval run --ci` builds one fresh SQLite store per gate case (so cases never observe each other's writes), and at 8:1 that is 20 × 9 = 180 fresh stores per run. Scaling to 100 targets (100 × 9 = 900 stores at 8:1) would push the CI job well past a reasonable timeout. 20 targets already gives 180 gate evaluations at the gated ratio, which is enough to detect a real regression in gate behavior; the count is a `const` in `synthetic.rs` and is the first thing to raise if more statistical power is needed later.

Each corpus memory is annotated with a stable `key`, `content`, `memoryType`, `provenance`, `importance`, an optional `anchor` (`{key, targetCosine}`) that engineers its embedding to an exact cosine similarity to another memory in the same corpus or gate case instead of an independent random axis, and an optional `scope` (defaults to `workspace`, matching every retrieval-corpus fixture). `scope` is what makes `GateDecision::Reject` reachable in a `GateCase`: the runner opens a separate `SqliteMemoryStore` instance per distinct scope needed (`store()` rejects a memory whose scope doesn't match the store's own configured scope), so an `existing` memory in a *broader* scope than the candidate is visible to the gate's novelty check via `visible_scopes()` without being writable through the candidate's own store. Retrieval-corpus queries additionally carry `relevantKeys` (ground truth), optional `relevanceGrades` (0-3, for NDCG), and optional `contradicts` (hallucination-rate ground truth).

### Deviation: deterministic axis embeddings, not an `EmbeddingProvider`

The harness needs exact, reproducible control over cosine similarity between specific memories and queries — the entire synthetic-distractor and hallucination-rate methodology depends on it. Rather than a network-capable `EmbeddingProvider` (even a stubbed one), `plugins/memory/src/eval/embedding.rs` assigns each corpus entity a unique "home axis" in the 768-dimensional embedding space (`axis_vector`) and constructs any desired exact cosine similarity to another entity as a two-axis unit vector (`vector_at_similarity`) — generalizing the `axis_embedding`/`cosine_embedding` pattern already used by `plugins/memory/tests/cli.rs`. Two facts about this crate's store made this the correct design rather than a workaround: `MemoryCandidate.embedding` and `SearchQuery.embedding` both accept a precomputed vector directly (`novelty_embedding` in `gate.rs` prefers `candidate.embedding` over calling any provider), and `SqliteMemoryStore::store_embedding` clears `embedding_stale` on write. No `EmbeddingProvider` is constructed anywhere in the eval harness, which trivially satisfies "no network access" for every metric, not just the ones a corpus format happens to make easy.

## Metrics

Every row below has a corresponding gate key in `plugins/memory/eval-harness-v1.json`, computed and compared every `eval run`. "Aspirational target" is this design's original number; "Phase A gate" is what is actually checked in today, where they differ.

| Metric | Source | Aspirational target | Phase A gate |
|---|---|---|---|
| recall@10 | Top-k retrieval accuracy | ≥ 0.90 | ≥ 0.90 (met) |
| precision@10 | Precision in top-k | ≥ 0.80 | ≥ 0.80 (met) |
| NDCG@10 | Rank-weighted accuracy | ≥ 0.85 | ≥ 0.85 (met) |
| Hallucination rate | Fraction of contradiction-bearing queries where a contradicting memory outranks (or stands in for a missing) correct answer — see deviation below for why this is rank-sensitive, not "any co-occurrence" | < 0.05 | < 0.05 (met) |
| Gate accuracy (8:1) | % of correct gate decisions, synthetic distractor corpus | ≥ 0.95 | ≥ 0.95 (met) |
| Gate accuracy (full) *(added in Phase A follow-through, not in the original design)* | % of correct gate decisions, hand-authored `gate-decisions-v1.json` — the only corpus that exercises all five `GateDecision` variants including Reject, which the synthetic 8:1 corpus deliberately never reaches (see the Corpus section) | — | ≥ 1.0 (met; 6 deterministic, hand-verified cases) |
| Write p50/p95 latency | Time for `gate.evaluate` + `store()` + `store_embedding()`, isolated 30-sample pass | p50 < 200ms, p95 < 500ms | p50 < 200ms, p95 < 500ms (met) |
| Retrieval p50/p95 latency | Time for `search()`, bulk pass at `RETRIEVAL_SCALE_MEMORY_COUNT` memories | p50 < 100ms, p95 < 200ms at 10k memories | **p50 < 100ms, p95 < 200ms at 2,000 memories (met — real KNN, see deviation below)** |
| Storage efficiency | *Marginal* bytes / active memory, measured at `RETRIEVAL_SCALE_MEMORY_COUNT` (see deviation below for why) | ≤ 2 KB average | ≤ 4.5 KB average — see deviation below |
| Stale embedding ratio | stale_embeddings / total, deliberately non-zero (the golden corpus's last memory is left unembedded) | < 5% | < 5% (met) |

### Deviation: hallucination rate redefined as rank-sensitive (now meets the aspirational target)

The original definition — "contradictory retrievals / total retrievals," counting any co-occurrence in top-k — degenerates on a small corpus: a query with only ~2 candidates above the relevance floor trivially returns both, regardless of whether the system actually preferred the wrong one. Phase A's first pass calibrated the gate to 0.15 to work around this; that was treating a metric-definition defect as a tuning problem. The metric is now rank-sensitive (`plugins/memory/src/eval/metrics.rs::hallucination_rate`): a query counts as hallucinated only when a contradicting memory outranks the correct one, or the correct one is missing entirely while a contradicting one is present. A contradicting memory merely appearing *alongside* a correctly-ranked-first answer no longer counts — which is the actual property worth gating, since a system that always ranks the truth first isn't hallucinating regardless of what else surfaces. Under this definition the measured rate against the golden corpus is 0.0 (the two engineered contradicting memories, anchored at 0.1 cosine similarity, never outrank their 1.0-cosine correct counterparts), so the gate reverts to the spec's original aspirational < 0.05.

### Deviation: storage efficiency redefined as marginal cost, measured at scale (target adjusted; still not the aspirational 2 KB)

`total_storage_bytes` is `PRAGMA page_count * page_size` for the whole database file, not scoped to active memories. Measured decomposition at N=28 on Phase A's first pass (before real KNN existed): 241,664 B (67%) was fixed schema/index overhead that exists before the first memory is ever written (55 b-tree root pages), not storage the corpus itself caused. The runner snapshots `total_storage_bytes` on a fresh, empty store before injecting anything and reports `(bytes_after − bytes_before) / active_count` — genuine marginal cost, insensitive to fixed schema overhead.

That fix alone was not enough once real `vec0` KNN landed (see below): `vec0` uses chunked storage, allocating space for a batch of rows on the *first* insert into an empty table. Measured against the ~28-memory golden corpus, that one-time chunk allocation dominated the sample the same way fixed schema overhead did before — over 100 KB/memory. **Storage efficiency is therefore now measured against the `RETRIEVAL_SCALE_MEMORY_COUNT`-memory bulk pass** (`measure_retrieval_latencies_at_scale_ms`, which now also snapshots storage), not the small golden corpus — the same fix pattern (isolate marginal cost from a fixed allocation) required a larger sample to actually amortize against. Measured there: ~3,900 B/memory, stable across repeated runs.

That is still not 2 KB, and can't be with the current encoding: a 768-dimensional f32 embedding is 3,072 bytes of payload before any row/index/chunk overhead, already 50% over the 2 KB target on payload alone. The Phase A gate is set to 4,608 B (headroom above the measured ~3,900 B). Reaching 2 KB requires shrinking the stored embedding itself — int8 quantization (768 B payload, likely ~1 KB/memory marginal) is the cheapest lever, but changes the `vec0` column type, which is a MAJOR schema bump requiring a `ReembedMigration` per `docs/adr/2026-07-08-memory-contract-versioning.md`. Tracked as follow-up work, not fixed here.

### Retrieval latency: real KNN implemented, meets the aspirational target

Phase A's first pass calibrated this gate to 450/600ms because vector search was a brute-force scan: `load_vector_similarity_scores` decoded every candidate's embedding blob and computed cosine similarity in Rust, regardless of whether a `vec0` table was present, because `sqlite-vec` was not a Cargo dependency and no KNN query existed anywhere in the crate. Both are now fixed:

- **`shared/sqlite-vec-init`** (new crate) registers the `sqlite-vec` extension via `sqlite3_auto_extension` — the one FFI call in the workspace that needs `unsafe`, isolated in its own crate (no `[lints] workspace = true`) so `unsafe_code = "forbid"` stays in force everywhere else. `elegy-memory`'s `init_database` calls it once per process, guarded by a `OnceLock`, before opening any connection.
- `ensure_vec_memories_object` now declares `vec_memories` with `distance_metric=cosine` (confirmed empirically: identical vectors → distance 0, orthogonal vectors → distance 1). `vec_memories_uses_native_module` checks `sqlite_master.sql` so callers know which table variant is present.
- `load_vector_similarity_scores` splits into `_via_knn` (a real `MATCH` query — vec0 requires an explicit `k = ?` constraint directly alongside `MATCH`, confirmed empirically: an outer `ORDER BY ... LIMIT` alone raises "A LIMIT or 'k = ?' constraint is required on vec0 knn queries" as soon as the vec0 table is joined to another table) and `_via_scan` (the original Rust-side loop, kept as the defensive fallback for the — now essentially unreachable — case where registration somehow fails).
- The unbounded (`threshold = 0.0`) vector channel of hybrid search now requests `VECTOR_CANDIDATE_POOL_SIZE` (1,000) nearest neighbors via KNN rather than literally every positively-similar memory — a deliberate, standard hybrid-search bound. `budget_active_max` defaults to 500/scope, so this never actually truncates results in normal operation.
- A zero-magnitude stored vector makes `vec0` return a NULL distance (cosine similarity is undefined for the zero vector); mapped to "no match" in Rust, matching `similarity.rs::cosine_similarity`'s existing zero-norm handling.

Measured against the `RETRIEVAL_SCALE_MEMORY_COUNT`-memory bulk pass, in release mode: p50 ≈ 26ms, p95 ≈ 29ms — comfortably under the spec's original aspirational 100/200ms, not just a calibrated compromise. The gate is set to those aspirational values directly.

**Known gap, deliberately out of scope for this pass:** a database created before this change has the plain-table fallback, not a real `vec0` table, and `ensure_vec_memories_object` never re-runs its DDL once `vec_memories` exists — so an existing database stays on the slower scan path forever unless migrated. Migrating in place (copy every `(rowid, embedding)` row into a freshly created `vec0` table) is real, tracked follow-up work. Given `elegy-memory` is documented as local-single-user-posture with no reviewed cross-host install receipt (`docs/readiness.md`), the practical exposure is low today, but this is a genuine compatibility gap, not a hypothetical one.

## Architecture

`test_harness/` at repo root is not a permitted directory kind (`docs/repo-layout.md` enumerates the allowed roots, and `scripts/check-repo-shape.ps1 -FailOnIssues` blocks it in two CI jobs) — the implementation lives crate-local instead, under the same `plugins/memory/` root every other memory concern already uses:

```
plugins/memory/
  src/eval/
    mod.rs         # pub(crate) surface: run_eval, sweep_threshold, list_corpora
    corpus.rs       # RetrievalCorpus / GateCorpus schema, embedded + file loaders
    embedding.rs    # deterministic axis-based vectors, no EmbeddingProvider
    synthetic.rs    # Write-Time Gating-adapted distractor generator
    metrics.rs      # pure metric functions, several shared across gated metrics
    gates.rs        # eval-harness-v1.json load + threshold comparison
    runner.rs        # fresh store -> inject -> query -> measure -> compute -> gate
  fixtures/eval/
    golden-v1.json          # hand-authored recall/precision/NDCG/hallucination corpus
    gate-decisions-v1.json  # hand-authored Accept/Archive/Merge/Contradiction/Reject cases
  eval-harness-v1.json      # gate thresholds (single source for CI + sweep)
  tests/eval.rs             # subprocess driver against the built binary
```

`src/eval` is `pub(crate)` — internal to the crate, not part of its public API — because three things it needs (`similarity::cosine_similarity`, `SqliteMemoryStore::learned_weights_report()`, `embedding::prepare_embedding_input`) are themselves crate-private, and `eval run` is a CLI subcommand regardless, so `src/cli.rs` is the only consumer either way. `tests/eval.rs` therefore drives it as a subprocess against `env!("CARGO_BIN_EXE_elegy-memory")`, matching the existing convention in `tests/cli.rs`, rather than exposing a second, test-only public surface.

The runner:
1. Opens a fresh `SqliteMemoryStore` for each store it needs — one for the golden retrieval corpus, one per gate-decision case (so cases never observe each other's writes; a case whose existing memories span more than one scope opens one store per distinct scope against the same database file, since a store instance can only write into the scope it was opened with), one for the isolated write-latency pass, one for the bulk retrieval-at-scale pass
2. Injects memories directly via `store()` + `store_embedding()`, bypassing the write-time gate entirely for the golden corpus so a gate decision (e.g. an unexpected merge between two similar golden entries) can never perturb retrieval ground truth
3. Runs queries built from `blended_vector` over each query's `relevantKeys`, and separately evaluates both the synthetic 8:1 and hand-authored gate-decision corpora through the real `DefaultSalienceGate`, computing every gated metric
4. Compares every metric against `eval-harness-v1.json`
5. Reports pass/fail per metric, plus an overall `passed`

Two behaviors of the existing store the runner works around, both discovered while validating this implementation against the real gate:
- `storage::schema::initialize_scope_config` force-rewrites `merge_similarity_threshold`, `novelty_doubt_threshold`, and `dedup_threshold` to fixed values on every `init_database` call. `sweep_threshold` therefore writes its swept `scope_config` value via a direct SQL update *after* store construction (`override_scope_config` in `runner.rs`), the same technique `cli.rs`'s `execute_reembed_command` already uses for read-only diagnostic queries against the store's persisted tables — never before, which would be silently discarded.
- `SqliteMemoryStore::record_feedback` recomputes and persists learned ranking weights into `scope_config` as a side effect, and `search()` reloads `scope_config` per query. Every eval run and every sweep point therefore gets its own fresh store; nothing in the harness calls `record_feedback`.

## CLI Integration

```bash
cargo run -p elegy-memory -- eval run [--corpus path] [--output path] [--thresholds path] [--ci]
cargo run -p elegy-memory -- eval list-corpora
cargo run -p elegy-memory -- eval sweep-threshold --param <name> --range <start>..<end>
cargo run -p elegy-memory -- eval export-labels [--db path] [--scope workspace] [--output labels.jsonl]
```

The `eval` subcommand is a `v2` CLI command family (like `feedback`, `weights`, `traverse`) — the first with a real nested `#[command(subcommand)]` in this file, since its four actions take genuinely different arguments; `command_name` reports them dotted (`eval.run`, `eval.list-corpora`, `eval.sweep-threshold`, `eval.export-labels`), following the convention `contradictions.resolve` already established.

`--thresholds path` on `eval run` is not in the original design. It points at a file with the same shape as `eval-harness-v1.json` and defaults to the embedded thresholds when omitted. It exists so a test (or a maintainer sweeping a proposed threshold change) can force a gate failure without mutating the checked-in threshold file — `plugins/memory/tests/eval.rs`'s fail-closed tests use it directly.

## CI Gates

The `memory-eval` job in `.github/workflows/rust-ci.yml` runs `cargo run --release -p elegy-memory -- eval run --ci` on every PR. No path-filter change was needed: `plugins/**` at the top of that workflow already subsumes every file below.
- `src/gate.rs`
- `src/decay.rs`
- `src/similarity.rs`
- `src/storage/sqlite_store.rs` (search + scoring paths)
- `src/storage/schema.rs` (scope_config defaults)
- `src/consolidator.rs`

A failing gate fails the job the same way any other job in that workflow fails a PR — there is no separate soft-gate mechanism in this repo to model this on. `tests/eval.rs` already exercises the same `eval run --ci` invocation, in debug mode, on all three OSes, inside the existing `test` job's `cargo test --workspace --all-targets --all-features` step; `memory-eval` adds the one thing that doesn't cover — a release-mode run, under a name a reviewer can point to as "the eval gate."

Thresholds live in `plugins/memory/eval-harness-v1.json`, not at repo root as originally designed — the repo root holds no `.json` files (only `.toml`), and every other crate-local JSON config (`readiness.json`, `capability-catalog.json`) is namespaced one level down. `eval-harness-v1.json` is that one source for both this CI job and `eval sweep-threshold`.

## Labeled Feedback Export

The existing `retrieval_feedback` table already stores relevance judgments. The eval harness exports labeled evaluation sets from it, scoped the same way every other command in this CLI is scoped (via a join through `memories.scope`, since `retrieval_feedback` has no scope column of its own):

```bash
cargo run -p elegy-memory -- eval export-labels --db path.sqlite3 [--scope workspace] [--output labels.jsonl]
```

Format: one JSON object per line, camelCase (matching every other JSON surface in this crate, not the snake_case used descriptively above): `feedbackId`, `memoryId`, `queryText`, `relevant`, `recordedAt`.

## Acceptance Criteria

- [x] `cargo run -p elegy-memory -- eval run --ci` exits non-zero when any metric in the Metrics table misses its gate target against the embedded corpus — verified against a `--thresholds` override with an impossible `recall_at_10` target, and against a threshold file missing a required metric key.
- [x] Every metric in the Metrics table has a corresponding assertion in the harness; no metric is computed without a gate. `EvalGateThresholds::require_all` fails closed (a hard error, not a skipped metric) if any of the twelve required threshold keys (`gates::REQUIRED_METRIC_NAMES`) is absent from a threshold file, embedded or supplied.
- [x] `cargo run -p elegy-memory -- eval sweep-threshold` can run against a changed threshold without modifying the checked-in corpus — it writes the swept `scope_config` value directly to each sweep point's own fresh temp store, never to `eval-harness-v1.json` or the fixture files.
- [x] CI runs the harness on every PR touching the file list under CI Gates, and a failing gate blocks merge the same way `cargo test` does today (the `memory-eval` job; see CI Gates above for why this criterion is also independently met by the existing `test` job).
- [x] The corpus used by `cargo test -p elegy-memory --test eval` requires no network access and no external download — confirmed by construction: the harness never constructs an `EmbeddingProvider`, and every corpus it runs against is either `include_str!`-embedded or generated at runtime.

Deferred, not claimed as met: fetching or importing LoCoMo/LongMemEval (Phase B), and the broader MaRS-inspired forgetting-policy scenario suite (Phase C — five policies and one bounded retention qualification protocol are implemented; see Corpus above).

## Validation

```bash
cargo test -p elegy-memory --test eval
```

Runs the full harness (all six tests, including the two fail-closed cases) against the embedded corpus, with no network access and no external download — the full annotated corpus described as living in CI storage in the original design does not exist; see the corpus-acquisition deviation above for what replaces it.
