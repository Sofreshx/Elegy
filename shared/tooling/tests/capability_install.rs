use elegy_plugin_sdk::capability_package::{
    canonical_json_sha256, pack_elegy_package_v1, ElegyCapabilityReferenceV1, ElegyLockV1,
    ElegyPackageEntrypointKind, ElegyPackageEntrypointV1, ElegyPackageFileV1,
    ElegyPackagePublisherV1, ElegyPackageV1, ELEGY_LOCK_V1_SCHEMA_VERSION,
    ELEGY_PACKAGE_V1_SCHEMA_VERSION,
};
use elegy_tooling::capability_installer::{
    install_elegy_package_from_archive, uninstall_elegy_package, update_elegy_package_from_archive,
    verify_elegy_installation,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn write_package(root: &Path) -> (ElegyPackageV1, serde_json::Value) {
    fs::create_dir_all(root.join("bin")).expect("create bin");
    fs::write(root.join("bin/tool"), b"tool binary").expect("write executable");
    let catalog = json!({
        "schemaVersion": "elegy-capability-catalog/v2",
        "plugin": "portable-tool",
        "pluginVersion": "1.0.0",
        "capabilities": [{
            "kind": "cli",
            "id": "portable-tool.echo",
            "description": "Echo input",
            "contractVersion": "v1",
            "sideEffectClass": "query",
            "readiness": "production",
            "invocation": {
                "executable": "./bin/tool",
                "command": ["--json"],
                "inputSchema": {"type": "object"},
                "outputSchema": {"type": "object"}
            }
        }]
    });
    fs::write(
        root.join("capability-catalog.json"),
        serde_json::to_vec_pretty(&catalog).expect("serialize catalog"),
    )
    .expect("write catalog");
    let package = ElegyPackageV1 {
        schema_version: ELEGY_PACKAGE_V1_SCHEMA_VERSION.to_string(),
        name: "portable-tool".to_string(),
        version: "1.0.0".to_string(),
        description: "Portable tool".to_string(),
        publisher: ElegyPackagePublisherV1 {
            name: "Elegy Tests".to_string(),
            repository: "https://github.com/Sofreshx/Elegy".to_string(),
            workflow_identity: None,
        },
        license: "Apache-2.0".to_string(),
        targets: vec!["any".to_string()],
        capability_catalog: "./capability-catalog.json".to_string(),
        readiness: None,
        entrypoints: vec![ElegyPackageEntrypointV1 {
            id: "portable-tool-cli".to_string(),
            kind: ElegyPackageEntrypointKind::Cli,
            executable: "./bin/tool".to_string(),
            command: vec!["--json".to_string()],
        }],
        files: vec![
            ElegyPackageFileV1 {
                path: "./bin/tool".to_string(),
                role: "executable".to_string(),
                sha256: None,
            },
            ElegyPackageFileV1 {
                path: "./capability-catalog.json".to_string(),
                role: "capability-catalog".to_string(),
                sha256: None,
            },
        ],
        skills: Vec::new(),
        provenance: None,
    };
    fs::write(
        root.join("elegy-package.json"),
        serde_json::to_vec_pretty(&package).expect("serialize package"),
    )
    .expect("write package");
    (package, catalog)
}

fn lock_for(
    package: &ElegyPackageV1,
    catalog: &serde_json::Value,
    archive_sha256: String,
) -> ElegyLockV1 {
    ElegyLockV1 {
        schema_version: ELEGY_LOCK_V1_SCHEMA_VERSION.to_string(),
        agent_id: "test-agent".to_string(),
        packages: vec![ElegyCapabilityReferenceV1 {
            name: package.name.clone(),
            version: package.version.clone(),
            target: "x86_64-pc-windows-msvc".to_string(),
            source: "https://github.com/Sofreshx/Elegy/releases".to_string(),
            archive_sha256,
            manifest_sha256: canonical_json_sha256(package).expect("manifest digest"),
            capability_catalog_sha256: canonical_json_sha256(catalog).expect("catalog digest"),
            executable_digests: BTreeMap::from([(
                "./bin/tool".to_string(),
                format!("{:x}", Sha256::digest(b"tool binary")),
            )]),
            publisher: package.publisher.repository.clone(),
            allowed_capabilities: vec!["portable-tool.echo".to_string()],
        }],
    }
}

#[test]
fn install_and_verify_use_the_exact_lock_and_file_hashes() {
    let temp = tempfile::tempdir().expect("temporary directory");
    let package_root = temp.path().join("package");
    let (package, catalog) = write_package(&package_root);
    let archive = temp.path().join("portable-tool.zip");
    let archive_sha256 = pack_elegy_package_v1(&package_root, &archive).expect("pack package");
    let lock = lock_for(&package, &catalog, archive_sha256);
    let install_root = temp.path().join("installed");

    let receipt = install_elegy_package_from_archive(
        &archive,
        &install_root,
        &lock,
        "x86_64-pc-windows-msvc",
    )
    .expect("install exact package");
    assert_eq!(receipt.name, package.name);
    assert!(receipt.files.contains_key("bin/tool"));
    verify_elegy_installation(
        Path::new(&receipt.install_dir),
        &lock,
        "x86_64-pc-windows-msvc",
    )
    .expect("verify installed package");

    fs::write(
        Path::new(&receipt.install_dir).join("bin/tool"),
        b"tampered",
    )
    .expect("tamper installed file");
    let error = verify_elegy_installation(
        Path::new(&receipt.install_dir),
        &lock,
        "x86_64-pc-windows-msvc",
    )
    .expect_err("tampered installed file must fail");
    assert!(error.to_string().contains("digest"), "{error}");
}

#[test]
fn install_rejects_archive_digest_drift_before_writing() {
    let temp = tempfile::tempdir().expect("temporary directory");
    let package_root = temp.path().join("package");
    let (package, catalog) = write_package(&package_root);
    let archive = temp.path().join("portable-tool.zip");
    let archive_sha256 = pack_elegy_package_v1(&package_root, &archive).expect("pack package");
    let mut lock = lock_for(&package, &catalog, archive_sha256);
    lock.packages[0].archive_sha256 = "a".repeat(64);
    let install_root = temp.path().join("installed");

    let error = install_elegy_package_from_archive(
        &archive,
        &install_root,
        &lock,
        "x86_64-pc-windows-msvc",
    )
    .expect_err("wrong archive digest must fail");
    assert!(error.to_string().contains("archive digest"), "{error}");
    assert!(!install_root.join("portable-tool").exists());
}

#[test]
fn install_rejects_executable_digest_drift_before_writing() {
    let temp = tempfile::tempdir().expect("temporary directory");
    let package_root = temp.path().join("package");
    let (package, catalog) = write_package(&package_root);
    let archive = temp.path().join("portable-tool.zip");
    let archive_sha256 = pack_elegy_package_v1(&package_root, &archive).expect("pack package");
    let mut lock = lock_for(&package, &catalog, archive_sha256);
    lock.packages[0]
        .executable_digests
        .insert("./bin/tool".to_string(), "a".repeat(64));
    let install_root = temp.path().join("installed");

    let error = install_elegy_package_from_archive(
        &archive,
        &install_root,
        &lock,
        "x86_64-pc-windows-msvc",
    )
    .expect_err("wrong executable digest must fail");
    assert!(error.to_string().contains("executable digest"), "{error}");
    assert!(!install_root.join("portable-tool").exists());
}

#[test]
fn updates_are_integrity_checked_and_uninstall_is_lock_guarded() {
    let temp = tempfile::tempdir().expect("temporary directory");
    let package_root = temp.path().join("package");
    let (package, catalog) = write_package(&package_root);
    let archive = temp.path().join("portable-tool.zip");
    let archive_sha256 = pack_elegy_package_v1(&package_root, &archive).expect("pack package");
    let lock = lock_for(&package, &catalog, archive_sha256);
    let install_root = temp.path().join("installed");
    let receipt = install_elegy_package_from_archive(
        &archive,
        &install_root,
        &lock,
        "x86_64-pc-windows-msvc",
    )
    .expect("initial install");

    update_elegy_package_from_archive(&archive, &install_root, &lock, "x86_64-pc-windows-msvc")
        .expect("verified atomic update");
    assert!(Path::new(&receipt.install_dir).is_dir());

    fs::write(
        Path::new(&receipt.install_dir).join("bin/tool"),
        b"tampered",
    )
    .expect("tamper existing install");
    let error =
        update_elegy_package_from_archive(&archive, &install_root, &lock, "x86_64-pc-windows-msvc")
            .expect_err("tampered install must block update");
    assert!(error.to_string().contains("digest"), "{error}");

    fs::write(
        Path::new(&receipt.install_dir).join("bin/tool"),
        b"tool binary",
    )
    .expect("restore installed fixture");
    uninstall_elegy_package(
        Path::new(&receipt.install_dir),
        &lock,
        "x86_64-pc-windows-msvc",
    )
    .expect("verified uninstall");
    assert!(!Path::new(&receipt.install_dir).exists());
}
