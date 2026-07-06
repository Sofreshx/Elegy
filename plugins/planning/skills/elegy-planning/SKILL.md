---
name: elegy-planning
description: Use when an agent needs to create, inspect, update, validate, or export durable planning state — goals, roadmaps, plans, work points, todos, issues, review points, insights, and project runs — through the dedicated elegy-planning CLI over SQLite.
---

# Elegy Planning

> Use when an agent needs to create, inspect, update, validate, or export durable planning state — goals, roadmaps, plans, work points, todos, issues, review points, insights, and project runs — through the dedicated `elegy-planning` CLI over SQLite.

SQLite is the durable authority. Markdown and JSON projections are
generated, derived outputs. Omitted scope defaults to `default` and
that silent default is a common source of agent mistakes — always pass
`--scope <scope-key>` explicitly.

## Setup and authority

- Run from the Elegy repo root during development:
  `cargo run -p elegy-planning -- --help`.
- Use `--db <path>` when the default repo-local SQLite database is not the
  intended planning authority.
- Use `--scope <scope-key>` on every call. Scope is part of the planning
  identity, not display metadata.
- Use `--json --non-interactive --correlation-id <id>` on every mutating call.
- Do not edit SQLite directly. Use CLI commands so validation, FTS, tags, and
  event history stay synchronized.
- Serialize mutating calls. SQLite has a busy timeout, but agent workflows
  should not launch parallel writes.

## Ideology

- `elegy-planning` is an agent contract, not a note-taking format.
- SQLite is authority; projections, templates, and skill docs are routing
  surfaces.
- Durable writes should append events. Events are the review trail for later
  agents.
- Validation is advisory but persistent. Structural write invariants block bad
  records; planning-quality findings guide follow-up work.
- Stable explicit IDs are automation handles. Prefer them over generated IDs
  in agent-authored workflows.
- Batch authoring should be previewable. Use `roadmap scaffold --dry-run`,
  inspect `rejected`, then apply.

## Quick start

1. Resolve the scope key. Use
   `elegy-planning --scope <scope-key> scope list --json` to confirm the
   scope exists. If the user did not name one, ask.
2. Create a goal:
   `elegy-planning --scope <scope-key> --json --non-interactive --correlation-id <id> goal create --id <slug> --title <t> --description <d> --acceptance <a> --acceptance <a> --rejection <r> --rejection <r>`.
   Repeat `--acceptance` and `--rejection` for multiple criteria. Do
   not comma-join values.
3. Add a work point to a roadmap:
   `elegy-planning --scope <scope-key> --json --non-interactive --correlation-id <id> roadmap add-section --roadmap-id <r> --id <s> --slug <slug> --title <t>` followed by
   `roadmap add-work-point --roadmap-id <r> --id <wp> --title <t> --summary <s> --effort-tier <fast|balanced|deep> --file-scope <type:intent:selector>`.
4. For batch roadmap authoring, render `roadmap-workflow` and apply it:
   `elegy-planning template render --template roadmap-workflow --output roadmap.yaml`, then
   `elegy-planning --scope <scope-key> --json --non-interactive --correlation-id <id> roadmap scaffold --file roadmap.yaml --dry-run`.
5. Inspect context before deep work:
   `elegy-planning --scope <scope-key> --json context --entity-type goal --entity-id <id>` to load the goal plus related insights and
   token estimates.
6. Run a full validation pass:
   `elegy-planning --scope <scope-key> --json validate all` to surface
   referential integrity issues and stale references.

## Tool-call guardrails

### Read family (goal/roadmap/plan/work-point/todo/issue/review-point
show & list, scope, search-extended, tags-list, context, work-graph,
next-runnable)

- Argument shape: `<entity> show --<entity>-id <id> --json`. `show --id <id>`
  is also accepted for goal, roadmap, work-point, plan, issue, insight, graph
  node, graph edge, acceptance, and evidence show commands.
  The `--json` flag is required for machine-mode parsing; do not omit it even
  on a "quick check".
- For list commands, pass `--limit <n>` to cap the result set; the
  default limit is conservative but explicit is safer.
- `search` and entity-local `search` commands support `--title`, `--tag`,
  `--status`, and `--fts`. Pass each filter as a separate flag; do not stack
  them in `--query`.
- `context --entity-type <type> --entity-id <id>` returns progressive
  disclosure bundles with token estimates. The estimate is
  informational; do not parse it.
- `work-graph` and `next-runnable` are read-only but can return
  large payloads; always pass `--limit` for `next-runnable` to avoid
  pulling the whole work queue.
- `insight list --all` lists all insights in the active scope.
  Omit `--all` and pass `--parent-type` + `--parent-id` for
  parent-specific listing (existing behavior).
- Side-effect class: `read_only`.
- Approval posture: `none`.

### Mutate family (create / update-status / plan-revise / insight-record)

- Always pass `--json --non-interactive --correlation-id <id>` on every
  mutating call. The CLI refuses interactive prompts when
  `--non-interactive` is set, and a missing `--correlation-id` causes
  the call to fail under machine mode. Both flags together are the
  contract.
- Multi-value flags (`--acceptance`, `--rejection`, `--tag`,
  `--file-scope`, `--related-entity`) must be **repeated** per value:
  `--acceptance <a1> --acceptance <a2>`. Comma-joining is silently
  dropped.
- `plan-revise` removal semantics: passing `--routing-hint ""` or
  omitting `--file-scopes` does **not** clear existing values. Use
  `--clear-routing-hint` and `--clear-file-scopes` to remove
  previously set values. These two flags are the only reliable way
  to clear.
- `--effort-tier` is required for `roadmap add-work-point` and
  recommended for plan and todo authoring. Valid values are
  `fast`, `balanced`, `deep`. The value affects validation depth,
  not the durable record.
- `roadmap add-work-point --dependency-id <id>` only accepts dependencies
  that belong to the same roadmap. Cross-roadmap dependencies are
  rejected at write time with `"status": "invalid"` and a
  descriptive `error` message.
- Use `work-point revise --work-point-id <id> --dependency-id <id>...`
  to add, replace, or `--clear-dependencies` to remove work-point
  dependencies. Do not attempt direct SQLite repair.
- File-scope selector grammar: `<type>:<intent>:<selector>`. Types
  are `exact` or `glob`. Intents are `primary`, `review`, or
  `affected`. Example: `--file-scope glob:primary:shared/core/**`.
- `--status` on `*-update-status` accepts the entity's lifecycle
  states (e.g. `draft`, `proposed`, `active`, `validated`,
  `invalidated`, `superseded`, `abandoned` for goals). Do not
  transition to a state the entity is not currently allowed to
  leave.
- `scope create --metadata-file <path>` reads metadata from a JSON
  file. Mutually exclusive with `--metadata-json`. Errors include
  the file path for faster diagnosis.
- Side-effect class: `disk_write` against the SQLite database.
- Approval posture: `advisory`. The host may require approval for
  specific transitions (e.g. `validated`, `invalidated`).
- Run mutating commands and read-after-write checks sequentially. SQLite uses a
  local file lock; parallel show/list calls during writes can still race on busy
  workstations.

### Roadmap scaffold family

- Use `roadmap scaffold --file <yaml|json> --dry-run` before `--apply`.
- The scaffold file creates v1 records: scope, goal, roadmap, sections,
  work-points, plan, and todos. It is separate from graph `manifest`.
- Dry-run and apply use the same transaction path. Apply rolls back all
  scaffold-created or scaffold-updated rows when any entity is rejected.
- Work-point dependencies may reference work-points declared later in the same
  scaffold file.
- Omitted ordering on update preserves the existing order.
- Dry-run and apply return `created`, `updated`, `unchanged`, `skipped`,
  `rejected`, `validationFindings`, and `nextRunnableWorkPoints`.
- `--if-exists fail` is the default for planning records. Use
  `--if-exists skip` for idempotent create-only automation.
- `--if-exists update` updates supported content fields, reports `updated`
  only when a persisted change occurred, and rejects parent-link drift such as
  moving a roadmap to another goal.
- Scaffold `validationFindings` are limited to touched scaffold entities and
  directly affected parents. Run `validate all` for a full-scope audit.
- Side-effect class: `disk_write` with `--apply`; read-only planning preview
  with `--dry-run`.
- Approval posture: `advisory`.

### Project-run family (claim / activate / release / add-evidence)

- `project-run-claim` is a durable lease. It is **not** a soft
  reservation; if the lease exists, the work point is considered
  in-flight until `release` is called. Always pass the full
  scope: `--goal-id`, `--roadmap-id`, `--work-point-id`, `--repo`,
  `--branch`, `--worktree`, `--session`, `--profile`.
- `project-run-add-evidence` appends evidence to a run; evidence is
  immutable once recorded. Do not "fix" a run by re-adding evidence
  — open a new run or supersede the old one.
- Side-effect class: `disk_write` plus cross-host lease visibility.
- Approval posture: `required`. The host must explicitly approve
  lease creation or release.

### Validation / health / export (validate all, health, project-export,
project-render)

- `validate all` validates only the active scope by default. Use
  `validate all --all-scopes` for explicit global audits across
  every scope. The output includes `scopeMode` (`"single"` or
  `"all"`) and `scopeKey` to confirm which scope(s) were checked.
- `health` is read-only but expensive on large databases. Schedule
  it, do not run it per-keystroke.
- `health.data.fts` reports `tablesPresent`, aggregate counts, per-entity
  `byEntityType` drift, and `findings`. Treat FTS drift as a search reliability
  issue.
- `project-export` and `project-render` write to disk under the path
  passed via `--output <path>`. Confirm the path with the user
  before invoking; the file is overwritten if it exists.
- `project-export` emits JSON; `project-render` emits Markdown.
  Pick the right one for the consumer.
- Side-effect class: `disk_write` for export/render; `read_only` for
  validate/health.
- Approval posture: `advisory` for validate/health; `required` for
  export/render if the output path is outside the user's working
  directory.

## Workflow

1. Resolve scope.
   - If the user did not name a scope, call `scope list --json` and
     ask. Never let `--scope` default to `default` silently.
2. Author top-down.
   - Goal first, then roadmap, then plan, then work points, then
     todos. Authoring in this order lets `--file-scope` selectors
     reference the upstream entity and lets validation catch
     referential breaks early.
3. Record insights as you go.
   - Every time the user makes a non-obvious decision, call
     `insight record` with `--insight-type <type> --tag <tag>`. The
     next session's `context` call will surface them.
4. Validate before declaring done.
   - Run `validate all` and check that the result has no Critical
     findings. Treat High findings as blockers for a "done" claim.
5. Render or export for human consumption.
   - `project-render` for Markdown review, `project-export` for
     machine-readable handoff. The output is a derived artifact,
     not authority.

## Capability index

| id | side-effect | purpose |
| -- | -- | -- |
| `planning-goal-create` | disk_write | Create a durable goal with acceptance and rejection criteria |
| `planning-goal-show` | read-only | Show one goal plus linked context |
| `planning-goal-list` | read-only | List goals in the active scope |
| `planning-goal-update-status` | disk_write | Transition a goal to a new lifecycle state |
| `planning-roadmap-create` | disk_write | Create a roadmap under a goal |
| `planning-roadmap-add-section` | disk_write | Add a section to a roadmap |
| `planning-roadmap-add-work-point` | disk_write | Attach a work point with file scopes and effort tier |
| `planning-roadmap-scaffold` | disk_write | Batch create a v1 goal, roadmap, sections, work-points, plan, and todos from YAML or JSON |
| `planning-roadmap-show` | read-only | Show one roadmap with sections and work points |
| `planning-roadmap-list` | read-only | List roadmaps in the active scope |
| `planning-roadmap-update-status` | disk_write | Transition a roadmap |
| `planning-plan-create` | disk_write | Create a plan under a roadmap section |
| `planning-plan-show` | read-only | Show one plan with todos and evidence |
| `planning-plan-list` | read-only | List plans in the active scope |
| `planning-plan-revise` | disk_write | Revise a plan; use `--clear-routing-hint` / `--clear-file-scopes` to remove |
| `planning-plan-update-status` | disk_write | Transition a plan |
| `planning-work-point-list` | read-only | List work points in the active scope |
| `planning-work-point-show` | read-only | Show one work point with file scopes |
| `planning-work-point-update-status` | disk_write | Transition a work point |
| `planning-work-point-next-runnable` | read-only | List runnable work points ordered by effort and readiness |
| `planning-work-point-revise` | disk_write | Revise a work point's dependency list or other fields |
| `planning-work-point-work-graph` | read-only | Render the work graph for the active scope |
| `planning-todo-create` | disk_write | Create a todo under a plan |
| `planning-todo-list` | read-only | List todos in the active scope |
| `planning-todo-update-status` | disk_write | Transition a todo |
| `planning-issue-record` | disk_write | Record an issue tied to a planning entity |
| `planning-issue-list` | read-only | List issues in the active scope |
| `planning-issue-update-status` | disk_write | Transition an issue |
| `planning-review-point-record` | disk_write | Record a review point on a planning entity |
| `planning-review-point-update-status` | disk_write | Transition a review point |
| `planning-insight-record` | disk_write | Record a reasoning insight attached to any planning entity |
| `planning-events-list` | read-only | List the planning event log for the active scope |
| `planning-scope-list` | read-only | List all known scopes |
| `planning-scope-show` | read-only | Show one scope with its entities |
| `planning-scope-create` | disk_write | Create a new scope |
| `planning-tags-list` | read-only | List all indexed tags across entities |
| `planning-search-extended` | read-only | Title / tag / status / FTS search |
| `planning-context-entity` | read-only | Progressive disclosure bundle for one entity |
| `planning-context-session` | read-only | Progressive disclosure bundle for a session |
| `planning-validate-all` | read-only | Run referential integrity and freshness validation |
| `planning-health` | read-only | Surface database health, FTS5 index drift, lease state |
| `planning-project-export` | disk_write | Export a scope to JSON |
| `planning-project-render` | disk_write | Render a scope to Markdown |
| `planning-project-run-claim` | disk_write | Claim a durable execution lease on a work point |
| `planning-project-run-activate` | disk_write | Activate a claimed run |
| `planning-project-run-release` | disk_write | Release a lease |
| `planning-project-run-add-evidence` | disk_write | Append immutable evidence to a run |
| `planning-project-run-list` | read-only | List active project runs |
| `planning-project-run-show` | read-only | Show one project run with full evidence trail |

## Output envelope

- Envelope: `planning-result/v1` (declared in
  `plugins/planning/schemas/planning-result.schema.json`).
- `status`: `ok`, `partial`, or `error`. Partial means the call
  succeeded but some inner sub-result failed; surface the inner
  failures.
- `data`: entity payload or list of payloads, depending on the call.
- `validation`: validation findings, when the call performed any
  validation (e.g. `validate all`).
- `correlationId`: echoes the `--correlation-id` passed to the call.
  Use this for cross-call lineage.
- `error`: machine-readable error code plus human message. The
  machine code is in `error.code`; the message is in `error.message`.
- `data.scopeMode` (for `validate all`): `"single"` or `"all"`,
  indicating validation scope.
- `data.scopeKey`: the active scope key, or `"all"` for global audits.
- `data.insights`: list of insight records (for `insight list --all`).
- `data.scopeKey` in each finding: the scope the finding belongs to.
- `data.fingerprint` in each finding: stable identifier for
  deduplication (`entityType::entityId::scopeKey::code`).

## Common issues

| Symptom | Cause | Solution |
| -- | -- | -- |
| The call returns results from a different scope than the user asked about. | `--scope` was omitted and the CLI defaulted to `default`. | Always pass `--scope <scope-key>` explicitly. The silent default is the most common planning bug. |
| `goal create` rejects the call with "missing correlation-id" even though the user did not specify one. | Machine mode requires `--correlation-id` on every mutation. | Generate a fresh id (`uuidgen` or the host's equivalent) and pass it on every mutating call. |
| `plan revise` appears to succeed but the routing hint or file scopes are not actually cleared. | Empty values are dropped; only the explicit `--clear-routing-hint` and `--clear-file-scopes` flags clear. | Add the explicit clear flags. Re-run `plan show` to confirm the cleared state. |
| Multi-value flags silently drop all but the first value. | The agent joined values with commas or `;` instead of repeating the flag. | Repeat the flag once per value. The CLI does not warn. |
| `roadmap add-work-point` rejects with "selector grammar invalid". | The `<type>:<intent>:<selector>` shape was malformed (missing colons, unknown type, unknown intent). | Re-emit with the exact grammar. Types are `exact` or `glob`. Intents are `primary`, `review`, or `affected`. |
| `context --entity-type goal --entity-id <id>` returns a huge payload. | The goal has many linked insights and a wide work graph. | Pass `--include <entity-type>[,<entity-type>...]` to narrow the bundle. The default is "all linked entities". |
| `validate all` returns Critical findings that did not exist yesterday. | A recent mutation broke referential integrity (orphan work point, dangling roadmap reference). | Re-author the broken upstream entity and re-run validation. Do not delete the broken record without surfacing it to the user first. |
| `project-run-claim` returns "lease already held". | Another session claimed the same work point. | List active runs (`project-run list`) and either wait for release, pick a different work point, or coordinate with the holding session. Do not force-release another session's lease. |
| `project-export` overwrites an existing file the user cared about. | `--output` points at an existing path and the CLI does not prompt in non-interactive mode. | Confirm the path with the user before invoking. Pick a fresh `--output` path for each export. |
| `health` shows FTS5 index drift. | The FTS5 mirror was not updated after a bulk insert. | Run the FTS5 rebuild command documented in the planning health reference, or recreate the FTS5 mirror from the source table. |
| `next-runnable` returns work points that look ready but are blocked. | The work point's upstream dependencies have not all reached `validated`. | Inspect the work graph with `work-graph`; the ready-set excludes unvalidated upstream by default but a `--include-blocked` flag changes that. |
| `roadmap add-work-point --dependency-id` is rejected even though the work point exists. | The dependency work point belongs to a different roadmap. | Use only same-roadmap dependencies for now. Cross-roadmap sequencing is deferred to a later model. |
| `validate all` returns findings from a different scope. | `--all-scopes` was passed (or omitted unintentionally). | By default, `validate all` only validates the active scope. Pass `--all-scopes` to include all scopes. |

## Version compatibility

- Minimum supported `elegy-planning` version: `0.1.0`. The CLI is
  pinned to its companion Rust workspace; check `elegy --version`
  before invoking.
- SQLite is the only durable backend in scope. There is no
  PostgreSQL or remote-database path; the host-local SQLite file is
  the source of truth.
- Semver rule: minor must be ≥ the version that introduced the
  capability (e.g. `planning-project-run-claim` is only present in
  versions that ship the project-run feature). Patch is unconstrained.

## Examples

### Example 1 — create a goal and a roadmap

```text
elegy-planning --scope repo:elegy --json --non-interactive \
  --correlation-id $(uuidgen) \
  goal create \
  --id skill-rename-v1 \
  --title "Rename skill-definition-v2 to skill across the repo" \
  --description "Drop the v2 suffix in filenames, manifests, and prose." \
  --acceptance "All fixtures renamed to skill.<surface>.json" \
  --acceptance "cargo test --workspace passes" \
  --rejection "v2 suffix reintroduced in any new file" \
  --tag migration --tag skills
```

Expected: `status: "ok"`, `data.goal.id = "skill-rename-v1"`,
`correlationId` echoes the input.

### Example 2 — add a work point with file scopes

```text
elegy-planning --scope repo:elegy --json --non-interactive \
  --correlation-id $(uuidgen) \
  roadmap add-work-point \
  --roadmap-id skill-rename-roadmap \
  --work-point-id update-fixtures \
  --effort-tier balanced \
  --file-scope glob:primary:plugins/planning/fixtures/skill.*.json
```

Expected: `status: "ok"`, `data.workPoint.fileScopes` lists the
selector in declaration order.

### Example 3 — clear file scopes on a plan

```text
elegy-planning --scope repo:elegy --json --non-interactive \
  --correlation-id $(uuidgen) \
  plan revise \
  --plan-id update-fixtures \
  --clear-file-scopes
```

Expected: `status: "ok"`, `data.plan.fileScopes = []`. Re-running
`plan show` should confirm the empty list.

### Example 4 — revise work-point dependencies

```text
elegy-planning --scope repo:elegy --json --non-interactive \
  --correlation-id $(uuidgen) \
  work-point revise \
  --work-point-id wp-b \
  --clear-dependencies
```

Expected: `status: "ok"`, `data.record.dependencyIds = []`.

### Example 5 — validate specific scope

```text
elegy-planning --scope repo:elegy --json --non-interactive \
  --correlation-id $(uuidgen) \
  validate all
```

Expected: `status: "ok"`, `data.scopeMode = "single"`, `data.scopeKey = "repo:elegy"`, findings include only `repo:elegy` entities.

### Example 6 — global validation audit

```text
elegy-planning --json --non-interactive \
  --correlation-id $(uuidgen) \
  validate all --all-scopes
```

Expected: `status: "ok"`, `data.scopeMode = "all"`, `data.scopeKey = "all"`, findings span all known scopes.

## Boundaries

- This skill owns: durable planning records (goals, roadmaps, plans,
  work points, todos, issues, review points, insights, project runs)
  and their SQLite storage.
- This skill does not own: vault operations, repo operations, agent
  host projection, or MCP tool registration. Those live in their
  own skills.
- This skill does not own: planning state on other systems. Even when
  another system mirrors planning state, the SQLite file under the
  active scope is authority.
- Companion skills:
  - `elegy-memory` — for facts, preferences, and procedural
    memories that span planning sessions.
  - `elegy-obsidian` — for vault-side mirrors; planning is the
    authority and Obsidian is the read/write target.
  - `elegy-skills` — for registry operations; planning does not
    register skills.
  - `elegy-skill-authoring` — for SKILL.md audit and review.

## References

- Governed source: `plugins/planning/fixtures/skill.elegy-planning.json`.
- Discovery projection:
  `plugins/planning/fixtures/skill-discovery-index.elegy-planning.json`.
- Architecture: `plugins/planning/docs/architecture/v1.md`.
- Spec: `plugins/planning/docs/specs/index.md`.
- Result envelope schema:
  `plugins/planning/schemas/planning-result.schema.json`.
- Companion: `elegy-doc-practices` for cross-repo documentation
  doctrine when planning work touches ADRs or specs.
