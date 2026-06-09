use elegy_contracts::{
    builtin_capability_definitions, default_support_manifest_path, export_contract_bundle,
    load_agent_capability_profile_fixture_from_dir, load_capability_definition_fixture_from_dir,
    load_compatibility_manifest_from_dir, load_consumer_support_manifest,
    load_elegy_configuration_profile_fixture_from_dir,
    load_elegy_configuration_receipt_fixture_from_dir,
    load_elegy_configuration_template_fixture_from_dir, load_elegy_plugin_package_fixture_from_dir,
    load_execution_event_fixture_from_dir, load_invocation_request_fixture_from_dir,
    load_invocation_response_fixture_from_dir, load_mcp_analysis_result_fixture_from_dir,
    load_mcp_server_descriptor_fixture_from_dir, load_observation_event_fixture_from_dir,
    load_observation_session_fixture_from_dir, load_observation_summary_fixture_from_dir,
    load_skill_definition_v2_fixture_from_dir, load_skill_discovery_index_fixture_from_dir,
    load_structured_failure_fixture_from_dir, resolve_upstream_contracts_dir,
    validate_agent_capability_profile, validate_capability_definition,
    validate_elegy_configuration_profile, validate_elegy_configuration_receipt,
    validate_elegy_configuration_template, validate_elegy_plugin_package, validate_execution_event,
    validate_invocation_request, validate_invocation_response, validate_mcp_analysis_result,
    validate_mcp_server_descriptor, validate_observation_event, validate_observation_session,
    validate_observation_summary, validate_skill_definition_v2, validate_structured_failure,
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

    assert!(schema_names.contains("skill"));
    assert!(schema_names.contains("elegy-plugin-package"));
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
        .expect("load upstream skill fixture");

    validate_skill_definition_v2(&definition).expect("fixture should validate");
}

#[test]
fn builtin_skill_capability_projections_are_semantically_valid() {
    let definitions = builtin_capability_definitions()
        .expect("built-in capability definitions should project cleanly");

    assert!(
        !definitions.is_empty(),
        "expected at least one built-in capability definition"
    );

    for definition in definitions {
        let validation = validate_capability_definition(&definition);
        assert!(
            validation.is_valid(),
            "unexpected issues for {}: {:?}",
            definition.id,
            validation.issues
        );
    }
}

#[test]
fn upstream_elegy_plugin_package_fixture_is_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let package = load_elegy_plugin_package_fixture_from_dir(&contracts_dir)
        .expect("load upstream elegy-plugin-package fixture");

    let validation = validate_elegy_plugin_package(&package);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(package.schema_version, "elegy-plugin-package/v1");
    assert_eq!(
        package.identity.package_id,
        "elegy.demo-configuration-plugin"
    );
    assert_eq!(package.components.configuration_templates.len(), 1);
    assert_eq!(package.components.configuration_profiles.len(), 1);
}

#[test]
fn plugin_package_validator_rejects_invalid_uri_fields() {
    let mut package = load_elegy_plugin_package_fixture_from_dir(&resolve_upstream_contracts_dir())
        .expect("load upstream elegy-plugin-package fixture");

    package.metadata = Some(elegy_contracts::ElegyPluginPackageMetadata {
        homepage: Some("not-a-uri".to_string()),
        documentation_uri: Some("also-not-a-uri".to_string()),
        ..package.metadata.unwrap_or_default()
    });
    package.publishing = Some(elegy_contracts::ElegyPluginPackagePublishingMetadata {
        source_repository: Some("still-not-a-uri".to_string()),
        ..package.publishing.unwrap_or_default()
    });

    let validation = validate_elegy_plugin_package(&package);
    assert!(validation
        .issues
        .contains(&"metadata.homepage must be a valid URI.".to_string()));
    assert!(validation
        .issues
        .contains(&"metadata.documentationUri must be a valid URI.".to_string()));
    assert!(validation
        .issues
        .contains(&"publishing.sourceRepository must be a valid URI.".to_string()));
}

#[test]
fn plugin_package_fixture_has_unified_schema_version() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let package = load_elegy_plugin_package_fixture_from_dir(&contracts_dir)
        .expect("load upstream elegy-plugin-package fixture");

    assert_eq!(package.schema_version, "elegy-plugin-package/v1");
    assert_eq!(
        package.identity.package_id,
        "elegy.demo-configuration-plugin"
    );
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
            "contracts/fixtures/skill-discovery-index.elegy-documentation.json",
            ".agents/skills/elegy-documentation/SKILL.md",
        ),
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
        (
            "contracts/fixtures/skill-discovery-index.elegy-obsidian.json",
            ".agents/skills/elegy-obsidian/SKILL.md",
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
        .join("elegy-plugin-package.schema.json")
        .is_file());
    assert!(output_path
        .join("elegy-plugin-package.schema.json")
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
        .join("elegy-plugin-package.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("elegy-plugin-package.minimal.json")
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
        assert!(archive.by_name("elegy-plugin-package.schema.json").is_ok());
        assert!(archive.by_name("elegy-plugin-package.schema.json").is_ok());
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
            .by_name("fixtures/elegy-plugin-package.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/elegy-plugin-package.minimal.json")
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

#[test]
fn upstream_dedicated_skill_definitions_round_trip_host_projection() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let planning_path = contracts_dir
        .join("fixtures")
        .join("skill.elegy-planning.json");
    let skills_path = contracts_dir
        .join("fixtures")
        .join("skill.elegy-skills.json");

    let planning: SkillDefinitionV2 =
        serde_json::from_str(&fs::read_to_string(&planning_path).expect("read planning fixture"))
            .expect("parse planning fixture");
    let skills: SkillDefinitionV2 =
        serde_json::from_str(&fs::read_to_string(&skills_path).expect("read skills fixture"))
            .expect("parse skills fixture");

    let planning_projection = planning
        .host_projection
        .as_ref()
        .expect("planning host_projection should be populated");
    let skills_projection = skills
        .host_projection
        .as_ref()
        .expect("skills host_projection should be populated");

    assert_eq!(planning_projection.cli_name, "elegy-planning");
    assert_eq!(planning_projection.output_contract_id, "elegy-planning-v1");
    assert_eq!(skills_projection.cli_name, "elegy-skills");
    assert_eq!(skills_projection.output_contract_id, "elegy-skills-v1");

    validate_skill_definition_v2(&planning).expect("planning fixture should validate");
    validate_skill_definition_v2(&skills).expect("skills fixture should validate");

    let planning_reserialized =
        serde_json::to_string(&planning).expect("re-serialize planning fixture");
    let planning_reparsed: SkillDefinitionV2 =
        serde_json::from_str(&planning_reserialized).expect("reparse planning fixture");
    assert_eq!(planning, planning_reparsed);
    assert!(
        planning_reserialized.contains("\"hostProjection\""),
        "re-serialized planning fixture should preserve hostProjection"
    );

    let skills_reserialized = serde_json::to_string(&skills).expect("re-serialize skills fixture");
    let skills_reparsed: SkillDefinitionV2 =
        serde_json::from_str(&skills_reserialized).expect("reparse skills fixture");
    assert_eq!(skills, skills_reparsed);
    assert!(
        skills_reserialized.contains("\"hostProjection\""),
        "re-serialized skills fixture should preserve hostProjection"
    );
}

#[test]
fn skill_definition_validator_rejects_invalid_host_projection() {
    let make_capability = |id: &str| elegy_contracts::SkillCapability {
        id: id.to_string(),
        name: "Sample".to_string(),
        description: "Sample capability".to_string(),
        implementation: Some(SkillImplementation {
            execution_type: "subprocess".to_string(),
            executable_name: "sample".to_string(),
            arguments: Vec::new(),
        }),
        ..elegy_contracts::SkillCapability::default()
    };

    let base_skill = || SkillDefinitionV2 {
        skill_format: "elegy-skill-definition".to_string(),
        skill_version: 2,
        identity: SkillIdentityV2 {
            namespace: "example".to_string(),
            name: "host-projection-skill".to_string(),
            version: "0.1.0".to_string(),
            ..SkillIdentityV2::default()
        },
        capabilities: vec![
            make_capability("example-cap"),
            make_capability("example-other-cap"),
        ],
        lifecycle_state: "active".to_string(),
        ..SkillDefinitionV2::default()
    };

    let unknown_capability = base_skill();
    let mut bad = unknown_capability.clone();
    bad.host_projection = Some(elegy_contracts::SkillHostProjection {
        cli_name: "example-cli".to_string(),
        output_contract_id: "example-v1".to_string(),
        default_side_effect_class: elegy_contracts::HostSideEffectClass::ReadOnly,
        capability_projections: vec![elegy_contracts::SkillHostCapabilityProjection {
            capability_id: "missing-capability".to_string(),
            function_name: "example_fn".to_string(),
            side_effect_class: None,
            is_deterministic: None,
        }],
    });
    let error = validate_skill_definition_v2(&bad)
        .expect_err("unknown capability id should fail validation");
    assert!(error.to_string().contains(
        "hostProjection.capabilityProjections[].capabilityId 'missing-capability' does not match any capability"
    ));

    let mut duplicate_function = base_skill();
    duplicate_function.host_projection = Some(elegy_contracts::SkillHostProjection {
        cli_name: "example-cli".to_string(),
        output_contract_id: "example-v1".to_string(),
        default_side_effect_class: elegy_contracts::HostSideEffectClass::ReadOnly,
        capability_projections: vec![
            elegy_contracts::SkillHostCapabilityProjection {
                capability_id: "example-cap".to_string(),
                function_name: "duplicate_fn".to_string(),
                side_effect_class: None,
                is_deterministic: None,
            },
            elegy_contracts::SkillHostCapabilityProjection {
                capability_id: "example-other-cap".to_string(),
                function_name: "duplicate_fn".to_string(),
                side_effect_class: None,
                is_deterministic: None,
            },
        ],
    });
    let error = validate_skill_definition_v2(&duplicate_function)
        .expect_err("duplicate function name should fail validation");
    assert!(error
        .to_string()
        .contains("functionName 'duplicate_fn' is duplicated"));

    let mut duplicate_capability = base_skill();
    duplicate_capability.host_projection = Some(elegy_contracts::SkillHostProjection {
        cli_name: "example-cli".to_string(),
        output_contract_id: "example-v1".to_string(),
        default_side_effect_class: elegy_contracts::HostSideEffectClass::ReadOnly,
        capability_projections: vec![
            elegy_contracts::SkillHostCapabilityProjection {
                capability_id: "example-cap".to_string(),
                function_name: "first_fn".to_string(),
                side_effect_class: None,
                is_deterministic: None,
            },
            elegy_contracts::SkillHostCapabilityProjection {
                capability_id: "EXAMPLE-CAP".to_string(),
                function_name: "second_fn".to_string(),
                side_effect_class: None,
                is_deterministic: None,
            },
        ],
    });
    let error = validate_skill_definition_v2(&duplicate_capability)
        .expect_err("duplicate capability id should fail validation");
    assert!(error
        .to_string()
        .contains("capabilityId 'EXAMPLE-CAP' is duplicated"));

    let mut blank_function = base_skill();
    blank_function.host_projection = Some(elegy_contracts::SkillHostProjection {
        cli_name: "example-cli".to_string(),
        output_contract_id: "example-v1".to_string(),
        default_side_effect_class: elegy_contracts::HostSideEffectClass::ReadOnly,
        capability_projections: vec![elegy_contracts::SkillHostCapabilityProjection {
            capability_id: "example-cap".to_string(),
            function_name: "   ".to_string(),
            side_effect_class: None,
            is_deterministic: None,
        }],
    });
    let error = validate_skill_definition_v2(&blank_function)
        .expect_err("blank function name should fail validation");
    assert!(error
        .to_string()
        .contains("functionName for capability 'example-cap' must not be empty"));

    let mut blank_cli = base_skill();
    blank_cli.host_projection = Some(elegy_contracts::SkillHostProjection {
        cli_name: String::new(),
        output_contract_id: "example-v1".to_string(),
        default_side_effect_class: elegy_contracts::HostSideEffectClass::ReadOnly,
        capability_projections: Vec::new(),
    });
    let error =
        validate_skill_definition_v2(&blank_cli).expect_err("blank cliName should fail validation");
    assert!(error
        .to_string()
        .contains("hostProjection.cliName must not be empty"));

    let mut blank_output = base_skill();
    blank_output.host_projection = Some(elegy_contracts::SkillHostProjection {
        cli_name: "example-cli".to_string(),
        output_contract_id: String::new(),
        default_side_effect_class: elegy_contracts::HostSideEffectClass::ReadOnly,
        capability_projections: Vec::new(),
    });
    let error = validate_skill_definition_v2(&blank_output)
        .expect_err("blank outputContractId should fail validation");
    assert!(error
        .to_string()
        .contains("hostProjection.outputContractId must not be empty"));
}

fn load_dedicated_plugin_package(
    contracts_dir: &Path,
    file_name: &str,
) -> elegy_contracts::ElegyPluginPackage {
    let path = contracts_dir.join("fixtures").join(file_name);
    serde_json::from_str(&fs::read_to_string(&path).expect("read dedicated plugin package"))
        .expect("parse dedicated plugin package")
}

#[test]
fn dedicated_elegy_plugin_package_fixtures_are_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();

    let planning_package =
        load_dedicated_plugin_package(&contracts_dir, "elegy-plugin-package.elegy-planning.json");
    let skills_package =
        load_dedicated_plugin_package(&contracts_dir, "elegy-plugin-package.elegy-skills.json");

    for package in [&planning_package, &skills_package] {
        let validation = validate_elegy_plugin_package(package);
        assert!(
            validation.is_valid(),
            "unexpected issues for {}: {:?}",
            package.identity.package_id,
            validation.issues
        );
    }

    assert_eq!(planning_package.schema_version, "elegy-plugin-package/v1");
    assert_eq!(
        planning_package.identity.package_id,
        "elegy.planning-plugin"
    );
    assert_eq!(skills_package.schema_version, "elegy-plugin-package/v1");
    assert_eq!(skills_package.identity.package_id, "elegy.skills-plugin");
    assert!(!planning_package
        .components
        .capability_projections
        .is_empty());
    assert!(!skills_package.components.capability_projections.is_empty());
}

#[test]
fn exported_contract_bundle_includes_dedicated_plugin_package_fixtures() {
    let temp_root = env::temp_dir().join(format!(
        "elegy-contracts-dedicated-export-{}-{}",
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

    for fixture in [
        "fixtures/elegy-plugin-package.elegy-skills.json",
        "fixtures/elegy-plugin-package.elegy-planning.json",
    ] {
        let on_disk = output_path.join(fixture);
        assert!(
            on_disk.is_file(),
            "exported bundle directory must contain {fixture} (path={})",
            on_disk.display()
        );
        assert!(
            export.files.iter().any(|path| path == &on_disk),
            "export manifest should report {fixture}"
        );
    }

    let archive_file = fs::File::open(&archive_path).expect("open bundle archive");
    let mut archive = ZipArchive::new(archive_file).expect("read bundle archive");
    for entry in [
        "fixtures/elegy-plugin-package.elegy-skills.json",
        "fixtures/elegy-plugin-package.elegy-planning.json",
    ] {
        assert!(
            archive.by_name(entry).is_ok(),
            "bundle archive must contain {entry}"
        );
    }

    fs::remove_dir_all(&temp_root).expect("remove temp export root");
}

#[test]
fn readiness_side_effect_summary_serializes_with_schema_snake_case_keys() {
    let summary = elegy_contracts::ElegyPluginReadinessSideEffectSummary {
        none: 1,
        read_only: 2,
        disk_read: 3,
        disk_write: 4,
        network_outbound: 5,
        process_spawn: 6,
        desktop_ui: 7,
    };

    let json = serde_json::to_string(&summary).expect("serialize side effect summary");
    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("parse side effect summary JSON");

    // Schema expects snake_case keys
    assert_eq!(parsed["none"], 1);
    assert_eq!(parsed["read_only"], 2);
    assert_eq!(parsed["disk_read"], 3);
    assert_eq!(parsed["disk_write"], 4);
    assert_eq!(parsed["network_outbound"], 5);
    assert_eq!(parsed["process_spawn"], 6);
    assert_eq!(parsed["desktop_ui"], 7);

    // Verify camelCase variants are NOT produced
    assert!(
        parsed.get("readOnly").is_none(),
        "read_only must not serialize as readOnly"
    );
    assert!(
        parsed.get("diskRead").is_none(),
        "disk_read must not serialize as diskRead"
    );
    assert!(
        parsed.get("diskWrite").is_none(),
        "disk_write must not serialize as diskWrite"
    );
    assert!(
        parsed.get("networkOutbound").is_none(),
        "network_outbound must not serialize as networkOutbound"
    );
    assert!(
        parsed.get("processSpawn").is_none(),
        "process_spawn must not serialize as processSpawn"
    );
    assert!(
        parsed.get("desktopUi").is_none(),
        "desktop_ui must not serialize as desktopUi"
    );
}

#[test]
fn readiness_side_effect_summary_round_trips_through_deserialization() {
    let json = r#"{"none":0,"read_only":1,"disk_read":2,"disk_write":3,"network_outbound":4,"process_spawn":5,"desktop_ui":6}"#;
    let summary: elegy_contracts::ElegyPluginReadinessSideEffectSummary =
        serde_json::from_str(json).expect("deserialize snake_case JSON");

    assert_eq!(summary.none, 0);
    assert_eq!(summary.read_only, 1);
    assert_eq!(summary.disk_read, 2);
    assert_eq!(summary.disk_write, 3);
    assert_eq!(summary.network_outbound, 4);
    assert_eq!(summary.process_spawn, 5);
    assert_eq!(summary.desktop_ui, 6);

    let reserialized = serde_json::to_string(&summary).expect("re-serialize");
    let reparsed: serde_json::Value =
        serde_json::from_str(&reserialized).expect("re-parse serialized");
    assert_eq!(
        reparsed["read_only"], 1,
        "read_only must survive round-trip as snake_case"
    );
    assert_eq!(
        reparsed["disk_read"], 2,
        "disk_read must survive round-trip as snake_case"
    );
    assert_eq!(
        reparsed["network_outbound"], 4,
        "network_outbound must survive round-trip as snake_case"
    );
}
