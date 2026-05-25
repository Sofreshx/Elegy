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
fn invalid_profile_emits_structured_error_with_machine_flags() {
    let temp_dir = unique_temp_dir("elegy-skills-machine-profile-error");
    let profile_path = temp_dir.join("bad-profile.json");
    fs::write(
        &profile_path,
        r#"{
  "schemaVersion": "agent-capability-profile/v1",
  "profileId": "bad-profile",
  "includeSkills": ["not-a-skill"],
  "alwaysIncludeRouter": false
}"#,
    )
    .expect("write bad profile");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-skills-profile-err-1",
            "--profile",
            profile_path.to_str().expect("utf-8 profile path"),
            "list",
        ])
        .output()
        .expect("run elegy-skills list with invalid profile");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("\"correlationId\": \"corr-skills-profile-err-1\""));
    assert!(stdout.contains("\"nonInteractive\": true"));
    assert!(stdout.contains("unknown skill 'not-a-skill'"));
}
