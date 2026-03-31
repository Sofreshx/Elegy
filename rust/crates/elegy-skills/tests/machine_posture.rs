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
fn generate_command_supports_machine_flags_and_correlation_id() {
    let temp_dir = unique_temp_dir("elegy-skills-machine");
    let descriptor_path = temp_dir.join("descriptor.json");
    let output_dir = temp_dir.join("skills");

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
    }
  ]
}
"#,
    )
    .expect("write descriptor fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-skills-1",
            "generate",
            "--descriptor",
            descriptor_path.to_str().expect("utf-8 descriptor path"),
            "--output-dir",
            output_dir.to_str().expect("utf-8 output dir"),
        ])
        .output()
        .expect("run elegy-skills generate");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"ok\""));
    assert!(stdout.contains("\"correlationId\": \"corr-skills-1\""));
    assert!(stdout.contains("\"nonInteractive\": true"));
    assert!(stdout.contains("mcp-weather-server-get-weather"));
}

#[test]
fn generate_command_emits_structured_error_with_machine_flags() {
    let temp_dir = unique_temp_dir("elegy-skills-machine-error");
    let missing_path = temp_dir.join("missing.json");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-skills-err-1",
            "generate",
            "--descriptor",
            missing_path.to_str().expect("utf-8 descriptor path"),
        ])
        .output()
        .expect("run elegy-skills generate missing descriptor");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"error\""));
    assert!(stdout.contains("\"correlationId\": \"corr-skills-err-1\""));
    assert!(stdout.contains("\"nonInteractive\": true"));
}
