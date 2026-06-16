use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("rust workspace root")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn fixture(name: &str) -> PathBuf {
    repo_root().join("contracts").join("fixtures").join(name)
}

fn write_temp_json(name: &str, value: Value) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("elegy-generator-test-{nonce}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join(name);
    fs::write(
        &path,
        serde_json::to_vec_pretty(&value).expect("serialize temp json"),
    )
    .expect("write temp json");
    path
}

#[test]
fn generator_validate_accepts_minimal_manifest() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "generator",
            "validate",
            fixture("elegy-generator.manifest.minimal.json")
                .to_str()
                .expect("fixture path"),
            "--json",
        ])
        .output()
        .expect("run generator validate");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: Value = serde_json::from_slice(&output.stdout).expect("parse stdout");
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["status"], "success");
    assert_eq!(parsed["data"]["schema"]["status"], "success");
}

#[test]
fn generator_validate_rejects_unknown_top_level_field() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "generator",
            "validate",
            fixture("elegy-generator.contract-meta.unknown-top-level.invalid.json")
                .to_str()
                .expect("fixture path"),
            "--json",
        ])
        .output()
        .expect("run generator validate invalid");

    assert!(!output.status.success());
    let parsed: Value = serde_json::from_slice(&output.stdout).expect("parse stdout");
    assert_eq!(parsed["status"], "invalid");
    assert_eq!(parsed["data"]["schema"]["status"], "failed");
}

#[test]
fn generator_validate_warns_on_future_kind() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "generator",
            "validate",
            fixture("elegy-generator.manifest.future-kind.json")
                .to_str()
                .expect("fixture path"),
            "--json",
        ])
        .output()
        .expect("run generator validate future kind");

    assert!(output.status.success());
    let parsed: Value = serde_json::from_slice(&output.stdout).expect("parse stdout");
    assert_eq!(parsed["data"]["status"], "warning");
    assert_eq!(parsed["data"]["warnings"][0]["code"], "UNKNOWN_KIND");
}

#[test]
fn generator_check_run_reports_unsupported_check_kind() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "generator",
            "check",
            "run",
            fixture("elegy-generator.check.unsupported-kind.json")
                .to_str()
                .expect("fixture path"),
            "--context",
            fixture("elegy-generator.manifest.minimal.json")
                .parent()
                .expect("fixture parent")
                .to_str()
                .expect("fixture parent path"),
            "--json",
        ])
        .output()
        .expect("run generator check");

    assert!(!output.status.success());
    let parsed: Value = serde_json::from_slice(&output.stdout).expect("parse stdout");
    assert_eq!(parsed["status"], "unsupported");
    assert_eq!(parsed["data"]["receipt"]["status"], "unsupported");
    assert_eq!(
        parsed["data"]["warnings"][0]["code"],
        "UNSUPPORTED_CHECK_KIND"
    );
}

#[test]
fn generator_manifest_plan_emits_receipt_and_no_outputs() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "generator",
            "manifest",
            "plan",
            fixture("elegy-generator.manifest.unsupported-backend.json")
                .to_str()
                .expect("fixture path"),
            "--input",
            "name=demo",
            "--json",
        ])
        .output()
        .expect("run generator manifest plan");

    assert!(!output.status.success());
    let parsed: Value = serde_json::from_slice(&output.stdout).expect("parse stdout");
    assert_eq!(parsed["status"], "unsupported");
    assert_eq!(parsed["data"]["receipt"]["status"], "unsupported");
    assert_eq!(parsed["data"]["receipt"]["outputs"], json!([]));
    assert_eq!(parsed["data"]["warnings"][0]["code"], "UNSUPPORTED_BACKEND");
}

#[test]
fn generator_check_run_fails_on_target_schema_version_mismatch() {
    let check = write_temp_json(
        "elegy-generator.check.target-mismatch.json",
        json!({
            "schemaVersion": "elegy-generator.check/v0",
            "id": "elegy.generator.test.target-mismatch",
            "kind": "check",
            "version": "0.1.0",
            "checkKind": "schema",
            "target": {
                "path": fixture("elegy-generator.manifest.minimal.json"),
                "schemaVersion": "elegy-generator.registry/v0"
            },
            "extensions": {}
        }),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "generator",
            "check",
            "run",
            check.to_str().expect("check path"),
            "--context",
            fixture("elegy-generator.manifest.minimal.json")
                .parent()
                .expect("fixture parent")
                .to_str()
                .expect("fixture parent path"),
            "--json",
        ])
        .output()
        .expect("run generator check target mismatch");

    assert!(!output.status.success());
    let parsed: Value = serde_json::from_slice(&output.stdout).expect("parse stdout");
    assert_eq!(parsed["status"], "invalid");
    assert_eq!(parsed["data"]["status"], "failed");
    assert_eq!(
        parsed["data"]["errors"][0]["code"],
        "TARGET_SCHEMA_VERSION_MISMATCH"
    );
}

#[test]
fn generator_validate_accepts_non_object_backend_config() {
    let manifest = write_temp_json(
        "elegy-generator.manifest.scalar-backend-config.json",
        json!({
            "schemaVersion": "elegy-generator.manifest/v0",
            "id": "elegy.generator.test.scalar-backend-config",
            "kind": "solved_unit",
            "version": "0.1.0",
            "backend": {
                "kind": "template",
                "config": "template-v0"
            },
            "extensions": {}
        }),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "generator",
            "validate",
            manifest.to_str().expect("manifest path"),
            "--json",
        ])
        .output()
        .expect("run generator validate scalar backend config");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: Value = serde_json::from_slice(&output.stdout).expect("parse stdout");
    assert_eq!(parsed["data"]["schema"]["status"], "success");
    assert_eq!(parsed["data"]["warnings"][0]["code"], "UNSUPPORTED_BACKEND");
}

#[test]
fn generator_registry_resolves_manifest_by_id() {
    let root = repo_root().join("contracts").join("fixtures");
    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "generator",
            "registry",
            "resolve",
            "elegy.generator.example.create-doc",
            root.to_str().expect("fixtures path"),
            "--json",
        ])
        .output()
        .expect("run generator registry resolve");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: Value = serde_json::from_slice(&output.stdout).expect("parse stdout");
    assert_eq!(parsed["status"], "ok");
    assert_eq!(
        parsed["data"]["contract"]["id"],
        "elegy.generator.example.create-doc"
    );
}
