use elegy_contracts::{
    validate_execution_event, validate_invocation_request, validate_invocation_response,
    validate_structured_failure, ExecutionEvent, ExecutionEventStatus, ExecutionEventType,
    InvocationContext, InvocationRequest, InvocationResponse, InvocationStatus,
    StructuredFailure, CLI_SCHEMA_VERSION,
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
        requested_at: "2026-05-28T12:20:00Z".to_string(),
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
fn health_command_machine_output_maps_cleanly_to_invocation_contracts() {
    let temp_dir = unique_temp_dir("elegy-memory-health-conformance");
    let db_path = temp_dir.join("memory.sqlite3");

    let add = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "add",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "Conformance health memory",
        ])
        .output()
        .expect("seed memory database");
    assert!(
        add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-memory-map-1",
            "health",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
        ])
        .output()
        .expect("run elegy-memory health for conformance");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let envelope = parse_machine_envelope(&output.stdout);
    assert_eq!(envelope.schema_version, CLI_SCHEMA_VERSION);
    assert_eq!(envelope.status, "ok");
    assert_eq!(envelope.correlation_id, "corr-memory-map-1");
    assert!(envelope.non_interactive);
    assert_eq!(envelope.command, ["health"]);

    let data = envelope.data.clone().expect("successful command data");
    assert!(
        data["averageImportance"].is_number(),
        "unexpected health data: {data}"
    );

    let request = InvocationRequest {
        request_id: "invoke-memory-health-1".to_string(),
        capability_id: "elegy.memory.health".to_string(),
        input: json!({
            "command": envelope.command,
            "dbPath": db_path.display().to_string(),
        }),
        context: build_invocation_context(
            &envelope.correlation_id,
            "exec-memory-health-1",
            "integration-test:elegy-memory-conformance",
            "trace-memory-health-1",
            "elegy-memory-machine-envelope",
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
            "command": ["health"],
            "data": data,
        })),
        failure: None,
        completed_at: Some("2026-05-28T12:20:01Z".to_string()),
        trace_ref: request.context.trace_ref.clone(),
        metadata: BTreeMap::from([
            ("surface".to_string(), "elegy-memory".to_string()),
            (
                "mappedFrom".to_string(),
                "memory-health-machine-output".to_string(),
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
        event_id: "exec-event-memory-health-1".to_string(),
        request_id: request.request_id.clone(),
        execution_id: request.context.execution_id.clone(),
        sequence: 1,
        timestamp: "2026-05-28T12:20:01Z".to_string(),
        event_type: ExecutionEventType::Completed,
        status: ExecutionEventStatus::Completed,
        correlation_id: Some(request.context.correlation_id.clone()),
        trace_ref: request.context.trace_ref.clone(),
        capability_id: Some(request.capability_id.clone()),
        message: Some("elegy-memory health completed successfully".to_string()),
        progress: None,
        failure: None,
        metadata: BTreeMap::from([
            ("surface".to_string(), "elegy-memory".to_string()),
            ("command".to_string(), "health".to_string()),
        ]),
    };
    let event_validation = validate_execution_event(&event);
    assert!(
        event_validation.is_valid(),
        "unexpected event issues: {:?}",
        event_validation.issues
    );

    assert!(
        response.output.as_ref().expect("completed output")["data"]["averageImportance"].is_number()
    );
}

#[test]
fn purge_without_yes_machine_output_maps_cleanly_to_failure_contracts() {
    let temp_dir = unique_temp_dir("elegy-memory-purge-conformance");
    let db_path = temp_dir.join("memory.sqlite3");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-memory-invalid-1",
            "purge",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
        ])
        .output()
        .expect("run elegy-memory purge without yes for conformance");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let envelope = parse_machine_envelope(&output.stdout);
    assert_eq!(envelope.schema_version, CLI_SCHEMA_VERSION);
    assert_eq!(envelope.status, "invalid");
    assert_eq!(envelope.correlation_id, "corr-memory-invalid-1");
    assert!(envelope.non_interactive);
    assert_eq!(envelope.command, ["purge"]);
    assert!(envelope.data.is_none());

    let failure = envelope.failure.expect("structured failure");
    assert_eq!(failure.code, "CLI-INVALID-INPUT");
    assert_eq!(failure.correlation_id.as_deref(), Some("corr-memory-invalid-1"));
    assert!(failure
        .message
        .contains("purge requires --yes when --non-interactive is set"));

    let failure_validation = validate_structured_failure(&failure);
    assert!(
        failure_validation.is_valid(),
        "unexpected failure issues: {:?}",
        failure_validation.issues
    );

    let request = InvocationRequest {
        request_id: "invoke-memory-purge-invalid-1".to_string(),
        capability_id: "elegy.memory.purge".to_string(),
        input: json!({
            "command": ["purge"],
            "dbPath": db_path.display().to_string(),
            "nonInteractive": true,
        }),
        context: build_invocation_context(
            "corr-memory-invalid-1",
            "exec-memory-purge-invalid-1",
            "integration-test:elegy-memory-conformance",
            "trace-memory-purge-invalid-1",
            "elegy-memory-machine-envelope",
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
        completed_at: Some("2026-05-28T12:22:00Z".to_string()),
        trace_ref: request.context.trace_ref.clone(),
        metadata: BTreeMap::from([
            ("surface".to_string(), "elegy-memory".to_string()),
            (
                "mappedFrom".to_string(),
                "memory-purge-machine-output".to_string(),
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
        event_id: "exec-event-memory-purge-invalid-1".to_string(),
        request_id: request.request_id.clone(),
        execution_id: request.context.execution_id.clone(),
        sequence: 1,
        timestamp: "2026-05-28T12:22:00Z".to_string(),
        event_type: ExecutionEventType::Failed,
        status: ExecutionEventStatus::Failed,
        correlation_id: Some(request.context.correlation_id.clone()),
        trace_ref: request.context.trace_ref.clone(),
        capability_id: Some(request.capability_id.clone()),
        message: Some("elegy-memory purge failed".to_string()),
        progress: None,
        failure: Some(failure),
        metadata: BTreeMap::from([
            ("surface".to_string(), "elegy-memory".to_string()),
            ("command".to_string(), "purge".to_string()),
        ]),
    };
    let event_validation = validate_execution_event(&event);
    assert!(
        event_validation.is_valid(),
        "unexpected event issues: {:?}",
        event_validation.issues
    );
}
