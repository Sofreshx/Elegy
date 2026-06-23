---
spec_id: agent-safe-planning-core
title: Agent-Safe Deterministic Planning Core
status: active
owner: Elegy
type: feature
updated: 2026-06-13
---

# Agent-Safe Deterministic Planning Core

## Intent

Enhance `elegy-planning` so agents make fewer consistency mistakes by default. Move key workflow rules from "skill guidance + advisory validation" into deterministic CLI/storage behavior: status transition enforcement, corrective work metadata, downstream blocking, explicit scope requirements in machine mode, richer session context, and validation tightening. Preserve explicit escape hatches for exceptional human/agent recovery.

## Context Evidence

- `rust/features/elegy-planning/src/model.rs:53-184` — Status enums for all entity types. No transition rules enforced; any status can be set from any other.
- `rust/features/elegy-planning/src/storage.rs:1849-2159` — `update_status()` method. Parses status string into enum and writes to DB. No transition validation exists for Goal, Roadmap, WorkPoint, Plan, Todo, Issue, ReviewPoint, or Insight. Only ProjectRun has explicit gate checks at lines 2586-2755.
- `rust/features/elegy-planning/src/storage.rs:3082-3169` — `find_runnable_work_points()`. Ranks only by `ordering_index ASC, id ASC`. No priority/urgency/lease-rank awareness.
- `rust/features/elegy-planning/src/storage.rs:1628-1704` — `session_context()`. Returns only `entitiesTouched`, `insightsRecorded`, `validationSummary`, `tokenEstimate`. No active project runs, work points, plans, pending todos, open blockers, or recommended next action.
- `rust/features/elegy-planning/src/storage.rs:4310-4347` — Scope enforcement. Mutations currently allow silent `default` scope in `--json --non-interactive` mode.
- `rust/features/elegy-planning/src/validation.rs:1-737` — 28 validation findings. All advisory-only (warn/error severity, none block writes). No findings for corrective work without targets, blocked downstream work, or session/lease consistency.
- `rust/features/elegy-planning/src/model.rs:338-355` — WorkPointRecord has `dependency_ids`, `validation_expectations`, `effort_tier`, `file_scopes`, `tags`. No `kind`, `priority`, `repairsWorkPointIds`, `supersedesWorkPointIds`, or `blocksWorkPointIds`.
- `rust/features/elegy-planning/src/model.rs:503-511` — `SessionContextBundle` definition. No fields for active runs, work points, plans, pending todos, or blockers.
- `rust/features/elegy-planning/src/session.rs:1-125` — Session file model. Tracks session ID, scope, timestamps. No active work metadata.
- `../index.md:469-474` — Current spec explicitly defers lifecycle transition enforcement to v1. This spec delivers that deferred feature.
- `../index.md` (Critical Analysis, item 3) — "No lifecycle transition enforcement…this is the biggest practical risk. Without transition rules, update-status is semantically equivalent to setting a free-text tag."
- `../index.md` (Critical Analysis, item 2) — "Advisory-first validation is the right call for an LLM-operated tool." This spec promotes 12 structural-integrity findings to preflight rejection. An ADR (`docs/adr/planning-preflight-boundary.md`) should document the promotion principle and agent UX tradeoffs. See `Drift Notes`.

## Requirements

### R1: Corrective Work Metadata on Work Points

Add four new fields to `WorkPointRecord`:

| Field | Type | Purpose |
|---|---|---|
| `kind` | `WorkPointKind` enum: `Feature \| Corrective \| ReviewFix \| ValidationFix \| FollowUp` | Classifies the nature of the work |
| `priority` | `Priority` enum: `Urgent \| High \| Medium \| Low` | Drives ranking in `next-runnable` |
| `repairs_work_point_ids` | `Vec<String>` | Work points this corrective work repairs |
| `supersedes_work_point_ids` | `Vec<String>` | Work points this work replaces |
| `blocks_work_point_ids` | `Vec<String>` | Work points blocked while this is active |

Storage schema: new columns in `work_points` table plus JSON arrays in the record.

CLI: extend `roadmap add-work-point` with optional `--kind`, `--priority`, `--repairs-work-point-id`, `--supersedes-work-point-id`, `--blocks-work-point-id` flags.

Default values for backward compatibility:
- `kind`: `Feature`
- `priority`: `Medium`
- `repairs_work_point_ids`: `[]`
- `supersedes_work_point_ids`: `[]`
- `blocks_work_point_ids`: `[]`

Existing databases must migrate with safe defaults.

### R2: Updated `next-runnable` Ranking

Change `find_runnable_work_points()` sort order from `ordering ASC, id ASC` to a ranked priority system:

1. **Active project-run lease first** — work points held by the current session's active project run always sort to the top.
2. **Corrective/blocker fixes** — `kind=Corrective\|ReviewFix\|ValidationFix` with `priority=Urgent` or `priority=High`.
3. **Issue/review point resolution** — work points that resolve open high/critical issues or review points.
4. **Normal candidates** — remaining candidates sorted by `ordering ASC, id ASC`.

`required_reason` field added to each `RunnableWorkPointCandidate` (the primary reason this candidate was selected at its current rank; the existing `reasons: Vec<String>` field remains as a detail/context list):
- `"active_lease"` — held by current session
- `"urgent_fix"` — urgent corrective/review/validation fix
- `"resolves_blocker"` — resolves an open high/critical issue or review point
- `"ready"` — normal candidate

### R3: Deterministic Downstream Blocking

Work points whose IDs appear in any active (non-completed, non-cancelled, non-released) corrective work point's `blocks_work_point_ids` are excluded from normal runnable candidates until the blocker completes.

Blocking rules:
- A work point with `kind != Feature` and `blocks_work_point_ids` non-empty creates a blocking relationship.
- The blocker must reach `Completed`, `Cancelled`, or `Invalidated` before blocked work points become runnable again.
- Blocked work points return a reason `"blocked_by:<blocker-id>"` in `next-runnable` output explaining why they are excluded.
- Cyclic blocking is rejected at preflight: if A blocks B and B blocks A directly or transitively, the `roadmap add-work-point` or `work-point revise` call is rejected.

### R4: Status Transition Enforcement

Enforce valid lifecycle transitions for all entity types. Invalid transitions are rejected at preflight with error `INVALID_STATUS_TRANSITION` and `{ entityType, entityId, currentStatus, requestedStatus, allowedTransitions }`.

#### Allowed Transition Tables

**Goal:**
| From | To |
|---|---|
| `Draft` | `Proposed`, `Abandoned` |
| `Proposed` | `Active`, `Abandoned` |
| `Active` | `Validated`, `Invalidated`, `Abandoned` |
| `Validated` | `Superseded` |
| `Invalidated` | `Active` (re-activation), `Abandoned` |
| `Superseded` | _(terminal)_ |
| `Abandoned` | `Draft` (re-draft) |

**Roadmap:**
| From | To |
|---|---|
| `Draft` | `Proposed`, `Cancelled` |
| `Proposed` | `Active`, `Cancelled` |
| `Active` | `Blocked`, `Completed`, `Cancelled` |
| `Blocked` | `Active`, `Cancelled` |
| `Completed` | _(terminal)_ |
| `Cancelled` | `Draft` (re-draft) |
| `Invalidated` | `Draft` (re-draft) |

**WorkPoint (matches Roadmap):**
| From | To |
|---|---|
| `Draft` | `Proposed`, `Cancelled` |
| `Proposed` | `Active`, `Cancelled` |
| `Active` | `Blocked`, `Completed`, `Cancelled` |
| `Blocked` | `Active`, `Cancelled` |
| `Completed` | _(terminal)_ |
| `Cancelled` | `Draft` (re-draft) |
| `Invalidated` | `Draft` (re-draft) |

**Plan:**
| From | To |
|---|---|
| `Draft` | `Proposed`, `Cancelled` |
| `Proposed` | `Active`, `Cancelled` |
| `Active` | `Blocked`, `Completed`, `Cancelled` |
| `Blocked` | `Active`, `Cancelled` |
| `Completed` | _(terminal)_ |
| `Cancelled` | `Draft` (re-draft) |
| `Invalidated` | `Draft` (re-draft) |

**Todo:**
| From | To |
|---|---|
| `Pending` | `InProgress`, `Cancelled` |
| `InProgress` | `Blocked`, `Completed`, `Cancelled` |
| `Blocked` | `Pending`, `InProgress`, `Cancelled` |
| `Completed` | _(terminal)_ |
| `Cancelled` | `Pending` (re-open) |

**Issue:**
| From | To |
|---|---|
| `Open` | `Blocked`, `Resolved` |
| `Blocked` | `Open`, `Resolved` |
| `Resolved` | `Reopened` |
| `Reopened` | `Open`, `Blocked`, `Resolved` |

**ReviewPoint:**
| From | To |
|---|---|
| `Open` | `Resolved`, `AcceptedRisk` |
| `Resolved` | _(terminal)_ |
| `AcceptedRisk` | _(terminal)_ |

**Insight:**
| From | To |
|---|---|
| `Active` | `Superseded`, `Archived` |
| `Superseded` | `Active` (re-activate), `Archived` |
| `Archived` | `Active` (un-archive) |

**ProjectRun** — existing transitions enforced at storage.rs:2586-2755 remain unchanged.

#### Design Rationale: Terminal Completed Status

`Completed` is a terminal status for Roadmap, WorkPoint, Plan, and Todo. There is no `→ Reopened` transition. When an agent prematurely completes an entity, the recovery path is to create a **corrective work point** (kind=`Corrective` or `ReviewFix`) that repairs or replaces the prematurely completed work, rather than reopening the completed entity. This design:
- Preserves the audit trail: completed → corrective work → new completion is clearer than completed → reopened → re-completed.
- Prevents status thrash (completed/uncompleted flip-flops) that degrades data quality.
- Forces explicit corrective intent rather than silent reversal.

#### Escape Hatch

`--override-transition --reason "<text>"` on `*-update-status` commands. When supplied:
- The transition is accepted despite being normally invalid.
- An event with `event_type = "<entity>.status-overridden"` is recorded (distinct from normal `.status-updated` events).
- The reason text is stored in the event's `payload_json`.
- Validation surfaces the override as a `STATUS-TRANSITION-OVERRIDDEN` advisory finding with severity `Warning`.

For ProjectRun, transition enforcement remains unchanged (the existing gate checks at storage.rs:2586-2755 are authoritative).

Without the override flag, invalid transitions are rejected at preflight with status `"invalid"`.

### R5: Explicit Scope Required for Machine-Mode Mutations

When both `--json` and `--non-interactive` are set on a mutation command, and no `--scope` is provided (or the scope value is the default `"default"`), the command must be rejected with:

```json
{
  "status": "invalid",
  "error": {
    "code": "SCOPE_REQUIRED",
    "message": "Explicit --scope is required in machine mode. No silent 'default' scope for mutations."
  }
}
```

Read-only commands (`list`, `show`, `search`, `context`, `validate`, `health`, `tags`, `events`, `work-graph`, `next-runnable`) continue to accept implicit `default` scope.

Commands that require explicit scope:
- `goal create`, `goal update-status`
- `roadmap create`, `roadmap add-section`, `roadmap add-work-point`, `roadmap update-status`
- `work-point update-status`, `work-point revise`
- `plan create`, `plan revise`, `plan update-status`
- `todo create`, `todo update-status`
- `issue record`, `issue update-status`
- `review-point record`, `review-point update-status`
- `insight record`, `insight update-status`
- `project-run claim`, `project-run activate`, `project-run release`, `project-run add-evidence`
- `scope create`

Session-initiated commands that have an active session with a known scope continue to work normally (the session provides the scope).

`session init` is exempt from the explicit-scope requirement because it establishes the scope identity for subsequent session-linked commands.

### R6: Extended `context --session` Output

Add the following fields to `SessionContextBundle` (model.rs line ~503):

```rust
pub struct SessionContextBundle {
    // existing fields preserved
    pub session_id: Option<String>,
    pub correlation_id: Option<String>,
    pub entities_touched: Vec<SearchResult>,
    pub insights_recorded: Vec<InsightRecord>,
    pub validation_summary: SessionValidationSummary,
    pub token_estimate: TokenEstimate,

    // new fields
    pub active_project_runs: Vec<ProjectRunRecord>,
    pub active_work_points: Vec<WorkPointRecord>,
    pub active_plans: Vec<PlanRecord>,
    pub next_pending_todos: Vec<TodoRecord>,
    pub open_blocking_issues: Vec<IssueRecord>,
    pub open_blocking_review_points: Vec<ReviewPointRecord>,
    pub recommended_next_action: Option<String>,
    pub context_warnings: Vec<String>,
}
```

**Active project runs** — all project runs claimed by the session that are in `Claimed|Active|Interrupted` status, with the run's linked work point, roadmap, and goal resolved.

**Active work points** — work points currently in `Active` status within the session's scope.

**Active plans** — plans currently in `Active` status within the session's scope.

**Next pending todos** — pending todos sorted by `ordering_index ASC, id ASC`, limited to 10.

**Open blocking issues** — open issues with `severity >= High` within the scope, sorted by severity descending, limited to 10.

**Open blocking review points** — open review points with `severity >= High` within the scope, sorted by severity descending, limited to 10.

**Recommended next action** — a computed string:
- If the session has an active project run and the count of todos in `Pending|InProgress|Blocked` status (not limited to 10) is >0 → `"continue <project-run-work-point-title>: <next-pending-todo-title>"`
- If the session has no active project run and runnable work points exist → `"claim <next-runnable-work-point-title>"`
- If there are open blockers → `"resolve <highest-severity-blocker-title>"`
- Otherwise → `"review open goals or create a new plan"`

**Context warnings** — strings surfaced when:
- Session has no active project run: `"No active project run. Use project-run claim to start work."`
- Session has no active plan: `"No active plan. Create a plan with plan create."`
- Open high/critical blockers exist: `"Blocked: <count> unresolved high/critical issue(s) and <count> open review point(s)."`

### R7: Project-Run Session-Linked State Updates

When `project-run claim/activate/release/add-evidence` is called, update session-linked state so that subsequent `context --session` calls can reconstruct what the agent is actively doing:

- **claim**: Records the claimed work point, roadmap, and goal IDs in the session file under `active_project_run`. Sets `last_active_work_point_id`.
- **activate**: Sets `active_project_run.status = "active"` and `active_project_run.activated_at` in the session file.
- **add-evidence**: Appends to `active_project_run.evidence_refs` in the session file. Also writes to the database project-run evidence (existing behavior).
- **release**: Clears `active_project_run` from the session file. Records `last_completed_work_point_id`.

The session file at `~/.elegy/planning-session.json` gains an `active_project_run` field:

```json
{
  "sessionId": "uuid",
  "scope": "repo:example",
  "createdAt": "2026-06-13T...",
  "activeProjectRun": {
    "projectRunId": "pr-1",
    "goalId": "g1",
    "roadmapId": "r1",
    "workPointId": "wp-1",
    "status": "active",
    "claimedAt": "2026-06-13T...",
    "activatedAt": "2026-06-13T...",
    "evidenceRefs": ["commit:abc123"]
  },
  "lastActiveWorkPointId": "wp-1",
  "lastCompletedWorkPointId": null
}
```

### R8: Validation Tightening

#### R8.1: Promote Consistency-Critical Findings to Preflight Rejection

The following validation checks move from advisory (stored in `validation_findings`) to preflight rejection (blocking the write with `"status": "invalid"`):

| Code | Condition | New Behavior |
|---|---|---|
| `WORK-POINT-DEPENDENCY-CROSS-ROADMAP` | Dependency belongs to a different roadmap | Preflight rejection (already partially enforced in `revise_work_point()`; fully enforced for all write paths including `add-work-point`) |
| `WORK-POINT-DEPENDENCY-MISSING` | Dependency does not exist | Preflight rejection (already partially enforced; tighten to always reject at write) |
| `WORK-POINT-SECTION-MISMATCH` | Section belongs to different roadmap | Preflight rejection |
| `WORK-POINT-SECTION-MISSING` | Section doesn't exist | Preflight rejection |
| `PLAN-GOAL-ROADMAP-MISMATCH` | Plan's goal != roadmap's goal | Preflight rejection |
| `PLAN-WORK-POINT-ROADMAP-MISMATCH` | Targeted WP belongs to different roadmap | Preflight rejection |
| `PLAN-WORK-POINT-MISSING` | Targeted WP doesn't exist | Preflight rejection |
| `ISSUE-RELATED-ENTITY-MISSING` | Related entity doesn't exist | Preflight rejection |
| `REVIEW-POINT-ATTACHED-ENTITY-MISSING` | Attached entity doesn't exist | Preflight rejection |
| `INSIGHT-EMPTY-CONTENT` | Content is empty | Preflight rejection |
| `INSIGHT-NO-PARENT` | Parent entity missing | Preflight rejection |
| `ROADMAP-GOAL-NOT-ACTIVE` | Goal is invalidated/superseded/abandoned | Preflight rejection |
| `CROSS-SCOPE-REFERENCE` | Cross-scope entity references | Preflight rejection |

**Promotion principle:** Findings that represent structural integrity violations (dangling references, cross-scope links, missing parents, impossible relationships) become preflight rejections because they indicate a write that would produce nonsensical data. Findings that represent planning soundness concerns (missing acceptance criteria, open blockers, completion contradictions) remain advisory because they reflect judgment calls that the operator may intentionally override.

These findings remain advisory (warning/error in validation but do not block writes):
- `GOAL-ACCEPTANCE-MISSING`, `GOAL-REJECTION-MISSING`, `GOAL-VALIDATED-WITHOUT-ROADMAP`
- `ROADMAP-NO-WORK-POINTS`, `ROADMAP-COMPLETED-WITH-OPEN-WORK`, `ROADMAP-SECTION-EMPTY`
- `WORK-POINT-NO-VALIDATION`, `WORK-POINT-DEPENDENCY-CYCLE`, `WORK-POINT-COMPLETED-WITH-OPEN-DEPENDENCY`
- `PLAN-NO-TARGETED-WORK`, `PLAN-NO-VALIDATION-STEPS`, `PLAN-NO-TODOS`, `PLAN-COMPLETED-WITH-OPEN-TODOS`, `PLAN-BLOCKING-ISSUES`, `PLAN-OPEN-REVIEW-POINTS`
- `TODO-STANDALONE`, `TODO-COMPLETED-WITHOUT-EVIDENCE`, `TODO-PLAN-WORK-POINT-MISMATCH`
- `ISSUE-PARTIAL-ENTITY-LINK`, `ISSUE-BLOCKED-LOW-SEVERITY`
- `REVIEW-POINT-CRITICAL-OPEN`
- `INSIGHT-TAG-ORPHAN`
- `PROJECT-RUN-COMPLETED-WITHOUT-EVIDENCE`, `PROJECT-RUN-WORK-POINT-INVALID`, `PROJECT-RUN-GOAL-NOT-ACTIVE`
- `STATUS-TRANSITION-OVERRIDDEN` (new — see R4 escape hatch)

#### R8.2: New Validation Findings

| Code | Severity | Condition |
|---|---|---|
| `WORK-POINT-CORRECTIVE-NO-TARGET` | Warning | `kind != Feature` but `repairsWorkPointIds`, `supersedesWorkPointIds`, and `blocksWorkPointIds` are all empty |
| `WORK-POINT-BLOCKED-DOWNSTREAM-ACTIVE` | Error | A work point is `Active` or `Proposed` but is blocked by an active corrective work point (listed in its blocker's `blocksWorkPointIds`) |
| `PROJECT-RUN-ON-COMPLETED-CANCELLED-WORK` | Error | An active/claimed project run is attached to a work point with status `Completed`, `Cancelled`, or `Invalidated` |
| `SESSION-NO-ACTIVE-PLAN-CONTEXT` | Warning | Session has an active project run but no active plan exists (may indicate the agent lost track of its plan) |
| `GOAL-INVALIDATED-WITH-ACTIVE-WORK` | Error | Goal is `Invalidated` or `Abandoned` but has active work points or plans |
| `CROSS-SCOPE-REFERENCE` | Error | An entity references another entity in a different scope (broader than existing parent-ref checks; covers related entity, dependency, repair, supersede, and block references) |

## Non-Goals

- Do not change the `--override-transition` escape hatch to require multi-party approval or host policy — it remains a simple flag for now.
- Do not add email/Slack/webhook notifications for blocked downstream work.
- Do not implement event replay or subscription/push APIs.
- Do not add batch scope-migration commands.
- Do not change the entity hierarchy (goal → roadmap → plan remains 1:1).
- Do not add multi-goal plans or cross-cutting work modeling.
- Do not change the FTS5 index behavior (update-side rebuild is separate follow-up).
- Do not add a web dashboard or UI.
- Do not change the `elegy` umbrella CLI beyond what `elegy-planning` exposes.
- Do not modify the MCP projection or JSON-RPC transport — spec-only concerns the CLI and storage layer.
- Do not add a `Reopened` transition from `Completed` for any entity type. Recovery from premature completion uses corrective work points (kind=`Corrective`), not status reversal.

## Acceptance Checks

- **AC1: Corrective work metadata round-trips**
  → verify: `cargo test -p elegy-planning --test integration -- corrective_work_metadata`

- **AC2: Invalid lifecycle transition rejected**
  → verify: `cargo test -p elegy-planning --test integration -- status_transition_rejected`

- **AC3: Override transition accepted with reason and event**
  → verify: `cargo test -p elegy-planning --test integration -- override_transition`

- **AC4: Corrective work blocks downstream next-runnable**
  → verify: `cargo test -p elegy-planning --test integration -- downstream_blocking`

- **AC5: Active project run ranks first in next-runnable**
  → verify: `cargo test -p elegy-planning --test integration -- active_lease_ranks_first`

- **AC6: Session context returns active runs, work points, plans, next todo, blockers, warnings**
  → verify: `cargo test -p elegy-planning --test integration -- session_context_extended`

- **AC7: Machine-mode mutation without explicit scope rejected**
  → verify: `cargo test -p elegy-planning --test integration -- scope_required_machine_mode`

- **AC8: Preflight-rejected findings block writes with invalid status**
  → verify: `cargo test -p elegy-planning --test integration -- preflight_rejection`

- **AC9: Existing databases migrate with safe defaults**
  → verify: `cargo test -p elegy-planning --test integration -- migration_safe_defaults`

- **AC10: Full validation pass reports new findings correctly**
  → verify: `cargo test -p elegy-planning --test integration -- new_validation_findings`

- **AC11: CLI help reflects new arguments**
  → verify: `cargo run -p elegy-planning -- --help` shows `--kind`, `--priority`, `--override-transition`, `--repairs-work-point-id`, `--supersedes-work-point-id`, `--blocks-work-point-id`

- **AC12: project-run claim/activate/release/add-evidence updates session file**
  → verify: `cargo test -p elegy-planning --test integration -- project_run_session_state`

## Implementation Links

- `rust/features/elegy-planning/src/model.rs` — Add `WorkPointKind` enum, new `WorkPointRecord` fields, new `SessionContextBundle` fields
- `rust/features/elegy-planning/src/storage.rs` — Schema migration v8, transition enforcement, `find_runnable_work_points()` ranking, session context builder, scope-explicit gate
- `rust/features/elegy-planning/src/validation.rs` — New validation findings, promotion of existing findings to preflight
- `rust/features/elegy-planning/src/cli.rs` — New CLI arguments (`--kind`, `--priority`, `--override-transition`, `--repairs-work-point-id`, `--supersedes-work-point-id`, `--blocks-work-point-id`)
- `rust/features/elegy-planning/src/service.rs` — Service-layer updates for new arguments and rejection flows
- `rust/features/elegy-planning/src/session.rs` — Extended session file model with `activeProjectRun`, `lastActiveWorkPointId`, `lastCompletedWorkPointId`
- `rust/features/elegy-planning/tests/` — Integration tests for all acceptance checks
- `../index.md` — Update to reflect delivered lifecycle enforcement (remove "deferred to v1" note)

## Validation Evidence

- `cargo test -p elegy-planning` — 44/44 tests passed (15 unit + 12 integration + 17 machine_posture)
- `cargo clippy -p elegy-planning -- -D warnings` — 0 warnings
- `cargo fmt --check -p elegy-planning` — 0 diffs
- AC1: `cargo test -p elegy-planning --test integration -- corrective_work_metadata` — PASSED
- AC2: `cargo test -p elegy-planning --test integration -- status_transition_rejected` — PASSED
- AC3: `cargo test -p elegy-planning --test integration -- override_transition` — PASSED
- AC4: `cargo test -p elegy-planning --test integration -- downstream_blocking` — PASSED
- AC5: `cargo test -p elegy-planning --test integration -- active_lease_ranks_first` — PASSED
- AC6: `cargo test -p elegy-planning --test integration -- session_context_extended` — PASSED
- AC7: `cargo test -p elegy-planning --test integration -- scope_required_machine_mode` — PASSED
- AC8: `cargo test -p elegy-planning --test integration -- preflight_rejection` — PASSED
- AC9: `cargo test -p elegy-planning --test integration -- migration_safe_defaults` — PASSED
- AC10: `cargo test -p elegy-planning --test integration -- new_validation_findings` — PASSED
- AC11: `cargo test -p elegy-planning --test integration -- cli_help_new_args` — PASSED
- AC12: `cargo test -p elegy-planning --test integration -- project_run_session_state` — PASSED
- Schema migration v7→v8 verified — existing databases migrate cleanly with safe defaults
- Phase 3 scope gate verified — machine-mode mutations without explicit scope rejected with `SCOPE_REQUIRED`

## Drift Notes

- ADR for preflight-promotion boundary (`docs/adr/planning-preflight-boundary.md`) is not yet written. The promotion principle (structural integrity → preflight, planning soundness → advisory) is documented in R8.1 of this spec.
- Spec pre-commit hook (`scripts/install-spec-hooks.mjs`) is not yet installed in the Elegy repo.
- Current test coverage for all 12 acceptance checks is via integration tests (`tests/integration.rs`). No unit-test-level coverage for individual transition tables or preflight rejection functions yet — this is acceptable for initial delivery but should be added before promoting to `approved`.
