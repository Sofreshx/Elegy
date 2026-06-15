use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn elegy() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elegy"))
}

fn parse_stdout(output: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be valid json")
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

#[test]
fn docs_init_creates_default_files() {
    let project = unique_temp_dir("elegy-cli-docs-init");
    let output = elegy()
        .args([
            "--json",
            "--project",
            project.to_str().expect("utf-8 project path"),
            "docs",
            "init",
        ])
        .output()
        .expect("run elegy docs init");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = parse_stdout(&output);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["data"]["configPath"], ".elegy/docs.yaml");
    assert!(project.join(".elegy/docs.yaml").is_file());
    assert!(project.join("docs/adr/README.md").is_file());
    assert!(project.join("docs/specs/README.md").is_file());
    assert!(project.join("docs/docs-index.md").is_file());
}

#[test]
fn docs_new_spec_and_check_round_trip() {
    let project = unique_temp_dir("elegy-cli-docs-spec");

    let create = elegy()
        .args([
            "--json",
            "--project",
            project.to_str().expect("utf-8 project path"),
            "docs",
            "new",
            "spec",
            "--title",
            "Docs CLI acceptance",
            "--owner",
            "Elegy",
            "--status",
            "active",
        ])
        .output()
        .expect("run elegy docs new spec");

    assert!(
        create.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&create.stderr)
    );
    let created = parse_stdout(&create);
    assert_eq!(created["status"], "ok");
    assert_eq!(created["data"]["docType"], "spec");
    assert!(project.join("docs/specs/docs-cli-acceptance.md").is_file());

    let check = elegy()
        .args([
            "--json",
            "--project",
            project.to_str().expect("utf-8 project path"),
            "docs",
            "check",
        ])
        .output()
        .expect("run elegy docs check");

    assert!(
        check.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&check.stderr)
    );
    let checked = parse_stdout(&check);
    assert_eq!(checked["status"], "ok");
    assert_eq!(checked["data"]["valid"], true);
    assert_eq!(checked["data"]["docsChecked"], 1);
}

#[test]
fn docs_check_returns_invalid_for_objective_failures() {
    let project = unique_temp_dir("elegy-cli-docs-invalid");
    fs::create_dir_all(project.join("docs/adr")).expect("create adr dir");
    fs::write(
        project.join("docs/adr/bad-name.md"),
        "---\ntitle: Broken ADR\nstatus: draft\ndate: 2026-05-25\nowner: Elegy\n---\n\n# Broken ADR\n\n## Context\n\n- Context.\n\n## Decision\n\n- Decision.\n",
    )
    .expect("write invalid adr");

    let output = elegy()
        .args([
            "--json",
            "--project",
            project.to_str().expect("utf-8 project path"),
            "docs",
            "check",
        ])
        .output()
        .expect("run elegy docs check invalid");

    assert_eq!(output.status.code(), Some(1));
    let body = parse_stdout(&output);
    assert_eq!(body["status"], "invalid");
    assert!(body["data"]["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .any(|issue| issue["code"] == "DOCS-CHECK-001"));
}

#[test]
fn docs_index_writes_index_file() {
    let project = unique_temp_dir("elegy-cli-docs-index");
    let status = elegy()
        .args([
            "--project",
            project.to_str().expect("utf-8 project path"),
            "docs",
            "new",
            "adr",
            "--title",
            "Centralize doc doctrine",
            "--owner",
            "Elegy",
            "--status",
            "accepted",
        ])
        .status()
        .expect("run elegy docs new adr");
    assert!(status.success());

    let output = elegy()
        .args([
            "--json",
            "--project",
            project.to_str().expect("utf-8 project path"),
            "docs",
            "index",
        ])
        .output()
        .expect("run elegy docs index");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body = parse_stdout(&output);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["data"]["adrCount"], 1);
    let index_path = project.join("docs/docs-index.md");
    assert!(index_path.is_file());
    let content = fs::read_to_string(index_path).expect("read docs index");
    assert!(content.contains("Centralize doc doctrine"));
    assert!(content.contains("](adr/"));
}

#[test]
fn docs_check_reports_malformed_frontmatter_in_report() {
    let project = unique_temp_dir("elegy-cli-docs-malformed-frontmatter");
    fs::create_dir_all(project.join("docs/adr")).expect("create adr dir");
    fs::write(
        project.join("docs/adr/2026-05-25-malformed-frontmatter.md"),
        "---\ntitle: Broken ADR\nstatus: accepted\ndate: 2026-05-25\nowner: [Elegy\n---\n\n# Broken ADR\n",
    )
    .expect("write malformed adr");

    let output = elegy()
        .args([
            "--json",
            "--project",
            project.to_str().expect("utf-8 project path"),
            "docs",
            "check",
        ])
        .output()
        .expect("run elegy docs check malformed");

    assert_eq!(output.status.code(), Some(1));
    let body = parse_stdout(&output);
    assert_eq!(body["status"], "invalid");
    assert!(body["data"]["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .any(|issue| issue["code"] == "DOCS-CHECK-009"));
}
