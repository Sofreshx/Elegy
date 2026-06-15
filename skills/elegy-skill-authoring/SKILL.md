---
name: elegy-skill-authoring
description: Use when drafting a new Elegy skill, auditing an existing one against the canonical template, or fixing structural and content gaps. Covers section order, required-vs-optional sections, tool-call guardrails, common-issue tables, version compatibility, and severity ranking for findings.
---

# Elegy Skill Authoring

This skill is doctrine for **writing, auditing, and fixing Elegy skills**. It
defines the canonical SKILL.md structure, the audit checklist, the anti-pattern
catalogue, and the severity ladder for findings. It does not own a CLI yet;
audit runs are manual against the checklist in `references/audit-checklist.md`
until the future `elegy skill audit` command lands.

## Quick start

1. Pick the surface. Confirm the governed fixture path
   `contracts/fixtures/skill.<surface>.json` and the discovery projection
   `contracts/fixtures/skill-discovery-index.<surface>.json` exist or will be
   added in the same change.
2. Copy `assets/skill-template.md` to the new SKILL.md location (mirror lane).
3. Fill every required section in order. The required set is enforced by
   `references/audit-checklist.md`.
4. For each capability family, write a Tool-call guardrails sub-section. If
   the family is read-only, document fetch-before-mutate even if no mutation
   exists yet — that signals the future mutation contract.
5. Add at least one Common issues row per real pitfall you have personally
   hit or seen in a review. Empty Common issues is an automatic High
   finding.
6. Add at least two Examples with literal expected stdout or JSON. Generic
   "run the command and see what happens" is a Medium finding.
7. Run the audit: walk `references/audit-checklist.md` top to bottom and
   record findings with severity from `references/severity-ladder.md`.
8. Fix every Critical and High finding before opening the change. Medium and
   Low can land in a follow-up if explicitly noted in the change description.

## Workflow

### Drafting a new skill

1. Confirm scope.
   - If the skill replaces or duplicates an existing one, stop and update the
     existing skill instead. Do not add a sibling that does the same job.
   - If the skill needs new governed fixtures, draft the fixture first and
     validate it against `contracts/schemas/skill.schema.json` before writing
     prose.
2. Use the template. Copy `assets/skill-template.md` verbatim; replace
   every `<placeholder>`; do not reorder sections. Section order is
   normative because agents index on it.
3. Frontmatter rules.
   - `name` is the canonical skill id and must match
     `identity.name` in the governed fixture.
   - `description` is the trigger text. Keep it under 200 characters. It
     must read as one sentence, start with "Use when", and include the main
     verb the agent will see in a user request.
4. Capability index table must list every capability id present in the
   governed fixture. Extra rows with no backing capability are a Medium
   finding. Missing rows are a High finding.
5. Validate. Run the contract validator over the fixture:
   `elegy-skills validate --file contracts/fixtures/skill.<surface>.json --json`.
   Run the docs check if the change adds new docs: `elegy-documentation
   check --project .`.

### Auditing an existing skill

1. Read the governed fixture first. The SKILL.md is a derived view; the
   fixture is authority.
2. Open `references/audit-checklist.md` and walk every check.
3. For each finding, assign a severity from `references/severity-ladder.md`
   and capture: skill id, section, evidence (line excerpt), and a concrete
   fix.
4. Hand the findings back as a triage list, not as raw prose. Group
   Critical and High first.

### Fixing a skill

1. Read the audit findings.
2. For each finding, edit only the affected section. Do not rewrite the
   whole file unless multiple Critical findings make a rewrite cheaper.
3. Re-run the audit.
4. If the change touches a mirror lane, update all mirrors to the same
   content. Mirrors are non-authoritative but must not drift.

## Tool-call guardrails

### Writing the audit report

- Use one finding per row. Do not bundle multiple issues under one
  "Section X is weak" entry.
- Cite evidence as `path:line` or `path#heading-anchor`, never as a
  paragraph description.
- Severity must come from `references/severity-ladder.md`. Do not invent
  custom severities.
- Do not mark a finding fixed without re-running the audit and showing the
  checklist now passes for that check.

### Editing a skill body

- Preserve section order. Reordering breaks agents that index on
  `## Quick start` being first.
- Do not collapse `## Tool-call guardrails` into a single bullet. The
  per-family sub-section structure is required.
- Do not move Common issues to a separate file. It must live in the
  SKILL.md body so a single file read gives the agent every failure mode.
- Do not remove `## Examples` to save space. Examples are the most-skipped
  and most-valuable section per the audit.

## Capability index

| id | side-effect | purpose |
| -- | -- | -- |
| `skill-audit-checklist` | read-only | Walk the objective audit checks over a skill body or fixture and produce a findings list. |
| `skill-anti-pattern-lookup` | read-only | Look up the canonical anti-pattern catalogue and the matching severity. |
| `skill-template-emit` | read-only | Emit the canonical SKILL.md skeleton with placeholders preserved. |

These capabilities are doctrinal. There is no Rust implementation yet; agents
read the references directly. The future `elegy skill audit` command will
back `skill-audit-checklist` and `skill-anti-pattern-lookup` with machine
checks.

## Output envelope

- Envelope: `skill-audit-finding/v1` (planned).
- For now, audit reports are Markdown findings lists with the shape:
  - Heading per finding: `### [<SEVERITY>] <short title>`.
  - Required body lines: `Skill: <id>`, `Section: <heading>`, `Evidence:
    <path:line or excerpt>`, `Fix: <concrete action>`.
- Findings are sorted by severity, then by section, then by line.

## Common issues

| Symptom | Cause | Solution |
| -- | -- | -- |
| SKILL.md describes the skill but never says how to invoke it. | Author treated the body as documentation, not as a recipe. | Add a 3-5 step Quick start with exact `${parameter}` placeholders. |
| Tool-call guardrails is a single bullet list, not per-family sub-sections. | Author did not read the template. | Re-emit the section from `assets/skill-template.md` and add one sub-section per capability family. |
| Common issues is empty or absent. | Author did not have a failure history yet. | Mine recent PR reviews, support threads, and "agent did the wrong thing" reports for at least 3 real pitfalls. Empty Common issues is a High finding. |
| Examples say "expected output depends on your environment" or similar. | Author skipped the verification step. | Run the command locally, capture the literal stdout, paste it. Generic Examples are a Medium finding. |
| Capability index table is missing rows that exist in the governed fixture. | Author wrote the table before the fixture was finalized. | Re-emit the table from the fixture's `capabilities[].id` list, in declaration order. |
| Mirror lanes drift from the source SKILL.md. | Mirrors were treated as install pages and edited by hand over time. | Pick one mirror as canonical, regenerate the others from it. The mirror rule in `docs/architecture/agent-skill-bridge-mirrors.md` already requires this. |
| Description is a marketing paragraph longer than 200 characters. | Author optimized for humans reading the registry, not for agent triggers. | Rewrite to one sentence starting with "Use when" that names the main verb and the main object. |

## Version compatibility

- Minimum supported CLI version: aligned with the current
  `elegy-skills` and `elegy` umbrella. Confirm via
  `elegy --version` before editing fixtures; the validator rejects
  fixtures against incompatible schemas.
- The audit checklist is the contract. When the checklist changes, the
  skill is automatically considered updated, but re-audit every existing
  skill in the same change.

## Examples

### Example 1 — drafting a new skill from the template

```text
cp skills/elegy-skill-authoring/assets/skill-template.md skills/<new-skill>/SKILL.md
```

Expected output: a fresh skeleton at the new path with every required
section heading present and empty. Fill in the `<placeholder>`s; do not
delete section headings.

### Example 2 — running the audit manually

```text
# Read the audit checklist
open skills/elegy-skill-authoring/references/audit-checklist.md

# Read the skill under review
open skills/<skill>/SKILL.md

# Walk every check; record findings as
### [<SEVERITY>] <short title>
Skill: <id>
Section: <heading>
Evidence: <path:line>
Fix: <concrete action>
```

Expected output: a triage list grouped by severity, ready to hand to the
author.

## Boundaries

- This skill owns: SKILL.md structure, audit checklist, anti-pattern
  catalogue, severity ladder.
- This skill does not own: governed fixture shape (`contracts/` is
  authority for that), agent-host projection metadata, or MCP tool
  registration.
- Companion skills:
  - `elegy-doc-practices` — sibling for documentation, not for skills.
  - `elegy-skills` — registry lookup, validation, and capability
    resolution. Use this skill for skill bodies; use `elegy-skills` for
    registry operations.

## References

- `assets/skill-template.md` — canonical SKILL.md skeleton.
- `references/audit-checklist.md` — objective audit checks.
- `references/anti-patterns.md` — anti-pattern catalogue.
- `references/severity-ladder.md` — severity definitions.
- `evals/scenarios.yaml` — audit scenarios for future CLI work.
- Governed source: `contracts/fixtures/skill.elegy-skill-authoring.json`.
- Discovery projection: `contracts/fixtures/skill-discovery-index.elegy-skill-authoring.json`.
- Related doctrine: `docs/architecture/agent-skill-bridge-mirrors.md` —
  mirror lane split.
