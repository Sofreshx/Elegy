---
title: Elegy Obsidian Skill and CLI
status: draft
owner: Elegy
date: 2026-07-08
---

# Elegy Obsidian Skill and CLI — Foundation Spec

Status: foundation (draft)
Owner: Elegy contributors
Governs: `elegy-obsidian` skill foundation; the future `elegy-planning obsidian mirror/attach/resolve/list` commands are out of scope for this document and belong to a follow-up spec.

## 1. Problem and motivation

Elegy treats planning state as durable and authoritative in SQLite via the `elegy-planning` CLI. Operators and agents still need a clean, governed way to interact with the user's local **Obsidian vault** — a separate tool that the operator runs locally, that the user authors notes in, and that may eventually host a non-canonical mirror of planning state.

Today there is no first-party Elegy skill for that. The pablo-mano/Obsidian-CLI-skill demonstrates a viable shape (a thin skill wrapping the official Obsidian CLI), but Elegy needs a version that:

- Sits inside the Elegy contracts and discovery surface (`skill`, `skill-discovery-index`).
- Distributed as a skill-only package under `skills/elegy-obsidian/SKILL.md`. No binary to install — skill is consumed by hosts directly.
- Agents reason over raw CLI stdout directly (no structured envelope imposed).
- Leaves a clean extension point for future mirror commands without committing to them yet.

This document captures the foundation slice.

## 2. Scope

In scope:

- A governed skill definition: `skills/elegy-obsidian/SKILL.md` (plugin skill-only package).
- Discovery projection: the `elegy-skills` registry discovers this skill from `skills/elegy-obsidian/`.
- A skill-only plugin: `skills/elegy-obsidian/SKILL.md` plus per-command reference and install guidance in the skill body.
- A foundation spec: this document.

Out of scope (follow-up work):

- No dedicated Rust binary. This is a skill-only surface that wraps the user's installed Obsidian CLI.
- New subcommands on `elegy-planning` (`obsidian mirror/attach/resolve/list`) and the mirror schemas that go with them.
- Replacement of the third-party `obsidian-cli.exe` referenced by `instruction-engine/docs/system/obsidian-synced-notes-contract.md`. That contract governs a separate obsidian integration lane and remains non-canonical; the new skill does not change its authority.

## 3. Authority model

The skill follows the Elegy one-way authority chain. The canonical
source is `skills/elegy-obsidian/` in the Elegy repo; the
`elegy-skills` registry discovers it by scanning the SKILL.md and
`.elegy-plugin/plugin.json`. No separate `contracts/fixtures/` tree is
used for skill-package discovery in this repo.

| Artifact | Location | Role |
|---|---|---|
| **SKILL.md** (canonical) | `skills/elegy-obsidian/SKILL.md` | Governed source of truth |
| **plugin.json** (canonical) | `skills/elegy-obsidian/.elegy-plugin/plugin.json` | Plugin package manifest |
| **CLI catalog** (canonical) | `skills/elegy-obsidian/references/obsidian-cli-catalog.md` | Full command/capability reference |
| **Surface registration** | `distribution/surfaces.json` (`elegy-obsidian` entry) | Release catalog |
| **Marketplace packaging** | `.elegy/marketplace.json` (`elegy-obsidian` entry) | Install/archive packaging |
| **Consumer-side definition** (mirror) | `instruction-engine/contracts/elegy/fixtures/skill-definition-v2.elegy-obsidian.json` | Governed definition for consumer repo |
| **Consumer-side discovery** (mirror) | `instruction-engine/contracts/elegy/fixtures/skill-discovery-index.elegy-obsidian.json` | Discovery projection for consumer repo |
| **Consumer-side SKILL.md** (mirror) | `instruction-engine/catalog-assets/shared-skills/elegy-obsidian/SKILL.md` | Consumer repo mirror |

`skills/elegy-obsidian/` is a skill-only plugin package. The
implementation does not live in this repo; the user's Obsidian Desktop
installation provides the `obsidian` binary. The skill resolves the
binary at runtime via PATH or WSL fallback (see Binary Resolution in
the SKILL.md).

## 4. Capability surface

The skill exposes 17 capabilities, organized as:

- **Precondition** — `obsidian-version`, `obsidian-vault-list`.
- **Read** — `obsidian-file-read`, `obsidian-search`, `obsidian-daily-read`, `obsidian-random-note`, `obsidian-tag-list`, `obsidian-tag-notes`, `obsidian-task-list`.
- **Write** — `obsidian-file-create`, `obsidian-file-append`, `obsidian-file-patch`, `obsidian-file-move`, `obsidian-file-delete`, `obsidian-daily-append`, `obsidian-task-toggle`.
- **Escape hatch** — `obsidian-command` to invoke any registered Obsidian command by id.

Every capability is `executionType: subprocess` and targets `executableName: obsidian`. The CLI returns text by default; agents reason over raw stdout (see Section 5).

Side-effect classification:

- `read_only` for preconditions and read operations.
- `disk_write` for file and task mutators.
- `process_spawn` for the escape hatch `obsidian-command`.
- The wrapper's `defaultSideEffectClass` is `desktop_ui` because the official CLI is a desktop-application channel; per-capability projections override this where appropriate.

## 5. Output convention

The official `obsidian` CLI returns text by default. The skill does not
wrap CLI output in a structured envelope — agents reason over raw
stdout directly. Some commands support `format=json|tsv|csv` for
structured output; check `obsidian help <command>` for specifics.

Error handling: non-zero exit codes indicate failure; read stderr for
diagnostics. The skill's Binary Resolution protocol handles the
common "command not found" case on WSL/non-PATH environments.

## 6. Non-authoritative vault boundary

Obsidian is **non-canonical**. Durable planning state continues to flow through `elegy-planning` and SQLite. This skill encodes that boundary two ways:

1. Three constraints are mandatory:
   - `external-binary-dependency` — the skill shells out to the official CLI; no custom binary.
   - `non-authoritative-vault` — the skill must never be the source of truth for planning entities.
   - `no-projection-of-authority` — the skill must not write into paths that shadow planning authority (`.elegy/backlogs/`, `roadmaps/`, ADR/spec locations, etc.).
2. Mirror notes that the skill produces must carry `ie_kind: planning-mirror` frontmatter. That frontmatter is the future parsing contract for `elegy-planning obsidian resolve` and `attach`.

## 7. Installation and consumer story

- **Elegy-side** — `elegy-obsidian` is a recognized surface. The skill package is registered in `distribution/surfaces.json` as `kind: skill-package`. Hosts resolve it through the `elegy-skills` registry. There is no Rust binary; the skill is a routing/convention layer over the official `obsidian` CLI.
- **Consumer machines** — the user must enable the official CLI once via Obsidian Desktop's Settings -> General -> Command line interface. The skill includes Binary Resolution for WSL/non-PATH environments.
- **instruction-engine side** — the skill is mirrored into `catalog-assets/shared-skills/elegy-obsidian/` (SKILL.md + references/). The consumer-side governed fixtures live in `contracts/elegy/fixtures/`. An `obsidian-lanes.md` routing node documents the three Obsidian lanes (agent skill, vault-notes, planning-mirror).
- **Obsidian lanes** — `instruction-engine/docs/system/obsidian-synced-notes-contract.md` governs the planning-mirror lane (third-party CLI, `obsidian-planning.json`). `instruction-engine/docs/system/repo-backed-obsidian-docs.md` governs using Obsidian as a viewer over repo docs. The `elegy-obsidian` skill is **additive** — it does not replace either contract. A routing node (`obsidian-lanes.md`) maps need to lane.

## 8. Future extension point

The research note describes the longer-term direction: add `elegy-planning obsidian mirror/attach/resolve/list` subcommands, plus a new schema family for mirror notes. The foundation deliberately leaves clean seams for that work:

- the fixture's `lifecycleState` is `draft`, not `active`. Promotion to `active` will accompany the mirror command set, not this foundation alone.
- The skill's `capabilityHints` are listed in priority order in the research note; the foundation implements priorities 2 (vault/file/daily/tag/task capabilities), 3 (search), 4 (command/eval escape hatch via `obsidian-command`), and 7 (`obsidian-version` precondition). Priorities 1 (mirror commands), 5 (link/follow/unlinked), and 6 (bookmarks) remain future work.
- The mirror frontmatter convention is documented in the skill's SKILL.md body content (refer to skill frontmatter for invocation details) and will be the parsing contract for the future `elegy-planning obsidian resolve` and `attach` commands.
- Adding a future plugin crate (under `plugins/`) is a localized change: drop in a new `cliCrate` in `delegatesTo`, add a new entry in the canonical installer's `Get-CliSurfaceMetadata` table, and update the plugin package's `instructionSkills` projection.

## 9. Acceptance criteria for this foundation

The foundation is complete when all of the following are true:

- The SKILL.md frontmatter validates. Registry discovery validates through the `SkillRegistry` in `tools/skills`.
- The `elegy-skills` registry resolves the surface metadata from the standalone root package (`cargo test -p elegy-skills`).
- The SKILL.md includes Binary Resolution, Vault Context, and Orient-Once Protocol sections.
- Installation guidance lives in the skill body content of `skills/elegy-obsidian/SKILL.md` that operators can follow to enable the official CLI.
- The `elegy-obsidian` skill is registered in the `elegy-skills` registry with `lifecycleState: "draft"`, signaling that the foundation is ready for review but not yet promoted to active.
- The `distribution/surfaces.json` entry for `elegy-obsidian` registers the surface for release and install.
- The consumer-side mirror in `instruction-engine/catalog-assets/shared-skills/elegy-obsidian/` matches the canonical source.
- A routing node (`instruction-engine/docs/system/obsidian-lanes.md`) maps each Obsidian need to its lane.

## 10. Open questions

- Should the skill be promoted to `lifecycleState: "active"` independently, or only after the mirror command set lands? The current draft is a deliberate gate.
- Will the `obsidian-command` escape hatch get per-command whitelisting, or remain a free-form dispatcher? The foundation takes the simpler path and notes the trade-off.
