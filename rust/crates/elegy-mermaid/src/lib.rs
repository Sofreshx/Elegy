use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const MERMAID_FLOWCHART_DIRECTION: &str = "flowchart TD";
pub const MERMAID_PROJECTION_KIND: &str = "workflow-graph-semantics";
pub const MERMAID_NARRATIVE_POSTURE: &str =
    "derived Mermaid projection only; canonical workflow authority remains outside Mermaid";
const MERMAID_EDGE_LABEL_PIPE_ESCAPE: &str = "&#124;";
const MERMAID_EDGE_LABEL_AMP_ESCAPE: &str = "&amp;";

#[derive(Debug, Error)]
pub enum MermaidToolError {
    #[error("failed to parse canonical JSON: {source}")]
    Json {
        #[source]
        source: serde_json::Error,
    },
    #[error("unsupported canonical workflow document")]
    UnsupportedCanonicalDocument,
    #[error("failed to parse canonical workflow graph: {source}")]
    CanonicalWorkflowGraph {
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to parse canonical workflow: {source}")]
    CanonicalWorkflow {
        #[source]
        source: serde_json::Error,
    },
    #[error("canonical workflow field `{field}` declares duplicate id `{id}`")]
    DuplicateCanonicalWorkflowId { field: &'static str, id: String },
    #[error("canonical workflow reference `{field}` targets undeclared step `{step_id}`")]
    InvalidCanonicalWorkflowReference {
        field: &'static str,
        step_id: String,
    },
    #[error("canonical workflow graph field `{field}` declares duplicate id `{id}`")]
    DuplicateCanonicalWorkflowGraphId { field: &'static str, id: String },
    #[error("canonical workflow graph reference `{field}` targets undeclared node `{node_id}`")]
    InvalidCanonicalWorkflowGraphReference {
        field: &'static str,
        node_id: String,
    },
    #[error("unsupported Mermaid diagram; expected `{MERMAID_FLOWCHART_DIRECTION}`")]
    UnsupportedMermaidDocument,
    #[error("unsupported Mermaid directive on line {line}: `{directive}`; expected `{MERMAID_FLOWCHART_DIRECTION}`")]
    UnsupportedMermaidDirective { line: usize, directive: String },
    #[error("invalid Mermaid line {line}: {message}")]
    InvalidMermaidLine { line: usize, message: String },
    #[error("Mermaid node `{node_id}` is declared more than once")]
    DuplicateMermaidNodeId { node_id: String },
    #[error("Mermaid edge `{field}` targets undeclared node `{node_id}`")]
    InvalidMermaidReference {
        field: &'static str,
        node_id: String,
    },
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MermaidProjectionSourceKind {
    CanonicalWorkflow,
    CanonicalWorkflowGraph,
    MermaidFlowchartTd,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MermaidProjectionNodeRole {
    Activity,
    Trigger,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MermaidProjectionNodeSourceKind {
    WorkflowStep,
    WorkflowGraphNode,
    WorkflowTrigger,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum MermaidProjectionEdgeRelation {
    Activates,
    TransitionsTo,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MermaidWorkflowProjection {
    pub projection_kind: &'static str,
    pub source_kind: MermaidProjectionSourceKind,
    pub direction: &'static str,
    pub nodes: Vec<MermaidProjectionNode>,
    pub edges: Vec<MermaidProjectionEdge>,
    pub entry_node_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MermaidProjectionNode {
    pub node_id: String,
    pub label: String,
    pub node_role: MermaidProjectionNodeRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<MermaidProjectionNodeSourceKind>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MermaidProjectionEdge {
    pub from_node_id: String,
    pub to_node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub relation: MermaidProjectionEdgeRelation,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MermaidNarrative {
    pub source_kind: MermaidProjectionSourceKind,
    pub posture: &'static str,
    pub sentences: Vec<String>,
    pub text: String,
}

pub fn render_from_json_str(input: &str) -> Result<String, MermaidToolError> {
    let projection = project_from_json_str(input)?;
    Ok(render_projection(&projection))
}

pub fn render_from_json_value(value: &Value) -> Result<String, MermaidToolError> {
    let projection = project_from_json_value(value)?;
    Ok(render_projection(&projection))
}

pub fn project_from_json_str(input: &str) -> Result<MermaidWorkflowProjection, MermaidToolError> {
    let value =
        serde_json::from_str::<Value>(input).map_err(|source| MermaidToolError::Json { source })?;
    project_from_json_value(&value)
}

pub fn project_from_json_value(
    value: &Value,
) -> Result<MermaidWorkflowProjection, MermaidToolError> {
    match detect_document_kind(value)? {
        DocumentKind::CanonicalWorkflowGraph => {
            let document = serde_json::from_value::<CanonicalWorkflowGraph>(value.clone())
                .map_err(|source| MermaidToolError::CanonicalWorkflowGraph { source })?;
            validate_canonical_workflow_graph(&document)?;
            Ok(build_canonical_workflow_graph_projection(document))
        }
        DocumentKind::CanonicalWorkflow => {
            let document = serde_json::from_value::<CanonicalWorkflow>(value.clone())
                .map_err(|source| MermaidToolError::CanonicalWorkflow { source })?;
            validate_canonical_workflow(&document)?;
            Ok(build_canonical_workflow_projection(document))
        }
    }
}

pub fn reverse_from_mermaid_str(
    input: &str,
) -> Result<MermaidWorkflowProjection, MermaidToolError> {
    parse_mermaid_flowchart(input)
}

pub fn narrate_from_json_str(
    input: &str,
) -> Result<(MermaidNarrative, MermaidWorkflowProjection), MermaidToolError> {
    let projection = project_from_json_str(input)?;
    let narrative = narrate_projection(&projection);
    Ok((narrative, projection))
}

pub fn narrate_from_json_value(
    value: &Value,
) -> Result<(MermaidNarrative, MermaidWorkflowProjection), MermaidToolError> {
    let projection = project_from_json_value(value)?;
    let narrative = narrate_projection(&projection);
    Ok((narrative, projection))
}

pub fn narrate_from_mermaid_str(
    input: &str,
) -> Result<(MermaidNarrative, MermaidWorkflowProjection), MermaidToolError> {
    let projection = reverse_from_mermaid_str(input)?;
    let narrative = narrate_projection(&projection);
    Ok((narrative, projection))
}

pub fn narrate_projection(projection: &MermaidWorkflowProjection) -> MermaidNarrative {
    let node_lookup = projection
        .nodes
        .iter()
        .map(|node| (node.node_id.as_str(), node))
        .collect::<BTreeMap<_, _>>();

    let mut sentences = vec![MERMAID_NARRATIVE_POSTURE.to_string()];

    let activation_edges = projection
        .edges
        .iter()
        .filter(|edge| edge.relation == MermaidProjectionEdgeRelation::Activates)
        .collect::<Vec<_>>();
    let transition_edges = projection
        .edges
        .iter()
        .filter(|edge| edge.relation == MermaidProjectionEdgeRelation::TransitionsTo)
        .collect::<Vec<_>>();

    if activation_edges.is_empty() && transition_edges.is_empty() {
        let activity_nodes = projection
            .nodes
            .iter()
            .filter(|node| node.node_role == MermaidProjectionNodeRole::Activity)
            .collect::<Vec<_>>();

        match activity_nodes.as_slice() {
            [] => sentences.push("The projection does not contain any workflow nodes.".to_string()),
            [node] => sentences.push(format!(
                "The flow contains a single activity: {}.",
                sentence_label(&node.label)
            )),
            _ => sentences.push(format!(
                "The flow contains {} activities with no explicit transitions.",
                activity_nodes.len()
            )),
        }
    }

    for edge in activation_edges {
        let from_label = node_lookup
            .get(edge.from_node_id.as_str())
            .map(|node| sentence_label(&node.label))
            .unwrap_or_else(|| edge.from_node_id.clone());
        let to_label = node_lookup
            .get(edge.to_node_id.as_str())
            .map(|node| sentence_label(&node.label))
            .unwrap_or_else(|| edge.to_node_id.clone());
        sentences.push(format!("{from_label} activates {to_label}."));
    }

    for edge in transition_edges {
        let from_label = node_lookup
            .get(edge.from_node_id.as_str())
            .map(|node| sentence_label(&node.label))
            .unwrap_or_else(|| edge.from_node_id.clone());
        let to_label = node_lookup
            .get(edge.to_node_id.as_str())
            .map(|node| sentence_label(&node.label))
            .unwrap_or_else(|| edge.to_node_id.clone());

        match edge
            .label
            .as_deref()
            .filter(|label| !label.trim().is_empty())
        {
            Some(label) => sentences.push(format!(
                "{from_label} transitions to {to_label} when {}.",
                sentence_label(label)
            )),
            None => sentences.push(format!("{from_label} transitions to {to_label}.")),
        }
    }

    let text = sentences.join(" ");
    MermaidNarrative {
        source_kind: projection.source_kind,
        posture: MERMAID_NARRATIVE_POSTURE,
        sentences,
        text,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DocumentKind {
    CanonicalWorkflowGraph,
    CanonicalWorkflow,
}

fn detect_document_kind(value: &Value) -> Result<DocumentKind, MermaidToolError> {
    let Some(object) = value.as_object() else {
        return Err(MermaidToolError::UnsupportedCanonicalDocument);
    };

    if value
        .get("canonicalFormat")
        .and_then(Value::as_str)
        .is_some_and(|format| format == "canonical-workflow-graph")
    {
        return Ok(DocumentKind::CanonicalWorkflowGraph);
    }

    let graph_keys = [
        "canonicalFormat",
        "canonicalVersion",
        "trigger",
        "entryStepId",
        "nodes",
        "edges",
        "variables",
    ];
    if graph_keys.iter().any(|key| object.contains_key(*key)) {
        return Ok(DocumentKind::CanonicalWorkflowGraph);
    }

    let workflow_keys = [
        "specVersion",
        "canonicalAuthority",
        "conflictPolicy",
        "blueprint",
        "triggers",
        "steps",
        "connections",
        "layout",
    ];
    if workflow_keys.iter().any(|key| object.contains_key(*key)) {
        return Ok(DocumentKind::CanonicalWorkflow);
    }

    Err(MermaidToolError::UnsupportedCanonicalDocument)
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CanonicalWorkflow {
    id: String,
    name: String,
    #[serde(deserialize_with = "deserialize_workflow_spec_version")]
    spec_version: String,
    canonical_authority: CanonicalAuthority,
    conflict_policy: ConflictPolicy,
    blueprint: BlueprintMetadata,
    triggers: Vec<WorkflowTrigger>,
    steps: Vec<WorkflowStep>,
    connections: Vec<WorkflowConnection>,
    layout: WorkflowLayout,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum CanonicalAuthority {
    Blueprint,
    Tenant,
    Runtime,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ConflictPolicy {
    Reject,
    Reconcile,
    Override,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BlueprintMetadata {
    blueprint_id: String,
    version: String,
    is_pinned: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkflowTrigger {
    id: String,
    name: String,
    #[serde(rename = "type")]
    trigger_type: String,
    target_step_id: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkflowStep {
    id: String,
    name: String,
    #[serde(rename = "type")]
    step_type: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkflowConnection {
    id: String,
    from_step_id: String,
    to_step_id: String,
    label: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkflowLayout {
    groups: Vec<WorkflowGroupLayout>,
    positions: Vec<WorkflowStepPosition>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkflowGroupLayout {
    id: String,
    name: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkflowStepPosition {
    step_id: String,
    x: f64,
    y: f64,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CanonicalWorkflowGraph {
    canonical_format: CanonicalWorkflowGraphFormat,
    #[serde(deserialize_with = "deserialize_canonical_workflow_graph_version")]
    canonical_version: u64,
    entry_step_id: Option<String>,
    trigger: Option<WorkflowGraphTrigger>,
    nodes: Vec<WorkflowGraphNode>,
    edges: Vec<WorkflowGraphEdge>,
    variables: BTreeMap<String, WorkflowVariable>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
enum CanonicalWorkflowGraphFormat {
    #[serde(rename = "canonical-workflow-graph")]
    CanonicalWorkflowGraph,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkflowGraphTrigger {
    #[serde(rename = "type")]
    trigger_type: String,
    cron_expression: Option<String>,
    timezone: Option<String>,
    event_type: Option<String>,
    input_schema: Vec<PortDefinition>,
}

impl WorkflowGraphTrigger {
    fn label(&self) -> &str {
        self.event_type
            .as_deref()
            .unwrap_or(self.trigger_type.as_str())
    }

    fn synthetic_id_seed(&self) -> &str {
        self.event_type
            .as_deref()
            .unwrap_or(self.trigger_type.as_str())
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PortDefinition {
    name: String,
    label: Option<String>,
    description: Option<String>,
    type_descriptor: Option<Value>,
    data_type: String,
    required: bool,
    default_value: Option<Value>,
    allow_multiple: bool,
    schema: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkflowVariable {
    name: String,
    description: Option<String>,
    data_type: String,
    default_value: Option<Value>,
    is_secret: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkflowGraphNode {
    id: String,
    name: String,
    #[serde(rename = "type")]
    node_type: String,
    description: Option<String>,
    piece_id: Option<String>,
    piece_type: Option<String>,
    tool_id: Option<String>,
    addon_version: Option<i64>,
    inputs: Vec<PortDefinition>,
    outputs: Vec<PortDefinition>,
    config: BTreeMap<String, Value>,
    input_mappings: BTreeMap<String, String>,
    input_resolutions: BTreeMap<String, InputResolution>,
    on_failure: String,
    max_retries: i64,
    retry_delay_seconds: i64,
    retry_config: Option<RetryConfig>,
    timeout_seconds: i64,
    condition: Option<String>,
    rollback_tool_id: Option<String>,
    schedule: Option<ScheduleConfig>,
    human_review: Option<HumanReviewConfig>,
    persist_output: bool,
    is_enabled: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkflowGraphEdge {
    from_step_id: String,
    from_port: String,
    to_step_id: String,
    to_port: String,
    transform: Option<ConnectionTransform>,
    condition: Option<String>,
    label: Option<String>,
    priority: i64,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InputResolution {
    source_expression: Option<String>,
    static_value: Option<Value>,
    transform: Option<ConnectionTransform>,
    default_value: Option<Value>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConnectionTransform {
    #[serde(rename = "type")]
    transform_type: String,
    template: Option<String>,
    lookup_table: Option<BTreeMap<String, String>>,
    target_type: Option<Value>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetryConfig {
    max_retries: i64,
    initial_delay: String,
    max_delay: String,
    backoff_multiplier: f64,
    retryable_error_codes: Option<Vec<String>>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScheduleConfig {
    kind: Option<String>,
    delay_seconds: Option<i64>,
    execute_at: Option<String>,
    cron_expression: Option<String>,
    interval_value: Option<i64>,
    interval_unit: Option<String>,
    start_at: Option<String>,
    end_at: Option<String>,
    max_occurrences: Option<i64>,
    timezone: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HumanReviewConfig {
    approver_user_ids: Vec<String>,
    approver_roles: Vec<String>,
    instructions: Option<String>,
    timeout_hours: Option<i64>,
    send_notification: bool,
}

fn deserialize_workflow_spec_version<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let version = String::deserialize(deserializer)?;
    if version == "1.0" {
        Ok(version)
    } else {
        Err(de::Error::invalid_value(
            de::Unexpected::Str(&version),
            &"the exact canonical workflow specVersion `1.0`",
        ))
    }
}

fn deserialize_canonical_workflow_graph_version<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u64::deserialize(deserializer)?;
    if version == 1 {
        Ok(version)
    } else {
        Err(de::Error::invalid_value(
            de::Unexpected::Unsigned(version),
            &"the exact canonical workflow graph canonicalVersion `1`",
        ))
    }
}

fn validate_canonical_workflow(document: &CanonicalWorkflow) -> Result<(), MermaidToolError> {
    if let Some(step_id) = find_duplicate_id(document.steps.iter().map(|step| step.id.as_str())) {
        return Err(MermaidToolError::DuplicateCanonicalWorkflowId {
            field: "steps.id",
            id: step_id,
        });
    }

    if let Some(trigger_id) =
        find_duplicate_id(document.triggers.iter().map(|trigger| trigger.id.as_str()))
    {
        return Err(MermaidToolError::DuplicateCanonicalWorkflowId {
            field: "triggers.id",
            id: trigger_id,
        });
    }

    let step_ids = document
        .steps
        .iter()
        .map(|step| step.id.as_str())
        .collect::<BTreeSet<_>>();

    for trigger in &document.triggers {
        if let Some(target_step_id) = trigger.target_step_id.as_deref() {
            if !step_ids.contains(target_step_id) {
                return Err(MermaidToolError::InvalidCanonicalWorkflowReference {
                    field: "triggers.targetStepId",
                    step_id: target_step_id.to_string(),
                });
            }
        }
    }

    for connection in &document.connections {
        if !step_ids.contains(connection.from_step_id.as_str()) {
            return Err(MermaidToolError::InvalidCanonicalWorkflowReference {
                field: "connections.fromStepId",
                step_id: connection.from_step_id.clone(),
            });
        }

        if !step_ids.contains(connection.to_step_id.as_str()) {
            return Err(MermaidToolError::InvalidCanonicalWorkflowReference {
                field: "connections.toStepId",
                step_id: connection.to_step_id.clone(),
            });
        }
    }

    Ok(())
}

fn validate_canonical_workflow_graph(
    document: &CanonicalWorkflowGraph,
) -> Result<(), MermaidToolError> {
    if let Some(node_id) = find_duplicate_id(document.nodes.iter().map(|node| node.id.as_str())) {
        return Err(MermaidToolError::DuplicateCanonicalWorkflowGraphId {
            field: "nodes.id",
            id: node_id,
        });
    }

    let node_ids = document
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();

    if let Some(entry_step_id) = document.entry_step_id.as_deref() {
        if !node_ids.contains(entry_step_id) {
            return Err(MermaidToolError::InvalidCanonicalWorkflowGraphReference {
                field: "entryStepId",
                node_id: entry_step_id.to_string(),
            });
        }
    }

    for edge in &document.edges {
        if !node_ids.contains(edge.from_step_id.as_str()) {
            return Err(MermaidToolError::InvalidCanonicalWorkflowGraphReference {
                field: "edges.fromStepId",
                node_id: edge.from_step_id.clone(),
            });
        }

        if !node_ids.contains(edge.to_step_id.as_str()) {
            return Err(MermaidToolError::InvalidCanonicalWorkflowGraphReference {
                field: "edges.toStepId",
                node_id: edge.to_step_id.clone(),
            });
        }
    }

    Ok(())
}

fn build_canonical_workflow_projection(
    mut document: CanonicalWorkflow,
) -> MermaidWorkflowProjection {
    document.steps.sort_by(|left, right| {
        compare_ordinal(&left.id, &right.id).then(compare_ordinal(&left.name, &right.name))
    });
    document.triggers.sort_by(|left, right| {
        compare_ordinal(&left.id, &right.id).then(compare_ordinal(&left.name, &right.name))
    });
    document.connections.sort_by(|left, right| {
        compare_ordinal(&left.id, &right.id)
            .then(compare_ordinal(&left.from_step_id, &right.from_step_id))
            .then(compare_ordinal(&left.to_step_id, &right.to_step_id))
            .then(compare_optional_ordinal(
                left.label.as_deref(),
                right.label.as_deref(),
            ))
    });

    let step_node_ids =
        assign_prefixed_node_ids("step", document.steps.iter().map(|step| step.id.as_str()));
    let trigger_node_ids = assign_prefixed_node_ids(
        "trigger",
        document.triggers.iter().map(|trigger| trigger.id.as_str()),
    );

    let mut projection = MermaidWorkflowProjection {
        projection_kind: MERMAID_PROJECTION_KIND,
        source_kind: MermaidProjectionSourceKind::CanonicalWorkflow,
        direction: MERMAID_FLOWCHART_DIRECTION,
        nodes: Vec::new(),
        edges: Vec::new(),
        entry_node_ids: Vec::new(),
    };

    for step in &document.steps {
        if let Some(node_id) = step_node_ids.get(&step.id) {
            projection.nodes.push(MermaidProjectionNode {
                node_id: node_id.clone(),
                label: non_empty_label(&step.name, &step.id).to_string(),
                node_role: MermaidProjectionNodeRole::Activity,
                source_id: Some(step.id.clone()),
                source_kind: Some(MermaidProjectionNodeSourceKind::WorkflowStep),
            });
        }
    }

    for trigger in &document.triggers {
        if let Some(trigger_node_id) = trigger_node_ids.get(&trigger.id) {
            projection.nodes.push(MermaidProjectionNode {
                node_id: trigger_node_id.clone(),
                label: non_empty_label(&trigger.name, &trigger.id).to_string(),
                node_role: MermaidProjectionNodeRole::Trigger,
                source_id: Some(trigger.id.clone()),
                source_kind: Some(MermaidProjectionNodeSourceKind::WorkflowTrigger),
            });

            if let Some(target_step_id) = trigger.target_step_id.as_deref() {
                let target_node_id = step_node_ids
                    .get(target_step_id)
                    .expect("validated workflow trigger target step");
                projection.edges.push(MermaidProjectionEdge {
                    from_node_id: trigger_node_id.clone(),
                    to_node_id: target_node_id.clone(),
                    label: None,
                    relation: MermaidProjectionEdgeRelation::Activates,
                });
                projection.entry_node_ids.push(target_node_id.clone());
            }
        }
    }

    for connection in &document.connections {
        let from_node_id = step_node_ids
            .get(&connection.from_step_id)
            .expect("validated workflow connection source step")
            .clone();
        let to_node_id = step_node_ids
            .get(&connection.to_step_id)
            .expect("validated workflow connection destination step")
            .clone();

        projection.edges.push(MermaidProjectionEdge {
            from_node_id,
            to_node_id,
            label: sanitize_optional_label(connection.label.as_deref()),
            relation: MermaidProjectionEdgeRelation::TransitionsTo,
        });
    }

    normalize_projection(&mut projection);
    projection
}

fn build_canonical_workflow_graph_projection(
    mut document: CanonicalWorkflowGraph,
) -> MermaidWorkflowProjection {
    document.nodes.sort_by(|left, right| {
        compare_ordinal(&left.id, &right.id).then(compare_ordinal(&left.name, &right.name))
    });
    document.edges.sort_by(|left, right| {
        compare_ordinal(&left.from_step_id, &right.from_step_id)
            .then(compare_ordinal(&left.to_step_id, &right.to_step_id))
            .then(compare_ordinal(&left.from_port, &right.from_port))
            .then(compare_ordinal(&left.to_port, &right.to_port))
            .then(left.priority.cmp(&right.priority))
            .then(compare_optional_ordinal(
                left.label.as_deref(),
                right.label.as_deref(),
            ))
    });

    let graph_node_ids =
        assign_prefixed_node_ids("node", document.nodes.iter().map(|node| node.id.as_str()));

    let mut projection = MermaidWorkflowProjection {
        projection_kind: MERMAID_PROJECTION_KIND,
        source_kind: MermaidProjectionSourceKind::CanonicalWorkflowGraph,
        direction: MERMAID_FLOWCHART_DIRECTION,
        nodes: Vec::new(),
        edges: Vec::new(),
        entry_node_ids: Vec::new(),
    };

    for node in &document.nodes {
        if let Some(node_id) = graph_node_ids.get(&node.id) {
            projection.nodes.push(MermaidProjectionNode {
                node_id: node_id.clone(),
                label: non_empty_label(&node.name, &node.id).to_string(),
                node_role: MermaidProjectionNodeRole::Activity,
                source_id: Some(node.id.clone()),
                source_kind: Some(MermaidProjectionNodeSourceKind::WorkflowGraphNode),
            });
        }
    }

    if let Some(entry_step_id) = document.entry_step_id.as_deref() {
        if let Some(entry_node_id) = graph_node_ids.get(entry_step_id) {
            projection.entry_node_ids.push(entry_node_id.clone());
        }
    }

    if let Some(trigger) = document.trigger.as_ref() {
        let trigger_node_id = prefixed_node_id("trigger", trigger.synthetic_id_seed());
        projection.nodes.push(MermaidProjectionNode {
            node_id: trigger_node_id.clone(),
            label: trigger.label().to_string(),
            node_role: MermaidProjectionNodeRole::Trigger,
            source_id: None,
            source_kind: None,
        });

        if let Some(entry_step_id) = document.entry_step_id.as_deref() {
            let entry_node_id = graph_node_ids
                .get(entry_step_id)
                .expect("validated workflow graph entry step")
                .clone();
            projection.edges.push(MermaidProjectionEdge {
                from_node_id: trigger_node_id,
                to_node_id: entry_node_id,
                label: None,
                relation: MermaidProjectionEdgeRelation::Activates,
            });
        }
    }

    for edge in &document.edges {
        let from_node_id = graph_node_ids
            .get(&edge.from_step_id)
            .expect("validated workflow graph edge source node")
            .clone();
        let to_node_id = graph_node_ids
            .get(&edge.to_step_id)
            .expect("validated workflow graph edge destination node")
            .clone();

        projection.edges.push(MermaidProjectionEdge {
            from_node_id,
            to_node_id,
            label: sanitize_optional_label(edge.label.as_deref()),
            relation: MermaidProjectionEdgeRelation::TransitionsTo,
        });
    }

    normalize_projection(&mut projection);
    projection
}

fn normalize_projection(projection: &mut MermaidWorkflowProjection) {
    projection.nodes.sort_by(|left, right| {
        compare_ordinal(&left.node_id, &right.node_id)
            .then(compare_ordinal(&left.label, &right.label))
    });
    projection.edges.sort_by(|left, right| {
        left.relation
            .cmp(&right.relation)
            .then(compare_ordinal(&left.from_node_id, &right.from_node_id))
            .then(compare_ordinal(&left.to_node_id, &right.to_node_id))
            .then(compare_optional_ordinal(
                left.label.as_deref(),
                right.label.as_deref(),
            ))
    });
    projection.entry_node_ids.sort();
    projection.entry_node_ids.dedup();
}

fn render_projection(projection: &MermaidWorkflowProjection) -> String {
    let mut lines = vec![MERMAID_FLOWCHART_DIRECTION.to_string()];

    for node in &projection.nodes {
        let rendered_label = escape_node_label(&node.label);
        match node.node_role {
            MermaidProjectionNodeRole::Activity => {
                lines.push(format!("    {}[\"{}\"]", node.node_id, rendered_label));
            }
            MermaidProjectionNodeRole::Trigger => {
                lines.push(format!("    {}((\"{}\"))", node.node_id, rendered_label));
            }
        }
    }

    for edge in &projection.edges {
        let edge_segment = edge
            .label
            .as_deref()
            .filter(|label| !label.trim().is_empty())
            .map(|label| format!("|{}|", escape_edge_label(label)))
            .unwrap_or_default();
        lines.push(format!(
            "    {} -->{} {}",
            edge.from_node_id, edge_segment, edge.to_node_id
        ));
    }

    lines.join("\n")
}

fn parse_mermaid_flowchart(input: &str) -> Result<MermaidWorkflowProjection, MermaidToolError> {
    let mut non_empty_lines = input
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some((index + 1, trimmed))
            }
        })
        .peekable();

    let Some((line_number, first_line)) = non_empty_lines.next() else {
        return Err(MermaidToolError::UnsupportedMermaidDocument);
    };

    if first_line != MERMAID_FLOWCHART_DIRECTION {
        if first_line.starts_with("flowchart") {
            return Err(MermaidToolError::UnsupportedMermaidDirective {
                line: line_number,
                directive: first_line.to_string(),
            });
        }

        return Err(MermaidToolError::UnsupportedMermaidDocument);
    }

    let mut nodes = BTreeMap::new();
    let mut edges = Vec::new();

    for (line_number, line) in non_empty_lines {
        if line.contains("-->") {
            let edge = parse_mermaid_edge_line(line_number, line)?;
            edges.push(edge);
            continue;
        }

        let node = parse_mermaid_node_line(line_number, line)?;
        if nodes.insert(node.node_id.clone(), node).is_some() {
            return Err(MermaidToolError::DuplicateMermaidNodeId {
                node_id: line
                    .split(['[', '('])
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            });
        }
    }

    for edge in &edges {
        if !nodes.contains_key(edge.from_node_id.as_str()) {
            return Err(MermaidToolError::InvalidMermaidReference {
                field: "edges.fromNodeId",
                node_id: edge.from_node_id.clone(),
            });
        }

        if !nodes.contains_key(edge.to_node_id.as_str()) {
            return Err(MermaidToolError::InvalidMermaidReference {
                field: "edges.toNodeId",
                node_id: edge.to_node_id.clone(),
            });
        }
    }

    for edge in &mut edges {
        let from_node = nodes
            .get(edge.from_node_id.as_str())
            .expect("validated Mermaid edge source node");
        edge.relation = infer_edge_relation(from_node.node_role);
    }

    let entry_node_ids = derive_reverse_entry_node_ids(&nodes, &edges);

    let mut projection = MermaidWorkflowProjection {
        projection_kind: MERMAID_PROJECTION_KIND,
        source_kind: MermaidProjectionSourceKind::MermaidFlowchartTd,
        direction: MERMAID_FLOWCHART_DIRECTION,
        nodes: nodes.into_values().collect(),
        edges,
        entry_node_ids,
    };

    normalize_projection(&mut projection);
    Ok(projection)
}

fn parse_mermaid_node_line(
    line_number: usize,
    line: &str,
) -> Result<MermaidProjectionNode, MermaidToolError> {
    if let Some((node_id, raw_label)) = line.split_once("((\"") {
        if !raw_label.ends_with("\"))") {
            return Err(MermaidToolError::InvalidMermaidLine {
                line: line_number,
                message: "trigger node declarations must end with `((\"label\"))`".to_string(),
            });
        }

        return build_mermaid_node(
            node_id,
            &raw_label[..raw_label.len() - 3],
            MermaidProjectionNodeRole::Trigger,
        )
        .map_err(|message| MermaidToolError::InvalidMermaidLine {
            line: line_number,
            message,
        });
    }

    if let Some((node_id, raw_label)) = line.split_once("[\"") {
        if !raw_label.ends_with("\"]") {
            return Err(MermaidToolError::InvalidMermaidLine {
                line: line_number,
                message: "activity node declarations must end with `[\"label\"]`".to_string(),
            });
        }

        return build_mermaid_node(
            node_id,
            &raw_label[..raw_label.len() - 2],
            MermaidProjectionNodeRole::Activity,
        )
        .map_err(|message| MermaidToolError::InvalidMermaidLine {
            line: line_number,
            message,
        });
    }

    Err(MermaidToolError::InvalidMermaidLine {
        line: line_number,
        message: "unsupported Mermaid subset line; expected a node declaration or `-->` edge"
            .to_string(),
    })
}

fn build_mermaid_node(
    raw_node_id: &str,
    raw_label: &str,
    node_role: MermaidProjectionNodeRole,
) -> Result<MermaidProjectionNode, String> {
    let node_id = raw_node_id.trim();
    if node_id.is_empty() {
        return Err("Mermaid node ids must not be empty".to_string());
    }

    let label = unescape_node_label(raw_label.trim());
    let source_kind = infer_reverse_source_kind(node_id, node_role);

    Ok(MermaidProjectionNode {
        node_id: node_id.to_string(),
        label,
        node_role,
        source_id: None,
        source_kind,
    })
}

fn parse_mermaid_edge_line(
    line_number: usize,
    line: &str,
) -> Result<MermaidProjectionEdge, MermaidToolError> {
    let Some((from_node_id, remainder)) = line.split_once("-->") else {
        return Err(MermaidToolError::InvalidMermaidLine {
            line: line_number,
            message: "Mermaid edge declarations must contain `-->`".to_string(),
        });
    };

    let from_node_id = from_node_id.trim();
    if from_node_id.is_empty() {
        return Err(MermaidToolError::InvalidMermaidLine {
            line: line_number,
            message: "Mermaid edge source node id must not be empty".to_string(),
        });
    }

    let remainder = remainder.trim();
    let (label, to_node_id) = if let Some(label_remainder) = remainder.strip_prefix('|') {
        let Some((raw_label, raw_to_node_id)) = label_remainder.split_once('|') else {
            return Err(MermaidToolError::InvalidMermaidLine {
                line: line_number,
                message: "Mermaid edge labels must use the `-->|label| target` form".to_string(),
            });
        };
        let decoded_label = unescape_edge_label(raw_label);
        (
            sanitize_optional_label(Some(&decoded_label)),
            raw_to_node_id.trim(),
        )
    } else {
        (None, remainder)
    };

    if to_node_id.is_empty() {
        return Err(MermaidToolError::InvalidMermaidLine {
            line: line_number,
            message: "Mermaid edge target node id must not be empty".to_string(),
        });
    }

    Ok(MermaidProjectionEdge {
        from_node_id: from_node_id.to_string(),
        to_node_id: to_node_id.to_string(),
        label,
        relation: MermaidProjectionEdgeRelation::TransitionsTo,
    })
}

fn derive_reverse_entry_node_ids(
    nodes: &BTreeMap<String, MermaidProjectionNode>,
    edges: &[MermaidProjectionEdge],
) -> Vec<String> {
    let activation_entry_node_ids = edges
        .iter()
        .filter(|edge| edge.relation == MermaidProjectionEdgeRelation::Activates)
        .map(|edge| edge.to_node_id.clone())
        .collect::<Vec<_>>();

    if !activation_entry_node_ids.is_empty() {
        return activation_entry_node_ids;
    }

    let incoming_node_ids = edges
        .iter()
        .map(|edge| edge.to_node_id.as_str())
        .collect::<BTreeSet<_>>();

    nodes
        .keys()
        .filter(|node_id| !incoming_node_ids.contains(node_id.as_str()))
        .cloned()
        .collect()
}

fn infer_reverse_source_kind(
    node_id: &str,
    node_role: MermaidProjectionNodeRole,
) -> Option<MermaidProjectionNodeSourceKind> {
    match node_role {
        MermaidProjectionNodeRole::Trigger => {
            if node_id.starts_with("trigger_") {
                Some(MermaidProjectionNodeSourceKind::WorkflowTrigger)
            } else {
                None
            }
        }
        MermaidProjectionNodeRole::Activity => {
            if node_id.starts_with("step_") {
                return Some(MermaidProjectionNodeSourceKind::WorkflowStep);
            }

            if node_id.starts_with("node_") {
                return Some(MermaidProjectionNodeSourceKind::WorkflowGraphNode);
            }

            None
        }
    }
}

fn infer_edge_relation(node_role: MermaidProjectionNodeRole) -> MermaidProjectionEdgeRelation {
    match node_role {
        MermaidProjectionNodeRole::Trigger => MermaidProjectionEdgeRelation::Activates,
        MermaidProjectionNodeRole::Activity => MermaidProjectionEdgeRelation::TransitionsTo,
    }
}

fn assign_prefixed_node_ids<'a>(
    prefix: &str,
    raw_ids: impl IntoIterator<Item = &'a str>,
) -> BTreeMap<String, String> {
    let ordered_raw_ids = raw_ids
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let mut node_ids = BTreeMap::new();
    let mut used_node_ids = BTreeSet::new();

    for raw_id in ordered_raw_ids {
        let base_node_id = prefixed_node_id(prefix, &raw_id);
        let mut node_id = base_node_id.clone();
        let mut suffix = 2;

        while !used_node_ids.insert(node_id.clone()) {
            node_id = format!("{base_node_id}_{suffix}");
            suffix += 1;
        }

        node_ids.insert(raw_id, node_id);
    }

    node_ids
}

fn prefixed_node_id(prefix: &str, raw_id: &str) -> String {
    format!("{prefix}_{}", normalize_node_id(raw_id))
}

fn normalize_node_id(raw_id: &str) -> String {
    if raw_id.trim().is_empty() {
        return "node".to_string();
    }

    let normalized = raw_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();

    if normalized.trim_matches('_').is_empty() {
        return "node".to_string();
    }

    if normalized
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        return format!("n_{normalized}");
    }

    normalized
}

fn escape_node_label(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\r', '\n'], " ")
}

fn unescape_node_label(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars();

    while let Some(character) = chars.next() {
        if character == '\\' {
            match chars.next() {
                Some('\\') => output.push('\\'),
                Some('"') => output.push('"'),
                Some(other) => output.push(other),
                None => output.push('\\'),
            }
        } else {
            output.push(character);
        }
    }

    output
}

fn escape_edge_label(value: &str) -> String {
    value
        .replace('&', MERMAID_EDGE_LABEL_AMP_ESCAPE)
        .replace('|', MERMAID_EDGE_LABEL_PIPE_ESCAPE)
        .replace(['\r', '\n'], " ")
}

fn unescape_edge_label(value: &str) -> String {
    value
        .replace(MERMAID_EDGE_LABEL_PIPE_ESCAPE, "|")
        .replace(MERMAID_EDGE_LABEL_AMP_ESCAPE, "&")
}

fn sanitize_optional_label(value: Option<&str>) -> Option<String> {
    value
        .map(|label| label.replace(['\r', '\n'], " ").trim().to_string())
        .filter(|label| !label.is_empty())
}

fn non_empty_label<'a>(preferred: &'a str, fallback: &'a str) -> &'a str {
    if preferred.trim().is_empty() {
        fallback
    } else {
        preferred
    }
}

fn sentence_label(value: &str) -> String {
    value.trim().to_string()
}

fn compare_ordinal(left: &str, right: &str) -> Ordering {
    left.cmp(right)
}

fn compare_optional_ordinal(left: Option<&str>, right: Option<&str>) -> Ordering {
    left.unwrap_or_default().cmp(right.unwrap_or_default())
}

fn find_duplicate_id<'a>(ids: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut seen = BTreeSet::new();

    for id in ids {
        if !seen.insert(id) {
            return Some(id.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{
        narrate_from_json_str, narrate_from_mermaid_str, project_from_json_value,
        render_from_json_str, render_from_json_value, reverse_from_mermaid_str,
        MermaidProjectionEdgeRelation, MermaidProjectionNodeRole, MermaidProjectionNodeSourceKind,
        MermaidProjectionSourceKind, MermaidToolError, MERMAID_NARRATIVE_POSTURE,
    };
    use serde_json::{json, Value};

    const CANONICAL_WORKFLOW_GRAPH_FIXTURE: &str =
        include_str!("../../../../contracts/fixtures/canonical-workflow-graph.minimal.json");
    const CANONICAL_WORKFLOW_FIXTURE: &str =
        include_str!("../../../../contracts/fixtures/canonical-workflow.minimal.json");
    const RENDERED_WORKFLOW_FIXTURE: &str = concat!(
        "flowchart TD\n",
        "    step_step_fulfill[\"Fulfill Order\"]\n",
        "    step_step_review[\"Review Order\"]\n",
        "    trigger_trigger_order_created((\"Order Created\"))\n",
        "    trigger_trigger_order_created --> step_step_review\n",
        "    step_step_review -->|approved| step_step_fulfill"
    );

    fn workflow_fixture_value() -> Value {
        serde_json::from_str(CANONICAL_WORKFLOW_FIXTURE)
            .expect("canonical workflow fixture should parse as JSON")
    }

    fn workflow_graph_fixture_value() -> Value {
        serde_json::from_str(CANONICAL_WORKFLOW_GRAPH_FIXTURE)
            .expect("canonical workflow graph fixture should parse as JSON")
    }

    #[test]
    fn renders_canonical_workflow_fixture() {
        let rendered = render_from_json_str(CANONICAL_WORKFLOW_FIXTURE)
            .expect("workflow fixture should render to Mermaid");

        assert_eq!(rendered, RENDERED_WORKFLOW_FIXTURE);
    }

    #[test]
    fn renders_canonical_workflow_graph_fixture() {
        let rendered = render_from_json_str(CANONICAL_WORKFLOW_GRAPH_FIXTURE)
            .expect("workflow graph fixture should render to Mermaid");

        assert_eq!(
            rendered,
            concat!(
                "flowchart TD\n",
                "    node_step_collect[\"Collect\"]\n",
                "    trigger_contract_updated((\"contract.updated\"))\n",
                "    trigger_contract_updated --> node_step_collect"
            )
        );
    }

    #[test]
    fn rendering_is_deterministic_for_reordered_workflow_arrays() {
        let rendered_a = render_from_json_value(&workflow_fixture_value())
            .expect("first workflow should render");

        let mut reordered = workflow_fixture_value();
        reordered["steps"]
            .as_array_mut()
            .expect("steps should be an array")
            .reverse();

        let rendered_b = render_from_json_value(&reordered).expect("second workflow should render");

        assert_eq!(rendered_a, rendered_b);
    }

    #[test]
    fn normalizes_node_ids_and_escapes_labels() {
        let rendered = render_from_json_value(&json!({
            "id": "wf.escape-check",
            "name": "Escape Check",
            "specVersion": "1.0",
            "canonicalAuthority": "blueprint",
            "conflictPolicy": "reconcile",
            "blueprint": {
                "blueprintId": "bp.escape-check",
                "version": "2026.01",
                "isPinned": true
            },
            "triggers": [
                {
                    "id": "1.start",
                    "name": "Kick \"off\"\nNow",
                    "type": "event",
                    "targetStepId": "01.review-phase"
                }
            ],
            "steps": [
                {
                    "id": "01.review-phase",
                    "name": "Review \"A\"\nB",
                    "type": "human-task"
                },
                {
                    "id": "01.review_phase",
                    "name": "Ship",
                    "type": "service-task"
                }
            ],
            "connections": [
                {
                    "id": "conn.ship",
                    "fromStepId": "01.review-phase",
                    "toStepId": "01.review_phase",
                    "label": "approve | ship"
                }
            ],
            "layout": {
                "groups": [
                    {
                        "id": "group.main",
                        "name": "Main Lane",
                        "x": 0,
                        "y": 0,
                        "width": 1200,
                        "height": 400
                    }
                ],
                "positions": [
                    {
                        "stepId": "01.review-phase",
                        "x": 200,
                        "y": 120
                    },
                    {
                        "stepId": "01.review_phase",
                        "x": 600,
                        "y": 120
                    }
                ]
            }
        }))
        .expect("escaped workflow should render");

        assert!(rendered.contains("step_n_01_review_phase[\"Review \\\"A\\\" B\"]"));
        assert!(rendered.contains("step_n_01_review_phase_2[\"Ship\"]"));
        assert!(rendered.contains("trigger_n_1_start((\"Kick \\\"off\\\" Now\"))"));
        assert!(rendered
            .contains("step_n_01_review_phase -->|approve &#124; ship| step_n_01_review_phase_2"));
    }

    #[test]
    fn renders_canonical_edge_labels_with_reversible_pipe_encoding() {
        let mut workflow = workflow_fixture_value();
        workflow["connections"][0]["label"] = json!("approved | escalated");

        let rendered = render_from_json_value(&workflow)
            .expect("workflow fixture with pipe label should render to Mermaid");

        assert_eq!(
            rendered,
            concat!(
                "flowchart TD\n",
                "    step_step_fulfill[\"Fulfill Order\"]\n",
                "    step_step_review[\"Review Order\"]\n",
                "    trigger_trigger_order_created((\"Order Created\"))\n",
                "    trigger_trigger_order_created --> step_step_review\n",
                "    step_step_review -->|approved &#124; escalated| step_step_fulfill"
            )
        );
    }

    #[test]
    fn reverse_projects_rendered_workflow_fixture() {
        let projection = reverse_from_mermaid_str(RENDERED_WORKFLOW_FIXTURE)
            .expect("rendered workflow fixture should reverse into a projection");

        assert_eq!(
            projection.source_kind,
            MermaidProjectionSourceKind::MermaidFlowchartTd
        );
        assert_eq!(projection.nodes.len(), 3);
        assert_eq!(projection.edges.len(), 2);
        assert_eq!(
            projection.entry_node_ids,
            vec!["step_step_review".to_string()]
        );
        assert_eq!(projection.nodes[0].node_id, "step_step_fulfill");
        assert_eq!(
            projection.nodes[0].node_role,
            MermaidProjectionNodeRole::Activity
        );
        assert_eq!(
            projection.nodes[0].source_kind,
            Some(MermaidProjectionNodeSourceKind::WorkflowStep)
        );
        assert_eq!(
            projection.edges[0].relation,
            MermaidProjectionEdgeRelation::Activates
        );
        assert_eq!(projection.edges[1].label.as_deref(), Some("approved"));
    }

    #[test]
    fn reverse_and_narrate_recover_original_pipe_edge_label_content() {
        let mut workflow = workflow_fixture_value();
        workflow["connections"][0]["label"] = json!("approved | escalated");

        let rendered = render_from_json_value(&workflow)
            .expect("workflow fixture with pipe label should render to Mermaid");
        let reversed = reverse_from_mermaid_str(&rendered)
            .expect("rendered Mermaid should reverse into a projection");
        let (narrative, narrated_projection) =
            narrate_from_mermaid_str(&rendered).expect("rendered Mermaid should narrate");

        assert!(rendered.contains("approved &#124; escalated"));
        assert_eq!(
            reversed
                .edges
                .iter()
                .find(|edge| edge.relation == MermaidProjectionEdgeRelation::TransitionsTo)
                .and_then(|edge| edge.label.as_deref()),
            Some("approved | escalated")
        );
        assert_eq!(
            narrated_projection
                .edges
                .iter()
                .find(|edge| edge.relation == MermaidProjectionEdgeRelation::TransitionsTo)
                .and_then(|edge| edge.label.as_deref()),
            Some("approved | escalated")
        );
        assert!(narrative.text.contains("approved | escalated"));
    }

    #[test]
    fn reverse_treats_activity_ids_with_trigger_prefix_as_transition_sources() {
        let projection = reverse_from_mermaid_str(concat!(
            "flowchart TD\n",
            "    trigger_review_phase[\"Review Phase\"]\n",
            "    step_publish[\"Publish\"]\n",
            "    trigger_review_phase --> step_publish"
        ))
        .expect("Mermaid fixture should reverse into a projection");

        let source_node = projection
            .nodes
            .iter()
            .find(|node| node.node_id == "trigger_review_phase")
            .expect("activity node should be present");
        assert_eq!(source_node.node_role, MermaidProjectionNodeRole::Activity);
        assert_eq!(projection.edges.len(), 1);
        assert_eq!(
            projection.edges[0].relation,
            MermaidProjectionEdgeRelation::TransitionsTo
        );
    }

    #[test]
    fn reverse_derives_entry_node_from_graph_root_when_activation_edges_are_absent() {
        let mut workflow_graph = workflow_graph_fixture_value();
        workflow_graph
            .as_object_mut()
            .expect("workflow graph fixture should be an object")
            .remove("trigger");

        let rendered = render_from_json_value(&workflow_graph)
            .expect("workflow graph without trigger should render");
        let projection = reverse_from_mermaid_str(&rendered)
            .expect("rendered Mermaid should reverse into a projection");

        assert!(projection
            .edges
            .iter()
            .all(|edge| edge.relation != MermaidProjectionEdgeRelation::Activates));
        assert_eq!(
            projection.entry_node_ids,
            vec!["node_step_collect".to_string()]
        );
    }

    #[test]
    fn reverse_does_not_fabricate_source_ids_for_normalized_or_colliding_node_ids() {
        let projection = reverse_from_mermaid_str(concat!(
            "flowchart TD\n",
            "    step_review_phase[\"Review Phase\"]\n",
            "    step_review_phase_2[\"Review Phase Duplicate\"]\n",
            "    trigger_user_signup((\"User Signup\"))\n",
            "    trigger_user_signup --> step_review_phase\n",
            "    step_review_phase --> step_review_phase_2"
        ))
        .expect("normalized Mermaid fixture should reverse into a projection");

        let primary_step = projection
            .nodes
            .iter()
            .find(|node| node.node_id == "step_review_phase")
            .expect("primary step node should be present");
        assert_eq!(primary_step.source_id, None);
        assert_eq!(
            primary_step.source_kind,
            Some(MermaidProjectionNodeSourceKind::WorkflowStep)
        );

        let colliding_step = projection
            .nodes
            .iter()
            .find(|node| node.node_id == "step_review_phase_2")
            .expect("colliding step node should be present");
        assert_eq!(colliding_step.source_id, None);
        assert_eq!(
            colliding_step.source_kind,
            Some(MermaidProjectionNodeSourceKind::WorkflowStep)
        );

        let trigger = projection
            .nodes
            .iter()
            .find(|node| node.node_id == "trigger_user_signup")
            .expect("trigger node should be present");
        assert_eq!(trigger.source_id, None);
        assert_eq!(
            trigger.source_kind,
            Some(MermaidProjectionNodeSourceKind::WorkflowTrigger)
        );
    }

    #[test]
    fn narrates_from_canonical_workflow_json() {
        let (narrative, projection) = narrate_from_json_str(CANONICAL_WORKFLOW_FIXTURE)
            .expect("canonical workflow fixture should narrate");

        assert_eq!(
            projection.source_kind,
            MermaidProjectionSourceKind::CanonicalWorkflow
        );
        assert_eq!(
            narrative.source_kind,
            MermaidProjectionSourceKind::CanonicalWorkflow
        );
        assert_eq!(narrative.posture, MERMAID_NARRATIVE_POSTURE);
        assert_eq!(
            narrative.text,
            concat!(
                "derived Mermaid projection only; canonical workflow authority remains outside Mermaid ",
                "Order Created activates Review Order. ",
                "Review Order transitions to Fulfill Order when approved."
            )
        );
    }

    #[test]
    fn narrates_from_mermaid_projection() {
        let (narrative, projection) = narrate_from_mermaid_str(RENDERED_WORKFLOW_FIXTURE)
            .expect("Mermaid fixture should narrate");

        assert_eq!(
            projection.source_kind,
            MermaidProjectionSourceKind::MermaidFlowchartTd
        );
        assert_eq!(narrative.sentences.len(), 3);
        assert!(narrative
            .text
            .contains("Order Created activates Review Order."));
    }

    #[test]
    fn rejects_workflow_missing_required_field() {
        let mut workflow = workflow_fixture_value();
        workflow
            .as_object_mut()
            .expect("workflow fixture should be an object")
            .remove("layout");

        assert!(matches!(
            project_from_json_value(&workflow),
            Err(MermaidToolError::CanonicalWorkflow { .. })
        ));
    }

    #[test]
    fn rejects_workflow_unknown_field() {
        let mut workflow = workflow_fixture_value();
        workflow
            .as_object_mut()
            .expect("workflow fixture should be an object")
            .insert("unexpected".to_string(), json!(true));

        assert!(matches!(
            project_from_json_value(&workflow),
            Err(MermaidToolError::CanonicalWorkflow { .. })
        ));
    }

    #[test]
    fn rejects_workflow_trigger_target_that_is_not_declared() {
        let mut workflow = workflow_fixture_value();
        workflow["triggers"]
            .as_array_mut()
            .expect("triggers should be an array")[0]["targetStepId"] = json!("step.missing");

        match project_from_json_value(&workflow) {
            Err(MermaidToolError::InvalidCanonicalWorkflowReference { field, step_id }) => {
                assert_eq!(field, "triggers.targetStepId");
                assert_eq!(step_id, "step.missing");
            }
            other => panic!("expected invalid workflow trigger target error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_workflow_connection_endpoint_that_is_not_declared() {
        let mut workflow = workflow_fixture_value();
        workflow["connections"]
            .as_array_mut()
            .expect("connections should be an array")[0]["toStepId"] = json!("step.missing");

        match project_from_json_value(&workflow) {
            Err(MermaidToolError::InvalidCanonicalWorkflowReference { field, step_id }) => {
                assert_eq!(field, "connections.toStepId");
                assert_eq!(step_id, "step.missing");
            }
            other => panic!("expected invalid workflow connection error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_duplicate_workflow_step_ids() {
        let mut workflow = workflow_fixture_value();
        let duplicate_step = workflow["steps"]
            .as_array()
            .expect("steps should be an array")[0]
            .clone();
        workflow["steps"]
            .as_array_mut()
            .expect("steps should be an array")
            .push(duplicate_step);

        match project_from_json_value(&workflow) {
            Err(MermaidToolError::DuplicateCanonicalWorkflowId { field, id }) => {
                assert_eq!(field, "steps.id");
                assert_eq!(id, "step.review");
            }
            other => panic!("expected duplicate workflow step id error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_duplicate_workflow_trigger_ids() {
        let mut workflow = workflow_fixture_value();
        let duplicate_trigger = workflow["triggers"]
            .as_array()
            .expect("triggers should be an array")[0]
            .clone();
        workflow["triggers"]
            .as_array_mut()
            .expect("triggers should be an array")
            .push(duplicate_trigger);

        match project_from_json_value(&workflow) {
            Err(MermaidToolError::DuplicateCanonicalWorkflowId { field, id }) => {
                assert_eq!(field, "triggers.id");
                assert_eq!(id, "trigger.order-created");
            }
            other => panic!("expected duplicate workflow trigger id error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_workflow_graph_missing_required_field() {
        let mut workflow_graph = workflow_graph_fixture_value();
        workflow_graph
            .as_object_mut()
            .expect("workflow graph fixture should be an object")
            .remove("variables");

        assert!(matches!(
            project_from_json_value(&workflow_graph),
            Err(MermaidToolError::CanonicalWorkflowGraph { .. })
        ));
    }

    #[test]
    fn rejects_workflow_graph_unknown_field() {
        let mut workflow_graph = workflow_graph_fixture_value();
        workflow_graph["nodes"]
            .as_array_mut()
            .expect("nodes should be an array")[0]
            .as_object_mut()
            .expect("node should be an object")
            .insert("unexpected".to_string(), json!(true));

        assert!(matches!(
            project_from_json_value(&workflow_graph),
            Err(MermaidToolError::CanonicalWorkflowGraph { .. })
        ));
    }

    #[test]
    fn rejects_workflow_graph_entry_step_id_that_is_not_declared() {
        let mut workflow_graph = workflow_graph_fixture_value();
        workflow_graph["entryStepId"] = json!("step.missing");

        match project_from_json_value(&workflow_graph) {
            Err(MermaidToolError::InvalidCanonicalWorkflowGraphReference { field, node_id }) => {
                assert_eq!(field, "entryStepId");
                assert_eq!(node_id, "step.missing");
            }
            other => panic!("expected invalid workflow graph entry step error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_workflow_graph_edge_endpoint_that_is_not_declared() {
        let mut workflow_graph = workflow_graph_fixture_value();
        workflow_graph["edges"] = json!([
            {
                "fromStepId": "step.collect",
                "fromPort": "result",
                "toStepId": "step.missing",
                "toPort": "payload",
                "priority": 0
            }
        ]);

        match project_from_json_value(&workflow_graph) {
            Err(MermaidToolError::InvalidCanonicalWorkflowGraphReference { field, node_id }) => {
                assert_eq!(field, "edges.toStepId");
                assert_eq!(node_id, "step.missing");
            }
            other => panic!("expected invalid workflow graph edge error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_duplicate_workflow_graph_node_ids() {
        let mut workflow_graph = workflow_graph_fixture_value();
        let duplicate_node = workflow_graph["nodes"]
            .as_array()
            .expect("nodes should be an array")[0]
            .clone();
        workflow_graph["nodes"]
            .as_array_mut()
            .expect("nodes should be an array")
            .push(duplicate_node);

        match project_from_json_value(&workflow_graph) {
            Err(MermaidToolError::DuplicateCanonicalWorkflowGraphId { field, id }) => {
                assert_eq!(field, "nodes.id");
                assert_eq!(id, "step.collect");
            }
            other => panic!("expected duplicate workflow graph node id error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_mermaid_with_unsupported_directive() {
        match reverse_from_mermaid_str("flowchart LR\n    step_a[\"A\"]") {
            Err(MermaidToolError::UnsupportedMermaidDirective { line, directive }) => {
                assert_eq!(line, 1);
                assert_eq!(directive, "flowchart LR");
            }
            other => panic!("expected unsupported directive error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_mermaid_edge_to_unknown_node() {
        match reverse_from_mermaid_str(concat!(
            "flowchart TD\n",
            "    step_a[\"A\"]\n",
            "    step_a --> step_missing"
        )) {
            Err(MermaidToolError::InvalidMermaidReference { field, node_id }) => {
                assert_eq!(field, "edges.toNodeId");
                assert_eq!(node_id, "step_missing");
            }
            other => panic!("expected invalid Mermaid reference error, got {other:?}"),
        }
    }
}
