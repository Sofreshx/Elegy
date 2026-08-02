use elegy_capability_mcp::{BridgeOptions, CapabilityMcpBridge};
use elegy_plugin_sdk::capability_package::{
    canonical_json_sha256, ElegyCapabilityReferenceV1, ElegyLockV1, ElegyPackageEntrypointKind,
    ElegyPackageEntrypointV1, ElegyPackageFileV1, ElegyPackagePublisherV1, ElegyPackageV1,
    ELEGY_LOCK_V1_SCHEMA_VERSION, ELEGY_PACKAGE_V1_SCHEMA_VERSION,
};
use rmcp::{model::CallToolRequestParams, ClientHandler, ServiceExt};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

fn write_package(root: &Path, side_effect_class: &str) {
    fs::create_dir_all(root.join("bin")).expect("create bin");
    let fixture = env!("CARGO_BIN_EXE_elegy-capability-fixture");
    fs::copy(fixture, root.join("bin/fixture.exe")).expect("copy fixture");

    let package = ElegyPackageV1 {
        schema_version: ELEGY_PACKAGE_V1_SCHEMA_VERSION.to_string(),
        name: "fixture-tool".to_string(),
        version: "0.1.0".to_string(),
        description: "Bridge fixture".to_string(),
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
            id: "fixture-cli".to_string(),
            kind: ElegyPackageEntrypointKind::Cli,
            executable: "./bin/fixture.exe".to_string(),
            command: Vec::new(),
        }],
        files: vec![
            ElegyPackageFileV1 {
                path: "./bin/fixture.exe".to_string(),
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
    fs::write(
        root.join("capability-catalog.json"),
        serde_json::to_vec_pretty(&json!({
            "schemaVersion": "elegy-capability-catalog/v2",
            "plugin": "fixture-tool",
            "pluginVersion": "0.1.0",
            "capabilities": [{
                "kind": "cli",
                "id": "fixture.echo",
                "description": "Echo JSON input for bridge tests.",
                "contractVersion": "v1",
                "sideEffectClass": side_effect_class,
                "readiness": "usable",
                "invocation": {
                    "executable": "./bin/fixture.exe",
                    "command": ["--fixture"],
                    "inputSchema": {
                        "type": "object",
                        "properties": {"value": {"type": "string"}},
                        "required": ["value"]
                    },
                    "outputSchema": {
                        "type": "object",
                        "properties": {"echo": {"type": "string"}},
                        "required": ["echo"]
                    }
                }
            }]
        }))
        .expect("serialize catalog"),
    )
    .expect("write catalog");
}

fn write_package_with_command(root: &Path, side_effect_class: &str, command: &[&str]) {
    write_package(root, side_effect_class);
    let catalog_path = root.join("capability-catalog.json");
    let mut catalog: Value =
        serde_json::from_slice(&fs::read(&catalog_path).expect("read catalog")).expect("catalog");
    catalog["capabilities"][0]["invocation"]["command"] =
        serde_json::to_value(command).expect("command JSON");
    fs::write(
        catalog_path,
        serde_json::to_vec_pretty(&catalog).expect("serialize command catalog"),
    )
    .expect("write command catalog");
}

#[test]
fn bridge_lists_only_routable_query_capabilities_by_default() {
    let temp = tempfile::tempdir().expect("temporary directory");
    write_package(temp.path(), "query");
    let bridge = CapabilityMcpBridge::load(BridgeOptions::new(temp.path())).expect("load bridge");

    let tools = bridge.tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "fixture.echo");
    assert_eq!(tools[0].input_schema["required"], json!(["value"]));
    assert_eq!(
        tools[0].output_schema.as_ref().expect("output schema")["type"],
        "object"
    );
    assert_eq!(
        tools[0]
            .annotations
            .as_ref()
            .expect("annotations")
            .read_only_hint,
        Some(true)
    );
}

#[test]
fn bridge_hides_side_effecting_capabilities_without_explicit_opt_in() {
    let temp = tempfile::tempdir().expect("temporary directory");
    write_package(temp.path(), "mutation");

    let bridge = CapabilityMcpBridge::load(BridgeOptions::new(temp.path())).expect("load bridge");
    assert!(bridge.tools().is_empty());

    let bridge = CapabilityMcpBridge::load(BridgeOptions::new(temp.path()).with_side_effects(true))
        .expect("load side-effect bridge");
    assert_eq!(bridge.tools().len(), 1);
}

#[tokio::test]
async fn bridge_invokes_cli_without_a_shell_and_validates_json() {
    let temp = tempfile::tempdir().expect("temporary directory");
    write_package(temp.path(), "query");
    let bridge = CapabilityMcpBridge::load(BridgeOptions::new(temp.path())).expect("load bridge");

    let result = bridge
        .invoke("fixture.echo", json!({"value": "hello"}))
        .await
        .expect("invoke fixture");
    assert_eq!(result, json!({"echo": "hello"}));

    let error = bridge
        .invoke("fixture.echo", json!({"value": 7}))
        .await
        .expect_err("invalid input must fail");
    assert!(error.to_string().contains("input schema"));
}

#[tokio::test]
async fn bridge_rejects_unknown_capabilities_before_process_start() {
    let temp = tempfile::tempdir().expect("temporary directory");
    write_package(temp.path(), "query");
    let bridge = CapabilityMcpBridge::load(BridgeOptions::new(temp.path())).expect("load bridge");

    let error = bridge
        .invoke("not-allowed", Value::Object(Default::default()))
        .await
        .expect_err("unknown capability must fail");
    assert!(error.to_string().contains("not declared"));
}

#[tokio::test]
async fn bridge_serves_the_same_contract_to_an_mcp_client() {
    let temp = tempfile::tempdir().expect("temporary directory");
    write_package(temp.path(), "query");
    let bridge = CapabilityMcpBridge::load(BridgeOptions::new(temp.path())).expect("load bridge");
    let (server_transport, client_transport) = tokio::io::duplex(16_384);
    let server_task = tokio::spawn(async move {
        let service = bridge
            .serve(server_transport)
            .await
            .expect("bridge server initializes");
        service
            .waiting()
            .await
            .expect("bridge server stops cleanly");
    });

    #[derive(Clone, Default)]
    struct TestClient;
    impl ClientHandler for TestClient {}

    let client = TestClient
        .serve(client_transport)
        .await
        .expect("MCP client initializes");
    let tools = client.list_all_tools().await.expect("list MCP tools");
    assert_eq!(tools.len(), 1);
    let result = client
        .call_tool(
            CallToolRequestParams::new("fixture.echo")
                .with_arguments(serde_json::from_value(json!({"value": "mcp"})).expect("args")),
        )
        .await
        .expect("call MCP tool");
    assert_eq!(result.structured_content, Some(json!({"echo": "mcp"})));

    client.cancel().await.expect("cancel MCP client");
    server_task.await.expect("join bridge server");
}

#[test]
fn lock_backed_bridge_requires_a_matching_install_receipt() {
    let temp = tempfile::tempdir().expect("temporary directory");
    write_package(temp.path(), "query");
    let package: ElegyPackageV1 = serde_json::from_slice(
        &fs::read(temp.path().join("elegy-package.json")).expect("read package"),
    )
    .expect("parse package");
    let catalog: Value = serde_json::from_slice(
        &fs::read(temp.path().join("capability-catalog.json")).expect("read catalog"),
    )
    .expect("parse catalog");
    let fixture_digest = format!(
        "{:x}",
        Sha256::digest(fs::read(temp.path().join("bin/fixture.exe")).expect("fixture bytes"))
    );
    let lock = ElegyLockV1 {
        schema_version: ELEGY_LOCK_V1_SCHEMA_VERSION.to_string(),
        agent_id: "bridge-agent".to_string(),
        packages: vec![ElegyCapabilityReferenceV1 {
            name: package.name.clone(),
            version: package.version.clone(),
            target: "any".to_string(),
            source: "https://github.com/example/fixture-tool/releases".to_string(),
            archive_sha256: "a".repeat(64),
            manifest_sha256: canonical_json_sha256(&package).expect("manifest digest"),
            capability_catalog_sha256: canonical_json_sha256(&catalog).expect("catalog digest"),
            executable_digests: BTreeMap::from([("./bin/fixture.exe".to_string(), fixture_digest)]),
            publisher: package.publisher.repository.clone(),
            allowed_capabilities: vec!["fixture.echo".to_string()],
        }],
    };
    let lock_path = temp.path().join("elegy.lock.json");
    fs::write(
        &lock_path,
        serde_json::to_vec_pretty(&lock).expect("serialize lock"),
    )
    .expect("write lock");

    let error = CapabilityMcpBridge::load(BridgeOptions::new(temp.path()).with_lock(&lock_path))
        .expect_err("lock-backed bridge must require receipt");
    assert!(error.to_string().contains("install receipt"), "{error}");

    fs::write(
        temp.path().join("capability-install-receipt.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": "elegy-capability-installer/v1",
            "name": package.name,
            "version": package.version,
            "target": "any",
            "publisher": package.publisher.repository,
            "archiveSha256": "a".repeat(64),
            "manifestSha256": canonical_json_sha256(&package).expect("manifest digest"),
            "capabilityCatalogSha256": canonical_json_sha256(&catalog).expect("catalog digest"),
            "installDir": temp.path().display().to_string(),
            "files": {
                "elegy-package.json": format!("{:x}", Sha256::digest(serde_json::to_vec_pretty(&package).expect("manifest bytes"))),
                "bin/fixture.exe": format!("{:x}", Sha256::digest(fs::read(temp.path().join("bin/fixture.exe")).expect("fixture bytes"))),
                "capability-catalog.json": format!("{:x}", Sha256::digest(fs::read(temp.path().join("capability-catalog.json")).expect("catalog bytes")))
            }
        }))
        .expect("serialize receipt"),
    )
    .expect("write receipt");
    let bridge = CapabilityMcpBridge::load(BridgeOptions::new(temp.path()).with_lock(&lock_path))
        .expect("matching lock receipt should load");
    assert_eq!(bridge.tools().len(), 1);
}

#[tokio::test]
async fn bridge_bounds_timeout_output_and_malformed_json() {
    let malformed = tempfile::tempdir().expect("temporary directory");
    write_package_with_command(malformed.path(), "query", &["--malformed"]);
    let bridge = CapabilityMcpBridge::load(BridgeOptions::new(malformed.path()))
        .expect("load malformed fixture");
    let error = bridge
        .invoke("fixture.echo", json!({"value": "x"}))
        .await
        .expect_err("malformed output must fail");
    assert!(error.to_string().contains("parse CLI output"), "{error}");

    let large = tempfile::tempdir().expect("temporary directory");
    write_package_with_command(large.path(), "query", &["--large"]);
    let bridge =
        CapabilityMcpBridge::load(BridgeOptions::new(large.path()).with_max_output_bytes(64))
            .expect("load large fixture");
    let error = bridge
        .invoke("fixture.echo", json!({"value": "x"}))
        .await
        .expect_err("large output must fail");
    assert!(error.to_string().contains("output bytes"), "{error}");

    let slow = tempfile::tempdir().expect("temporary directory");
    write_package_with_command(slow.path(), "query", &["--sleep"]);
    let bridge = CapabilityMcpBridge::load(
        BridgeOptions::new(slow.path()).with_timeout(Duration::from_millis(20)),
    )
    .expect("load slow fixture");
    let error = bridge
        .invoke("fixture.echo", json!({"value": "x"}))
        .await
        .expect_err("slow output must time out");
    assert!(error.to_string().contains("timed out"), "{error}");
}
