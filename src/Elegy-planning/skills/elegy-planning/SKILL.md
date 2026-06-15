---
name: elegy-planning
description: Use when an agent needs to create, inspect, update, validate, or export durable planning state — goals, roadmaps, plans, work points, todos, issues, review points, insights, and project runs — through the dedicated elegy-planning CLI over SQLite.
---

# Elegy-planning Surface Bridge

This file is the surface-local, non-authoritative skill bridge shipped
with the `src/Elegy-planning` wrapper surface and the
`elegy-planning-wrapper-<bundleVersion>.zip` archive. It is a thin
install-and-handoff page; the canonical operational body lives in the
in-tree `skills/elegy-planning/SKILL.md` and is mirrored to
`.agents/skills/elegy-planning/SKILL.md` and
`.github/skills/elegy-planning/SKILL.md`.

Authority stays one-way:

1. `contracts/fixtures/skill.elegy-planning.json` is the governed
   source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-planning.json` is
   the governed discovery projection.
3. `skills/elegy-planning/SKILL.md` is the canonical operational body.
4. This file mirrors install and CLI handoff needed by wrapper
   consumers.

## Wrapper install

- Run `./install.ps1` from this wrapper root to stage the contracts
  bundle, the `elegy-planning` CLI surface, and this wrapper surface
  together.
- Pass `-LocalArtifactsRoot <path>` when validating against local
  archives instead of GitHub release assets.

## Current commands

```text
elegy-planning --scope <scope-key> scope create|list|show ...
elegy-planning --scope <scope-key> goal create|list|show|update-status|search ...
elegy-planning --scope <scope-key> roadmap create|list|show|add-section|add-work-point|update-status|search ...
elegy-planning --scope <scope-key> work-point list|show|revise|update-status|next-runnable|work-graph ...
elegy-planning --scope <scope-key> plan create|list|show|revise|update-status|search ...
elegy-planning --scope <scope-key> todo create|list|update-status|search ...
elegy-planning --scope <scope-key> issue record|list|show|update-status|search ...
elegy-planning --scope <scope-key> review-point record|update-status ...
elegy-planning --scope <scope-key> insight record|list|show|search|update-status ...
elegy-planning --scope <scope-key> context --entity-type <type> --entity-id <id>
elegy-planning --scope <scope-key> context --session --correlation-id <id>
elegy-planning --scope <scope-key> tags [--entity-type <type>]
elegy-planning --scope <scope-key> search [--tag <tag>] [--fts <query>] [--title <pattern>] [--status <s>]
elegy-planning --scope <scope-key> validate all
elegy-planning --scope <scope-key> health
elegy-planning --scope <scope-key> project export|render ...
elegy-planning --scope <scope-key> project run claim|activate|release|add-evidence|list|show ...
elegy-planning --json --non-interactive --correlation-id <id> ...
```

## Behavior notes

- SQLite remains the durable authority. Markdown and JSON projections
  are generated, derived outputs.
- Omitted scope defaults to `default`. Always pass `--scope` explicitly
  in agent-driven calls; the silent default is a frequent source of
  cross-scope pollution.
- Insights are first-class entities that capture reasoning attached
  to any planning entity. Use them liberally.
- Tags are indexed for fast cross-entity correlation search.
- FTS5 provides full-text content search across entities and
  insights; if `health` shows FTS5 drift, rebuild the index.
- Context commands return progressive disclosure bundles with token
  estimates.
- Project runs are durable leases; once claimed, a work point is
  considered in-flight until `release` is called.
- `scope create --metadata-file <path>` reads metadata from a JSON
  file (mutually exclusive with `--metadata-json`).
- `insight list --all` lists all insights in the active scope.
  Omit `--all` for parent-specific listing (existing behavior).
- `validate all` validates only the active scope by default.
  Pass `--all-scopes` for a global audit across all scopes.
  The output includes `scopeMode` and `scopeKey` to confirm scope.

## Agent invocation guidance

- Prefer machine mode for all mutations:
  `--json --non-interactive --correlation-id <id>`.
- Repeat multi-value flags once per value instead of comma-joining
  (`--acceptance <a1> --acceptance <a2>`, `--tag <t1> --tag <t2>`).
- For work-point, plan, and todo authoring, use
  `--effort-tier <fast|balanced|deep>` and repeat
  `--file-scope <type:intent:selector>` as needed.
- File-scope selector grammar: `<type>:<intent>:<selector>`. Types
  are `exact` or `glob`. Intents are `primary`, `review`, or
  `affected`.
- Plan revise clearing is explicit: use `--clear-routing-hint` and
  `--clear-file-scopes` when you need removal semantics. Empty
  values are dropped, not cleared.
- Record insights with `--insight-type <type>` and `--tag <tag>` for
  discoverability across sessions.
- Use `context --entity-type <type> --entity-id <id>` to load full
  context including related insights and token estimates before
  deep work.
- Use `tags list` to discover available tags before searching.
- For the full guardrails, common issues, and worked examples, load
  the canonical body: `../../../skills/elegy-planning/SKILL.md`.
