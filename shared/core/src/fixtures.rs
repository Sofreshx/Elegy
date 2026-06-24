use crate::error::ContractsError;
use crate::machine_types::{
    AgentEventEnvelope, AgentRequestEnvelope, AgentResponseEnvelope, InvocationRequest,
    InvocationResponse, StructuredFailure,
};
use std::fs;
use std::path::Path;

fn load_json_file<T>(path: &Path) -> Result<T, ContractsError>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let content = fs::read_to_string(path).map_err(|source| ContractsError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&content).map_err(|source| ContractsError::Json {
        path: path.to_path_buf(),
        source,
    })
}

pub fn load_structured_failure_fixture_from_dir(
    dir: &Path,
) -> Result<StructuredFailure, ContractsError> {
    load_json_file(&dir.join("fixtures").join("structured-failure.minimal.json"))
}

pub fn load_invocation_request_fixture_from_dir(
    dir: &Path,
) -> Result<InvocationRequest, ContractsError> {
    load_json_file(&dir.join("fixtures").join("invocation-request.minimal.json"))
}

pub fn load_invocation_response_fixture_from_dir(
    dir: &Path,
) -> Result<InvocationResponse, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("invocation-response.minimal.json"),
    )
}

pub fn load_agent_capability_profile_fixture_from_dir(
    dir: &Path,
) -> Result<crate::machine_types::AgentCapabilityProfile, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("agent-capability-profile.minimal.json"),
    )
}

pub fn load_agent_request_envelope_fixture_from_dir(
    dir: &Path,
) -> Result<AgentRequestEnvelope, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("agent-request-envelope.minimal.json"),
    )
}

pub fn load_agent_response_envelope_fixture_from_dir(
    dir: &Path,
) -> Result<AgentResponseEnvelope, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("agent-response-envelope.minimal.json"),
    )
}

pub fn load_agent_event_envelope_fixture_from_dir(
    dir: &Path,
) -> Result<AgentEventEnvelope, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("agent-event-envelope.minimal.json"),
    )
}
