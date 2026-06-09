# Elegy-planning V1

## Purpose

`elegy-planning` is the durable planning authority surface in this repo. It owns
the dedicated `elegy-planning` binary for goal, roadmap, plan, todo, issue, review
point, insight, work-point graph, and project-run lease state.

`rust/crates/elegy-planning` is the implementation center. `rust/crates/elegy-cli`
keeps no permanent compatibility bridge for planning commands. The contributor-
navigation overlay under `src/Elegy-planning` is a pointer shell only, not a
repo center, authority layer, implementation center, or release surface.

Alongside the existing `elegy` CLI surface, this in-repo `elegy-planning`
surface is one of the current shipped operator surfaces.

It currently covers:

- a bounded SQLite-backed store for goals, roadmaps, sections, work points,
  plans, todos, issues, review points, insights, and project runs
- deterministic, advisory-first validation that runs after every mutation
- scope isolation at the storage layer (workspace, user, agent, session)
- a work-point graph projection (nodes with lease and plan counts, edges from
  declared dependencies) and a `next-runnable` candidate resolver
- a project-run lease surface that lets agents claim, activate, release, and
  annotate durable leases on a single work point
- machine-first JSON output with versioned envelopes, correlation IDs, and
  non-interactive flags
- projection rendering, FTS5 full-text search, tag indexes, and event history

## Authority chain

The authority chain is explicit and one-way:

1. `contracts/schemas/skill.schema.json` defines the durable
   capability contract shape.
2. `contracts/fixtures/skill.elegy-planning.json` is the
   governed skill definition source of truth.
3. `contracts/fixtures/skill-discovery-index.elegy-planning.json` is the
   governed discovery projection derived from that definition.
4. `contracts/fixtures/elegy-plugin-package.elegy-planning.json` is the
   portable Holon plugin package that carries capability projections for
   package-level consumers.
5. `.agents/skills/elegy-planning/SKILL.md` and
   `.github/skills/elegy-planning/SKILL.md` are rendered local outputs and are
   not authoritative.
6. `docs/specs/elegy-planning.md` is the implementation-facing spec and the
   authority for entity model, lifecycle, and validation rules.
7. This document (`docs/architecture/elegy-planning-v1.md`) is the
   architecture mirror that summarizes the current ship state.

Contributors should update the governed fixtures first, then the spec, then
this architecture mirror, and only then the rendered markdown output when the
materialized skill text needs to change.

## Shipped CLI surface

The implemented CLI surface in `rust/crates/elegy-planning/src/cli.rs` is:

- `elegy-planning scope create|list|show`
- `elegy-planning goal create|list|show|update-status|search`
- `elegy-planning roadmap create|list|show|update-status|search|add-section|add-work-point`
- `elegy-planning work-point list|show|update-status|next-runnable|work-graph`
- `elegy-planning plan create|list|show|revise|update-status|search`
- `elegy-planning todo create|list|update-status|search`
- `elegy-planning issue record|list|show|update-status|search`
- `elegy-planning review-point record|update-status`
- `elegy-planning insight record|list|show|update-status|search`
- `elegy-planning project-run claim|activate|release|add-evidence|list|show`
- `elegy-planning validate all`
- `elegy-planning events list`
- `elegy-planning health`
- `elegy-planning project render|export`
- `elegy-planning context --entity-type <type> --entity-id <id>`
- `elegy-planning tags list`
- `elegy-planning search <query>`
- `elegy-planning session init`

## Current behavior

The current MVP CLI behavior is intentionally narrow:

- the default database path is `~/.elegy/planning.db`; `--db` overrides
- the default scope is `default`; `--scope` overrides
- `next-runnable` runs `validate_all` first to surface graph issues before
  returning candidates; the candidate order is `ordering` then `id`
- `work-graph` runs `validate_all` first; nodes carry `hasActiveLease` and
  `planCount`; edges come from declared `dependencyIds`
- `project-run claim` is rejected with `ACTIVE-LEASE-CONFLICT` if the target
  work point already has a `claimed`, `active`, or `interrupted` lease
- `project-run activate` is the only path from `claimed` to `active`
- `project-run release` accepts `claimed`, `active`, or `interrupted` as
  from-statuses; the new `--status` value drives the final recorded state
- `project-run add-evidence` rejects updates to `completed` or `released`
  runs; the run must be in `claimed` or `active` to accept evidence
- validation is advisory; an invalid plan still exists in the database and
  carries a `validation` payload in the response envelope
- `--json --non-interactive` produces a stable versioned envelope on every
  command; `--correlation-id` may be supplied globally or per command, or
  auto-resolved from the active session

## Entity model

The durable entity model is:

```
Scope
  Goal
    Roadmap
      RoadmapSection
        WorkPoint
          ProjectRun (lease + evidence)
  Plan
    Todo
  Issue
  ReviewPoint
  Insight
  PlanningEvent (append-only audit log)
```

A `ProjectRun` is a first-class entity attached to exactly one work point and
exactly one goal/roadmap. It carries claim metadata (branch, worktree, session,
profile, run id), lifecycle status, and a structured `ProjectRunEvidence`
record (implementation run refs, structured warning records, validation
finding refs, commit SHA, PR URL, linked spec ids).

## What stays out of scope

- Lifecycle transition enforcement at the type level is deferred. The stored
  values are constrained per entity, but the system does not yet reject
  invalid transitions such as `Draft → Completed`. Project-run transitions are
  partially enforced because they go through dedicated methods rather than
  `update-status`, but the rule set is not generalized.
- Event replay is not implemented. The `planning_events` table is append-only
  audit storage, not a recoverable projection source.
- Cross-aggregate plans (a single plan that targets two goals) are not
  modeled. Use a synthetic aggregate goal or duplicate the plan.
- Compatibility import from legacy `instruction-engine` Markdown conventions
  is not implemented.
- Host-side Mermaid reverse projection of `work-graph` is bounded analysis
  only, not canonical workflow reconstruction.

## Related

- [elegy-planning Spec](../specs/elegy-planning.md)
- [Skill Core V1](skill-core-v1.md)
- [Elegy Configuration V1](elegy-configuration-v1.md)
- [Elegy Memory V1](elegy-memory-v1.md)
- [Agent Integration](../agent-integration.md)
- [Distribution and downstream consumption](../distribution.md)
- [Substrate Governance](substrate-governance.md)
