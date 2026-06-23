use std::path::Path;

use crate::{
    Manifest, ManifestEdge, ManifestNode, PlanningEdgeKind, PlanningIntent, PlanningNodeKind,
    PlanningStoreError,
};

/// Parse an intent file and expand it to a manifest YAML string.
pub fn expand_intent_file(path: &Path) -> Result<String, PlanningStoreError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        PlanningStoreError::InvalidInput(format!("failed to read intent file: {e}"))
    })?;
    let intent: PlanningIntent = if path.extension().map(|e| e == "json").unwrap_or(false) {
        serde_json::from_str(&content)
            .map_err(|e| PlanningStoreError::InvalidInput(format!("invalid JSON intent: {e}")))?
    } else {
        serde_yaml::from_str(&content)
            .map_err(|e| PlanningStoreError::InvalidInput(format!("invalid YAML intent: {e}")))?
    };
    expand_intent(&intent)
}

/// Expand a PlanningIntent into a manifest YAML string.
fn expand_intent(intent: &PlanningIntent) -> Result<String, PlanningStoreError> {
    let scope = intent.scope.trim();
    if scope.is_empty() {
        return Err(PlanningStoreError::InvalidInput(
            "intent scope must not be empty".to_string(),
        ));
    }

    let title_slug = slugify(&intent.intent, 40);

    let mut nodes: Vec<ManifestNode> = Vec::new();
    let mut edges: Vec<ManifestEdge> = Vec::new();

    let goal_id = format!("g-{title_slug}");
    let mut deliverable_ids = Vec::new();
    let mut constraint_ids = Vec::new();
    let mut verification_ids = Vec::new();

    // Goal node
    nodes.push(ManifestNode {
        id: Some(goal_id.clone()),
        kind: PlanningNodeKind::Goal,
        title: intent.intent.clone(),
        summary: intent.intent.clone(),
        status: "active".to_string(),
        payload: serde_json::json!({
            "acceptanceCriteria": intent.verification,
            "rejectionCriteria": [],
        }),
        tags: Vec::new(),
        depends_on: Vec::new(),
        blocks: Vec::new(),
        decomposes_to: Vec::new(),
        planned_by: Vec::new(),
        targeted_work: Vec::new(),
        repairs: Vec::new(),
        supersedes: Vec::new(),
        acceptance_kind: None,
        description: None,
        verification_policy: None,
        required_evidence_kinds: Vec::new(),
        waiver: None,
        evidence_kind: None,
        reference: None,
        content: None,
        captured_at: None,
    });

    // Constraints → acceptance nodes
    for (i, constraint) in intent.constraints.iter().enumerate() {
        let cid = format!("ac-constraint-{title_slug}-{i}");
        nodes.push(ManifestNode {
            id: Some(cid.clone()),
            kind: PlanningNodeKind::Acceptance,
            title: constraint.clone(),
            summary: format!("Constraint: {constraint}"),
            status: "active".to_string(),
            acceptance_kind: Some("abstract".to_string()),
            description: Some(constraint.clone()),
            verification_policy: Some("manual-review".to_string()),
            required_evidence_kinds: Vec::new(),
            waiver: None,
            payload: serde_json::json!({}),
            tags: vec!["constraint".to_string()],
            depends_on: Vec::new(),
            blocks: Vec::new(),
            decomposes_to: Vec::new(),
            planned_by: Vec::new(),
            targeted_work: Vec::new(),
            repairs: Vec::new(),
            supersedes: Vec::new(),
            evidence_kind: None,
            reference: None,
            content: None,
            captured_at: None,
        });
        edges.push(ManifestEdge {
            id: Some(format!("er-goal-constraint-{title_slug}-{i}")),
            kind: PlanningEdgeKind::Requires,
            source_node_id: goal_id.clone(),
            target_node_id: cid.clone(),
            status: "active".to_string(),
            payload: serde_json::json!({}),
        });
        constraint_ids.push(cid);
    }

    // Deliverables → work nodes
    for (i, deliverable) in intent.deliverables.iter().enumerate() {
        let did = format!("wp-{}-{}", slugify(deliverable, 30), i);
        nodes.push(ManifestNode {
            id: Some(did.clone()),
            kind: PlanningNodeKind::Work,
            title: deliverable.clone(),
            summary: deliverable.clone(),
            status: "proposed".to_string(),
            payload: serde_json::json!({}),
            tags: vec!["deliverable".to_string()],
            depends_on: Vec::new(),
            blocks: Vec::new(),
            decomposes_to: Vec::new(),
            planned_by: Vec::new(),
            targeted_work: Vec::new(),
            repairs: Vec::new(),
            supersedes: Vec::new(),
            acceptance_kind: None,
            description: None,
            verification_policy: None,
            required_evidence_kinds: Vec::new(),
            waiver: None,
            evidence_kind: None,
            reference: None,
            content: None,
            captured_at: None,
        });
        edges.push(ManifestEdge {
            id: Some(format!("ed-goal-deliverable-{title_slug}-{i}")),
            kind: PlanningEdgeKind::DecomposesTo,
            source_node_id: goal_id.clone(),
            target_node_id: did.clone(),
            status: "active".to_string(),
            payload: serde_json::json!({}),
        });
        deliverable_ids.push(did);
    }

    // Verification → acceptance nodes (concrete)
    for (i, verification) in intent.verification.iter().enumerate() {
        let vid = format!("ac-verify-{title_slug}-{i}");
        nodes.push(ManifestNode {
            id: Some(vid.clone()),
            kind: PlanningNodeKind::Acceptance,
            title: verification.clone(),
            summary: format!("Verification: {verification}"),
            status: "active".to_string(),
            acceptance_kind: Some("concrete".to_string()),
            description: Some(verification.clone()),
            verification_policy: Some("automated-ci".to_string()),
            required_evidence_kinds: vec!["test-result".to_string()],
            waiver: None,
            payload: serde_json::json!({}),
            tags: vec!["verification".to_string()],
            depends_on: Vec::new(),
            blocks: Vec::new(),
            decomposes_to: Vec::new(),
            planned_by: Vec::new(),
            targeted_work: Vec::new(),
            repairs: Vec::new(),
            supersedes: Vec::new(),
            evidence_kind: None,
            reference: None,
            content: None,
            captured_at: None,
        });
        // Link each deliverable to verification
        for (j, did) in deliverable_ids.iter().enumerate() {
            edges.push(ManifestEdge {
                id: Some(format!("er-verify-{title_slug}-{i}-{j}")),
                kind: PlanningEdgeKind::Requires,
                source_node_id: did.clone(),
                target_node_id: vid.clone(),
                status: "active".to_string(),
                payload: serde_json::json!({}),
            });
        }
        verification_ids.push(vid);
    }

    // Dependencies → added as comments in the manifest (since they're not edges)
    let dep_comments: Vec<String> = intent
        .dependencies
        .iter()
        .map(|d| {
            format!(
                "# Dependency ({kind}): {desc}",
                kind = d.kind,
                desc = d.description
            )
        })
        .collect();

    // Non-goals → also as comments
    let non_goal_comments: Vec<String> = intent
        .non_goals
        .iter()
        .map(|ng| format!("# Non-goal: {ng}"))
        .collect();

    // Build the manifest
    let manifest = Manifest {
        schema_version: "planning-manifest/v1".to_string(),
        scope: scope.to_string(),
        nodes,
        edges,
    };

    let mut yaml = serde_yaml::to_string(&manifest).map_err(|e| {
        PlanningStoreError::InvalidInput(format!("failed to serialize manifest: {e}"))
    })?;

    // Prepend comments about dependencies and non-goals
    if !dep_comments.is_empty() || !non_goal_comments.is_empty() {
        let mut header = String::from("# Generated from planning intent\n");
        header.push_str("# Fill in <PLACEHOLDERS>, remove unused nodes/edges, then apply with:\n");
        header.push_str("#   elegy-planning manifest apply --dry-run --file plan.yaml\n");
        header.push_str("#   elegy-planning manifest apply --file plan.yaml\n");
        header.push('\n');
        for c in &dep_comments {
            header.push_str(c);
            header.push('\n');
        }
        for c in &non_goal_comments {
            header.push_str(c);
            header.push('\n');
        }
        header.push('\n');
        yaml = format!("{header}{yaml}");
    }

    Ok(yaml)
}

/// Generate a kebab-case slug from text, truncated to max_len.
fn slugify(text: &str, max_len: usize) -> String {
    let slug: String = text
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-');
    if slug.len() <= max_len {
        slug.to_string()
    } else {
        slug[..max_len].trim_end_matches('-').to_string()
    }
}
