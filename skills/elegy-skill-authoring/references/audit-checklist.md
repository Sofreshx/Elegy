# Audit Checklist

Objective checks for an Elegy SKILL.md body. Walk top to bottom; record a
finding for every check that fails. Severity comes from `severity-ladder.md`.

## Required sections

A passing skill must contain every heading below, in this order, with
non-empty content. Section order is normative.

1. `# <Skill name>` — top-level H1 matching the frontmatter `name`.
2. `> One-line trigger description` — blockquote, identical to the
   frontmatter `description`.
3. `## Quick start` — at least 3 numbered steps, each one tool call.
4. `## Tool-call guardrails` — at least one `### <family>` sub-section
   per capability family declared in the governed fixture.
5. `## Workflow` — at least 3 numbered steps with at least one
   decision branch.
6. `## Capability index` — Markdown table with columns
   `id | side-effect | purpose`, one row per capability id in the
   governed fixture.
7. `## Output envelope` — at least the envelope name/version, the
   `data` / `rawOutput` semantics, and the error shape.
8. `## Common issues` — Markdown table with columns
   `Symptom | Cause | Solution`, at least 3 rows.
9. `## Version compatibility` — at least the minimum supported CLI
   version and the semver rule.
10. `## Examples` — at least 2 worked examples with literal expected
    stdout or JSON.
11. `## Boundaries` — what the skill owns, does not own, and companion
    skills.
12. `## References` — at minimum the governed source path and the
    discovery projection path.

## Frontmatter

- `name` is present, lowercase, matches `^[a-z][a-z0-9-]*$`, and equals
  `identity.name` in the governed fixture.
- `description` is present, ≤ 200 characters, starts with "Use when",
  and is a single sentence.

## Quick start quality

- Each step names a real capability id or a real CLI command — not "set
  up the environment" or "configure the system".
- Each step fits on a small number of lines; no step contains a nested
  numbered list.
- The first 3 steps cover the most common path. Do not bury the
  canonical flow under setup steps.

## Tool-call guardrails quality

- One `### <family>` sub-section per capability family. Families are
  grouped by side-effect class or by argument shape, at author
  discretion; document the grouping rule in a single line at the top of
  the section.
- Each sub-section covers argument shape, fetch-before-mutate (if any
  mutation exists in the family), a literal "Do not" anti-pattern,
  side-effect class, and approval posture.
- The "Do not" anti-patterns are real failures, not stylistic
  preferences. "Do not write in the second person" is not a finding;
  "Do not pass `obsidian-fetch` a connected-source URL" is.

## Workflow quality

- At least one step contains an explicit `If <condition>, <branch>`
  decision.
- The workflow ends with either a handoff to another skill or a
  completion signal. Workflows that just "loop back to step 1" are a
  Medium finding.

## Capability index quality

- Row count equals the count of `capabilities[].id` in the governed
  fixture, in declaration order.
- Every `side-effect` cell is one of: `read-only`, `disk_write`,
  `desktop_ui`, `process_spawn`, `network`, `secrets`, or
  `unspecified`. Free-form values are a Medium finding.
- Every row's `purpose` fits in one line and starts with a verb.

## Output envelope quality

- The envelope name and version are present and match the value used
  by the runtime, if any.
- `data` vs `rawOutput` semantics are documented. If only one is
  populated in practice, say so explicitly.
- The error shape names the JSON field(s) the agent should parse first.

## Common issues quality

- At least 3 rows.
- Every row's `Symptom` is an observable error or behavior, not an
  internal state.
- Every row's `Solution` is a concrete action, not "investigate" or
  "check the docs".
- The first row covers the failure the author hit most recently. This
  rule exists to keep the section honest.

## Version compatibility quality

- A specific minimum CLI version is named (e.g. `≥ 1.8.0`), not "the
  latest".
- If the skill depends on an external binary (e.g. `obsidian`), the
  minimum external version is also named.
- The semver rule states what the agent must check before invoking
  (e.g. "minor must be ≥ Y, patch is unconstrained").

## Examples quality

- At least 2 worked examples.
- Each example has a literal command and a literal expected output.
  No "approximately", "depends on", or "varies by environment".
- Expected output is captured from a real local run during authoring.
  If the author cannot run the command, the example is a Critical
  finding until the run is performed.

## Boundaries quality

- "This skill owns" lists only what the skill is authoritative for,
  not what it touches.
- "This skill does not own" lists at least one item the user might
  mistakenly route here.
- "Companion skills" lists at least one related skill with a one-line
  purpose.

## References quality

- The governed source path is present and points to a file that
  actually exists at the repo root.
- The discovery projection path is present and points to a file that
  actually exists.
- Any external link has a real `path` to a file in the repo; do not
  cite bare URLs without a local pointer.
