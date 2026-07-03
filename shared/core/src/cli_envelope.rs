// ── CLI Machine Envelope types ──────────────────────────────────────────

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::machine_types::{StructuredFailure, StructuredFailureCategory};

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
