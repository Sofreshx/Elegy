use serde_json::Value;
use std::process::Command;

#[test]
fn dedicated_configuration_cli_supports_machine_flags_and_correlation_id() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-configuration"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-config-1",
            "list",
        ])
        .output()
        .expect("run elegy-configuration list with machine flags");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["correlationId"], "corr-config-1");
    assert_eq!(body["nonInteractive"], true);
}

#[test]
fn dedicated_configuration_cli_generates_correlation_id_for_blank_input() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-configuration"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "",
            "list",
        ])
        .output()
        .expect("run elegy-configuration list with blank correlation id");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    let correlation_id = body["correlationId"]
        .as_str()
        .expect("correlationId should be a string");
    assert!(
        correlation_id.starts_with("elegy-configuration-"),
        "unexpected generated correlation id: {correlation_id}"
    );
    assert_eq!(body["nonInteractive"], true);
}
