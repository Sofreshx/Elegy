---
title: "Implementation Plan: elegy-planning graph core phase 1"
status: draft
owner: Elegy
created: 2026-06-15
updated: 2026-06-15
doc_kind: spec
summary: "First implementation phase for the elegy-planning graph core: additive graph schema, Rust model types, storage APIs, invariant helpers, and focused tests without changing existing v1 command behavior."
---

# Implementation Plan: elegy-planning graph core phase 1

## Summary

Phase 1 establishes the graph substrate behind `elegy-planning` without
switching existing goal, roadmap, work-point, plan, todo, project-run, issue, or
review commands to the graph model yet. The goal is to land a safe additive
foundation: node and edge model types, schema migration, low-level storage APIs,
basic invariant helpers, and focused tests. Existing v1 behavior must keep
working exactly as it does today.

This phase deliberately does not implement acceptance coverage, run trace
context bundles, compatibility migration, or graph-backed command aliases. Those
belong to later phases once the graph substrate is proven.

## Phase 1 Scope

### In Scope

- Add graph model enums and records:
  - `PlanningNodeKind`
  - `PlanningEdgeKind`
  - `PlanningGraphNode`
  - `PlanningGraphEdge`
  - create input structs for graph node and edge storage
- Add additive SQLite schema version `9`:
  - `planning_nodes`
  - `planning_edges`
  - indexes for scope, kind, source, target, and edge traversal
- Add storage methods:
  - create/load/list graph nodes
  - create/load/list graph edges
  - list outgoing and incoming edges for a node
  - delete or retire edges only if the current lifecycle policy already has a
    safe status pattern; otherwise defer mutation beyond create/read
- Add preflight helpers for:
  - scope existence
  - source/target node existence
  - source/target scope match
  - valid edge-kind source/target node-kind combinations
  - acyclic `decomposes-to` and `depends-on` edges
- Add tests for schema creation, v8-to-v9 migration, record round-trip, invalid
  edge rejection, cross-scope rejection, and cycle rejection.

### Out Of Scope

- Rewriting existing v1 commands to write graph nodes/edges.
- Migrating existing v1 records into graph records.
- Public CLI graph commands.
- Acceptance/evidence verification.
- Run trace recording.
- Context bundle changes.
- Graph-backed `next-runnable` and parallel group queries.
- MCP/plugin projection updates.

## Implementation Steps

### Step 1: Add Model Types

File: `plugins/planning/src/model.rs`

Add string enums using the existing `string_enum!` macro:

- `PlanningNodeKind`
  - `Goal`
  - `Roadmap`
  - `Milestone`
  - `Work`
  - `Plan`
  - `Task`
  - `Run`
  - `Acceptance`
  - `Evidence`
  - `Issue`
  - `Review`
  - `Insight`
- `PlanningEdgeKind`
  - `DecomposesTo`
  - `DependsOn`
  - `Blocks`
  - `ParallelSafeWith`
  - `PlannedBy`
  - `ExecutedBy`
  - `Contains`
  - `Requires`
  - `Satisfies`
  - `EvidencedBy`
  - `Found`
  - `AddressedBy`
  - `Repairs`
  - `Supersedes`

Add graph records:

```rust
pub struct PlanningGraphNode {
    pub id: String,
    pub scope_key: String,
    pub kind: PlanningNodeKind,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub payload: serde_json::Value,
    pub tags: Vec<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

pub struct PlanningGraphEdge {
    pub id: String,
    pub scope_key: String,
    pub kind: PlanningEdgeKind,
    pub source_node_id: String,
    pub target_node_id: String,
    pub status: String,
    pub payload: serde_json::Value,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}
```

Use `serde(rename_all = "camelCase")` on records and kebab-case enum values to
match the rest of the planning JSON surface.

### Step 2: Add Storage Input Types

File: `plugins/planning/src/storage.rs`

Add internal/public input structs near the existing create inputs:

```rust
pub struct CreateGraphNodeInput {
    pub id: String,
    pub scope_key: String,
    pub kind: PlanningNodeKind,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub payload: serde_json::Value,
    pub tags: Vec<String>,
}

pub struct CreateGraphEdgeInput {
    pub id: String,
    pub scope_key: String,
    pub kind: PlanningEdgeKind,
    pub source_node_id: String,
    pub target_node_id: String,
    pub status: String,
    pub payload: serde_json::Value,
}
```

These inputs do not need CLI exposure in phase 1. They exist so tests and later
service work can use typed APIs instead of ad hoc SQL.

### Step 3: Add Schema Version 9

File: `plugins/planning/src/storage.rs`

Update `CURRENT_SCHEMA_VERSION` from `"8"` to `"9"`.

Add `planning_nodes` to `create_schema`:

```sql
CREATE TABLE IF NOT EXISTS planning_nodes (
    id TEXT PRIMARY KEY,
    scope_key TEXT NOT NULL REFERENCES scopes(scope_key) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    status TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    tags_json TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

Add `planning_edges` to `create_schema`:

```sql
CREATE TABLE IF NOT EXISTS planning_edges (
    id TEXT PRIMARY KEY,
    scope_key TEXT NOT NULL REFERENCES scopes(scope_key) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    source_node_id TEXT NOT NULL REFERENCES planning_nodes(id) ON DELETE CASCADE,
    target_node_id TEXT NOT NULL REFERENCES planning_nodes(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

Add indexes:

```sql
CREATE INDEX IF NOT EXISTS idx_planning_nodes_scope_kind
    ON planning_nodes(scope_key, kind);
CREATE INDEX IF NOT EXISTS idx_planning_edges_scope_kind
    ON planning_edges(scope_key, kind);
CREATE INDEX IF NOT EXISTS idx_planning_edges_source
    ON planning_edges(source_node_id, kind);
CREATE INDEX IF NOT EXISTS idx_planning_edges_target
    ON planning_edges(target_node_id, kind);
CREATE UNIQUE INDEX IF NOT EXISTS idx_planning_edges_unique_active
    ON planning_edges(scope_key, kind, source_node_id, target_node_id)
    WHERE status = 'active';
```

Add `migrate_v8_to_v9` and wire it into `ensure_schema_version`. The migration
must create only the graph tables/indexes and update `planning_config` to `"9"`.
It must not backfill graph data from existing v1 tables.

### Step 4: Add Row Mapping Helpers

File: `plugins/planning/src/storage.rs`

Add:

- `row_to_graph_node`
- `row_to_graph_edge`
- strict enum parsers for `PlanningNodeKind` and `PlanningEdgeKind`

Use existing helpers for JSON parsing and `map_not_found` patterns. Keep column
orders explicit and consistent across all graph SELECT statements.

### Step 5: Add Graph Storage Methods

File: `plugins/planning/src/storage.rs`

Add methods on `PlanningStore`:

- `create_graph_node(input) -> Result<MutationResult<PlanningGraphNode>, PlanningStoreError>`
- `graph_node(id) -> Result<PlanningGraphNode, PlanningStoreError>`
- `list_graph_nodes(scope_key, kind: Option<PlanningNodeKind>) -> Result<Vec<PlanningGraphNode>, PlanningStoreError>`
- `create_graph_edge(input) -> Result<MutationResult<PlanningGraphEdge>, PlanningStoreError>`
- `graph_edge(id) -> Result<PlanningGraphEdge, PlanningStoreError>`
- `list_graph_edges(scope_key, kind: Option<PlanningEdgeKind>) -> Result<Vec<PlanningGraphEdge>, PlanningStoreError>`
- `list_outgoing_edges(node_id, kind: Option<PlanningEdgeKind>) -> Result<Vec<PlanningGraphEdge>, PlanningStoreError>`
- `list_incoming_edges(node_id, kind: Option<PlanningEdgeKind>) -> Result<Vec<PlanningGraphEdge>, PlanningStoreError>`

For phase 1, validation reports may be empty/valid for graph node and edge
mutations. Full graph validation findings can wait until graph validation has
its own spec slice.

Every create method must:

- require non-empty id, title/kind/status where applicable
- require the scope to exist
- append a planning event using the existing event machinery
- persist tag indexes for graph nodes if existing tag indexing can accept the
  new entity type; if not, skip graph tag indexing in phase 1 and document the
  follow-up rather than forcing `EntityType` expansion prematurely

### Step 6: Add Edge Preflight Invariants

File: `plugins/planning/src/storage.rs`

Before inserting a graph edge:

- load source and target nodes
- reject if either node is missing
- reject if source or target scope differs from `input.scope_key`
- reject if source and target scopes differ
- reject invalid source/target kinds for the edge kind
- reject active duplicate edge via preflight before the unique index catches it
- reject cycles for active `decomposes-to` and `depends-on` edges

Initial source/target rules:

| Edge kind | Allowed source | Allowed target |
|---|---|---|
| `decomposes-to` | goal, roadmap, milestone, work, plan, run | roadmap, milestone, work, task, turn-summary when later added |
| `depends-on` | work, task | work, task |
| `blocks` | work, issue, review | work, task, acceptance |
| `parallel-safe-with` | work | work |
| `planned-by` | work | plan |
| `executed-by` | work, plan | run |
| `contains` | run, plan | task, evidence, issue, review, insight |
| `requires` | goal, roadmap, milestone, work, plan | acceptance |
| `satisfies` | acceptance | acceptance |
| `evidenced-by` | acceptance, work, plan, run, issue, review | evidence |
| `found` | run, work, plan | issue, review |
| `addressed-by` | issue, review | work, plan |
| `repairs` | work | work |
| `supersedes` | work, plan, acceptance | work, plan, acceptance |

For `satisfies`, phase 1 only verifies both sides are `acceptance` nodes.
Abstract/concrete direction checks belong to the acceptance/evidence phase once
acceptance payload shape is implemented.

Cycle detection should traverse only active edges of the same acyclic family:

- `decomposes-to`
- `depends-on`

Do not cycle-check `blocks` in phase 1; blocking semantics belong to a later
execution/readiness phase.

### Step 7: Add Service Re-exports Only If Needed

File: `plugins/planning/src/lib.rs`

Export the new model and storage input types so integration tests and later
phases can use them. Avoid adding CLI commands in this phase.

### Step 8: Add Tests

Preferred location: `plugins/planning/tests/integration.rs`

Add focused tests:

- `graph_schema_created_for_new_database`
  - create a fresh store, call `health` or `init`, assert schema version is `9`
  - verify graph tables accept inserts through typed APIs
- `graph_migrates_v8_to_v9_without_backfill`
  - build a minimal v8 database fixture
  - initialize store
  - assert `planning_nodes` and `planning_edges` exist and are empty
  - assert existing v1 tables remain readable
- `graph_node_round_trips`
  - create a scope, create a graph node, load it, compare kind/status/payload/tags
- `graph_edge_round_trips`
  - create two valid nodes, create a valid edge, load it, list incoming/outgoing
- `graph_edge_rejects_missing_node`
  - attempt edge to unknown target, expect invalid input
- `graph_edge_rejects_cross_scope`
  - create source and target in different scopes, expect invalid input
- `graph_edge_rejects_invalid_kind_pair`
  - attempt `planned-by` from goal to plan, expect invalid input
- `graph_edge_rejects_dependency_cycle`
  - create work A -> B, B -> C, attempt C -> A, expect invalid input
- `graph_edge_rejects_decomposition_cycle`
  - create goal/roadmap/milestone chain, attempt reverse edge, expect invalid input
- `graph_active_duplicate_rejected`
  - create the same active edge twice, expect invalid input

If integration test setup becomes too large, put row-level storage tests in the
existing `#[cfg(test)]` module in `storage.rs` and keep only command-visible or
crate-public behavior in `tests/integration.rs`.

## Execution Order

```text
Model types
  -> schema v9 + migration
  -> row mappers
  -> graph node storage methods
  -> graph edge storage methods
  -> edge preflight/cycle helpers
  -> tests
  -> docs/spec status update only after implementation is complete
```

## Validation

Run the narrowest checks first:

```text
cargo test -p elegy-planning graph_
```

Then run the full crate validation:

```text
cargo test -p elegy-planning
cargo fmt --check -p elegy-planning
cargo clippy -p elegy-planning -- -D warnings
```

If docs are updated during implementation, also run:

```text
elegy-documentation --json check --project .
```

## Risk Notes

- Schema versioning is the highest-risk part of phase 1. The migration chain
  must not repeat the earlier bug pattern where old migrations write the current
  schema constant instead of their target version.
- Do not expand `EntityType` for graph nodes in phase 1 unless necessary. It
  could ripple into validation, context, event, and tag logic before those
  behaviors are specified.
- Keep graph APIs storage-level for now. Public CLI shape should wait until the
  typed command-handler design is implemented.
- Do not backfill v1 records yet. Backfill needs its own migration/compatibility
  acceptance criteria and rollback plan.

## Done Criteria

- Fresh databases create graph tables and report schema version `9`.
- Existing v8 databases migrate to v9 without changing existing v1 records.
- Graph nodes and edges round-trip through typed Rust APIs.
- Invalid edges are rejected before write for missing nodes, cross-scope refs,
  invalid kind pairs, duplicate active edges, and acyclic-family cycles.
- Existing v1 tests continue to pass.
- No public CLI behavior changes in this phase.

## Links

- [Graph core spec](elegy-planning-graph-core.md)
- [Deterministic state machine spec](elegy-planning-state-machine.md)
- [Acceptance and evidence spec](elegy-planning-acceptance-evidence.md)
- [Run trace and context spec](elegy-planning-run-trace-context.md)
- [Adopt elegy-planning graph core ADR](../adr/2026-06-15-adopt-elegy-planning-graph-core.md)
