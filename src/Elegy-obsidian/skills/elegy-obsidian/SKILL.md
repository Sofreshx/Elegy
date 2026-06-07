---
name: elegy-obsidian
description: Use when an agent needs to create, read, search, patch, or toggle tasks inside the user's local Obsidian vault through the official Obsidian v1.12+ CLI. Treats Obsidian as a non-authoritative vault mirror; durable planning state continues to flow through elegy-planning and SQLite.
---

# Elegy-obsidian Surface Bridge

This file is the surface-local, non-authoritative skill bridge shipped
with the `src/Elegy-obsidian` wrapper surface and the
`elegy-obsidian-wrapper-<bundleVersion>.zip` archive. It is a thin
install-and-handoff page; the canonical operational body lives in the
in-tree `skills/elegy-obsidian/SKILL.md` and is mirrored to
`.agents/skills/elegy-obsidian/SKILL.md` and
`.github/skills/elegy-obsidian/SKILL.md`.

Authority stays one-way:

1. `contracts/fixtures/skill.elegy-obsidian.json` is the governed
   source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-obsidian.json` is
   the governed discovery projection.
3. `skills/elegy-obsidian/SKILL.md` is the canonical operational body.
4. This file mirrors install and CLI handoff needed by wrapper
   consumers.

## Wrapper install

- Run `./install.ps1` from this wrapper root to stage the contracts
  bundle and this wrapper surface together.
- Pass `-LocalArtifactsRoot <path>` when validating against local
  archives instead of GitHub release assets.
- The wrapper archive does **not** ship the `obsidian` executable. The
  user must enable the official CLI in Obsidian Desktop (Settings →
  General → Command line interface) before any capability will run. See
  `../../../skills/elegy-obsidian/references/install-obsidian-cli.md`.

## External binary

The skill targets the official `obsidian` CLI shipped with Obsidian
Desktop 1.12+. There is no Elegy-owned `elegy-obsidian` binary and
there is no Rust crate in `rust/crates/`. The executable must be on
`PATH` for the skill to work. The canonical operational body documents
the full capability index, tool-call guardrails, and the
`obsidian-result/v1` envelope at
`../../../skills/elegy-obsidian/SKILL.md`.

## Behavior notes

- Obsidian is a non-authoritative vault. SQLite via `elegy-planning`
  remains the durable planning authority.
- Mirror notes written through this skill should carry `ie_kind`,
  `ie_source`, `ie_entity_type`, and `ie_entity_id` frontmatter so
  future `elegy-planning obsidian resolve` and `attach` commands can
  parse them.
- The official CLI is a desktop-application channel. Some commands
  open Obsidian windows or focus the application. Treat side effects
  as `desktop_ui` by default; down-classify to `disk_write` only
  when you have verified the specific command does not interact with
  the UI.

## Agent invocation guidance

- Always run `obsidian version` first to confirm the CLI is enabled
  and modern enough. Minimum supported is 1.12.0.
- Prefer read-only capabilities first. Confirm scope before
  `obsidian-file-delete` and `obsidian-task-toggle`.
- Use `obsidian-daily-append` with `date=<YYYY-MM-DD>` to backfill
  historical daily notes; omit `date` for today.
- Keep the `obsidian-result/v1` envelope when calling from agent
  surfaces. Populate `rawOutput` even when `data` is structured so
  callers that need the raw text can use it.
- For the full guardrails, common issues, and worked examples, load
  the canonical body: `../../../skills/elegy-obsidian/SKILL.md`.
