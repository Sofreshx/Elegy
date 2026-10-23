---
title: Memory recall quality corrections
status: active
owner: Elegy Memory
doc_kind: planning
---

# Memory recall quality corrections

Goal: correct the retrieval failure found by the 23 October controlled session
qualification and test whether clearer evidence-use guidance reduces the observed
answer failures. The user authorized fixes, then push and merge only after clean
validation. This does not authorize installations, paid APIs, changes to real
memory, permanent host configuration, or claims of universal answer reliability.

Authority: root AGENTS → architecture README → contextual recall v1 spec →
Memory AGENTS → CONTRIBUTING validation matrix. Existing qualification branch
`codex/memory-realistic-20261023` is preserved; this correction branch starts at
`619f5dd705e6383a12b7d806369633df291fc1a3`.

## Evidence and design

- q17/q18 inject the short Harbor-Sandbox record although the applicable Harbor
  record exists. OR FTS matching and relative BM25 normalization favor the shorter
  near-name record. Scope, owner and sensitivity filters work; there is no
  structured project boundary in this binding.
- q08/q11 omit facts that are actually recalled. q09 substitutes a runbook name
  for an unknown snapshot identifier. These are model answer failures, not missing
  database rows.
- Keep the SQL access filters, read-only source, separate journal, three-record
  limit, complete-envelope byte budget, event/feedback contracts and JSON shape.
- For contextual lexical retrieval only, preserve hyphenated surface tokens and
  rank by distinct exact query-token matches in the text actually delivered,
  normalized to the highest eligible lexical match count. Ordinary search and vector
  scoring remain as before. This is a relevance heuristic, not project isolation.
- Make the fixed envelope explicitly require facts for the same context and
  requested attribute, abstention when missing, and priority for current
  instructions. Keep the instruction/data boundary and full byte accounting.

The lexical candidate set remains OR FTS with the existing SQL access filters.
The new lexical signal also participates in hybrid recall; the vector channel
and its blending remain unchanged. Normalize over keyword candidates after SQL
scope/owner/state/sensitivity predicates and before similarity floor, journal
suppression and byte budget. Zero exact matches yields zero lexical signal.
Equal counts keep the existing secondary ranking/tie rules. A single candidate
can still score 1: this remains relative relevance, never confidence or security.

Surface tokens retain ASCII internal hyphens and underscores, trim boundary
hyphens, lowercase with Rust Unicode casing and preserve accents. Other
punctuation separates tokens. `Atlas-Lab` is one token, distinct from `Atlas`
and from the spaced form `Atlas Lab`; no inferred alias or fuzzy project match.
Count distinct query terms once, regardless of repetitions. Use the same shorter
nonempty summary-or-content rule as delivery. Content and tags still generate
FTS candidates, but matches absent from the delivered snippet get no lexical
credit. Tests must cover summary and tag-only candidates, case, duplicates,
empty lexical matches, hybrid access controls and full-envelope byte accounting.
The current stopword list, minimum of three characters and cap of twelve unique
query terms remain in force. Ordinary search and vector-only scores need an
old/new control on the same synthetic rows; hybrid output may change through its
lexical component, with ownership/scope filters still mandatory. Compare q08/q11
recalled content and order mechanically before interpreting answer differences.

## Work and acceptance

- [ ] Add integration regressions first: Harbor q17/q18, explicit Sandbox query,
  independent Atlas/Atlas-Lab pair, unchanged source. Observe the intended red
  result before changing production code.
- [ ] Review the plan, then implement the smallest correction and align the
  existing recall spec. No database migration or new provider/dependency.
- [ ] Run focused regressions, complete Memory and MCP suites, hook tests,
  formatting, Clippy, governed contracts and documentation checks. Preserve all
  failures; attribute base/environment failures before changing unrelated code.
- [ ] Commit exact source before qualifying new executables; record source and
  binary hashes. Use isolated fictitious stores and fresh model contexts only.
- [ ] Reuse the frozen 24-question corpus and rubric. Compare exact old/new recall
  packets. For answer quality, retain all trials: three realizations of q08, q09,
  q11, q17 and q18, one realization of other controls. No prompt tuning after
  seeing these answers and no retry that replaces an adverse result.
- [ ] Each of the five problematic cases must pass individually on every retained
  trial. There is no three-of-five threshold. q08/q11 already have the correct
  recalled text; any improvement there can only be attributed to the answer
  guidance/model behavior, not repaired candidate generation.
- [ ] Report lexical score separately from semantic reading and each unsupported
  answer/omission. No new contamination, secret leak or hostile instruction use;
  explicitly distinguish passed finite controls from general reliability.
- [ ] Inspect exact final diff, independent review, private-content/metadata gates,
  publication effects, remote target and CI tied to the actual head. Push the
  task branch and merge only if applicable gates are satisfied; verify remote SHA.

Stop and reassess if data outside scratch is needed, paid capacity or installation
is required, a source changes during qualification, or model failures persist.
An unavailable second software host stays blocked. Green deterministic tests do
not turn failed answer controls into a passing autonomous-use claim.

Rollback: revert the isolated correction commits; keep previous proofs and failed
trials. No published-history rewriting or force push.
