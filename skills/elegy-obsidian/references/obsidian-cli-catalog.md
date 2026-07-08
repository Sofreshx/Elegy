# Obsidian CLI Command Catalog

## Binary Invocation

The official `obsidian` CLI ships with Obsidian Desktop 1.12.7+.

- **Windows (native shell)**: `obsidian` is on PATH via app-execution alias.
- **WSL / non-PATH shells**: resolve via `cmd.exe /c "where obsidian"`,
  convert the Windows path to `/mnt/c/.../Obsidian.com`, and use the
  full path. Known install root:
  `C:\Users\<user>\AppData\Local\Programs\Obsidian\Obsidian.com`.
- **Vault targeting**: pass `vault=<name>` on every command when more
  than one vault is registered. The active-vault default is
  platform-specific and unreliable.

## Core Commands (official `obsidian` CLI v1.12.7+)

```text
obsidian version
obsidian vault list
obsidian vault=<name> [command] [key=value ...]
obsidian help
obsidian read file=<path> [vault=<name>]
obsidian create file=<path> content=<text> [vault=<name>]
obsidian append file=<path> content=<text> [vault=<name>]
obsidian patch file=<path> [<key>=<value> ...] [vault=<name>]
obsidian move from=<path> to=<path> [vault=<name>]
obsidian delete file=<path> [vault=<name>]
obsidian search query=<text> [path=<prefix>] [limit=<n>] [vault=<name>]
obsidian daily [date=YYYY-MM-DD] [vault=<name>]
obsidian daily:read [date=YYYY-MM-DD] [vault=<name>]
obsidian daily:append content=<text> [vault=<name>]
obsidian random [vault=<name>]
obsidian tags [vault=<name>]
obsidian tag notes=<tag> [vault=<name>]
obsidian tasks [status=open|done|all] [vault=<name>]
obsidian task toggle file=<path> line=<n> [vault=<name>]
obsidian command <name> [key=value ...] [vault=<name>]
```

### Extended commands (v1.12.7+)

```text
obsidian aliases [file=<name>] [path=<path>] [total] [verbose] [active] [vault=<name>]
obsidian backlinks file=<name> [counts] [total] [format=json|tsv|csv] [vault=<name>]
obsidian base:create file=<name> [path=<path>] view=<name> name=<name> [content=<text>] [vault=<name>]
```

Use `obsidian help` for the full command list on the installed version.

### Notes

- `file` resolves by name (like wikilinks); `path` is exact (folder/note.md).
- Most commands default to the active file when `file`/`path` is omitted.
- Quote values with spaces: `name="My Note"`.
- Use `\n` for newline, `\t` for tab in `content` values.
- Some commands support `format=json|tsv|csv` for structured output
  (default is `tsv`). Check `obsidian help <command>` for specifics.

## Capability Catalog (17 capabilities across 7 groups)

| Group | Capabilities | Side-effect class |
|---|---|---|
| Introspection | `vault-list`, `version` | `read_only` |
| Files | `file-read`, `file-create`, `file-append`, `file-patch`, `file-move`, `file-delete` | `read_only` / `disk_write` |
| Search | `search` | `read_only` |
| Daily note | `daily-read`, `daily-append`, `random-note` | `read_only` / `disk_write` |
| Tags | `tag-list`, `tag-notes` | `read_only` |
| Tasks | `task-list`, `task-toggle` | `read_only` / `disk_write` |
| Escape hatch | `command` (raw `obsidian <subcommand>` pass-through) | `process_spawn` |

Read-only commands print text to stdout. Side-effecting commands
(`create`, `append`, `patch`, `move`, `delete`, `daily:append`,
`task toggle`) return a short confirmation line.

## Output Behavior

The official `obsidian` CLI returns **text by default** (not JSON).
Some commands support `format=json` for structured output, but most
do not. Agents should reason over raw stdout. Do not expect a
structured envelope — parse text output directly.

## Error Handling

- Non-zero exit codes indicate failure. Read stderr for diagnostics.
- Common failures:
  - `obsidian` not found → binary resolution needed (see Binary Invocation).
  - App not running → Obsidian Desktop must be open.
  - Vault not found → check `obsidian vault list` for registered names.
  - File not found → `file` resolves by name (wikilink-style); use
    `path` for exact paths.
