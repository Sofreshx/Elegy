use elegy_contracts::{
    validate_elegy_configuration_profile, validate_elegy_configuration_receipt,
    validate_elegy_configuration_template, validate_elegy_plugin_package, ContractsError,
    ElegyConfigurationAssetFamily, ElegyConfigurationOperation, ElegyConfigurationPathBase,
    ElegyConfigurationPathRef, ElegyConfigurationProfile, ElegyConfigurationReceipt,
    ElegyConfigurationReceiptAction, ElegyConfigurationReceiptEntry, ElegyConfigurationReceiptMode,
    ElegyConfigurationReceiptSourceKind, ElegyConfigurationReceiptSubjectKind,
    ElegyConfigurationReceiptSummary, ElegyConfigurationTemplate, ElegyPluginPackage,
    ELEGY_CONFIGURATION_RECEIPT_SCHEMA_VERSION, ELEGY_CONFIGURATION_TEMPLATE_SCHEMA_VERSION,
    ELEGY_PLUGIN_PACKAGE_V2_SCHEMA_VERSION,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

const BUILTIN_TEMPLATE_FILES: &[(&str, &str, &str)] = &[
    (
        "repo-skill-mirror-minimal",
        include_str!(
            "../../../../contracts/configuration/templates/repo-skill-mirror-minimal.json"
        ),
        "contracts/configuration/builtin/repo-skill-mirror-minimal",
    ),
    (
        "repo-opencode-agentic-minimal",
        include_str!(
            "../../../../contracts/configuration/templates/repo-opencode-agentic-minimal.json"
        ),
        "contracts/configuration/builtin/repo-opencode-agentic-minimal",
    ),
    (
        "codex-home-minimal",
        include_str!("../../../../contracts/configuration/templates/codex-home-minimal.json"),
        "contracts/configuration/builtin/codex-home-minimal",
    ),
];

const BUILTIN_PROFILE_FILES: &[(&str, &str)] = &[
    (
        "repo-opencode-minimal",
        include_str!("../../../../contracts/configuration/profiles/repo-opencode-minimal.json"),
    ),
    (
        "repo-codex-minimal",
        include_str!("../../../../contracts/configuration/profiles/repo-codex-minimal.json"),
    ),
];

#[derive(Debug, Error)]
pub enum ConfigurationError {
    #[error("failed to parse built-in template '{template_id}': {source}")]
    BuiltinTemplateJson {
        template_id: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to parse built-in profile '{profile_id}': {source}")]
    BuiltinProfileJson {
        profile_id: String,
        #[source]
        source: serde_json::Error,
    },
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
    #[error("invalid configuration template '{template_id}'")]
    InvalidTemplate {
        template_id: String,
        issues: Vec<String>,
    },
    #[error("invalid configuration profile '{profile_id}'")]
    InvalidProfile {
        profile_id: String,
        issues: Vec<String>,
    },
    #[error("invalid configuration receipt '{receipt_id}'")]
    InvalidReceipt {
        receipt_id: String,
        issues: Vec<String>,
    },
    #[error("unknown built-in template '{template_id}'")]
    UnknownBuiltinTemplate { template_id: String },
    #[error("unknown built-in profile '{profile_id}'")]
    UnknownBuiltinProfile { profile_id: String },
    #[error("plugin package in {path} is invalid")]
    InvalidPluginPackage { path: PathBuf, issues: Vec<String> },
    #[error("plugin package in {path} requires schemaVersion '{required}'")]
    UnsupportedPluginPackageVersion {
        path: PathBuf,
        required: &'static str,
    },
    #[error("plugin package in {path} does not contain configuration template '{template_id}'")]
    UnknownPackageTemplate { path: PathBuf, template_id: String },
    #[error("plugin package in {path} does not contain configuration profile '{profile_id}'")]
    UnknownPackageProfile { path: PathBuf, profile_id: String },
    #[error("missing required binding '{binding_key}'")]
    MissingBinding { binding_key: String },
    #[error("template '{template_id}' references missing template root asset '{path}'")]
    MissingTemplateAsset { template_id: String, path: String },
    #[error("template '{template_id}' source path does not exist: {path}")]
    MissingSourcePath { template_id: String, path: PathBuf },
    #[error("operation '{operation_id}' has unsupported existing JSON shape at {path}")]
    JsonDestinationNotObject { operation_id: String, path: PathBuf },
    #[error("operation '{operation_id}' produced invalid JSON: {source}")]
    MergeJsonInvalid {
        operation_id: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("operation '{operation_id}' detected a conflict at {path}")]
    Conflict { operation_id: String, path: PathBuf },
    #[error("contract error: {0}")]
    Contracts(String),
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationTemplateSummary {
    pub template_id: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub harness: Option<String>,
    pub scope: String,
    pub operation_count: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationProfileSummary {
    pub profile_id: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub template_count: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationCatalog {
    pub schema_version: &'static str,
    pub template_count: usize,
    pub profile_count: usize,
    pub templates: Vec<ConfigurationTemplateSummary>,
    pub profiles: Vec<ConfigurationProfileSummary>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationShowResult {
    pub source_kind: &'static str,
    pub source_ref: String,
    pub template: ElegyConfigurationTemplate,
}

#[derive(Clone, Debug)]
pub struct ApplyConfigurationRequest {
    pub target_root: PathBuf,
    pub dry_run: bool,
    pub force: bool,
    pub bindings: BTreeMap<String, String>,
    pub package_path: Option<PathBuf>,
    pub template_id: Option<String>,
    pub template_path: Option<PathBuf>,
    pub profile_id: Option<String>,
    pub profile_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct VerifyConfigurationRequest {
    pub target_root: PathBuf,
    pub bindings: BTreeMap<String, String>,
    pub package_path: Option<PathBuf>,
    pub template_id: Option<String>,
    pub template_path: Option<PathBuf>,
    pub profile_id: Option<String>,
    pub profile_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
enum TemplateSource {
    Builtin {
        id: String,
        template_root: PathBuf,
        template: ElegyConfigurationTemplate,
    },
    File {
        path: PathBuf,
        template_root: PathBuf,
        template: ElegyConfigurationTemplate,
    },
    Package {
        package_path: PathBuf,
        component_id: String,
        template_root: PathBuf,
        template: ElegyConfigurationTemplate,
    },
}

impl TemplateSource {
    fn source_kind(&self) -> ElegyConfigurationReceiptSourceKind {
        match self {
            Self::Builtin { .. } => ElegyConfigurationReceiptSourceKind::Builtin,
            Self::File { .. } => ElegyConfigurationReceiptSourceKind::File,
            Self::Package { .. } => ElegyConfigurationReceiptSourceKind::Package,
        }
    }

    fn source_ref(&self) -> String {
        match self {
            Self::Builtin { id, .. } => id.clone(),
            Self::File { path, .. } => path.display().to_string(),
            Self::Package {
                package_path,
                component_id,
                ..
            } => format!("{}#{}", package_path.display(), component_id),
        }
    }

    fn template(&self) -> &ElegyConfigurationTemplate {
        match self {
            Self::Builtin { template, .. }
            | Self::File { template, .. }
            | Self::Package { template, .. } => template,
        }
    }

    fn template_root(&self) -> &Path {
        match self {
            Self::Builtin { template_root, .. }
            | Self::File { template_root, .. }
            | Self::Package { template_root, .. } => template_root,
        }
    }
}

#[derive(Clone, Debug)]
enum ProfileSource {
    Builtin {
        id: String,
        profile: ElegyConfigurationProfile,
    },
    File {
        path: PathBuf,
        profile_root: PathBuf,
        profile: ElegyConfigurationProfile,
    },
    Package {
        package_path: PathBuf,
        component_id: String,
        profile_root: PathBuf,
        profile: ElegyConfigurationProfile,
    },
}

impl ProfileSource {
    fn source_kind(&self) -> ElegyConfigurationReceiptSourceKind {
        match self {
            Self::Builtin { .. } => ElegyConfigurationReceiptSourceKind::Builtin,
            Self::File { .. } => ElegyConfigurationReceiptSourceKind::File,
            Self::Package { .. } => ElegyConfigurationReceiptSourceKind::Package,
        }
    }

    fn source_ref(&self) -> String {
        match self {
            Self::Builtin { id, .. } => id.clone(),
            Self::File { path, .. } => path.display().to_string(),
            Self::Package {
                package_path,
                component_id,
                ..
            } => format!("{}#{}", package_path.display(), component_id),
        }
    }

    fn profile(&self) -> &ElegyConfigurationProfile {
        match self {
            Self::Builtin { profile, .. }
            | Self::File { profile, .. }
            | Self::Package { profile, .. } => profile,
        }
    }

    fn profile_root(&self) -> Option<&Path> {
        match self {
            Self::Builtin { .. } => None,
            Self::File { profile_root, .. } | Self::Package { profile_root, .. } => {
                Some(profile_root)
            }
        }
    }

    fn package_path(&self) -> Option<&Path> {
        match self {
            Self::Package { package_path, .. } => Some(package_path),
            Self::Builtin { .. } | Self::File { .. } => None,
        }
    }
}

#[derive(Clone, Debug)]
struct PackageContext {
    path: PathBuf,
    root: PathBuf,
    package: ElegyPluginPackage,
}

pub fn list_builtin_configuration_catalog() -> Result<ConfigurationCatalog, ConfigurationError> {
    let templates = load_builtin_templates()?;
    let profiles = load_builtin_profiles()?;

    Ok(ConfigurationCatalog {
        schema_version: ELEGY_CONFIGURATION_TEMPLATE_SCHEMA_VERSION,
        template_count: templates.len(),
        profile_count: profiles.len(),
        templates: templates
            .into_iter()
            .map(|template| ConfigurationTemplateSummary {
                template_id: template.template_id,
                display_name: template.display_name,
                description: template.description,
                harness: template.harness,
                scope: format_scope(&template.scope),
                operation_count: template.operations.len(),
            })
            .collect(),
        profiles: profiles
            .into_iter()
            .map(|profile| ConfigurationProfileSummary {
                profile_id: profile.profile_id,
                display_name: profile.display_name,
                description: profile.description,
                template_count: profile.templates.len(),
            })
            .collect(),
    })
}

pub fn show_configuration_template(
    package_path: Option<&Path>,
    template_id: Option<&str>,
    template_path: Option<&Path>,
) -> Result<ConfigurationShowResult, ConfigurationError> {
    let template_source = resolve_template_source(package_path, template_id, template_path)?;
    Ok(ConfigurationShowResult {
        source_kind: match template_source.source_kind() {
            ElegyConfigurationReceiptSourceKind::Builtin => "builtin",
            ElegyConfigurationReceiptSourceKind::File => "file",
            ElegyConfigurationReceiptSourceKind::Package => "package",
        },
        source_ref: template_source.source_ref(),
        template: template_source.template().clone(),
    })
}

pub fn apply_configuration(
    request: ApplyConfigurationRequest,
) -> Result<ElegyConfigurationReceipt, ConfigurationError> {
    let plan = resolve_subject(
        request.package_path.as_deref(),
        request.template_id.as_deref(),
        request.template_path.as_deref(),
        request.profile_id.as_deref(),
        request.profile_path.as_deref(),
        &request.bindings,
    )?;
    let receipt = apply_plan(plan, &request.target_root, request.dry_run, request.force)?;
    validate_receipt_or_error(&receipt)?;
    Ok(receipt)
}

pub fn verify_configuration(
    request: VerifyConfigurationRequest,
) -> Result<ElegyConfigurationReceipt, ConfigurationError> {
    let plan = resolve_subject(
        request.package_path.as_deref(),
        request.template_id.as_deref(),
        request.template_path.as_deref(),
        request.profile_id.as_deref(),
        request.profile_path.as_deref(),
        &request.bindings,
    )?;
    let receipt = verify_plan(plan, &request.target_root)?;
    validate_receipt_or_error(&receipt)?;
    Ok(receipt)
}

#[derive(Clone, Debug)]
struct PlannedTemplate {
    source: TemplateSource,
    bindings: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
enum PlannedSubject {
    Template {
        source: TemplateSource,
        bindings: BTreeMap<String, String>,
    },
    Profile {
        profile: ElegyConfigurationProfile,
        source_ref: String,
        source_kind: ElegyConfigurationReceiptSourceKind,
        templates: Vec<PlannedTemplate>,
        bindings: BTreeMap<String, String>,
    },
}

fn resolve_subject(
    package_path: Option<&Path>,
    template_id: Option<&str>,
    template_path: Option<&Path>,
    profile_id: Option<&str>,
    profile_path: Option<&Path>,
    bindings: &BTreeMap<String, String>,
) -> Result<PlannedSubject, ConfigurationError> {
    let template_selected = template_id.is_some() || template_path.is_some();
    let profile_selected = profile_id.is_some() || profile_path.is_some();

    if template_selected == profile_selected {
        return Err(ConfigurationError::Contracts(
            "exactly one of template or profile must be selected".to_string(),
        ));
    }

    if template_selected {
        let source = resolve_template_source(package_path, template_id, template_path)?;
        let resolved_bindings = resolve_bindings(source.template(), bindings)?;
        return Ok(PlannedSubject::Template {
            source,
            bindings: resolved_bindings,
        });
    }

    let profile_source = resolve_profile_source(package_path, profile_id, profile_path)?;
    let mut planned_templates = Vec::new();
    for selection in &profile_source.profile().templates {
        let source = resolve_profile_template_source(&profile_source, selection)?;
        let mut merged_bindings = profile_source.profile().bindings.clone();
        for (key, value) in bindings {
            merged_bindings.insert(key.clone(), value.clone());
        }
        for (key, value) in &selection.bindings {
            merged_bindings.insert(key.clone(), value.clone());
        }
        let resolved_bindings = resolve_bindings(source.template(), &merged_bindings)?;
        planned_templates.push(PlannedTemplate {
            source,
            bindings: resolved_bindings,
        });
    }

    let mut resolved_subject_bindings = profile_source.profile().bindings.clone();
    for (key, value) in bindings {
        resolved_subject_bindings.insert(key.clone(), value.clone());
    }

    Ok(PlannedSubject::Profile {
        profile: profile_source.profile().clone(),
        source_ref: profile_source.source_ref(),
        source_kind: profile_source.source_kind(),
        templates: planned_templates,
        bindings: resolved_subject_bindings,
    })
}

fn apply_plan(
    subject: PlannedSubject,
    target_root: &Path,
    dry_run: bool,
    force: bool,
) -> Result<ElegyConfigurationReceipt, ConfigurationError> {
    match subject {
        PlannedSubject::Template { source, bindings } => {
            let mut entries = Vec::new();
            let mut issues = Vec::new();
            for operation in &source.template().operations {
                execute_apply_operation(
                    &source,
                    source.template(),
                    operation,
                    &bindings,
                    target_root,
                    dry_run,
                    force,
                    &mut entries,
                    &mut issues,
                )?;
            }
            Ok(build_receipt(
                if dry_run {
                    ElegyConfigurationReceiptMode::DryRun
                } else {
                    ElegyConfigurationReceiptMode::Apply
                },
                ElegyConfigurationReceiptSubjectKind::Template,
                source.template().template_id.clone(),
                source.source_kind(),
                source.source_ref(),
                target_root,
                force,
                bindings,
                entries,
                issues,
            ))
        }
        PlannedSubject::Profile {
            profile,
            source_ref,
            source_kind,
            templates,
            bindings,
        } => {
            let mut entries = Vec::new();
            let mut issues = Vec::new();
            for planned in templates {
                for operation in &planned.source.template().operations {
                    execute_apply_operation(
                        &planned.source,
                        planned.source.template(),
                        operation,
                        &planned.bindings,
                        target_root,
                        dry_run,
                        force,
                        &mut entries,
                        &mut issues,
                    )?;
                }
            }
            Ok(build_receipt(
                if dry_run {
                    ElegyConfigurationReceiptMode::DryRun
                } else {
                    ElegyConfigurationReceiptMode::Apply
                },
                ElegyConfigurationReceiptSubjectKind::Profile,
                profile.profile_id,
                source_kind,
                source_ref,
                target_root,
                force,
                bindings,
                entries,
                issues,
            ))
        }
    }
}

fn verify_plan(
    subject: PlannedSubject,
    target_root: &Path,
) -> Result<ElegyConfigurationReceipt, ConfigurationError> {
    match subject {
        PlannedSubject::Template { source, bindings } => {
            let mut entries = Vec::new();
            let mut issues = Vec::new();
            for operation in &source.template().operations {
                execute_verify_operation(
                    &source,
                    source.template(),
                    operation,
                    &bindings,
                    target_root,
                    &mut entries,
                    &mut issues,
                )?;
            }
            Ok(build_receipt(
                ElegyConfigurationReceiptMode::Verify,
                ElegyConfigurationReceiptSubjectKind::Template,
                source.template().template_id.clone(),
                source.source_kind(),
                source.source_ref(),
                target_root,
                false,
                bindings,
                entries,
                issues,
            ))
        }
        PlannedSubject::Profile {
            profile,
            source_ref,
            source_kind,
            templates,
            bindings,
        } => {
            let mut entries = Vec::new();
            let mut issues = Vec::new();
            for planned in templates {
                for operation in &planned.source.template().operations {
                    execute_verify_operation(
                        &planned.source,
                        planned.source.template(),
                        operation,
                        &planned.bindings,
                        target_root,
                        &mut entries,
                        &mut issues,
                    )?;
                }
            }
            Ok(build_receipt(
                ElegyConfigurationReceiptMode::Verify,
                ElegyConfigurationReceiptSubjectKind::Profile,
                profile.profile_id,
                source_kind,
                source_ref,
                target_root,
                false,
                bindings,
                entries,
                issues,
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_apply_operation(
    source: &TemplateSource,
    template: &ElegyConfigurationTemplate,
    operation: &ElegyConfigurationOperation,
    bindings: &BTreeMap<String, String>,
    target_root: &Path,
    dry_run: bool,
    force: bool,
    entries: &mut Vec<ElegyConfigurationReceiptEntry>,
    issues: &mut Vec<String>,
) -> Result<(), ConfigurationError> {
    match operation {
        ElegyConfigurationOperation::CopyFile {
            operation_id,
            family,
            source: src,
            destination,
        } => {
            let source_path = resolve_source_path(source, src, bindings, target_root)?;
            let destination_path = resolve_target_path(destination, bindings, target_root);
            let content = read_file_bytes(template, &source_path)?;
            let desired_hash = hash_bytes(&content);
            let action = write_file(&destination_path, &content, dry_run, force)?;
            let actual_hash = hash_existing_file(&destination_path)?;
            maybe_issue_on_conflict(&action, &destination_path, issues);
            entries.push(build_entry(
                template,
                operation_id,
                family.clone(),
                action,
                destination_path,
                Some(desired_hash),
                actual_hash,
                None,
            ));
        }
        ElegyConfigurationOperation::CopyDirectory {
            operation_id,
            family,
            source: src,
            destination,
        }
        | ElegyConfigurationOperation::MirrorDirectory {
            operation_id,
            family,
            source: src,
            destination,
            ..
        } => {
            let source_path = resolve_source_path(source, src, bindings, target_root)?;
            let destination_path = resolve_target_path(destination, bindings, target_root);
            let desired_hash = hash_directory(&source_path)?;
            let action = sync_directory(
                &source_path,
                &destination_path,
                matches!(
                    operation,
                    ElegyConfigurationOperation::MirrorDirectory { prune: true, .. }
                ),
                dry_run,
                force,
            )?;
            let actual_hash = if destination_path.exists() {
                Some(hash_directory(&destination_path)?)
            } else {
                None
            };
            maybe_issue_on_conflict(&action, &destination_path, issues);
            entries.push(build_entry(
                template,
                operation_id,
                family.clone(),
                action,
                destination_path,
                Some(desired_hash),
                actual_hash,
                None,
            ));
        }
        ElegyConfigurationOperation::PatchTextBlock {
            operation_id,
            family,
            destination,
            start_marker,
            end_marker,
            content,
            content_path,
            create_if_missing,
        } => {
            let destination_path = resolve_target_path(destination, bindings, target_root);
            let block_content = resolve_block_content(
                template,
                source,
                content,
                content_path,
                bindings,
                target_root,
            )?;
            let action = patch_managed_block(
                &destination_path,
                start_marker,
                end_marker,
                &block_content,
                *create_if_missing,
                dry_run,
                force,
            )?;
            let expected_block = patch_output_for_hash(start_marker, end_marker, &block_content);
            let expected_hash = Some(hash_string(&expected_block));
            let actual_hash = if destination_path.exists() {
                let actual = fs::read_to_string(&destination_path).map_err(|source| {
                    ConfigurationError::Io {
                        path: destination_path.clone(),
                        source,
                    }
                })?;
                extract_managed_block_hash(&actual, start_marker, end_marker)
            } else {
                None
            };
            maybe_issue_on_conflict(&action, &destination_path, issues);
            entries.push(build_entry(
                template,
                operation_id,
                family.clone(),
                action,
                destination_path,
                expected_hash,
                actual_hash,
                None,
            ));
        }
        ElegyConfigurationOperation::PatchTomlBlock {
            operation_id,
            family,
            destination,
            start_marker,
            end_marker,
            content,
            content_path,
            create_if_missing,
        } => {
            let destination_path = resolve_target_path(destination, bindings, target_root);
            let block_content = resolve_block_content(
                template,
                source,
                content,
                content_path,
                bindings,
                target_root,
            )?;
            let action = patch_managed_block(
                &destination_path,
                start_marker,
                end_marker,
                &block_content,
                *create_if_missing,
                dry_run,
                force,
            )?;
            let expected_block = patch_output_for_hash(start_marker, end_marker, &block_content);
            let expected_hash = Some(hash_string(&expected_block));
            let actual_hash = if destination_path.exists() {
                let actual = fs::read_to_string(&destination_path).map_err(|source| {
                    ConfigurationError::Io {
                        path: destination_path.clone(),
                        source,
                    }
                })?;
                extract_managed_block_hash(&actual, start_marker, end_marker)
            } else {
                None
            };
            maybe_issue_on_conflict(&action, &destination_path, issues);
            entries.push(build_entry(
                template,
                operation_id,
                family.clone(),
                action,
                destination_path,
                expected_hash,
                actual_hash,
                None,
            ));
        }
        ElegyConfigurationOperation::MergeJson {
            operation_id,
            family,
            destination,
            value,
            source: merge_source,
        } => {
            let destination_path = resolve_target_path(destination, bindings, target_root);
            let merge_value =
                resolve_merge_value(template, source, value, merge_source, bindings, target_root)?;
            let action =
                merge_json_document(&destination_path, merge_value, dry_run, force, operation_id)?;
            let actual_hash = if destination_path.exists() {
                let current = fs::read_to_string(&destination_path).map_err(|source| {
                    ConfigurationError::Io {
                        path: destination_path.clone(),
                        source,
                    }
                })?;
                Some(hash_string(&normalize_json_text(&current, operation_id)?))
            } else {
                None
            };
            maybe_issue_on_conflict(&action, &destination_path, issues);
            entries.push(build_entry(
                template,
                operation_id,
                family.clone(),
                action,
                destination_path,
                None,
                actual_hash,
                None,
            ));
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn execute_verify_operation(
    source: &TemplateSource,
    template: &ElegyConfigurationTemplate,
    operation: &ElegyConfigurationOperation,
    bindings: &BTreeMap<String, String>,
    target_root: &Path,
    entries: &mut Vec<ElegyConfigurationReceiptEntry>,
    issues: &mut Vec<String>,
) -> Result<(), ConfigurationError> {
    match operation {
        ElegyConfigurationOperation::CopyFile {
            operation_id,
            family,
            source: src,
            destination,
        } => {
            let source_path = resolve_source_path(source, src, bindings, target_root)?;
            let destination_path = resolve_target_path(destination, bindings, target_root);
            let expected_hash = hash_file(&source_path)?;
            let actual_hash = hash_existing_file(&destination_path)?;
            let (action, detail) = compare_hashes(&expected_hash, actual_hash.as_deref());
            if matches!(action, ElegyConfigurationReceiptAction::Mismatched) {
                issues.push(format!(
                    "template '{}' operation '{}' does not match {}",
                    template.template_id,
                    operation_id,
                    destination_path.display()
                ));
            }
            entries.push(build_entry(
                template,
                operation_id,
                family.clone(),
                action,
                destination_path,
                Some(expected_hash),
                actual_hash,
                detail,
            ));
        }
        ElegyConfigurationOperation::CopyDirectory {
            operation_id,
            family,
            source: src,
            destination,
        }
        | ElegyConfigurationOperation::MirrorDirectory {
            operation_id,
            family,
            source: src,
            destination,
            ..
        } => {
            let source_path = resolve_source_path(source, src, bindings, target_root)?;
            let destination_path = resolve_target_path(destination, bindings, target_root);
            let expected_hash = hash_directory(&source_path)?;
            let actual_hash = if destination_path.exists() {
                Some(hash_directory(&destination_path)?)
            } else {
                None
            };
            let (action, detail) = compare_hashes(&expected_hash, actual_hash.as_deref());
            if matches!(action, ElegyConfigurationReceiptAction::Mismatched) {
                issues.push(format!(
                    "template '{}' operation '{}' does not match {}",
                    template.template_id,
                    operation_id,
                    destination_path.display()
                ));
            }
            entries.push(build_entry(
                template,
                operation_id,
                family.clone(),
                action,
                destination_path,
                Some(expected_hash),
                actual_hash,
                detail,
            ));
        }
        ElegyConfigurationOperation::PatchTextBlock {
            operation_id,
            family,
            destination,
            start_marker,
            end_marker,
            content,
            content_path,
            ..
        }
        | ElegyConfigurationOperation::PatchTomlBlock {
            operation_id,
            family,
            destination,
            start_marker,
            end_marker,
            content,
            content_path,
            ..
        } => {
            let destination_path = resolve_target_path(destination, bindings, target_root);
            let block_content = resolve_block_content(
                template,
                source,
                content,
                content_path,
                bindings,
                target_root,
            )?;
            let expected_block = patch_output_for_hash(start_marker, end_marker, &block_content);
            let actual_hash = if destination_path.exists() {
                let actual = fs::read_to_string(&destination_path).map_err(|source| {
                    ConfigurationError::Io {
                        path: destination_path.clone(),
                        source,
                    }
                })?;
                let has_block = actual.contains(start_marker) && actual.contains(end_marker);
                if has_block {
                    Some(hash_string(&actual))
                } else {
                    None
                }
            } else {
                None
            };
            let expected_hash = hash_string(&expected_block);
            let (action, detail) = compare_hashes(&expected_hash, actual_hash.as_deref());
            if matches!(action, ElegyConfigurationReceiptAction::Mismatched) {
                issues.push(format!(
                    "template '{}' operation '{}' does not match {}",
                    template.template_id,
                    operation_id,
                    destination_path.display()
                ));
            }
            entries.push(build_entry(
                template,
                operation_id,
                family.clone(),
                action,
                destination_path,
                Some(expected_hash),
                actual_hash,
                detail,
            ));
        }
        ElegyConfigurationOperation::MergeJson {
            operation_id,
            family,
            destination,
            value,
            source: merge_source,
        } => {
            let destination_path = resolve_target_path(destination, bindings, target_root);
            let merge_value =
                resolve_merge_value(template, source, value, merge_source, bindings, target_root)?;
            let expected_hash =
                hash_string(&serde_json::to_string(&merge_value).map_err(|source| {
                    ConfigurationError::MergeJsonInvalid {
                        operation_id: operation_id.clone(),
                        source,
                    }
                })?);
            let actual_hash = if destination_path.exists() {
                let current = fs::read_to_string(&destination_path).map_err(|source| {
                    ConfigurationError::Io {
                        path: destination_path.clone(),
                        source,
                    }
                })?;
                Some(hash_string(&normalize_json_text(&current, operation_id)?))
            } else {
                None
            };
            let (action, detail) = compare_hashes(&expected_hash, actual_hash.as_deref());
            if matches!(action, ElegyConfigurationReceiptAction::Mismatched) {
                issues.push(format!(
                    "template '{}' operation '{}' does not match {}",
                    template.template_id,
                    operation_id,
                    destination_path.display()
                ));
            }
            entries.push(build_entry(
                template,
                operation_id,
                family.clone(),
                action,
                destination_path,
                Some(expected_hash),
                actual_hash,
                detail,
            ));
        }
    }

    Ok(())
}

fn resolve_template_source(
    package_path: Option<&Path>,
    template_id: Option<&str>,
    template_path: Option<&Path>,
) -> Result<TemplateSource, ConfigurationError> {
    match (package_path, template_id, template_path) {
        (Some(package_path), Some(id), None) => load_package_template(package_path, id),
        (Some(_), None, Some(path)) => load_template_from_file(path),
        (Some(_), None, None) => Err(ConfigurationError::Contracts(
            "exactly one template selector must be provided".to_string(),
        )),
        (Some(_), Some(_), Some(_)) => Err(ConfigurationError::Contracts(
            "exactly one template selector must be provided".to_string(),
        )),
        (None, Some(id), None) => load_builtin_template(id),
        (None, None, Some(path)) => load_template_from_file(path),
        _ => Err(ConfigurationError::Contracts(
            "exactly one template selector must be provided".to_string(),
        )),
    }
}

fn resolve_profile_source(
    package_path: Option<&Path>,
    profile_id: Option<&str>,
    profile_path: Option<&Path>,
) -> Result<ProfileSource, ConfigurationError> {
    match (package_path, profile_id, profile_path) {
        (Some(package_path), Some(id), None) => load_package_profile(package_path, id),
        (Some(_), None, Some(path)) => load_profile_from_file(path),
        (Some(_), None, None) => Err(ConfigurationError::Contracts(
            "exactly one profile selector must be provided".to_string(),
        )),
        (Some(_), Some(_), Some(_)) => Err(ConfigurationError::Contracts(
            "exactly one profile selector must be provided".to_string(),
        )),
        (None, Some(id), None) => Ok(ProfileSource::Builtin {
            id: id.to_string(),
            profile: load_builtin_profile(id)?,
        }),
        (None, None, Some(path)) => load_profile_from_file(path),
        _ => Err(ConfigurationError::Contracts(
            "exactly one profile selector must be provided".to_string(),
        )),
    }
}

fn resolve_profile_template_source(
    profile_source: &ProfileSource,
    selection: &elegy_contracts::ElegyConfigurationProfileTemplateSelection,
) -> Result<TemplateSource, ConfigurationError> {
    match (
        selection.template_id.as_deref(),
        selection.template_path.as_deref(),
    ) {
        (Some(template_id), None) => {
            if let Some(package_path) = profile_source.package_path() {
                match load_package_template(package_path, template_id) {
                    Ok(source) => return Ok(source),
                    Err(ConfigurationError::UnknownPackageTemplate { .. }) => {
                        return load_builtin_template(template_id)
                    }
                    Err(error) => return Err(error),
                }
            }
            load_builtin_template(template_id)
        }
        (None, Some(template_path)) => {
            let resolved_path = profile_source
                .profile_root()
                .map(|root| root.join(template_path))
                .unwrap_or_else(|| PathBuf::from(template_path));
            load_template_from_file(&resolved_path)
        }
        _ => Err(ConfigurationError::Contracts(
            "each profile template selection must choose exactly one template selector".to_string(),
        )),
    }
}

fn load_builtin_templates() -> Result<Vec<ElegyConfigurationTemplate>, ConfigurationError> {
    BUILTIN_TEMPLATE_FILES
        .iter()
        .map(|(template_id, json, _)| {
            let template: ElegyConfigurationTemplate =
                serde_json::from_str(json).map_err(|source| {
                    ConfigurationError::BuiltinTemplateJson {
                        template_id: (*template_id).to_string(),
                        source,
                    }
                })?;
            validate_template_or_error(&template)?;
            Ok(template)
        })
        .collect()
}

fn load_builtin_profiles() -> Result<Vec<ElegyConfigurationProfile>, ConfigurationError> {
    BUILTIN_PROFILE_FILES
        .iter()
        .map(|(profile_id, json)| {
            let profile: ElegyConfigurationProfile =
                serde_json::from_str(json).map_err(|source| {
                    ConfigurationError::BuiltinProfileJson {
                        profile_id: (*profile_id).to_string(),
                        source,
                    }
                })?;
            validate_profile_or_error(&profile)?;
            Ok(profile)
        })
        .collect()
}

fn load_builtin_template(template_id: &str) -> Result<TemplateSource, ConfigurationError> {
    let Some((_, json, root_rel)) = BUILTIN_TEMPLATE_FILES
        .iter()
        .find(|(candidate, _, _)| *candidate == template_id)
    else {
        return Err(ConfigurationError::UnknownBuiltinTemplate {
            template_id: template_id.to_string(),
        });
    };

    let template: ElegyConfigurationTemplate =
        serde_json::from_str(json).map_err(|source| ConfigurationError::BuiltinTemplateJson {
            template_id: template_id.to_string(),
            source,
        })?;
    validate_template_or_error(&template)?;
    Ok(TemplateSource::Builtin {
        id: template_id.to_string(),
        template_root: contracts_repo_root().join(root_rel),
        template,
    })
}

fn load_builtin_profile(profile_id: &str) -> Result<ElegyConfigurationProfile, ConfigurationError> {
    let Some((_, json)) = BUILTIN_PROFILE_FILES
        .iter()
        .find(|(candidate, _)| *candidate == profile_id)
    else {
        return Err(ConfigurationError::UnknownBuiltinProfile {
            profile_id: profile_id.to_string(),
        });
    };

    let profile: ElegyConfigurationProfile =
        serde_json::from_str(json).map_err(|source| ConfigurationError::BuiltinProfileJson {
            profile_id: profile_id.to_string(),
            source,
        })?;
    validate_profile_or_error(&profile)?;
    Ok(profile)
}

fn load_template_from_file(path: &Path) -> Result<TemplateSource, ConfigurationError> {
    let content = fs::read_to_string(path).map_err(|source| ConfigurationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let template: ElegyConfigurationTemplate =
        serde_json::from_str(&content).map_err(|source| ConfigurationError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    validate_template_or_error(&template)?;
    Ok(TemplateSource::File {
        path: path.to_path_buf(),
        template_root: path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
        template,
    })
}

fn load_profile_from_file(path: &Path) -> Result<ProfileSource, ConfigurationError> {
    let content = fs::read_to_string(path).map_err(|source| ConfigurationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let profile: ElegyConfigurationProfile =
        serde_json::from_str(&content).map_err(|source| ConfigurationError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    validate_profile_or_error(&profile)?;
    Ok(ProfileSource::File {
        path: path.to_path_buf(),
        profile_root: path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
        profile,
    })
}

fn load_package_context(path: &Path) -> Result<PackageContext, ConfigurationError> {
    let content = fs::read_to_string(path).map_err(|source| ConfigurationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let package = serde_json::from_str::<ElegyPluginPackage>(&content).map_err(|source| {
        ConfigurationError::Json {
            path: path.to_path_buf(),
            source,
        }
    })?;
    let validation = validate_elegy_plugin_package(&package);
    if !validation.is_valid() {
        return Err(ConfigurationError::InvalidPluginPackage {
            path: path.to_path_buf(),
            issues: validation.issues,
        });
    }
    if package.schema_version != ELEGY_PLUGIN_PACKAGE_V2_SCHEMA_VERSION {
        return Err(ConfigurationError::UnsupportedPluginPackageVersion {
            path: path.to_path_buf(),
            required: ELEGY_PLUGIN_PACKAGE_V2_SCHEMA_VERSION,
        });
    }

    Ok(PackageContext {
        path: path.to_path_buf(),
        root: path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
        package,
    })
}

fn load_package_template(
    path: &Path,
    template_id: &str,
) -> Result<TemplateSource, ConfigurationError> {
    let context = load_package_context(path)?;
    let Some(component) = context
        .package
        .components
        .configuration_templates
        .iter()
        .find(|component| component.id == template_id)
    else {
        return Err(ConfigurationError::UnknownPackageTemplate {
            path: context.path.clone(),
            template_id: template_id.to_string(),
        });
    };
    let template_path = context.root.join(&component.path);
    let content = fs::read_to_string(&template_path).map_err(|source| ConfigurationError::Io {
        path: template_path.clone(),
        source,
    })?;
    let template: ElegyConfigurationTemplate =
        serde_json::from_str(&content).map_err(|source| ConfigurationError::Json {
            path: template_path.clone(),
            source,
        })?;
    validate_template_or_error(&template)?;
    Ok(TemplateSource::Package {
        package_path: context.path,
        component_id: component.id.clone(),
        template_root: template_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
        template,
    })
}

fn load_package_profile(
    path: &Path,
    profile_id: &str,
) -> Result<ProfileSource, ConfigurationError> {
    let context = load_package_context(path)?;
    let Some(component) = context
        .package
        .components
        .configuration_profiles
        .iter()
        .find(|component| component.id == profile_id)
    else {
        return Err(ConfigurationError::UnknownPackageProfile {
            path: context.path.clone(),
            profile_id: profile_id.to_string(),
        });
    };
    let profile_path = context.root.join(&component.path);
    let content = fs::read_to_string(&profile_path).map_err(|source| ConfigurationError::Io {
        path: profile_path.clone(),
        source,
    })?;
    let profile: ElegyConfigurationProfile =
        serde_json::from_str(&content).map_err(|source| ConfigurationError::Json {
            path: profile_path.clone(),
            source,
        })?;
    validate_profile_or_error(&profile)?;
    Ok(ProfileSource::Package {
        package_path: context.path,
        component_id: component.id.clone(),
        profile_root: profile_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
        profile,
    })
}

fn validate_template_or_error(
    template: &ElegyConfigurationTemplate,
) -> Result<(), ConfigurationError> {
    let validation = validate_elegy_configuration_template(template);
    if validation.is_valid() {
        Ok(())
    } else {
        Err(ConfigurationError::InvalidTemplate {
            template_id: template.template_id.clone(),
            issues: validation.issues,
        })
    }
}

fn validate_profile_or_error(
    profile: &ElegyConfigurationProfile,
) -> Result<(), ConfigurationError> {
    let validation = validate_elegy_configuration_profile(profile);
    if validation.is_valid() {
        Ok(())
    } else {
        Err(ConfigurationError::InvalidProfile {
            profile_id: profile.profile_id.clone(),
            issues: validation.issues,
        })
    }
}

fn validate_receipt_or_error(
    receipt: &ElegyConfigurationReceipt,
) -> Result<(), ConfigurationError> {
    let validation = validate_elegy_configuration_receipt(receipt);
    if validation.is_valid() {
        Ok(())
    } else {
        Err(ConfigurationError::InvalidReceipt {
            receipt_id: receipt.subject_id.clone(),
            issues: validation.issues,
        })
    }
}

fn resolve_bindings(
    template: &ElegyConfigurationTemplate,
    overrides: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, ConfigurationError> {
    let mut resolved = BTreeMap::new();
    for (key, definition) in &template.bindings {
        if let Some(value) = overrides.get(key) {
            resolved.insert(key.clone(), value.clone());
            continue;
        }
        if let Some(default_value) = &definition.default_value {
            resolved.insert(key.clone(), default_value.clone());
            continue;
        }
        if definition.required {
            return Err(ConfigurationError::MissingBinding {
                binding_key: key.clone(),
            });
        }
    }

    for (key, value) in overrides {
        resolved.entry(key.clone()).or_insert_with(|| value.clone());
    }

    Ok(resolved)
}

fn resolve_source_path(
    source: &TemplateSource,
    path_ref: &ElegyConfigurationPathRef,
    bindings: &BTreeMap<String, String>,
    target_root: &Path,
) -> Result<PathBuf, ConfigurationError> {
    let rendered = render_path_template(&path_ref.path, bindings)?;
    let path = match path_ref.base {
        ElegyConfigurationPathBase::TargetRoot => target_root.join(rendered),
        ElegyConfigurationPathBase::TemplateRoot => source.template_root().join(rendered),
    };

    if path.exists() {
        Ok(path)
    } else {
        let template_id = source.template().template_id.clone();
        if matches!(path_ref.base, ElegyConfigurationPathBase::TemplateRoot) {
            Err(ConfigurationError::MissingTemplateAsset {
                template_id,
                path: path.display().to_string(),
            })
        } else {
            Err(ConfigurationError::MissingSourcePath { template_id, path })
        }
    }
}

fn resolve_target_path(
    path_ref: &ElegyConfigurationPathRef,
    bindings: &BTreeMap<String, String>,
    target_root: &Path,
) -> PathBuf {
    let rendered =
        render_path_template(&path_ref.path, bindings).unwrap_or_else(|_| path_ref.path.clone());
    match path_ref.base {
        ElegyConfigurationPathBase::TargetRoot => target_root.join(rendered),
        ElegyConfigurationPathBase::TemplateRoot => target_root.join(rendered),
    }
}

fn resolve_block_content(
    template: &ElegyConfigurationTemplate,
    source: &TemplateSource,
    content: &Option<String>,
    content_path: &Option<ElegyConfigurationPathRef>,
    bindings: &BTreeMap<String, String>,
    target_root: &Path,
) -> Result<String, ConfigurationError> {
    if let Some(content) = content {
        return Ok(render_path_template(content, bindings)?);
    }
    let content_path = content_path.as_ref().ok_or_else(|| {
        ConfigurationError::Contracts(format!(
            "template '{}' block content source is missing",
            template.template_id
        ))
    })?;
    let path = resolve_source_path(source, content_path, bindings, target_root)?;
    fs::read_to_string(&path).map_err(|source| ConfigurationError::Io { path, source })
}

fn resolve_merge_value(
    template: &ElegyConfigurationTemplate,
    source: &TemplateSource,
    inline_value: &Option<Value>,
    merge_source: &Option<ElegyConfigurationPathRef>,
    bindings: &BTreeMap<String, String>,
    target_root: &Path,
) -> Result<Value, ConfigurationError> {
    if let Some(value) = inline_value {
        return Ok(value.clone());
    }
    let merge_source = merge_source.as_ref().ok_or_else(|| {
        ConfigurationError::Contracts(format!(
            "template '{}' mergeJson source is missing",
            template.template_id
        ))
    })?;
    let path = resolve_source_path(source, merge_source, bindings, target_root)?;
    let content = fs::read_to_string(&path).map_err(|source| ConfigurationError::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&content).map_err(|source| ConfigurationError::Json { path, source })
}

fn render_path_template(
    input: &str,
    bindings: &BTreeMap<String, String>,
) -> Result<String, ConfigurationError> {
    let mut rendered = input.to_string();
    for (key, value) in bindings {
        let placeholder = format!("${{{key}}}");
        rendered = rendered.replace(&placeholder, value);
    }

    if let Some(start) = rendered.find("${") {
        let rest = &rendered[start + 2..];
        let end = rest.find('}').unwrap_or(rest.len());
        let key = &rest[..end];
        return Err(ConfigurationError::MissingBinding {
            binding_key: key.to_string(),
        });
    }

    Ok(rendered)
}

fn ensure_parent_dir(path: &Path) -> Result<(), ConfigurationError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ConfigurationError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

fn read_file_bytes(
    template: &ElegyConfigurationTemplate,
    path: &Path,
) -> Result<Vec<u8>, ConfigurationError> {
    fs::read(path).map_err(|_source| ConfigurationError::MissingSourcePath {
        template_id: template.template_id.clone(),
        path: PathBuf::from(path),
    })
}

fn write_file(
    destination: &Path,
    desired: &[u8],
    dry_run: bool,
    force: bool,
) -> Result<ElegyConfigurationReceiptAction, ConfigurationError> {
    if !destination.exists() {
        if dry_run {
            return Ok(ElegyConfigurationReceiptAction::WouldCreate);
        }
        ensure_parent_dir(destination)?;
        fs::write(destination, desired).map_err(|source| ConfigurationError::Io {
            path: destination.to_path_buf(),
            source,
        })?;
        return Ok(ElegyConfigurationReceiptAction::Created);
    }

    let current = fs::read(destination).map_err(|source| ConfigurationError::Io {
        path: destination.to_path_buf(),
        source,
    })?;
    if current == desired {
        return Ok(ElegyConfigurationReceiptAction::Skipped);
    }
    if !force {
        return Ok(ElegyConfigurationReceiptAction::Conflict);
    }

    if dry_run {
        return Ok(ElegyConfigurationReceiptAction::WouldUpdate);
    }

    fs::write(destination, desired).map_err(|source| ConfigurationError::Io {
        path: destination.to_path_buf(),
        source,
    })?;
    Ok(ElegyConfigurationReceiptAction::Updated)
}

fn sync_directory(
    source: &Path,
    destination: &Path,
    prune: bool,
    dry_run: bool,
    force: bool,
) -> Result<ElegyConfigurationReceiptAction, ConfigurationError> {
    if !destination.exists() {
        if dry_run {
            return Ok(ElegyConfigurationReceiptAction::WouldCreate);
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|source| ConfigurationError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        copy_dir_recursive(source, destination)?;
        return Ok(ElegyConfigurationReceiptAction::Created);
    }

    let source_hash = hash_directory(source)?;
    let destination_hash = hash_directory(destination)?;
    if source_hash == destination_hash {
        return Ok(ElegyConfigurationReceiptAction::Skipped);
    }
    if !force {
        return Ok(ElegyConfigurationReceiptAction::Conflict);
    }

    if dry_run {
        return Ok(ElegyConfigurationReceiptAction::WouldUpdate);
    }

    if prune {
        fs::remove_dir_all(destination).map_err(|source| ConfigurationError::Io {
            path: destination.to_path_buf(),
            source,
        })?;
    }
    copy_dir_recursive(source, destination)?;
    Ok(ElegyConfigurationReceiptAction::Updated)
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), ConfigurationError> {
    fs::create_dir_all(destination).map_err(|source| ConfigurationError::Io {
        path: destination.to_path_buf(),
        source,
    })?;
    for entry in fs::read_dir(source).map_err(|error| ConfigurationError::Io {
        path: source.to_path_buf(),
        source: error,
    })? {
        let entry = entry.map_err(|error| ConfigurationError::Io {
            path: source.to_path_buf(),
            source: error,
        })?;
        let entry_path = entry.path();
        let target_path = destination.join(entry.file_name());
        let file_type = entry.file_type().map_err(|source| ConfigurationError::Io {
            path: entry_path.clone(),
            source,
        })?;
        if file_type.is_dir() {
            copy_dir_recursive(&entry_path, &target_path)?;
        } else if file_type.is_file() {
            ensure_parent_dir(&target_path)?;
            fs::copy(&entry_path, &target_path).map_err(|source| ConfigurationError::Io {
                path: target_path.clone(),
                source,
            })?;
        }
    }
    Ok(())
}

fn patch_managed_block(
    destination: &Path,
    start_marker: &str,
    end_marker: &str,
    content: &str,
    create_if_missing: bool,
    dry_run: bool,
    force: bool,
) -> Result<ElegyConfigurationReceiptAction, ConfigurationError> {
    let block = patch_output_for_hash(start_marker, end_marker, content);
    let existing = if destination.exists() {
        Some(
            fs::read_to_string(destination).map_err(|source| ConfigurationError::Io {
                path: destination.to_path_buf(),
                source,
            })?,
        )
    } else {
        None
    };

    let next = match existing.as_deref() {
        None => {
            if !create_if_missing {
                return Ok(ElegyConfigurationReceiptAction::Conflict);
            }
            block.clone()
        }
        Some(existing) => upsert_managed_block(existing, start_marker, end_marker, &block),
    };

    if existing.as_deref() == Some(next.as_str()) {
        return Ok(ElegyConfigurationReceiptAction::Skipped);
    }

    if existing.is_some()
        && !force
        && !existing_contains_markers(
            existing.as_deref().unwrap_or_default(),
            start_marker,
            end_marker,
        )
    {
        return Ok(ElegyConfigurationReceiptAction::Conflict);
    }

    if dry_run {
        return Ok(if existing.is_some() {
            ElegyConfigurationReceiptAction::WouldUpdate
        } else {
            ElegyConfigurationReceiptAction::WouldCreate
        });
    }

    ensure_parent_dir(destination)?;
    fs::write(destination, next).map_err(|source| ConfigurationError::Io {
        path: destination.to_path_buf(),
        source,
    })?;

    if existing.is_some() {
        Ok(ElegyConfigurationReceiptAction::Updated)
    } else {
        Ok(ElegyConfigurationReceiptAction::Created)
    }
}

fn existing_contains_markers(existing: &str, start_marker: &str, end_marker: &str) -> bool {
    existing.contains(start_marker) && existing.contains(end_marker)
}

fn upsert_managed_block(
    existing: &str,
    start_marker: &str,
    end_marker: &str,
    block: &str,
) -> String {
    let normalized = normalize_text(existing);
    if let (Some(start), Some(end)) = (normalized.find(start_marker), normalized.find(end_marker)) {
        if end >= start {
            let before = normalized[..start].trim_end();
            let after = normalized[end + end_marker.len()..].trim_start();
            let mut parts = Vec::new();
            if !before.is_empty() {
                parts.push(before.to_string());
            }
            parts.push(block.trim_end().to_string());
            if !after.is_empty() {
                parts.push(after.to_string());
            }
            return ensure_trailing_newline(&parts.join("\n\n"));
        }
    }

    if normalized.trim().is_empty() {
        ensure_trailing_newline(block)
    } else {
        ensure_trailing_newline(&format!(
            "{}\n\n{}",
            normalized.trim_end(),
            block.trim_end()
        ))
    }
}

fn merge_json_document(
    destination: &Path,
    merge_value: Value,
    dry_run: bool,
    force: bool,
    operation_id: &str,
) -> Result<ElegyConfigurationReceiptAction, ConfigurationError> {
    let existing = if destination.exists() {
        let content = fs::read_to_string(destination).map_err(|source| ConfigurationError::Io {
            path: destination.to_path_buf(),
            source,
        })?;
        let parsed: Value =
            serde_json::from_str(&content).map_err(|source| ConfigurationError::Json {
                path: destination.to_path_buf(),
                source,
            })?;
        if !parsed.is_object() {
            return Err(ConfigurationError::JsonDestinationNotObject {
                operation_id: operation_id.to_string(),
                path: destination.to_path_buf(),
            });
        }
        parsed
    } else {
        Value::Object(Default::default())
    };

    let mut merged = existing;
    deep_merge_json(&mut merged, &merge_value);
    let serialized = serde_json::to_string_pretty(&merged).map_err(|source| {
        ConfigurationError::MergeJsonInvalid {
            operation_id: operation_id.to_string(),
            source,
        }
    })? + "\n";

    if destination.exists() {
        let current = fs::read_to_string(destination).map_err(|source| ConfigurationError::Io {
            path: destination.to_path_buf(),
            source,
        })?;
        if normalize_text(&current) == normalize_text(&serialized) {
            return Ok(ElegyConfigurationReceiptAction::Skipped);
        }
        if !force {
            return Ok(ElegyConfigurationReceiptAction::Conflict);
        }
        if dry_run {
            return Ok(ElegyConfigurationReceiptAction::WouldUpdate);
        }
        fs::write(destination, serialized).map_err(|source| ConfigurationError::Io {
            path: destination.to_path_buf(),
            source,
        })?;
        return Ok(ElegyConfigurationReceiptAction::Updated);
    }

    if dry_run {
        return Ok(ElegyConfigurationReceiptAction::WouldCreate);
    }

    ensure_parent_dir(destination)?;
    fs::write(destination, serialized).map_err(|source| ConfigurationError::Io {
        path: destination.to_path_buf(),
        source,
    })?;
    Ok(ElegyConfigurationReceiptAction::Created)
}

fn deep_merge_json(target: &mut Value, incoming: &Value) {
    match (target, incoming) {
        (Value::Object(target_map), Value::Object(incoming_map)) => {
            for (key, incoming_value) in incoming_map {
                match target_map.get_mut(key) {
                    Some(existing) => deep_merge_json(existing, incoming_value),
                    None => {
                        target_map.insert(key.clone(), incoming_value.clone());
                    }
                }
            }
        }
        (target_slot, incoming_value) => {
            *target_slot = incoming_value.clone();
        }
    }
}

fn build_receipt(
    mode: ElegyConfigurationReceiptMode,
    subject_kind: ElegyConfigurationReceiptSubjectKind,
    subject_id: String,
    source_kind: ElegyConfigurationReceiptSourceKind,
    source_ref: String,
    target_root: &Path,
    force: bool,
    bindings: BTreeMap<String, String>,
    entries: Vec<ElegyConfigurationReceiptEntry>,
    issues: Vec<String>,
) -> ElegyConfigurationReceipt {
    let summary = summarize_entries(&entries, issues.len());
    let verified = summary.mismatched == 0 && summary.conflicts == 0 && summary.issues == 0;
    ElegyConfigurationReceipt {
        schema_version: ELEGY_CONFIGURATION_RECEIPT_SCHEMA_VERSION.to_string(),
        mode,
        subject_kind,
        subject_id,
        source_kind,
        source_ref,
        target_root: target_root.display().to_string(),
        force,
        bindings,
        entries,
        issues,
        summary,
        verified,
    }
}

fn summarize_entries(
    entries: &[ElegyConfigurationReceiptEntry],
    issue_count: usize,
) -> ElegyConfigurationReceiptSummary {
    let mut summary = ElegyConfigurationReceiptSummary::default();
    for entry in entries {
        match entry.action {
            ElegyConfigurationReceiptAction::Created
            | ElegyConfigurationReceiptAction::WouldCreate => summary.created += 1,
            ElegyConfigurationReceiptAction::Updated
            | ElegyConfigurationReceiptAction::WouldUpdate => summary.updated += 1,
            ElegyConfigurationReceiptAction::Skipped => summary.skipped += 1,
            ElegyConfigurationReceiptAction::Verified => summary.verified += 1,
            ElegyConfigurationReceiptAction::Mismatched => summary.mismatched += 1,
            ElegyConfigurationReceiptAction::Conflict => summary.conflicts += 1,
        }
    }
    summary.issues = issue_count;
    summary
}

fn build_entry(
    template: &ElegyConfigurationTemplate,
    operation_id: &str,
    family: ElegyConfigurationAssetFamily,
    action: ElegyConfigurationReceiptAction,
    path: PathBuf,
    expected_hash: Option<String>,
    actual_hash: Option<String>,
    detail: Option<String>,
) -> ElegyConfigurationReceiptEntry {
    ElegyConfigurationReceiptEntry {
        template_id: template.template_id.clone(),
        operation_id: operation_id.to_string(),
        family,
        action,
        path: path.display().to_string(),
        expected_hash,
        actual_hash,
        detail,
    }
}

fn compare_hashes(
    expected_hash: &str,
    actual_hash: Option<&str>,
) -> (ElegyConfigurationReceiptAction, Option<String>) {
    match actual_hash {
        Some(actual) if actual == expected_hash => {
            (ElegyConfigurationReceiptAction::Verified, None)
        }
        Some(actual) => (
            ElegyConfigurationReceiptAction::Mismatched,
            Some(format!("expected '{expected_hash}' but found '{actual}'")),
        ),
        None => (
            ElegyConfigurationReceiptAction::Mismatched,
            Some(
                "target path is missing or does not contain the expected managed state".to_string(),
            ),
        ),
    }
}

fn maybe_issue_on_conflict(
    action: &ElegyConfigurationReceiptAction,
    path: &Path,
    issues: &mut Vec<String>,
) {
    if matches!(action, ElegyConfigurationReceiptAction::Conflict) {
        issues.push(format!(
            "conflict detected at {} ; re-run with --force to reconcile managed content",
            path.display()
        ));
    }
}

fn format_scope(scope: &elegy_contracts::ElegyConfigurationScope) -> String {
    match scope {
        elegy_contracts::ElegyConfigurationScope::UserGlobal => "user-global",
        elegy_contracts::ElegyConfigurationScope::Repo => "repo",
        elegy_contracts::ElegyConfigurationScope::Workspace => "workspace",
    }
    .to_string()
}

fn contracts_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}

fn normalize_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn ensure_trailing_newline(text: &str) -> String {
    if text.ends_with('\n') {
        text.to_string()
    } else {
        format!("{text}\n")
    }
}

fn patch_output_for_hash(start_marker: &str, end_marker: &str, content: &str) -> String {
    ensure_trailing_newline(&format!(
        "{start_marker}\n{}\n{end_marker}",
        content.trim_end()
    ))
}

fn extract_managed_block_hash(
    content: &str,
    start_marker: &str,
    end_marker: &str,
) -> Option<String> {
    let normalized = normalize_text(content);
    let start = normalized.find(start_marker)?;
    let end = normalized.find(end_marker)?;
    if end < start {
        return None;
    }
    let block = normalized[start..end + end_marker.len()].to_string();
    Some(hash_string(&ensure_trailing_newline(&block)))
}

fn normalize_json_text(content: &str, operation_id: &str) -> Result<String, ConfigurationError> {
    let parsed: Value =
        serde_json::from_str(content).map_err(|source| ConfigurationError::MergeJsonInvalid {
            operation_id: operation_id.to_string(),
            source,
        })?;
    serde_json::to_string(&parsed).map_err(|source| ConfigurationError::MergeJsonInvalid {
        operation_id: operation_id.to_string(),
        source,
    })
}

fn hash_file(path: &Path) -> Result<String, ConfigurationError> {
    let bytes = fs::read(path).map_err(|source| ConfigurationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(hash_bytes(&bytes))
}

fn hash_existing_file(path: &Path) -> Result<Option<String>, ConfigurationError> {
    if !path.exists() {
        return Ok(None);
    }
    hash_file(path).map(Some)
}

fn hash_directory(path: &Path) -> Result<String, ConfigurationError> {
    let mut parts = Vec::new();
    collect_directory_hash_entries(path, path, &mut parts)?;
    parts.sort();
    Ok(hash_string(&parts.join("\n")))
}

fn collect_directory_hash_entries(
    base: &Path,
    current: &Path,
    parts: &mut Vec<String>,
) -> Result<(), ConfigurationError> {
    for entry in fs::read_dir(current).map_err(|source| ConfigurationError::Io {
        path: current.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ConfigurationError::Io {
            path: current.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| ConfigurationError::Io {
            path: path.clone(),
            source,
        })?;
        if file_type.is_dir() {
            collect_directory_hash_entries(base, &path, parts)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let hash = hash_file(&path)?;
            parts.push(format!("{relative}\0{hash}"));
        }
    }
    Ok(())
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut state: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(0x100000001b3);
    }
    format!("fnv64:{state:016x}")
}

fn hash_string(text: &str) -> String {
    hash_bytes(normalize_text(text).as_bytes())
}

impl From<ContractsError> for ConfigurationError {
    fn from(value: ContractsError) -> Self {
        Self::Contracts(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn list_builtin_catalog_includes_templates_and_profiles() {
        let catalog = list_builtin_configuration_catalog().expect("catalog");
        assert!(catalog.template_count >= 3);
        assert!(catalog.profile_count >= 2);
        assert!(catalog
            .templates
            .iter()
            .any(|template| template.template_id == "repo-opencode-agentic-minimal"));
    }

    #[test]
    fn apply_repo_skill_mirror_respects_binding_override() {
        let temp = tempdir().expect("temp dir");
        let repo_root = temp.path();
        fs::create_dir_all(repo_root.join("custom-authority/example-skill"))
            .expect("authority dir");
        fs::write(
            repo_root.join("custom-authority/example-skill/SKILL.md"),
            "# Example\n",
        )
        .expect("write skill");

        let receipt = apply_configuration(ApplyConfigurationRequest {
            target_root: repo_root.to_path_buf(),
            dry_run: false,
            force: false,
            bindings: BTreeMap::from([
                (
                    "authority.skills".to_string(),
                    "custom-authority".to_string(),
                ),
                ("target.skills".to_string(), ".opencode/skills".to_string()),
            ]),
            package_path: None,
            template_id: Some("repo-skill-mirror-minimal".to_string()),
            template_path: None,
            profile_id: None,
            profile_path: None,
        })
        .expect("apply");

        assert!(receipt.verified);
        assert!(repo_root
            .join(".opencode/skills/example-skill/SKILL.md")
            .exists());
    }

    #[test]
    fn verify_repo_skill_mirror_detects_drift() {
        let temp = tempdir().expect("temp dir");
        let repo_root = temp.path();
        fs::create_dir_all(repo_root.join(".github/skills/example-skill")).expect("authority dir");
        fs::write(
            repo_root.join(".github/skills/example-skill/SKILL.md"),
            "# Example\n",
        )
        .expect("write skill");
        fs::create_dir_all(repo_root.join(".agents/skills/example-skill")).expect("mirror dir");
        fs::write(
            repo_root.join(".agents/skills/example-skill/SKILL.md"),
            "# Drifted\n",
        )
        .expect("write drifted skill");

        let receipt = verify_configuration(VerifyConfigurationRequest {
            target_root: repo_root.to_path_buf(),
            bindings: BTreeMap::new(),
            package_path: None,
            template_id: Some("repo-skill-mirror-minimal".to_string()),
            template_path: None,
            profile_id: None,
            profile_path: None,
        })
        .expect("verify");

        assert!(!receipt.verified);
        assert_eq!(receipt.summary.mismatched, 1);
    }

    #[test]
    fn apply_opencode_profile_creates_agents_hooks_and_mcp_files() {
        let temp = tempdir().expect("temp dir");
        let repo_root = temp.path();
        fs::create_dir_all(repo_root.join(".github/skills/example-skill")).expect("authority dir");
        fs::write(
            repo_root.join(".github/skills/example-skill/SKILL.md"),
            "# Example\n",
        )
        .expect("write skill");

        let receipt = apply_configuration(ApplyConfigurationRequest {
            target_root: repo_root.to_path_buf(),
            dry_run: false,
            force: true,
            bindings: BTreeMap::new(),
            package_path: None,
            template_id: None,
            template_path: None,
            profile_id: Some("repo-opencode-minimal".to_string()),
            profile_path: None,
        })
        .expect("apply");

        assert!(receipt.verified);
        assert!(repo_root
            .join(".opencode/skills/example-skill/SKILL.md")
            .exists());
        assert!(repo_root.join("AGENTS.md").exists());
        assert!(repo_root
            .join(".github/hooks/opencode-agentic.json")
            .exists());
        assert!(repo_root.join(".vscode/mcp.json").exists());
    }

    #[test]
    fn apply_codex_home_minimal_patches_toml_block() {
        let temp = tempdir().expect("temp dir");
        let home_root = temp.path();

        let receipt = apply_configuration(ApplyConfigurationRequest {
            target_root: home_root.to_path_buf(),
            dry_run: false,
            force: true,
            bindings: BTreeMap::new(),
            package_path: None,
            template_id: Some("codex-home-minimal".to_string()),
            template_path: None,
            profile_id: None,
            profile_path: None,
        })
        .expect("apply");

        assert!(receipt.verified);
        let config = fs::read_to_string(home_root.join("config.toml")).expect("config.toml");
        assert!(config.contains("BEGIN elegy-configuration managed codex defaults"));
        assert!(config.contains("review_model = \"gpt-5.4\""));
    }

    #[test]
    fn apply_package_profile_resolves_package_template_paths() {
        let temp = tempdir().expect("temp dir");
        let root = temp.path();
        let package_dir = root.join("package");
        fs::create_dir_all(package_dir.join("configuration/assets")).expect("package dirs");
        fs::write(
            package_dir.join("configuration/template.json"),
            r#"{
  "schemaVersion": "elegy-configuration-template/v1",
  "templateId": "demo-template",
  "displayName": "Demo Template",
  "scope": "repo",
  "operations": [
    {
      "operationType": "copyFile",
      "operationId": "copy-demo-file",
      "family": "support-file",
      "source": {
        "base": "template-root",
        "path": "assets/demo.txt"
      },
      "destination": {
        "base": "target-root",
        "path": "generated/demo.txt"
      }
    }
  ]
}"#,
        )
        .expect("write template");
        fs::write(
            package_dir.join("configuration/profile.json"),
            r#"{
  "schemaVersion": "elegy-configuration-profile/v1",
  "profileId": "demo-profile",
  "templates": [
    {
      "templatePath": "template.json"
    }
  ]
}"#,
        )
        .expect("write profile");
        fs::write(package_dir.join("configuration/assets/demo.txt"), "demo\n")
            .expect("write asset");
        fs::write(
            package_dir.join("package.json"),
            r#"{
  "schemaVersion": "elegy-plugin-package/v2",
  "identity": {
    "packageId": "elegy.demo-config",
    "name": "demo-config",
    "version": "0.1.0"
  },
  "components": {
    "configurationTemplates": [
      {
        "id": "demo-template",
        "path": "configuration/template.json"
      }
    ],
    "configurationProfiles": [
      {
        "id": "demo-profile",
        "path": "configuration/profile.json"
      }
    ]
  }
}"#,
        )
        .expect("write package");

        let target = root.join("target");
        let receipt = apply_configuration(ApplyConfigurationRequest {
            target_root: target.clone(),
            dry_run: false,
            force: true,
            bindings: BTreeMap::new(),
            package_path: Some(package_dir.join("package.json")),
            template_id: None,
            template_path: None,
            profile_id: Some("demo-profile".to_string()),
            profile_path: None,
        })
        .expect("apply package profile");

        assert!(receipt.verified);
        assert_eq!(
            receipt.source_kind,
            ElegyConfigurationReceiptSourceKind::Package
        );
        assert_eq!(
            fs::read_to_string(target.join("generated/demo.txt")).expect("generated file"),
            "demo\n"
        );
    }

    #[test]
    fn apply_dry_run_reports_preview_without_writing() {
        let temp = tempdir().expect("temp dir");
        let repo_root = temp.path();
        fs::create_dir_all(repo_root.join(".github/skills/example-skill")).expect("authority dir");
        fs::write(
            repo_root.join(".github/skills/example-skill/SKILL.md"),
            "# Example\n",
        )
        .expect("write skill");

        let receipt = apply_configuration(ApplyConfigurationRequest {
            target_root: repo_root.to_path_buf(),
            dry_run: true,
            force: false,
            bindings: BTreeMap::new(),
            package_path: None,
            template_id: Some("repo-skill-mirror-minimal".to_string()),
            template_path: None,
            profile_id: None,
            profile_path: None,
        })
        .expect("dry-run apply");

        assert!(receipt.verified);
        assert_eq!(receipt.mode, ElegyConfigurationReceiptMode::DryRun);
        assert_eq!(receipt.summary.created, 1);
        assert!(matches!(
            receipt.entries[0].action,
            ElegyConfigurationReceiptAction::WouldCreate
        ));
        assert!(!repo_root
            .join(".agents/skills/example-skill/SKILL.md")
            .exists());
    }
}
