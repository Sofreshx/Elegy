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

pub const ELEGY_PLUGIN_PACKAGE_V1_SCHEMA_VERSION: &str = "elegy-plugin-package/v1";
pub const ELEGY_PLUGIN_LOCK_V1_SCHEMA_VERSION: &str = "elegy-plugin-lock/v1";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginPackage {
    pub schema_version: String,
    pub identity: ElegyPluginPackageIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ElegyPluginPackageMetadata>,
    pub components: ElegyPluginPackageComponents,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_policy_hints: Option<ElegyPluginPackagePolicyHints>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publishing: Option<ElegyPluginPackagePublishingMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elegy_compatibility: Option<ElegyPluginPackageElegyCompatibility>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginPackageElegyCompatibility {
    pub contract_bundle_version: String,
    pub schema_line: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_elegy_tooling_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contracts_source: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginPackageIdentity {
    pub package_id: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginPackageMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subset_of: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginPackageComponents {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skill_definitions: Vec<ElegyPluginPackageSkillDefinitionComponent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instruction_skills: Vec<ElegyPluginPackagePathComponent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_projections: Vec<ElegyPluginPackageCapabilityProjectionComponent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<ElegyPluginPackagePathComponent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub configuration_templates: Vec<ElegyPluginPackageConfigurationComponent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub configuration_profiles: Vec<ElegyPluginPackageConfigurationComponent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_requirements: Vec<ElegyPluginPackageToolRequirement>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginPackageSkillDefinitionComponent {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition: Option<SkillDefinitionV2>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginPackagePathComponent {
    pub id: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginPackageConfigurationComponent {
    pub id: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginPackageCapabilityProjectionComponent {
    pub id: String,
    pub skill: String,
    pub capability: String,
    pub lane: String,
    pub supports_dry_run: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side_effect_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection: Option<ElegyPluginPackageProjectionMetadata>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginPackageProjectionMetadata {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projections: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_tool_name: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginPackageCapabilityRef {
    pub skill: String,
    pub capability: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginPackagePolicyHints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side_effect_class: Option<String>,
    #[serde(default)]
    pub requires_approval: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_tags: Vec<String>,
}

/// Publishing metadata for an Elegy plugin package (marketplace, provenance, signatures).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginPackagePublishingMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crate_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marketplace_target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changelog_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signature_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compatibility: Vec<ElegyPluginPackageCompatibilityMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_bridge: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_publish_hook: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_publish_hook: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginPackageCompatibilityMetadata {
    pub host: String,
    pub version_range: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginPackageToolRequirement {
    pub tool_name: String,
    pub cli_binary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ElegyPluginPackageValidationResult {
    pub issues: Vec<String>,
}

impl ElegyPluginPackageValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

pub fn validate_elegy_plugin_package(
    package: &ElegyPluginPackage,
) -> ElegyPluginPackageValidationResult {
    let mut issues = Vec::new();

    if package.schema_version != ELEGY_PLUGIN_PACKAGE_V1_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{}'.",
            ELEGY_PLUGIN_PACKAGE_V1_SCHEMA_VERSION
        ));
    }
    if !is_package_id(&package.identity.package_id) {
        issues.push(
            "identity.packageId must contain only ASCII letters, digits, '.', '_' or '-'."
                .to_string(),
        );
    }
    if package.identity.name.trim().is_empty() {
        issues.push("identity.name must not be empty.".to_string());
    } else if !is_codex_plugin_slug(&package.identity.name) {
        issues.push(
            "identity.name must be a Codex plugin slug containing only lowercase ASCII letters, digits, or '-'."
                .to_string(),
        );
    }
    if package.identity.version.trim().is_empty() {
        issues.push("identity.version must not be empty.".to_string());
    }
    if let Some(metadata) = &package.metadata {
        if let Some(homepage) = &metadata.homepage {
            validate_uri("metadata.homepage", homepage, &mut issues);
        }
        if let Some(documentation_uri) = &metadata.documentation_uri {
            validate_uri("metadata.documentationUri", documentation_uri, &mut issues);
        }
    }
    if let Some(compat) = &package.elegy_compatibility {
        if let Some(source) = &compat.contracts_source {
            validate_uri("elegyCompatibility.contractsSource", source, &mut issues);
        }
    }

    validate_component_ids(
        "components.skillDefinitions",
        package
            .components
            .skill_definitions
            .iter()
            .map(|component| component.id.as_str()),
        &mut issues,
    );
    validate_component_ids(
        "components.instructionSkills",
        package
            .components
            .instruction_skills
            .iter()
            .map(|component| component.id.as_str()),
        &mut issues,
    );
    validate_component_ids(
        "components.capabilityProjections",
        package
            .components
            .capability_projections
            .iter()
            .map(|component| component.id.as_str()),
        &mut issues,
    );
    validate_component_ids(
        "components.docs",
        package
            .components
            .docs
            .iter()
            .map(|component| component.id.as_str()),
        &mut issues,
    );
    validate_component_ids(
        "components.configurationTemplates",
        package
            .components
            .configuration_templates
            .iter()
            .map(|component| component.id.as_str()),
        &mut issues,
    );
    validate_component_ids(
        "components.configurationProfiles",
        package
            .components
            .configuration_profiles
            .iter()
            .map(|component| component.id.as_str()),
        &mut issues,
    );
    validate_component_ids(
        "components.toolRequirements",
        package
            .components
            .tool_requirements
            .iter()
            .map(|component| component.tool_name.as_str()),
        &mut issues,
    );

    let mut capability_refs = BTreeSet::new();
    let mut capability_side_effects: BTreeMap<(String, String), HostSideEffectClass> =
        BTreeMap::new();
    for component in &package.components.skill_definitions {
        if component.definition_ref.is_none() && component.definition.is_none() {
            issues.push(format!(
                "components.skillDefinitions entry '{}' must declare definitionRef or definition.",
                component.id
            ));
        }
        if let Some(definition_ref) = &component.definition_ref {
            validate_portable_relative_path(
                &format!(
                    "components.skillDefinitions['{}'].definitionRef",
                    component.id
                ),
                definition_ref,
                &mut issues,
            );
        }
        if let Some(definition) = &component.definition {
            if let Err(error) = validate_skill_definition_v2(definition) {
                issues.push(format!(
                    "components.skillDefinitions entry '{}' contains invalid skill definition: {error}",
                    component.id
                ));
            }
            let skill_ref = format!(
                "{}.{}",
                definition.identity.namespace, definition.identity.name
            );
            for capability in &definition.capabilities {
                capability_refs.insert((skill_ref.clone(), capability.id.clone()));
            }
            if let Some(host_projection) = &definition.host_projection {
                for host_cap in &host_projection.capability_projections {
                    let class = host_cap
                        .side_effect_class
                        .unwrap_or(host_projection.default_side_effect_class);
                    capability_side_effects
                        .insert((skill_ref.clone(), host_cap.capability_id.clone()), class);
                }
                for capability in &definition.capabilities {
                    if !host_projection
                        .capability_projections
                        .iter()
                        .any(|p| p.capability_id == capability.id)
                    {
                        capability_side_effects.insert(
                            (skill_ref.clone(), capability.id.clone()),
                            host_projection.default_side_effect_class,
                        );
                    }
                }
            }
        }
    }

    for component in package
        .components
        .instruction_skills
        .iter()
        .chain(package.components.docs.iter())
    {
        validate_portable_relative_path(
            &format!("component path '{}'", component.id),
            &component.path,
            &mut issues,
        );
    }

    for component in package
        .components
        .configuration_templates
        .iter()
        .chain(package.components.configuration_profiles.iter())
    {
        validate_portable_relative_path(
            &format!("component path '{}'", component.id),
            &component.path,
            &mut issues,
        );
    }

    for projection in &package.components.capability_projections {
        if !matches!(
            projection.lane.as_str(),
            "api" | "mcp" | "plugin" | "cli" | "subprocess" | "rust"
        ) {
            issues.push(format!(
                "components.capabilityProjections entry '{}' uses unsupported lane '{}'.",
                projection.id, projection.lane
            ));
        }
        let capability_ref = ElegyPluginPackageCapabilityRef {
            skill: projection.skill.clone(),
            capability: projection.capability.clone(),
        };
        validate_package_capability_ref(
            "components.capabilityProjections",
            &capability_ref,
            &capability_refs,
            &mut issues,
        );

        if let (Some(declared), Some(underlying)) = (
            projection
                .side_effect_class
                .as_deref()
                .and_then(HostSideEffectClass::from_label),
            capability_side_effects
                .get(&(projection.skill.clone(), projection.capability.clone()))
                .copied(),
        ) {
            let declared_rank = declared.invasiveness();
            let underlying_rank = underlying.invasiveness();
            if declared_rank > underlying_rank {
                issues.push(format!(
                    "components.capabilityProjections entry '{}' loosens side-effect class from '{}' to '{}'.",
                    projection.id,
                    underlying.as_str(),
                    declared.as_str()
                ));
            } else if declared_rank < underlying_rank
                && underlying_rank >= HostSideEffectClass::DiskWrite.invasiveness()
            {
                issues.push(format!(
                    "components.capabilityProjections entry '{}' tightens side-effect class from '{}' to '{}' but the underlying capability has side effects.",
                    projection.id,
                    underlying.as_str(),
                    declared.as_str()
                ));
            }
        }
    }

    if let Some(publishing) = &package.publishing {
        validate_plugin_package_publishing_metadata(package, publishing, &mut issues);
    }

    ElegyPluginPackageValidationResult { issues }
}

pub const ELEGY_PLUGIN_READINESS_V1_SCHEMA_VERSION: &str = "elegy-plugin-readiness/v1";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginReadinessV1 {
    pub schema_version: String,
    pub package_identity: ElegyPluginReadinessPackageIdentity,
    pub readiness: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verified_skills: Vec<ElegyPluginReadinessVerifiedSkill>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projected_tools: Vec<ElegyPluginReadinessProjectedTool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_statuses: Vec<ElegyPluginReadinessToolStatus>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub omitted_capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unsupported_capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side_effect_summary: Option<ElegyPluginReadinessSideEffectSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<ElegyPluginReadinessFinding>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginReadinessPackageIdentity {
    pub package_id: String,
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginReadinessVerifiedSkill {
    pub skill_id: String,
    pub status: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginReadinessProjectedTool {
    pub tool_name: String,
    pub function_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginReadinessToolStatus {
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli_binary: Option<String>,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginReadinessSideEffectSummary {
    #[serde(default)]
    pub none: i32,
    #[serde(default, rename = "read_only")]
    pub read_only: i32,
    #[serde(default, rename = "disk_read")]
    pub disk_read: i32,
    #[serde(default, rename = "disk_write")]
    pub disk_write: i32,
    #[serde(default, rename = "network_outbound")]
    pub network_outbound: i32,
    #[serde(default, rename = "process_spawn")]
    pub process_spawn: i32,
    #[serde(default, rename = "desktop_ui")]
    pub desktop_ui: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginReadinessFinding {
    pub code: String,
    pub severity: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Install receipt parsed from a host-generated install-receipt.json.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginInstallReceiptV1 {
    pub package_id: String,
    pub install_path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub installed_binaries: Vec<ElegyPluginInstallReceiptBinary>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginInstallReceiptBinary {
    pub tool_name: String,
    pub binary_path: String,
}

/// Lock file that pins Elegy contract bundle version for a plugin package.
/// Schema: elegy-plugin-lock/v1
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginLockV1 {
    pub schema_version: String,
    pub lock_version: u32,
    pub elegy_compatibility: ElegyPluginLockCompatibility,
    pub generated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_package_ref: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginLockCompatibility {
    pub contract_bundle_version: String,
    pub schema_line: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_asset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

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
pub struct ObservationValidationResult {
    pub issues: Vec<String>,
}

impl ObservationValidationResult {
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

// ─── Skill Definition V2 ────────────────────────────────────────────

/// Unified skill definition (v2) combining governance, lifecycle,
/// discovery metadata, and per-capability implementation details
/// for agent-consumable invocation.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillDefinitionV2 {
    /// Must be `"elegy-skill-definition"`.
    pub skill_format: String,
    /// Must be `2`.
    pub skill_version: u32,
    /// Skill identity: namespace, name, version.
    pub identity: SkillIdentityV2,
    /// Optional display metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SkillMetadataV2>,
    /// One or more capabilities this skill exposes.
    pub capabilities: Vec<SkillCapability>,
    /// Constraints that apply to the skill as a whole.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<SkillConstraint>,
    /// Governance posture: risk level, approval requirement, allowed contexts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub governance: Option<SkillGovernance>,
    /// Discovery metadata: keywords, triggers, hints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery: Option<SkillDiscovery>,
    /// Provenance: how this definition was created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<SkillOriginV2>,
    /// Lifecycle state of the skill.
    pub lifecycle_state: String,
    /// Optional explicit host projection metadata describing how runtime hosts
    /// (such as Holon) should register this skill's capabilities as callable
    /// tools, including the CLI binary, output contract family, default
    /// side-effect class, and per-capability function-calling projections.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_projection: Option<SkillHostProjection>,
}

/// Side-effect class advertised by [`SkillHostProjection`] for runtime host
/// tool registration. Distinct from [`CapabilitySideEffectClass`] so that
/// host-facing vocabulary stays aligned with the governed JSON schema and
/// does not change when capability-side classification changes.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostSideEffectClass {
    #[default]
    None,
    ReadOnly,
    DiskRead,
    DiskWrite,
    NetworkOutbound,
    ProcessSpawn,
    DesktopUi,
}

impl HostSideEffectClass {
    pub fn as_str(self) -> &'static str {
        match self {
            HostSideEffectClass::None => "none",
            HostSideEffectClass::ReadOnly => "read_only",
            HostSideEffectClass::DiskRead => "disk_read",
            HostSideEffectClass::DiskWrite => "disk_write",
            HostSideEffectClass::NetworkOutbound => "network_outbound",
            HostSideEffectClass::ProcessSpawn => "process_spawn",
            HostSideEffectClass::DesktopUi => "desktop_ui",
        }
    }

    /// Numeric invasiveness rank for tightening/loosening comparisons.
    /// Higher value = more invasive. The order matches the contract authority
    /// in `docs/specs/plugin-tool-availability.md` R2.5.
    pub fn invasiveness(self) -> u8 {
        match self {
            HostSideEffectClass::None => 0,
            HostSideEffectClass::ReadOnly => 1,
            HostSideEffectClass::DiskRead => 2,
            HostSideEffectClass::DiskWrite => 3,
            HostSideEffectClass::NetworkOutbound => 4,
            HostSideEffectClass::ProcessSpawn => 5,
            HostSideEffectClass::DesktopUi => 6,
        }
    }

    pub fn from_label(value: &str) -> Option<Self> {
        match value {
            "none" => Some(HostSideEffectClass::None),
            "read_only" => Some(HostSideEffectClass::ReadOnly),
            "disk_read" => Some(HostSideEffectClass::DiskRead),
            "disk_write" => Some(HostSideEffectClass::DiskWrite),
            "network_outbound" => Some(HostSideEffectClass::NetworkOutbound),
            "process_spawn" => Some(HostSideEffectClass::ProcessSpawn),
            "desktop_ui" => Some(HostSideEffectClass::DesktopUi),
            _ => None,
        }
    }
}

/// Host-facing function-calling projection for a single capability within a
/// [`SkillHostProjection`].
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillHostCapabilityProjection {
    /// Governed capability id this projection derives from. Must match an
    /// existing capability in the parent skill definition.
    pub capability_id: String,
    /// Stable function-calling name for runtime host tool registration.
    pub function_name: String,
    /// Optional capability-level side-effect class override. When absent the
    /// projection falls back to the parent host projection's default class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side_effect_class: Option<HostSideEffectClass>,
    /// Whether this capability always produces the same output for the same
    /// input. Used by hosts to enable caching or simplify reasoning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_deterministic: Option<bool>,
}

/// Explicit host projection metadata describing how a skill definition's
/// capabilities map to runtime host tool surfaces (CLI subprocess, function
/// calling, etc.).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillHostProjection {
    /// Stable CLI binary name for subprocess invocation (e.g.
    /// `"elegy-planning"`, `"elegy-skills"`).
    pub cli_name: String,
    /// Versioned output contract family identifier used by hosts for
    /// envelope validation (e.g. `"elegy-planning-v1"`,
    /// `"elegy-skills-v1"`).
    pub output_contract_id: String,
    /// Skill-level default side-effect class. Individual capabilities may
    /// override this with a more specific class.
    pub default_side_effect_class: HostSideEffectClass,
    /// Per-capability function-calling projections, including stable
    /// function names and optional side-effect class overrides.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_projections: Vec<SkillHostCapabilityProjection>,
}

/// Identity block for a skill definition.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillIdentityV2 {
    /// Organizational namespace (e.g. `"elegy"`).
    pub namespace: String,
    /// Skill name (e.g. `"diagram"`).
    pub name: String,
    /// Semver version string.
    pub version: String,
    /// Human-friendly display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Alternative names agents can use to refer to this skill.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

/// Display and categorization metadata for a Skill.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillMetadataV2 {
    /// Human-friendly display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Longer description of what the skill does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// One-sentence summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Category tag (e.g. `"design"`, `"memory"`, `"projection"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Author or team.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// SPDX license identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Free-form tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Owning teams.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owners: Vec<String>,
    /// Link to documentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation_uri: Option<String>,
}

/// A single capability within a skill definition.
///
/// Each capability maps to a specific CLI invocation or MCP tool
/// that an agent can call.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillCapability {
    /// Unique capability identifier (e.g. `"diagram-patch"`).
    pub id: String,
    /// Human-readable capability name.
    pub name: String,
    /// Description of what this capability does.
    pub description: String,
    /// How to invoke this capability (subprocess, library, mcp).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation: Option<SkillImplementation>,
    /// Input parameters and stdin format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<SkillCapabilityInput>,
    /// Output description and schema reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<SkillCapabilityOutput>,
    /// Execution characteristics (determinism, side effects, timeout).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<SkillCapabilityExecution>,
    /// Optional composition hints for agent chaining.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composes_well: Option<SkillComposition>,
}

/// How a capability is invoked.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillImplementation {
    /// Invocation mechanism: `"subprocess"`, `"library"`, or `"mcp"`.
    pub execution_type: String,
    /// Binary or library name to invoke.
    pub executable_name: String,
    /// CLI arguments, possibly containing `${var}` placeholders.
    #[serde(default)]
    pub arguments: Vec<String>,
}

/// Input specification for a capability.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillCapabilityInput {
    /// Typed parameter definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<SkillParameterV2>,
    /// Expected stdin format: `"json"`, `"text"`, or absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdin_format: Option<String>,
    /// Reference to a JSON Schema for the input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_ref: Option<String>,
}

/// A single typed parameter within a v2 capability's input block.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillParameterV2 {
    /// Parameter name.
    pub name: String,
    /// Type identifier (e.g. `"string"`, `"path"`, `"boolean"`).
    #[serde(rename = "type")]
    pub param_type: String,
    /// What this parameter controls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether this parameter is mandatory.
    #[serde(default)]
    pub required: bool,
    /// Default value when the parameter is omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

/// Output specification for a capability.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillCapabilityOutput {
    /// Type of the result (e.g. `"CanonicalDiagram"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_type: Option<String>,
    /// Reference to a JSON Schema for the output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_ref: Option<String>,
    /// Whether the result is a collection.
    #[serde(default)]
    pub returns_collection: bool,
    /// Human-readable description of what is returned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Execution characteristics for a capability.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillCapabilityExecution {
    /// Execution mode: `"requestResponse"`, `"longRunning"`, `"streaming"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Whether the capability always produces the same output for the same input.
    #[serde(default)]
    pub is_deterministic: bool,
    /// Whether the capability has side effects (writes files, mutates state).
    #[serde(default)]
    pub has_side_effects: bool,
    /// Optional timeout in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u32>,
}

/// Composition hints so agents can chain capabilities.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillComposition {
    /// Capabilities that typically follow this one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub typical_next: Vec<String>,
    /// Capabilities this one can pipe output to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pipeable_to: Vec<String>,
    /// Capabilities that consume this one's output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_consumed_by: Vec<String>,
}

/// Governance posture for a Skill.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillGovernance {
    /// Risk level: `"low"`, `"medium"`, `"high"`, `"critical"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<String>,
    /// Approval requirement: `"none"`, `"advisory"`, `"required"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_requirement: Option<String>,
    /// References to policy documents.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_refs: Vec<String>,
    /// Contexts in which this skill may be used.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_contexts: Vec<String>,
}

/// Discovery metadata for agents to find a Skill.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillDiscovery {
    /// Searchable keywords.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    /// Trigger patterns and their descriptions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<SkillTrigger>,
    /// Capability hint strings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_hints: Vec<String>,
    /// Whether this skill should be excluded from default listings.
    #[serde(default)]
    pub is_hidden: bool,
}

/// Origin/provenance of a skill definition.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillOriginV2 {
    /// How the definition was created: `"declared"` or `"dynamic"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialization_kind: Option<String>,
    /// Source type: `"manual"`, `"imported"`, `"generated"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    /// Path or URI to the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    /// Version of the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_version: Option<String>,
}

/// Validate a skill definition for structural correctness.
///
/// Checks required fields and basic invariants without performing
/// schema validation against the JSON Schema.
pub fn validate_skill_definition_v2(def: &SkillDefinitionV2) -> Result<(), ContractsError> {
    if def.skill_format != "elegy-skill-definition" {
        return Err(ContractsError::Compatibility(format!(
            "expected skillFormat \"elegy-skill-definition\", got \"{}\"",
            def.skill_format,
        )));
    }
    if def.skill_version != 2 {
        return Err(ContractsError::Compatibility(format!(
            "expected skillVersion 2, got {}",
            def.skill_version,
        )));
    }
    if def.identity.namespace.is_empty() {
        return Err(ContractsError::Compatibility(
            "identity.namespace must not be empty".to_string(),
        ));
    }
    if def.identity.name.is_empty() {
        return Err(ContractsError::Compatibility(
            "identity.name must not be empty".to_string(),
        ));
    }
    if def.capabilities.is_empty() {
        return Err(ContractsError::Compatibility(
            "capabilities must contain at least one entry".to_string(),
        ));
    }
    if has_duplicate_values(def.identity.aliases.iter().map(String::as_str)) {
        return Err(ContractsError::Compatibility(
            "identity aliases must be unique".to_string(),
        ));
    }
    if let Some(metadata) = &def.metadata {
        if metadata.tags.iter().any(|tag| tag.trim().is_empty()) {
            return Err(ContractsError::Compatibility(
                "metadata tags must not be blank".to_string(),
            ));
        }
        if metadata.owners.iter().any(|owner| owner.trim().is_empty()) {
            return Err(ContractsError::Compatibility(
                "metadata owners must not be blank".to_string(),
            ));
        }
    }
    if def
        .constraints
        .iter()
        .any(|constraint| constraint.constraint_id.trim().is_empty())
    {
        return Err(ContractsError::Compatibility(
            "constraints must define non-empty constraint IDs".to_string(),
        ));
    }
    if let Some(governance) = &def.governance {
        if governance.approval_requirement.as_deref() == Some("required")
            && governance.policy_refs.is_empty()
        {
            return Err(ContractsError::Compatibility(
                "skills that require approval must declare at least one policy reference"
                    .to_string(),
            ));
        }
    }
    if let Some(discovery) = &def.discovery {
        if discovery
            .triggers
            .iter()
            .any(|trigger| trigger.pattern.trim().is_empty())
        {
            return Err(ContractsError::Compatibility(
                "discovery triggers must define non-empty patterns".to_string(),
            ));
        }
    }
    if let Some(origin) = &def.origin {
        let is_dynamic = origin.materialization_kind.as_deref() == Some("dynamic");
        let is_manual = origin
            .source_kind
            .as_deref()
            .is_none_or(|kind| kind == "manual");
        if is_dynamic && is_manual && origin.source_ref.is_none() {
            return Err(ContractsError::Compatibility(
                "dynamic skills must declare either a source reference or a non-manual source kind"
                    .to_string(),
            ));
        }
    }

    let mut capability_ids = BTreeSet::new();
    for cap in &def.capabilities {
        if cap.id.is_empty() {
            return Err(ContractsError::Compatibility(
                "capability id must not be empty".to_string(),
            ));
        }
        if !capability_ids.insert(cap.id.to_ascii_lowercase()) {
            return Err(ContractsError::Compatibility(format!(
                "capability id '{}' is duplicated",
                cap.id
            )));
        }
        if cap.name.trim().is_empty() {
            return Err(ContractsError::Compatibility(format!(
                "capability '{}' must define a name",
                cap.id
            )));
        }
        if cap.description.trim().is_empty() {
            return Err(ContractsError::Compatibility(format!(
                "capability '{}' must define a description",
                cap.id
            )));
        }
        let Some(implementation) = &cap.implementation else {
            return Err(ContractsError::Compatibility(format!(
                "capability '{}' must define an implementation",
                cap.id
            )));
        };
        if implementation.execution_type.trim().is_empty() {
            return Err(ContractsError::Compatibility(format!(
                "capability '{}' implementation.executionType must not be empty",
                cap.id
            )));
        }
        if implementation.executable_name.trim().is_empty() {
            return Err(ContractsError::Compatibility(format!(
                "capability '{}' implementation.executableName must not be empty",
                cap.id
            )));
        }
        if let Some(input) = &cap.input {
            let mut parameter_names = BTreeSet::new();
            for parameter in &input.parameters {
                if parameter.name.trim().is_empty() {
                    return Err(ContractsError::Compatibility(format!(
                        "capability '{}' input parameters must define non-empty names",
                        cap.id
                    )));
                }
                if !parameter_names.insert(parameter.name.to_ascii_lowercase()) {
                    return Err(ContractsError::Compatibility(format!(
                        "capability '{}' input parameter '{}' is duplicated",
                        cap.id, parameter.name
                    )));
                }
            }
        }
    }
    if let Some(host_projection) = &def.host_projection {
        validate_skill_host_projection(def, host_projection)?;
    }
    Ok(())
}

fn validate_skill_host_projection(
    def: &SkillDefinitionV2,
    host_projection: &SkillHostProjection,
) -> Result<(), ContractsError> {
    if host_projection.cli_name.trim().is_empty() {
        return Err(ContractsError::Compatibility(
            "hostProjection.cliName must not be empty".to_string(),
        ));
    }
    if host_projection.output_contract_id.trim().is_empty() {
        return Err(ContractsError::Compatibility(
            "hostProjection.outputContractId must not be empty".to_string(),
        ));
    }

    let capability_ids = def
        .capabilities
        .iter()
        .map(|cap| cap.id.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();

    let mut seen_capability_ids = BTreeSet::new();
    let mut seen_function_names = BTreeSet::new();
    for projection in &host_projection.capability_projections {
        if projection.capability_id.trim().is_empty() {
            return Err(ContractsError::Compatibility(
                "hostProjection.capabilityProjections[].capabilityId must not be empty".to_string(),
            ));
        }
        let normalized_capability = projection.capability_id.to_ascii_lowercase();
        if !capability_ids.contains(&normalized_capability) {
            return Err(ContractsError::Compatibility(format!(
                "hostProjection.capabilityProjections[].capabilityId '{}' does not match any capability declared on skill '{}'",
                projection.capability_id, def.identity.name
            )));
        }
        if !seen_capability_ids.insert(normalized_capability) {
            return Err(ContractsError::Compatibility(format!(
                "hostProjection.capabilityProjections[].capabilityId '{}' is duplicated",
                projection.capability_id
            )));
        }

        if projection.function_name.trim().is_empty() {
            return Err(ContractsError::Compatibility(format!(
                "hostProjection.capabilityProjections[].functionName for capability '{}' must not be empty",
                projection.capability_id
            )));
        }
        if !seen_function_names.insert(projection.function_name.to_ascii_lowercase()) {
            return Err(ContractsError::Compatibility(format!(
                "hostProjection.capabilityProjections[].functionName '{}' is duplicated",
                projection.function_name
            )));
        }
    }
    Ok(())
}

/// Strict validation for skill definitions that additionally enforces
/// output.schemaRef on every subprocess machine-invokable capability.
///
/// Policy: every subprocess machine-invokable capability MUST declare
/// output.schemaRef. Capabilities without it are rejected.
///
/// See `contracts/schemas/skill.schema.json` comment and
/// `contracts/fixtures/skill.negative-no-output-schema.json`.
pub fn validate_skill_definition_v2_strict(def: &SkillDefinitionV2) -> Result<(), ContractsError> {
    validate_skill_definition_v2(def)?;

    for cap in &def.capabilities {
        if let Some(implementation) = &cap.implementation {
            if implementation.execution_type == "subprocess" {
                let has_schema_ref = cap
                    .output
                    .as_ref()
                    .and_then(|o| o.schema_ref.as_deref())
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false);
                if !has_schema_ref {
                    return Err(ContractsError::Compatibility(format!(
                        "capability '{}' is a subprocess machine-invokable capability and must declare output.schemaRef",
                        cap.id
                    )));
                }
            }
        }
    }

    Ok(())
}

/// One built-in skill definition loaded at runtime from fixture files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuiltinSkillDefinition {
    /// Runtime skill identifier, matching `identity.name`.
    pub id: String,
    /// UTF-8 JSON text for the skill definition.
    pub json: String,
}

/// Return the built-in Skill registry (loaded at runtime from fixture files).
pub fn builtin_skill_definitions() -> Vec<BuiltinSkillDefinition> {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures");
    let mut definitions = Vec::new();
    if let Ok(entries) = fs::read_dir(&fixtures_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if name.starts_with("skill.elegy-") && name.ends_with(".json") {
                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
                    let skill_name = stem.strip_prefix("skill.elegy-").unwrap_or(stem);
                    if let Ok(json) = fs::read_to_string(&path) {
                        definitions.push(BuiltinSkillDefinition {
                            id: skill_name.to_string(),
                            json,
                        });
                    }
                }
            }
        }
    }
    definitions
}

/// Parse and validate every built-in skill definition from fixture files on disk.
pub fn parse_builtin_skill_definitions() -> Result<Vec<SkillDefinitionV2>, ContractsError> {
    let definitions = builtin_skill_definitions();
    definitions
        .iter()
        .map(|entry| {
            let definition =
                serde_json::from_str::<SkillDefinitionV2>(&entry.json).map_err(|source| {
                    ContractsError::Compatibility(format!(
                        "built-in skill definition '{}' is invalid JSON: {source}",
                        entry.id
                    ))
                })?;
            validate_skill_definition_v2_strict(&definition)?;
            Ok(definition)
        })
        .collect()
}

/// Find a built-in skill definition by `identity.name`.
pub fn find_builtin_skill_definition(
    skill_id: &str,
) -> Result<Option<SkillDefinitionV2>, ContractsError> {
    for definition in parse_builtin_skill_definitions()? {
        if definition.identity.name == skill_id {
            return Ok(Some(definition));
        }
    }
    Ok(None)
}

/// Find a built-in capability and its parent skill definition.
pub fn find_builtin_skill_capability(
    capability_id: &str,
) -> Result<Option<(SkillCapability, SkillDefinitionV2)>, ContractsError> {
    for definition in parse_builtin_skill_definitions()? {
        for capability in &definition.capabilities {
            if capability.id == capability_id {
                return Ok(Some((capability.clone(), definition)));
            }
        }
    }
    Ok(None)
}

/// Returns [`CapabilityDefinition`] projections for all built-in skill capabilities.
pub fn builtin_capability_definitions() -> Result<Vec<CapabilityDefinition>, ContractsError> {
    let mut definitions = Vec::new();
    for skill_definition in parse_builtin_skill_definitions()? {
        for capability in &skill_definition.capabilities {
            definitions.push(project_skill_capability_definition(
                &skill_definition,
                capability,
            ));
        }
    }
    Ok(definitions)
}

/// Looks up a single built-in capability by ID and returns its [`CapabilityDefinition`] projection.
pub fn projected_builtin_capability_definition(
    capability_id: &str,
) -> Result<Option<CapabilityDefinition>, ContractsError> {
    Ok(
        find_builtin_skill_capability(capability_id)?.map(|(capability, skill_definition)| {
            project_skill_capability_definition(&skill_definition, &capability)
        }),
    )
}

/// Projects a skill capability into a [`CapabilityDefinition`] for agent-facing consumption.
pub fn project_skill_capability_definition(
    skill_definition: &SkillDefinitionV2,
    capability: &SkillCapability,
) -> CapabilityDefinition {
    CapabilityDefinition {
        id: capability.id.clone(),
        display_name: capability.name.clone(),
        version: skill_definition.identity.version.clone(),
        description: Some(capability.description.clone()),
        family: CapabilityFamily::Skill,
        tags: skill_definition
            .metadata
            .as_ref()
            .map(|metadata| metadata.tags.clone())
            .unwrap_or_default(),
        input: CapabilitySchemaReference {
            schema: None,
            schema_ref: capability
                .input
                .as_ref()
                .and_then(|input| input.schema_ref.clone()),
            description: capability
                .input
                .as_ref()
                .map(|_| capability.description.clone()),
        },
        output: CapabilitySchemaReference {
            schema: None,
            schema_ref: capability
                .output
                .as_ref()
                .and_then(|output| output.schema_ref.clone()),
            description: capability
                .output
                .as_ref()
                .and_then(|output| output.description.clone()),
        },
        execution: CapabilityExecutionContract {
            side_effect_class: if skill_capability_has_side_effects(capability) {
                CapabilitySideEffectClass::Write
            } else if skill_capability_is_deterministic(capability) {
                CapabilitySideEffectClass::None
            } else {
                CapabilitySideEffectClass::Read
            },
            auth_mode: CapabilityAuthMode::None,
            idempotence: if skill_capability_is_deterministic(capability) {
                CapabilityIdempotenceHint::Always
            } else {
                CapabilityIdempotenceHint::Conditional
            },
            cost_hint: CapabilityCostHint::Low,
            latency_hint: CapabilityLatencyHint::Interactive,
            timeout_seconds: capability
                .execution
                .as_ref()
                .and_then(|execution| execution.timeout_seconds)
                .map(|timeout| timeout as i32),
        },
        governance: CapabilityGovernance {
            trust_level: CapabilityTrustLevel::Trusted,
            approval_requirement: match skill_definition
                .governance
                .as_ref()
                .and_then(|governance| governance.approval_requirement.as_deref())
            {
                Some("required") => CapabilityApprovalRequirement::Required,
                Some("advisory") => CapabilityApprovalRequirement::Advisory,
                _ => CapabilityApprovalRequirement::None,
            },
            policy_refs: skill_definition
                .governance
                .as_ref()
                .map(|governance| governance.policy_refs.clone())
                .unwrap_or_default(),
        },
        source: CapabilitySource {
            source_kind: CapabilitySourceKind::Projected,
            source_ref: skill_definition
                .origin
                .as_ref()
                .and_then(|origin| origin.source_ref.clone()),
            artifact_ref: Some(format!(
                "skill:{}#{}",
                skill_definition.identity.name, capability.id
            )),
        },
        observability: CapabilityObservability {
            labels: vec![
                skill_definition.identity.name.clone(),
                capability.id.clone(),
            ],
            correlation_required: true,
            emits_execution_events: false,
        },
        lifecycle_state: match skill_definition.lifecycle_state.as_str() {
            "active" => CapabilityLifecycleState::Active,
            "deprecated" => CapabilityLifecycleState::Deprecated,
            "archived" => CapabilityLifecycleState::Archived,
            _ => CapabilityLifecycleState::Draft,
        },
    }
}

fn skill_capability_has_side_effects(capability: &SkillCapability) -> bool {
    capability
        .execution
        .as_ref()
        .map(|execution| execution.has_side_effects)
        .unwrap_or(false)
}

fn skill_capability_is_deterministic(capability: &SkillCapability) -> bool {
    capability
        .execution
        .as_ref()
        .map(|execution| execution.is_deterministic)
        .unwrap_or(false)
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

pub fn load_capability_definition_fixture_from_dir(
    dir: &Path,
) -> Result<CapabilityDefinition, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("capability-definition.minimal.json"),
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

pub fn load_skill_definition_v2_fixture_from_dir(
    dir: &Path,
) -> Result<SkillDefinitionV2, ContractsError> {
    load_json_file(&dir.join("fixtures").join("skill.minimal.json"))
}

pub fn load_elegy_plugin_package_fixture_from_dir(
    dir: &Path,
) -> Result<ElegyPluginPackage, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("elegy-plugin-package.minimal.json"),
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

fn is_package_id(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
}

fn validate_component_ids<'a>(
    field: &str,
    ids: impl Iterator<Item = &'a str>,
    issues: &mut Vec<String>,
) {
    let mut seen = BTreeSet::new();
    for id in ids {
        if id.trim().is_empty() {
            issues.push(format!("{field} ids must not be empty."));
            continue;
        }

        let normalized = id.trim().to_ascii_lowercase();
        if !seen.insert(normalized) {
            issues.push(format!("{field} must not contain duplicate id '{id}'."));
        }
    }
}

fn validate_portable_relative_path(field: &str, value: &str, issues: &mut Vec<String>) {
    let value = value.trim();
    if value.is_empty()
        || value.starts_with('/')
        || value.starts_with('\\')
        || value.contains(':')
        || value
            .split(['/', '\\'])
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        issues.push(format!(
            "{field} must be a portable relative package path without traversal."
        ));
    }
}

fn is_codex_plugin_slug(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

fn validate_package_capability_ref(
    field: &str,
    capability_ref: &ElegyPluginPackageCapabilityRef,
    capability_refs: &BTreeSet<(String, String)>,
    issues: &mut Vec<String>,
) {
    if capability_ref.skill.trim().is_empty() || capability_ref.capability.trim().is_empty() {
        issues.push(format!("{field} capability references must not be blank."));
        return;
    }
    if !capability_refs.is_empty()
        && !capability_refs.contains(&(
            capability_ref.skill.clone(),
            capability_ref.capability.clone(),
        ))
    {
        issues.push(format!(
            "{field} references unknown capability '{}.{}'",
            capability_ref.skill, capability_ref.capability
        ));
    }
}

fn validate_plugin_package_publishing_metadata(
    _package: &ElegyPluginPackage,
    publishing: &ElegyPluginPackagePublishingMetadata,
    issues: &mut Vec<String>,
) {
    if let Some(source_repository) = &publishing.source_repository {
        validate_uri("publishing.sourceRepository", source_repository, issues);
    }

    if let Some(marketplace_target) = &publishing.marketplace_target {
        if marketplace_target.trim().is_empty() {
            issues
                .push("publishing.marketplaceTarget must not be empty when provided.".to_string());
        }
    }

    if let Some(import_mode) = &publishing.import_mode {
        if !matches!(import_mode.as_str(), "package" | "dry_run") {
            issues.push(
                "publishing.importMode must be 'package' or 'dry_run' when provided.".to_string(),
            );
        }
    }

    let mut seen_signatures = BTreeSet::new();
    for signature_ref in &publishing.signature_refs {
        validate_portable_relative_path("publishing.signatureRefs", signature_ref, issues);
        let normalized = signature_ref.trim().to_ascii_lowercase();
        if !normalized.is_empty() && !seen_signatures.insert(normalized) {
            issues.push(format!(
                "publishing.signatureRefs must not contain duplicate entry '{}'.",
                signature_ref
            ));
        }
    }

    let mut compatibility_hosts = BTreeSet::new();
    for compatibility in &publishing.compatibility {
        if compatibility.host.trim().is_empty() {
            issues.push("publishing.compatibility host must not be empty.".to_string());
        }
        if compatibility.version_range.trim().is_empty() {
            issues.push("publishing.compatibility versionRange must not be empty.".to_string());
        }
        let normalized = compatibility.host.trim().to_ascii_lowercase();
        if !normalized.is_empty() && !compatibility_hosts.insert(normalized) {
            issues.push(format!(
                "publishing.compatibility must not contain duplicate host '{}'.",
                compatibility.host
            ));
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_elegy_plugin_package_rejects_unsafe_plugin_output_names() {
        for invalid_name in [
            "../target",
            "nested/name",
            r"nested\\name",
            "foo:bar",
            "DemoPlugin",
        ] {
            let package = ElegyPluginPackage {
                schema_version: ELEGY_PLUGIN_PACKAGE_V1_SCHEMA_VERSION.to_string(),
                identity: ElegyPluginPackageIdentity {
                    package_id: "elegy.demo-plugin".to_string(),
                    name: invalid_name.to_string(),
                    version: "0.1.0".to_string(),
                    display_name: None,
                },
                metadata: None,
                components: ElegyPluginPackageComponents {
                    skill_definitions: vec![ElegyPluginPackageSkillDefinitionComponent {
                        id: "demo-skill".to_string(),
                        definition_ref: None,
                        definition: Some(SkillDefinitionV2 {
                            skill_format: "elegy-skill-definition".to_string(),
                            skill_version: 2,
                            identity: SkillIdentityV2 {
                                namespace: "elegy".to_string(),
                                name: "demo-plugin".to_string(),
                                version: "0.1.0".to_string(),
                                ..Default::default()
                            },
                            capabilities: vec![SkillCapability {
                                id: "demo-cap".to_string(),
                                name: "Demo Cap".to_string(),
                                description: "Demo capability".to_string(),
                                implementation: Some(SkillImplementation {
                                    execution_type: "subprocess".to_string(),
                                    executable_name: "demo".to_string(),
                                    arguments: Vec::new(),
                                }),
                                ..Default::default()
                            }],
                            lifecycle_state: "active".to_string(),
                            ..Default::default()
                        }),
                    }],
                    ..Default::default()
                },
                host_policy_hints: None,
                publishing: None,
                elegy_compatibility: None,
                extensions: None,
            };

            let validation = validate_elegy_plugin_package(&package);
            assert!(
                validation
                    .issues
                    .iter()
                    .any(|issue| issue.contains("identity.name must be a Codex plugin slug")),
                "expected invalid plugin slug issue for {invalid_name:?}, got {:?}",
                validation.issues
            );
        }
    }

    #[test]
    fn parse_minimal_skill_fixture() {
        let json = include_str!("../../../../contracts/fixtures/skill.minimal.json");
        let def: SkillDefinitionV2 =
            serde_json::from_str(json).expect("minimal skill fixture should parse");
        assert_eq!(def.skill_format, "elegy-skill-definition");
        assert_eq!(def.skill_version, 2);
        assert_eq!(def.identity.name, "minimal-example");
        assert_eq!(def.capabilities.len(), 1);
        validate_skill_definition_v2(&def).expect("minimal fixture should validate");
    }

    #[test]
    fn parse_v2_diagram_fixture() {
        let json = include_str!("../../../../contracts/fixtures/skill.elegy-diagram.json");
        let def: SkillDefinitionV2 =
            serde_json::from_str(json).expect("diagram skill fixture should parse");
        assert_eq!(def.identity.namespace, "elegy");
        assert_eq!(def.identity.name, "diagram");
        assert_eq!(def.capabilities.len(), 4);
        assert_eq!(def.lifecycle_state, "active");
        validate_skill_definition_v2(&def).expect("diagram fixture should validate");
    }

    #[test]
    fn builtin_registry_contains_only_valid_v2_definitions() {
        let definitions =
            parse_builtin_skill_definitions().expect("built-in skill registry should parse");
        assert_eq!(definitions.len(), 17);
        assert!(definitions
            .iter()
            .any(|definition| definition.identity.name == "documentation"));
        assert!(definitions
            .iter()
            .any(|definition| definition.identity.name == "memory"));
        assert!(definitions
            .iter()
            .any(|definition| definition.identity.name == "mermaid"));
        assert!(definitions
            .iter()
            .any(|definition| definition.identity.name == "planning"));
        assert!(definitions.iter().all(|definition| definition
            .capabilities
            .iter()
            .all(|capability| capability.implementation.is_some())));
    }

    #[test]
    fn validate_rejects_empty_namespace() {
        let def = SkillDefinitionV2 {
            skill_format: "elegy-skill-definition".to_string(),
            skill_version: 2,
            identity: SkillIdentityV2 {
                namespace: String::new(),
                name: "test".to_string(),
                ..Default::default()
            },
            capabilities: vec![SkillCapability {
                id: "cap".to_string(),
                name: "Cap".to_string(),
                description: "d".to_string(),
                ..Default::default()
            }],
            lifecycle_state: "draft".to_string(),
            ..Default::default()
        };
        assert!(validate_skill_definition_v2(&def).is_err());
    }

    #[test]
    fn validate_rejects_wrong_format() {
        let def = SkillDefinitionV2 {
            skill_format: "wrong".to_string(),
            skill_version: 2,
            identity: SkillIdentityV2 {
                namespace: "x".to_string(),
                name: "y".to_string(),
                ..Default::default()
            },
            capabilities: vec![SkillCapability {
                id: "c".to_string(),
                name: "C".to_string(),
                description: "d".to_string(),
                ..Default::default()
            }],
            lifecycle_state: "draft".to_string(),
            ..Default::default()
        };
        assert!(validate_skill_definition_v2(&def).is_err());
    }

    #[test]
    fn validate_rejects_empty_capabilities() {
        let def = SkillDefinitionV2 {
            skill_format: "elegy-skill-definition".to_string(),
            skill_version: 2,
            identity: SkillIdentityV2 {
                namespace: "x".to_string(),
                name: "y".to_string(),
                ..Default::default()
            },
            capabilities: vec![],
            lifecycle_state: "draft".to_string(),
            ..Default::default()
        };
        assert!(validate_skill_definition_v2(&def).is_err());
    }

    #[test]
    fn strict_validation_rejects_subprocess_capability_without_output_schema_ref() {
        let json =
            include_str!("../../../../contracts/fixtures/skill.negative-no-output-schema.json");
        let def: SkillDefinitionV2 =
            serde_json::from_str(json).expect("negative fixture should parse");
        // Base validation should pass (it doesn't enforce output.schemaRef)
        validate_skill_definition_v2(&def)
            .expect("base validation should accept fixture with missing output.schemaRef");
        // Strict validation MUST reject subprocess capability without output.schemaRef
        let strict_result = validate_skill_definition_v2_strict(&def);
        assert!(
            strict_result.is_err(),
            "strict validation must reject subprocess capability without output.schemaRef"
        );
        let err_msg = strict_result
            .expect_err(
                "strict validation must reject subprocess capability without output.schemaRef",
            )
            .to_string();
        assert!(
            err_msg.contains("must declare output.schemaRef"),
            "error message must mention output.schemaRef, got: {err_msg}"
        );
    }
}
