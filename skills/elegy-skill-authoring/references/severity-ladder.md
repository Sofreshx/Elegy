# Severity Ladder

Use these definitions when writing audit findings. The four-level scale
is intentionally coarse so triage is fast.

## Critical

The skill actively misleads the agent or causes data loss.

- The Quick start leads the agent to a destructive action without a
  precondition check.
- The Tool-call guardrails invent a command that does not exist.
- The Capability index table misrepresents side-effect class for a
  mutating capability, hiding that the call writes to disk.
- The Examples claim an output the command does not produce.
- The Workflow is unreachable because the first step requires a
  tool the skill does not own.

Critical findings must be fixed in the same change. Do not defer.

## High

The skill will silently produce wrong results, or it omits content the
agent needs to avoid a known failure.

- Common issues is missing or has fewer than 3 rows.
- Examples are missing, have fewer than 2 worked examples, or use
  fuzzy "depends on environment" language.
- Tool-call guardrails lacks fetch-before-mutate for a mutation family.
- Capability index table is missing rows for capabilities that exist
  in the governed fixture.
- Version compatibility does not name a minimum version.
- The description is longer than 200 characters or does not start with
  "Use when".

High findings should be fixed in the same change. If a deferral is
unavoidable, the change description must call it out explicitly.

## Medium

The skill is correct but inconsistent with the canonical template in
ways that hurt agents that index on section order or naming.

- Section order deviates from the template.
- "Do not" anti-patterns are stylistic preferences rather than real
  foot-guns.
- A capability family has no sub-section in Tool-call guardrails.
- A row in the Common issues table is generic ("check the logs")
  rather than specific.
- Mirror lanes disagree on non-substantive content (comments, blank
  lines).

Medium findings can be deferred if explicitly triaged.

## Low

Cosmetic or stylistic issues. Track but do not block on them.

- Description could be tightened by a few words.
- A heading is not in title case.
- An example uses a non-canonical emoji policy.
- A reference link is one character off (trailing slash, missing
  anchor).

Low findings are tracked in the audit report and may be batched.
