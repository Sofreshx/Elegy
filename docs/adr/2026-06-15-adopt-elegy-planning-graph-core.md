---
title: Adopt elegy-planning graph core
status: proposed
date: 2026-06-15
owner: Elegy
---

# Adopt elegy-planning graph core

## Context

`elegy-planning` currently models durable planning through a mostly linear
hierarchy: goals, roadmaps, sections, work points, plans, todos, issues, review
points, insights, and project-run leases. This works for simple single-threaded
work, but it does not naturally represent branching work, parallel execution,
acceptance traceability, repeated fix attempts, review history, or agent context
loading across sessions.

The next model needs to support flexible planning graphs without becoming an
unbounded graph database. Agents should be able to decompose work, branch,
identify parallel-safe work, attach abstract and concrete acceptance criteria,
record execution traces, and load bounded context for a worker, reviewer,
fixer, planner, or validator. At the same time, the system must remain
deterministic: impossible transformations should be rejected before write,
internal planning side effects should happen atomically, and clients should not
be responsible for preserving invariants by remembering follow-up commands.

## Decision

Adopt a governed typed graph as the successor core for `elegy-planning`.

- Store planning state as typed nodes and typed edges, with command handlers and
  invariant checks as the only normal mutation path.
- Keep familiar concepts such as goal, roadmap, work, plan, task, run, issue,
  review, insight, acceptance, and evidence as node kinds rather than separate
  hierarchy-owned tables.
- Retain roadmaps as optional strategic nodes and lenses for complex branching
  work, not mandatory containers between goals and work.
- Make acceptance requirements first-class graph nodes with abstract and
  concrete kinds. Concrete acceptance must link to abstract acceptance before
  upstream validation can complete.
- Make run traces first-class graph state so agents can load prior attempts,
  findings, fixes, command results, and evidence without relying on raw chat
  transcripts.
- Use typed commands for normal operations and reserve generic graph mutation
  for administrative or migration tooling.

## Alternatives

Option A: extend the existing hierarchy with more fields and dependency edges.
Rejected because it keeps cross-cutting work, multi-branch planning, and
acceptance traceability as exceptions around the model rather than the model's
native shape.

Option B: expose a pure generic graph with arbitrary node and edge payloads.
Rejected because it would be flexible but too easy for agents to corrupt. The
core value of `elegy-planning` is deterministic, governed state, not general
graph storage.

Option C: keep planning records static and push execution traces into
`elegy-memory`. Rejected because execution traces are forward-progress state
with lifecycle, evidence, and validation semantics. Memory remains for distilled
retrospective observations.

## Consequences

- Positive: branching, parallelism, corrective work, review history, and
  acceptance coverage become native graph queries.
- Positive: agents can load situation-specific context bundles from structured
  traces instead of replaying full conversation history.
- Positive: the command-handler boundary gives a clear place to enforce
  preflight invariants and atomically apply internal side effects.
- Negative: this is a major v2 model change and needs compatibility migration
  from existing v1 records.
- Negative: typed graph validation is more complex than the current table
  hierarchy and must be specified before implementation.
- Negative: generic graph escape hatches must be carefully limited so they do
  not bypass invariants in normal agent workflows.

## Links

- [elegy-planning graph core spec](../specs/elegy-planning-graph-core.md)
- [elegy-planning deterministic state machine spec](../specs/elegy-planning-state-machine.md)
- [elegy-planning acceptance and evidence spec](../specs/elegy-planning-acceptance-evidence.md)
- [elegy-planning run trace and context spec](../specs/elegy-planning-run-trace-context.md)
- [elegy-planning v1 spec](../specs/elegy-planning.md)
