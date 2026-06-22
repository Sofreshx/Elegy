use serde_json::{json, Value};
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

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .and_then(|path| path.parent())
        .expect("repo root")
        .to_path_buf()
}

#[test]
fn dedicated_configuration_cli_supports_package_profiles() {
    let temp_dir = unique_temp_dir("elegy-configuration-cli-package-apply");
    let target_dir = temp_dir.join("target");
    let package_path = repo_root()
        .join("contracts")
        .join("fixtures")
        .join("elegy-plugin-package.demo-config.json");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-configuration"))
        .args([
            "apply",
            "--package",
            package_path.to_str().expect("utf-8 package path"),
            "--profile-id",
            "demo-profile",
            "--target",
            target_dir.to_str().expect("utf-8 target path"),
            "--json",
        ])
        .output()
        .expect("run elegy-configuration apply --package");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["command"], json!(["apply"]));
    assert_eq!(body["data"]["sourceKind"], "package");
    assert_eq!(body["data"]["subjectKind"], "profile");
    assert_eq!(body["data"]["subjectId"], "demo-profile");
    assert_eq!(body["data"]["verified"], true);
    assert_eq!(body["data"]["summary"]["created"], 1);
    assert!(body["data"]["sourceRef"]
        .as_str()
        .expect("sourceRef string")
        .contains("#demo-profile"));
    let generated =
        fs::read_to_string(target_dir.join("generated").join("demo.txt")).expect("generated file");
    assert_eq!(generated.trim_end_matches(['\r', '\n']), "demo");
}

#[test]
fn dedicated_configuration_cli_verifies_package_profiles() {
    let temp_dir = unique_temp_dir("elegy-configuration-cli-package-verify");
    let target_dir = temp_dir.join("target");
    let package_path = repo_root()
        .join("contracts")
        .join("fixtures")
        .join("elegy-plugin-package.demo-config.json");

    let apply_output = Command::new(env!("CARGO_BIN_EXE_elegy-configuration"))
        .args([
            "apply",
            "--package",
            package_path.to_str().expect("utf-8 package path"),
            "--profile-id",
            "demo-profile",
            "--target",
            target_dir.to_str().expect("utf-8 target path"),
            "--json",
        ])
        .output()
        .expect("run elegy-configuration apply before verify");
    assert!(
        apply_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&apply_output.stderr)
    );

    let verify_output = Command::new(env!("CARGO_BIN_EXE_elegy-configuration"))
        .args([
            "verify",
            "--package",
            package_path.to_str().expect("utf-8 package path"),
            "--profile-id",
            "demo-profile",
            "--target",
            target_dir.to_str().expect("utf-8 target path"),
            "--json",
        ])
        .output()
        .expect("run elegy-configuration verify --package");

    assert!(
        verify_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&verify_output.stderr)
    );

    let body: Value =
        serde_json::from_slice(&verify_output.stdout).expect("stdout should be valid json");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["command"], json!(["verify"]));
    assert_eq!(body["data"]["sourceKind"], "package");
    assert_eq!(body["data"]["subjectKind"], "profile");
    assert_eq!(body["data"]["subjectId"], "demo-profile");
    assert_eq!(body["data"]["verified"], true);
    assert_eq!(body["data"]["summary"]["verified"], 1);
    assert_eq!(body["data"]["summary"]["mismatched"], 0);
}

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
