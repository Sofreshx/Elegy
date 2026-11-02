---
title: Memory production integration and reproducible handoff
status: completed
owner: elegy-memory
doc_kind: planning
---

# Memory production integration and reproducible handoff

Goal: finish and validate the retained Memory/MCP source, preserve reproducible
evidence and deliver the production candidate through origin/main. The user
authorized completion, push and merge. Real-data activation, cloud credentials
and a persistent tunnel remain paused.

Authority: root AGENTS.md -> docs/architecture/README.md -> existing recall,
Memory MCP and readiness specs -> plugin/host AGENTS.md. Existing interfaces,
authorization and instruction/data boundaries remain authoritative. Synthetic
checks do not promote readiness.

## Delivered source

All eleven original commits from the four local-only source/evidence branches
are retained in the integration ancestry. Shared-author reads, the six-commit
descriptor/host correction and both historical qualification campaigns are
preserved with their original refs and worktrees. Main cdaaacc already included
the earlier lexical recall and Tokio corrections.

- Explicit shared-agent read allowlists preserve original authors and owner-only
  mutations. Legacy private reads including unowned rows now authorize in SQL
  before candidate limits and metadata effects. Two reproduced failures involved
  1,001 hidden vectors starving an authorized result and a non-returned row
  incorrectly receiving an access update.
- Repository resources, a real stdio handshake in degraded configuration,
  structured diagnostics, stderr logging and a transport-free preflight work.
- Opt-in Windows helpers verify backups, resume interrupted applies, roll back
  only the Memory configuration, pin executable bytes, isolate child environments
  and supervise owned-process cleanup. Applying a newly reviewed private plan
  requires one installer and all configuration writers closed; this prerequisite
  is not an indivisible compare-and-swap or universal power-loss guarantee.
- Installed-binary receipts detect altered/missing installed bytes. CI runs
  portable helper fixtures and the checksum-pinned official free Gitleaks CLI;
  only exact reviewed historical false positives are excluded.
- A hostile multiline regression preserves quoted JSON data, trusted event/record
  metadata, the complete byte budget and source database bytes. Production retains
  the original 202-byte recall header; the proposed 200-byte header was withdrawn.

## Accepted local runtime evidence

The [integration receipt](../../hosts/memory-mcp/evidence/production-closure-2026-11-02.json)
records source bindings and validation. The [production runtime record](../../plugins/memory/evidence/production-closure-2026-11-02/runtime-release/README.md)
preserves the committed release subject and provider-free results. Exact source:
`5c246906c7edf7fb876901b610d1de8352307715`, package version 0.1.0. The CLI does
not expose a version flag. The runtime and spec match the previously validated
cdcb3957 behavior, with the new boundary test retained for the 202-byte header.

Fresh rollback suites: **409 Memory + 90 Memory MCP = 499 passed**, zero failed.
One manual release benchmark is ignored by the ordinary suite and passed
separately below. All 32 contextual cases, all-target Clippy, formatting and
whitespace checks pass. Descriptor/host: 18 tests and four resources. Helpers:
20 Python passed, one Windows junction-creation skip, seven PowerShell fixture
groups and five installed-receipt tests passed; junction detection is covered
separately. Three feedback/journal regressions and the valid-request failing-child
regression also passed. Independent review has no unresolved material finding.

The final release build completed in 96.104 seconds from a clean checkout.
All 301 source hashes and both pinned executable hashes matched at every phase
check and at the end. Fresh native CLI/MCP replay passed eight episodes and
24 source-unchanged packets, with 23 nonempty original-header contexts and
16 verified-empty cleanup namespaces. Two fresh local claims, `recall.contract`
and `forgetting.retention`, are satisfied; their original receipts and ledger
are preserved together with status/next/history results. All three external
host/answer-quality claims remain unverified. No new model/provider call was
made for this final runtime, and Cargo ran offline.

The final release lexical library benchmark uses 1,005 synthetic rows, five
warmups, 100 measured calls and independent sessions. Results: p50 **255.4061 ms**,
p95 **345.9311 ms**, maximum **412.3922 ms**. All 105 selections were correct;
source database bytes, WAL and counters were unchanged. No known competing
build or publication scan ran during the coordinated measurement. The measured
p95 meets the 500 ms target for this Windows x64 workload. It excludes process
startup, providers, embeddings, concurrent writers, model response time and
real production data; it is not a general latency guarantee.

## Refuted candidate and preserved history

The [2 November evidence](../../plugins/memory/evidence/production-closure-2026-11-02/README.md)
keeps three experiments separate. The baseline before the proposed field-policy
change remains blocked on q09. One with-only prompt-overlay experiment passed
its own criteria but did not qualify a native runtime. The subsequent
[native candidate](../../plugins/memory/evidence/production-closure-2026-11-02/native-final/README.md)
at clean ef50a694 was **REFUTED**: q19 omitted two facts present in its packet,
giving 23/24 semantic completeness against the predeclared 24/24 requirement
and a new error among previously correct cases. Two of ten criteria failed.

All 48 native candidate answers remain intact. Frozen means of 0.875 with recall
and 0.2986111111 without are descriptive gains, not acceptance of that candidate.
The 200-byte change was withdrawn without a retry, another candidate or weaker
threshold. The original header's known model-answer limitations remain visible.
The 4 October and 23 October campaigns and the earlier Q8 proof retain their
original verdicts and source bindings. No unseen-holdout or universal model
reliability claim is made.

## Handoff and publication

Source implementation and local runtime qualification are closed; publication
and CI state are observable through [PR #51](https://github.com/Sofreshx/Elegy/pull/51)
and Git ancestry. The task branch is `codex/memory-production-closure-20261102`.
Use a merge commit to retain the original source/evidence objects. Verify the
actual published SHA, final-head checks and target before merging. A main merge
triggers the existing main-snapshot workflow; its release result is separate
from source validation. Do not infer that result from this static record.

Another agent starts with the [qualification guide](../../plugins/memory/docs/qualification.md),
reloads exact inventory/evidence and uses a new disposable store for new
experiments. Source/build/executable/OS bindings determine freshness; a changed
build or second machine must not inherit success automatically. Preserve failed
and inconclusive receipts with their append-only ledger. Never delete a failed
run or rewrite its hashes to expose an older success.

Readiness stays **implemented**, non-routable by default. There is no new clean
real installation, installed cross-host task or cloud-connectivity proof. The
real database/configuration was not opened, migrated or activated; private helper
pins were not silently updated. OBS-003 and unrelated portfolio follow-ups remain
outside this integration. This does not declare every Elegy feature complete.

Publication gates remain the configured private exclusions over content, paths,
metadata, refs and reachable history, with hooks active. Stop on a match,
unexpected user edit, unapproved cost, real-data/credential boundary, source drift
or failed criterion; preserve the evidence and reassess rather than retry away
an adverse answer or promote readiness.
