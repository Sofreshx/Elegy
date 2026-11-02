---
title: Final 202-byte production runtime provider-free qualification
status: active
owner: Elegy Memory
doc_kind: guide
---

# Final production runtime: provider-free checks passed

**The final runtime passed its declared provider-free mechanical checks.** It uses
the restored 202-byte header at clean source
`5c246906c7edf7fb876901b610d1de8352307715`, package version `0.1.0`.
There were **zero model, provider or network calls**, with Cargo offline enforced.
This is runtime-mechanics evidence; answer quality, external host operation,
real installation, remote connectivity and readiness promotion are not certified.

The [200-byte native candidate](../native-final/README.md) remains **REFUTED**:
23/24 semantically complete answers, required 24/24. Its failed q19 answer and all
prior baseline/overlay results remain unchanged. No new model trial was run to
replace that failure. Historical references to this runtime phase as pending
describe its earlier capture state; this directory records the completed phase.

[Clean-head binding](clean-head-binding.json), [source snapshot](release-source-before.json),
[release build](release-build.json) and [qualification summary](qualification-summary.json)
bind 301 source inputs that remained unchanged through the recorded endpoint.
The tracked checkout and source HEAD were clean at the recorded phase checks,
and pinned executable hashes matched at the end. Build duration was 96.104 seconds.
The earlier 499 passing tests, one ignored test, formatting and Clippy pass are
reported by the coordinator and bound to their summary hash in the clean-head
receipt; the preservation worker did not rerun those suites.

| Pinned artifact, excluded from this portable directory | SHA-256 |
| --- | --- |
| `elegy-memory.exe` | `989c537e201cbc9cabfd69af416b002a4cf00cb777fa3da61a5455a4cf41795d` |
| `elegy-memory-mcp-stdio.exe` | `ac0e39fcabcbd6faba96b9611ae6d6168eca748a995fe3623bc2654221e912ef` |

[Release benchmark](release-benchmark.json): 1,005 synthetic rows, five warmups,
100 measured calls and 105 expected selections; p50 255.4061 ms, p95 345.9311 ms,
maximum 412.3922 ms. Source database bytes, WAL and counters remained unchanged.
The measured p95 is below 500 ms for this local lexical workload. It excludes
model latency, providers, embeddings, host startup and production databases.

[Native replay](replay-01/run.json) completed eight episodes and 24 packets, with
23 nonempty contexts containing the native 202-byte prefix, unchanged durable
source data for all packets and all 16 cleanup namespaces empty. No overlay was
applied. The [replay summary](replay-summary.json) retains the original producer
metadata, subject to the prefix-hash clarification below.

Two fresh local claims, `recall.contract` and `forgetting.retention`, are satisfied
in the unchanged [ledger](local-claims/ledger.json) and its original receipt wrappers.
The [claim summary](local-claim-summary.json) records all three external claims as
`unverified`: cross-session host operation, agent isolation in an installed host,
and answer quality. History is retained in
[local-claim-history.json](local-claim-history.json). The path-bearing status/next
outputs remain hash-bound omissions; their safe canonical statuses are preserved
in the claim summary. No actual Memory data, configuration, installation, key or
tunnel was changed.

## Producer prefix-hash metadata clarification

The logical native prefix is **202 UTF-8 bytes**, SHA-256
`b4d6645648f1a4874c27c10e4c6622ee81e7fbef0452d2c18e8a98f0588e5a43`.
It matches the predeclaration, protocol and all 23 nonempty native contexts.
The unchanged [expected-prefix.txt](expected-prefix.txt) is a **203-byte CRLF file**,
SHA-256 `a288d6c38b27b6db49befcc1be34ba15b7604233100ba07506380564f8bfc5b0`.
Reading that file as UTF-8 text with CRLF-to-LF normalization yields exactly the
202-byte logical prefix; no source file was rewritten during preservation.

**Producer metadata erratum:** `replay-summary.originalPrefixSha256`, also embedded
in `qualification-summary.nativeReplay`, binds the raw 203-byte file rather than
the 202-byte native prefix suggested by its name. Its original value is retained.
The protocol/predeclaration and direct packet checks correctly bind the logical
202 bytes. This is a metadata naming issue, with no native behavior error or
weakened payload assertion. [Preservation provenance](preservation.json) and
[offline verification](verification.json) record both meanings explicitly.

## Portable preservation

37 source files were copied byte for byte, with zero derived files. Thirteen
machine-path-bearing auxiliary scripts, command records, status/next outputs and
build logs were omitted with original hashes and reasons; their originals remain
in scratch. Two unchanged historical replay scripts are reused from
[the frozen campaign](../../realistic-2026-10-23/README.md), rather than duplicated.
The [original source manifest](source-sha256-manifest.json) retains all 51 source
bindings, including omitted and reused inputs. No binary, database/WAL state,
runtime/client store, private configuration or credential was copied.

[Offline verification](verification.json) records 409 passing checks, including all
37 source/destination hashes, the 51-entry original manifest, both ledger/payload
chains, source/build/protocol/prefix bindings, packet/source invariants, cleanup,
benchmark limits, zero-call scope and 259 unchanged earlier source copies.
The [parent manifest](../sha256-manifest.json) covers every portable file. Its
README contains the manifest verification command. Byte-preserving Git attributes
also accept CR at end of original lines without disabling other whitespace checks.

No new qualification command, model call, actual database access, private-policy
read, Git mutation or publication was performed by the preservation worker. The
structural path check is separate from the coordinator's publication exclusions.
