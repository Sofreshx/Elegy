use elegy_plugin_sdk::capability_package::{
    canonical_json_sha256, pack_elegy_package_v1, validate_elegy_lock_v1,
    validate_elegy_package_v1, validate_elegy_sbom_v1, verify_elegy_package_v1,
    ElegyCapabilityReferenceV1, ElegyLockV1, ElegyPackageEntrypointKind, ElegyPackageEntrypointV1,
    ElegyPackageFileV1, ElegyPackageProvenanceV1, ElegyPackagePublisherV1, ElegyPackageV1,
    ElegySbomFileV1, ElegySbomV1, ELEGY_LOCK_V1_SCHEMA_VERSION, ELEGY_PACKAGE_V1_SCHEMA_VERSION,
    ELEGY_SBOM_V1_SCHEMA_VERSION,
};
use elegy_plugin_sdk::generate_plugin_schema_artifacts;
use std::collections::BTreeMap;

fn package() -> ElegyPackageV1 {
    ElegyPackageV1 {
        schema_version: ELEGY_PACKAGE_V1_SCHEMA_VERSION.to_string(),
        name: "example-tool".to_string(),
        version: "1.2.3".to_string(),
        description: "A small JSON-first tool.".to_string(),
        publisher: ElegyPackagePublisherV1 {
            name: "Elegy Contributors".to_string(),
            repository: "https://github.com/example/example-tool".to_string(),
            workflow_identity: Some(".github/workflows/release.yml".to_string()),
        },
        license: "Apache-2.0".to_string(),
        targets: vec!["x86_64-pc-windows-msvc".to_string()],
        capability_catalog: "./capability-catalog.json".to_string(),
        readiness: Some("./readiness.json".to_string()),
        entrypoints: vec![ElegyPackageEntrypointV1 {
            id: "example-cli".to_string(),
            kind: ElegyPackageEntrypointKind::Cli,
            executable: "./bin/example-tool.exe".to_string(),
            command: vec!["--json".to_string()],
        }],
        files: vec![
            ElegyPackageFileV1 {
                path: "./bin/example-tool.exe".to_string(),
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
            ElegyPackageFileV1 {
                path: "./skills/example-tool/SKILL.md".to_string(),
                role: "skill".to_string(),
                sha256: None,
            },
        ],
        skills: vec!["./skills/example-tool/SKILL.md".to_string()],
        provenance: None,
    }
}

fn lock() -> ElegyLockV1 {
    ElegyLockV1 {
        schema_version: ELEGY_LOCK_V1_SCHEMA_VERSION.to_string(),
        agent_id: "example-agent".to_string(),
        packages: vec![ElegyCapabilityReferenceV1 {
            name: "example-tool".to_string(),
            version: "1.2.3".to_string(),
            target: "x86_64-pc-windows-msvc".to_string(),
            source:
                "https://github.com/example/example-tool/releases/download/v1.2.3/example-tool.zip"
                    .to_string(),
            archive_sha256: "a".repeat(64),
            manifest_sha256: "b".repeat(64),
            capability_catalog_sha256: "c".repeat(64),
            executable_digests: BTreeMap::from([(
                "./bin/example-tool.exe".to_string(),
                "d".repeat(64),
            )]),
            publisher: "https://github.com/example/example-tool".to_string(),
            allowed_capabilities: vec!["example.read".to_string()],
        }],
    }
}

#[test]
fn validates_a_host_neutral_package_manifest() {
    let issues = validate_elegy_package_v1(&package());

    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
}

#[test]
fn rejects_package_paths_that_escape_the_package_root() {
    let mut value = package();
    value.files[0].path = "../outside.exe".to_string();

    let issues = validate_elegy_package_v1(&value);

    assert!(issues
        .iter()
        .any(|issue| issue.contains("safe package-relative")));
}

#[test]
fn rejects_platform_specific_backslash_package_paths() {
    let mut value = package();
    value.files[0].path = ".\\bin\\example-tool.exe".to_string();

    let issues = validate_elegy_package_v1(&value);

    assert!(issues
        .iter()
        .any(|issue| issue.contains("safe package-relative")));
}

#[test]
fn validates_an_exact_agent_lock() {
    let issues = validate_elegy_lock_v1(&lock());

    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
}

#[test]
fn rejects_duplicate_capabilities_in_an_agent_lock() {
    let mut value = lock();
    value.packages[0]
        .allowed_capabilities
        .push("example.read".to_string());

    let issues = validate_elegy_lock_v1(&value);

    assert!(issues
        .iter()
        .any(|issue| issue.contains("duplicate allowed capability")));
}

#[test]
fn canonical_json_digest_is_stable_for_the_same_value() {
    let first = canonical_json_sha256(&lock()).expect("serialize lock");
    let second = canonical_json_sha256(&lock()).expect("serialize lock");

    assert_eq!(first, second);
    assert_eq!(first.len(), 64);
}

#[test]
fn generates_package_and_lock_schema_artifacts() {
    let artifacts = generate_plugin_schema_artifacts().expect("generate schemas");

    assert!(artifacts.contains_key("elegy-package-v1.schema.json"));
    assert!(artifacts.contains_key("elegy-lock-v1.schema.json"));
    assert!(artifacts.contains_key("elegy-sbom-v1.schema.json"));
}

#[test]
fn validates_a_deterministic_sbom_contract() {
    let sbom = ElegySbomV1 {
        schema_version: ELEGY_SBOM_V1_SCHEMA_VERSION.to_string(),
        package: "example-tool".to_string(),
        version: "1.2.3".to_string(),
        publisher: "https://github.com/example/example-tool".to_string(),
        archive_sha256: "a".repeat(64),
        files: vec![ElegySbomFileV1 {
            path: "elegy-package.json".to_string(),
            role: "package-manifest".to_string(),
            sha256: "b".repeat(64),
        }],
        provenance: None,
    };
    assert!(validate_elegy_sbom_v1(&sbom).is_empty());
}

#[test]
fn packs_and_verifies_an_independent_capability_package() {
    let temp = tempfile::tempdir().expect("temporary package root");
    let root = temp.path().join("example-tool");
    std::fs::create_dir_all(root.join("bin")).expect("create bin");
    std::fs::create_dir_all(root.join("skills/example-tool")).expect("create skills");
    std::fs::write(root.join("bin/example-tool.exe"), b"binary").expect("write executable");
    std::fs::write(
        root.join("capability-catalog.json"),
        b"{\"capabilities\":[]}",
    )
    .expect("write catalog");
    std::fs::write(root.join("readiness.json"), b"{\"stage\":\"implemented\"}")
        .expect("write readiness");
    std::fs::write(
        root.join("skills/example-tool/SKILL.md"),
        b"# Example tool\n",
    )
    .expect("write skill");
    std::fs::write(
        root.join("elegy-package.json"),
        serde_json::to_vec_pretty(&package()).expect("serialize package"),
    )
    .expect("write package manifest");
    let archive = temp.path().join("example-tool.zip");

    let digest = pack_elegy_package_v1(&root, &archive).expect("pack package");
    let loaded = verify_elegy_package_v1(&root).expect("verify package");

    assert_eq!(loaded, package());
    assert_eq!(digest.len(), 64);
    assert!(archive.is_file());
}

#[test]
fn pack_rejects_archive_output_that_overwrites_package_inputs() {
    let temp = tempfile::tempdir().expect("temporary package root");
    let root = temp.path().join("example-tool");
    std::fs::create_dir_all(root.join("bin")).expect("create bin");
    std::fs::create_dir_all(root.join("skills/example-tool")).expect("create skills");
    std::fs::write(root.join("bin/example-tool.exe"), b"binary").expect("write executable");
    std::fs::write(root.join("capability-catalog.json"), b"catalog").expect("write catalog");
    std::fs::write(root.join("readiness.json"), b"readiness").expect("write readiness");
    std::fs::write(
        root.join("skills/example-tool/SKILL.md"),
        b"# Example tool\n",
    )
    .expect("write skill");
    std::fs::write(
        root.join("elegy-package.json"),
        serde_json::to_vec_pretty(&package()).expect("serialize package"),
    )
    .expect("write package manifest");

    let catalog_path = root.join("capability-catalog.json");
    let catalog_before = std::fs::read(&catalog_path).expect("read catalog before pack");
    let catalog_result = pack_elegy_package_v1(&root, &catalog_path);
    assert!(catalog_result.is_err(), "pack must reject input collisions");
    assert_eq!(
        std::fs::read(&catalog_path).expect("read catalog after pack"),
        catalog_before
    );

    let manifest_path = root.join("elegy-package.json");
    let manifest_before = std::fs::read(&manifest_path).expect("read manifest before pack");
    let manifest_result = pack_elegy_package_v1(&root, &manifest_path);
    assert!(
        manifest_result.is_err(),
        "pack must reject manifest collisions"
    );
    assert_eq!(
        std::fs::read(&manifest_path).expect("read manifest after pack"),
        manifest_before
    );
}

#[test]
fn package_archives_are_byte_for_byte_reproducible() {
    let temp = tempfile::tempdir().expect("temporary package root");
    let root = temp.path().join("example-tool");
    std::fs::create_dir_all(root.join("bin")).expect("create bin");
    std::fs::create_dir_all(root.join("skills/example-tool")).expect("create skills");
    std::fs::write(root.join("bin/example-tool.exe"), b"binary").expect("write executable");
    std::fs::write(root.join("capability-catalog.json"), b"catalog").expect("write catalog");
    std::fs::write(root.join("readiness.json"), b"readiness").expect("write readiness");
    std::fs::write(
        root.join("skills/example-tool/SKILL.md"),
        b"# Example tool\n",
    )
    .expect("write skill");
    std::fs::write(
        root.join("elegy-package.json"),
        serde_json::to_vec_pretty(&package()).expect("serialize package"),
    )
    .expect("write package manifest");
    let first = temp.path().join("first.zip");
    let second = temp.path().join("second.zip");

    pack_elegy_package_v1(&root, &first).expect("pack first archive");
    pack_elegy_package_v1(&root, &second).expect("pack second archive");

    assert_eq!(
        std::fs::read(first).expect("read first archive"),
        std::fs::read(second).expect("read second archive")
    );
}

#[test]
fn package_file_digests_and_release_provenance_are_validated() {
    let mut value = package();
    value.files[0].sha256 = Some("a".repeat(64));
    value.provenance = Some(ElegyPackageProvenanceV1 {
        source_commit: Some("0123456789abcdef0123456789abcdef01234567".to_string()),
        build_workflow: Some("https://github.com/example/example-tool/actions".to_string()),
        builder: Some("github-actions".to_string()),
    });
    assert!(validate_elegy_package_v1(&value).is_empty());

    value.files[0].sha256 = Some("not-a-digest".to_string());
    assert!(validate_elegy_package_v1(&value)
        .iter()
        .any(|issue| issue.contains("sha256")));
}
