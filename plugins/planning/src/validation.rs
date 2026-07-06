use std::str::FromStr;

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    storage::{
        attached_entity_correlation_id, list_incoming_edges_in_scope, list_outgoing_edges_for_node,
        list_outgoing_edges_in_scope, list_review_points_for_entity, list_todos_for_plan,
        list_work_points_for_roadmap, load_goal, load_graph_edge, load_graph_node, load_insight,
        load_issue, load_plan, load_project_run, load_roadmap, load_todo, load_work_point,
        validate_edge_kind_pair, would_create_graph_cycle,
    },
    AcceptanceKind, EntityType, EvidenceKind, GoalStatus, IssueStatus, PlanningEdgeKind,
    PlanningNodeKind, PlanningStoreError, ProjectRunStatus, ReviewPointStatus, Severity,
    TodoStatus, ValidationFinding, ValidationSeverity, WorkPointKind, WorkPointStatus,
};

pub(crate) fn validate_entity(
    connection: &Connection,
    entity_type: EntityType,
    entity_id: &str,
) -> Result<Vec<ValidationFinding>, PlanningStoreError> {
    match entity_type {
        EntityType::Scope => Ok(Vec::new()),
        EntityType::Goal => validate_goal(connection, entity_id),
        EntityType::Roadmap => validate_roadmap(connection, entity_id),
        EntityType::RoadmapSection => validate_roadmap_section(connection, entity_id),
        EntityType::WorkPoint => validate_work_point(connection, entity_id),
        EntityType::Plan => validate_plan(connection, entity_id),
        EntityType::Todo => validate_todo(connection, entity_id),
        EntityType::Issue => validate_issue(connection, entity_id),
        EntityType::ReviewPoint => validate_review_point(connection, entity_id),
        EntityType::Insight => validate_insight(connection, entity_id),
        EntityType::ProjectRun => validate_project_run(connection, entity_id),
        EntityType::GraphNode => validate_graph_node(connection, entity_id),
        EntityType::GraphEdge => validate_graph_edge(connection, entity_id),
        EntityType::DiscoveryNode => Ok(Vec::new()),
        EntityType::DiscoveryRelationship => Ok(Vec::new()),
        EntityType::DiscoveryCheckpoint => Ok(Vec::new()),
    }
}

fn validate_project_run(
    connection: &Connection,
    project_run_id: &str,
) -> Result<Vec<ValidationFinding>, PlanningStoreError> {
    let run = load_project_run(connection, project_run_id)?;
    let mut findings = Vec::new();

    let goal = load_goal(connection, &run.goal_id)?;
    if matches!(
        goal.status,
        GoalStatus::Invalidated | GoalStatus::Superseded | GoalStatus::Abandoned
    ) {
        findings.push(warning(
            EntityType::ProjectRun,
            project_run_id,
            &run.scope_key,
            "PROJECT-RUN-GOAL-NOT-ACTIVE",
            "project run references a goal that is no longer active",
        )?);
    }

    let work_point = load_work_point(connection, &run.work_point_id)?;
    if matches!(
        work_point.status,
        WorkPointStatus::Cancelled | WorkPointStatus::Invalidated
    ) {
        findings.push(error(
            EntityType::ProjectRun,
            project_run_id,
            &run.scope_key,
            "PROJECT-RUN-WORK-POINT-INVALID",
            "project run references a work point that has been cancelled or invalidated",
        )?);
    }

    // PROJECT-RUN-ON-COMPLETED-CANCELLED-WORK: active project run on terminated work point
    if matches!(
        run.status,
        ProjectRunStatus::Claimed | ProjectRunStatus::Active | ProjectRunStatus::Interrupted
    ) && matches!(
        work_point.status,
        WorkPointStatus::Completed | WorkPointStatus::Cancelled | WorkPointStatus::Invalidated
    ) {
        findings.push(error(
            EntityType::ProjectRun,
            project_run_id,
            &run.scope_key,
            "PROJECT-RUN-ON-COMPLETED-CANCELLED-WORK",
            "active project run references a work point that has been completed, cancelled, or invalidated",
        )?);
    }

    if run.status == ProjectRunStatus::Completed {
        let has_evidence = !run.evidence.implementation_run_refs.is_empty()
            || !run.evidence.validation_finding_refs.is_empty()
            || !run.evidence.linked_spec_ids.is_empty()
            || run.evidence.commit_sha.is_some()
            || run.evidence.pr_url.is_some();
        if !has_evidence {
            findings.push(warning(
                EntityType::ProjectRun,
                project_run_id,
                &run.scope_key,
                "PROJECT-RUN-COMPLETED-WITHOUT-EVIDENCE",
                "project run is completed but has no evidence refs",
            )?);
        }
    }

    // CROSS-SCOPE-REFERENCE: verify referenced entities share the same scope
    if let Ok(goal) = load_goal(connection, &run.goal_id) {
        if goal.scope_key != run.scope_key {
            findings.push(error(
                EntityType::ProjectRun,
                project_run_id,
                &run.scope_key,
                "CROSS-SCOPE-REFERENCE",
                &format!(
                    "references goal `{}` in scope `{}` but project run is in scope `{}`",
                    run.goal_id, goal.scope_key, run.scope_key
                ),
            )?);
        }
    }
    if work_point.scope_key != run.scope_key {
        findings.push(error(
            EntityType::ProjectRun,
            project_run_id,
            &run.scope_key,
            "CROSS-SCOPE-REFERENCE",
            &format!(
                "references work point `{}` in scope `{}` but project run is in scope `{}`",
                run.work_point_id, work_point.scope_key, run.scope_key
            ),
        )?);
    }

    // PROJECT-RUN-GOAL-ROADMAP-MISMATCH
    if let Ok(roadmap) = load_roadmap(connection, &run.roadmap_id) {
        if roadmap.goal_id != run.goal_id {
            findings.push(error(
                EntityType::ProjectRun,
                project_run_id,
                &run.scope_key,
                "PROJECT-RUN-GOAL-ROADMAP-MISMATCH",
                &format!(
                    "roadmap '{}' belongs to goal '{}', but project run references goal '{}'",
                    run.roadmap_id, roadmap.goal_id, run.goal_id
                ),
            )?);
        }
    }

    // PROJECT-RUN-WORK-POINT-ROADMAP-MISMATCH
    if let Ok(wp) = load_work_point(connection, &run.work_point_id) {
        if wp.roadmap_id != run.roadmap_id {
            findings.push(error(
                EntityType::ProjectRun,
                project_run_id,
                &run.scope_key,
                "PROJECT-RUN-WORK-POINT-ROADMAP-MISMATCH",
                &format!(
                    "work point '{}' belongs to roadmap '{}', but project run references roadmap '{}'",
                    run.work_point_id, wp.roadmap_id, run.roadmap_id
                ),
            )?);
        }
    }

    Ok(findings)
}

fn validate_goal(
    connection: &Connection,
    goal_id: &str,
) -> Result<Vec<ValidationFinding>, PlanningStoreError> {
    let goal = load_goal(connection, goal_id)?;
    let mut findings = Vec::new();

    if goal.acceptance_criteria.is_empty() {
        findings.push(error(
            EntityType::Goal,
            goal_id,
            &goal.scope_key,
            "GOAL-ACCEPTANCE-MISSING",
            "goal should define at least one acceptance criterion",
        )?);
    }
    if goal.rejection_criteria.is_empty() {
        findings.push(error(
            EntityType::Goal,
            goal_id,
            &goal.scope_key,
            "GOAL-REJECTION-MISSING",
            "goal should define at least one rejection criterion",
        )?);
    }

    if goal.status == GoalStatus::Validated {
        let roadmap_count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM roadmaps WHERE goal_id = ?1",
            params![goal_id],
            |row| row.get(0),
        )?;
        if roadmap_count == 0 {
            findings.push(warning(
                EntityType::Goal,
                goal_id,
                &goal.scope_key,
                "GOAL-VALIDATED-WITHOUT-ROADMAP",
                "validated goal has no linked roadmaps yet",
            )?);
        }
    }

    // GOAL-INVALIDATED-WITH-ACTIVE-WORK: goal is invalidated/abandoned but has active work points or plans
    if matches!(goal.status, GoalStatus::Invalidated | GoalStatus::Abandoned) {
        let active_wp_count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM work_points wp JOIN roadmaps r ON wp.roadmap_id = r.id WHERE r.goal_id = ?1 AND wp.status NOT IN ('completed', 'cancelled', 'invalidated')",
            params![goal_id],
            |row| row.get(0),
        ).unwrap_or(0);
        let active_plan_count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM plans WHERE goal_id = ?1 AND status NOT IN ('completed', 'cancelled', 'invalidated')",
            params![goal_id],
            |row| row.get(0),
        ).unwrap_or(0);
        if active_wp_count > 0 || active_plan_count > 0 {
            findings.push(error(
                EntityType::Goal,
                goal_id,
                &goal.scope_key,
                "GOAL-INVALIDATED-WITH-ACTIVE-WORK",
                &format!(
                    "goal has been invalidated or abandoned but has {} active work point(s) and {} active plan(s)",
                    active_wp_count, active_plan_count
                ),
            )?);
        }
    }

    // CROSS-SCOPE-REFERENCE: verify referenced roadmaps share the same scope
    {
        let mut stmt = connection.prepare("SELECT scope_key FROM roadmaps WHERE goal_id = ?1")?;
        let rows = stmt.query_map(params![goal_id], |row| row.get::<_, String>(0))?;
        for r_scope in rows.flatten() {
            if r_scope != goal.scope_key {
                findings.push(error(
                    EntityType::Goal,
                    goal_id,
                    &goal.scope_key,
                    "CROSS-SCOPE-REFERENCE",
                    &format!(
                        "linked roadmap is in scope `{r_scope}` but goal is in scope `{}`",
                        goal.scope_key
                    ),
                )?);
            }
        }
    }

    Ok(findings)
}

fn validate_roadmap(
    connection: &Connection,
    roadmap_id: &str,
) -> Result<Vec<ValidationFinding>, PlanningStoreError> {
    let roadmap = load_roadmap(connection, roadmap_id)?;
    let goal = load_goal(connection, &roadmap.goal_id)?;
    let work_points = list_work_points_for_roadmap(connection, roadmap_id)?;
    let mut findings = Vec::new();

    if work_points.is_empty() {
        findings.push(warning(
            EntityType::Roadmap,
            roadmap_id,
            &roadmap.scope_key,
            "ROADMAP-NO-WORK-POINTS",
            "roadmap has no work points yet",
        )?);
    }

    if matches!(
        goal.status,
        GoalStatus::Invalidated | GoalStatus::Superseded | GoalStatus::Abandoned
    ) {
        findings.push(error(
            EntityType::Roadmap,
            roadmap_id,
            &roadmap.scope_key,
            "ROADMAP-GOAL-NOT-ACTIVE",
            "roadmap links to a goal that is invalidated, superseded, or abandoned",
        )?);
    }

    if roadmap.status.as_str() == "completed"
        && work_points
            .iter()
            .any(|work_point| work_point.status.as_str() != "completed")
    {
        findings.push(error(
            EntityType::Roadmap,
            roadmap_id,
            &roadmap.scope_key,
            "ROADMAP-COMPLETED-WITH-OPEN-WORK",
            "completed roadmap still has non-completed work points",
        )?);
    }

    Ok(findings)
}

fn validate_roadmap_section(
    connection: &Connection,
    section_id: &str,
) -> Result<Vec<ValidationFinding>, PlanningStoreError> {
    let section = connection
        .query_row(
            "SELECT roadmap_id, slug, scope_key FROM roadmap_sections WHERE id = ?1",
            params![section_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => PlanningStoreError::NotFound {
                entity_type: EntityType::RoadmapSection.as_str().to_string(),
                entity_id: section_id.to_string(),
            },
            other => PlanningStoreError::Sqlite(other),
        })?;

    let work_point_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM work_points WHERE section_id = ?1",
        params![section_id],
        |row| row.get(0),
    )?;

    let mut findings = Vec::new();
    if work_point_count == 0 {
        findings.push(warning(
            EntityType::RoadmapSection,
            section_id,
            &section.2,
            "ROADMAP-SECTION-EMPTY",
            &format!("roadmap section `{}` has no work points yet", section.1),
        )?);
    }
    let _ = load_roadmap(connection, &section.0)?;
    Ok(findings)
}

fn validate_work_point(
    connection: &Connection,
    work_point_id: &str,
) -> Result<Vec<ValidationFinding>, PlanningStoreError> {
    let work_point = load_work_point(connection, work_point_id)?;
    let mut findings = Vec::new();
    let _ = load_roadmap(connection, &work_point.roadmap_id)?;

    if let Some(section_id) = &work_point.section_id {
        let section_roadmap_id: Option<String> = connection
            .query_row(
                "SELECT roadmap_id FROM roadmap_sections WHERE id = ?1",
                params![section_id],
                |row| row.get(0),
            )
            .optional()?;
        match section_roadmap_id {
            Some(roadmap_id) if roadmap_id != work_point.roadmap_id => findings.push(error(
                EntityType::WorkPoint,
                work_point_id,
                &work_point.scope_key,
                "WORK-POINT-SECTION-MISMATCH",
                "work point section belongs to a different roadmap",
            )?),
            None => findings.push(error(
                EntityType::WorkPoint,
                work_point_id,
                &work_point.scope_key,
                "WORK-POINT-SECTION-MISSING",
                "work point references a missing roadmap section",
            )?),
            Some(_) => {}
        }
    }

    if work_point.validation_expectations.is_empty() {
        findings.push(warning(
            EntityType::WorkPoint,
            work_point_id,
            &work_point.scope_key,
            "WORK-POINT-NO-VALIDATION",
            "work point has no validation expectations yet",
        )?);
    }

    for dependency_id in &work_point.dependency_ids {
        let dependency = load_work_point(connection, dependency_id);
        match dependency {
            Ok(dependency) => {
                if dependency.roadmap_id != work_point.roadmap_id {
                    findings.push(error(
                        EntityType::WorkPoint,
                        work_point_id,
                        &work_point.scope_key,
                        "WORK-POINT-DEPENDENCY-CROSS-ROADMAP",
                        &format!("dependency `{dependency_id}` belongs to a different roadmap"),
                    )?);
                }
                if work_point.status.as_str() == "completed"
                    && dependency.status.as_str() != "completed"
                {
                    findings.push(error(
                        EntityType::WorkPoint,
                        work_point_id,
                        &work_point.scope_key,
                        "WORK-POINT-COMPLETED-WITH-OPEN-DEPENDENCY",
                        &format!(
                            "completed work point still depends on non-completed work point `{dependency_id}`"
                        ),
                    )?);
                }
            }
            Err(PlanningStoreError::NotFound { .. }) => findings.push(error(
                EntityType::WorkPoint,
                work_point_id,
                &work_point.scope_key,
                "WORK-POINT-DEPENDENCY-MISSING",
                &format!("dependency `{dependency_id}` does not exist"),
            )?),
            Err(error) => return Err(error),
        }
    }

    if !work_point.dependency_ids.is_empty() {
        let mut visited = std::collections::HashSet::new();
        let mut stack = work_point.dependency_ids.clone();
        while let Some(dep_id) = stack.pop() {
            if dep_id == work_point_id {
                findings.push(error(
                    EntityType::WorkPoint,
                    work_point_id,
                    &work_point.scope_key,
                    "WORK-POINT-DEPENDENCY-CYCLE",
                    "work point dependency graph contains a cycle",
                )?);
                break;
            }
            if !visited.insert(dep_id.clone()) {
                continue;
            }
            if let Ok(dep) = load_work_point(connection, &dep_id) {
                stack.extend(dep.dependency_ids.iter().cloned());
            }
        }
    }

    // WORK-POINT-CORRECTIVE-NO-TARGET: kind != Feature but no corrective targets
    if work_point.kind != WorkPointKind::Feature
        && work_point.repairs_work_point_ids.is_empty()
        && work_point.supersedes_work_point_ids.is_empty()
        && work_point.blocks_work_point_ids.is_empty()
    {
        findings.push(warning(
            EntityType::WorkPoint,
            work_point_id,
            &work_point.scope_key,
            "WORK-POINT-CORRECTIVE-NO-TARGET",
            "corrective work point (non-feature) has no repairs, supersedes, or blocks targets",
        )?);
    }

    // WORK-POINT-BLOCKED-DOWNSTREAM-ACTIVE: this WP is blocked by another active corrective WP
    {
        let mut stmt = connection.prepare(
            "SELECT id FROM work_points WHERE scope_key = ?1 AND kind != 'feature' AND status NOT IN ('completed', 'cancelled', 'invalidated')",
        )?;
        let rows = stmt.query_map(params![work_point.scope_key], |row| row.get::<_, String>(0))?;
        for wp_id in rows.flatten() {
            if wp_id == work_point_id {
                continue;
            }
            if let Ok(other_wp) = load_work_point(connection, &wp_id) {
                if other_wp
                    .blocks_work_point_ids
                    .contains(&work_point_id.to_string())
                {
                    findings.push(error(
                        EntityType::WorkPoint,
                        work_point_id,
                        &work_point.scope_key,
                        "WORK-POINT-BLOCKED-DOWNSTREAM-ACTIVE",
                        &format!(
                            "work point is blocked by active corrective work point `{}`",
                            other_wp.title
                        ),
                    )?);
                }
            }
        }
    }

    // CROSS-SCOPE-REFERENCE: verify referenced entities share the same scope
    for dep_id in &work_point.dependency_ids {
        if let Ok(dep) = load_work_point(connection, dep_id) {
            if dep.scope_key != work_point.scope_key {
                findings.push(error(
                    EntityType::WorkPoint,
                    work_point_id,
                    &work_point.scope_key,
                    "CROSS-SCOPE-REFERENCE",
                    &format!(
                        "dependency `{dep_id}` is in scope `{}` but work point is in scope `{}`",
                        dep.scope_key, work_point.scope_key
                    ),
                )?);
            }
        }
    }
    for repair_id in &work_point.repairs_work_point_ids {
        if let Ok(repair) = load_work_point(connection, repair_id) {
            if repair.scope_key != work_point.scope_key {
                findings.push(error(
                    EntityType::WorkPoint,
                    work_point_id,
                    &work_point.scope_key,
                    "CROSS-SCOPE-REFERENCE",
                    &format!("repairs target `{repair_id}` is in scope `{}` but work point is in scope `{}`", repair.scope_key, work_point.scope_key),
                )?);
            }
        }
    }
    for supersede_id in &work_point.supersedes_work_point_ids {
        if let Ok(supersede) = load_work_point(connection, supersede_id) {
            if supersede.scope_key != work_point.scope_key {
                findings.push(error(
                    EntityType::WorkPoint,
                    work_point_id,
                    &work_point.scope_key,
                    "CROSS-SCOPE-REFERENCE",
                    &format!("supersedes target `{supersede_id}` is in scope `{}` but work point is in scope `{}`", supersede.scope_key, work_point.scope_key),
                )?);
            }
        }
    }
    for block_id in &work_point.blocks_work_point_ids {
        if let Ok(blocked) = load_work_point(connection, block_id) {
            if blocked.scope_key != work_point.scope_key {
                findings.push(error(
                    EntityType::WorkPoint,
                    work_point_id,
                    &work_point.scope_key,
                    "CROSS-SCOPE-REFERENCE",
                    &format!("blocks target `{block_id}` is in scope `{}` but work point is in scope `{}`", blocked.scope_key, work_point.scope_key),
                )?);
            }
        }
    }

    Ok(findings)
}

fn validate_plan(
    connection: &Connection,
    plan_id: &str,
) -> Result<Vec<ValidationFinding>, PlanningStoreError> {
    let plan = load_plan(connection, plan_id)?;
    let roadmap = load_roadmap(connection, &plan.roadmap_id)?;
    let mut findings = Vec::new();

    if roadmap.goal_id != plan.goal_id {
        findings.push(error(
            EntityType::Plan,
            plan_id,
            &plan.scope_key,
            "PLAN-GOAL-ROADMAP-MISMATCH",
            "plan goal does not match roadmap goal",
        )?);
    }

    if plan.targeted_work_point_ids.is_empty() {
        findings.push(warning(
            EntityType::Plan,
            plan_id,
            &plan.scope_key,
            "PLAN-NO-TARGETED-WORK",
            "plan does not target any work points yet",
        )?);
    }

    if plan.validation_steps.is_empty() {
        findings.push(warning(
            EntityType::Plan,
            plan_id,
            &plan.scope_key,
            "PLAN-NO-VALIDATION-STEPS",
            "plan does not define validation steps yet",
        )?);
    }

    for work_point_id in &plan.targeted_work_point_ids {
        let work_point = load_work_point(connection, work_point_id);
        match work_point {
            Ok(work_point) => {
                if work_point.roadmap_id != plan.roadmap_id {
                    findings.push(error(
                        EntityType::Plan,
                        plan_id,
                        &plan.scope_key,
                        "PLAN-WORK-POINT-ROADMAP-MISMATCH",
                        &format!(
                            "targeted work point `{work_point_id}` belongs to a different roadmap"
                        ),
                    )?);
                }
            }
            Err(PlanningStoreError::NotFound { .. }) => findings.push(error(
                EntityType::Plan,
                plan_id,
                &plan.scope_key,
                "PLAN-WORK-POINT-MISSING",
                &format!("targeted work point `{work_point_id}` does not exist"),
            )?),
            Err(error) => return Err(error),
        }
    }

    let todos = list_todos_for_plan(connection, plan_id)?;
    if todos.is_empty() {
        findings.push(warning(
            EntityType::Plan,
            plan_id,
            &plan.scope_key,
            "PLAN-NO-TODOS",
            "plan has no todo records yet",
        )?);
    }

    if plan.status.as_str() == "completed"
        && todos
            .iter()
            .any(|todo| !matches!(todo.status, TodoStatus::Completed | TodoStatus::Cancelled))
    {
        findings.push(error(
            EntityType::Plan,
            plan_id,
            &plan.scope_key,
            "PLAN-COMPLETED-WITH-OPEN-TODOS",
            "completed plan still has incomplete todos",
        )?);
    }

    // CROSS-SCOPE-REFERENCE: verify targeted work points share the same scope
    for wp_id in &plan.targeted_work_point_ids {
        if let Ok(wp) = load_work_point(connection, wp_id) {
            if wp.scope_key != plan.scope_key {
                findings.push(error(
                    EntityType::Plan,
                    plan_id,
                    &plan.scope_key,
                    "CROSS-SCOPE-REFERENCE",
                    &format!(
                        "targeted work point `{wp_id}` is in scope `{}` but plan is in scope `{}`",
                        wp.scope_key, plan.scope_key
                    ),
                )?);
            }
        }
    }
    if roadmap.scope_key != plan.scope_key {
        findings.push(error(
            EntityType::Plan,
            plan_id,
            &plan.scope_key,
            "CROSS-SCOPE-REFERENCE",
            &format!(
                "linked roadmap is in scope `{}` but plan is in scope `{}`",
                roadmap.scope_key, plan.scope_key
            ),
        )?);
    }

    let blocking_issue_count: i64 = connection.query_row(
        r#"
        SELECT COUNT(*)
        FROM issues
        WHERE related_entity_type = 'plan'
          AND related_entity_id = ?1
          AND status IN ('open', 'blocked', 'reopened')
          AND severity IN ('high', 'critical')
        "#,
        params![plan_id],
        |row| row.get(0),
    )?;
    if blocking_issue_count > 0 {
        findings.push(error(
            EntityType::Plan,
            plan_id,
            &plan.scope_key,
            "PLAN-BLOCKING-ISSUES",
            "plan has unresolved high-severity or critical issues attached",
        )?);
    }

    let review_points = list_review_points_for_entity(connection, EntityType::Plan, plan_id)?;
    if review_points.iter().any(|point| {
        !matches!(
            point.status,
            ReviewPointStatus::Resolved | ReviewPointStatus::AcceptedRisk
        ) && matches!(point.severity, Severity::High | Severity::Critical)
    }) {
        findings.push(error(
            EntityType::Plan,
            plan_id,
            &plan.scope_key,
            "PLAN-OPEN-REVIEW-POINTS",
            "plan has unresolved high-severity or critical review points",
        )?);
    }

    Ok(findings)
}

fn validate_todo(
    connection: &Connection,
    todo_id: &str,
) -> Result<Vec<ValidationFinding>, PlanningStoreError> {
    let todo = load_todo(connection, todo_id)?;
    let mut findings = Vec::new();

    if todo.plan_id.is_none() && todo.work_point_id.is_none() {
        findings.push(warning(
            EntityType::Todo,
            todo_id,
            &todo.scope_key,
            "TODO-STANDALONE",
            "todo is standalone and should be linked to a plan or work point when possible",
        )?);
    }

    if matches!(todo.status, TodoStatus::Completed) && todo.evidence_refs.is_empty() {
        findings.push(warning(
            EntityType::Todo,
            todo_id,
            &todo.scope_key,
            "TODO-COMPLETED-WITHOUT-EVIDENCE",
            "completed todo has no evidence references",
        )?);
    }

    if let (Some(plan_id), Some(work_point_id)) = (&todo.plan_id, &todo.work_point_id) {
        let plan = load_plan(connection, plan_id)?;
        if !plan
            .targeted_work_point_ids
            .iter()
            .any(|id| id == work_point_id)
        {
            findings.push(warning(
                EntityType::Todo,
                todo_id,
                &todo.scope_key,
                "TODO-PLAN-WORK-POINT-MISMATCH",
                "todo links to both a plan and work point, but the plan does not target that work point",
            )?);
        }
    }

    Ok(findings)
}

fn validate_issue(
    connection: &Connection,
    issue_id: &str,
) -> Result<Vec<ValidationFinding>, PlanningStoreError> {
    let issue = load_issue(connection, issue_id)?;
    let mut findings = Vec::new();

    match (&issue.related_entity_type, &issue.related_entity_id) {
        (Some(_), None) | (None, Some(_)) => findings.push(warning(
            EntityType::Issue,
            issue_id,
            &issue.scope_key,
            "ISSUE-PARTIAL-ENTITY-LINK",
            "issue should declare both related entity type and related entity id when linking to another record",
        )?),
        (Some(entity_type), Some(entity_id)) => {
            if let Err(PlanningStoreError::NotFound { .. }) =
                attached_entity_correlation_id(connection, *entity_type, entity_id)
            {
                findings.push(error(
                    EntityType::Issue,
                    issue_id,
                    &issue.scope_key,
                    "ISSUE-RELATED-ENTITY-MISSING",
                    "issue references a missing related entity",
                )?);
            }
        }
        (None, None) => {}
    }

    if matches!(issue.status, IssueStatus::Blocked)
        && !matches!(issue.severity, Severity::High | Severity::Critical)
    {
        findings.push(warning(
            EntityType::Issue,
            issue_id,
            &issue.scope_key,
            "ISSUE-BLOCKED-LOW-SEVERITY",
            "blocked issue should usually be medium severity or higher",
        )?);
    }

    Ok(findings)
}

fn validate_review_point(
    connection: &Connection,
    review_point_id: &str,
) -> Result<Vec<ValidationFinding>, PlanningStoreError> {
    let review_point = connection
        .query_row(
            "SELECT attached_entity_type, attached_entity_id, scope_key, status, severity FROM review_points WHERE id = ?1",
            params![review_point_id],
            |row| {
                Ok((
                    crate::EntityType::from_str(&row.get::<_, String>(0)?).map_err(sql_string_err)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    crate::ReviewPointStatus::from_str(&row.get::<_, String>(3)?).map_err(sql_string_err)?,
                    crate::Severity::from_str(&row.get::<_, String>(4)?).map_err(sql_string_err)?,
                ))
            },
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => PlanningStoreError::NotFound {
                entity_type: EntityType::ReviewPoint.as_str().to_string(),
                entity_id: review_point_id.to_string(),
            },
            other => PlanningStoreError::Sqlite(other),
        })?;

    let mut findings = Vec::new();
    if let Err(PlanningStoreError::NotFound { .. }) =
        attached_entity_correlation_id(connection, review_point.0, &review_point.1)
    {
        findings.push(error(
            EntityType::ReviewPoint,
            review_point_id,
            &review_point.2,
            "REVIEW-POINT-ATTACHED-ENTITY-MISSING",
            "review point references a missing attached entity",
        )?);
    }

    if matches!(review_point.3, ReviewPointStatus::Open)
        && matches!(review_point.4, Severity::Critical)
    {
        findings.push(warning(
            EntityType::ReviewPoint,
            review_point_id,
            &review_point.2,
            "REVIEW-POINT-CRITICAL-OPEN",
            "critical review point remains open and should be resolved or explicitly accepted",
        )?);
    }

    Ok(findings)
}

fn validate_insight(
    connection: &Connection,
    insight_id: &str,
) -> Result<Vec<ValidationFinding>, PlanningStoreError> {
    let insight = load_insight(connection, insight_id)?;
    let mut findings = Vec::new();

    if insight.content.trim().is_empty() {
        findings.push(error(
            EntityType::Insight,
            insight_id,
            &insight.scope_key,
            "INSIGHT-EMPTY-CONTENT",
            "insight content must not be empty",
        )?);
    }

    if insight.tags.is_empty() {
        findings.push(warning(
            EntityType::Insight,
            insight_id,
            &insight.scope_key,
            "INSIGHT-TAG-ORPHAN",
            "insight has no tags; add tags for discoverability",
        )?);
    }

    if let Err(PlanningStoreError::NotFound { .. }) = attached_entity_correlation_id(
        connection,
        insight.parent_entity_type,
        &insight.parent_entity_id,
    ) {
        findings.push(error(
            EntityType::Insight,
            insight_id,
            &insight.scope_key,
            "INSIGHT-NO-PARENT",
            &format!(
                "insight references missing parent {} `{}`",
                insight.parent_entity_type.as_str(),
                insight.parent_entity_id
            ),
        )?);
    }

    Ok(findings)
}

fn warning(
    entity_type: EntityType,
    entity_id: &str,
    scope_key: &str,
    code: &str,
    message: &str,
) -> Result<ValidationFinding, PlanningStoreError> {
    finding(
        entity_type,
        entity_id,
        scope_key,
        ValidationSeverity::Warning,
        code,
        message,
    )
}

fn error(
    entity_type: EntityType,
    entity_id: &str,
    scope_key: &str,
    code: &str,
    message: &str,
) -> Result<ValidationFinding, PlanningStoreError> {
    finding(
        entity_type,
        entity_id,
        scope_key,
        ValidationSeverity::Error,
        code,
        message,
    )
}

fn finding(
    entity_type: EntityType,
    entity_id: &str,
    scope_key: &str,
    severity: ValidationSeverity,
    code: &str,
    message: &str,
) -> Result<ValidationFinding, PlanningStoreError> {
    let fingerprint = format!(
        "{}::{}::{}::{}",
        entity_type.as_str(),
        entity_id,
        scope_key,
        code
    );
    Ok(ValidationFinding {
        finding_id: uuid::Uuid::new_v4().to_string(),
        entity_type,
        entity_id: entity_id.to_string(),
        severity,
        code: code.to_string(),
        message: message.to_string(),
        scope_key: scope_key.to_string(),
        fingerprint,
        created_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|_| PlanningStoreError::TimeFormat)?,
    })
}

fn validate_graph_node(
    connection: &Connection,
    node_id: &str,
) -> Result<Vec<ValidationFinding>, PlanningStoreError> {
    let node = match load_graph_node(connection, node_id) {
        Ok(n) => n,
        Err(_) => return Ok(Vec::new()), // node missing — edge validators will catch this
    };
    let mut findings = Vec::new();

    // GRAPH-STATUS-INVALID: check status is valid kebab-case
    let status = node.status.trim();
    let is_valid_status = !status.is_empty()
        && status
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && status.starts_with(|c: char| c.is_ascii_lowercase())
        && !status.ends_with('-')
        && !status.contains("--");

    if !is_valid_status {
        findings.push(warning(
            EntityType::GraphNode,
            node_id,
            &node.scope_key,
            "GRAPH-STATUS-INVALID",
            &format!(
                "graph node status '{}' is not a valid lowercase kebab-case token",
                node.status
            ),
        )?);
    }

    // ACCEPTANCE-KIND-INVALID: acceptance node must have a valid acceptanceKind in payload
    if node.kind == PlanningNodeKind::Acceptance {
        let acceptance_kind_str = node
            .payload
            .get("acceptanceKind")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if AcceptanceKind::from_str(acceptance_kind_str).is_err() {
            findings.push(warning(
                EntityType::GraphNode,
                node_id,
                &node.scope_key,
                "ACCEPTANCE-KIND-INVALID",
                &format!(
                    "acceptance node '{}' has invalid or missing acceptanceKind: '{}' (expected 'abstract' or 'concrete')",
                    node_id, acceptance_kind_str
                ),
            )?);
        }

        // ACCEPTANCE-COVERAGE-MISSING: abstract acceptance must have at least one active concrete satisfies edge
        if acceptance_kind_str == "abstract" {
            let incoming =
                list_incoming_edges_in_scope(connection, node_id, &node.scope_key, None)?;
            let has_active_coverage = incoming.iter().any(|e| {
                e.kind == PlanningEdgeKind::Satisfies
                    && e.status == "active"
                    && e.source_node_id != node_id
                    && {
                        if let Ok(source_node) = load_graph_node(connection, &e.source_node_id) {
                            source_node.kind == PlanningNodeKind::Acceptance
                                && source_node
                                    .payload
                                    .get("acceptanceKind")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    == "concrete"
                        } else {
                            false
                        }
                    }
            });
            if !has_active_coverage {
                findings.push(warning(
                    EntityType::GraphNode,
                    node_id,
                    &node.scope_key,
                    "ACCEPTANCE-COVERAGE-MISSING",
                    &format!(
                        "abstract acceptance '{}' has no active concrete acceptance satisfying it",
                        node_id
                    ),
                )?);
            }
        }

        // ACCEPTANCE-EVIDENCE-MISSING: concrete acceptance requiring evidence must have active evidence
        if acceptance_kind_str == "concrete" {
            let required_kinds: Vec<String> = node
                .payload
                .get("requiredEvidenceKinds")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            if !required_kinds.is_empty() {
                let outgoing =
                    list_outgoing_edges_in_scope(connection, node_id, &node.scope_key, None)?;
                let evidence_edges: Vec<_> = outgoing
                    .iter()
                    .filter(|e| e.kind == PlanningEdgeKind::EvidencedBy && e.status == "active")
                    .collect();

                // Check each required evidence kind is covered by at least one attached evidence node
                for required_kind in &required_kinds {
                    let covered = evidence_edges.iter().any(|edge| {
                        if let Ok(evidence_node) = load_graph_node(connection, &edge.target_node_id)
                        {
                            if evidence_node.kind == PlanningNodeKind::Evidence {
                                let evidence_kind = evidence_node
                                    .payload
                                    .get("evidenceKind")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                return evidence_kind == required_kind;
                            }
                        }
                        false
                    });
                    if !covered {
                        findings.push(warning(
                            EntityType::GraphNode,
                            node_id,
                            &node.scope_key,
                            "ACCEPTANCE-EVIDENCE-MISSING",
                            &format!(
                                "concrete acceptance '{}' requires evidence of kind '{}' but none is attached",
                                node_id, required_kind
                            ),
                        )?);
                    }
                }
            }
        }
    }

    // EVIDENCE-KIND-INVALID: evidence node must have a valid evidenceKind in payload
    if node.kind == PlanningNodeKind::Evidence {
        let evidence_kind_str = node
            .payload
            .get("evidenceKind")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if EvidenceKind::from_str(evidence_kind_str).is_err() {
            findings.push(warning(
                EntityType::GraphNode,
                node_id,
                &node.scope_key,
                "EVIDENCE-KIND-INVALID",
                &format!(
                    "evidence node '{}' has invalid or missing evidenceKind: '{}'",
                    node_id, evidence_kind_str
                ),
            )?);
        }
    }

    Ok(findings)
}

fn validate_graph_edge(
    connection: &Connection,
    edge_id: &str,
) -> Result<Vec<ValidationFinding>, PlanningStoreError> {
    let edge = match load_graph_edge(connection, edge_id) {
        Ok(e) => e,
        Err(_) => return Ok(Vec::new()), // edge missing — nothing to validate
    };
    let mut findings = Vec::new();
    let scope_key = &edge.scope_key;

    // GRAPH-STATUS-INVALID: check edge status is valid kebab-case
    let status = edge.status.trim();
    let is_valid_status = !status.is_empty()
        && status
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && status.starts_with(|c: char| c.is_ascii_lowercase())
        && !status.ends_with('-')
        && !status.contains("--");

    if !is_valid_status {
        findings.push(warning(
            EntityType::GraphEdge,
            edge_id,
            scope_key,
            "GRAPH-STATUS-INVALID",
            &format!(
                "graph edge status '{}' is not a valid lowercase kebab-case token",
                edge.status
            ),
        )?);
    }

    // Load source and target nodes (may not exist)
    let source = load_graph_node(connection, &edge.source_node_id);
    let target = load_graph_node(connection, &edge.target_node_id);

    // GRAPH-EDGE-MISSING-NODE
    if source.is_err() {
        findings.push(warning(
            EntityType::GraphEdge,
            edge_id,
            scope_key,
            "GRAPH-EDGE-MISSING-NODE",
            &format!(
                "source node '{}' referenced by edge '{}' does not exist",
                edge.source_node_id, edge_id
            ),
        )?);
    }
    if target.is_err() {
        findings.push(warning(
            EntityType::GraphEdge,
            edge_id,
            scope_key,
            "GRAPH-EDGE-MISSING-NODE",
            &format!(
                "target node '{}' referenced by edge '{}' does not exist",
                edge.target_node_id, edge_id
            ),
        )?);
    }

    // Only continue if both nodes exist (avoid cascading findings)
    if let (Ok(source_node), Ok(target_node)) = (&source, &target) {
        // GRAPH-EDGE-SELF-LOOP
        if edge.source_node_id == edge.target_node_id {
            findings.push(warning(
                EntityType::GraphEdge,
                edge_id,
                scope_key,
                "GRAPH-EDGE-SELF-LOOP",
                &format!(
                    "edge '{}' is a self-loop (source == target == '{}')",
                    edge_id, edge.source_node_id
                ),
            )?);
        }

        // GRAPH-EDGE-CROSS-SCOPE: node scopes must match edge scope
        if source_node.scope_key != *scope_key {
            findings.push(warning(
                EntityType::GraphEdge,
                edge_id,
                scope_key,
                "GRAPH-EDGE-CROSS-SCOPE",
                &format!(
                    "source node '{}' is in scope '{}' but edge '{}' is in scope '{}'",
                    edge.source_node_id, source_node.scope_key, edge_id, scope_key
                ),
            )?);
        }
        if target_node.scope_key != *scope_key {
            findings.push(warning(
                EntityType::GraphEdge,
                edge_id,
                scope_key,
                "GRAPH-EDGE-CROSS-SCOPE",
                &format!(
                    "target node '{}' is in scope '{}' but edge '{}' is in scope '{}'",
                    edge.target_node_id, target_node.scope_key, edge_id, scope_key
                ),
            )?);
        }

        // GRAPH-EDGE-KIND-MISMATCH
        if let Err(err) = validate_edge_kind_pair(&edge.kind, &source_node.kind, &target_node.kind)
        {
            findings.push(warning(
                EntityType::GraphEdge,
                edge_id,
                scope_key,
                "GRAPH-EDGE-KIND-MISMATCH",
                &format!("edge '{}' has invalid kind pair: {}", edge_id, err),
            )?);
        }

        // GRAPH-EDGE-DUPLICATE-ACTIVE: detect duplicate active edges
        if edge.status == "active" {
            // Check for outgoing duplicates
            let outgoing =
                list_outgoing_edges_for_node(connection, &edge.source_node_id, Some(edge.kind))?;
            let dup_count = outgoing
                .iter()
                .filter(|e| {
                    e.id != edge.id
                        && e.target_node_id == edge.target_node_id
                        && e.status == "active"
                        && e.scope_key == *scope_key
                })
                .count();
            if dup_count > 0 {
                findings.push(warning(
                    EntityType::GraphEdge,
                    edge_id,
                    scope_key,
                    "GRAPH-EDGE-DUPLICATE-ACTIVE",
                    &format!(
                        "duplicate active {} edge from '{}' to '{}' in scope '{}'",
                        edge.kind.as_str(),
                        edge.source_node_id,
                        edge.target_node_id,
                        scope_key
                    ),
                )?);
            }
        }

        // GRAPH-EDGE-CYCLE: check for would-be cycles in decomposes-to / depends-on
        // Only active edges can participate in cycles
        if edge.status == "active" {
            match edge.kind {
                PlanningEdgeKind::DecomposesTo | PlanningEdgeKind::DependsOn => {
                    match would_create_graph_cycle(
                        connection,
                        &edge.source_node_id,
                        &edge.target_node_id,
                        &edge.kind,
                    ) {
                        Ok(true) => {
                            findings.push(warning(
                                EntityType::GraphEdge,
                                edge_id,
                                scope_key,
                                "GRAPH-EDGE-CYCLE",
                                &format!(
                                    "{} edge from '{}' to '{}' creates a cycle",
                                    edge.kind.as_str(),
                                    edge.source_node_id,
                                    edge.target_node_id
                                ),
                            )?);
                        }
                        Err(_) => {} // cycle check failure is non-fatal for validation
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    // ACCEPTANCE-RATIONALE-MISSING: active Satisfies edge must have a non-empty rationale
    if edge.kind == PlanningEdgeKind::Satisfies && edge.status == "active" {
        let rationale = edge
            .payload
            .get("rationale")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if rationale.trim().is_empty() {
            findings.push(warning(
                EntityType::GraphEdge,
                edge_id,
                scope_key,
                "ACCEPTANCE-RATIONALE-MISSING",
                &format!(
                    "active Satisfies edge '{}' has no non-empty rationale",
                    edge_id
                ),
            )?);
        }
    }

    Ok(findings)
}

fn sql_string_err(message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )),
    )
}
