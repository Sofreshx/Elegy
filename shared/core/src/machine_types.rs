// ── Machine types (StructuredFailure, Invocation, Execution, Observation, Agent) ──

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Compatibility Manifest ───────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityManifest {
    pub manifest_version: String,
    pub package: ContractPackage,
    pub schemas: Vec<SchemaEntry>,
    #[serde(default)]
    pub supplemental_fixtures: Vec<String>,
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

pub const AGENT_CAPABILITY_PROFILE_SCHEMA_VERSION: &str = "agent-capability-profile/v1";

// ── Agent Capability Profile ──────────────────────────────────────────────

fn default_agent_profile_always_include_router() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilityProfile {
    pub schema_version: String,
    pub profile_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include_skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include_capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_capabilities: Vec<String>,
    #[serde(default = "default_agent_profile_always_include_router")]
    pub always_include_router: bool,
}

impl Default for AgentCapabilityProfile {
    fn default() -> Self {
        Self {
            schema_version: AGENT_CAPABILITY_PROFILE_SCHEMA_VERSION.to_string(),
            profile_id: String::new(),
            include_skills: Vec::new(),
            include_capabilities: Vec::new(),
            exclude_capabilities: Vec::new(),
            always_include_router: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentCapabilityProfileValidationResult {
    pub issues: Vec<String>,
}

impl AgentCapabilityProfileValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

pub fn validate_agent_capability_profile(
    profile: &AgentCapabilityProfile,
) -> AgentCapabilityProfileValidationResult {
    let mut issues = Vec::new();

    if profile.schema_version != AGENT_CAPABILITY_PROFILE_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{AGENT_CAPABILITY_PROFILE_SCHEMA_VERSION}'."
        ));
    }
    if profile.profile_id.trim().is_empty() {
        issues.push("profileId must not be empty.".to_string());
    }

    for (field, values) in [
        ("includeSkills", &profile.include_skills),
        ("includeCapabilities", &profile.include_capabilities),
        ("excludeCapabilities", &profile.exclude_capabilities),
    ] {
        let mut seen = BTreeSet::new();
        for value in values {
            if value.trim().is_empty() {
                issues.push(format!("{field} entries must not be empty."));
            }
            let normalized = value.to_ascii_lowercase();
            if !seen.insert(normalized) {
                issues.push(format!(
                    "{field} must not contain duplicate entry '{value}'."
                ));
            }
        }
    }

    AgentCapabilityProfileValidationResult { issues }
}

// ── Agent Skill Frontmatter ───────────────────────────────────────────────

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSkillFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
}

pub fn parse_agent_skill_frontmatter(
    content: &str,
) -> Result<(AgentSkillFrontmatter, String), String> {
    let content = content.trim_start();
    if !content.starts_with("---") {
        return Err("Content must start with a '---' frontmatter fence.".into());
    }

    let after_open = &content[3..];
    let after_newline = after_open
        .strip_prefix('\n')
        .or_else(|| after_open.strip_prefix("\r\n"))
        .ok_or_else(|| "Opening '---' must be followed by a newline.".to_string())?;

    let close_pos = after_newline
        .find("\n---")
        .or_else(|| after_newline.find("\r\n---"))
        .ok_or_else(|| "Missing closing '---' frontmatter fence.".to_string())?;

    let yaml_str = &after_newline[..close_pos];
    let remainder_start = close_pos
        + if after_newline[close_pos..].starts_with("\r\n---") {
            5
        } else {
            4
        };
    let body = after_newline[remainder_start..].trim_start().to_string();

    let frontmatter: AgentSkillFrontmatter = serde_yaml::from_str(yaml_str)
        .map_err(|e| format!("Failed to parse YAML frontmatter: {e}"))?;

    Ok((frontmatter, body))
}

pub fn validate_agent_skill_frontmatter(frontmatter: &AgentSkillFrontmatter) -> Vec<String> {
    let mut issues = Vec::new();
    if frontmatter.name.trim().is_empty() {
        issues.push("Skill name must not be empty.".into());
    } else if !validate_kebab_case_name(&frontmatter.name) {
        issues.push(format!(
            "Skill name '{}' is not valid lowercase kebab-case.",
            frontmatter.name
        ));
    }
    if frontmatter.description.trim().is_empty() {
        issues.push("Skill description must not be empty.".into());
    }
    issues
}

// ── Path / Name helpers ───────────────────────────────────────────────────

pub fn is_safe_package_relative_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return false;
    }
    if path.len() >= 2 && path.as_bytes()[1] == b':' {
        return false;
    }
    let bytes = path.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if i + 1 < len && bytes[i] == b'.' && bytes[i + 1] == b'.' {
            let before_is_boundary = i == 0 || bytes[i - 1] == b'/' || bytes[i - 1] == b'\\';
            let after_is_boundary = i + 2 >= len || bytes[i + 2] == b'/' || bytes[i + 2] == b'\\';
            if before_is_boundary && after_is_boundary {
                return false;
            }
        }
        i += 1;
    }
    true
}

pub fn validate_kebab_case_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let bytes = name.as_bytes();
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

pub fn validate_semver(version: &str) -> bool {
    semver::Version::parse(version).is_ok()
}

// ── URI validation ────────────────────────────────────────────────────────

pub fn validate_uri(field: &str, value: &str, issues: &mut Vec<String>) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }

    match url::Url::parse(value) {
        Ok(url) if !url.scheme().is_empty() => {}
        _ => issues.push(format!("{field} must be a valid URI.")),
    }
}

// ── Agent Message types ───────────────────────────────────────────────────

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

// ── Structured Failure ────────────────────────────────────────────────────

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

// ── Invocation ────────────────────────────────────────────────────────────

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

// ── Execution Event ───────────────────────────────────────────────────────

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

// ── Observation types ─────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObservationSummary {
    #[serde(default)]
    pub scope: ObservationScope,
    #[serde(default)]
    pub representation: ObservationRepresentation,
    pub summary: String,
    pub observation_count: u64,
    #[serde(default)]
    pub observation_kinds: BTreeMap<String, u64>,
    #[serde(default)]
    pub salient_events: Vec<ObservationSalientEvent>,
    pub time_range: Option<ObservationTimeRange>,
    pub token_estimate: Option<ObservationTokenEstimate>,
    pub raw_events_persisted: bool,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ObservationScope {
    Run,
    #[default]
    Session,
    Workspace,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ObservationRepresentation {
    #[default]
    ObservationSummary,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObservationSalientEvent {
    pub kind: String,
    pub summary: String,
    pub count: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObservationTimeRange {
    pub started_at_utc: String,
    pub ended_at_utc: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObservationTokenEstimate {
    pub summary_chars: u64,
    pub salient_event_chars: u64,
    pub total: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObservationBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObservationWindow {
    pub hwnd: u64,
    pub title: String,
    pub process_id: u32,
    pub bounds: ObservationBounds,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObservationEvent {
    pub event_id: String,
    pub session_id: String,
    pub sequence: u64,
    pub observed_at_utc: String,
    #[serde(default)]
    pub observation_kind: ObservationKind,
    pub summary: String,
    pub window: Option<ObservationWindow>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ObservationKind {
    #[default]
    ForegroundWindowChanged,
    VisibleWindowSnapshot,
    ClipboardChanged,
    ProcessSnapshot,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObservationSession {
    pub artifact_kind: String,
    pub session_id: String,
    #[serde(default)]
    pub scope: ObservationScope,
    #[serde(default)]
    pub recorder_kind: ObservationRecorderKind,
    pub opened_at_utc: String,
    pub closed_at_utc: String,
    pub duration_seconds: Option<u64>,
    pub poll_interval_ms: Option<u64>,
    pub event_count: u64,
    #[serde(default)]
    pub events_preview: Vec<ObservationEvent>,
    #[serde(default)]
    pub summary: ObservationSummary,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ObservationRecorderKind {
    #[default]
    ForegroundWindowPolling,
}

// ── Validation result types ───────────────────────────────────────────────

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
pub struct ObservationValidationResult {
    pub issues: Vec<String>,
}

impl ObservationValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

// ── Helper: metadata checks ───────────────────────────────────────────────

pub fn has_blank_metadata_entries(metadata: &BTreeMap<String, String>) -> bool {
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

// ── Structured Failure validation ─────────────────────────────────────────

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

    if failure.correlation_id.as_deref().is_some_and(str::is_empty) {
        issues
            .push("Structured failure correlationId must not be blank when provided.".to_string());
    }

    if failure
        .details
        .as_ref()
        .is_some_and(|details| !details.is_object())
    {
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

// ── Invocation validation ─────────────────────────────────────────────────

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
        issues.push(
            "Invocation request timeoutSeconds must be greater than zero when set.".to_string(),
        );
    }

    if request
        .context
        .caller_ref
        .as_deref()
        .is_some_and(str::is_empty)
    {
        issues.push("Invocation request callerRef must not be blank when provided.".to_string());
    }

    if request
        .context
        .trace_ref
        .as_deref()
        .is_some_and(str::is_empty)
    {
        issues.push("Invocation request traceRef must not be blank when provided.".to_string());
    }

    if request
        .context
        .policy_context
        .as_ref()
        .is_some_and(has_blank_metadata_entries)
    {
        issues.push(
            "Invocation request policyContext must not contain blank keys or values.".to_string(),
        );
    }

    if has_blank_metadata_entries(&request.context.metadata) {
        issues
            .push("Invocation request metadata must not contain blank keys or values.".to_string());
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
        issues.push(
            "Invocation response metadata must not contain blank keys or values.".to_string(),
        );
    }

    if matches!(response.status, InvocationStatus::Completed) && response.output.is_none() {
        issues.push("Completed invocation responses must include an output payload.".to_string());
    }

    if !matches!(response.status, InvocationStatus::Completed) && response.failure.is_none() {
        issues.push(
            "Failed or cancelled invocation responses must include a structured failure."
                .to_string(),
        );
    }

    if let Some(failure) = &response.failure {
        issues.extend(validate_structured_failure(failure).issues);
    }

    InvocationValidationResult { issues }
}

// ── Execution event validation ────────────────────────────────────────────

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
            issues.push(
                "Execution event progress total must be greater than or equal to current."
                    .to_string(),
            );
        }

        if progress.unit.as_deref().is_some_and(str::is_empty) {
            issues
                .push("Execution event progress unit must not be blank when provided.".to_string());
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
        issues.push(
            "Failed or cancelled execution events must include a structured failure.".to_string(),
        );
    }

    ExecutionEventValidationResult { issues }
}

// ── Observation validation ────────────────────────────────────────────────

pub fn validate_observation_event(event: &ObservationEvent) -> ObservationValidationResult {
    let mut issues = Vec::new();

    if event.event_id.trim().is_empty() {
        issues.push("Observation event must declare an eventId.".to_string());
    }
    if event.session_id.trim().is_empty() {
        issues.push("Observation event must declare a sessionId.".to_string());
    }
    if event.sequence == 0 {
        issues.push("Observation event sequence must be greater than zero.".to_string());
    }
    if event.observed_at_utc.trim().is_empty() {
        issues.push("Observation event must declare observedAtUtc.".to_string());
    }
    if event.summary.trim().is_empty() {
        issues.push("Observation event summary must not be blank.".to_string());
    }
    if event.summary.chars().count() > 280 {
        issues.push("Observation event summary must not exceed 280 characters.".to_string());
    }
    if has_blank_metadata_entries(&event.metadata) {
        issues
            .push("Observation event metadata must not contain blank keys or values.".to_string());
    }
    if let Some(window) = &event.window {
        if window.title.trim().is_empty() {
            issues.push(
                "Observation event window title must not be blank when provided.".to_string(),
            );
        }
        if window.bounds.width < 0 || window.bounds.height < 0 {
            issues.push("Observation event window bounds must not be negative.".to_string());
        }
    }

    ObservationValidationResult { issues }
}

pub fn validate_observation_summary(summary: &ObservationSummary) -> ObservationValidationResult {
    let mut issues = Vec::new();

    if summary.summary.trim().is_empty() {
        issues.push("Observation summary text must not be blank.".to_string());
    }
    if summary.summary.chars().count() > 4000 {
        issues.push("Observation summary text must not exceed 4000 characters.".to_string());
    }
    if summary.observation_kinds.len() > 16 {
        issues.push("Observation summary observationKinds must not exceed 16 entries.".to_string());
    }
    if summary.salient_events.len() > 8 {
        issues.push("Observation summary salientEvents must not exceed 8 entries.".to_string());
    }
    for event in &summary.salient_events {
        if event.kind.trim().is_empty() {
            issues.push("Observation summary salientEvents kinds must not be blank.".to_string());
        }
        if event.summary.trim().is_empty() {
            issues
                .push("Observation summary salientEvents summaries must not be blank.".to_string());
        }
        if event.summary.chars().count() > 280 {
            issues.push(
                "Observation summary salientEvents summaries must not exceed 280 characters."
                    .to_string(),
            );
        }
    }
    if summary.raw_events_persisted {
        issues.push("Observation summary rawEventsPersisted must remain false.".to_string());
    }
    if let Some(time_range) = &summary.time_range {
        if time_range.started_at_utc.trim().is_empty() || time_range.ended_at_utc.trim().is_empty()
        {
            issues.push("Observation summary timeRange timestamps must not be blank.".to_string());
        }
    }

    ObservationValidationResult { issues }
}

pub fn validate_observation_session(session: &ObservationSession) -> ObservationValidationResult {
    let mut issues = Vec::new();

    if session.artifact_kind != "observation-session" {
        issues.push("Observation session artifactKind must be 'observation-session'.".to_string());
    }
    if session.session_id.trim().is_empty() {
        issues.push("Observation session must declare a sessionId.".to_string());
    }
    if session.opened_at_utc.trim().is_empty() || session.closed_at_utc.trim().is_empty() {
        issues.push("Observation session timestamps must not be blank.".to_string());
    }
    if session.events_preview.len() > 8 {
        issues.push("Observation session eventsPreview must not exceed 8 entries.".to_string());
    }
    if session.event_count < session.events_preview.len() as u64 {
        issues.push(
            "Observation session eventCount must be greater than or equal to preview length."
                .to_string(),
        );
    }
    if let Some(poll_interval_ms) = session.poll_interval_ms {
        if poll_interval_ms == 0 {
            issues
                .push("Observation session pollIntervalMs must be greater than zero.".to_string());
        }
    }
    if has_blank_metadata_entries(&session.metadata) {
        issues.push(
            "Observation session metadata must not contain blank keys or values.".to_string(),
        );
    }
    for event in &session.events_preview {
        issues.extend(validate_observation_event(event).issues);
    }
    issues.extend(validate_observation_summary(&session.summary).issues);

    ObservationValidationResult { issues }
}

// ── Agent envelope validation ─────────────────────────────────────────────

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
        AgentEventType::MessageDelta | AgentEventType::ReasoningDelta
            if event
                .payload
                .delta_content
                .as_ref()
                .is_none_or(|delta| delta.trim().is_empty()) =>
        {
            issues.push("Delta agent events must include non-empty delta content.".to_string());
        }
        AgentEventType::MessageCompleted | AgentEventType::ReasoningCompleted
            if event
                .payload
                .content
                .as_ref()
                .is_none_or(|content| content.trim().is_empty()) =>
        {
            issues.push("Completed message events must include non-empty content.".to_string());
        }
        AgentEventType::ToolCallStarted
            if event
                .payload
                .tool_call_id
                .as_ref()
                .is_none_or(|tool_call_id| tool_call_id.trim().is_empty())
                || event
                    .payload
                    .tool_name
                    .as_ref()
                    .is_none_or(|tool_name| tool_name.trim().is_empty()) =>
        {
            issues.push(
                "Tool call started events must include a tool call ID and tool name.".to_string(),
            );
        }
        AgentEventType::ToolCallCompleted
            if event
                .payload
                .tool_call_id
                .as_ref()
                .is_none_or(|tool_call_id| tool_call_id.trim().is_empty()) =>
        {
            issues.push("Tool call completed events must include a tool call ID.".to_string());
        }
        AgentEventType::Error | AgentEventType::RunFailed
            if event
                .payload
                .error_message
                .as_ref()
                .is_none_or(|message| message.trim().is_empty())
                && event
                    .payload
                    .error_code
                    .as_ref()
                    .is_none_or(|code| code.trim().is_empty()) =>
        {
            issues.push(
                "Error agent events must include an error code or error message.".to_string(),
            );
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
