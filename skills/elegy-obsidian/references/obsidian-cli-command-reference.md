# Obsidian CLI Command Reference

This reference distills the official Obsidian v1.12+ CLI surface into the argument shapes used by the `elegy-obsidian` skill. Use it to verify the argument vectors emitted by the v2 fixture and to debug unexpected behavior.

The official CLI ships with Obsidian Desktop. Once enabled (see `install-obsidian-cli.md`), the `obsidian` binary lives on PATH. Most commands take `key=value` arguments; some accept free positional arguments. JSON output is opt-in for a small subset; the rest return plain text.

## Quick map

| Skill capability        | Obsidian command vector                                                | Side effect            | Notes                                                                                      |
| ----------------------- | ---------------------------------------------------------------------- | ---------------------- | ------------------------------------------------------------------------------------------ |
| `obsidian-vault-list`   | `obsidian vault`                                                       | read-only              | One vault per line. Use `obsidian vault` not `obsidian vault list` (varies by build).      |
| `obsidian-version`      | `obsidian version`                                                     | read-only              | First line is the version string.                                                          |
| `obsidian-file-read`    | `obsidian read file=<path>`                                            | read-only              | Returns the file's full text to stdout.                                                    |
| `obsidian-file-create`  | `obsidian create file=<path> content=<text>`                           | disk write             | Fails if the file already exists. Use a different path or delete first.                    |
| `obsidian-file-append`  | `obsidian append file=<path> content=<text>`                           | disk write             | Appends verbatim; does not add a leading newline.                                          |
| `obsidian-file-patch`   | `obsidian patch file=<path> start-line=<n> end-line=<n> mode=<m> content=<text>` | disk write             | Modes: `replace` (default), `insert-before`, `insert-after`, `delete`.                     |
| `obsidian-file-move`    | `obsidian move file=<path> to=<new-path>`                              | disk write             | Obsidian rewrites inbound wiki links when it can.                                          |
| `obsidian-file-delete`  | `obsidian delete file=<path>`                                          | disk write (destructive) | No recycle bin. Confirm scope first.                                                      |
| `obsidian-search`       | `obsidian search query=<text> path=<path> limit=<n> format=<text|json>` | read-only              | `format=json` requires a recent build; fall back to `rawOutput` text parsing.              |
| `obsidian-daily-read`   | `obsidian daily date=<YYYY-MM-DD>`                                     | read-only              | Omit `date` for today.                                                                     |
| `obsidian-daily-append` | `obsidian daily date=<YYYY-MM-DD> append=<text>`                        | disk write             | Omit `date` to append to today's daily note.                                               |
| `obsidian-random-note`  | `obsidian random path=<path>`                                          | read-only              | Prints the random note's contents.                                                         |
| `obsidian-tag-list`     | `obsidian tags`                                                        | read-only              | `tag count` then `name` per line in modern builds.                                         |
| `obsidian-tag-notes`    | `obsidian tag name=<tag>`                                              | read-only              | One file path per line.                                                                    |
| `obsidian-task-list`    | `obsidian tasks state=<all|done|undone> path=<path>`                   | read-only              | Lines are `file:line` plus the task text.                                                  |
| `obsidian-task-toggle`  | `obsidian task file=<path> line=<n> state=<done|undone>`               | disk write             | Omit `state` to toggle.                                                                    |
| `obsidian-command`      | `obsidian command id=<command-id>`                                     | process spawn          | Command id format is `plugin-id:command-name`, e.g. `app:reload`.                          |

## Argument encoding

- All path arguments are **vault-relative**. Do not pass absolute filesystem paths; the CLI rejects them. Use forward slashes.
- Tag values do **not** include the leading `#`.
- The CLI does not accept POSIX-style `--flag value` arguments for these commands. Stick to `key=value`.
- Some commands support `path=<path>` scoping; some only support a vault-wide operation. See the table above for per-command behavior.
- Quoting: if a value contains spaces, the `obsidian` binary generally tolerates unquoted values that match the shell tokenization. When in doubt, escape with the host shell rules (e.g. PowerShell: `'"some value"'`).

## Exit codes and result shape

- Exit code 0 with non-empty stdout -> `status: "ok"`, `data` populated when the caller parses it, `rawOutput` always populated.
- Exit code 0 with empty stdout -> `status: "ok"`, `rawOutput: ""`. Treat as "no matches" for `search` and similar.
- Non-zero exit code -> `status: "error"`, `error` carries stderr (or the human-readable exit reason), `data` may still be populated if the CLI wrote a partial result to stdout.

The `obsidian-result/v1` envelope never echoes the original argv in clear text logs because the official CLI may include content snippets in argv values; redact before logging.

## CLI build differences

Obsidian ships new CLI commands with each minor release. Capabilities that depend on newer flags should fail soft:

- `obsidian search format=json` requires Obsidian 1.5+; older builds return an "unknown argument" error and you must fall back to text parsing of `rawOutput`.
- `obsidian task state=<value>` requires Obsidian 1.4+; older builds only support the toggle form (omit `state`).
- `obsidian patch mode=insert-after` requires Obsidian 1.6+; if unavailable, the CLI returns an error and the agent should rewrite the file with `read` + `append` or `create`.

Always run `obsidian version` once per session before exercising capabilities that depend on newer build features.

## Non-authoritative mirror convention

When the agent uses this skill to materialize a non-authoritative mirror of durable planning state, the resulting note must include the following frontmatter so downstream mirrors stay parseable:

```yaml
---
ie_kind: planning-mirror
ie_source: elegy-planning
ie_source_version: <planning schema version>
ie_correlation_id: <correlation id from the planning mutation>
ie_entity_type: <goal|roadmap|work-point|plan|todo|issue|review-point>
ie_entity_id: <id>
ie_mirrored_at: <ISO-8601 timestamp>
---
```

This frontmatter is the contract for the future `elegy-planning obsidian resolve` and `elegy-planning obsidian attach` commands. See `../../docs/specs/obsidian-skill-and-cli.md` and `../../docs/research/obsidian-figma-and-vision-models-for-elegy.md` for the broader design.
