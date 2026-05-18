use std::path::PathBuf;
use std::process::Command;

fn elegy() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elegy"))
}

fn parse_stdout(output: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be valid json")
}

fn governed_skill_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/fixtures")
        .join(name)
}

#[test]
fn skills_list_uses_builtin_v2_registry() {
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

    assert!(skills.len() >= 13);
    assert!(skills.iter().any(|skill| skill["id"] == "memory"));
    assert!(skills.iter().any(|skill| skill["id"] == "mermaid"));
    assert!(skills.iter().any(|skill| skill["id"] == "planning"));
    assert!(skills.iter().all(|skill| skill["capabilitiesCount"]
        .as_u64()
        .is_some_and(|count| count > 0)));
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
    assert_eq!(body["data"]["skillFormat"], "elegy-skill-definition");
    assert_eq!(body["data"]["skillVersion"], 2);
    assert_eq!(body["data"]["identity"]["name"], "memory");
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
    assert_eq!(body["data"]["topSkill"]["id"], "repo");
    assert!(body["data"]["results"]
        .as_array()
        .expect("results array")
        .first()
        .is_some());
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
    assert_eq!(body["data"]["topSkill"]["id"], "planning");
    assert_eq!(body["data"]["results"][0]["id"], "planning");
}

#[test]
fn skills_capability_returns_projected_capability_card() {
    let output = elegy()
        .args([
            "--json",
            "skills",
            "capability",
            "--capability-id",
            "repo-status",
        ])
        .output()
        .expect("run elegy skills capability");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = parse_stdout(&output);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["data"]["skillId"], "repo");
    assert_eq!(body["data"]["capabilityId"], "repo-status");
    assert_eq!(body["data"]["capabilityDefinition"]["id"], "repo-status");
}

#[test]
fn skills_validate_accepts_governed_skill_fixture() {
    let fixture = governed_skill_fixture("skill-definition-v2.elegy-planning.json");
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
