---
name: elegy-memory
description: Use when an agent needs to add, search, list, inspect, purge, health-check, export, or contradiction-review local memory records through the dedicated elegy-memory CLI over SQLite. The current MVP is keyword-only and SQLite-backed; embedding providers are preview-only.
---

# Elegy Memory

> Use when an agent needs to add, search, list, inspect, purge, health-check, export, or contradiction-review local memory records through the dedicated `elegy-memory` CLI over SQLite. The current MVP is keyword-only and SQLite-backed; embedding providers are preview-only.

Local memory is a non-authoritative, agent-local SQLite cache. The
salience gate is mandatory on every write: a memory that fails the
gate is rejected, not stored. Provenance is required: every record
records who stated the fact and how.

## Quick start

1. Confirm the database path. Default is `~/.elegy/memory.db`. Pass
   `--db <path>` to use a different one.
2. Add a distilled memory:
   `elegy-memory add "User prefers TypeScript strict mode" --type preference --importance 0.8 --provenance user-stated --scope workspace --format json`.
3. Search for it:
   `elegy-memory search "TypeScript strict" --scope workspace --limit 10 --format json`. The current `search` is keyword-only — semantic search is preview.
4. Inspect a specific record:
   `elegy-memory inspect <id> --scope workspace --format json`.
5. Run a contradiction review on a scope:
   `elegy-memory contradictions --scope workspace --format json` to
   surface pairs that may disagree.

## Tool-call guardrails

### Add (`memory-add`)

- `content` is the **distilled** memory. Do not pass raw transcripts,
  raw tool output, or entire conversation blocks. Distill first; the
  salience gate expects concise, durable facts.
- `type` is required (or default to `observation`): `fact`,
  `preference`, `decision`, `procedure`, `observation`. The type
  affects how the salience gate scores the candidate.
- `provenance` is required (or default to `user-stated`):
  `user-stated`, `agent-observed`, `consolidated`, `imported`,
  `agent-inferred`. Storing without provenance is rejected.
- `importance` is `0.0` to `1.0`. Values above `0.9` are reserved for
  decisions and explicit user-stated rules; the gate will reject
  `1.0` as a sign of an over-confident write.
- `scope` is `session`, `workspace`, `user`, or `agent`. Scopes do
  not cross-query: a `session`-scoped memory is invisible to a
  `workspace`-scoped `search`. Pick the most-restrictive scope that
  still lets the next session find it.
- Side-effect class: `disk_write` against the SQLite database.
- Approval posture: `advisory` for normal writes; `required` for
  `provenance: agent-inferred` and `provenance: imported`.

### Search (`memory-search`)

- The current MVP is **keyword-only**. Substring match across
  `content` and `tags`. There is no semantic search yet.
- `--include-dormant` is required to surface records marked dormant
  by the contradiction review. The default excludes dormant.
- `--limit` defaults to a small number. Set it explicitly when the
  caller cares about the long tail.
- `--embedding-provider <name>` is preview-only. The CLI accepts the
  flag but does not call any provider yet. See the reembed family
  below.
- Side-effect class: `read_only`.
- Approval posture: `none`.

### List / inspect (`memory-list`, `memory-inspect`)

- `list` filters on `--type`, `--state`, `--scope`, and `--limit`.
  `state` is `active`, `dormant`, or `deleted`. `deleted` records
  exist in the database but are excluded from default listing.
- `inspect` requires a record id from a prior `add`, `list`, or
  `search` result. Do not construct ids.
- Side-effect class: `read_only`.
- Approval posture: `none`.

### Purge (`memory-purge`)

- `purge` deletes records across a scope. It is destructive and
  irreversible.
- Pass `--yes` to skip the interactive confirmation. Without
  `--yes`, the CLI prompts; in machine mode this is a failure.
- Confirm scope and filters with the user before invoking. The
  default scope is `workspace`; a `purge --scope user` is a much
  larger blast radius.
- Side-effect class: `disk_write` (destructive).
- Approval posture: `required`. The host must explicitly approve a
  purge.

### Health / export / reembed / contradictions

- `health` is read-only and reports database size, FTS5 index state,
  dormant count, and embedding provider status.
- `export` writes a JSON snapshot to `--output <path>`. Confirm the
  path with the user before invoking; the file is overwritten if it
  exists.
- `reembed` is **preview-only** in the current MVP. The CLI accepts
  the call and emits a preview report describing what *would* be
  re-embedded, but does not call any provider. Do not assume
  embeddings have been refreshed until the host wires a provider.
- `contradictions` runs a rule-based review and emits pairs that
  may disagree. Resolution is manual; the CLI does not merge or
  delete conflicting records.
- Side-effect classes: `read_only` for health, export (writes a
  file), reembed (preview), contradictions. Approval posture:
  `none` for read-only; `advisory` for export; `advisory` for
  reembed (preview only).

## Workflow

1. Pick the scope.
   - `session` for in-flight context that should not survive a
     session boundary.
   - `workspace` for facts about the current project that should
     survive across sessions in the same workspace.
   - `user` for preferences and decisions about the human user
     that should follow them across workspaces.
   - `agent` for facts the agent has learned that should follow
     the agent identity across users and workspaces.
2. Distill, then add.
   - Distill raw observations into one or two short sentences. The
     salience gate rewards concise, durable statements and
     penalizes vague or duplicative writes.
3. Search before adding.
   - The next call will be `search`. If the new memory would
     duplicate an existing record, prefer updating the existing
     record (and recording a `consolidated` provenance) over
     adding a near-duplicate.
4. Inspect, then update.
   - Use `inspect` to see the full record and the source provenance
     chain before deciding to update or supersede.
5. Run contradiction review.
   - Periodically (and before declaring a long-running agent done),
     call `contradictions` to surface disagreements. Resolve by
     superseding, not by deletion.

## Capability index

| id | side-effect | purpose |
| -- | -- | -- |
| `memory-add` | disk_write | Add a distilled memory with type, importance, provenance, scope |
| `memory-search` | read-only | Keyword search; semantic preview-only |
| `memory-list` | read-only | List memories filtered by type, state, scope |
| `memory-inspect` | read-only | Show one record plus provenance chain |
| `memory-purge` | disk_write | Destructive: delete records across a scope (requires `--yes`) |
| `memory-health` | read-only | Report database size, FTS5 state, dormant count, provider status |
| `memory-export` | disk_write | Write a JSON snapshot to `--output <path>` |
| `memory-reembed` | read-only (preview) | Preview re-embedding; no provider wired yet |
| `memory-contradictions` | read-only | Rule-based contradiction review across a scope |

## Output envelope

- Envelope: `memory-add-result/v1`, `memory-search-result/v1`,
  `memory-list-result/v1`, `memory-inspect-result/v1`,
  `memory-purge-result/v1`, `memory-health-result/v1`,
  `memory-export-result/v1`, `memory-reembed-result/v1`,
  `memory-contradictions-result/v1` — one per command family. Each
  envelope declares its `schemaVersion` as the first field; validate
  before parsing.
- Common fields:
  - `status`: `ok`, `partial`, or `error`.
  - `data`: command-specific payload.
  - `gate`: salience gate verdict for `add`. `accepted`, `rejected`,
    or `coalesced`. `rejected` includes a reason in `gate.reason`.
  - `correlationId`: optional, populated when the host passes one.
  - `error`: machine-readable error code plus human message on
    failure.

## Common issues

| Symptom | Cause | Solution |
| -- | -- | -- |
| `memory add` returns `gate: "rejected"` with a vague reason. | The content was too long, too vague, or duplicated an existing record. | Distill to a single durable sentence. Re-run `search` to see existing records and prefer `consolidated` provenance if the new content is a refinement. |
| `memory search` returns nothing for a query the user expected to match. | The current MVP is keyword-only. Synonyms, paraphrases, and semantic intent do not match. | Re-query with literal substrings the user used. Do not assume semantic search is wired. |
| `--include-dormant` does not surface a record the user knows exists. | The record's `state` is `deleted`, not `dormant`. `deleted` records are not surfaced even with `--include-dormant`. | `memory inspect` with the known id; if the id is unknown, the record is not in the active scope or has been hard-deleted. |
| `memory reembed` reports zero processed despite stale records. | The provider is not configured. The current MVP does not wire any embedding provider; `reembed` is a preview. | Treat the output as advisory. The records remain stale until a provider ships. Do not claim embeddings have been refreshed. |
| `memory purge --scope user` purges more than the user expected. | `user` is the largest scope. The user-scope covers all workspaces for the user. | Confirm the scope with the user. Consider `workspace` for project-scoped cleanup, `session` for in-flight cleanup. |
| `memory export` overwrites an existing file. | The CLI does not prompt in machine mode and the path exists. | Confirm `--output` is a fresh path or coordinate with the user before invoking. |
| `memory contradictions` returns many pairs that look the same. | The rule-based review can over-fire on near-duplicates. | Treat as a triage signal, not a hard conflict. Inspect the high-importance pairs first. |
| `memory add` rejects with "provenance required" even though the agent inferred the fact. | `agent-inferred` is a valid provenance, but the gate weighs it lower; the rejection may actually be about importance, not provenance. | Lower `importance` to a value the gate accepts (`< 0.7` for `agent-inferred`), or upgrade to `user-stated` after the user confirms. |
| Search across scopes returns records from the wrong scope. | Scopes do not cross-query, but `--scope` is optional and defaults to `workspace`. | Always pass `--scope` explicitly to make the query scope-visible. |

## Version compatibility

- Minimum supported `elegy-memory` version: `0.1.0`.
- The MVP is SQLite-only. There is no PostgreSQL or remote path in
  scope.
- `reembed` is preview-only and the embedded provider surface is
  not yet wired. Treat the capability as advisory until the
  provider ships.
- Semver rule: minor must be ≥ the version that introduced the
  capability. `contradictions` is recent; `reembed` is preview.

## Examples

### Example 1 — add a preference and confirm with search

```text
elegy-memory add "User prefers tabs over spaces" \
  --type preference --importance 0.7 \
  --provenance user-stated --scope user --format json
```

Expected: `gate: "accepted"`, `data.memory.id` returned, `data.memory.provenance: "user-stated"`.

Then confirm:

```text
elegy-memory search "tabs spaces" --scope user --limit 5 --format json
```

Expected: the record returned with `matchType: "keyword"`.

### Example 2 — contradiction review

```text
elegy-memory contradictions --scope workspace --format json
```

Expected: a list of pairs with `recordA` and `recordB` plus a
`reason` string. The CLI does not merge or delete; the agent
chooses the resolution.

## Boundaries

- This skill owns: local memory records, the salience gate, the
  scope model, the provenance chain, and the keyword search
  surface.
- This skill does not own: durable planning records (use
  `elegy-planning`), vault operations (use `elegy-obsidian`), or
  agent-host projection.
- This skill does not own: cross-host memory sync. Memory is local
  to the SQLite file; do not assume another host can see it.
- Companion skills:
  - `elegy-planning` — for planning state, not free-form memory.
  - `elegy-obsidian` — for vault-side mirrors.
  - `elegy-skills` — for registry operations.
  - `elegy-skill-authoring` — for SKILL.md audit and review.

## References

- Governed source: `contracts/fixtures/skill.elegy-memory.json`.
- Discovery projection:
  `contracts/fixtures/skill-discovery-index.elegy-memory.json`.
- Architecture:
  `rust/crates/elegy-memory/docs/architecture/ARCHITECTURE.md`.
- Guardrails: `rust/crates/elegy-memory/AGENTS.md`.
- Result envelope schemas: `contracts/schemas/memory-*.schema.json`.
- MVP scope: `rust/crates/elegy-memory/docs/mvp-scope.md`.
