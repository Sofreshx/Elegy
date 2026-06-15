# Implementation Plan: Agent-Safe Deterministic Planning Core

## Summary

This plan implements a deterministic, agent-safe planning core for elegy-planning across ten phases. The work adds explicit work-point categorization (kind, priority, repair/supersede/block linkages), enforced lifecycle state transitions with override capability, a scope gate for machine-mode mutations, ranked next-runnable querying, downstream blocking awareness, extended session context with project-run state tracking, tightened validation with preflight enforcement, expanded CLI arguments, comprehensive integration tests, and documentation updates. Each phase is self-contained with clear file scopes, dependencies, and risk notes. The schema changes are additive and backward-compatible with v7 databases.

## Phases

### Phase 1: Model & Schema Foundation

**Files:** `src/model.rs`, `src/storage.rs`
**Dependencies:** None
**Risks:** Schema migration must be backward-compatible with v7 databases

#### Step 1.1: Add WorkPointKind enum
- Add `WorkPointKind` enum to `src/model.rs` with variants: `Feature`, `Corrective`, `ReviewFix`, `ValidationFix`, `FollowUp`.
- Derive `Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema`.
- Implement `Display` trait with kebab-case output (e.g., `"review-fix"` for `ReviewFix`).

#### Step 1.2: Extend WorkPointRecord
- Add the following fields to `WorkPointRecord`:
  - `kind: WorkPointKind`
  - `priority: Priority`
  - `repairs_work_point_ids: Vec<String>`
  - `supersedes_work_point_ids: Vec<String>`
  - `blocks_work_point_ids: Vec<String>`

#### Step 1.3: Bump schema version
- Update schema version constant from `"7"` to `"8"`.

#### Step 1.4: Add migration logic
- Add `ALTER TABLE work_points ADD COLUMN kind TEXT NOT NULL DEFAULT 'Feature'`
- Add `ALTER TABLE work_points ADD COLUMN priority TEXT NOT NULL DEFAULT 'Medium'`
- Add `ALTER TABLE work_points ADD COLUMN repairs_work_point_ids TEXT NOT NULL DEFAULT '[]'`
- Add `ALTER TABLE work_points ADD COLUMN supersedes_work_point_ids TEXT NOT NULL DEFAULT '[]'`
- Add `ALTER TABLE work_points ADD COLUMN blocks_work_point_ids TEXT NOT NULL DEFAULT '[]'`

#### Step 1.5: Update create_work_point
- Accept the new fields and store them in the INSERT statement.

#### Step 1.6: Update row_to_work_point deserialization
- Read the new columns from SQLite result rows and populate `WorkPointRecord` fields.

#### Step 1.7: Update the three work_point SELECT queries
- `load_work_point()` (storage.rs ~4695): add `kind, priority, repairs_work_point_ids, supersedes_work_point_ids, blocks_work_point_ids` to the SELECT column list.
- `list_work_points_for_roadmap()` (storage.rs ~4776): add the 5 new columns.
- `list_work_points_for_roadmap_in_scope()` (storage.rs ~4795): add the 5 new columns.

These queries use `row.get(0..14)` positional indexing. After adding columns, indices shift — update `row_to_work_point()` to use the new indices (the new columns will be at positions 15-19 after all existing columns).

#### Step 1.8: Update `ensure_schema_version()` migration chain
a. Add arm `Some("7") => migrate_v7_to_v8(connection),` to the version match statement.
b. Fix `migrate_v6_to_v7` to write the literal string `"7"` instead of `CURRENT_SCHEMA_VERSION` in its `INSERT OR REPLACE INTO planning_config` statement. After `CURRENT_SCHEMA_VERSION` changes to `"8"`, past migrations must not be affected.
c. Add `fn migrate_v7_to_v8(connection: &Connection) -> Result<(), PlanningStoreError>` that executes the ALTER TABLE statements from Step 1.4 and writes schema version `"8"`.

#### Step 1.9: Update add-work-point args
- Update `roadmap_add_work_point_args` in storage to pass defaults or provided values for the new fields.

### Phase 2: Status Transition Enforcement

**Files:** `src/storage.rs`, `src/cli.rs`, `src/model.rs`
**Dependencies:** Phase 1 (schema migration needed for new entity types)
**Risks:** Must not break existing ProjectRun transition logic at lines 2586-2755

#### Step 2.1: Define transition tables
- Define transition tables as `const` arrays or `HashMap<&str, Vec<&str>>` per entity type: Goal, Roadmap, WorkPoint, Plan, Todo, Issue, ReviewPoint, Insight.
- Implement `fn allowed_transitions(entity_type: EntityType, current: &str) -> Vec<&str>`.

#### Step 2.2: Gate update_status
- In `update_status()` (storage.rs ~1849), after parsing entity type and requested status, call `allowed_transitions()`.
- If the requested transition is not in the allowed list and `--override-transition` is not set, return `PlanningStoreError::InvalidStatusTransition` with entity type, ID, current status, requested status, and allowed list.

#### Step 2.3: Add override flags to CLI
- Add `--override-transition` and `--reason <text>` to all `*-update-status` CLI subcommands in `cli.rs`.
- Pass them through to service.rs and storage.rs.

#### Step 2.4: Record override events
- When `--override-transition` is supplied with `--reason`, accept the transition.
- Write a row to `planning_events` with `event_type = "<entity>.status-overridden"` (distinct from `".status-updated"`).
- Store the reason in `payload_json` as `{"reason": "<text>"}`.

#### Step 2.5: Add override validation finding
- In `validation.rs`, add `STATUS-TRANSITION-OVERRIDDEN` finding at Warning severity.
- Emit when an event with `event_type` ending in `.status-overridden` is detected for the entity.

#### Step 2.6: Preserve existing ProjectRun logic
- Do NOT modify the existing ProjectRun transition gate checks at storage.rs:2586-2755.

### Phase 3: Explicit Scope Gate

**Files:** `src/cli.rs`, `src/storage.rs` or `src/service.rs`
**Dependencies:** None (can be done in parallel with Phase 1)
**Risks:** Must not break read-only commands or session-linked commands

#### Step 3.1: Detect machine-mode mutations
- In `cli.rs`, after parsing global flags: detect if `--json` AND `--non-interactive` are both set AND the command is a mutation (not list/show/search/context/validate/health/tags/events/work-graph/next-runnable).

#### Step 3.2: Require scope
- If mutation and scope is unset or equals `"default"` and no active session provides a scope: return an error with `status = "invalid"`, `code = "SCOPE_REQUIRED"`, and the specified message.

#### Step 3.3: Exempt session init
- Exempt `session init` from the scope gate check.

#### Step 3.4: Fail fast
- Add the check before any DB interaction, in the CLI dispatch layer.

### Phase 4: next-runnable Ranking

**Files:** `src/storage.rs`, `src/model.rs`
**Dependencies:** Phase 1 (needs WorkPointKind, Priority)
Note: Phase 4 does NOT depend on Phase 5. The Tier 3 ranking uses repairs/supersedes references checked against the issues and review_points tables, not against blocks_work_point_ids. The blocked-candidate exclusion (Phase 5) is applied separately after ranking.
**Risks:** Performance — ranking logic adds queries per candidate

#### Step 4.1: Extend RunnableWorkPointCandidate
- Add `required_reason: Option<String>` field to `RunnableWorkPointCandidate` in model.rs.

#### Step 4.2: Implement tiered ranking
- Modify `find_runnable_work_points()` in storage.rs (~3082).
- After the existing filtering pass, build candidate list:
  a. Pre-load optimization: before iterating candidates, bulk-load all open issues with severity >= High and all open review_points with severity >= High for the entire scope into HashMaps keyed by entity_id. Use these for O(1) lookups during Tier 3 ranking instead of per-candidate SQLite queries.
  b. Rank candidates by:
    - **Tier 1:** Candidates with an active project-run lease for the current session → `required_reason = "active_lease"`
    - **Tier 2:** Candidates with `kind in (Corrective, ReviewFix, ValidationFix)` AND `priority in (Urgent, High)` → `required_reason = "urgent_fix"`
    - **Tier 3:** Candidates whose `repairs_work_point_ids` or `supersedes_work_point_ids` reference work points with open high/critical issues or review points → `required_reason = "resolves_blocker"`
    - **Tier 4:** Remaining candidates → `required_reason = "ready"`, sorted by `ordering ASC, id ASC`.

#### Step 4.3: Accept optional session_id
- Accept an optional `session_id` parameter. If None, skip Tier 1 (no active-lease boost).

### Phase 5: Downstream Blocking

**Files:** `src/model.rs`, `src/storage.rs`, `src/validation.rs`
**Dependencies:** Phase 1
**Risks:** Cycle detection may need DFS; transitive blocking could be expensive on large graphs

#### Step 5.1: Add BlockedCandidate struct
- Add `BlockedCandidate` struct and `blocked: Vec<BlockedCandidate>` field to `RunnableCandidates` in model.rs:
  ```rust
  pub struct BlockedCandidate {
      pub work_point_id: String,
      pub work_point_title: String,
      pub blocker_id: String,
      pub blocker_title: String,
      pub reason: String,  // e.g., "blocked_by:wp-1"
  }
  ```
  And add to `RunnableCandidates`:
  ```rust
  pub blocked: Vec<BlockedCandidate>,
  ```

#### Step 5.2: Blocked candidate exclusion
- In `find_runnable_work_points()`, after the initial candidate pass:
  - Collect all `blocks_work_point_ids` from active corrective work points (status not Completed/Cancelled/Invalidated, kind != Feature, blocks_work_point_ids non-empty).
  - For each candidate, if its ID appears in any blocker's `blocks_work_point_ids`, exclude it from candidates and add a `BlockedCandidate` to `RunnableCandidates.blocked` with `reason = "blocked_by:<blocker-id>"` and the blocker's title.

#### Step 5.3: Cycle detection on block graph
- Add cycle detection to `roadmap add-work-point` and `work-point revise` preflight:
  - Before inserting/updating, build a directed graph of all `blocks_work_point_ids` relationships within the scope.
  - If adding/updating the current work point's `blocks_work_point_ids` would create a cycle (DFS from each blocked ID checking if it reaches back to the blocker), reject with `INVALID_BLOCK_CYCLE` error.

#### Step 5.4: Add validation finding
- In `validation.rs`, add `WORK-POINT-BLOCKED-DOWNSTREAM-ACTIVE` finding at Error severity.
- Find work points with status Active/Proposed whose IDs appear in another active corrective work point's `blocks_work_point_ids`.

### Phase 6: Session Context & Project-Run State

**Files:** `src/model.rs`, `src/storage.rs`, `src/session.rs`, `src/service.rs`, `src/cli.rs`
**Dependencies:** Phase 1 (for WorkPointRecord fields), Phase 4 (for next-runnable awareness)
**Risks:** Session file writes must be atomic; corruption on crash must not break session

#### Step 6.1: Extend SessionContextBundle
- Add the 8 new fields from specification section R6 to `SessionContextBundle` in model.rs.

#### Step 6.2: Add ActiveProjectRunState
- Add `ActiveProjectRunState` struct to session.rs with fields: `projectRunId`, `goalId`, `roadmapId`, `workPointId`, `status`, `claimedAt`, `activatedAt`, `evidenceRefs`.

#### Step 6.3: Extend SessionState
- Add to `SessionState` in session.rs:
  - `active_project_run: Option<ActiveProjectRunState>`
  - `last_active_work_point_id: Option<String>`
  - `last_completed_work_point_id: Option<String>`

#### Step 6.4: Rewrite session_context
- Rewrite `session_context()` in storage.rs to populate the new fields:
  - Query project_runs for the session's scope with status Claimed/Active/Interrupted.
  - Query work_points with status Active.
  - Query plans with status Active.
  - Query pending todos ordered by ordering_index, limit 10.
  - Query issues with severity High/Critical and status Open, limit 10.
  - Query review_points with severity High/Critical and status Open, limit 10.
  - Compute `recommended_next_action` using the R6 rules.
  - Build `context_warnings` using the R6 rules.

#### Step 6.5: Update project-run claim
- After successful claim: write `active_project_run` to the session file via `SessionState::set_active_project_run()`. Use atomic write pattern: serialize to `planning-session.json.tmp`, then `fs::rename` to `planning-session.json`. This prevents corruption on crash mid-write.

#### Step 6.6: Update project-run activate/release
- Update `project-run activate`, `add-evidence`, `release` similarly to persist state to session file.

#### Step 6.7: Merge session state in service.rs
- In service.rs, load session state before context commands and merge with DB state.

### Phase 7: Validation Tightening

**Files:** `src/validation.rs`, `src/storage.rs`, `src/service.rs`
**Dependencies:** Phase 1, Phase 2, Phase 5
**Risks:** Preflight rejections may break existing workflows that rely on advisory-only validation

#### Step 7.1: Move findings to preflight
- Move the 13 findings listed in specification section R8.1 from advisory checks to preflight checks.
- Checks must fire BEFORE the DB write in service/storage methods, returning `"status": "invalid"` instead of allowing the write to proceed.
- Preflight checks should go into service.rs (or the dispatch layer in storage.rs) before calling the inner write method.
- Use `ensure_*` functions (e.g., `ensure_work_point_dependency_cross_roadmap()`).

#### Step 7.2: Keep remaining findings as advisory
- Keep the remaining 19 findings as advisory validation in validation.rs.

#### Step 7.3: Add new validation findings
- Add the 6 new validation findings from R8.2:
  - `WORK-POINT-CORRECTIVE-NO-TARGET` — check work points with kind != Feature and empty repairs/supersedes/blocks arrays.
  - `WORK-POINT-BLOCKED-DOWNSTREAM-ACTIVE` — described in Phase 5.
  - `PROJECT-RUN-ON-COMPLETED-CANCELLED-WORK` — check project_runs with active status but target work point is Completed/Cancelled/Invalidated.
  - `SESSION-NO-ACTIVE-PLAN-CONTEXT` — check if session has active run but no active plans.
  - `GOAL-INVALIDATED-WITH-ACTIVE-WORK` — check goals with Invalidated/Abandoned status but linked work points or plans are active.
  - `CROSS-SCOPE-REFERENCE` — check any entity reference (parent, dependency, repair, supersede, block) points to a different scope.

### Phase 8: CLI Arguments & Help

**Files:** `src/cli.rs`
**Dependencies:** All previous phases
**Risks:** Flag collisions with existing arguments

#### Step 8.1: Add kind and priority flags
- Add `--kind <WorkPointKind>` and `--priority <Priority>` to `roadmap add-work-point` args.

#### Step 8.2: Add reference flags
- Add `--repairs-work-point-id <id>` (repeatable), `--supersedes-work-point-id <id>` (repeatable), `--blocks-work-point-id <id>` (repeatable) to `roadmap add-work-point` and `work-point revise` args.

#### Step 8.3: Add override flags to update-status commands
- Add `--override-transition` and `--reason <text>` to all `*-update-status` subcommands (goal, roadmap, work-point, plan, todo, issue, review-point, insight).

#### Step 8.4: Add clear flags
- Add `--clear-repairs`, `--clear-supersedes`, `--clear-blocks` to `work-point revise`.

#### Step 8.5: Update help text
- Update help text descriptions for all new flags.

### Phase 9: Integration Tests

**Files:** `tests/integration.rs` (new file — create alongside existing `tests/machine_posture.rs`)
**Dependencies:** All phases
**Risks:** Tests must create and destroy temp databases

Create a new file `tests/integration.rs` alongside the existing `tests/machine_posture.rs`. Add `#[cfg(test)] mod integration { ... }` containing all 12 test functions:

#### Step 9.1: Corrective work metadata test
- `test_corrective_work_metadata` — create work point with kind/priority/repairs/supersedes/blocks, read back, verify all fields.

#### Step 9.2: Status transition rejection test
- `test_status_transition_rejected` — try Draft→Completed on a work point, verify invalid status.

#### Step 9.3: Override transition test
- `test_override_transition` — use `--override-transition --reason "test"` to force an invalid transition, verify event recorded.

#### Step 9.4: Downstream blocking test
- `test_downstream_blocking` — create blocker WP with blocks_work_point_ids, verify blocked WP excluded from next-runnable.

#### Step 9.5: Active lease ranking test
- `test_active_lease_ranks_first` — claim a WP with project-run, verify it sorts first in next-runnable.

#### Step 9.6: Session context extended test
- `test_session_context_extended` — create session, claim run, verify context `--session` returns all new fields.

#### Step 9.7: Scope gate test
- `test_scope_required_machine_mode` — run mutation without `--scope` in json+non-interactive mode, verify SCOPE_REQUIRED error.

#### Step 9.8: Preflight rejection test
- `test_preflight_rejection` — try to create a plan with non-existent work point, verify invalid status (not just validation warning).

#### Step 9.9: Migration safety test
- `test_migration_safe_defaults` — start with v7 database, run migration, verify kind=Feature, priority=Medium, empty arrays.

#### Step 9.10: New validation findings test
- `test_new_validation_findings` — create scenarios that trigger each new finding, verify they appear in validate output.

#### Step 9.11: CLI help test
- `test_cli_help_new_args` — parse help output for new flag names.

#### Step 9.12: Project-run session state test
- `test_project_run_session_state` — claim/activate/release, verify session file reflects each state.

All tests use `--format json` and parse JSON output. Use tempfile for database isolation.

### Phase 10: Documentation Update

**Files:** `docs/specs/elegy-planning.md`
**Dependencies:** All phases complete
**Risks:** Low

#### Step 10.1: Update Design Risks section
- In `docs/specs/elegy-planning.md`, update "Design Risks and Issues" section item 3 to note that lifecycle transition enforcement is now delivered.

#### Step 10.2: Update Entity Lifecycles section
- Update the "Entity Lifecycles" section to note that transitions are enforced.

#### Step 10.3: Bump frontmatter date
- Update the `updated` frontmatter date to the current date.

#### Step 10.4: Add spec reference
- Add a reference to `docs/specs/agent-safe-planning-core/spec.md` in the Links section.

## Execution Order

```
Phase 1 (Model/Schema) ──┬── Phase 2 (Transitions) ── Phase 8 (CLI Args)
                         │
                         ├── Phase 4 (Ranking)
                         │
                         ├── Phase 5 (Blocking) ───── Phase 7 (Validation)
                         │
                         └── Phase 6 (Session Context)

Phase 3 (Scope Gate) — independent, can run in parallel with Phase 1

Phase 9 (Tests) — after Phase 1-8
Phase 10 (Docs) — after Phase 9
```

## Validation

After all phases:
```bash
cargo test -p elegy-planning
cargo build -p elegy-planning
cargo clippy -p elegy-planning
cargo fmt --check -p elegy-planning
```

## Rollback

If migration v8 causes issues:
1. **Code:** Revert `CURRENT_SCHEMA_VERSION` to "7", remove the `Some("7")` arm from `ensure_schema_version`, remove `migrate_v7_to_v8`, and revert the 5 new columns from all SELECT/INSERT statements and `row_to_work_point`.
2. **Database:** Set `planning_config.schema_version` to "7" and run `ALTER TABLE work_points DROP COLUMN kind;` (repeat for priority, repairs_work_point_ids, supersedes_work_point_ids, blocks_work_point_ids). SQLite 3.35+ supports DROP COLUMN.
3. **Session file:** Remove `active_project_run`, `last_active_work_point_id`, `last_completed_work_point_id` fields from session JSON — these are non-DB state that must be manually cleaned or ignored by older code.
4. **No data loss:** All existing v7 data is preserved; the new columns only carry default values until explicitly set.
