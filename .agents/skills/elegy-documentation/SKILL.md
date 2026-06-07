---
name: elegy-documentation
description: Use when an agent needs to initialize, inspect, map, objectively check, or export repo-local documentation configuration — authority roots, entrypoints, derived surfaces, freshness, broken links, and machine-readable bundles — through the dedicated elegy-documentation CLI.
---

# Elegy Documentation

> Use when an agent needs to initialize, inspect, map, objectively check, or export repo-local documentation configuration through the dedicated `elegy-documentation` CLI.

This skill owns the deterministic docs surface. Source documents remain
authoritative; generated `llms` and `bundle` outputs remain derived.
The umbrella `elegy docs ...` commands are a compatibility scaffold;
this skill uses the dedicated surface for faster, narrower checks.

## Quick start

1. Confirm the project has a docs config:
   `elegy-documentation inspect --project . --json`. If the result is
   empty or missing, run `init` first.
2. Map the current posture:
   `elegy-documentation map --project . --json` to list authority
   roots, entrypoints, derived surfaces, and freshness status.
3. Run objective checks:
   `elegy-documentation check --project . --json` to surface missing
   metadata, broken relative links, and stale derived surfaces.
4. Export for LLM context:
   `elegy-documentation export llms --project . --output <path> --json`
   to write a concatenated Markdown bundle consumable by agents.
5. Export for archival:
   `elegy-documentation export bundle --project . --output <path> --json`
   for the full structured bundle.

## Tool-call guardrails

### Init (`documentation-init`)

- Creates `.elegy/docs.yaml` at the project root with authority
  roots, entrypoints, derived surfaces, and freshness defaults.
- `--dry-run` previews without writing. Always pre-run with
  `--dry-run` before the real call so the user sees what will be
  created.
- `--non-interactive --correlation-id <id>` is required for
  machine-safe invocation. Omitting either fails the call.
- Side-effect class: `disk_write` (creates the config file).
- Approval posture: `advisory` for a new config; confirm with the
  user that the repo should have a docs config.

### Inspect / map / check (`documentation-inspect`, `documentation-map`,
`documentation-check`)

- All three are read-only. `inspect` shows the current config.
  `map` shows posture plus derived surfaces. `check` runs objective
  validation and returns a pass/fail report.
- `check` is objective only: metadata (title, status, owner),
  frontmatter validity, required headings, and broken relative
  links. Prose quality and architecture correctness are not
  evaluated. Do not describe `check` as a quality gate.
- Side-effect class: `read_only`.
- Approval posture: `none`.

### Export (`documentation-export-llms`, `documentation-export-bundle`)

- Both write to `--output <path>`. Confirm the path with the user
  before invoking; the file is overwritten if it exists.
- `export llms` produces a single concatenated Markdown file of all
  source documents in authority-root order.
- `export bundle` produces a structured JSON bundle with individual
  source entries plus metadata.
- Side-effect class: `disk_write`.
- Approval posture: `advisory` for within-project paths;
  `required` for paths outside the project.

## Workflow

1. Init, if the config is missing.
   - Run `init --dry-run` first to show the skeleton. If the user
     approves, re-run without `--dry-run`.
2. Map.
   - Run `map` after every structural change (new ADR, moved spec,
     new derived surface). The map is deterministic; it does not
     interpret quality.
3. Check.
   - Run `check` before declaring docs-healthy. Treat objective
     failures as blocking for publishing steps; failed checks do
     not prove bad docs, but broken links and missing metadata
     must be fixed.
4. Export for the downstream consumer.
   - `export llms` for agent context. `export bundle` for archival
     or programmatic consumers. Pick the right export for the job.

## Capability index

| id | side-effect | purpose |
| -- | -- | -- |
| `documentation-init` | disk_write | Init `.elegy/docs.yaml` at the project root |
| `documentation-inspect` | read-only | Show the current docs config |
| `documentation-map` | read-only | Map authority roots, entrypoints, derived surfaces, freshness |
| `documentation-check` | read-only | Run objective checks (metadata, frontmatter, headings, links) |
| `documentation-export-llms` | disk_write | Export a concatenated Markdown bundle for LLM context |
| `documentation-export-bundle` | disk_write | Export a structured JSON bundle |

## Output envelope

- Envelope: each capability returns its own result type
  (`DocumentationInitResult`, `DocumentationInspectResult`,
  `DocumentationMapResult`, `DocumentationCheckResult`,
  `DocumentationExportResult`) with a matched schema under
  `contracts/schemas/`.
- Common fields: `status: "ok" | "error"`, `data`, `correlationId`
  on init/check, `error` on failure.
- `DocumentationCheckResult.data.issues[]` has the objective
  findings. Each issue has `severity`, `file`, `line`, `rule`,
  `message`. Parse `severity` to decide blocking.
- `DocumentationExportResult.data.path` is the written output
  path. `data.size` is the byte count of the written file.

## Common issues

| Symptom | Cause | Solution |
| -- | -- | -- |
| `inspect` returns an empty or missing result. | `.elegy/docs.yaml` does not exist at the project root. | Run `init` first. `inspect` and `map` are no-op on a repo without a config. |
| `check` reports broken relative links that look correct to you. | The link targets a file outside the docs root as declared in the config, or the file was renamed without updating the referencing doc. | Run `map` to see the doc root layout, then fix the link or add the missing path to the authority root list. |
| `check` reports missing metadata for an ADR you just created. | The ADR frontmatter is missing `title`, `status`, or `owner`. `check` requires all three. | Add the missing frontmatter field and re-run. |
| `export llms` produces a huge file that takes minutes to write. | The docs root covers many source documents, and the concatenated output includes every one. | Narrow the authority roots in `.elegy/docs.yaml` to only the docs the consumer needs, or export per-root. |
| `init --dry-run` shows a config that looks wrong. | The repo already has partial docs config or an unusual directory layout. | Adjust `.elegy/docs.yaml` by hand after init, or pass custom entrypoint paths during init. |
| `export bundle` overwrites an existing bundle the user expected to keep. | `--output` points at an existing path and the CLI overwrites. | Confirm `--output` is fresh or back up the existing bundle before overwriting. The CLI does not warn in machine mode. |
| `map` shows stale derived surfaces after a source document was updated. | The freshness calculation is driven by file modification timestamps. A `git checkout` or `git pull` can reset timestamps. | Re-export the derived surface (`export llms` or `export bundle`) to bring it up to date, then re-run `map`. |
| `check` fails CI but the build is green. | `check` is objective, not CI-gating. A failed check means objective issues exist, not that the build is broken. | Fix the objective findings. `check` does not grade prose quality; it catches metadata and structural issues only. |

## Version compatibility

- Minimum supported `elegy-documentation` version: `0.1.0`.
- The `.elegy/docs.yaml` config schema is versioned in the first
  field of the config. Check the config version before changing
  structure; an older config version may miss a newer field.
- Semver rule: minor must be ≥ the version that introduced the
  required field (e.g. `freshnessThresholdHours` is recent).

## Examples

### Example 1 — init and check

```text
elegy-documentation --non-interactive --correlation-id abc123 \
  --json init --project . --dry-run
```

Expected: `status: "ok"`, `data.dryRun: true`,
`data.files.initialized` lists paths that would be created.

After user approval, re-run without `--dry-run`, then:

```text
elegy-documentation --json check --project .
```

Expected: `status: "ok"`, `data.issues` lists objective findings
or is empty for a clean project.

### Example 2 — export for agent context

```text
elegy-documentation --json export llms \
  --project . --output ./agent-context.md
```

Expected: `status: "ok"`, `data.path: "./agent-context.md"`,
`data.size` reports a positive byte count. The file contains
concatenated Markdown from all declared authority roots.

## Boundaries

- This skill owns: `.elegy/docs.yaml` initialization, objective
  docs validation (metadata, frontmatter, headings, links),
  document mapping, and deterministic export.
- This skill does not own: prose quality review, architecture
  correctness, or spec validation against behavior. Those are
  manual review gates or live in `elegy-doc-practices`.
- Companion skills:
  - `elegy-doc-practices` — doctrine for document type choice,
    placement, and review.
  - `elegy-planning` — for tracking spec and ADR work.
  - `elegy-skills` — for registry-first discovery of this skill
    from the governed catalog.

## References

- Governed source: `contracts/fixtures/skill.elegy-documentation.json`.
- Discovery projection:
  `contracts/fixtures/skill-discovery-index.elegy-documentation.json`.
- Architecture: `docs/architecture/documentation-practices.md`.
- Spec: `docs/specs/documentation-practices-skill-and-cli.md`.
- Docs config schema:
  `contracts/schemas/elegy-documentation-v2.schema.json`.
- Doctrine: `skills/elegy-doc-practices/SKILL.md`.
