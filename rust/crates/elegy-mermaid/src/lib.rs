use serde::de::{self, Deserializer};
use serde::Deserialize;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const MERMAID_FLOWCHART_DIRECTION: &str = "flowchart TD";

#[derive(Debug, Error)]
pub enum MermaidRenderError {
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
    DuplicateCanonicalWorkflowId {
        field: &'static str,
        id: String,
    },
    #[error("canonical workflow reference `{field}` targets undeclared step `{step_id}`")]
    InvalidCanonicalWorkflowReference {
        field: &'static str,
        step_id: String,
    },
    #[error("canonical workflow graph field `{field}` declares duplicate id `{id}`")]
    DuplicateCanonicalWorkflowGraphId {
        field: &'static str,
        id: String,
    },
    #[error("canonical workflow graph reference `{field}` targets undeclared node `{node_id}`")]
    InvalidCanonicalWorkflowGraphReference {
        field: &'static str,
        node_id: String,
    },
}

pub fn render_from_json_str(input: &str) -> Result<String, MermaidRenderError> {
    let value = serde_json::from_str::<Value>(input)
        .map_err(|source| MermaidRenderError::Json { source })?;
    render_from_json_value(&value)
}

pub fn render_from_json_value(value: &Value) -> Result<String, MermaidRenderError> {
    match detect_document_kind(value)? {
        DocumentKind::CanonicalWorkflowGraph => {
            let document = serde_json::from_value::<CanonicalWorkflowGraph>(value.clone())
                .map_err(|source| MermaidRenderError::CanonicalWorkflowGraph { source })?;
            validate_canonical_workflow_graph(&document)?;
            Ok(render_canonical_workflow_graph(document))
        }
        DocumentKind::CanonicalWorkflow => {
            let document = serde_json::from_value::<CanonicalWorkflow>(value.clone())
                .map_err(|source| MermaidRenderError::CanonicalWorkflow { source })?;
            validate_canonical_workflow(&document)?;
            Ok(render_canonical_workflow(document))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DocumentKind {
    CanonicalWorkflowGraph,
    CanonicalWorkflow,
}

fn detect_document_kind(value: &Value) -> Result<DocumentKind, MermaidRenderError> {
    let Some(object) = value.as_object() else {
        return Err(MermaidRenderError::UnsupportedCanonicalDocument);
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

    Err(MermaidRenderError::UnsupportedCanonicalDocument)
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

fn deserialize_canonical_workflow_graph_version<'de, D>(
    deserializer: D,
) -> Result<u64, D::Error>
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

fn validate_canonical_workflow(document: &CanonicalWorkflow) -> Result<(), MermaidRenderError> {
    if let Some(step_id) = find_duplicate_id(document.steps.iter().map(|step| step.id.as_str())) {
        return Err(MermaidRenderError::DuplicateCanonicalWorkflowId {
            field: "steps.id",
            id: step_id,
        });
    }

    if let Some(trigger_id) =
        find_duplicate_id(document.triggers.iter().map(|trigger| trigger.id.as_str()))
    {
        return Err(MermaidRenderError::DuplicateCanonicalWorkflowId {
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
                return Err(MermaidRenderError::InvalidCanonicalWorkflowReference {
                    field: "triggers.targetStepId",
                    step_id: target_step_id.to_string(),
                });
            }
        }
    }

    for connection in &document.connections {
        if !step_ids.contains(connection.from_step_id.as_str()) {
            return Err(MermaidRenderError::InvalidCanonicalWorkflowReference {
                field: "connections.fromStepId",
                step_id: connection.from_step_id.clone(),
            });
        }

        if !step_ids.contains(connection.to_step_id.as_str()) {
            return Err(MermaidRenderError::InvalidCanonicalWorkflowReference {
                field: "connections.toStepId",
                step_id: connection.to_step_id.clone(),
            });
        }
    }

    Ok(())
}

fn validate_canonical_workflow_graph(
    document: &CanonicalWorkflowGraph,
) -> Result<(), MermaidRenderError> {
    if let Some(node_id) = find_duplicate_id(document.nodes.iter().map(|node| node.id.as_str())) {
        return Err(MermaidRenderError::DuplicateCanonicalWorkflowGraphId {
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
            return Err(MermaidRenderError::InvalidCanonicalWorkflowGraphReference {
                field: "entryStepId",
                node_id: entry_step_id.to_string(),
            });
        }
    }

    for edge in &document.edges {
        if !node_ids.contains(edge.from_step_id.as_str()) {
            return Err(MermaidRenderError::InvalidCanonicalWorkflowGraphReference {
                field: "edges.fromStepId",
                node_id: edge.from_step_id.clone(),
            });
        }

        if !node_ids.contains(edge.to_step_id.as_str()) {
            return Err(MermaidRenderError::InvalidCanonicalWorkflowGraphReference {
                field: "edges.toStepId",
                node_id: edge.to_step_id.clone(),
            });
        }
    }

    Ok(())
}

fn render_canonical_workflow(mut document: CanonicalWorkflow) -> String {
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
            .then(compare_optional_ordinal(left.label.as_deref(), right.label.as_deref()))
    });

    let step_node_ids = assign_prefixed_node_ids(
        "step",
        document.steps.iter().map(|step| step.id.as_str()),
    );
    let trigger_node_ids = assign_prefixed_node_ids(
        "trigger",
        document.triggers.iter().map(|trigger| trigger.id.as_str()),
    );

    let mut lines = vec![MERMAID_FLOWCHART_DIRECTION.to_string()];

    for step in &document.steps {
        if let Some(node_id) = step_node_ids.get(&step.id) {
            lines.push(format!(
                "    {node_id}[\"{}\"]",
                escape_node_label(non_empty_label(&step.name, &step.id))
            ));
        }
    }

    for trigger in &document.triggers {
        if let Some(trigger_node_id) = trigger_node_ids.get(&trigger.id) {
            lines.push(format!(
                "    {trigger_node_id}((\"{}\"))",
                escape_node_label(non_empty_label(&trigger.name, &trigger.id))
            ));

            if let Some(target_step_id) = trigger.target_step_id.as_deref() {
                let target_node_id = step_node_ids
                    .get(target_step_id)
                    .expect("validated workflow trigger target step");
                lines.push(format!("    {trigger_node_id} --> {target_node_id}"));
            }
        }
    }

    for connection in &document.connections {
        let from_node_id = step_node_ids
            .get(&connection.from_step_id)
            .expect("validated workflow connection source step");
        let to_node_id = step_node_ids
            .get(&connection.to_step_id)
            .expect("validated workflow connection destination step");

        let edge_segment = connection
            .label
            .as_deref()
            .filter(|label| !label.trim().is_empty())
            .map(|label| format!("|{}|", escape_edge_label(label)))
            .unwrap_or_default();

        lines.push(format!(
            "    {from_node_id} -->{edge_segment} {to_node_id}"
        ));
    }

    lines.join("\n")
}

fn render_canonical_workflow_graph(mut document: CanonicalWorkflowGraph) -> String {
    document.nodes.sort_by(|left, right| {
        compare_ordinal(&left.id, &right.id).then(compare_ordinal(&left.name, &right.name))
    });
    document.edges.sort_by(|left, right| {
        compare_ordinal(&left.from_step_id, &right.from_step_id)
            .then(compare_ordinal(&left.to_step_id, &right.to_step_id))
            .then(compare_ordinal(&left.from_port, &right.from_port))
            .then(compare_ordinal(&left.to_port, &right.to_port))
            .then(left.priority.cmp(&right.priority))
            .then(compare_optional_ordinal(left.label.as_deref(), right.label.as_deref()))
    });

    let graph_node_ids = assign_prefixed_node_ids(
        "node",
        document
            .nodes
            .iter()
            .map(|node| node.id.as_str()),
    );

    let mut lines = vec![MERMAID_FLOWCHART_DIRECTION.to_string()];

    for node in &document.nodes {
        if let Some(node_id) = graph_node_ids.get(&node.id) {
            lines.push(format!(
                "    {node_id}[\"{}\"]",
                escape_node_label(non_empty_label(&node.name, &node.id))
            ));
        }
    }

    if let Some(trigger) = document.trigger.as_ref() {
        let trigger_node_id = prefixed_node_id("trigger", &trigger.synthetic_id_seed());
        lines.push(format!(
            "    {trigger_node_id}((\"{}\"))",
            escape_node_label(&trigger.label())
        ));

        if let Some(entry_step_id) = document.entry_step_id.as_deref() {
            let entry_node_id = graph_node_ids
                .get(entry_step_id)
                .expect("validated workflow graph entry step");
            lines.push(format!("    {trigger_node_id} --> {entry_node_id}"));
        }
    }

    for edge in &document.edges {
        let from_node_id = graph_node_ids
            .get(&edge.from_step_id)
            .expect("validated workflow graph edge source node");
        let to_node_id = graph_node_ids
            .get(&edge.to_step_id)
            .expect("validated workflow graph edge destination node");

        let edge_segment = edge
            .label
            .as_deref()
            .filter(|label| !label.trim().is_empty())
            .map(|label| format!("|{}|", escape_edge_label(label)))
            .unwrap_or_default();

        lines.push(format!(
            "    {from_node_id} -->{edge_segment} {to_node_id}"
        ));
    }

    lines.join("\n")
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

fn escape_edge_label(value: &str) -> String {
    value.replace('|', "/").replace(['\r', '\n'], " ")
}

fn non_empty_label<'a>(preferred: &'a str, fallback: &'a str) -> &'a str {
    if preferred.trim().is_empty() {
        fallback
    } else {
        preferred
    }
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
    use super::{render_from_json_str, render_from_json_value, MermaidRenderError};
    use serde_json::{json, Value};

    const CANONICAL_WORKFLOW_GRAPH_FIXTURE: &str = include_str!(
        "../../../../contracts/fixtures/canonical-workflow-graph.minimal.json"
    );
    const CANONICAL_WORKFLOW_FIXTURE: &str =
        include_str!("../../../../contracts/fixtures/canonical-workflow.minimal.json");

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

        assert_eq!(
            rendered,
            concat!(
                "flowchart TD\n",
                "    step_step_fulfill[\"Fulfill Order\"]\n",
                "    step_step_review[\"Review Order\"]\n",
                "    trigger_trigger_order_created((\"Order Created\"))\n",
                "    trigger_trigger_order_created --> step_step_review\n",
                "    step_step_review -->|approved| step_step_fulfill"
            )
        );
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

        let rendered_b = render_from_json_value(&reordered)
            .expect("second workflow should render");

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
        assert!(rendered.contains(
            "step_n_01_review_phase -->|approve / ship| step_n_01_review_phase_2"
        ));
    }

    #[test]
    fn rejects_workflow_missing_required_field() {
        let mut workflow = workflow_fixture_value();
        workflow
            .as_object_mut()
            .expect("workflow fixture should be an object")
            .remove("layout");

        assert!(matches!(
            render_from_json_value(&workflow),
            Err(MermaidRenderError::CanonicalWorkflow { .. })
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
            render_from_json_value(&workflow),
            Err(MermaidRenderError::CanonicalWorkflow { .. })
        ));
    }

    #[test]
    fn rejects_workflow_trigger_target_that_is_not_declared() {
        let mut workflow = workflow_fixture_value();
        workflow["triggers"]
            .as_array_mut()
            .expect("triggers should be an array")[0]["targetStepId"] = json!("step.missing");

        match render_from_json_value(&workflow) {
            Err(MermaidRenderError::InvalidCanonicalWorkflowReference { field, step_id }) => {
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

        match render_from_json_value(&workflow) {
            Err(MermaidRenderError::InvalidCanonicalWorkflowReference { field, step_id }) => {
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

        match render_from_json_value(&workflow) {
            Err(MermaidRenderError::DuplicateCanonicalWorkflowId { field, id }) => {
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

        match render_from_json_value(&workflow) {
            Err(MermaidRenderError::DuplicateCanonicalWorkflowId { field, id }) => {
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
            render_from_json_value(&workflow_graph),
            Err(MermaidRenderError::CanonicalWorkflowGraph { .. })
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
            render_from_json_value(&workflow_graph),
            Err(MermaidRenderError::CanonicalWorkflowGraph { .. })
        ));
    }

    #[test]
    fn rejects_workflow_graph_entry_step_id_that_is_not_declared() {
        let mut workflow_graph = workflow_graph_fixture_value();
        workflow_graph["entryStepId"] = json!("step.missing");

        match render_from_json_value(&workflow_graph) {
            Err(MermaidRenderError::InvalidCanonicalWorkflowGraphReference {
                field,
                node_id,
            }) => {
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

        match render_from_json_value(&workflow_graph) {
            Err(MermaidRenderError::InvalidCanonicalWorkflowGraphReference {
                field,
                node_id,
            }) => {
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

        match render_from_json_value(&workflow_graph) {
            Err(MermaidRenderError::DuplicateCanonicalWorkflowGraphId { field, id }) => {
                assert_eq!(field, "nodes.id");
                assert_eq!(id, "step.collect");
            }
            other => panic!("expected duplicate workflow graph node id error, got {other:?}"),
        }
    }
}