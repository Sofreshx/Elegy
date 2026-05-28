use crate::{display_path, write_text_file, ToolingError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use time::format_description;
use time::OffsetDateTime;

const DOCS_CONFIG_SCHEMA_VERSION: &str = "elegy-docs/v1";
const DOCS_CONFIG_PATH: &str = ".elegy/docs.yaml";
const DEFAULT_ADR_PATH: &str = "docs/adr";
const DEFAULT_SPEC_PATH: &str = "docs/specs";
const DEFAULT_INDEX_PATH: &str = "docs/docs-index.md";
const DEFAULT_OWNER: &str = "TBD";
const DEFAULT_ADR_STATUS: &str = "proposed";
const DEFAULT_SPEC_STATUS: &str = "draft";
const ADR_ALLOWED_STATUSES: &[&str] = &["proposed", "accepted", "superseded", "rejected"];
const SPEC_ALLOWED_STATUSES: &[&str] =
    &["draft", "active", "completed", "superseded", "deprecated"];
const KNOWN_REQUIRED_DOC_TRIGGERS: &[&str] = &[
    "architecture-change",
    "durable-decision",
    "behavior-change",
    "cross-repo-impact",
    "onboarding-change",
];

/// Repo-local documentation practices configuration.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocsConfig {
    #[serde(default = "default_docs_schema_version", alias = "schema_version")]
    pub schema_version: String,
    #[serde(default = "default_adr_path", alias = "adr_path")]
    pub adr_path: String,
    #[serde(default = "default_spec_path", alias = "spec_path")]
    pub spec_path: String,
    #[serde(default = "default_index_path", alias = "index_path")]
    pub index_path: String,
    #[serde(
        default = "default_required_doc_triggers",
        alias = "required_doc_triggers"
    )]
    pub required_doc_triggers: Vec<String>,
    #[serde(default, alias = "local_exceptions")]
    pub local_exceptions: Vec<String>,
}

impl Default for DocsConfig {
    fn default() -> Self {
        Self {
            schema_version: default_docs_schema_version(),
            adr_path: default_adr_path(),
            spec_path: default_spec_path(),
            index_path: default_index_path(),
            required_doc_triggers: default_required_doc_triggers(),
            local_exceptions: Vec::new(),
        }
    }
}

/// Input for `elegy docs new ...`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewDocRequest {
    pub title: String,
    pub owner: Option<String>,
    pub slug: Option<String>,
    pub status: Option<String>,
}

/// Result of `elegy docs init`.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocsInitResult {
    pub root_path: String,
    pub config_found: bool,
    pub config_path: String,
    pub config: DocsConfig,
    pub created: Vec<String>,
    pub skipped: Vec<String>,
}

/// Result of `elegy docs new adr|spec`.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocsCreateResult {
    pub root_path: String,
    pub doc_type: String,
    pub title: String,
    pub status: String,
    pub owner: String,
    pub slug: String,
    pub output_path: String,
    pub config_path: String,
}

/// One discovered ADR or spec document.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocsDocumentSummary {
    pub doc_type: String,
    pub path: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

/// One objective documentation validation issue.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocsCheckIssue {
    pub code: String,
    pub path: String,
    pub message: String,
}

/// Result of `elegy docs check`.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocsCheckReport {
    pub valid: bool,
    pub root_path: String,
    pub config_found: bool,
    pub config_path: String,
    pub config: DocsConfig,
    pub files_checked: usize,
    pub docs_checked: usize,
    pub documents: Vec<DocsDocumentSummary>,
    pub issues: Vec<DocsCheckIssue>,
}

/// Result of `elegy docs index`.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocsIndexResult {
    pub root_path: String,
    pub config_found: bool,
    pub config_path: String,
    pub output_path: String,
    pub adr_count: usize,
    pub spec_count: usize,
    pub documents: Vec<DocsDocumentSummary>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DocKind {
    Adr,
    Spec,
}

impl DocKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Adr => "adr",
            Self::Spec => "spec",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Adr => "ADR",
            Self::Spec => "Spec",
        }
    }

    fn heading_requirements(self) -> &'static [&'static str] {
        match self {
            Self::Adr => &[
                "Context",
                "Decision",
                "Alternatives",
                "Consequences",
                "Links",
            ],
            Self::Spec => &[
                "Problem",
                "Goals",
                "Non-Goals",
                "Behavior",
                "Acceptance Criteria",
                "Validation",
                "Links",
            ],
        }
    }

    fn allowed_statuses(self) -> &'static [&'static str] {
        match self {
            Self::Adr => ADR_ALLOWED_STATUSES,
            Self::Spec => SPEC_ALLOWED_STATUSES,
        }
    }

    fn default_status(self) -> &'static str {
        match self {
            Self::Adr => DEFAULT_ADR_STATUS,
            Self::Spec => DEFAULT_SPEC_STATUS,
        }
    }

    fn file_name(self, slug: &str, date: &str) -> String {
        match self {
            Self::Adr => format!("{date}-{slug}.md"),
            Self::Spec => format!("{slug}.md"),
        }
    }
}

#[derive(Clone, Debug)]
struct ResolvedDocsConfig {
    root: PathBuf,
    config_path: PathBuf,
    config_found: bool,
    config: DocsConfig,
}

impl ResolvedDocsConfig {
    fn adr_dir(&self) -> PathBuf {
        self.root.join(&self.config.adr_path)
    }

    fn spec_dir(&self) -> PathBuf {
        self.root.join(&self.config.spec_path)
    }

    fn index_path(&self) -> PathBuf {
        self.root.join(&self.config.index_path)
    }

    fn relative_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .map(relative_display_path)
            .unwrap_or_else(|_| display_path(path))
    }

    fn is_exception(&self, relative_path: &str) -> bool {
        let normalized = normalize_rel_string(relative_path);
        self.config.local_exceptions.iter().any(|entry| {
            let exception = normalize_rel_string(entry);
            normalized == exception || normalized.starts_with(&(exception + "/"))
        })
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct DocFrontmatter {
    title: Option<String>,
    status: Option<String>,
    date: Option<String>,
    owner: Option<String>,
}

#[derive(Clone, Debug)]
struct ParsedDocument {
    frontmatter: Option<DocFrontmatter>,
    body: String,
    title_fallback: String,
}

/// Create the default repo-local docs configuration and seed files.
pub fn docs_init(project_root: &Path) -> Result<DocsInitResult, ToolingError> {
    let resolved = load_docs_config(project_root)?;
    let mut created = Vec::new();
    let mut skipped = Vec::new();

    if !resolved.config_found {
        write_docs_config(&resolved.config_path, &resolved.config)?;
        created.push(resolved.relative_path(&resolved.config_path));
    } else {
        skipped.push(resolved.relative_path(&resolved.config_path));
    }

    seed_file_if_missing(
        &resolved,
        &resolved.adr_dir().join("README.md"),
        &render_lane_readme(DocKind::Adr),
        &mut created,
        &mut skipped,
    )?;
    seed_file_if_missing(
        &resolved,
        &resolved.spec_dir().join("README.md"),
        &render_lane_readme(DocKind::Spec),
        &mut created,
        &mut skipped,
    )?;

    let index_content = render_index_content(&resolved, &collect_documents(&resolved)?);
    seed_file_if_missing(
        &resolved,
        &resolved.index_path(),
        &index_content,
        &mut created,
        &mut skipped,
    )?;

    Ok(DocsInitResult {
        root_path: display_path(project_root),
        config_found: resolved.config_found,
        config_path: resolved.relative_path(&resolved.config_path),
        config: resolved.config,
        created,
        skipped,
    })
}

/// Create a new ADR markdown file from the compact template.
pub fn docs_new_adr(
    project_root: &Path,
    request: NewDocRequest,
) -> Result<DocsCreateResult, ToolingError> {
    docs_new(project_root, request, DocKind::Adr)
}

/// Create a new spec markdown file from the compact template.
pub fn docs_new_spec(
    project_root: &Path,
    request: NewDocRequest,
) -> Result<DocsCreateResult, ToolingError> {
    docs_new(project_root, request, DocKind::Spec)
}

/// Validate the configured ADR/spec files plus objective internal-link checks.
pub fn docs_check(project_root: &Path) -> Result<DocsCheckReport, ToolingError> {
    let resolved = load_docs_config(project_root)?;
    let documents = collect_documents_for_check(&resolved)?;
    let issues = collect_validation_issues(&resolved, &documents)?;
    let files_checked = collect_link_check_files(&resolved)?.len();
    let docs_checked = documents.len();

    Ok(DocsCheckReport {
        valid: issues.is_empty(),
        root_path: display_path(project_root),
        config_found: resolved.config_found,
        config_path: resolved.relative_path(&resolved.config_path),
        config: resolved.config,
        files_checked,
        docs_checked,
        documents: documents.into_iter().map(|doc| doc.summary).collect(),
        issues,
    })
}

/// Regenerate the configured documentation index file.
pub fn docs_index(project_root: &Path) -> Result<DocsIndexResult, ToolingError> {
    let resolved = load_docs_config(project_root)?;
    let documents = collect_documents(&resolved)?;
    let content = render_index_content(&resolved, &documents);
    let output_path = resolved.index_path();
    write_text_file(&output_path, &content, true)?;

    let adr_count = documents
        .iter()
        .filter(|doc| doc.kind == DocKind::Adr)
        .count();
    let spec_count = documents
        .iter()
        .filter(|doc| doc.kind == DocKind::Spec)
        .count();

    Ok(DocsIndexResult {
        root_path: display_path(project_root),
        config_found: resolved.config_found,
        config_path: resolved.relative_path(&resolved.config_path),
        output_path: resolved.relative_path(&output_path),
        adr_count,
        spec_count,
        documents: documents.into_iter().map(|doc| doc.summary).collect(),
    })
}

#[derive(Clone, Debug)]
struct CollectedDocument {
    kind: DocKind,
    summary: DocsDocumentSummary,
    path: PathBuf,
    parsed: ParsedDocument,
    parse_issue: Option<DocsCheckIssue>,
}

fn docs_new(
    project_root: &Path,
    request: NewDocRequest,
    kind: DocKind,
) -> Result<DocsCreateResult, ToolingError> {
    let resolved = load_docs_config(project_root)?;
    let title = trimmed_required("title", request.title)?;
    let slug = match request.slug {
        Some(slug) => validate_slug(trimmed_required("slug", slug)?),
        None => validate_slug(slugify(&title)),
    }?;
    let owner = request
        .owner
        .map(|value| trimmed_required("owner", value))
        .transpose()?
        .unwrap_or_else(|| DEFAULT_OWNER.to_string());
    let status = request
        .status
        .map(|value| trimmed_required("status", value))
        .transpose()?
        .unwrap_or_else(|| kind.default_status().to_string());
    validate_status(kind, &status, None)?;

    let date = current_utc_date()?;
    let file_name = kind.file_name(&slug, &date);
    let output_path = match kind {
        DocKind::Adr => resolved.adr_dir().join(file_name),
        DocKind::Spec => resolved.spec_dir().join(file_name),
    };
    let content = match kind {
        DocKind::Adr => render_adr_template(&title, &status, &owner, &date),
        DocKind::Spec => render_spec_template(&title, &status, &owner),
    };
    write_text_file(&output_path, &content, false)?;

    Ok(DocsCreateResult {
        root_path: display_path(project_root),
        doc_type: kind.as_str().to_string(),
        title,
        status,
        owner,
        slug,
        output_path: resolved.relative_path(&output_path),
        config_path: resolved.relative_path(&resolved.config_path),
    })
}

fn load_docs_config(project_root: &Path) -> Result<ResolvedDocsConfig, ToolingError> {
    let config_path = project_root.join(DOCS_CONFIG_PATH);
    let (config_found, config) = if config_path.exists() {
        let content = fs::read_to_string(&config_path).map_err(|source| ToolingError::Io {
            operation: "read",
            path: config_path.clone(),
            source,
        })?;
        let parsed =
            serde_yaml::from_str::<DocsConfig>(&content).map_err(|source| ToolingError::Yaml {
                path: config_path.clone(),
                source,
            })?;
        validate_docs_config(&config_path, &parsed)?;
        (true, parsed)
    } else {
        (false, DocsConfig::default())
    };

    Ok(ResolvedDocsConfig {
        root: project_root.to_path_buf(),
        config_path,
        config_found,
        config,
    })
}

fn validate_docs_config(path: &Path, config: &DocsConfig) -> Result<(), ToolingError> {
    let mut issues = Vec::new();

    if config.schema_version.trim() != DOCS_CONFIG_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be {DOCS_CONFIG_SCHEMA_VERSION}"
        ));
    }
    validate_config_dir_path("adrPath", &config.adr_path, &mut issues);
    validate_config_dir_path("specPath", &config.spec_path, &mut issues);
    validate_config_file_path("indexPath", &config.index_path, &mut issues);

    for trigger in &config.required_doc_triggers {
        if !KNOWN_REQUIRED_DOC_TRIGGERS.contains(&trigger.as_str()) {
            issues.push(format!(
                "requiredDocTriggers contains unsupported value `{trigger}`"
            ));
        }
    }

    for exception in &config.local_exceptions {
        if exception.trim().is_empty() {
            issues.push("localExceptions cannot contain empty entries".to_string());
            continue;
        }
        if path_escapes_repo(exception) || Path::new(exception).is_absolute() {
            issues.push(format!(
                "localExceptions entry `{exception}` must stay repo-relative"
            ));
        }
    }

    if !issues.is_empty() {
        return Err(ToolingError::InvalidDocsConfig {
            path: path.to_path_buf(),
            issues,
        });
    }

    Ok(())
}

fn validate_config_dir_path(field: &str, value: &str, issues: &mut Vec<String>) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        issues.push(format!("{field} must not be empty"));
        return;
    }
    if Path::new(trimmed).is_absolute() || path_escapes_repo(trimmed) {
        issues.push(format!("{field} must stay repo-relative"));
    }
    if trimmed.ends_with(".md") {
        issues.push(format!("{field} must point to a directory"));
    }
}

fn validate_config_file_path(field: &str, value: &str, issues: &mut Vec<String>) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        issues.push(format!("{field} must not be empty"));
        return;
    }
    if Path::new(trimmed).is_absolute() || path_escapes_repo(trimmed) {
        issues.push(format!("{field} must stay repo-relative"));
    }
    if !trimmed.ends_with(".md") {
        issues.push(format!("{field} must point to a markdown file"));
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

fn write_docs_config(path: &Path, config: &DocsConfig) -> Result<(), ToolingError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ToolingError::Io {
            operation: "create directory",
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let content = serde_yaml::to_string(config).map_err(|source| ToolingError::Yaml {
        path: path.to_path_buf(),
        source,
    })?;
    write_text_file(path, &content, false)
}

fn seed_file_if_missing(
    resolved: &ResolvedDocsConfig,
    path: &Path,
    content: &str,
    created: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<(), ToolingError> {
    if path.exists() {
        skipped.push(resolved.relative_path(path));
        return Ok(());
    }
    write_text_file(path, content, false)?;
    created.push(resolved.relative_path(path));
    Ok(())
}

fn collect_documents(
    resolved: &ResolvedDocsConfig,
) -> Result<Vec<CollectedDocument>, ToolingError> {
    let mut documents = Vec::new();
    documents.extend(collect_documents_in_dir(
        resolved,
        &resolved.adr_dir(),
        DocKind::Adr,
    )?);
    documents.extend(collect_documents_in_dir(
        resolved,
        &resolved.spec_dir(),
        DocKind::Spec,
    )?);
    documents.sort_by(|left, right| left.summary.path.cmp(&right.summary.path));
    Ok(documents)
}

fn collect_documents_for_check(
    resolved: &ResolvedDocsConfig,
) -> Result<Vec<CollectedDocument>, ToolingError> {
    let mut documents = Vec::new();
    documents.extend(collect_documents_in_dir_for_check(
        resolved,
        &resolved.adr_dir(),
        DocKind::Adr,
    )?);
    documents.extend(collect_documents_in_dir_for_check(
        resolved,
        &resolved.spec_dir(),
        DocKind::Spec,
    )?);
    documents.sort_by(|left, right| left.summary.path.cmp(&right.summary.path));
    Ok(documents)
}

fn collect_documents_in_dir(
    resolved: &ResolvedDocsConfig,
    dir: &Path,
    kind: DocKind,
) -> Result<Vec<CollectedDocument>, ToolingError> {
    let mut documents = Vec::new();
    for path in collect_markdown_files(dir)? {
        let relative_path = resolved.relative_path(&path);
        if resolved.is_exception(&relative_path) {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("README.md"))
        {
            continue;
        }
        let content = fs::read_to_string(&path).map_err(|source| ToolingError::Io {
            operation: "read",
            path: path.clone(),
            source,
        })?;
        let parsed = parse_document(&content)?;
        let summary = build_document_summary(kind, &relative_path, &parsed);
        documents.push(CollectedDocument {
            kind,
            summary,
            path,
            parsed,
            parse_issue: None,
        });
    }
    Ok(documents)
}

fn collect_documents_in_dir_for_check(
    resolved: &ResolvedDocsConfig,
    dir: &Path,
    kind: DocKind,
) -> Result<Vec<CollectedDocument>, ToolingError> {
    let mut documents = Vec::new();
    for path in collect_markdown_files(dir)? {
        let relative_path = resolved.relative_path(&path);
        if resolved.is_exception(&relative_path) {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("README.md"))
        {
            continue;
        }
        let content = fs::read_to_string(&path).map_err(|source| ToolingError::Io {
            operation: "read",
            path: path.clone(),
            source,
        })?;
        let (parsed, parse_issue) = match try_parse_document(&content) {
            Ok(parsed) => (parsed, None),
            Err(message) => {
                let issue = DocsCheckIssue {
                    code: "DOCS-CHECK-009".to_string(),
                    path: relative_path.clone(),
                    message,
                };
                (fallback_parsed_document(&content), Some(issue))
            }
        };
        let summary = build_document_summary(kind, &relative_path, &parsed);
        documents.push(CollectedDocument {
            kind,
            summary,
            path,
            parsed,
            parse_issue,
        });
    }
    Ok(documents)
}

fn collect_markdown_files(dir: &Path) -> Result<Vec<PathBuf>, ToolingError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let metadata = fs::metadata(dir).map_err(|source| ToolingError::Io {
        operation: "inspect",
        path: dir.to_path_buf(),
        source,
    })?;
    if !metadata.is_dir() {
        return Err(ToolingError::InvalidDocsConfig {
            path: dir.to_path_buf(),
            issues: vec!["configured docs path must point to a directory".to_string()],
        });
    }

    let mut files = Vec::new();
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

fn collect_validation_issues(
    resolved: &ResolvedDocsConfig,
    documents: &[CollectedDocument],
) -> Result<Vec<DocsCheckIssue>, ToolingError> {
    let mut issues = Vec::new();
    for document in documents {
        issues.extend(validate_document(resolved, document));
    }

    let link_files = collect_link_check_files(resolved)?;
    for path in link_files {
        let relative_path = resolved.relative_path(&path);
        if resolved.is_exception(&relative_path) {
            continue;
        }
        let content = fs::read_to_string(&path).map_err(|source| ToolingError::Io {
            operation: "read",
            path: path.clone(),
            source,
        })?;
        issues.extend(validate_internal_links(resolved, &path, &content));
    }

    issues.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });
    Ok(issues)
}

fn collect_link_check_files(resolved: &ResolvedDocsConfig) -> Result<Vec<PathBuf>, ToolingError> {
    let mut files = BTreeSet::new();
    for path in collect_markdown_files(&resolved.adr_dir())? {
        files.insert(path);
    }
    for path in collect_markdown_files(&resolved.spec_dir())? {
        files.insert(path);
    }
    let index_path = resolved.index_path();
    if index_path.exists() {
        files.insert(index_path);
    }
    Ok(files.into_iter().collect())
}

fn validate_document(
    resolved: &ResolvedDocsConfig,
    document: &CollectedDocument,
) -> Vec<DocsCheckIssue> {
    let mut issues = Vec::new();
    let relative_path = document.summary.path.clone();
    let file_name = document
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if !matches_file_name(document.kind, file_name) {
        issues.push(DocsCheckIssue {
            code: "DOCS-CHECK-001".to_string(),
            path: relative_path.clone(),
            message: format!(
                "{} filename must follow the configured convention",
                document.kind.display_name()
            ),
        });
    }

    if let Some(parse_issue) = &document.parse_issue {
        issues.push(parse_issue.clone());
        return issues;
    }

    let Some(frontmatter) = document.parsed.frontmatter.as_ref() else {
        issues.push(DocsCheckIssue {
            code: "DOCS-CHECK-002".to_string(),
            path: relative_path,
            message: "document must start with YAML frontmatter".to_string(),
        });
        return issues;
    };

    if frontmatter
        .title
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        issues.push(DocsCheckIssue {
            code: "DOCS-CHECK-003".to_string(),
            path: document.summary.path.clone(),
            message: "frontmatter must include a non-empty title".to_string(),
        });
    }
    if frontmatter
        .owner
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        issues.push(DocsCheckIssue {
            code: "DOCS-CHECK-004".to_string(),
            path: document.summary.path.clone(),
            message: "frontmatter must include a non-empty owner".to_string(),
        });
    }

    match validate_status(
        document.kind,
        frontmatter.status.as_deref().unwrap_or(""),
        Some(&document.path),
    ) {
        Ok(()) => {}
        Err(ToolingError::InvalidDocsRequest {
            issues: status_issues,
        }) => {
            for issue in status_issues {
                issues.push(DocsCheckIssue {
                    code: "DOCS-CHECK-005".to_string(),
                    path: document.summary.path.clone(),
                    message: issue,
                });
            }
        }
        Err(_) => {}
    }

    if document.kind == DocKind::Adr {
        if frontmatter
            .date
            .as_ref()
            .map(|value| !is_valid_iso_date(value.trim()))
            .unwrap_or(true)
        {
            issues.push(DocsCheckIssue {
                code: "DOCS-CHECK-006".to_string(),
                path: document.summary.path.clone(),
                message: "ADR frontmatter must include a YYYY-MM-DD date".to_string(),
            });
        }
    }

    let headings = collect_second_level_headings(&document.parsed.body);
    for required_heading in document.kind.heading_requirements() {
        if !headings.iter().any(|heading| heading == required_heading) {
            issues.push(DocsCheckIssue {
                code: "DOCS-CHECK-007".to_string(),
                path: document.summary.path.clone(),
                message: format!("missing required heading `## {required_heading}`"),
            });
        }
    }

    if resolved.is_exception(&document.summary.path) {
        return Vec::new();
    }

    issues
}

fn validate_internal_links(
    resolved: &ResolvedDocsConfig,
    source_path: &Path,
    content: &str,
) -> Vec<DocsCheckIssue> {
    let source_relative = resolved.relative_path(source_path);
    let mut issues = Vec::new();
    for target in extract_markdown_links(content) {
        let Some(target_path) = link_target_path(&target) else {
            continue;
        };
        let candidate_relative_to_source = if target_path.starts_with('/') {
            resolved.root.join(target_path.trim_start_matches('/'))
        } else {
            source_path
                .parent()
                .unwrap_or(&resolved.root)
                .join(&target_path)
        };
        if !candidate_relative_to_source.exists() {
            issues.push(DocsCheckIssue {
                code: "DOCS-CHECK-008".to_string(),
                path: source_relative.clone(),
                message: format!("broken internal link `{target}`"),
            });
        }
    }
    issues
}

fn render_lane_readme(kind: DocKind) -> String {
    match kind {
        DocKind::Adr => "# ADRs\n\nStore durable architecture and governance decisions here. Create new records with `elegy docs new adr --title \"...\"`.\n".to_string(),
        DocKind::Spec => "# Specs\n\nStore implementation-facing behavior specs here. Create new records with `elegy docs new spec --title \"...\"`.\n".to_string(),
    }
}

fn render_adr_template(title: &str, status: &str, owner: &str, date: &str) -> String {
    format!(
        "---\ntitle: {title}\nstatus: {status}\ndate: {date}\nowner: {owner}\n---\n\n# {title}\n\n## Context\n\n- Describe the decision pressure and current constraints.\n\n## Decision\n\n- State the durable decision.\n\n## Alternatives\n\n- Option A:\n- Option B:\n\n## Consequences\n\n- Positive:\n- Negative:\n\n## Links\n\n- None yet.\n"
    )
}

fn render_spec_template(title: &str, status: &str, owner: &str) -> String {
    format!(
        "---\ntitle: {title}\nstatus: {status}\nowner: {owner}\n---\n\n# {title}\n\n## Problem\n\n- Describe the implementation problem to solve.\n\n## Goals\n\n- Goal 1\n\n## Non-Goals\n\n- Non-goal 1\n\n## Behavior\n\n- Describe the intended behavior.\n\n## Acceptance Criteria\n\n- [ ] Criterion 1\n\n## Validation\n\n- Command or proof to run.\n\n## Links\n\n- None yet.\n"
    )
}

fn render_index_content(resolved: &ResolvedDocsConfig, documents: &[CollectedDocument]) -> String {
    let index_path = resolved.index_path();
    let mut content = String::new();
    content.push_str("# Documentation Index\n\n");
    content.push_str("Generated by `elegy docs index`. Update source files, then regenerate this index when the set of ADRs or specs changes.\n\n");
    content.push_str("## Configuration\n\n");
    content.push_str(&format!("- ADR path: `{}`\n", resolved.config.adr_path));
    content.push_str(&format!("- Spec path: `{}`\n", resolved.config.spec_path));
    content.push_str(&format!("- Index path: `{}`\n", resolved.config.index_path));
    if !resolved.config.required_doc_triggers.is_empty() {
        content.push_str("- Required doc triggers: ");
        content.push_str(&resolved.config.required_doc_triggers.join(", "));
        content.push('\n');
    }
    content.push_str("\n## ADRs\n\n");
    let adrs = documents
        .iter()
        .filter(|doc| doc.kind == DocKind::Adr)
        .collect::<Vec<_>>();
    if adrs.is_empty() {
        content.push_str("- None yet.\n");
    } else {
        for document in adrs {
            let status = document.summary.status.clone().unwrap_or_default();
            let owner = document.summary.owner.clone().unwrap_or_default();
            let date = document.summary.date.clone().unwrap_or_default();
            let link = relative_link_path(&index_path, &document.path, &resolved.root);
            content.push_str(&format!(
                "- [{}]({}) - status: `{}`; date: `{}`; owner: `{}`\n",
                document.summary.title, link, status, date, owner
            ));
        }
    }
    content.push_str("\n## Specs\n\n");
    let specs = documents
        .iter()
        .filter(|doc| doc.kind == DocKind::Spec)
        .collect::<Vec<_>>();
    if specs.is_empty() {
        content.push_str("- None yet.\n");
    } else {
        for document in specs {
            let status = document.summary.status.clone().unwrap_or_default();
            let owner = document.summary.owner.clone().unwrap_or_default();
            let link = relative_link_path(&index_path, &document.path, &resolved.root);
            content.push_str(&format!(
                "- [{}]({}) - status: `{}`; owner: `{}`\n",
                document.summary.title, link, status, owner
            ));
        }
    }
    content
}

fn parse_document(content: &str) -> Result<ParsedDocument, ToolingError> {
    try_parse_document(content).map_err(|message| ToolingError::InvalidDocsRequest {
        issues: vec![message],
    })
}

fn try_parse_document(content: &str) -> Result<ParsedDocument, String> {
    let normalized = content.replace("\r\n", "\n");
    let title_fallback = extract_title_fallback(&normalized);
    let Some(stripped) = normalized.strip_prefix("---\n") else {
        return Ok(ParsedDocument {
            frontmatter: None,
            body: normalized,
            title_fallback,
        });
    };
    let Some(end_index) = stripped.find("\n---\n") else {
        return Err("invalid document frontmatter: missing closing `---` delimiter".to_string());
    };
    let frontmatter_str = &stripped[..end_index];
    let body = stripped[end_index + "\n---\n".len()..].to_string();
    let frontmatter = serde_yaml::from_str::<DocFrontmatter>(frontmatter_str)
        .map_err(|source| format!("invalid document frontmatter: {source}"))?;
    Ok(ParsedDocument {
        frontmatter: Some(frontmatter),
        body,
        title_fallback,
    })
}

fn fallback_parsed_document(content: &str) -> ParsedDocument {
    let normalized = content.replace("\r\n", "\n");
    let title_fallback = extract_title_fallback(&normalized);
    ParsedDocument {
        frontmatter: None,
        body: normalized,
        title_fallback,
    }
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

fn collect_second_level_headings(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| line.trim().strip_prefix("## "))
        .map(|heading| heading.trim().to_string())
        .collect()
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

fn matches_file_name(kind: DocKind, file_name: &str) -> bool {
    match kind {
        DocKind::Adr => {
            let Some(stem) = file_name.strip_suffix(".md") else {
                return false;
            };
            if stem.len() < 12 {
                return false;
            }
            let date = &stem[..10];
            let Some(slug) = stem.strip_prefix(&(date.to_string() + "-")) else {
                return false;
            };
            is_valid_iso_date(date) && is_valid_slug(slug)
        }
        DocKind::Spec => file_name.strip_suffix(".md").is_some_and(is_valid_slug),
    }
}

fn validate_status(kind: DocKind, status: &str, path: Option<&Path>) -> Result<(), ToolingError> {
    let trimmed = status.trim();
    if trimmed.is_empty() {
        return Err(ToolingError::InvalidDocsRequest {
            issues: vec!["frontmatter must include a non-empty status".to_string()],
        });
    }
    if kind.allowed_statuses().contains(&trimmed) {
        return Ok(());
    }
    let mut message = format!(
        "status `{trimmed}` is invalid for {} documents; expected one of {}",
        kind.as_str(),
        kind.allowed_statuses().join(", ")
    );
    if let Some(path) = path {
        message.push_str(&format!(" ({})", display_path(path)));
    }
    Err(ToolingError::InvalidDocsRequest {
        issues: vec![message],
    })
}

fn trimmed_required(field: &str, value: String) -> Result<String, ToolingError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ToolingError::InvalidDocsRequest {
            issues: vec![format!("{field} must not be empty")],
        });
    }
    Ok(trimmed.to_string())
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn validate_slug(slug: String) -> Result<String, ToolingError> {
    if is_valid_slug(&slug) {
        return Ok(slug);
    }
    Err(ToolingError::InvalidDocsRequest {
        issues: vec![
            "slug must use lowercase ASCII letters, digits, and single hyphens".to_string(),
        ],
    })
}

fn is_valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && !slug.starts_with('-')
        && !slug.ends_with('-')
        && !slug.contains("--")
        && slug
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

fn is_valid_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !(bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..10].iter().all(u8::is_ascii_digit))
    {
        return false;
    }
    let month = value[5..7].parse::<u8>().ok();
    let day = value[8..10].parse::<u8>().ok();
    matches!(month, Some(1..=12)) && matches!(day, Some(1..=31))
}

fn current_utc_date() -> Result<String, ToolingError> {
    let format = format_description::parse("[year]-[month]-[day]").map_err(|error| {
        ToolingError::InvalidDocsRequest {
            issues: vec![format!("failed to prepare date formatter: {error}")],
        }
    })?;
    OffsetDateTime::now_utc()
        .format(&format)
        .map_err(|error| ToolingError::InvalidDocsRequest {
            issues: vec![format!("failed to render current UTC date: {error}")],
        })
}

fn normalize_rel_string(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_matches('/')
        .to_ascii_lowercase()
}

fn build_document_summary(
    kind: DocKind,
    relative_path: &str,
    parsed: &ParsedDocument,
) -> DocsDocumentSummary {
    DocsDocumentSummary {
        doc_type: kind.as_str().to_string(),
        path: relative_path.to_string(),
        title: parsed
            .frontmatter
            .as_ref()
            .and_then(|frontmatter| frontmatter.title.as_ref())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| parsed.title_fallback.clone()),
        status: parsed
            .frontmatter
            .as_ref()
            .and_then(|frontmatter| frontmatter.status.as_ref())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        owner: parsed
            .frontmatter
            .as_ref()
            .and_then(|frontmatter| frontmatter.owner.as_ref())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        date: parsed
            .frontmatter
            .as_ref()
            .and_then(|frontmatter| frontmatter.date.as_ref())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    }
}

fn relative_link_path(from_file: &Path, to_path: &Path, repo_root: &Path) -> String {
    let from_dir = from_file.parent().unwrap_or(repo_root);
    let from_parts = repo_relative_parts(repo_root, from_dir);
    let to_parts = repo_relative_parts(repo_root, to_path);
    let mut common_prefix = 0usize;
    while common_prefix < from_parts.len()
        && common_prefix < to_parts.len()
        && from_parts[common_prefix] == to_parts[common_prefix]
    {
        common_prefix += 1;
    }

    let mut parts = Vec::new();
    for _ in common_prefix..from_parts.len() {
        parts.push("..".to_string());
    }
    parts.extend(to_parts[common_prefix..].iter().cloned());
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

fn repo_relative_parts(repo_root: &Path, path: &Path) -> Vec<String> {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect()
}

fn relative_display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn default_docs_schema_version() -> String {
    DOCS_CONFIG_SCHEMA_VERSION.to_string()
}

fn default_adr_path() -> String {
    DEFAULT_ADR_PATH.to_string()
}

fn default_spec_path() -> String {
    DEFAULT_SPEC_PATH.to_string()
}

fn default_index_path() -> String {
    DEFAULT_INDEX_PATH.to_string()
}

fn default_required_doc_triggers() -> Vec<String> {
    KNOWN_REQUIRED_DOC_TRIGGERS
        .iter()
        .map(|value| (*value).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        docs_check, docs_index, docs_init, docs_new_adr, docs_new_spec, DocsConfig, NewDocRequest,
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
    fn docs_check_allows_empty_repo_without_config() {
        let root = unique_temp_dir("elegy-tooling-docs-empty");
        let report = docs_check(&root).expect("check docs");
        assert!(report.valid);
        assert!(!report.config_found);
        assert_eq!(report.files_checked, 0);
        assert!(report.documents.is_empty());
    }

    #[test]
    fn docs_init_writes_default_seed_files() {
        let root = unique_temp_dir("elegy-tooling-docs-init");
        let result = docs_init(&root).expect("init docs");
        assert!(result.created.iter().any(|path| path == ".elegy/docs.yaml"));
        assert!(root.join("docs/adr/README.md").is_file());
        assert!(root.join("docs/specs/README.md").is_file());
        assert!(root.join("docs/docs-index.md").is_file());
    }

    #[test]
    fn docs_new_creates_expected_adr_and_spec_files() {
        let root = unique_temp_dir("elegy-tooling-docs-new");
        let adr = docs_new_adr(
            &root,
            NewDocRequest {
                title: "Adopt documentation practice checks".to_string(),
                owner: Some("Elegy".to_string()),
                slug: None,
                status: None,
            },
        )
        .expect("new adr");
        let spec = docs_new_spec(
            &root,
            NewDocRequest {
                title: "Docs CLI behavior".to_string(),
                owner: Some("Elegy".to_string()),
                slug: Some("docs-cli-behavior".to_string()),
                status: Some("active".to_string()),
            },
        )
        .expect("new spec");

        assert!(adr
            .output_path
            .ends_with("adopt-documentation-practice-checks.md"));
        assert_eq!(spec.output_path, "docs/specs/docs-cli-behavior.md");
        assert!(root.join(&spec.output_path).is_file());
    }

    #[test]
    fn docs_check_reports_invalid_metadata_and_broken_links() {
        let root = unique_temp_dir("elegy-tooling-docs-invalid");
        docs_init(&root).expect("init docs");
        fs::write(
            root.join("docs/adr/2026-05-25-invalid-adr.md"),
            "---\ntitle: Invalid ADR\nstatus: draft\ndate: 2026-05-25\nowner: Elegy\n---\n\n# Invalid ADR\n\n## Context\n\n- Context.\n\n## Decision\n\n- Decision.\n\n## Links\n\n- [Broken](missing-spec.md)\n",
        )
        .expect("write invalid adr");

        let report = docs_check(&root).expect("check docs");
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "DOCS-CHECK-005"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "DOCS-CHECK-007"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "DOCS-CHECK-008"));
    }

    #[test]
    fn docs_check_respects_local_config_overrides() {
        let root = unique_temp_dir("elegy-tooling-docs-overrides");
        fs::create_dir_all(root.join(".elegy")).expect("create config dir");
        let config = DocsConfig {
            adr_path: "records/decisions".to_string(),
            spec_path: "records/specs".to_string(),
            index_path: "records/index.md".to_string(),
            local_exceptions: vec!["records/specs/legacy".to_string()],
            ..DocsConfig::default()
        };
        fs::write(
            root.join(".elegy/docs.yaml"),
            serde_yaml::to_string(&config).expect("serialize config"),
        )
        .expect("write config");
        fs::create_dir_all(root.join("records/decisions")).expect("create adr dir");
        fs::write(
            root.join("records/decisions/2026-05-25-custom-path.md"),
            "---\ntitle: Custom path ADR\nstatus: proposed\ndate: 2026-05-25\nowner: Elegy\n---\n\n# Custom path ADR\n\n## Context\n\n- Context.\n\n## Decision\n\n- Decision.\n\n## Alternatives\n\n- Alternative.\n\n## Consequences\n\n- Consequence.\n\n## Links\n\n- None yet.\n",
        )
        .expect("write adr");

        let report = docs_check(&root).expect("check docs");
        assert!(report.valid);
        assert_eq!(
            report.documents[0].path,
            "records/decisions/2026-05-25-custom-path.md"
        );
    }

    #[test]
    fn docs_index_writes_document_index() {
        let root = unique_temp_dir("elegy-tooling-docs-index");
        docs_new_adr(
            &root,
            NewDocRequest {
                title: "Keep docs centralized".to_string(),
                owner: Some("Elegy".to_string()),
                slug: Some("keep-docs-centralized".to_string()),
                status: Some("accepted".to_string()),
            },
        )
        .expect("new adr");
        docs_new_spec(
            &root,
            NewDocRequest {
                title: "Documentation CLI behaviors".to_string(),
                owner: Some("Elegy".to_string()),
                slug: Some("documentation-cli-behaviors".to_string()),
                status: None,
            },
        )
        .expect("new spec");

        let result = docs_index(&root).expect("index docs");
        let content = fs::read_to_string(root.join(&result.output_path)).expect("read index");
        assert!(content.contains("Keep docs centralized"));
        assert!(content.contains("Documentation CLI behaviors"));
        assert!(content.contains("](adr/"));
        assert!(content.contains("](specs/documentation-cli-behaviors.md)"));
        assert_eq!(result.adr_count, 1);
        assert_eq!(result.spec_count, 1);
    }

    #[test]
    fn docs_check_reports_malformed_frontmatter_as_structured_issue() {
        let root = unique_temp_dir("elegy-tooling-docs-malformed-frontmatter");
        docs_init(&root).expect("init docs");
        fs::write(
            root.join("docs/adr/2026-05-25-malformed-frontmatter.md"),
            "---\ntitle: Malformed ADR\nstatus: accepted\nowner: [Elegy\ndate: 2026-05-25\n---\n\n# Malformed ADR\n\n## Context\n\n- Context.\n",
        )
        .expect("write malformed adr");

        let report = docs_check(&root).expect("check docs");
        assert!(!report.valid);
        assert_eq!(report.docs_checked, 1);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "DOCS-CHECK-009"));
    }
}
