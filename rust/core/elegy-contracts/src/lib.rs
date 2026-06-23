mod configuration;

pub use configuration::*;

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use url::Url;
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

/// Schema version constant for all Elegy CLI machine-readable envelopes.
pub const CLI_SCHEMA_VERSION: &str = "elegy.cli/v1";

/// Shared JSON envelope for all Elegy CLI machine-readable output.
///
/// Every dedicated CLI surface (`elegy-skills`, `elegy-mcp`, `elegy-planning`, etc.)
/// emits this envelope when `--json` or `--format json` is active. The envelope
/// carries the schema version, a correlation ID for event tracing, the command
/// that produced the result, and either [`data`] on success or [`failure`] on error.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CliMachineEnvelope<T>
where
    T: Serialize,
{
    pub schema_version: &'static str,
    pub correlation_id: String,
    #[serde(skip_serializing_if = "is_false")]
    pub non_interactive: bool,
    pub command: Vec<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_schema: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<StructuredFailure>,
}

/// Resolved machine-mode context shared across all Elegy CLI surfaces.
///
/// Holds the `non_interactive` flag and a resolved correlation ID (either
/// user-provided or auto-generated). Built by [`build_cli_machine_context`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliMachineContext {
    pub non_interactive: bool,
    pub correlation_id: String,
}

/// Classifies the kind of CLI failure for structured error envelopes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliFailureKind {
    /// The request was invalid (bad input, missing required field, scope mismatch).
    InvalidInput,
    /// An internal runtime error occurred.
    Runtime,
    /// The requested operation is not supported by this surface.
    Unsupported,
}

impl CliFailureKind {
    fn status(self) -> &'static str {
        match self {
            CliFailureKind::InvalidInput => "invalid",
            CliFailureKind::Runtime | CliFailureKind::Unsupported => "error",
        }
    }

    fn category(self) -> StructuredFailureCategory {
        match self {
            CliFailureKind::InvalidInput => StructuredFailureCategory::InvalidInput,
            CliFailureKind::Runtime => StructuredFailureCategory::Internal,
            CliFailureKind::Unsupported => StructuredFailureCategory::Unavailable,
        }
    }

    fn code(self) -> &'static str {
        match self {
            CliFailureKind::InvalidInput => "CLI-INVALID-INPUT",
            CliFailureKind::Runtime => "CLI-RUNTIME-FAILURE",
            CliFailureKind::Unsupported => "CLI-UNSUPPORTED",
        }
    }
}

/// Resolves a correlation ID from user input, falling back to an auto-generated
/// value with the given `prefix` when the input is `None` or blank.
pub fn resolve_cli_correlation_id(correlation_id: Option<String>, prefix: &str) -> String {
    if let Some(value) = correlation_id {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let timestamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);

    format!("{prefix}-{}-{timestamp_nanos}", std::process::id())
}

/// Builds a [`CliMachineContext`] from CLI flags, auto-generating a correlation
/// ID with the given `prefix` when one is not provided.
pub fn build_cli_machine_context(
    non_interactive: bool,
    correlation_id: Option<String>,
    prefix: &str,
) -> CliMachineContext {
    CliMachineContext {
        non_interactive,
        correlation_id: resolve_cli_correlation_id(correlation_id, prefix),
    }
}

/// Builds a success [`CliMachineEnvelope`] with `status: "ok"` and the given data.
pub fn build_cli_success_envelope<T, S>(
    context: &CliMachineContext,
    command: impl IntoIterator<Item = S>,
    data: T,
) -> CliMachineEnvelope<T>
where
    T: Serialize,
    S: Into<String>,
{
    CliMachineEnvelope {
        schema_version: CLI_SCHEMA_VERSION,
        correlation_id: context.correlation_id.clone(),
        non_interactive: context.non_interactive,
        command: command.into_iter().map(Into::into).collect(),
        status: "ok".to_string(),
        data_schema: None,
        data: Some(data),
        failure: None,
    }
}

/// Builds a failure [`CliMachineEnvelope`] with a [`StructuredFailure`] payload
/// classified by the given [`CliFailureKind`].
pub fn build_cli_failure_envelope<T, S>(
    context: &CliMachineContext,
    command: impl IntoIterator<Item = S>,
    kind: CliFailureKind,
    message: impl Into<String>,
    details: Option<Value>,
) -> CliMachineEnvelope<T>
where
    T: Serialize,
    S: Into<String>,
{
    let message = message.into();
    CliMachineEnvelope {
        schema_version: CLI_SCHEMA_VERSION,
        correlation_id: context.correlation_id.clone(),
        non_interactive: context.non_interactive,
        command: command.into_iter().map(Into::into).collect(),
        status: kind.status().to_string(),
        data_schema: None,
        data: None,
        failure: Some(StructuredFailure {
            code: kind.code().to_string(),
            message,
            category: kind.category(),
            retryable: false,
            correlation_id: Some(context.correlation_id.clone()),
            details,
            cause: None,
        }),
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

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

// ── elegy-plugin/v1 ──────────────────────────────────────────────────────

pub const ELEGY_PLUGIN_V1_SCHEMA_VERSION: &str = "elegy-plugin/v1";

/// Minimal agent capability plugin manifest (elegy-plugin/v1).
///
/// Models only identity, discovery metadata, and convention-rooted paths
/// to skills and MCP servers.  Does **not** carry publishing,
/// permissions, approvals, trust declarations, configuration systems,
/// runtime state, or release orchestration.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginV1 {
    pub schema_version: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<ElegyPluginV1Author>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// Relative path to `skills/` directory root (Agent Skills standard layout).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<String>,
    /// Relative path to `mcp/` directory root (MCP server descriptors).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginV1Author {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

// ── (Legacy plugin-package types removed in migration) ──

// ── Plugin V1 validation ──────────────────────────────────────────────────

/// Validation result for ElegyPluginV1.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ElegyPluginV1ValidationResult {
    pub issues: Vec<String>,
}

impl ElegyPluginV1ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

pub fn validate_elegy_plugin_v1(plugin: &ElegyPluginV1) -> ElegyPluginV1ValidationResult {
    let mut issues = Vec::new();

    if plugin.schema_version != ELEGY_PLUGIN_V1_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{}', found '{}'.",
            ELEGY_PLUGIN_V1_SCHEMA_VERSION, plugin.schema_version
        ));
    }

    if plugin.name.is_empty() {
        issues.push("name must not be empty.".into());
    } else if !validate_kebab_case_name(&plugin.name) {
        issues.push(format!(
            "name '{}' is not valid lowercase kebab-case (must start with a letter, contain only a-z, 0-9, hyphens).",
            plugin.name
        ));
    }

    if plugin.version.is_empty() {
        issues.push("version must not be empty.".into());
    } else if !validate_semver(&plugin.version) {
        issues.push(format!(
            "version '{}' is not valid SemVer 2.0.0.",
            plugin.version
        ));
    }

    if plugin.description.is_empty() {
        issues.push("description must not be empty.".into());
    } else if plugin.description.trim().is_empty() {
        issues.push("description must not be only whitespace.".into());
    }

    // Validate relative paths for skills, mcpServers
    for (field_name, path) in &[
        ("skills", &plugin.skills),
        ("mcpServers", &plugin.mcp_servers),
    ] {
        if let Some(p) = path {
            if !is_safe_package_relative_path(p) {
                issues.push(format!(
                    "{field_name} path '{p}' is not a safe package-relative path.",
                ));
            }
        }
    }

    // Author validation
    if let Some(author) = &plugin.author {
        if author.name.trim().is_empty() {
            issues.push("author.name must not be empty when author is present.".into());
        }
        if let Some(url) = &author.url {
            validate_uri("author.url", url, &mut issues);
        }
        if author.email.as_deref().is_some_and(|e| e.trim().is_empty()) {
            issues.push("author.email must not be empty.".into());
        }
    }

    // Repository validation
    if let Some(repo) = &plugin.repository {
        validate_uri("repository", repo, &mut issues);
    }

    // At least one of skills or mcpServers required
    if plugin.skills.is_none() && plugin.mcp_servers.is_none() {
        issues.push("At least one of skills or mcpServers must be declared.".into());
    }

    // Extensions validation
    if let Some(extensions) = &plugin.extensions {
        if !extensions.is_empty() {
            for (key, value) in extensions {
                if !key.contains('.') {
                    issues.push(format!(
                        "Extension key '{key}' must be namespaced (contain at least one dot)."
                    ));
                }
                if !value.is_object() {
                    issues.push(format!("Extension '{key}' value must be an object."));
                } else if let Some(obj) = value.as_object() {
                    if !obj.contains_key("schemaVersion") {
                        issues.push(format!(
                            "Extension '{key}' must include a required 'schemaVersion' string field."
                        ));
                    }
                }
            }
        }
    }

    ElegyPluginV1ValidationResult { issues }
}

/// Stub: validate that MCP-delegated tools in the plugin reference declared MCP server descriptors.
///
/// The full directory traversal logic lives in elegy-tooling. This stub exists so
/// conformance tests can be written against the contract shape. It returns no issues
/// for now.
// TODO(Task 2.x): Implement directory traversal to validate MCP tool references against
//                 declared server descriptors in the plugin's mcpServers directory.
pub fn validate_plugin_mcp_tool_references(
    _plugin: &ElegyPluginV1,
    _plugin_root: &Path,
) -> Vec<String> {
    Vec::new()
}

// ── Agent Skill Frontmatter (YAML) ──────────────────────────────────────

/// Parsed YAML frontmatter block from an Agent Skills `SKILL.md` file.
///
/// Frontmatter is delimited by `---` fences at the top of the file. The
/// remaining body content after the frontmatter is returned separately.
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

/// Parse a `SKILL.md` content string into its YAML frontmatter and remaining body.
///
/// Expects the content to start with `---\n`, followed by YAML fields, then
/// `\n---\n` or `\n---` to close the frontmatter.
pub fn parse_agent_skill_frontmatter(
    content: &str,
) -> Result<(AgentSkillFrontmatter, String), String> {
    let content = content.trim_start();
    if !content.starts_with("---") {
        return Err("Content must start with a '---' frontmatter fence.".into());
    }

    // Skip the opening ---
    let after_open = &content[3..];
    let after_newline = after_open
        .strip_prefix('\n')
        .or_else(|| after_open.strip_prefix("\r\n"))
        .ok_or_else(|| "Opening '---' must be followed by a newline.".to_string())?;

    // Find the closing ---
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

/// Validate an Agent Skill frontmatter for required fields and naming conventions.
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

/// Returns true if the path is a safe package-relative path.
///
/// Rejects:
/// - empty paths
/// - absolute paths (starts with `/` or `\`)
/// - Windows drive letters (`C:`)
/// - directory traversal (`..` as a path component)
fn is_safe_package_relative_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    // Reject absolute paths
    if path.starts_with('/') || path.starts_with('\\') {
        return false;
    }
    // Reject Windows drive letters
    if path.len() >= 2 && path.as_bytes()[1] == b':' {
        return false;
    }
    // Reject `..` as a path *component* (not just anywhere in the string)
    // Match: start-of-string `..` followed by `/` or end, or `/..` followed by `/` or end
    let bytes = path.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        // Look for `..` at current position
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

/// Returns true if `name` is valid lowercase kebab-case: starts with a letter,
/// contains only lowercase letters, digits, and hyphens.
pub fn validate_kebab_case_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let bytes = name.as_bytes();
    // Must start with a lowercase letter
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    // Remaining characters: lowercase letters, digits, hyphens
    bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// SemVer 2.0.0 validation via the `semver` crate.
pub fn validate_semver(version: &str) -> bool {
    semver::Version::parse(version).is_ok()
}

// ── (Readiness, lock, and install-receipt types removed in Phase 3 cleanup) ──

fn validate_uri(field: &str, value: &str, issues: &mut Vec<String>) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }

    match Url::parse(value) {
        Ok(url) if !url.scheme().is_empty() => {}
        _ => issues.push(format!("{field} must be a valid URI.")),
    }
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

/// Structured failure payload embedded in [`CliMachineEnvelope`] on error.
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

/// High-level classification of a [`StructuredFailure`].
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

/// Optional upstream cause chain for a [`StructuredFailure`].
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContractsBundleExport {
    pub output_path: PathBuf,
    pub archive_path: Option<PathBuf>,
    pub package_version: String,
    pub schema_version: String,
    pub files: Vec<PathBuf>,
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
pub struct SkillTrigger {
    pub pattern: String,
    #[serde(default)]
    pub description: Option<String>,
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
pub struct ObservationValidationResult {
    pub issues: Vec<String>,
}

impl ObservationValidationResult {
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

    let schema_version = "1.0.0".to_string();
    let package_version = "1.0.0".to_string();

    let mut relative_files = BTreeSet::new();

    // Collect schemas
    let schemas_dir = contracts_source_dir.join("schemas");
    if schemas_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&schemas_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(std::ffi::OsStr::to_str) == Some("json") {
                    if let Some(name) = path.file_name().and_then(std::ffi::OsStr::to_str) {
                        relative_files.insert(PathBuf::from(format!("schemas/{name}")));
                    }
                }
            }
        }
    }

    // Collect fixtures
    let fixtures_dir = contracts_source_dir.join("fixtures");
    if fixtures_dir.is_dir() {
        collect_fixture_files(&fixtures_dir, "fixtures", &mut relative_files)?;
    }

    // Collect configuration
    let config_dir = contracts_source_dir.join("configuration");
    if config_dir.is_dir() {
        collect_fixture_files(&config_dir, "configuration", &mut relative_files)?;
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

    let archive_path = if create_archive || archive_output_path.is_some() {
        let resolved_archive_path = archive_output_path
            .map(Path::to_path_buf)
            .unwrap_or_else(|| default_contracts_archive_path(&repo_root, &package_version));
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

fn collect_fixture_files(
    dir: &Path,
    prefix: &str,
    relative_files: &mut BTreeSet<PathBuf>,
) -> Result<(), ContractsError> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let sub_prefix = format!(
                    "{}/{}",
                    prefix,
                    path.file_name()
                        .and_then(std::ffi::OsStr::to_str)
                        .unwrap_or("")
                );
                collect_fixture_files(&path, &sub_prefix, relative_files)?;
            } else if path.extension().and_then(std::ffi::OsStr::to_str) == Some("json") {
                if let Some(name) = path.file_name().and_then(std::ffi::OsStr::to_str) {
                    relative_files.insert(PathBuf::from(format!("{prefix}/{name}")));
                }
            }
        }
    }
    Ok(())
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
    load_json_file(
        &dir.join("fixtures")
            .join("invocation-response.minimal.json"),
    )
}

pub fn load_execution_event_fixture_from_dir(dir: &Path) -> Result<ExecutionEvent, ContractsError> {
    load_json_file(&dir.join("fixtures").join("execution-event.minimal.json"))
}

pub fn load_observation_event_fixture_from_dir(
    dir: &Path,
) -> Result<ObservationEvent, ContractsError> {
    load_json_file(&dir.join("fixtures").join("observation-event.minimal.json"))
}

pub fn load_observation_session_fixture_from_dir(
    dir: &Path,
) -> Result<ObservationSession, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("observation-session.minimal.json"),
    )
}

pub fn load_observation_summary_fixture_from_dir(
    dir: &Path,
) -> Result<ObservationSummary, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("observation-summary.minimal.json"),
    )
}

pub fn load_agent_capability_profile_fixture_from_dir(
    dir: &Path,
) -> Result<AgentCapabilityProfile, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("agent-capability-profile.minimal.json"),
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
