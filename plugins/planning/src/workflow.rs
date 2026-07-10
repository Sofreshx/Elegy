use std::collections::HashSet;

use serde_json::Value;

use crate::{
    EffortTier, EntityType, EvidenceKind, FileScopeIntent, FileScopeRecord, FileScopeSelectorType,
    PlanningGraphEdge, PlanningGraphNode, PlanningNodeKind, ProjectRunRecord, ProjectRunStatus,
};

pub(crate) fn normalize_file_scopes(file_scopes: Vec<FileScopeRecord>) -> Vec<FileScopeRecord> {
    let mut scopes: Vec<FileScopeRecord> = file_scopes
        .into_iter()
        .map(|scope| FileScopeRecord {
            selector_type: scope.selector_type,
            selector: scope.selector.trim().to_string(),
            intent: scope.intent,
        })
        .filter(|scope| !scope.selector.is_empty())
        .collect();
    scopes.sort_by(|left, right| {
        left.selector_type
            .as_str()
            .cmp(right.selector_type.as_str())
            .then_with(|| left.intent.as_str().cmp(right.intent.as_str()))
            .then_with(|| left.selector.cmp(&right.selector))
    });
    scopes.dedup_by(|left, right| {
        left.selector_type == right.selector_type
            && left.intent == right.intent
            && left.selector == right.selector
    });
    scopes
}

pub(crate) fn workflow_delegation_hint_for_node(
    node: &PlanningGraphNode,
) -> crate::WorkflowDelegationHint {
    let role = if node
        .tags
        .iter()
        .any(|tag| tag == "review" || tag == "review-fix")
    {
        "reviewer"
    } else if node
        .tags
        .iter()
        .any(|tag| tag == "validation" || tag == "test")
    {
        "test-runner"
    } else {
        "implementation"
    };
    let effort_tier = workflow_effort_tier_for_node(node);
    let file_scopes = workflow_file_scopes_for_node(node);
    let max_context_tokens_estimate = workflow_context_token_estimate(effort_tier);
    let model_tier = workflow_model_tier_for_node(node, role, effort_tier);
    let worker_profile = workflow_worker_profile(role, effort_tier);

    crate::WorkflowDelegationHint {
        node_id: node.id.clone(),
        role: role.to_string(),
        worker_profile: worker_profile.to_string(),
        recommended_subagent: workflow_recommended_subagent(worker_profile).to_string(),
        model_tier,
        effort_tier,
        file_scopes,
        allowed_actions: workflow_allowed_actions(role),
        max_concurrency: 1,
        max_context_tokens_estimate,
        wall_time_minutes_estimate: workflow_wall_time_minutes_estimate(effort_tier),
        retry_policy: crate::WorkflowRetryPolicy {
            max_attempts: 2,
            retryable_stop_conditions: vec![
                "transient validation failure".to_string(),
                "tool/runtime interruption".to_string(),
            ],
            escalation: "return to orchestrator for review or corrective work".to_string(),
        },
        rationale: "derived from graph work node tags and conservative default policy".to_string(),
    }
}

pub(crate) fn workflow_work_point_id_for_node(node: &PlanningGraphNode) -> Option<String> {
    node.payload
        .get("workPointId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            let reference = node.payload.get("planningRef").and_then(Value::as_object)?;
            if reference.get("entityType").and_then(Value::as_str) != Some("work-point") {
                return None;
            }
            reference
                .get("entityId")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

pub(crate) fn workflow_idempotency_key(
    entity_type: EntityType,
    entity_id: &str,
    node_id: &str,
    node_revision: i64,
    correlation_id: &str,
) -> String {
    format!(
        "workflow:{}:{}:{}:{}:{}",
        entity_type.as_str(),
        entity_id,
        node_id,
        node_revision,
        correlation_id
    )
}

pub(crate) struct WorkflowDispatchInput<'a> {
    pub(crate) scope_key: &'a str,
    pub(crate) source: &'a crate::WorkflowViewSource,
    pub(crate) phase_id: &'a str,
    pub(crate) node: &'a PlanningGraphNode,
    pub(crate) hint: &'a crate::WorkflowDelegationHint,
    pub(crate) adapter_id: &'a str,
    pub(crate) requested_capabilities: &'a [String],
    pub(crate) project_run: ProjectRunRecord,
    pub(crate) evidence_policy: &'a crate::WorkflowEvidencePolicy,
}

pub(crate) fn workflow_dispatch_from_parts(
    input: WorkflowDispatchInput<'_>,
) -> crate::WorkflowDispatch {
    let WorkflowDispatchInput {
        scope_key,
        source,
        phase_id,
        node,
        hint,
        adapter_id,
        requested_capabilities,
        project_run,
        evidence_policy,
    } = input;
    let complexity = match hint.effort_tier {
        EffortTier::Fast => "light",
        EffortTier::Balanced => "standard",
        EffortTier::Deep => "heavy",
    };
    let reasoning_class = match hint.effort_tier {
        EffortTier::Fast => "low",
        EffortTier::Balanced => "medium",
        EffortTier::Deep => "high",
    };
    let required_evidence_kinds = evidence_policy.required_evidence_kinds.clone();
    let required_capabilities = workflow_required_capabilities(node, hint, requested_capabilities);
    crate::WorkflowDispatch {
        schema_version: "orchestrator-dispatch/v1".to_string(),
        dispatch_id: format!("dispatch-{}", project_run.id),
        scope_key: scope_key.to_string(),
        adapter_id: adapter_id.to_string(),
        required_capabilities,
        source: source.clone(),
        node_id: node.id.clone(),
        source_revision: node.revision,
        fencing_token: project_run.fencing_token,
        idempotency_key: project_run.idempotency_key.clone().unwrap_or_default(),
        phase_id: phase_id.to_string(),
        role: hint.role.clone(),
        worker_profile: hint.worker_profile.clone(),
        recommended_subagent: hint.recommended_subagent.clone(),
        complexity: complexity.to_string(),
        reasoning_class: reasoning_class.to_string(),
        file_scopes: hint.file_scopes.clone(),
        allowed_actions: hint.allowed_actions.clone(),
        required_evidence_kinds: required_evidence_kinds.clone(),
        handoff: crate::WorkflowHandoff {
            title: node.title.clone(),
            summary: node.summary.clone(),
            acceptance: required_evidence_kinds
                .iter()
                .map(|kind| kind.as_str().to_string())
                .collect(),
            stop_conditions: vec![
                "planning source or file scope becomes stale".to_string(),
                "required validation cannot run".to_string(),
                "new work exceeds the dispatch scope".to_string(),
            ],
        },
        budget: crate::WorkflowWorkerBudget {
            max_context_tokens_estimate: hint.max_context_tokens_estimate.min(12_000),
            max_output_bytes: 32 * 1024,
            wall_time_minutes_estimate: hint.wall_time_minutes_estimate,
            max_attempts: hint.retry_policy.max_attempts,
        },
        project_run,
    }
}

fn workflow_required_capabilities(
    node: &PlanningGraphNode,
    hint: &crate::WorkflowDelegationHint,
    requested_capabilities: &[String],
) -> Vec<String> {
    let mut capabilities = requested_capabilities
        .iter()
        .map(String::as_str)
        .chain(
            node.payload
                .get("requiredCapabilities")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str),
        )
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    if hint.role == "test-runner" {
        capabilities.push("validation".to_string());
    }
    for tag in ["browser", "container", "e2e"] {
        if node.tags.iter().any(|value| value == tag) {
            capabilities.push(tag.to_string());
        }
    }
    capabilities.sort();
    capabilities.dedup();
    capabilities
}

pub(crate) fn workflow_result_graph_status(status: crate::WorkflowResultStatus) -> String {
    match status {
        crate::WorkflowResultStatus::Completed => "completed".to_string(),
        crate::WorkflowResultStatus::Failed
        | crate::WorkflowResultStatus::TimedOut
        | crate::WorkflowResultStatus::Malformed => "blocked".to_string(),
        crate::WorkflowResultStatus::Cancelled => "cancelled".to_string(),
    }
}

pub(crate) fn workflow_result_project_run_status(
    status: crate::WorkflowResultStatus,
) -> ProjectRunStatus {
    match status {
        crate::WorkflowResultStatus::Completed => ProjectRunStatus::Completed,
        crate::WorkflowResultStatus::Cancelled => ProjectRunStatus::Cancelled,
        crate::WorkflowResultStatus::Failed
        | crate::WorkflowResultStatus::TimedOut
        | crate::WorkflowResultStatus::Malformed => ProjectRunStatus::Failed,
    }
}

pub(crate) fn workflow_effort_tier_for_node(node: &PlanningGraphNode) -> EffortTier {
    if let Some(value) = node
        .payload
        .get("effortTier")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| value.parse::<EffortTier>().ok())
    {
        return value;
    }

    if node.tags.iter().any(|tag| tag == "deep") {
        EffortTier::Deep
    } else if node.tags.iter().any(|tag| tag == "fast") {
        EffortTier::Fast
    } else {
        EffortTier::Balanced
    }
}

pub(crate) fn workflow_model_tier_for_node(
    node: &PlanningGraphNode,
    role: &str,
    effort_tier: EffortTier,
) -> String {
    if let Some(value) = node
        .payload
        .get("modelTier")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        let value = value.trim().to_ascii_lowercase();
        if value == "xhigh"
            && node
                .payload
                .get("allowXhighWorker")
                .and_then(Value::as_bool)
                != Some(true)
        {
            return "high".to_string();
        }
        return value;
    }

    match (role, effort_tier) {
        ("reviewer", _) | (_, EffortTier::Deep) => "high".to_string(),
        ("test-runner", _) | (_, EffortTier::Balanced) => "medium".to_string(),
        _ => "low".to_string(),
    }
}

pub(crate) fn workflow_worker_profile(role: &str, effort_tier: EffortTier) -> &'static str {
    match (role, effort_tier) {
        ("reviewer", _) => "reviewer",
        ("test-runner", _) => "test-runner",
        ("implementation", EffortTier::Fast) => "implementation-light",
        ("implementation", EffortTier::Deep) => "implementation-heavy",
        _ => "implementation",
    }
}

pub(crate) fn workflow_recommended_subagent(worker_profile: &str) -> &'static str {
    match worker_profile {
        "reviewer" => "reviewer",
        "test-runner" => "test-runner",
        "implementation-light" => "impl-light",
        "implementation-heavy" => "impl-heavy",
        _ => "impl",
    }
}

pub(crate) fn workflow_allowed_actions(role: &str) -> Vec<String> {
    match role {
        "reviewer" => vec![
            "read".to_string(),
            "review".to_string(),
            "validate".to_string(),
        ],
        "test-runner" => vec![
            "read".to_string(),
            "execute-validation".to_string(),
            "record-evidence".to_string(),
        ],
        _ => vec![
            "read".to_string(),
            "edit".to_string(),
            "execute-validation".to_string(),
            "record-evidence".to_string(),
        ],
    }
}

pub(crate) fn workflow_context_token_estimate(effort_tier: EffortTier) -> usize {
    match effort_tier {
        EffortTier::Fast => 6_000,
        EffortTier::Balanced => 12_000,
        EffortTier::Deep => 24_000,
    }
}

pub(crate) fn workflow_wall_time_minutes_estimate(effort_tier: EffortTier) -> usize {
    match effort_tier {
        EffortTier::Fast => 15,
        EffortTier::Balanced => 45,
        EffortTier::Deep => 90,
    }
}

pub(crate) fn workflow_file_scopes_for_node(node: &PlanningGraphNode) -> Vec<FileScopeRecord> {
    let Some(scopes) = node
        .payload
        .get("fileScopes")
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };

    normalize_file_scopes(
        scopes
            .iter()
            .filter_map(|scope| {
                let selector = scope
                    .get("selector")
                    .and_then(serde_json::Value::as_str)?
                    .trim();
                if selector.is_empty() {
                    return None;
                }
                let selector_type = scope
                    .get("selectorType")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|value| value.parse::<FileScopeSelectorType>().ok())
                    .unwrap_or(FileScopeSelectorType::Glob);
                let intent = scope
                    .get("intent")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|value| value.parse::<FileScopeIntent>().ok())
                    .unwrap_or(FileScopeIntent::Primary);
                Some(FileScopeRecord {
                    selector_type,
                    selector: selector.to_string(),
                    intent,
                })
            })
            .collect(),
    )
}

pub(crate) fn workflow_required_evidence_kinds(nodes: &[PlanningGraphNode]) -> Vec<EvidenceKind> {
    let mut kinds = vec![EvidenceKind::CommandResult, EvidenceKind::TestResult];
    let mut seen = kinds
        .iter()
        .map(|kind| kind.as_str().to_string())
        .collect::<HashSet<_>>();

    for node in nodes
        .iter()
        .filter(|node| node.kind == PlanningNodeKind::Acceptance)
    {
        let Some(required) = node
            .payload
            .get("requiredEvidenceKinds")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };

        for value in required {
            let Some(kind) = value
                .as_str()
                .and_then(|value| value.parse::<EvidenceKind>().ok())
            else {
                continue;
            };
            if seen.insert(kind.as_str().to_string()) {
                kinds.push(kind);
            }
        }
    }

    kinds
}

pub(crate) fn workflow_execution_plan(
    runnable: &crate::GraphRunnableResult,
    delegation_hints: &[crate::WorkflowDelegationHint],
    edges: &[PlanningGraphEdge],
) -> crate::WorkflowExecutionPlan {
    let node_ids = runnable
        .candidates
        .iter()
        .map(|candidate| candidate.node_id.clone())
        .collect::<Vec<_>>();
    let blocked_node_ids = runnable
        .blocked
        .iter()
        .map(|candidate| candidate.node_id.clone())
        .collect::<Vec<_>>();

    if node_ids.is_empty() {
        return crate::WorkflowExecutionPlan {
            strategy: "blocked-or-empty".to_string(),
            phases: Vec::new(),
            blocked_node_ids,
            rationale: "no runnable graph work is currently available".to_string(),
        };
    }

    let parallel_safe_count =
        workflow_parallel_safe_candidate_count(runnable, delegation_hints, edges);
    let max_concurrency = parallel_safe_count.clamp(1, 3);
    let mode = if max_concurrency > 1 {
        "bounded-parallel"
    } else {
        "sequential"
    };

    let mut worker_profiles = delegation_hints
        .iter()
        .filter(|hint| node_ids.iter().any(|node_id| node_id == &hint.node_id))
        .map(|hint| hint.worker_profile.clone())
        .collect::<Vec<_>>();
    worker_profiles.sort();
    worker_profiles.dedup();

    crate::WorkflowExecutionPlan {
        strategy: "next-runnable-batch".to_string(),
        phases: vec![crate::WorkflowExecutionPhase {
            id: "phase-1".to_string(),
            index: 1,
            mode: mode.to_string(),
            node_ids,
            max_concurrency,
            worker_profiles,
            rationale: "phase contains currently runnable graph work; later phases require refreshed planning state".to_string(),
        }],
        blocked_node_ids,
        rationale: "derive one executable batch from current runnable graph state and require refresh after writeback".to_string(),
    }
}

pub(crate) fn workflow_nodes_for_source(
    nodes: Vec<PlanningGraphNode>,
    entity_type: EntityType,
    entity_id: &str,
) -> Vec<PlanningGraphNode> {
    let has_bindings = nodes.iter().any(workflow_node_has_planning_ref);
    if !has_bindings {
        return nodes;
    }

    nodes
        .into_iter()
        .filter(|node| workflow_node_matches_source(node, entity_type, entity_id))
        .collect()
}

pub(crate) fn workflow_node_has_planning_ref(node: &PlanningGraphNode) -> bool {
    node.payload
        .get("planningRef")
        .and_then(Value::as_object)
        .and_then(|reference| reference.get("entityType"))
        .and_then(Value::as_str)
        .is_some()
}

pub(crate) fn workflow_node_matches_source(
    node: &PlanningGraphNode,
    entity_type: EntityType,
    entity_id: &str,
) -> bool {
    let Some(reference) = node.payload.get("planningRef").and_then(Value::as_object) else {
        return false;
    };
    reference
        .get("entityType")
        .and_then(Value::as_str)
        .is_some_and(|value| value == entity_type.as_str())
        && reference
            .get("entityId")
            .and_then(Value::as_str)
            .is_some_and(|value| value == entity_id)
}

pub(crate) fn workflow_project_run_matches_source(
    run: &ProjectRunRecord,
    entity_type: EntityType,
    entity_id: &str,
) -> bool {
    match entity_type {
        EntityType::Goal => run.goal_id == entity_id,
        EntityType::Roadmap => run.roadmap_id == entity_id,
        EntityType::WorkPoint => run.work_point_id == entity_id,
        _ => true,
    }
}

pub(crate) fn workflow_parallel_safe_candidate_count(
    runnable: &crate::GraphRunnableResult,
    delegation_hints: &[crate::WorkflowDelegationHint],
    edges: &[PlanningGraphEdge],
) -> usize {
    if runnable.candidates.len() < 2 {
        return runnable.candidates.len();
    }

    let candidates = runnable
        .candidates
        .iter()
        .filter_map(|candidate| {
            delegation_hints
                .iter()
                .find(|hint| hint.node_id == candidate.node_id)
                .filter(|hint| !hint.file_scopes.is_empty())
                .map(|hint| (candidate.node_id.as_str(), hint))
        })
        .collect::<Vec<_>>();

    if candidates.len() != runnable.candidates.len() {
        return 0;
    }

    let has_parallel_edge = |left: &str, right: &str| {
        edges.iter().any(|edge| {
            edge.kind == crate::PlanningEdgeKind::ParallelSafeWith
                && edge.status == "active"
                && ((edge.source_node_id == left && edge.target_node_id == right)
                    || (edge.source_node_id == right && edge.target_node_id == left))
        })
    };

    for (index, (left_id, left_hint)) in candidates.iter().enumerate() {
        for (right_id, right_hint) in candidates.iter().skip(index + 1) {
            if !has_parallel_edge(left_id, right_id)
                || left_hint.file_scopes.iter().any(|left| {
                    right_hint
                        .file_scopes
                        .iter()
                        .any(|right| file_scopes_overlap(left, right))
                })
            {
                return 0;
            }
        }
    }

    candidates.len()
}

pub(crate) fn file_scopes_overlap(left: &FileScopeRecord, right: &FileScopeRecord) -> bool {
    let left_selector = left.selector.replace('\\', "/");
    let right_selector = right.selector.replace('\\', "/");

    if left.selector_type == FileScopeSelectorType::Exact
        && right.selector_type == FileScopeSelectorType::Exact
    {
        return left_selector == right_selector;
    }

    let left_prefix = workflow_literal_prefix(&left_selector);
    let right_prefix = workflow_literal_prefix(&right_selector);
    left_prefix.is_empty()
        || right_prefix.is_empty()
        || left_prefix.starts_with(right_prefix)
        || right_prefix.starts_with(left_prefix)
}

pub(crate) fn workflow_literal_prefix(selector: &str) -> &str {
    selector
        .find(['*', '?', '['])
        .map(|index| &selector[..index])
        .unwrap_or(selector)
}

pub(crate) fn project_run_status_counts(
    project_runs: &[ProjectRunRecord],
) -> crate::ProjectRunStatusCounts {
    let mut counts = crate::ProjectRunStatusCounts::default();
    for run in project_runs {
        match run.status {
            ProjectRunStatus::Suggested => counts.suggested += 1,
            ProjectRunStatus::Claimed => counts.claimed += 1,
            ProjectRunStatus::Active => counts.active += 1,
            ProjectRunStatus::Interrupted => counts.interrupted += 1,
            ProjectRunStatus::Completed => counts.completed += 1,
            ProjectRunStatus::Failed => counts.failed += 1,
            ProjectRunStatus::Cancelled => counts.cancelled += 1,
            ProjectRunStatus::Released => counts.released += 1,
        }
    }
    counts
}
