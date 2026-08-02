use elegy_plugin_sdk::capability_package::{
    ElegyPackageEntrypointKind, ElegyPackageEntrypointV1, ElegyPackageFileV1,
    ElegyPackagePublisherV1, ElegyPackageV1, ELEGY_PACKAGE_V1_SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::Command;

fn write_package(root: &Path) {
    fs::create_dir_all(root.join("bin")).expect("create bin");
    fs::write(root.join("bin/demo-tool"), b"demo tool").expect("write executable");
    fs::write(
        root.join("capability-catalog.json"),
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": "elegy-capability-catalog/v2",
            "plugin": "demo-tool",
            "pluginVersion": "0.1.0",
            "capabilities": [{
                "kind": "cli",
                "id": "demo-tool.example",
                "description": "Demo capability",
                "contractVersion": "v1",
                "sideEffectClass": "query",
                "readiness": "usable",
                "invocation": {
                    "executable": "./bin/demo-tool",
                    "command": ["--json"],
                    "inputSchema": {"type": "object"},
                    "outputSchema": {"type": "object"}
                }
            }]
        }))
        .expect("serialize catalog"),
    )
    .expect("write catalog");
    fs::write(
        root.join("readiness.json"),
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": "elegy-readiness/v1",
            "surface": "demo-tool",
            "surfaceVersion": "0.1.0",
            "stage": "usable",
            "summary": "Fixture package readiness.",
            "worksToday": ["The fixture is packaged."],
            "limitations": ["The fixture is test-only."],
            "supportedEnvironments": ["any"],
            "installation": "Install the package archive.",
            "invocation": "Invoke the JSON CLI.",
            "evidence": [
                {"kind": "source-tests", "path": "./readiness.json", "summary": "Fixture tests pass."},
                {"kind": "package-verification", "path": "./readiness.json", "summary": "Package files verify."},
                {"kind": "clean-install", "path": "./readiness.json", "summary": "Fixture installs cleanly."},
                {"kind": "real-task", "path": "./readiness.json", "summary": "Fixture completes a real task.", "nonFixture": true}
            ]
        }))
        .expect("serialize readiness"),
    )
    .expect("write readiness");
    let mut package = ElegyPackageV1 {
        schema_version: ELEGY_PACKAGE_V1_SCHEMA_VERSION.to_string(),
        name: "demo-tool".to_string(),
        version: "0.1.0".to_string(),
        description: "Demo capability".to_string(),
        publisher: ElegyPackagePublisherV1 {
            name: "Test Publisher".to_string(),
            repository: "https://github.com/example/demo-tool".to_string(),
            workflow_identity: None,
        },
        license: "Apache-2.0".to_string(),
        targets: vec!["any".to_string()],
        capability_catalog: "./capability-catalog.json".to_string(),
        readiness: Some("./readiness.json".to_string()),
        entrypoints: vec![ElegyPackageEntrypointV1 {
            id: "demo".to_string(),
            kind: ElegyPackageEntrypointKind::Cli,
            executable: "./bin/demo-tool".to_string(),
            command: vec!["--json".to_string()],
        }],
        files: vec![
            ElegyPackageFileV1 {
                path: "./bin/demo-tool".to_string(),
                role: "executable".to_string(),
                sha256: None,
            },
            ElegyPackageFileV1 {
                path: "./capability-catalog.json".to_string(),
                role: "capability-catalog".to_string(),
                sha256: None,
            },
            ElegyPackageFileV1 {
                path: "./readiness.json".to_string(),
                role: "readiness".to_string(),
                sha256: None,
            },
        ],
        skills: Vec::new(),
        provenance: None,
    };
    for file in &mut package.files {
        let bytes = fs::read(root.join(file.path.trim_start_matches("./")))
            .expect("read package file for digest");
        file.sha256 = Some(format!("{:x}", Sha256::digest(bytes)));
    }
    fs::write(
        root.join("elegy-package.json"),
        serde_json::to_vec_pretty(&package).expect("serialize package"),
    )
    .expect("write package");
}

#[test]
fn init_creates_a_host_neutral_package_scaffold() {
    let temp = tempfile::tempdir().expect("temporary directory");
    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "init",
            "--name",
            "demo-tool",
            "--output",
            temp.path().to_str().expect("utf8 path"),
        ])
        .output()
        .expect("run elegy init");

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(temp.path().join("elegy-package.json").is_file());
    assert!(temp.path().join("capability-catalog.json").is_file());
    assert!(temp.path().join("readiness.json").is_file());

    fs::write(temp.path().join("bin/demo-tool"), b"placeholder executable")
        .expect("write placeholder executable");
    let check = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "check",
            "--package",
            temp.path().to_str().expect("utf8 path"),
        ])
        .output()
        .expect("run elegy check on scaffold");
    assert!(check.status.success(), "stderr: {:?}", check.stderr);
}

#[test]
fn check_and_pack_accept_a_complete_host_neutral_package() {
    let temp = tempfile::tempdir().expect("temporary directory");
    write_package(temp.path());
    let archive = temp.path().join("demo-tool.zip");

    let check = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "check",
            "--package",
            temp.path().to_str().expect("utf8 path"),
        ])
        .output()
        .expect("run elegy check");
    assert!(check.status.success(), "stderr: {:?}", check.stderr);

    let pack = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "pack",
            "--package",
            temp.path().to_str().expect("utf8 path"),
            "--output",
            archive.to_str().expect("utf8 archive path"),
        ])
        .output()
        .expect("run elegy pack");
    assert!(pack.status.success(), "stderr: {:?}", pack.stderr);
    assert!(archive.is_file());
    assert!(temp.path().join("demo-tool.zip.sbom.json").is_file());
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(temp.path().join("elegy-package.json")).expect("read packed manifest"),
    )
    .expect("parse packed manifest");
    assert!(manifest["files"][0]["sha256"].as_str().is_some());
}

#[test]
fn pack_refreshes_file_digests_after_declared_file_changes() {
    let temp = tempfile::tempdir().expect("temporary directory");
    write_package(temp.path());
    let archive = temp.path().join("demo-tool.zip");

    let first_pack = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "pack",
            "--package",
            temp.path().to_str().expect("utf8 package path"),
            "--output",
            archive.to_str().expect("utf8 archive path"),
        ])
        .output()
        .expect("run first elegy pack");
    assert!(
        first_pack.status.success(),
        "stderr: {:?}",
        first_pack.stderr
    );
    let first_manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(temp.path().join("elegy-package.json")).expect("read first manifest"),
    )
    .expect("parse first manifest");
    let first_digest = first_manifest["files"][0]["sha256"]
        .as_str()
        .expect("first file digest")
        .to_string();

    fs::write(temp.path().join("bin/demo-tool"), b"changed executable")
        .expect("change declared executable");
    let second_pack = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "pack",
            "--package",
            temp.path().to_str().expect("utf8 package path"),
            "--output",
            archive.to_str().expect("utf8 archive path"),
        ])
        .output()
        .expect("run second elegy pack");
    assert!(
        second_pack.status.success(),
        "stderr: {:?}",
        second_pack.stderr
    );
    let second_manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(temp.path().join("elegy-package.json")).expect("read second manifest"),
    )
    .expect("parse second manifest");
    let second_digest = second_manifest["files"][0]["sha256"]
        .as_str()
        .expect("second file digest");
    assert_ne!(first_digest, second_digest);
}

#[test]
fn pack_records_release_provenance_in_the_manifest_and_sbom() {
    let temp = tempfile::tempdir().expect("temporary directory");
    write_package(temp.path());
    let archive = temp.path().join("demo-tool.zip");
    let pack = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "pack",
            "--package",
            temp.path().to_str().expect("utf8 package path"),
            "--output",
            archive.to_str().expect("utf8 archive path"),
            "--source-commit",
            "0123456789abcdef",
            "--build-workflow",
            "https://github.com/example/demo-tool/actions",
            "--builder",
            "github-actions",
        ])
        .output()
        .expect("run elegy pack with provenance");
    assert!(pack.status.success(), "stderr: {:?}", pack.stderr);
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(temp.path().join("elegy-package.json")).expect("read manifest"),
    )
    .expect("parse manifest");
    assert_eq!(manifest["provenance"]["builder"], "github-actions");
    let sbom: serde_json::Value = serde_json::from_slice(
        &fs::read(temp.path().join("demo-tool.zip.sbom.json")).expect("read SBOM"),
    )
    .expect("parse SBOM");
    assert_eq!(sbom["schemaVersion"], "elegy-sbom/v1");
    assert_eq!(sbom["provenance"]["sourceCommit"], "0123456789abcdef");
}

#[test]
fn test_runs_the_host_neutral_contract_checks() {
    let temp = tempfile::tempdir().expect("temporary directory");
    write_package(temp.path());

    let test = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "test",
            "--package",
            temp.path().to_str().expect("utf8 package path"),
        ])
        .output()
        .expect("run elegy test");
    assert!(test.status.success(), "stderr: {:?}", test.stderr);
}

#[test]
fn check_rejects_a_cli_capability_without_schemas() {
    let temp = tempfile::tempdir().expect("temporary directory");
    write_package(temp.path());
    let catalog_path = temp.path().join("capability-catalog.json");
    let mut catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(&catalog_path).expect("read catalog"))
            .expect("parse catalog");
    let invocation = catalog["capabilities"][0]["invocation"]
        .as_object_mut()
        .expect("invocation object");
    invocation.remove("inputSchema");
    invocation.remove("outputSchema");
    fs::write(
        &catalog_path,
        serde_json::to_vec(&catalog).expect("serialize catalog"),
    )
    .expect("write catalog");
    let manifest_path = temp.path().join("elegy-package.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    let catalog_digest = format!(
        "{:x}",
        Sha256::digest(fs::read(&catalog_path).expect("read changed catalog"))
    );
    for file in manifest["files"].as_array_mut().expect("files array") {
        if file["path"] == "./capability-catalog.json" {
            file["sha256"] = serde_json::json!(catalog_digest);
        }
    }
    fs::write(
        manifest_path,
        serde_json::to_vec(&manifest).expect("serialize changed manifest"),
    )
    .expect("write changed manifest");

    let check = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "check",
            "--package",
            temp.path().to_str().expect("utf8 path"),
        ])
        .output()
        .expect("run elegy check");

    assert!(
        !check.status.success(),
        "a CLI without schemas must be rejected"
    );
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(stderr.contains("inputSchema"), "stderr: {stderr}");
}

#[test]
fn lock_create_install_and_verify_pin_one_exact_package() {
    let temp = tempfile::tempdir().expect("temporary directory");
    write_package(temp.path());
    let archive = temp.path().join("demo-tool.zip");
    let lock = temp.path().join("elegy.lock.json");
    let install_root = temp.path().join("installed");

    let pack = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "pack",
            "--package",
            temp.path().to_str().expect("utf8 package path"),
            "--output",
            archive.to_str().expect("utf8 archive path"),
        ])
        .output()
        .expect("run elegy pack");
    assert!(pack.status.success(), "stderr: {:?}", pack.stderr);

    let create_lock = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "lock",
            "create",
            "--package",
            temp.path().to_str().expect("utf8 package path"),
            "--archive",
            archive.to_str().expect("utf8 archive path"),
            "--agent-id",
            "test-agent",
            "--target",
            "any",
            "--source",
            "https://github.com/Sofreshx/Elegy/releases",
            "--publisher",
            "https://github.com/example/demo-tool",
            "--capability",
            "demo-tool.example",
            "--output",
            lock.to_str().expect("utf8 lock path"),
        ])
        .output()
        .expect("run elegy lock create");
    assert!(
        create_lock.status.success(),
        "stderr: {:?}",
        create_lock.stderr
    );
    let lock_value: serde_json::Value =
        serde_json::from_slice(&fs::read(&lock).expect("read lock")).expect("parse lock");
    assert_eq!(lock_value["packages"][0]["version"], "0.1.0");
    assert_eq!(
        lock_value["packages"][0]["executableDigests"]["./bin/demo-tool"],
        serde_json::Value::String(format!("{:x}", Sha256::digest(b"demo tool")))
    );

    let install = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "install",
            "--archive",
            archive.to_str().expect("utf8 archive path"),
            "--lock",
            lock.to_str().expect("utf8 lock path"),
            "--target",
            "any",
            "--install-root",
            install_root.to_str().expect("utf8 install path"),
        ])
        .output()
        .expect("run elegy install");
    assert!(install.status.success(), "stderr: {:?}", install.stderr);

    let verify = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "verify",
            "--package",
            install_root
                .join("demo-tool")
                .to_str()
                .expect("utf8 installed path"),
            "--lock",
            lock.to_str().expect("utf8 lock path"),
            "--target",
            "any",
        ])
        .output()
        .expect("run elegy verify");
    assert!(verify.status.success(), "stderr: {:?}", verify.stderr);

    let locked_projection = temp.path().join("locked-mcp");
    let project = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "project",
            "--package",
            install_root
                .join("demo-tool")
                .to_str()
                .expect("utf8 installed path"),
            "--host",
            "mcp",
            "--lock",
            lock.to_str().expect("utf8 lock path"),
            "--target",
            "any",
            "--output",
            locked_projection.to_str().expect("utf8 projection path"),
        ])
        .output()
        .expect("project verified install");
    assert!(project.status.success(), "stderr: {:?}", project.stderr);
    assert!(locked_projection.join("elegy.lock.json").is_file());
    assert!(locked_projection
        .join("capability-install-receipt.json")
        .is_file());
    let locked_config: serde_json::Value = serde_json::from_slice(
        &fs::read(locked_projection.join("mcp.json")).expect("read locked MCP projection"),
    )
    .expect("parse locked MCP projection");
    assert_eq!(
        locked_config["mcpServers"]["demo-tool"]["args"],
        serde_json::json!([
            "--package",
            ".",
            "--lock",
            "./elegy.lock.json",
            "--target",
            "any"
        ])
    );
}

#[test]
fn project_generates_host_bindings_from_the_same_catalog() {
    let temp = tempfile::tempdir().expect("temporary directory");
    write_package(temp.path());
    let output = temp.path().join("mcp-projection");

    let project = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "project",
            "--package",
            temp.path().to_str().expect("utf8 package path"),
            "--host",
            "mcp",
            "--output",
            output.to_str().expect("utf8 output path"),
        ])
        .output()
        .expect("run elegy project");
    assert!(project.status.success(), "stderr: {:?}", project.stderr);

    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("mcp.json")).expect("read MCP projection"))
            .expect("parse MCP projection");
    assert_eq!(
        config["mcpServers"]["demo-tool"]["command"],
        "elegy-capability-mcp"
    );
    assert_eq!(
        config["mcpServers"]["demo-tool"]["x-elegy"]["capabilityIds"],
        serde_json::json!(["demo-tool.example"])
    );

    let codex_output = temp.path().join("codex-projection");
    let project = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "project",
            "--package",
            temp.path().to_str().expect("utf8 package path"),
            "--host",
            "codex",
            "--output",
            codex_output.to_str().expect("utf8 output path"),
        ])
        .output()
        .expect("run codex project");
    assert!(project.status.success(), "stderr: {:?}", project.stderr);
    assert!(codex_output.join(".codex-plugin/plugin.json").is_file());
}
