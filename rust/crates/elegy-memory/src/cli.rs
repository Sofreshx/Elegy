use crate::{
    GovernedMemoryRecord, GovernedMemoryRecordImportOptions, LocalMemoryCatalogEntry,
    LocalMemoryExportResult, LocalMemoryLifecycleState, LocalMemoryPaths, LocalMemoryQueryOptions,
    LocalMemoryStore, LocalMemoryStoreError, LocalMemoryStoredRecord, SessionContextScope,
    SummaryOnlySessionContextEnvelope, LOCAL_MEMORY_AUTHORITY_POSTURE,
    LOCAL_MEMORY_DETERMINISTIC_ORDERING, LOCAL_MEMORY_SINGLE_WRITER_POSTURE,
    SUMMARY_ONLY_REPRESENTATION, SUMMARY_ONLY_SESSION_CONTEXT_ARTIFACT_KIND,
};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::json;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const MEMORY_CLI_SCHEMA_VERSION: &str = "0.1.0";
const SESSION_CONTEXT_PREVIEW_LIMIT: usize = 160;
const LOCAL_DEFAULT_ROOT_DIR: &str = ".elegy-local-memory";
const LOCAL_DEFAULT_VISIBILITY_POSTURE: &str =
    "default active-only queries exclude superseded and tombstoned local records";
const SESSION_CONTEXT_NEUTRAL_VALIDATION_SCOPE: &str =
    "artifact-shape validation over the governed summary-only envelope only";
const SESSION_CONTEXT_AUTHORITY_POSTURE: &str =
    "non-authoritative CLI surface; host-owned authority remains in SAASTools";
const SESSION_CONTEXT_ADAPTER_POSTURE: &str =
    "mirror-or-inspect-only; adapters cannot promote, invalidate, or override host-owned truth";
const SESSION_CONTEXT_HOST_OWNER: &str = "SAASTools";

#[derive(Parser, Debug)]
#[command(name = "elegy-memory")]
#[command(
    about = "CLI for governed session-context inspection and local non-authoritative memory artifact management"
)]
struct Cli {
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    format: OutputFormat,
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Subcommand, Debug)]
enum Command {
    Inspect,
    Validate {
        #[arg(long)]
        input: PathBuf,
    },
    Init {
        #[command(flatten)]
        root: LocalRootArgs,
    },
    Import {
        #[command(flatten)]
        root: LocalRootArgs,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        record_id: String,
        #[arg(long)]
        imported_at_utc: String,
    },
    List {
        #[command(flatten)]
        root: LocalRootArgs,
        #[command(flatten)]
        visibility: LocalVisibilityArgs,
    },
    Show {
        #[command(flatten)]
        root: LocalRootArgs,
        #[arg(long)]
        record_id: String,
        #[command(flatten)]
        visibility: LocalVisibilityArgs,
    },
    Export {
        #[command(flatten)]
        root: LocalRootArgs,
        #[arg(long)]
        record_id: String,
        #[arg(long)]
        output_path: Option<PathBuf>,
        #[command(flatten)]
        visibility: LocalVisibilityArgs,
    },
    Supersede {
        #[command(flatten)]
        root: LocalRootArgs,
        #[arg(long)]
        record_id: String,
        #[arg(long)]
        superseded_by_record_id: String,
    },
    Tombstone {
        #[command(flatten)]
        root: LocalRootArgs,
        #[arg(long)]
        record_id: String,
        #[arg(long)]
        tombstoned_at_utc: String,
        #[arg(long)]
        reason: String,
    },
}

#[derive(Args, Clone, Debug)]
struct LocalRootArgs {
    #[arg(long)]
    root: Option<PathBuf>,
}

#[derive(Args, Clone, Debug, Default)]
struct LocalVisibilityArgs {
    #[arg(long)]
    include_superseded: bool,
    #[arg(long)]
    include_tombstoned: bool,
}

impl LocalVisibilityArgs {
    fn query_options(&self) -> LocalMemoryQueryOptions {
        LocalMemoryQueryOptions {
            include_superseded: self.include_superseded,
            include_tombstoned: self.include_tombstoned,
        }
    }
}

#[derive(Serialize)]
struct Envelope<T>
where
    T: Serialize,
{
    schema_version: &'static str,
    command: Vec<String>,
    status: &'static str,
    summary: Summary,
    data: T,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Default, Serialize)]
struct Summary {
    errors: usize,
    warnings: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticLocation {
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Diagnostic {
    severity: DiagnosticSeverity,
    code: String,
    message: String,
    location: DiagnosticLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<String>,
}

impl Diagnostic {
    fn error(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code: code.to_string(),
            message: message.into(),
            location: DiagnosticLocation::default(),
            hint: None,
        }
    }

    fn with_path(mut self, path: String) -> Self {
        self.location.path = Some(path);
        self
    }

    fn with_field(mut self, field: impl Into<String>) -> Self {
        self.location.field = Some(field.into());
        self
    }

    fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionContextInspection {
    capability: &'static str,
    contract_field: &'static str,
    schema_file: &'static str,
    representation: &'static str,
    supported_scopes: Vec<&'static str>,
    intended_consumers: Vec<&'static str>,
    bounded_fields: Vec<&'static str>,
    raw_transcript_persisted: bool,
    transcript_bodies_allowed_in_artifact: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionContextValidationReport {
    input_path: String,
    artifact_kind: &'static str,
    representation: &'static str,
    scope: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    captured_at_utc: Option<String>,
    summary_length: usize,
    summary_preview: String,
    salient_facts_count: usize,
    instruction_context_count: usize,
    raw_transcript_persisted: bool,
    read_only: bool,
    neutral_validation_scope: &'static str,
    authority_posture: &'static str,
    host_validation_owner: &'static str,
    host_promotion_owner: &'static str,
    host_invalidation_owner: &'static str,
    adapter_posture: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalInitReport {
    root_path: String,
    artifacts_path: String,
    state_path: String,
    write_lock_path: String,
    exports_path: String,
    authority_posture: &'static str,
    single_writer_posture: &'static str,
    deterministic_ordering: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalListReport {
    root_path: String,
    authority_posture: &'static str,
    default_visibility: String,
    deterministic_ordering: &'static str,
    records: Vec<LocalMemoryCatalogEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalRecordReport {
    root_path: String,
    artifact_path: String,
    default_export_path: String,
    authority_posture: &'static str,
    default_visibility: String,
    deterministic_ordering: &'static str,
    record: GovernedMemoryRecord,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalExportReport {
    root_path: String,
    output_path: String,
    authority_posture: &'static str,
    default_visibility: String,
    deterministic_ordering: &'static str,
    record: GovernedMemoryRecord,
    exported_envelope: SummaryOnlySessionContextEnvelope,
}

pub fn run_from_env() -> Result<ExitCode, serde_json::Error> {
    run_from(std::env::args_os())
}

pub fn run_from<I, T>(args: I) -> Result<ExitCode, serde_json::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    dispatch(Cli::parse_from(args))
}

fn dispatch(cli: Cli) -> Result<ExitCode, serde_json::Error> {
    match cli.command {
        Command::Inspect => execute_inspect_command(cli.format),
        Command::Validate { input } => execute_validate_command(input, cli.format),
        Command::Init { root } => execute_init_command(root.root, cli.format),
        Command::Import {
            root,
            input,
            record_id,
            imported_at_utc,
        } => execute_import_command(root.root, input, record_id, imported_at_utc, cli.format),
        Command::List { root, visibility } => {
            execute_list_command(root.root, visibility.query_options(), cli.format)
        }
        Command::Show {
            root,
            record_id,
            visibility,
        } => execute_show_command(root.root, record_id, visibility.query_options(), cli.format),
        Command::Export {
            root,
            record_id,
            output_path,
            visibility,
        } => execute_export_command(
            root.root,
            record_id,
            output_path,
            visibility.query_options(),
            cli.format,
        ),
        Command::Supersede {
            root,
            record_id,
            superseded_by_record_id,
        } => execute_supersede_command(root.root, record_id, superseded_by_record_id, cli.format),
        Command::Tombstone {
            root,
            record_id,
            tombstoned_at_utc,
            reason,
        } => execute_tombstone_command(root.root, record_id, tombstoned_at_utc, reason, cli.format),
    }
}

pub fn execute_inspect_command(format: OutputFormat) -> Result<ExitCode, serde_json::Error> {
    let inspection = session_context_inspection();

    match format {
        OutputFormat::Text => print_session_context_text(&inspection),
        OutputFormat::Json => print_json(&Envelope {
            schema_version: MEMORY_CLI_SCHEMA_VERSION,
            command: vec!["inspect".to_string()],
            status: "ok",
            summary: Summary::default(),
            data: inspection,
            diagnostics: Vec::new(),
        })?,
    }

    Ok(ExitCode::SUCCESS)
}

pub fn execute_validate_command(
    input: PathBuf,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let input_path = input.display().to_string();
    let file_contents = match fs::read_to_string(&input) {
        Ok(value) => value,
        Err(error) => {
            let diagnostic = Diagnostic::error(
                "CLI-MEMORY-001",
                format!(
                    "failed to read session-context artifact {}: {error}",
                    input.display()
                ),
            )
            .with_path(input_path.clone())
            .with_hint("supply a readable JSON file containing a governed summary-only artifact");

            return emit_diagnostics(
                format,
                vec!["validate"],
                vec![diagnostic],
                json!({ "inputPath": input_path }),
                "invalid",
                ExitCode::from(1),
            );
        }
    };

    let artifact = match serde_json::from_str::<SummaryOnlySessionContextEnvelope>(&file_contents) {
        Ok(value) => value,
        Err(error) => {
            let diagnostic = Diagnostic::error(
                "CLI-MEMORY-002",
                format!(
                    "failed to parse or validate summary-only session-context artifact {}: {error}",
                    input.display()
                ),
            )
            .with_path(input_path.clone())
            .with_hint(
                "only governed summary-only envelopes are supported; mutation, promotion, persistence, and transcript-bearing payloads are out of scope",
            );

            return emit_diagnostics(
                format,
                vec!["validate"],
                vec![diagnostic],
                json!({ "inputPath": input_path }),
                "invalid",
                ExitCode::from(1),
            );
        }
    };

    let report = build_session_context_validation_report(&input, &artifact);
    match format {
        OutputFormat::Text => print_validated_session_context_text(&report),
        OutputFormat::Json => print_json(&Envelope {
            schema_version: MEMORY_CLI_SCHEMA_VERSION,
            command: vec!["validate".to_string()],
            status: "ok",
            summary: Summary::default(),
            data: report,
            diagnostics: Vec::new(),
        })?,
    }

    Ok(ExitCode::SUCCESS)
}

pub fn execute_init_command(
    root: Option<PathBuf>,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let root = resolve_local_root(root);
    let store = LocalMemoryStore::new(&root);

    match store.init() {
        Ok(initialized) => {
            let report = build_local_init_report(&initialized.paths);
            match format {
                OutputFormat::Text => print_local_init_text(&report),
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: MEMORY_CLI_SCHEMA_VERSION,
                    command: vec!["init".to_string()],
                    status: "ok",
                    summary: Summary::default(),
                    data: report,
                    diagnostics: Vec::new(),
                })?,
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_local_store_error(
            error,
            format,
            vec!["init"],
            json!({ "rootPath": root.display().to_string() }),
        ),
    }
}

pub fn execute_import_command(
    root: Option<PathBuf>,
    input: PathBuf,
    record_id: String,
    imported_at_utc: String,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let root = resolve_local_root(root);
    let store = LocalMemoryStore::new(&root);
    let input_path = input.display().to_string();
    let file_contents = match fs::read_to_string(&input) {
        Ok(value) => value,
        Err(error) => {
            let diagnostic = Diagnostic::error(
                "CLI-LOCAL-001",
                format!("failed to read local import artifact {}: {error}", input.display()),
            )
            .with_path(input_path.clone())
            .with_hint(
                "supply a readable summary-only session-context envelope; local import does not establish host authority",
            );

            return emit_diagnostics(
                format,
                vec!["import"],
                vec![diagnostic],
                json!({ "inputPath": input_path, "rootPath": root.display().to_string() }),
                "invalid",
                ExitCode::from(1),
            );
        }
    };

    let artifact = match serde_json::from_str::<SummaryOnlySessionContextEnvelope>(&file_contents) {
        Ok(value) => value,
        Err(error) => {
            let diagnostic = Diagnostic::error(
                "CLI-LOCAL-002",
                format!(
                    "failed to parse or validate local import artifact {}: {error}",
                    input.display()
                ),
            )
            .with_path(input_path.clone())
            .with_hint(
                "only governed summary-only envelopes are accepted here; local import remains non-authoritative artifact management only",
            );

            return emit_diagnostics(
                format,
                vec!["import"],
                vec![diagnostic],
                json!({ "inputPath": input_path, "rootPath": root.display().to_string() }),
                "invalid",
                ExitCode::from(1),
            );
        }
    };

    match store.import_summary_only_envelope(
        &artifact,
        GovernedMemoryRecordImportOptions {
            record_id,
            imported_at_utc,
        },
    ) {
        Ok(stored) => {
            let report =
                build_local_record_report(&root, &stored, &LocalMemoryQueryOptions::default());
            match format {
                OutputFormat::Text => {
                    print_local_record_text("imported local non-authoritative artifact", &report)
                }
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: MEMORY_CLI_SCHEMA_VERSION,
                    command: vec!["import".to_string()],
                    status: "ok",
                    summary: Summary::default(),
                    data: report,
                    diagnostics: Vec::new(),
                })?,
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_local_store_error(
            error,
            format,
            vec!["import"],
            json!({ "inputPath": input_path, "rootPath": root.display().to_string() }),
        ),
    }
}

pub fn execute_list_command(
    root: Option<PathBuf>,
    options: LocalMemoryQueryOptions,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let root = resolve_local_root(root);
    let store = LocalMemoryStore::new(&root);

    match store.list_records(&options) {
        Ok(records) => {
            let report = build_local_list_report(&root, &options, records);
            match format {
                OutputFormat::Text => print_local_list_text(&report),
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: MEMORY_CLI_SCHEMA_VERSION,
                    command: vec!["list".to_string()],
                    status: "ok",
                    summary: Summary::default(),
                    data: report,
                    diagnostics: Vec::new(),
                })?,
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_local_store_error(
            error,
            format,
            vec!["list"],
            json!({ "rootPath": root.display().to_string() }),
        ),
    }
}

pub fn execute_show_command(
    root: Option<PathBuf>,
    record_id: String,
    options: LocalMemoryQueryOptions,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let root = resolve_local_root(root);
    let store = LocalMemoryStore::new(&root);

    match store.show_record(&record_id, &options) {
        Ok(stored) => {
            let report = build_local_record_report(&root, &stored, &options);
            match format {
                OutputFormat::Text => {
                    print_local_record_text("local non-authoritative artifact record", &report)
                }
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: MEMORY_CLI_SCHEMA_VERSION,
                    command: vec!["show".to_string()],
                    status: "ok",
                    summary: Summary::default(),
                    data: report,
                    diagnostics: Vec::new(),
                })?,
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_local_store_error(
            error,
            format,
            vec!["show"],
            json!({
                "rootPath": root.display().to_string(),
                "recordId": record_id,
            }),
        ),
    }
}

pub fn execute_export_command(
    root: Option<PathBuf>,
    record_id: String,
    output_path: Option<PathBuf>,
    options: LocalMemoryQueryOptions,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let root = resolve_local_root(root);
    let store = LocalMemoryStore::new(&root);

    match store.export_summary_only_envelope(&record_id, output_path.as_deref(), &options) {
        Ok(result) => {
            let report = build_local_export_report(&root, &result, &options);
            match format {
                OutputFormat::Text => print_local_export_text(&report),
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: MEMORY_CLI_SCHEMA_VERSION,
                    command: vec!["export".to_string()],
                    status: "ok",
                    summary: Summary::default(),
                    data: report,
                    diagnostics: Vec::new(),
                })?,
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_local_store_error(
            error,
            format,
            vec!["export"],
            json!({
                "rootPath": root.display().to_string(),
                "recordId": record_id,
            }),
        ),
    }
}

pub fn execute_supersede_command(
    root: Option<PathBuf>,
    record_id: String,
    superseded_by_record_id: String,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let root = resolve_local_root(root);
    let store = LocalMemoryStore::new(&root);

    match store.supersede_record(&record_id, &superseded_by_record_id) {
        Ok(stored) => {
            let report = build_local_record_report(
                &root,
                &stored,
                &LocalMemoryQueryOptions {
                    include_superseded: true,
                    include_tombstoned: false,
                },
            );
            match format {
                OutputFormat::Text => {
                    print_local_record_text("marked local artifact as superseded", &report)
                }
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: MEMORY_CLI_SCHEMA_VERSION,
                    command: vec!["supersede".to_string()],
                    status: "ok",
                    summary: Summary::default(),
                    data: report,
                    diagnostics: Vec::new(),
                })?,
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_local_store_error(
            error,
            format,
            vec!["supersede"],
            json!({
                "rootPath": root.display().to_string(),
                "recordId": record_id,
                "supersededByRecordId": superseded_by_record_id,
            }),
        ),
    }
}

pub fn execute_tombstone_command(
    root: Option<PathBuf>,
    record_id: String,
    tombstoned_at_utc: String,
    reason: String,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let root = resolve_local_root(root);
    let store = LocalMemoryStore::new(&root);

    match store.tombstone_record(&record_id, &tombstoned_at_utc, &reason) {
        Ok(stored) => {
            let report = build_local_record_report(
                &root,
                &stored,
                &LocalMemoryQueryOptions {
                    include_superseded: true,
                    include_tombstoned: true,
                },
            );
            match format {
                OutputFormat::Text => print_local_record_text("tombstoned local artifact", &report),
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: MEMORY_CLI_SCHEMA_VERSION,
                    command: vec!["tombstone".to_string()],
                    status: "ok",
                    summary: Summary::default(),
                    data: report,
                    diagnostics: Vec::new(),
                })?,
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_local_store_error(
            error,
            format,
            vec!["tombstone"],
            json!({
                "rootPath": root.display().to_string(),
                "recordId": record_id,
            }),
        ),
    }
}

fn emit_diagnostics<T: Serialize>(
    format: OutputFormat,
    command: Vec<&str>,
    diagnostics: Vec<Diagnostic>,
    data: T,
    status: &'static str,
    exit_code: ExitCode,
) -> Result<ExitCode, serde_json::Error> {
    let summary = summarize(&diagnostics);
    match format {
        OutputFormat::Text => print_diagnostics_text(&diagnostics),
        OutputFormat::Json => print_json(&Envelope {
            schema_version: MEMORY_CLI_SCHEMA_VERSION,
            command: command.into_iter().map(str::to_string).collect(),
            status,
            summary,
            data,
            diagnostics,
        })?,
    }
    Ok(exit_code)
}

fn summarize(diagnostics: &[Diagnostic]) -> Summary {
    Summary {
        errors: diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
            .count(),
        warnings: diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
            .count(),
    }
}

fn print_session_context_text(inspection: &SessionContextInspection) {
    println!("summary-only session context artifact");
    println!("capability: {}", inspection.capability);
    println!("contract field: {}", inspection.contract_field);
    println!("schema: {}", inspection.schema_file);
    println!("representation: {}", inspection.representation);
    println!(
        "supported scopes: {}",
        inspection.supported_scopes.join(", ")
    );
    println!("consumers: {}", inspection.intended_consumers.join(", "));
    println!("bounded fields: {}", inspection.bounded_fields.join(", "));
    println!(
        "raw transcript persisted: {}",
        inspection.raw_transcript_persisted
    );
    println!(
        "transcript bodies allowed in artifact: {}",
        inspection.transcript_bodies_allowed_in_artifact
    );
}

fn print_validated_session_context_text(report: &SessionContextValidationReport) {
    println!("summary-only session context artifact is valid");
    println!("input: {}", report.input_path);
    println!("artifact kind: {}", report.artifact_kind);
    println!("representation: {}", report.representation);
    println!("scope: {}", report.scope);
    if let Some(request_id) = &report.request_id {
        println!("request id: {request_id}");
    }
    if let Some(run_id) = &report.run_id {
        println!("run id: {run_id}");
    }
    if let Some(captured_at_utc) = &report.captured_at_utc {
        println!("captured at utc: {captured_at_utc}");
    }
    println!("summary length: {}", report.summary_length);
    println!("summary preview: {}", report.summary_preview);
    println!("salient facts: {}", report.salient_facts_count);
    println!(
        "instruction context items: {}",
        report.instruction_context_count
    );
    println!(
        "raw transcript persisted: {}",
        report.raw_transcript_persisted
    );
    println!("read only: {}", report.read_only);
    println!(
        "neutral validation scope: {}",
        report.neutral_validation_scope
    );
    println!("authority posture: {}", report.authority_posture);
    println!("host validation owner: {}", report.host_validation_owner);
    println!("host promotion owner: {}", report.host_promotion_owner);
    println!(
        "host invalidation owner: {}",
        report.host_invalidation_owner
    );
    println!("adapter posture: {}", report.adapter_posture);
}

fn print_local_init_text(report: &LocalInitReport) {
    println!("initialized local non-authoritative artifact root");
    println!("root: {}", report.root_path);
    println!("artifacts: {}", report.artifacts_path);
    println!("state: {}", report.state_path);
    println!("write lock: {}", report.write_lock_path);
    println!("exports: {}", report.exports_path);
    println!("authority posture: {}", report.authority_posture);
    println!("single writer posture: {}", report.single_writer_posture);
    println!("deterministic ordering: {}", report.deterministic_ordering);
}

fn print_local_list_text(report: &LocalListReport) {
    println!("local non-authoritative artifact list");
    println!("root: {}", report.root_path);
    println!("authority posture: {}", report.authority_posture);
    println!("default visibility: {}", report.default_visibility);
    println!("deterministic ordering: {}", report.deterministic_ordering);
    println!("records: {}", report.records.len());
    for record in &report.records {
        println!(
            "- {} [{}] {} | {}",
            record.record_id,
            format_local_lifecycle_state(record.lifecycle_state),
            format_scope(record.scope),
            record.scope_captured_at_record_id
        );
    }
}

fn print_local_record_text(header: &str, report: &LocalRecordReport) {
    println!("{header}");
    println!("root: {}", report.root_path);
    println!("record id: {}", report.record.record_id);
    println!(
        "lifecycle state: {}",
        format_local_lifecycle_state(report.record.local_lifecycle.state)
    );
    println!(
        "scope: {}",
        format_scope(report.record.session_context.scope)
    );
    if let Some(captured_at_utc) = &report.record.provenance.captured_at_utc {
        println!("captured at utc: {captured_at_utc}");
    }
    println!(
        "imported at utc: {}",
        report.record.provenance.imported_at_utc
    );
    println!("artifact: {}", report.artifact_path);
    println!("default export: {}", report.default_export_path);
    println!(
        "summary preview: {}",
        truncate_for_preview(&report.record.session_context.summary)
    );
    if let Some(superseded_by_record_id) = &report.record.local_lifecycle.superseded_by_record_id {
        println!("superseded by local record: {superseded_by_record_id}");
    }
    if let Some(tombstone) = &report.record.local_lifecycle.tombstone {
        println!("tombstoned at utc: {}", tombstone.tombstoned_at_utc);
        println!("tombstone reason: {}", tombstone.reason);
    }
    println!("authority posture: {}", report.authority_posture);
    println!("default visibility: {}", report.default_visibility);
    println!("deterministic ordering: {}", report.deterministic_ordering);
}

fn print_local_export_text(report: &LocalExportReport) {
    println!("exported local non-authoritative summary-only artifact");
    println!("root: {}", report.root_path);
    println!("record id: {}", report.record.record_id);
    println!("output: {}", report.output_path);
    println!(
        "lifecycle state: {}",
        format_local_lifecycle_state(report.record.local_lifecycle.state)
    );
    println!(
        "scope: {}",
        format_scope(report.record.session_context.scope)
    );
    println!("authority posture: {}", report.authority_posture);
    println!("default visibility: {}", report.default_visibility);
    println!("deterministic ordering: {}", report.deterministic_ordering);
}

fn print_diagnostics_text(diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        let mut location = String::new();
        if let Some(path) = &diagnostic.location.path {
            location.push_str(path);
        }
        if let Some(field) = &diagnostic.location.field {
            if !location.is_empty() {
                location.push('#');
            }
            location.push_str(field);
        }

        if location.is_empty() {
            eprintln!(
                "{}[{}]: {}",
                severity_label(diagnostic),
                diagnostic.code,
                diagnostic.message
            );
        } else {
            eprintln!(
                "{}[{}] {}: {}",
                severity_label(diagnostic),
                diagnostic.code,
                location,
                diagnostic.message
            );
        }

        if let Some(hint) = &diagnostic.hint {
            eprintln!("  hint: {hint}");
        }
    }
}

fn severity_label(diagnostic: &Diagnostic) -> &'static str {
    match diagnostic.severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
    }
}

fn session_context_inspection() -> SessionContextInspection {
    SessionContextInspection {
        capability: "summary-only-session-context-envelope",
        contract_field: "summary-only-session-context-envelope.sessionContext",
        schema_file: "contracts/schemas/summary-only-session-context-envelope.schema.json",
        representation: "summary-only",
        supported_scopes: vec!["run", "session", "workspace"],
        intended_consumers: vec!["instruction-engine", "workspace-bootstrap", "agent-runtime"],
        bounded_fields: vec![
            "summary",
            "salientFacts",
            "instructionContext",
            "rawTranscriptPersisted",
        ],
        raw_transcript_persisted: false,
        transcript_bodies_allowed_in_artifact: false,
    }
}

fn build_session_context_validation_report(
    input: &Path,
    artifact: &SummaryOnlySessionContextEnvelope,
) -> SessionContextValidationReport {
    SessionContextValidationReport {
        input_path: input.display().to_string(),
        artifact_kind: SUMMARY_ONLY_SESSION_CONTEXT_ARTIFACT_KIND,
        representation: SUMMARY_ONLY_REPRESENTATION,
        scope: format_scope(artifact.session_context.scope),
        request_id: artifact.request_id.clone(),
        run_id: artifact.run_id.clone(),
        captured_at_utc: artifact.captured_at_utc.clone(),
        summary_length: artifact.session_context.summary.chars().count(),
        summary_preview: truncate_for_preview(&artifact.session_context.summary),
        salient_facts_count: artifact.session_context.salient_facts.len(),
        instruction_context_count: artifact.session_context.instruction_context.len(),
        raw_transcript_persisted: artifact.session_context.raw_transcript_persisted,
        read_only: true,
        neutral_validation_scope: SESSION_CONTEXT_NEUTRAL_VALIDATION_SCOPE,
        authority_posture: SESSION_CONTEXT_AUTHORITY_POSTURE,
        host_validation_owner: SESSION_CONTEXT_HOST_OWNER,
        host_promotion_owner: SESSION_CONTEXT_HOST_OWNER,
        host_invalidation_owner: SESSION_CONTEXT_HOST_OWNER,
        adapter_posture: SESSION_CONTEXT_ADAPTER_POSTURE,
    }
}

fn build_local_init_report(paths: &LocalMemoryPaths) -> LocalInitReport {
    LocalInitReport {
        root_path: paths.root.display().to_string(),
        artifacts_path: paths.artifacts_dir.display().to_string(),
        state_path: paths.state_dir.display().to_string(),
        write_lock_path: paths.write_lock_path.display().to_string(),
        exports_path: paths.exports_dir.display().to_string(),
        authority_posture: LOCAL_MEMORY_AUTHORITY_POSTURE,
        single_writer_posture: LOCAL_MEMORY_SINGLE_WRITER_POSTURE,
        deterministic_ordering: LOCAL_MEMORY_DETERMINISTIC_ORDERING,
    }
}

fn build_local_list_report(
    root: &Path,
    options: &LocalMemoryQueryOptions,
    records: Vec<LocalMemoryCatalogEntry>,
) -> LocalListReport {
    LocalListReport {
        root_path: root.display().to_string(),
        authority_posture: LOCAL_MEMORY_AUTHORITY_POSTURE,
        default_visibility: format!(
            "{}; {}",
            options.default_filter_label(),
            LOCAL_DEFAULT_VISIBILITY_POSTURE
        ),
        deterministic_ordering: LOCAL_MEMORY_DETERMINISTIC_ORDERING,
        records,
    }
}

fn build_local_record_report(
    root: &Path,
    stored: &LocalMemoryStoredRecord,
    options: &LocalMemoryQueryOptions,
) -> LocalRecordReport {
    let store = LocalMemoryStore::new(root);
    LocalRecordReport {
        root_path: root.display().to_string(),
        artifact_path: stored.artifact_path.display().to_string(),
        default_export_path: store
            .default_export_path(&stored.record.record_id)
            .display()
            .to_string(),
        authority_posture: LOCAL_MEMORY_AUTHORITY_POSTURE,
        default_visibility: format!(
            "{}; {}",
            options.default_filter_label(),
            LOCAL_DEFAULT_VISIBILITY_POSTURE
        ),
        deterministic_ordering: LOCAL_MEMORY_DETERMINISTIC_ORDERING,
        record: stored.record.clone(),
    }
}

fn build_local_export_report(
    root: &Path,
    result: &LocalMemoryExportResult,
    options: &LocalMemoryQueryOptions,
) -> LocalExportReport {
    LocalExportReport {
        root_path: root.display().to_string(),
        output_path: result.output_path.display().to_string(),
        authority_posture: LOCAL_MEMORY_AUTHORITY_POSTURE,
        default_visibility: format!(
            "{}; {}",
            options.default_filter_label(),
            LOCAL_DEFAULT_VISIBILITY_POSTURE
        ),
        deterministic_ordering: LOCAL_MEMORY_DETERMINISTIC_ORDERING,
        record: result.record.clone(),
        exported_envelope: result.exported_envelope.clone(),
    }
}

fn emit_local_store_error<T: Serialize>(
    error: LocalMemoryStoreError,
    format: OutputFormat,
    command: Vec<&str>,
    data: T,
) -> Result<ExitCode, serde_json::Error> {
    emit_diagnostics(
        format,
        command,
        local_store_error_diagnostics(error),
        data,
        "invalid",
        ExitCode::from(1),
    )
}

fn local_store_error_diagnostics(error: LocalMemoryStoreError) -> Vec<Diagnostic> {
    match error {
        LocalMemoryStoreError::RootNotInitialized { root } => vec![Diagnostic::error(
            "CLI-LOCAL-003",
            format!("local artifact root is not initialized: {}", root.display()),
        )
        .with_path(root.display().to_string())
        .with_hint("run `elegy-memory init --root <path>` before using local artifact commands")],
        LocalMemoryStoreError::ConcurrentWriterRejected { root } => vec![Diagnostic::error(
            "CLI-LOCAL-004",
            format!(
                "concurrent local writer rejected for {}; state/write.lock already exists",
                root.display()
            ),
        )
        .with_path(root.display().to_string())
        .with_hint(
            "local artifact writes assume a single writer; retry after the current writer exits or remove a stale lock intentionally",
        )],
        LocalMemoryStoreError::RecordNotFound { record_id } => vec![Diagnostic::error(
            "CLI-LOCAL-005",
            format!("local record was not found: {record_id}"),
        )
        .with_field(record_id)],
        LocalMemoryStoreError::RecordExcludedByLifecycle { record_id, state } => {
            vec![Diagnostic::error(
                "CLI-LOCAL-006",
                format!(
                    "local record `{record_id}` is `{}` and is hidden by the default active-only filter",
                    format_local_lifecycle_state(state)
                ),
            )
            .with_field(record_id)
            .with_hint(
                "pass --include-superseded or --include-tombstoned when you explicitly want non-active local records",
            )]
        }
        LocalMemoryStoreError::RecordIdConflict { record_id } => vec![Diagnostic::error(
            "CLI-LOCAL-007",
            format!("local record ID collision detected for {record_id}"),
        )
        .with_field(record_id)
        .with_hint(
            "reuse the same record ID only for the same governed artifact contents; choose a different local record ID otherwise",
        )],
        LocalMemoryStoreError::SelfSupersede { record_id } => vec![Diagnostic::error(
            "CLI-LOCAL-008",
            format!("local record `{record_id}` cannot supersede itself"),
        )
        .with_field(record_id)],
        LocalMemoryStoreError::SuccessorRecordNotFound { record_id } => vec![Diagnostic::error(
            "CLI-LOCAL-009",
            format!("successor local record was not found: {record_id}"),
        )
        .with_field(record_id)
        .with_hint("import the successor local record before linking another record to it")],
        LocalMemoryStoreError::Io {
            operation,
            path,
            source,
        } => vec![Diagnostic::error(
            "CLI-LOCAL-010",
            format!("failed to {operation} {}: {source}", path.display()),
        )
        .with_path(path.display().to_string())],
        LocalMemoryStoreError::InvalidArtifactJson { path, source } => vec![Diagnostic::error(
            "CLI-LOCAL-011",
            format!("invalid local governed-memory artifact JSON {}: {source}", path.display()),
        )
        .with_path(path.display().to_string())],
        LocalMemoryStoreError::InvalidJsonSerialization { path, source } => vec![Diagnostic::error(
            "CLI-LOCAL-012",
            format!("failed to serialize local JSON {}: {source}", path.display()),
        )
        .with_path(path.display().to_string())],
        LocalMemoryStoreError::MemoryValidation(error) => vec![Diagnostic::error(
            "CLI-LOCAL-013",
            error.to_string(),
        )
        .with_hint(
            "local artifact management accepts only governed memory shapes and bounded local lifecycle metadata",
        )],
    }
}

fn resolve_local_root(root: Option<PathBuf>) -> PathBuf {
    root.unwrap_or_else(|| PathBuf::from(LOCAL_DEFAULT_ROOT_DIR))
}

fn format_scope(scope: SessionContextScope) -> &'static str {
    match scope {
        SessionContextScope::Run => "run",
        SessionContextScope::Session => "session",
        SessionContextScope::Workspace => "workspace",
    }
}

fn format_local_lifecycle_state(state: LocalMemoryLifecycleState) -> &'static str {
    match state {
        LocalMemoryLifecycleState::Active => "active",
        LocalMemoryLifecycleState::Superseded => "superseded",
        LocalMemoryLifecycleState::Tombstoned => "tombstoned",
    }
}

fn truncate_for_preview(value: &str) -> String {
    let char_count = value.chars().count();
    if char_count <= SESSION_CONTEXT_PREVIEW_LIMIT {
        return value.to_string();
    }

    let preview: String = value.chars().take(SESSION_CONTEXT_PREVIEW_LIMIT).collect();
    format!("{preview}...")
}

fn print_json<T: Serialize>(value: &T) -> Result<(), serde_json::Error> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
