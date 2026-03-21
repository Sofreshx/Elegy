use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContractsError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("compatibility manifest is missing schema '{0}'")]
    MissingSchema(String),
    #[error("{0}")]
    Compatibility(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityManifest {
    pub manifest_version: String,
    pub package: ContractPackage,
    pub schemas: Vec<SchemaEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContractPackage {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SchemaEntry {
    pub name: String,
    pub schema_version: String,
    pub file: String,
    pub fixtures: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConsumerSupportManifest {
    pub consumer: String,
    pub consumer_version: String,
    pub upstream_package: ContractPackage,
    pub schemas: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentMessageRole {
    System,
    #[default]
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessage {
    #[serde(default)]
    pub message_id: String,
    #[serde(default)]
    pub role: AgentMessageRole,
    #[serde(default)]
    pub content: String,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRequestContext {
    #[serde(default)]
    pub correlation_id: String,
    pub session_id: Option<String>,
    pub conversation_id: Option<String>,
    pub requested_skill_id: Option<String>,
    #[serde(default)]
    pub capability_hints: Vec<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRequestEnvelope {
    #[serde(default)]
    pub request_id: String,
    #[serde(default)]
    pub messages: Vec<AgentMessage>,
    #[serde(default)]
    pub context: AgentRequestContext,
    pub streaming_requested: bool,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentResponseStatus {
    #[default]
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentUsage {
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentResponseEnvelope {
    #[serde(default)]
    pub request_id: String,
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub status: AgentResponseStatus,
    #[serde(default)]
    pub messages: Vec<AgentMessage>,
    #[serde(default)]
    pub usage: AgentUsage,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentEventType {
    #[default]
    RequestAccepted,
    RunStarted,
    MessageDelta,
    MessageCompleted,
    ReasoningDelta,
    ReasoningCompleted,
    ToolCallStarted,
    ToolCallCompleted,
    Warning,
    Error,
    RunCompleted,
    RunFailed,
    RunCancelled,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentEventSource {
    Client,
    #[default]
    Broker,
    Model,
    Tool,
    System,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentEventPayload {
    pub message_id: Option<String>,
    pub role: Option<AgentMessageRole>,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub content: Option<String>,
    pub delta_content: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub usage: Option<AgentUsage>,
    pub metadata: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentEventEnvelope {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub stream_id: String,
    pub sequence: u64,
    pub parent_event_id: Option<String>,
    #[serde(default)]
    pub timestamp: String,
    pub ephemeral: bool,
    #[serde(default)]
    pub event_type: AgentEventType,
    #[serde(default)]
    pub source: AgentEventSource,
    #[serde(default)]
    pub payload: AgentEventPayload,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillDefinition {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub identity: SkillIdentity,
    #[serde(default)]
    pub metadata: SkillMetadata,
    #[serde(default)]
    pub triggers: Vec<SkillTrigger>,
    #[serde(default)]
    pub constraints: Vec<SkillConstraint>,
    #[serde(default)]
    pub input: SkillInputContract,
    #[serde(default)]
    pub output: SkillOutputContract,
    #[serde(default)]
    pub execution: SkillExecutionContract,
    #[serde(default)]
    pub governance: SkillGovernanceMetadata,
    #[serde(default)]
    pub discovery: SkillDiscoveryMetadata,
    #[serde(default)]
    pub origin: SkillOrigin,
    #[serde(default)]
    pub lifecycle_state: SkillLifecycleState,
}

impl SkillDefinition {
    pub fn effective_id(&self) -> &str {
        if self.identity.definition_id.trim().is_empty() {
            self.id.as_str()
        } else {
            self.identity.definition_id.as_str()
        }
    }

    pub fn effective_name(&self) -> &str {
        if self.identity.display_name.trim().is_empty() {
            self.name.as_str()
        } else {
            self.identity.display_name.as_str()
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillIdentity {
    #[serde(default)]
    pub definition_id: String,
    #[serde(default)]
    pub display_name: String,
    pub namespace: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillMetadata {
    pub summary: Option<String>,
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub owners: Vec<String>,
    pub documentation_uri: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillTrigger {
    pub pattern: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillConstraint {
    pub constraint_id: String,
    pub description: Option<String>,
    pub required: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillInputContract {
    #[serde(default)]
    pub parameters: Vec<SkillParameter>,
    pub schema_ref: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillParameter {
    pub name: String,
    pub r#type: String,
    pub description: Option<String>,
    pub required: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillOutputContract {
    pub result_type: Option<String>,
    pub schema_ref: Option<String>,
    pub returns_collection: bool,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillExecutionContract {
    #[serde(default)]
    pub mode: SkillExecutionMode,
    pub is_deterministic: bool,
    pub has_side_effects: bool,
    pub timeout_seconds: Option<i32>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillExecutionMode {
    #[default]
    RequestResponse,
    LongRunning,
    Streaming,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillGovernanceMetadata {
    #[serde(default)]
    pub risk_level: SkillRiskLevel,
    #[serde(default)]
    pub approval_requirement: SkillApprovalRequirement,
    #[serde(default)]
    pub policy_refs: Vec<String>,
    #[serde(default)]
    pub allowed_contexts: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillRiskLevel {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillApprovalRequirement {
    #[default]
    None,
    Advisory,
    Required,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillDiscoveryMetadata {
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub capability_hints: Vec<String>,
    pub is_hidden: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillOrigin {
    #[serde(default)]
    pub materialization_kind: SkillMaterializationKind,
    #[serde(default)]
    pub source_kind: SkillSourceKind,
    pub source_ref: Option<String>,
    pub source_version: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillMaterializationKind {
    #[default]
    Declared,
    Dynamic,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillSourceKind {
    #[default]
    Manual,
    Imported,
    Generated,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillLifecycleState {
    #[default]
    Draft,
    Active,
    Deprecated,
    Archived,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillDiscoveryIndex {
    pub schema_version: i32,
    pub built_at: Option<String>,
    #[serde(default)]
    pub entries: Vec<SkillDiscoveryEntry>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillDiscoveryEntry {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub lifecycle_state: SkillLifecycleState,
    #[serde(default)]
    pub triggers_on: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub capability_hints: Vec<String>,
    #[serde(default)]
    pub manifest: SkillDiscoveryManifest,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillDiscoveryManifest {
    pub id: String,
    #[serde(default)]
    pub load_mode: SkillLoadMode,
    pub vault_ref: Option<String>,
    #[serde(default)]
    pub source_kind: SkillSourceKind,
    #[serde(default)]
    pub materialization_kind: SkillMaterializationKind,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillLoadMode {
    Always,
    #[default]
    OnDemand,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerDescriptor {
    pub server_name: String,
    #[serde(default)]
    pub transport: McpTransportKind,
    #[serde(default)]
    pub tools: Vec<McpToolDefinition>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum McpTransportKind {
    #[default]
    Stdio,
    Http,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpAnalysisResult {
    pub server_name: String,
    #[serde(default)]
    pub analyses: Vec<McpToolAnalysis>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpToolAnalysis {
    #[serde(default)]
    pub tool: McpToolDefinition,
    #[serde(default)]
    pub extracted_triggers: Vec<SkillTrigger>,
    pub has_valid_schema: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillValidationResult {
    pub issues: Vec<String>,
}

impl SkillValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct McpValidationResult {
    pub issues: Vec<String>,
}

impl McpValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentEnvelopeValidationResult {
    pub issues: Vec<String>,
}

impl AgentEnvelopeValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

pub fn default_support_manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
    .join("..")
        .join("contracts")
    .join("support")
        .join("elegy-rust-support.json")
}

pub fn resolve_upstream_contracts_dir() -> PathBuf {
    if let Some(path) = env::var_os("ELEGY_CONTRACTS_DIR") {
        return PathBuf::from(path);
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("artifacts")
        .join("contracts")
}

pub fn load_compatibility_manifest_from_dir(
    dir: &Path,
) -> Result<CompatibilityManifest, ContractsError> {
    load_json_file(&dir.join("compatibility-manifest.json"))
}

pub fn load_consumer_support_manifest(
    path: &Path,
) -> Result<ConsumerSupportManifest, ContractsError> {
    load_json_file(path)
}

pub fn load_skill_definition_fixture_from_dir(
    dir: &Path,
) -> Result<SkillDefinition, ContractsError> {
    load_json_file(&dir.join("fixtures").join("skill-definition.minimal.json"))
}

pub fn load_skill_discovery_index_fixture_from_dir(
    dir: &Path,
) -> Result<SkillDiscoveryIndex, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("skill-discovery-index.minimal.json"),
    )
}

pub fn load_mcp_server_descriptor_fixture_from_dir(
    dir: &Path,
) -> Result<McpServerDescriptor, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("mcp-server-descriptor.minimal.json"),
    )
}

pub fn load_mcp_analysis_result_fixture_from_dir(
    dir: &Path,
) -> Result<McpAnalysisResult, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("mcp-analysis-result.minimal.json"),
    )
}

pub fn load_agent_request_envelope_fixture_from_dir(
    dir: &Path,
) -> Result<AgentRequestEnvelope, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("agent-request-envelope.minimal.json"),
    )
}

pub fn load_agent_response_envelope_fixture_from_dir(
    dir: &Path,
) -> Result<AgentResponseEnvelope, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("agent-response-envelope.minimal.json"),
    )
}

pub fn load_agent_event_envelope_fixture_from_dir(
    dir: &Path,
) -> Result<AgentEventEnvelope, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("agent-event-envelope.minimal.json"),
    )
}

pub fn validate_support_manifest_against_upstream(
    support: &ConsumerSupportManifest,
    upstream: &CompatibilityManifest,
) -> Result<(), ContractsError> {
    if support.upstream_package.name != upstream.package.name {
        return Err(ContractsError::Compatibility(format!(
            "support manifest expects upstream package '{}', but bundle package is '{}'",
            support.upstream_package.name, upstream.package.name
        )));
    }

    if support.upstream_package.version != upstream.package.version {
        return Err(ContractsError::Compatibility(format!(
            "support manifest expects upstream package version '{}', but bundle version is '{}'",
            support.upstream_package.version, upstream.package.version
        )));
    }

    for (schema_name, expected_version) in &support.schemas {
        let entry = upstream
            .schemas
            .iter()
            .find(|candidate| candidate.name == *schema_name)
            .ok_or_else(|| ContractsError::MissingSchema(schema_name.clone()))?;

        if entry.schema_version != *expected_version {
            return Err(ContractsError::Compatibility(format!(
                "support manifest expects schema '{}' at version '{}', but bundle provides '{}'",
                schema_name, expected_version, entry.schema_version
            )));
        }
    }

    Ok(())
}

pub fn validate_skill_definition(definition: &SkillDefinition) -> SkillValidationResult {
    let mut issues = Vec::new();

    if definition.effective_id().trim().is_empty() {
        issues.push("Skill definition ID is required.".to_string());
    }

    if definition.effective_name().trim().is_empty() {
        issues.push("Skill name is required.".to_string());
    }

    if definition
        .triggers
        .iter()
        .any(|trigger| trigger.pattern.trim().is_empty())
    {
        issues.push("Skill triggers must define a non-empty pattern.".to_string());
    }

    if definition
        .constraints
        .iter()
        .any(|constraint| constraint.constraint_id.trim().is_empty())
    {
        issues.push("Skill constraints must define a non-empty constraint ID.".to_string());
    }

    if definition
        .identity
        .aliases
        .iter()
        .any(|alias| alias.trim().is_empty())
    {
        issues.push("Skill identity aliases must not be blank.".to_string());
    }

    if has_duplicate_values(definition.identity.aliases.iter().map(String::as_str)) {
        issues.push("Skill identity aliases must be unique.".to_string());
    }

    if definition
        .metadata
        .tags
        .iter()
        .any(|tag| tag.trim().is_empty())
    {
        issues.push("Skill metadata tags must not be blank.".to_string());
    }

    if definition
        .metadata
        .owners
        .iter()
        .any(|owner| owner.trim().is_empty())
    {
        issues.push("Skill metadata owners must not be blank.".to_string());
    }

    if definition
        .input
        .parameters
        .iter()
        .any(|parameter| parameter.name.trim().is_empty())
    {
        issues.push("Skill input parameters must define a non-empty name.".to_string());
    }

    if has_duplicate_values(
        definition
            .input
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str()),
    ) {
        issues.push("Skill input parameter names must be unique.".to_string());
    }

    if definition
        .input
        .parameters
        .iter()
        .any(|parameter| parameter.r#type.trim().is_empty())
    {
        issues.push("Skill input parameters must define a non-empty type.".to_string());
    }

    if definition
        .execution
        .timeout_seconds
        .is_some_and(|timeout| timeout <= 0)
    {
        issues.push(
            "Skill execution timeout, when provided, must be greater than zero seconds."
                .to_string(),
        );
    }

    if definition.governance.approval_requirement != SkillApprovalRequirement::None
        && definition.governance.policy_refs.is_empty()
    {
        issues.push(
            "Skills that require approval must declare at least one policy reference.".to_string(),
        );
    }

    if definition
        .governance
        .policy_refs
        .iter()
        .any(|policy_ref| policy_ref.trim().is_empty())
    {
        issues.push("Skill governance policy references must not be blank.".to_string());
    }

    if definition
        .governance
        .allowed_contexts
        .iter()
        .any(|context| context.trim().is_empty())
    {
        issues.push("Skill governance allowed contexts must not be blank.".to_string());
    }

    if definition
        .discovery
        .keywords
        .iter()
        .any(|keyword| keyword.trim().is_empty())
    {
        issues.push("Skill discovery keywords must not be blank.".to_string());
    }

    if definition
        .discovery
        .capability_hints
        .iter()
        .any(|hint| hint.trim().is_empty())
    {
        issues.push("Skill discovery capability hints must not be blank.".to_string());
    }

    if definition.origin.materialization_kind == SkillMaterializationKind::Dynamic
        && definition.origin.source_kind == SkillSourceKind::Manual
        && definition
            .origin
            .source_ref
            .as_ref()
            .is_none_or(|source_ref| source_ref.trim().is_empty())
    {
        issues.push(
            "Dynamic skills must declare either a source reference or a non-manual source kind."
                .to_string(),
        );
    }

    SkillValidationResult { issues }
}

pub fn validate_agent_request_envelope(
    request: &AgentRequestEnvelope,
) -> AgentEnvelopeValidationResult {
    let mut issues = Vec::new();

    if request.request_id.trim().is_empty() {
        issues.push("Agent request envelope must declare a request ID.".to_string());
    }

    if request.messages.is_empty() {
        issues.push("Agent request envelope must include at least one message.".to_string());
    }

    validate_agent_messages(&request.messages, "Agent request messages", &mut issues);

    if request
        .context
        .capability_hints
        .iter()
        .any(|hint| hint.trim().is_empty())
    {
        issues.push("Agent request capability hints must not be blank.".to_string());
    }

    if has_blank_metadata_entries(&request.context.metadata) {
        issues.push("Agent request metadata must not contain blank keys or values.".to_string());
    }

    AgentEnvelopeValidationResult { issues }
}

pub fn validate_agent_response_envelope(
    response: &AgentResponseEnvelope,
) -> AgentEnvelopeValidationResult {
    let mut issues = Vec::new();

    if response.request_id.trim().is_empty() {
        issues.push("Agent response envelope must declare a request ID.".to_string());
    }

    if response.run_id.trim().is_empty() {
        issues.push("Agent response envelope must declare a run ID.".to_string());
    }

    validate_agent_messages(&response.messages, "Agent response messages", &mut issues);

    if matches!(response.status, AgentResponseStatus::Completed) && response.messages.is_empty() {
        issues.push("Completed agent responses must include at least one message.".to_string());
    }

    if matches!(
        response.status,
        AgentResponseStatus::Failed | AgentResponseStatus::Cancelled
    ) && response
        .error_message
        .as_ref()
        .is_none_or(|message| message.trim().is_empty())
        && response
            .error_code
            .as_ref()
            .is_none_or(|code| code.trim().is_empty())
    {
        issues.push(
            "Failed or cancelled agent responses must declare an error code or error message."
                .to_string(),
        );
    }

    if has_negative_usage(&response.usage) {
        issues.push("Agent usage values must not be negative.".to_string());
    }

    if usage_total_is_inconsistent(&response.usage) {
        issues.push(
            "Agent usage total tokens must be greater than or equal to input and output tokens."
                .to_string(),
        );
    }

    if has_blank_metadata_entries(&response.metadata) {
        issues.push("Agent response metadata must not contain blank keys or values.".to_string());
    }

    AgentEnvelopeValidationResult { issues }
}

pub fn validate_agent_event_envelope(event: &AgentEventEnvelope) -> AgentEnvelopeValidationResult {
    let mut issues = Vec::new();

    if event.event_id.trim().is_empty() {
        issues.push("Agent event envelope must declare an event ID.".to_string());
    }

    if event.run_id.trim().is_empty() {
        issues.push("Agent event envelope must declare a run ID.".to_string());
    }

    if event.stream_id.trim().is_empty() {
        issues.push("Agent event envelope must declare a stream ID.".to_string());
    }

    if event.sequence == 0 {
        issues.push("Agent event envelope sequence must be greater than zero.".to_string());
    }

    if event.timestamp.trim().is_empty() {
        issues.push("Agent event envelope must declare a timestamp.".to_string());
    }

    match event.event_type {
        AgentEventType::MessageDelta | AgentEventType::ReasoningDelta => {
            if event
                .payload
                .delta_content
                .as_ref()
                .is_none_or(|delta| delta.trim().is_empty())
            {
                issues.push(
                    "Delta agent events must include non-empty delta content.".to_string(),
                );
            }
        }
        AgentEventType::MessageCompleted | AgentEventType::ReasoningCompleted => {
            if event
                .payload
                .content
                .as_ref()
                .is_none_or(|content| content.trim().is_empty())
            {
                issues.push(
                    "Completed message events must include non-empty content.".to_string(),
                );
            }
        }
        AgentEventType::ToolCallStarted => {
            if event
                .payload
                .tool_call_id
                .as_ref()
                .is_none_or(|tool_call_id| tool_call_id.trim().is_empty())
                || event
                    .payload
                    .tool_name
                    .as_ref()
                    .is_none_or(|tool_name| tool_name.trim().is_empty())
            {
                issues.push(
                    "Tool call started events must include a tool call ID and tool name."
                        .to_string(),
                );
            }
        }
        AgentEventType::ToolCallCompleted => {
            if event
                .payload
                .tool_call_id
                .as_ref()
                .is_none_or(|tool_call_id| tool_call_id.trim().is_empty())
            {
                issues.push(
                    "Tool call completed events must include a tool call ID.".to_string(),
                );
            }
        }
        AgentEventType::Error | AgentEventType::RunFailed => {
            if event
                .payload
                .error_message
                .as_ref()
                .is_none_or(|message| message.trim().is_empty())
                && event
                    .payload
                    .error_code
                    .as_ref()
                    .is_none_or(|code| code.trim().is_empty())
            {
                issues.push(
                    "Error agent events must include an error code or error message."
                        .to_string(),
                );
            }
        }
        _ => {}
    }

    if event.payload.usage.as_ref().is_some_and(has_negative_usage) {
        issues.push("Agent event usage values must not be negative.".to_string());
    }

    if event
        .payload
        .usage
        .as_ref()
        .is_some_and(usage_total_is_inconsistent)
    {
        issues.push(
            "Agent event usage total tokens must be greater than or equal to input and output tokens."
                .to_string(),
        );
    }

    if event
        .payload
        .metadata
        .as_ref()
        .is_some_and(has_blank_metadata_entries)
    {
        issues.push("Agent event metadata must not contain blank keys or values.".to_string());
    }

    AgentEnvelopeValidationResult { issues }
}

pub fn validate_mcp_server_descriptor(descriptor: &McpServerDescriptor) -> McpValidationResult {
    let mut issues = Vec::new();

    if descriptor.server_name.trim().is_empty() {
        issues.push("MCP server descriptor must declare a server name.".to_string());
    }

    if descriptor
        .tools
        .iter()
        .any(|tool| tool.name.trim().is_empty())
    {
        issues.push("MCP server descriptor tools must define a non-empty name.".to_string());
    }

    if has_duplicate_values(descriptor.tools.iter().map(|tool| tool.name.as_str())) {
        issues.push("MCP server descriptor tool names must be unique.".to_string());
    }

    McpValidationResult { issues }
}

pub fn validate_mcp_analysis_result(result: &McpAnalysisResult) -> McpValidationResult {
    let mut issues = Vec::new();

    if result.server_name.trim().is_empty() {
        issues.push("MCP analysis result must declare a server name.".to_string());
    }

    if result
        .analyses
        .iter()
        .any(|analysis| analysis.tool.name.trim().is_empty())
    {
        issues.push("MCP analysis entries must define a non-empty tool name.".to_string());
    }

    if has_duplicate_values(
        result
            .analyses
            .iter()
            .map(|analysis| analysis.tool.name.as_str()),
    ) {
        issues.push("MCP analysis entries must be unique per tool name.".to_string());
    }

    if result.analyses.iter().any(|analysis| {
        analysis
            .extracted_triggers
            .iter()
            .any(|trigger| trigger.pattern.trim().is_empty())
    }) {
        issues.push("MCP analysis extracted triggers must define a non-empty pattern.".to_string());
    }

    if result
        .analyses
        .iter()
        .any(|analysis| analysis.has_valid_schema && analysis.tool.input_schema.is_none())
    {
        issues.push(
            "MCP analysis entries marked as having a valid schema must include an input schema."
                .to_string(),
        );
    }

    McpValidationResult { issues }
}

fn load_json_file<T>(path: &Path) -> Result<T, ContractsError>
where
    T: for<'de> Deserialize<'de>,
{
    let content = fs::read_to_string(path).map_err(|source| ContractsError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    serde_json::from_str(&content).map_err(|source| ContractsError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn has_duplicate_values<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut distinct = BTreeSet::new();

    for value in values {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            continue;
        }

        if !distinct.insert(normalized) {
            return true;
        }
    }

    false
}

fn validate_agent_messages(messages: &[AgentMessage], label: &str, issues: &mut Vec<String>) {
    if messages.iter().any(|message| message.content.trim().is_empty()) {
        issues.push(format!("{label} must include non-empty content."));
    }

    if has_duplicate_values(
        messages
            .iter()
            .filter(|message| !message.message_id.trim().is_empty())
            .map(|message| message.message_id.as_str()),
    ) {
        issues.push(format!("{label} must not reuse message IDs."));
    }

    if messages
        .iter()
        .filter_map(|message| message.name.as_ref())
        .any(|name| name.trim().is_empty())
    {
        issues.push(format!("{label} must not contain blank message names."));
    }
}

fn has_blank_metadata_entries(metadata: &BTreeMap<String, String>) -> bool {
    metadata
        .iter()
        .any(|(key, value)| key.trim().is_empty() || value.trim().is_empty())
}

fn has_negative_usage(usage: &AgentUsage) -> bool {
    usage.input_tokens.is_some_and(|value| value < 0)
        || usage.output_tokens.is_some_and(|value| value < 0)
        || usage.total_tokens.is_some_and(|value| value < 0)
}

fn usage_total_is_inconsistent(usage: &AgentUsage) -> bool {
    let Some(total_tokens) = usage.total_tokens else {
        return false;
    };

    usage.input_tokens.is_some_and(|value| value > total_tokens)
        || usage.output_tokens.is_some_and(|value| value > total_tokens)
}
