---
name: elegy-configuration
description: Use when an agent needs to list, inspect, apply, or verify deterministic configuration templates and profiles — materializing governed config artifacts into target directories through the dedicated elegy-configuration CLI or the umbrella elegy configuration compatibility surface.
---

# Elegy Configuration

> Use when an agent needs to list, inspect, apply, or verify deterministic configuration templates and profiles through the dedicated `elegy-configuration` CLI.

This skill owns deterministic materialization and drift verification
of governed configuration artifacts. The same behavior is available
on the umbrella `elegy configuration ...` commands.

## Quick start

1. List available configuration packages and templates:
   `elegy-configuration list --format json` to inspect what is
   loadable.
2. Show a template's contents before applying:
   `elegy-configuration show --template-id <id> --format json`.
3. Apply a template to a target directory:
   `elegy-configuration apply --target <dir> --template-id <id> --binding KEY=VALUE --dry-run --format json`.
   Always `--dry-run` first to preview the materialized output.
4. Verify a target directory matches its template:
   `elegy-configuration verify --target <dir> --template-id <id> --binding KEY=VALUE --format json` to detect drift.

## Tool-call guardrails

### List / show (`elegy-configuration list`, `elegy-configuration show`)

- `list` shows available packages and templates from the local
  package load path. No remote fetch; the packages must be in the
  configured directory.
- `show` requires a `--template-id` or `--template-path`. The id
  is resolved against the loaded packages. If the id is not found,
  the call fails.
- `show` is read-only. Use it to confirm the template's content and
  binding surface before applying.
- Side-effect class: `read_only`.
- Approval posture: `none`.

### Apply (`elegy-configuration apply`)

- `--target` is the directory where materialized files land. The
  CLI creates directories as needed. Confirm the target path with
  the user before invoking.
- `--dry-run` previews without writing. Always pre-run with
  `--dry-run` before the real call.
- `--binding KEY=VALUE` substitutes template variables. Repeat
  `--binding` once per variable. Unbound variables cause the call
  to fail; there is no silent default.
- `--force` overwrites existing files at the target. Without
  `--force`, the CLI fails if any output file already exists.
- `--package <path>` loads a portable package file. Supported
  packages carry governed configuration components; arbitrary
  local YAML is rejected.
- Side-effect class: `disk_write` (writes files to `--target`).
- Approval posture: `required` when `--force` is passed or the
  target is outside the user's working directory. `advisory` for
  new directories under the workspace.

### Verify (`elegy-configuration verify`)

- `verify` compares the target directory against the template
  output. It does not write files.
- Returns a drift report: files present in the target but missing
  from the template, files with mismatched content, and files
  missing from the target.
- `verify` is best-effort. It does not check file permissions,
  timestamps, or binary files.
- Side-effect class: `read_only`.
- Approval posture: `none`.

## Workflow

1. List the catalog.
   - Run `list` to see what packages and templates are available.
     If the catalog is empty, direct the user to configure the
     package load path.
2. Show the candidate.
   - Before applying, always show the template with all planned
     bindings so the user can confirm the intended output.
3. Apply with `--dry-run` first.
   - Preview the materialized files. Check the `data.created` and
     `data.skipped` lists. If anything looks wrong, adjust
     bindings and re-run `--dry-run`.
4. Apply without `--dry-run`.
   - After the user approves the dry-run output, re-run without
     `--dry-run`. Confirm the result matches the dry run.
5. Verify after external changes.
   - When the user or another tool changes the target directory,
     run `verify` to catch drift. Drift is not automatically
     repaired; re-apply after the user confirms the desired state.

## Capability index

| id | side-effect | purpose |
| -- | -- | -- |
| `configuration-list` | read-only | List available packages, templates, and profiles |
| `configuration-show` | read-only | Show a template or profile's contents before applying |
| `configuration-apply` | disk_write | Materialize a template into a target directory |
| `configuration-verify` | read-only | Compare a target directory against its template for drift |

## Output envelope

- Envelope: `configuration-result/v1`.
- `status`: `ok` or `error`.
- `apply` / `--dry-run`: `data.created[]`, `data.skipped[]`,
  `data.errors[]` listing every file and its disposition.
- `verify`: `data.drift[]` listing files with mismatches,
  including `expected` and `actual` content digests.
- `list`: `data.packages[]` and `data.templates[]`.
- `show`: `data.template` with the full content and declared
  bindings.
- `error`: machine-readable code plus human message on failure.

## Common issues

| Symptom | Cause | Solution |
| -- | -- | -- |
| `list` returns an empty catalog. | No package files are in the configured load path. | Add a portable package file to the configured directory, or pass `--package <path>` directly. |
| `show --template-id <id>` says "not found". | The id is not declared in any loaded package, or the package was loaded from an older path. | Run `list` to see available ids. Use the canonical id from the list output. |
| `apply` fails with "unbound variable" despite the binding looking correct. | The template declares a variable with a different key than the one passed in `--binding`. | Use `show` to inspect the template's declared bindings and use the exact key. Keys are case-sensitive. |
| `apply` overwrites a file the user expected to keep. | `--force` was passed or the file was not present in the template. | Confirm `--target` is a clean directory, or re-run with `--dry-run` to see which files will be created before applying. |
| `verify` reports drift that `apply` did not create. | The user or another tool edited the target directory manually. | Show the drift report to the user and ask whether to re-apply the template or accept the manual changes. |
| `apply` with `--package <path>` fails on a valid-looking package. | The package file uses a schema version the current CLI does not support. | Check the package schema version field and confirm it matches the CLI's supported versions. |
| `--binding` values containing `=` or spaces are split incorrectly. | The CLI splits on the first `=`, and the remaining string becomes the value. | Double-check values with embedded `=`. If the value must contain `=`, confirm the CLI's parsing behavior by testing with a `--dry-run` first. |

## Version compatibility

- Minimum supported `elegy-configuration` version: `0.1.0`.
- The portable package schema version is declared in
  `contracts/schemas/elegy-plugin-package.schema.json`. The CLI
  only supports the current schema version.
- Template and profile schemas are versioned independently under
  `contracts/configuration/`. Confirm the schema version of the
  loaded artifact matches the CLI. Mismatches cause apply/verify
  to fail.

## Examples

### Example 1 — dry-run a template against a clean directory

```text
elegy-configuration apply \
  --target ./output \
  --template-id agent-readme \
  --binding REPO_NAME=elegy \
  --binding AI_HOST=openai \
  --dry-run --format json
```

Expected: `status: "ok"`, `data.created` lists one or more files,
`data.skipped` is empty.

### Example 2 — verify a target for drift

```text
elegy-configuration verify \
  --target ./output \
  --template-id agent-readme \
  --binding REPO_NAME=elegy \
  --binding AI_HOST=openai --format json
```

Expected: `status: "ok"`, `data.drift` is empty for a clean target.
For a drifted target, one or more drift entries with `file`,
`expected` digest, and `actual` digest are listed.

## Boundaries

- This skill owns: deterministic configuration materialization
  and drift verification over governed templates and profiles.
- This skill does not own: host install state, authentication,
  secrets, approvals, or runtime registration. Those are host
  policy, not configuration materialization.
- This skill does not own: env-file injection into templates.
  Templates that reference env vars are the package author's
  responsibility; the CLI does not resolve env refs.
- Companion skills:
  - `elegy-skills` — for registry discovery.
  - `elegy-documentation` — for docs config.
  - `elegy-skill-authoring` — for SKILL.md audit and review.

## References

- Configuration schemas: `contracts/configuration/`.
- Portable package schema: `contracts/schemas/elegy-plugin-package.schema.json`.
- Umbrella surface: `elegy configuration ...` commands.
- Wrapper surface: `src/Elegy-configuration/`.
