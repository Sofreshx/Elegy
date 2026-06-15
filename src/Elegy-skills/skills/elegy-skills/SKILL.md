---
name: elegy-skills
description: Use when an agent needs to search, resolve, get, validate, or inspect capabilities across the governed skill registry through the dedicated elegy-skills CLI or the umbrella elegy skills compatibility surface.
---

# Elegy-skills Surface Bridge

This file is the surface-local, non-authoritative skill bridge shipped
with the `src/Elegy-skills` wrapper surface and the
`elegy-skills-wrapper-<bundleVersion>.zip` archive.

Authority stays one-way:

1. `contracts/fixtures/skill.elegy-skills.json` is the governed source
   of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-skills.json` is the
   governed discovery projection.
3. `skills/elegy-skills/SKILL.md` is the canonical operational body.
4. This file mirrors install and CLI handoff.

## Wrapper install

- Run `./install.ps1` from this wrapper root.
- Pass `-LocalArtifactsRoot <path>` for local archives.

## Current commands

```text
elegy-skills list [--category <name>] [--lifecycle <state>] [--detail] --json
elegy-skills search --query <task> --json
elegy-skills resolve --query <task> --json
elegy-skills get --skill-id <id-or-alias> --json
elegy-skills capability --capability-id <id> --json
elegy-skills validate --file <path> --json
elegy-skills validate --dir <path> --json
```

## Surface posture

- Registry-first: list, search, resolve, get, capability, and
  validate governed skills.
- The same behavior is mirrored by the umbrella `elegy skills ...`
  commands.
- For the full guardrails, common issues, and examples, load the
  canonical body: `../../../skills/elegy-skills/SKILL.md`.
