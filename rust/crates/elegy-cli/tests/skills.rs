use std::process::Command;

fn elegy() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elegy"))
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

    let body: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("skills list should emit JSON");
    let skills = body["data"]["skills"]
        .as_array()
        .expect("skills should be an array");

    assert!(skills.len() >= 12);
    assert!(skills.iter().any(|skill| skill["id"] == "memory"));
    assert!(skills.iter().any(|skill| skill["id"] == "mermaid"));
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

    let body: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("skills describe should emit JSON");
    assert_eq!(body["data"]["skillFormat"], "elegy-skill-definition");
    assert_eq!(body["data"]["skillVersion"], 2);
    assert_eq!(body["data"]["identity"]["name"], "memory");
}
