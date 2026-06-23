use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn elegy() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elegy"))
}

fn parse_stdout(output: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be valid json")
}

fn governed_skill_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../skills")
        .join(name)
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}-{unique}"));
    std::fs::create_dir_all(&dir).expect("create temp directory");
    dir
}

#[test]
fn skills_list_uses_builtin_skill_registry() {
    let output = elegy()
        .args(["--json", "skills", "list"])
        .output()
        .expect("run elegy skills list");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = parse_stdout(&output);
    let skills = body["data"]["skills"]
        .as_array()
        .expect("skills should be an array");

    assert!(skills.len() >= 14);
    assert!(skills
        .iter()
        .any(|skill| skill["id"] == "elegy-documentation"));
    assert!(skills.iter().any(|skill| skill["id"] == "elegy-memory"));
    assert!(skills.iter().any(|skill| skill["id"] == "elegy-mermaid"));
    assert!(skills.iter().any(|skill| skill["id"] == "elegy-planning"));
    assert!(skills.iter().all(|skill| skill["lifecycleState"]
        .as_str()
        .is_some_and(|state| !state.is_empty())));
}

#[test]
fn skills_describe_accepts_aliases() {
    let output = elegy()
        .args(["--json", "skills", "describe", "--skill-id", "elegy-memory"])
        .output()
        .expect("run elegy skills describe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = parse_stdout(&output);
    assert_eq!(body["data"]["id"], "elegy-memory");
    assert_eq!(body["data"]["name"], "elegy-memory");
    assert!(body["data"]["description"]
        .as_str()
        .is_some_and(|d| !d.is_empty()));
}

#[test]
fn skills_resolve_returns_registry_match_data() {
    let output = elegy()
        .args(["--json", "skills", "resolve", "--query", "repo status"])
        .output()
        .expect("run elegy skills resolve");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = parse_stdout(&output);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["data"]["query"], "repo status");
    assert_eq!(body["data"]["topSkill"]["id"], "elegy-repo");
    assert!(!body["data"]["results"]
        .as_array()
        .expect("results array")
        .is_empty());
}

#[test]
fn skills_resolve_returns_planning_for_roadmap_queries() {
    let output = elegy()
        .args(["--json", "skills", "resolve", "--query", "roadmap planning"])
        .output()
        .expect("run elegy skills resolve for planning");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = parse_stdout(&output);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["data"]["topSkill"]["id"], "elegy-planning");
    assert_eq!(body["data"]["results"][0]["id"], "elegy-planning");
}

#[test]
fn skills_resolve_returns_documentation_for_agent_readable_docs() {
    let output = elegy()
        .args([
            "--json",
            "skills",
            "resolve",
            "--query",
            "agent readable docs",
        ])
        .output()
        .expect("run elegy skills resolve for documentation");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = parse_stdout(&output);
    assert_eq!(body["status"], "ok");
    assert!(body["data"]["topSkill"]["id"]
        .as_str()
        .is_some_and(|id| id.starts_with("elegy-")));
    assert!(!body["data"]["results"]
        .as_array()
        .expect("results array")
        .is_empty());
}

#[test]
fn skills_validate_accepts_governed_skill_fixture() {
    let fixture = governed_skill_dir("elegy-documentation").join("SKILL.md");
    let output = elegy()
        .args([
            "--json",
            "skills",
            "validate",
            "--file",
            fixture.to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("run elegy skills validate");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = parse_stdout(&output);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["data"]["valid"], true);
    assert!(body["data"]["issues"]
        .as_array()
        .expect("issues array")
        .is_empty());
}

#[test]
fn skills_validate_reports_invalid_fixture_with_summary_and_diagnostics() {
    let temp_dir = unique_temp_dir("elegy-cli-skills-invalid-skill");
    let bad_file = temp_dir.join("invalid-skill.md");
    std::fs::write(
        &bad_file,
        "---\nname: \"\"\ndescription: \"\"\nversion: \"1.0\"\n---\n",
    )
    .expect("write invalid skill fixture");

    let output = elegy()
        .args([
            "--json",
            "skills",
            "validate",
            "--file",
            bad_file.to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("run elegy skills validate invalid fixture");

    assert!(!output.status.success());
    let body = parse_stdout(&output);
    assert_eq!(body["status"], "invalid");
    assert_eq!(body["data"]["valid"], false);
    assert!(body["summary"]["errors"]
        .as_u64()
        .is_some_and(|count| count >= 1));
    assert!(body["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .any(|diagnostic| diagnostic["message"]
            .as_str()
            .is_some_and(|message| message.contains("Skill name must not be empty")
                || message.contains("Skill description must not be empty"))));
}

#[test]
fn skills_validate_reports_malformed_json_as_invalid() {
    let temp_dir = unique_temp_dir("elegy-cli-skills-invalid-json");
    let bad_file = temp_dir.join("bad.json");
    std::fs::write(&bad_file, "{not-json").expect("write malformed fixture");

    let output = elegy()
        .args([
            "--json",
            "skills",
            "validate",
            "--file",
            bad_file.to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("run elegy skills validate malformed json");

    assert!(!output.status.success());
    let body = parse_stdout(&output);
    assert_eq!(body["status"], "invalid");
    assert_eq!(body["data"]["valid"], false);
    assert!(body["summary"]["errors"]
        .as_u64()
        .is_some_and(|count| count >= 1));
    assert!(body["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .any(|diagnostic| diagnostic["message"]
            .as_str()
            .is_some_and(|message| message.contains("must start with a '---' frontmatter fence"))));
}
