use elegy_contracts::{
    export_contract_bundle, load_agent_capability_profile_fixture_from_dir,
    load_elegy_configuration_profile_fixture_from_dir,
    load_elegy_configuration_receipt_fixture_from_dir,
    load_elegy_configuration_template_fixture_from_dir,
    load_execution_event_fixture_from_dir, load_invocation_request_fixture_from_dir,
    load_invocation_response_fixture_from_dir, load_mcp_analysis_result_fixture_from_dir,
    load_mcp_server_descriptor_fixture_from_dir, load_observation_event_fixture_from_dir,
    load_observation_session_fixture_from_dir, load_observation_summary_fixture_from_dir,
    load_structured_failure_fixture_from_dir, parse_agent_skill_frontmatter,
    resolve_upstream_contracts_dir, validate_agent_capability_profile,
    validate_agent_skill_frontmatter, validate_elegy_configuration_profile,
    validate_elegy_configuration_receipt, validate_elegy_configuration_template,
    validate_elegy_plugin_v1, validate_execution_event, validate_invocation_request,
    validate_invocation_response, validate_mcp_analysis_result, validate_mcp_server_descriptor,
    validate_observation_event, validate_observation_session, validate_observation_summary,
    validate_structured_failure, ElegyPluginV1, ExecutionEvent, ExecutionEventStatus,
    ExecutionEventType, InvocationRequest, InvocationResponse, InvocationStatus, McpAnalysisResult,
    McpServerDescriptor, McpToolAnalysis, McpToolDefinition, StructuredFailure,
    StructuredFailureCategory, StructuredFailureCause,
};
use std::env;
use std::fs;

use std::process;
use std::time::{SystemTime, UNIX_EPOCH};
use zip::ZipArchive;

#[test]
fn upstream_agent_capability_profile_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let profile = load_agent_capability_profile_fixture_from_dir(&contracts_dir)
        .expect("load upstream agent-capability-profile fixture");

    let validation = validate_agent_capability_profile(&profile);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(profile.profile_id, "generic-agent-host");
    assert!(profile.always_include_router);
}

#[test]
fn upstream_structured_failure_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let failure = load_structured_failure_fixture_from_dir(&contracts_dir)
        .expect("load upstream structured-failure fixture");

    let validation = validate_structured_failure(&failure);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(failure.code, "capability.invalid-input");
    assert_eq!(failure.category, StructuredFailureCategory::InvalidInput);
}

#[test]
fn upstream_invocation_request_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let request = load_invocation_request_fixture_from_dir(&contracts_dir)
        .expect("load upstream invocation-request fixture");

    let validation = validate_invocation_request(&request);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(request.request_id, "invoke-req-1");
    assert_eq!(request.capability_id, "cap.example.echo");
}

#[test]
fn upstream_invocation_response_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let response = load_invocation_response_fixture_from_dir(&contracts_dir)
        .expect("load upstream invocation-response fixture");

    let validation = validate_invocation_response(&response);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(response.request_id, "invoke-req-1");
    assert_eq!(response.status, InvocationStatus::Completed);
}

#[test]
fn upstream_execution_event_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let event = load_execution_event_fixture_from_dir(&contracts_dir)
        .expect("load upstream execution-event fixture");

    let validation = validate_execution_event(&event);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(event.event_id, "exec-event-1");
    assert_eq!(event.event_type, ExecutionEventType::Accepted);
    assert_eq!(event.status, ExecutionEventStatus::Pending);
}

#[test]
fn upstream_observation_event_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let event = load_observation_event_fixture_from_dir(&contracts_dir)
        .expect("load upstream observation-event fixture");

    let validation = validate_observation_event(&event);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(event.event_id, "obs-event-1");
}

#[test]
fn upstream_observation_summary_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let summary = load_observation_summary_fixture_from_dir(&contracts_dir)
        .expect("load upstream observation-summary fixture");

    let validation = validate_observation_summary(&summary);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(summary.observation_count, 1);
}

#[test]
fn upstream_observation_session_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let session = load_observation_session_fixture_from_dir(&contracts_dir)
        .expect("load upstream observation-session fixture");

    let validation = validate_observation_session(&session);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(session.artifact_kind, "observation-session");
}

#[test]
fn upstream_elegy_configuration_fixtures_are_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let template = load_elegy_configuration_template_fixture_from_dir(&contracts_dir)
        .expect("load upstream configuration template fixture");
    let profile = load_elegy_configuration_profile_fixture_from_dir(&contracts_dir)
        .expect("load upstream configuration profile fixture");
    let receipt = load_elegy_configuration_receipt_fixture_from_dir(&contracts_dir)
        .expect("load upstream configuration receipt fixture");

    let template_validation = validate_elegy_configuration_template(&template);
    assert!(
        template_validation.is_valid(),
        "unexpected template issues: {:?}",
        template_validation.issues
    );

    let profile_validation = validate_elegy_configuration_profile(&profile);
    assert!(
        profile_validation.is_valid(),
        "unexpected profile issues: {:?}",
        profile_validation.issues
    );

    let receipt_validation = validate_elegy_configuration_receipt(&receipt);
    assert!(
        receipt_validation.is_valid(),
        "unexpected receipt issues: {:?}",
        receipt_validation.issues
    );

    assert_eq!(template.template_id, "repo-opencode-agentic-minimal");
    assert_eq!(profile.profile_id, "repo-opencode-minimal");
    assert_eq!(
        receipt.mode,
        elegy_contracts::ElegyConfigurationReceiptMode::DryRun
    );
}

#[test]
fn invocation_validators_reject_missing_fields_and_missing_failure() {
    let invalid_request = InvocationRequest {
        request_id: String::new(),
        capability_id: String::new(),
        input: serde_json::json!("bad"),
        ..InvocationRequest::default()
    };

    let request_validation = validate_invocation_request(&invalid_request);
    assert!(request_validation
        .issues
        .contains(&"Invocation request must declare a requestId.".to_string()));
    assert!(request_validation
        .issues
        .contains(&"Invocation request must declare a capabilityId.".to_string()));
    assert!(request_validation
        .issues
        .contains(&"Invocation request input must be a JSON object.".to_string()));
    assert!(request_validation
        .issues
        .contains(&"Invocation request context must declare a correlationId.".to_string()));
    assert!(request_validation
        .issues
        .contains(&"Invocation request context must declare an executionId.".to_string()));
    assert!(request_validation
        .issues
        .contains(&"Invocation request context must declare requestedAt.".to_string()));

    let invalid_response = InvocationResponse {
        request_id: String::new(),
        execution_id: String::new(),
        status: InvocationStatus::Failed,
        failure: None,
        ..InvocationResponse::default()
    };

    let response_validation = validate_invocation_response(&invalid_response);
    assert!(response_validation
        .issues
        .contains(&"Invocation response must declare a requestId.".to_string()));
    assert!(response_validation
        .issues
        .contains(&"Invocation response must declare an executionId.".to_string()));
    assert!(response_validation.issues.contains(
        &"Failed or cancelled invocation responses must include a structured failure.".to_string()
    ));
}

#[test]
fn execution_event_validator_rejects_missing_fields_and_missing_failure() {
    let invalid = ExecutionEvent {
        event_id: String::new(),
        request_id: String::new(),
        execution_id: String::new(),
        sequence: 0,
        timestamp: String::new(),
        event_type: ExecutionEventType::Failed,
        status: ExecutionEventStatus::Failed,
        failure: None,
        ..ExecutionEvent::default()
    };

    let validation = validate_execution_event(&invalid);
    assert!(validation
        .issues
        .contains(&"Execution event must declare an eventId.".to_string()));
    assert!(validation
        .issues
        .contains(&"Execution event must declare a requestId.".to_string()));
    assert!(validation
        .issues
        .contains(&"Execution event must declare an executionId.".to_string()));
    assert!(validation
        .issues
        .contains(&"Execution event sequence must be greater than zero.".to_string()));
    assert!(validation
        .issues
        .contains(&"Execution event must declare a timestamp.".to_string()));
    assert!(validation.issues.contains(
        &"Failed or cancelled execution events must include a structured failure.".to_string()
    ));
}

#[test]
fn structured_failure_validator_rejects_blank_fields_and_non_object_details() {
    let invalid = StructuredFailure {
        code: String::new(),
        message: String::new(),
        correlation_id: Some(String::new()),
        details: Some(serde_json::json!(7)),
        cause: Some(StructuredFailureCause {
            code: String::new(),
            message: String::new(),
        }),
        ..StructuredFailure::default()
    };

    let validation = validate_structured_failure(&invalid);
    assert!(validation
        .issues
        .contains(&"Structured failure code must not be blank.".to_string()));
    assert!(validation
        .issues
        .contains(&"Structured failure message must not be blank.".to_string()));
    assert!(validation.issues.contains(
        &"Structured failure correlationId must not be blank when provided.".to_string()
    ));
    assert!(validation
        .issues
        .contains(&"Structured failure details must be a JSON object when provided.".to_string()));
    assert!(validation
        .issues
        .contains(&"Structured failure cause code must not be blank.".to_string()));
    assert!(validation
        .issues
        .contains(&"Structured failure cause message must not be blank.".to_string()));
}

#[test]
fn upstream_mcp_server_descriptor_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let descriptor = load_mcp_server_descriptor_fixture_from_dir(&contracts_dir)
        .expect("load upstream mcp-server-descriptor fixture");

    let validation = validate_mcp_server_descriptor(&descriptor);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(descriptor.server_name, "weather-server");
    assert_eq!(descriptor.tools.len(), 1);
    assert_eq!(descriptor.tools[0].name, "get-weather");
}

#[test]
fn upstream_mcp_analysis_result_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let analysis = load_mcp_analysis_result_fixture_from_dir(&contracts_dir)
        .expect("load upstream mcp-analysis-result fixture");

    let validation = validate_mcp_analysis_result(&analysis);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(analysis.server_name, "weather-server");
    assert_eq!(analysis.analyses.len(), 1);
    assert_eq!(analysis.analyses[0].tool.name, "get-weather");
    assert_eq!(
        analysis.analyses[0].extracted_triggers[0].pattern,
        "get weather"
    );
}

#[test]
fn mcp_validators_reject_duplicate_and_inconsistent_entries() {
    let descriptor = McpServerDescriptor {
        server_name: "duplicate-server".to_string(),
        tools: vec![
            McpToolDefinition {
                name: "get-weather".to_string(),
                ..McpToolDefinition::default()
            },
            McpToolDefinition {
                name: "get-weather".to_string(),
                ..McpToolDefinition::default()
            },
        ],
        ..McpServerDescriptor::default()
    };

    let descriptor_validation = validate_mcp_server_descriptor(&descriptor);
    assert!(descriptor_validation
        .issues
        .contains(&"MCP server descriptor tool names must be unique.".to_string()));

    let analysis = McpAnalysisResult {
        server_name: "duplicate-server".to_string(),
        analyses: vec![McpToolAnalysis {
            tool: McpToolDefinition {
                name: "get-weather".to_string(),
                input_schema: None,
                ..McpToolDefinition::default()
            },
            has_valid_schema: true,
            ..McpToolAnalysis::default()
        }],
    };

    let analysis_validation = validate_mcp_analysis_result(&analysis);
    assert!(analysis_validation.issues.contains(
        &"MCP analysis entries marked as having a valid schema must include an input schema."
            .to_string()
    ));
}

#[test]
fn export_contract_bundle_creates_expected_directory_and_archive() {
    let temp_root = env::temp_dir().join(format!(
        "elegy-contracts-export-{}-{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    ));
    let output_path = temp_root.join("contracts");
    let archive_path = temp_root.join("distribution").join("bundle.zip");

    let export = export_contract_bundle(Some(&output_path), true, Some(&archive_path))
        .expect("export contracts bundle");

    assert_eq!(export.output_path, output_path);
    assert_eq!(export.archive_path.as_deref(), Some(archive_path.as_path()));
    assert!(output_path
        .join("schemas")
        .join("observation-session.schema.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("structured-failure.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("invocation-request.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("invocation-response.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("execution-event.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("observation-event.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("observation-session.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("observation-summary.minimal.json")
        .is_file());

    {
        let archive_file = fs::File::open(&archive_path).expect("open bundle archive");
        let mut archive = ZipArchive::new(archive_file).expect("read bundle archive");
        assert!(archive
            .by_name("schemas/observation-session.schema.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/structured-failure.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/invocation-request.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/invocation-response.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/execution-event.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/observation-event.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/observation-session.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/observation-summary.minimal.json")
            .is_ok());
    }

    fs::remove_dir_all(&temp_root).expect("remove temp export root");
}

// ── elegy-plugin/v1 conformance ──────────────────────────────────────────

#[test]
fn elegy_plugin_v1_minimal_is_valid() {
    let json = r#"{
        "schemaVersion": "elegy-plugin/v1",
        "name": "minimal",
        "version": "1.0.0",
        "description": "A minimal plugin.",
        "skills": "./skills"
    }"#;
    let plugin: ElegyPluginV1 = serde_json::from_str(json).expect("deserialize minimal plugin");
    let validation = validate_elegy_plugin_v1(&plugin);
    assert!(validation.is_valid(), "issues: {:?}", validation.issues);
}

#[test]
fn elegy_plugin_v1_skill_only_is_valid() {
    let json = r#"{
        "schemaVersion": "elegy-plugin/v1",
        "name": "skill-only",
        "version": "1.0.0",
        "description": "A skill-only plugin.",
        "skills": "./skills"
    }"#;
    let plugin: ElegyPluginV1 = serde_json::from_str(json).expect("deserialize skill-only plugin");
    let validation = validate_elegy_plugin_v1(&plugin);
    assert!(validation.is_valid(), "issues: {:?}", validation.issues);
}

#[test]
fn elegy_plugin_v1_mcp_only_is_valid() {
    let json = r#"{
        "schemaVersion": "elegy-plugin/v1",
        "name": "mcp-only",
        "version": "1.0.0",
        "description": "An MCP-only plugin.",
        "mcpServers": "./mcp"
    }"#;
    let plugin: ElegyPluginV1 = serde_json::from_str(json).expect("deserialize mcp-only plugin");
    let validation = validate_elegy_plugin_v1(&plugin);
    assert!(validation.is_valid(), "issues: {:?}", validation.issues);
}

#[test]
fn elegy_plugin_v1_rejects_bad_paths() {
    let cases = vec![
        ("../escape", "traversal"),
        ("/absolute", "absolute unix"),
        ("\\\\server\\share", "UNC path"),
        ("C:\\windows", "windows drive"),
        ("", "empty"),
    ];
    for (path, label) in cases {
        let json = serde_json::json!({
            "schemaVersion": "elegy-plugin/v1",
            "name": "test",
            "version": "1.0.0",
            "description": "testing",
            "skills": path
        });
        let plugin: ElegyPluginV1 =
            serde_json::from_value(json.clone()).expect(&format!("deserialize {} plugin", label));
        let validation = validate_elegy_plugin_v1(&plugin);
        assert!(
            !validation.is_valid(),
            "should reject {} path '{}', but got valid. Issues: {:?}",
            label,
            path,
            validation.issues
        );
    }
}

#[test]
fn elegy_plugin_v1_rejects_missing_required() {
    let missing_name = r#"{
        "schemaVersion": "elegy-plugin/v1",
        "version": "1.0.0",
        "description": "Missing name."
    }"#;
    let err = serde_json::from_str::<ElegyPluginV1>(missing_name);
    assert!(err.is_err(), "should reject missing 'name' field");

    let missing_desc = r#"{
        "schemaVersion": "elegy-plugin/v1",
        "name": "test",
        "version": "1.0.0"
    }"#;
    let err = serde_json::from_str::<ElegyPluginV1>(missing_desc);
    assert!(err.is_err(), "should reject missing 'description' field");
}

#[test]
fn elegy_plugin_v1_extensions_round_trip() {
    let json = r#"{
        "schemaVersion": "elegy-plugin/v1",
        "name": "ext-test",
        "version": "1.0.0",
        "description": "Testing extension round-trip.",
        "extensions": {
            "com.example.a": { "version": 1 },
            "org.other.b": { "enabled": true, "config": { "key": "val" } }
        }
    }"#;
    let plugin: ElegyPluginV1 =
        serde_json::from_str(json).expect("deserialize plugin with extensions");
    let round_trip = serde_json::to_string_pretty(&plugin).expect("serialize back");
    let plugin2: ElegyPluginV1 = serde_json::from_str(&round_trip).expect("deserialize again");
    assert_eq!(plugin.extensions, plugin2.extensions);
}

#[test]
fn elegy_plugin_v1_rejects_invalid_name_pattern() {
    let cases = vec!["INVALID", "123start", "_underscore", "CamelCase", ""];
    for name in cases {
        let json = serde_json::json!({
            "schemaVersion": "elegy-plugin/v1",
            "name": name,
            "version": "1.0.0",
            "description": "testing"
        });
        // Deserialization may succeed or fail depending on whether name is empty
        if let Ok(plugin) = serde_json::from_value::<ElegyPluginV1>(json) {
            let validation = validate_elegy_plugin_v1(&plugin);
            assert!(
                !validation.is_valid(),
                "should reject invalid name '{}', but got valid. Issues: {:?}",
                name,
                validation.issues
            );
        }
        // If deserialization fails, that's also acceptable (serde catches the empty case)
    }
}

#[test]
fn elegy_plugin_v1_rejects_invalid_semver() {
    let cases = vec!["not-semver", "1", "1.0", "v1.0.0", "01.1.1", "1.02.1"];
    for version in cases {
        let json = serde_json::json!({
            "schemaVersion": "elegy-plugin/v1",
            "name": "test",
            "version": version,
            "description": "testing"
        });
        let plugin: ElegyPluginV1 = serde_json::from_value(json)
            .expect(&format!("deserialize plugin with version '{}'", version));
        let validation = validate_elegy_plugin_v1(&plugin);
        assert!(
            !validation.is_valid(),
            "should reject invalid SemVer '{}', but got valid. Issues: {:?}",
            version,
            validation.issues
        );
    }
}

// ── Additional conformance tests ─────────────────────────────────────────

#[test]
fn elegy_plugin_v1_description_trimmed() {
    let json = r#"{
        "schemaVersion": "elegy-plugin/v1",
        "name": "ws-desc",
        "version": "1.0.0",
        "description": "   ",
        "skills": "./skills",
        "mcpServers": "./mcp"
    }"#;
    let plugin: ElegyPluginV1 = serde_json::from_str(json).expect("deserialize");
    let validation = validate_elegy_plugin_v1(&plugin);
    assert!(
        validation
            .issues
            .iter()
            .any(|i| i.contains("only whitespace")),
        "should flag whitespace-only description. Issues: {:?}",
        validation.issues
    );
}

#[test]
fn elegy_plugin_v1_requires_at_least_one_component() {
    let json = r#"{
        "schemaVersion": "elegy-plugin/v1",
        "name": "no-components",
        "version": "1.0.0",
        "description": "Plugin with no skills, tools, or mcpServers."
    }"#;
    let plugin: ElegyPluginV1 = serde_json::from_str(json).expect("deserialize");
    let validation = validate_elegy_plugin_v1(&plugin);
    assert!(
        validation
            .issues
            .iter()
            .any(|i| i.contains("At least one of skills")),
        "should require at least one component. Issues: {:?}",
        validation.issues
    );
}

#[test]
fn elegy_plugin_v1_author_name_required() {
    let json = r#"{
        "schemaVersion": "elegy-plugin/v1",
        "name": "author-test",
        "version": "1.0.0",
        "description": "Testing author validation.",
        "author": {
            "name": "",
            "email": "test@example.com"
        },
        "skills": "./skills"
    }"#;
    let plugin: ElegyPluginV1 = serde_json::from_str(json).expect("deserialize");
    let validation = validate_elegy_plugin_v1(&plugin);
    assert!(
        validation
            .issues
            .iter()
            .any(|i| i.contains("author.name must not be empty")),
        "should reject empty author name. Issues: {:?}",
        validation.issues
    );
}

#[test]
fn elegy_plugin_v1_rejects_null_roots() {
    // serde_json with Option<String> accepts null and maps it to None,
    // so deserialization should succeed. The validation should then catch
    // that no skills/tools/mcpServers are declared.
    let json = serde_json::json!({
        "schemaVersion": "elegy-plugin/v1",
        "name": "null-skills",
        "version": "1.0.0",
        "description": "Has skills: null",
        "skills": null
    });
    let plugin: Result<ElegyPluginV1, _> = serde_json::from_value(json);
    assert!(
        plugin.is_ok(),
        "Option<String> with null should deserialize to None"
    );
    let plugin = plugin.unwrap();
    assert!(plugin.skills.is_none(), "skills should be None");
    let validation = validate_elegy_plugin_v1(&plugin);
    assert!(
        validation
            .issues
            .iter()
            .any(|i| i.contains("At least one of skills")),
        "should require at least one component. Issues: {:?}",
        validation.issues
    );
}

#[test]
fn elegy_skill_frontmatter_round_trip() {
    let content = r#"---
name: my-test-skill
description: A test skill
version: 1.0.0
tags:
  - test
  - demo
---
# Body content
This is the skill body.
"#;
    let (frontmatter, body) = parse_agent_skill_frontmatter(content).expect("parse frontmatter");
    assert_eq!(frontmatter.name, "my-test-skill");
    assert_eq!(frontmatter.description, "A test skill");
    assert_eq!(frontmatter.version.as_deref(), Some("1.0.0"));
    assert_eq!(
        frontmatter.tags,
        Some(vec!["test".to_string(), "demo".to_string()])
    );
    assert_eq!(body.trim(), "# Body content\nThis is the skill body.");

    // Validate
    let issues = validate_agent_skill_frontmatter(&frontmatter);
    assert!(
        issues.is_empty(),
        "valid frontmatter should have no issues: {:?}",
        issues
    );
}

#[test]
fn elegy_plugin_v1_accepts_valid_semver() {
    let cases = vec![
        "0.1.0",
        "1.0.0",
        "10.20.30",
        "1.0.0-alpha",
        "1.0.0-alpha.1",
        "1.0.0+build",
        "1.0.0-alpha+build",
    ];
    for version in cases {
        let json = serde_json::json!({
            "schemaVersion": "elegy-plugin/v1",
            "name": "test",
            "version": version,
            "description": "testing",
            "skills": "./skills"
        });
        let plugin: ElegyPluginV1 = serde_json::from_value(json)
            .expect(&format!("deserialize plugin with version '{}'", version));
        let validation = validate_elegy_plugin_v1(&plugin);
        assert!(
            validation.is_valid(),
            "should accept valid SemVer '{}', but got invalid. Issues: {:?}",
            version,
            validation.issues
        );
    }
}

#[test]
fn elegy_plugin_v1_accepts_valid_names() {
    let cases = vec!["elegy", "my-plugin", "test123", "a-b-c", "x"];
    for name in cases {
        let json = serde_json::json!({
            "schemaVersion": "elegy-plugin/v1",
            "name": name,
            "version": "1.0.0",
            "description": "testing",
            "skills": "./skills"
        });
        let plugin: ElegyPluginV1 = serde_json::from_value(json)
            .expect(&format!("deserialize plugin with name '{}'", name));
        let validation = validate_elegy_plugin_v1(&plugin);
        assert!(
            validation.is_valid(),
            "should accept valid name '{}', but got invalid. Issues: {:?}",
            name,
            validation.issues
        );
    }
}
