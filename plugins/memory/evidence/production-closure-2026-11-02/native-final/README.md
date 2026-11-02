---
title: Rejected native 200-byte candidate evidence
status: active
owner: Elegy Memory
doc_kind: guide
---

# Rejected native 200-byte candidate

**REJECTED CANDIDATE — not the production runtime.** The native acceptance verdict
is `REFUTED`: 23/24 with-recall answers were semantically complete, below the
predeclared requirement of 24/24. In q19 the answer declared two requested facts
unavailable even though the correct native packet contained `REGION_AMBER` and
`ROLLBACK_LARCH`. This was a new omission among previously correct cases.

The coordinator withdrew the 200-byte header and restored the earlier 202-byte
header for production. Its separate [provider-free runtime proof](../runtime-release/README.md)
is pending. This directory retains the rejected source, artifacts' hashes and all
48 answers; it does not qualify that final production runtime.

| Recorded result | With recall | Without recall |
| --- | ---: | ---: |
| Fresh isolated model sessions | 24 | 24 |
| Frozen lexical mean | 0.875 | 0.2986111111 |
| Fully correct under the frozen lexical rubric | 21/24 | 6/24 |
| Complete in supplemental semantic review | 23/24 | 8/24 |
| Forbidden mentions | 0 | 0 |

The paired mean difference is 0.5763888889, descriptive on this known finite corpus.
Eight of ten [predeclared acceptance criteria](native-verdict.json) passed; the
previously-correct-case and all-24 semantic criteria failed. q08, q09 and the Harbor
cases were correct, and the negative controls passed. Those successes do not replace
the failed criteria. There was no retry, weaker threshold, rubric change, answer
replacement or second candidate. The original
[assessment](assessment.json), [semantic review](semantic-review.json),
[frozen lexical gate](verdicts.json) and [rejection decision](rejected-candidate.json)
remain distinct. The lexical gate says `review-required`; the stronger predeclared
native semantic acceptance is `REFUTED`. Review is grounded agent interpretation,
not independent human certification.

The tested source is `ef50a694238081527e2a28baa31f2e43691b8c2b`, after prefix commit
`686fcb59055cf072140fee58418ede873d3d14c1`, with package version `0.1.0`.
[Clean-head binding](clean-head-binding.json), [source snapshot](release-source-before.json),
[release build](release-build.json), [subject](subject.json) and
[qualification summary](qualification-summary.json) bind the recorded candidate.
Its pinned CLI SHA-256 is
`a583ba9a66a41c111d8b5f00570ae2e527687e10842dc685ffc5d00d29f10bb9`;
its MCP SHA-256 is
`0452d71e50ec8cc76c787dac02f12f70fc48ef99d8fa5faa08cfc82bcc5c9298`.
Executables are excluded from this portable directory.

The guide was briefly edited after the artifact pin and then restored to its exact
original bytes before the final source check. All 301 broad inventory hashes matched
at the recorded endpoint; **uninterrupted immutability of all 301 is not claimed**.
The guide was outside the runtime fingerprint. The 226 Rust/Cargo inputs, pinned
binary hashes and all 48 actual prompt hashes remained unchanged. The summary
preserves this interval explicitly rather than presenting a continuously clean tree.
The original predeclaration and later clean-head binding are retained separately.

[Actual native replay](replay-01/run.json) completed eight episodes and 24 packets,
with the 200-byte prefix produced by the executable, no overlay, unchanged durable
source data and 16 empty cleanup namespaces. The [candidate benchmark](release-benchmark.json)
used 1,005 rows, five warmups and 100 measured calls: p50 241.791 ms,
p95 325.3459 ms and maximum 375.2361 ms, with expected selection and unchanged source,
WAL and counters. It is a local lexical workload, not model latency or production
performance proof. Its mechanical successes do not qualify the rejected candidate.

The successful [local-claim ledger](local-claims-native-context/ledger.json) retains
two original receipt wrappers for recall and retention. The separate restricted
attempt failed hard-link publication and had no ledger. Its original
[pending wrapper](failed-restricted-attempt/pending-receipt.json),
[failure output](local-claim-recall-contract.log) and
[restricted-attempt boundary](restricted-claim-boundary.json) remain separate.
The pending bytes were neither repaired nor promoted into a successful receipt.

The first model action was rejected before execution. An
[exact public synthetic outbound-payload proof](outbound-payload-proof.json)
then supported transparent reconsideration of the same action; the
[approval boundary](approval-boundary.json) records permitted execution without an
alternate route. All 48 actual prompt hashes match the approved payload. The 48
payload text files retain original Windows file bytes; the approved manifest hashes
the logical UTF-8 prompt sent to the model. Those logical hashes differ from CRLF
file hashes, which are separately preserved. No hash or payload was rewritten.

[Preservation provenance](preservation.json) records 146 byte-identical source copies,
zero derived files and 11 path-bearing auxiliary originals omitted with original
hashes and reasons. This includes all 48 raw answers and all 48 approved payloads.
The [original source manifest](source-sha256-manifest.json) is retained unchanged
under a distinct name. Reuse the historical five harness scripts; no duplicated
harness implementation or hardcoded-machine orchestration script is included.

[Final offline preservation verification](verification.json) records 517 passing
checks, including exact byte-for-byte frozen assessment reproduction, 48 raw and
nine replay bindings, 48 approved prompt bindings, source-manifest hashes, the
rejection and edit-interval provenance, and unchanged earlier baseline/overlay bytes.
The earlier [47 closed-stage checks](closed-stage-verification.json) describe the
partial state before model completion, not current acceptance. The
[parent manifest](../sha256-manifest.json) covers this directory. No new replay,
model call, actual database access or publication occurred during preservation.
