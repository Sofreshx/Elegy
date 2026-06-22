---
title: Codegraph Diff, Review, and Validate Commands (Deferred)
status: draft
owner: Elegy
created: 2026-06-15
updated: 2026-06-15
doc_kind: spec
summary: Deferred spec stub for the elegy-codegraph diff, review, and validate commands. Queued for implementation after the v0 extract + query prototype is validated against the acceptance checks in codegraph-and-quality-plugin-research.md.
---

# Codegraph Diff, Review, and Validate Commands (Deferred)

## Status

**Deferred — queued after v0 extract + query.** The v0 prototype implements only `extract` and `query`. The `diff`, `review`, and `validate` commands described here are the next logical expansion once the extractor quality, query usefulness, and agent integration patterns are proven on real TypeScript and Rust repos.

## Motivation

The `extract` + `query` surface gives agents structural evidence about a codebase at a point in time. The next frontier is **change-aware** and **quality-aware** evidence:

- **diff:** what structurally changed between two branches, commits, or file sets.
- **review:** apply portable rule packs (conventions, correctness, security) against the extracted graph.
- **validate:** check graph freshness, schema compliance, and provenance health.

Together, these three commands close the loop from "understand the codebase" to "understand what changed and whether it's correct."

## Command Proposals

### `elegy-codegraph diff`

```
elegy-codegraph diff --base <graph.bin> --target <graph.bin> [--path <file>]
```

Compare two graph snapshots and emit structurally relevant changes:

- **Added entities/edges:** new symbols, new dependencies, new test coverage.
- **Removed entities/edges:** deleted symbols, broken references.
- **Modified entities:** symbol signature changes (inputs/outputs changed, sideEffects changed).
- **Impact summary:** which files are affected by the changes.

Output: JSON diff envelope with before/after entity+edge pairs and a summary of net changes. Each change cites provenance from both snapshots.

### `elegy-codegraph review`

```
elegy-codegraph review --graph <graph.bin> --rules <rules.toml> [--path <file>]
```

Apply portable rule packs against the graph:

- **Convention rules:** naming patterns, file/module organization, dependency direction (e.g. "src may not import from tests").
- **Correctness rules:** unused symbols, circular dependencies, missing tests for exported symbols.
- **Security rules:** dangerous side-effect patterns (e.g. `process.exec` without input validation).

Rules are authored in a TOML format with a graph-query DSL (exact syntax TBD — candidates include a simple predicate language or integration with ast-grep/Semgrep rule formats).

### `elegy-codegraph validate`

```
elegy-codegraph validate --graph <graph.bin>
```

Validate graph health:

- **Schema compliance:** does the graph conform to `elegy-codegraph.graph.v0`?
- **Provenance health:** are there entities/edges with missing or low-confidence provenance?
- **Staleness:** is the graph older than the repo's last commit? (requires `extract` timestamp metadata)
- **Referential integrity:** are all edge `src`/`dst` IDs present in the entities table?

Output: JSON validation report with findings keyed by severity (error, warning, info).

## Acceptance Criteria Template

- [ ] `diff` correctly identifies added/removed/modified entities and edges between two graphs extracted from the same repo at different commits.
- [ ] `review` runs a rule pack against a graph and surfaces findings with provenance (file:span + rule reference).
- [ ] `validate` surfaces schema violations, provenance gaps, staleness, and referential breaks.
- [ ] All three commands emit JSON with provenance and confidence.
- [ ] The plugin boundary stays host-neutral: no host imports, no MCP requirement.
- [ ] Integration tests exist for each command against fixture repos.

## Integration Plan

1. **Prerequisite:** v0 `extract` + `query` passes all acceptance checks in `codegraph-and-quality-plugin-research.md`.
2. **Phase A:** implement `diff` against two stored graph snapshots (simplest command, no external tool dependency).
3. **Phase B:** implement `validate` (schema + provenance checks, also no external dependency).
4. **Phase C:** implement `review` with a TOML rule format and graph-query DSL (most complex, requires DSL design).
5. **Agent integration:** expose all three commands via CLI invocation templates; evaluate MCP adapter after CLI validation.

## Non-Goals

- Do not implement any of these commands in v0 of `elegy-codegraph`.
- Do not design the rule-query DSL until the extractor and query patterns are stable.
- Do not add host-specific UI, dashboards, or approval workflows.

## Implementation Links

- `./plugin-research.md` — parent v0 spec
- `contracts/schemas/elegy-codegraph.graph.v0.json` — governed IR schema
- `rust/features/elegy-codegraph/src/main.rs` — TODO stubs for deferred commands

## Drift Notes

- This spec records design intent only. The command surface, rule format, and integration timing may change based on lessons learned from the v0 prototype.
- If research during v0 shows that existing tools (e.g., Semgrep diff, git diff --stat) already provide enough of the `diff`/`review` surface, prefer wrapping those tools over building custom implementations.
- Promote to `active` only after v0 acceptance checks are met and the team decides to proceed.
