use elegy_tooling::DocsConfig as LegacyDocsConfig;
use serde::{Deserialize, Serialize};
use serde_yaml::Value as YamlValue;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::{Date, Month, OffsetDateTime};

/// Repo-relative path to the documentation config file.
pub const DOCS_CONFIG_PATH: &str = ".elegy/docs.yaml";
/// Schema version identifier for legacy v1 docs config.
pub const DOCS_CONFIG_V1_SCHEMA_VERSION: &str = "elegy-docs/v1";
/// Schema version identifier for v2 documentation config.
pub const DOCS_CONFIG_V2_SCHEMA_VERSION: &str = "elegy-documentation/v2";
/// Schema version for documentation init result envelopes.
pub const DOCUMENTATION_INIT_RESULT_SCHEMA_VERSION: &str = "documentation-init-result/v1";
/// Schema version for documentation map result envelopes.
pub const DOCUMENTATION_MAP_RESULT_SCHEMA_VERSION: &str = "documentation-map-result/v1";
/// Schema version for documentation check result envelopes.
pub const DOCUMENTATION_CHECK_RESULT_SCHEMA_VERSION: &str = "documentation-check-result/v1";
/// Schema version for documentation export result envelopes.
pub const DOCUMENTATION_EXPORT_RESULT_SCHEMA_VERSION: &str = "documentation-export-result/v1";
const BUNDLE_EXPORT_FILE_SCHEMA_VERSION: &str = "documentation-bundle/v1";
const AUTHORITY_POSTURE: &str = "derived output; source files remain authoritative";
const SUPPORTED_REQUIRED_FRONTMATTER_FIELDS: &[&str] = &[
    "title",
    "summary",
    "status",
    "doc_kind",
    "schema_version",
    "created",
    "updated",
    "owner",
];

const DOC_KIND_VALUES: &[&str] = &[
    "adr",
    "guide",
    "generated",
    "index",
    "planning",
    "reference",
    "research",
    "spec",
    "system",
];

const STATUS_VALUES: &[&str] = &[
    "accepted",
    "active",
    "archived",
    "blocked",
    "cancelled",
    "completed",
    "current",
    "deprecated",
    "draft",
    "exploratory",
    "planned",
    "proposed",
    "reference",
    "rejected",
    "stable",
    "superseded",
];

const CURRENT_DISALLOWED_STATUSES: &[&str] = &[
    "blocked",
    "cancelled",
    "completed",
    "exploratory",
    "planned",
];
const NON_CURRENT_TRUTH_STATUSES: &[&str] = &["accepted", "current", "stable"];

/// Errors returned by documentation operations.
#[derive(Debug, Error)]
pub enum DocumentationError {
    #[error("failed to {operation} {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse YAML in {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("failed to parse JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid documentation config in {path}")]
    InvalidConfig { path: PathBuf, issues: Vec<String> },
    #[error("invalid documentation request")]
    InvalidRequest { issues: Vec<String> },
}

/// V2 documentation configuration loaded from `docs.yaml`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationConfigV2 {
    #[serde(default = "default_config_schema_version", alias = "schema_version")]
    pub schema_version: String,
    #[serde(default, alias = "authority_roots")]
    pub authority_roots: DocumentationAuthorityRoots,
    #[serde(default = "default_entrypoints")]
    pub entrypoints: Vec<String>,
    #[serde(default, alias = "derived_surfaces")]
    pub derived_surfaces: DocumentationDerivedSurfaces,
    #[serde(
        default = "default_required_frontmatter",
        alias = "required_frontmatter"
    )]
    pub required_frontmatter: Vec<String>,
    #[serde(default, alias = "freshness_warnings")]
    pub freshness_warnings: DocumentationFreshnessWarnings,
    #[serde(default, alias = "local_exceptions")]
    pub local_exceptions: Vec<String>,
}

impl Default for DocumentationConfigV2 {
    fn default() -> Self {
        Self {
            schema_version: default_config_schema_version(),
            authority_roots: DocumentationAuthorityRoots::default(),
            entrypoints: default_entrypoints(),
            derived_surfaces: DocumentationDerivedSurfaces::default(),
            required_frontmatter: default_required_frontmatter(),
            freshness_warnings: DocumentationFreshnessWarnings::default(),
            local_exceptions: Vec::new(),
        }
    }
}

/// Authority root directories grouped by classification.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationAuthorityRoots {
    #[serde(default = "default_current_roots")]
    pub current: Vec<String>,
    #[serde(default = "default_planning_roots")]
    pub planning: Vec<String>,
    #[serde(default = "default_research_roots")]
    pub research: Vec<String>,
    #[serde(default = "default_generated_roots")]
    pub generated: Vec<String>,
}

impl Default for DocumentationAuthorityRoots {
    fn default() -> Self {
        Self {
            current: default_current_roots(),
            planning: default_planning_roots(),
            research: default_research_roots(),
            generated: default_generated_roots(),
        }
    }
}

/// Paths to generated derived surface outputs.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationDerivedSurfaces {
    #[serde(default)]
    pub sidebars: Vec<String>,
    #[serde(default)]
    pub manifests: Vec<String>,
    #[serde(default)]
    pub llms: Vec<String>,
    #[serde(default)]
    pub bundles: Vec<String>,
}

/// Freshness warning thresholds in days per authority class.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationFreshnessWarnings {
    #[serde(default = "default_current_warning_days")]
    pub current_days: u32,
    #[serde(default = "default_planning_warning_days")]
    pub planning_days: u32,
    #[serde(default = "default_research_warning_days")]
    pub research_days: u32,
}

impl Default for DocumentationFreshnessWarnings {
    fn default() -> Self {
        Self {
            current_days: default_current_warning_days(),
            planning_days: default_planning_warning_days(),
            research_days: default_research_warning_days(),
        }
    }
}

/// Result of initializing documentation config for a project.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationInitResult {
    pub project_root: String,
    pub config_found: bool,
    pub config_path: String,
    pub dry_run: bool,
    pub config: DocumentationConfigView,
    pub created: Vec<String>,
    pub skipped: Vec<String>,
}

/// Summary of discovered documents under an authority class.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationRootSummary {
    pub authority_class: String,
    pub configured_roots: Vec<String>,
    pub discovered_document_count: usize,
}

/// A configured entrypoint document with its resolved metadata.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationEntrypoint {
    pub path: String,
    pub exists: bool,
    pub authority_class: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// A discovered documentation file with frontmatter and freshness state.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationDocument {
    pub path: String,
    pub authority_class: String,
    pub source_of_truth: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,
    pub freshness: String,
}

/// Drift state of a single derived surface output file.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationDerivedSurfaceState {
    pub path: String,
    pub surface_kind: String,
    pub exists: bool,
    pub drift_status: String,
}

/// Normalized view of documentation config regardless of schema version.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationConfigView {
    pub schema_version: String,
    pub compatibility_mode: String,
    pub authority_roots: DocumentationAuthorityRoots,
    pub entrypoints: Vec<String>,
    pub derived_surfaces: DocumentationDerivedSurfaces,
    pub required_frontmatter: Vec<String>,
    pub freshness_warnings: DocumentationFreshnessWarnings,
    pub local_exceptions: Vec<String>,
}

/// Full documentation map including documents, entrypoints, and surfaces.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationMapResult {
    pub schema_version: String,
    pub project_root: String,
    pub config_found: bool,
    pub config_path: String,
    pub config: DocumentationConfigView,
    pub entrypoints: Vec<DocumentationEntrypoint>,
    pub root_summaries: Vec<DocumentationRootSummary>,
    pub documents: Vec<DocumentationDocument>,
    pub derived_surfaces: Vec<DocumentationDerivedSurfaceState>,
    pub recommended_reading_order: Vec<String>,
}

/// A single check issue found during documentation validation.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationCheckIssue {
    pub code: String,
    pub severity: String,
    pub path: String,
    pub message: String,
}

/// Result of running documentation checks across the project.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationCheckResult {
    pub schema_version: String,
    pub project_root: String,
    pub config_found: bool,
    pub config_path: String,
    pub config: DocumentationConfigView,
    pub valid: bool,
    pub files_checked: usize,
    pub document_count: usize,
    pub documents: Vec<DocumentationDocument>,
    pub derived_surfaces: Vec<DocumentationDerivedSurfaceState>,
    pub issues: Vec<DocumentationCheckIssue>,
}

/// Result of exporting documentation to llms.txt or bundle format.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationExportResult {
    pub schema_version: String,
    pub project_root: String,
    pub config_path: String,
    pub export_kind: String,
    pub output_path: String,
    pub document_count: usize,
    pub source_count: usize,
    pub generated_at: String,
    pub authority_posture: String,
    pub drift_detected: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct DocumentationFrontmatter {
    title: Option<String>,
    summary: Option<String>,
    status: Option<String>,
    #[serde(alias = "docKind")]
    doc_kind: Option<String>,
    #[serde(alias = "schemaVersion")]
    schema_version: Option<String>,
    created: Option<String>,
    updated: Option<String>,
    owner: Option<String>,
    date: Option<String>,
}

#[derive(Clone, Debug)]
struct ParsedDocument {
    frontmatter: Option<DocumentationFrontmatter>,
    title_fallback: String,
    summary_fallback: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
enum AuthorityClass {
    Current,
    Planning,
    Research,
    Generated,
    Other,
}

impl AuthorityClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Planning => "planning",
            Self::Research => "research",
            Self::Generated => "generated",
            Self::Other => "other",
        }
    }

    fn source_of_truth(self) -> &'static str {
        match self {
            Self::Current => "current-canon",
            Self::Planning => "planning-non-canon",
            Self::Research => "research-non-canon",
            Self::Generated => "generated-derived",
            Self::Other => "unclassified",
        }
    }

    fn freshness_threshold_days(self, freshness: &DocumentationFreshnessWarnings) -> Option<u32> {
        match self {
            Self::Current => Some(freshness.current_days),
            Self::Planning => Some(freshness.planning_days),
            Self::Research => Some(freshness.research_days),
            Self::Generated | Self::Other => None,
        }
    }
}

#[derive(Clone, Debug)]
struct CollectedDocument {
    path: PathBuf,
    relative_path: String,
    authority_class: AuthorityClass,
    parsed: ParsedDocument,
    parse_issue: Option<String>,
}

#[derive(Clone, Debug)]
struct ResolvedDocumentationConfig {
    root: PathBuf,
    config_path: PathBuf,
    config_found: bool,
    view: DocumentationConfigView,
    legacy_index_path: Option<String>,
}

impl ResolvedDocumentationConfig {
    fn relative_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .map(path_display)
            .unwrap_or_else(|_| display_path(path))
    }

    fn is_exception(&self, relative_path: &str) -> bool {
        let normalized = normalize_rel_string(relative_path);
        self.view.local_exceptions.iter().any(|entry| {
            let exception = normalize_rel_string(entry);
            normalized == exception || normalized.starts_with(&(exception + "/"))
        })
    }

    fn roots_for(&self, authority_class: AuthorityClass) -> &[String] {
        match authority_class {
            AuthorityClass::Current => &self.view.authority_roots.current,
            AuthorityClass::Planning => &self.view.authority_roots.planning,
            AuthorityClass::Research => &self.view.authority_roots.research,
            AuthorityClass::Generated => &self.view.authority_roots.generated,
            AuthorityClass::Other => &[],
        }
    }

    fn configured_roots(&self) -> Vec<(AuthorityClass, String)> {
        let mut roots = Vec::new();
        for value in &self.view.authority_roots.current {
            roots.push((AuthorityClass::Current, value.clone()));
        }
        for value in &self.view.authority_roots.planning {
            roots.push((AuthorityClass::Planning, value.clone()));
        }
        for value in &self.view.authority_roots.research {
            roots.push((AuthorityClass::Research, value.clone()));
        }
        for value in &self.view.authority_roots.generated {
            roots.push((AuthorityClass::Generated, value.clone()));
        }
        roots
    }

    fn legacy_index_path(&self) -> Option<PathBuf> {
        self.legacy_index_path
            .as_ref()
            .map(|path| self.root.join(path))
    }

    fn is_derived_surface_path(&self, relative_path: &str) -> bool {
        let normalized = normalize_rel_string(relative_path);
        self.view
            .derived_surfaces
            .sidebars
            .iter()
            .chain(self.view.derived_surfaces.manifests.iter())
            .chain(self.view.derived_surfaces.llms.iter())
            .chain(self.view.derived_surfaces.bundles.iter())
            .any(|entry| normalize_rel_string(entry) == normalized)
    }
}

/// Initializes documentation config, creating `docs.yaml` if missing.
pub fn documentation_init(
    project_root: &Path,
    dry_run: bool,
) -> Result<DocumentationInitResult, DocumentationError> {
    let resolved = resolve_docs_config(project_root)?;
    let mut created = Vec::new();
    let mut skipped = Vec::new();

    if resolved.config_found {
        skipped.push(resolved.relative_path(&resolved.config_path));
    } else {
        created.push(resolved.relative_path(&resolved.config_path));
        if !dry_run {
            write_docs_config(&resolved.config_path, &DocumentationConfigV2::default())?;
        }
    }

    Ok(DocumentationInitResult {
        project_root: display_path(project_root),
        config_found: resolved.config_found,
        config_path: resolved.relative_path(&resolved.config_path),
        dry_run,
        config: resolved.view,
        created,
        skipped,
    })
}

/// Inspects documentation state (alias for `documentation_map`).
pub fn documentation_inspect(
    project_root: &Path,
) -> Result<DocumentationMapResult, DocumentationError> {
    documentation_map(project_root)
}

/// Maps all documentation files, entrypoints, and derived surfaces.
pub fn documentation_map(
    project_root: &Path,
) -> Result<DocumentationMapResult, DocumentationError> {
    let resolved = resolve_docs_config(project_root)?;
    let documents = collect_documents(&resolved)?;
    let derived_surfaces = collect_derived_surface_states(&resolved, &documents)?;
    let entrypoints = collect_entrypoints(&resolved)?;
    let root_summaries = build_root_summaries(&resolved, &documents);
    let recommended_reading_order = build_recommended_reading_order(&entrypoints, &documents);

    Ok(DocumentationMapResult {
        schema_version: DOCUMENTATION_MAP_RESULT_SCHEMA_VERSION.to_string(),
        project_root: display_path(project_root),
        config_found: resolved.config_found,
        config_path: resolved.relative_path(&resolved.config_path),
        config: resolved.view,
        entrypoints,
        root_summaries,
        documents,
        derived_surfaces,
        recommended_reading_order,
    })
}

/// Checks documentation for frontmatter, link, and drift issues.
pub fn documentation_check(
    project_root: &Path,
) -> Result<DocumentationCheckResult, DocumentationError> {
    let resolved = resolve_docs_config(project_root)?;
    let collected = collect_collected_documents(&resolved)?;
    let documents = collected
        .iter()
        .map(|document| project_document(document, &resolved.view.freshness_warnings))
        .collect::<Vec<_>>();
    let mut issues = collect_document_issues(&resolved, &collected)?;
    let derived_surfaces = collect_derived_surface_states(&resolved, &documents)?;
    issues.extend(collect_derived_surface_issues(&resolved, &documents)?);
    issues.extend(collect_entrypoint_issues(&resolved, &documents));
    issues.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.severity.cmp(&right.severity))
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });

    let valid = !issues.iter().any(|issue| issue.severity == "error");
    let files_checked = count_checked_files(&resolved, &collected);

    Ok(DocumentationCheckResult {
        schema_version: DOCUMENTATION_CHECK_RESULT_SCHEMA_VERSION.to_string(),
        project_root: display_path(project_root),
        config_found: resolved.config_found,
        config_path: resolved.relative_path(&resolved.config_path),
        config: resolved.view,
        valid,
        files_checked,
        document_count: documents.len(),
        documents,
        derived_surfaces,
        issues,
    })
}

/// Exports documentation as a deterministic `llms.txt` file.
pub fn documentation_export_llms(
    project_root: &Path,
    output_path: &Path,
) -> Result<DocumentationExportResult, DocumentationError> {
    export_documentation(project_root, output_path, ExportKind::Llms)
}

/// Exports documentation as a deterministic JSON bundle.
pub fn documentation_export_bundle(
    project_root: &Path,
    output_path: &Path,
) -> Result<DocumentationExportResult, DocumentationError> {
    export_documentation(project_root, output_path, ExportKind::Bundle)
}

#[derive(Clone, Copy)]
enum ExportKind {
    Llms,
    Bundle,
}

impl ExportKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Llms => "llms",
            Self::Bundle => "bundle",
        }
    }
}

fn export_documentation(
    project_root: &Path,
    output_path: &Path,
    export_kind: ExportKind,
) -> Result<DocumentationExportResult, DocumentationError> {
    let map = documentation_map(project_root)?;
    let generated_at = render_now_rfc3339()?;
    let content = match export_kind {
        ExportKind::Llms => render_llms_export(&map),
        ExportKind::Bundle => render_bundle_export(&map)?,
    };
    write_output_file(output_path, &content)?;

    Ok(DocumentationExportResult {
        schema_version: DOCUMENTATION_EXPORT_RESULT_SCHEMA_VERSION.to_string(),
        project_root: display_path(project_root),
        config_path: map.config_path,
        export_kind: export_kind.as_str().to_string(),
        output_path: display_path(output_path),
        document_count: map.documents.len(),
        source_count: map.documents.len()
            + map.entrypoints.iter().filter(|entry| entry.exists).count(),
        generated_at,
        authority_posture: AUTHORITY_POSTURE.to_string(),
        drift_detected: false,
    })
}

fn resolve_docs_config(
    project_root: &Path,
) -> Result<ResolvedDocumentationConfig, DocumentationError> {
    let config_path = project_root.join(DOCS_CONFIG_PATH);
    if !config_path.exists() {
        return Ok(ResolvedDocumentationConfig {
            root: project_root.to_path_buf(),
            config_path,
            config_found: false,
            view: DocumentationConfigView {
                schema_version: DOCS_CONFIG_V2_SCHEMA_VERSION.to_string(),
                compatibility_mode: "v2".to_string(),
                authority_roots: DocumentationAuthorityRoots::default(),
                entrypoints: default_entrypoints(),
                derived_surfaces: DocumentationDerivedSurfaces::default(),
                required_frontmatter: default_required_frontmatter(),
                freshness_warnings: DocumentationFreshnessWarnings::default(),
                local_exceptions: Vec::new(),
            },
            legacy_index_path: None,
        });
    }

    let content = fs::read_to_string(&config_path).map_err(|source| DocumentationError::Io {
        operation: "read",
        path: config_path.clone(),
        source,
    })?;
    let raw =
        serde_yaml::from_str::<YamlValue>(&content).map_err(|source| DocumentationError::Yaml {
            path: config_path.clone(),
            source,
        })?;

    let schema_version = yaml_mapping_value(&raw, &["schemaVersion", "schema_version"])
        .and_then(YamlValue::as_str)
        .map(str::trim)
        .unwrap_or("");

    let (view, legacy_index_path) = if schema_version == DOCS_CONFIG_V1_SCHEMA_VERSION
        || looks_like_v1_config(&raw)
    {
        let config = serde_yaml::from_str::<LegacyDocsConfig>(&content).map_err(|source| {
            DocumentationError::Yaml {
                path: config_path.clone(),
                source,
            }
        })?;
        validate_v1_config(&config_path, &config)?;
        let legacy_index_path = config.index_path.clone();
        (map_v1_to_view(config), Some(legacy_index_path))
    } else {
        let config = serde_yaml::from_str::<DocumentationConfigV2>(&content).map_err(|source| {
            DocumentationError::Yaml {
                path: config_path.clone(),
                source,
            }
        })?;
        validate_v2_config(&config_path, &config)?;
        (map_v2_to_view(config), None)
    };

    Ok(ResolvedDocumentationConfig {
        root: project_root.to_path_buf(),
        config_path,
        config_found: true,
        view,
        legacy_index_path,
    })
}

fn looks_like_v1_config(value: &YamlValue) -> bool {
    yaml_mapping_value(value, &["adrPath", "adr_path"]).is_some()
        || yaml_mapping_value(value, &["specPath", "spec_path"]).is_some()
        || yaml_mapping_value(value, &["indexPath", "index_path"]).is_some()
}

fn yaml_mapping_value<'a>(value: &'a YamlValue, keys: &[&str]) -> Option<&'a YamlValue> {
    let YamlValue::Mapping(mapping) = value else {
        return None;
    };
    for key in keys {
        let candidate = YamlValue::String((*key).to_string());
        if let Some(value) = mapping.get(&candidate) {
            return Some(value);
        }
    }
    None
}

fn map_v1_to_view(config: LegacyDocsConfig) -> DocumentationConfigView {
    DocumentationConfigView {
        schema_version: DOCS_CONFIG_V1_SCHEMA_VERSION.to_string(),
        compatibility_mode: "v1-compat".to_string(),
        authority_roots: DocumentationAuthorityRoots {
            current: vec![config.adr_path, config.spec_path],
            planning: Vec::new(),
            research: Vec::new(),
            generated: Vec::new(),
        },
        entrypoints: vec!["README.md".to_string()],
        derived_surfaces: DocumentationDerivedSurfaces::default(),
        required_frontmatter: vec![
            "title".to_string(),
            "status".to_string(),
            "owner".to_string(),
        ],
        freshness_warnings: DocumentationFreshnessWarnings::default(),
        local_exceptions: config.local_exceptions,
    }
}

fn map_v2_to_view(config: DocumentationConfigV2) -> DocumentationConfigView {
    DocumentationConfigView {
        schema_version: config.schema_version,
        compatibility_mode: "v2".to_string(),
        authority_roots: config.authority_roots,
        entrypoints: config.entrypoints,
        derived_surfaces: config.derived_surfaces,
        required_frontmatter: config.required_frontmatter,
        freshness_warnings: config.freshness_warnings,
        local_exceptions: config.local_exceptions,
    }
}

fn validate_v1_config(path: &Path, config: &LegacyDocsConfig) -> Result<(), DocumentationError> {
    let mut issues = Vec::new();
    if config.schema_version.trim() != DOCS_CONFIG_V1_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be {DOCS_CONFIG_V1_SCHEMA_VERSION}"
        ));
    }
    validate_repo_relative_dir_path("adrPath", &config.adr_path, &mut issues);
    validate_repo_relative_dir_path("specPath", &config.spec_path, &mut issues);
    validate_repo_relative_markdown_path("indexPath", &config.index_path, &mut issues);
    validate_local_exceptions(&config.local_exceptions, &mut issues);
    if !issues.is_empty() {
        return Err(DocumentationError::InvalidConfig {
            path: path.to_path_buf(),
            issues,
        });
    }
    Ok(())
}

fn validate_v2_config(
    path: &Path,
    config: &DocumentationConfigV2,
) -> Result<(), DocumentationError> {
    let mut issues = Vec::new();
    if config.schema_version.trim() != DOCS_CONFIG_V2_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be {DOCS_CONFIG_V2_SCHEMA_VERSION}"
        ));
    }
    for (field, values) in [
        ("authorityRoots.current", &config.authority_roots.current),
        ("authorityRoots.planning", &config.authority_roots.planning),
        ("authorityRoots.research", &config.authority_roots.research),
        (
            "authorityRoots.generated",
            &config.authority_roots.generated,
        ),
    ] {
        for value in values {
            validate_repo_relative_path(field, value, false, &mut issues);
        }
    }
    for entrypoint in &config.entrypoints {
        validate_repo_relative_path("entrypoints", entrypoint, true, &mut issues);
    }
    for (field, values) in [
        (
            "derivedSurfaces.sidebars",
            &config.derived_surfaces.sidebars,
        ),
        (
            "derivedSurfaces.manifests",
            &config.derived_surfaces.manifests,
        ),
        ("derivedSurfaces.llms", &config.derived_surfaces.llms),
        ("derivedSurfaces.bundles", &config.derived_surfaces.bundles),
    ] {
        for value in values {
            validate_repo_relative_path(field, value, true, &mut issues);
        }
    }
    if config.required_frontmatter.is_empty() {
        issues.push("requiredFrontmatter must not be empty".to_string());
    }
    let mut seen_required_frontmatter = BTreeSet::new();
    for field in &config.required_frontmatter {
        let trimmed = field.trim();
        if trimmed.is_empty() {
            issues.push("requiredFrontmatter cannot contain empty values".to_string());
            continue;
        }
        if trimmed != field {
            issues.push(format!(
                "requiredFrontmatter value `{field}` must not contain surrounding whitespace"
            ));
        }
        if !SUPPORTED_REQUIRED_FRONTMATTER_FIELDS.contains(&trimmed) {
            issues.push(format!(
                "requiredFrontmatter value `{trimmed}` is unsupported"
            ));
        }
        if !seen_required_frontmatter.insert(trimmed.to_string()) {
            issues.push(format!(
                "requiredFrontmatter must not contain duplicate value `{trimmed}`"
            ));
        }
    }
    validate_local_exceptions(&config.local_exceptions, &mut issues);
    if !issues.is_empty() {
        return Err(DocumentationError::InvalidConfig {
            path: path.to_path_buf(),
            issues,
        });
    }
    Ok(())
}

fn validate_local_exceptions(values: &[String], issues: &mut Vec<String>) {
    for value in values {
        if value.trim().is_empty() {
            issues.push("localExceptions cannot contain empty values".to_string());
            continue;
        }
        if path_escapes_repo(value) || Path::new(value).is_absolute() {
            issues.push(format!(
                "localExceptions entry `{value}` must stay repo-relative"
            ));
        }
    }
}

fn validate_repo_relative_dir_path(field: &str, value: &str, issues: &mut Vec<String>) {
    validate_repo_relative_path(field, value, false, issues);
}

fn validate_repo_relative_markdown_path(field: &str, value: &str, issues: &mut Vec<String>) {
    validate_repo_relative_path(field, value, true, issues);
    if !value.trim().ends_with(".md") {
        issues.push(format!("{field} must point to a markdown file"));
    }
}

fn validate_repo_relative_path(
    field: &str,
    value: &str,
    allow_file: bool,
    issues: &mut Vec<String>,
) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        issues.push(format!("{field} must not be empty"));
        return;
    }
    if Path::new(trimmed).is_absolute() || path_escapes_repo(trimmed) {
        issues.push(format!("{field} must stay repo-relative"));
        return;
    }
    if !allow_file && trimmed.ends_with(".md") {
        issues.push(format!("{field} must point to a directory"));
    }
}

fn path_escapes_repo(value: &str) -> bool {
    Path::new(value).components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    })
}

fn write_docs_config(
    path: &Path,
    config: &DocumentationConfigV2,
) -> Result<(), DocumentationError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| DocumentationError::Io {
            operation: "create directory",
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let content = serde_yaml::to_string(config).map_err(|source| DocumentationError::Yaml {
        path: path.to_path_buf(),
        source,
    })?;
    fs::write(path, content).map_err(|source| DocumentationError::Io {
        operation: "write",
        path: path.to_path_buf(),
        source,
    })
}

fn collect_documents(
    resolved: &ResolvedDocumentationConfig,
) -> Result<Vec<DocumentationDocument>, DocumentationError> {
    let collected = collect_collected_documents(resolved)?;
    Ok(collected
        .iter()
        .map(|document| project_document(document, &resolved.view.freshness_warnings))
        .collect())
}

fn collect_collected_documents(
    resolved: &ResolvedDocumentationConfig,
) -> Result<Vec<CollectedDocument>, DocumentationError> {
    let mut documents = BTreeMap::new();
    for (authority_class, root) in resolved.configured_roots() {
        let absolute_root = resolved.root.join(&root);
        for path in collect_markdown_files(&absolute_root)? {
            insert_collected_document(resolved, &mut documents, path, authority_class)?;
        }
    }

    if resolved.view.compatibility_mode == "v2" {
        collect_other_documents(resolved, &mut documents)?;
    }

    Ok(documents.into_values().collect())
}

fn collect_other_documents(
    resolved: &ResolvedDocumentationConfig,
    documents: &mut BTreeMap<String, CollectedDocument>,
) -> Result<(), DocumentationError> {
    let docs_root = resolved.root.join("docs");
    for path in collect_markdown_files(&docs_root)? {
        let relative_path = resolved.relative_path(&path);
        if documents.contains_key(&relative_path)
            || resolved.is_exception(&relative_path)
            || resolved.is_derived_surface_path(&relative_path)
        {
            continue;
        }
        if classify_path(&relative_path, &resolved.view.authority_roots) != AuthorityClass::Other {
            continue;
        }
        insert_collected_document(resolved, documents, path, AuthorityClass::Other)?;
    }
    Ok(())
}

fn insert_collected_document(
    resolved: &ResolvedDocumentationConfig,
    documents: &mut BTreeMap<String, CollectedDocument>,
    path: PathBuf,
    authority_class: AuthorityClass,
) -> Result<(), DocumentationError> {
    let relative_path = resolved.relative_path(&path);
    if resolved.is_exception(&relative_path)
        || resolved.is_derived_surface_path(&relative_path)
        || documents.contains_key(&relative_path)
    {
        return Ok(());
    }

    let content = fs::read_to_string(&path).map_err(|source| DocumentationError::Io {
        operation: "read",
        path: path.clone(),
        source,
    })?;
    let (parsed, parse_issue) = match try_parse_document(&content) {
        Ok(parsed) => (parsed, None),
        Err(message) => (fallback_parsed_document(&content), Some(message)),
    };
    documents.insert(
        relative_path.clone(),
        CollectedDocument {
            path,
            relative_path,
            authority_class,
            parsed,
            parse_issue,
        },
    );
    Ok(())
}

fn collect_entrypoints(
    resolved: &ResolvedDocumentationConfig,
) -> Result<Vec<DocumentationEntrypoint>, DocumentationError> {
    let mut entrypoints = Vec::new();
    for entry in &resolved.view.entrypoints {
        let path = resolved.root.join(entry);
        if !path.exists() {
            entrypoints.push(DocumentationEntrypoint {
                path: entry.clone(),
                exists: false,
                authority_class: classify_path(entry, &resolved.view.authority_roots)
                    .as_str()
                    .to_string(),
                title: None,
                summary: None,
            });
            continue;
        }

        let content = fs::read_to_string(&path).map_err(|source| DocumentationError::Io {
            operation: "read",
            path: path.clone(),
            source,
        })?;
        let parsed =
            try_parse_document(&content).unwrap_or_else(|_| fallback_parsed_document(&content));
        entrypoints.push(DocumentationEntrypoint {
            path: entry.clone(),
            exists: true,
            authority_class: classify_path(entry, &resolved.view.authority_roots)
                .as_str()
                .to_string(),
            title: Some(document_title(&parsed)),
            summary: document_summary_text(&parsed),
        });
    }
    Ok(entrypoints)
}

fn build_root_summaries(
    resolved: &ResolvedDocumentationConfig,
    documents: &[DocumentationDocument],
) -> Vec<DocumentationRootSummary> {
    [
        AuthorityClass::Current,
        AuthorityClass::Planning,
        AuthorityClass::Research,
        AuthorityClass::Generated,
    ]
    .into_iter()
    .map(|authority_class| DocumentationRootSummary {
        authority_class: authority_class.as_str().to_string(),
        configured_roots: resolved.roots_for(authority_class).to_vec(),
        discovered_document_count: documents
            .iter()
            .filter(|document| document.authority_class == authority_class.as_str())
            .count(),
    })
    .collect()
}

fn build_recommended_reading_order(
    entrypoints: &[DocumentationEntrypoint],
    documents: &[DocumentationDocument],
) -> Vec<String> {
    let mut order = Vec::new();
    for entrypoint in entrypoints {
        if entrypoint.exists {
            order.push(entrypoint.path.clone());
        }
    }

    for authority_class in [
        AuthorityClass::Current,
        AuthorityClass::Planning,
        AuthorityClass::Research,
        AuthorityClass::Other,
    ] {
        let mut docs = documents
            .iter()
            .filter(|document| document.authority_class == authority_class.as_str())
            .cloned()
            .collect::<Vec<_>>();
        docs.sort_by(|left, right| {
            document_order_key(left)
                .cmp(&document_order_key(right))
                .then(left.path.cmp(&right.path))
        });
        order.extend(docs.into_iter().map(|document| document.path));
    }
    order
}

fn document_order_key(document: &DocumentationDocument) -> u8 {
    match document.doc_kind.as_deref() {
        Some("system") => 0,
        Some("guide") => 1,
        Some("reference") => 2,
        Some("adr") => 3,
        Some("spec") => 4,
        Some("planning") => 5,
        Some("research") => 6,
        Some("generated") => 7,
        _ => 8,
    }
}

fn project_document(
    document: &CollectedDocument,
    freshness_warnings: &DocumentationFreshnessWarnings,
) -> DocumentationDocument {
    let frontmatter = document.parsed.frontmatter.as_ref();
    let doc_kind = document_doc_kind(document);
    let created = document_created(document);
    let updated = document_updated(document);

    DocumentationDocument {
        path: document.relative_path.clone(),
        authority_class: document.authority_class.as_str().to_string(),
        source_of_truth: document.authority_class.source_of_truth().to_string(),
        title: document_title(&document.parsed),
        status: frontmatter
            .and_then(|frontmatter| frontmatter.status.as_ref())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        doc_kind,
        summary: document_summary_text(&document.parsed),
        created,
        updated: updated.clone(),
        schema_version: frontmatter
            .and_then(|frontmatter| frontmatter.schema_version.as_ref())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        freshness: freshness_state(
            document.authority_class,
            updated.as_deref(),
            freshness_warnings,
        )
        .to_string(),
    }
}

fn count_checked_files(
    resolved: &ResolvedDocumentationConfig,
    documents: &[CollectedDocument],
) -> usize {
    let mut files = BTreeSet::new();
    for document in documents {
        files.insert(document.relative_path.clone());
    }
    for entry in &resolved.view.entrypoints {
        let path = resolved.root.join(entry);
        if path.exists() {
            files.insert(entry.clone());
        }
    }
    if let Some(path) = resolved.legacy_index_path() {
        if path.exists() {
            files.insert(resolved.relative_path(&path));
        }
    }
    files.len()
}

fn collect_document_issues(
    resolved: &ResolvedDocumentationConfig,
    documents: &[CollectedDocument],
) -> Result<Vec<DocumentationCheckIssue>, DocumentationError> {
    let mut issues = Vec::new();
    for document in documents {
        issues.extend(validate_document(resolved, document));
    }

    for document in documents {
        let content =
            fs::read_to_string(&document.path).map_err(|source| DocumentationError::Io {
                operation: "read",
                path: document.path.clone(),
                source,
            })?;
        issues.extend(validate_internal_links(
            resolved,
            &document.relative_path,
            &document.path,
            &content,
        ));
    }

    for entry in &resolved.view.entrypoints {
        let path = resolved.root.join(entry);
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path).map_err(|source| DocumentationError::Io {
            operation: "read",
            path: path.clone(),
            source,
        })?;
        issues.extend(validate_internal_links(resolved, entry, &path, &content));
    }

    if let Some(path) = resolved.legacy_index_path() {
        if path.exists() {
            let relative_path = resolved.relative_path(&path);
            let content = fs::read_to_string(&path).map_err(|source| DocumentationError::Io {
                operation: "read",
                path: path.clone(),
                source,
            })?;
            issues.extend(validate_internal_links(
                resolved,
                &relative_path,
                &path,
                &content,
            ));
        }
    }

    Ok(issues)
}

fn validate_document(
    resolved: &ResolvedDocumentationConfig,
    document: &CollectedDocument,
) -> Vec<DocumentationCheckIssue> {
    let mut issues = Vec::new();
    let path = document.relative_path.clone();
    let frontmatter = document.parsed.frontmatter.as_ref();

    if let Some(parse_issue) = &document.parse_issue {
        issues.push(error_issue("DOCS-CHECK-001", &path, parse_issue.clone()));
        return issues;
    }

    if frontmatter.is_none() {
        issues.push(error_issue(
            "DOCS-CHECK-002",
            &path,
            "document must start with YAML frontmatter".to_string(),
        ));
        return issues;
    }

    let Some(frontmatter) = frontmatter else {
        return issues;
    };
    for field in &resolved.view.required_frontmatter {
        if !frontmatter_field_present(frontmatter, document, field) {
            issues.push(error_issue(
                "DOCS-CHECK-003",
                &path,
                format!("frontmatter must include a non-empty `{field}` value"),
            ));
        }
    }

    if resolved.view.compatibility_mode == "v2" {
        let Some(doc_kind) = document_doc_kind(document) else {
            issues.push(error_issue(
                "DOCS-CHECK-004",
                &path,
                "frontmatter must include a supported doc_kind value".to_string(),
            ));
            return issues;
        };
        if !DOC_KIND_VALUES.contains(&doc_kind.as_str()) {
            issues.push(error_issue(
                "DOCS-CHECK-004",
                &path,
                format!("doc_kind `{doc_kind}` is unsupported"),
            ));
        }
        issues.extend(validate_authority_alignment(document, &doc_kind));
    }

    if let Some(status) = frontmatter
        .status
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        if !STATUS_VALUES.contains(&status.as_str()) {
            issues.push(error_issue(
                "DOCS-CHECK-005",
                &path,
                format!("status `{status}` is unsupported"),
            ));
        }
        issues.extend(validate_status_authority_alignment(
            document.authority_class,
            &path,
            &status,
        ));
    }

    for (field, value, code) in [
        ("created", frontmatter.created.as_deref(), "DOCS-CHECK-006"),
        ("updated", frontmatter.updated.as_deref(), "DOCS-CHECK-007"),
    ] {
        if let Some(value) = value {
            if !value.trim().is_empty() && !is_parseable_date(value.trim()) {
                issues.push(error_issue(
                    code,
                    &path,
                    format!("frontmatter field `{field}` must contain a parseable date"),
                ));
            }
        }
    }

    if resolved.view.compatibility_mode == "v1-compat"
        && document_doc_kind(document).as_deref() == Some("adr")
        && frontmatter
            .date
            .as_ref()
            .is_none_or(|value| !is_parseable_date(value.trim()))
    {
        issues.push(error_issue(
            "DOCS-CHECK-008",
            &path,
            "ADR frontmatter must include a parseable date value".to_string(),
        ));
    }

    let freshness = freshness_state(
        document.authority_class,
        document_updated(document).as_deref(),
        &resolved.view.freshness_warnings,
    );
    if freshness == "warning" {
        issues.push(warning_issue(
            "DOCS-CHECK-009",
            &path,
            "document appears stale relative to configured freshness warning defaults".to_string(),
        ));
    }

    issues
}

fn validate_authority_alignment(
    document: &CollectedDocument,
    doc_kind: &str,
) -> Vec<DocumentationCheckIssue> {
    let mut issues = Vec::new();
    let path = document.relative_path.clone();
    match document.authority_class {
        AuthorityClass::Current if ["planning", "research", "generated"].contains(&doc_kind) => {
            issues.push(error_issue(
                "DOCS-CHECK-010",
                &path,
                format!("current authority roots must not classify documents as `{doc_kind}`"),
            ));
        }
        AuthorityClass::Current => {}
        AuthorityClass::Planning if doc_kind != "planning" => issues.push(error_issue(
            "DOCS-CHECK-010",
            &path,
            "planning roots must use doc_kind `planning`".to_string(),
        )),
        AuthorityClass::Research if doc_kind != "research" => issues.push(error_issue(
            "DOCS-CHECK-010",
            &path,
            "research roots must use doc_kind `research`".to_string(),
        )),
        AuthorityClass::Generated if doc_kind != "generated" && doc_kind != "index" => {
            issues.push(error_issue(
                "DOCS-CHECK-010",
                &path,
                "generated roots must use doc_kind `generated` or `index`".to_string(),
            ))
        }
        AuthorityClass::Other => {}
        _ => {}
    }
    issues
}

fn validate_status_authority_alignment(
    authority_class: AuthorityClass,
    path: &str,
    status: &str,
) -> Vec<DocumentationCheckIssue> {
    let mut issues = Vec::new();
    match authority_class {
        AuthorityClass::Current => {
            if CURRENT_DISALLOWED_STATUSES.contains(&status) {
                issues.push(error_issue(
                    "DOCS-CHECK-011",
                    path,
                    format!(
                        "current authority roots must not use planning/research status `{status}`"
                    ),
                ));
            }
        }
        AuthorityClass::Planning | AuthorityClass::Research => {
            if NON_CURRENT_TRUTH_STATUSES.contains(&status) {
                issues.push(error_issue(
                    "DOCS-CHECK-011",
                    path,
                    format!(
                        "planning and research roots must not be promoted with current-truth status `{status}`"
                    ),
                ));
            }
        }
        AuthorityClass::Generated | AuthorityClass::Other => {}
    }
    issues
}

fn validate_internal_links(
    resolved: &ResolvedDocumentationConfig,
    source_relative: &str,
    source_path: &Path,
    content: &str,
) -> Vec<DocumentationCheckIssue> {
    let mut issues = Vec::new();
    for link in extract_markdown_links(content) {
        let Some(target_path) = link_target_path(&link) else {
            continue;
        };
        let candidate = if target_path.starts_with('/') {
            resolved.root.join(target_path.trim_start_matches('/'))
        } else {
            source_path
                .parent()
                .unwrap_or(&resolved.root)
                .join(&target_path)
        };
        if !candidate.exists() {
            issues.push(error_issue(
                "DOCS-CHECK-012",
                source_relative,
                format!("broken internal link `{link}`"),
            ));
        }
    }
    issues
}

fn collect_derived_surface_states(
    resolved: &ResolvedDocumentationConfig,
    documents: &[DocumentationDocument],
) -> Result<Vec<DocumentationDerivedSurfaceState>, DocumentationError> {
    let entrypoints = collect_entrypoints(resolved)?;
    let map = DocumentationMapResult {
        schema_version: DOCUMENTATION_MAP_RESULT_SCHEMA_VERSION.to_string(),
        project_root: display_path(&resolved.root),
        config_found: resolved.config_found,
        config_path: resolved.relative_path(&resolved.config_path),
        config: resolved.view.clone(),
        entrypoints: entrypoints.clone(),
        root_summaries: build_root_summaries(resolved, documents),
        documents: documents.to_vec(),
        derived_surfaces: Vec::new(),
        recommended_reading_order: build_recommended_reading_order(&entrypoints, documents),
    };
    let mut states = Vec::new();
    states.extend(surface_states_for_kind(
        resolved,
        &map,
        "sidebar",
        &resolved.view.derived_surfaces.sidebars,
    )?);
    states.extend(surface_states_for_kind(
        resolved,
        &map,
        "manifest",
        &resolved.view.derived_surfaces.manifests,
    )?);
    states.extend(surface_states_for_kind(
        resolved,
        &map,
        "llms",
        &resolved.view.derived_surfaces.llms,
    )?);
    states.extend(surface_states_for_kind(
        resolved,
        &map,
        "bundle",
        &resolved.view.derived_surfaces.bundles,
    )?);
    states.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.surface_kind.cmp(&right.surface_kind))
    });
    Ok(states)
}

fn surface_states_for_kind(
    resolved: &ResolvedDocumentationConfig,
    map: &DocumentationMapResult,
    surface_kind: &str,
    paths: &[String],
) -> Result<Vec<DocumentationDerivedSurfaceState>, DocumentationError> {
    let mut states = Vec::new();
    for relative_path in paths {
        let absolute_path = resolved.root.join(relative_path);
        let exists = absolute_path.exists();
        let drift_status = match surface_kind {
            "llms" if exists => match fs::read_to_string(&absolute_path) {
                Ok(content) if content == render_llms_export(map) => "current".to_string(),
                Ok(_) => "drifted".to_string(),
                Err(_) => "unreadable".to_string(),
            },
            "bundle" if exists => match fs::read_to_string(&absolute_path) {
                Ok(content) if content == render_bundle_export(map)? => "current".to_string(),
                Ok(_) => "drifted".to_string(),
                Err(_) => "unreadable".to_string(),
            },
            _ if exists => "present".to_string(),
            _ => "missing".to_string(),
        };
        states.push(DocumentationDerivedSurfaceState {
            path: relative_path.clone(),
            surface_kind: surface_kind.to_string(),
            exists,
            drift_status,
        });
    }
    Ok(states)
}

fn collect_derived_surface_issues(
    resolved: &ResolvedDocumentationConfig,
    documents: &[DocumentationDocument],
) -> Result<Vec<DocumentationCheckIssue>, DocumentationError> {
    let states = collect_derived_surface_states(resolved, documents)?;
    let mut issues = Vec::new();
    for state in states {
        match state.surface_kind.as_str() {
            "sidebar" | "manifest" if !state.exists => issues.push(error_issue(
                "DOCS-CHECK-013",
                &state.path,
                format!("configured {} path does not exist", state.surface_kind),
            )),
            "llms" | "bundle" if !state.exists => issues.push(error_issue(
                "DOCS-CHECK-014",
                &state.path,
                format!("configured {} export is missing", state.surface_kind),
            )),
            "llms" | "bundle" if state.drift_status == "drifted" => issues.push(error_issue(
                "DOCS-CHECK-015",
                &state.path,
                format!(
                    "configured {} export has drifted from deterministic output",
                    state.surface_kind
                ),
            )),
            _ => {}
        }
    }
    Ok(issues)
}

fn collect_entrypoint_issues(
    resolved: &ResolvedDocumentationConfig,
    documents: &[DocumentationDocument],
) -> Vec<DocumentationCheckIssue> {
    let mut issues = Vec::new();
    let mut doc_lookup = BTreeMap::new();
    for document in documents {
        doc_lookup.insert(document.path.clone(), document.authority_class.clone());
    }
    for entrypoint in &resolved.view.entrypoints {
        match doc_lookup.get(entrypoint) {
            Some(authority)
                if authority == "planning"
                    || authority == "research"
                    || authority == "generated" =>
            {
                issues.push(error_issue(
                    "DOCS-CHECK-016",
                    entrypoint,
                    "entrypoints must lead with current or neutral navigation, not planning/research/generated material"
                        .to_string(),
                ));
            }
            _ => {}
        }
    }
    issues
}

fn render_llms_export(map: &DocumentationMapResult) -> String {
    let mut output = String::new();
    output.push_str("# llms.txt\n\n");
    output.push_str("Derived by `elegy-documentation`. This file is non-authoritative. Source documents remain the truth.\n\n");
    output.push_str(&format!("Project root: {}\n\n", map.project_root));

    output.push_str("## Entrypoints\n\n");
    if map.entrypoints.is_empty() {
        output.push_str("- None configured.\n\n");
    } else {
        for entrypoint in &map.entrypoints {
            output.push_str(&format!(
                "- {} [{}]{}\n",
                entrypoint.path,
                entrypoint.authority_class,
                entrypoint
                    .summary
                    .as_ref()
                    .map(|summary| format!(": {summary}"))
                    .unwrap_or_default()
            ));
        }
        output.push('\n');
    }

    for authority_class in [
        AuthorityClass::Current,
        AuthorityClass::Planning,
        AuthorityClass::Research,
        AuthorityClass::Other,
    ] {
        output.push_str(&format!("## {}\n\n", section_title(authority_class)));
        let docs = map
            .documents
            .iter()
            .filter(|document| document.authority_class == authority_class.as_str())
            .collect::<Vec<_>>();
        if docs.is_empty() {
            output.push_str("- None.\n\n");
            continue;
        }
        for document in docs {
            output.push_str(&format!(
                "- {} | {} | authority={} | status={} | updated={}{}\n",
                document.path,
                document.title,
                document.authority_class,
                document.status.as_deref().unwrap_or("unknown"),
                document
                    .updated
                    .as_deref()
                    .or(document.created.as_deref())
                    .unwrap_or("unknown"),
                document
                    .summary
                    .as_ref()
                    .map(|summary| format!(" | summary={summary}"))
                    .unwrap_or_default()
            ));
        }
        output.push('\n');
    }

    output.push_str("## Generated Surfaces\n\n");
    let configured_surfaces = configured_surface_lines(&map.config.derived_surfaces);
    if configured_surfaces.is_empty() {
        output.push_str("- None configured.\n");
    } else {
        for line in configured_surfaces {
            output.push_str(&format!("- {line}\n"));
        }
    }

    output
}

fn render_bundle_export(map: &DocumentationMapResult) -> Result<String, DocumentationError> {
    let value = serde_json::json!({
        "schemaVersion": BUNDLE_EXPORT_FILE_SCHEMA_VERSION,
        "authorityPosture": AUTHORITY_POSTURE,
        "projectRoot": map.project_root,
        "configPath": map.config_path,
        "config": map.config,
        "entrypoints": map.entrypoints,
        "documents": map.documents,
        "configuredDerivedSurfaces": map.config.derived_surfaces,
        "recommendedReadingOrder": map.recommended_reading_order,
    });
    serde_json::to_string_pretty(&value).map_err(|source| DocumentationError::Json {
        path: PathBuf::from("<documentation-bundle>"),
        source,
    })
}

fn configured_surface_lines(surfaces: &DocumentationDerivedSurfaces) -> Vec<String> {
    let mut lines = Vec::new();
    for path in &surfaces.sidebars {
        lines.push(format!("{path} | kind=sidebar"));
    }
    for path in &surfaces.manifests {
        lines.push(format!("{path} | kind=manifest"));
    }
    for path in &surfaces.llms {
        lines.push(format!("{path} | kind=llms"));
    }
    for path in &surfaces.bundles {
        lines.push(format!("{path} | kind=bundle"));
    }
    lines.sort();
    lines
}

fn write_output_file(path: &Path, content: &str) -> Result<(), DocumentationError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| DocumentationError::Io {
            operation: "create directory",
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, content).map_err(|source| DocumentationError::Io {
        operation: "write",
        path: path.to_path_buf(),
        source,
    })
}

fn collect_markdown_files(root: &Path) -> Result<Vec<PathBuf>, DocumentationError> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let metadata = fs::metadata(root).map_err(|source| DocumentationError::Io {
        operation: "inspect",
        path: root.to_path_buf(),
        source,
    })?;
    if metadata.is_file() {
        return Ok(
            if root
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
            {
                vec![root.to_path_buf()]
            } else {
                Vec::new()
            },
        );
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(root).map_err(|source| DocumentationError::Io {
        operation: "read directory",
        path: root.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| DocumentationError::Io {
            operation: "read directory entry",
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_markdown_files(&path)?);
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn try_parse_document(content: &str) -> Result<ParsedDocument, String> {
    let normalized = content.replace("\r\n", "\n");
    let Some(stripped) = normalized.strip_prefix("---\n") else {
        let body = body_content_for_fallback(&normalized);
        return Ok(ParsedDocument {
            frontmatter: None,
            title_fallback: extract_title_fallback(body),
            summary_fallback: extract_summary_fallback(body),
        });
    };
    let Some(end_index) = stripped.find("\n---\n") else {
        return Err("invalid document frontmatter: missing closing `---` delimiter".to_string());
    };
    let frontmatter_str = &stripped[..end_index];
    let body = &stripped[end_index + "\n---\n".len()..];
    let frontmatter = serde_yaml::from_str::<DocumentationFrontmatter>(frontmatter_str)
        .map_err(|source| format!("invalid document frontmatter: {source}"))?;
    Ok(ParsedDocument {
        frontmatter: Some(frontmatter),
        title_fallback: extract_title_fallback(body),
        summary_fallback: extract_summary_fallback(body),
    })
}

fn fallback_parsed_document(content: &str) -> ParsedDocument {
    let normalized = content.replace("\r\n", "\n");
    let body = body_content_for_fallback(&normalized);
    ParsedDocument {
        frontmatter: None,
        title_fallback: extract_title_fallback(body),
        summary_fallback: extract_summary_fallback(body),
    }
}

fn body_content_for_fallback(content: &str) -> &str {
    let Some(stripped) = content.strip_prefix("---\n") else {
        return content;
    };
    let Some(end_index) = stripped.find("\n---\n") else {
        return content;
    };
    &stripped[end_index + "\n---\n".len()..]
}

fn extract_title_fallback(content: &str) -> String {
    for line in content.lines() {
        if let Some(title) = line.trim().strip_prefix("# ") {
            let trimmed = title.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    "Untitled document".to_string()
}

fn extract_summary_fallback(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed == "---"
            || trimmed.contains(':') && !trimmed.contains(' ')
        {
            continue;
        }
        return Some(trimmed.to_string());
    }
    None
}

fn document_title(parsed: &ParsedDocument) -> String {
    parsed
        .frontmatter
        .as_ref()
        .and_then(|frontmatter| frontmatter.title.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| parsed.title_fallback.clone())
}

fn document_summary_text(parsed: &ParsedDocument) -> Option<String> {
    parsed
        .frontmatter
        .as_ref()
        .and_then(|frontmatter| frontmatter.summary.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| parsed.summary_fallback.clone())
}

fn document_doc_kind(document: &CollectedDocument) -> Option<String> {
    document
        .parsed
        .frontmatter
        .as_ref()
        .and_then(|frontmatter| frontmatter.doc_kind.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            if document.relative_path.contains("/adr/")
                || document.relative_path.contains("\\adr\\")
            {
                Some("adr".to_string())
            } else if document.relative_path.contains("/spec")
                || document.relative_path.contains("\\spec")
            {
                Some("spec".to_string())
            } else {
                None
            }
        })
}

fn document_created(document: &CollectedDocument) -> Option<String> {
    document
        .parsed
        .frontmatter
        .as_ref()
        .and_then(|frontmatter| {
            frontmatter
                .created
                .as_ref()
                .or(frontmatter.date.as_ref())
                .map(|value| value.trim().to_string())
        })
        .filter(|value| !value.is_empty())
}

fn document_updated(document: &CollectedDocument) -> Option<String> {
    document
        .parsed
        .frontmatter
        .as_ref()
        .and_then(|frontmatter| frontmatter.updated.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn frontmatter_field_present(
    frontmatter: &DocumentationFrontmatter,
    document: &CollectedDocument,
    field: &str,
) -> bool {
    match field {
        "title" => frontmatter
            .title
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty()),
        "summary" => frontmatter
            .summary
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty()),
        "status" => frontmatter
            .status
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty()),
        "doc_kind" => document_doc_kind(document).is_some(),
        "schema_version" => frontmatter
            .schema_version
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty()),
        "created" => document_created(document).is_some(),
        "updated" => document_updated(document).is_some(),
        "owner" => frontmatter
            .owner
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty()),
        _ => false,
    }
}

fn is_parseable_date(value: &str) -> bool {
    parse_dateish(value).is_some()
}

fn parse_dateish(value: &str) -> Option<Date> {
    parse_iso_date(value).or_else(|| {
        OffsetDateTime::parse(value, &Rfc3339)
            .ok()
            .map(|value| value.date())
    })
}

fn parse_iso_date(value: &str) -> Option<Date> {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    if !(bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..10].iter().all(u8::is_ascii_digit))
    {
        return None;
    }

    let year = value[..4].parse::<i32>().ok()?;
    let month = Month::try_from(value[5..7].parse::<u8>().ok()?).ok()?;
    let day = value[8..10].parse::<u8>().ok()?;
    Date::from_calendar_date(year, month, day).ok()
}

fn freshness_state(
    authority_class: AuthorityClass,
    updated: Option<&str>,
    freshness: &DocumentationFreshnessWarnings,
) -> &'static str {
    let Some(updated) = updated else {
        return "unknown";
    };
    let Some(updated_date) = parse_dateish(updated) else {
        return "unknown";
    };
    let Some(threshold_days) = authority_class.freshness_threshold_days(freshness) else {
        return "n/a";
    };
    let today = OffsetDateTime::now_utc().date();
    let age_days = (today.to_julian_day() - updated_date.to_julian_day()).max(0) as u32;
    if age_days > threshold_days {
        "warning"
    } else {
        "fresh"
    }
}

fn extract_markdown_links(content: &str) -> Vec<String> {
    let bytes = content.as_bytes();
    let mut links = Vec::new();
    let mut index = 0;
    while index + 1 < bytes.len() {
        if bytes[index] == b']' && bytes[index + 1] == b'(' {
            let start = index + 2;
            if let Some(end) = find_link_end(bytes, start) {
                let raw = String::from_utf8_lossy(&bytes[start..end])
                    .trim()
                    .to_string();
                if !raw.is_empty() {
                    links.push(raw);
                }
                index = end + 1;
                continue;
            }
        }
        index += 1;
    }
    links
}

fn find_link_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, byte) in bytes[start..].iter().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                if depth == 0 {
                    return Some(start + offset);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

fn link_target_path(link: &str) -> Option<String> {
    let trimmed = link.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let normalized = if trimmed.starts_with('<') && trimmed.ends_with('>') && trimmed.len() > 2 {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed.split_whitespace().next().unwrap_or(trimmed)
    };
    if normalized.contains("://")
        || normalized.starts_with("mailto:")
        || normalized.starts_with("tel:")
        || normalized.starts_with("data:")
    {
        return None;
    }
    let path_only = normalized.split('#').next().unwrap_or_default().trim();
    if path_only.is_empty() {
        return None;
    }
    Some(path_only.to_string())
}

fn classify_path(relative_path: &str, roots: &DocumentationAuthorityRoots) -> AuthorityClass {
    let normalized = normalize_rel_string(relative_path);
    let candidates = [
        (AuthorityClass::Current, &roots.current),
        (AuthorityClass::Planning, &roots.planning),
        (AuthorityClass::Research, &roots.research),
        (AuthorityClass::Generated, &roots.generated),
    ];

    let mut best: Option<(AuthorityClass, usize)> = None;
    for (authority_class, entries) in candidates {
        for entry in entries {
            let entry = normalize_rel_string(entry);
            if normalized == entry || normalized.starts_with(&(entry.clone() + "/")) {
                let score = entry.len();
                if best.is_none_or(|(_, current_score)| score > current_score) {
                    best = Some((authority_class, score));
                }
            }
        }
    }

    best.map(|(authority_class, _)| authority_class)
        .unwrap_or(AuthorityClass::Other)
}

fn normalize_rel_string(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_matches('/')
        .to_ascii_lowercase()
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn path_display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn error_issue(code: &str, path: &str, message: String) -> DocumentationCheckIssue {
    DocumentationCheckIssue {
        code: code.to_string(),
        severity: "error".to_string(),
        path: path.to_string(),
        message,
    }
}

fn warning_issue(code: &str, path: &str, message: String) -> DocumentationCheckIssue {
    DocumentationCheckIssue {
        code: code.to_string(),
        severity: "warning".to_string(),
        path: path.to_string(),
        message,
    }
}

fn render_now_rfc3339() -> Result<String, DocumentationError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| DocumentationError::InvalidRequest {
            issues: vec!["failed to render RFC3339 timestamp".to_string()],
        })
}

fn section_title(authority_class: AuthorityClass) -> &'static str {
    match authority_class {
        AuthorityClass::Current => "Current Canon",
        AuthorityClass::Planning => "Planning",
        AuthorityClass::Research => "Research",
        AuthorityClass::Generated => "Generated",
        AuthorityClass::Other => "Other",
    }
}

fn default_config_schema_version() -> String {
    DOCS_CONFIG_V2_SCHEMA_VERSION.to_string()
}

fn default_current_roots() -> Vec<String> {
    vec![
        "docs/system".to_string(),
        "docs/adr".to_string(),
        "docs/specs".to_string(),
    ]
}

fn default_planning_roots() -> Vec<String> {
    vec!["docs/planning".to_string(), "docs/roadmaps".to_string()]
}

fn default_research_roots() -> Vec<String> {
    vec!["docs/research".to_string()]
}

fn default_generated_roots() -> Vec<String> {
    vec!["docs/generated".to_string()]
}

fn default_entrypoints() -> Vec<String> {
    vec![
        "README.md".to_string(),
        "docs/index.md".to_string(),
        "docs/system/index.md".to_string(),
    ]
}

fn default_required_frontmatter() -> Vec<String> {
    vec![
        "created".to_string(),
        "updated".to_string(),
        "status".to_string(),
        "doc_kind".to_string(),
        "summary".to_string(),
        "schema_version".to_string(),
    ]
}

fn default_current_warning_days() -> u32 {
    120
}

fn default_planning_warning_days() -> u32 {
    45
}

fn default_research_warning_days() -> u32 {
    90
}

#[cfg(test)]
mod tests {
    use super::{
        documentation_check, documentation_export_bundle, documentation_export_llms,
        documentation_init, documentation_map, DocumentationError, DOCS_CONFIG_PATH,
    };
    use std::fs;
    use std::path::PathBuf;
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

    #[test]
    fn init_writes_default_v2_config() {
        let root = unique_temp_dir("elegy-documentation-init");
        let result = documentation_init(&root, false).expect("init docs config");
        assert!(result.created.iter().any(|path| path == DOCS_CONFIG_PATH));
        assert!(root.join(DOCS_CONFIG_PATH).is_file());
    }

    #[test]
    fn map_classifies_holon_shaped_docs() {
        let root = unique_temp_dir("elegy-documentation-map");
        fs::create_dir_all(root.join(".elegy")).expect("create config dir");
        fs::create_dir_all(root.join("docs/system")).expect("create system dir");
        fs::create_dir_all(root.join("docs/planning")).expect("create planning dir");
        fs::create_dir_all(root.join("docs/research")).expect("create research dir");
        fs::write(
            root.join(DOCS_CONFIG_PATH),
            "schemaVersion: elegy-documentation/v2\nauthorityRoots:\n  current:\n    - docs/system\n  planning:\n    - docs/planning\n  research:\n    - docs/research\nentrypoints:\n  - README.md\nrequiredFrontmatter:\n  - created\n  - updated\n  - status\n  - doc_kind\n  - summary\n  - schema_version\n",
        )
        .expect("write config");
        fs::write(root.join("README.md"), "# Holon\n\nProject entrypoint.\n")
            .expect("write readme");
        fs::write(
            root.join("docs/system/index.md"),
            "---\ncreated: 2026-05-01\nupdated: 2026-05-20\nstatus: active\ndoc_kind: system\nsummary: System overview.\nschema_version: documentation-doc/v1\n---\n\n# System\n",
        )
        .expect("write system doc");
        fs::write(
            root.join("docs/planning/roadmap.md"),
            "---\ncreated: 2026-05-01\nupdated: 2026-05-20\nstatus: planned\ndoc_kind: planning\nsummary: Planning overview.\nschema_version: documentation-doc/v1\n---\n\n# Planning\n",
        )
        .expect("write planning doc");
        fs::write(
            root.join("docs/research/note.md"),
            "---\ncreated: 2026-05-01\nupdated: 2026-05-20\nstatus: exploratory\ndoc_kind: research\nsummary: Research note.\nschema_version: documentation-doc/v1\n---\n\n# Research\n",
        )
        .expect("write research doc");

        let result = documentation_map(&root).expect("map docs");
        assert_eq!(result.documents.len(), 3);
        assert!(result
            .documents
            .iter()
            .any(|document| document.authority_class == "current"));
        assert!(result
            .documents
            .iter()
            .any(|document| document.authority_class == "planning"));
        assert!(result
            .documents
            .iter()
            .any(|document| document.authority_class == "research"));
    }

    #[test]
    fn check_supports_v1_compatibility() {
        let root = unique_temp_dir("elegy-documentation-v1");
        fs::create_dir_all(root.join(".elegy")).expect("create config dir");
        fs::create_dir_all(root.join("docs/adr")).expect("create adr dir");
        fs::write(
            root.join(DOCS_CONFIG_PATH),
            "schemaVersion: elegy-docs/v1\nadrPath: docs/adr\nspecPath: docs/specs\nindexPath: docs/docs-index.md\nrequiredDocTriggers:\n  - architecture-change\nlocalExceptions: []\n",
        )
        .expect("write legacy config");
        fs::write(
            root.join("docs/adr/2026-05-25-centralize.md"),
            "---\ntitle: Centralize\nstatus: accepted\ndate: 2026-05-25\nowner: Elegy\n---\n\n# Centralize\n\n## Context\n\n- Context.\n",
        )
        .expect("write adr");

        let result = documentation_check(&root).expect("check docs");
        assert_eq!(result.config.compatibility_mode, "v1-compat");
        assert!(result.valid, "issues: {:?}", result.issues);
        assert_eq!(result.document_count, 1);
    }

    #[test]
    fn map_includes_docs_outside_configured_roots_as_other() {
        let root = unique_temp_dir("elegy-documentation-other");
        fs::create_dir_all(root.join(".elegy")).expect("create config dir");
        fs::create_dir_all(root.join("docs/system")).expect("create system dir");
        fs::create_dir_all(root.join("docs/misc")).expect("create misc dir");
        fs::write(
            root.join(DOCS_CONFIG_PATH),
            "schemaVersion: elegy-documentation/v2\nauthorityRoots:\n  current:\n    - docs/system\n  planning: []\n  research: []\n  generated: []\nentrypoints:\n  - README.md\nderivedSurfaces:\n  sidebars: []\n  manifests: []\n  llms: []\n  bundles: []\nrequiredFrontmatter:\n  - created\n  - updated\n  - status\n  - doc_kind\n  - summary\n  - schema_version\nfreshnessWarnings:\n  currentDays: 120\n  planningDays: 45\n  researchDays: 90\n",
        )
        .expect("write config");
        fs::write(
            root.join("docs/system/index.md"),
            "---\ncreated: 2026-05-01\nupdated: 2026-05-20\nstatus: active\ndoc_kind: system\nsummary: System overview.\nschema_version: documentation-doc/v1\n---\n\n# System\n",
        )
        .expect("write system doc");
        fs::write(
            root.join("docs/misc/note.md"),
            "---\ncreated: 2026-05-01\nupdated: 2026-05-20\nstatus: draft\ndoc_kind: guide\nsummary: Misc note.\nschema_version: documentation-doc/v1\n---\n\n# Misc\n",
        )
        .expect("write other doc");

        let result = documentation_map(&root).expect("map docs");
        assert!(result
            .documents
            .iter()
            .any(|document| document.path == "docs/misc/note.md"
                && document.authority_class == "other"
                && document.source_of_truth == "unclassified"));
    }

    #[test]
    fn map_rejects_unsupported_required_frontmatter_values() {
        let root = unique_temp_dir("elegy-documentation-invalid-frontmatter");
        fs::create_dir_all(root.join(".elegy")).expect("create config dir");
        fs::write(
            root.join(DOCS_CONFIG_PATH),
            "schemaVersion: elegy-documentation/v2\nauthorityRoots:\n  current:\n    - docs/system\n  planning: []\n  research: []\n  generated: []\nentrypoints:\n  - README.md\nderivedSurfaces:\n  sidebars: []\n  manifests: []\n  llms: []\n  bundles: []\nrequiredFrontmatter:\n  - bogus_field\nfreshnessWarnings:\n  currentDays: 120\n  planningDays: 45\n  researchDays: 90\n",
        )
        .expect("write config");

        let error = documentation_map(&root).expect_err("map should reject invalid config");
        match error {
            DocumentationError::InvalidConfig { issues, .. } => {
                assert!(issues.iter().any(|issue| {
                    issue.contains("requiredFrontmatter value `bogus_field` is unsupported")
                }));
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn map_summary_fallback_uses_body_content() {
        let root = unique_temp_dir("elegy-documentation-summary");
        fs::create_dir_all(root.join(".elegy")).expect("create config dir");
        fs::create_dir_all(root.join("docs/adr")).expect("create adr dir");
        fs::write(
            root.join(DOCS_CONFIG_PATH),
            "schemaVersion: elegy-docs/v1\nadrPath: docs/adr\nspecPath: docs/specs\nindexPath: docs/docs-index.md\nrequiredDocTriggers:\n  - architecture-change\nlocalExceptions: []\n",
        )
        .expect("write legacy config");
        fs::write(
            root.join("docs/adr/2026-05-25-centralize.md"),
            "---\ntitle: Centralize\nstatus: accepted\ndate: 2026-05-25\nowner: Elegy\n---\n\n# Centralize\n\nA real body summary.\n",
        )
        .expect("write adr");

        let result = documentation_map(&root).expect("map docs");
        assert_eq!(result.documents.len(), 1);
        assert_eq!(
            result.documents[0].summary.as_deref(),
            Some("A real body summary.")
        );
    }

    #[test]
    fn check_reports_drift_for_llms_export() {
        let root = unique_temp_dir("elegy-documentation-drift");
        fs::create_dir_all(root.join(".elegy")).expect("create config dir");
        fs::create_dir_all(root.join("docs/system")).expect("create system dir");
        fs::write(
            root.join(DOCS_CONFIG_PATH),
            "schemaVersion: elegy-documentation/v2\nauthorityRoots:\n  current:\n    - docs/system\nderivedSurfaces:\n  llms:\n    - llms.txt\nrequiredFrontmatter:\n  - created\n  - updated\n  - status\n  - doc_kind\n  - summary\n  - schema_version\n",
        )
        .expect("write config");
        fs::write(
            root.join("docs/system/index.md"),
            "---\ncreated: 2026-05-01\nupdated: 2026-05-20\nstatus: active\ndoc_kind: system\nsummary: System overview.\nschema_version: documentation-doc/v1\n---\n\n# System\n",
        )
        .expect("write system doc");
        let output = root.join("llms.txt");
        documentation_export_llms(&root, &output).expect("export llms");
        fs::write(&output, "drifted").expect("rewrite llms file");

        let result = documentation_check(&root).expect("check docs");
        assert!(!result.valid);
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.code == "DOCS-CHECK-015"));
    }

    #[test]
    fn bundle_export_is_deterministic() {
        let root = unique_temp_dir("elegy-documentation-bundle");
        fs::create_dir_all(root.join("docs/system")).expect("create system dir");
        fs::write(
            root.join("docs/system/index.md"),
            "---\ncreated: 2026-05-01\nupdated: 2026-05-20\nstatus: active\ndoc_kind: system\nsummary: System overview.\nschema_version: documentation-doc/v1\n---\n\n# System\n",
        )
        .expect("write system doc");
        let output = root.join("bundle.json");
        let first = documentation_export_bundle(&root, &output).expect("export bundle");
        let first_content = fs::read_to_string(&output).expect("read bundle");
        let second = documentation_export_bundle(&root, &output).expect("export bundle again");
        let second_content = fs::read_to_string(&output).expect("read bundle again");
        assert_eq!(first.document_count, second.document_count);
        assert_eq!(first_content, second_content);
    }
}
