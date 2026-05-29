---
name: elegy-skills
description: "Surface-local non-authoritative bridge shipped with the Elegy-skills wrapper surface and wrapper archive."
---

# Elegy-skills Surface Bridge

This file is a surface-local, non-authoritative skill bridge shipped with the `src/Elegy-skills` wrapper surface and the `elegy-skills-wrapper-<bundleVersion>.zip` archive.

Authority stays one-way:

1. `contracts/fixtures/skill-definition-v2.elegy-skills.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-skills.json` is the governed discovery projection.
3. `.agents/skills/elegy-skills/SKILL.md` and `.github/skills/elegy-skills/SKILL.md` remain repo-local contributor-routing outputs.
4. This file mirrors the install and CLI handoff needed by wrapper consumers.

## Wrapper install

- Run `./install.ps1` from this wrapper root to stage the contracts bundle, the `elegy-skills` CLI surface, and this wrapper surface together.
- Pass `-LocalArtifactsRoot <path>` when validating against local archives instead of GitHub release assets.

## Current commands

```text
elegy-skills list [--category <name>] [--lifecycle <state>] [--detail]
elegy-skills search --query <task>
elegy-skills resolve --query <task>
elegy-skills get --skill-id <id-or-alias>
elegy-skills capability --capability-id <id>
elegy-skills validate --file <path>
elegy-skills validate --dir <path>
```

`--json` or `--format json` is available when structured output is needed, depending on the command surface version.

## Surface posture

- This dedicated surface is registry-first: list, search, resolve, get, capability, and validate governed v2 skills.
- The same registry behavior is also available on the umbrella `elegy skills ...` commands.
- Downstream Rust hosts can prefer the reusable `rust/crates/elegy-skills` API over shelling out when direct in-process integration is a better fit.
