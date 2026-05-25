use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn elegy() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elegy"))
}

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
    let dir = unique_temp_dir("elegy-cli-agent-profile");
    let path = dir.join(name);
    fs::write(&path, body).expect("write profile");
    path
}

fn parse_stdout(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be valid json")
}

#[test]
fn agent_manifest_emits_integration_packet_without_profile() {
    let output = elegy()
        .args(["--json", "agent", "manifest"])
        .output()
        .expect("run agent manifest");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = parse_stdout(&output);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["data"]["integrationVersion"], "elegy.agent/v1");
    assert_eq!(body["data"]["invocation"]["defaultPath"], "cli");
    assert_eq!(body["data"]["profile"]["profileProvided"], false);
    assert!(body["data"]["selected"]["skills"]
        .as_array()
        .expect("skills array")
        .iter()
        .any(|skill| skill["id"] == "memory"));
}

#[test]
fn agent_manifest_profile_reflects_selected_skills_and_capabilities() {
    let profile = write_profile(
        "profile.json",
        r#"{
  "schemaVersion": "agent-capability-profile/v1",
  "profileId": "memory-host",
  "includeSkills": ["memory"],
  "includeCapabilities": ["data-validate"],
  "excludeCapabilities": ["memory-purge"],
  "alwaysIncludeRouter": true
}"#,
    );

    let output = elegy()
        .args([
            "--json",
            "agent",
            "manifest",
            "--profile",
            profile.to_str().expect("utf-8 profile path"),
        ])
        .output()
        .expect("run agent manifest with profile");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = parse_stdout(&output);
    let capabilities = body["data"]["selected"]["capabilities"]
        .as_array()
        .expect("capabilities array");

    assert_eq!(body["data"]["profile"]["profileId"], "memory-host");
    assert!(capabilities.iter().any(|cap| cap["id"] == "memory-add"));
    assert!(capabilities.iter().any(|cap| cap["id"] == "data-validate"));
    assert!(!capabilities.iter().any(|cap| cap["id"] == "memory-purge"));
    assert!(capabilities
        .iter()
        .any(|cap| cap["id"] == "router-skill-search"));
}

#[test]
fn agent_check_reports_malformed_profile_json() {
    let profile = write_profile("bad-profile.json", "{not-json");

    let output = elegy()
        .args([
            "--json",
            "agent",
            "check",
            "--profile",
            profile.to_str().expect("utf-8 profile path"),
        ])
        .output()
        .expect("run agent check with malformed profile");

    assert!(!output.status.success());
    let body = parse_stdout(&output);
    assert_eq!(body["status"], "invalid");
    assert_eq!(body["summary"]["errors"], 1);
    assert_eq!(body["diagnostics"][0]["code"], "CLI-AGENT-002");
}

#[test]
fn agent_check_reports_unknown_profile_ids() {
    let profile = write_profile(
        "unknown-profile.json",
        r#"{
  "schemaVersion": "agent-capability-profile/v1",
  "profileId": "unknown-host",
  "includeSkills": ["missing-skill"],
  "includeCapabilities": ["missing-capability"],
  "alwaysIncludeRouter": true
}"#,
    );

    let output = elegy()
        .args([
            "--json",
            "agent",
            "check",
            "--profile",
            profile.to_str().expect("utf-8 profile path"),
        ])
        .output()
        .expect("run agent check with unknown ids");

    assert!(!output.status.success());
    let body = parse_stdout(&output);
    let codes = body["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .map(|diagnostic| diagnostic["code"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();

    assert!(codes.contains(&"CLI-AGENT-004"));
    assert!(codes.contains(&"CLI-AGENT-005"));
}

#[test]
fn agent_discover_filters_query_results_by_profile() {
    let profile = write_profile(
        "repo-profile.json",
        r#"{
  "schemaVersion": "agent-capability-profile/v1",
  "profileId": "repo-host",
  "includeSkills": ["repo"],
  "alwaysIncludeRouter": true
}"#,
    );

    let output = elegy()
        .args([
            "--json",
            "agent",
            "discover",
            "--query",
            "memory",
            "--profile",
            profile.to_str().expect("utf-8 profile path"),
        ])
        .output()
        .expect("run agent discover with filtered profile");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = parse_stdout(&output);
    let results = body["data"]["results"].as_array().expect("results array");
    assert!(results.iter().all(|result| result["id"] != "memory"));
}

#[test]
fn agent_discover_returns_planning_for_roadmap_queries() {
    let output = elegy()
        .args(["--json", "agent", "discover", "--query", "roadmap planning"])
        .output()
        .expect("run agent discover for planning");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = parse_stdout(&output);
    let results = body["data"]["results"].as_array().expect("results array");
    assert!(!results.is_empty());
    assert_eq!(results[0]["id"], "planning");
}

#[test]
fn agent_discover_detail_includes_only_allowed_capability_implementations() {
    let profile = write_profile(
        "repo-capability-profile.json",
        r#"{
  "schemaVersion": "agent-capability-profile/v1",
  "profileId": "repo-capability-host",
  "includeCapabilities": ["repo-status"],
  "alwaysIncludeRouter": false
}"#,
    );

    let output = elegy()
        .args([
            "--json",
            "agent",
            "discover",
            "--query",
            "repo",
            "--detail",
            "--profile",
            profile.to_str().expect("utf-8 profile path"),
        ])
        .output()
        .expect("run agent discover detail");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = parse_stdout(&output);
    let results = body["data"]["results"].as_array().expect("results array");
    let repo = results
        .iter()
        .find(|result| result["id"] == "repo")
        .expect("repo result");
    let capabilities = repo["capabilities"].as_array().expect("capabilities array");

    assert_eq!(capabilities.len(), 1);
    assert_eq!(capabilities[0]["id"], "repo-status");
    assert_eq!(capabilities[0]["implementation"]["arguments"][0], "repo");

    let profile_arg = profile.to_str().expect("utf-8 profile path");
    assert_eq!(
        repo["expandCommand"],
        format!("elegy agent discover --query repo --detail --json --profile {profile_arg}")
    );
    assert_eq!(
        repo["expandCommandArgs"],
        json!([
            "agent",
            "discover",
            "--query",
            "repo",
            "--detail",
            "--json",
            "--profile",
            profile_arg
        ])
    );
}
