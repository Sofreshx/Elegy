use std::path::Path;

use crate::{
    CreateGraphEdgeInput, CreateGraphNodeInput, Manifest, ManifestEdge, ManifestNode,
    PlanningEdgeKind, PlanningNodeKind, PlanningStoreError,
};

/// Parsed manifest ready for application.
#[derive(Clone, Debug)]
pub struct ParsedManifest {
    pub scope: String,
    pub nodes: Vec<CreateGraphNodeInput>,
    pub edges: Vec<CreateGraphEdgeInput>,
}

/// Parse a manifest file (YAML or JSON).
pub fn parse_manifest_file(path: &Path) -> Result<Manifest, PlanningStoreError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        PlanningStoreError::InvalidInput(format!("failed to read manifest file: {e}"))
    })?;
    parse_manifest(&content, path)
}

/// Parse manifest content (YAML or JSON) into a Manifest struct.
pub fn parse_manifest(content: &str, path: &Path) -> Result<Manifest, PlanningStoreError> {
    let manifest: Manifest = if path.extension().map(|e| e == "json").unwrap_or(false) {
        serde_json::from_str(content)
            .map_err(|e| PlanningStoreError::InvalidInput(format!("invalid JSON manifest: {e}")))?
    } else {
        serde_yaml::from_str(content)
            .map_err(|e| PlanningStoreError::InvalidInput(format!("invalid YAML manifest: {e}")))?
    };
    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// Validate manifest-internal consistency.
fn validate_manifest(manifest: &Manifest) -> Result<(), PlanningStoreError> {
    if manifest.scope.trim().is_empty() {
        return Err(PlanningStoreError::InvalidInput(
            "manifest scope must not be empty".to_string(),
        ));
    }

    // Collect all node IDs declared in the manifest
    let mut declared_ids = std::collections::HashSet::new();
    for node in &manifest.nodes {
        if let Some(ref id) = node.id {
            if !declared_ids.insert(id.clone()) {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "duplicate node ID `{id}` in manifest"
                )));
            }
        }
    }

    // Validate required fields per node
    for node in &manifest.nodes {
        if node.title.trim().is_empty() {
            return Err(PlanningStoreError::InvalidInput(format!(
                "node {:?} has empty title",
                node.id
            )));
        }
        if node.summary.trim().is_empty() {
            return Err(PlanningStoreError::InvalidInput(format!(
                "node {:?} has empty summary",
                node.id
            )));
        }
        if node.status.trim().is_empty() {
            return Err(PlanningStoreError::InvalidInput(format!(
                "node {:?} has empty status",
                node.id
            )));
        }
    }

    // Validate edge cross-references within manifest
    for edge in &manifest.edges {
        if edge.source_node_id.trim().is_empty() {
            return Err(PlanningStoreError::InvalidInput(
                "edge has empty source_node_id".to_string(),
            ));
        }
        if edge.target_node_id.trim().is_empty() {
            return Err(PlanningStoreError::InvalidInput(
                "edge has empty target_node_id".to_string(),
            ));
        }
        if edge.source_node_id == edge.target_node_id {
            return Err(PlanningStoreError::InvalidInput(format!(
                "self-referential {} edge from `{}` to itself is not allowed",
                edge.kind.as_str(),
                edge.source_node_id
            )));
        }
    }

    // Validate shorthand fields reference declared IDs
    for node in &manifest.nodes {
        for dep_id in &node.depends_on {
            if !declared_ids.contains(dep_id) {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "node {:?} depends_on references undeclared ID `{}`",
                    node.id, dep_id
                )));
            }
        }
        for block_id in &node.blocks {
            if !declared_ids.contains(block_id) {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "node {:?} blocks references undeclared ID `{}`",
                    node.id, block_id
                )));
            }
        }
        for decomp_id in &node.decomposes_to {
            if !declared_ids.contains(decomp_id) {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "node {:?} decomposes_to references undeclared ID `{}`",
                    node.id, decomp_id
                )));
            }
        }
        for plan_id in &node.planned_by {
            if !declared_ids.contains(plan_id) {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "node {:?} planned_by references undeclared ID `{}`",
                    node.id, plan_id
                )));
            }
        }
        for work_id in &node.targeted_work {
            if !declared_ids.contains(work_id) {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "node {:?} targeted_work references undeclared ID `{}`",
                    node.id, work_id
                )));
            }
        }
        for repair_id in &node.repairs {
            if !declared_ids.contains(repair_id) {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "node {:?} repairs references undeclared ID `{}`",
                    node.id, repair_id
                )));
            }
        }
        for supersede_id in &node.supersedes {
            if !declared_ids.contains(supersede_id) {
                return Err(PlanningStoreError::InvalidInput(format!(
                    "node {:?} supersedes references undeclared ID `{}`",
                    node.id, supersede_id
                )));
            }
        }
    }

    Ok(())
}

/// Convert a Manifest into a ParsedManifest by expanding shorthand edges and building
/// CreateGraphNodeInput / CreateGraphEdgeInput structs.
pub fn expand_manifest(manifest: Manifest, correlation_id: &str) -> ParsedManifest {
    let scope = manifest.scope.trim().to_string();
    let mut nodes: Vec<CreateGraphNodeInput> = Vec::new();
    let mut edges: Vec<CreateGraphEdgeInput> = Vec::new();

    // -- Expand nodes --
    for node in &manifest.nodes {
        let payload = build_node_payload(node);
        let node_input = CreateGraphNodeInput {
            id: node.id.clone(),
            scope_key: Some(scope.clone()),
            correlation_id: correlation_id.to_string(),
            kind: node.kind,
            title: node.title.clone(),
            summary: node.summary.clone(),
            status: node.status.clone(),
            payload,
            tags: node.tags.clone(),
            run_id: None,
        };
        nodes.push(node_input);
    }

    // -- Expand shorthand edges from nodes --
    let mut expanded_edges: Vec<ManifestEdge> = Vec::new();
    for node in &manifest.nodes {
        let source_id = node
            .id
            .clone()
            .unwrap_or_else(|| format!("<auto:{}>", node.title));

        // depends_on → depends-on edges FROM this node TO each listed ID
        for target_id in &node.depends_on {
            expanded_edges.push(shorthand_edge(
                &source_id,
                target_id,
                PlanningEdgeKind::DependsOn,
            ));
        }

        // blocks → blocks edges
        for target_id in &node.blocks {
            expanded_edges.push(shorthand_edge(
                &source_id,
                target_id,
                PlanningEdgeKind::Blocks,
            ));
        }

        // decomposes_to → decomposes-to edges
        for target_id in &node.decomposes_to {
            expanded_edges.push(shorthand_edge(
                &source_id,
                target_id,
                PlanningEdgeKind::DecomposesTo,
            ));
        }

        // planned_by → planned-by edges FROM this node TO each plan ID
        for plan_id in &node.planned_by {
            expanded_edges.push(shorthand_edge(
                &source_id,
                plan_id,
                PlanningEdgeKind::PlannedBy,
            ));
        }

        // targeted_work (on plan nodes) → planned-by edges FROM each work ID TO this plan
        for work_id in &node.targeted_work {
            expanded_edges.push(shorthand_edge(
                work_id,
                &source_id,
                PlanningEdgeKind::PlannedBy,
            ));
        }

        // repairs → repairs edges
        for target_id in &node.repairs {
            expanded_edges.push(shorthand_edge(
                &source_id,
                target_id,
                PlanningEdgeKind::Repairs,
            ));
        }

        // supersedes → supersedes edges
        for target_id in &node.supersedes {
            expanded_edges.push(shorthand_edge(
                &source_id,
                target_id,
                PlanningEdgeKind::Supersedes,
            ));
        }
    }

    // -- Deduplicate shorthand edges: same (source, target, kind) → keep first --
    let mut seen_edges = std::collections::HashSet::new();
    let mut deduped_edges: Vec<ManifestEdge> = Vec::new();
    for edge in expanded_edges {
        let key = (
            edge.source_node_id.clone(),
            edge.target_node_id.clone(),
            edge.kind.as_str().to_string(),
        );
        if seen_edges.insert(key) {
            deduped_edges.push(edge);
        }
    }

    // -- Convert explicit manifest edges --
    for edge in &manifest.edges {
        let key = (
            edge.source_node_id.clone(),
            edge.target_node_id.clone(),
            edge.kind.as_str().to_string(),
        );
        // Skip if a shorthand edge already covers this (explicit self-dupes are caught in validation)
        if !seen_edges.insert(key) {
            continue;
        }
        deduped_edges.push(edge.clone());
    }

    // -- Convert all edges to CreateGraphEdgeInput --
    for edge in &deduped_edges {
        let edge_input = CreateGraphEdgeInput {
            id: edge.id.clone(),
            scope_key: Some(scope.clone()),
            correlation_id: correlation_id.to_string(),
            kind: edge.kind,
            source_node_id: edge.source_node_id.clone(),
            target_node_id: edge.target_node_id.clone(),
            status: edge.status.clone(),
            payload: edge.payload.clone(),
            run_id: None,
        };
        edges.push(edge_input);
    }

    ParsedManifest {
        scope,
        nodes,
        edges,
    }
}

/// Build typed payload JSON from manifest node fields based on node kind.
fn build_node_payload(node: &ManifestNode) -> serde_json::Value {
    match node.kind {
        PlanningNodeKind::Acceptance => {
            let mut payload = serde_json::json!({
                "acceptanceKind": node.acceptance_kind.as_deref().unwrap_or("abstract"),
                "description": node.description.as_deref().unwrap_or(""),
                "verificationPolicy": node.verification_policy.as_deref().unwrap_or("manual-review"),
                "requiredEvidenceKinds": node.required_evidence_kinds,
            });
            if let Some(ref waiver) = node.waiver {
                payload["waiver"] = serde_json::json!(waiver);
            }
            payload
        }
        PlanningNodeKind::Evidence => serde_json::json!({
            "evidenceKind": node.evidence_kind.as_deref().unwrap_or("artifact-ref"),
            "summary": node.summary,
            "reference": node.reference.as_deref().unwrap_or(""),
            "content": node.content.as_deref().unwrap_or(""),
            "capturedAt": node.captured_at.as_deref().unwrap_or(""),
        }),
        _ => {
            if node.payload.is_null()
                || node
                    .payload
                    .as_object()
                    .map(|o| o.is_empty())
                    .unwrap_or(true)
            {
                // No custom payload — build one from known node-kind fields
                let mut payload = serde_json::json!({});
                if node.acceptance_kind.is_some()
                    || node.description.is_some()
                    || node.verification_policy.is_some()
                {
                    // These fields belong on acceptance nodes, but if set on other kinds, include them
                }
                if let Some(ref ek) = node.evidence_kind {
                    payload["evidenceKind"] = serde_json::json!(ek);
                }
                payload
            } else {
                node.payload.clone()
            }
        }
    }
}

/// Build a shorthand edge with a deterministic stable ID: `sh-{source}-{kind}-{target}`.
fn shorthand_edge(source: &str, target: &str, kind: PlanningEdgeKind) -> ManifestEdge {
    let id = format!("sh-{}-{}-{}", source, kind.as_str(), target);
    ManifestEdge {
        id: Some(id),
        kind,
        source_node_id: source.to_string(),
        target_node_id: target.to_string(),
        status: "active".to_string(),
        payload: serde_json::json!({}),
    }
}
