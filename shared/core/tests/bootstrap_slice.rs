use elegy_core::{
    compose_runtime, compose_runtime_state, load_agent_event_envelope_fixture_from_dir,
    load_agent_request_envelope_fixture_from_dir, load_agent_response_envelope_fixture_from_dir,
    resolve_upstream_contracts_dir, validate_agent_event_envelope, validate_agent_request_envelope,
    validate_agent_response_envelope, validate_descriptor_set, Catalog, ProjectLocator,
};
use elegy_mcp::{
    validate_mcp_analysis_result, validate_mcp_server_descriptor, McpAnalysisResult,
    McpServerDescriptor,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn create_mcp_project() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_nanos();
    let project_dir = std::env::temp_dir().join(format!("elegy-core-mcp-{unique}"));
    fs::create_dir_all(project_dir.join("elegy.resources.d")).expect("create descriptor directory");
    fs::write(
        project_dir.join("elegy.toml"),
        r#"version = 1

[project]
name = "core-mcp"

[descriptors]
include = ["elegy.resources.d/*.toml"]
"#,
    )
    .expect("write project config");
    fs::write(
        project_dir.join("elegy.resources.d").join("mcp.toml"),
        r#"version = 1
name = "mcp"

[[resources]]
kind = "static"
id = "Weather MCP"
uri = "elegy://core-mcp/resource/weather-mcp"
mime_type = "application/json"
content = '''{
  "serverName": "weather-server",
  "transport": "stdio",
  "tools": [
    {
      "name": "get-weather",
      "description": "Look up a weather report",
      "inputSchema": { "type": "object" }
    },
    {
      "name": "list-alerts",
      "description": "List active weather alerts"
    }
  ]
}'''
"#,
    )
    .expect("write MCP descriptor");

    project_dir
}

#[test]
fn fs_static_example_validates() {
    let example = repo_root().join("examples/fs-static-minimal");
    let inspection =
        validate_descriptor_set(ProjectLocator::Path(example)).expect("config validates");

    assert_eq!(inspection.project_name, "fs-static-minimal");
    assert_eq!(inspection.root_config, "elegy.toml");
    assert_eq!(
        inspection.descriptor_files,
        vec!["elegy.resources.d/fs-static.toml"]
    );
    assert_eq!(inspection.resource_count, 2);
}

#[test]
fn fs_static_example_catalog_is_deterministic() {
    let example = repo_root().join("examples/fs-static-minimal");
    let first =
        compose_runtime(ProjectLocator::Path(example.clone())).expect("first composition succeeds");
    let second = compose_runtime(ProjectLocator::Path(example.clone()))
        .expect("second composition succeeds");
    let expected: Catalog = serde_json::from_str(
        &fs::read_to_string(example.join("expected-resources.json"))
            .expect("read expected manifest"),
    )
    .expect("parse expected manifest");

    assert_eq!(first, second);
    assert_eq!(first, expected);
}

#[test]
fn http_example_catalog_is_deterministic() {
    let example = repo_root().join("examples/http-minimal");
    let first =
        compose_runtime(ProjectLocator::Path(example.clone())).expect("first composition succeeds");
    let second = compose_runtime(ProjectLocator::Path(example.clone()))
        .expect("second composition succeeds");
    let expected: Catalog = serde_json::from_str(
        &fs::read_to_string(example.join("expected-resources.json"))
            .expect("read expected manifest"),
    )
    .expect("parse expected manifest");

    assert_eq!(first, second);
    assert_eq!(first, expected);
}

#[test]
fn http_openapi_example_still_rejects_openapi_runtime_execution() {
    let example = repo_root().join("examples/http-openapi-minimal");
    let error = compose_runtime(ProjectLocator::Path(example))
        .expect_err("open_api runtime support should remain scaffold-only");
    let codes: Vec<&str> = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();

    assert_eq!(codes, vec!["RUNTIME-UNSUPPORTED-FAMILY-002"]);
}

#[test]
fn duplicate_resource_uris_are_rejected() {
    let fixture = repo_root().join("shared/core/tests/fixtures/fs/duplicate-uri");
    let error =
        compose_runtime(ProjectLocator::Path(fixture)).expect_err("duplicate URI should fail");
    let codes: Vec<&str> = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();

    assert!(codes.contains(&"RUNTIME-DUPLICATE-URI-001"));
}

fn load_mcp_fixture<T: serde::de::DeserializeOwned>(
    dir: &PathBuf,
    name: &str,
) -> Result<T, String> {
    let path = dir.join("fixtures").join(name);
    let content =
        fs::read_to_string(&path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&content).map_err(|e| format!("failed to parse {}: {e}", path.display()))
}

#[test]
fn upstream_mcp_contract_fixtures_validate_through_core_facade() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let descriptor: McpServerDescriptor =
        load_mcp_fixture(&contracts_dir, "mcp-server-descriptor.minimal.json")
            .expect("load upstream mcp-server-descriptor fixture");
    let analysis: McpAnalysisResult =
        load_mcp_fixture(&contracts_dir, "mcp-analysis-result.minimal.json")
            .expect("load upstream mcp-analysis-result fixture");

    let descriptor_validation = validate_mcp_server_descriptor(&descriptor);
    assert!(
        descriptor_validation.is_valid(),
        "unexpected descriptor issues: {:?}",
        descriptor_validation.issues
    );

    let analysis_validation = validate_mcp_analysis_result(&analysis);
    assert!(
        analysis_validation.is_valid(),
        "unexpected analysis issues: {:?}",
        analysis_validation.issues
    );
}

#[test]
fn upstream_agent_contract_fixtures_validate_through_core_facade() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let request = load_agent_request_envelope_fixture_from_dir(&contracts_dir)
        .expect("load upstream agent-request-envelope fixture");
    let response = load_agent_response_envelope_fixture_from_dir(&contracts_dir)
        .expect("load upstream agent-response-envelope fixture");
    let event = load_agent_event_envelope_fixture_from_dir(&contracts_dir)
        .expect("load upstream agent-event-envelope fixture");

    let request_validation = validate_agent_request_envelope(&request);
    assert!(
        request_validation.is_valid(),
        "unexpected request issues: {:?}",
        request_validation.issues
    );

    let response_validation = validate_agent_response_envelope(&response);
    assert!(
        response_validation.is_valid(),
        "unexpected response issues: {:?}",
        response_validation.issues
    );

    let event_validation = validate_agent_event_envelope(&event);
    assert!(
        event_validation.is_valid(),
        "unexpected event issues: {:?}",
        event_validation.issues
    );
}

#[test]
fn compose_runtime_state_exposes_runtime_mcp_consumers_through_core() {
    let project_dir = create_mcp_project();
    let uri = "elegy://core-mcp/resource/weather-mcp";

    let state = compose_runtime_state(ProjectLocator::Path(project_dir.clone()))
        .expect("project with embedded MCP descriptor should compose");
    let analysis = state
        .analyze_mcp_server(uri)
        .expect("core facade should surface runtime MCP analysis");

    assert_eq!(analysis.server_name, "weather-server");
    assert_eq!(analysis.analyses.len(), 2);

    fs::remove_dir_all(project_dir).expect("cleanup temp project");
}
