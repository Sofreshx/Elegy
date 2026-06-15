---
name: elegy-skill-authoring
description: Derived repo-local skill bridge mirror for the Elegy skill authoring doctrine skill. Use when drafting, auditing, or fixing Elegy SKILL.md bodies against the canonical template, audit checklist, and severity ladder.
---

# Elegy Skill Authoring

This file is a repo-local, non-authoritative rendered skill bridge mirror
for the `elegy-skill-authoring` doctrine skill.

The authority chain is one-way:

1. `contracts/fixtures/skill.elegy-skill-authoring.json` is the governed
   source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-skill-authoring.json`
   is the governed discovery projection derived from that definition.
3. `.agents/skills/elegy-skill-authoring/SKILL.md` and
   `.github/skills/elegy-skill-authoring/SKILL.md` are repo-local
   rendered mirrors only.
4. The canonical content lives in `skills/elegy-skill-authoring/`
   (SKILL.md plus `assets/`, `references/`, `evals/`).

## What this skill owns

- The canonical SKILL.md skeleton: `assets/skill-template.md`.
- The audit checklist: `references/audit-checklist.md`.
- The anti-pattern catalogue: `references/anti-patterns.md`.
- The severity ladder: `references/severity-ladder.md`.
- Eval scenarios for the future `elegy skill audit` CLI:
  `evals/scenarios.yaml`.

## What this skill does not own

- Governed fixture shape (the schema is authority for that).
- Agent-host projection metadata (the host's projection logic is
  authority).
- MCP tool registration (the host owns that).

## Surface posture

- This is a doctrine skill. Its capabilities resolve to the `elegy` umbrella
  CLI. The `elegy skill audit`, `elegy skill anti-pattern`, and
  `elegy skill template` subcommands are planned but not yet shipped.
  Until they land, audit and anti-pattern lookup are manual processes
  against the in-tree references (`references/audit-checklist.md`,
  `references/anti-patterns.md`). There is no dedicated
  `elegy-skill-authoring` binary.
- Sibling doctrine: `elegy-doc-practices` covers documentation
  classification, placement, and review.
- Companion: `elegy-skills` covers registry lookup, validation, and
  capability resolution.

## References

- `skills/elegy-skill-authoring/SKILL.md` — canonical body.
- `skills/elegy-skill-authoring/assets/skill-template.md` — skeleton.
- `skills/elegy-skill-authoring/references/audit-checklist.md` — checks.
- `skills/elegy-skill-authoring/references/anti-patterns.md` — AP-1..AP-10.
- `skills/elegy-skill-authoring/references/severity-ladder.md` — severity.
- `skills/elegy-skill-authoring/evals/scenarios.yaml` — eval scenarios.
- `contracts/fixtures/skill.elegy-skill-authoring.json` — governed source.
- `contracts/fixtures/skill-discovery-index.elegy-skill-authoring.json` —
  discovery projection.
