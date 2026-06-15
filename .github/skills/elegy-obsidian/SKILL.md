---
name: elegy-obsidian
description: Use when an agent needs to create, read, search, patch, or toggle tasks inside the user's local Obsidian vault through the official Obsidian v1.12+ CLI. Treats Obsidian as a non-authoritative vault mirror; durable planning state continues to flow through elegy-planning and SQLite.
---

# Elegy Obsidian

> Use when an agent needs to create, read, search, patch, or toggle tasks inside the user's local Obsidian vault through the official Obsidian v1.12+ CLI.

This skill wraps the **official `obsidian` CLI** shipped with Obsidian
Desktop 1.12+. It is not a parallel binary; the user's Obsidian
installation provides the executable. Obsidian content is
non-authoritative — durable planning state still flows through
`elegy-planning` and SQLite.

## Quick start

1. Confirm the CLI is on PATH and modern enough:
   `obsidian version` — the wrapper expects `obsidian-result/v1` and a
   minimum CLI build of `1.12.0`.
2. List vaults the desktop app knows about:
   `obsidian vault=<name> vaults` — pick the right vault for the user
   before any file operation.
3. Read the note the user named:
   `obsidian vault=<name> read file=<path>` — use the returned
   `obsidian-result/v1` `data` payload as the source of truth; fall back
   to `rawOutput` only when `data` is null.
4. Append to today's daily note:
   `obsidian vault=<name> daily append file=<YYYY-MM-DD>.md content=<text>` —
   omit `file` to target today; pass an explicit date to backfill
   historical dailies.
5. Toggle a specific task bullet:
   `obsidian vault=<name> task toggle file=<path> line=<n>` — first
   `obsidian-file-read` the note to confirm the line index, then call
   task-toggle against the verified line.

## Tool-call guardrails

### Read capabilities (`obsidian-vault-list`, `obsidian-version`,
`obsidian-file-read`, `obsidian-search`, `obsidian-daily-read`,
`obsidian-random-note`, `obsidian-tag-list`, `obsidian-tag-notes`,
`obsidian-task-list`)

- Argument shape: the official CLI uses `key=value` for scalar arguments
  and a positional first argument for the subcommand. Subcommands seen
  in this skill include `vaults`, `version`, `read`, `search`, `daily`,
  `random`, `tags`, `task`. Do not pass `--flag=value`; that shape
  fails silently with an unhelpful error from the official CLI.
- `obsidian-search` accepts one literal query per call. Do not join
  terms with `or` or `+` inside a single query string. Run separate
  searches for separate intents.
- `obsidian-file-read` accepts a vault-relative path. Do not pass an
  absolute filesystem path; the CLI resolves paths against the chosen
  vault.
- Side-effect class: `read_only`.
- Approval posture: `none`.

### Mutating capabilities (`obsidian-file-create`, `obsidian-file-append`,
`obsidian-file-patch`, `obsidian-file-move`, `obsidian-file-delete`,
`obsidian-daily-append`, `obsidian-task-toggle`)

- Fetch-before-mutate: always run `obsidian-file-read` first against
  the same path and use its result as the input to the mutation. Do
  not construct mutation input from cached or pre-call state.
- `obsidian-file-create` fails if the file already exists. Confirm
  absence with `obsidian-file-read` (which returns `status: "error"`
  for missing files) before calling create.
- `obsidian-file-patch` is line-range based. Always pass a validated
  `line-start` and `line-end` taken from the read result.
- `obsidian-file-delete` is destructive and irreversible from the CLI.
  Confirm with the user before invoking. There is no undo path.
- `obsidian-task-toggle` toggles a specific line. First read the file
  and locate the task bullet at the expected line; do not guess line
  indices.
- `obsidian-daily-append` with `file=<YYYY-MM-DD>.md` backfills a
  historical daily. Omit `file` to target today.
- Side-effect class: `disk_write` (these write to the vault on disk).
- Approval posture: `advisory`. The host policy decides whether
  approval is required.

### Process-spawn capability (`obsidian-command`)

- `obsidian-command` invokes a registered Obsidian command by id. The
  command id must come from the user's installed plugin or core
  command set. Do not invent command ids.
- The executed Obsidian command may itself mutate vault state, open
  windows, or focus the application. Treat the result as
  `desktop_ui` even if the inner command looks like a write.
- Side-effect class: `process_spawn`.
- Approval posture: `required`. The host must explicitly approve.

## Workflow

1. Verify the CLI.
   - Run `obsidian-version`. If the binary is missing or the version
     string is below `1.12.0`, stop and direct the user to
     `references/install-obsidian-cli.md` to enable the CLI in
     Obsidian Desktop. Do not fall back to a custom binary.
2. Resolve the vault.
   - If the user did not name a vault, call `obsidian-vault-list` and
     ask. With multiple vaults, never default silently.
3. Read first for any mutating capability.
   - Run `obsidian-file-read` (or `obsidian-task-list`) and use the
     returned `data` as the source for the mutation's arguments.
   - For `obsidian-file-patch` and `obsidian-task-toggle`, capture the
     exact line range or line index from the read.
4. Invoke the mutation.
   - Pass `--json` style flags only when the capability advertises
     them. This skill does not currently advertise JSON flags; do not
     invent `--json` for `obsidian` calls.
   - Wrap the subprocess result in the `obsidian-result/v1` envelope.
5. Confirm and surface.
   - Re-read the file for any write that affects visible content, and
     report the post-mutation state. For deletes, report the
     pre-deletion read so the user can recover from a backup.

## Capability index

| id | side-effect | purpose |
| -- | -- | -- |
| `obsidian-vault-list` | read-only | Discover registered vaults |
| `obsidian-version` | read-only | Precondition / version gating |
| `obsidian-file-read` | read-only | Read a note's contents |
| `obsidian-file-create` | disk_write | Create a new note (fails if it exists) |
| `obsidian-file-append` | disk_write | Append text to an existing note |
| `obsidian-file-patch` | disk_write | Insert / replace / delete at a line range |
| `obsidian-file-move` | disk_write | Rename or relocate a note |
| `obsidian-file-delete` | disk_write | Remove a note (destructive, irreversible) |
| `obsidian-search` | read-only | Free-text search across the vault |
| `obsidian-daily-read` | read-only | Read today's or a dated daily note |
| `obsidian-daily-append` | disk_write | Append to today's or a dated daily note |
| `obsidian-random-note` | read-only | Sample a random note |
| `obsidian-tag-list` | read-only | Discover all tags in use |
| `obsidian-tag-notes` | read-only | Find all notes carrying a specific tag |
| `obsidian-task-list` | read-only | List done / undone tasks |
| `obsidian-task-toggle` | disk_write | Toggle a specific task bullet |
| `obsidian-command` | process_spawn | Invoke a registered Obsidian command by id |

## Output envelope

- Envelope: `obsidian-result/v1`.
- `schemaVersion`: literal string `obsidian-result/v1`. Validate
  before parsing `data`.
- `data`: structured payload, shape varies by command family. Null
  when the official CLI did not return structured data and only
  `rawOutput` is available.
- `rawOutput`: original stdout from the official CLI. Populated
  even when `data` is structured so callers that need the raw text can
  use it.
- `status`: `ok` on zero exit, `error` otherwise.
- `command`: the full argv vector that was executed, including the
  `obsidian` binary and every argument. Useful for debugging
  unexpected behavior.
- `error`: human-readable error string when `status` is `error`. Do
  not parse — surface to the user.

## Common issues

| Symptom | Cause | Solution |
| -- | -- | -- |
| `obsidian: command not found` from the wrapper. | The official CLI is not enabled in Obsidian Desktop, or the `obsidian` binary is not on `PATH`. | Direct the user to `references/install-obsidian-cli.md`. In Obsidian Desktop: Settings → General → Command line interface. After enabling, restart the shell so `PATH` updates. |
| `obsidian-file-read` returns `status: "error"` for a file the user just created. | The CLI was invoked against a different vault than the one the user has open. | Re-run `obsidian-vault-list`, confirm with the user which vault the file lives in, and pass `vault=<name>` explicitly. |
| `obsidian-file-create` fails on a note the agent believes does not exist. | The note exists under a different case (the CLI is case-sensitive on some platforms) or in a folder the user has not opened in the desktop app. | Run `obsidian-search` to find the canonical path and case, then call `obsidian-file-append` instead of `obsidian-file-create`. |
| `obsidian-task-toggle` toggles the wrong line. | The line index was guessed or copied from a stale read. | Always run `obsidian-file-read` immediately before `obsidian-task-toggle` and use the current line index from the read result. |
| `obsidian-daily-append` overwrites an existing daily note. | The CLI's daily-append appends; the issue is that the file did not exist and the call effectively created a new file but with a date that conflicts with the user's daily-note plugin. | Confirm the user's daily-note plugin and naming convention. Pass `file=<YYYY-MM-DD>.md` explicitly when the user names a date, and confirm with the user before creating. |
| `obsidian-command` returns `desktop_ui` but the expected command did not run. | The command id is wrong, or the plugin that owns the command is disabled. | List the user's enabled plugins and their command ids. Do not invent command ids. |
| The wrapper returns `data: null` and only `rawOutput` is populated. | The official CLI does not emit JSON for the called subcommand. This is normal for several subcommands. | Parse `rawOutput` instead. Do not assume `data` is always populated. |
| `obsidian-search` returns connected-source URLs from Notion, web clips, or similar. | The CLI's search index covers connected sources, not just vault files. | These are valid for context but cannot be passed to `obsidian-file-read` (which only accepts vault paths). Surface as citations only. |

## Version compatibility

- Minimum supported Obsidian Desktop version: `1.12.0` (when the
  official CLI became enabled by default).
- Minimum supported CLI protocol: any build that returns
  `obsidian-result/v1`-compatible exit semantics.
- Semver rule: Obsidian Desktop follows semver on the major.minor
  axis. Patch is unconstrained. The agent should refuse to call
  `obsidian-command` if the reported CLI version is older than the
  build that registered the command, since the id may have been
  removed.

## Examples

### Example 1 — read a note and append a paragraph

```text
obsidian vault=work read file=notes/2026-06-04.md
```

Expected `obsidian-result/v1` shape (abbreviated):

```json
{
  "schemaVersion": "obsidian-result/v1",
  "command": ["obsidian", "vault=work", "read", "file=notes/2026-06-04.md"],
  "status": "ok",
  "vault": "work",
  "data": { "content": "# 2026-06-04\n\n- [x] morning sync\n" },
  "rawOutput": "# 2026-06-04\n\n- [x] morning sync\n",
  "error": null
}
```

Then append:

```text
obsidian vault=work append file=notes/2026-06-04.md content="- [ ] review planning mirror\n"
```

Expected: `status: "ok"`, `data: null`, `rawOutput` carries any
echo from the CLI.

### Example 2 — toggle a specific task bullet

```text
obsidian vault=work read file=notes/todo.md
obsidian vault=work task toggle file=notes/todo.md line=7
```

The first call returns the current contents; the agent locates the
target bullet at line 7 in `data.content` and uses that exact line
index. The second call toggles that line and returns `status: "ok"`.
Re-read to confirm the toggle landed on the intended bullet.

## Boundaries

- This skill owns: vault note operations and the `obsidian-result/v1`
  envelope shape for those operations.
- This skill does not own: durable planning state (goals, roadmaps,
  plans, todos-as-planning-entities, review points). Those still flow
  through `elegy-planning` and SQLite.
- This skill does not own: agent-host projection metadata, MCP
  registration, or portable package composition.
- Companion skills:
  - `elegy-planning` — durable planning authority.
  - `elegy-documentation` — authority-aware documentation
    inspection over repo docs.
  - `elegy-skills` — registry-first discovery.
  - `elegy-skill-authoring` — SKILL.md audit and review.

## References

- Governed source: `contracts/fixtures/skill.elegy-obsidian.json`.
- Discovery projection:
  `contracts/fixtures/skill-discovery-index.elegy-obsidian.json`.
- Per-command reference:
  `skills/elegy-obsidian/references/obsidian-cli-command-reference.md`.
- Install / enable the official CLI:
  `skills/elegy-obsidian/references/install-obsidian-cli.md`.
- Foundation spec: `docs/specs/obsidian-skill-and-cli.md`.
- Architecture: `docs/architecture/agent-skill-bridge-mirrors.md`.
- Mirror envelope schema: `contracts/schemas/obsidian-result.schema.json`.
