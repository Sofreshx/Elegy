---
title: Resumable Memory qualification implementation plan
status: active
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
- [ ] Commit coherent slices, verify publication identity and authorized origin,
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

Saved experiment receipts and remote durability are still pending. Test
fixtures are not installed-host qualification evidence.
