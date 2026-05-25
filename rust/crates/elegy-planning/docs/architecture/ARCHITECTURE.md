# elegy-planning Architecture

## Position in Elegy

`elegy-planning` is the planning subsystem for durable execution intent and progress.

It is designed to sit alongside `elegy-memory`, not inside it.

- `elegy-memory` stores distilled memory artifacts
- `elegy-planning` stores durable planning structure, transitions, and validation state

## Authority Model

MVP authority is SQLite.

Repo-visible markdown or JSON renderings are projections only.

This matters because planning needs:

- deterministic validation
- stable IDs
- correlation IDs
- linked state transitions
- queryable progress
- retained issue and review residue

Those are poor fits for markdown as the primary authority.

## Durable Concepts

Implemented in MVP:

- `goal`
- `roadmap`
- `roadmap section`
- `work point`
- `plan`
- `todo`
- `issue`
- `review point`
- `planning event`
- `validation finding`

## Core Design Choices

### 1. Goal is required above roadmap

Every roadmap must link to a goal.

### 2. Todos may be linked or standalone

Standalone todos are permitted because they are useful during authoring and triage.
They produce warnings so the system still nudges toward structured linkage.

### 3. Validation is advisory-first

The system records validation errors and warnings without blocking writes when the core schema remains valid.

This keeps planning authoring usable for LLM and operator workflows while preserving deterministic steering.

Required parent references are still preflighted before insert. Those fail as invalid input rather than surfacing raw SQLite foreign-key errors.

### 4. Issues are first-class, reviews are attached

Issues exist as top-level aggregates.
Review points are attached to another entity and intentionally lighter weight.

### 5. Projections are on-demand

MVP rendering is explicit and operator-driven through `project render`.
The DB remains authoritative.

## Event + Projection Model

Every successful write appends an event to `planning_events`.

Event metadata currently includes:

- `event_id`
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

Current-state tables provide fast reads for CLI and projections.

This is a pragmatic MVP version of event-sourced authority: append event, update projection table, validate current state, store findings.

The flat `events list` view uses append order. `sequence` remains stream-local, not a global history position.

## Validation Layers

Validation currently checks:

- goals missing acceptance or rejection criteria
- roadmaps missing work points
- roadmaps linked to inactive goals
- roadmaps marked complete while work remains open
- work points with missing or cross-roadmap dependencies
- plans whose goal and roadmap disagree
- plans with missing targeted work points
- plans with no validation steps or no todos
- completed plans with incomplete todos
- plans with unresolved high-severity issues or review points
- standalone todos
- completed todos without evidence refs
- issues with partial or invalid related-entity links
- review points attached to missing entities

The validator intentionally returns findings instead of rejecting normal authoring writes.

Dependent entities are revalidated after writes when their validation can be affected by the mutation, so persisted findings remain aligned with current state.

## CLI Posture

The CLI copies the same machine-usable posture already used by `elegy-skills`:

- `--json`
- `--non-interactive`
- `--correlation-id`
- structured `ok` / `invalid` / `error` envelopes

Unexpected runtime failures in machine mode emit structured `error` envelopes instead of raw stderr-only output.

This makes the planning subsystem suitable for automation and skills.

## Current Gaps

Not yet implemented:

- compatibility import from `instruction-engine` roadmap workflow artifacts
- automatic export/projection into shared repo planning docs
- update and transition subcommands beyond initial create flows
- richer evidence aggregates
- replay-based rebuild from events alone
- explicit migration history beyond schema version `1`
- dedicated wrapper projection for repo-local skill bridge surfaces

## Intended Next Steps

1. Add update and transition commands for statuses and revisions.
2. Add compatibility bridges to current `instruction-engine` planning artifacts.
3. Add explicit evidence records.
4. Add projection/import commands for shared repo planning surfaces.
5. Add governed capability exposure on the umbrella Elegy surfaces when the command model stabilizes.
