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
- durable `project-run` leases with fencing, heartbeat, release, and immutable evidence append
- registered `worktree` records for scope-aware execution context
- graph nodes, graph edges, acceptance nodes, evidence nodes, runnable-work queries, and bulk graph transitions
- append-only planning events in `planning_events`
- current-state projection tables for fast reads
- non-blocking validation findings in `validation_findings`
- machine-friendly CLI output with `--json`, `--non-interactive`, and `--correlation-id`
- on-demand markdown or JSON projections for key entities
- FTS5 search across v1 entities with typed and scoped index rows
- discovery records for defects, deferred work, review findings, insights, and observations
- transactional v1 roadmap scaffolding from YAML or JSON
- manifest apply/diff and intent-to-manifest expansion for graph authoring

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
- `scopes`
- `goals`
- `roadmaps`
- `roadmap_sections`
- `work_points`
- `plans`
- `todos`
- `issues`
- `review_points`
- `project_runs`
- `worktrees`
- `planning_nodes`
- `planning_edges`
- `acceptance_links`
- `evidence_links`
- `discovery_nodes`
- `discovery_relationships`
- `discovery_checkpoints`
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
- `version`
- `capabilities [--detail]`
- `project render|export`
- `workflow render|export|view|bundle|prepare|record-result|import-artifact|export-artifact`
- `session init|use|show|resume|list`
- `search`
- `insight create|list|show|update-status`
- `context`
- `tags`
- `project-run claim|activate|heartbeat|release|add-evidence|list|show`
- `worktree list|show|attach|archive|cleanup-intent`
- `graph node create|show|list|status|revise|finalize`
- `graph edge create|show|list|incoming|outgoing|status|revise`
- `graph acceptance create|show|list|satisfy`
- `graph evidence create|show|list|attach`
- `graph runnable|bulk`
- `manifest --file <yaml|json> [--dry-run]`
- `diff --manifest <yaml|json>`
- `discovery record|show|list|triage|promote|resolve|reopen|checkpoint`
- `template list|render`
- `intent --file <intent.yaml> [--output <manifest.yaml>]`

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

## Workflow Projection

`workflow render|export` is the workflow-oriented alias for projection output.
It uses the same authority and scope checks as `project render|export`, but gives
hosts a stable noun for orchestration-facing projections:

```bash
cargo run -p elegy-planning -- --scope <scope-key> --json --non-interactive \
  --correlation-id corr-workflow-1 workflow render \
  --entity-type roadmap --entity-id <roadmap-id> \
  --projection-format json --output workflow.json
```

The current command does not spawn subagents or execute workflow steps. It is a
low-friction projection surface for Codex, OpenCode, Holon, or another host to
consume before they perform host-local delegation.

The target host-neutral projection contract is documented in
`docs/specs/workflow-view.md`.

`workflow view` returns an initial `workflow-view/v1` JSON payload inside the
standard machine envelope:

```bash
cargo run -p elegy-planning -- --scope <scope-key> --json --non-interactive \
  workflow view --entity-type roadmap --entity-id <roadmap-id>
```

`workflow prepare` is the native-first execution boundary. It compiles one
bounded runnable batch, claims and activates its project runs transactionally,
and emits `orchestrator-dispatch/v1` records for the host to execute with
native Codex or Holon workers:

```bash
cargo run -p elegy-planning -- --scope <scope-key> --json --non-interactive \
  --correlation-id corr-workflow-prepare workflow prepare \
  --entity-type roadmap --entity-id <roadmap-id> --max-workers 3
```

Each worker returns `orchestrator-worker-result/v1` JSON. Record it with the
fenced, idempotent writeback command:

```bash
cargo run -p elegy-planning -- --scope <scope-key> --json --non-interactive \
  workflow record-result --file ./worker-result.json
```

The writeback validates the project-run fence, dispatch identity, source
revision, and idempotency key, then atomically records evidence, attaches it to
the work node, updates graph status, and releases the project run.
Pass `--adapter-id` and repeated `--capability` flags to `workflow prepare` when
the host requires capabilities such as browser, container, or E2E validation.

`workflow bundle` materializes the current workflow view into a host-facing
execution bundle:

```bash
cargo run -p elegy-planning -- --scope <scope-key> --json --non-interactive \
  workflow bundle --entity-type roadmap --entity-id <roadmap-id> \
  --output ./workflow-bundle --host codex
```

`workflow import-artifact|export-artifact` are compatibility bridges for
`instruction-engine` roadmap workflow artifacts. Import parses a markdown
artifact with a `## Structured State` JSON block, derives a roadmap scaffold,
and supports the same `--dry-run|--apply --if-exists` transaction path as
`roadmap scaffold`. Export writes a markdown artifact with
`schemaVersion: "1"` structured state:

```bash
cargo run -p elegy-planning -- --scope <scope-key> --json --non-interactive \
  workflow import-artifact --file ./artifact.md --dry-run --if-exists update

cargo run -p elegy-planning -- --scope <scope-key> --json --non-interactive \
  workflow export-artifact --roadmap-id <roadmap-id> \
  --slice-id <work-point-id> --output ./artifact.md
```

The view includes compact graph nodes and edges, runnable/blocked candidates,
adapter policy, a next-batch execution plan, conservative delegation hints,
evidence policy, budgets, and metrics. Delegation hints expose worker profile,
recommended subagent role, model tier, payload-derived file scopes when
present, allowed actions, retry policy, context-token estimate, and wall-time
estimate. Hosts still own actual model selection, native worker spawning,
cancellation, sandboxing, and approval UI. Unknown or overlapping file scopes
remain sequential; the portable worker cap is three.

## Scope Behavior

- default scope remains `default` when `--scope` is omitted
- reads and writes reject out-of-scope entity IDs under the active scope
- linked creates reject cross-scope parent or attached references
- `plan revise --scope-key` is the only explicit scope-transfer operation, and it is rejected when linked roadmap, work point, todo, issue, or review-point records would remain in another scope

## Relationship to instruction-engine

This crate is intended to become the canonical authority for durable planning
state while existing repo-visible planning docs remain compatibility or sharing
surfaces.

The current implementation imports and exports `instruction-engine` roadmap
workflow artifacts through `workflow import-artifact|export-artifact`. The
bridge is intentionally narrow: the artifact is converted into the existing
roadmap scaffold transaction path, and SQLite remains the durable authority.

## Validation Philosophy

`elegy-planning` deliberately separates:

- core schema validity required to write a record
- deterministic planning validation findings recorded after the write

The system therefore avoids hard-blocking normal authoring while still surfacing the next fixes the operator or LLM should handle before claiming completion.
