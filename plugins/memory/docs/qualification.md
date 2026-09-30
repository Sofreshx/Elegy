---
title: Resumable Memory qualification
status: active
owner: Elegy Memory
doc_kind: guide
---

# Resumable Memory qualification

An agent starts with the exact inventory, runs a bounded experiment, and
resumes its evidence in the next session. Run from an Elegy source checkout:

```text
cargo build -p elegy-memory
elegy-memory eval status --json
elegy-memory eval next --json
elegy-memory eval run --claim recall.contract --json
elegy-memory eval run --claim forgetting.retention --json
elegy-memory eval history --claim recall.contract --json
```

Use the just-built executable (`target/debug/elegy-memory`, with `.exe` on
Windows) if a PATH installation is older. Every qualification command accepts
`--project ABSOLUTE_REPO` and `--evidence-dir DIRECTORY`. Relative evidence paths
resolve against the project; the default is
`plugins/memory/evidence/qualification`. Observation input paths resolve against
the caller's working directory. Qualification currently requires the source
checkout and Git; external MCP subjects can be installed independently.

`status` and `next` do not create files. They return protocol/version/digest,
required checks, stale reasons, receipt-relative paths and an argument-array
invocation retaining the requested project/evidence directory. Treat that array
as process arguments, never as a shell script. An external invocation contains
`FILE`, which must be replaced with reviewed observations after the experiment.
`next` sorts refutations, stale results, inconclusive results, unverified claims,
then external review; it prefers local scenarios within a rank, then claim ID.
If source and build differ, `next` returns a rebuild invocation before any claim
execution. Rerun it with the newly built executable.

## Five initial claims

| Claim | Experiment | Meaning of success |
| --- | --- | --- |
| `recall.contract` | Real recall APIs over synthetic SQLite state, with positive/negative controls, feedback and journal checks | Contract checks pass in this source-library scenario |
| `forgetting.retention` | Real budget enforcement for all five policies, identical labelled fixtures | Every count budget holds and the default retains the labelled important facts |
| `host.cross-session` | Store through installed MCP, close client, recall by ID/search in a fresh session | Reported persistence in that installation |
| `host.agent-isolation` | A writes, A recalls, B attempts ID/search/list against the same database | Reported namespace isolation in that installation |
| `recall.answer-quality` | At least ten matched with/without-recall cases with fixed model/configuration and predeclared rubric | Reported positive mean score difference on that finite sample |

The inventory's executable definition is the `Claim` table in
[`qualification.rs`](../src/eval/qualification.rs), projected as
`memory-claim-inventory/v1`. Protocol hashes include this inventory version and
the complete claim, including required checks. Fixture details and recorded
measurements live in [`scenarios.rs`](../src/eval/scenarios.rs). The forgetting
experiment is a small local scenario; it does not claim published benchmark
parity. Priority decay uses the store's real clock; each policy's execution time
window is recorded. Its retained IDs describe that run, not a timeless ranking.
Freshness establishes the declared budget/default-retention contract, not an
identical future ranking for time-dependent comparative measurements.

## Outcomes and freshness

`satisfied` means all required checks passed; `refuted` means a stated check
failed. Execution errors or missing protocol checks are `inconclusive`.
Absence is `unverified`. A changed source, build, executable, environment or
protocol makes a previous result `stale`. An unrelated Git commit alone does
not. Source edits during execution make the attempt inconclusive.

The build and runtime share [`fingerprint.rs`](../src/eval/fingerprint.rs):
sorted relative filenames and exact bytes, each length-prefixed before SHA-256.
Its `INPUTS` list covers workspace/Memory manifests and lock, Memory code,
tests, fixtures, schemas, hook integration, eval thresholds, shared source and
the two relevant specs. Only listed source-like extensions are hashed;
generated binaries, caches and evidence receipts are excluded. Coverage is
conservative: shared-source changes can invalidate unrelated Memory claims.
Source/build mismatch blocks automated runs until rebuilding. Executable bytes
capture build-option changes; OS and architecture also participate in freshness.
Git SHA and dirty state are recorded as context, not proof of a clean release.

External results are `reported-satisfied`, `reported-refuted`, or
`reported-inconclusive`, with `reviewRequired: true`. The recorder's source and
binary are separate from the **tested subject** (artifact/configuration hashes,
host and host version). This CLI cannot inspect an external host's current
state: `externalSubjectRequiresRecheck` remains true. Review the actual subject
and evidence before reuse. No qualification command changes readiness artifacts.

## Installed-host and answer observations

Follow the protocol returned by `status` using a dedicated synthetic namespace.
Do not use existing user memories or auto-load transcripts. Keep session/client
identifiers as non-personal pseudonyms. For answer quality, freeze inputs,
rubric and negative controls before running; use fresh contexts and an independent
blinded assessor or deterministic scoring. Record both costs in one declared
unit and actual latency per case. A useful/irrelevant memory judgment alone is
not an answer-quality measurement.

Prepare JSON matching the strict
[observation schema](../schemas/qualification-observation.schema.json).
Required fields are `schemaVersion: memory-observation/v1`, `claimId`,
`protocolVersion`, `subject`, at least two distinct `sessions`, `startedAt`,
`completedAt`, exact named `checks`, and `evidence`. Each check has `name`,
`passed`, and a short reviewed `detail`. The caller cannot supply an outcome.
Set `error` when an experiment could not complete; this yields an inconclusive
receipt. All protocol checks still appear, with details identifying unexecuted
steps. For answer quality, include 10–1000 unique `pairedCases` and `costUnit`;
the CLI computes the mean improvement itself from scores normalized to 0–1.

Evidence entries contain a portable relative `path` and its `sha256`. Place
reviewed evidence beneath the evidence directory, outside `receipts/`.
Paths with parent traversal, symlinks or Windows reparse points are rejected.
Hash the file before recording; every subsequent history/status read verifies it.
The maximum is sixteen files, eight MiB each. Raw prompts, transcripts, memory
contents, secrets and personal paths must be removed before sharing evidence.
External observations remain assertions by their reporter even when digests match.

```text
elegy-memory eval record --claim host.cross-session --observation observation.json --json
elegy-memory eval history --claim host.cross-session --json
```

`run --claim` and `record` return a nonzero exit for refuted or inconclusive
outcomes while still returning/persisting the receipt. Invalid input creates no
receipt. Legacy `eval run --ci`, corpora, threshold sweeps and label export
continue unchanged; claim runs reject legacy corpus/threshold/output overrides.

## Durable follow-up and recovery

Each receipt is created once under `receipts/UUID.json` and hashed in
`ledger.json`. Sequence in the ledger defines history order. A create-new writer
lock serializes appends; concurrent writers retry after the active writer ends.
A same-directory atomic link publishes a complete receipt; the ledger is then
atomically replaced. A crash between these operations leaves a detectable
inconsistency. Malformed, deleted, altered or unsupported receipts block the
entire inventory; deleting a failed run cannot reveal an older success as current.
Digests and the ledger detect accidental corruption, not malicious rewriting of
the complete store. Protect/backup the directory according to its data sensitivity.

After an interrupted writer, preserve the directory and inspect its lock,
pending file, receipts and ledger. Restore a consistent version from a durable
backup before retrying. Do not remove failed receipts or repair hashes to make
status pass. Never delete a lock until its writer is confirmed stopped.

Commit synthetic receipts and their ledger together or back them up to the
approved private evidence destination. A temporary directory is suitable for
tests, not the only copy of useful evidence. Preserve unsuccessful experiments.

For cross-session context, store a distilled Memory procedure containing the
logical project name, claim IDs, relative evidence location and `eval next`
invocation. Always reload exact status before reporting success. Memory salience,
consolidation, learning and forgetting never determine the open-claim inventory.
The host or Planning owns scheduling and responsibility; this CLI adds no timer,
provider cost or automatic MCP call. Readiness promotion still follows the
[readiness contract](../../../docs/specs/readiness-v1.md).
