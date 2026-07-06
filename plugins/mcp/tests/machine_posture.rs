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
fn author_command_supports_machine_flags_and_correlation_id() {
    let temp_dir = unique_temp_dir("elegy-mcp-machine");
    let output_path = temp_dir.join("server.json");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-mcp"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-mcp-1",
            "author",
            "--server-name",
            "weather-server",
            "--tool",
            "get-weather=Look up weather",
            "--output",
            output_path.to_str().expect("utf-8 output path"),
        ])
        .output()
        .expect("run elegy-mcp author");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_path.is_file());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"ok\""));
    assert!(stdout.contains("\"correlationId\": \"corr-mcp-1\""));
    assert!(stdout.contains("\"nonInteractive\": true"));
}

#[test]
fn analyze_command_emits_structured_error_with_machine_flags() {
    let temp_dir = unique_temp_dir("elegy-mcp-machine-error");
    let missing_path = temp_dir.join("missing.json");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-mcp"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-mcp-err-1",
            "analyze",
            "--descriptor",
            missing_path.to_str().expect("utf-8 descriptor path"),
        ])
        .output()
        .expect("run elegy-mcp analyze missing descriptor");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("\"correlationId\": \"corr-mcp-err-1\""));
    assert!(stdout.contains("\"nonInteractive\": true"));
    assert!(stdout.contains("\"failure\""));
}

#[test]
fn author_command_generates_correlation_id_when_absent() {
    let temp_dir = unique_temp_dir("elegy-mcp-machine-generated-correlation");
    let output_path = temp_dir.join("server.json");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-mcp"))
        .args([
            "--json",
            "--non-interactive",
            "author",
            "--server-name",
            "weather-server",
            "--tool",
            "get-weather=Look up weather",
            "--output",
            output_path.to_str().expect("utf-8 output path"),
        ])
        .output()
        .expect("run elegy-mcp author without correlation id");

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
        correlation_id.starts_with("elegy-mcp-"),
        "unexpected generated correlation id: {correlation_id}"
    );
}

#[test]
fn blank_correlation_id_argument_is_treated_as_absent() {
    let temp_dir = unique_temp_dir("elegy-mcp-machine-blank-correlation");
    let output_path = temp_dir.join("server.json");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-mcp"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "",
            "author",
            "--server-name",
            "weather-server",
            "--tool",
            "get-weather=Look up weather",
            "--output",
            output_path.to_str().expect("utf-8 output path"),
        ])
        .output()
        .expect("run elegy-mcp author with blank correlation id");

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
        correlation_id.starts_with("elegy-mcp-"),
        "unexpected generated correlation id: {correlation_id}"
    );
}
