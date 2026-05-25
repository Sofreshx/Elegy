---
name: elegy-skills
description: "Derived repo-local skill bridge mirror for Elegy's current dedicated skill-registry surface. Use for governed skill search, resolve, inspect, and validation through the dedicated elegy-skills CLI."
---

# Elegy Skills

This file is a repo-local, non-authoritative rendered skill bridge mirror.

The authority chain is one-way:

1. `contracts/fixtures/skill-definition-v2.elegy-skills.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-skills.json` is the governed discovery projection derived from that definition.
3. `.agents/skills/elegy-skills/SKILL.md` and `.github/skills/elegy-skills/SKILL.md` are repo-local rendered mirrors only.

## When to use

- Prefer the dedicated `elegy-skills` binary for registry-first skill work: searching, resolving, inspecting, and validating governed v2 skills.
- Use the same crate directly from Rust when a host such as Holon should avoid CLI subprocess overhead for registry access.
- Treat `elegy skills ...` as the umbrella compatibility surface for the same registry features.

## Do not use

- Do not treat this skill as authority for runtime-side skill registration, autonomous authoring, or host-specific orchestration.
- Do not infer that repo-local markdown is authoritative. Governed fixtures remain the source of truth.
- Do not infer that the overlay under `src/Elegy-skills` is an implementation center or release surface.

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

`--format json` is available on the CLI when structured output is needed.

## Surface posture

- This CLI is the dedicated registry surface over the governed v2 skill catalog plus a reusable Rust API for in-process hosts.
- The dedicated surface is intentionally bounded to discovery, resolution, inspection, and validation over governed skill artifacts.
- Governed skill artifacts remain rooted in `contracts/` and versioned through `governance/`.
