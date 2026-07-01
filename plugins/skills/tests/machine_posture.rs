use std::fs;
use std::path::PathBuf;
use std::process::Command;
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
fn list_command_supports_machine_flags_and_correlation_id() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-skills-1",
            "list",
        ])
        .output()
        .expect("run elegy-skills list");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"ok\""));
    assert!(stdout.contains("\"correlationId\": \"corr-skills-1\""));
    assert!(stdout.contains("\"nonInteractive\": true"));
    assert!(stdout.contains("\"skills\""));
}

#[test]
fn validate_command_emits_structured_error_with_machine_flags() {
    let temp_dir = unique_temp_dir("elegy-skills-machine-error");
    let missing_path = temp_dir.join("missing.json");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-skills-err-1",
            "validate",
            "--file",
            missing_path.to_str().expect("utf-8 descriptor path"),
        ])
        .output()
        .expect("run elegy-skills validate missing file");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"error\""));
    assert!(stdout.contains("\"correlationId\": \"corr-skills-err-1\""));
    assert!(stdout.contains("\"nonInteractive\": true"));
}

#[test]
fn list_command_generates_correlation_id_when_absent() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args(["--json", "--non-interactive", "list"])
        .output()
        .expect("run elegy-skills list without correlation id");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    let correlation_id = stdout["correlationId"]
        .as_str()
        .expect("correlationId should be a string");
    assert!(
        correlation_id.starts_with("elegy-skills-"),
        "unexpected generated correlation id: {correlation_id}"
    );
}

#[test]
fn blank_correlation_id_argument_is_treated_as_absent() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "",
            "list",
        ])
        .output()
        .expect("run elegy-skills list with blank correlation id");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    let correlation_id = stdout["correlationId"]
        .as_str()
        .expect("correlationId should be a string");
    assert!(
        correlation_id.starts_with("elegy-skills-"),
        "unexpected generated correlation id: {correlation_id}"
    );
}
