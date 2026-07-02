use std::process::Command;

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
    assert!(skills.iter().any(|skill| skill["id"] == "elegy-planning"));
    assert!(skills.iter().any(|skill| skill["id"] == "elegy-skills"));
    assert!(skills
        .iter()
        .any(|skill| skill["id"] == "elegy-documentation"));
}

#[test]
fn direct_skills_binary_resolves_planning() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args(["--format", "json", "resolve", "--query", "plan"])
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
    assert_eq!(stdout["data"]["topSkill"]["id"], "elegy-planning");
    assert!(!stdout["data"]["results"]
        .as_array()
        .expect("results array")
        .is_empty());
}

#[test]
fn direct_skills_binary_resolves_agent_readable_docs() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-skills"))
        .args(["--format", "json", "resolve", "--query", "documentation"])
        .output()
        .expect("run elegy-skills resolve for documentation");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("resolve stdout should be valid json");
    assert_eq!(stdout["status"], "ok");
    assert_eq!(stdout["data"]["topSkill"]["id"], "elegy-documentation");
}
