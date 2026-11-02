---
title: Memory production integration and reproducible handoff
status: active
owner: elegy-memory
doc_kind: planning
---

# Memory production integration and reproducible handoff

Goal: finish and validate the retained Memory/MCP source, preserve reproducible
evidence and deliver a production candidate through the existing origin/main
workflow. The user authorized completion, push and merge. Real-data activation,
cloud credentials and a persistent tunnel remain paused.

Authority: root AGENTS.md -> docs/architecture/README.md -> existing recall,
Memory MCP and readiness specs -> plugin/host AGENTS.md. Extend the existing
contracts and retain their instruction/data, authorization and source-state
boundaries. Passing synthetic checks does not promote readiness.

## Recovered work

The integration retains all eleven original commits from the four local-only
source/evidence branches. Shared-author reads, the descriptor/host correction
and both historical qualification campaigns are ancestors of the integration
branch; their original refs and worktrees remain available. Main cdaaacc
already contained the earlier lexical recall and Tokio corrections.

The finished changes include:

- Explicit shared-agent read allowlists with original authors and owner-only
  mutations. Legacy private reads including unowned rows now apply SQL
  authorization before candidate limits and metadata effects. Two failures
  involving 1,001 hidden vectors and a non-returned row were reproduced first.
- Repository resources, a real stdio handshake in degraded configuration,
  structured diagnostics, stderr logging and a transport-free preflight.
- Opt-in Windows installation and tunnel helpers with verified backups,
  interrupted-apply recovery, configuration rollback, pinned executable bytes,
  isolated child environments and owned-process cleanup. Installation requires
  a newly reviewed private plan and all configuration writers closed; the
  single-writer prerequisite is not an indivisible compare-and-swap guarantee.
- Installed-binary receipts that detect modified/missing installed bytes, plus
  portable fixture checks in CI. The security scan uses the checksum-pinned
  official free Gitleaks CLI; only exact reviewed historical false positives
  are excluded.
- A hostile multiline regression preserving JSON boundaries, the complete byte
  budget, event/record metadata and source database bytes. The tested 200-byte
  field-policy candidate was withdrawn after native qualification refuted it;
  production retains the original 202-byte header.

## Evidence and acceptance

The [integration receipt](../../hosts/memory-mcp/evidence/production-closure-2026-11-02.json)
records source bindings and validation. The [preserved campaign](../../plugins/memory/evidence/production-closure-2026-11-02/README.md)
separates the adverse baseline before the field-policy correction from its
single controlled prompt-overlay experiment and its rejected native campaign.
None is relabelled as model qualification of the final production runtime.

Source tests for the tested candidate: **409 Memory and 90 Memory MCP
passed**, zero failed; one manual release benchmark is intentionally ignored by
the ordinary test command. All 32 contextual recall tests, all-target Clippy,
formatting and whitespace checks pass. Descriptor/host checks: 18 passed and
four repository resources resolved. Helper fixtures: 20 Python passed, one
Windows junction-creation skip, seven PowerShell fixture groups and five
installed-receipt tests passed. Junction detection has separate coverage.
Three existing feedback/journal regressions and the valid-request failing-child
regression also passed. Independent review has no unresolved material finding.

The rejected candidate source is committed at 686fcb5. Its clean checkout
ef50a694238081527e2a28baa31f2e43691b8c2b includes that source and the preserved
baseline/overlay evidence. Before and after its release build, all 301 measured
source hashes and the clean checkout state were unchanged. The package version
is 0.1.0; the CLI does not expose a version flag.

The rejected-candidate benchmark on that source uses 1,005 synthetic rows, five warmups,
100 measured calls and independent sessions. Results: p50 **241.791 ms**,
p95 **325.3459 ms**, maximum **375.2361 ms**. All 105 selections were correct;
source database bytes, WAL and counters were unchanged. No known competing
build/scan was active at the start. The measured p95 meets the 500 ms target
for this Windows x64 lexical library workload. It excludes process startup,
providers, embeddings, concurrent writers, model response time and real data;
it is not a general latency guarantee.

The native candidate produced 48 valid fresh answers with zero forbidden
mentions. Its frozen means were 0.875 with recall and 0.2986111111 without;
21/24 with-recall answers were fully correct under the unchanged lexical rubric.
Those descriptive gains do not satisfy its stronger predeclared criterion:
q19 omitted two facts explicitly present in the packet, giving 23/24 semantic
completeness and a new error among previously correct cases. Two of ten
acceptance checks failed. The candidate is **REFUTED**, not accepted, and was
withdrawn without retries, another candidate or relaxed thresholds. The
original header's known answer limitations remain visible in the baseline.

The separate provider-free [production runtime record](../../plugins/memory/evidence/production-closure-2026-11-02/runtime-release/README.md)
will bind the retained runtime, its release artifacts, contract/retention
receipts and fresh CLI/MCP replay. It makes no new model-answer claim.

Current gates:

- [x] Recover source and evidence with original ancestry and historical verdicts.
- [x] Reproduce/fix authorization, installed-byte and helper recovery failures.
- [x] Pass focused/full source tests and independent implementation review.
- [x] Run the frozen native paired campaign and preserve its refutation.
- [ ] Validate the restored 202-byte runtime, then qualify committed release
  artifacts with a fresh provider-free benchmark, replay and local claim store.
- [ ] Validate final docs, generated projections, contracts and repository shape.
- [ ] Pass exact staged/publication privacy gates with hooks active; verify the
  published head and applicable CI before the authorized merge.

## Handoff and limits

[PR #51](https://github.com/Sofreshx/Elegy/pull/51) owns the observable integration
and CI state. The task branch is `codex/memory-production-closure-20261102`.
Retain the original commits through a merge commit; do not force-push or rewrite
published history. A main merge triggers the existing main-snapshot build and
release workflow; its result must be observed separately from source validation.

Another agent should start with the [qualification guide](../../plugins/memory/docs/qualification.md),
reload the exact inventory and evidence, and use a new disposable store for new
experiments. Source/build/executable/OS bindings determine freshness; an old
success is not evidence for a changed source or a second machine. Preserve
failed and inconclusive receipts together with the append-only ledger.

Readiness remains **implemented**, non-routable by default. There is no new
clean real installation, installed cross-host task or cloud-connectivity proof.
The real database/configuration was not opened, migrated or activated. Old
private helper pins were not silently updated. OBS-003 and unrelated portfolio
follow-ups are outside this integration, and this plan does not declare all
Elegy features or roadmap work complete.

Stop on a private scan match, unexpected user edit, unapproved provider cost,
real-data/credential boundary, changed source during qualification or a failed
predeclared criterion. Preserve the result and reassess; do not weaken tests,
retry away an adverse answer, fabricate qualification or promote readiness.
