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
fn direct_skills_binary_generates_expected_skill_artifacts() {
    let temp_dir = unique_temp_dir("elegy-skills-cli");
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

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args([
            "--format",
            "json",
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

    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("generate stdout should be valid json");
    assert_eq!(stdout["status"], "ok");
    assert_eq!(stdout["data"]["generated_skills"].as_array().map(Vec::len), Some(1));
    assert_eq!(stdout["data"]["skipped_tools"].as_array().map(Vec::len), Some(1));

    let generated_skill_path = output_dir.join("mcp-weather-server-get-weather.json");
    assert!(generated_skill_path.is_file());

    let generated_skill = fs::read_to_string(&generated_skill_path)
        .expect("read generated skill definition");
    assert!(generated_skill.contains("mcp-weather-server-get-weather"));
    assert!(generated_skill.contains("get-weather"));

    fs::remove_dir_all(temp_dir).expect("cleanup temp directory");
}