---
name: skill-template
description: Canonical SKILL.md skeleton for Elegy skills. Use as the starting point when drafting, reviewing, or fixing an Elegy skill. Section order and naming are normative; required sections must be present and non-empty.
---

# <Skill name>

> One-line trigger description that matches the frontmatter `description` verbatim.

## Quick start

1. <Most common action>: <exact CLI invocation or capability id>.
2. <Second most common action>.
3. <Third most common action>.
4. <Optional fourth action for the second main path>.
5. <Optional fifth action for a common read first>.

Keep each step to one tool call. Replace generic verbs with the actual command
shape and the exact `${parameter}` placeholders that the capability advertises.

## Tool-call guardrails

### <Capability family 1, e.g. "Read capabilities">

- Argument shape: <`key=value` vs `--flag`, repeat-per-value vs comma-joined>.
- Fetch-before-mutate: <which read must happen before which write>.
- Do not: <literal anti-patterns the agent must not invent>.
- Side-effect class: <`read-only` | `disk_write` | `desktop_ui` | `process_spawn` | ...>.
- Approval posture: <none | advisory | required>.

### <Capability family 2, e.g. "Mutating capabilities">

- Repeat the same five bullets for the next family.

## Workflow

1. <First decision or precondition check>.
   - If <condition>, <branch A>.
   - If <condition>, <branch B>.
2. <Second decision or step>.
3. <Third decision or step>.
4. <Final step or handoff>.

## Capability index

| id | side-effect | purpose |
| -- | -- | -- |
| `<capability-id>` | `<read-only\|disk_write\|...>` | <one-line purpose> |

## Output envelope

- Envelope: `<envelope name>/<version>` (e.g. `obsidian-result/v1`).
- `data`: <when populated, when null>.
- `rawOutput`: <when populated, when null>.
- Error shape: <machine-readable error fields and exit code>.
- Parse guidance: <which field to trust first, fallback order>.

## Common issues

| Symptom | Cause | Solution |
| -- | -- | -- |
| <Observable error or unexpected behavior> | <Root cause, with link to a file or section> | <Concrete fix or workaround> |
| ... | ... | ... |

## Version compatibility

- Minimum supported CLI version: <X.Y.Z>.
- Minimum supported external dep version: <X.Y.Z or "n/a">.
- Semver rule: <what the agent must check before invoking>.

## Examples

### Example 1 — <most common case>

```text
<exact command>
```

Expected output:

```text
<expected stdout or JSON>
```

### Example 2 — <second common case>

```text
<exact command>
```

Expected output:

```text
<expected stdout or JSON>
```

## Boundaries

- This skill owns: <what it is authoritative for>.
- This skill does not own: <what other skills or surfaces do instead>.
- Companion skills: <list of related skills with one-line purpose>.

## References

- Governed source: `contracts/fixtures/skill.<surface>.json`.
- Discovery projection: `contracts/fixtures/skill-discovery-index.<surface>.json`.
- <Architecture doc>: `docs/architecture/<file>.md`.
- <Spec or research note>: `docs/specs/<file>.md` or `docs/research/<file>.md`.
- <Per-command reference>: `references/<file>.md` (if applicable).
