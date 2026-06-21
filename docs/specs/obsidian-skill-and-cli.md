---
title: Elegy Obsidian Skill and CLI
status: draft
owner: Elegy
date: 2026-06-02
---

# Elegy Obsidian Skill and CLI — Foundation Spec

Status: foundation (draft)
Owner: Elegy contributors
Governs: `elegy-obsidian` skill foundation; the future `elegy-planning obsidian mirror/attach/resolve/list` commands are out of scope for this document and belong to a follow-up spec.

## 1. Problem and motivation

Elegy treats planning state as durable and authoritative in SQLite via the `elegy-planning` CLI. Operators and agents still need a clean, governed way to interact with the user's local **Obsidian vault** — a separate tool that the operator runs locally, that the user authors notes in, and that may eventually host a non-canonical mirror of planning state.

Today there is no first-party Elegy skill for that. The pablo-mano/Obsidian-CLI-skill demonstrates a viable shape (a thin skill wrapping the official Obsidian CLI), but Elegy needs a version that:

- Sits inside the Elegy contracts and discovery surface (`skill`, `skill-discovery-index`).
- Is installable through the same `scripts/install-distribution.ps1` flow as other Elegy surfaces.
- Exposes a uniform `obsidian-result/v1` envelope so callers in any agent can reason about outcomes.
- Leaves a clean extension point for future mirror commands without committing to them yet.

This document captures the foundation slice.

## 2. Scope

In scope:

- A governed skill definition: `contracts/fixtures/skill.elegy-obsidian.json`.
- A discovery projection: `contracts/fixtures/skill-discovery-index.elegy-obsidian.json`.
- A result envelope schema: `contracts/schemas/obsidian-result.schema.json`.
- A repo-local skill: `skills/elegy-obsidian/SKILL.md` plus a per-command reference and an install guide.
- A wrapper surface: `src/Elegy-obsidian/` (entrypoint, README, install.ps1, helper lanes, surface-local bridge).
- Installer wiring: `elegy-obsidian` added to `Get-WrapperSurfaceMetadata` in `scripts/install-distribution.ps1`.
- A foundation spec: this document.

Out of scope (follow-up work):

- A Rust crate at `rust/crates/elegy-obsidian/`. The current implementation is the user's installed `obsidian` CLI.
- New subcommands on `elegy-planning` (`obsidian mirror/attach/resolve/list`) and the mirror schemas that go with them. See `docs/research/obsidian-figma-and-vision-models-for-elegy.md` for the proposed shape.
- Wiring this skill into `instruction-engine`'s own skill catalog (catalog-assets or opencode-assets). That is the consumer-repo change and belongs in a separate PR against `instruction-engine`.
- Replacement of the third-party `obsidian-cli.exe` referenced by `instruction-engine/docs/system/obsidian-synced-notes-contract.md`. That contract governs a separate obsidian integration lane and remains non-canonical; the new skill does not change its authority.

## 3. Authority model

The skill follows the standard Elegy one-way authority chain:

```
contracts/fixtures/skill.elegy-obsidian.json      (governed source of truth)
        |
        v
contracts/fixtures/skill-discovery-index.elegy-obsidian.json   (discovery projection)
        |
        v
skills/elegy-obsidian/SKILL.md                                 (repo-local skill output)
        |
        v
src/Elegy-obsidian/skills/elegy-obsidian/SKILL.md              (wrapper surface bridge)
```

`src/Elegy-obsidian/` is a **wrapper overlay** in the sense the Elegy `AGENTS.md` defines for `src/Elegy-*` directories: contributor-navigation, not authority. The implementation does not live in this repo; the user's Obsidian Desktop installation provides the `obsidian` binary.

The wrapper surface points to the external executable explicitly:

```json
"delegatesTo": {
  "externalExecutable": {
    "name": "obsidian",
    "kind": "binary",
    "source": "obsidian-desktop",
    "minVersion": "1.12.0"
  }
}
```

`authority.implementation` is set to `external://obsidian-desktop` to make the boundary explicit in the wrapper contract.

## 4. Capability surface

The skill exposes 17 capabilities, organized as:

- **Precondition** — `obsidian-version`, `obsidian-vault-list`.
- **Read** — `obsidian-file-read`, `obsidian-search`, `obsidian-daily-read`, `obsidian-random-note`, `obsidian-tag-list`, `obsidian-tag-notes`, `obsidian-task-list`.
- **Write** — `obsidian-file-create`, `obsidian-file-append`, `obsidian-file-patch`, `obsidian-file-move`, `obsidian-file-delete`, `obsidian-daily-append`, `obsidian-task-toggle`.
- **Escape hatch** — `obsidian-command` to invoke any registered Obsidian command by id.

Every capability is `executionType: subprocess` and targets `executableName: obsidian`. The `obsidian-result/v1` envelope captures the outcome uniformly.

Side-effect classification:

- `read_only` for preconditions and read operations.
- `disk_write` for file and task mutators.
- `process_spawn` for the escape hatch `obsidian-command`.
- The wrapper's `defaultSideEffectClass` is `desktop_ui` because the official CLI is a desktop-application channel; per-capability projections override this where appropriate.

## 5. Result envelope

`contracts/schemas/obsidian-result.schema.json` defines a minimal, forward-compatible envelope:

```json
{
  "schemaVersion": "obsidian-result/v1",
  "command": ["obsidian", "read", "file=notes/foo.md"],
  "status": "ok" | "error",
  "vault": "vault-name" | null,
  "data": <freeform>,
  "rawOutput": "verbatim stdout",
  "error": "human-readable error" | null
}
```

The `data` shape is intentionally freeform because the official CLI returns text by default and only a small subset of commands support `format=json`. Callers that need a structured value must parse `rawOutput` themselves; the skill does not impose a tighter shape at the foundation stage.

## 6. Non-authoritative vault boundary

Obsidian is **non-canonical**. Durable planning state continues to flow through `elegy-planning` and SQLite. This skill encodes that boundary two ways:

1. In the fixture, three constraints are mandatory:
   - `external-binary-dependency` — the skill shells out to the official CLI; no custom binary.
   - `non-authoritative-vault` — the skill must never be the source of truth for planning entities.
   - `no-projection-of-authority` — the skill must not write into paths that shadow planning authority (`.copilot/backlogs`, `~/.copilot/backlogs/{repo}/planning/`, etc.).
2. In the SKILL.md and the wrapper README, mirror notes that the skill produces must carry the `ie_kind: planning-mirror` frontmatter described in `docs/research/obsidian-figma-and-vision-models-for-elegy.md`. The frontmatter is the future contract that `elegy-planning obsidian resolve` and `attach` will rely on.

## 7. Installation and consumer story

- **Elegy-side** — `elegy-obsidian` is now a recognized wrapper surface. The repo `scripts/install-distribution.ps1` accepts `-WrapperSurfaces @('elegy-obsidian')`. The `elegy-obsidian-wrapper-<bundleVersion>.zip` archive ships the contracts bundle and the wrapper surface. There is no `bin/elegy-obsidian/` directory because there is no Rust binary.
- **elegant-obsidian-side on consumer machines** — the user must enable the official CLI once via Obsidian Desktop's Settings -> General -> Command line interface. The wrapper does not ship the binary, does not download it, and does not install it.
- **elegy-copilot / instruction-engine side** — to make the skill loadable from opencode and from the elegy-copilot runtime, the skill must be mirrored into the consumer repo (`instruction-engine`) under its skill discovery lane. That is a follow-up change in the consumer repo and is tracked as out-of-scope for this foundation PR.
- **elegy-copilot/obsidian contract** — `instruction-engine/docs/system/obsidian-synced-notes-contract.md` already defines a separate Obsidian integration lane that uses a third-party `obsidian-cli.exe` binary. The new skill is **additive** — it does not modify that contract or replace that binary. The two lanes can coexist.

## 8. Future extension point

The research note describes the longer-term direction: add `elegy-planning obsidian mirror/attach/resolve/list` subcommands, plus a new schema family for mirror notes. The foundation deliberately leaves clean seams for that work:

- the fixture's `lifecycleState` is `draft`, not `active`. Promotion to `active` will accompany the mirror command set, not this foundation alone.
- The skill's `capabilityHints` are listed in priority order in the research note; the foundation implements priorities 2 (vault/file/daily/tag/task capabilities), 3 (search), 4 (command/eval escape hatch via `obsidian-command`), and 7 (`obsidian-version` precondition). Priorities 1 (mirror commands), 5 (link/follow/unlinked), and 6 (bookmarks) remain future work.
- The mirror frontmatter convention is documented in `skills/elegy-obsidian/references/obsidian-cli-command-reference.md` and will be the parsing contract for the future `elegy-planning obsidian resolve` and `attach` commands.
- The wrapper surface structure mirrors the existing `src/Elegy-planning/` shape, so adding a future `rust/crates/elegy-obsidian/` crate is a localized change: drop in a new `cliCrate` in `delegatesTo`, add an entry in `Get-CliSurfaceMetadata`, and add a new install layout key.

## 9. Acceptance criteria for this foundation

The foundation is complete when all of the following are true:

- the fixture validates against `contracts/schemas/skill.schema.json` and the discovery index validates against `contracts/schemas/skill-discovery-index.schema.json`.
- The result envelope schema validates against the JSON Schema 2020-12 grammar.
- `scripts/install-distribution.ps1` accepts `-WrapperSurfaces @('elegy-obsidian')` and resolves the wrapper surface metadata.
- A runbook exists in `skills/elegy-obsidian/references/install-obsidian-cli.md` that operators can follow to enable the official CLI.
- The `elegy-obsidian` skill is registered in `skill-discovery-index.elegy-obsidian.json` with `lifecycleState: "draft"`, signaling that the foundation is ready for review but not yet promoted to active.
- No file under `src/Elegy-obsidian/` claims implementation authority — the surface remains a wrapper overlay.

## 10. Open questions

- Where exactly should the skill mirror in `instruction-engine` (catalog-assets vs opencode-assets vs both)? This is a consumer-repo decision and a follow-up PR.
- Should the skill be promoted to `lifecycleState: "active"` independently, or only after the mirror command set lands? The current draft is a deliberate gate.
- Will the `obsidian-command` escape hatch get per-command whitelisting, or remain a free-form dispatcher? The foundation takes the simpler path and notes the trade-off.
