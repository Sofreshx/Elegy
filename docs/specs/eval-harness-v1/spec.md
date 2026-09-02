---
title: Eval harness v1
status: active
owner: Elegy Memory
---

# Eval Harness v1

## Contract

An evaluation harness for regression-testing elegy-memory's write→store→retrieve pipeline. Anchored on three academic benchmarks plus a synthetic distractor corpus. Metrics gate production readiness of threshold changes, decay parameter changes, and scoring weight changes.

**Implementation status**: not started. There is no `test_harness/` tree, no `eval` CLI subcommand, no corpus files, and no CI gate in this repository as of this writing. Everything below is the target design; the one existing prerequisite is the `retrieval_feedback` table (`plugins/memory/src/storage/schema.rs`), which the Labeled Feedback Export section already depends on.

## Corpus

| Source | Use | Reference |
|---|---|---|
| LoCoMo | Multi-session long-conversation recall | Hindsight paper (arXiv 2512.12818) |
| FiFA (MaRS) | Forgetting policy quality | MaRS paper (arXiv 2512.12856) |
| LongMemEval | Factual recall after distraction | Hindsight paper (arXiv 2512.12818) |
| Synthetic distractors | Write-time gating accuracy at 1:1, 4:1, 8:1 ratios | Write-Time Gating paper (arXiv 2603.15994) |

Each corpus is annotated with:
- Expected retrieval results per query (ground-truth memory IDs)
- Query text
- Optional metadata: session_id, memory_type, provenance, expected importance, expected decay state

### Synthetic Distractor Corpus

Reproduces the methodology from §4 of the Write-Time Gating paper:

```
For each target memory T:
  Generate D distractors with {cosine_sim(T, D_i) ~ uniform(0.0, 1.0)}
  Build query Q that matches T
  Assert T appears in top-k results
  Assert no D_i outranks T when similarity_gap < threshold
```

Ratios: 1:1, 4:1, 8:1 distractors per target. Minimum 100 target memories per ratio.

## Metrics

| Metric | Source | Gate target |
|---|---|---|
| recall@k | Top-k retrieval accuracy | ≥ 0.90 at k=10 |
| precision@k | Precision in top-k | ≥ 0.80 at k=10 |
| NDCG@k | Rank-weighted accuracy | ≥ 0.85 at k=10 |
| Hallucination rate | Contradictory retrievals / total retrievals | < 0.05 |
| Gate accuracy | % of correct gate decisions (accept/reject/merge/contradiction) | ≥ 0.95 at 8:1 ratio |
| Write p50/p95 latency | Time for store() including gate + optional embed | p50 < 200ms, p95 < 500ms |
| Retrieval p50/p95 latency | Time for search() | p50 < 100ms, p95 < 200ms at 10k memories |
| Storage efficiency | bytes / active memory | ≤ 2 KB average |
| Stale embedding ratio | stale_embeddings / total | < 5% |

## Architecture

```
test_harness/
  corpus/              # Annotated corpus files (JSONL)
  runner.rs            # Orchestrates pipeline replay
  metrics.rs           # Metric computation
  gates.rs             # CI gate thresholds
  reports/             # Output reports (JSON + HTML)
```

The runner:
1. Opens a fresh `SqliteMemoryStore` for each test run
2. Injects memories from corpus (simulating agent write)
3. Runs queries with expected results
4. Computes metrics
5. Compares against CI gate thresholds
6. Reports pass/fail per metric

## CLI Integration

```bash
cargo run -p elegy-memory -- eval run [--corpus path] [--output path] [--ci]
cargo run -p elegy-memory -- eval list-corpora
cargo run -p elegy-memory -- eval sweep-threshold [--param merge_similarity] [--range 0.80..0.90]
```

The `eval` subcommand is a `v2` CLI command (like `feedback`, `weights`, `traverse`).

## CI Gates

The eval harness runs in CI on every PR that touches:
- `src/gate.rs`
- `src/decay.rs`
- `src/similarity.rs`
- `src/storage/sqlite_store.rs` (search + scoring paths)
- `src/storage/schema.rs` (scope_config defaults)
- `src/consolidator.rs`

Failures block the PR. Thresholds live in the Metrics table above until the harness exists; once implemented, they move to `eval-harness-v1.json` at repo root so CI and `eval sweep-threshold` share one source.

## Labeled Feedback Export

The existing `retrieval_feedback` table already stores relevance judgments. The eval harness exports labeled evaluation sets from it:

```bash
cargo run -p elegy-memory -- eval export-labels [--scope workspace] [--output labels.jsonl]
```

Format: one JSON object per line with `memory_id`, `relevant`, `query_text`, `recorded_at`.

## Acceptance Criteria

- `cargo run -p elegy-memory -- eval run --ci` exits non-zero when any metric in the Metrics table misses its gate target against the embedded minimal corpus.
- Every metric in the Metrics table has a corresponding assertion in the harness; no metric is computed without a gate.
- `cargo run -p elegy-memory -- eval sweep-threshold` can run against a changed threshold without modifying the checked-in corpus.
- CI runs the harness on every PR touching the file list under CI Gates, and a failing gate blocks merge the same way `cargo test` does today.
- The minimal embedded corpus used by `cargo test -p elegy-memory --test eval` requires no network access and no external download.

## Validation

`cargo test -p elegy-memory --test eval` runs the full harness against a minimal embedded corpus (no external downloads). The full annotated corpus lives in CI storage and is fetched via `eval run --download-corpus`.
