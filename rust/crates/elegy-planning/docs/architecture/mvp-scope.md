# MVP Scope

> This document is the source of truth for what `elegy-planning` should implement right now.

## Milestones

- **MVP** — implement now with tests
- **v1** — scaffold or document, implement after MVP stabilizes
- **v2** — document only

## Feature Matrix

### Storage

| Feature | Milestone | Notes |
| --- | --- | --- |
| SQLite authority store | **MVP** | Single local DB file |
| Schema version bootstrap | **MVP** | `planning_config.schema_version` |
| Append-only planning event log | **MVP** | `planning_events` |
| Current-state projection tables | **MVP** | Goals, roadmaps, plans, todos, issues, review points |
| Replay-only reconstruction from events | **v1** | Current MVP writes both events and projections |

### Durable Entities

| Feature | Milestone | Notes |
| --- | --- | --- |
| Goal | **MVP** | Required parent for every roadmap |
| Roadmap | **MVP** | Goal-linked durable sequencing layer |
| Roadmap section | **MVP** | Structural grouping |
| Work point | **MVP** | Durable roadmap item |
| Plan | **MVP** | Single implementation pass artifact |
| Todo | **MVP** | Linked or standalone |
| Issue | **MVP** | First-class aggregate |
| Review point | **MVP** | Attached record, not top-level review aggregate |
| Evidence aggregate | **v1** | MVP uses string evidence refs on todos |

### Validation

| Feature | Milestone | Notes |
| --- | --- | --- |
| Deterministic validation findings | **MVP** | Stored in `validation_findings` |
| Non-blocking validation posture | **MVP** | Findings steer fixes without stopping authoring |
| Dependent validation refresh | **MVP** | Ancestor and attached-entity findings refresh after writes |
| Dependency validation | **MVP** | Work point dependency checks |
| Completion gating checks | **MVP** | Roadmap/plan completion contradictions |
| Evidence enforcement by evidence table | **v1** | After explicit evidence aggregate lands |

### CLI

| Feature | Milestone | Notes |
| --- | --- | --- |
| Machine-friendly JSON envelopes | **MVP** | Same posture as `elegy-skills` |
| Structured machine-mode runtime errors | **MVP** | Missing-parent failures return machine-readable `invalid` envelopes |
| Goal create/list/show | **MVP** | Implemented |
| Roadmap create/add-section/add-work-point/list/show | **MVP** | Implemented |
| Plan create/list/show | **MVP** | Implemented |
| Todo create/list | **MVP** | Implemented |
| Issue record/list/show | **MVP** | Implemented |
| Review point record | **MVP** | Implemented |
| Validate all | **MVP** | Implemented |
| Events list | **MVP** | Implemented |
| Health | **MVP** | Implemented |
| Projection render | **MVP** | Markdown or JSON |
| Update and transition commands | **v1** | Not yet implemented |

### Projections

| Feature | Milestone | Notes |
| --- | --- | --- |
| On-demand markdown projection | **MVP** | Goal, roadmap, plan, issue |
| On-demand JSON projection | **MVP** | Goal, roadmap, plan, issue |
| Cached projection index | **v1** | Not yet implemented |
| Repo-doc sync/export bridge | **v1** | Intended for instruction-engine integration |

### Compatibility

| Feature | Milestone | Notes |
| --- | --- | --- |
| Internal planning authority independent from memory | **MVP** | Implemented by separate crate |
| Import from instruction-engine workflow artifacts | **v1** | Not yet implemented |
| Export to repo-backed planning docs | **v1** | Not yet implemented |
| Skill and capability registry wiring | **MVP** | Governed skill fixture is exposed through the built-in registry and packaged wrapper bridge surfaces |

## MVP Summary

The MVP is a dedicated planning crate with a real SQLite authority store, durable planning entities, event history, advisory validation findings, and a machine-friendly CLI.

It is intentionally enough to support daily use for:

- durable planning authoring
- deterministic progress checks
- issue and review steering
- projection into temporary human-readable files
