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