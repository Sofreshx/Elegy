---
title: Controlled Memory prompt-overlay candidate experiment
status: active
owner: Elegy Memory
doc_kind: guide
---

# Controlled prompt-overlay candidate experiment

**The single candidate passed its nine predeclared criteria on this known corpus.**
This is a prompt-overlay experiment using 24 new with-only Codex sessions and zero
new without sessions. The 48 answers from the
[baseline before the field-policy fix](../README.md) remain unchanged; its no-recall
answers provide descriptive context only. There is no fresh paired causal A/B claim,
universal reliability claim, native runtime change or native-binary qualification.

The [predeclaration](predeclaration.json) was written with zero model calls and
before all 24 answer invocations. One 200-byte [candidate prefix](candidate.txt)
replaced the 202-byte [original prefix](original.txt). All record payload bytes,
IDs, versions, recalled contents and configured 600-unit budgets remained unchanged;
empty recall stayed empty. There was no retry, second candidate, model substitution,
threshold change or rubric change.

| Recorded result | Value |
| --- | ---: |
| New with-only sessions | 24 |
| New without sessions | 0 |
| Predeclared criteria passed | 9/9 |
| Frozen lexical mean | 0.9583333333 |
| Fully correct under the frozen lexical rubric | 23/24 |
| Complete in supplemental semantic review | 24/24 |
| Forbidden mentions | 0 |

The [verdict](verdict.json), [strict assessment](assessment.json) and
[supplemental semantic review](semantic-review.json) remain separate source files.
The review is grounded agent interpretation, not independent human certification.
q24 remains semantically complete and scores zero under the unchanged lexical
aliases. q08 supplies the 11-day retention; q09 treats the unspecified restore
identifier as unknown; the Harbor cases retain the correct project values.
Those observations support this candidate on this fixed sample only.

[All 24 new raw answers](answers-01/run.json) retain original JSONL, distinct session
IDs, usage, elapsed times, prompt/configuration digests and public invocation arguments.
The requested model is `gpt-5.6-luna`, effort `medium`, using the same installed Codex
executable and invocation configuration as the baseline. No new paid route was used.

[Source and overlay packets](source-and-overlay-packets.json) bind every copied
original packet to the preserved baseline episode manifest and its original hash.
They record original and overlay context hashes, the unchanged record-payload hash,
prefix hashes, byte lengths and budget. The candidate SHA-256 is
`c0b4adc6caff9c3fa8231c260efd14866e13fe4b760a78354d7195a10553e3c1`.

[Preservation provenance](preservation.json) records 34 lossless copies, with zero
path findings, zero derived files and zero omitted source files. The unchanged
[orchestration script](run_experiment.py) is provenance: it expects the original
`current-replay` sibling layout and imports the original campaign wrapper omitted
from the portable baseline. It cannot be run directly as a standalone artifact here.
No new wrapper or executable is introduced.

[Offline verification](verification.json) records 239 passing checks. It verifies
all 34 copied file hashes, all 48 unchanged baseline raw bindings, nine baseline
replay bindings, 24 original-packet and overlay transformations, 24 reconstructed
prompts and strict frozen scores, raw/review bindings, configuration identity,
distinct new sessions and predeclaration timing. The baseline gate remains blocked.
The predeclaration's `sourceNativeQualificationSha256` still binds the original
baseline summary; the original/derived mapping in
[baseline provenance](../preservation.json) resolves its machine-path-only public copy.

The [parent SHA-256 manifest](../sha256-manifest.json) includes every overlay file.
Use its documented offline verification command from the repository root. No replay,
model call, actual database access or private policy access occurred during preservation.
Native integration and freshly bound source verification are a separate later phase.
