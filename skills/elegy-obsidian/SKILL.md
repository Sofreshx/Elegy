---
name: elegy-obsidian
description: "Read, write, and search a local Obsidian vault via the official Obsidian Desktop CLI (v1.12.7+). Non-authoritative convenience layer; durable planning state continues to flow through elegy-planning and SQLite. User-invoked; do not auto-load."
license: Apache-2.0
version: "2.1"
metadata: {"aliasKeys":["obsidian","obsidian-cli","elegy-obsidian"],"stacks":["vault","notes"],"tags":["obsidian","vault","notes","search","tasks","tags","elegy","non-authoritative-mirror"]}
disable-model-invocation: true
---

> **Invocation posture**: user-invoked only. This skill wraps the Obsidian CLI and can mutate vault content (create, append, patch, move, delete, task toggle). It must not be auto-invoked by the model.

# Elegy Obsidian

## Purpose

`elegy-obsidian` is a convenience layer over the official `obsidian` CLI
shipped with Obsidian Desktop 1.12.7+. It is a foundation surface only;
it deliberately does **not** mirror, attach, resolve, or list Obsidian
vault state into the Elegy planning authority. That work belongs to a
future `elegy-planning obsidian ...` Rust subcommand set.

Obsidian is treated as a **non-authoritative** mirror:

- The official `obsidian` CLI is a black-box process boundary; the
  skill does not parse or normalize its output into an Elegy
  planning schema.
- Durable cross-session state continues to live in `elegy-planning`
  (goals, roadmaps, plans, todos, issues, review points) backed by
  SQLite.
- Vault reads, file appends, and search results must not be written
  into paths that would shadow planning authority (for example,
  `.elegy/backlogs/`, `roadmaps/`, or any spec/ADR location owned
  by another skill).

## Prerequisite

The official `obsidian` binary must be available. It is shipped with
Obsidian Desktop 1.12.7+; the CLI is not enabled by default. The
Obsidian Desktop app must also be running for the CLI to function —
commands fail if the app is not open.

If the `obsidian` CLI is not installed or not reachable, this skill
cannot function — inform the user and offer to install Obsidian
Desktop or enable the CLI in Obsidian Settings -> General ->
"Command line interface".

## Binary Resolution

The `obsidian` binary must be reachable from the current shell. Use
this resolution order:

1. **Check PATH**: run `obsidian version`. If it resolves, use `obsidian`
   directly for all commands.
2. **WSL / non-PATH fallback**: on Windows Subsystem for Linux, the
   `obsidian` app-execution alias is on the Windows PATH but not the
   WSL PATH. Resolve via:
   ```bash
   cmd.exe /c "where obsidian"
   ```
   This returns the full Windows path (e.g.,
   `C:\Users\<user>\AppData\Local\Programs\Obsidian\Obsidian.com`).
   Convert to a WSL path and use it for all subsequent invocations.
3. **Known install root**: if `where` fails, check the default
   Obsidian Desktop install location (typically under the user's
   local AppData Programs directory).

Once resolved, use the full binary path for every `obsidian` call in
the session. Do not re-resolve unless the first attempt fails.

If none of the above resolve, the CLI is not installed or not enabled.
Guide the user to install Obsidian Desktop 1.12.7+ and enable the CLI
under Settings -> General -> Command line interface.

## Vault Context

Before invoking any vault command, load the vault context:

1. **Read config**: `~/.elegy/obsidian-vault.json` contains
   `vaultPath` (the vault root). Derive the vault name from the
   folder basename (e.g., a vault at `~/Documents/Dev` → vault
   name `Dev`).
2. **Fallback**: if config is absent, run `obsidian vault list` to
   discover registered vaults. Use the first (or prompt the user
   to choose if multiple).
3. **Pin vault name**: pass `vault=<name>` on every subsequent
   `obsidian` call. The active-vault default is platform-specific
   and unreliable when multiple vaults are registered.

### Vault layout conventions

Obsidian vaults use theme-based subdirectories. The standard layout
(for a vault named `Dev`):

```text
Dev/
  Projects/       ← project notes (theme: Projects)
  Research/       ← research notes (theme: Research)
  Tasks/          ← task lists (theme: Tasks)
  Resources/      ← resource/link collections (theme: Resources)
  Index.md        ← navigation hub (links to all sections)
```

Frontmatter conventions (per the vault-notes spec):

```yaml
---
id: my-note-slug        # derived from filename (My-Note.md → my-note)
title: My Note          # human-readable title
theme: Projects         # derived from parent directory name
tags: [tag1, tag2]      # YAML array
created: ISO-timestamp
updated: ISO-timestamp
archived: false
---
```

These are conventions, not enforced. The `theme` field corresponds to
the parent directory name. `Index.md` at the vault root is the
navigation hub — read it first to orient within the vault.

## Orient-Once Protocol

At the start of each session (or when the user asks to interact with
the vault), run this 3-command bootstrap:

1. `obsidian version` — confirm CLI is reachable, note version.
2. `obsidian vault list` — confirm vault name and path (or read from
   `~/.elegy/obsidian-vault.json`).
3. `obsidian read file=Index.md vault=<name>` — load the navigation
   hub to understand the vault's structure and current state.

After this orient pass, the session has the binary path, vault name,
and vault layout pinned. No further re-discovery is needed.

**Orient is complete when all three commands succeed** — the binary
path, vault name, and vault layout are pinned for the session.

## Installation

### Enable the official `obsidian` CLI

1. Install Obsidian Desktop 1.12.7 or later from
   <https://obsidian.md/download>.
2. In Obsidian, open **Settings -> General** and enable
   **Command line interface** (also exposed under
   **Settings -> General -> Advanced -> Command line interface** on
   some builds).
3. Verify the CLI:
   ```bash
   obsidian version
   obsidian help
   ```
4. The CLI uses `key=value` arguments and the active vault by
   default; pass `vault=<name>` to target a specific vault.

### No `elegy-obsidian` binary

This skill does **not** ship an executable. It is a routing and
convention layer over the official `obsidian` CLI.

## Core Commands

Full command reference: [`references/obsidian-cli-catalog.md`](references/obsidian-cli-catalog.md).

Key commands: `read`, `search`, `daily`, `daily:read`, `tags`, `tasks` (read-only).
Side-effecting commands (`create`, `append`, `patch`, `move`, `delete`, `daily:append`, `task toggle`, `command`) require explicit user approval.

## Quick Reference

| Task | Command |
|---|---|
| List vaults | `obsidian vault list` |
| Read a note | `obsidian read file=notes/foo.md vault=Dev` |
| Append to a note | `obsidian append file=notes/foo.md content="New line" vault=Dev` |
| Search the vault | `obsidian search query="planning" limit=20 vault=Dev` |
| Read today's daily note | `obsidian daily:read vault=Dev` |
| Append to today's daily note | `obsidian daily:append content="- [ ] triage obsidian queue" vault=Dev` |
| List open tasks | `obsidian tasks status=open vault=Dev` |
| Toggle a task | `obsidian task toggle file=notes/daily.md line=12 vault=Dev` |

## Rules

- Always pass `vault=<name>` explicitly when more than one vault is
  registered; the active-vault default is platform-specific.
- Treat the `obsidian` CLI as a black-box process boundary. Do not
  parse, normalize, or reinterpret its output into Elegy planning
  schemas.
- Do not write vault content into paths that would shadow Elegy
  planning authority (`.elegy/backlogs/`, `roadmaps/`, ADR/spec
  locations, governance nodes per the repo discovery chain).
- Respect the three mandatory constraints:
  `external-binary-dependency`, `non-authoritative-vault`,
  `no-projection-of-authority`.
- Use the resolved binary path consistently across the session; do
  not re-resolve unless an invocation fails.

### Rollback notes

- File mutations (`create`, `append`, `patch`, `move`, `delete`) are
  not reversible through the CLI. Recommend backing up before
  deletes/moves.
- `delete` is irreversible — the file is removed from the vault.
- `move` can be reversed by moving back, but wiki-link rewrites
  applied by Obsidian during the move may not fully reverse.
- `task toggle` is reversible by toggling again.
- `daily:append` can be undone by editing the daily note in Obsidian.

## Authority Chain

| Artifact | Location | Role |
|---|---|---|
| **SKILL.md** (canonical) | `skills/elegy-obsidian/SKILL.md` | Governed source of truth for the skill |
| **plugin.json** (canonical) | `skills/elegy-obsidian/.elegy-plugin/plugin.json` | Plugin package manifest |
| **Surface registration** | `distribution/surfaces.json` (`elegy-obsidian` entry) | Release catalog entry |
| **Marketplace packaging** | `.elegy/marketplace.json` (`elegy-obsidian` entry) | Install/archive packaging |
| **CLI catalog** (canonical) | `skills/elegy-obsidian/references/obsidian-cli-catalog.md` | Full command/capability reference |
| **Consumer-side definition** (mirror) | `instruction-engine/contracts/elegy/fixtures/skill-definition-v2.elegy-obsidian.json` | Governed definition for consumer repo |
| **Consumer-side discovery** (mirror) | `instruction-engine/contracts/elegy/fixtures/skill-discovery-index.elegy-obsidian.json` | Discovery projection for consumer repo |
| **Consumer-side SKILL.md** (mirror) | `instruction-engine/catalog-assets/shared-skills/elegy-obsidian/SKILL.md` | Consumer repo mirror of this skill |

The `elegy-skills` registry in the Elegy repo discovers this skill by
scanning `skills/elegy-obsidian/SKILL.md` and
`.elegy-plugin/plugin.json`. No separate `contracts/fixtures/` tree is
used for skill-package discovery in this repo.

## Future Work (not in foundation)

The following belong to a future Rust surface under
`rust/crates/elegy-planning/src/obsidian.rs` and the
`elegy-planning` CLI umbrella, **not** to this skill:

- `elegy-planning obsidian mirror` -- snapshot a vault subtree into
  the planning catalog as a non-authoritative mirror.
- `elegy-planning obsidian attach` -- bind a vault path to a goal,
  roadmap, plan, or work point as a related surface.
- `elegy-planning obsidian resolve` -- answer a planning query
  (entity, identifier) by walking the attached mirrors.
- `elegy-planning obsidian list` -- enumerate attached mirrors and
  their state.
- Promotion of this skill's `lifecycleState` from `draft` to
  `active` once the mirror command set lands.
