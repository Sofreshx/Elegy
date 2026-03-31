use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

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
    #[error("failed to write archive {path}: {source}")]
    Archive {
        path: PathBuf,
        #[source]
        source: zip::result::ZipError,
    },
    #[error("compatibility manifest is missing schema '{0}'")]
    MissingSchema(String),
    #[error("{0}")]
    Compatibility(String),
}

const SUPPLEMENTAL_FIXTURE_FILES: &[&str] = &[
    "fixtures/mcp-server-descriptor.parity.json",
    "fixtures/mcp-analysis-result.parity.json",
    "fixtures/mcp-parity-expected.json",
];

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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StructuredFailure {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub category: StructuredFailureCategory,
    pub retryable: bool,
    pub correlation_id: Option<String>,
    pub details: Option<Value>,
    pub cause: Option<StructuredFailureCause>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StructuredFailureCategory {
    InvalidInput,
    Policy,
    Authentication,
    Authorization,
    Timeout,
    Dependency,
    Unavailable,
    Conflict,
    Internal,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StructuredFailureCause {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InvocationRequest {
    pub request_id: String,
    pub capability_id: String,
    pub input: Value,
    #[serde(default)]
    pub context: InvocationContext,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InvocationContext {
    pub correlation_id: String,
    pub execution_id: String,
    pub requested_at: String,
    pub timeout_seconds: Option<i32>,
    pub caller_ref: Option<String>,
    pub policy_context: Option<BTreeMap<String, String>>,
    pub trace_ref: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InvocationResponse {
    pub request_id: String,
    pub execution_id: String,
    #[serde(default)]
    pub status: InvocationStatus,
    pub output: Option<Value>,
    pub failure: Option<StructuredFailure>,
    pub completed_at: Option<String>,
    pub trace_ref: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InvocationStatus {
    #[default]
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionEvent {
    pub event_id: String,
    pub request_id: String,
    pub execution_id: String,
    pub sequence: u64,
    pub timestamp: String,
    #[serde(default)]
    pub event_type: ExecutionEventType,
    #[serde(default)]
    pub status: ExecutionEventStatus,
    pub correlation_id: Option<String>,
    pub trace_ref: Option<String>,
    pub capability_id: Option<String>,
    pub message: Option<String>,
    pub progress: Option<ExecutionProgress>,
    pub failure: Option<StructuredFailure>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionEventType {
    #[default]
    Accepted,
    Started,
    Progress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionEventStatus {
    #[default]
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionProgress {
    pub current: u64,
    pub total: u64,
    pub unit: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityDefinition {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub description: Option<String>,
    #[serde(default)]
    pub family: CapabilityFamily,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub input: CapabilitySchemaReference,
    #[serde(default)]
    pub output: CapabilitySchemaReference,
    #[serde(default)]
    pub execution: CapabilityExecutionContract,
    #[serde(default)]
    pub governance: CapabilityGovernance,
    #[serde(default)]
    pub source: CapabilitySource,
    #[serde(default)]
    pub observability: CapabilityObservability,
    #[serde(default)]
    pub lifecycle_state: CapabilityLifecycleState,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CapabilityFamily {
    #[default]
    Skill,
    McpTool,
    WorkflowNode,
    RetrievalSource,
    Adapter,
    Custom,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitySchemaReference {
    pub schema: Option<Value>,
    pub schema_ref: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityExecutionContract {
    #[serde(default)]
    pub side_effect_class: CapabilitySideEffectClass,
    #[serde(default)]
    pub auth_mode: CapabilityAuthMode,
    #[serde(default)]
    pub idempotence: CapabilityIdempotenceHint,
    #[serde(default)]
    pub cost_hint: CapabilityCostHint,
    #[serde(default)]
    pub latency_hint: CapabilityLatencyHint,
    pub timeout_seconds: Option<i32>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CapabilitySideEffectClass {
    #[default]
    None,
    Read,
    Write,
    External,
    Destructive,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CapabilityAuthMode {
    #[default]
    None,
    Delegated,
    User,
    Service,
    Environment,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CapabilityIdempotenceHint {
    #[default]
    Unknown,
    Conditional,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CapabilityCostHint {
    #[default]
    Unknown,
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CapabilityLatencyHint {
    #[default]
    Unknown,
    Interactive,
    Background,
    Batch,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityGovernance {
    #[serde(default)]
    pub trust_level: CapabilityTrustLevel,
    #[serde(default)]
    pub approval_requirement: CapabilityApprovalRequirement,
    #[serde(default)]
    pub policy_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CapabilityTrustLevel {
    Untrusted,
    Sandboxed,
    #[default]
    Trusted,
    Privileged,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CapabilityApprovalRequirement {
    #[default]
    None,
    Advisory,
    Required,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitySource {
    #[serde(default)]
    pub source_kind: CapabilitySourceKind,
    pub source_ref: Option<String>,
    pub artifact_ref: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CapabilitySourceKind {
    #[default]
    Manual,
    Imported,
    Generated,
    Projected,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityObservability {
    #[serde(default)]
    pub labels: Vec<String>,
    pub correlation_required: bool,
    pub emits_execution_events: bool,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CapabilityLifecycleState {
    #[default]
    Draft,
    Active,
    Deprecated,
    Archived,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContractsBundleExport {
    pub output_path: PathBuf,
    pub archive_path: Option<PathBuf>,
    pub package_version: String,
    pub schema_version: String,
    pub files: Vec<PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct VersionPolicyDocument {
    bundle_version: String,
    schema_version: String,
    manifest_package: ContractPackage,
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StructuredFailureValidationResult {
    pub issues: Vec<String>,
}

impl StructuredFailureValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InvocationValidationResult {
    pub issues: Vec<String>,
}

impl InvocationValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecutionEventValidationResult {
    pub issues: Vec<String>,
}

impl ExecutionEventValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CapabilityValidationResult {
    pub issues: Vec<String>,
}

impl CapabilityValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

pub fn default_support_manifest_path() -> PathBuf {
    resolve_contracts_source_dir()
        .join("support")
        .join("elegy-rust-support.json")
}

pub fn resolve_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}

pub fn resolve_contracts_source_dir() -> PathBuf {
    resolve_repo_root().join("contracts")
}

pub fn default_contracts_output_dir() -> PathBuf {
    resolve_repo_root().join("artifacts").join("contracts")
}

pub fn resolve_upstream_contracts_dir() -> PathBuf {
    if let Some(path) = env::var_os("ELEGY_CONTRACTS_DIR") {
        return PathBuf::from(path);
    }

    let source_contracts = resolve_contracts_source_dir();
    if source_contracts
        .join("manifests")
        .join("compatibility-manifest.json")
        .is_file()
    {
        return source_contracts;
    }

    let exported_contracts = default_contracts_output_dir();
    if exported_contracts
        .join("compatibility-manifest.json")
        .is_file()
    {
        return exported_contracts;
    }

    source_contracts
}

pub fn load_compatibility_manifest_from_dir(
    dir: &Path,
) -> Result<CompatibilityManifest, ContractsError> {
    let bundled_manifest = dir.join("compatibility-manifest.json");
    if bundled_manifest.is_file() {
        return load_json_file(&bundled_manifest);
    }

    load_json_file(&dir.join("manifests").join("compatibility-manifest.json"))
}

pub fn export_contract_bundle(
    output_dir: Option<&Path>,
    create_archive: bool,
    archive_output_path: Option<&Path>,
) -> Result<ContractsBundleExport, ContractsError> {
    let repo_root = resolve_repo_root();
    let contracts_source_dir = resolve_contracts_source_dir();
    let version_policy_path = repo_root.join("governance").join("version-policy.json");
    let support_manifest_path = default_support_manifest_path();
    let compatibility_manifest_path = contracts_source_dir
        .join("manifests")
        .join("compatibility-manifest.json");
    let compatibility_matrix_path = contracts_source_dir
        .join("manifests")
        .join("compatibility-matrix.json");

    for required_path in [
        &support_manifest_path,
        &compatibility_manifest_path,
        &compatibility_matrix_path,
        &version_policy_path,
    ] {
        require_file(required_path)?;
    }

    let version_policy = load_version_policy_document(&version_policy_path)?;
    let bundle_version = version_policy.bundle_version.clone();
    let package_version = version_policy.manifest_package.version.clone();
    let schema_version = version_policy.schema_version.clone();
    let compatibility_manifest = load_compatibility_manifest_from_dir(&contracts_source_dir)?;
    let compatibility_matrix: Value = load_json_file(&compatibility_matrix_path)?;

    if compatibility_manifest.package.name != version_policy.manifest_package.name {
        return Err(ContractsError::Compatibility(format!(
            "compatibility manifest package name '{}' does not match governance/version-policy.json manifest package name '{}'",
            compatibility_manifest.package.name, version_policy.manifest_package.name
        )));
    }

    if compatibility_manifest.package.version != package_version {
        return Err(ContractsError::Compatibility(format!(
            "compatibility manifest package version '{}' does not match governance/version-policy.json manifest package version '{}'",
            compatibility_manifest.package.version, package_version
        )));
    }

    let canonical_schema_manifest = compatibility_manifest
        .schemas
        .iter()
        .find(|entry| entry.name == "canonical-workflow")
        .ok_or_else(|| {
            ContractsError::Compatibility(
                "compatibility manifest is missing the canonical-workflow entry".to_string(),
            )
        })?;

    if canonical_schema_manifest.schema_version != schema_version {
        return Err(ContractsError::Compatibility(format!(
            "compatibility manifest schema version '{}' does not match governance/version-policy.json schemaVersion '{}'",
            canonical_schema_manifest.schema_version, schema_version
        )));
    }

    if compatibility_matrix
        .get("matrixVersion")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        return Err(ContractsError::Compatibility(
            "compatibility matrix is missing matrixVersion".to_string(),
        ));
    }

    if compatibility_matrix
        .get("entries")
        .and_then(Value::as_array)
        .is_none_or(|entries| entries.is_empty())
    {
        return Err(ContractsError::Compatibility(
            "compatibility matrix must include at least one entry".to_string(),
        ));
    }

    let mut relative_files = BTreeSet::new();
    for schema_entry in &compatibility_manifest.schemas {
        relative_files.insert(PathBuf::from(&schema_entry.file));
        for fixture in &schema_entry.fixtures {
            relative_files.insert(PathBuf::from(fixture));
        }
    }

    for fixture in SUPPLEMENTAL_FIXTURE_FILES {
        relative_files.insert(PathBuf::from(fixture));
    }

    relative_files.insert(PathBuf::from("compatibility-manifest.json"));
    relative_files.insert(PathBuf::from("compatibility-matrix.json"));

    for relative_path in &relative_files {
        require_file(&resolve_contracts_source_path(
            &contracts_source_dir,
            relative_path,
        ))?;
    }

    let output_path = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(default_contracts_output_dir);

    if output_path.exists() {
        fs::remove_dir_all(&output_path).map_err(|source| ContractsError::Io {
            path: output_path.clone(),
            source,
        })?;
    }

    fs::create_dir_all(&output_path).map_err(|source| ContractsError::Io {
        path: output_path.clone(),
        source,
    })?;

    let mut exported_files = Vec::new();
    for relative_path in &relative_files {
        let source_path = resolve_contracts_source_path(&contracts_source_dir, relative_path);
        let destination_path = output_path.join(relative_path);

        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent).map_err(|source| ContractsError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        fs::copy(&source_path, &destination_path).map_err(|source| ContractsError::Io {
            path: destination_path.clone(),
            source,
        })?;
        exported_files.push(destination_path);
    }
    exported_files.sort();

    let rust_support_mirror_path = repo_root
        .join("rust")
        .join("contracts")
        .join("elegy-rust-support.json");
    if let Some(parent) = rust_support_mirror_path.parent() {
        fs::create_dir_all(parent).map_err(|source| ContractsError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::copy(&support_manifest_path, &rust_support_mirror_path).map_err(|source| {
        ContractsError::Io {
            path: rust_support_mirror_path.clone(),
            source,
        }
    })?;

    let archive_path = if create_archive || archive_output_path.is_some() {
        let resolved_archive_path = archive_output_path
            .map(Path::to_path_buf)
            .unwrap_or_else(|| default_contracts_archive_path(&repo_root, &bundle_version));
        write_contract_archive(&resolved_archive_path, &output_path, &relative_files)?;
        Some(resolved_archive_path)
    } else {
        None
    };

    Ok(ContractsBundleExport {
        output_path,
        archive_path,
        package_version,
        schema_version,
        files: exported_files,
    })
}

pub fn load_consumer_support_manifest(
    path: &Path,
) -> Result<ConsumerSupportManifest, ContractsError> {
    load_json_file(path)
}

pub fn load_structured_failure_fixture_from_dir(
    dir: &Path,
) -> Result<StructuredFailure, ContractsError> {
    load_json_file(&dir.join("fixtures").join("structured-failure.minimal.json"))
}

pub fn load_invocation_request_fixture_from_dir(
    dir: &Path,
) -> Result<InvocationRequest, ContractsError> {
    load_json_file(&dir.join("fixtures").join("invocation-request.minimal.json"))
}

pub fn load_invocation_response_fixture_from_dir(
    dir: &Path,
) -> Result<InvocationResponse, ContractsError> {
    load_json_file(&dir.join("fixtures").join("invocation-response.minimal.json"))
}

pub fn load_execution_event_fixture_from_dir(
    dir: &Path,
) -> Result<ExecutionEvent, ContractsError> {
    load_json_file(&dir.join("fixtures").join("execution-event.minimal.json"))
}

pub fn load_capability_definition_fixture_from_dir(
    dir: &Path,
) -> Result<CapabilityDefinition, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("capability-definition.minimal.json"),
    )
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

pub fn validate_structured_failure(
    failure: &StructuredFailure,
) -> StructuredFailureValidationResult {
    let mut issues = Vec::new();

    if failure.code.trim().is_empty() {
        issues.push("Structured failure code must not be blank.".to_string());
    }

    if failure.message.trim().is_empty() {
        issues.push("Structured failure message must not be blank.".to_string());
    }

    if failure
        .correlation_id
        .as_deref()
        .is_some_and(str::is_empty)
    {
        issues.push("Structured failure correlationId must not be blank when provided.".to_string());
    }

    if failure.details.as_ref().is_some_and(|details| !details.is_object()) {
        issues.push("Structured failure details must be a JSON object when provided.".to_string());
    }

    if let Some(cause) = &failure.cause {
        if cause.code.trim().is_empty() {
            issues.push("Structured failure cause code must not be blank.".to_string());
        }

        if cause.message.trim().is_empty() {
            issues.push("Structured failure cause message must not be blank.".to_string());
        }
    }

    StructuredFailureValidationResult { issues }
}

pub fn validate_invocation_request(request: &InvocationRequest) -> InvocationValidationResult {
    let mut issues = Vec::new();

    if request.request_id.trim().is_empty() {
        issues.push("Invocation request must declare a requestId.".to_string());
    }

    if request.capability_id.trim().is_empty() {
        issues.push("Invocation request must declare a capabilityId.".to_string());
    }

    if !request.input.is_object() {
        issues.push("Invocation request input must be a JSON object.".to_string());
    }

    if request.context.correlation_id.trim().is_empty() {
        issues.push("Invocation request context must declare a correlationId.".to_string());
    }

    if request.context.execution_id.trim().is_empty() {
        issues.push("Invocation request context must declare an executionId.".to_string());
    }

    if request.context.requested_at.trim().is_empty() {
        issues.push("Invocation request context must declare requestedAt.".to_string());
    }

    if request
        .context
        .timeout_seconds
        .is_some_and(|timeout| timeout <= 0)
    {
        issues.push("Invocation request timeoutSeconds must be greater than zero when set.".to_string());
    }

    if request.context.caller_ref.as_deref().is_some_and(str::is_empty) {
        issues.push("Invocation request callerRef must not be blank when provided.".to_string());
    }

    if request.context.trace_ref.as_deref().is_some_and(str::is_empty) {
        issues.push("Invocation request traceRef must not be blank when provided.".to_string());
    }

    if request
        .context
        .policy_context
        .as_ref()
        .is_some_and(has_blank_metadata_entries)
    {
        issues.push("Invocation request policyContext must not contain blank keys or values.".to_string());
    }

    if has_blank_metadata_entries(&request.context.metadata) {
        issues.push("Invocation request metadata must not contain blank keys or values.".to_string());
    }

    InvocationValidationResult { issues }
}

pub fn validate_invocation_response(response: &InvocationResponse) -> InvocationValidationResult {
    let mut issues = Vec::new();

    if response.request_id.trim().is_empty() {
        issues.push("Invocation response must declare a requestId.".to_string());
    }

    if response.execution_id.trim().is_empty() {
        issues.push("Invocation response must declare an executionId.".to_string());
    }

    if response.trace_ref.as_deref().is_some_and(str::is_empty) {
        issues.push("Invocation response traceRef must not be blank when provided.".to_string());
    }

    if has_blank_metadata_entries(&response.metadata) {
        issues.push("Invocation response metadata must not contain blank keys or values.".to_string());
    }

    if matches!(response.status, InvocationStatus::Completed) && response.output.is_none() {
        issues.push("Completed invocation responses must include an output payload.".to_string());
    }

    if !matches!(response.status, InvocationStatus::Completed) && response.failure.is_none() {
        issues.push("Failed or cancelled invocation responses must include a structured failure.".to_string());
    }

    if let Some(failure) = &response.failure {
        issues.extend(validate_structured_failure(failure).issues);
    }

    InvocationValidationResult { issues }
}

pub fn validate_execution_event(event: &ExecutionEvent) -> ExecutionEventValidationResult {
    let mut issues = Vec::new();

    if event.event_id.trim().is_empty() {
        issues.push("Execution event must declare an eventId.".to_string());
    }

    if event.request_id.trim().is_empty() {
        issues.push("Execution event must declare a requestId.".to_string());
    }

    if event.execution_id.trim().is_empty() {
        issues.push("Execution event must declare an executionId.".to_string());
    }

    if event.sequence == 0 {
        issues.push("Execution event sequence must be greater than zero.".to_string());
    }

    if event.timestamp.trim().is_empty() {
        issues.push("Execution event must declare a timestamp.".to_string());
    }

    if event.correlation_id.as_deref().is_some_and(str::is_empty) {
        issues.push("Execution event correlationId must not be blank when provided.".to_string());
    }

    if event.trace_ref.as_deref().is_some_and(str::is_empty) {
        issues.push("Execution event traceRef must not be blank when provided.".to_string());
    }

    if event.capability_id.as_deref().is_some_and(str::is_empty) {
        issues.push("Execution event capabilityId must not be blank when provided.".to_string());
    }

    if event.message.as_deref().is_some_and(str::is_empty) {
        issues.push("Execution event message must not be blank when provided.".to_string());
    }

    if has_blank_metadata_entries(&event.metadata) {
        issues.push("Execution event metadata must not contain blank keys or values.".to_string());
    }

    if let Some(progress) = &event.progress {
        if progress.total < progress.current {
            issues.push("Execution event progress total must be greater than or equal to current.".to_string());
        }

        if progress.unit.as_deref().is_some_and(str::is_empty) {
            issues.push("Execution event progress unit must not be blank when provided.".to_string());
        }
    }

    if let Some(failure) = &event.failure {
        issues.extend(validate_structured_failure(failure).issues);
    }

    if matches!(
        event.event_type,
        ExecutionEventType::Failed | ExecutionEventType::Cancelled
    ) && event.failure.is_none()
    {
        issues.push("Failed or cancelled execution events must include a structured failure.".to_string());
    }

    ExecutionEventValidationResult { issues }
}

pub fn validate_capability_definition(
    definition: &CapabilityDefinition,
) -> CapabilityValidationResult {
    let mut issues = Vec::new();

    if definition.id.trim().is_empty() {
        issues.push("Capability definition id must not be blank.".to_string());
    }

    if definition.display_name.trim().is_empty() {
        issues.push("Capability definition displayName must not be blank.".to_string());
    }

    if definition.version.trim().is_empty() {
        issues.push("Capability definition version must not be blank.".to_string());
    }

    if definition.tags.iter().any(|tag| tag.trim().is_empty()) {
        issues.push("Capability definition tags must not be blank.".to_string());
    }

    if definition.governance.approval_requirement == CapabilityApprovalRequirement::Required
        && definition.governance.policy_refs.is_empty()
    {
        issues.push(
            "Capabilities that require approval must declare at least one policy reference."
                .to_string(),
        );
    }

    if definition
        .governance
        .policy_refs
        .iter()
        .any(|policy_ref| policy_ref.trim().is_empty())
    {
        issues.push("Capability policy references must not be blank.".to_string());
    }

    if definition
        .observability
        .labels
        .iter()
        .any(|label| label.trim().is_empty())
    {
        issues.push("Capability observability labels must not be blank.".to_string());
    }

    if definition
        .execution
        .timeout_seconds
        .is_some_and(|timeout| timeout <= 0)
    {
        issues.push("Capability timeoutSeconds must be greater than zero when set.".to_string());
    }

    if definition
        .source
        .source_ref
        .as_deref()
        .is_some_and(str::is_empty)
    {
        issues.push("Capability sourceRef must not be blank when provided.".to_string());
    }

    if definition
        .source
        .artifact_ref
        .as_deref()
        .is_some_and(str::is_empty)
    {
        issues.push("Capability artifactRef must not be blank when provided.".to_string());
    }

    let source_ref_present = definition
        .source
        .source_ref
        .as_deref()
        .is_some_and(|source_ref| !source_ref.trim().is_empty());
    let artifact_ref_present = definition
        .source
        .artifact_ref
        .as_deref()
        .is_some_and(|artifact_ref| !artifact_ref.trim().is_empty());

    if definition.source.source_kind != CapabilitySourceKind::Manual
        && !source_ref_present
        && !artifact_ref_present
    {
        issues.push(
            "Imported, generated, or projected capabilities must declare a sourceRef or artifactRef."
                .to_string(),
        );
    }

    CapabilityValidationResult { issues }
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
                issues.push("Delta agent events must include non-empty delta content.".to_string());
            }
        }
        AgentEventType::MessageCompleted | AgentEventType::ReasoningCompleted => {
            if event
                .payload
                .content
                .as_ref()
                .is_none_or(|content| content.trim().is_empty())
            {
                issues.push("Completed message events must include non-empty content.".to_string());
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
                issues.push("Tool call completed events must include a tool call ID.".to_string());
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
                    "Error agent events must include an error code or error message.".to_string(),
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

fn require_file(path: &Path) -> Result<(), ContractsError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(ContractsError::Compatibility(format!(
            "missing required file: {}",
            path.display()
        )))
    }
}

fn load_version_policy_document(path: &Path) -> Result<VersionPolicyDocument, ContractsError> {
    load_json_file(path)
}

fn resolve_contracts_source_path(contracts_root: &Path, relative_path: &Path) -> PathBuf {
    let direct_path = contracts_root.join(relative_path);
    if direct_path.is_file() {
        return direct_path;
    }

    let schema_path = contracts_root.join("schemas").join(relative_path);
    if schema_path.is_file() {
        return schema_path;
    }

    contracts_root.join("manifests").join(relative_path)
}

fn default_contracts_archive_path(repo_root: &Path, bundle_version: &str) -> PathBuf {
    repo_root
        .join("artifacts")
        .join("distribution")
        .join(format!("elegy-contracts-{bundle_version}.zip"))
}

fn write_contract_archive(
    archive_path: &Path,
    output_path: &Path,
    relative_files: &BTreeSet<PathBuf>,
) -> Result<(), ContractsError> {
    if let Some(parent) = archive_path.parent() {
        fs::create_dir_all(parent).map_err(|source| ContractsError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let archive_file = fs::File::create(archive_path).map_err(|source| ContractsError::Io {
        path: archive_path.to_path_buf(),
        source,
    })?;
    let mut archive = zip::ZipWriter::new(archive_file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    for relative_path in relative_files {
        let bundle_path = output_path.join(relative_path);
        let archive_name = relative_path.to_string_lossy().replace('\\', "/");
        archive
            .start_file(&archive_name, options)
            .map_err(|source| ContractsError::Archive {
                path: archive_path.to_path_buf(),
                source,
            })?;
        let file_bytes = fs::read(&bundle_path).map_err(|source| ContractsError::Io {
            path: bundle_path,
            source,
        })?;
        archive
            .write_all(&file_bytes)
            .map_err(|source| ContractsError::Io {
                path: archive_path.to_path_buf(),
                source,
            })?;
    }

    archive.finish().map_err(|source| ContractsError::Archive {
        path: archive_path.to_path_buf(),
        source,
    })?;

    Ok(())
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
    if messages
        .iter()
        .any(|message| message.content.trim().is_empty())
    {
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
        || usage
            .output_tokens
            .is_some_and(|value| value > total_tokens)
}
