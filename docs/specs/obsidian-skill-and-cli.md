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
- Distributed as a skill-only package under `plugins/obsidian/SKILL.md`. No binary to install — skill is consumed by hosts directly.
- Exposes a uniform `obsidian-result/v1` envelope so callers in any agent can reason about outcomes.
- Leaves a clean extension point for future mirror commands without committing to them yet.

This document captures the foundation slice.

## 2. Scope

In scope:

- A governed skill definition: `plugins/obsidian/SKILL.md` (plugin skill-only package).
- Discovery projection: the `elegy-skills` registry discovers this skill from `plugins/obsidian/`.
- Result envelope schema: defined in the shared `plugin-sdk` (`AgentSkillFrontmatter` struct).
- A skill-only plugin: `plugins/obsidian/SKILL.md` plus per-command reference and install guidance in the skill body.
- A foundation spec: this document.

Out of scope (the `src/Elegy-*/` wrapper installer lanes are retired; the skill package is at `plugins/obsidian/`. Hosts discover it through the `elegy-skills` registry.):

Out of scope (follow-up work):

- No dedicated Rust binary. This is a skill-only surface that wraps the user's installed Obsidian CLI.
- New subcommands on `elegy-planning` (`obsidian mirror/attach/resolve/list`) and the mirror schemas that go with them.
- Wiring this skill into `instruction-engine`'s own skill catalog (catalog-assets or opencode-assets). That is the consumer-repo change and belongs in a separate PR against `instruction-engine`.
- Replacement of the third-party `obsidian-cli.exe` referenced by `instruction-engine/docs/system/obsidian-synced-notes-contract.md`. That contract governs a separate obsidian integration lane and remains non-canonical; the new skill does not change its authority.

## 3. Authority model

The skill follows the standard Elegy one-way authority chain:

```
plugins/obsidian/SKILL.md                          (plugin package — governed source of truth)
        |
        v
elegy-skills registry                              (discovery projection)
        |
        v
plugins/obsidian/SKILL.md                                (skill-only plugin package)
        |
        v
plugins/obsidian/SKILL.md                                (skill-only plugin package)
```

`plugins/obsidian/` is the skill-only plugin package, not a wrapper overlay. The implementation does not live in this repo; the user's Obsidian Desktop installation provides the `obsidian` binary.

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

The result envelope, defined in the shared `plugin-sdk` (`AgentSkillFrontmatter` struct), provides a minimal, forward-compatible envelope:

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
2. In the SKILL.md and the wrapper README, mirror notes that the skill produces must carry the `ie_kind: planning-mirror` frontmatter. The frontmatter is the future contract that `elegy-planning obsidian resolve` and `attach` will rely on.

## 7. Installation and consumer story

- **Elegy-side** — `elegy-obsidian` is a recognized surface. The skill package is registered in `distribution/surfaces.json` as `kind: skill-only`. Hosts resolve it through the `elegy-skills` registry. There is no `bin/elegy-obsidian/` directory because there is no Rust binary.
- **elegant-obsidian-side on consumer machines** — the user must enable the official CLI once via Obsidian Desktop's Settings -> General -> Command line interface. The plugin package does not ship the binary, does not download it, and does not install it.
- **elegy-copilot / instruction-engine side** — to make the skill loadable from opencode and from the elegy-copilot runtime, the skill must be mirrored into the consumer repo (`instruction-engine`) under its skill discovery lane. That is a follow-up change in the consumer repo and is tracked as out-of-scope for this foundation PR.
- **elegy-copilot/obsidian contract** — `instruction-engine/docs/system/obsidian-synced-notes-contract.md` already defines a separate Obsidian integration lane that uses a third-party `obsidian-cli.exe` binary. The new skill is **additive** — it does not modify that contract or replace that binary. The two lanes can coexist.

## 8. Future extension point

The research note describes the longer-term direction: add `elegy-planning obsidian mirror/attach/resolve/list` subcommands, plus a new schema family for mirror notes. The foundation deliberately leaves clean seams for that work:

- the fixture's `lifecycleState` is `draft`, not `active`. Promotion to `active` will accompany the mirror command set, not this foundation alone.
- The skill's `capabilityHints` are listed in priority order in the research note; the foundation implements priorities 2 (vault/file/daily/tag/task capabilities), 3 (search), 4 (command/eval escape hatch via `obsidian-command`), and 7 (`obsidian-version` precondition). Priorities 1 (mirror commands), 5 (link/follow/unlinked), and 6 (bookmarks) remain future work.
- The mirror frontmatter convention is documented in the skill's SKILL.md body content (refer to skill frontmatter for invocation details) and will be the parsing contract for the future `elegy-planning obsidian resolve` and `attach` commands.
- Adding a future plugin crate (under `plugins/`) is a localized change: drop in a new `cliCrate` in `delegatesTo`, add a new entry in the canonical installer's `Get-CliSurfaceMetadata` table, and update the plugin package's `instructionSkills` projection.

## 9. Acceptance criteria for this foundation

The foundation is complete when all of the following are true:

- the SKILL.md frontmatter validates against the `AgentSkillFrontmatter` struct in `shared/plugin-sdk`. Registry discovery validates through the `SkillRegistry` in `plugins/skills`.
- The result envelope schema validates against the JSON Schema 2020-12 grammar.
- The `elegy-skills` registry resolves the surface metadata from the standalone root package.
- Installation guidance lives in the skill body content of `plugins/obsidian/SKILL.md` that operators can follow to enable the official CLI.
- The `elegy-obsidian` skill is registered in the `elegy-skills` registry with `lifecycleState: "draft"`, signaling that the foundation is ready for review but not yet promoted to active.
- The `distribution/surfaces.json` entry for `elegy-obsidian` registers the surface for release and install.

## 10. Open questions

- Where exactly should the skill mirror in `instruction-engine` (catalog-assets vs opencode-assets vs both)? This is a consumer-repo decision and a follow-up PR.
- Should the skill be promoted to `lifecycleState: "active"` independently, or only after the mirror command set lands? The current draft is a deliberate gate.
- Will the `obsidian-command` escape hatch get per-command whitelisting, or remain a free-form dispatcher? The foundation takes the simpler path and notes the trade-off.
