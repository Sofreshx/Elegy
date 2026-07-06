# elegy-planning

`elegy-planning` is Elegy's dedicated durable planning authority for goals, roadmaps, plans, todos, issues, attached review points, validation findings, and event history.

It is intentionally not an extension of `elegy-memory`.

## Purpose

`elegy-planning` exists for planning state that should be:

- durable
- queryable
- deterministically validated
- usable across code and non-code workflows
- machine-friendly for CLIs, harnesses, and skills

This crate uses SQLite as the MVP authority and keeps a structured event log plus current-state projection tables.

## Setup

Run from the repo root during development:

```bash
cargo run -p elegy-planning -- --help
```

Use a stable database path for durable local planning state:

```bash
cargo run -p elegy-planning -- --db .elegy/planning.db --scope <scope-key> --json --non-interactive health
```

Agent automation should always pass:

- `--db <path>` when the default repo-local database is not intended
- `--scope <scope-key>` instead of relying on `default`
- `--json --non-interactive --correlation-id <id>` on mutating commands

SQLite is opened with a bounded busy timeout. Agents should still serialize
mutating calls instead of launching parallel writes.

## MVP Behavior

The implemented MVP supports:

- durable `goal`, `roadmap`, `roadmap section`, `work point`, `plan`, `todo`, `issue`, and attached `review point` records
- append-only planning events in `planning_events`
- current-state projection tables for fast reads
- non-blocking validation findings in `validation_findings`
- machine-friendly CLI output with `--json`, `--non-interactive`, and `--correlation-id`
- on-demand markdown or JSON projections for key entities
- FTS5 search across v1 entities with typed and scoped index rows
- transactional v1 roadmap scaffolding from YAML or JSON

Machine-mode runtime failures emit structured JSON envelopes. Missing required parent references are rejected as invalid input before SQLite insert time so callers do not receive raw foreign-key failures.

The current MVP intentionally keeps validation advisory-first:

- writes are accepted when the core schema is valid
- deterministic validation findings are stored after writes
- warnings and errors steer future work without blocking normal authoring

When a write changes another entity's validation posture, dependent findings are refreshed as part of the same write flow so reads do not return stale validation state.

This follows the current design choice to help an LLM or operator keep moving while surfacing what must be fixed next.

## Operating Ideology

`elegy-planning` is an agent contract, not a note-taking format.

- SQLite is authority. Projections, templates, and skill docs route agents to
  the authority; they do not replace it.
- Scope is part of identity. Agents should pass scope explicitly and reject
  surprising cross-scope references.
- Authoring should be batch-safe. `roadmap scaffold` dry-run and apply use the
  same transaction path so an agent can preview, inspect `rejected`, then apply
  without changing semantics.
- Validation is advisory but durable. Structural write invariants block bad
  records; planning quality findings are stored and surfaced so the next agent
  can continue from evidence.
- Events matter. Durable writes should append events so later review can
  reconstruct what happened and why.
- IDs are automation handles. Prefer explicit stable IDs over generated IDs in
  agent-authored workflows.

## Implemented Entity Rules

### Goals

- every roadmap must link to a goal
- goals should declare explicit acceptance criteria and rejection criteria

### Roadmaps

- every roadmap links to exactly one goal in MVP
- roadmaps warn when they have no work points
- roadmaps error when marked complete while work points remain open

### Plans

- every plan links to one goal and one roadmap
- plan goal must match roadmap goal
- plans warn when they have no targeted work points, no validation steps, or no todos yet
- plans error when targeted work points are missing or belong to another roadmap
- plans error when high-severity open issues or review points remain attached

### Todos

- todos may be linked to a plan, a work point, both, or neither
- standalone todos are allowed in MVP but produce a warning
- completed todos without evidence refs produce a warning

### Issues

- issues are first-class aggregates in MVP
- partial related-entity links warn
- related entity references that do not resolve error

### Review Points

- review points are attached to other planning entities instead of being top-level review aggregates
- missing attached entities error
- critical open review points warn on the review point itself and can invalidate a plan

## Storage Model

The SQLite schema currently includes:

- `planning_config`
- `goals`
- `roadmaps`
- `roadmap_sections`
- `work_points`
- `plans`
- `todos`
- `issues`
- `review_points`
- `planning_events`
- `validation_findings`
- `tag_index`
- `entities_fts`
- `insights_fts`

`planning_events` stores append-only event history with:

- `event_id`
- `scope_key`
- `entity_type`
- `entity_id`
- `aggregate_type`
- `aggregate_id`
- `correlation_id`
- `run_id`
- `stream_id`
- `sequence`
- `parent_event_id`
- `timestamp`
- `payload_json`

The event log is paired with current-state projection tables for simpler CLI reads.

Event listing is scope-aware: `events` returns only events for the active scope.

`sequence` is stream-local to `stream_id`. Flat event listing uses append order rather than treating `sequence` as a global ordering key.

`entities_fts` stores typed and scoped rows: `entity_id`, `entity_type`,
`scope_key`, `title`, and `content`. `health` reports aggregate and per-entity
drift so index problems are visible to agents.

## CLI

Global flags:

- `--db`
- `--scope`
- `--json`
- `--non-interactive`
- `--correlation-id`

Implemented commands:

- `scope create|list|show`
- `goal create|list|show|update-status`
- `roadmap create|add-section|add-work-point|list|show|update-status`
- `roadmap scaffold --file <yaml|json> --dry-run|--apply --if-exists fail|skip|update`
- `work-point list|show|revise|update-status`
- `plan create|list|show|revise|update-status`
- `todo create|list|show|update-status`
- `issue record|list|show|update-status`
- `review-point record|update-status`
- `validate all`
- `events`
- `health`
- `project render|export`

Examples:

```bash
cargo run -p elegy-planning -- --json --non-interactive --correlation-id corr-plan-1 goal create \
  --id goal-1 \
  --title "Ship planning subsystem" \
  --description "Create a dedicated planning authority in Elegy." \
  --acceptance "CLI exists" \
  --rejection "Planning authority remains split"
```

```bash
cargo run -p elegy-planning -- --json --non-interactive --correlation-id corr-plan-1 roadmap create \
  --id roadmap-1 \
  --goal-id goal-1 \
  --title "Planning MVP" \
  --summary "Land the first planning authority slice."
```

```bash
cargo run -p elegy-planning -- --json validate all
```

## Roadmap Scaffold

`roadmap scaffold` is the v1 batch authoring surface for the common
goal → roadmap → sections → work-points → plan → todos flow.

```bash
cargo run -p elegy-planning -- --scope <scope-key> --json --non-interactive \
  --correlation-id corr-scaffold-1 roadmap scaffold \
  --file roadmap.yaml --dry-run
```

```bash
cargo run -p elegy-planning -- --scope <scope-key> --json --non-interactive \
  --correlation-id corr-scaffold-1 roadmap scaffold \
  --file roadmap.yaml --apply --if-exists update
```

Semantics:

- dry-run and apply share the same transaction path
- apply rolls back every scaffold-created or scaffold-updated row when any
  entity is rejected
- work-point dependencies may reference work-points declared later in the same
  file
- omitted ordering on update preserves existing ordering
- `--if-exists fail` rejects existing records
- `--if-exists skip` leaves existing records untouched
- `--if-exists update` updates supported content fields and rejects parent-link
  drift such as moving a roadmap to another goal
- result fields are `created`, `updated`, `unchanged`, `skipped`, `rejected`,
  `validationFindings`, and `nextRunnableWorkPoints`

Scaffold validation output is limited to touched scaffold entities and directly
affected parents. Use `validate all` for a full-scope audit.

Use `template render --template roadmap-workflow --output roadmap.yaml` to start
from the maintained scaffold template.

## Projection Rendering

`project render` writes non-authoritative projections for human or LLM consumption.

Projection reads are scope-enforced in the same way as `show`: out-of-scope entity IDs are rejected instead of rendering cross-scope data.

Currently supported rendered entity types:

- `goal`
- `roadmap`
- `plan`
- `issue`

Formats:

- `markdown`
- `json`

Use `--projection-format` on `project render` so projection rendering does not collide with the global CLI `--format` output flag.

These projections are derived outputs, not the planning source of truth.

## Scope Behavior

- default scope remains `default` when `--scope` is omitted
- reads and writes reject out-of-scope entity IDs under the active scope
- linked creates reject cross-scope parent or attached references
- `plan revise --scope-key` is the only explicit scope-transfer operation, and it is rejected when linked roadmap, work point, todo, issue, or review-point records would remain in another scope

## Relationship to instruction-engine

This crate is intended to become the canonical authority for durable planning state while existing repo-visible planning docs remain compatibility or sharing surfaces.

The current implementation does not yet import or export `instruction-engine` roadmap workflow artifacts automatically. That bridge is the next integration step.

## Validation Philosophy

`elegy-planning` deliberately separates:

- core schema validity required to write a record
- deterministic planning validation findings recorded after the write

The system therefore avoids hard-blocking normal authoring while still surfacing the next fixes the operator or LLM should handle before claiming completion.
