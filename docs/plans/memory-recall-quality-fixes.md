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

- [x] Add integration regressions first: Harbor q17/q18, explicit Sandbox query,
  independent Atlas/Atlas-Lab pair, unchanged source. Observe the intended red
  result before changing production code.
- [x] Review the plan, then implement the smallest correction and align the
  existing recall spec. No database migration or new provider/dependency.
- [x] Run focused regressions, complete Memory and MCP suites, hook tests,
  formatting, Clippy, governed contracts and documentation checks. Preserve all
  failures; attribute base/environment failures before changing unrelated code.
- [x] Commit exact source before qualifying new executables; record source and
  binary hashes. Use isolated fictitious stores and fresh model contexts only.
- [x] Reuse the frozen 24-question corpus and rubric. Compare exact old/new recall
  packets. For answer quality, retain all trials: three realizations of q08, q09,
  q11, q17 and q18, one realization of other controls. No prompt tuning after
  seeing these answers and no retry that replaces an adverse result.
- [ ] Each of the five problematic cases must pass individually on every retained
  trial. There is no three-of-five threshold. q08/q11 already have the correct
  recalled text; any improvement there can only be attributed to the answer
  guidance/model behavior, not repaired candidate generation.
- [x] Report lexical score separately from semantic reading and each unsupported
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

## Qualification checkpoint — 23 October

**Blocked for push and merge.** Product source `b828223` fixes the Harbor
selection regression, and q17/q18 each pass all three retained answer trials.
The frozen acceptance criterion fails for q08, q09 and q11: one retention
omission, three runbook/snapshot substitutions and two time omissions across
the 15 target answers. Do not retune this corpus or discard adverse trials.
The guidance change is not qualified as a remedy for those model failures.

The full Memory suite passes (408 tests, including 31 contextual recall tests),
as do all 67 MCP tests, 14 hook tests, 14 evidence tests, Clippy, formatting,
contracts, documentation checks and the CLI/MCP build. The actual second
software host preflight is blocked by HTTP 403 from its advertised free provider;
no paid fallback or permanent configuration was introduced.

See [the correction qualification report](../../plugins/memory/evidence/recall-fixes-2026-10-23/README.md)
and its individual answers, source/binary bindings and reproducible adverse gate.
Safe local checkpoints remain authorized; the user's conditional publication
authorization has not been satisfied.

## Resumed correction — evidence-use envelope

The user now explicitly asks to resolve the three answer failures and then push
and merge after all applicable checks pass. The second software host is excluded
by that same user's direction; preserve its historical result without making it
a gate for this resumed slice.

The prior source and all failed answers remain immutable. Correct facts were
transmitted in q08/q11; q09's requested snapshot identifier was absent. A fresh
controlled comparison will vary only the fixed envelope prefix: require matching
context and field, extracting all requested values and never substituting a
different field's value. This is a testable hypothesis, not yet an accepted fix.
The candidate is 197 bytes, within the prior envelope's budget, and contains no
corpus labels or answers. No product source is changed before the comparison.

1. Freeze the diagnostic protocol and run 30 fresh contexts: five cases, three
   trials, current versus candidate prefix with identical JSON records.
2. Require all fifteen candidate answers to pass and at least one current
   control failure; retain every trial and inspect actual prose independently.
3. If supported, apply the single envelope change, align the existing spec,
   run focused and full checks, and commit exact source before new binaries.
4. Qualify actual rebuilt outputs with the unchanged 24 paired questions, three
   realizations of all five critical cases, and twelve frozen transfer questions
   using new fictional labels and values. The transfer sample is synthetic,
   structurally related coverage, not an independent real-world benchmark.
5. Require each critical answer in all three trials, all transfer with-recall
   answers and all safety/abstention/override controls to pass. No test weakening,
   answer replacement, model substitution or economic claim.
6. Verify source/binary/evidence bindings, independent review, publication gates
   and exact-head CI before push/merge. Existing main-snapshot automation is
   already disclosed and authorized by the requested merge.

Diagnostic and final acceptance remain separate. If the candidate fails, use
the retained errors to reassess the mechanism rather than promoting simulated
packets or repeating trials until they pass. All execution stays in disposable
stores, journals and model contexts; existing ChatGPT quota only.

### Diagnostic decisions retained

The generic 197-byte wording hypothesis is rejected: both conditions passed
10/15 lexical checks, with actual q08 omissions and q09 substitutions. Changing
record order also failed; removing metadata is an ineligible ablation and did
not reliably recover the corrected values. Preserve every answer and the
semantic exception for the valid `14h50` spelling in the layout review.

The next predeclared diagnostic separates two observed ambiguities in a 2x2
design: historical data versus latest stored versions, and generic field
substitution versus method-name/instance-ID confusion. Only the fixed prefix
varies; JSON record bodies remain byte-identical. The proposed latest/typed
prefix is 202 bytes, the same length as the current product prefix, with no
corpus labels or answer values. "Latest stored" describes row version selection
inside the source snapshot, never independently verified real-world freshness.

Run all 44 declared contexts once and retain each result. Acceptance requires
all eleven proposed-prefix answers and independent prose review. Report both
factors separately, including adverse interactions; these simulations never
replace qualification against a rebuilt executable. Before that final campaign,
make the transfer corpus executable without editing either frozen corpus and
require per-answer prose verdicts alongside the unchanged lexical rubric.

### Resumed diagnostic result and target decision

All 129 prospective diagnostic contexts completed without execution/provider
errors: wording 30, layout 33, semantic factors 44, paragraph placement 22.
None supports a production fix for the retained response failures. The latest/
typed prefix passes 8/11 prose checks; separate paragraph placement passes 6/11
under strict review, including one ambiguous time answer counted as failure.
Every trial remains available, including false lexical negatives for `14 h 50`
and a lexical false positive that mentions the time but disclaims an updated one.

No new Rust behavior was changed. The validated ranking correction and its
408 Memory / 67 MCP tests remain the product source under test. Do not rerun
those unchanged suites merely to replace the adverse response result.

The installed normal Codex configuration is `gpt-6.1-sol` / `max`, whereas this
controlled answer proxy uses `gpt-5.6-luna` / `medium`. This is an observed target
mismatch, not proof that the configured model would succeed. A user clarification
is pending: qualify the actual configured host/model in a separate prospective
cohort while keeping Luna failures, or require Luna too and design a different
caller/answer-validation boundary. Do not silently substitute models or weaken
the original acceptance. Push and merge remain conditional and unperformed.
