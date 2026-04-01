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
fn direct_mcp_binary_authors_and_analyzes_descriptor() {
    let temp_dir = unique_temp_dir("elegy-mcp-cli");
    let descriptor_path = temp_dir.join("weather-mcp.json");

    let author = Command::new(env!("CARGO_BIN_EXE_elegy-mcp"))
        .args([
            "--format",
            "json",
            "author",
            "--server-name",
            "weather-server",
            "--tool",
            "get-weather=Look up a weather report",
            "--tool",
            "list-alerts",
            "--output",
            descriptor_path.to_str().expect("utf-8 descriptor path"),
        ])
        .output()
        .expect("run elegy-mcp author");

    assert!(
        author.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&author.stderr)
    );
    assert!(descriptor_path.is_file());

    let author_stdout: serde_json::Value = serde_json::from_slice(&author.stdout)
        .expect("author stdout should be valid json");
    assert_eq!(author_stdout["status"], "ok");

    let descriptor = fs::read_to_string(&descriptor_path).expect("read authored descriptor");
    assert!(descriptor.contains("weather-server"));
    assert!(descriptor.contains("get-weather"));

    let analyze = Command::new(env!("CARGO_BIN_EXE_elegy-mcp"))
        .args([
            "--format",
            "json",
            "analyze",
            "--descriptor",
            descriptor_path.to_str().expect("utf-8 descriptor path"),
        ])
        .output()
        .expect("run elegy-mcp analyze");

    assert!(
        analyze.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&analyze.stderr)
    );

    let analyze_stdout = String::from_utf8(analyze.stdout).expect("stdout should be utf-8");
    assert!(analyze_stdout.contains("\"status\": \"ok\""));
    assert!(analyze_stdout.contains("weather-server"));
    assert!(analyze_stdout.contains("get-weather"));
    assert!(analyze_stdout.contains("list-alerts"));

    fs::remove_dir_all(temp_dir).expect("cleanup temp directory");
}