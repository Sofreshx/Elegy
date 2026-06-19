use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiagramError {
    #[error("failed to parse diagram JSON: {source}")]
    Json {
        #[source]
        source: serde_json::Error,
    },
    #[error("duplicate node ID: {id}")]
    DuplicateNodeId { id: String },
    #[error("duplicate edge ID: {id}")]
    DuplicateEdgeId { id: String },
    #[error("edge `{edge_id}` references undeclared target node `{node_id}`")]
    InvalidTargetReference { edge_id: String, node_id: String },
    #[error("edge `{edge_id}` references undeclared source node `{node_id}`")]
    InvalidSourceReference { edge_id: String, node_id: String },
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalDiagram {
    pub diagram_type: String, // e.g., "architecture", "concept", "mindmap"
    pub version: u64,
    pub nodes: Vec<DiagramNode>,
    pub edges: Vec<DiagramEdge>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub groups: Vec<DiagramGroup>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagramNode {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concept_type: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub properties: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagramEdge {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_type: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub properties: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagramGroup {
    pub id: String,
    pub label: String,
    pub node_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiagramPatch {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub add_nodes: Vec<DiagramNode>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub remove_node_ids: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub add_edges: Vec<DiagramEdge>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub remove_edge_ids: Vec<String>,
}

impl CanonicalDiagram {
    pub fn validate(&self) -> Result<(), DiagramError> {
        let mut node_ids = std::collections::BTreeSet::new();
        for node in &self.nodes {
            if !node_ids.insert(&node.id) {
                return Err(DiagramError::DuplicateNodeId {
                    id: node.id.clone(),
                });
            }
        }

        let mut edge_ids = std::collections::BTreeSet::new();
        for edge in &self.edges {
            if !edge_ids.insert(&edge.id) {
                return Err(DiagramError::DuplicateEdgeId {
                    id: edge.id.clone(),
                });
            }
            if !node_ids.contains(&edge.source_id) {
                return Err(DiagramError::InvalidSourceReference {
                    edge_id: edge.id.clone(),
                    node_id: edge.source_id.clone(),
                });
            }
            if !node_ids.contains(&edge.target_id) {
                return Err(DiagramError::InvalidTargetReference {
                    edge_id: edge.id.clone(),
                    node_id: edge.target_id.clone(),
                });
            }
        }
        Ok(())
    }

    pub fn apply_patch(&mut self, patch: DiagramPatch) {
        // Remove nodes
        for id in &patch.remove_node_ids {
            self.nodes.retain(|n| &n.id != id);
            // Cascading remove of connected edges
            self.edges
                .retain(|e| &e.source_id != id && &e.target_id != id);
        }

        // Add/Update nodes
        for node in patch.add_nodes {
            if let Some(existing) = self.nodes.iter_mut().find(|n| n.id == node.id) {
                *existing = node;
            } else {
                self.nodes.push(node);
            }
        }

        // Remove edges
        for id in &patch.remove_edge_ids {
            self.edges.retain(|e| &e.id != id);
        }

        // Add/Update edges
        for edge in patch.add_edges {
            if let Some(existing) = self.edges.iter_mut().find(|e| e.id == edge.id) {
                *existing = edge;
            } else {
                self.edges.push(edge);
            }
        }

        // Clean up groups avoiding removed nodes
        let current_node_ids: std::collections::HashSet<_> =
            self.nodes.iter().map(|n| &n.id).collect();
        for group in &mut self.groups {
            group.node_ids.retain(|id| current_node_ids.contains(&id));
        }
    }

    pub fn render_mermaid(&self) -> String {
        let mut out = String::from("flowchart TD\n");
        for node in &self.nodes {
            out.push_str(&format!("    {}[{}]\n", node.id, node.label));
        }
        for edge in &self.edges {
            if let Some(label) = &edge.label {
                out.push_str(&format!(
                    "    {} -- \"{}\" --> {}\n",
                    edge.source_id, label, edge.target_id
                ));
            } else {
                out.push_str(&format!("    {} --> {}\n", edge.source_id, edge.target_id));
            }
        }
        out
    }

    pub fn narrate_diagram(&self) -> String {
        let mut out = format!(
            "The diagram is of type '{}' with {} nodes and {} edges.\n",
            self.diagram_type,
            self.nodes.len(),
            self.edges.len()
        );

        let node_map: std::collections::HashMap<_, _> =
            self.nodes.iter().map(|n| (&n.id, &n.label)).collect();

        if self.nodes.is_empty() {
            out.push_str("The diagram is currently empty.\n");
            return out;
        }

        out.push_str("Nodes:\n");
        for node in &self.nodes {
            out.push_str(&format!("- ID: {}, Label: '{}'\n", node.id, node.label));
        }

        if !self.edges.is_empty() {
            out.push_str("Connections:\n");
            for edge in &self.edges {
                let source_label = node_map
                    .get(&edge.source_id)
                    .copied()
                    .unwrap_or(&edge.source_id);
                let target_label = node_map
                    .get(&edge.target_id)
                    .copied()
                    .unwrap_or(&edge.target_id);
                if let Some(label) = &edge.label {
                    out.push_str(&format!("- '{source_label}' connects to '{target_label}' via relationship '{label}'\n"));
                } else {
                    out.push_str(&format!(
                        "- '{source_label}' connects to '{target_label}'\n"
                    ));
                }
            }
        }

        out
    }
}
