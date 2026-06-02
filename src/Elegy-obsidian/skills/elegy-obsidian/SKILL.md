---
name: elegy-obsidian
description: "Surface-local non-authoritative bridge shipped with the Elegy-obsidian wrapper surface and wrapper archive. Wraps the official Obsidian v1.12+ CLI for vault-aware note operations."
---

# Elegy-obsidian Surface Bridge

This file is a surface-local, non-authoritative skill bridge shipped with the `src/Elegy-obsidian` wrapper surface and the `elegy-obsidian-wrapper-<bundleVersion>.zip` archive.

Authority stays one-way:

1. `contracts/fixtures/skill-definition-v2.elegy-obsidian.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-obsidian.json` is the governed discovery projection.
3. `skills/elegy-obsidian/SKILL.md` remains the repo-local contributor-routing output.
4. This file mirrors install and CLI handoff needed by wrapper consumers.

## Wrapper install

- Run `./install.ps1` from this wrapper root to stage the contracts bundle and this wrapper surface together.
- Pass `-LocalArtifactsRoot <path>` when validating against local archives instead of GitHub release assets.
- The wrapper archive does **not** ship the `obsidian` executable. The user must enable the official CLI in Obsidian Desktop (Settings -> General -> Command line interface) before any capability will run. See `../../../skills/elegy-obsidian/references/install-obsidian-cli.md`.

## External binary

The skill targets the official `obsidian` CLI shipped with Obsidian Desktop 1.12+. There is no Elegy-owned `elegy-obsidian` binary and there is no Rust crate in `rust/crates/`. The executable must be on `PATH` for the skill to work.

## Capability index

The skill exposes the capability set defined in `contracts/fixtures/skill-definition-v2.elegy-obsidian.json`. Highlights:

- `obsidian-vault-list`, `obsidian-version` — precondition checks.
- `obsidian-file-read`, `obsidian-search`, `obsidian-daily-read`, `obsidian-tag-list`, `obsidian-tag-notes`, `obsidian-task-list`, `obsidian-random-note` — read-only operations.
- `obsidian-file-create`, `obsidian-file-append`, `obsidian-file-patch`, `obsidian-file-move`, `obsidian-file-delete`, `obsidian-daily-append`, `obsidian-task-toggle`, `obsidian-command` — mutating operations.

See `../../../skills/elegy-obsidian/references/obsidian-cli-command-reference.md` for the full per-command argument shape and CLI build differences.

## Behavior notes

- Obsidian is a non-authoritative vault. SQLite via `elegy-planning` remains the durable planning authority.
- Mirror notes written through this skill should carry `ie_kind`, `ie_source`, `ie_entity_type`, and `ie_entity_id` frontmatter so future `elegy-planning obsidian resolve` and `attach` commands can parse them.
- The official CLI is a desktop-application channel. Some commands open Obsidian windows or focus the application. Treat side effects as `desktop_ui` by default; down-classify to `disk_write` only when you have verified the specific command does not interact with the UI.

## Agent invocation guidance

- Always run `obsidian version` first to confirm the CLI is enabled and modern enough. Minimum supported is 1.12.0.
- Prefer read-only capabilities first. Confirm scope before `obsidian-file-delete` and `obsidian-task-toggle`.
- Use `obsidian-daily-append` with `date=<YYYY-MM-DD>` to backfill historical daily notes; omit `date` for today.
- Keep the `obsidian-result/v1` envelope when calling from agent surfaces. Populate `rawOutput` even when `data` is structured so callers that need the raw text can use it.
