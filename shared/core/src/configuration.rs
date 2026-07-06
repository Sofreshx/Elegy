use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const ELEGY_CONFIGURATION_TEMPLATE_SCHEMA_VERSION: &str = "elegy-configuration-template/v1";
pub const ELEGY_CONFIGURATION_PROFILE_SCHEMA_VERSION: &str = "elegy-configuration-profile/v1";
pub const ELEGY_CONFIGURATION_RECEIPT_SCHEMA_VERSION: &str = "elegy-configuration-receipt/v1";

fn default_prune() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ElegyConfigurationScope {
    UserGlobal,
    Repo,
    Workspace,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ElegyConfigurationAssetFamily {
    Instruction,
    Skill,
    Agent,
    Mcp,
    Hook,
    SupportFile,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ElegyConfigurationPathBase {
    TargetRoot,
    TemplateRoot,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyConfigurationPathRef {
    pub base: ElegyConfigurationPathBase,
    pub path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyConfigurationBindingDefinition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operationType", rename_all = "camelCase", deny_unknown_fields)]
pub enum ElegyConfigurationOperation {
    CopyFile {
        #[serde(rename = "operationId")]
        operation_id: String,
        family: ElegyConfigurationAssetFamily,
        source: ElegyConfigurationPathRef,
        destination: ElegyConfigurationPathRef,
    },
    CopyDirectory {
        #[serde(rename = "operationId")]
        operation_id: String,
        family: ElegyConfigurationAssetFamily,
        source: ElegyConfigurationPathRef,
        destination: ElegyConfigurationPathRef,
    },
    MirrorDirectory {
        #[serde(rename = "operationId")]
        operation_id: String,
        family: ElegyConfigurationAssetFamily,
        source: ElegyConfigurationPathRef,
        destination: ElegyConfigurationPathRef,
        #[serde(default = "default_prune")]
        prune: bool,
    },
    PatchTextBlock {
        #[serde(rename = "operationId")]
        operation_id: String,
        family: ElegyConfigurationAssetFamily,
        destination: ElegyConfigurationPathRef,
        #[serde(rename = "startMarker")]
        start_marker: String,
        #[serde(rename = "endMarker")]
        end_marker: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(
            default,
            rename = "contentPath",
            skip_serializing_if = "Option::is_none"
        )]
        content_path: Option<ElegyConfigurationPathRef>,
        #[serde(default, rename = "createIfMissing")]
        create_if_missing: bool,
    },
    MergeJson {
        #[serde(rename = "operationId")]
        operation_id: String,
        family: ElegyConfigurationAssetFamily,
        destination: ElegyConfigurationPathRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<ElegyConfigurationPathRef>,
    },
    PatchTomlBlock {
        #[serde(rename = "operationId")]
        operation_id: String,
        family: ElegyConfigurationAssetFamily,
        destination: ElegyConfigurationPathRef,
        #[serde(rename = "startMarker")]
        start_marker: String,
        #[serde(rename = "endMarker")]
        end_marker: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(
            default,
            rename = "contentPath",
            skip_serializing_if = "Option::is_none"
        )]
        content_path: Option<ElegyConfigurationPathRef>,
        #[serde(default, rename = "createIfMissing")]
        create_if_missing: bool,
    },
}

impl ElegyConfigurationOperation {
    pub fn operation_id(&self) -> &str {
        match self {
            Self::CopyFile { operation_id, .. }
            | Self::CopyDirectory { operation_id, .. }
            | Self::MirrorDirectory { operation_id, .. }
            | Self::PatchTextBlock { operation_id, .. }
            | Self::MergeJson { operation_id, .. }
            | Self::PatchTomlBlock { operation_id, .. } => operation_id,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyConfigurationTemplate {
    pub schema_version: String,
    pub template_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    pub scope: ElegyConfigurationScope,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bindings: BTreeMap<String, ElegyConfigurationBindingDefinition>,
    pub operations: Vec<ElegyConfigurationOperation>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyConfigurationProfileTemplateSelection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_path: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bindings: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyConfigurationProfile {
    pub schema_version: String,
    pub profile_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bindings: BTreeMap<String, String>,
    pub templates: Vec<ElegyConfigurationProfileTemplateSelection>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ElegyConfigurationReceiptMode {
    Apply,
    DryRun,
    Verify,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ElegyConfigurationReceiptSubjectKind {
    Template,
    Profile,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ElegyConfigurationReceiptSourceKind {
    Builtin,
    File,
    Package,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ElegyConfigurationReceiptAction {
    Created,
    Updated,
    WouldCreate,
    WouldUpdate,
    Skipped,
    Verified,
    Mismatched,
    Conflict,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyConfigurationReceiptEntry {
    pub template_id: String,
    pub operation_id: String,
    pub family: ElegyConfigurationAssetFamily,
    pub action: ElegyConfigurationReceiptAction,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyConfigurationReceiptSummary {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub verified: usize,
    pub mismatched: usize,
    pub conflicts: usize,
    pub issues: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElegyConfigurationReceipt {
    pub schema_version: String,
    pub mode: ElegyConfigurationReceiptMode,
    pub subject_kind: ElegyConfigurationReceiptSubjectKind,
    pub subject_id: String,
    pub source_kind: ElegyConfigurationReceiptSourceKind,
    pub source_ref: String,
    pub target_root: String,
    pub force: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bindings: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<ElegyConfigurationReceiptEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<String>,
    pub summary: ElegyConfigurationReceiptSummary,
    pub verified: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ElegyConfigurationTemplateValidationResult {
    pub issues: Vec<String>,
}

impl ElegyConfigurationTemplateValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ElegyConfigurationProfileValidationResult {
    pub issues: Vec<String>,
}

impl ElegyConfigurationProfileValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ElegyConfigurationReceiptValidationResult {
    pub issues: Vec<String>,
}

impl ElegyConfigurationReceiptValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

pub fn validate_elegy_configuration_template(
    template: &ElegyConfigurationTemplate,
) -> ElegyConfigurationTemplateValidationResult {
    let mut issues = Vec::new();

    if template.schema_version != ELEGY_CONFIGURATION_TEMPLATE_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{ELEGY_CONFIGURATION_TEMPLATE_SCHEMA_VERSION}'."
        ));
    }
    if template.template_id.trim().is_empty() {
        issues.push("templateId must not be empty.".to_string());
    }
    if template.operations.is_empty() {
        issues.push("operations must include at least one entry.".to_string());
    }

    validate_string_key_map("bindings", template.bindings.keys().cloned(), &mut issues);

    let mut operation_ids = BTreeSet::new();
    for operation in &template.operations {
        let operation_id = operation.operation_id();
        if operation_id.trim().is_empty() {
            issues.push("operationId must not be empty.".to_string());
        } else if !operation_ids.insert(operation_id.to_string()) {
            issues.push(format!(
                "operationId '{operation_id}' must be unique within a template."
            ));
        }

        match operation {
            ElegyConfigurationOperation::CopyFile {
                source,
                destination,
                ..
            }
            | ElegyConfigurationOperation::CopyDirectory {
                source,
                destination,
                ..
            }
            | ElegyConfigurationOperation::MirrorDirectory {
                source,
                destination,
                ..
            } => {
                validate_path_ref("source", source, &mut issues);
                validate_path_ref("destination", destination, &mut issues);
            }
            ElegyConfigurationOperation::PatchTextBlock {
                destination,
                start_marker,
                end_marker,
                content,
                content_path,
                ..
            }
            | ElegyConfigurationOperation::PatchTomlBlock {
                destination,
                start_marker,
                end_marker,
                content,
                content_path,
                ..
            } => {
                validate_path_ref("destination", destination, &mut issues);
                validate_markers(start_marker, end_marker, &mut issues);
                validate_exactly_one_content_source(content, content_path, &mut issues);
            }
            ElegyConfigurationOperation::MergeJson {
                destination,
                value,
                source,
                ..
            } => {
                validate_path_ref("destination", destination, &mut issues);
                let has_value = value.is_some();
                let has_source = source.is_some();
                if has_value == has_source {
                    issues.push(
                        "mergeJson operations must declare exactly one of value or source."
                            .to_string(),
                    );
                }
                if let Some(source) = source {
                    validate_path_ref("source", source, &mut issues);
                }
                if let Some(value) = value {
                    if !value.is_object() {
                        issues.push("mergeJson value must be a JSON object.".to_string());
                    }
                }
            }
        }
    }

    ElegyConfigurationTemplateValidationResult { issues }
}

pub fn validate_elegy_configuration_profile(
    profile: &ElegyConfigurationProfile,
) -> ElegyConfigurationProfileValidationResult {
    let mut issues = Vec::new();

    if profile.schema_version != ELEGY_CONFIGURATION_PROFILE_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{ELEGY_CONFIGURATION_PROFILE_SCHEMA_VERSION}'."
        ));
    }
    if profile.profile_id.trim().is_empty() {
        issues.push("profileId must not be empty.".to_string());
    }
    if profile.templates.is_empty() {
        issues.push("templates must include at least one entry.".to_string());
    }

    validate_string_key_map("bindings", profile.bindings.keys().cloned(), &mut issues);

    for (index, selection) in profile.templates.iter().enumerate() {
        let has_template_id = selection
            .template_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
        let has_template_path = selection
            .template_path
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
        if has_template_id == has_template_path {
            issues.push(format!(
                "templates[{index}] must declare exactly one of templateId or templatePath."
            ));
        }
        validate_string_key_map(
            &format!("templates[{index}].bindings"),
            selection.bindings.keys().cloned(),
            &mut issues,
        );
    }

    ElegyConfigurationProfileValidationResult { issues }
}

pub fn validate_elegy_configuration_receipt(
    receipt: &ElegyConfigurationReceipt,
) -> ElegyConfigurationReceiptValidationResult {
    let mut issues = Vec::new();

    if receipt.schema_version != ELEGY_CONFIGURATION_RECEIPT_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{ELEGY_CONFIGURATION_RECEIPT_SCHEMA_VERSION}'."
        ));
    }
    if receipt.subject_id.trim().is_empty() {
        issues.push("subjectId must not be empty.".to_string());
    }
    if receipt.source_ref.trim().is_empty() {
        issues.push("sourceRef must not be empty.".to_string());
    }
    if receipt.target_root.trim().is_empty() {
        issues.push("targetRoot must not be empty.".to_string());
    }
    validate_string_key_map("bindings", receipt.bindings.keys().cloned(), &mut issues);

    for (index, entry) in receipt.entries.iter().enumerate() {
        if entry.template_id.trim().is_empty() {
            issues.push(format!("entries[{index}].templateId must not be empty."));
        }
        if entry.operation_id.trim().is_empty() {
            issues.push(format!("entries[{index}].operationId must not be empty."));
        }
        if entry.path.trim().is_empty() {
            issues.push(format!("entries[{index}].path must not be empty."));
        }
    }

    ElegyConfigurationReceiptValidationResult { issues }
}

fn validate_string_key_map(
    field: &str,
    keys: impl IntoIterator<Item = String>,
    issues: &mut Vec<String>,
) {
    let mut seen = BTreeSet::new();
    for key in keys {
        if key.trim().is_empty() {
            issues.push(format!("{field} keys must not be empty."));
            continue;
        }
        if !seen.insert(key.clone()) {
            issues.push(format!("{field} must not contain duplicate key '{key}'."));
        }
    }
}

fn validate_path_ref(field: &str, path_ref: &ElegyConfigurationPathRef, issues: &mut Vec<String>) {
    if path_ref.path.trim().is_empty() {
        issues.push(format!("{field}.path must not be empty."));
    }
}

fn validate_markers(start_marker: &str, end_marker: &str, issues: &mut Vec<String>) {
    if start_marker.trim().is_empty() {
        issues.push("startMarker must not be empty.".to_string());
    }
    if end_marker.trim().is_empty() {
        issues.push("endMarker must not be empty.".to_string());
    }
    if !start_marker.trim().is_empty() && start_marker == end_marker {
        issues.push("startMarker and endMarker must differ.".to_string());
    }
}

fn validate_exactly_one_content_source(
    content: &Option<String>,
    content_path: &Option<ElegyConfigurationPathRef>,
    issues: &mut Vec<String>,
) {
    let has_content = content
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_content_path = content_path.is_some();
    if has_content == has_content_path {
        issues.push(
            "block patch operations must declare exactly one of content or contentPath."
                .to_string(),
        );
    }
    if let Some(content_path) = content_path {
        validate_path_ref("contentPath", content_path, issues);
    }
}
