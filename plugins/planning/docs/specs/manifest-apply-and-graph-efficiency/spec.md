---
title: "Manifest Apply and Graph Efficiency Enhancements"
status: active
owner: Elegy
created: 2026-06-20
updated: 2026-06-20
doc_kind: spec
summary: Transactional manifest apply, idempotent upsert, dry-run, graph diff, and compact machine output on the typed graph model to collapse multi-step planning authoring into a single atomic operation.
schema_version: elegy-planning-manifest-apply/v0
---

# Manifest Apply and Graph Efficiency Enhancements

## Problem

Today, an agent building a planning graph must issue 10-15 sequential CLI commands
(`graph node create` x N, `graph edge create` x M, `graph acceptance create` x K,
`graph evidence create` x L), each with its own transaction and each requiring
read-back verification. This is slow, error-prone, and exposes partial state when
a mid-sequence command fails.

## Goals

1. Allow an agent to author a complete planning graph (nodes + edges) as a single
   YAML or JSON manifest file, then apply it atomically in one transaction.
2. Make repeated manifest application idempotent via stable user-provided IDs and
   graph-level upsert semantics.
3. Provide a dry-run mode that validates and reports what would change without
   committing.
4. Provide a diff command that compares a manifest against current database state.
5. Provide concise machine output to reduce token consumption for LLM consumers.

## Non-Goals

- Do not target legacy entity tables (goals, roadmaps, work_points, plans, todos).
  The manifest operates exclusively on the graph model (`planning_nodes` +
  `planning_edges`).
- Do not implement schema migration or compatibility bridges from legacy entities
  to the graph model.
- Do not replace individual graph CLI commands. They remain as the interactive
  surface for single-entity operations.
- Do not auto-generate IDs from content hashes. IDs are either user-provided slugs
  or auto-generated UUIDs.

## Behavior

### Manifest Schema

A manifest is a YAML or JSON file with this shape:

```yaml
# planning-manifest/v1
scope: "repo:myproject"
nodes:
  - id: g-ship-mvp                    # stable slug (optional; UUID if omitted)
    kind: goal
    title: "Ship MVP by June"
    summary: "Deliver the minimum viable product"
    status: active
    tags: [mvp, q2]
    payload:                           # kind-specific JSON (optional)
      acceptanceCriteria:
        - "All core tests pass"
        - "Deployment pipeline green"
      rejectionCriteria:
        - "Cannot deploy to staging"

  - id: r-mvp-roadmap
    kind: roadmap
    title: "MVP Roadmap"
    summary: "Phase-by-phase delivery plan"
    status: active

  - id: wp-auth
    kind: work
    title: "Implement authentication"
    summary: "OAuth2 with refresh tokens"
    status: proposed
    effortTier: balanced

  - id: wp-api
    kind: work
    title: "Build REST API"
    summary: "Core CRUD endpoints"
    status: proposed
    effortTier: balanced
    dependsOn: [wp-auth]               # shorthand for depends-on edges

  - id: p-auth-impl
    kind: plan
    title: "Auth implementation plan"
    summary: "Session-based plan for auth work"
    status: draft
    targetedWork: [wp-auth]

  - id: ac-auth-tests
    kind: acceptance
    title: "Auth tests pass"
    summary: "All authentication integration tests green"
    acceptanceKind: concrete
    description: "Run the full auth test suite"
    verificationPolicy: automated-ci

  - id: ev-test-run-1
    kind: evidence
    title: "Test run #42"
    summary: "All 128 auth tests passing"
    evidenceKind: test-result
    reference: "https://ci.example.com/runs/42"
    content: "128 passed, 0 failed, 0 skipped"

edges:
  - id: e-decomp-1
    kind: decomposes-to
    sourceNodeId: g-ship-mvp
    targetNodeId: r-mvp-roadmap
    status: active

  - id: e-decomp-2
    kind: decomposes-to
    sourceNodeId: r-mvp-roadmap
    targetNodeId: wp-auth
    status: active

  - id: e-dep-1
    kind: depends-on
    sourceNodeId: wp-api
    targetNodeId: wp-auth
    status: active

  - id: e-plan-1
    kind: planned-by
    sourceNodeId: wp-auth
    targetNodeId: p-auth-impl
    status: active

  - id: e-req-1
    kind: requires
    sourceNodeId: wp-auth
    targetNodeId: ac-auth-tests
    status: active

  - id: e-ev-1
    kind: evidenced-by
    sourceNodeId: ac-auth-tests
    targetNodeId: ev-test-run-1
    status: active
```

**Shorthand fields on nodes:**

| Field | Expands to |
|---|---|
| `dependsOn: [A, B]` | Two `depends-on` edges from this node to A and B |
| `blocks: [A, B]` | Two `blocks` edges from this node to A and B |
| `decomposesTo: [A, B]` | Two `decomposes-to` edges from this node to A and B |
| `plannedBy: [A]` | One `planned-by` edge from this node to A |
| `targetedWork: [A, B]` | On plan nodes: `planned-by` edges from A and B to this plan |
| `requiresEvidence: [...]` | List of evidence-kind requirements for acceptance nodes |
| `repairs: [A]` | One `repairs` edge from this node to A |
| `supersedes: [A]` | One `supersedes` edge from this node to A |

Shorthand edges are expanded during parsing and merged with explicit edges in
the `edges` list. Conflicts (same source+target+kind) are detected at parse time.

### Idempotent Upsert

When `planning manifest apply` runs:

- **Node with ID X exists in the same scope:** UPDATE title, summary, status,
  payload, tags. Revision is incremented.
- **Node with ID X does not exist:** INSERT new node.
- **Node with ID X exists in a different scope:** REJECT with `CROSS_SCOPE_CONFLICT`.
- **Edge with same (source, target, kind) and status=active exists:** Skip (no-op).
  If the edge exists but with different status or payload, UPDATE it.
- **Edge does not exist:** INSERT new edge.

This makes repeated runs idempotent.

### Transactional Apply

```
planning manifest apply --file plan.yaml

1. Parse manifest (YAML/JSON)
2. Validate manifest-internal consistency:
   - All edge source/target IDs exist within the manifest OR the DB
   - No cycles in depends-on, decomposes-to, or blocks subgraphs
   - No duplicate active edges
   - All scope keys match
3. Open IMMEDIATE transaction
4. For each node: upsert (INSERT or UPDATE)
5. For each edge: upsert (INSERT or UPDATE)
6. Run validation on all touched entities
7. Commit
8. Return: { createdNodes: [ids], revisedNodes: [ids], unchangedNodes: [ids],
              createdEdges: [ids], revisedEdges: [ids], unchangedEdges: [ids],
              conflicts: [{id, reason}], validation: {...} }
```

### Dry-Run

```
planning manifest apply --file plan.yaml --dry-run
```

Same as apply but rolls back the transaction at step 7. Output is identical
in shape, showing what *would* happen.

### Graph Diff

```
planning diff --manifest plan.yaml
```

Compares the manifest against the current database state without any transaction:

- **Added nodes:** in manifest, not in DB
- **Removed nodes:** in DB, not in manifest (only reported, not deleted)
- **Changed nodes:** field-level differences
- **Unchanged nodes:** identical
- **Added/removed/changed edges:** same semantics

### Concise Output

`--compact` flag added to all graph commands and manifest apply:

- Node output: `{ id, kind, title, status }` only
- Edge output: `{ id, kind, sourceNodeId, targetNodeId, status }` only
- manifest apply output: `{ createdNodes: [ids], revisedNodes: [ids], ... }` (IDs only, not full records)
- `--expanded` (default) restores full record output

### Manifest Apply CLI

```bash
# Apply a manifest
elegy-planning manifest apply --file plan.yaml --scope repo:myproject

# Dry-run
elegy-planning manifest apply --file plan.yaml --scope repo:myproject --dry-run

# With compact output
elegy-planning manifest apply --file plan.yaml --compact

# Diff manifest against DB
elegy-planning diff --manifest plan.yaml --scope repo:myproject

# Diff with compact output
elegy-planning diff --manifest plan.yaml --compact
```

## Acceptance Criteria

- [ ] `planning manifest apply --file plan.yaml` creates/updates all nodes and edges in one transaction.
- [ ] Repeated apply with the same manifest is a no-op (idempotent).
- [ ] `--dry-run` reports changes without committing.
- [ ] Cross-scope node conflicts are rejected with `CROSS_SCOPE_CONFLICT`.
- [ ] Shorthand edge fields on nodes (dependsOn, blocks, decomposesTo, plannedBy, repairs, supersedes) expand correctly.
- [ ] `planning diff --manifest plan.yaml` reports field-level differences.
- [ ] `--compact` flag reduces output to minimal fields on all graph commands.
- [ ] `cargo test -p elegy-planning` passes all existing and new tests.
- [ ] `cargo clippy -p elegy-planning -- -D warnings` passes.
- [ ] `cargo fmt --check -p elegy-planning` passes.

## Implementation Links

- `rust/features/elegy-planning/src/manifest.rs` — New module: manifest types, parser, expansion
- `rust/features/elegy-planning/src/storage.rs` — Upsert methods, transactional apply, diff queries
- `rust/features/elegy-planning/src/cli.rs` — New Command::Manifest variant, diff command, --compact flag
- `rust/features/elegy-planning/src/model.rs` — Manifest types, apply result types
- `rust/features/elegy-planning/contracts/schemas/planning-manifest.schema.json` — JSON Schema for manifest format
- `rust/features/elegy-planning/tests/integration.rs` — Integration tests for manifest apply, diff, compact output

## Links

- [Graph core spec](../graph-core.md)
- [Agent-safe planning core spec](../agent-safe-planning-core/spec.md)
- [Graph-core phase 1 plan](../graph-core-phase-1-plan.md)
