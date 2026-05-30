---
name: elegy-planning
description: "Surface-local non-authoritative bridge shipped with the Elegy-planning wrapper surface and wrapper archive."
---

# Elegy-planning Surface Bridge

This file is a surface-local, non-authoritative skill bridge shipped with the `src/Elegy-planning` wrapper surface and the `elegy-planning-wrapper-<bundleVersion>.zip` archive.

Authority stays one-way:

1. `contracts/fixtures/skill-definition-v2.elegy-planning.json` is the governed source of truth.
2. `contracts/fixtures/skill-discovery-index.elegy-planning.json` is the governed discovery projection.
3. `.github/skills/elegy-planning/SKILL.md` remains the repo-local contributor-routing output.
4. This file mirrors install and CLI handoff needed by wrapper consumers.

## Wrapper install

- Run `./install.ps1` from this wrapper root to stage the contracts bundle, the `elegy-planning` CLI surface, and this wrapper surface together.
- Pass `-LocalArtifactsRoot <path>` when validating against local archives instead of GitHub release assets.

## Current commands

```text
elegy-planning --scope <scope-key> scope create|list|show ...
elegy-planning --scope <scope-key> goal create|list|show|update-status|search ...
elegy-planning --scope <scope-key> roadmap create|list|show|add-section|add-work-point|update-status|search ...
elegy-planning --scope <scope-key> work-point list|show|update-status ...
elegy-planning --scope <scope-key> plan create|list|show|revise|update-status|search ...
elegy-planning --scope <scope-key> todo create|list|update-status|search ...
elegy-planning --scope <scope-key> issue record|list|show|update-status|search ...
elegy-planning --scope <scope-key> review-point record|update-status ...
elegy-planning --scope <scope-key> insight record|list|show|search|update-status ...
elegy-planning --scope <scope-key> context --entity-type <type> --entity-id <id>
elegy-planning --scope <scope-key> context --session --correlation-id <id>
elegy-planning --scope <scope-key> tags [--entity-type <type>]
elegy-planning --scope <scope-key> search [--tag <tag>] [--fts <query>] [--title <pattern>] [--status <s>]
elegy-planning --json --non-interactive --correlation-id <id> ...
elegy-planning project export|render ...
```

## Behavior notes

- SQLite remains the durable authority.
- Omitted scope defaults to `default`.
- Markdown and JSON projections are generated sharing artifacts, not authority.
- Insights are first-class entities that capture reasoning attached to any planning entity.
- Tags are indexed for fast cross-entity correlation search.
- FTS5 provides full-text content search across entities and insights.
- Context commands return progressive disclosure bundles with token estimates.

## Agent invocation guidance

- Prefer machine mode for all mutations: `--json --non-interactive --correlation-id <id>`.
- Repeat multi-value flags once per value instead of comma-joining.
- For work-point, plan, and todo authoring, use `--effort-tier <fast|balanced|deep>` and repeat `--file-scope <selector-type:intent:selector>` as needed.
- File-scope selector types are `exact` and `glob`; intents are `primary`, `review`, or `affected`.
- Plan revise clearing is explicit: use `--clear-routing-hint` and `--clear-file-scopes` when you need removal semantics.
- Record insights with `--insight-type <type>` and `--tag <tag>` for discoverability.
- Use `context --entity-type <type> --entity-id <id>` to load full context including related insights and token estimates before deep work.
- Use `tags list` to discover available tags before searching.
