---
title: Resumable Memory qualification implementation plan
status: completed
owner: Elegy Memory
doc_kind: planning
---

# Resumable Memory qualification implementation plan

Goal: make Memory claims resumable, falsifiable and bound to durable evidence.
Authority: root AGENTS → architecture → eval-harness-v1 spec → Memory AGENTS.
The user approved the design and implementation on 2026-09-30.

Architecture: extend the product-local eval CLI. Exact claim inventory and
append-only receipts own qualification state; Memory can recall pointers;
Planning/hosts retain scheduling. Source, build and binary fingerprints detect
stale evidence. Existing metric evaluations remain compatible.

## Delivery checkpoints

- [x] Add bounded Rust scenarios in `src/eval/scenarios.rs`: recall safety and
  comparative retention through all five actual budget policies. Test success,
  refutation and observable measurements. Worker owns this file only.
- [x] Close contextual recall regression gaps in the existing Rust and Python
  tests. Worker owns those two test files only.
- [x] Add claim inventory, provenance, strict external observations, receipt
  persistence/history, integrity checks and deterministic status/next selection.
  Parent owns `src/eval/qualification.rs` and fingerprint/build support.
- [x] Wire CLI commands and integration tests; preserve legacy eval flags.
- [x] Update spec/guide/index and execute focused tests, formatting, docs and
  contract checks. Review source/binary binding and artifact corruption paths.
- [x] Commit coherent slices, verify publication identity and authorized origin,
  push the current task branch and re-read the remote SHA.

Success: an agent can ask what remains, execute a known experiment, see failures
distinct from missing proof, and resume the same receipt history in a new session.
Host success and answer improvement remain explicit external experiments until
actual observations exist. Stop for a changed publication destination, private
identity violation or a new requirement to exercise private user data/providers.

## Validation evidence

- `cargo test -p elegy-memory`: 394 passed, zero failed/ignored, including
  25 contextual-recall and seven qualification integration tests.
- Python Codex recall hook suite: 14 passed, including a valid request whose
  child process fails.
- `cargo fmt --all -- --check`: passed.
- `cargo test -p elegy-memory-mcp`: 67 passed, zero failed/ignored.
- `cargo clippy -p elegy-memory --all-targets -- -D warnings`: passed after
  removing an unnecessary owned path comparison.
- Observation JSON Schema: ten accept/reject cases passed with the real Rust
  validator; the same cases are now a durable qualification integration test.
- Final qualification target: all eight tests passed after adding schema
  coverage. An additional targeted regression passed for rejecting the ledger
  and writer lock as external evidence; both bookkeeping files remain unchanged.
- Final Clippy and formatting checks: passed after the bookkeeping guard.
- Documentation inspect/map/check: passed; existing freshness warning for
  `docs/roadmaps/observation-substrate-roadmap.md` remains.
- Ten local Markdown links in changed guides/spec/routes: resolved.
- Contract validation: exit zero; existing absent compatibility-manifest
  warning remains. Observation schema is included in the exported bundle.
- Independent implementation review: no important issues remaining after
  corrections to rebuild guidance, default-policy identity, execution-time
  provenance and bounded runtime diagnostics.

## Saved qualification evidence

Both experiments ran on clean source commit
`64175e0dc0d5821ca5d6cd327ccddbcf1c1d1c43`, using Memory 0.1.0 on Windows x86-64.
Executable SHA-256:
`bfed6f83d67f50eabfab7cdb75f85f7515d12debdf6d64eedc8f1f32480be372`.

- [`recall.contract` receipt](../../plugins/memory/evidence/qualification/receipts/42a53d7b-cbf0-477d-9e1b-3e8a0f647652.json):
  satisfied, 16 checks, including isolated dismiss suppression and journal identity.
- [`forgetting.retention` receipt](../../plugins/memory/evidence/qualification/receipts/ac79a402-c03f-44ec-9e9c-0ea73f19635a.json):
  satisfied, seven checks. Every policy enforced the two-memory budget. Default,
  LRU and priority-decay retained both labelled facts; FIFO and random-drop
  retained neither. This is a measured counterexample within a six-memory fixture,
  not a universal policy ranking.

The receipts and their ledger are kept together in the default evidence directory.
Actual cross-session MCP use, agent isolation on an installed host and paired
answer quality remain `unverified`. No readiness artifact was promoted.

Implementation and evidence were pushed to the authorized
`origin/codex/memory-contextual-recall`; the remote SHA was independently re-read
as `adff2ae9278e1da7bed9486350e8e8989814b2b4`. The tree was clean at that checkpoint.
This completion record changes no qualified source input. Test fixtures and the
same-session navigation-pointer check are not installed-host qualification evidence.

### Refreshed evidence after delivery fixes

The dependency security update and portable test-fixture corrections changed the
qualification fingerprint. Both local scenarios were rerun against source commit
`c22043c431f01c374b695c02f6f04260bd2af739` on Windows x86-64:

- [Recall receipt](../../plugins/memory/evidence/qualification/receipts/e021213a-24bc-4213-90c6-2ab1ed565a82.json):
  satisfied, 16 checks.
- [Forgetting receipt](../../plugins/memory/evidence/qualification/receipts/d10b96dd-37a5-425b-82f9-5c87466892ee.json):
  satisfied, seven checks.

Both record matching source/build SHA-256
`63ab7b0eb0f2e9fa6e11985c8a7765844b0252d49940c1bfec0493d5ebd892e3`
and executable SHA-256
`d4484b9e879dbfb9a9176be953048143baece41114891514b8472ae21d32862c`.
The first run records a clean tree; the second records a dirty tree because the
first receipt and ledger update were then uncommitted. Qualified source bytes
were unchanged. Earlier receipts remain available as historical evidence.

The subsequent macOS cosine-rounding test correction changed the source
fingerprint again. A rerun against
`74b2f34abd63e9a01c741b6d476e4604a7d27e2b` produced
[16 satisfied recall checks](../../plugins/memory/evidence/qualification/receipts/e22f8f98-1617-4483-8cc0-eafab9631bc3.json)
and [seven satisfied forgetting checks](../../plugins/memory/evidence/qualification/receipts/201bf934-1fba-4110-a931-2ee3c5446b7e.json).
Both receipts record matching source/build fingerprints and the same executable;
the clean-first-run and evidence-only-dirty-second-run distinction also applies.

After isolating CLI test subprocesses from parallel-harness contention, both
scenarios were rerun against `9e95e08ecbf0033e4aa24a7f9c311a80f7bed4a3`:
[recall: 16 satisfied checks](../../plugins/memory/evidence/qualification/receipts/88658f98-a6d8-48ee-a469-0375378b3d0a.json)
and [forgetting: seven satisfied checks](../../plugins/memory/evidence/qualification/receipts/96191fda-eb5a-4a5c-8c33-5c7152b90519.json).
The receipts share matching source/build fingerprints and one executable hash,
with the same clean-first/evidence-only-dirty-second distinction. The production
750 ms recall budget is unchanged; these functional checks are not a latency
guarantee under load.

### Deterministic latency-discard regression

Serializing test subprocesses did not eliminate Windows failures. Review found
that the internal 750 ms check happened after candidate ranking: it discarded
completed results without bounding the expensive work and journaled an empty
attempt. The private-start-time regression
`storage::recall_store::tests::overdue_recall_still_returns_ranked_candidate`
failed with `empty` while that check remained, then passed with `selected`, one
recall and reported duration at least 1000 ms after its removal. Its start time
is simulated; this is behavioral evidence, not a latency benchmark.

Source commit `86b1b0d0e5f3223865abd5caa642c05b01fd2dd1` removes that check and
the temporary test mutex. The 25 contextual tests passed with normal parallelism;
the 14 hook tests passed with the one-second external cutoff unchanged.
The revised specification retains count, envelope and access protections.

Fresh receipts on those sources:
[recall: 16 satisfied checks](../../plugins/memory/evidence/qualification/receipts/0f739dd5-e258-425d-8171-f85261e33a51.json)
and [forgetting: seven satisfied checks](../../plugins/memory/evidence/qualification/receipts/ffc1e506-8c7f-48f9-bd53-2b9808651792.json).
Their source/build fingerprints match, their executable hash is identical, and
the clean-first/evidence-only-dirty-second distinction still applies.
