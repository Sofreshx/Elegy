---
title: Memory production integration and reproducible handoff
status: active
owner: elegy-memory
doc_kind: planning
---

# Memory production integration and reproducible handoff

Goal: finish the locally retained Memory and MCP changes, preserve their
evidence, and deliver a reviewed, validated production candidate recoverable
from origin. User authority: 2 November 2026 request to finish all retained
work and make it usable and pushable to production main; prior push/merge
authorization remains valid. No real-data activation, cloud key, persistent
tunnel or readiness promotion is implied.

Authority: root AGENTS.md -> docs/architecture/README.md -> existing recall,
Memory MCP and readiness specs -> plugin/host AGENTS.md. Extend existing
contracts, preserve public interfaces and untrusted-memory boundaries.

Facts: main cdaaacc already contains the Harbor/Harbor-Sandbox recall fixes
and Tokio update. Shared-author commit 48df62f has 408 Memory and 88 MCP
passing tests. The older realistic evaluation documents pre-fix failures and
must remain historical. The two activation helpers are uncommitted; the
descriptor-host branch contains six older commits needing current integration.
Real data/configuration and keys remain private, outside the source handoff.

Approach: reuse the shared-author checkout on a new codex integration branch;
retain all original refs/worktrees. Integrate qualification artifacts intact;
adapt the old descriptor changes to current contracts rather than overwrite
current README, lockfile or readiness with obsolete versions. Finish the
Windows helpers with isolated, repeatable tests and failure recovery. Measure
current recall in synthetic databases and preserve honest quality limits.

Success criteria:

- [ ] Every retained useful source/evidence slice is in a named durable ref,
  with clear current versus historical verdicts and no private runtime data.
- [x] Startup diagnostics/preflight and current project descriptors work;
  failed configuration cannot masquerade as a healthy resource catalog.
- [x] Shared reads preserve owner-only mutations and access filtering.
- [x] Installation dry-run/apply/rollback/retry preserve unrelated config and
  source data; tunnel key/process handling has reproducible synthetic checks.
- [x] Release-mode recall latency is measured with stated corpus, sample count,
  p50/p95/max and environment; no universal performance guarantee is claimed.
- [ ] Relevant Rust suites, Python/PowerShell fixtures, formatting, Clippy,
  contracts, docs and repo checks pass on the exact candidate. Reuse unchanged
  historical tests; run new integration checks where combined code changes.
- [ ] Independent review has no unresolved material finding. Before publication,
  private exclusions cover tree, paths, metadata, refs and reachable history;
  preserve hooks. Verify destination, automation and actual remote SHA.
- [ ] Final report distinguishes code delivery, publication, observed host use,
  real installation, cloud connectivity and missing evidence.

Stop conditions: private scan match; unapproved paid provider/key or external
exposure; unexpected user edits; unresolvable authority ambiguity. Continue
independent safe work and report the exact unresolved boundary. Do not rewrite
published history, delete originals, fabricate a passing evaluation or promote
readiness based on tests alone.

Execution units: (1) audit/integrate retained source and evaluation artifacts;
(2) finish helper recovery/tests/docs; (3) current synthetic recall measurement
and focused reliability checks; (4) combined validation/review; (5) privacy,
atomic commits, authorized publication and verified handoff.

## Current evidence

The [integration receipt](../../hosts/memory-mcp/evidence/production-closure-2026-11-02.json)
binds the source files, test results and synthetic benchmark. Descriptor/host:
18 passed and four repo resources resolved. Installer: 18 passed, with one
restricted-environment skip for real Windows junction creation (detection was
tested separately). Windows process fixtures: four passed. Executable receipt:
five passed after three installed-byte integrity failures were reproduced and
fixed. Three existing feedback/journal regressions and the valid-request
failing-child regression passed on current source.

Release lexical library benchmark: 1,005 rows, five warmups, 100 measured
calls and independent sessions; p50 243.18 ms, p95 306.76 ms, max 358.18 ms.
The expected authorized record was selected every time and source database
bytes, WAL data and access counters were unchanged. This meets the measured
500 ms p95 target for this workload, excluding providers and model latency.

The pre-existing 408 Memory and 88 MCP tests are reused for unchanged shared
runtime source. The historical campaign's Harbor retrieval failures already
have current regressions; its model omissions and adverse answer-quality
verdict remain limits, not silently converted to successes. Readiness stays
implemented until actual clean-install and non-fixture task evidence exists.

Remaining integration gates: combined Clippy/contracts/docs/repo checks,
independent review, exact staged-tree privacy, commits and remote verification.
The Windows helpers are explicitly opt-in and require a newly reviewed private
plan/pins for an actual activation; old private commands must not be silently
rebound to changed helper bytes.
