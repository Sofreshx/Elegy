use serde::Serialize;

use crate::cli::RESULT_SCHEMA_VERSION;

/// Machine-mode output status
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MachineStatus {
    Ok,
    Invalid,
    Error,
}

impl MachineStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MachineStatus::Ok => "ok",
            MachineStatus::Invalid => "invalid",
            MachineStatus::Error => "error",
        }
    }
}

/// Typed envelope for all machine-mode CLI output.
///
/// Conforms to `planning-result/v1` wire format declared in
/// `schemas/planning-result.schema.json`.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MachineEnvelope<T>
where
    T: Serialize,
{
    pub schema_version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub non_interactive: bool,
    pub command: Vec<String>,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> MachineEnvelope<T> {
    pub fn ok(
        correlation_id: Option<String>,
        non_interactive: bool,
        command: Vec<String>,
        data: T,
    ) -> Self {
        Self {
            schema_version: RESULT_SCHEMA_VERSION,
            correlation_id,
            non_interactive,
            command,
            status: MachineStatus::Ok.as_str(),
            data: Some(data),
            error: None,
        }
    }

    pub fn error(
        correlation_id: Option<String>,
        non_interactive: bool,
        command: Vec<String>,
        status: MachineStatus,
        error_message: String,
    ) -> Self {
        Self {
            schema_version: RESULT_SCHEMA_VERSION,
            correlation_id,
            non_interactive,
            command,
            status: status.as_str(),
            data: None,
            error: Some(error_message),
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}
