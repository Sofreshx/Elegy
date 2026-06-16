use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::ToolingError;

pub const GENERATOR_META_SCHEMA_VERSION: &str = "elegy-generator.contract-meta/v0";
pub const GENERATOR_MANIFEST_SCHEMA_VERSION: &str = "elegy-generator.manifest/v0";
pub const GENERATOR_CHECK_SCHEMA_VERSION: &str = "elegy-generator.check/v0";
pub const GENERATOR_REGISTRY_SCHEMA_VERSION: &str = "elegy-generator.registry/v0";
pub const GENERATOR_RECEIPT_SCHEMA_VERSION: &str = "elegy-generator.receipt/v0";

const RUNTIME_NAME: &str = "elegy-tooling";
const RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorMeta {
    pub schema_version: String,
    pub id: String,
    pub kind: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<Value>,
    #[serde(default)]
    pub extensions: Map<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorManifest {
    #[serde(flatten)]
    pub meta: GeneratorMeta,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preconditions: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emits: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<BackendRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorCheck {
    #[serde(flatten)]
    pub meta: GeneratorMeta,
    pub check_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<CheckTarget>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckTarget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendRef {
    pub kind: String,
    #[serde(default)]
    pub config: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorRegistry {
    #[serde(flatten)]
    pub meta: GeneratorMeta,
    pub entries: Vec<RegistryEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryEntry {
    pub id: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorReceipt {
    pub schema_version: String,
    pub id: String,
    pub kind: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_ref: Option<ManifestRef>,
    pub operation: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: String,
    pub runtime: RuntimeRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs_hash: Option<String>,
    pub outputs: Vec<Value>,
    pub checks: Vec<Value>,
    pub warnings: Vec<GeneratorFinding>,
    pub errors: Vec<GeneratorFinding>,
    #[serde(default)]
    pub extensions: Map<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestRef {
    pub id: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeRef {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneratorFinding {
    pub code: String,
    pub message: String,
    #[serde(flatten)]
    pub details: BTreeMap<String, Value>,
}

impl GeneratorFinding {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: BTreeMap::new(),
        }
    }

    fn with_detail(mut self, key: impl Into<String>, value: Value) -> Self {
        self.details.insert(key.into(), value);
        self
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorContractSummary {
    pub path: String,
    pub schema_version: String,
    pub id: String,
    pub kind: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorValidationReport {
    pub file: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<GeneratorContractSummary>,
    pub schema: ValidationPhase,
    pub semantic: ValidationPhase,
    pub warnings: Vec<GeneratorFinding>,
    pub errors: Vec<GeneratorFinding>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationPhase {
    pub status: String,
    pub issues: Vec<GeneratorFinding>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorShowReport {
    pub contract: GeneratorContractSummary,
    pub value: Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorRegistryReport {
    pub root: String,
    pub entries: Vec<GeneratorContractSummary>,
    pub warnings: Vec<GeneratorFinding>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorResolveReport {
    pub root: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<GeneratorContractSummary>,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorCheckRunReport {
    pub status: String,
    pub check: GeneratorContractSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<GeneratorValidationReport>,
    pub receipt: GeneratorReceipt,
    pub warnings: Vec<GeneratorFinding>,
    pub errors: Vec<GeneratorFinding>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorPlanReport {
    pub status: String,
    pub manifest: GeneratorContractSummary,
    pub inputs: Value,
    pub receipt: GeneratorReceipt,
    pub warnings: Vec<GeneratorFinding>,
    pub errors: Vec<GeneratorFinding>,
}

pub fn validate_generator_contract_file(
    path: &Path,
) -> Result<GeneratorValidationReport, ToolingError> {
    let value = load_json_value(path)?;
    let schema_version = schema_version_from_value(path, &value)?;
    let schema_phase = validate_value_schema(&value, &schema_version);
    let schema_ok = schema_phase.issues.is_empty();
    let contract = contract_summary_from_value(path, &value).ok();

    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let semantic_phase = if schema_ok {
        let semantic_issues = semantic_warnings(&value, &schema_version);
        warnings.extend(semantic_issues.clone());
        ValidationPhase {
            status: if semantic_issues.is_empty() {
                "success".to_string()
            } else {
                "warning".to_string()
            },
            issues: semantic_issues,
        }
    } else {
        errors.extend(schema_phase.issues.clone());
        ValidationPhase {
            status: "skipped".to_string(),
            issues: Vec::new(),
        }
    };

    let status = if !errors.is_empty() {
        "failed"
    } else if !warnings.is_empty() {
        "warning"
    } else {
        "success"
    };

    Ok(GeneratorValidationReport {
        file: display_path(path),
        status: status.to_string(),
        contract,
        schema: schema_phase,
        semantic: semantic_phase,
        warnings,
        errors,
    })
}

pub fn show_generator_contract_file(path: &Path) -> Result<GeneratorShowReport, ToolingError> {
    let value = load_json_value(path)?;
    Ok(GeneratorShowReport {
        contract: contract_summary_from_value(path, &value)?,
        value,
    })
}

pub fn list_generator_registry(path: &Path) -> Result<GeneratorRegistryReport, ToolingError> {
    let mut entries = Vec::new();
    let mut warnings = Vec::new();

    for file in generator_json_files(path)? {
        match validate_generator_contract_file(&file) {
            Ok(report) if report.schema.status == "success" => {
                if let Some(contract) = report.contract {
                    entries.push(contract);
                }
            }
            Ok(report) => warnings.push(
                GeneratorFinding::new(
                    "REGISTRY_ENTRY_INVALID",
                    "Generator registry skipped a schema-invalid contract.",
                )
                .with_detail("path", json!(report.file))
                .with_detail("status", json!(report.status)),
            ),
            Err(error) => warnings.push(
                GeneratorFinding::new(
                    "REGISTRY_ENTRY_UNREADABLE",
                    "Generator registry skipped an unreadable contract.",
                )
                .with_detail("path", json!(display_path(&file)))
                .with_detail("error", json!(error.to_string())),
            ),
        }
    }

    entries.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(GeneratorRegistryReport {
        root: display_path(path),
        entries,
        warnings,
    })
}

pub fn resolve_generator_registry_entry(
    id: &str,
    path: &Path,
) -> Result<GeneratorResolveReport, ToolingError> {
    let registry = list_generator_registry(path)?;
    let contract = registry
        .entries
        .iter()
        .find(|entry| entry.id == id)
        .cloned();
    let status = if contract.is_some() {
        "found"
    } else {
        "missing"
    };

    Ok(GeneratorResolveReport {
        root: display_path(path),
        id: id.to_string(),
        contract,
        status: status.to_string(),
    })
}

pub fn run_generator_check_file(
    check_path: &Path,
    context_path: &Path,
) -> Result<GeneratorCheckRunReport, ToolingError> {
    let value = load_json_value(check_path)?;
    let check: GeneratorCheck = deserialize_after_schema(check_path, value)?;
    let check_summary = contract_summary_from_meta(check_path, &check.meta);
    let started_at = now_rfc3339();

    if check.check_kind != "schema" {
        let warning = GeneratorFinding::new(
            "UNSUPPORTED_CHECK_KIND",
            format!(
                "Check kind '{}' is schema-valid but unsupported in v0.1.",
                check.check_kind
            ),
        )
        .with_detail("checkKind", json!(check.check_kind));
        let receipt = build_receipt(
            "check",
            "unsupported",
            None,
            started_at,
            vec![warning.clone()],
            Vec::new(),
        );
        return Ok(GeneratorCheckRunReport {
            status: "unsupported".to_string(),
            check: check_summary,
            target: None,
            receipt,
            warnings: vec![warning],
            errors: Vec::new(),
        });
    }

    let target_path = resolve_check_target_path(&check, check_path, context_path);
    let target = validate_generator_contract_file(&target_path)?;
    let mut errors = if target.status == "failed" {
        target.errors.clone()
    } else {
        Vec::new()
    };
    let warnings = target.warnings.clone();

    if let Some(expected_schema_version) = check
        .target
        .as_ref()
        .and_then(|target| target.schema_version.as_deref())
        .filter(|schema_version| !schema_version.trim().is_empty())
    {
        let actual_schema_version = target
            .contract
            .as_ref()
            .map(|contract| contract.schema_version.as_str());
        if actual_schema_version != Some(expected_schema_version) {
            let actual = actual_schema_version.unwrap_or("<unknown>");
            errors.push(
                GeneratorFinding::new(
                    "TARGET_SCHEMA_VERSION_MISMATCH",
                    format!(
                        "Check target expected schemaVersion '{expected_schema_version}' but found '{actual}'."
                    ),
                )
                .with_detail("expectedSchemaVersion", json!(expected_schema_version))
                .with_detail("actualSchemaVersion", json!(actual)),
            );
        }
    }

    let has_errors = !errors.is_empty();
    let receipt = build_receipt(
        "check",
        if has_errors { "failed" } else { "success" },
        None,
        started_at,
        warnings.clone(),
        errors.clone(),
    );

    Ok(GeneratorCheckRunReport {
        status: if has_errors { "failed" } else { "success" }.to_string(),
        check: check_summary,
        target: Some(target),
        receipt,
        warnings,
        errors,
    })
}

pub fn plan_generator_manifest_file(
    manifest_path: &Path,
    inputs: Value,
) -> Result<GeneratorPlanReport, ToolingError> {
    let value = load_json_value(manifest_path)?;
    let manifest: GeneratorManifest = deserialize_after_schema(manifest_path, value)?;
    let summary = contract_summary_from_meta(manifest_path, &manifest.meta);
    let started_at = now_rfc3339();

    let warning = match manifest.backend {
        Some(backend) => GeneratorFinding::new(
            "UNSUPPORTED_BACKEND",
            format!(
                "Backend kind '{}' is schema-valid but unsupported in v0.1.",
                backend.kind
            ),
        )
        .with_detail("backendKind", json!(backend.kind)),
        None => GeneratorFinding::new(
            "UNSUPPORTED_BACKEND",
            "No backend implementation is available in v0.1.",
        ),
    };

    let receipt = build_receipt(
        "plan",
        "unsupported",
        Some(ManifestRef {
            id: manifest.meta.id,
            version: manifest.meta.version,
        }),
        started_at,
        vec![warning.clone()],
        Vec::new(),
    );

    Ok(GeneratorPlanReport {
        status: "unsupported".to_string(),
        manifest: summary,
        inputs,
        receipt,
        warnings: vec![warning],
        errors: Vec::new(),
    })
}

fn deserialize_after_schema<T>(path: &Path, value: Value) -> Result<T, ToolingError>
where
    T: for<'de> Deserialize<'de>,
{
    let schema_version = schema_version_from_value(path, &value)?;
    let schema_phase = validate_value_schema(&value, &schema_version);
    if !schema_phase.issues.is_empty() {
        return Err(ToolingError::InvalidGeneratorContract {
            path: path.to_path_buf(),
            issues: schema_phase
                .issues
                .into_iter()
                .map(|issue| issue.message)
                .collect(),
        });
    }
    serde_json::from_value(value).map_err(|source| ToolingError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn validate_value_schema(value: &Value, schema_version: &str) -> ValidationPhase {
    let schema_value = match schema_value_for_version(schema_version) {
        Some(schema) => schema,
        None => {
            return ValidationPhase {
                status: "failed".to_string(),
                issues: vec![GeneratorFinding::new(
                    "UNKNOWN_SCHEMA_VERSION",
                    format!("No generator schema is registered for '{schema_version}'."),
                )],
            };
        }
    };

    let compiled = match JSONSchema::compile(&schema_value) {
        Ok(compiled) => compiled,
        Err(error) => {
            return ValidationPhase {
                status: "failed".to_string(),
                issues: vec![GeneratorFinding::new(
                    "SCHEMA_COMPILE_ERROR",
                    format!("Generator schema failed to compile: {error}"),
                )],
            };
        }
    };

    let issues = match compiled.validate(value) {
        Ok(()) => Vec::new(),
        Err(errors) => errors
            .map(|error| {
                GeneratorFinding::new("SCHEMA_VALIDATION_ERROR", error.to_string())
                    .with_detail("instancePath", json!(error.instance_path.to_string()))
            })
            .collect(),
    };

    ValidationPhase {
        status: if issues.is_empty() {
            "success".to_string()
        } else {
            "failed".to_string()
        },
        issues,
    }
}

fn semantic_warnings(value: &Value, schema_version: &str) -> Vec<GeneratorFinding> {
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut warnings = Vec::new();

    if schema_version == GENERATOR_MANIFEST_SCHEMA_VERSION {
        if kind != "solved_unit" {
            warnings.push(
                GeneratorFinding::new(
                    "UNKNOWN_KIND",
                    format!(
                        "Contract is schema-valid but this runtime has no handler for kind '{kind}'."
                    ),
                )
                .with_detail("kind", json!(kind)),
            );
        }
        if let Some(backend_kind) = value
            .get("backend")
            .and_then(|backend| backend.get("kind"))
            .and_then(Value::as_str)
        {
            warnings.push(
                GeneratorFinding::new(
                    "UNSUPPORTED_BACKEND",
                    format!(
                        "Backend kind '{backend_kind}' is schema-valid but unsupported in v0.1."
                    ),
                )
                .with_detail("backendKind", json!(backend_kind)),
            );
        }
    }

    if schema_version == GENERATOR_CHECK_SCHEMA_VERSION {
        if let Some(check_kind) = value.get("checkKind").and_then(Value::as_str) {
            if check_kind != "schema" {
                warnings.push(
                    GeneratorFinding::new(
                        "UNSUPPORTED_CHECK_KIND",
                        format!(
                            "Check kind '{check_kind}' is schema-valid but unsupported in v0.1."
                        ),
                    )
                    .with_detail("checkKind", json!(check_kind)),
                );
            }
        }
    }

    warnings
}

fn schema_value_for_version(schema_version: &str) -> Option<Value> {
    let raw = match schema_version {
        GENERATOR_META_SCHEMA_VERSION => {
            include_str!("../../../../../contracts/schemas/elegy-generator.contract-meta.v0.json")
        }
        GENERATOR_MANIFEST_SCHEMA_VERSION => {
            include_str!("../../../../../contracts/schemas/elegy-generator.manifest.v0.json")
        }
        GENERATOR_CHECK_SCHEMA_VERSION => {
            include_str!("../../../../../contracts/schemas/elegy-generator.check.v0.json")
        }
        GENERATOR_REGISTRY_SCHEMA_VERSION => {
            include_str!("../../../../../contracts/schemas/elegy-generator.registry.v0.json")
        }
        GENERATOR_RECEIPT_SCHEMA_VERSION => {
            include_str!("../../../../../contracts/schemas/elegy-generator.receipt.v0.json")
        }
        _ => return None,
    };
    serde_json::from_str(raw).ok()
}

fn schema_version_from_value(path: &Path, value: &Value) -> Result<String, ToolingError> {
    match value.get("schemaVersion").and_then(Value::as_str) {
        Some(schema_version) if !schema_version.trim().is_empty() => Ok(schema_version.to_string()),
        _ => Err(ToolingError::InvalidGeneratorContract {
            path: path.to_path_buf(),
            issues: vec!["schemaVersion must be a non-empty string".to_string()],
        }),
    }
}

fn contract_summary_from_value(
    path: &Path,
    value: &Value,
) -> Result<GeneratorContractSummary, ToolingError> {
    Ok(GeneratorContractSummary {
        path: display_path(path),
        schema_version: required_string(value, "schemaVersion")?,
        id: required_string(value, "id")?,
        kind: required_string(value, "kind")?,
        version: required_string(value, "version")?,
        status: value
            .get("status")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn contract_summary_from_meta(path: &Path, meta: &GeneratorMeta) -> GeneratorContractSummary {
    GeneratorContractSummary {
        path: display_path(path),
        schema_version: meta.schema_version.clone(),
        id: meta.id.clone(),
        kind: meta.kind.clone(),
        version: meta.version.clone(),
        status: meta.status.clone(),
    }
}

fn required_string(value: &Value, field: &str) -> Result<String, ToolingError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ToolingError::InvalidGeneratorContract {
            path: PathBuf::from("<memory>"),
            issues: vec![format!("{field} must be a non-empty string")],
        })
}

fn load_json_value(path: &Path) -> Result<Value, ToolingError> {
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

fn generator_json_files(path: &Path) -> Result<Vec<PathBuf>, ToolingError> {
    let mut files = Vec::new();
    if path.is_file() {
        files.push(path.to_path_buf());
        return Ok(files);
    }

    let entries = fs::read_dir(path).map_err(|source| ToolingError::Io {
        operation: "read_dir",
        path: path.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| ToolingError::Io {
            operation: "read_dir_entry",
            path: path.to_path_buf(),
            source,
        })?;
        let file_path = entry.path();
        if file_path.is_file()
            && file_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("elegy-generator.") && name.ends_with(".json"))
                .unwrap_or(false)
            && !file_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.contains(".invalid."))
                .unwrap_or(false)
        {
            files.push(file_path);
        }
    }

    Ok(files)
}

fn resolve_check_target_path(
    check: &GeneratorCheck,
    check_path: &Path,
    context_path: &Path,
) -> PathBuf {
    if let Some(path) = check
        .target
        .as_ref()
        .and_then(|target| target.path.as_ref())
        .filter(|path| !path.trim().is_empty())
    {
        let target_path = PathBuf::from(path);
        if target_path.is_absolute() {
            return target_path;
        }
        if context_path.is_dir() {
            return context_path.join(target_path);
        }
        if let Some(parent) = check_path.parent() {
            return parent.join(target_path);
        }
    }
    context_path.to_path_buf()
}

fn build_receipt(
    operation: &str,
    status: &str,
    manifest_ref: Option<ManifestRef>,
    started_at: String,
    warnings: Vec<GeneratorFinding>,
    errors: Vec<GeneratorFinding>,
) -> GeneratorReceipt {
    let finished_at = now_rfc3339();
    GeneratorReceipt {
        schema_version: GENERATOR_RECEIPT_SCHEMA_VERSION.to_string(),
        id: format!(
            "elegy.generator.receipt.{}.{}",
            operation,
            finished_at.replace([':', '-', '.'], "")
        ),
        kind: "receipt".to_string(),
        version: "0.1.0".to_string(),
        manifest_ref,
        operation: operation.to_string(),
        status: status.to_string(),
        started_at,
        finished_at,
        runtime: RuntimeRef {
            name: RUNTIME_NAME.to_string(),
            version: RUNTIME_VERSION.to_string(),
        },
        inputs_hash: None,
        outputs: Vec::new(),
        checks: Vec::new(),
        warnings,
        errors,
        extensions: Map::new(),
    }
}

fn now_rfc3339() -> String {
    match OffsetDateTime::now_utc().format(&Rfc3339) {
        Ok(value) => value,
        Err(_) => "1970-01-01T00:00:00Z".to_string(),
    }
}

fn display_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .expect("repo root")
    }

    #[test]
    fn validates_minimal_manifest() {
        let path = repo_root()
            .join("contracts")
            .join("fixtures")
            .join("elegy-generator.manifest.minimal.json");
        let report = validate_generator_contract_file(&path).expect("validate manifest");
        assert_eq!(report.status, "success");
        assert_eq!(report.schema.status, "success");
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        let path = repo_root()
            .join("contracts")
            .join("fixtures")
            .join("elegy-generator.contract-meta.unknown-top-level.invalid.json");
        let report = validate_generator_contract_file(&path).expect("validate invalid meta");
        assert_eq!(report.status, "failed");
        assert_eq!(report.schema.status, "failed");
    }

    #[test]
    fn warns_for_future_manifest_kind() {
        let path = repo_root()
            .join("contracts")
            .join("fixtures")
            .join("elegy-generator.manifest.future-kind.json");
        let report = validate_generator_contract_file(&path).expect("validate future manifest");
        assert_eq!(report.status, "warning");
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.code == "UNKNOWN_KIND"));
    }

    #[test]
    fn preserves_unknown_extension_values() {
        let path = repo_root()
            .join("contracts")
            .join("fixtures")
            .join("elegy-generator.contract-meta.unknown-extension.json");
        let value = load_json_value(&path).expect("load extension fixture");
        let meta: GeneratorMeta =
            deserialize_after_schema(&path, value).expect("deserialize extension fixture");
        assert!(meta.extensions.contains_key("elegy.experimental"));
    }

    #[test]
    fn unsupported_backend_plan_returns_receipt_without_outputs() {
        let path = repo_root()
            .join("contracts")
            .join("fixtures")
            .join("elegy-generator.manifest.unsupported-backend.json");
        let report = plan_generator_manifest_file(&path, json!({})).expect("plan manifest");
        assert_eq!(report.status, "unsupported");
        assert!(report.receipt.outputs.is_empty());
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.code == "UNSUPPORTED_BACKEND"));
    }

    #[test]
    fn unsupported_check_kind_returns_unsupported() {
        let root = repo_root().join("contracts").join("fixtures");
        let check = root.join("elegy-generator.check.unsupported-kind.json");
        let report = run_generator_check_file(&check, &root).expect("run check");
        assert_eq!(report.status, "unsupported");
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.code == "UNSUPPORTED_CHECK_KIND"));
    }
}
