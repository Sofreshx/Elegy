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
fn author_mcp_command_writes_descriptor_file() {
    let temp_dir = unique_temp_dir("elegy-cli-author");
    let output_path = temp_dir.join("weather-mcp.json");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "author",
            "mcp",
            "--server-name",
            "weather-server",
            "--tool",
            "get-weather=Look up a weather report",
            "--tool",
            "list-alerts",
            "--output",
            output_path.to_str().expect("utf-8 output path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy author mcp");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_path.is_file());

    let descriptor = fs::read_to_string(&output_path).expect("read authored descriptor");
    assert!(descriptor.contains("weather-server"));
    assert!(descriptor.contains("get-weather"));

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"ok\""));
    assert!(stdout.contains("\"output_path\""));
}

#[test]
fn analyze_and_generate_commands_use_same_descriptor_input() {
    let temp_dir = unique_temp_dir("elegy-cli-generate");
    let descriptor_path = temp_dir.join("weather-mcp.json");
    let output_dir = temp_dir.join("generated-skills");

    fs::write(
        &descriptor_path,
        r#"{
  "serverName": "weather-server",
  "transport": "stdio",
  "tools": [
    {
      "name": "get-weather",
      "description": "Look up a weather report",
      "inputSchema": { "type": "object" }
    },
    {
      "name": "list-alerts",
      "description": "List active weather alerts"
    }
  ]
}
"#,
    )
    .expect("write descriptor fixture");

    let analysis = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "analyze",
            "mcp",
            "--descriptor",
            descriptor_path.to_str().expect("utf-8 descriptor path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy analyze mcp");

    assert!(
        analysis.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&analysis.stderr)
    );
    let analysis_stdout = String::from_utf8(analysis.stdout).expect("stdout should be utf-8");
    assert!(analysis_stdout.contains("weather-server"));
    assert!(analysis_stdout.contains("get-weather"));

    let generation = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "generate",
            "skills",
            "--descriptor",
            descriptor_path.to_str().expect("utf-8 descriptor path"),
            "--output-dir",
            output_dir.to_str().expect("utf-8 output dir"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy generate skills");

    assert!(
        generation.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&generation.stderr)
    );
    assert!(output_dir.join("mcp-weather-server-get-weather.json").is_file());

    let generation_stdout =
        String::from_utf8(generation.stdout).expect("stdout should be utf-8");
    assert!(generation_stdout.contains("mcp-weather-server-get-weather"));
    assert!(generation_stdout.contains("list-alerts"));
}

#[test]
fn validate_session_context_command_reports_bounded_json_result() {
    let temp_dir = unique_temp_dir("elegy-cli-session-context-json");
    let input_path = temp_dir.join("session-context.json");

    fs::write(
        &input_path,
        r#"{
  "artifactKind": "summary-only-session-context-envelope",
  "requestId": "request-1",
  "runId": "run-1",
  "capturedAtUtc": "2026-03-22T00:00:00Z",
  "sessionContext": {
    "scope": "workspace",
    "representation": "summary-only",
    "summary": "Workspace context persists only bounded summaries for instruction assembly and follow-on agent runs.",
    "salientFacts": [
      "Persist summary and context artifacts only.",
      "Raw execution logs remain transient and are not stored durably."
    ],
    "instructionContext": [
      "Use this summary context when assembling workspace-level instructions."
    ],
    "rawTranscriptPersisted": false
  }
}
"#,
    )
    .expect("write session context fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "validate",
            "session-context",
            "--input",
            input_path.to_str().expect("utf-8 input path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy validate session-context");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"ok\""));
    assert!(stdout.contains("\"artifactKind\": \"summary-only-session-context-envelope\""));
    assert!(stdout.contains("\"scope\": \"workspace\""));
    assert!(stdout.contains("\"readOnly\": true"));
    assert!(stdout.contains("\"hostValidationOwner\": \"SAASTools\""));
}

#[test]
fn validate_session_context_command_reports_bounded_text_result() {
    let temp_dir = unique_temp_dir("elegy-cli-session-context-text");
    let input_path = temp_dir.join("session-context.json");

    fs::write(
        &input_path,
        r#"{
  "artifactKind": "summary-only-session-context-envelope",
  "sessionContext": {
    "scope": "session",
    "representation": "summary-only",
    "summary": "Short handoff summary.",
    "rawTranscriptPersisted": false
  }
}
"#,
    )
    .expect("write session context fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "validate",
            "session-context",
            "--input",
            input_path.to_str().expect("utf-8 input path"),
        ])
        .output()
        .expect("run elegy validate session-context text mode");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("summary-only session context artifact is valid"));
    assert!(stdout.contains("scope: session"));
    assert!(stdout.contains("read only: true"));
    assert!(stdout.contains("host validation owner: SAASTools"));
}

#[test]
fn validate_session_context_command_rejects_invalid_artifact() {
    let temp_dir = unique_temp_dir("elegy-cli-session-context-invalid");
    let input_path = temp_dir.join("invalid-session-context.json");

    fs::write(
        &input_path,
        r#"{
  "artifactKind": "summary-only-session-context-envelope",
  "sessionContext": {
    "scope": "workspace",
    "representation": "summary-only",
    "summary": "Portable summary only.",
    "rawTranscriptPersisted": true
  }
}
"#,
    )
    .expect("write invalid session context fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "validate",
            "session-context",
            "--input",
            input_path.to_str().expect("utf-8 input path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy validate session-context invalid artifact");

    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("CLI-MEMORY-002"));
    assert!(stdout.contains("rawTranscriptPersisted must be false"));
}