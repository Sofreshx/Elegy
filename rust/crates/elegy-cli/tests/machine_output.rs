use serde_json::Value;
use std::process::Command;

#[test]
fn version_json_generates_correlation_id_when_absent() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args(["--version", "--json"])
        .output()
        .expect("run elegy version in json mode");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let envelope: Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be valid machine json");
    let correlation_id = envelope["correlationId"]
        .as_str()
        .expect("correlationId should be a string");

    assert!(
        correlation_id.starts_with("elegy-cli-"),
        "unexpected generated correlation id: {correlation_id}"
    );
    assert!(
        correlation_id.len() > "elegy-cli-".len(),
        "generated correlation id should not be empty"
    );
}

#[test]
fn blank_correlation_id_argument_is_treated_as_absent() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args(["--version", "--json", "--correlation-id", "   "])
        .output()
        .expect("run elegy version with blank correlation id");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let envelope: Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be valid machine json");
    let correlation_id = envelope["correlationId"]
        .as_str()
        .expect("correlationId should be a string");

    assert!(
        correlation_id.starts_with("elegy-cli-"),
        "unexpected generated correlation id: {correlation_id}"
    );
}

