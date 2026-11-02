---
title: Memory baseline before the field-policy fix and controlled overlay, 2 November 2026
status: active
owner: Elegy Memory
doc_kind: guide
---

# Memory baseline before the field-policy fix

**The baseline answer-quality gate remains blocked.** This baseline records source
`cdcb3957b2a3d4b4bfa2eb178f914254cb436dd2`, **before the field-policy fix**.
It replays the known 24-question synthetic regression corpus with 48 isolated Codex
answers. Its adverse responses, frozen scores and blocked verdict remain unchanged.
It does not establish readiness for autonomous use or promote a readiness stage.

The distinct [controlled prompt-overlay candidate experiment](overlay-experiment/README.md)
is preserved in its own subdirectory: 24 new with-only answers, zero new without
answers, nine predeclared criteria recorded passed, and no native runtime change.
Its candidate result is not a fresh paired A/B claim or native/source qualification.
Any native verification belongs to a later, separately bound source phase.

The tested source is `cdcb3957b2a3d4b4bfa2eb178f914254cb436dd2`, tree
`40b9eff9085b76a76643892c84918b1ef53fbb94`, workspace package version `0.1.0`.
The clean release build is bound to 301 source files in
[the build receipt](release-build.json) and [the source snapshot](release-source-before.json).
Later documentation edits made the working tree globally dirty; the 226 recorded
code inputs remained unchanged. The subject is the pinned release artifacts,
not an unqualified executable subsequently found on PATH.

| Measurement on this finite sample | With recall | Without recall |
| --- | ---: | ---: |
| Frozen lexical mean | 0.8402777778 | 0.2986111111 |
| Fully correct under the frozen lexical rubric | 19/24 | 6/24 |
| Complete in supplemental semantic review | 22/24 | 8/24 |
| Forbidden mentions under the frozen gate | 1 | 0 |

The paired lexical mean difference is 0.5416666667. The
[unchanged deterministic assessment](assessment.json) and
[supplemental semantic review](semantic-review.json) answer different questions:
the latter is grounded agent interpretation, not independent human certification,
and never replaces the frozen lexical scores. The lexical discrepancies q10,
q15 and q24 remain visible. No threshold, expected answer, raw response or score
was changed during preservation.

The [finite acceptance gate](verdicts.json) is `blocked`: q09 presents a runbook
name as a restore identifier although no snapshot identifier was supplied.
Semantic review also records q08 omitting the retrieved 11-day retention.
The prior Harbor counterexamples q17/q18 now retrieve and answer with the
correct Harbor record. Their improvement does not remove q08/q09 or make this
known corpus an unseen holdout. The
[historical adverse campaign](../realistic-2026-10-23/README.md) remains unchanged.

The [local replay](replay-01/run.json) completed eight episodes and 24 recall
packets, with unchanged durable source data for every packet and all 16 cleanup
namespaces verified empty. Its manifests retain the MCP exchanges, exact recalled
packets, source snapshots and cleanup results; databases, WALs and runtime state
are excluded. [All 48 raw answers](answers-01/run.json) are retained with distinct
session identifiers, original JSONL, usage, elapsed times, prompts/configuration
digests and the actual public invocation arguments. The requested model was
`gpt-5.6-luna`, effort `medium`, client `codex-cli 0.166.0-alpha.5.1`, using existing
ChatGPT authentication. No additional paid route was used. The unsuccessful
[first preflight](preflight-01/run.json) and successful
[second preflight](preflight-02/run.json) remain separate attempts.

The [release benchmark](release-benchmark.json) reports p50 250.1333 ms,
p95 301.8666 ms and maximum 415.8057 ms over 100 measured lexical library calls
after five warmups, using 1,005 synthetic rows. The p95 was below the 500 ms target;
source database bytes, WAL and counters remained unchanged. A publication safety
scan overlapped the measurement, so this is not an idle-machine result. It excludes
model response time, network, embeddings, host startup and real production workloads.
After the benchmark replaced a Cargo target artifact, the
[artifact pin receipt](release-artifact-pin.json) verified restoration of the
original clean-release bytes used in replay:

| Artifact, excluded from this evidence directory | SHA-256 |
| --- | --- |
| `elegy-memory.exe` | `3f7cc338a8550726e8ed9d11ea2e0eca39bba58deea35e6f37c2d9f58091cc72` |
| `elegy-memory-mcp-stdio.exe` | `a4410052a9004234ce9157a2ba39dadaac11dc700d67cfa1799468fd1787918a` |

## Preservation and offline reproduction

Baseline [preservation provenance](preservation.json) records 79 byte-identical copied files,
three explicitly derived JSON receipts, original and derived hashes, omitted-source
hashes, and retained original hash references. The only substitutions in
`subject.json`, `release-build.json` and `qualification-summary.json` replace the
JSON string at `/build/targetDirectory` with `<release-target>`; every other byte
is unchanged. These three copies do not claim their originals' hashes.

The hardcoded-path campaign wrapper, raw build/benchmark logs, and failed-preflight
raw stderr were omitted because they contain machine paths. Their originals remain
in the source scratch area. Their hashes and omission reasons remain in provenance;
in particular, `orchestratorSha256` in the answer/preflight manifests and subject
still identifies the omitted original wrapper. No existing hash reference was
silently rebound. This directory alone cannot validate the omitted source bytes.
No binary, database, WAL, runtime home, client store, download, private configuration
or credential is included. This structural check is separate from the root-owned
publication exclusion gate.

[Offline verification](verification.json) records 203 successful checks: all copied
source hashes, the three exact derived transformations, 48 raw-answer and nine replay
assessment bindings, corpus/protocol/runner/scorer bindings, both consumers of the
assessment, source invariants, cleanup and inventory. Reusing the historical scorer
reproduced `assessment.json` byte for byte and the same blocked verdict. These checks
made no new replay or model call.

The portable [SHA-256 manifest](sha256-manifest.json) covers every file in this
directory except itself. From the repository root, with Python 3 available:

```powershell
$proof = Join-Path (Get-Location) 'plugins/memory/evidence/production-closure-2026-11-02'
python -c "import hashlib,json,pathlib,sys; r=pathlib.Path(sys.argv[1]); m=json.loads((r/'sha256-manifest.json').read_text()); assert all(hashlib.sha256((r/e['path']).read_bytes()).hexdigest()==e['sha256'] for e in m['files']); print('PASS')" $proof
```

Reuse the five already committed historical scripts without duplicating or editing
them in this evidence directory. Prepare a new disposable harness and regrade saved
answers with the current protocol, preserving all existing attempts:

```powershell
$frozen = Join-Path (Get-Location) 'plugins/memory/evidence/realistic-2026-10-23'
$scratch = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
$harness = Join-Path $scratch 'harness'
New-Item -ItemType Directory -Path $harness | Out-Null
'run_replay.py','rpc_support.py','answer_runner.py','grade.py','check_results.py' | ForEach-Object { Copy-Item -LiteralPath (Join-Path $frozen $_) -Destination $harness }
'corpus.json','protocol.json' | ForEach-Object { Copy-Item -LiteralPath (Join-Path $proof "harness/$_") -Destination $harness }
python (Join-Path $harness 'grade.py') --answers (Join-Path $proof 'answers-01') --replay (Join-Path $proof 'replay-01') --output (Join-Path $scratch 'assessment-recheck.json')
python (Join-Path $harness 'check_results.py') --assessment (Join-Path $scratch 'assessment-recheck.json') --output (Join-Path $scratch 'verdicts-recheck.json')
# The last command must return exit code 1, status blocked, critical case q09.
```

The saved `rawHashes`, `replayHashes` and `rawPath` use Windows separators relative
to `answers-01` and `replay-01`; normalize separators only when resolving or comparing
paths on another OS. Keep the saved bytes and hashes unchanged. JSON object comparison
of a regrade on another OS may require this path normalization.

For a new local replay, use a separate clean checkout of the bound source and the
new harness above. The exact recorded build, benchmark, artifact restoration and
replay command arguments are:

```powershell
cargo build --release --locked -p elegy-memory -p elegy-memory-mcp --bin elegy-memory --bin elegy-memory-mcp-stdio
cargo test --release --locked -p elegy-memory --test recall_performance -- --ignored --nocapture
cargo build --release --locked -p elegy-memory -p elegy-memory-mcp --bin elegy-memory --bin elegy-memory-mcp-stdio
$cli = Join-Path (Get-Location) 'target/release/elegy-memory.exe'
$mcp = Join-Path (Get-Location) 'target/release/elegy-memory-mcp-stdio.exe'
# Verify both artifact hashes against the table before using them as this subject.
python (Join-Path $harness 'run_replay.py') --cli $cli --mcp $mcp --output (Join-Path $scratch 'replay-new')
```

A fresh model campaign is a new experiment and cannot guarantee identical answers:
the hosted model alias is mutable and there is one realization per arm. The reusable
`answer_runner.configuration` and `answer_runner.invoke` functions, frozen prompt,
exact configuration, ordering and saved packets are retained. The historical runner's
`main` queries login status and lacks this campaign's stop-on-pending orchestration;
it is not an exact replacement for the omitted wrapper. No additional model call was made during preservation. Neither this baseline nor
the controlled overlay establishes readiness promotion, second-host qualification,
daily-use reliability, statistical generalization or monetary saving.
