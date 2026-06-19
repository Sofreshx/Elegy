use elegy_contracts::{
    validate_execution_event, validate_invocation_request, validate_invocation_response,
    validate_structured_failure, ExecutionEvent, ExecutionEventStatus, ExecutionEventType,
    InvocationContext, InvocationRequest, InvocationResponse, InvocationStatus, StructuredFailure,
    CLI_SCHEMA_VERSION,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MachineEnvelope {
    schema_version: String,
    correlation_id: String,
    #[serde(default)]
    non_interactive: bool,
    command: Vec<String>,
    status: String,
    #[serde(default)]
    data: Option<Value>,
    #[serde(default)]
    failure: Option<StructuredFailure>,
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}-{unique}"));
    fs::create_dir_all(&dir).expect("create temp directory");
    dir
}

fn write_profile(name: &str, body: &str) -> PathBuf {
    let dir = unique_temp_dir("elegy-skills-conformance-profile");
    let path = dir.join(name);
    fs::write(&path, body).expect("write profile");
    path
}

fn parse_machine_envelope(stdout: &[u8]) -> MachineEnvelope {
    serde_json::from_slice(stdout).expect("stdout should be valid machine json")
}

fn build_invocation_context(
    correlation_id: &str,
    execution_id: &str,
    caller_ref: &str,
    trace_ref: &str,
    surface: &str,
) -> InvocationContext {
    InvocationContext {
        correlation_id: correlation_id.to_string(),
        execution_id: execution_id.to_string(),
        requested_at: "2026-05-28T12:00:00Z".to_string(),
        timeout_seconds: Some(30),
        caller_ref: Some(caller_ref.to_string()),
        policy_context: Some(BTreeMap::from([(
            "mode".to_string(),
            "non-interactive".to_string(),
        )])),
        trace_ref: Some(trace_ref.to_string()),
        metadata: BTreeMap::from([("surface".to_string(), surface.to_string())]),
    }
}

#[test]
fn list_command_machine_output_maps_cleanly_to_invocation_contracts() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-skills-map-1",
            "list",
        ])
        .output()
        .expect("run elegy-skills list for conformance");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let envelope = parse_machine_envelope(&output.stdout);
    assert_eq!(envelope.schema_version, CLI_SCHEMA_VERSION);
    assert_eq!(envelope.status, "ok");
    assert_eq!(envelope.correlation_id, "corr-skills-map-1");
    assert!(envelope.non_interactive);
    assert_eq!(envelope.command, ["list"]);

    let data = envelope.data.clone().expect("successful command data");
    assert!(data["skills"].is_array(), "unexpected list data: {data}");

    let request = InvocationRequest {
        request_id: "invoke-skills-list-1".to_string(),
        capability_id: "elegy.skills.list".to_string(),
        input: json!({
            "command": envelope.command,
        }),
        context: build_invocation_context(
            &envelope.correlation_id,
            "exec-skills-list-1",
            "integration-test:elegy-skills-conformance",
            "trace-skills-list-1",
            "elegy-skills-machine-envelope",
        ),
    };
    let request_validation = validate_invocation_request(&request);
    assert!(
        request_validation.is_valid(),
        "unexpected request issues: {:?}",
        request_validation.issues
    );

    let response = InvocationResponse {
        request_id: request.request_id.clone(),
        execution_id: request.context.execution_id.clone(),
        status: InvocationStatus::Completed,
        output: Some(json!({
            "command": ["list"],
            "data": data,
        })),
        failure: None,
        completed_at: Some("2026-05-28T12:00:01Z".to_string()),
        trace_ref: request.context.trace_ref.clone(),
        metadata: BTreeMap::from([
            ("surface".to_string(), "elegy-skills".to_string()),
            (
                "mappedFrom".to_string(),
                "skills-list-machine-output".to_string(),
            ),
        ]),
    };
    let response_validation = validate_invocation_response(&response);
    assert!(
        response_validation.is_valid(),
        "unexpected response issues: {:?}",
        response_validation.issues
    );

    let event = ExecutionEvent {
        event_id: "exec-event-skills-list-1".to_string(),
        request_id: request.request_id.clone(),
        execution_id: request.context.execution_id.clone(),
        sequence: 1,
        timestamp: "2026-05-28T12:00:01Z".to_string(),
        event_type: ExecutionEventType::Completed,
        status: ExecutionEventStatus::Completed,
        correlation_id: Some(request.context.correlation_id.clone()),
        trace_ref: request.context.trace_ref.clone(),
        capability_id: Some(request.capability_id.clone()),
        message: Some("elegy-skills list completed successfully".to_string()),
        progress: None,
        failure: None,
        metadata: BTreeMap::from([
            ("surface".to_string(), "elegy-skills".to_string()),
            ("command".to_string(), "list".to_string()),
        ]),
    };
    let event_validation = validate_execution_event(&event);
    assert!(
        event_validation.is_valid(),
        "unexpected event issues: {:?}",
        event_validation.issues
    );

    assert!(response.output.as_ref().expect("completed output")["data"]["skills"].is_array());
}

#[test]
fn invalid_profile_machine_output_maps_cleanly_to_failure_contracts() {
    let profile = write_profile(
        "bad-profile.json",
        r#"{
  "schemaVersion": "agent-capability-profile/v1",
  "profileId": "bad-profile",
  "includeSkills": ["not-a-skill"],
  "alwaysIncludeRouter": false
}"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-skills-invalid-1",
            "--profile",
            profile.to_str().expect("utf-8 profile path"),
            "list",
        ])
        .output()
        .expect("run elegy-skills invalid profile conformance");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let envelope = parse_machine_envelope(&output.stdout);
    assert_eq!(envelope.schema_version, CLI_SCHEMA_VERSION);
    assert_eq!(envelope.status, "invalid");
    assert_eq!(envelope.correlation_id, "corr-skills-invalid-1");
    assert!(envelope.non_interactive);
    assert_eq!(envelope.command, ["list"]);

    let data = envelope.data.clone().expect("invalid command diagnostics");
    assert_eq!(data["profileProvided"], json!(true));

    let failure = envelope.failure.expect("structured failure");
    assert_eq!(failure.code, "CLI-INVALID-INPUT");
    assert_eq!(
        failure.correlation_id.as_deref(),
        Some("corr-skills-invalid-1")
    );
    assert!(failure.message.contains("unknown skill 'not-a-skill'"));

    let failure_validation = validate_structured_failure(&failure);
    assert!(
        failure_validation.is_valid(),
        "unexpected failure issues: {:?}",
        failure_validation.issues
    );

    let request = InvocationRequest {
        request_id: "invoke-skills-invalid-1".to_string(),
        capability_id: "elegy.skills.list".to_string(),
        input: json!({
            "command": ["list"],
            "profilePath": profile.display().to_string(),
        }),
        context: build_invocation_context(
            "corr-skills-invalid-1",
            "exec-skills-invalid-1",
            "integration-test:elegy-skills-conformance",
            "trace-skills-invalid-1",
            "elegy-skills-machine-envelope",
        ),
    };
    let request_validation = validate_invocation_request(&request);
    assert!(
        request_validation.is_valid(),
        "unexpected request issues: {:?}",
        request_validation.issues
    );

    let response = InvocationResponse {
        request_id: request.request_id.clone(),
        execution_id: request.context.execution_id.clone(),
        status: InvocationStatus::Failed,
        output: None,
        failure: Some(failure.clone()),
        completed_at: Some("2026-05-28T12:05:00Z".to_string()),
        trace_ref: request.context.trace_ref.clone(),
        metadata: BTreeMap::from([
            ("surface".to_string(), "elegy-skills".to_string()),
            (
                "mappedFrom".to_string(),
                "skills-invalid-profile-machine-output".to_string(),
            ),
        ]),
    };
    let response_validation = validate_invocation_response(&response);
    assert!(
        response_validation.is_valid(),
        "unexpected response issues: {:?}",
        response_validation.issues
    );

    let event = ExecutionEvent {
        event_id: "exec-event-skills-invalid-1".to_string(),
        request_id: request.request_id.clone(),
        execution_id: request.context.execution_id.clone(),
        sequence: 1,
        timestamp: "2026-05-28T12:05:00Z".to_string(),
        event_type: ExecutionEventType::Failed,
        status: ExecutionEventStatus::Failed,
        correlation_id: Some(request.context.correlation_id.clone()),
        trace_ref: request.context.trace_ref.clone(),
        capability_id: Some(request.capability_id.clone()),
        message: Some("elegy-skills list failed".to_string()),
        progress: None,
        failure: Some(failure),
        metadata: BTreeMap::from([
            ("surface".to_string(), "elegy-skills".to_string()),
            ("command".to_string(), "list".to_string()),
        ]),
    };
    let event_validation = validate_execution_event(&event);
    assert!(
        event_validation.is_valid(),
        "unexpected event issues: {:?}",
        event_validation.issues
    );
}
