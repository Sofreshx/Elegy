---
name: elegy-obsidian
description: "Use when an agent needs to create, read, search, patch, or toggle tasks inside the user's local Obsidian vault through the official Obsidian v1.12+ CLI. Treats Obsidian as a non-authoritative vault mirror; durable planning state continues to flow through elegy-planning and SQLite."
---

# Elegy Obsidian

Use this skill when an agent needs a deterministic, capability-shaped interface into the user's local Obsidian vault. The skill wraps the **official `obsidian` CLI** (Obsidian Desktop v1.12+) rather than a parallel binary, so the contract stays in lockstep with the desktop app's own command surface.

Obsidian is a vault, not a database. This skill is therefore a non-authoritative mirror foundation. It gives agents a clean, governed surface for vault operations today, and it leaves a clean extension point for the future `elegy-planning obsidian mirror/attach/resolve/list` commands that will treat Obsidian as a non-canonical planning mirror.

## When to load

Load this skill when the user asks for any of the following against their Obsidian vault:

- "create a note in my vault" / "open / read this vault note"
- "search my vault" / "find notes with tag X"
- "append to today's daily note"
- "toggle this task done" / "list the open tasks in this folder"
- "list the vaults Obsidian knows about" / "what version of obsidian is installed"

Do **not** load this skill when the user wants durable planning state changes (goals, roadmaps, plans, todos-as-planning-entities, review points, issues). Those still flow through `elegy-planning` and SQLite.

## Workflow

1. Confirm the `obsidian` binary is reachable and modern enough.
   - Run `obsidian version` first. The expected minimum is `1.12.0` because that is when the official CLI became enabled-by-default in Obsidian Desktop.
   - If `obsidian` is not on PATH, point the user to `skills/elegy-obsidian/references/install-obsidian-cli.md` and stop; do not fall back to a custom binary.
2. Pick the smallest capability that fits. Prefer read-only capabilities first. Mutating capabilities (create / append / patch / move / delete / task-toggle / command) are advisory-approved but not required-approval under the current governance profile.
3. Build the argv vector. The official CLI uses `key=value` argument form, not POSIX-style flags. The v2 fixture already encodes the exact argument order per capability.
4. Run the command with subprocess, capture stdout, and wrap the result in the `obsidian-result/v1` envelope.
5. Surface structured data through `data` and keep the original text in `rawOutput` so callers can pick their preferred shape.

## Capability index

| Capability ID               | Side effect  | Use it for                                              |
| --------------------------- | ------------ | ------------------------------------------------------- |
| `obsidian-vault-list`       | read-only    | Discover registered vaults                              |
| `obsidian-version`          | read-only    | Precondition / version gating                           |
| `obsidian-file-read`        | read-only    | Fetch a note's contents                                 |
| `obsidian-file-create`      | disk write   | Create a new note (fails if it exists)                  |
| `obsidian-file-append`      | disk write   | Append text to an existing note                         |
| `obsidian-file-patch`       | disk write   | Insert / replace / delete at a specific line range      |
| `obsidian-file-move`        | disk write   | Rename or relocate a note                               |
| `obsidian-file-delete`      | disk write   | Remove a note (destructive)                             |
| `obsidian-search`           | read-only    | Free-text search across the vault                       |
| `obsidian-daily-read`       | read-only    | Read today's (or a dated) daily note                    |
| `obsidian-daily-append`     | disk write   | Append to today's (or a dated) daily note               |
| `obsidian-random-note`      | read-only    | Sample a random note                                    |
| `obsidian-tag-list`         | read-only    | Discover all tags in use                                |
| `obsidian-tag-notes`        | read-only    | Find all notes carrying a specific tag                  |
| `obsidian-task-list`        | read-only    | List done / undone tasks                                |
| `obsidian-task-toggle`      | disk write   | Toggle a specific task bullet                           |
| `obsidian-command`          | process spawn| Invoke a registered Obsidian command by id              |

See `references/obsidian-cli-command-reference.md` for the per-command argument shape, exit behavior, and known CLI build differences.

## Boundaries

- This skill wraps an **external** binary. It is not a Rust CLI; there is no `elegy-obsidian` binary. The Elegy wrapper archive only contains the installable skill bridge, contracts bundle, and docs. The user's Obsidian installation provides the executable.
- Obsidian content is **non-authoritative**. SQLite via `elegy-planning` is the durable authority for plans, goals, roadmaps, todos-as-planning-entities, and review points.
- Mirror files must never shadow canonical planning paths (e.g. `.copilot/backlogs`, `~/.copilot/backlogs/{repo}/planning/`). Use a vault folder the user controls — typically `inbox/`, `notes/`, or `planning-mirror/` — and stamp each mirror note with `ie_kind` and `ie_source` frontmatter as captured in the research note.
- The CLI is a desktop-application channel. Some commands open Obsidian windows, launch URIs, or focus the application. Treat it as `desktop_ui` for side-effect classification, not as a pure file write.
- Do not assume JSON output. The official CLI returns text by default. Use `rawOutput` to capture the verbatim text, and only emit `data` when the calling code has parsed it.

## Companion skills

- `elegy-planning` — durable planning authority. Use for any change to goals, roadmaps, plans, todos-as-planning-entities, or review points. Use this skill (elegy-obsidian) for any change to vault notes.
- `elegy-documentation` — authority-aware documentation inspection, mapping, and validation against the repo. Use for repo docs posture, not for vault content.
- `elegy-skills` — registry-first discovery. Resolve `elegy-obsidian` through the registry when in doubt.

## Related doctrine

- `../../docs/specs/obsidian-skill-and-cli.md` — foundation spec and the extension point for `elegy-planning obsidian mirror/attach/resolve/list`
- `../../docs/research/obsidian-figma-and-vision-models-for-elegy.md` — research note that motivated this skill and the future planning-mirror commands
- `../../contracts/fixtures/skill-definition-v2.elegy-obsidian.json` — governed source of truth for this skill
- `../../contracts/fixtures/skill-discovery-index.elegy-obsidian.json` — discovery projection
- `references/obsidian-cli-command-reference.md` — per-command argument shape and CLI quirks
- `references/install-obsidian-cli.md` — how to enable and verify the official `obsidian` CLI
