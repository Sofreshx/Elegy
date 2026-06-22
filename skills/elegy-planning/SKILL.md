---
name: elegy-planning
description: Durable planning authority for goals, roadmaps, plans, todos, issues, review points, work-point graphs, project-run leases, session management, worktree registry, validation, and projection rendering.
version: "2.0"
---

# Elegy Planning Authority

Durable planning authority for goals, roadmaps, plans, todos, issues, review points, work-point graphs, project-run leases, session management, worktree registry, validation, and projection rendering.

## Capabilities

- `planning-goal-create`: Create a durable goal with explicit acceptance and rejection criteria.
- `planning-goal-show`: Show one durable goal including linked planning context and validation state.
- `planning-roadmap-create`: Create a roadmap linked to an existing goal.
- `planning-roadmap-add-work-point`: Attach a work point to a roadmap, optionally under one roadmap section.
- `planning-roadmap-show`: Show one roadmap with sections, work points, attached findings, and validation state.
- `planning-plan-create`: Create a plan linked to one goal and one roadmap.
- `planning-plan-show`: Show one plan with linked work points, todos, and validation state.
- `planning-todo-create`: Create a durable todo, optionally linked to a plan, a work point, or both.
- `planning-issue-record`: Record a durable planning issue, optionally attached to another planning entity.
- `planning-review-point-record`: Attach a review point to an existing planning entity.
- `planning-validate-all`: Run full deterministic planning validation over the current database.
- `planning-health`: Show planning database health and record counts.
- `planning-project-render`: Render a non-authoritative markdown or JSON projection for one planning entity.
- `planning-scope-list`: List known planning scopes.
- `planning-scope-create`: Create a scope record for scoped planning state.
- `planning-plan-revise`: Revise assumptions, stop conditions, validation steps, targeted work points, and tags for an existing plan.
- `planning-todo-update-status`: Update todo status with optional completion evidence references.
- `planning-scope-show`: Show one scope record.
- `planning-goal-list`: List goals in one scope.
- `planning-goal-update-status`: Transition goal lifecycle status.
- `planning-roadmap-list`: List roadmaps in one scope.
- `planning-roadmap-update-status`: Transition roadmap lifecycle status.
- `planning-work-point-list`: List work points in one scope.
- `planning-work-point-show`: Show one work point.
- `planning-work-point-update-status`: Transition work point lifecycle status.
- `planning-plan-list`: List plans in one scope.
- `planning-plan-update-status`: Transition plan lifecycle status.
