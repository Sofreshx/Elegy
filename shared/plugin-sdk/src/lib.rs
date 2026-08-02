// ── Elegy Plugin SDK ──────────────────────────────────────────────────────
// Self-contained SDK for building Elegy plugin repositories.
// Zero internal Elegy workspace dependencies.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub mod capability_package;

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

// ── Structured Failure validation ─────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StructuredFailureValidationResult {
    pub issues: Vec<String>,
}

impl StructuredFailureValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
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

// ── Plugin V1 ─────────────────────────────────────────────────────────────

pub const ELEGY_PLUGIN_V1_SCHEMA_VERSION: &str = "elegy-plugin/v1";
pub const ELEGY_PLUGIN_V2_SCHEMA_VERSION: &str = "elegy-plugin/v2";
pub const ELEGY_PLUGIN_V3_SCHEMA_VERSION: &str = "elegy-plugin/v3";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_catalog: Option<ElegyPluginCapabilityCatalog>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connections: Option<ElegyPluginConnections>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<ElegyPluginReadiness>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Current plugin manifest shape. The v1 type name remains as a source-compatible
/// alias while readers support both `elegy-plugin/v1` and `elegy-plugin/v2`.
pub type ElegyPluginV2 = ElegyPluginV1;

/// Codex-compatible plugin envelope with Elegy governance kept in one
/// namespaced object. Codex-native values deliberately remain JSON values
/// where Codex accepts multiple wire shapes (path, path list, or inline map).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ElegyPluginV3 {
    pub schema_version: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<ElegyPluginV1Author>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apps: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<CodexPluginInterface>,
    pub elegy: ElegyPluginGovernanceV3,
    /// Preserve future Codex-native fields so import/export never silently
    /// erases a field merely because this SDK predates it.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginGovernanceV3 {
    pub surface_class: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_catalog: Option<ElegyPluginCapabilityCatalog>,
    pub connections: ElegyPluginConnections,
    pub readiness: ElegyPluginReadiness,
    #[serde(default)]
    pub mcp_authentication: BTreeMap<String, ElegyMcpAuthenticationExpectation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub package_assets: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ElegyMcpAuthenticationMode {
    None,
    McpOauth,
    BearerEnv,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyMcpAuthenticationExpectation {
    pub mode: ElegyMcpAuthenticationMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_variable: Option<String>,
}

pub const ELEGY_READINESS_V1_SCHEMA_VERSION: &str = "elegy-readiness/v1";

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ElegyReadinessStage {
    Concept,
    #[default]
    Implemented,
    Usable,
    Production,
}

impl ElegyReadinessStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Concept => "concept",
            Self::Implemented => "implemented",
            Self::Usable => "usable",
            Self::Production => "production",
        }
    }

    pub fn is_agent_routable(self) -> bool {
        matches!(self, Self::Usable | Self::Production)
    }
}

impl std::fmt::Display for ElegyReadinessStage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginReadiness {
    pub stage: ElegyReadinessStage,
    pub path: String,
    pub schema_version: String,
}

impl ElegyPluginV1 {
    /// Missing readiness is backward-compatible, but never agent-routable.
    pub fn readiness_stage(&self) -> ElegyReadinessStage {
        self.readiness
            .as_ref()
            .map(|readiness| readiness.stage)
            .unwrap_or(ElegyReadinessStage::Implemented)
    }

    pub fn is_agent_routable(&self) -> bool {
        self.readiness.is_some() && self.readiness_stage().is_agent_routable()
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ElegyReadinessEvidenceKind {
    SourceTests,
    PackageVerification,
    CleanInstall,
    RealTask,
    Release,
    Consumer,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyReadinessEvidence {
    pub kind: ElegyReadinessEvidenceKind,
    pub path: String,
    pub summary: String,
    #[serde(default)]
    pub non_fixture: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyReadinessV1 {
    pub schema_version: String,
    pub surface: String,
    pub surface_version: String,
    pub stage: ElegyReadinessStage,
    pub summary: String,
    pub works_today: Vec<String>,
    pub limitations: Vec<String>,
    pub supported_environments: Vec<String>,
    pub installation: String,
    pub invocation: String,
    pub evidence: Vec<ElegyReadinessEvidence>,
}

impl ElegyReadinessV1 {
    pub fn validation_issues(&self) -> Vec<String> {
        let mut issues = Vec::new();
        if self.schema_version != ELEGY_READINESS_V1_SCHEMA_VERSION {
            issues.push(format!(
                "schemaVersion must be '{}'.",
                ELEGY_READINESS_V1_SCHEMA_VERSION
            ));
        }
        if !validate_kebab_case_name(&self.surface) {
            issues.push("surface must be lowercase kebab-case.".to_string());
        }
        if !validate_semver(&self.surface_version) {
            issues.push("surfaceVersion must be valid SemVer.".to_string());
        }
        for (field, values) in [
            ("worksToday", &self.works_today),
            ("limitations", &self.limitations),
            ("supportedEnvironments", &self.supported_environments),
        ] {
            if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
                issues.push(format!("{field} must contain non-blank entries."));
            }
        }
        for (field, value) in [
            ("summary", self.summary.as_str()),
            ("installation", self.installation.as_str()),
            ("invocation", self.invocation.as_str()),
        ] {
            if value.trim().is_empty() {
                issues.push(format!("{field} must not be blank."));
            }
        }
        for evidence in &self.evidence {
            if !is_safe_package_relative_path(&evidence.path) {
                issues.push(format!(
                    "evidence path '{}' is not a safe package-relative path.",
                    evidence.path
                ));
            }
            if evidence.summary.trim().is_empty() {
                issues.push("evidence summary must not be blank.".to_string());
            }
        }

        let has = |kind| self.evidence.iter().any(|item| item.kind == kind);
        if matches!(
            self.stage,
            ElegyReadinessStage::Implemented
                | ElegyReadinessStage::Usable
                | ElegyReadinessStage::Production
        ) {
            for (kind, label) in [
                (ElegyReadinessEvidenceKind::SourceTests, "source-tests"),
                (
                    ElegyReadinessEvidenceKind::PackageVerification,
                    "package-verification",
                ),
            ] {
                if !has(kind) {
                    issues.push(format!(
                        "{label} evidence is required for implemented readiness."
                    ));
                }
            }
        }
        if matches!(
            self.stage,
            ElegyReadinessStage::Usable | ElegyReadinessStage::Production
        ) {
            if !has(ElegyReadinessEvidenceKind::CleanInstall) {
                issues.push("clean-install evidence is required for usable readiness.".to_string());
            }
            if !self
                .evidence
                .iter()
                .any(|item| item.kind == ElegyReadinessEvidenceKind::RealTask && item.non_fixture)
            {
                issues.push(
                    "real-task evidence marked non-fixture is required for usable readiness."
                        .to_string(),
                );
            }
        }
        if self.stage == ElegyReadinessStage::Production {
            if !has(ElegyReadinessEvidenceKind::Release) {
                issues.push("release evidence is required for production readiness.".to_string());
            }
            if !has(ElegyReadinessEvidenceKind::Consumer) {
                issues.push("consumer evidence is required for production readiness.".to_string());
            }
        }
        issues
    }

    pub fn is_agent_routable(&self) -> bool {
        self.stage.is_agent_routable() && self.validation_issues().is_empty()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginConnections {
    pub requirements: ElegyPluginConnectionRequirements,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ElegyPluginConnectionProvider>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginConnectionRequirements {
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginConnectionProvider {
    pub path: String,
    pub schema_version: String,
}

pub const ELEGY_PLUGIN_CONNECTIONS_V1_SCHEMA_VERSION: &str = "elegy-plugin-connections/v1";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginConnectionsV1 {
    pub schema_version: String,
    pub plugin: String,
    pub plugin_version: String,
    pub requirements: Vec<ElegyConnectionRequirement>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyConnectionRequirement {
    pub id: String,
    pub service: String,
    pub required: bool,
    pub description: String,
}

pub const ELEGY_CONNECTION_PROVIDER_V1_SCHEMA_VERSION: &str = "elegy-connection-provider/v1";
pub const ELEGY_CONNECTION_CONTROL_V1_PROTOCOL_VERSION: &str = "elegy-connection-control/v1";

/// Host-neutral descriptor for a plugin that brokers connection lifecycle.
///
/// The invocation exposes connection control, not credentials. Hosts retain
/// responsibility for authentication UX and secure credential storage.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyConnectionProviderV1 {
    pub schema_version: String,
    pub id: String,
    pub control_protocol: String,
    pub invocation: ElegyConnectionProviderInvocation,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyConnectionProviderInvocation {
    pub executable: String,
    pub command: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginCapabilityCatalog {
    pub path: String,
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness_command: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyPluginV1Author {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

pub const ELEGY_MARKETPLACE_V1_SCHEMA_VERSION: &str = "elegy-marketplace/v1";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyMarketplaceV1 {
    pub schema_version: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<ElegyMarketplaceInterface>,
    pub plugins: Vec<ElegyMarketplacePlugin>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyMarketplaceInterface {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyMarketplacePlugin {
    pub name: String,
    pub source: ElegyMarketplaceSource,
    pub category: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ElegyMarketplaceArtifact>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyMarketplaceSource {
    pub source: String,
    pub path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyMarketplaceArtifact {
    pub target: String,
    pub url: String,
    pub checksum_url: String,
}

pub const ELEGY_MARKETPLACE_V2_SCHEMA_VERSION: &str = "elegy-marketplace/v2";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyMarketplaceV2 {
    pub schema_version: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<ElegyMarketplaceInterface>,
    pub plugins: Vec<ElegyMarketplacePluginV2>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyMarketplacePluginV2 {
    pub name: String,
    pub source: ElegyMarketplaceSourceV2,
    pub policy: ElegyMarketplacePolicy,
    pub category: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ElegyMarketplaceArtifact>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(tag = "source", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ElegyMarketplaceSourceV2 {
    Local {
        path: String,
    },
    Git {
        url: String,
        #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sha: Option<String>,
    },
    GitSubdirectory {
        url: String,
        root: String,
        #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sha: Option<String>,
    },
    Npm {
        package: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        registry: Option<String>,
    },
    ElegyArtifact {
        path: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyMarketplacePolicy {
    pub installation: ElegyMarketplaceInstallationPolicy,
    pub authentication: ElegyMarketplaceAuthenticationPolicy,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub enum ElegyMarketplaceInstallationPolicy {
    #[serde(rename = "NOT_AVAILABLE")]
    NotAvailable,
    #[serde(rename = "AVAILABLE")]
    Available,
    #[serde(rename = "INSTALLED_BY_DEFAULT")]
    InstalledByDefault,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub enum ElegyMarketplaceAuthenticationPolicy {
    #[serde(rename = "ON_INSTALL")]
    OnInstall,
    #[serde(rename = "ON_USE")]
    OnUse,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ElegyMarketplaceValidationResult {
    pub issues: Vec<String>,
}

impl ElegyMarketplaceValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

pub fn validate_elegy_marketplace_v2(
    marketplace: &ElegyMarketplaceV2,
) -> ElegyMarketplaceValidationResult {
    let mut issues = Vec::new();
    if marketplace.schema_version != ELEGY_MARKETPLACE_V2_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{}'.",
            ELEGY_MARKETPLACE_V2_SCHEMA_VERSION
        ));
    }
    if !validate_kebab_case_name(&marketplace.name) {
        issues.push("marketplace name must be lowercase kebab-case.".to_string());
    }
    let mut names = BTreeSet::new();
    for plugin in &marketplace.plugins {
        if !validate_kebab_case_name(&plugin.name) {
            issues.push(format!(
                "plugin name '{}' must be lowercase kebab-case.",
                plugin.name
            ));
        } else if !names.insert(plugin.name.clone()) {
            issues.push(format!("duplicate plugin name '{}'.", plugin.name));
        }
        if plugin.category.trim().is_empty() {
            issues.push(format!(
                "plugin '{}' category must not be blank.",
                plugin.name
            ));
        }
        match &plugin.source {
            ElegyMarketplaceSourceV2::Local { path }
            | ElegyMarketplaceSourceV2::ElegyArtifact { path } => {
                if !is_safe_marketplace_source_path(path) {
                    issues.push(format!(
                        "plugin '{}' source path must be a safe ./-prefixed relative path.",
                        plugin.name
                    ));
                }
            }
            ElegyMarketplaceSourceV2::Git { url, .. }
            | ElegyMarketplaceSourceV2::GitSubdirectory { url, .. } => {
                validate_https_url(
                    &format!("plugin '{}' source url", plugin.name),
                    url,
                    &mut issues,
                );
            }
            ElegyMarketplaceSourceV2::Npm {
                package, registry, ..
            } => {
                if package.trim().is_empty() {
                    issues.push(format!(
                        "plugin '{}' npm package must not be blank.",
                        plugin.name
                    ));
                }
                if let Some(registry) = registry {
                    validate_https_url(
                        &format!("plugin '{}' npm registry", plugin.name),
                        registry,
                        &mut issues,
                    );
                }
            }
        }
        if matches!(
            plugin.source,
            ElegyMarketplaceSourceV2::Git { .. }
                | ElegyMarketplaceSourceV2::GitSubdirectory { .. }
                | ElegyMarketplaceSourceV2::Npm { .. }
        ) && plugin.policy.installation != ElegyMarketplaceInstallationPolicy::NotAvailable
        {
            issues.push(format!(
                "plugin '{}' uses a descriptor-only source and must declare installation NOT_AVAILABLE until materialization is implemented.",
                plugin.name
            ));
        }
        if let ElegyMarketplaceSourceV2::GitSubdirectory { root, .. } = &plugin.source {
            if root.trim().is_empty()
                || root.starts_with('/')
                || root.contains('\\')
                || root.split('/').any(|segment| segment == "..")
            {
                issues.push(format!(
                    "plugin '{}' git subdirectory root is unsafe.",
                    plugin.name
                ));
            }
        }
        validate_marketplace_artifacts(&plugin.name, &plugin.artifacts, &mut issues);
    }
    ElegyMarketplaceValidationResult { issues }
}

pub fn validate_elegy_marketplace_v1(
    marketplace: &ElegyMarketplaceV1,
) -> ElegyMarketplaceValidationResult {
    let mut issues = Vec::new();
    if marketplace.schema_version != ELEGY_MARKETPLACE_V1_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{}', found '{}'.",
            ELEGY_MARKETPLACE_V1_SCHEMA_VERSION, marketplace.schema_version
        ));
    }
    if !validate_kebab_case_name(&marketplace.name) {
        issues.push("marketplace name must be lowercase kebab-case.".to_string());
    }
    let mut names = BTreeSet::new();
    for plugin in &marketplace.plugins {
        if !validate_kebab_case_name(&plugin.name) {
            issues.push(format!(
                "plugin name '{}' must be lowercase kebab-case.",
                plugin.name
            ));
        } else if !names.insert(plugin.name.clone()) {
            issues.push(format!("duplicate plugin name '{}'.", plugin.name));
        }
        if plugin.source.source != "local" {
            issues.push(format!(
                "plugin '{}' source.source must be 'local'.",
                plugin.name
            ));
        }
        if !is_safe_marketplace_source_path(&plugin.source.path) {
            issues.push(format!(
                "plugin '{}' source.path must be a safe ./-prefixed relative path.",
                plugin.name
            ));
        }
        if plugin.category.trim().is_empty() {
            issues.push(format!(
                "plugin '{}' category must not be blank.",
                plugin.name
            ));
        }

        validate_marketplace_artifacts(&plugin.name, &plugin.artifacts, &mut issues);
    }

    ElegyMarketplaceValidationResult { issues }
}

fn validate_marketplace_artifacts(
    plugin_name: &str,
    artifacts: &[ElegyMarketplaceArtifact],
    issues: &mut Vec<String>,
) {
    let mut targets = BTreeSet::new();
    for artifact in artifacts {
        if !matches!(
            artifact.target.as_str(),
            "any" | "x86_64-pc-windows-msvc" | "x86_64-unknown-linux-gnu" | "aarch64-apple-darwin"
        ) {
            issues.push(format!(
                "plugin '{plugin_name}' has unsupported artifact target '{}'.",
                artifact.target
            ));
        } else if !targets.insert(artifact.target.clone()) {
            issues.push(format!(
                "plugin '{plugin_name}' has duplicate artifact target '{}'.",
                artifact.target
            ));
        }
        validate_https_url(
            &format!("plugin '{plugin_name}' artifact url"),
            &artifact.url,
            issues,
        );
        validate_https_url(
            &format!("plugin '{plugin_name}' artifact checksumUrl"),
            &artifact.checksum_url,
            issues,
        );
    }
}

fn is_safe_marketplace_source_path(path: &str) -> bool {
    is_safe_package_relative_path(path)
        && path
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "/._-".contains(character))
}

pub fn select_marketplace_artifact<'a>(
    plugin: &'a ElegyMarketplacePlugin,
    target: &str,
) -> Option<&'a ElegyMarketplaceArtifact> {
    plugin
        .artifacts
        .iter()
        .find(|artifact| artifact.target == target)
        .or_else(|| {
            plugin
                .artifacts
                .iter()
                .find(|artifact| artifact.target == "any")
        })
}

pub fn select_marketplace_artifact_v2<'a>(
    plugin: &'a ElegyMarketplacePluginV2,
    target: &str,
) -> Option<&'a ElegyMarketplaceArtifact> {
    plugin
        .artifacts
        .iter()
        .find(|artifact| artifact.target == target)
        .or_else(|| {
            plugin
                .artifacts
                .iter()
                .find(|artifact| artifact.target == "any")
        })
}

fn validate_https_url(field: &str, value: &str, issues: &mut Vec<String>) {
    match url::Url::parse(value) {
        Ok(url) if url.scheme() == "https" && url.host_str().is_some() => {}
        _ => issues.push(format!("{field} must be an absolute HTTPS URL.")),
    }
}

// ── Capability Catalog V1 ─────────────────────────────────────────────────

pub const ELEGY_CAPABILITY_CATALOG_V1_SCHEMA_VERSION: &str = "elegy-capability-catalog/v1";

/// Capability kind taxonomy discriminator.
///
/// Determines which Codex export surface a capability maps to.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ElegyCapabilityKind {
    /// Executable deterministic or controlled commands. Invoked via `elegy-*` binaries.
    #[default]
    Cli,
    /// Typed agent-facing tool server.
    Mcp,
    /// Host-authenticated external-service connector (GitHub, Gmail, Slack, etc.).
    AppBinding,
}

/// Side-effect classification for a capability.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ElegySideEffectClass {
    /// No side effects, pure computation.
    Pure,
    /// Read-only query.
    Query,
    /// State-changing mutation.
    #[default]
    Mutation,
    /// Mutation with fencing token for concurrency control.
    FencedMutation,
}

/// Shared governed `elegy-capability-catalog/v1` contract.
///
/// Referenced by the portable `elegy-plugin/v1` manifest via `capabilityCatalog.path`.
/// The catalog is a portable, host-neutral artifact. Codex-specific projection
/// (such as `.app.json` connector files) is derived by the host exporter.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyCapabilityCatalogV1 {
    pub schema_version: String,
    pub plugin: String,
    pub plugin_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    pub capabilities: Vec<ElegyCapability>,
}

/// A single capability entry in the catalog.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyCapability {
    pub id: String,
    /// Capability kind. Defaults to `cli` on read when absent (backward compat).
    #[serde(default)]
    pub kind: ElegyCapabilityKind,
    pub side_effect_class: ElegySideEffectClass,
    pub contract_version: String,
    pub description: String,
    /// Invocation metadata. Required for `cli` and `mcp` kinds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation: Option<ElegyCapabilityInvocation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    /// Fallback surface for hosts that do not support the primary kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<ElegyCapabilityFallback>,
    /// App-binding metadata. Required for `app-binding` kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_binding: Option<ElegyAppBinding>,
}

/// Invocation metadata for a capability.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyCapabilityInvocation {
    pub executable: String,
    pub command: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_args: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_args: Vec<String>,
    /// MCP tool name (for `mcp` kind).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

/// Fallback surface for a capability.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyCapabilityFallback {
    pub kind: ElegyCapabilityKind,
    pub invocation: ElegyCapabilityInvocation,
}

// ── Capability Catalog V2 ─────────────────────────────────────────────────

pub const ELEGY_CAPABILITY_CATALOG_V2_SCHEMA_VERSION: &str = "elegy-capability-catalog/v2";

/// Strict v2 capability kinds. Legacy `mcp` and `app-binding` are intentionally
/// absent; those values are accepted only by the sealed v1 reader.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ElegyCapabilityKindV2 {
    Cli,
    McpResource,
    McpTool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyCapabilityInvocationV2 {
    pub executable: String,
    pub command: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_args: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "inputSchema")]
    pub input_schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "outputSchema")]
    pub output_schema: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyCapabilityV2Common {
    pub id: String,
    pub description: String,
    pub contract_version: String,
    pub side_effect_class: ElegySideEffectClass,
    pub readiness: ElegyReadinessStage,
}

/// A v2 capability entry. The internally tagged representation makes the
/// concrete interface and its required fields explicit on the wire.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ElegyCapabilityV2 {
    Cli {
        #[serde(flatten)]
        common: ElegyCapabilityV2Common,
        invocation: ElegyCapabilityInvocationV2,
    },
    McpResource {
        #[serde(flatten)]
        common: ElegyCapabilityV2Common,
        #[serde(rename = "resourceUri", alias = "resourceTemplate")]
        resource_uri: String,
        #[serde(rename = "outputSchema")]
        output_schema: Value,
    },
    McpTool {
        #[serde(flatten)]
        common: ElegyCapabilityV2Common,
        #[serde(rename = "toolName")]
        tool_name: String,
        #[serde(rename = "inputSchema")]
        input_schema: Value,
        #[serde(rename = "outputSchema")]
        output_schema: Value,
    },
}

impl ElegyCapabilityV2 {
    pub fn common(&self) -> &ElegyCapabilityV2Common {
        match self {
            Self::Cli { common, .. }
            | Self::McpResource { common, .. }
            | Self::McpTool { common, .. } => common,
        }
    }

    pub fn kind(&self) -> ElegyCapabilityKindV2 {
        match self {
            Self::Cli { .. } => ElegyCapabilityKindV2::Cli,
            Self::McpResource { .. } => ElegyCapabilityKindV2::McpResource,
            Self::McpTool { .. } => ElegyCapabilityKindV2::McpTool,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyCapabilityCatalogV2 {
    pub schema_version: String,
    pub plugin: String,
    pub plugin_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    pub capabilities: Vec<ElegyCapabilityV2>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ElegyCapabilityCatalog {
    V1(ElegyCapabilityCatalogV1),
    V2(ElegyCapabilityCatalogV2),
}

/// App-binding metadata for a capability.
///
/// Declares the portable external-service identity. The Codex exporter
/// emits `connector` as the `id` in `.app.json` and `category` as the `category`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyAppBinding {
    /// External-service identity (e.g. `github`, `gmail`, `slack`). Portable and host-neutral.
    pub connector: String,
    /// Display category for the connector (e.g. `Developer Tools`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

/// Codex-specific extension metadata under `extensions["codex.plugin/v1"]`.
/// Declares host-specific fields that do not belong in the base manifest.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexPluginExtensionV1 {
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<CodexPluginInterface>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apps: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_bindings: Option<BTreeMap<String, CodexConnectionBinding>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<String>,
    /// Relative path(s) to additional non-skill assets to include in the Codex export.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexConnectionBinding {
    pub id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexPluginInterface {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<String>>,
    #[serde(
        default,
        rename = "websiteURL",
        skip_serializing_if = "Option::is_none"
    )]
    pub website_url: Option<String>,
    #[serde(
        default,
        rename = "privacyPolicyURL",
        skip_serializing_if = "Option::is_none"
    )]
    pub privacy_policy_url: Option<String>,
    #[serde(
        default,
        rename = "termsOfServiceURL",
        skip_serializing_if = "Option::is_none"
    )]
    pub terms_of_service_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_prompt: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composer_icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_dark: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshots: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brand_color: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexPluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<ElegyPluginV1Author>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apps: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<CodexPluginInterface>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CodexAppsFile {
    #[serde(default)]
    pub apps: BTreeMap<String, CodexAppReference>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexAppReference {
    pub id: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct CodexHooksConfig {
    #[serde(default)]
    pub hooks: BTreeMap<String, Vec<CodexHookMatcher>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexHookMatcher {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    #[serde(default)]
    pub hooks: Vec<CodexHookHandler>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexHookHandler {
    #[serde(rename = "type")]
    pub handler_type: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_windows: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    #[serde(
        default,
        rename = "async",
        alias = "async_",
        skip_serializing_if = "Option::is_none"
    )]
    pub async_: Option<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Extract the `codex.plugin/v1` extension from a plugin manifest's extensions map.
pub fn extract_codex_extension_v1(
    extensions: &Option<serde_json::Map<String, serde_json::Value>>,
) -> Option<CodexPluginExtensionV1> {
    let map = extensions.as_ref()?;
    let raw = map.get("codex.plugin/v1")?;
    serde_json::from_value::<CodexPluginExtensionV1>(raw.clone()).ok()
}

pub const PLUGIN_SCHEMA_ARTIFACTS: [(&str, &str); 15] = [
    ("elegy-plugin-v1.schema.json", "elegy-plugin/v1"),
    ("elegy-plugin-v2.schema.json", "elegy-plugin/v2"),
    ("elegy-plugin-v3.schema.json", "elegy-plugin/v3"),
    (
        "elegy-plugin-connections-v1.schema.json",
        "elegy-plugin-connections/v1",
    ),
    (
        "elegy-connection-provider-v1.schema.json",
        "elegy-connection-provider/v1",
    ),
    ("elegy-marketplace-v1.schema.json", "elegy-marketplace/v1"),
    ("elegy-marketplace-v2.schema.json", "elegy-marketplace/v2"),
    ("codex-plugin-extension-v1.schema.json", "codex.plugin/v1"),
    ("codex-plugin-manifest.schema.json", "codex-plugin-manifest"),
    (
        "elegy-capability-catalog-v1.schema.json",
        "elegy-capability-catalog/v1",
    ),
    ("elegy-readiness-v1.schema.json", "elegy-readiness/v1"),
    (
        "elegy-capability-catalog-v2.schema.json",
        "elegy-capability-catalog/v2",
    ),
    ("elegy-package-v1.schema.json", "elegy-package/v1"),
    ("elegy-lock-v1.schema.json", "elegy-lock/v1"),
    ("elegy-sbom-v1.schema.json", "elegy-sbom/v1"),
];

fn generate_plugin_v2_schema() -> Result<Value, serde_json::Error> {
    let mut schema = serde_json::to_value(schema_for!(ElegyPluginV2))?;
    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        properties.insert(
            "schemaVersion".to_string(),
            serde_json::json!({"const": ELEGY_PLUGIN_V2_SCHEMA_VERSION}),
        );
    }
    if let Some(required) = schema.get_mut("required").and_then(Value::as_array_mut) {
        for field in ["capabilityCatalog", "connections", "readiness"] {
            if !required.iter().any(|value| value == field) {
                required.push(Value::String(field.to_string()));
            }
        }
    }
    Ok(schema)
}

fn generate_plugin_v3_schema() -> Result<Value, serde_json::Error> {
    let mut schema = serde_json::to_value(schema_for!(ElegyPluginV3))?;
    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        properties.insert(
            "schemaVersion".to_string(),
            serde_json::json!({"const": ELEGY_PLUGIN_V3_SCHEMA_VERSION}),
        );
        let path_list_or_map = serde_json::json!({
            "oneOf": [
                {"type":"string", "minLength":1},
                {
                    "type":"array",
                    "minItems":1,
                    "items":{"type":["string","object"]}
                },
                {"type":"object", "minProperties":1}
            ]
        });
        for field in ["skills", "apps", "hooks", "assets"] {
            properties.insert(field.to_string(), path_list_or_map.clone());
        }
        properties.insert(
            "mcpServers".to_string(),
            serde_json::json!({
                "oneOf": [
                    {"type":"string", "minLength":1},
                    {
                        "type":"object",
                        "minProperties":1,
                        "additionalProperties":{"type":"object"}
                    }
                ]
            }),
        );
    }
    Ok(schema)
}

fn generate_marketplace_v2_schema() -> Result<Value, serde_json::Error> {
    let mut schema = serde_json::to_value(schema_for!(ElegyMarketplaceV2))?;
    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        properties.insert(
            "schemaVersion".to_string(),
            serde_json::json!({"const": ELEGY_MARKETPLACE_V2_SCHEMA_VERSION}),
        );
    }
    Ok(schema)
}

fn generate_capability_catalog_v2_schema() -> Result<Value, serde_json::Error> {
    let mut schema = serde_json::to_value(schema_for!(ElegyCapabilityCatalogV2))?;
    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        properties.insert(
            "schemaVersion".to_string(),
            serde_json::json!({"const": ELEGY_CAPABILITY_CATALOG_V2_SCHEMA_VERSION}),
        );
    }
    if let Some(branches) = schema
        .get_mut("$defs")
        .and_then(Value::as_object_mut)
        .and_then(|defs| defs.get_mut("ElegyCapabilityV2"))
        .and_then(|entry| entry.get_mut("oneOf"))
        .and_then(Value::as_array_mut)
    {
        for branch in branches {
            let kind = branch
                .get("properties")
                .and_then(|properties| properties.get("kind"))
                .and_then(|kind| kind.get("const"))
                .and_then(Value::as_str);
            if matches!(kind, Some("mcp-resource")) {
                if let Some(properties) =
                    branch.get_mut("properties").and_then(Value::as_object_mut)
                {
                    properties.insert(
                        "outputSchema".to_string(),
                        serde_json::json!({"type": "object"}),
                    );
                }
            } else if matches!(kind, Some("mcp-tool")) {
                if let Some(properties) =
                    branch.get_mut("properties").and_then(Value::as_object_mut)
                {
                    properties.insert(
                        "inputSchema".to_string(),
                        serde_json::json!({"type": "object"}),
                    );
                    properties.insert(
                        "outputSchema".to_string(),
                        serde_json::json!({"type": "object"}),
                    );
                }
            }
        }
    }
    Ok(schema)
}

pub fn generate_plugin_schema_artifacts() -> Result<BTreeMap<&'static str, String>, ToolingError> {
    let schemas = [
        (
            PLUGIN_SCHEMA_ARTIFACTS[0].0,
            serde_json::to_value(schema_for!(ElegyPluginV1)),
        ),
        (PLUGIN_SCHEMA_ARTIFACTS[1].0, generate_plugin_v2_schema()),
        (PLUGIN_SCHEMA_ARTIFACTS[2].0, generate_plugin_v3_schema()),
        (
            PLUGIN_SCHEMA_ARTIFACTS[3].0,
            serde_json::to_value(schema_for!(ElegyPluginConnectionsV1)),
        ),
        (
            PLUGIN_SCHEMA_ARTIFACTS[4].0,
            serde_json::to_value(schema_for!(ElegyConnectionProviderV1)),
        ),
        (
            PLUGIN_SCHEMA_ARTIFACTS[5].0,
            serde_json::to_value(schema_for!(ElegyMarketplaceV1)),
        ),
        (
            PLUGIN_SCHEMA_ARTIFACTS[6].0,
            generate_marketplace_v2_schema(),
        ),
        (
            PLUGIN_SCHEMA_ARTIFACTS[7].0,
            serde_json::to_value(schema_for!(CodexPluginExtensionV1)),
        ),
        (
            PLUGIN_SCHEMA_ARTIFACTS[8].0,
            serde_json::to_value(schema_for!(CodexPluginManifest)),
        ),
        (
            PLUGIN_SCHEMA_ARTIFACTS[9].0,
            serde_json::to_value(schema_for!(ElegyCapabilityCatalogV1)),
        ),
        (
            PLUGIN_SCHEMA_ARTIFACTS[10].0,
            serde_json::to_value(schema_for!(ElegyReadinessV1)),
        ),
        (
            PLUGIN_SCHEMA_ARTIFACTS[11].0,
            generate_capability_catalog_v2_schema(),
        ),
        (
            PLUGIN_SCHEMA_ARTIFACTS[12].0,
            serde_json::to_value(schema_for!(capability_package::ElegyPackageV1)),
        ),
        (
            PLUGIN_SCHEMA_ARTIFACTS[13].0,
            serde_json::to_value(schema_for!(capability_package::ElegyLockV1)),
        ),
        (
            PLUGIN_SCHEMA_ARTIFACTS[14].0,
            serde_json::to_value(schema_for!(capability_package::ElegySbomV1)),
        ),
    ];
    let mut artifacts = BTreeMap::new();
    for (file_name, schema) in schemas {
        let schema = schema.map_err(|source| ToolingError::Json {
            path: PathBuf::from(file_name),
            source,
        })?;
        let mut content =
            serde_json::to_string_pretty(&schema).map_err(|source| ToolingError::Json {
                path: PathBuf::from(file_name),
                source,
            })?;
        content.push('\n');
        artifacts.insert(file_name, content);
    }
    Ok(artifacts)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginArchiveBinary<'a> {
    pub source_path: &'a Path,
    pub archive_path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ElegyPluginV1ValidationResult {
    pub issues: Vec<String>,
}

impl ElegyPluginV1ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

pub fn validate_elegy_plugin_v3(plugin: &ElegyPluginV3) -> ElegyPluginV1ValidationResult {
    let mut issues = Vec::new();

    if plugin.schema_version != ELEGY_PLUGIN_V3_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{}', found '{}'.",
            ELEGY_PLUGIN_V3_SCHEMA_VERSION, plugin.schema_version
        ));
    }
    if !validate_kebab_case_name(&plugin.name) {
        issues.push("name must be lowercase kebab-case.".to_string());
    }
    if !validate_semver(&plugin.version) {
        issues.push("version must be valid SemVer 2.0.0.".to_string());
    }
    if plugin.description.trim().is_empty() {
        issues.push("description must not be blank.".to_string());
    }
    if !matches!(
        plugin.elegy.surface_class.as_str(),
        "adapter-plugin"
            | "tool"
            | "skill"
            | "host-adapter"
            | "host-extension"
            | "package-envelope"
    ) {
        issues.push(
            "elegy.surfaceClass must be adapter-plugin, tool, skill, host-adapter, host-extension, or package-envelope."
                .to_string(),
        );
    }
    if plugin.elegy.surface_class == "adapter-plugin" && plugin.elegy.capability_catalog.is_none() {
        issues.push(
            "adapter-plugin packages require elegy.capabilityCatalog as typed discovery authority."
                .to_string(),
        );
    }
    if plugin.elegy.readiness.schema_version != ELEGY_READINESS_V1_SCHEMA_VERSION {
        issues.push(format!(
            "elegy.readiness.schemaVersion must be '{}'.",
            ELEGY_READINESS_V1_SCHEMA_VERSION
        ));
    }
    if !is_safe_package_relative_path(&plugin.elegy.readiness.path) {
        issues.push("elegy.readiness.path must be a safe package-relative path.".to_string());
    }
    for asset in &plugin.elegy.package_assets {
        if !is_safe_package_relative_path(asset) {
            issues.push(format!(
                "elegy.packageAssets path '{asset}' is not package-relative."
            ));
        }
    }

    validate_codex_native_shape(
        "skills",
        plugin.skills.as_ref(),
        CodexShape::PathListOrMap,
        &mut issues,
    );
    validate_codex_native_shape(
        "mcpServers",
        plugin.mcp_servers.as_ref(),
        CodexShape::PathOrServerMap,
        &mut issues,
    );
    validate_codex_native_shape(
        "apps",
        plugin.apps.as_ref(),
        CodexShape::PathListOrMap,
        &mut issues,
    );
    validate_codex_native_shape(
        "hooks",
        plugin.hooks.as_ref(),
        CodexShape::PathListOrMap,
        &mut issues,
    );
    validate_codex_native_shape(
        "assets",
        plugin.assets.as_ref(),
        CodexShape::PathListOrMap,
        &mut issues,
    );

    if plugin.mcp_servers.is_none() && !plugin.elegy.mcp_authentication.is_empty() {
        issues.push(
            "elegy.mcpAuthentication cannot declare servers when mcpServers is absent.".to_string(),
        );
    }

    if let Some(Value::Object(servers)) = &plugin.mcp_servers {
        for (server_name, server) in servers {
            let is_server_entry = server.is_object();
            if is_server_entry && !plugin.elegy.mcp_authentication.contains_key(server_name) {
                issues.push(format!(
                    "MCP server '{server_name}' requires an explicit authentication expectation."
                ));
            }
            let remote = server.as_object().is_some_and(|server| {
                server.contains_key("url")
                    || server.contains_key("httpUrl")
                    || server.contains_key("http_url")
            });
            if remote
                && plugin
                    .elegy
                    .mcp_authentication
                    .get(server_name)
                    .is_some_and(|expectation| expectation.mode == ElegyMcpAuthenticationMode::None)
            {
                issues.push(format!(
                    "remote MCP server '{server_name}' cannot declare unauthenticated mode."
                ));
            }
        }
        for declared in plugin.elegy.mcp_authentication.keys() {
            if !servers.contains_key(declared) {
                issues.push(format!(
                    "elegy.mcpAuthentication declares unknown MCP server '{declared}'."
                ));
            }
        }
    }

    for (server_name, expectation) in &plugin.elegy.mcp_authentication {
        if expectation.mode == ElegyMcpAuthenticationMode::BearerEnv {
            if expectation
                .environment_variable
                .as_deref()
                .is_none_or(|name| !is_environment_variable_name(name))
            {
                issues.push(format!(
                    "MCP server '{server_name}' uses bearer-env but elegy.mcpAuthentication declares no valid environmentVariable."
                ));
            }
        } else if expectation.environment_variable.is_some() {
            issues.push(format!(
                "MCP server '{server_name}' declares environmentVariable without bearer-env mode."
            ));
        }
    }

    if serde_json::to_value(plugin)
        .ok()
        .is_some_and(|value| contains_plaintext_authentication_material(&value))
    {
        issues.push(
            "plugin manifest contains plaintext authentication material; use host OAuth or environment-backed bindings."
                .to_string(),
        );
    }

    ElegyPluginV1ValidationResult { issues }
}

fn is_environment_variable_name(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

#[derive(Clone, Copy)]
enum CodexShape {
    PathListOrMap,
    PathOrServerMap,
}

fn validate_codex_native_shape(
    field: &str,
    value: Option<&Value>,
    shape: CodexShape,
    issues: &mut Vec<String>,
) {
    let Some(value) = value else {
        return;
    };
    let valid_path = |value: &str| !value.trim().is_empty() && is_safe_package_relative_path(value);
    let valid = match (shape, value) {
        (_, Value::String(path)) => valid_path(path),
        (CodexShape::PathListOrMap, Value::Array(values)) => {
            !values.is_empty()
                && values.iter().all(|value| match value {
                    Value::String(path) => valid_path(path),
                    Value::Object(object) => !object.is_empty(),
                    _ => false,
                })
        }
        (CodexShape::PathListOrMap, Value::Object(values)) => {
            !values.is_empty()
                && values.values().all(|value| match value {
                    Value::String(path) => valid_path(path),
                    Value::Object(object) => !object.is_empty(),
                    _ => false,
                })
        }
        (CodexShape::PathOrServerMap, Value::Object(values)) => {
            !values.is_empty() && values.values().all(Value::is_object)
        }
        _ => false,
    };
    if !valid {
        issues.push(format!(
            "{field} must use a supported non-empty Codex path, list, or inline object shape."
        ));
    }
}

fn contains_plaintext_authentication_material(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, child)| {
            let normalized = key
                .chars()
                .filter(|character| character.is_ascii_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>();
            let secret_key = normalized.ends_with("authorization")
                || normalized == "cookie"
                || normalized == "setcookie"
                || normalized.ends_with("apikey")
                || normalized.ends_with("token")
                || normalized.ends_with("secret")
                || normalized.ends_with("password");
            secret_key || contains_plaintext_authentication_material(child)
        }),
        Value::Array(values) => values
            .iter()
            .any(contains_plaintext_authentication_material),
        Value::String(value) => {
            let normalized = value.trim_start().to_ascii_lowercase();
            normalized.starts_with("bearer ") || normalized.starts_with("basic ")
        }
        _ => false,
    }
}

/// Produce the native Codex manifest from a v3 envelope. Elegy governance is
/// the only information removed; all Codex-native and future fields survive.
pub fn project_codex_plugin_v3(plugin: &ElegyPluginV3) -> Result<Value, ToolingError> {
    let validation = validate_elegy_plugin_v3(plugin);
    if !validation.is_valid() {
        return Err(ToolingError::InvalidPluginPackage {
            path: PathBuf::from(".elegy-plugin/plugin.json"),
            issues: validation.issues,
        });
    }
    let mut value = serde_json::to_value(plugin).map_err(|source| ToolingError::Json {
        path: PathBuf::from(".elegy-plugin/plugin.json"),
        source,
    })?;
    let object = value
        .as_object_mut()
        .expect("serialized plugin manifest is always an object");
    object.remove("schemaVersion");
    object.remove("elegy");
    Ok(value)
}

pub fn validate_elegy_plugin_v1(plugin: &ElegyPluginV1) -> ElegyPluginV1ValidationResult {
    let mut issues = Vec::new();

    if !matches!(
        plugin.schema_version.as_str(),
        ELEGY_PLUGIN_V1_SCHEMA_VERSION | ELEGY_PLUGIN_V2_SCHEMA_VERSION
    ) {
        issues.push(format!(
            "schemaVersion must be '{}' or '{}', found '{}'.",
            ELEGY_PLUGIN_V1_SCHEMA_VERSION, ELEGY_PLUGIN_V2_SCHEMA_VERSION, plugin.schema_version
        ));
    }

    if plugin.schema_version == ELEGY_PLUGIN_V2_SCHEMA_VERSION && plugin.connections.is_none() {
        issues.push(
            "elegy-plugin/v2 requires an explicit connections.requirements declaration.".into(),
        );
    }
    if plugin.schema_version == ELEGY_PLUGIN_V2_SCHEMA_VERSION && plugin.readiness.is_none() {
        issues.push("elegy-plugin/v2 requires readiness evidence declaration.".into());
    }
    if plugin.schema_version == ELEGY_PLUGIN_V2_SCHEMA_VERSION
        && plugin.capability_catalog.is_none()
    {
        issues.push(
            "elegy-plugin/v2 requires capabilityCatalog as typed executable discovery authority."
                .into(),
        );
    }

    if let Some(readiness) = &plugin.readiness {
        if !is_safe_package_relative_path(&readiness.path) {
            issues.push(format!(
                "readiness path '{}' is not a safe package-relative path.",
                readiness.path
            ));
        }
        if readiness.schema_version != ELEGY_READINESS_V1_SCHEMA_VERSION {
            issues.push(format!(
                "readiness schemaVersion must be '{}'.",
                ELEGY_READINESS_V1_SCHEMA_VERSION
            ));
        }
    }

    if let Some(connections) = &plugin.connections {
        match connections.requirements.mode.as_str() {
            "none" => {
                if connections.requirements.path.is_some()
                    || connections.requirements.schema_version.is_some()
                {
                    issues.push(
                        "connections.requirements mode 'none' must not declare path or schemaVersion."
                            .into(),
                    );
                }
            }
            "declared" => {
                match connections.requirements.path.as_deref() {
                    Some(path) if is_safe_package_relative_path(path) => {}
                    Some(path) => issues.push(format!(
                        "connections.requirements path '{path}' is not a safe package-relative path."
                    )),
                    None => issues.push(
                        "connections.requirements mode 'declared' requires path.".into(),
                    ),
                }
                if connections
                    .requirements
                    .schema_version
                    .as_deref()
                    .is_none_or(str::is_empty)
                {
                    issues.push(
                        "connections.requirements mode 'declared' requires schemaVersion.".into(),
                    );
                }
            }
            other => issues.push(format!(
                "connections.requirements mode must be 'none' or 'declared', found '{other}'."
            )),
        }

        if let Some(provider) = &connections.provider {
            if !is_safe_package_relative_path(&provider.path) {
                issues.push(format!(
                    "connections.provider path '{}' is not a safe package-relative path.",
                    provider.path
                ));
            }
            if provider.schema_version.trim().is_empty() {
                issues.push("connections.provider.schemaVersion must not be empty.".into());
            }
        }
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

    if let Some(path) = &plugin.skills {
        if !is_safe_skill_package_path(path) {
            issues.push(format!(
                "skills path '{path}' is not a safe package-relative path.",
            ));
        }
    }

    if let Some(path) = &plugin.mcp_servers {
        if !is_safe_package_relative_path(path) {
            issues.push(format!(
                "mcpServers path '{path}' is not a safe package-relative path.",
            ));
        }
    }

    if let Some(catalog) = &plugin.capability_catalog {
        if !is_safe_package_relative_path(&catalog.path) {
            issues.push(format!(
                "capabilityCatalog path '{}' is not a safe package-relative path.",
                catalog.path
            ));
        }
        if catalog.schema_version.trim().is_empty() {
            issues.push("capabilityCatalog.schemaVersion must not be empty.".into());
        }
    }

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

    if let Some(repo) = &plugin.repository {
        validate_uri("repository", repo, &mut issues);
    }

    if plugin.skills.is_none() && plugin.mcp_servers.is_none() && !is_marketplace_wrapper(plugin) {
        issues.push("At least one of skills or mcpServers must be declared.".into());
    }

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

            if let Some(codex_ext) = extract_codex_extension_v1(&plugin.extensions) {
                validate_codex_extension_v1(&codex_ext, &mut issues);
            }
        }
    }

    ElegyPluginV1ValidationResult { issues }
}

fn is_marketplace_wrapper(plugin: &ElegyPluginV1) -> bool {
    plugin
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.get("elegy.marketplace-wrapper/v1"))
        .and_then(serde_json::Value::as_object)
        .and_then(|extension| extension.get("schemaVersion"))
        .and_then(serde_json::Value::as_str)
        == Some("elegy.marketplace-wrapper/v1")
}

fn validate_codex_extension_v1(codex_ext: &CodexPluginExtensionV1, issues: &mut Vec<String>) {
    if codex_ext.schema_version != "codex.plugin/v1" {
        issues.push(format!(
            "codex.plugin/v1 extension schemaVersion must be 'codex.plugin/v1', found '{}'.",
            codex_ext.schema_version
        ));
    }

    for (field_name, path) in [
        ("extensions.codex.plugin/v1.apps", &codex_ext.apps),
        ("extensions.codex.plugin/v1.hooks", &codex_ext.hooks),
        (
            "extensions.codex.plugin/v1.mcpServers",
            &codex_ext.mcp_servers,
        ),
    ] {
        if let Some(path) = path {
            if !is_safe_package_relative_path(path) {
                issues.push(format!(
                    "{field_name} path '{path}' is not a safe package-relative path.",
                ));
            }
        }
    }

    if let Some(assets) = &codex_ext.assets {
        for asset in assets {
            if !is_safe_package_relative_path(asset) {
                issues.push(format!(
                    "extensions.codex.plugin/v1.assets path '{asset}' is not a safe package-relative path.",
                ));
            }
        }
    }

    if let Some(interface) = &codex_ext.interface {
        validate_codex_interface_paths(interface, issues);
        for (field, value) in [
            ("interface.websiteURL", &interface.website_url),
            ("interface.privacyPolicyURL", &interface.privacy_policy_url),
            (
                "interface.termsOfServiceURL",
                &interface.terms_of_service_url,
            ),
        ] {
            if let Some(value) = value {
                validate_uri(field, value, issues);
            }
        }
    }
}

fn validate_codex_interface_paths(interface: &CodexPluginInterface, issues: &mut Vec<String>) {
    for (field_name, path) in [
        ("interface.composerIcon", &interface.composer_icon),
        ("interface.logo", &interface.logo),
        ("interface.logoDark", &interface.logo_dark),
    ] {
        if let Some(path) = path {
            if !is_safe_package_relative_path(path) && !path_is_uri(path) {
                issues.push(format!(
                    "{field_name} path '{path}' is not a safe package-relative path or URI.",
                ));
            }
        }
    }

    if let Some(screenshots) = &interface.screenshots {
        for screenshot in screenshots {
            if !is_safe_package_relative_path(screenshot) && !path_is_uri(screenshot) {
                issues.push(format!(
                    "interface.screenshots path '{screenshot}' is not a safe package-relative path or URI.",
                ));
            }
        }
    }
}

pub fn import_codex_plugin_v1(codex_plugin_path: &Path) -> Result<ElegyPluginV1, ToolingError> {
    let (package_root, manifest_path) = resolve_codex_plugin_root(codex_plugin_path)?;
    let raw = fs::read_to_string(&manifest_path).map_err(|e| ToolingError::Io {
        operation: "read",
        path: manifest_path.clone(),
        source: e,
    })?;
    let codex: CodexPluginManifest =
        serde_json::from_str(&raw).map_err(|e| ToolingError::Json {
            path: manifest_path,
            source: e,
        })?;

    let mut codex_ext = CodexPluginExtensionV1 {
        schema_version: "codex.plugin/v1".to_string(),
        homepage: codex.homepage,
        keywords: codex.keywords,
        interface: codex.interface,
        apps: codex.apps,
        hooks: codex.hooks,
        mcp_servers: codex.mcp_servers,
        extra: codex.extra,
        ..CodexPluginExtensionV1::default()
    };

    let assets = collect_codex_interface_assets(&package_root, &codex_ext.interface);
    if !assets.is_empty() {
        codex_ext.assets = Some(assets);
    }

    let mut extensions = serde_json::Map::new();
    extensions.insert(
        "codex.plugin/v1".to_string(),
        serde_json::to_value(codex_ext).map_err(|source| ToolingError::Json {
            path: PathBuf::from("codex.plugin/v1"),
            source,
        })?,
    );

    Ok(ElegyPluginV1 {
        schema_version: ELEGY_PLUGIN_V1_SCHEMA_VERSION.to_string(),
        name: codex.name,
        version: codex.version,
        description: codex.description,
        author: codex.author,
        license: codex.license,
        repository: codex.repository,
        skills: codex.skills,
        mcp_servers: None,
        capability_catalog: None,
        connections: None,
        readiness: None,
        extensions: Some(extensions),
    })
}

/// Import a Codex package into the v3 envelope without translating native
/// fields. Imported packages are deliberately `concept` and non-adapter until
/// a maintainer supplies Elegy capability and readiness authority.
pub fn import_codex_plugin_v3(codex_plugin_path: &Path) -> Result<ElegyPluginV3, ToolingError> {
    let (_package_root, manifest_path) = resolve_codex_plugin_root(codex_plugin_path)?;
    let raw = fs::read_to_string(&manifest_path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: manifest_path.clone(),
        source,
    })?;
    let mut value: Value = serde_json::from_str(&raw).map_err(|source| ToolingError::Json {
        path: manifest_path.clone(),
        source,
    })?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| ToolingError::InvalidPluginPackage {
            path: manifest_path.clone(),
            issues: vec!["Codex plugin manifest must be a JSON object.".to_string()],
        })?;
    if object.contains_key("schemaVersion") || object.contains_key("elegy") {
        return Err(ToolingError::InvalidPluginPackage {
            path: manifest_path.clone(),
            issues: vec![
                "Codex import reserves schemaVersion and elegy for the Elegy envelope.".to_string(),
            ],
        });
    }

    let mcp_authentication = object
        .get("mcpServers")
        .and_then(Value::as_object)
        .map(|servers| {
            servers
                .iter()
                .filter_map(|(name, server)| {
                    let server = server.as_object()?;
                    server.contains_key("command").then_some((
                        name.clone(),
                        ElegyMcpAuthenticationExpectation {
                            mode: ElegyMcpAuthenticationMode::None,
                            environment_variable: None,
                        },
                    ))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    object.insert(
        "schemaVersion".to_string(),
        Value::String(ELEGY_PLUGIN_V3_SCHEMA_VERSION.to_string()),
    );
    object.insert(
        "elegy".to_string(),
        serde_json::json!({
            "surfaceClass": "package-envelope",
            "connections": {"requirements": {"mode": "none"}},
            "readiness": {
                "stage": "concept",
                "path": "./readiness.json",
                "schemaVersion": ELEGY_READINESS_V1_SCHEMA_VERSION
            },
            "mcpAuthentication": mcp_authentication
        }),
    );
    serde_json::from_value(value).map_err(|source| ToolingError::Json {
        path: manifest_path,
        source,
    })
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
    let Some(relative) = path.strip_prefix("./") else {
        return false;
    };
    if relative.is_empty() || relative.contains('\\') || relative.contains(':') {
        return false;
    }
    let relative = relative.strip_suffix('/').unwrap_or(relative);
    if relative.is_empty() {
        return false;
    }
    relative
        .split('/')
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn is_safe_skill_package_path(path: &str) -> bool {
    path == "./" || is_safe_package_relative_path(path)
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

fn path_is_uri(value: &str) -> bool {
    url::Url::parse(value).is_ok()
}

fn collect_codex_interface_assets(
    package_root: &Path,
    interface: &Option<CodexPluginInterface>,
) -> Vec<String> {
    let Some(interface) = interface else {
        return Vec::new();
    };

    let mut assets = BTreeSet::new();
    for path in [
        &interface.composer_icon,
        &interface.logo,
        &interface.logo_dark,
    ]
    .into_iter()
    .flatten()
    {
        add_existing_relative_asset(package_root, path, &mut assets);
    }
    if let Some(screenshots) = &interface.screenshots {
        for screenshot in screenshots {
            add_existing_relative_asset(package_root, screenshot, &mut assets);
        }
    }

    assets.into_iter().collect()
}

fn add_existing_relative_asset(package_root: &Path, path: &str, assets: &mut BTreeSet<String>) {
    if path_is_uri(path) || !is_safe_package_relative_path(path) {
        return;
    }
    let normalized = normalize_package_relative_path(path);
    if package_root.join(&normalized).exists() {
        assets.insert(normalized);
    }
}

fn normalize_package_relative_path(path: &str) -> String {
    path.strip_prefix("./").unwrap_or(path).replace('\\', "/")
}

fn resolve_package_path(package_root: &Path, path: &str) -> PathBuf {
    package_root.join(normalize_package_relative_path(path))
}

// ── CLI Machine Envelope types ────────────────────────────────────────────

/// Schema version constant for all Elegy CLI machine-readable envelopes.
pub const CLI_SCHEMA_VERSION: &str = "elegy.cli/v1";

/// Shared JSON envelope for all Elegy CLI machine-readable output.
///
/// Every dedicated CLI surface emits this envelope when `--json` or `--format json` is active.
/// The envelope carries the schema version, a correlation ID for event tracing, the command
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

#[allow(dead_code)]
fn is_false(value: &bool) -> bool {
    !*value
}

// ── MCP (Model Context Protocol) types ────────────────────────────────────

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

// ── MCP Helpers ───────────────────────────────────────────────────────────

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

// ── MCP validation ────────────────────────────────────────────────────────

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

// ── McpToolAnalyzer ───────────────────────────────────────────────────────

pub struct McpToolAnalyzer;

impl McpToolAnalyzer {
    pub fn analyze(&self, descriptor: &McpServerDescriptor) -> McpAnalysisResult {
        McpAnalysisResult {
            server_name: descriptor.server_name.clone(),
            analyses: descriptor
                .tools
                .iter()
                .cloned()
                .map(|tool| McpToolAnalysis {
                    extracted_triggers: extract_triggers(&tool.name),
                    has_valid_schema: tool.input_schema.is_some(),
                    tool,
                })
                .collect(),
        }
    }
}

fn extract_triggers(tool_name: &str) -> Vec<SkillTrigger> {
    if tool_name.trim().is_empty() {
        return Vec::new();
    }

    let mut words = Vec::new();
    for part in tool_name.split(['-', '_']) {
        if part.is_empty() {
            continue;
        }

        words.extend(split_camel_case(part));
    }

    let pattern = words
        .into_iter()
        .map(|word| word.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ");

    vec![SkillTrigger {
        pattern,
        description: Some("Extracted from MCP tool name".to_string()),
    }]
}

fn split_camel_case(part: &str) -> Vec<String> {
    let chars: Vec<char> = part.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }

    let mut words = Vec::new();
    let mut current = String::new();

    for (index, character) in chars.iter().enumerate() {
        if index > 0 {
            let previous = chars[index - 1];
            let next = chars.get(index + 1).copied();
            let boundary = (previous.is_ascii_lowercase() && character.is_ascii_uppercase())
                || (previous.is_ascii_uppercase()
                    && character.is_ascii_uppercase()
                    && next.is_some_and(|next| next.is_ascii_lowercase()));

            if boundary && !current.is_empty() {
                words.push(current);
                current = String::new();
            }
        }

        current.push(*character);
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
}

// ── Plugin Tooling types ──────────────────────────────────────────────────

fn generated_skill_id(server_name: &str, tool_name: &str) -> String {
    let slug = build_slug(server_name, tool_name);
    format!("mcp-{slug}")
}

fn build_slug(server_name: &str, tool_name: &str) -> String {
    let combined = format!("{server_name}-{tool_name}");
    let mut slug = String::new();
    for character in combined.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if matches!(character, '-' | '_') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorMcpDescriptorRequest {
    pub server_name: String,
    pub transport: McpTransportKind,
    pub tools: Vec<AuthorMcpToolRequest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorMcpToolRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct AuthoredMcpDescriptor {
    pub output_path: String,
    pub descriptor: McpServerDescriptor,
}

/// Lightweight skill info for generated MCP skills.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct GeneratedSkillInfo {
    pub skill_name: String,
    pub display_name: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct GeneratedSkillArtifacts {
    pub source_descriptor: String,
    pub analysis: McpAnalysisResult,
    pub generated_skills: Vec<GeneratedSkillInfo>,
    pub skipped_tools: Vec<McpToolDefinition>,
    pub written_files: Vec<String>,
}

/// Shared return type for all host exports.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedHostExport {
    pub source_package: String,
    pub plugin_name: String,
    pub plugin_version: String,
    pub lossless: bool,
    pub routable: bool,
    pub losses: Vec<String>,
    pub emitted_components: GeneratedHostExportComponents,
    pub written_files: Vec<String>,
}

/// Component summary for a host export.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedHostExportComponents {
    pub plugin_manifest: String,
    pub skills_dir: String,
    pub skills_count: usize,
    pub apps_emitted: bool,
    pub mcp_servers_emitted: bool,
    pub hooks_emitted: bool,
}

#[derive(Debug, Error)]
pub enum ToolingError {
    #[error("failed to {operation} {path}: {source}")]
    Io {
        operation: &'static str,
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
    #[error("failed to parse YAML in {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("invalid MCP descriptor in {path}")]
    InvalidMcpDescriptor { path: PathBuf, issues: Vec<String> },
    #[error("invalid MCP analysis result for {path}")]
    InvalidMcpAnalysis { path: PathBuf, issues: Vec<String> },
    #[error("generated skill definition {skill_id} is invalid")]
    InvalidSkillDefinition {
        skill_id: String,
        issues: Vec<String>,
    },
    #[error("invalid Elegy plugin package in {path}")]
    InvalidPluginPackage { path: PathBuf, issues: Vec<String> },
    #[error("invalid docs config in {path}")]
    InvalidDocsConfig { path: PathBuf, issues: Vec<String> },
    #[error("invalid docs request")]
    InvalidDocsRequest { issues: Vec<String> },
    #[error("duplicate generated skill ID: {skill_id}")]
    DuplicateSkillId { skill_id: String },
    #[error("output file already exists: {path}")]
    OutputExists { path: PathBuf },
    #[error("unsupported host target: {host}")]
    UnsupportedHostTarget { host: String },
    #[error("host '{host}' cannot represent this plugin: {reason}")]
    UnsupportedHostProjection { host: String, reason: String },
}

// ── Plugin path resolution ────────────────────────────────────────────────

/// Resolve a plugin path to canonical (repo_root, manifest_path).
///
/// Accepts three forms:
/// - `<repo_root>` — directory containing `.elegy-plugin/plugin.json`
/// - `<repo_root>/.elegy-plugin` — the .elegy-plugin directory itself
/// - `<repo_root>/.elegy-plugin/plugin.json` — the manifest file
///
/// Returns `(repo_root, manifest_path)` on success.
pub fn resolve_plugin_root(plugin_path: &Path) -> Result<(PathBuf, PathBuf), ToolingError> {
    let path = plugin_path;
    if path.is_file() && path.file_name().is_some_and(|n| n == "plugin.json") {
        // Direct path to plugin.json
        let manifest = path.to_path_buf();
        let repo_root = path
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(Path::new("."));
        return Ok((repo_root.to_path_buf(), manifest));
    }
    if path.is_dir() && path.file_name().is_some_and(|n| n == ".elegy-plugin") {
        // .elegy-plugin directory
        let manifest = path.join("plugin.json");
        if !manifest.exists() {
            return Err(ToolingError::Io {
                operation: "resolve plugin manifest",
                path: manifest.clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "plugin.json not found in .elegy-plugin directory",
                ),
            });
        }
        let repo_root = path.parent().unwrap_or(Path::new("."));
        return Ok((repo_root.to_path_buf(), manifest));
    }
    if path.is_dir() {
        // Repo root — look for .elegy-plugin/plugin.json
        let manifest = path.join(".elegy-plugin").join("plugin.json");
        if manifest.exists() {
            return Ok((path.to_path_buf(), manifest));
        }
        Err(ToolingError::Io {
            operation: "resolve plugin root",
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No .elegy-plugin/plugin.json found in directory",
            ),
        })
    } else {
        Err(ToolingError::Io {
            operation: "resolve plugin path",
            path: path.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "Path does not exist"),
        })
    }
}

fn resolve_codex_plugin_root(plugin_path: &Path) -> Result<(PathBuf, PathBuf), ToolingError> {
    if plugin_path.is_file() && plugin_path.file_name().is_some_and(|n| n == "plugin.json") {
        let manifest = plugin_path.to_path_buf();
        let repo_root = plugin_path
            .parent()
            .and_then(|p| p.parent())
            .map(Path::to_path_buf)
            .ok_or_else(|| ToolingError::Io {
                operation: "resolve parent",
                path: plugin_path.to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "plugin.json must be inside .codex-plugin",
                ),
            })?;
        return Ok((repo_root, manifest));
    }

    if plugin_path.is_dir()
        && plugin_path
            .file_name()
            .is_some_and(|n| n == ".codex-plugin")
    {
        let manifest = plugin_path.join("plugin.json");
        if manifest.exists() {
            let repo_root = plugin_path.parent().unwrap_or(Path::new(".")).to_path_buf();
            return Ok((repo_root, manifest));
        }
    }

    let manifest = plugin_path.join(".codex-plugin").join("plugin.json");
    if manifest.exists() {
        return Ok((plugin_path.to_path_buf(), manifest));
    }

    Err(ToolingError::Io {
        operation: "resolve Codex plugin",
        path: plugin_path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "could not find .codex-plugin/plugin.json",
        ),
    })
}

/// Resolve plugin root and load the ElegyPluginV1 manifest.
pub fn resolve_and_load_plugin_v1(
    plugin_path: &Path,
) -> Result<(PathBuf, ElegyPluginV1), ToolingError> {
    let (repo_root, manifest_path) = resolve_plugin_root(plugin_path)?;
    let raw = fs::read_to_string(&manifest_path).map_err(|e| ToolingError::Io {
        operation: "read",
        path: manifest_path.clone(),
        source: e,
    })?;
    let plugin: ElegyPluginV1 = serde_json::from_str(&raw).map_err(|e| ToolingError::Json {
        path: manifest_path.clone(),
        source: e,
    })?;
    Ok((repo_root, plugin))
}

// ── MCP authoring and analysis ────────────────────────────────────────────

pub fn author_mcp_descriptor_to_path(
    request: AuthorMcpDescriptorRequest,
    output_path: &Path,
    overwrite: bool,
) -> Result<AuthoredMcpDescriptor, ToolingError> {
    let descriptor = build_mcp_descriptor(request)?;
    write_json_file(output_path, &descriptor, overwrite)?;

    Ok(AuthoredMcpDescriptor {
        output_path: display_path(output_path),
        descriptor,
    })
}

pub fn analyze_mcp_descriptor_file(path: &Path) -> Result<McpAnalysisResult, ToolingError> {
    let descriptor = load_mcp_descriptor_file(path)?;
    let analysis = analyze_descriptor(&descriptor);
    let validation = validate_mcp_analysis_result(&analysis);

    if !validation.is_valid() {
        return Err(ToolingError::InvalidMcpAnalysis {
            path: path.to_path_buf(),
            issues: validation.issues,
        });
    }

    Ok(analysis)
}

pub fn generate_skills_from_descriptor_file(
    descriptor_path: &Path,
    output_dir: Option<&Path>,
    overwrite: bool,
) -> Result<GeneratedSkillArtifacts, ToolingError> {
    let analysis = analyze_mcp_descriptor_file(descriptor_path)?;
    let _descriptor = load_mcp_descriptor_file(descriptor_path)?;

    let mut generated_skills = Vec::new();
    let mut skipped_tools = Vec::new();
    let mut written_files = Vec::new();

    if let Some(output_dir) = output_dir.filter(|_| !overwrite) {
        for tool_analysis in &analysis.analyses {
            if !tool_analysis.has_valid_schema {
                continue;
            }
            let skill_name = generated_skill_id(&analysis.server_name, &tool_analysis.tool.name);
            let skill_path = output_dir.join(skill_name).join("SKILL.md");
            if skill_path.exists() {
                return Err(ToolingError::OutputExists { path: skill_path });
            }
        }
    }

    // For each tool with a valid schema, generate a SKILL.md file
    for tool_analysis in &analysis.analyses {
        if !tool_analysis.has_valid_schema {
            skipped_tools.push(tool_analysis.tool.clone());
            continue;
        }

        let skill_name = generated_skill_id(&analysis.server_name, &tool_analysis.tool.name);
        let display_name = tool_analysis.tool.name.clone();
        let description = tool_analysis
            .tool
            .description
            .clone()
            .unwrap_or_else(|| format!("Call MCP tool '{}'.", tool_analysis.tool.name));

        generated_skills.push(GeneratedSkillInfo {
            skill_name: skill_name.clone(),
            display_name: display_name.clone(),
            description: description.clone(),
        });

        if let Some(output_dir) = output_dir {
            let skill_dir = output_dir.join(&skill_name);
            let skill_path = skill_dir.join("SKILL.md");

            if skill_path.exists() && !overwrite {
                return Err(ToolingError::OutputExists { path: skill_path });
            }

            fs::create_dir_all(&skill_dir).map_err(|e| ToolingError::Io {
                operation: "create directory",
                path: skill_dir.clone(),
                source: e,
            })?;

            let skill_md = format!(
                r#"---
name: {name}
description: {description}
version: "1.0"
---

# {display_name}

{description}

## Capabilities

- `{name}`: {description}

## Details

Generated from MCP server `{server}`.
"#,
                name = skill_name,
                description = description,
                display_name = display_name,
                server = analysis.server_name,
            );

            fs::write(&skill_path, &skill_md).map_err(|e| ToolingError::Io {
                operation: "write",
                path: skill_path.clone(),
                source: e,
            })?;

            written_files.push(display_path(&skill_path));
        }
    }

    Ok(GeneratedSkillArtifacts {
        source_descriptor: display_path(descriptor_path),
        analysis,
        generated_skills,
        skipped_tools,
        written_files,
    })
}

// ── V1 plugin verification, inspection, and export ────────────────────────

/// Simple verification result for a v1 plugin.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginV1VerifyResult {
    pub valid: bool,
    pub plugin_name: String,
    pub plugin_version: String,
    pub has_skills: bool,
    pub skill_count: usize,
    pub has_mcp: bool,
    pub mcp_server_count: usize,
    pub has_apps: bool,
    pub app_count: usize,
    pub has_hooks: bool,
    pub hook_event_count: usize,
    pub has_codex_interface: bool,
    pub has_codex_mcp_servers: bool,
    pub has_capability_catalog: bool,
    pub catalog_app_binding_count: usize,
    pub issues: Vec<String>,
}

pub fn verify_plugin_v3(package_dir: &Path) -> Result<PluginV1VerifyResult, ToolingError> {
    let plugin_path = package_dir.join("plugin.json");
    let raw = fs::read_to_string(&plugin_path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: plugin_path.clone(),
        source,
    })?;
    let plugin: ElegyPluginV3 =
        serde_json::from_str(&raw).map_err(|source| ToolingError::Json {
            path: plugin_path,
            source,
        })?;
    let package_root = package_dir.parent().unwrap_or(Path::new("."));
    let validation = validate_elegy_plugin_v3(&plugin);
    let mut issues = validation.issues;

    if let Some(catalog) = &plugin.elegy.capability_catalog {
        let path = resolve_package_path(package_root, &catalog.path);
        if !path.is_file() {
            issues.push(format!(
                "capability catalog '{}' does not exist.",
                catalog.path
            ));
        } else {
            match load_capability_catalog(&path) {
                Ok(ElegyCapabilityCatalog::V1(loaded)) => {
                    for issue in validate_elegy_capability_catalog_v1(&loaded).issues {
                        issues.push(format!("capability catalog: {issue}"));
                    }
                    if loaded.plugin != plugin.name {
                        issues.push(
                            "capability catalog plugin does not match manifest name.".to_string(),
                        );
                    }
                    if loaded.plugin_version != plugin.version {
                        issues.push(
                            "capability catalog pluginVersion does not match manifest version."
                                .to_string(),
                        );
                    }
                }
                Ok(ElegyCapabilityCatalog::V2(loaded)) => {
                    for issue in validate_elegy_capability_catalog_v2(&loaded).issues {
                        issues.push(format!("capability catalog: {issue}"));
                    }
                    if loaded.plugin != plugin.name {
                        issues.push(
                            "capability catalog plugin does not match manifest name.".to_string(),
                        );
                    }
                    if loaded.plugin_version != plugin.version {
                        issues.push(
                            "capability catalog pluginVersion does not match manifest version."
                                .to_string(),
                        );
                    }
                }
                Err(error) => issues.push(format!(
                    "capability catalog '{}' is invalid: {error}",
                    catalog.path
                )),
            }
        }
    }

    let readiness_path = resolve_package_path(package_root, &plugin.elegy.readiness.path);
    match fs::read_to_string(&readiness_path) {
        Ok(raw) => match serde_json::from_str::<ElegyReadinessV1>(&raw) {
            Ok(readiness) => {
                for issue in readiness.validation_issues() {
                    issues.push(format!("readiness: {issue}"));
                }
                if readiness.surface != plugin.name {
                    issues.push("readiness surface does not match manifest name.".to_string());
                }
                if readiness.surface_version != plugin.version {
                    issues.push(
                        "readiness surfaceVersion does not match manifest version.".to_string(),
                    );
                }
                if readiness.stage != plugin.elegy.readiness.stage {
                    issues.push("readiness stage does not match manifest declaration.".to_string());
                }
                for evidence in readiness.evidence {
                    let evidence_path = resolve_package_path(package_root, &evidence.path);
                    if !evidence_path.is_file() {
                        issues.push(format!(
                            "readiness evidence file '{}' does not exist.",
                            evidence.path
                        ));
                    }
                }
            }
            Err(error) => issues.push(format!(
                "readiness file '{}' is invalid: {error}",
                plugin.elegy.readiness.path
            )),
        },
        Err(_) => issues.push(format!(
            "readiness file '{}' does not exist.",
            plugin.elegy.readiness.path
        )),
    }

    let mut skill_count = 0;
    for path in v3_string_paths(plugin.skills.as_ref()) {
        let full_path = resolve_package_path(package_root, path);
        if !full_path.exists() {
            issues.push(format!("skills path '{path}' does not exist."));
            continue;
        }
        skill_count += if full_path.join("SKILL.md").is_file() {
            1
        } else {
            fs::read_dir(&full_path)
                .map(|entries| {
                    entries
                        .flatten()
                        .filter(|entry| entry.path().join("SKILL.md").is_file())
                        .count()
                })
                .unwrap_or_default()
        };
    }
    for path in v3_string_paths(plugin.apps.as_ref())
        .into_iter()
        .chain(v3_string_paths(plugin.hooks.as_ref()))
    {
        if !resolve_package_path(package_root, path).exists() {
            issues.push(format!("declared Codex component '{path}' does not exist."));
        }
    }
    for path in v3_asset_paths(plugin.assets.as_ref()) {
        if path_is_uri(path) {
            continue;
        }
        if !is_safe_package_relative_path(path) {
            issues.push(format!(
                "declared Codex asset '{path}' is not package-relative."
            ));
        } else if !resolve_package_path(package_root, path).exists() {
            issues.push(format!("declared Codex asset '{path}' does not exist."));
        }
    }
    for path in plugin
        .elegy
        .package_assets
        .iter()
        .map(String::as_str)
        .chain(v3_interface_asset_paths(plugin.interface.as_ref()))
    {
        if !is_safe_package_relative_path(path) {
            issues.push(format!(
                "declared package asset '{path}' is not package-relative."
            ));
        } else if !resolve_package_path(package_root, path).exists() {
            issues.push(format!("declared package asset '{path}' does not exist."));
        }
    }
    if plugin.mcp_servers.as_ref().is_some_and(Value::is_string) {
        for path in v3_string_paths(plugin.mcp_servers.as_ref()) {
            let descriptor_path = resolve_package_path(package_root, path);
            if !descriptor_path.exists() {
                issues.push(format!("MCP server descriptor '{path}' does not exist."));
                continue;
            }
            let descriptor = fs::read_to_string(&descriptor_path)
                .ok()
                .and_then(|raw| serde_json::from_str::<Value>(&raw).ok());
            let Some(servers) = descriptor
                .as_ref()
                .and_then(|value| value.get("mcpServers"))
                .and_then(Value::as_object)
            else {
                issues.push(format!(
                    "MCP server descriptor '{path}' has no mcpServers object."
                ));
                continue;
            };
            for (server_name, server) in servers {
                let Some(expectation) = plugin.elegy.mcp_authentication.get(server_name) else {
                    issues.push(format!(
                        "MCP server '{server_name}' requires an explicit authentication expectation."
                    ));
                    continue;
                };
                let remote = server.as_object().is_some_and(|server| {
                    server.contains_key("url")
                        || server.contains_key("httpUrl")
                        || server.contains_key("http_url")
                });
                if remote && expectation.mode == ElegyMcpAuthenticationMode::None {
                    issues.push(format!(
                        "remote MCP server '{server_name}' cannot declare unauthenticated mode."
                    ));
                }
                if expectation.mode == ElegyMcpAuthenticationMode::BearerEnv
                    && expectation
                        .environment_variable
                        .as_deref()
                        .is_none_or(|name| !is_environment_variable_name(name))
                {
                    issues.push(format!(
                        "MCP server '{server_name}' uses bearer-env but elegy.mcpAuthentication declares no valid environmentVariable."
                    ));
                }
            }
            for declared in plugin.elegy.mcp_authentication.keys() {
                if !servers.contains_key(declared) {
                    issues.push(format!(
                        "elegy.mcpAuthentication declares unknown MCP server '{declared}'."
                    ));
                }
            }
        }
    }

    let mcp_server_count = match plugin.mcp_servers.as_ref() {
        Some(Value::Object(servers)) => servers.len(),
        Some(_) => v3_string_paths(plugin.mcp_servers.as_ref())
            .into_iter()
            .filter_map(|path| {
                fs::read_to_string(resolve_package_path(package_root, path))
                    .ok()
                    .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
                    .and_then(|value| {
                        value
                            .get("mcpServers")
                            .and_then(Value::as_object)
                            .map(serde_json::Map::len)
                    })
            })
            .sum(),
        None => 0,
    };
    let valid = issues.is_empty();
    Ok(PluginV1VerifyResult {
        valid,
        plugin_name: plugin.name,
        plugin_version: plugin.version,
        has_skills: plugin.skills.is_some(),
        skill_count,
        has_mcp: plugin.mcp_servers.is_some(),
        mcp_server_count,
        has_apps: plugin.apps.is_some(),
        app_count: usize::from(plugin.apps.is_some()),
        has_hooks: plugin.hooks.is_some(),
        hook_event_count: usize::from(plugin.hooks.is_some()),
        has_codex_interface: plugin.interface.is_some(),
        has_codex_mcp_servers: plugin.mcp_servers.is_some(),
        has_capability_catalog: plugin.elegy.capability_catalog.is_some(),
        catalog_app_binding_count: 0,
        issues,
    })
}

fn v3_string_paths(value: Option<&Value>) -> Vec<&str> {
    match value {
        Some(Value::String(path)) => vec![path],
        Some(Value::Array(values)) => values.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

fn v3_asset_paths(value: Option<&Value>) -> Vec<&str> {
    fn collect<'a>(value: &'a Value, paths: &mut Vec<&'a str>) {
        match value {
            Value::String(path) => paths.push(path),
            Value::Array(values) => {
                for value in values {
                    collect(value, paths);
                }
            }
            Value::Object(values) => {
                for value in values.values() {
                    collect(value, paths);
                }
            }
            _ => {}
        }
    }

    let mut paths = Vec::new();
    if let Some(value) = value {
        collect(value, &mut paths);
    }
    paths
}

/// Verify a v1-format plugin manifest.
///
/// Loads `.elegy-plugin/plugin.json`, validates it structurally,
/// and checks that referenced component directories exist and contain
/// well-formed entries.
pub fn verify_plugin_v1(package_dir: &Path) -> Result<PluginV1VerifyResult, ToolingError> {
    let plugin_path = package_dir.join("plugin.json");

    // Load the plugin manifest
    let raw = fs::read_to_string(&plugin_path).map_err(|e| ToolingError::Io {
        operation: "read",
        path: plugin_path.clone(),
        source: e,
    })?;

    let plugin: ElegyPluginV1 = serde_json::from_str(&raw).map_err(|e| ToolingError::Json {
        path: plugin_path.clone(),
        source: e,
    })?;

    // Component paths are package-relative (relative to repo root,
    // which is the parent of .elegy-plugin/).
    let package_root = package_dir.parent().unwrap_or(Path::new("."));

    let validation = validate_elegy_plugin_v1(&plugin);
    let manifest_valid = validation.is_valid();
    let mut issues = validation.issues.clone();

    if let Some(readiness_ref) = &plugin.readiness {
        let readiness_path = resolve_package_path(package_root, &readiness_ref.path);
        match fs::read_to_string(&readiness_path) {
            Ok(raw) => match serde_json::from_str::<ElegyReadinessV1>(&raw) {
                Ok(readiness) => {
                    for issue in readiness.validation_issues() {
                        issues.push(format!("readiness: {issue}"));
                    }
                    if readiness.surface != plugin.name {
                        issues.push(format!(
                            "readiness surface '{}' does not match manifest plugin '{}'.",
                            readiness.surface, plugin.name
                        ));
                    }
                    if readiness.surface_version != plugin.version {
                        issues.push(format!(
                            "readiness surfaceVersion '{}' does not match manifest version '{}'.",
                            readiness.surface_version, plugin.version
                        ));
                    }
                    if readiness.stage != readiness_ref.stage {
                        issues.push(format!(
                            "readiness stage '{:?}' does not match manifest stage '{:?}'.",
                            readiness.stage, readiness_ref.stage
                        ));
                    }
                    if readiness.schema_version != readiness_ref.schema_version {
                        issues.push(
                            "readiness schemaVersion does not match the manifest declaration."
                                .to_string(),
                        );
                    }
                    for evidence in &readiness.evidence {
                        let evidence_path = resolve_package_path(package_root, &evidence.path);
                        if !evidence_path.is_file() {
                            issues.push(format!(
                                "readiness evidence file '{}' does not exist.",
                                evidence_path.display()
                            ));
                        }
                    }
                }
                Err(error) => issues.push(format!(
                    "readiness file '{}' is invalid: {error}",
                    readiness_path.display()
                )),
            },
            Err(_) => issues.push(format!(
                "readiness file '{}' does not exist.",
                readiness_path.display()
            )),
        }
    }

    if let Some(connections) = &plugin.connections {
        if connections.requirements.mode == "declared" {
            if let Some(path) = &connections.requirements.path {
                let requirements_path = resolve_package_path(package_root, path);
                match load_connection_requirements_v1(&requirements_path) {
                    Ok(declared) => {
                        for issue in validate_elegy_plugin_connections_v1(&declared) {
                            issues.push(format!("connections.requirements: {issue}"));
                        }
                        if declared.plugin != plugin.name {
                            issues.push(format!(
                                "connections requirements plugin '{}' does not match manifest plugin '{}'.",
                                declared.plugin, plugin.name
                            ));
                        }
                        if declared.plugin_version != plugin.version {
                            issues.push(format!(
                                "connections requirements pluginVersion '{}' does not match manifest version '{}'.",
                                declared.plugin_version, plugin.version
                            ));
                        }
                        if connections.requirements.schema_version.as_deref()
                            != Some(declared.schema_version.as_str())
                        {
                            issues.push(
                                "connections requirements schemaVersion does not match the manifest declaration."
                                    .to_string(),
                            );
                        }
                    }
                    Err(_error) if !requirements_path.is_file() => issues.push(format!(
                        "declared connection requirements file '{}' does not exist.",
                        requirements_path.display()
                    )),
                    Err(error) => issues.push(format!(
                        "declared connection requirements file '{}' is invalid: {error}",
                        requirements_path.display()
                    )),
                }
            }
        }
        if let Some(provider) = &connections.provider {
            let provider_path = resolve_package_path(package_root, &provider.path);
            match load_connection_provider_v1(&provider_path) {
                Ok(descriptor) => {
                    for issue in validate_elegy_connection_provider_v1(&descriptor) {
                        issues.push(format!("connection provider: {issue}"));
                    }
                    if provider.schema_version != descriptor.schema_version {
                        issues.push(
                            "connection provider schemaVersion does not match the manifest declaration."
                                .to_string(),
                        );
                    }
                }
                Err(_error) if !provider_path.is_file() => issues.push(format!(
                    "declared connection provider file '{}' does not exist.",
                    provider_path.display()
                )),
                Err(error) => issues.push(format!(
                    "declared connection provider file '{}' is invalid: {error}",
                    provider_path.display()
                )),
            }
        }
    }

    // Check skills directory
    let (has_skills, skill_count) = if let Some(ref skills_path) = plugin.skills {
        let skills_dir = if let Some(stripped) = skills_path.strip_prefix("./") {
            package_root.join(stripped)
        } else {
            package_root.join(skills_path)
        };
        if skills_dir.exists() && skills_dir.is_dir() {
            let mut count = 0;
            let direct_skill_md = skills_dir.join("SKILL.md");
            if direct_skill_md.is_file() {
                count += 1;
                let skill_name = skills_dir
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("<root>");
                match fs::read_to_string(&direct_skill_md) {
                    Ok(content) => match parse_agent_skill_frontmatter(&content) {
                        Ok((frontmatter, _)) => {
                            for issue in validate_agent_skill_frontmatter(&frontmatter) {
                                issues.push(format!("skills.{skill_name}: {issue}"));
                            }
                        }
                        Err(issue) => {
                            issues.push(format!("skills.{skill_name}: {issue}"));
                        }
                    },
                    Err(error) => {
                        issues.push(format!(
                            "skills.{skill_name}: unable to read SKILL.md: {error}."
                        ));
                    }
                }
            } else if let Ok(entries) = fs::read_dir(&skills_dir) {
                for entry in entries.flatten() {
                    let skill_dir = entry.path();
                    if skill_dir.is_dir() {
                        let skill_md = skill_dir.join("SKILL.md");
                        let skill_name = skill_dir
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("<invalid>");
                        if !skill_md.is_file() {
                            issues.push(format!("skills.{skill_name}: missing required SKILL.md."));
                            continue;
                        }
                        count += 1;
                        match fs::read_to_string(&skill_md) {
                            Ok(content) => match parse_agent_skill_frontmatter(&content) {
                                Ok((frontmatter, _)) => {
                                    for issue in validate_agent_skill_frontmatter(&frontmatter) {
                                        issues.push(format!("skills.{skill_name}: {issue}"));
                                    }
                                }
                                Err(issue) => {
                                    issues.push(format!("skills.{skill_name}: {issue}"));
                                }
                            },
                            Err(error) => {
                                issues.push(format!(
                                    "skills.{skill_name}: unable to read SKILL.md: {error}."
                                ));
                            }
                        }
                    }
                }
            }
            (true, count)
        } else {
            issues.push(format!(
                "skills directory '{}' does not exist.",
                skills_path
            ));
            (false, 0)
        }
    } else {
        (false, 0)
    };

    // Check MCP servers directory
    let (has_mcp, mcp_server_count) = if let Some(ref mcp_path) = plugin.mcp_servers {
        let mcp_dir = if let Some(stripped) = mcp_path.strip_prefix("./") {
            package_root.join(stripped)
        } else {
            package_root.join(mcp_path)
        };
        if mcp_dir.exists() && mcp_dir.is_dir() {
            let mut count = 0;
            if let Ok(entries) = fs::read_dir(&mcp_dir) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.extension().is_some_and(|e| e == "json") {
                        count += 1;
                        let label = entry_path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("<invalid>");
                        match fs::read_to_string(&entry_path)
                            .ok()
                            .and_then(|raw| serde_json::from_str::<McpServerDescriptor>(&raw).ok())
                        {
                            Some(descriptor) => {
                                for issue in validate_mcp_server_descriptor(&descriptor).issues {
                                    issues.push(format!("mcpServers.{label}: {issue}"));
                                }
                            }
                            None => issues.push(format!(
                                "mcpServers.{label}: expected a valid MCP server descriptor."
                            )),
                        }
                    }
                }
            }
            if count == 0 {
                issues.push(format!(
                    "mcpServers directory '{}' contains no JSON descriptors.",
                    mcp_path
                ));
            }
            (true, count)
        } else {
            issues.push(format!(
                "mcpServers directory '{}' does not exist.",
                mcp_path
            ));
            (false, 0)
        }
    } else {
        (false, 0)
    };

    let codex_ext = extract_codex_extension_v1(&plugin.extensions);
    if let (Some(extension), Some(requirements)) = (
        codex_ext.as_ref(),
        plugin
            .connections
            .as_ref()
            .map(|connections| &connections.requirements)
            .filter(|requirements| requirements.mode == "declared"),
    ) {
        if let Some(path) = &requirements.path {
            let requirements_path = resolve_package_path(package_root, path);
            if let Ok(declared) = load_connection_requirements_v1(&requirements_path) {
                let empty_bindings = BTreeMap::new();
                let bindings = extension
                    .connection_bindings
                    .as_ref()
                    .unwrap_or(&empty_bindings);
                if let Err(binding_issues) = build_codex_apps_from_connections(&declared, bindings)
                {
                    for issue in binding_issues {
                        issues.push(format!(
                            "extensions.codex.plugin/v1.connectionBindings: {issue}"
                        ));
                    }
                }
            }
        }
    }
    let (has_apps, app_count) =
        if let Some(apps_path) = codex_ext.as_ref().and_then(|ext| ext.apps.as_ref()) {
            let apps_file_path = resolve_package_path(package_root, apps_path);
            match load_codex_apps_file(&apps_file_path) {
                Ok(apps_file) => {
                    for issue in validate_codex_apps_file(&apps_file) {
                        issues.push(format!("apps file '{}': {issue}", apps_path));
                    }
                    (true, apps_file.apps.len())
                }
                Err(err) => {
                    issues.push(format!("apps file '{}' is invalid: {err}", apps_path));
                    (false, 0)
                }
            }
        } else {
            (false, 0)
        };

    let (has_hooks, hook_event_count) =
        if let Some(hooks_path) = codex_ext.as_ref().and_then(|ext| ext.hooks.as_ref()) {
            let hooks_file_path = resolve_package_path(package_root, hooks_path);
            match load_codex_hooks_config(&hooks_file_path) {
                Ok(hooks_config) => {
                    for issue in validate_codex_hooks_config(&hooks_config) {
                        issues.push(format!("hooks file '{}': {issue}", hooks_path));
                    }
                    (true, hooks_config.hooks.len())
                }
                Err(err) => {
                    issues.push(format!("hooks file '{}' is invalid: {err}", hooks_path));
                    (false, 0)
                }
            }
        } else {
            let default_hooks_path = package_root.join("hooks").join("hooks.json");
            if default_hooks_path.exists() {
                match load_codex_hooks_config(&default_hooks_path) {
                    Ok(hooks_config) => {
                        for issue in validate_codex_hooks_config(&hooks_config) {
                            issues.push(format!("hooks/hooks.json: {issue}"));
                        }
                        (true, hooks_config.hooks.len())
                    }
                    Err(err) => {
                        issues.push(format!("hooks/hooks.json is invalid: {err}"));
                        (false, 0)
                    }
                }
            } else {
                (false, 0)
            }
        };

    let has_codex_interface = codex_ext
        .as_ref()
        .and_then(|ext| ext.interface.as_ref())
        .is_some();
    let has_codex_mcp_servers = codex_ext
        .as_ref()
        .and_then(|ext| ext.mcp_servers.as_ref())
        .is_some();
    if let Some(mcp_path) = codex_ext.as_ref().and_then(|ext| ext.mcp_servers.as_ref()) {
        let path = resolve_package_path(package_root, mcp_path);
        for issue in validate_codex_mcp_config_file(&path) {
            issues.push(format!("extensions.codex.plugin/v1.mcpServers: {issue}"));
        }
    }
    if let Some(ext) = &codex_ext {
        for asset in ext.assets.iter().flatten() {
            if !resolve_package_path(package_root, asset).exists() {
                issues.push(format!(
                    "extensions.codex.plugin/v1.assets path '{asset}' does not exist."
                ));
            }
        }
        if let Some(interface) = &ext.interface {
            for (field, value) in [
                ("composerIcon", &interface.composer_icon),
                ("logo", &interface.logo),
                ("logoDark", &interface.logo_dark),
            ] {
                if let Some(value) = value {
                    if !path_is_uri(value) && !resolve_package_path(package_root, value).is_file() {
                        issues.push(format!(
                            "extensions.codex.plugin/v1.interface.{field} path '{value}' does not exist."
                        ));
                    }
                }
            }
            for screenshot in interface.screenshots.iter().flatten() {
                if !path_is_uri(screenshot)
                    && !resolve_package_path(package_root, screenshot).is_file()
                {
                    issues.push(format!(
                        "extensions.codex.plugin/v1.interface.screenshots path '{screenshot}' does not exist."
                    ));
                }
            }
        }
    }

    // Validate capability catalog if present
    let (has_capability_catalog, catalog_app_binding_count) =
        if let Some(cat_config) = &plugin.capability_catalog {
            let catalog_path = resolve_package_path(package_root, &cat_config.path);
            if catalog_path.exists() {
                match load_capability_catalog(&catalog_path) {
                    Ok(ElegyCapabilityCatalog::V1(catalog)) => {
                        let catalog_validation = validate_elegy_capability_catalog_v1(&catalog);
                        for issue in &catalog_validation.issues {
                            issues.push(format!("capabilityCatalog: {issue}"));
                        }
                        let app_binding_count = catalog
                            .capabilities
                            .iter()
                            .filter(|c| c.kind == ElegyCapabilityKind::AppBinding)
                            .count();
                        (true, app_binding_count)
                    }
                    Ok(ElegyCapabilityCatalog::V2(catalog)) => {
                        let catalog_validation = validate_elegy_capability_catalog_v2(&catalog);
                        for issue in &catalog_validation.issues {
                            issues.push(format!("capabilityCatalog: {issue}"));
                        }
                        (true, 0)
                    }
                    Err(err) => {
                        issues.push(format!("capabilityCatalog: invalid catalog file: {err}"));
                        (false, 0)
                    }
                }
            } else {
                issues.push(format!(
                    "capabilityCatalog path '{}' does not exist.",
                    cat_config.path
                ));
                (false, 0)
            }
        } else {
            (false, 0)
        };

    Ok(PluginV1VerifyResult {
        valid: manifest_valid && issues.is_empty(),
        plugin_name: plugin.name,
        plugin_version: plugin.version,
        has_skills,
        skill_count,
        has_mcp,
        mcp_server_count,
        has_apps,
        app_count,
        has_hooks,
        hook_event_count,
        has_codex_interface,
        has_codex_mcp_servers,
        has_capability_catalog,
        catalog_app_binding_count,
        issues,
    })
}

/// Inspect a v1-format plugin and return a JSON summary.
pub fn inspect_plugin_v1(package_dir: &Path) -> Result<serde_json::Value, ToolingError> {
    let plugin_path = package_dir.join("plugin.json");
    let raw = fs::read_to_string(&plugin_path).map_err(|e| ToolingError::Io {
        operation: "read",
        path: plugin_path.clone(),
        source: e,
    })?;
    let plugin: ElegyPluginV1 = serde_json::from_str(&raw).map_err(|e| ToolingError::Json {
        path: plugin_path,
        source: e,
    })?;
    let codex_ext = extract_codex_extension_v1(&plugin.extensions);
    let connection_mode = plugin
        .connections
        .as_ref()
        .map(|connections| connections.requirements.mode.as_str())
        .unwrap_or("legacy-unknown");
    let connection_requirements = plugin
        .connections
        .as_ref()
        .filter(|connections| connections.requirements.mode == "declared")
        .and_then(|connections| connections.requirements.path.as_ref())
        .and_then(|path| {
            let requirements_path = package_root_from_package_dir(package_dir)
                .join(normalize_package_relative_path(path));
            load_connection_requirements_v1(&requirements_path).ok()
        });
    let connection_requirement_count = connection_requirements
        .as_ref()
        .map_or(0, |connections| connections.requirements.len());
    let required_connection_count = connection_requirements.as_ref().map_or(0, |connections| {
        connections
            .requirements
            .iter()
            .filter(|requirement| requirement.required)
            .count()
    });

    // Load capability catalog if present
    let (has_capability_catalog, catalog_capability_count, catalog_app_binding_count) = plugin
        .capability_catalog
        .as_ref()
        .and_then(|cat_config| {
            let catalog_path = package_root_from_package_dir(package_dir)
                .join(normalize_package_relative_path(&cat_config.path));
            load_capability_catalog(&catalog_path).ok()
        })
        .map(|catalog| match catalog {
            ElegyCapabilityCatalog::V1(catalog) => {
                let app_binding_count = catalog
                    .capabilities
                    .iter()
                    .filter(|c| c.kind == ElegyCapabilityKind::AppBinding)
                    .count();
                (true, catalog.capabilities.len(), app_binding_count)
            }
            ElegyCapabilityCatalog::V2(catalog) => (true, catalog.capabilities.len(), 0),
        })
        .unwrap_or((false, 0, 0));

    Ok(serde_json::json!({
        "schemaVersion": plugin.schema_version,
        "name": plugin.name,
        "version": plugin.version,
        "description": plugin.description,
        "author": plugin.author.map(|a| serde_json::json!({
            "name": a.name,
            "email": a.email,
            "url": a.url,
        })),
        "license": plugin.license,
        "repository": plugin.repository,
        "hasSkills": plugin.skills.is_some(),
        "hasMcpServers": plugin.mcp_servers.is_some(),
        "connectionMode": connection_mode,
        "connectionRequirementCount": connection_requirement_count,
        "requiredConnectionCount": required_connection_count,
        "providesConnectionAdapter": plugin.connections.as_ref().and_then(|connections| connections.provider.as_ref()).is_some(),
        "hasCapabilityCatalog": has_capability_catalog,
        "catalogCapabilityCount": catalog_capability_count,
        "catalogAppBindingCount": catalog_app_binding_count,
        "hasCodexApps": codex_ext.as_ref().and_then(|e| e.apps.as_ref()).is_some(),
        "hasCodexHooks": codex_ext.as_ref().and_then(|e| e.hooks.as_ref()).is_some(),
        "hasCodexInterface": codex_ext.as_ref().and_then(|e| e.interface.as_ref()).is_some(),
        "hasCodexMcpServers": codex_ext.as_ref().and_then(|e| e.mcp_servers.as_ref()).is_some(),
        "extensionKeys": plugin.extensions.as_ref().map(|e| e.keys().collect::<Vec<_>>()),
    }))
}

fn package_root_from_package_dir(package_dir: &Path) -> PathBuf {
    package_dir.parent().unwrap_or(Path::new(".")).to_path_buf()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CodexProjectionMode {
    #[default]
    Current,
    Experimental,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HostProjectionPolicy {
    #[default]
    Strict,
    AllowLossy,
}

/// Export a current plugin manifest. V3 packages use the loss-aware projection
/// contract; legacy v1/v2 packages remain readable for migration only.
pub fn export_plugin_with_policy(
    plugin_path: &Path,
    host: &str,
    output_dir: &Path,
    overwrite: bool,
    policy: HostProjectionPolicy,
) -> Result<GeneratedHostExport, ToolingError> {
    export_plugin_with_policy_and_binary(plugin_path, host, output_dir, overwrite, policy, None)
}

pub fn export_plugin_with_policy_and_binary(
    plugin_path: &Path,
    host: &str,
    output_dir: &Path,
    overwrite: bool,
    policy: HostProjectionPolicy,
    binary: Option<PluginArchiveBinary<'_>>,
) -> Result<GeneratedHostExport, ToolingError> {
    let (_, manifest_path) = resolve_plugin_root(plugin_path)?;
    let raw = fs::read_to_string(&manifest_path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: manifest_path.clone(),
        source,
    })?;
    let schema_version = serde_json::from_str::<Value>(&raw)
        .ok()
        .and_then(|value| {
            value
                .get("schemaVersion")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();
    if schema_version == ELEGY_PLUGIN_V3_SCHEMA_VERSION {
        return export_plugin_v3(plugin_path, host, output_dir, overwrite, policy, binary);
    }
    Err(ToolingError::UnsupportedHostProjection {
        host: host.to_string(),
        reason: "legacy elegy-plugin/v1 and v2 packages are readable for migration only; migrate to elegy-plugin/v3 before export".to_string(),
    })
}

fn export_plugin_v3(
    plugin_path: &Path,
    host: &str,
    output_dir: &Path,
    overwrite: bool,
    policy: HostProjectionPolicy,
    binary: Option<PluginArchiveBinary<'_>>,
) -> Result<GeneratedHostExport, ToolingError> {
    let (package_root, manifest_path) = resolve_plugin_root(plugin_path)?;
    let raw = fs::read_to_string(&manifest_path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: manifest_path.clone(),
        source,
    })?;
    let plugin: ElegyPluginV3 =
        serde_json::from_str(&raw).map_err(|source| ToolingError::Json {
            path: manifest_path.clone(),
            source,
        })?;
    let validation = validate_elegy_plugin_v3(&plugin);
    if !validation.is_valid() {
        return Err(ToolingError::InvalidPluginPackage {
            path: manifest_path,
            issues: validation.issues,
        });
    }
    let package_dir = manifest_path.parent().unwrap_or(Path::new("."));
    let verification = verify_plugin_v3(package_dir)?;
    if !verification.valid {
        return Err(ToolingError::InvalidPluginPackage {
            path: manifest_path.clone(),
            issues: verification.issues,
        });
    }
    if !matches!(host, "codex" | "claude" | "opencode") {
        return Err(ToolingError::UnsupportedHostTarget {
            host: host.to_string(),
        });
    }

    let losses = v3_projection_losses(&plugin, host);
    if !losses.is_empty() && policy == HostProjectionPolicy::Strict {
        return Err(ToolingError::UnsupportedHostProjection {
            host: host.to_string(),
            reason: losses.join("; "),
        });
    }

    fs::create_dir_all(output_dir).map_err(|source| ToolingError::Io {
        operation: "create directory",
        path: output_dir.to_path_buf(),
        source,
    })?;
    let mut written_files = Vec::new();
    let portable_manifest_path = output_dir.join(".elegy-plugin").join("plugin.json");
    let portable_value = serde_json::to_value(&plugin).map_err(|source| ToolingError::Json {
        path: portable_manifest_path.clone(),
        source,
    })?;
    write_json_file(&portable_manifest_path, &portable_value, overwrite)?;
    written_files.push(display_path(&portable_manifest_path));

    copy_v3_value_paths(
        &package_root,
        output_dir,
        plugin.skills.as_ref(),
        overwrite,
        &mut written_files,
    )?;
    copy_v3_value_paths(
        &package_root,
        output_dir,
        plugin.apps.as_ref(),
        overwrite,
        &mut written_files,
    )?;
    copy_v3_value_paths(
        &package_root,
        output_dir,
        plugin.hooks.as_ref(),
        overwrite,
        &mut written_files,
    )?;
    for path in v3_asset_paths(plugin.assets.as_ref()) {
        if !path_is_uri(path) {
            copy_v3_package_path(
                &package_root,
                output_dir,
                path,
                overwrite,
                &mut written_files,
            )?;
        }
    }
    if plugin.mcp_servers.as_ref().is_some_and(Value::is_string) {
        copy_v3_value_paths(
            &package_root,
            output_dir,
            plugin.mcp_servers.as_ref(),
            overwrite,
            &mut written_files,
        )?;
    }
    for path in v3_interface_asset_paths(plugin.interface.as_ref()) {
        copy_v3_package_path(
            &package_root,
            output_dir,
            path,
            overwrite,
            &mut written_files,
        )?;
    }
    for path in &plugin.elegy.package_assets {
        copy_v3_package_path(
            &package_root,
            output_dir,
            path,
            overwrite,
            &mut written_files,
        )?;
    }
    if let Some(catalog) = &plugin.elegy.capability_catalog {
        copy_v3_package_path(
            &package_root,
            output_dir,
            &catalog.path,
            overwrite,
            &mut written_files,
        )?;
    }
    copy_v3_package_path(
        &package_root,
        output_dir,
        &plugin.elegy.readiness.path,
        overwrite,
        &mut written_files,
    )?;
    if let Some(path) = &plugin.elegy.connections.requirements.path {
        copy_v3_package_path(
            &package_root,
            output_dir,
            path,
            overwrite,
            &mut written_files,
        )?;
    }
    if let Some(provider) = &plugin.elegy.connections.provider {
        copy_v3_package_path(
            &package_root,
            output_dir,
            &provider.path,
            overwrite,
            &mut written_files,
        )?;
    }

    let plugin_manifest = match host {
        "codex" => {
            let path = output_dir.join(".codex-plugin").join("plugin.json");
            let projected = project_codex_plugin_v3(&plugin)?;
            write_json_file(&path, &projected, overwrite)?;
            written_files.push(display_path(&path));
            ".codex-plugin/plugin.json".to_string()
        }
        "claude" => {
            let path = output_dir.join(".claude-plugin").join("plugin.json");
            let projected = serde_json::json!({
                "name": plugin.name,
                "version": plugin.version,
                "description": plugin.description,
                "skills": plugin.skills,
            });
            write_json_file(&path, &projected, overwrite)?;
            written_files.push(display_path(&path));
            ".claude-plugin/plugin.json".to_string()
        }
        "opencode" => String::new(),
        _ => unreachable!("host was validated"),
    };

    if !losses.is_empty() {
        let report_path = output_dir.join("projection-report.json");
        write_json_file(
            &report_path,
            &serde_json::json!({
                "schemaVersion": "elegy-projection-report/v1",
                "sourcePlugin": plugin.name,
                "targetHost": host,
                "lossless": false,
                "routable": false,
                "losses": losses,
            }),
            overwrite,
        )?;
        written_files.push(display_path(&report_path));
    }
    if let Some(binary) = binary {
        if !is_safe_archive_path(&binary.archive_path) || !binary.source_path.is_file() {
            return Err(ToolingError::InvalidPluginPackage {
                path: binary.source_path.to_path_buf(),
                issues: vec!["binary source or destination path is invalid.".to_string()],
            });
        }
        let destination = output_dir.join(normalize_package_relative_path(&binary.archive_path));
        copy_file_component(binary.source_path, &destination, overwrite)?;
        written_files.push(display_path(&destination));
    }

    let lossless = losses.is_empty();
    let routable = lossless && plugin.elegy.readiness.stage.is_agent_routable();
    let skills_count = count_v3_skill_paths(plugin.skills.as_ref());
    let apps_emitted = plugin.apps.is_some() && host == "codex";
    let mcp_servers_emitted = plugin.mcp_servers.is_some() && host == "codex";
    let hooks_emitted = plugin.hooks.is_some() && host == "codex";
    Ok(GeneratedHostExport {
        source_package: format!("{}-v{}", plugin.name, plugin.version),
        plugin_name: plugin.name,
        plugin_version: plugin.version,
        lossless,
        routable,
        losses,
        emitted_components: GeneratedHostExportComponents {
            plugin_manifest,
            skills_dir: "skills".to_string(),
            skills_count,
            apps_emitted,
            mcp_servers_emitted,
            hooks_emitted,
        },
        written_files,
    })
}

fn v3_projection_losses(plugin: &ElegyPluginV3, host: &str) -> Vec<String> {
    if host == "codex" {
        return Vec::new();
    }
    let mut losses = Vec::new();
    if plugin.apps.is_some() {
        losses.push(format!("{host} projection cannot represent Codex apps"));
    }
    if plugin.hooks.is_some() {
        losses.push(format!("{host} projection cannot represent Codex hooks"));
    }
    if plugin.interface.is_some() {
        losses.push(format!(
            "{host} projection cannot represent Codex interface metadata"
        ));
    }
    if plugin.assets.is_some() {
        losses.push(format!(
            "{host} projection cannot represent Codex package assets"
        ));
    }
    if plugin.mcp_servers.is_some() {
        losses.push(format!(
            "{host} projection cannot represent the declared MCP server shape"
        ));
    }
    if plugin
        .elegy
        .mcp_authentication
        .values()
        .any(|expectation| expectation.mode != ElegyMcpAuthenticationMode::None)
    {
        losses.push(format!(
            "{host} projection cannot represent delegated MCP authentication"
        ));
    }
    if !plugin.extra.is_empty() {
        losses.push(format!(
            "{host} projection cannot represent unknown Codex-native fields"
        ));
    }
    if plugin.elegy.connections.requirements.mode != "none"
        || plugin.elegy.connections.provider.is_some()
    {
        losses.push(format!(
            "{host} projection cannot represent Elegy connection bindings"
        ));
    }
    losses
}

fn copy_v3_value_paths(
    package_root: &Path,
    output_dir: &Path,
    value: Option<&Value>,
    overwrite: bool,
    written_files: &mut Vec<String>,
) -> Result<(), ToolingError> {
    match value {
        Some(Value::String(path)) => {
            copy_v3_package_path(package_root, output_dir, path, overwrite, written_files)
        }
        Some(Value::Array(values)) => {
            for value in values {
                if let Value::String(path) = value {
                    copy_v3_package_path(package_root, output_dir, path, overwrite, written_files)?;
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn copy_v3_package_path(
    package_root: &Path,
    output_dir: &Path,
    relative_path: &str,
    overwrite: bool,
    written_files: &mut Vec<String>,
) -> Result<(), ToolingError> {
    if !is_safe_package_relative_path(relative_path) {
        return Err(ToolingError::InvalidPluginPackage {
            path: package_root.join(relative_path),
            issues: vec![format!(
                "declared component path '{relative_path}' is not package-relative"
            )],
        });
    }
    let source = resolve_package_path(package_root, relative_path);
    if !source.exists() {
        return Err(ToolingError::InvalidPluginPackage {
            path: source,
            issues: vec![format!(
                "declared component '{relative_path}' does not exist"
            )],
        });
    }
    let destination = output_dir.join(normalize_package_relative_path(relative_path));
    if source.is_dir() {
        if destination.exists() && !overwrite {
            return Err(ToolingError::OutputExists { path: destination });
        }
        copy_dir_all(&source, &destination)?;
        for path in walk_dir_files(&destination)? {
            written_files.push(display_path(&path));
        }
    } else {
        copy_file_component(&source, &destination, overwrite)?;
        written_files.push(display_path(&destination));
    }
    Ok(())
}

fn v3_interface_asset_paths(interface: Option<&CodexPluginInterface>) -> Vec<&str> {
    let Some(interface) = interface else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    for path in [
        interface.composer_icon.as_deref(),
        interface.logo.as_deref(),
        interface.logo_dark.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if !path_is_uri(path) {
            paths.push(path);
        }
    }
    if let Some(screenshots) = &interface.screenshots {
        paths.extend(
            screenshots
                .iter()
                .map(String::as_str)
                .filter(|path| !path_is_uri(path)),
        );
    }
    paths
}

fn count_v3_skill_paths(value: Option<&Value>) -> usize {
    match value {
        Some(Value::String(_)) => 1,
        Some(Value::Array(values)) => values.iter().filter(|value| value.is_string()).count(),
        _ => 0,
    }
}

/// Export v1 plugin skills for a host target.
///
/// Accepts any of the three path forms supported by `resolve_plugin_root`.
/// Copies the ENTIRE skill directory contents (not just SKILL.md).
pub fn export_plugin_v1(
    plugin_path: &Path,
    host: &str, // "codex", "opencode", "claude"
    output_dir: &Path,
    overwrite: bool,
) -> Result<GeneratedHostExport, ToolingError> {
    export_plugin_v1_with_codex_mode(
        plugin_path,
        host,
        output_dir,
        overwrite,
        CodexProjectionMode::Current,
    )
}

pub fn export_plugin_v1_with_codex_mode(
    plugin_path: &Path,
    host: &str,
    output_dir: &Path,
    overwrite: bool,
    codex_mode: CodexProjectionMode,
) -> Result<GeneratedHostExport, ToolingError> {
    export_plugin_v1_with_codex_mode_and_binary(
        plugin_path,
        host,
        output_dir,
        overwrite,
        codex_mode,
        None,
    )
}

pub fn export_plugin_v1_with_codex_mode_and_binary(
    plugin_path: &Path,
    host: &str,
    output_dir: &Path,
    overwrite: bool,
    codex_mode: CodexProjectionMode,
    binary: Option<PluginArchiveBinary<'_>>,
) -> Result<GeneratedHostExport, ToolingError> {
    let (package_root, manifest_path) = resolve_plugin_root(plugin_path)?;
    let verification = verify_plugin_v1(&package_root.join(".elegy-plugin"))?;
    if !verification.valid {
        return Err(ToolingError::InvalidPluginPackage {
            path: manifest_path.clone(),
            issues: verification.issues,
        });
    }

    let raw = fs::read_to_string(&manifest_path).map_err(|e| ToolingError::Io {
        operation: "read",
        path: manifest_path.clone(),
        source: e,
    })?;
    let plugin: ElegyPluginV1 = serde_json::from_str(&raw).map_err(|e| ToolingError::Json {
        path: manifest_path.clone(),
        source: e,
    })?;

    let codex_ext = extract_codex_extension_v1(&plugin.extensions);
    if host == "codex" && codex_mode == CodexProjectionMode::Current {
        let issues = validate_current_codex_projection(&plugin, codex_ext.as_ref());
        if !issues.is_empty() {
            return Err(ToolingError::InvalidPluginPackage {
                path: manifest_path,
                issues,
            });
        }
    }

    // Load capability catalog if present
    let catalog = plugin.capability_catalog.as_ref().and_then(|cat_config| {
        let catalog_path = resolve_package_path(&package_root, &cat_config.path);
        load_capability_catalog(&catalog_path).ok()
    });

    // Connection declarations are authoritative for v2. Catalog-derived apps
    // remain a legacy v1 compatibility path.
    let connection_apps =
        if host == "codex" {
            if let Some(requirements) = plugin
                .connections
                .as_ref()
                .map(|connections| &connections.requirements)
                .filter(|requirements| requirements.mode == "declared")
            {
                let path = requirements.path.as_deref().ok_or_else(|| {
                    ToolingError::InvalidPluginPackage {
                        path: manifest_path.clone(),
                        issues: vec![
                            "declared connection requirements are missing their path.".to_string()
                        ],
                    }
                })?;
                let declared =
                    load_connection_requirements_v1(&resolve_package_path(&package_root, path))?;
                let empty_bindings = BTreeMap::new();
                let bindings = codex_ext
                    .as_ref()
                    .and_then(|extension| extension.connection_bindings.as_ref())
                    .unwrap_or(&empty_bindings);
                Some(
                    build_codex_apps_from_connections(&declared, bindings).map_err(|issues| {
                        ToolingError::InvalidPluginPackage {
                            path: manifest_path.clone(),
                            issues,
                        }
                    })?,
                )
            } else {
                None
            }
        } else {
            None
        };
    let catalog_apps = if plugin.schema_version == ELEGY_PLUGIN_V1_SCHEMA_VERSION {
        catalog.as_ref().and_then(|catalog| match catalog {
            ElegyCapabilityCatalog::V1(catalog) => build_codex_apps_from_catalog(catalog),
            ElegyCapabilityCatalog::V2(_) => None,
        })
    } else {
        None
    };
    let derived_apps = connection_apps.or(catalog_apps);
    let codex_projection_digest = if host == "codex" {
        Some(compute_codex_projection_digest(
            &package_root,
            &raw,
            &plugin,
            codex_ext.as_ref(),
            binary.as_ref(),
        )?)
    } else {
        None
    };

    let mut written_files = Vec::new();
    let mut skills_count = 0usize;
    let mut mcp_servers_emitted = false;
    let mut apps_emitted = false;
    let mut hooks_emitted = false;

    // Determine host-specific output layout
    let (host_skills_dir, needs_codex_manifest, needs_claude_manifest) = match host {
        "codex" => (output_dir.join("skills"), true, false),
        "opencode" => (output_dir.join("skills"), false, false),
        "claude" => (output_dir.join("skills"), false, true),
        _ => {
            return Err(ToolingError::UnsupportedHostTarget {
                host: host.to_string(),
            });
        }
    };

    // Create output directory if needed
    fs::create_dir_all(&host_skills_dir).map_err(|e| ToolingError::Io {
        operation: "create directory",
        path: host_skills_dir.clone(),
        source: e,
    })?;

    // Preserve the host-neutral adapter authority in every host projection.
    // Host-only extensions are omitted so the portable manifest never points
    // at projection assets that another host does not receive.
    let portable_manifest_path = output_dir.join(".elegy-plugin").join("plugin.json");
    let mut portable_manifest =
        serde_json::to_value(&plugin).map_err(|source| ToolingError::Json {
            path: portable_manifest_path.clone(),
            source,
        })?;
    if let Some(object) = portable_manifest.as_object_mut() {
        object.remove("extensions");
    }
    write_json_file(&portable_manifest_path, &portable_manifest, overwrite)?;
    written_files.push(display_path(&portable_manifest_path));

    if let Some(catalog_ref) = &plugin.capability_catalog {
        let source = resolve_package_path(&package_root, &catalog_ref.path);
        let destination = output_dir.join(normalize_package_relative_path(&catalog_ref.path));
        copy_file_component(&source, &destination, overwrite)?;
        written_files.push(display_path(&destination));
    }
    if let Some(readiness_ref) = &plugin.readiness {
        let source = resolve_package_path(&package_root, &readiness_ref.path);
        let destination = output_dir.join(normalize_package_relative_path(&readiness_ref.path));
        copy_file_component(&source, &destination, overwrite)?;
        written_files.push(display_path(&destination));
        let readiness_raw = fs::read_to_string(&source).map_err(|error| ToolingError::Io {
            operation: "read",
            path: source.clone(),
            source: error,
        })?;
        let readiness: ElegyReadinessV1 =
            serde_json::from_str(&readiness_raw).map_err(|error| ToolingError::Json {
                path: source,
                source: error,
            })?;
        let mut copied_evidence = BTreeSet::new();
        for evidence in readiness.evidence {
            if !copied_evidence.insert(evidence.path.clone()) {
                continue;
            }
            let source = resolve_package_path(&package_root, &evidence.path);
            let destination = output_dir.join(normalize_package_relative_path(&evidence.path));
            copy_file_component(&source, &destination, overwrite)?;
            written_files.push(display_path(&destination));
        }
    }
    if let Some(connections) = &plugin.connections {
        if let Some(requirements_path) = &connections.requirements.path {
            let source = resolve_package_path(&package_root, requirements_path);
            let destination = output_dir.join(normalize_package_relative_path(requirements_path));
            copy_file_component(&source, &destination, overwrite)?;
            written_files.push(display_path(&destination));
        }
        if let Some(provider) = &connections.provider {
            let source = resolve_package_path(&package_root, &provider.path);
            let destination = output_dir.join(normalize_package_relative_path(&provider.path));
            copy_file_component(&source, &destination, overwrite)?;
            written_files.push(display_path(&destination));
        }
    }

    // Export skills — copy entire skill directories
    if let Some(ref skills_path) = plugin.skills {
        let skills_src = if let Some(stripped) = skills_path.strip_prefix("./") {
            package_root.join(stripped)
        } else {
            package_root.join(skills_path)
        };

        if skills_src.exists() && skills_src.is_dir() {
            if skills_src.join("SKILL.md").is_file() {
                let dest_dir = host_skills_dir.join(&plugin.name);
                if dest_dir.exists() && !overwrite {
                    return Err(ToolingError::OutputExists { path: dest_dir });
                }
                fs::create_dir_all(&dest_dir).map_err(|e| ToolingError::Io {
                    operation: "create directory",
                    path: dest_dir.clone(),
                    source: e,
                })?;
                if let Ok(entries) = fs::read_dir(&skills_src) {
                    for entry in entries.flatten() {
                        if matches!(
                            entry.file_name().to_str(),
                            Some(
                                ".elegy-plugin"
                                    | ".codex-plugin"
                                    | ".claude-plugin"
                                    | "install-receipt.json"
                            )
                        ) {
                            continue;
                        }
                        let source = entry.path();
                        let destination = dest_dir.join(entry.file_name());
                        if source.is_dir() {
                            copy_dir_all(&source, &destination)?;
                        } else if source.is_file() {
                            fs::copy(&source, &destination).map_err(|e| ToolingError::Io {
                                operation: "copy",
                                path: source,
                                source: e,
                            })?;
                        }
                    }
                }
                if host == "codex" {
                    let skill_path = dest_dir.join("SKILL.md");
                    let content =
                        fs::read_to_string(&skill_path).map_err(|e| ToolingError::Io {
                            operation: "read",
                            path: skill_path.clone(),
                            source: e,
                        })?;
                    let frontmatter_end = content.find("\n---").ok_or_else(|| {
                        ToolingError::InvalidPluginPackage {
                            path: skill_path.clone(),
                            issues: vec!["skill frontmatter is missing its closing fence".into()],
                        }
                    })?;
                    let (frontmatter, body) = content.split_at(frontmatter_end);
                    let normalized_frontmatter = frontmatter
                        .replacen(
                            "disable-model-invocation: true",
                            "disable-model-invocation: false",
                            1,
                        )
                        .replacen(
                            "disable_model_invocation: true",
                            "disable_model_invocation: false",
                            1,
                        );
                    if normalized_frontmatter != frontmatter {
                        fs::write(&skill_path, format!("{normalized_frontmatter}{body}")).map_err(
                            |e| ToolingError::Io {
                                operation: "write",
                                path: skill_path,
                                source: e,
                            },
                        )?;
                    }
                }
                if let Ok(walked) = walk_dir_files(&dest_dir) {
                    for f in walked {
                        written_files.push(display_path(&f));
                    }
                }
                skills_count += 1;
            } else if let Ok(entries) = fs::read_dir(&skills_src) {
                for entry in entries.flatten() {
                    let skill_dir = entry.path();
                    if !skill_dir.is_dir() {
                        continue;
                    }
                    let skill_name = skill_dir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");

                    let dest_dir = host_skills_dir.join(skill_name);

                    // Copy the entire skill directory
                    if dest_dir.exists() && !overwrite {
                        return Err(ToolingError::OutputExists { path: dest_dir });
                    }
                    copy_dir_all(&skill_dir, &dest_dir)?;

                    // Track written files
                    if let Ok(walked) = walk_dir_files(&dest_dir) {
                        for f in walked {
                            written_files.push(display_path(&f));
                        }
                    }
                    skills_count += 1;
                }
            }
        }
    }

    // Portable MCP descriptors remain beside the portable manifest for every
    // host. A host may use them directly or derive its own protocol config.
    if let Some(ref mcp_path) = plugin.mcp_servers {
        let mcp_src = resolve_package_path(&package_root, mcp_path);
        let mcp_dest = output_dir.join(normalize_package_relative_path(mcp_path));
        if mcp_src.exists() && mcp_src.is_dir() {
            if mcp_dest.exists() && !overwrite {
                return Err(ToolingError::OutputExists { path: mcp_dest });
            }
            copy_dir_all(&mcp_src, &mcp_dest)?;
            if let Ok(walked) = walk_dir_files(&mcp_dest) {
                for f in walked {
                    written_files.push(display_path(&f));
                }
            }
            mcp_servers_emitted = true;
        }
    }

    // Copy Codex-specific assets if present
    if host == "codex" {
        if let Some(ref ext) = codex_ext {
            // Catalog-driven .app.json generation takes priority over hand-authored file.
            // If the catalog has app-binding capabilities, generate .app.json from them.
            // Otherwise, fall back to copying the hand-authored file for backward compat.
            if let Some(ref codex_apps) = derived_apps {
                let apps_dest = output_dir.join(".app.json");
                let apps_json =
                    serde_json::to_value(codex_apps).map_err(|source| ToolingError::Json {
                        path: apps_dest.clone(),
                        source,
                    })?;
                write_json_file(&apps_dest, &apps_json, overwrite)?;
                written_files.push(display_path(&apps_dest));
                apps_emitted = true;
            }
            if !apps_emitted {
                if let Some(ref apps_path) = ext.apps {
                    let apps_src = resolve_package_path(&package_root, apps_path);
                    let apps_dest = output_dir.join(normalize_package_relative_path(apps_path));
                    copy_file_component(&apps_src, &apps_dest, overwrite)?;
                    written_files.push(display_path(&apps_dest));
                    apps_emitted = true;
                }
            }

            if let Some(ref hooks_path) = ext.hooks {
                let hooks_src = resolve_package_path(&package_root, hooks_path);
                let hooks_dest = if codex_mode == CodexProjectionMode::Current {
                    output_dir.join("hooks").join("hooks.json")
                } else {
                    output_dir.join(normalize_package_relative_path(hooks_path))
                };
                copy_file_component(&hooks_src, &hooks_dest, overwrite)?;
                written_files.push(display_path(&hooks_dest));
                hooks_emitted = true;
            } else {
                let default_hooks_src = package_root.join("hooks").join("hooks.json");
                if default_hooks_src.exists() {
                    let default_hooks_dest = output_dir.join("hooks").join("hooks.json");
                    copy_file_component(&default_hooks_src, &default_hooks_dest, overwrite)?;
                    written_files.push(display_path(&default_hooks_dest));
                    hooks_emitted = true;
                }
            }

            if let Some(ref mcp_path) = ext.mcp_servers {
                let mcp_src = resolve_package_path(&package_root, mcp_path);
                let mcp_dest = output_dir.join(normalize_package_relative_path(mcp_path));
                if mcp_src.is_dir() {
                    if mcp_dest.exists() && !overwrite {
                        return Err(ToolingError::OutputExists { path: mcp_dest });
                    }
                    copy_dir_all(&mcp_src, &mcp_dest)?;
                    if let Ok(walked) = walk_dir_files(&mcp_dest) {
                        for f in walked {
                            written_files.push(display_path(&f));
                        }
                    }
                } else {
                    copy_file_component(&mcp_src, &mcp_dest, overwrite)?;
                    written_files.push(display_path(&mcp_dest));
                }
                mcp_servers_emitted = true;
            }

            if let Some(ref assets) = ext.assets {
                for asset_rel in assets {
                    let asset_src = resolve_package_path(&package_root, asset_rel);
                    let asset_dest = output_dir.join(normalize_package_relative_path(asset_rel));
                    if asset_src.exists() {
                        if asset_src.is_dir() {
                            if asset_dest.exists() && !overwrite {
                                return Err(ToolingError::OutputExists { path: asset_dest });
                            }
                            copy_dir_all(&asset_src, &asset_dest)?;
                            if let Ok(walked) = walk_dir_files(&asset_dest) {
                                for f in walked {
                                    written_files.push(display_path(&f));
                                }
                            }
                        } else if asset_src.is_file() {
                            copy_file_component(&asset_src, &asset_dest, overwrite)?;
                            written_files.push(display_path(&asset_dest));
                        }
                    }
                }
            }
        }
    }

    // Write host-specific plugin manifest if applicable
    if needs_codex_manifest {
        let manifest_dir = output_dir.join(".codex-plugin");
        fs::create_dir_all(&manifest_dir).map_err(|e| ToolingError::Io {
            operation: "create directory",
            path: manifest_dir.clone(),
            source: e,
        })?;
        let mut codex_manifest = serde_json::json!({
            "name": plugin.name,
            "version": codex_projection_version(&plugin.version, codex_projection_digest.as_deref()),
            "description": plugin.description,
            "author": plugin.author.as_ref().map(|a| serde_json::json!({"name": a.name})),
            "license": plugin.license,
            "repository": plugin.repository,
            "skills": "./skills/",
        });
        if let Some(ref ext) = codex_ext {
            if let Some(ref homepage) = ext.homepage {
                codex_manifest["homepage"] = serde_json::json!(homepage);
            }
            if let Some(ref keywords) = ext.keywords {
                codex_manifest["keywords"] = serde_json::json!(keywords);
            }
            // Catalog-driven app-bindings take priority for the apps path.
            if apps_emitted {
                if derived_apps.is_some() {
                    codex_manifest["apps"] = serde_json::json!("./.app.json");
                } else if let Some(ref apps) = ext.apps {
                    codex_manifest["apps"] = serde_json::json!(apps);
                }
            }
            if let Some(ref hooks) = ext.hooks {
                if codex_mode == CodexProjectionMode::Experimental {
                    codex_manifest["hooks"] = serde_json::json!(hooks);
                }
            } else if hooks_emitted && codex_mode == CodexProjectionMode::Experimental {
                codex_manifest["hooks"] = serde_json::json!("./hooks/hooks.json");
            }
            if let Some(ref mcp_servers) = ext.mcp_servers {
                codex_manifest["mcpServers"] = serde_json::json!(mcp_servers);
            }
            if let Some(ref interface) = ext.interface {
                codex_manifest["interface"] =
                    serde_json::to_value(interface).map_err(|source| ToolingError::Json {
                        path: PathBuf::from("codex.plugin/v1.interface"),
                        source,
                    })?;
            }
            if codex_mode == CodexProjectionMode::Experimental {
                for (key, value) in &ext.extra {
                    if codex_manifest.get(key).is_none() {
                        codex_manifest[key] = value.clone();
                    }
                }
            }
        }
        let manifest_path = manifest_dir.join("plugin.json");
        write_json_file(&manifest_path, &codex_manifest, overwrite)?;
        written_files.push(display_path(&manifest_path));
    }

    if needs_claude_manifest {
        let manifest_dir = output_dir.join(".claude-plugin");
        fs::create_dir_all(&manifest_dir).map_err(|e| ToolingError::Io {
            operation: "create directory",
            path: manifest_dir.clone(),
            source: e,
        })?;
        let claude_manifest = serde_json::json!({
            "name": plugin.name,
            "version": plugin.version,
            "description": plugin.description,
            "author": plugin.author.as_ref().map(|a| serde_json::json!({"name": a.name})),
            "skills": "./skills/",
        });
        let manifest_path = manifest_dir.join("plugin.json");
        write_json_file(&manifest_path, &claude_manifest, overwrite)?;
        written_files.push(display_path(&manifest_path));
    }

    if let Some(binary) = binary {
        if !is_safe_archive_path(&binary.archive_path) {
            return Err(ToolingError::InvalidPluginPackage {
                path: manifest_path,
                issues: vec![format!(
                    "binary archive path '{}' is not a safe relative path.",
                    binary.archive_path
                )],
            });
        }
        if !binary.source_path.is_file() {
            return Err(ToolingError::Io {
                operation: "read",
                path: binary.source_path.to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "binary path does not exist or is not a file",
                ),
            });
        }
        let destination = output_dir.join(normalize_package_relative_path(&binary.archive_path));
        copy_file_component(binary.source_path, &destination, overwrite)?;
        written_files.push(display_path(&destination));
    }

    let legacy_routable = host == "codex" && plugin.is_agent_routable();
    Ok(GeneratedHostExport {
        source_package: format!("{}-v{}", plugin.name, plugin.version),
        plugin_name: plugin.name,
        plugin_version: plugin.version,
        lossless: host == "codex",
        routable: legacy_routable,
        losses: if host == "codex" {
            Vec::new()
        } else {
            vec![
                "legacy projection does not prove preservation of host-specific behavior"
                    .to_string(),
            ]
        },
        emitted_components: GeneratedHostExportComponents {
            plugin_manifest: match host {
                "codex" => ".codex-plugin/plugin.json".to_string(),
                "claude" => ".claude-plugin/plugin.json".to_string(),
                _ => String::new(),
            },
            skills_dir: host.to_string(),
            skills_count,
            apps_emitted,
            mcp_servers_emitted,
            hooks_emitted,
        },
        written_files,
    })
}

fn codex_projection_version(base_version: &str, digest: Option<&str>) -> String {
    let base = base_version
        .split_once('+')
        .map_or(base_version, |(base, _)| base);
    match digest {
        Some(digest) => format!("{base}+codex.{}", &digest[..digest.len().min(12)]),
        None => base.to_string(),
    }
}

fn compute_codex_projection_digest(
    package_root: &Path,
    manifest_raw: &str,
    plugin: &ElegyPluginV1,
    codex_ext: Option<&CodexPluginExtensionV1>,
    binary: Option<&PluginArchiveBinary<'_>>,
) -> Result<String, ToolingError> {
    let mut hasher = Sha256::new();
    hash_named_bytes(
        &mut hasher,
        ".elegy-plugin/plugin.json",
        manifest_raw.as_bytes(),
    );

    if let Some(skills_path) = &plugin.skills {
        hash_package_component(&mut hasher, package_root, skills_path)?;
    }
    if let Some(catalog) = &plugin.capability_catalog {
        hash_package_component(&mut hasher, package_root, &catalog.path)?;
    }
    if let Some(mcp_servers) = &plugin.mcp_servers {
        hash_package_component(&mut hasher, package_root, mcp_servers)?;
    }
    if let Some(ext) = codex_ext {
        if let Some(apps) = &ext.apps {
            hash_package_component(&mut hasher, package_root, apps)?;
        }
        if let Some(hooks) = &ext.hooks {
            hash_package_component(&mut hasher, package_root, hooks)?;
        } else {
            let default_hooks = package_root.join("hooks").join("hooks.json");
            if default_hooks.exists() {
                hash_path_component(&mut hasher, package_root, &default_hooks)?;
            }
        }
        if let Some(mcp_servers) = &ext.mcp_servers {
            hash_package_component(&mut hasher, package_root, mcp_servers)?;
        }
        if let Some(assets) = &ext.assets {
            for asset in assets {
                hash_package_component(&mut hasher, package_root, asset)?;
            }
        }
    }
    if let Some(binary) = binary {
        let bytes = fs::read(binary.source_path).map_err(|source| ToolingError::Io {
            operation: "read",
            path: binary.source_path.to_path_buf(),
            source,
        })?;
        hash_named_bytes(
            &mut hasher,
            &format!("binary/{}", binary.archive_path),
            &bytes,
        );
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn hash_package_component(
    hasher: &mut Sha256,
    package_root: &Path,
    relative_path: &str,
) -> Result<(), ToolingError> {
    let path = resolve_package_path(package_root, relative_path);
    if path.exists() {
        hash_path_component(hasher, package_root, &path)?;
    }
    Ok(())
}

fn hash_path_component(
    hasher: &mut Sha256,
    package_root: &Path,
    path: &Path,
) -> Result<(), ToolingError> {
    if path.is_file() {
        let relative = path
            .strip_prefix(package_root)
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));
        if should_skip_projection_digest_path(&relative) {
            return Ok(());
        }
        let bytes = fs::read(path).map_err(|source| ToolingError::Io {
            operation: "read",
            path: path.to_path_buf(),
            source,
        })?;
        hash_named_bytes(hasher, &relative, &bytes);
        return Ok(());
    }
    if path.is_dir() {
        let mut files = walk_dir_files(path)?;
        files.sort();
        for file in files {
            hash_path_component(hasher, package_root, &file)?;
        }
    }
    Ok(())
}

fn should_skip_projection_digest_path(relative: &str) -> bool {
    relative == "install-receipt.json"
        || relative == ".elegy-plugin/plugin.json"
        || relative == "plugin.json"
        || relative.starts_with(".codex-plugin/")
        || relative.starts_with(".claude-plugin/")
}

fn hash_named_bytes(hasher: &mut Sha256, name: &str, bytes: &[u8]) {
    hasher.update(name.as_bytes());
    hasher.update([0]);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    hasher.update([0xff]);
}

/// Recursively copy a directory.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), ToolingError> {
    fs::create_dir_all(dst).map_err(|e| ToolingError::Io {
        operation: "create directory",
        path: dst.to_path_buf(),
        source: e,
    })?;
    for entry in fs::read_dir(src).map_err(|e| ToolingError::Io {
        operation: "read directory",
        path: src.to_path_buf(),
        source: e,
    })? {
        let entry = entry.map_err(|e| ToolingError::Io {
            operation: "read directory entry",
            path: src.to_path_buf(),
            source: e,
        })?;
        let ty = entry.file_type().map_err(|e| ToolingError::Io {
            operation: "read file type",
            path: entry.path(),
            source: e,
        })?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else if ty.is_file() {
            fs::copy(entry.path(), dst.join(entry.file_name())).map_err(|e| ToolingError::Io {
                operation: "copy",
                path: entry.path(),
                source: e,
            })?;
        }
    }
    Ok(())
}

fn copy_file_component(src: &Path, dst: &Path, overwrite: bool) -> Result<(), ToolingError> {
    if dst.exists() && !overwrite {
        return Err(ToolingError::OutputExists {
            path: dst.to_path_buf(),
        });
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| ToolingError::Io {
            operation: "create directory",
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    fs::copy(src, dst).map_err(|e| ToolingError::Io {
        operation: "copy",
        path: src.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// Walk a directory tree and return all file paths.
fn walk_dir_files(dir: &Path) -> Result<Vec<PathBuf>, ToolingError> {
    let mut files = Vec::new();
    walk_dir_files_recursive(dir, &mut files)?;
    Ok(files)
}

fn walk_dir_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), ToolingError> {
    for entry in fs::read_dir(dir).map_err(|e| ToolingError::Io {
        operation: "read directory",
        path: dir.to_path_buf(),
        source: e,
    })? {
        let entry = entry.map_err(|e| ToolingError::Io {
            operation: "read directory entry",
            path: dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if path.is_dir() {
            walk_dir_files_recursive(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn validate_current_codex_projection(
    plugin: &ElegyPluginV1,
    extension: Option<&CodexPluginExtensionV1>,
) -> Vec<String> {
    let mut issues = Vec::new();
    if plugin
        .author
        .as_ref()
        .is_none_or(|author| author.name.trim().is_empty())
    {
        issues.push("current Codex export requires author.name.".to_string());
    }
    let Some(interface) = extension.and_then(|extension| extension.interface.as_ref()) else {
        issues.push(
            "current Codex export requires extensions.codex.plugin/v1.interface.".to_string(),
        );
        return issues;
    };
    for (field, value) in [
        ("displayName", &interface.display_name),
        ("shortDescription", &interface.short_description),
        ("longDescription", &interface.long_description),
        ("developerName", &interface.developer_name),
        ("category", &interface.category),
    ] {
        if value.as_deref().is_none_or(|value| value.trim().is_empty()) {
            issues.push(format!("current Codex export requires interface.{field}."));
        }
    }
    if interface.capabilities.as_ref().is_none_or(|values| {
        values.is_empty() || values.iter().any(|value| value.trim().is_empty())
    }) {
        issues.push("current Codex export requires non-empty interface.capabilities.".to_string());
    }
    if interface.default_prompt.as_ref().is_none_or(|values| {
        values.is_empty() || values.iter().any(|value| value.trim().is_empty())
    }) {
        issues.push("current Codex export requires non-empty interface.defaultPrompt.".to_string());
    }
    issues
}

pub fn pack_plugin_v3(plugin_path: &Path, output_zip: &Path) -> Result<String, ToolingError> {
    pack_plugin_v3_with_binary(plugin_path, output_zip, None)
}

pub fn pack_plugin_v3_with_binary(
    plugin_path: &Path,
    output_zip: &Path,
    binary: Option<PluginArchiveBinary<'_>>,
) -> Result<String, ToolingError> {
    let (repo_root, manifest_path) = resolve_plugin_root(plugin_path)?;
    let verification = verify_plugin_v3(&repo_root.join(".elegy-plugin"))?;
    if !verification.valid {
        return Err(ToolingError::InvalidPluginPackage {
            path: manifest_path,
            issues: verification.issues,
        });
    }
    let raw = fs::read_to_string(&manifest_path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: manifest_path.clone(),
        source,
    })?;
    let plugin: ElegyPluginV3 =
        serde_json::from_str(&raw).map_err(|source| ToolingError::Json {
            path: manifest_path.clone(),
            source,
        })?;
    let mut entries = vec![(manifest_path.clone(), "plugin.json".to_string())];

    for path in v3_string_paths(plugin.skills.as_ref())
        .into_iter()
        .chain(v3_string_paths(plugin.apps.as_ref()))
        .chain(v3_string_paths(plugin.hooks.as_ref()))
    {
        collect_component_path(&repo_root, path, &mut entries)?;
    }
    for path in v3_asset_paths(plugin.assets.as_ref()) {
        if !path_is_uri(path) {
            collect_component_path(&repo_root, path, &mut entries)?;
        }
    }
    if plugin.mcp_servers.as_ref().is_some_and(Value::is_string) {
        for path in v3_string_paths(plugin.mcp_servers.as_ref()) {
            collect_component_path(&repo_root, path, &mut entries)?;
        }
    }
    for path in v3_interface_asset_paths(plugin.interface.as_ref()) {
        collect_component_path(&repo_root, path, &mut entries)?;
    }
    for path in &plugin.elegy.package_assets {
        collect_component_path(&repo_root, path, &mut entries)?;
    }
    if let Some(catalog) = &plugin.elegy.capability_catalog {
        collect_component_path(&repo_root, &catalog.path, &mut entries)?;
    }
    collect_component_path(&repo_root, &plugin.elegy.readiness.path, &mut entries)?;
    let readiness_path = resolve_package_path(&repo_root, &plugin.elegy.readiness.path);
    let readiness: ElegyReadinessV1 =
        serde_json::from_str(&fs::read_to_string(&readiness_path).map_err(|source| {
            ToolingError::Io {
                operation: "read",
                path: readiness_path.clone(),
                source,
            }
        })?)
        .map_err(|source| ToolingError::Json {
            path: readiness_path,
            source,
        })?;
    let mut evidence_paths = BTreeSet::new();
    for evidence in readiness.evidence {
        if evidence_paths.insert(evidence.path.clone()) {
            collect_component_path(&repo_root, &evidence.path, &mut entries)?;
        }
    }
    if let Some(path) = &plugin.elegy.connections.requirements.path {
        collect_component_path(&repo_root, path, &mut entries)?;
    }
    if let Some(provider) = &plugin.elegy.connections.provider {
        collect_component_path(&repo_root, &provider.path, &mut entries)?;
    }
    if let Some(binary) = binary {
        if !is_safe_archive_path(&binary.archive_path) || !binary.source_path.is_file() {
            return Err(ToolingError::InvalidPluginPackage {
                path: binary.source_path.to_path_buf(),
                issues: vec!["binary source or destination path is invalid.".to_string()],
            });
        }
        entries.push((binary.source_path.to_path_buf(), binary.archive_path));
    }
    write_plugin_archive(entries, &manifest_path, output_zip)
}

fn write_plugin_archive(
    mut entries: Vec<(PathBuf, String)>,
    manifest_path: &Path,
    output_zip: &Path,
) -> Result<String, ToolingError> {
    entries.sort_by(|a, b| a.1.cmp(&b.1));
    if let Some(duplicate) = entries
        .windows(2)
        .find(|pair| pair[0].1 == pair[1].1)
        .map(|pair| pair[0].1.clone())
    {
        return Err(ToolingError::InvalidPluginPackage {
            path: manifest_path.to_path_buf(),
            issues: vec![format!("duplicate archive target '{duplicate}'.")],
        });
    }
    let file = fs::File::create(output_zip).map_err(|source| ToolingError::Io {
        operation: "create",
        path: output_zip.to_path_buf(),
        source,
    })?;
    let mut zip_writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut buffer = Vec::new();
    for (entry_path, relative_str) in &entries {
        if should_exclude_from_pack(relative_str) {
            continue;
        }
        let entry_options = options.unix_permissions(if relative_str.starts_with("bin/") {
            0o755
        } else {
            0o644
        });
        zip_writer
            .start_file(relative_str.clone(), entry_options)
            .map_err(|source| ToolingError::Io {
                operation: "write zip entry",
                path: PathBuf::from(relative_str),
                source: source.into(),
            })?;
        buffer.clear();
        let mut file = fs::File::open(entry_path).map_err(|source| ToolingError::Io {
            operation: "read",
            path: entry_path.clone(),
            source,
        })?;
        file.read_to_end(&mut buffer)
            .map_err(|source| ToolingError::Io {
                operation: "read",
                path: entry_path.clone(),
                source,
            })?;
        zip_writer
            .write_all(&buffer)
            .map_err(|source| ToolingError::Io {
                operation: "write zip content",
                path: entry_path.clone(),
                source,
            })?;
    }
    zip_writer.finish().map_err(|source| ToolingError::Io {
        operation: "finalize zip",
        path: output_zip.to_path_buf(),
        source: source.into(),
    })?;
    Ok(display_path(output_zip))
}

/// Pack a v1-format plugin into a portable zip archive.
///
/// Accepts the three path forms supported by `resolve_plugin_root`.
/// The manifest entry is placed at the archive root as `plugin.json`.
/// Only declared component directories are included.
pub fn pack_plugin_v1(plugin_path: &Path, output_zip: &Path) -> Result<String, ToolingError> {
    pack_plugin_v1_with_binary(plugin_path, output_zip, None)
}

/// Pack a v1-format plugin into a portable zip archive, optionally including a compiled binary.
pub fn pack_plugin_v1_with_binary(
    plugin_path: &Path,
    output_zip: &Path,
    binary: Option<PluginArchiveBinary<'_>>,
) -> Result<String, ToolingError> {
    let (repo_root, _manifest_path) = resolve_plugin_root(plugin_path)?;
    let plugin_dir = repo_root.join(".elegy-plugin");
    let manifest_path = plugin_dir.join("plugin.json");

    // Verify the plugin before packing
    let verify_result = verify_plugin_v1(&plugin_dir)?;
    if !verify_result.valid {
        return Err(ToolingError::InvalidPluginPackage {
            path: manifest_path,
            issues: verify_result.issues,
        });
    }

    // Load the plugin manifest to find component directories
    let raw = fs::read_to_string(&manifest_path).map_err(|e| ToolingError::Io {
        operation: "read",
        path: manifest_path.clone(),
        source: e,
    })?;
    let plugin: ElegyPluginV1 = serde_json::from_str(&raw).map_err(|e| ToolingError::Json {
        path: manifest_path.clone(),
        source: e,
    })?;
    let codex_ext = extract_codex_extension_v1(&plugin.extensions);

    // Collect all files to include
    let mut entries: Vec<(PathBuf, String)> = Vec::new();

    // Include the manifest file (will be renamed to plugin.json at root)
    entries.push((manifest_path.clone(), "plugin.json".to_string()));

    // Include declared component directories
    let component_roots: Vec<&str> = vec![plugin.skills.as_deref(), plugin.mcp_servers.as_deref()]
        .into_iter()
        .flatten()
        .collect();

    for root_str in &component_roots {
        collect_component_path(&repo_root, root_str, &mut entries)?;
    }

    // Include capability catalog if declared
    if let Some(cat_config) = &plugin.capability_catalog {
        let catalog_path = normalize_package_relative_path(&cat_config.path);
        let catalog_full = repo_root.join(&catalog_path);
        if catalog_full.exists() {
            entries.push((catalog_full, catalog_path));
        }
    }

    // Readiness and connection descriptors are package authority, not
    // repository-only verification inputs. Include them and their receipts in
    // every portable archive.
    if let Some(readiness_ref) = &plugin.readiness {
        let readiness_path = normalize_package_relative_path(&readiness_ref.path);
        let readiness_full = repo_root.join(&readiness_path);
        let readiness_raw =
            fs::read_to_string(&readiness_full).map_err(|source| ToolingError::Io {
                operation: "read",
                path: readiness_full.clone(),
                source,
            })?;
        let readiness: ElegyReadinessV1 =
            serde_json::from_str(&readiness_raw).map_err(|source| ToolingError::Json {
                path: readiness_full.clone(),
                source,
            })?;
        entries.push((readiness_full, readiness_path));
        let mut evidence_paths = BTreeSet::new();
        for evidence in readiness.evidence {
            if !evidence_paths.insert(evidence.path.clone()) {
                continue;
            }
            let evidence_path = normalize_package_relative_path(&evidence.path);
            entries.push((repo_root.join(&evidence_path), evidence_path));
        }
    }
    if let Some(connections) = &plugin.connections {
        if let Some(requirements_path) = &connections.requirements.path {
            let path = normalize_package_relative_path(requirements_path);
            entries.push((repo_root.join(&path), path));
        }
        if let Some(provider) = &connections.provider {
            let path = normalize_package_relative_path(&provider.path);
            entries.push((repo_root.join(&path), path));
        }
    }

    if let Some(ext) = &codex_ext {
        for path in [&ext.apps, &ext.hooks, &ext.mcp_servers]
            .into_iter()
            .flatten()
        {
            collect_component_path(&repo_root, path, &mut entries)?;
        }
        if ext.hooks.is_none() {
            let default_hooks = repo_root.join("hooks").join("hooks.json");
            if default_hooks.exists() {
                entries.push((default_hooks, "hooks/hooks.json".to_string()));
            }
        }
        if let Some(assets) = &ext.assets {
            for asset in assets {
                collect_component_path(&repo_root, asset, &mut entries)?;
            }
        }
    }

    if let Some(binary) = binary {
        if !is_safe_archive_path(&binary.archive_path) {
            return Err(ToolingError::InvalidPluginPackage {
                path: manifest_path,
                issues: vec![format!(
                    "binary archive path '{}' is not a safe relative path.",
                    binary.archive_path
                )],
            });
        }
        if !binary.source_path.exists() || !binary.source_path.is_file() {
            return Err(ToolingError::Io {
                operation: "read",
                path: binary.source_path.to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "binary path does not exist or is not a file",
                ),
            });
        }
        entries.push((binary.source_path.to_path_buf(), binary.archive_path));
    }

    // Sort for deterministic archives
    entries.sort_by(|a, b| a.1.cmp(&b.1));
    if let Some(duplicate) = entries
        .windows(2)
        .find(|pair| pair[0].1 == pair[1].1)
        .map(|pair| pair[0].1.clone())
    {
        return Err(ToolingError::InvalidPluginPackage {
            path: manifest_path,
            issues: vec![format!("duplicate archive target '{duplicate}'.")],
        });
    }

    // Create the zip archive
    let file = fs::File::create(output_zip).map_err(|source| ToolingError::Io {
        operation: "create",
        path: output_zip.to_path_buf(),
        source,
    })?;

    let mut zip_writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let mut buffer = Vec::new();

    for (entry_path, relative_str) in &entries {
        // Skip excluded patterns
        if should_exclude_from_pack(relative_str) {
            continue;
        }

        let entry_options = options.unix_permissions(if relative_str.starts_with("bin/") {
            0o755
        } else {
            0o644
        });
        zip_writer
            .start_file(relative_str.clone(), entry_options)
            .map_err(|source| ToolingError::Io {
                operation: "write zip entry",
                path: PathBuf::from(relative_str),
                source: source.into(),
            })?;

        if entry_path.is_file() {
            buffer.clear();
            let mut f = fs::File::open(entry_path).map_err(|source| ToolingError::Io {
                operation: "read",
                path: entry_path.clone(),
                source,
            })?;
            f.read_to_end(&mut buffer)
                .map_err(|source| ToolingError::Io {
                    operation: "read",
                    path: entry_path.clone(),
                    source,
                })?;
            zip_writer
                .write_all(&buffer)
                .map_err(|source| ToolingError::Io {
                    operation: "write zip content",
                    path: entry_path.clone(),
                    source,
                })?;
        }
    }

    zip_writer.finish().map_err(|source| ToolingError::Io {
        operation: "finalize zip",
        path: output_zip.to_path_buf(),
        source: source.into(),
    })?;

    Ok(display_path(output_zip))
}

fn is_safe_archive_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains('\\')
        && !path.contains(':')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn collect_files_recursive(
    repo_root: &Path,
    dir: &Path,
    entries: &mut Vec<(PathBuf, String)>,
) -> Result<(), ToolingError> {
    for entry in fs::read_dir(dir).map_err(|source| ToolingError::Io {
        operation: "read directory",
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ToolingError::Io {
            operation: "read directory entry",
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(repo_root, &path, entries)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(repo_root)
                .map(|rel| rel.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_default()
                });
            entries.push((path, relative));
        }
    }
    Ok(())
}

fn collect_component_path(
    repo_root: &Path,
    component_path: &str,
    entries: &mut Vec<(PathBuf, String)>,
) -> Result<(), ToolingError> {
    let normalized = normalize_package_relative_path(component_path);
    let path = repo_root.join(&normalized);
    if path.is_dir() {
        collect_files_recursive(repo_root, &path, entries)?;
    } else if path.is_file() {
        entries.push((path, normalized));
    } else {
        return Err(ToolingError::InvalidPluginPackage {
            path,
            issues: vec![format!(
                "declared component path '{component_path}' does not exist."
            )],
        });
    }
    Ok(())
}

/// Check if a relative path should be excluded from the plugin archive.
fn should_exclude_from_pack(relative_str: &str) -> bool {
    let parts: Vec<&str> = relative_str.split('/').collect();
    for part in &parts {
        if *part == ".git" || *part == "target" {
            return true;
        }
    }
    // Exclude temporary files
    if relative_str.ends_with(".tmp")
        || relative_str.ends_with(".swp")
        || relative_str.ends_with('~')
    {
        return true;
    }
    false
}

// ── MCP descriptor helpers ────────────────────────────────────────────────

fn build_mcp_descriptor(
    request: AuthorMcpDescriptorRequest,
) -> Result<McpServerDescriptor, ToolingError> {
    let descriptor = McpServerDescriptor {
        server_name: request.server_name,
        transport: request.transport,
        tools: request
            .tools
            .into_iter()
            .map(|tool| McpToolDefinition {
                name: tool.name,
                description: tool.description,
                input_schema: None,
            })
            .collect(),
    };

    let issues = descriptor_validation_issues(&descriptor);
    if !issues.is_empty() {
        return Err(ToolingError::InvalidMcpDescriptor {
            path: PathBuf::from("<in-memory>"),
            issues,
        });
    }

    Ok(descriptor)
}

fn load_mcp_descriptor_file(path: &Path) -> Result<McpServerDescriptor, ToolingError> {
    let content = fs::read_to_string(path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: path.to_path_buf(),
        source,
    })?;

    let descriptor = serde_json::from_str::<McpServerDescriptor>(&content).map_err(|source| {
        ToolingError::Json {
            path: path.to_path_buf(),
            source,
        }
    })?;

    let issues = descriptor_validation_issues(&descriptor);
    if !issues.is_empty() {
        return Err(ToolingError::InvalidMcpDescriptor {
            path: path.to_path_buf(),
            issues,
        });
    }

    Ok(descriptor)
}

fn validate_codex_mcp_config_file(path: &Path) -> Vec<String> {
    let mut issues = Vec::new();
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) => {
            issues.push(format!("unable to read '{}': {error}.", path.display()));
            return issues;
        }
    };
    let value: Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(error) => {
            issues.push(format!("'{}' is not valid JSON: {error}.", path.display()));
            return issues;
        }
    };
    let Some(root) = value.as_object() else {
        issues.push("companion file must contain a JSON object.".to_string());
        return issues;
    };
    let Some(servers) = root.get("mcpServers").and_then(Value::as_object) else {
        issues.push("companion file must contain an mcpServers object.".to_string());
        return issues;
    };
    if servers.is_empty() {
        issues.push("mcpServers must contain at least one server.".to_string());
    }
    for (name, config) in servers {
        if name.trim().is_empty() {
            issues.push("server names must not be empty.".to_string());
        }
        if !config.is_object() {
            issues.push(format!("server '{name}' config must be an object."));
        }
    }
    issues
}

fn load_codex_apps_file(path: &Path) -> Result<CodexAppsFile, ToolingError> {
    let content = fs::read_to_string(path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: path.to_path_buf(),
        source,
    })?;

    serde_json::from_str::<CodexAppsFile>(&content).map_err(|source| ToolingError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn validate_codex_apps_file(apps_file: &CodexAppsFile) -> Vec<String> {
    let mut issues = Vec::new();
    if apps_file.apps.is_empty() {
        issues.push("apps must contain at least one connector reference.".to_string());
    }
    for (app_name, app_ref) in &apps_file.apps {
        if !validate_codex_app_key(app_name) {
            issues.push(format!(
                "app key '{app_name}' must use lowercase letters, digits, hyphens, or underscores."
            ));
        }
        if app_ref.id.trim().is_empty() {
            issues.push(format!("app '{app_name}' id must not be empty."));
        }
        if app_ref
            .category
            .as_deref()
            .is_some_and(|category| category.trim().is_empty())
        {
            issues.push(format!("app '{app_name}' category must not be empty."));
        }
    }
    issues
}

fn validate_codex_app_key(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let bytes = name.as_bytes();
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-' || *b == b'_')
}

// ── Capability Catalog validation ────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ElegyCapabilityCatalogValidationResult {
    pub issues: Vec<String>,
}

impl ElegyCapabilityCatalogValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

pub fn validate_elegy_capability_catalog_v1(
    catalog: &ElegyCapabilityCatalogV1,
) -> ElegyCapabilityCatalogValidationResult {
    let mut issues = Vec::new();

    if catalog.schema_version != ELEGY_CAPABILITY_CATALOG_V1_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{}', found '{}'.",
            ELEGY_CAPABILITY_CATALOG_V1_SCHEMA_VERSION, catalog.schema_version
        ));
    }

    if catalog.plugin.is_empty() {
        issues.push("plugin must not be empty.".into());
    } else if !validate_kebab_case_name(&catalog.plugin) {
        issues.push(format!(
            "plugin '{}' is not valid lowercase kebab-case.",
            catalog.plugin
        ));
    }

    if catalog.plugin_version.is_empty() {
        issues.push("pluginVersion must not be empty.".into());
    } else if !validate_semver(&catalog.plugin_version) {
        issues.push(format!(
            "pluginVersion '{}' is not valid SemVer 2.0.0.",
            catalog.plugin_version
        ));
    }

    if catalog.capabilities.is_empty() {
        issues.push("capabilities must contain at least one entry.".into());
    }

    let mut ids = BTreeSet::new();
    for capability in &catalog.capabilities {
        for issue in validate_elegy_capability(capability) {
            issues.push(format!("capabilities.{}: {}", capability.id, issue));
        }
        if !ids.insert(capability.id.clone()) {
            issues.push(format!("duplicate capability id '{}'.", capability.id));
        }
    }

    ElegyCapabilityCatalogValidationResult { issues }
}

pub fn validate_elegy_capability_catalog_v2(
    catalog: &ElegyCapabilityCatalogV2,
) -> ElegyCapabilityCatalogValidationResult {
    let mut issues = Vec::new();
    if catalog.schema_version != ELEGY_CAPABILITY_CATALOG_V2_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{}', found '{}'.",
            ELEGY_CAPABILITY_CATALOG_V2_SCHEMA_VERSION, catalog.schema_version
        ));
    }
    if catalog.plugin.is_empty() {
        issues.push("plugin must not be empty.".into());
    } else if !validate_kebab_case_name(&catalog.plugin) {
        issues.push(format!(
            "plugin '{}' is not valid lowercase kebab-case.",
            catalog.plugin
        ));
    }
    if catalog.plugin_version.is_empty() {
        issues.push("pluginVersion must not be empty.".into());
    } else if !validate_semver(&catalog.plugin_version) {
        issues.push(format!(
            "pluginVersion '{}' is not valid SemVer 2.0.0.",
            catalog.plugin_version
        ));
    }
    if catalog.capabilities.is_empty() {
        issues.push("capabilities must contain at least one entry.".into());
    }
    let mut ids = BTreeSet::new();
    for capability in &catalog.capabilities {
        let common = capability.common();
        if common.id.is_empty() {
            issues.push("capabilities: id must not be empty.".into());
        }
        if !ids.insert(common.id.clone()) {
            issues.push(format!("duplicate capability id '{}'.", common.id));
        }
        if common.description.trim().is_empty() {
            issues.push(format!(
                "capabilities.{}: description must not be empty.",
                common.id
            ));
        }
        if common.contract_version.trim().is_empty() {
            issues.push(format!(
                "capabilities.{}: contractVersion must not be empty.",
                common.id
            ));
        }
        match capability {
            ElegyCapabilityV2::Cli { invocation, .. } => {
                if invocation.executable.trim().is_empty() {
                    issues.push(format!(
                        "capabilities.{}: invocation.executable must not be empty.",
                        common.id
                    ));
                }
                if invocation.command.is_empty() {
                    issues.push(format!(
                        "capabilities.{}: invocation.command must not be empty.",
                        common.id
                    ));
                }
            }
            ElegyCapabilityV2::McpResource {
                resource_uri,
                output_schema,
                ..
            } => {
                if resource_uri.trim().is_empty() {
                    issues.push(format!(
                        "capabilities.{}: resourceUri must not be empty.",
                        common.id
                    ));
                }
                if !output_schema.is_object() {
                    issues.push(format!(
                        "capabilities.{}: outputSchema must be a JSON object.",
                        common.id
                    ));
                }
            }
            ElegyCapabilityV2::McpTool {
                tool_name,
                input_schema,
                output_schema,
                ..
            } => {
                if tool_name.trim().is_empty() {
                    issues.push(format!(
                        "capabilities.{}: toolName must not be empty.",
                        common.id
                    ));
                }
                if !input_schema.is_object() {
                    issues.push(format!(
                        "capabilities.{}: inputSchema must be a JSON object.",
                        common.id
                    ));
                }
                if !output_schema.is_object() {
                    issues.push(format!(
                        "capabilities.{}: outputSchema must be a JSON object.",
                        common.id
                    ));
                }
            }
        }
    }
    ElegyCapabilityCatalogValidationResult { issues }
}

pub fn migrate_capability_catalog_v1_to_v2(
    catalog: &ElegyCapabilityCatalogV1,
) -> Result<ElegyCapabilityCatalogV2, Vec<String>> {
    let mut issues = Vec::new();
    let mut capabilities = Vec::with_capacity(catalog.capabilities.len());
    for capability in &catalog.capabilities {
        let common = || ElegyCapabilityV2Common {
            id: capability.id.clone(),
            description: capability.description.clone(),
            contract_version: capability.contract_version.clone(),
            side_effect_class: capability.side_effect_class,
            readiness: ElegyReadinessStage::Implemented,
        };
        match capability.kind {
            ElegyCapabilityKind::Cli => match capability.invocation.as_ref() {
                Some(invocation) => capabilities.push(ElegyCapabilityV2::Cli {
                    common: common(),
                    invocation: ElegyCapabilityInvocationV2 {
                        executable: invocation.executable.clone(),
                        command: invocation.command.clone(),
                        required_args: invocation.required_args.clone(),
                        optional_args: invocation.optional_args.clone(),
                        input_schema: None,
                        output_schema: None,
                    },
                }),
                None => issues.push(format!("capability '{}' is missing invocation.", capability.id)),
            },
            ElegyCapabilityKind::Mcp => match capability.invocation.as_ref() {
                Some(invocation) if invocation.tool_name.as_deref().is_some_and(|name| !name.trim().is_empty()) => {
                    match (capability.input_schema.as_ref(), capability.output_schema.as_ref()) {
                        (Some(input_schema), Some(output_schema))
                            if input_schema.is_object() && output_schema.is_object() =>
                        {
                            capabilities.push(ElegyCapabilityV2::McpTool {
                                common: common(),
                                tool_name: invocation.tool_name.clone().unwrap_or_default(),
                                input_schema: input_schema.clone(),
                                output_schema: output_schema.clone(),
                            });
                        }
                        _ => issues.push(format!(
                            "capability '{}' mcp migration requires object inputSchema and outputSchema contracts.",
                            capability.id
                        )),
                    }
                }
                _ => issues.push(format!(
                    "capability '{}' mcp migration requires invocation.toolName; resource inference is not supported.",
                    capability.id
                )),
            },
            ElegyCapabilityKind::AppBinding => issues.push(format!(
                "capability '{}' app-binding cannot be migrated to v2 without an explicit concrete interface.",
                capability.id
            )),
        }
    }
    if !issues.is_empty() {
        return Err(issues);
    }
    Ok(ElegyCapabilityCatalogV2 {
        schema_version: ELEGY_CAPABILITY_CATALOG_V2_SCHEMA_VERSION.to_string(),
        plugin: catalog.plugin.clone(),
        plugin_version: catalog.plugin_version.clone(),
        generated_at: catalog.generated_at.clone(),
        digest: catalog.digest.clone(),
        capabilities,
    })
}

fn validate_elegy_capability(capability: &ElegyCapability) -> Vec<String> {
    let mut issues = Vec::new();

    if capability.id.is_empty() {
        issues.push("id must not be empty.".into());
    }

    if capability.description.trim().is_empty() {
        issues.push("description must not be empty.".into());
    }

    if capability.contract_version.trim().is_empty() {
        issues.push("contractVersion must not be empty.".into());
    }

    match capability.kind {
        ElegyCapabilityKind::Cli | ElegyCapabilityKind::Mcp => match &capability.invocation {
            Some(invocation) => {
                if invocation.executable.trim().is_empty() {
                    issues.push("invocation.executable must not be empty.".into());
                }
                if invocation.command.is_empty() {
                    issues.push("invocation.command must not be empty.".into());
                }
            }
            None => {
                issues.push(format!("{:?} kind requires invocation.", capability.kind));
            }
        },
        ElegyCapabilityKind::AppBinding => {
            if capability.app_binding.is_none() {
                issues.push("app-binding kind requires appBinding.".into());
            }
        }
    }

    if let Some(app_binding) = &capability.app_binding {
        if app_binding.connector.trim().is_empty() {
            issues.push("appBinding.connector must not be empty.".into());
        }
    }

    if let Some(fallback) = &capability.fallback {
        for issue in validate_elegy_capability_fallback(fallback) {
            issues.push(format!("fallback: {issue}"));
        }
    }

    issues
}

fn validate_elegy_capability_fallback(fallback: &ElegyCapabilityFallback) -> Vec<String> {
    let mut issues = Vec::new();

    match fallback.kind {
        ElegyCapabilityKind::Cli | ElegyCapabilityKind::Mcp => {}
        ElegyCapabilityKind::AppBinding => {
            issues.push("fallback kind must be cli or mcp, not app-binding.".into());
        }
    }

    if fallback.invocation.executable.trim().is_empty() {
        issues.push("fallback invocation executable must not be empty.".into());
    }

    if fallback.invocation.command.is_empty() {
        issues.push("fallback invocation command must not be empty.".into());
    }

    issues
}

/// Load a `elegy-capability-catalog/v1` file from disk.
pub fn load_capability_catalog_v1(path: &Path) -> Result<ElegyCapabilityCatalogV1, ToolingError> {
    let content = fs::read_to_string(path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: path.to_path_buf(),
        source,
    })?;
    let catalog: ElegyCapabilityCatalogV1 =
        serde_json::from_str(&content).map_err(|source| ToolingError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(catalog)
}

/// Load either supported capability-catalog wire version. Version dispatch is
/// based solely on the declared schemaVersion; v1 is never upgraded implicitly.
pub fn load_capability_catalog(path: &Path) -> Result<ElegyCapabilityCatalog, ToolingError> {
    let content = fs::read_to_string(path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: path.to_path_buf(),
        source,
    })?;
    let value: Value = serde_json::from_str(&content).map_err(|source| ToolingError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    match value.get("schemaVersion").and_then(Value::as_str) {
        Some(ELEGY_CAPABILITY_CATALOG_V1_SCHEMA_VERSION) => serde_json::from_value(value)
            .map(ElegyCapabilityCatalog::V1)
            .map_err(|source| ToolingError::Json {
                path: path.to_path_buf(),
                source,
            }),
        Some(ELEGY_CAPABILITY_CATALOG_V2_SCHEMA_VERSION) => serde_json::from_value(value)
            .map(ElegyCapabilityCatalog::V2)
            .map_err(|source| ToolingError::Json {
                path: path.to_path_buf(),
                source,
            }),
        Some(version) => Err(ToolingError::InvalidPluginPackage {
            path: path.to_path_buf(),
            issues: vec![format!(
                "unsupported capability catalog schemaVersion '{version}'."
            )],
        }),
        None => Err(ToolingError::InvalidPluginPackage {
            path: path.to_path_buf(),
            issues: vec!["capability catalog schemaVersion is required.".to_string()],
        }),
    }
}

/// Load an `elegy-plugin-connections/v1` file from disk.
pub fn load_connection_requirements_v1(
    path: &Path,
) -> Result<ElegyPluginConnectionsV1, ToolingError> {
    let content = fs::read_to_string(path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&content).map_err(|source| ToolingError::Json {
        path: path.to_path_buf(),
        source,
    })
}

pub fn validate_elegy_plugin_connections_v1(connections: &ElegyPluginConnectionsV1) -> Vec<String> {
    let mut issues = Vec::new();
    if connections.schema_version != ELEGY_PLUGIN_CONNECTIONS_V1_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{}', found '{}'.",
            ELEGY_PLUGIN_CONNECTIONS_V1_SCHEMA_VERSION, connections.schema_version
        ));
    }
    if !validate_kebab_case_name(&connections.plugin) {
        issues.push("plugin must be lowercase kebab-case.".to_string());
    }
    if !validate_semver(&connections.plugin_version) {
        issues.push("pluginVersion must be valid SemVer.".to_string());
    }
    if connections.requirements.is_empty() {
        issues.push("requirements must contain at least one entry.".to_string());
    }
    let mut seen = BTreeSet::new();
    for requirement in &connections.requirements {
        if !validate_kebab_case_name(&requirement.id) {
            issues.push(format!(
                "requirement id '{}' must be lowercase kebab-case.",
                requirement.id
            ));
        }
        if !seen.insert(requirement.id.clone()) {
            issues.push(format!("duplicate requirement id '{}'.", requirement.id));
        }
        if !validate_kebab_case_name(&requirement.service) {
            issues.push(format!(
                "requirement '{}' service '{}' must be lowercase kebab-case.",
                requirement.id, requirement.service
            ));
        }
        if requirement.description.trim().is_empty() {
            issues.push(format!(
                "requirement '{}' description must not be empty.",
                requirement.id
            ));
        }
    }
    issues
}

/// Load an `elegy-connection-provider/v1` descriptor from disk.
pub fn load_connection_provider_v1(path: &Path) -> Result<ElegyConnectionProviderV1, ToolingError> {
    let content = fs::read_to_string(path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&content).map_err(|source| ToolingError::Json {
        path: path.to_path_buf(),
        source,
    })
}

pub fn validate_elegy_connection_provider_v1(provider: &ElegyConnectionProviderV1) -> Vec<String> {
    let mut issues = Vec::new();
    if provider.schema_version != ELEGY_CONNECTION_PROVIDER_V1_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{}', found '{}'.",
            ELEGY_CONNECTION_PROVIDER_V1_SCHEMA_VERSION, provider.schema_version
        ));
    }
    if !validate_kebab_case_name(&provider.id) {
        issues.push("id must be lowercase kebab-case.".to_string());
    }
    if provider.control_protocol != ELEGY_CONNECTION_CONTROL_V1_PROTOCOL_VERSION {
        issues.push(format!(
            "controlProtocol must be '{}', found '{}'.",
            ELEGY_CONNECTION_CONTROL_V1_PROTOCOL_VERSION, provider.control_protocol
        ));
    }
    if provider.invocation.executable.trim().is_empty() {
        issues.push("invocation executable must not be empty.".to_string());
    }
    if provider.invocation.command.is_empty()
        || provider
            .invocation
            .command
            .iter()
            .any(|segment| segment.trim().is_empty())
    {
        issues.push("invocation command must contain non-empty segments.".to_string());
    }
    issues
}

/// Build a `CodexAppsFile` from `app-binding` capabilities in a catalog.
///
/// Returns `None` if the catalog has no `app-binding` capabilities.
pub fn build_codex_apps_from_catalog(catalog: &ElegyCapabilityCatalogV1) -> Option<CodexAppsFile> {
    let mut apps = BTreeMap::new();
    for capability in &catalog.capabilities {
        if capability.kind == ElegyCapabilityKind::AppBinding {
            if let Some(app_binding) = &capability.app_binding {
                apps.insert(
                    app_binding.connector.clone(),
                    CodexAppReference {
                        id: app_binding.connector.clone(),
                        required: false,
                        category: app_binding.category.clone(),
                    },
                );
            }
        }
    }
    if apps.is_empty() {
        None
    } else {
        Some(CodexAppsFile { apps })
    }
}

/// Build Codex app references from portable logical requirements and explicit
/// host-issued bindings. Service names are never used as Codex app IDs.
pub fn build_codex_apps_from_connections(
    connections: &ElegyPluginConnectionsV1,
    bindings: &BTreeMap<String, CodexConnectionBinding>,
) -> Result<CodexAppsFile, Vec<String>> {
    let mut issues = Vec::new();
    if connections.schema_version != ELEGY_PLUGIN_CONNECTIONS_V1_SCHEMA_VERSION {
        issues.push(format!(
            "connection requirements schemaVersion must be '{}', found '{}'.",
            ELEGY_PLUGIN_CONNECTIONS_V1_SCHEMA_VERSION, connections.schema_version
        ));
    }
    if connections.plugin.trim().is_empty() {
        issues.push("connection requirements plugin must not be empty.".to_string());
    }
    if connections.plugin_version.trim().is_empty() {
        issues.push("connection requirements pluginVersion must not be empty.".to_string());
    }
    if connections.requirements.is_empty() {
        issues.push("connection requirements must contain at least one entry.".to_string());
    }

    let mut seen = BTreeSet::new();
    let mut apps = BTreeMap::new();
    for requirement in &connections.requirements {
        if !validate_kebab_case_name(&requirement.id) {
            issues.push(format!(
                "connection requirement id '{}' must be lowercase kebab-case.",
                requirement.id
            ));
        }
        if !seen.insert(requirement.id.clone()) {
            issues.push(format!(
                "duplicate connection requirement id '{}'.",
                requirement.id
            ));
        }
        if !validate_kebab_case_name(&requirement.service) {
            issues.push(format!(
                "connection requirement '{}' service '{}' must be lowercase kebab-case.",
                requirement.id, requirement.service
            ));
        }
        if requirement.description.trim().is_empty() {
            issues.push(format!(
                "connection requirement '{}' description must not be empty.",
                requirement.id
            ));
        }

        match bindings.get(&requirement.id) {
            Some(binding) if binding.id.trim().is_empty() => issues.push(format!(
                "Codex connection binding '{}' id must not be empty.",
                requirement.id
            )),
            Some(binding) if binding.id == requirement.service => issues.push(format!(
                "Codex connection binding '{}' must use an explicit host-issued app id, not service slug '{}'.",
                requirement.id, requirement.service
            )),
            Some(binding) => {
                apps.insert(
                    requirement.id.clone(),
                    CodexAppReference {
                        id: binding.id.clone(),
                        required: requirement.required,
                        category: None,
                    },
                );
            }
            None => issues.push(format!(
                "connection requirement '{}' has no Codex connection binding.",
                requirement.id
            )),
        }
    }
    for binding_id in bindings.keys() {
        if !seen.contains(binding_id) {
            issues.push(format!(
                "Codex connection binding '{}' has no matching connection requirement.",
                binding_id
            ));
        }
    }

    if issues.is_empty() {
        Ok(CodexAppsFile { apps })
    } else {
        Err(issues)
    }
}

fn load_codex_hooks_config(path: &Path) -> Result<CodexHooksConfig, ToolingError> {
    let content = fs::read_to_string(path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: path.to_path_buf(),
        source,
    })?;

    serde_json::from_str::<CodexHooksConfig>(&content).map_err(|source| ToolingError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn validate_codex_hooks_config(hooks_config: &CodexHooksConfig) -> Vec<String> {
    let mut issues = Vec::new();
    if hooks_config.hooks.is_empty() {
        issues.push("hooks must contain at least one event.".to_string());
    }
    for (event_name, matchers) in &hooks_config.hooks {
        if event_name.trim().is_empty() {
            issues.push("hook event name must not be empty.".to_string());
        }
        if matchers.is_empty() {
            issues.push(format!(
                "hook event '{event_name}' must contain at least one matcher group."
            ));
        }
        for matcher in matchers {
            if matcher.hooks.is_empty() {
                issues.push(format!(
                    "hook event '{event_name}' matcher group must contain at least one handler."
                ));
            }
            for handler in &matcher.hooks {
                if handler.handler_type.trim().is_empty() {
                    issues.push(format!(
                        "hook event '{event_name}' handler type must not be empty."
                    ));
                } else if handler.handler_type != "command" {
                    issues.push(format!(
                        "hook event '{event_name}' handler type '{}' is not supported; use 'command'.",
                        handler.handler_type
                    ));
                }
                if handler.command.trim().is_empty() {
                    issues.push(format!(
                        "hook event '{event_name}' command must not be empty."
                    ));
                }
            }
        }
    }
    issues
}

fn descriptor_validation_issues(descriptor: &McpServerDescriptor) -> Vec<String> {
    validate_mcp_server_descriptor(descriptor).issues
}

fn analyze_descriptor(descriptor: &McpServerDescriptor) -> McpAnalysisResult {
    let mut analysis = McpToolAnalyzer.analyze(descriptor);
    for tool_analysis in &mut analysis.analyses {
        tool_analysis.has_valid_schema = tool_analysis
            .tool
            .input_schema
            .as_ref()
            .is_some_and(is_supported_input_schema);
    }

    analysis
}

fn is_supported_input_schema(value: &Value) -> bool {
    matches!(value, Value::Object(_))
}

// ── Internal helpers ──────────────────────────────────────────────────────

pub(crate) fn write_json_file<T: Serialize>(
    output_path: &Path,
    value: &T,
    overwrite: bool,
) -> Result<(), ToolingError> {
    if output_path.exists() && !overwrite {
        return Err(ToolingError::OutputExists {
            path: output_path.to_path_buf(),
        });
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| ToolingError::Io {
            operation: "create directory",
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let mut content = serde_json::to_string_pretty(value).map_err(|source| ToolingError::Json {
        path: output_path.to_path_buf(),
        source,
    })?;
    content.push('\n');

    fs::write(output_path, content).map_err(|source| ToolingError::Io {
        operation: "write",
        path: output_path.to_path_buf(),
        source,
    })
}

pub(crate) fn display_path(path: &Path) -> String {
    path.display().to_string()
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{
        analyze_mcp_descriptor_file, author_mcp_descriptor_to_path, build_codex_apps_from_catalog,
        build_codex_apps_from_connections, contains_plaintext_authentication_material,
        copy_dir_all, export_plugin_v1, export_plugin_v1_with_codex_mode,
        export_plugin_v1_with_codex_mode_and_binary, export_plugin_with_policy,
        generate_plugin_schema_artifacts, generate_skills_from_descriptor_file,
        import_codex_plugin_v1, import_codex_plugin_v3, inspect_plugin_v1,
        is_safe_package_relative_path, load_capability_catalog,
        migrate_capability_catalog_v1_to_v2, pack_plugin_v1, pack_plugin_v1_with_binary,
        pack_plugin_v3, project_codex_plugin_v3, select_marketplace_artifact,
        validate_elegy_capability_catalog_v1, validate_elegy_capability_catalog_v2,
        validate_elegy_marketplace_v1, validate_elegy_marketplace_v2, validate_elegy_plugin_v1,
        validate_elegy_plugin_v3, verify_plugin_v1, verify_plugin_v3, AuthorMcpDescriptorRequest,
        AuthorMcpToolRequest, CodexConnectionBinding, CodexPluginExtensionV1, CodexProjectionMode,
        ElegyAppBinding, ElegyCapability, ElegyCapabilityCatalog, ElegyCapabilityCatalogV1,
        ElegyCapabilityCatalogV2, ElegyCapabilityFallback, ElegyCapabilityInvocation,
        ElegyCapabilityKind, ElegyCapabilityV2, ElegyConnectionRequirement,
        ElegyMarketplaceArtifact, ElegyMarketplaceAuthenticationPolicy,
        ElegyMarketplaceInstallationPolicy, ElegyMarketplacePlugin, ElegyMarketplacePluginV2,
        ElegyMarketplacePolicy, ElegyMarketplaceSource, ElegyMarketplaceSourceV2,
        ElegyMarketplaceV1, ElegyMarketplaceV2, ElegyPluginConnectionsV1, ElegyPluginV1,
        ElegyPluginV3, ElegyReadinessEvidence, ElegyReadinessEvidenceKind, ElegyReadinessStage,
        ElegyReadinessV1, ElegySideEffectClass, HostProjectionPolicy, McpServerDescriptor,
        McpToolAnalyzer, McpToolDefinition, PluginArchiveBinary, ToolingError,
        ELEGY_CAPABILITY_CATALOG_V1_SCHEMA_VERSION, ELEGY_CAPABILITY_CATALOG_V2_SCHEMA_VERSION,
        ELEGY_MARKETPLACE_V1_SCHEMA_VERSION, ELEGY_MARKETPLACE_V2_SCHEMA_VERSION,
        ELEGY_PLUGIN_V1_SCHEMA_VERSION, ELEGY_READINESS_V1_SCHEMA_VERSION,
    };
    use serde_json::{json, Value};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&dir).expect("create temp directory");
        dir
    }

    fn write_plugin_fixture(root: &Path, name: &str, description: &str, repository: Option<&str>) {
        fs::create_dir_all(root.join(".elegy-plugin")).expect("create manifest dir");
        fs::create_dir_all(root.join("skills").join(name)).expect("create skill dir");

        let mut manifest = json!({
            "schemaVersion": "elegy-plugin/v1",
            "name": name,
            "version": "0.1.0",
            "description": description,
            "author": {"name": "Test Author"},
            "license": "MIT",
            "skills": "./skills/"
        });
        if let Some(repository) = repository {
            manifest["repository"] = json!(repository);
        }

        fs::write(
            root.join(".elegy-plugin").join("plugin.json"),
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");

        fs::write(
            root.join("skills").join(name).join("SKILL.md"),
            format!(
                "---\nname: {name}\ndescription: {description}\n---\n\n# {name}\n\nUse this test fixture skill.\n"
            ),
        )
        .expect("write skill");
    }

    fn readiness_evidence(
        kind: ElegyReadinessEvidenceKind,
        path: &str,
        non_fixture: bool,
    ) -> ElegyReadinessEvidence {
        ElegyReadinessEvidence {
            kind,
            path: path.to_string(),
            summary: "Reviewed evidence.".to_string(),
            non_fixture,
        }
    }

    fn write_concept_readiness(root: &Path, surface: &str) {
        fs::write(
            root.join("readiness.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion": ELEGY_READINESS_V1_SCHEMA_VERSION,
                "surface": surface,
                "surfaceVersion": "0.1.0",
                "stage": "concept",
                "summary": "Test fixture concept.",
                "worksToday": ["Supports the behavior under test."],
                "limitations": ["Not a usable packaged capability."],
                "supportedEnvironments": ["test fixture"],
                "installation": "Not installable.",
                "invocation": "Used only by this test.",
                "evidence": []
            }))
            .expect("serialize readiness"),
        )
        .expect("write readiness");
    }

    #[test]
    fn missing_readiness_is_backward_compatible_but_not_agent_routable() {
        let plugin: ElegyPluginV1 = serde_json::from_value(json!({
            "schemaVersion": "elegy-plugin/v1",
            "name": "legacy-plugin",
            "version": "0.1.0",
            "description": "Legacy plugin.",
            "skills": "./skills/"
        }))
        .expect("legacy plugin parses");

        assert_eq!(plugin.readiness_stage(), ElegyReadinessStage::Implemented);
        assert!(!plugin.is_agent_routable());
    }

    #[test]
    fn v2_plugin_requires_typed_capability_catalog() {
        let plugin: ElegyPluginV1 = serde_json::from_value(json!({
            "schemaVersion": "elegy-plugin/v2",
            "name": "skill-shaped-package",
            "version": "0.1.0",
            "description": "A package with instructions but no executable discovery.",
            "skills": "./skills/",
            "connections": {"requirements": {"mode": "none"}},
            "readiness": {
                "stage": "concept",
                "path": "./readiness.json",
                "schemaVersion": "elegy-readiness/v1"
            }
        }))
        .expect("fixture parses");

        let validation = validate_elegy_plugin_v1(&plugin);

        assert!(validation
            .issues
            .iter()
            .any(|issue| issue.contains("requires capabilityCatalog")));
    }

    #[test]
    fn usable_readiness_requires_clean_install_and_non_fixture_real_task_evidence() {
        let readiness = ElegyReadinessV1 {
            schema_version: ELEGY_READINESS_V1_SCHEMA_VERSION.to_string(),
            surface: "test-plugin".to_string(),
            surface_version: "0.1.0".to_string(),
            stage: ElegyReadinessStage::Usable,
            summary: "Test readiness.".to_string(),
            works_today: vec!["Runs a deterministic command.".to_string()],
            limitations: vec!["Supports one declared environment.".to_string()],
            supported_environments: vec!["x86_64-pc-windows-msvc".to_string()],
            installation: "Install the packaged archive.".to_string(),
            invocation: "Run test-plugin status.".to_string(),
            evidence: vec![
                readiness_evidence(
                    ElegyReadinessEvidenceKind::SourceTests,
                    "./evidence/source-tests.json",
                    false,
                ),
                readiness_evidence(
                    ElegyReadinessEvidenceKind::PackageVerification,
                    "./evidence/package-verification.json",
                    false,
                ),
                readiness_evidence(
                    ElegyReadinessEvidenceKind::CleanInstall,
                    "./evidence/clean-install.json",
                    false,
                ),
                readiness_evidence(
                    ElegyReadinessEvidenceKind::RealTask,
                    "./evidence/real-task.json",
                    false,
                ),
            ],
        };

        let issues = readiness.validation_issues();

        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("real-task") && issue.contains("non-fixture")),
            "{issues:?}"
        );
        assert!(!readiness.is_agent_routable());
    }

    #[test]
    fn production_readiness_requires_release_and_consumer_evidence() {
        let readiness = ElegyReadinessV1 {
            schema_version: ELEGY_READINESS_V1_SCHEMA_VERSION.to_string(),
            surface: "test-plugin".to_string(),
            surface_version: "0.1.0".to_string(),
            stage: ElegyReadinessStage::Production,
            summary: "Test readiness.".to_string(),
            works_today: vec!["Runs a real task.".to_string()],
            limitations: vec!["Supports one declared environment.".to_string()],
            supported_environments: vec!["x86_64-pc-windows-msvc".to_string()],
            installation: "Install the packaged archive.".to_string(),
            invocation: "Run test-plugin status.".to_string(),
            evidence: vec![
                readiness_evidence(
                    ElegyReadinessEvidenceKind::SourceTests,
                    "./evidence/source-tests.json",
                    false,
                ),
                readiness_evidence(
                    ElegyReadinessEvidenceKind::PackageVerification,
                    "./evidence/package-verification.json",
                    false,
                ),
                readiness_evidence(
                    ElegyReadinessEvidenceKind::CleanInstall,
                    "./evidence/clean-install.json",
                    false,
                ),
                readiness_evidence(
                    ElegyReadinessEvidenceKind::RealTask,
                    "./evidence/real-task.json",
                    true,
                ),
            ],
        };

        let issues = readiness.validation_issues();

        assert!(
            issues.iter().any(|issue| issue.contains("release")),
            "{issues:?}"
        );
        assert!(
            issues.iter().any(|issue| issue.contains("consumer")),
            "{issues:?}"
        );
        assert!(!readiness.is_agent_routable());
    }

    #[test]
    fn plugin_v2_requires_a_safe_readiness_reference() {
        let mut plugin: ElegyPluginV1 = serde_json::from_value(json!({
            "schemaVersion": "elegy-plugin/v2",
            "name": "test-plugin",
            "version": "0.1.0",
            "description": "Test plugin.",
            "skills": "./skills/",
            "connections": {"requirements": {"mode": "none"}}
        }))
        .expect("plugin parses");

        let missing = validate_elegy_plugin_v1(&plugin);
        assert!(
            missing
                .issues
                .iter()
                .any(|issue| issue.contains("requires readiness")),
            "{:?}",
            missing.issues
        );

        plugin.readiness = Some(super::ElegyPluginReadiness {
            stage: ElegyReadinessStage::Implemented,
            path: "../readiness.json".to_string(),
            schema_version: ELEGY_READINESS_V1_SCHEMA_VERSION.to_string(),
        });
        let unsafe_path = validate_elegy_plugin_v1(&plugin);
        assert!(
            unsafe_path
                .issues
                .iter()
                .any(|issue| issue.contains("readiness path") && issue.contains("safe")),
            "{:?}",
            unsafe_path.issues
        );
    }

    #[test]
    fn plugin_verification_rejects_readiness_that_does_not_match_manifest() {
        let plugin_dir = unique_temp_dir("plugin-readiness-mismatch");
        write_plugin_fixture(&plugin_dir, "test-plugin", "Test plugin.", None);
        let manifest_path = plugin_dir.join(".elegy-plugin").join("plugin.json");
        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest["schemaVersion"] = json!("elegy-plugin/v2");
        manifest["connections"] = json!({"requirements": {"mode": "none"}});
        manifest["readiness"] = json!({
            "stage": "usable",
            "path": "./readiness.json",
            "schemaVersion": ELEGY_READINESS_V1_SCHEMA_VERSION
        });
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");
        fs::write(
            plugin_dir.join("readiness.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion": ELEGY_READINESS_V1_SCHEMA_VERSION,
                "surface": "different-plugin",
                "surfaceVersion": "0.1.0",
                "stage": "implemented",
                "summary": "Implemented only.",
                "worksToday": ["Runs tests."],
                "limitations": ["No clean-install proof."],
                "supportedEnvironments": ["source-checkout"],
                "installation": "Build from source.",
                "invocation": "Run the source binary.",
                "evidence": [
                    {"kind":"source-tests","path":"./evidence/source-tests.json","summary":"Tests passed."},
                    {"kind":"package-verification","path":"./evidence/package-verification.json","summary":"Package verified."}
                ]
            }))
            .expect("serialize readiness"),
        )
        .expect("write readiness");

        let result = verify_plugin_v1(&plugin_dir.join(".elegy-plugin"))
            .expect("verification returns issues");

        assert!(
            !result.valid
                && result
                    .issues
                    .iter()
                    .any(|issue| issue.contains("readiness surface")
                        || issue.contains("readiness stage")),
            "{:?}",
            result.issues
        );
        fs::remove_dir_all(plugin_dir).ok();
    }

    #[test]
    fn generated_plugin_schemas_match_checked_in_artifacts() {
        let schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas");
        let artifacts = generate_plugin_schema_artifacts().expect("generate plugin schemas");

        for (file_name, expected) in artifacts {
            let path = schema_dir.join(file_name);
            let actual = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert_eq!(
                actual.replace("\r\n", "\n"),
                expected.replace("\r\n", "\n"),
                "schema drift: {}",
                path.display()
            );
        }
    }

    #[test]
    fn generated_plugin_schemas_include_connection_contracts() {
        let artifacts = generate_plugin_schema_artifacts().expect("generate plugin schemas");

        assert!(artifacts.contains_key("elegy-plugin-v2.schema.json"));
        assert!(artifacts.contains_key("elegy-plugin-connections-v1.schema.json"));
        assert!(artifacts.contains_key("elegy-connection-provider-v1.schema.json"));
        assert!(artifacts.contains_key("elegy-readiness-v1.schema.json"));
        let v2: Value = serde_json::from_str(
            artifacts
                .get("elegy-plugin-v2.schema.json")
                .expect("v2 schema"),
        )
        .expect("parse v2 schema");
        let required = v2["required"].as_array().expect("required array");
        for field in ["capabilityCatalog", "connections", "readiness"] {
            assert!(
                required.iter().any(|value| value == field),
                "v2 schema must require {field}"
            );
        }
    }

    #[test]
    fn package_relative_paths_use_portable_dot_slash_form() {
        for valid in ["./skills", "./skills/", "./.app.json", "./assets/logo.png"] {
            assert!(is_safe_package_relative_path(valid), "{valid}");
        }
        for invalid in [
            "",
            ".",
            "./",
            "skills/",
            "../skills",
            "./../skills",
            "./skills/../other",
            "./skills//nested",
            "./skills\\nested",
            "/skills",
            "C:/skills",
            "./C:/skills",
        ] {
            assert!(!is_safe_package_relative_path(invalid), "{invalid}");
        }
    }

    #[test]
    fn plugin_validation_allows_root_skill_only_path() {
        let plugin = ElegyPluginV1 {
            schema_version: ELEGY_PLUGIN_V1_SCHEMA_VERSION.to_string(),
            name: "skill-only-plugin".to_string(),
            version: "0.1.0".to_string(),
            description: "Skill-only fixture.".to_string(),
            skills: Some("./".to_string()),
            ..ElegyPluginV1::default()
        };

        let validation = validate_elegy_plugin_v1(&plugin);

        assert!(validation.is_valid(), "{:?}", validation.issues);
    }

    #[test]
    fn plugin_v2_requires_an_explicit_connection_declaration() {
        let plugin: ElegyPluginV1 = serde_json::from_value(json!({
            "schemaVersion": "elegy-plugin/v2",
            "name": "connected-plugin",
            "version": "0.1.0",
            "description": "A plugin whose authentication posture must be explicit.",
            "skills": "./skills/"
        }))
        .expect("v2 manifest should deserialize");

        let validation = validate_elegy_plugin_v1(&plugin);

        assert!(
            validation
                .issues
                .iter()
                .any(|issue| issue.contains("connections.requirements")),
            "{:?}",
            validation.issues
        );
    }

    #[test]
    fn plugin_v2_accepts_an_explicit_connectionless_declaration() {
        let plugin: ElegyPluginV1 = serde_json::from_value(json!({
            "schemaVersion": "elegy-plugin/v2",
            "name": "local-plugin",
            "version": "0.1.0",
            "description": "A deliberately local-only plugin.",
            "skills": "./skills/",
            "capabilityCatalog": {
                "path": "./capability-catalog.json",
                "schemaVersion": "elegy-capability-catalog/v1"
            },
            "connections": {
                "requirements": {
                    "mode": "none"
                }
            },
            "readiness": {
                "stage": "concept",
                "path": "./readiness.json",
                "schemaVersion": ELEGY_READINESS_V1_SCHEMA_VERSION
            }
        }))
        .expect("v2 manifest should deserialize");

        let validation = validate_elegy_plugin_v1(&plugin);

        assert!(validation.is_valid(), "{:?}", validation.issues);
    }

    #[test]
    fn plugin_v2_verification_requires_the_declared_connection_file() {
        let plugin_dir = unique_temp_dir("plugin-v2-missing-connections");
        write_plugin_fixture(
            &plugin_dir,
            "connected-plugin",
            "Connected plugin fixture.",
            None,
        );
        let manifest_path = plugin_dir.join(".elegy-plugin").join("plugin.json");
        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest["schemaVersion"] = json!("elegy-plugin/v2");
        manifest["connections"] = json!({
            "requirements": {
                "mode": "declared",
                "path": "./connections.json",
                "schemaVersion": "elegy-plugin-connections/v1"
            }
        });
        manifest["readiness"] = json!({
            "stage": "concept",
            "path": "./readiness.json",
            "schemaVersion": ELEGY_READINESS_V1_SCHEMA_VERSION
        });
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");

        let result = verify_plugin_v1(&plugin_dir.join(".elegy-plugin"))
            .expect("verification should report package issues");

        assert!(
            !result.valid
                && result
                    .issues
                    .iter()
                    .any(|issue| issue.contains("declared connection requirements file")),
            "{:?}",
            result.issues
        );
        fs::remove_dir_all(&plugin_dir).ok();
    }

    #[test]
    fn plugin_v2_inspection_reports_connection_requirements() {
        let plugin_dir = unique_temp_dir("plugin-v2-inspect-connections");
        write_plugin_fixture(
            &plugin_dir,
            "connected-plugin",
            "Connected plugin fixture.",
            None,
        );
        let manifest_path = plugin_dir.join(".elegy-plugin").join("plugin.json");
        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest["schemaVersion"] = json!("elegy-plugin/v2");
        manifest["connections"] = json!({
            "requirements": {
                "mode": "declared",
                "path": "./connections.json",
                "schemaVersion": "elegy-plugin-connections/v1"
            }
        });
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");
        fs::write(
            plugin_dir.join("connections.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion": "elegy-plugin-connections/v1",
                "plugin": "connected-plugin",
                "pluginVersion": "0.1.0",
                "requirements": [{
                    "id": "github-main",
                    "service": "github",
                    "required": true,
                    "description": "Access GitHub."
                }]
            }))
            .expect("serialize connections"),
        )
        .expect("write connections");
        write_concept_readiness(&plugin_dir, "test-app-binding");

        let inspection =
            inspect_plugin_v1(&plugin_dir.join(".elegy-plugin")).expect("inspect plugin");

        assert_eq!(inspection["connectionMode"], "declared");
        assert_eq!(inspection["connectionRequirementCount"], 1);
        assert_eq!(inspection["requiredConnectionCount"], 1);
        fs::remove_dir_all(&plugin_dir).ok();
    }

    #[test]
    fn plugin_v2_verification_rejects_mismatched_connection_authority() {
        let plugin_dir = unique_temp_dir("plugin-v2-mismatched-connections");
        write_plugin_fixture(
            &plugin_dir,
            "connected-plugin",
            "Connected plugin fixture.",
            None,
        );
        let manifest_path = plugin_dir.join(".elegy-plugin").join("plugin.json");
        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest["schemaVersion"] = json!("elegy-plugin/v2");
        manifest["connections"] = json!({
            "requirements": {
                "mode": "declared",
                "path": "./connections.json",
                "schemaVersion": "elegy-plugin-connections/v1"
            }
        });
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");
        fs::write(
            plugin_dir.join("connections.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion": "elegy-plugin-connections/v1",
                "plugin": "different-plugin",
                "pluginVersion": "9.9.9",
                "requirements": [{
                    "id": "github-main",
                    "service": "github",
                    "required": true,
                    "description": "Access GitHub."
                }]
            }))
            .expect("serialize connections"),
        )
        .expect("write connections");

        let result = verify_plugin_v1(&plugin_dir.join(".elegy-plugin"))
            .expect("verification should report authority mismatch");

        assert!(
            !result.valid
                && result
                    .issues
                    .iter()
                    .any(|issue| issue.contains("does not match manifest plugin")),
            "{:?}",
            result.issues
        );
        fs::remove_dir_all(&plugin_dir).ok();
    }

    #[test]
    fn plugin_v2_verification_validates_the_connection_provider_descriptor() {
        let plugin_dir = unique_temp_dir("plugin-v2-invalid-provider");
        write_plugin_fixture(
            &plugin_dir,
            "connection-provider",
            "Connection provider fixture.",
            None,
        );
        let manifest_path = plugin_dir.join(".elegy-plugin").join("plugin.json");
        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest["schemaVersion"] = json!("elegy-plugin/v2");
        manifest["connections"] = json!({
            "requirements": {"mode": "none"},
            "provider": {
                "path": "./connection-provider.json",
                "schemaVersion": "elegy-connection-provider/v1"
            }
        });
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");
        fs::write(
            plugin_dir.join("connection-provider.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion": "elegy-connection-provider/v0",
                "id": "connection-provider",
                "controlProtocol": "wrong-protocol",
                "invocation": {"executable": "", "command": []}
            }))
            .expect("serialize provider"),
        )
        .expect("write provider");

        let result = verify_plugin_v1(&plugin_dir.join(".elegy-plugin"))
            .expect("verification should report invalid provider");

        assert!(
            !result.valid
                && result
                    .issues
                    .iter()
                    .any(|issue| issue.contains("connection provider")
                        && issue.contains("controlProtocol")),
            "{:?}",
            result.issues
        );
        fs::remove_dir_all(&plugin_dir).ok();
    }

    #[test]
    fn plugin_v2_verification_requires_codex_bindings_for_projected_connections() {
        let plugin_dir = unique_temp_dir("plugin-v2-missing-codex-binding");
        write_plugin_fixture(
            &plugin_dir,
            "connected-plugin",
            "Connected plugin fixture.",
            None,
        );
        let manifest_path = plugin_dir.join(".elegy-plugin").join("plugin.json");
        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest["schemaVersion"] = json!("elegy-plugin/v2");
        manifest["connections"] = json!({
            "requirements": {
                "mode": "declared",
                "path": "./connections.json",
                "schemaVersion": "elegy-plugin-connections/v1"
            }
        });
        manifest["extensions"] = json!({
            "codex.plugin/v1": {
                "schemaVersion": "codex.plugin/v1"
            }
        });
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");
        fs::write(
            plugin_dir.join("connections.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion": "elegy-plugin-connections/v1",
                "plugin": "connected-plugin",
                "pluginVersion": "0.1.0",
                "requirements": [{
                    "id": "github-main",
                    "service": "github",
                    "required": true,
                    "description": "Access GitHub."
                }]
            }))
            .expect("serialize connections"),
        )
        .expect("write connections");

        let result = verify_plugin_v1(&plugin_dir.join(".elegy-plugin"))
            .expect("verification should report missing binding");

        assert!(
            !result.valid
                && result
                    .issues
                    .iter()
                    .any(|issue| issue.contains("has no Codex connection binding")),
            "{:?}",
            result.issues
        );
        fs::remove_dir_all(&plugin_dir).ok();
    }

    #[test]
    fn marketplace_validation_and_target_selection_are_deterministic() {
        let marketplace = ElegyMarketplaceV1 {
            schema_version: ELEGY_MARKETPLACE_V1_SCHEMA_VERSION.to_string(),
            name: "elegy".to_string(),
            interface: None,
            plugins: vec![ElegyMarketplacePlugin {
                name: "elegy-planning".to_string(),
                source: ElegyMarketplaceSource {
                    source: "local".to_string(),
                    path: "./plugins/planning".to_string(),
                },
                category: "Developer Tools".to_string(),
                artifacts: vec![
                    ElegyMarketplaceArtifact {
                        target: "any".to_string(),
                        url: "https://example.com/portable.zip".to_string(),
                        checksum_url: "https://example.com/portable.zip.sha256".to_string(),
                    },
                    ElegyMarketplaceArtifact {
                        target: "x86_64-pc-windows-msvc".to_string(),
                        url: "https://example.com/windows.zip".to_string(),
                        checksum_url: "https://example.com/windows.zip.sha256".to_string(),
                    },
                ],
            }],
        };

        assert!(validate_elegy_marketplace_v1(&marketplace).is_valid());
        let plugin = &marketplace.plugins[0];
        assert_eq!(
            select_marketplace_artifact(plugin, "x86_64-pc-windows-msvc")
                .map(|artifact| artifact.target.as_str()),
            Some("x86_64-pc-windows-msvc")
        );
        assert_eq!(
            select_marketplace_artifact(plugin, "aarch64-apple-darwin")
                .map(|artifact| artifact.target.as_str()),
            Some("any")
        );
    }

    #[test]
    fn marketplace_validation_rejects_unsafe_and_duplicate_entries() {
        let marketplace = ElegyMarketplaceV1 {
            schema_version: ELEGY_MARKETPLACE_V1_SCHEMA_VERSION.to_string(),
            name: "elegy".to_string(),
            interface: None,
            plugins: vec![
                ElegyMarketplacePlugin {
                    name: "plugin".to_string(),
                    source: ElegyMarketplaceSource {
                        source: "local".to_string(),
                        path: "./../escape".to_string(),
                    },
                    category: String::new(),
                    artifacts: Vec::new(),
                },
                ElegyMarketplacePlugin {
                    name: "plugin".to_string(),
                    source: ElegyMarketplaceSource {
                        source: "git".to_string(),
                        path: "./plugins/plugin".to_string(),
                    },
                    category: "Other".to_string(),
                    artifacts: Vec::new(),
                },
            ],
        };

        let result = validate_elegy_marketplace_v1(&marketplace);
        assert!(!result.is_valid());
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.contains("duplicate plugin name")));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.contains("source.path")));
    }

    #[test]
    fn marketplace_allows_no_plugins_when_no_surface_is_agent_routable() {
        let marketplace = ElegyMarketplaceV1 {
            schema_version: ELEGY_MARKETPLACE_V1_SCHEMA_VERSION.to_string(),
            name: "elegy".to_string(),
            interface: None,
            plugins: Vec::new(),
        };

        let result = validate_elegy_marketplace_v1(&marketplace);

        assert!(result.is_valid(), "{:?}", result.issues);
    }

    #[test]
    fn analyze_tool_with_valid_schema_extracts_triggers_and_marks_valid() {
        let analyzer = McpToolAnalyzer;
        let descriptor = McpServerDescriptor {
            server_name: "test-server".to_string(),
            tools: vec![McpToolDefinition {
                name: "get-user".to_string(),
                description: Some("Gets a user".to_string()),
                input_schema: Some(json!({ "type": "object" })),
            }],
            ..McpServerDescriptor::default()
        };

        let result = analyzer.analyze(&descriptor);

        assert_eq!(result.server_name, "test-server");
        assert_eq!(result.analyses.len(), 1);
        assert!(result.analyses[0].has_valid_schema);
        assert_eq!(result.analyses[0].extracted_triggers.len(), 1);
        assert_eq!(result.analyses[0].extracted_triggers[0].pattern, "get user");
        assert_eq!(
            result.analyses[0].extracted_triggers[0]
                .description
                .as_deref(),
            Some("Extracted from MCP tool name")
        );
    }

    #[test]
    fn analyze_tool_without_schema_marks_invalid() {
        let analyzer = McpToolAnalyzer;
        let descriptor = McpServerDescriptor {
            server_name: "no-schema-server".to_string(),
            tools: vec![McpToolDefinition {
                name: "listItems".to_string(),
                description: Some("Lists items".to_string()),
                ..McpToolDefinition::default()
            }],
            ..McpServerDescriptor::default()
        };

        let result = analyzer.analyze(&descriptor);

        assert!(!result.analyses[0].has_valid_schema);
        assert_eq!(
            result.analyses[0].extracted_triggers[0].pattern,
            "list items"
        );
    }

    #[test]
    fn analyze_mixed_tools_returns_correct_count_and_results() {
        let analyzer = McpToolAnalyzer;
        let descriptor = McpServerDescriptor {
            server_name: "mixed-server".to_string(),
            tools: vec![
                McpToolDefinition {
                    name: "get-user".to_string(),
                    input_schema: Some(json!({ "type": "object" })),
                    ..McpToolDefinition::default()
                },
                McpToolDefinition {
                    name: "create_item".to_string(),
                    description: Some("Creates an item".to_string()),
                    ..McpToolDefinition::default()
                },
                McpToolDefinition {
                    name: "fetchOrderDetails".to_string(),
                    input_schema: Some(json!({ "type": "object" })),
                    ..McpToolDefinition::default()
                },
            ],
            ..McpServerDescriptor::default()
        };

        let result = analyzer.analyze(&descriptor);

        assert_eq!(result.server_name, "mixed-server");
        assert_eq!(result.analyses.len(), 3);
        assert!(result.analyses[0].has_valid_schema);
        assert_eq!(result.analyses[0].extracted_triggers[0].pattern, "get user");
        assert!(!result.analyses[1].has_valid_schema);
        assert_eq!(
            result.analyses[1].extracted_triggers[0].pattern,
            "create item"
        );
        assert!(result.analyses[2].has_valid_schema);
        assert_eq!(
            result.analyses[2].extracted_triggers[0].pattern,
            "fetch order details"
        );
    }

    #[test]
    fn author_mcp_descriptor_writes_valid_json() {
        let temp_dir = unique_temp_dir("elegy-tooling-author");
        let output_path = temp_dir.join("weather-mcp.json");

        let result = author_mcp_descriptor_to_path(
            AuthorMcpDescriptorRequest {
                server_name: "weather-server".to_string(),
                transport: super::McpTransportKind::Stdio,
                tools: vec![
                    AuthorMcpToolRequest {
                        name: "get-weather".to_string(),
                        description: Some("Look up a weather report".to_string()),
                    },
                    AuthorMcpToolRequest {
                        name: "list-alerts".to_string(),
                        description: None,
                    },
                ],
            },
            &output_path,
            false,
        )
        .expect("authoring should succeed");

        assert_eq!(result.descriptor.server_name, "weather-server");
        assert_eq!(result.descriptor.tools.len(), 2);
        assert!(output_path.is_file());

        let persisted = fs::read_to_string(&output_path).expect("read descriptor file");
        let parsed: McpServerDescriptor =
            serde_json::from_str(&persisted).expect("parse descriptor file");
        let validation = super::validate_mcp_server_descriptor(&parsed);
        assert!(
            validation.is_valid(),
            "unexpected issues: {:?}",
            validation.issues
        );
        assert!(
            parsed.tools.iter().all(|tool| tool.input_schema.is_none()),
            "authored MCP descriptors should not fabricate tool schemas"
        );
    }

    #[test]
    fn analyze_and_generate_skills_from_descriptor_file() {
        let temp_dir = unique_temp_dir("elegy-tooling-generate");
        let descriptor_path = temp_dir.join("weather-mcp.json");
        let output_dir = temp_dir.join("generated-skills");

        fs::write(
            &descriptor_path,
            r#"{
    "serverName": "weather-server",
    "transport": "stdio",
    "tools": [
        {
            "name": "get-weather",
            "description": "Look up a weather report",
            "inputSchema": { "type": "object" }
        },
        {
            "name": "list-alerts",
            "description": "List active weather alerts"
        }
    ]
}
"#,
        )
        .expect("write descriptor fixture");

        let analysis = analyze_mcp_descriptor_file(&descriptor_path)
            .expect("analysis should succeed for valid descriptor");
        assert_eq!(analysis.server_name, "weather-server");
        assert_eq!(analysis.analyses.len(), 2);

        let generated =
            generate_skills_from_descriptor_file(&descriptor_path, Some(&output_dir), false)
                .expect("skill generation should succeed");
        assert_eq!(generated.generated_skills.len(), 1);
        assert_eq!(
            generated.generated_skills[0].skill_name,
            "mcp-weather-server-get-weather"
        );
        assert_eq!(generated.skipped_tools.len(), 1);
        assert_eq!(generated.written_files.len(), 1);
        assert!(output_dir
            .join("mcp-weather-server-get-weather")
            .join("SKILL.md")
            .is_file());
    }

    #[test]
    fn verify_inspect_plugin_v1_fixture() {
        let temp_dir = unique_temp_dir("elegy-plugin-v1");
        let output_dir = temp_dir.join("my-plugin");

        write_plugin_fixture(
            &output_dir,
            "my-plugin",
            "Test plugin for verification",
            Some("https://github.com/example/my-plugin"),
        );

        let verify_result = verify_plugin_v1(&output_dir.join(".elegy-plugin"))
            .expect("verification should succeed");
        assert!(verify_result.valid, "plugin should be valid");
        assert_eq!(verify_result.plugin_name, "my-plugin");
        assert_eq!(verify_result.plugin_version, "0.1.0");
        assert!(verify_result.has_skills);
        assert_eq!(verify_result.skill_count, 1);

        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(output_dir.join(".elegy-plugin").join("plugin.json"))
                .expect("read scaffold manifest"),
        )
        .expect("parse scaffold manifest");
        assert!(manifest.get("mcpServers").is_none());
        assert!(manifest.get("extensions").is_none());

        let inspect_result =
            inspect_plugin_v1(&output_dir.join(".elegy-plugin")).expect("inspect should succeed");
        assert_eq!(inspect_result["name"], "my-plugin");
    }

    #[test]
    fn export_plugin_v1_opencode() {
        let temp_dir = unique_temp_dir("elegy-export-opencode");
        let plugin_dir = temp_dir.join("my-plugin");

        write_plugin_fixture(&plugin_dir, "my-plugin", "Test plugin for export", None);

        let export_dir = temp_dir.join("exported");
        let result = export_plugin_v1(&plugin_dir, "opencode", &export_dir, false)
            .expect("export should succeed");

        assert_eq!(result.plugin_name, "my-plugin");
        assert_eq!(result.emitted_components.skills_count, 1);
        assert!(!result.written_files.is_empty());
        assert!(export_dir
            .join("skills")
            .join("my-plugin")
            .join("SKILL.md")
            .exists());
    }

    #[test]
    fn host_exports_preserve_portable_manifest_and_capability_catalog() {
        let temp_dir = unique_temp_dir("elegy-export-portable-core");
        let plugin_dir = temp_dir.join("my-adapter");
        write_plugin_fixture(
            &plugin_dir,
            "my-adapter",
            "Typed adapter export fixture",
            None,
        );
        let manifest_path = plugin_dir.join(".elegy-plugin").join("plugin.json");
        let mut manifest: Value = serde_json::from_str(
            &fs::read_to_string(&manifest_path).expect("read fixture manifest"),
        )
        .expect("parse fixture manifest");
        manifest["capabilityCatalog"] = json!({
            "path": "./capability-catalog.json",
            "schemaVersion": "elegy-capability-catalog/v1"
        });
        manifest["extensions"] = json!({
            "codex.plugin/v1": {
                "schemaVersion": "codex.plugin/v1",
                "interface": {
                    "displayName": "My Adapter",
                    "shortDescription": "Typed adapter export fixture",
                    "longDescription": "Verifies that host projections preserve the portable adapter core.",
                    "developerName": "Test Author",
                    "category": "Developer Tools",
                    "capabilities": ["Read"],
                    "defaultPrompt": ["Read from the adapted system."]
                }
            }
        });
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");
        fs::write(
            plugin_dir.join("capability-catalog.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion": "elegy-capability-catalog/v1",
                "plugin": "my-adapter",
                "pluginVersion": "0.1.0",
                "capabilities": [{
                    "id": "example.read",
                    "kind": "cli",
                    "sideEffectClass": "query",
                    "contractVersion": "v1",
                    "description": "Read from the adapted system.",
                    "invocation": {
                        "executable": "my-adapter",
                        "command": ["read"]
                    }
                }]
            }))
            .expect("serialize catalog"),
        )
        .expect("write catalog");

        for host in ["codex", "opencode", "claude"] {
            let export_dir = temp_dir.join(host);
            export_plugin_v1(&plugin_dir, host, &export_dir, false).expect("export should succeed");
            assert!(export_dir
                .join(".elegy-plugin")
                .join("plugin.json")
                .is_file());
            assert!(export_dir.join("capability-catalog.json").is_file());
        }
    }

    #[test]
    fn export_plugin_v1_wraps_a_root_skill_with_its_support_files() {
        let temp_dir = unique_temp_dir("elegy-export-root-skill");
        let plugin_dir = temp_dir.join("root-skill-plugin");
        fs::create_dir_all(plugin_dir.join(".elegy-plugin")).expect("create manifest dir");
        fs::create_dir_all(plugin_dir.join("references")).expect("create references dir");
        fs::write(
            plugin_dir.join(".elegy-plugin").join("plugin.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion": "elegy-plugin/v1",
                "name": "root-skill-plugin",
                "version": "0.1.0",
                "description": "Root skill export fixture",
                "author": {"name": "Test Author"},
                "license": "MIT",
                "skills": "./",
                "extensions": {
                    "codex.plugin/v1": {
                        "schemaVersion": "codex.plugin/v1",
                        "interface": {
                            "displayName": "Root Skill Plugin",
                            "shortDescription": "Root skill export fixture",
                            "longDescription": "Verifies that a root skill and its support files remain one Codex skill.",
                            "developerName": "Test Author",
                            "category": "Developer Tools",
                            "capabilities": ["Read"],
                            "defaultPrompt": ["Use the root skill fixture."]
                        }
                    }
                }
            }))
            .expect("serialize manifest"),
        )
        .expect("write manifest");
        fs::write(
            plugin_dir.join("SKILL.md"),
            "---\nname: root-skill-plugin\ndescription: Root skill export fixture\ndisable-model-invocation: true\n---\n\n# Root skill\n",
        )
        .expect("write skill");
        fs::write(plugin_dir.join("references").join("guide.md"), "# Guide\n")
            .expect("write reference");
        fs::write(
            plugin_dir.join("install-receipt.json"),
            r#"{"installedAt":"volatile"}"#,
        )
        .expect("write generated install receipt");

        let export_dir = temp_dir.join("exported");
        let result = export_plugin_v1(&plugin_dir, "codex", &export_dir, false)
            .expect("export should succeed");

        assert_eq!(result.emitted_components.skills_count, 1);
        assert!(export_dir
            .join("skills")
            .join("root-skill-plugin")
            .join("SKILL.md")
            .is_file());
        assert!(export_dir
            .join("skills")
            .join("root-skill-plugin")
            .join("references")
            .join("guide.md")
            .is_file());
        assert!(!export_dir.join("skills").join("references").exists());
        assert!(!export_dir
            .join("skills")
            .join("root-skill-plugin")
            .join(".elegy-plugin")
            .exists());
        assert!(!export_dir
            .join("skills")
            .join("root-skill-plugin")
            .join("install-receipt.json")
            .exists());
        let exported_skill = fs::read_to_string(
            export_dir
                .join("skills")
                .join("root-skill-plugin")
                .join("SKILL.md"),
        )
        .expect("read exported skill");
        assert!(exported_skill.contains("disable-model-invocation: false"));
        assert!(!exported_skill.contains("disable-model-invocation: true"));
    }

    #[test]
    fn export_plugin_v1_includes_explicit_binary() {
        let temp_dir = unique_temp_dir("elegy-export-binary");
        let plugin_dir = temp_dir.join("my-plugin");
        write_plugin_fixture(
            &plugin_dir,
            "my-plugin",
            "Test plugin for binary export",
            None,
        );
        let binary = temp_dir.join("my-plugin.exe");
        fs::write(&binary, b"binary").expect("write binary");

        let export_dir = temp_dir.join("exported");
        let result = export_plugin_v1_with_codex_mode_and_binary(
            &plugin_dir,
            "opencode",
            &export_dir,
            false,
            CodexProjectionMode::Current,
            Some(PluginArchiveBinary {
                source_path: &binary,
                archive_path: "bin/my-plugin.exe".to_string(),
            }),
        )
        .expect("binary export should succeed");

        assert!(export_dir.join("bin").join("my-plugin.exe").is_file());
        assert!(result
            .written_files
            .iter()
            .any(|path| path.ends_with("my-plugin.exe")));
    }

    #[test]
    fn export_plugin_v1_codex_version_digest_changes_when_projection_changes() {
        let temp_dir = unique_temp_dir("elegy-export-codex-digest");
        let plugin_dir = temp_dir.join("my-plugin");
        write_plugin_fixture(
            &plugin_dir,
            "my-plugin",
            "Test plugin for Codex digest",
            Some("https://example.com/my-plugin"),
        );
        let manifest_path = plugin_dir.join(".elegy-plugin").join("plugin.json");
        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest["extensions"] = json!({
            "codex.plugin/v1": {
                "schemaVersion": "codex.plugin/v1",
                "interface": {
                    "displayName": "My Plugin",
                    "shortDescription": "Test digest",
                    "longDescription": "Test digest changes.",
                    "developerName": "Test Author",
                    "category": "Developer Tools",
                    "capabilities": ["Read"],
                    "defaultPrompt": ["Use My Plugin."]
                }
            }
        });
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");

        let binary_path = temp_dir.join("my-plugin.exe");
        fs::write(&binary_path, b"first").expect("write binary");
        let first_output = temp_dir.join("codex-first");
        export_plugin_v1_with_codex_mode_and_binary(
            &plugin_dir,
            "codex",
            &first_output,
            false,
            CodexProjectionMode::Current,
            Some(PluginArchiveBinary {
                source_path: &binary_path,
                archive_path: "bin/my-plugin.exe".to_string(),
            }),
        )
        .expect("first export");
        let first_manifest: Value = serde_json::from_str(
            &fs::read_to_string(first_output.join(".codex-plugin").join("plugin.json"))
                .expect("read first manifest"),
        )
        .expect("parse first manifest");

        fs::write(
            plugin_dir.join("skills").join("my-plugin").join("SKILL.md"),
            "---\nname: my-plugin\ndescription: changed\n---\n\n# Changed\n",
        )
        .expect("change skill");
        fs::write(&binary_path, b"second").expect("change binary");
        let second_output = temp_dir.join("codex-second");
        export_plugin_v1_with_codex_mode_and_binary(
            &plugin_dir,
            "codex",
            &second_output,
            false,
            CodexProjectionMode::Current,
            Some(PluginArchiveBinary {
                source_path: &binary_path,
                archive_path: "bin/my-plugin.exe".to_string(),
            }),
        )
        .expect("second export");
        let second_manifest: Value = serde_json::from_str(
            &fs::read_to_string(second_output.join(".codex-plugin").join("plugin.json"))
                .expect("read second manifest"),
        )
        .expect("parse second manifest");

        let first_version = first_manifest["version"].as_str().expect("first version");
        let second_version = second_manifest["version"].as_str().expect("second version");
        assert!(first_version.starts_with("0.1.0+codex."));
        assert!(second_version.starts_with("0.1.0+codex."));
        assert_ne!(first_version, second_version);
    }

    #[test]
    fn export_plugin_v1_codex_emits_apps_hooks_interface_and_assets() {
        let temp_dir = unique_temp_dir("elegy-export-codex");
        let plugin_dir = temp_dir.join("github-plugin");

        write_plugin_fixture(
            &plugin_dir,
            "github-plugin",
            "Test plugin for Codex export",
            Some("https://github.com/example/github-plugin"),
        );

        fs::create_dir_all(plugin_dir.join("assets")).expect("create assets");
        fs::write(plugin_dir.join("assets").join("logo.png"), b"logo").expect("write logo");
        fs::write(
            plugin_dir.join(".app.json"),
            r#"{"apps":{"google_drive":{"id":"connector_test","category":"Productivity"}}}"#,
        )
        .expect("write apps");
        fs::create_dir_all(plugin_dir.join("hooks")).expect("create hooks");
        fs::write(
            plugin_dir.join("hooks").join("hooks.json"),
            r#"{"hooks":{"SessionStart":[{"matcher":"startup","hooks":[{"type":"command","command":"echo ok","statusMessage":"Starting"}]}]}}"#,
        )
        .expect("write hooks");

        let manifest_path = plugin_dir.join(".elegy-plugin").join("plugin.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest["extensions"]["codex.plugin/v1"] = json!({
            "schemaVersion": "codex.plugin/v1",
            "homepage": "https://github.com/",
            "keywords": ["github", "pull-request"],
            "futureField": {"preserved": true},
            "apps": "./.app.json",
            "hooks": "./hooks/hooks.json",
            "assets": ["./assets/logo.png"],
            "interface": {
                "displayName": "GitHub",
                "shortDescription": "Work with GitHub",
                "longDescription": "Work with GitHub repositories and pull requests.",
                "developerName": "OpenAI",
                "category": "Developer Tools",
                "capabilities": ["Interactive", "Write"],
                "websiteURL": "https://github.com/",
                "defaultPrompt": ["Inspect a pull request"],
                "logo": "./assets/logo.png",
                "screenshots": []
            }
        });
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");

        let verify_result =
            verify_plugin_v1(&plugin_dir.join(".elegy-plugin")).expect("verify should succeed");
        assert!(
            verify_result.valid,
            "unexpected issues: {:?}",
            verify_result.issues
        );
        assert!(verify_result.has_apps);
        assert_eq!(verify_result.app_count, 1);
        assert!(verify_result.has_hooks);
        assert_eq!(verify_result.hook_event_count, 1);
        assert!(verify_result.has_codex_interface);

        let export_dir = temp_dir.join("exported");
        let result = export_plugin_v1_with_codex_mode(
            &plugin_dir,
            "codex",
            &export_dir,
            false,
            CodexProjectionMode::Experimental,
        )
        .expect("experimental export should succeed");

        assert!(result.emitted_components.apps_emitted);
        assert!(result.emitted_components.hooks_emitted);
        assert!(export_dir.join(".app.json").is_file());
        assert!(export_dir.join("hooks").join("hooks.json").is_file());
        assert!(export_dir.join("assets").join("logo.png").is_file());

        let codex_manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(export_dir.join(".codex-plugin").join("plugin.json"))
                .expect("read Codex manifest"),
        )
        .expect("parse Codex manifest");
        assert_eq!(codex_manifest["apps"], "./.app.json");
        assert_eq!(codex_manifest["hooks"], "./hooks/hooks.json");
        assert_eq!(codex_manifest["interface"]["displayName"], "GitHub");
        assert_eq!(codex_manifest["keywords"][0], "github");
        assert_eq!(codex_manifest["futureField"]["preserved"], true);

        let current_dir = temp_dir.join("current");
        export_plugin_v1(&plugin_dir, "codex", &current_dir, false)
            .expect("current export should succeed");
        let current_manifest: Value = serde_json::from_str(
            &fs::read_to_string(current_dir.join(".codex-plugin").join("plugin.json"))
                .expect("read current manifest"),
        )
        .expect("parse current manifest");
        assert!(current_manifest.get("hooks").is_none());
        assert!(current_manifest.get("futureField").is_none());
        assert!(current_dir.join("hooks").join("hooks.json").is_file());
    }

    #[test]
    fn import_codex_plugin_v1_preserves_codex_specific_fields() {
        let temp_dir = unique_temp_dir("codex-import");
        let plugin_dir = temp_dir.join("github");
        fs::create_dir_all(plugin_dir.join(".codex-plugin")).expect("create manifest dir");
        fs::create_dir_all(plugin_dir.join("assets")).expect("create assets");
        fs::write(plugin_dir.join("assets").join("logo.png"), b"logo").expect("write logo");
        fs::write(
            plugin_dir.join(".codex-plugin").join("plugin.json"),
            r##"{
  "name": "github",
  "version": "0.1.5",
  "description": "GitHub connector workflow",
  "author": {"name": "OpenAI", "email": "support@openai.com", "url": "https://openai.com/"},
  "homepage": "https://github.com/",
  "repository": "https://github.com/openai/plugins",
  "license": "MIT",
  "keywords": ["github", "ci"],
  "skills": "./skills/",
  "apps": "./.app.json",
  "interface": {
    "displayName": "GitHub",
    "shortDescription": "Triage PRs",
    "logo": "./assets/logo.png",
    "brandColor": "#24292F"
  },
  "bundledContentVariant": "backend-specific",
  "futureField": {"kept": true}
}"##,
        )
        .expect("write Codex manifest");

        let imported = import_codex_plugin_v1(&plugin_dir).expect("import should succeed");
        assert_eq!(imported.schema_version, "elegy-plugin/v1");
        assert_eq!(imported.name, "github");
        assert_eq!(imported.skills.as_deref(), Some("./skills/"));

        let ext = imported
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("codex.plugin/v1"))
            .cloned()
            .and_then(|value| serde_json::from_value::<CodexPluginExtensionV1>(value).ok())
            .expect("Codex extension should be present");

        assert_eq!(ext.homepage.as_deref(), Some("https://github.com/"));
        assert_eq!(ext.apps.as_deref(), Some("./.app.json"));
        assert_eq!(
            ext.assets.as_deref(),
            Some(&vec!["assets/logo.png".to_string()][..])
        );
        assert_eq!(
            ext.interface
                .as_ref()
                .and_then(|interface| interface.display_name.as_deref()),
            Some("GitHub")
        );
        assert_eq!(ext.extra["bundledContentVariant"], "backend-specific");
        assert_eq!(ext.extra["futureField"]["kept"], true);
    }

    #[test]
    fn validate_plugin_v1_rejects_wrong_codex_extension_schema_version() {
        let plugin = ElegyPluginV1 {
            schema_version: ELEGY_PLUGIN_V1_SCHEMA_VERSION.to_string(),
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Test plugin".to_string(),
            skills: Some("./skills/".to_string()),
            extensions: Some(serde_json::Map::from_iter([(
                "codex.plugin/v1".to_string(),
                json!({"schemaVersion": "codex.plugin/v2"}),
            )])),
            ..ElegyPluginV1::default()
        };

        let validation = validate_elegy_plugin_v1(&plugin);

        assert!(validation
            .issues
            .iter()
            .any(|issue| { issue.contains("schemaVersion must be 'codex.plugin/v1'") }));
    }

    #[test]
    fn validate_plugin_v1_allows_explicit_marketplace_wrapper() {
        let plugin = ElegyPluginV1 {
            schema_version: ELEGY_PLUGIN_V1_SCHEMA_VERSION.to_string(),
            name: "wrapped-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Marketplace wrapper for an external plugin.".to_string(),
            extensions: Some(serde_json::Map::from_iter([(
                "elegy.marketplace-wrapper/v1".to_string(),
                json!({"schemaVersion": "elegy.marketplace-wrapper/v1"}),
            )])),
            ..ElegyPluginV1::default()
        };

        let validation = validate_elegy_plugin_v1(&plugin);

        assert!(validation.is_valid(), "{:?}", validation.issues);
    }

    #[test]
    fn verify_plugin_v1_rejects_invalid_codex_apps_and_hooks() {
        let temp_dir = unique_temp_dir("elegy-invalid-codex");
        let plugin_dir = temp_dir.join("bad-plugin");

        write_plugin_fixture(
            &plugin_dir,
            "bad-plugin",
            "Test plugin for invalid Codex components",
            None,
        );

        fs::write(
            plugin_dir.join(".app.json"),
            r#"{"apps":{"github":{"id":""}}}"#,
        )
        .expect("write apps");
        fs::create_dir_all(plugin_dir.join("hooks")).expect("create hooks");
        fs::write(
            plugin_dir.join("hooks").join("hooks.json"),
            r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"prompt","command":"","async":true}]}]}}"#,
        )
        .expect("write hooks");

        let manifest_path = plugin_dir.join(".elegy-plugin").join("plugin.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest["extensions"]["codex.plugin/v1"] = json!({
            "schemaVersion": "codex.plugin/v1",
            "apps": "./.app.json",
            "hooks": "./hooks/hooks.json"
        });
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");

        let verify_result =
            verify_plugin_v1(&plugin_dir.join(".elegy-plugin")).expect("verify should run");

        assert!(!verify_result.valid);
        assert!(verify_result
            .issues
            .iter()
            .any(|issue| issue.contains("app 'github' id must not be empty")));
        assert!(verify_result
            .issues
            .iter()
            .any(|issue| issue.contains("handler type 'prompt' is not supported")));
        assert!(verify_result
            .issues
            .iter()
            .any(|issue| issue.contains("command must not be empty")));

        let hooks_config =
            super::load_codex_hooks_config(&plugin_dir.join("hooks").join("hooks.json"))
                .expect("hooks parse should preserve async");
        let handler = &hooks_config.hooks["SessionStart"][0].hooks[0];
        assert_eq!(handler.async_, Some(true));
        let serialized = serde_json::to_value(handler).expect("serialize hook handler");
        assert_eq!(serialized["async"], true);
    }

    #[test]
    fn verify_plugin_v1_rejects_malformed_declared_surfaces() {
        let temp_dir = unique_temp_dir("elegy-invalid-surfaces");
        let plugin_dir = temp_dir.join("bad-plugin");
        write_plugin_fixture(&plugin_dir, "bad-plugin", "Invalid surface fixture", None);

        fs::write(
            plugin_dir
                .join("skills")
                .join("bad-plugin")
                .join("SKILL.md"),
            "missing frontmatter",
        )
        .expect("write malformed skill");
        fs::write(plugin_dir.join(".mcp.json"), "{}").expect("write malformed MCP config");

        let manifest_path = plugin_dir.join(".elegy-plugin").join("plugin.json");
        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest["extensions"] = json!({
            "codex.plugin/v1": {
                "schemaVersion": "codex.plugin/v1",
                "mcpServers": "./.mcp.json",
                "assets": ["./assets/missing.png"],
                "interface": {"logo": "./assets/missing.png"}
            }
        });
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");

        let result =
            verify_plugin_v1(&plugin_dir.join(".elegy-plugin")).expect("verification runs");

        assert!(!result.valid);
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.contains("skills.bad-plugin")));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.contains("must contain an mcpServers object")));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.contains("assets path") && issue.contains("does not exist")));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.contains("interface.logo") && issue.contains("does not exist")));
    }

    #[test]
    fn pack_plugin_v1_with_binary_includes_compiled_binary() {
        let temp_dir = unique_temp_dir("elegy-pack-plugin-binary");
        let plugin_dir = temp_dir.join("my-plugin");

        write_plugin_fixture(&plugin_dir, "my-plugin", "Test plugin for packing", None);

        let binary_path = temp_dir.join("my-plugin.exe");
        fs::write(&binary_path, b"binary-bytes").expect("write fake binary");

        let archive_path = temp_dir.join("my-plugin.plugin.zip");
        pack_plugin_v1_with_binary(
            &plugin_dir,
            &archive_path,
            Some(PluginArchiveBinary {
                source_path: &binary_path,
                archive_path: "bin/my-plugin.exe".to_string(),
            }),
        )
        .expect("pack should succeed");

        let file = fs::File::open(&archive_path).expect("open archive");
        let mut zip = zip::ZipArchive::new(file).expect("read archive");
        let mut names = Vec::new();
        for i in 0..zip.len() {
            names.push(zip.by_index(i).expect("zip entry").name().to_string());
        }
        names.sort();

        assert!(names.iter().any(|name| name == "plugin.json"));
        assert!(names.iter().any(|name| name == "skills/my-plugin/SKILL.md"));
        assert!(names.iter().any(|name| name == "bin/my-plugin.exe"));
        assert_eq!(
            zip.by_name("bin/my-plugin.exe")
                .expect("binary entry")
                .unix_mode()
                .map(|mode| mode & 0o777),
            Some(0o755)
        );
    }

    #[test]
    fn pack_plugin_includes_readiness_and_evidence_authority() {
        let temp_dir = unique_temp_dir("elegy-pack-plugin-readiness");
        let plugin_dir = temp_dir.join("my-adapter");
        write_plugin_fixture(
            &plugin_dir,
            "my-adapter",
            "Readiness packaging fixture",
            None,
        );
        fs::create_dir_all(plugin_dir.join("evidence")).expect("create evidence");
        fs::write(
            plugin_dir.join("evidence").join("source.json"),
            r#"{"result":"passed"}"#,
        )
        .expect("write evidence");
        fs::write(
            plugin_dir.join("readiness.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion": "elegy-readiness/v1",
                "surface": "my-adapter",
                "surfaceVersion": "0.1.0",
                "stage": "implemented",
                "summary": "Fixture implementation.",
                "worksToday": ["Runs the fixture command."],
                "limitations": ["Not a real installed task."],
                "supportedEnvironments": ["test"],
                "installation": "Install the test archive.",
                "invocation": "Run my-adapter.",
                "evidence": [
                    {
                        "kind": "source-tests",
                        "path": "./evidence/source.json",
                        "summary": "Source tests passed."
                    },
                    {
                        "kind": "package-verification",
                        "path": "./evidence/source.json",
                        "summary": "Package verification passed."
                    }
                ]
            }))
            .expect("serialize readiness"),
        )
        .expect("write readiness");
        let manifest_path = plugin_dir.join(".elegy-plugin").join("plugin.json");
        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest["readiness"] = json!({
            "stage": "implemented",
            "path": "./readiness.json",
            "schemaVersion": "elegy-readiness/v1"
        });
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");

        let archive_path = temp_dir.join("my-adapter.plugin.zip");
        pack_plugin_v1(&plugin_dir, &archive_path).expect("pack should succeed");

        let file = fs::File::open(&archive_path).expect("open archive");
        let mut zip = zip::ZipArchive::new(file).expect("read archive");
        assert!(zip.by_name("readiness.json").is_ok());
        assert!(zip.by_name("evidence/source.json").is_ok());
    }

    #[test]
    fn pack_plugin_v1_rejects_duplicate_archive_targets() {
        let temp_dir = unique_temp_dir("elegy-pack-duplicate");
        let plugin_dir = temp_dir.join("plugin");
        write_plugin_fixture(&plugin_dir, "duplicate-plugin", "Duplicate fixture", None);
        fs::create_dir_all(plugin_dir.join("assets")).expect("create assets");
        fs::write(plugin_dir.join("assets").join("logo.png"), b"logo").expect("write asset");
        let manifest_path = plugin_dir.join(".elegy-plugin").join("plugin.json");
        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest["extensions"] = json!({
            "codex.plugin/v1": {
                "schemaVersion": "codex.plugin/v1",
                "assets": ["./assets/logo.png", "./assets/logo.png"]
            }
        });
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize"),
        )
        .expect("write manifest");

        let error = pack_plugin_v1(&plugin_dir, &temp_dir.join("plugin.zip"))
            .expect_err("duplicate target must fail");

        assert!(matches!(
            error,
            ToolingError::InvalidPluginPackage { issues, .. }
                if issues.iter().any(|issue| issue.contains("duplicate archive target"))
        ));
    }

    fn sample_cli_capability() -> ElegyCapability {
        ElegyCapability {
            id: "repo.scan.v1".to_string(),
            kind: ElegyCapabilityKind::Cli,
            side_effect_class: ElegySideEffectClass::Query,
            contract_version: "v1".to_string(),
            description: "Scan local repository.".to_string(),
            invocation: Some(ElegyCapabilityInvocation {
                executable: "elegy".to_string(),
                command: vec!["scan".to_string()],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn sample_app_binding_capability() -> ElegyCapability {
        ElegyCapability {
            id: "github.pr-triage.v1".to_string(),
            kind: ElegyCapabilityKind::AppBinding,
            side_effect_class: ElegySideEffectClass::Query,
            contract_version: "v1".to_string(),
            description: "Triage GitHub PRs.".to_string(),
            app_binding: Some(ElegyAppBinding {
                connector: "github".to_string(),
                category: Some("Developer Tools".to_string()),
            }),
            fallback: Some(ElegyCapabilityFallback {
                kind: ElegyCapabilityKind::Cli,
                invocation: ElegyCapabilityInvocation {
                    executable: "gh".to_string(),
                    command: vec!["pr".to_string(), "list".to_string()],
                    ..Default::default()
                },
            }),
            ..Default::default()
        }
    }

    fn sample_catalog(capabilities: Vec<ElegyCapability>) -> ElegyCapabilityCatalogV1 {
        ElegyCapabilityCatalogV1 {
            schema_version: ELEGY_CAPABILITY_CATALOG_V1_SCHEMA_VERSION.to_string(),
            plugin: "test-plugin".to_string(),
            plugin_version: "0.1.0".to_string(),
            generated_at: None,
            digest: None,
            capabilities,
        }
    }

    #[test]
    fn capability_catalog_validates_all_kinds() {
        let catalog = sample_catalog(vec![
            sample_cli_capability(),
            sample_app_binding_capability(),
        ]);
        let result = validate_elegy_capability_catalog_v1(&catalog);
        assert!(
            result.is_valid(),
            "expected valid catalog, got: {:?}",
            result.issues
        );
    }

    #[test]
    fn capability_catalog_rejects_cli_without_invocation() {
        let mut cap = sample_cli_capability();
        cap.invocation = None;
        let catalog = sample_catalog(vec![cap]);
        let result = validate_elegy_capability_catalog_v1(&catalog);
        assert!(!result.is_valid());
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("requires invocation")));
    }

    #[test]
    fn capability_catalog_rejects_cli_with_empty_executable() {
        let mut cap = sample_cli_capability();
        cap.invocation.as_mut().unwrap().executable = "  ".to_string();
        let catalog = sample_catalog(vec![cap]);
        let result = validate_elegy_capability_catalog_v1(&catalog);
        assert!(!result.is_valid());
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("executable must not be empty")));
    }

    #[test]
    fn capability_catalog_rejects_app_binding_without_app_binding_field() {
        let mut cap = sample_app_binding_capability();
        cap.app_binding = None;
        let catalog = sample_catalog(vec![cap]);
        let result = validate_elegy_capability_catalog_v1(&catalog);
        assert!(!result.is_valid());
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("requires appBinding")));
    }

    #[test]
    fn capability_catalog_rejects_app_binding_fallback() {
        let cap = ElegyCapability {
            id: "bad.fallback".to_string(),
            kind: ElegyCapabilityKind::AppBinding,
            side_effect_class: ElegySideEffectClass::Query,
            contract_version: "v1".to_string(),
            description: "Bad fallback.".to_string(),
            app_binding: Some(ElegyAppBinding {
                connector: "github".to_string(),
                category: None,
            }),
            fallback: Some(ElegyCapabilityFallback {
                kind: ElegyCapabilityKind::AppBinding,
                invocation: ElegyCapabilityInvocation {
                    executable: "x".to_string(),
                    command: vec!["y".to_string()],
                    ..Default::default()
                },
            }),
            ..Default::default()
        };
        let catalog = sample_catalog(vec![cap]);
        let result = validate_elegy_capability_catalog_v1(&catalog);
        assert!(!result.is_valid());
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("fallback kind must be cli or mcp")));
    }

    #[test]
    fn capability_catalog_rejects_duplicate_ids() {
        let catalog = sample_catalog(vec![sample_cli_capability(), sample_cli_capability()]);
        let result = validate_elegy_capability_catalog_v1(&catalog);
        assert!(!result.is_valid());
        assert!(result
            .issues
            .iter()
            .any(|i| i.contains("duplicate capability id")));
    }

    #[test]
    fn capability_catalog_kind_defaults_to_cli_on_deserialize() {
        let json = r#"{
            "schemaVersion": "elegy-capability-catalog/v1",
            "plugin": "test",
            "pluginVersion": "0.1.0",
            "capabilities": [
                {
                    "id": "legacy.cli",
                    "sideEffectClass": "query",
                    "contractVersion": "v1",
                    "description": "Legacy without kind.",
                    "invocation": { "executable": "x", "command": ["y"] }
                }
            ]
        }"#;
        let catalog: ElegyCapabilityCatalogV1 =
            serde_json::from_str(json).expect("deserialize legacy catalog");
        assert_eq!(catalog.capabilities[0].kind, ElegyCapabilityKind::Cli);
        assert!(validate_elegy_capability_catalog_v1(&catalog).is_valid());
    }

    #[test]
    fn capability_catalog_v2_accepts_only_concrete_kinds_and_kind_fields() {
        let catalog: ElegyCapabilityCatalogV2 = serde_json::from_value(json!({
            "schemaVersion": ELEGY_CAPABILITY_CATALOG_V2_SCHEMA_VERSION,
            "plugin": "example-plugin",
            "pluginVersion": "1.2.3",
            "capabilities": [
                {
                    "id": "status",
                    "kind": "cli",
                    "description": "Show status.",
                    "contractVersion": "v1",
                    "sideEffectClass": "query",
                    "readiness": "implemented",
                    "invocation": {"executable": "example", "command": ["status"]}
                },
                {
                    "id": "document",
                    "kind": "mcp-resource",
                    "description": "Read a document.",
                    "contractVersion": "v1",
                    "sideEffectClass": "query",
                    "readiness": "implemented",
                    "resourceUri": "memory://document/{id}",
                    "outputSchema": {"type": "object"}
                },
                {
                    "id": "search",
                    "kind": "mcp-tool",
                    "description": "Search documents.",
                    "contractVersion": "v1",
                    "sideEffectClass": "query",
                    "readiness": "implemented",
                    "toolName": "search",
                    "inputSchema": {"type": "object"},
                    "outputSchema": {"type": "array"}
                }
            ]
        }))
        .expect("v2 catalog parses");

        assert!(validate_elegy_capability_catalog_v2(&catalog).is_valid());
        assert_eq!(catalog.capabilities.len(), 3);
    }

    #[test]
    fn capability_catalog_v2_rejects_legacy_fallback_and_app_binding_fields() {
        let error = serde_json::from_value::<ElegyCapabilityCatalogV2>(json!({
            "schemaVersion": ELEGY_CAPABILITY_CATALOG_V2_SCHEMA_VERSION,
            "plugin": "example-plugin",
            "pluginVersion": "1.2.3",
            "capabilities": [{
                "id": "status",
                "kind": "cli",
                "description": "Show status.",
                "contractVersion": "v1",
                "sideEffectClass": "query",
                "readiness": "implemented",
                "invocation": {"executable": "example", "command": ["status"]},
                "fallback": {"kind": "cli", "invocation": {"executable": "x", "command": ["y"]}},
                "appBinding": {"connector": "github"}
            }]
        }))
        .expect_err("legacy v1 fields must be rejected by v2");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn v1_mcp_with_tool_name_migrates_to_mcp_tool_without_resource_inference() {
        let mut legacy = sample_catalog(vec![ElegyCapability {
            id: "search".to_string(),
            kind: ElegyCapabilityKind::Mcp,
            side_effect_class: ElegySideEffectClass::Query,
            contract_version: "v1".to_string(),
            description: "Search documents.".to_string(),
            invocation: Some(ElegyCapabilityInvocation {
                executable: "memory".to_string(),
                command: vec!["mcp-server".to_string()],
                tool_name: Some("search".to_string()),
                ..Default::default()
            }),
            input_schema: Some(json!({"type": "object"})),
            output_schema: Some(json!({"type": "array"})),
            ..Default::default()
        }]);
        legacy.schema_version = ELEGY_CAPABILITY_CATALOG_V1_SCHEMA_VERSION.to_string();
        let migrated = migrate_capability_catalog_v1_to_v2(&legacy).expect("migrate tool");
        assert!(matches!(
            migrated.capabilities[0],
            ElegyCapabilityV2::McpTool { .. }
        ));

        legacy.capabilities[0]
            .invocation
            .as_mut()
            .expect("invocation")
            .tool_name = None;
        let error = migrate_capability_catalog_v1_to_v2(&legacy).expect_err("resource inference");
        assert!(error.iter().any(|issue| issue.contains("toolName")));
    }

    #[test]
    fn capability_catalog_loader_dispatches_v1_and_v2() {
        let dir = unique_temp_dir("catalog-dispatch");
        let v2_path = dir.join("v2.json");
        fs::write(
            &v2_path,
            serde_json::to_string(&json!({
                "schemaVersion": ELEGY_CAPABILITY_CATALOG_V2_SCHEMA_VERSION,
                "plugin": "example-plugin",
                "pluginVersion": "1.2.3",
                "capabilities": [{
                    "id": "status", "kind": "cli", "description": "Status",
                    "contractVersion": "v1", "sideEffectClass": "query", "readiness": "implemented",
                    "invocation": {"executable": "example", "command": ["status"]}
                }]
            }))
            .expect("serialize v2"),
        )
        .expect("write v2");
        assert!(matches!(
            load_capability_catalog(&v2_path).expect("load v2"),
            ElegyCapabilityCatalog::V2(_)
        ));
    }

    #[test]
    fn build_codex_apps_from_catalog_extracts_app_bindings() {
        let catalog = sample_catalog(vec![
            sample_cli_capability(),
            sample_app_binding_capability(),
        ]);
        let apps = build_codex_apps_from_catalog(&catalog).expect("should produce apps");
        assert_eq!(apps.apps.len(), 1);
        let github = apps.apps.get("github").expect("github connector present");
        assert_eq!(github.id, "github");
        assert_eq!(github.category.as_deref(), Some("Developer Tools"));
    }

    #[test]
    fn build_codex_apps_from_catalog_returns_none_without_app_bindings() {
        let catalog = sample_catalog(vec![sample_cli_capability()]);
        assert!(build_codex_apps_from_catalog(&catalog).is_none());
    }

    #[test]
    fn codex_apps_use_explicit_opaque_connection_bindings() {
        let connections = ElegyPluginConnectionsV1 {
            schema_version: "elegy-plugin-connections/v1".to_string(),
            plugin: "connected-plugin".to_string(),
            plugin_version: "0.1.0".to_string(),
            requirements: vec![ElegyConnectionRequirement {
                id: "github-main".to_string(),
                service: "github".to_string(),
                required: true,
                description: "Read and update GitHub work.".to_string(),
            }],
        };
        let bindings = BTreeMap::from([(
            "github-main".to_string(),
            CodexConnectionBinding {
                id: "connector_76869538009648d5b282a4bb21c3d157".to_string(),
            },
        )]);

        let apps =
            build_codex_apps_from_connections(&connections, &bindings).expect("valid bindings");
        let github = apps.apps.get("github-main").expect("bound app");

        assert_eq!(github.id, "connector_76869538009648d5b282a4bb21c3d157");
        assert!(github.required);
    }

    #[test]
    fn export_plugin_v2_generates_codex_apps_from_connection_requirements() {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/app-binding-plugin");
        let temp_dir = unique_temp_dir("codex-v2-connections-export");
        let plugin_dir = temp_dir.join("plugin");
        copy_dir_all(&source, &plugin_dir).expect("copy fixture");

        let manifest_path = plugin_dir.join(".elegy-plugin").join("plugin.json");
        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest["schemaVersion"] = json!("elegy-plugin/v2");
        manifest["connections"] = json!({
            "requirements": {
                "mode": "declared",
                "path": "./connections.json",
                "schemaVersion": "elegy-plugin-connections/v1"
            }
        });
        manifest["readiness"] = json!({
            "stage": "concept",
            "path": "./readiness.json",
            "schemaVersion": ELEGY_READINESS_V1_SCHEMA_VERSION
        });
        manifest["extensions"]["codex.plugin/v1"]["connectionBindings"] = json!({
            "github-main": {
                "id": "connector_76869538009648d5b282a4bb21c3d157"
            }
        });
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");
        fs::write(
            plugin_dir.join("connections.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion": "elegy-plugin-connections/v1",
                "plugin": "test-app-binding",
                "pluginVersion": "0.1.0",
                "requirements": [{
                    "id": "github-main",
                    "service": "github",
                    "required": true,
                    "description": "Access GitHub work."
                }]
            }))
            .expect("serialize connections"),
        )
        .expect("write connections");
        write_concept_readiness(&plugin_dir, "test-app-binding");

        let output_dir = temp_dir.join("output");
        export_plugin_v1(&plugin_dir, "codex", &output_dir, true).expect("export succeeds");
        let apps: Value = serde_json::from_str(
            &fs::read_to_string(output_dir.join(".app.json")).expect("read generated apps"),
        )
        .expect("parse generated apps");

        assert_eq!(
            apps["apps"]["github-main"]["id"],
            "connector_76869538009648d5b282a4bb21c3d157"
        );
        assert_eq!(apps["apps"]["github-main"]["required"], true);
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn export_plugin_v2_never_infers_codex_apps_from_legacy_catalog_slugs() {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/app-binding-plugin");
        let temp_dir = unique_temp_dir("codex-v2-no-catalog-inference");
        let plugin_dir = temp_dir.join("plugin");
        copy_dir_all(&source, &plugin_dir).expect("copy fixture");

        let manifest_path = plugin_dir.join(".elegy-plugin").join("plugin.json");
        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest["schemaVersion"] = json!("elegy-plugin/v2");
        manifest["connections"] = json!({
            "requirements": {"mode": "none"}
        });
        manifest["readiness"] = json!({
            "stage": "concept",
            "path": "./readiness.json",
            "schemaVersion": ELEGY_READINESS_V1_SCHEMA_VERSION
        });
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");
        write_concept_readiness(&plugin_dir, "test-app-binding");

        let output_dir = temp_dir.join("output");
        let result =
            export_plugin_v1(&plugin_dir, "codex", &output_dir, true).expect("export succeeds");

        assert!(!result.emitted_components.apps_emitted);
        assert!(!output_dir.join(".app.json").exists());
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn export_plugin_v1_codex_generates_app_json_from_catalog() {
        let fixture_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/app-binding-plugin");
        let temp_dir = unique_temp_dir("codex-app-binding-export");
        let output_dir = temp_dir.join("codex-output");

        let result = export_plugin_v1(&fixture_dir, "codex", &output_dir, true)
            .expect("export should succeed");

        assert!(result.emitted_components.apps_emitted);

        let app_json_path = output_dir.join(".app.json");
        assert!(app_json_path.exists(), ".app.json must exist after export");

        let apps_raw = fs::read_to_string(&app_json_path).expect("read .app.json");
        let apps: Value = serde_json::from_str(&apps_raw).expect("parse .app.json");
        assert_eq!(apps["apps"]["github"]["id"], "github");
        assert_eq!(apps["apps"]["github"]["category"], "Developer Tools");

        let manifest_path = output_dir.join(".codex-plugin/plugin.json");
        let manifest_raw = fs::read_to_string(&manifest_path).expect("read plugin.json");
        let manifest: Value = serde_json::from_str(&manifest_raw).expect("parse plugin.json");
        assert_eq!(manifest["apps"], "./.app.json");

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn plugin_v3_projects_codex_native_fields_without_loss() {
        let manifest: ElegyPluginV3 = serde_json::from_value(json!({
            "schemaVersion": "elegy-plugin/v3",
            "name": "calendar-adapter",
            "version": "1.2.3",
            "description": "Connects an agent to a calendar API.",
            "author": {
                "name": "Elegy Contributors",
                "email": "maintainers@example.com",
                "url": "https://example.com/team"
            },
            "homepage": "https://example.com/calendar",
            "repository": "https://example.com/repository",
            "license": "Apache-2.0",
            "keywords": ["calendar", "adapter"],
            "skills": ["./skills/calendar", "./skills/scheduling"],
            "mcpServers": {
                "calendar": {
                    "url": "https://calendar.example.com/mcp"
                }
            },
            "apps": "./.app.json",
            "hooks": [
                "./hooks/a.json",
                {
                    "hooks": {
                        "SessionStart": [{
                            "hooks": [{"type": "command", "command": "calendar prepare"}]
                        }]
                    }
                }
            ],
            "interface": {
                "displayName": "Calendar",
                "shortDescription": "Calendar access",
                "longDescription": "Read and update a connected calendar.",
                "developerName": "Elegy Contributors",
                "category": "Productivity",
                "capabilities": ["Read", "Write"],
                "defaultPrompt": ["Show my next meeting."]
            },
            "elegy": {
                "surfaceClass": "adapter-plugin",
                "capabilityCatalog": {
                    "path": "./capability-catalog.json",
                    "schemaVersion": "elegy-capability-catalog/v1"
                },
                "connections": {
                    "requirements": {"mode": "none"}
                },
                "readiness": {
                    "stage": "implemented",
                    "path": "./readiness.json",
                    "schemaVersion": "elegy-readiness/v1"
                },
                "mcpAuthentication": {
                    "calendar": {"mode": "mcp-oauth"}
                }
            }
        }))
        .expect("v3 manifest parses");

        assert!(validate_elegy_plugin_v3(&manifest).is_valid());

        let projected = project_codex_plugin_v3(&manifest).expect("projection succeeds");
        let mut expected = serde_json::to_value(&manifest).expect("serialize source");
        expected
            .as_object_mut()
            .expect("manifest object")
            .remove("schemaVersion");
        expected
            .as_object_mut()
            .expect("manifest object")
            .remove("elegy");
        assert_eq!(projected, expected);
    }

    #[test]
    fn plugin_v3_rejects_http_mcp_without_declared_authentication() {
        let manifest: ElegyPluginV3 = serde_json::from_value(json!({
            "schemaVersion": "elegy-plugin/v3",
            "name": "calendar-adapter",
            "version": "1.2.3",
            "description": "Connects an agent to a calendar API.",
            "mcpServers": {
                "calendar": {"url": "https://calendar.example.com/mcp"}
            },
            "elegy": {
                "surfaceClass": "adapter-plugin",
                "capabilityCatalog": {
                    "path": "./capability-catalog.json",
                    "schemaVersion": "elegy-capability-catalog/v1"
                },
                "connections": {
                    "requirements": {"mode": "none"}
                },
                "readiness": {
                    "stage": "implemented",
                    "path": "./readiness.json",
                    "schemaVersion": "elegy-readiness/v1"
                },
                "mcpAuthentication": {}
            }
        }))
        .expect("v3 manifest parses");

        let validation = validate_elegy_plugin_v3(&manifest);
        assert!(validation
            .issues
            .iter()
            .any(|issue| { issue.contains("calendar") && issue.contains("authentication") }));
    }

    #[test]
    fn generated_schema_artifacts_include_strict_plugin_v3() {
        let artifacts = generate_plugin_schema_artifacts().expect("generate schemas");
        let schema: Value = serde_json::from_str(
            artifacts
                .get("elegy-plugin-v3.schema.json")
                .expect("v3 schema artifact"),
        )
        .expect("parse schema");

        assert_eq!(
            schema["properties"]["schemaVersion"]["const"],
            "elegy-plugin/v3"
        );
        assert!(schema["required"]
            .as_array()
            .is_some_and(|required| required.iter().any(|field| field == "elegy")));
    }

    #[test]
    fn plugin_v3_codex_export_is_lossless_and_other_hosts_fail_closed() {
        let root = unique_temp_dir("plugin-v3-export");
        fs::create_dir_all(root.join(".elegy-plugin")).expect("manifest directory");
        fs::create_dir_all(root.join("skills").join("calendar")).expect("skill directory");
        fs::write(
            root.join("skills").join("calendar").join("SKILL.md"),
            "---\nname: calendar\ndescription: Calendar adapter.\n---\n",
        )
        .expect("write skill");
        fs::write(
            root.join("capability-catalog.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion":"elegy-capability-catalog/v1",
                "plugin":"calendar-adapter",
                "pluginVersion":"1.2.3",
                "capabilities":[{
                    "id":"calendar.read",
                    "kind":"mcp",
                    "sideEffectClass":"query",
                    "contractVersion":"1.0.0",
                    "description":"Read calendar events.",
                    "invocation":{
                        "executable":"calendar-helper",
                        "command":["serve"],
                        "toolName":"calendar_read"
                    }
                }]
            }))
            .expect("serialize catalog"),
        )
        .expect("write catalog");
        fs::write(
            root.join("readiness.json"),
            serde_json::to_string_pretty(&json!({
                "schemaVersion":"elegy-readiness/v1",
                "surface":"calendar-adapter",
                "surfaceVersion":"1.2.3",
                "stage":"concept",
                "summary":"Projection fixture only.",
                "worksToday":["Preserves the tested package fields."],
                "limitations":["Does not exercise a Codex runtime."],
                "supportedEnvironments":["Test fixture"],
                "installation":"No supported installation.",
                "invocation":"Maintainer inspection only.",
                "evidence":[]
            }))
            .expect("serialize readiness"),
        )
        .expect("write readiness");
        let source = json!({
            "schemaVersion": "elegy-plugin/v3",
            "name": "calendar-adapter",
            "version": "1.2.3",
            "description": "Connects an agent to a calendar API.",
            "skills": "./skills/calendar",
            "mcpServers": {
                "calendar": {"url": "https://calendar.example.com/mcp"}
            },
            "hooks": {"hooks": {"SessionStart": [{"hooks": [{"type":"command","command":"echo ready"}]}]}},
            "interface": {"displayName": "Calendar"},
            "elegy": {
                "surfaceClass": "adapter-plugin",
                "capabilityCatalog": {
                    "path": "./capability-catalog.json",
                    "schemaVersion": "elegy-capability-catalog/v1"
                },
                "connections": {"requirements": {"mode": "none"}},
                "readiness": {
                    "stage": "concept",
                    "path": "./readiness.json",
                    "schemaVersion": "elegy-readiness/v1"
                },
                "mcpAuthentication": {
                    "calendar": {"mode": "mcp-oauth"}
                }
            }
        });
        fs::write(
            root.join(".elegy-plugin").join("plugin.json"),
            serde_json::to_string_pretty(&source).expect("serialize source"),
        )
        .expect("write manifest");

        let codex_output = root.join("codex");
        let result = export_plugin_with_policy(
            &root,
            "codex",
            &codex_output,
            true,
            HostProjectionPolicy::Strict,
        )
        .expect("Codex export");
        assert!(result.lossless);
        assert!(result.losses.is_empty());

        let mut expected = source;
        expected
            .as_object_mut()
            .expect("source object")
            .remove("schemaVersion");
        expected
            .as_object_mut()
            .expect("source object")
            .remove("elegy");
        let actual: Value = serde_json::from_str(
            &fs::read_to_string(codex_output.join(".codex-plugin/plugin.json"))
                .expect("read Codex manifest"),
        )
        .expect("parse Codex manifest");
        assert_eq!(actual, expected);

        let claude_error = export_plugin_with_policy(
            &root,
            "claude",
            &root.join("claude"),
            true,
            HostProjectionPolicy::Strict,
        )
        .expect_err("Claude projection must fail closed");
        assert!(claude_error.to_string().contains("cannot represent"));

        let lossy = export_plugin_with_policy(
            &root,
            "claude",
            &root.join("claude-lossy"),
            true,
            HostProjectionPolicy::AllowLossy,
        )
        .expect("explicit lossy export");
        assert!(!lossy.lossless);
        assert!(!lossy.routable);
        assert!(!lossy.losses.is_empty());
        assert!(root.join("claude-lossy/projection-report.json").is_file());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn codex_import_v3_preserves_flexible_native_shapes() {
        let root = unique_temp_dir("codex-import-v3");
        fs::create_dir_all(root.join(".codex-plugin")).expect("manifest directory");
        let source = json!({
            "name": "calendar-adapter",
            "version": "1.2.3",
            "description": "Connects an agent to a calendar API.",
            "skills": ["./skills/calendar", "./skills/scheduling"],
            "mcpServers": {
                "calendar": {"url": "https://calendar.example.com/mcp"},
                "local-helper": {"command": "calendar-helper", "args": ["serve"]}
            },
            "hooks": ["./hooks/a.json", {"hooks": {}}],
            "interface": {"displayName": "Calendar"},
            "futureCodexField": {"preserved": true}
        });
        fs::write(
            root.join(".codex-plugin/plugin.json"),
            serde_json::to_string_pretty(&source).expect("serialize source"),
        )
        .expect("write Codex manifest");

        let mut imported = import_codex_plugin_v3(&root).expect("import v3");
        assert!(!imported.elegy.mcp_authentication.contains_key("calendar"));
        assert_eq!(
            imported.elegy.mcp_authentication["local-helper"].mode,
            super::ElegyMcpAuthenticationMode::None
        );
        imported.elegy.mcp_authentication.insert(
            "calendar".to_string(),
            super::ElegyMcpAuthenticationExpectation {
                mode: super::ElegyMcpAuthenticationMode::McpOauth,
                environment_variable: None,
            },
        );
        let projected = project_codex_plugin_v3(&imported).expect("project imported manifest");
        assert_eq!(projected, source);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn plugin_v3_verification_checks_governed_files() {
        let root = unique_temp_dir("plugin-v3-verify");
        fs::create_dir_all(root.join(".elegy-plugin")).expect("manifest directory");
        let manifest = json!({
            "schemaVersion": "elegy-plugin/v3",
            "name": "calendar-adapter",
            "version": "1.2.3",
            "description": "Connects an agent to a calendar API.",
            "mcpServers": {"calendar": {"command": "calendar-helper"}},
            "assets": {
                "logo": "./missing-logo.svg",
                "gallery": ["./missing-screenshot.png"]
            },
            "interface": {"logo": "./missing-interface.svg"},
            "elegy": {
                "surfaceClass": "adapter-plugin",
                "capabilityCatalog": {
                    "path": "./missing-catalog.json",
                    "schemaVersion": "elegy-capability-catalog/v1"
                },
                "connections": {"requirements": {"mode": "none"}},
                "readiness": {
                    "stage": "implemented",
                    "path": "./missing-readiness.json",
                    "schemaVersion": "elegy-readiness/v1"
                },
                "mcpAuthentication": {"calendar": {"mode": "none"}},
                "packageAssets": ["./missing-package-assets/"]
            }
        });
        fs::write(
            root.join(".elegy-plugin/plugin.json"),
            serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");

        let result = verify_plugin_v3(&root.join(".elegy-plugin")).expect("verification result");
        assert!(!result.valid);
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.contains("missing-catalog.json")));
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.contains("missing-readiness.json")));
        for path in [
            "missing-logo.svg",
            "missing-screenshot.png",
            "missing-interface.svg",
            "missing-package-assets",
        ] {
            assert!(
                result.issues.iter().any(|issue| issue.contains(path)),
                "missing verification issue for {path}: {:?}",
                result.issues
            );
        }
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn plugin_v3_rejects_authentication_without_an_mcp_server_source() {
        let manifest: ElegyPluginV3 = serde_json::from_value(json!({
            "schemaVersion": "elegy-plugin/v3",
            "name": "orphan-auth",
            "version": "1.0.0",
            "description": "Authentication declaration without an MCP source.",
            "skills": "./skills/",
            "elegy": {
                "surfaceClass": "skill",
                "connections": {"requirements": {"mode": "none"}},
                "readiness": {
                    "stage": "concept",
                    "path": "./readiness.json",
                    "schemaVersion": "elegy-readiness/v1"
                },
                "mcpAuthentication": {"missing": {"mode": "mcp-oauth"}}
            }
        }))
        .expect("parse v3 manifest");

        let validation = validate_elegy_plugin_v3(&manifest);
        assert!(validation
            .issues
            .iter()
            .any(|issue| issue.contains("mcpServers is absent")));
    }

    #[test]
    fn plugin_v3_rejects_plaintext_authentication_material() {
        let manifest: ElegyPluginV3 = serde_json::from_value(json!({
            "schemaVersion": "elegy-plugin/v3",
            "name": "calendar-adapter",
            "version": "1.2.3",
            "description": "Connects an agent to a calendar API.",
            "mcpServers": {
                "calendar": {
                    "url": "https://calendar.example.com/mcp",
                    "httpHeaders": {"Authorization": "Bearer plaintext-secret"}
                }
            },
            "elegy": {
                "surfaceClass": "adapter-plugin",
                "capabilityCatalog": {
                    "path": "./capability-catalog.json",
                    "schemaVersion": "elegy-capability-catalog/v1"
                },
                "connections": {"requirements": {"mode": "none"}},
                "readiness": {
                    "stage": "implemented",
                    "path": "./readiness.json",
                    "schemaVersion": "elegy-readiness/v1"
                },
                "mcpAuthentication": {"calendar": {"mode": "mcp-oauth"}}
            }
        }))
        .expect("parse manifest");

        let validation = validate_elegy_plugin_v3(&manifest);
        assert!(validation
            .issues
            .iter()
            .any(|issue| issue.contains("plaintext authentication material")));
    }

    #[test]
    fn plaintext_authentication_scan_rejects_basic_and_opaque_token_headers() {
        for value in [
            json!({"Authorization": "Basic dXNlcjpwYXNz"}),
            json!({"X-Auth-Token": "opaque-value"}),
            json!({"X-Api-Token": "opaque-value"}),
            json!({"X-API-Key": "opaque-value"}),
            json!({"clientSecret": "opaque-value"}),
        ] {
            assert!(
                contains_plaintext_authentication_material(&value),
                "secret-bearing header was accepted: {value}"
            );
        }
        assert!(!contains_plaintext_authentication_material(
            &json!({"environmentVariable": "SERVICE_ACCESS_TOKEN"})
        ));
    }

    #[test]
    fn marketplace_v2_supports_codex_source_and_policy_parity() {
        let marketplace = ElegyMarketplaceV2 {
            schema_version: "elegy-marketplace/v2".to_string(),
            name: "elegy".to_string(),
            interface: None,
            plugins: vec![
                ElegyMarketplacePluginV2 {
                    name: "local-adapter".to_string(),
                    source: ElegyMarketplaceSourceV2::Local {
                        path: "./plugins/local-adapter".to_string(),
                    },
                    policy: super::ElegyMarketplacePolicy {
                        installation: ElegyMarketplaceInstallationPolicy::Available,
                        authentication: ElegyMarketplaceAuthenticationPolicy::OnUse,
                    },
                    category: "Productivity".to_string(),
                    artifacts: Vec::new(),
                },
                ElegyMarketplacePluginV2 {
                    name: "git-adapter".to_string(),
                    source: ElegyMarketplaceSourceV2::GitSubdirectory {
                        url: "https://github.com/example/plugins.git".to_string(),
                        root: "plugins/git-adapter".to_string(),
                        reference: Some("main".to_string()),
                        sha: None,
                    },
                    policy: super::ElegyMarketplacePolicy {
                        installation: ElegyMarketplaceInstallationPolicy::NotAvailable,
                        authentication: ElegyMarketplaceAuthenticationPolicy::OnInstall,
                    },
                    category: "Developer Tools".to_string(),
                    artifacts: Vec::new(),
                },
                ElegyMarketplacePluginV2 {
                    name: "npm-adapter".to_string(),
                    source: ElegyMarketplaceSourceV2::Npm {
                        package: "@example/npm-adapter".to_string(),
                        version: Some("1.0.0".to_string()),
                        registry: Some("https://registry.npmjs.org".to_string()),
                    },
                    policy: super::ElegyMarketplacePolicy {
                        installation: ElegyMarketplaceInstallationPolicy::NotAvailable,
                        authentication: ElegyMarketplaceAuthenticationPolicy::OnUse,
                    },
                    category: "Developer Tools".to_string(),
                    artifacts: Vec::new(),
                },
            ],
        };

        assert!(validate_elegy_marketplace_v2(&marketplace).is_valid());
        let round_trip: ElegyMarketplaceV2 = serde_json::from_value(
            serde_json::to_value(&marketplace).expect("serialize marketplace"),
        )
        .expect("deserialize marketplace");
        assert_eq!(round_trip, marketplace);
    }

    #[test]
    fn v3_rejects_malformed_codex_shapes_and_unknown_surface_classes() {
        let base = json!({
            "schemaVersion":"elegy-plugin/v3",
            "name":"shape-test",
            "version":"1.0.0",
            "description":"Shape validation fixture.",
            "skills":true,
            "mcpServers":{"broken":false},
            "apps":17,
            "hooks":[false],
            "elegy":{
                "surfaceClass":"imaginary-subsystem",
                "connections":{"requirements":{"mode":"none"}},
                "readiness":{
                    "stage":"implemented",
                    "path":"./readiness.json",
                    "schemaVersion":"elegy-readiness/v1"
                }
            }
        });
        let manifest: ElegyPluginV3 = serde_json::from_value(base).expect("parse envelope");
        let validation = validate_elegy_plugin_v3(&manifest);
        for field in ["skills", "mcpServers", "apps", "hooks", "surfaceClass"] {
            assert!(
                validation.issues.iter().any(|issue| issue.contains(field)),
                "missing {field} issue: {:?}",
                validation.issues
            );
        }
    }

    #[test]
    fn marketplace_v2_rejects_unsafe_or_duplicate_artifacts() {
        let marketplace = ElegyMarketplaceV2 {
            schema_version: ELEGY_MARKETPLACE_V2_SCHEMA_VERSION.to_string(),
            name: "elegy".to_string(),
            interface: None,
            plugins: vec![ElegyMarketplacePluginV2 {
                name: "unsafe".to_string(),
                source: ElegyMarketplaceSourceV2::Local {
                    path: "./plugins/unsafe".to_string(),
                },
                policy: ElegyMarketplacePolicy {
                    installation: ElegyMarketplaceInstallationPolicy::Available,
                    authentication: ElegyMarketplaceAuthenticationPolicy::OnUse,
                },
                category: "Developer Tools".to_string(),
                artifacts: vec![
                    ElegyMarketplaceArtifact {
                        target: "x86_64-pc-windows-msvc".to_string(),
                        url: "http://downloads.example.test/plugin.zip".to_string(),
                        checksum_url: "file:///tmp/plugin.sha256".to_string(),
                    },
                    ElegyMarketplaceArtifact {
                        target: "x86_64-pc-windows-msvc".to_string(),
                        url: "https://downloads.example.test/other.zip".to_string(),
                        checksum_url: "https://downloads.example.test/other.sha256".to_string(),
                    },
                ],
            }],
        };
        let validation = validate_elegy_marketplace_v2(&marketplace);
        assert!(validation
            .issues
            .iter()
            .any(|issue| issue.contains("duplicate")));
        assert!(validation
            .issues
            .iter()
            .any(|issue| issue.contains("artifact url")));
        assert!(validation
            .issues
            .iter()
            .any(|issue| issue.contains("checksum")));
    }

    #[test]
    fn generated_schema_artifacts_include_marketplace_v2() {
        let artifacts = generate_plugin_schema_artifacts().expect("generate schemas");
        let schema: Value = serde_json::from_str(
            artifacts
                .get("elegy-marketplace-v2.schema.json")
                .expect("marketplace v2 schema"),
        )
        .expect("parse marketplace schema");
        assert_eq!(
            schema["properties"]["schemaVersion"]["const"],
            "elegy-marketplace/v2"
        );
    }

    #[test]
    fn pack_plugin_v3_includes_governance_and_package_assets() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let plugin = repo.join("plugins/accounts");
        let temp = unique_temp_dir("pack-plugin-v3");
        let archive = temp.join("accounts.zip");

        pack_plugin_v3(&plugin, &archive).expect("pack v3");
        let file = fs::File::open(&archive).expect("open archive");
        let mut zip = zip::ZipArchive::new(file).expect("read archive");
        let names = (0..zip.len())
            .map(|index| zip.by_index(index).expect("zip entry").name().to_string())
            .collect::<Vec<_>>();
        assert!(names.iter().any(|name| name == "plugin.json"));
        assert!(names.iter().any(|name| name == "capability-catalog.json"));
        assert!(names
            .iter()
            .any(|name| name == "ui/account-center/index.html"));
        assert!(names.iter().any(|name| name == "providers/google.json"));
        fs::remove_dir_all(temp).ok();
    }
}
