use std::str::FromStr;

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    storage::{
        attached_entity_correlation_id, list_review_points_for_entity, list_todos_for_plan,
        list_work_points_for_roadmap, load_goal, load_insight, load_issue, load_plan,
        load_project_run, load_roadmap, load_todo, load_work_point,
    },
    EntityType, GoalStatus, IssueStatus, PlanningStoreError, ProjectRunStatus, ReviewPointStatus,
    Severity, TodoStatus, ValidationFinding, ValidationSeverity, WorkPointStatus,
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
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
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
