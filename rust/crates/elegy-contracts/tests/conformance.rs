use elegy_contracts::{
    default_support_manifest_path, export_contract_bundle,
    load_agent_capability_profile_fixture_from_dir, load_capability_definition_fixture_from_dir,
    load_compatibility_manifest_from_dir, load_consumer_support_manifest,
    load_elegy_configuration_profile_fixture_from_dir,
    load_elegy_configuration_receipt_fixture_from_dir,
    load_elegy_configuration_template_fixture_from_dir, load_elegy_plugin_package_fixture_from_dir,
    load_elegy_plugin_package_v2_fixture_from_dir, load_execution_event_fixture_from_dir,
    load_invocation_request_fixture_from_dir, load_invocation_response_fixture_from_dir,
    load_mcp_analysis_result_fixture_from_dir, load_mcp_server_descriptor_fixture_from_dir,
    load_observation_event_fixture_from_dir, load_observation_session_fixture_from_dir,
    load_observation_summary_fixture_from_dir, load_skill_definition_v2_fixture_from_dir,
    load_skill_discovery_index_fixture_from_dir, load_structured_failure_fixture_from_dir,
    resolve_upstream_contracts_dir, validate_agent_capability_profile,
    validate_capability_definition, validate_elegy_configuration_profile,
    validate_elegy_configuration_receipt, validate_elegy_configuration_template,
    validate_elegy_plugin_package, validate_execution_event, validate_invocation_request,
    validate_invocation_response, validate_mcp_analysis_result, validate_mcp_server_descriptor,
    validate_observation_event, validate_observation_session, validate_observation_summary,
    validate_skill_definition_v2, validate_structured_failure,
    validate_support_manifest_against_upstream, CapabilityApprovalRequirement,
    CapabilityDefinition, CapabilityGovernance, CapabilitySource, CapabilitySourceKind,
    ExecutionEvent, ExecutionEventStatus, ExecutionEventType, InvocationRequest,
    InvocationResponse, InvocationStatus, McpAnalysisResult, McpServerDescriptor, McpToolAnalysis,
    McpToolDefinition, SkillDefinitionV2, SkillGovernance, SkillIdentityV2, SkillImplementation,
    SkillOriginV2, StructuredFailure, StructuredFailureCategory, StructuredFailureCause,
};
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;
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

    assert!(schema_names.contains("skill-definition-v2"));
    assert!(schema_names.contains("elegy-plugin-package-v1"));
    assert!(schema_names.contains("elegy-plugin-package-v2"));
    assert!(schema_names.contains("skill-discovery-index"));
    assert!(schema_names.contains("mcp-tool-definition"));
    assert!(schema_names.contains("mcp-server-descriptor"));
    assert!(schema_names.contains("mcp-analysis-result"));
    assert!(schema_names.contains("capability-definition"));
    assert!(schema_names.contains("agent-capability-profile"));
    assert!(schema_names.contains("agent-manifest"));
    assert!(schema_names.contains("agent-check"));
    assert!(schema_names.contains("agent-discovery"));
    assert!(schema_names.contains("structured-failure"));
    assert!(schema_names.contains("invocation-request"));
    assert!(schema_names.contains("invocation-response"));
    assert!(schema_names.contains("execution-event"));
    assert!(schema_names.contains("observation-event"));
    assert!(schema_names.contains("observation-session"));
    assert!(schema_names.contains("observation-summary"));
}

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
    let definition = load_skill_definition_v2_fixture_from_dir(&contracts_dir)
        .expect("load upstream skill-definition-v2 fixture");

    validate_skill_definition_v2(&definition).expect("fixture should validate");
}

#[test]
fn upstream_elegy_plugin_package_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let package = load_elegy_plugin_package_v2_fixture_from_dir(&contracts_dir)
        .expect("load upstream elegy-plugin-package fixture");

    let validation = validate_elegy_plugin_package(&package);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(package.schema_version, "elegy-plugin-package/v2");
    assert_eq!(
        package.identity.package_id,
        "elegy.demo-configuration-plugin"
    );
    assert_eq!(package.components.configuration_templates.len(), 1);
    assert_eq!(package.components.configuration_profiles.len(), 1);
}

#[test]
fn plugin_package_fixture_helpers_keep_v1_and_v2_separate() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let package_v1 = load_elegy_plugin_package_fixture_from_dir(&contracts_dir)
        .expect("load upstream v1 elegy-plugin-package fixture");
    let package_v2 = load_elegy_plugin_package_v2_fixture_from_dir(&contracts_dir)
        .expect("load upstream v2 elegy-plugin-package fixture");

    assert_eq!(package_v1.schema_version, "elegy-plugin-package/v1");
    assert_eq!(package_v1.identity.package_id, "elegy.demo-plugin");
    assert!(package_v1.components.configuration_templates.is_empty());
    assert!(package_v1.components.configuration_profiles.is_empty());

    assert_eq!(package_v2.schema_version, "elegy-plugin-package/v2");
    assert_eq!(
        package_v2.identity.package_id,
        "elegy.demo-configuration-plugin"
    );
    assert_eq!(package_v2.components.configuration_templates.len(), 1);
    assert_eq!(package_v2.components.configuration_profiles.len(), 1);
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

    assert_eq!(template.template_id, "repo-skill-mirror-minimal");
    assert_eq!(profile.profile_id, "repo-opencode-minimal");
    assert_eq!(
        receipt.mode,
        elegy_contracts::ElegyConfigurationReceiptMode::DryRun
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
fn dedicated_skill_discovery_fixtures_reference_agents_skill_mirrors() {
    let repo_root = resolve_upstream_contracts_dir()
        .parent()
        .expect("repo root from contracts dir")
        .to_path_buf();
    let fixtures = [
        (
            "contracts/fixtures/skill-discovery-index.elegy-memory.json",
            ".agents/skills/elegy-memory/SKILL.md",
        ),
        (
            "contracts/fixtures/skill-discovery-index.elegy-mcp.json",
            ".agents/skills/elegy-mcp/SKILL.md",
        ),
        (
            "contracts/fixtures/skill-discovery-index.elegy-skills.json",
            ".agents/skills/elegy-skills/SKILL.md",
        ),
        (
            "contracts/fixtures/skill-discovery-index.elegy-planning.json",
            ".agents/skills/elegy-planning/SKILL.md",
        ),
        (
            "contracts/fixtures/skill-discovery-index.elegy-mermaid.json",
            ".agents/skills/elegy-mermaid/SKILL.md",
        ),
    ];

    for (fixture_path, expected_vault_ref) in fixtures {
        let path = repo_root.join(fixture_path);
        let index = serde_json::from_str::<elegy_contracts::SkillDiscoveryIndex>(
            &fs::read_to_string(&path).expect("read dedicated skill discovery fixture"),
        )
        .expect("parse dedicated skill discovery fixture");

        assert_eq!(
            index.entries.len(),
            1,
            "fixture {fixture_path} should have one entry"
        );
        let manifest = &index.entries[0].manifest;
        assert_eq!(manifest.vault_ref.as_deref(), Some(expected_vault_ref));
        assert_eq!(
            manifest.source_kind,
            elegy_contracts::SkillSourceKind::Generated
        );
        assert!(repo_root.join(Path::new(expected_vault_ref)).is_file());
    }
}

#[test]
fn validator_matches_phase_two_governance_and_origin_rules() {
    let approval_required = SkillDefinitionV2 {
        skill_format: "elegy-skill-definition".to_string(),
        skill_version: 2,
        identity: SkillIdentityV2 {
            namespace: "example".to_string(),
            name: "approval-required".to_string(),
            version: "0.1.0".to_string(),
            ..SkillIdentityV2::default()
        },
        capabilities: vec![elegy_contracts::SkillCapability {
            id: "approval-required".to_string(),
            name: "Approval Required".to_string(),
            description: "Example capability".to_string(),
            implementation: Some(SkillImplementation {
                execution_type: "subprocess".to_string(),
                executable_name: "example".to_string(),
                arguments: Vec::new(),
            }),
            ..elegy_contracts::SkillCapability::default()
        }],
        governance: Some(SkillGovernance {
            approval_requirement: Some("required".to_string()),
            ..SkillGovernance::default()
        }),
        lifecycle_state: "draft".to_string(),
        ..SkillDefinitionV2::default()
    };

    let approval_error = validate_skill_definition_v2(&approval_required)
        .expect_err("approval-required skills need policy refs");
    assert!(approval_error
        .to_string()
        .contains("require approval must declare at least one policy reference"));

    let dynamic_manual = SkillDefinitionV2 {
        skill_format: "elegy-skill-definition".to_string(),
        skill_version: 2,
        identity: SkillIdentityV2 {
            namespace: "example".to_string(),
            name: "dynamic-manual".to_string(),
            version: "0.1.0".to_string(),
            ..SkillIdentityV2::default()
        },
        capabilities: vec![elegy_contracts::SkillCapability {
            id: "dynamic-manual".to_string(),
            name: "Dynamic Manual".to_string(),
            description: "Example capability".to_string(),
            implementation: Some(SkillImplementation {
                execution_type: "subprocess".to_string(),
                executable_name: "example".to_string(),
                arguments: Vec::new(),
            }),
            ..elegy_contracts::SkillCapability::default()
        }],
        origin: Some(SkillOriginV2 {
            materialization_kind: Some("dynamic".to_string()),
            source_kind: Some("manual".to_string()),
            ..SkillOriginV2::default()
        }),
        lifecycle_state: "draft".to_string(),
        ..SkillDefinitionV2::default()
    };

    let origin_error =
        validate_skill_definition_v2(&dynamic_manual).expect_err("dynamic manual needs source");
    assert!(origin_error
        .to_string()
        .contains("dynamic skills must declare either a source reference"));
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
    assert!(output_path.join("compatibility-manifest.json").is_file());
    assert!(output_path.join("compatibility-matrix.json").is_file());
    assert!(output_path.join("canonical-workflow.schema.json").is_file());
    assert!(output_path.join("agent-manifest.schema.json").is_file());
    assert!(output_path.join("agent-check.schema.json").is_file());
    assert!(output_path.join("agent-discovery.schema.json").is_file());
    assert!(output_path
        .join("capability-definition.schema.json")
        .is_file());
    assert!(output_path
        .join("elegy-plugin-package-v1.schema.json")
        .is_file());
    assert!(output_path
        .join("elegy-plugin-package-v2.schema.json")
        .is_file());
    assert!(output_path.join("structured-failure.schema.json").is_file());
    assert!(output_path.join("invocation-request.schema.json").is_file());
    assert!(output_path
        .join("invocation-response.schema.json")
        .is_file());
    assert!(output_path.join("execution-event.schema.json").is_file());
    assert!(output_path.join("observation-event.schema.json").is_file());
    assert!(output_path
        .join("observation-session.schema.json")
        .is_file());
    assert!(output_path
        .join("observation-summary.schema.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("capability-definition.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("elegy-plugin-package-v1.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("elegy-plugin-package-v2.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("configuration")
        .join("demo-template.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("configuration")
        .join("demo-profile.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("configuration")
        .join("assets")
        .join("demo.txt")
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
    assert!(output_path
        .join("fixtures")
        .join("mcp-parity-expected.json")
        .is_file());

    {
        let archive_file = fs::File::open(&archive_path).expect("open bundle archive");
        let mut archive = ZipArchive::new(archive_file).expect("read bundle archive");
        assert!(archive.by_name("compatibility-manifest.json").is_ok());
        assert!(archive.by_name("agent-manifest.schema.json").is_ok());
        assert!(archive.by_name("agent-check.schema.json").is_ok());
        assert!(archive.by_name("agent-discovery.schema.json").is_ok());
        assert!(archive.by_name("capability-definition.schema.json").is_ok());
        assert!(archive
            .by_name("elegy-plugin-package-v1.schema.json")
            .is_ok());
        assert!(archive
            .by_name("elegy-plugin-package-v2.schema.json")
            .is_ok());
        assert!(archive.by_name("structured-failure.schema.json").is_ok());
        assert!(archive.by_name("invocation-request.schema.json").is_ok());
        assert!(archive.by_name("invocation-response.schema.json").is_ok());
        assert!(archive.by_name("execution-event.schema.json").is_ok());
        assert!(archive.by_name("observation-event.schema.json").is_ok());
        assert!(archive.by_name("observation-session.schema.json").is_ok());
        assert!(archive.by_name("observation-summary.schema.json").is_ok());
        assert!(archive
            .by_name("fixtures/capability-definition.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/elegy-plugin-package-v1.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/elegy-plugin-package-v2.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/configuration/demo-template.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/configuration/demo-profile.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/configuration/assets/demo.txt")
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
        assert!(archive.by_name("fixtures/mcp-parity-expected.json").is_ok());
    }

    fs::remove_dir_all(&temp_root).expect("remove temp export root");
}
