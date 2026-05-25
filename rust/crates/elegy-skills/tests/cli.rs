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

fn write_profile(name: &str, body: &str) -> PathBuf {
    let dir = unique_temp_dir("elegy-skills-profile");
    let path = dir.join(name);
    fs::write(&path, body).expect("write profile");
    path
}

#[test]
fn direct_skills_binary_lists_builtin_registry() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args(["--format", "json", "list"])
        .output()
        .expect("run elegy-skills list");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("list stdout should be valid json");
    assert_eq!(stdout["status"], "ok");
    let skills = stdout["data"]["skills"].as_array().expect("skills array");
    assert!(skills.iter().any(|skill| skill["id"] == "repo"));
    assert!(skills.iter().any(|skill| skill["id"] == "skills"));
}

#[test]
fn direct_skills_binary_resolves_repo_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args(["--format", "json", "resolve", "--query", "repo status"])
        .output()
        .expect("run elegy-skills resolve");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("resolve stdout should be valid json");
    assert_eq!(stdout["status"], "ok");
    assert_eq!(stdout["data"]["topSkill"]["id"], "repo");
    assert!(!stdout["data"]["results"]
        .as_array()
        .expect("results array")
        .is_empty());
}

#[test]
fn profile_search_detail_excludes_removed_capabilities() {
    let profile = write_profile(
        "repo-no-diff.json",
        r#"{
  "schemaVersion": "agent-capability-profile/v1",
  "profileId": "repo-no-diff",
  "includeSkills": ["repo"],
  "excludeCapabilities": ["repo-diff"],
  "alwaysIncludeRouter": false
}"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args([
            "--format",
            "json",
            "--profile",
            profile.to_str().expect("utf-8 profile path"),
            "search",
            "--query",
            "repo diff",
            "--detail",
        ])
        .output()
        .expect("run elegy-skills profile search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("search stdout should be valid json");
    let results = stdout["data"]["results"].as_array().expect("results array");
    let repo = results
        .iter()
        .find(|result| result["id"] == "repo")
        .expect("repo result");
    assert!(!repo["capabilities"]
        .as_array()
        .expect("capabilities array")
        .iter()
        .any(|capability| capability["id"] == "repo-diff"));
    assert!(repo["matchResult"]["matchedCapabilities"]
        .as_array()
        .is_none_or(|matched| !matched.iter().any(|capability| capability == "repo-diff")));
}

#[test]
fn profile_resolve_detail_excludes_removed_capabilities() {
    let profile = write_profile(
        "repo-no-diff.json",
        r#"{
  "schemaVersion": "agent-capability-profile/v1",
  "profileId": "repo-no-diff",
  "includeSkills": ["repo"],
  "excludeCapabilities": ["repo-diff"],
  "alwaysIncludeRouter": false
}"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args([
            "--format",
            "json",
            "--profile",
            profile.to_str().expect("utf-8 profile path"),
            "resolve",
            "--query",
            "repo diff",
            "--detail",
        ])
        .output()
        .expect("run elegy-skills profile resolve");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("resolve stdout should be valid json");
    assert_eq!(stdout["data"]["topSkill"]["id"], "repo");
    assert_eq!(stdout["data"]["topCapability"], serde_json::Value::Null);
    let results = stdout["data"]["results"].as_array().expect("results array");
    let repo = results.first().expect("top result");
    assert!(!repo["capabilities"]
        .as_array()
        .expect("capabilities array")
        .iter()
        .any(|capability| capability["id"] == "repo-diff"));
    assert!(repo["matchResult"]["matchedCapabilities"]
        .as_array()
        .is_none_or(|matched| !matched.iter().any(|capability| capability == "repo-diff")));
}

#[test]
fn profile_get_filters_excluded_capabilities() {
    let profile = write_profile(
        "repo-no-diff.json",
        r#"{
  "schemaVersion": "agent-capability-profile/v1",
  "profileId": "repo-no-diff",
  "includeSkills": ["repo"],
  "excludeCapabilities": ["repo-diff"],
  "alwaysIncludeRouter": false
}"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args([
            "--format",
            "json",
            "--profile",
            profile.to_str().expect("utf-8 profile path"),
            "get",
            "--skill-id",
            "repo",
        ])
        .output()
        .expect("run elegy-skills profile get");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("get stdout should be valid json");
    let capabilities = stdout["data"]["capabilities"]
        .as_array()
        .expect("capabilities array");
    assert!(!capabilities.is_empty());
    assert!(!capabilities
        .iter()
        .any(|capability| capability["id"] == "repo-diff"));
}

#[test]
fn invalid_profile_is_rejected() {
    let profile = write_profile(
        "bad-profile.json",
        r#"{
  "schemaVersion": "agent-capability-profile/v1",
  "profileId": "bad-profile",
  "includeSkills": ["not-a-skill"],
  "alwaysIncludeRouter": false
}"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args([
            "--format",
            "json",
            "--profile",
            profile.to_str().expect("utf-8 profile path"),
            "list",
        ])
        .output()
        .expect("run elegy-skills invalid profile list");

    assert!(!output.status.success());

    let stdout: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("list stdout should be valid json");
    assert_eq!(stdout["status"], "invalid");
    assert!(stdout["error"]
        .as_str()
        .expect("error string")
        .contains("unknown skill 'not-a-skill'"));
    assert_eq!(stdout["data"]["profileProvided"], true);
}
