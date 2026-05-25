# Elegy-skills Wrapper Surface

## Purpose

- This root is the thin wrapper and handoff surface for the dedicated `elegy-skills` registry tool.
- It is not an authority root, implementation center, or release orchestration center.

## Authority

- Governed skill truth remains under `contracts/`.
- `rust/crates/elegy-skills` owns the reusable registry API and dedicated CLI behavior.
- `.github/skills/elegy-skills/SKILL.md` and `skills/elegy-skills/SKILL.md` are non-authoritative routing outputs.

## What This Surface Represents

- The dedicated skill tools are registry-first: search, resolve, inspect, and validate governed skills.
- The same behavior is mirrored by the umbrella `elegy skills ...` commands.
- MCP-to-skill generation is lower-level contributor tooling and is not the main story for this wrapper surface.

## Editing Rules

- Keep this wrapper aligned with `docs/architecture/skill-core-v1.md` and `rust/crates/elegy-skills/AGENTS.md`.
- Do not add host-specific orchestration, approval, or runtime-registration claims here.
- If commands, archive contents, or install layout change, update `wrapper-entrypoint.json`, the bridge `SKILL.md`, and the distribution docs together.
