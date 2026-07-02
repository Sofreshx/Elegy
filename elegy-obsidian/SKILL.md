---
name: elegy-obsidian
description: Thin skill that wraps the official Obsidian v1.12+ CLI for vault-aware file creation, search, navigation, task toggling, and tag inspection. Non-authoritative mirror foundation for future elegy-planning obsidian planning-mirror commands.
version: "2.0"
---

# Elegy Obsidian Skill

Thin skill that wraps the official Obsidian v1.12+ CLI for vault-aware file creation, search, navigation, task toggling, and tag inspection.

## Capabilities

- `obsidian-vault-list`: List all Obsidian vaults registered with the desktop app.
- `obsidian-file-read`: Read the contents of a note by its path inside the active vault.
- `obsidian-file-create`: Create a new note in the active vault.
- `obsidian-file-append`: Append text to the end of a note without rewriting the existing content.
- `obsidian-file-patch`: Apply a targeted patch to a note: insert, replace, or delete at a specific line range.
- `obsidian-file-move`: Move or rename a note within the active vault, preserving Obsidian's wiki link rewrites when possible.
- `obsidian-file-delete`: Delete a note from the active vault.
- `obsidian-search`: Search the active vault for a free-text query and return matching file paths.
- `obsidian-daily-read`: Read today's daily note from the active vault.
- `obsidian-daily-append`: Append text to today's daily note in the active vault.
- `obsidian-random-note`: Open or print a random note from the active vault.
- `obsidian-tag-list`: List all tags used across the active vault.
- `obsidian-tag-notes`: List all notes in the active vault that carry a specific tag.
- `obsidian-task-list`: List tasks across the active vault.
- `obsidian-task-toggle`: Toggle the done state of a specific task.
- `obsidian-command`: Invoke a registered Obsidian command by its command id.
- `obsidian-version`: Report the installed obsidian CLI version.
