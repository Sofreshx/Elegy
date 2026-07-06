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
use thiserror::Error;

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
    pub extensions: Option<serde_json::Map<String, serde_json::Value>>,
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ElegyMarketplaceValidationResult {
    pub issues: Vec<String>,
}

impl ElegyMarketplaceValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
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
    if marketplace.plugins.is_empty() {
        issues.push("plugins must contain at least one entry.".to_string());
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

        let mut targets = BTreeSet::new();
        for artifact in &plugin.artifacts {
            if !matches!(
                artifact.target.as_str(),
                "any"
                    | "x86_64-pc-windows-msvc"
                    | "x86_64-unknown-linux-gnu"
                    | "aarch64-apple-darwin"
            ) {
                issues.push(format!(
                    "plugin '{}' has unsupported artifact target '{}'.",
                    plugin.name, artifact.target
                ));
            } else if !targets.insert(artifact.target.clone()) {
                issues.push(format!(
                    "plugin '{}' has duplicate artifact target '{}'.",
                    plugin.name, artifact.target
                ));
            }
            validate_https_url(
                &format!("plugin '{}' artifact url", plugin.name),
                &artifact.url,
                &mut issues,
            );
            validate_https_url(
                &format!("plugin '{}' artifact checksumUrl", plugin.name),
                &artifact.checksum_url,
                &mut issues,
            );
        }
    }

    ElegyMarketplaceValidationResult { issues }
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

fn validate_https_url(field: &str, value: &str, issues: &mut Vec<String>) {
    match url::Url::parse(value) {
        Ok(url) if url.scheme() == "https" && url.host_str().is_some() => {}
        _ => issues.push(format!("{field} must be an absolute HTTPS URL.")),
    }
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
    pub hooks: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<String>,
    /// Relative path(s) to additional non-skill assets to include in the Codex export.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
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

pub const PLUGIN_SCHEMA_ARTIFACTS: [(&str, &str); 4] = [
    ("elegy-plugin-v1.schema.json", "elegy-plugin/v1"),
    ("elegy-marketplace-v1.schema.json", "elegy-marketplace/v1"),
    ("codex-plugin-extension-v1.schema.json", "codex.plugin/v1"),
    ("codex-plugin-manifest.schema.json", "codex-plugin-manifest"),
];

pub fn generate_plugin_schema_artifacts() -> Result<BTreeMap<&'static str, String>, ToolingError> {
    let schemas = [
        (
            PLUGIN_SCHEMA_ARTIFACTS[0].0,
            serde_json::to_value(schema_for!(ElegyPluginV1)),
        ),
        (
            PLUGIN_SCHEMA_ARTIFACTS[1].0,
            serde_json::to_value(schema_for!(ElegyMarketplaceV1)),
        ),
        (
            PLUGIN_SCHEMA_ARTIFACTS[2].0,
            serde_json::to_value(schema_for!(CodexPluginExtensionV1)),
        ),
        (
            PLUGIN_SCHEMA_ARTIFACTS[3].0,
            serde_json::to_value(schema_for!(CodexPluginManifest)),
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
        extensions: Some(extensions),
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
    pub issues: Vec<String>,
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

    // Check skills directory
    let (has_skills, skill_count) = if let Some(ref skills_path) = plugin.skills {
        let skills_dir = if let Some(stripped) = skills_path.strip_prefix("./") {
            package_root.join(stripped)
        } else {
            package_root.join(skills_path)
        };
        if skills_dir.exists() && skills_dir.is_dir() {
            let mut count = 0;
            if let Ok(entries) = fs::read_dir(&skills_dir) {
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
        "hasCodexApps": codex_ext.as_ref().and_then(|e| e.apps.as_ref()).is_some(),
        "hasCodexHooks": codex_ext.as_ref().and_then(|e| e.hooks.as_ref()).is_some(),
        "hasCodexInterface": codex_ext.as_ref().and_then(|e| e.interface.as_ref()).is_some(),
        "hasCodexMcpServers": codex_ext.as_ref().and_then(|e| e.mcp_servers.as_ref()).is_some(),
        "extensionKeys": plugin.extensions.as_ref().map(|e| e.keys().collect::<Vec<_>>()),
    }))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CodexProjectionMode {
    #[default]
    Current,
    Experimental,
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

    // Export skills — copy entire skill directories
    if let Some(ref skills_path) = plugin.skills {
        let skills_src = if let Some(stripped) = skills_path.strip_prefix("./") {
            package_root.join(stripped)
        } else {
            package_root.join(skills_path)
        };

        if skills_src.exists() && skills_src.is_dir() {
            if let Ok(entries) = fs::read_dir(&skills_src) {
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

    // Export MCP server descriptors for claude export
    if host == "claude" {
        if let Some(ref mcp_path) = plugin.mcp_servers {
            let mcp_src = if let Some(stripped) = mcp_path.strip_prefix("./") {
                package_root.join(stripped)
            } else {
                package_root.join(mcp_path)
            };

            if mcp_src.exists() && mcp_src.is_dir() {
                let mcp_dest = output_dir.join("mcp");
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
    }

    // Copy Codex-specific assets if present
    if host == "codex" {
        if let Some(ref ext) = codex_ext {
            if let Some(ref apps_path) = ext.apps {
                let apps_src = resolve_package_path(&package_root, apps_path);
                let apps_dest = output_dir.join(normalize_package_relative_path(apps_path));
                copy_file_component(&apps_src, &apps_dest, overwrite)?;
                written_files.push(display_path(&apps_dest));
                apps_emitted = true;
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
            "version": plugin.version,
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
            if let Some(ref apps) = ext.apps {
                codex_manifest["apps"] = serde_json::json!(apps);
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

    Ok(GeneratedHostExport {
        source_package: format!("{}-v{}", plugin.name, plugin.version),
        plugin_name: plugin.name,
        plugin_version: plugin.version,
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
        analyze_mcp_descriptor_file, author_mcp_descriptor_to_path, export_plugin_v1,
        export_plugin_v1_with_codex_mode, export_plugin_v1_with_codex_mode_and_binary,
        generate_plugin_schema_artifacts, generate_skills_from_descriptor_file,
        import_codex_plugin_v1, inspect_plugin_v1, is_safe_package_relative_path, pack_plugin_v1,
        pack_plugin_v1_with_binary, select_marketplace_artifact, validate_elegy_marketplace_v1,
        validate_elegy_plugin_v1, verify_plugin_v1, AuthorMcpDescriptorRequest,
        AuthorMcpToolRequest, CodexPluginExtensionV1, CodexProjectionMode,
        ElegyMarketplaceArtifact, ElegyMarketplacePlugin, ElegyMarketplaceSource,
        ElegyMarketplaceV1, ElegyPluginV1, McpServerDescriptor, McpToolAnalyzer, McpToolDefinition,
        PluginArchiveBinary, ToolingError, ELEGY_MARKETPLACE_V1_SCHEMA_VERSION,
        ELEGY_PLUGIN_V1_SCHEMA_VERSION,
    };
    use serde_json::{json, Value};
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

    #[test]
    fn generated_plugin_schemas_match_checked_in_artifacts() {
        let schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas");
        let artifacts = generate_plugin_schema_artifacts().expect("generate plugin schemas");

        for (file_name, expected) in artifacts {
            let path = schema_dir.join(file_name);
            let actual = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert_eq!(actual, expected, "schema drift: {}", path.display());
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
}
