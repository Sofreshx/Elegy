use elegy_contracts::{
    default_support_manifest_path, export_contract_bundle,
    load_capability_definition_fixture_from_dir, load_compatibility_manifest_from_dir,
    load_consumer_support_manifest, load_mcp_analysis_result_fixture_from_dir,
    load_mcp_server_descriptor_fixture_from_dir, load_skill_definition_fixture_from_dir,
    load_skill_discovery_index_fixture_from_dir, load_structured_failure_fixture_from_dir,
    load_invocation_request_fixture_from_dir, load_invocation_response_fixture_from_dir,
    resolve_upstream_contracts_dir, validate_capability_definition,
    validate_invocation_request, validate_invocation_response, validate_mcp_analysis_result,
    validate_mcp_server_descriptor, validate_skill_definition, validate_structured_failure,
    validate_support_manifest_against_upstream,
    CapabilityApprovalRequirement, CapabilityDefinition, CapabilityGovernance, CapabilitySource,
    CapabilitySourceKind, InvocationRequest, InvocationResponse, InvocationStatus,
    McpAnalysisResult, McpServerDescriptor, McpToolAnalysis, McpToolDefinition,
    SkillApprovalRequirement, SkillDefinition, SkillGovernanceMetadata,
    SkillMaterializationKind, SkillOrigin, SkillSourceKind, StructuredFailure,
    StructuredFailureCause, StructuredFailureCategory,
};
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};
use zip::ZipArchive;

#[test]
fn upstream_bundle_contains_supported_schema_entries() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let upstream = load_compatibility_manifest_from_dir(&contracts_dir)
        .expect("load upstream compatibility manifest");
    let support = load_consumer_support_manifest(&default_support_manifest_path())
        .expect("load local support manifest");

    validate_support_manifest_against_upstream(&support, &upstream)
        .expect("support manifest should match upstream bundle");

    let schema_names = upstream
        .schemas
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<BTreeSet<_>>();

    assert!(schema_names.contains("skill-definition"));
    assert!(schema_names.contains("skill-discovery-index"));
    assert!(schema_names.contains("mcp-tool-definition"));
    assert!(schema_names.contains("mcp-server-descriptor"));
    assert!(schema_names.contains("mcp-analysis-result"));
    assert!(schema_names.contains("capability-definition"));
    assert!(schema_names.contains("structured-failure"));
    assert!(schema_names.contains("invocation-request"));
    assert!(schema_names.contains("invocation-response"));
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
fn upstream_capability_definition_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let definition = load_capability_definition_fixture_from_dir(&contracts_dir)
        .expect("load upstream capability-definition fixture");

    let validation = validate_capability_definition(&definition);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(definition.id, "cap.example.echo");
    assert_eq!(definition.display_name, "Example Echo Capability");
}

#[test]
fn upstream_skill_definition_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let definition = load_skill_definition_fixture_from_dir(&contracts_dir)
        .expect("load upstream skill-definition fixture");

    let validation = validate_skill_definition(&definition);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );
}

#[test]
fn upstream_skill_discovery_fixture_round_trips_as_projection() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let index = load_skill_discovery_index_fixture_from_dir(&contracts_dir)
        .expect("load upstream skill-discovery fixture");

    assert_eq!(index.schema_version, 1);
    assert_eq!(index.entries.len(), 1);
    assert_eq!(index.entries[0].skill_id, "example-skill");
    assert_eq!(index.entries[0].manifest.id, "example-skill");

    let json = serde_json::to_string(&index).expect("serialize discovery index");
    let reparsed = serde_json::from_str(&json).expect("deserialize discovery index");

    assert_eq!(index, reparsed);
}

#[test]
fn validator_matches_phase_two_governance_and_origin_rules() {
    let approval_required = SkillDefinition {
        id: "skill.example".to_string(),
        name: "Example skill".to_string(),
        governance: SkillGovernanceMetadata {
            approval_requirement: SkillApprovalRequirement::Required,
            ..SkillGovernanceMetadata::default()
        },
        ..SkillDefinition::default()
    };

    let approval_validation = validate_skill_definition(&approval_required);
    assert!(approval_validation.issues.contains(
        &"Skills that require approval must declare at least one policy reference.".to_string()
    ));

    let dynamic_manual = SkillDefinition {
        id: "skill.dynamic".to_string(),
        name: "Dynamic skill".to_string(),
        origin: SkillOrigin {
            materialization_kind: SkillMaterializationKind::Dynamic,
            source_kind: SkillSourceKind::Manual,
            ..SkillOrigin::default()
        },
        ..SkillDefinition::default()
    };

    let origin_validation = validate_skill_definition(&dynamic_manual);
    assert!(origin_validation.issues.contains(
        &"Dynamic skills must declare either a source reference or a non-manual source kind."
            .to_string()
    ));
}

#[test]
fn capability_validator_rejects_missing_policy_refs_and_missing_source_refs() {
    let invalid = CapabilityDefinition {
        id: "cap.invalid".to_string(),
        display_name: "Invalid capability".to_string(),
        version: "1.0.0".to_string(),
        governance: CapabilityGovernance {
            approval_requirement: CapabilityApprovalRequirement::Required,
            ..CapabilityGovernance::default()
        },
        source: CapabilitySource {
            source_kind: CapabilitySourceKind::Generated,
            source_ref: None,
            artifact_ref: None,
        },
        ..CapabilityDefinition::default()
    };

    let validation = validate_capability_definition(&invalid);
    assert!(validation.issues.contains(
        &"Capabilities that require approval must declare at least one policy reference."
            .to_string()
    ));
    assert!(validation.issues.contains(
        &"Imported, generated, or projected capabilities must declare a sourceRef or artifactRef."
            .to_string()
    ));
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
    assert!(validation.issues.contains(
        &"Structured failure details must be a JSON object when provided.".to_string()
    ));
    assert!(validation
        .issues
        .contains(&"Structured failure cause code must not be blank.".to_string()));
    assert!(validation.issues.contains(
        &"Structured failure cause message must not be blank.".to_string()
    ));
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
    assert!(output_path.join("compatibility-manifest.json").is_file());
    assert!(output_path.join("compatibility-matrix.json").is_file());
    assert!(output_path.join("canonical-workflow.schema.json").is_file());
    assert!(output_path
        .join("capability-definition.schema.json")
        .is_file());
    assert!(output_path.join("structured-failure.schema.json").is_file());
    assert!(output_path.join("invocation-request.schema.json").is_file());
    assert!(output_path.join("invocation-response.schema.json").is_file());
    assert!(output_path
        .join("fixtures")
        .join("capability-definition.minimal.json")
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
        .join("mcp-parity-expected.json")
        .is_file());

    {
        let archive_file = fs::File::open(&archive_path).expect("open bundle archive");
        let mut archive = ZipArchive::new(archive_file).expect("read bundle archive");
        assert!(archive.by_name("compatibility-manifest.json").is_ok());
        assert!(archive.by_name("capability-definition.schema.json").is_ok());
        assert!(archive.by_name("structured-failure.schema.json").is_ok());
        assert!(archive.by_name("invocation-request.schema.json").is_ok());
        assert!(archive.by_name("invocation-response.schema.json").is_ok());
        assert!(archive
            .by_name("fixtures/capability-definition.minimal.json")
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
        assert!(archive.by_name("fixtures/mcp-parity-expected.json").is_ok());
    }

    fs::remove_dir_all(&temp_root).expect("remove temp export root");
}
