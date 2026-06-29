---
name: elegy-skills
description: Use when an agent needs to search, resolve, get, validate, or inspect capabilities across the governed skill registry — the CLI-first discovery, resolution, and validation surface for Elegy skills.
---

# Elegy Skills

> Use when an agent needs to search, resolve, get, validate, or inspect capabilities across the governed skill registry through the dedicated `elegy-skills` CLI or the umbrella `elegy skills ...` compatibility surface.

The governed skill registry is the discovery authority. The `elegy-skills`
CLI is the dedicated surface; `elegy skills ...` is the umbrella
compatibility path. Both resolve against the same catalog.

## Quick start

1. List the built-in catalog:
   `elegy-skills list --json` to see every active governed skill and
   its lifecycle state.
2. Search for a task query:
   `elegy-skills search --query "repo status" --json` to get ranked
   matches with match reasons and context-budget hints.
3. Resolve the best match:
   `elegy-skills resolve --query "diagram" --json` to get the top
   skill/capability pair plus ranked alternatives.
4. Get full detail for one skill:
   `elegy-skills get --skill-id diagram --json` to load the full
   governed definition including every capability's argument template.
5. Validate a skill file or directory:
   `elegy-skills validate --file <path> --json` or
   `elegy-skills validate --dir <path> --json`.

## Tool-call guardrails

### Search / resolve / get / list (`skills-registry-search`,
`skills-registry-resolve`)

- All three are read-only and deterministic. They read the built-in
  governed registry; there is no remote fetch.
- `search` returns ranked matches with `matchReason` and
  `matchedCapabilities[]`. Use the match reason to decide whether
  to expand with `get` or `resolve`.
- `resolve` returns a single top match plus `alternatives[]`.
  `alternatives` is the ranked fallback surface; if the top match
  is wrong for the task, pick from `alternatives`.
- `get` accepts a skill id or alias. Aliases are first-match; if
  two skills share the same alias, the first in declaration order
  wins. Do not assume uniqueness.
- Side-effect class: `read_only`.
- Approval posture: `none`.

### Validate (`skills-registry-validate`)

- Accepts `--file <path>` for one definition or `--dir <path>` for
  a directory. The CLI recurses into the directory and validates
  every `.json` file it finds that declares the `skillFormat`
  constant.
- Validation is format-level: schema conformance, required fields,
  capability shape, output schema presence. It does not validate
  that the capability actually works or that the referenced
  `executableName` exists on PATH.
- The result is `status: "ok"` or `status: "invalid"`. Invalid
  status comes with `diagnostics[]` containing structured issues.
- Side-effect class: `read_only` (reads files from disk; does not
  write).
- Approval posture: `none`.

## Workflow

1. Prefer `resolve` for single-query discovery.
   - `resolve` returns the top match plus token-budget hints. Use
     it when you want the fastest path to the right skill.
2. Expand with `get` when you need the argument template.
   - `get` returns the full governed definition including
     `capabilities[].implementation.arguments`. Use the argument
     vector as the CLI invocation template.
3. Validate before publishing a change.
   - Run `validate` on every modified fixture and on the
     `plugins/skills/fixtures/` directory before opening a PR.
     Incomplete or broken fixtures fail CI downstream.
4. List for visibility checks.
   - `list` with `--lifecycle active` shows what is currently in
     the discovery surface. Use it to confirm a new skill is
     registered.

## Capability index

| id | side-effect | purpose |
| -- | -- | -- |
| `skills-registry-search` | read-only | Search the governed catalog by task or keyword |
| `skills-registry-resolve` | read-only | Resolve the best skill/capability for a task |
| `skills-registry-validate` | read-only | Validate one file or a directory against the governed format |

## Output envelope

- Envelope: `SkillDiscoveryResult` for search, resolve, and
  validate (declared in
   `plugins/skills/schemas/skill-discovery-result.schema.json`).
- `search` / `resolve`: `data.results[]` contains ranked matches.
  Each match has `skillId`, `name`, `description`, `matchReason`,
  `matchedCapabilities[]`, and `contextCostEstimate`.
- `validate`: `status: "ok" | "invalid"`. `invalid` carries
  `data.diagnostics[]` with `severity`, `path`, `line`, `field`,
  `message`. `severity` is `error`, `warning`, or `info`.
- `get`: returns the full governed definition in `data`. The
  fixture shape is the same as the source file; parse it from
  `data` directly.

## Common issues

| Symptom | Cause | Solution |
| -- | -- | -- |
| `search` returns no matches for a clear query. | The query matched against trigger tables and keywords, not free-text descriptions. A subtle synonym mismatch drops the result. | Re-query with a different keyword from the target skill's `discovery.keywords` or `triggers[].pattern`. |
| `resolve` returns the wrong skill. | The query has an ambiguous term that matches two skills, and the first in declaration order wins. | Use `get --skill-id <canonical-id>` instead, or narrow the query to include the namespace prefix. |
| `get --skill-id <alias>` resolves to the wrong skill. | The alias is shared and the first skill in declaration order claims it. | Use the canonical `identity.name` instead of the alias. `get` accepts both. |
| `validate --dir` reports `skillFormat` mismatch on a file you did not write. | The directory contains a JSON file that is not an Elegy skill fixture. | Move non-fixture JSON files out of the validation directory, or pass `--file` for single-file validation. |
| `validate` passes but `cargo test` still fails. | `validate` checks format; `cargo test` checks registry invariants (duplicate ids, missing discovery indexes). Both must pass. | Run the full conformance test suite: `cargo test -p elegy-contracts`. |
| `list` does not show a skill you just added. | The skill's `lifecycleState` is `draft`, and `list` defaults to `active` unless `--lifecycle draft` is passed. | Pass `--lifecycle draft` or promote the skill to `active`. |

## Version compatibility

- Minimum supported `elegy-skills` version: `0.1.0`. The CLI shares
  workspace version with the root `elegy`; check `elegy --version`.
- The governed registry format is declared by
   `plugins/skills/schemas/skill.schema.json`. Validate every fixture
  against this schema before publishing.

## Examples

### Example 1 — resolve the best skill for a task

```text
elegy-skills resolve --query "validate docs" --json
```

Expected:

```json
{
  "status": "ok",
  "data": {
    "results": [{
      "skillId": "documentation",
      "name": "Elegy Documentation",
      "matchReason": "Capability matched: 'documentation-check'",
      "matchedCapabilities": ["documentation-check"],
      "contextCostEstimate": 3200
    }],
    "alternatives": [{ "skillId": "skills", "name": "Elegy Skills" }]
  }
}
```

### Example 2 — validate a single fixture

```text
elegy-skills validate --file plugins/skills/fixtures/skill.minimal.json --json
```

Expected: `status: "ok"` with no diagnostics for a well-formed
fixture. For an invalid fixture: `status: "invalid"` with
`data.diagnostics[].message` describing each issue.

## Boundaries

- This skill owns: governed skill registry discovery, resolution,
  get, list, and validation.
- This skill does not own: skill content (bodies are in SKILL.md
  files), host projection or onboarding (use `elegy agent ...`),
  MCP to skill generation (use `elegy-mcp`).
- Companion skills:
  - `elegy-skill-authoring` — for writing and auditing SKILL.md
    bodies.
  - `elegy-mcp` — for authoring MCP descriptors.
  - `elegy-memory` — for adding procedural memories about skill
    usage.
  - `elegy-planning` — for planning skill registry changes.

## References

- Governed source: `plugins/skills/fixtures/skill.elegy-skills.json`.
- Discovery projection:
  `plugins/skills/fixtures/skill-discovery-index.elegy-skills.json`.
- Architecture: `docs/architecture/skill-core-v1.md`.
- Tooling placement: `docs/architecture/mcp-skill-tooling-placement.md`.
- Result envelope:
  `plugins/skills/schemas/skill-discovery-result.schema.json`.
