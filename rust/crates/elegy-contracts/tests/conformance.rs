use elegy_contracts::{
    builtin_capability_definitions, default_support_manifest_path, export_contract_bundle,
    load_agent_capability_profile_fixture_from_dir, load_capability_definition_fixture_from_dir,
    load_compatibility_manifest_from_dir, load_consumer_support_manifest,
    load_elegy_configuration_profile_fixture_from_dir,
    load_elegy_configuration_receipt_fixture_from_dir,
    load_elegy_configuration_template_fixture_from_dir, load_elegy_plugin_package_fixture_from_dir,
    load_elegy_plugin_package_v2_fixture_from_dir, load_execution_event_fixture_from_dir,
    load_invocation_request_fixture_from_dir, load_invocation_response_fixture_from_dir,
    load_mcp_analysis_result_fixture_from_dir, load_mcp_server_descriptor_fixture_from_dir,
    load_observation_event_fixture_from_dir, load_observation_session_fixture_from_dir,
    load_observation_summary_fixture_from_dir, load_piloting_action_intent_fixture_from_dir,
    load_piloting_action_result_fixture_from_dir, load_piloting_adapter_manifest_fixture_from_dir,
    load_piloting_fixture_pack_fixture_from_dir, load_piloting_observation_frame_fixture_from_dir,
    load_piloting_readiness_report_fixture_from_dir,
    load_piloting_surface_descriptor_fixture_from_dir,
    load_piloting_target_descriptor_fixture_from_dir, load_skill_definition_v2_fixture_from_dir,
    load_skill_discovery_index_fixture_from_dir, load_structured_failure_fixture_from_dir,
    resolve_upstream_contracts_dir, validate_agent_capability_profile,
    validate_capability_definition, validate_elegy_configuration_profile,
    validate_elegy_configuration_receipt, validate_elegy_configuration_template,
    validate_elegy_plugin_package, validate_execution_event, validate_invocation_request,
    validate_invocation_response, validate_mcp_analysis_result, validate_mcp_server_descriptor,
    validate_observation_event, validate_observation_session, validate_observation_summary,
    validate_piloting_action_intent, validate_piloting_action_result,
    validate_piloting_adapter_manifest, validate_piloting_fixture_pack,
    validate_piloting_fixture_pack_against_manifest, validate_piloting_observation_frame,
    validate_piloting_package_file, validate_piloting_readiness_report,
    validate_piloting_surface_descriptor, validate_piloting_target_descriptor,
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
    assert!(schema_names.contains("piloting-target-descriptor"));
    assert!(schema_names.contains("piloting-surface-descriptor"));
    assert!(schema_names.contains("piloting-observation-frame"));
    assert!(schema_names.contains("piloting-action-intent"));
    assert!(schema_names.contains("piloting-action-result"));
    assert!(schema_names.contains("piloting-readiness-report"));
    assert!(schema_names.contains("piloting-adapter-manifest"));
    assert!(schema_names.contains("piloting-fixture-pack"));
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
fn upstream_piloting_fixtures_are_semantically_valid() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let target = load_piloting_target_descriptor_fixture_from_dir(&contracts_dir)
        .expect("load upstream piloting target fixture");
    let surface = load_piloting_surface_descriptor_fixture_from_dir(&contracts_dir)
        .expect("load upstream piloting surface fixture");
    let observation = load_piloting_observation_frame_fixture_from_dir(&contracts_dir)
        .expect("load upstream piloting observation fixture");
    let action_intent = load_piloting_action_intent_fixture_from_dir(&contracts_dir)
        .expect("load upstream piloting action intent fixture");
    let action_result = load_piloting_action_result_fixture_from_dir(&contracts_dir)
        .expect("load upstream piloting action result fixture");
    let readiness = load_piloting_readiness_report_fixture_from_dir(&contracts_dir)
        .expect("load upstream piloting readiness fixture");
    let adapter_manifest = load_piloting_adapter_manifest_fixture_from_dir(&contracts_dir)
        .expect("load upstream piloting adapter manifest fixture");
    let fixture_pack = load_piloting_fixture_pack_fixture_from_dir(&contracts_dir)
        .expect("load upstream piloting fixture pack fixture");

    assert!(validate_piloting_target_descriptor(&target).is_valid());
    assert!(validate_piloting_surface_descriptor(&surface).is_valid());
    assert!(validate_piloting_observation_frame(&observation).is_valid());
    assert!(validate_piloting_action_intent(&action_intent).is_valid());
    assert!(validate_piloting_action_result(&action_result).is_valid());
    assert!(validate_piloting_readiness_report(&readiness).is_valid());
    assert!(validate_piloting_adapter_manifest(&adapter_manifest).is_valid());
    assert!(validate_piloting_fixture_pack(&fixture_pack).is_valid());
    assert!(
        validate_piloting_fixture_pack_against_manifest(&fixture_pack, &adapter_manifest)
            .is_valid()
    );

    assert_eq!(target.target_id, "blender.desktop");
    assert_eq!(surface.surface_id, "blender.desktop.main-window");
    assert_eq!(action_intent.action_id, "select-default-cube");
    assert_eq!(readiness.report_id, "readiness.blender.1");
    assert_eq!(adapter_manifest.adapter_id, "blender.piloting");
    assert_eq!(
        fixture_pack.fixture_pack_id,
        "blender.fixtures.layout-basic"
    );
    assert_eq!(fixture_pack.policy_decisions.len(), 1);
    assert_eq!(fixture_pack.simulation_results.len(), 1);
    assert_eq!(fixture_pack.replay_checkpoints.len(), 2);
    assert_eq!(fixture_pack.lifecycle_events.len(), 4);
}

#[test]
fn upstream_piloting_package_fixture_is_semantically_valid() {
    let repo_root = resolve_upstream_contracts_dir()
        .parent()
        .expect("repo root from contracts dir")
        .to_path_buf();
    let package: elegy_contracts::ElegyPluginPackage = serde_json::from_str(
        &fs::read_to_string(
            repo_root
                .join("contracts")
                .join("fixtures")
                .join("elegy-plugin-package-v2.piloting-blender.json"),
        )
        .expect("read piloting package fixture"),
    )
    .expect("parse piloting package fixture");

    let validation = validate_elegy_plugin_package(&package);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    assert_eq!(package.identity.package_id, "elegy.blender-piloting");
    assert_eq!(package.components.piloting_adapters.len(), 1);
    assert_eq!(package.components.fixture_packs.len(), 1);
    assert_eq!(
        package
            .publishing
            .as_ref()
            .and_then(|publishing| publishing.marketplace_target.as_deref()),
        Some("holon")
    );

    let package_path = repo_root
        .join("contracts")
        .join("fixtures")
        .join("elegy-plugin-package-v2.piloting-blender.json");
    let file_validation = validate_piloting_package_file(&package_path, &package);
    assert!(
        file_validation.is_valid(),
        "unexpected file-backed issues: {:?}",
        file_validation.issues
    );
}

#[test]
fn file_backed_piloting_package_supports_manifest_refs_and_fixture_pack_refs() {
    let temp_root = env::temp_dir().join(format!(
        "elegy-contracts-piloting-package-{}-{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    ));
    let package_dir = temp_root.join("package");
    fs::create_dir_all(&package_dir).expect("create package dir");
    fs::create_dir_all(package_dir.join("signatures")).expect("create signature dir");

    let repo_root = resolve_upstream_contracts_dir()
        .parent()
        .expect("repo root from contracts dir")
        .to_path_buf();
    fs::copy(
        repo_root
            .join("contracts")
            .join("fixtures")
            .join("piloting-adapter-manifest.minimal.json"),
        package_dir.join("adapter.json"),
    )
    .expect("copy adapter manifest");
    let mut adapter_manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(package_dir.join("adapter.json")).expect("read copied adapter"),
    )
    .expect("parse copied adapter");
    adapter_manifest["fixtures"][0]["path"] = serde_json::json!("fixture-pack.json");
    fs::write(
        package_dir.join("adapter.json"),
        serde_json::to_string_pretty(&adapter_manifest).expect("serialize copied adapter"),
    )
    .expect("rewrite copied adapter");
    fs::copy(
        repo_root
            .join("contracts")
            .join("fixtures")
            .join("piloting-fixture-pack.minimal.json"),
        package_dir.join("fixture-pack.json"),
    )
    .expect("copy fixture pack");
    fs::copy(
        repo_root
            .join("contracts")
            .join("fixtures")
            .join("piloting-readiness-report.minimal.json"),
        package_dir.join("provenance.json"),
    )
    .expect("copy provenance fixture");
    fs::write(package_dir.join("CHANGELOG.md"), "# Changelog\n").expect("write changelog");
    fs::write(package_dir.join("signatures").join("package.sig"), "sig\n")
        .expect("write signature");

    let package_path = package_dir.join("package.json");
    fs::write(
        &package_path,
        r#"{
  "schemaVersion": "elegy-plugin-package/v2",
  "identity": {
    "packageId": "elegy.ref-backed-piloting",
    "name": "ref-backed-piloting",
    "version": "0.1.0"
  },
  "metadata": {
    "description": "Ref-backed piloting package fixture.",
    "license": "Apache-2.0"
  },
  "components": {
    "pilotingAdapters": [
      {
        "id": "adapter",
        "manifestRef": "adapter.json"
      }
    ],
    "fixturePacks": [
      {
        "id": "fixture-pack",
        "fixturePackRef": "fixture-pack.json"
      }
    ]
  },
  "publishing": {
    "marketplaceTarget": "holon",
    "importMode": "package",
    "sourceRepository": "https://github.com/Sofreshx/Elegy.git",
    "sourceRef": "refs/heads/main",
    "sourceCommit": "8d062afa1b106e2db5f63e3afdd8b1198bc6e960",
    "changelogRef": "CHANGELOG.md",
    "provenanceRef": "provenance.json",
    "signatureRefs": ["signatures/package.sig"],
    "compatibility": [
      {
        "host": "holon",
        "versionRange": ">=0.1.0 <0.2.0"
      }
    ]
  }
}"#,
    )
    .expect("write ref-backed package");

    let package: elegy_contracts::ElegyPluginPackage =
        serde_json::from_str(&fs::read_to_string(&package_path).expect("read package"))
            .expect("parse ref-backed package");

    let validation = validate_elegy_plugin_package(&package);
    assert!(
        validation.is_valid(),
        "unexpected issues: {:?}",
        validation.issues
    );

    let file_validation = validate_piloting_package_file(&package_path, &package);
    assert!(
        file_validation.is_valid(),
        "unexpected file-backed issues: {:?}",
        file_validation.issues
    );

    fs::remove_dir_all(&temp_root).expect("remove temp package root");
}

#[test]
fn plugin_package_validator_rejects_invalid_uri_fields() {
    let mut package =
        load_elegy_plugin_package_v2_fixture_from_dir(&resolve_upstream_contracts_dir())
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
fn piloting_validators_reject_invalid_rfc3339_timestamps() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let mut observation = load_piloting_observation_frame_fixture_from_dir(&contracts_dir)
        .expect("load upstream piloting observation fixture");
    observation.observed_at_utc = "not-a-timestamp".to_string();
    let observation_validation = validate_piloting_observation_frame(&observation);
    assert!(observation_validation
        .issues
        .contains(&"observedAtUtc must be a valid RFC3339 date-time.".to_string()));

    let mut readiness = load_piloting_readiness_report_fixture_from_dir(&contracts_dir)
        .expect("load upstream piloting readiness fixture");
    readiness.generated_at_utc = "not-a-timestamp".to_string();
    let readiness_validation = validate_piloting_readiness_report(&readiness);
    assert!(readiness_validation
        .issues
        .contains(&"generatedAtUtc must be a valid RFC3339 date-time.".to_string()));

    let mut fixture_pack = load_piloting_fixture_pack_fixture_from_dir(&contracts_dir)
        .expect("load upstream piloting fixture pack fixture");
    fixture_pack.recorded_at_utc = "not-a-timestamp".to_string();
    let fixture_pack_validation = validate_piloting_fixture_pack(&fixture_pack);
    assert!(fixture_pack_validation
        .issues
        .contains(&"recordedAtUtc must be a valid RFC3339 date-time.".to_string()));
}

#[test]
fn file_backed_piloting_package_rejects_dual_source_manifest_drift() {
    let temp_root = env::temp_dir().join(format!(
        "elegy-contracts-piloting-dual-source-{}-{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    ));
    let package_dir = temp_root.join("package");
    fs::create_dir_all(&package_dir).expect("create package dir");

    let contracts_dir = resolve_upstream_contracts_dir();
    let inline_manifest = load_piloting_adapter_manifest_fixture_from_dir(&contracts_dir)
        .expect("load upstream piloting adapter fixture");
    let mut referenced_manifest = inline_manifest.clone();
    referenced_manifest.display_name = "Drifted Manifest".to_string();
    fs::write(
        package_dir.join("adapter.json"),
        serde_json::to_string_pretty(&referenced_manifest).expect("serialize referenced manifest"),
    )
    .expect("write referenced manifest");

    let inline_fixture_pack = load_piloting_fixture_pack_fixture_from_dir(&contracts_dir)
        .expect("load upstream piloting fixture pack fixture");
    let mut referenced_fixture_pack = inline_fixture_pack.clone();
    referenced_fixture_pack.expected_result_checks[0].expected_status = "failed".to_string();
    fs::write(
        package_dir.join("fixture-pack.json"),
        serde_json::to_string_pretty(&referenced_fixture_pack)
            .expect("serialize referenced fixture pack"),
    )
    .expect("write referenced fixture pack");

    let package = elegy_contracts::ElegyPluginPackage {
        schema_version: "elegy-plugin-package/v2".to_string(),
        identity: elegy_contracts::ElegyPluginPackageIdentity {
            package_id: "elegy.dual-source-piloting".to_string(),
            name: "dual-source-piloting".to_string(),
            version: "0.1.0".to_string(),
            display_name: None,
        },
        components: elegy_contracts::ElegyPluginPackageComponents {
            piloting_adapters: vec![
                elegy_contracts::ElegyPluginPackagePilotingAdapterComponent {
                    id: "adapter".to_string(),
                    manifest_ref: Some("adapter.json".to_string()),
                    manifest: Some(inline_manifest),
                },
            ],
            fixture_packs: vec![
                elegy_contracts::ElegyPluginPackagePilotingFixturePackComponent {
                    id: "fixture-pack".to_string(),
                    fixture_pack_ref: Some("fixture-pack.json".to_string()),
                    fixture_pack: Some(inline_fixture_pack),
                },
            ],
            ..Default::default()
        },
        ..Default::default()
    };

    let package_path = package_dir.join("package.json");
    let file_validation = validate_piloting_package_file(&package_path, &package);
    assert!(file_validation.issues.iter().any(|issue| issue
        .contains("must keep manifestRef 'adapter.json' aligned with the inline manifest.")));
    assert!(file_validation.issues.iter().any(|issue| issue.contains(
        "must keep fixturePackRef 'fixture-pack.json' aligned with the inline fixture pack."
    )));

    fs::remove_dir_all(&temp_root).expect("remove temp package root");
}

#[test]
fn piloting_fixture_pack_rejects_unknown_policy_and_replay_refs() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let mut fixture_pack = load_piloting_fixture_pack_fixture_from_dir(&contracts_dir)
        .expect("load upstream piloting fixture pack fixture");

    fixture_pack.simulation_results[0].policy_decision_ref = Some("missing-policy".to_string());
    fixture_pack.replay_checkpoints[0].state_ref = "missing-state".to_string();
    fixture_pack.lifecycle_events[1].ref_id = Some("missing-ref".to_string());

    let validation = validate_piloting_fixture_pack(&fixture_pack);
    assert!(validation.issues.iter().any(|issue| issue.contains(
        "simulationResults entry 'sim.select-default-cube.1' references unknown policyDecisionRef 'missing-policy'."
    )));
    assert!(validation.issues.iter().any(|issue| issue.contains(
        "replayCheckpoints entry 'checkpoint.select-default-cube.before' references unknown stateRef 'missing-state'."
    )));
    assert!(validation.issues.iter().any(|issue| issue.contains(
        "lifecycleEvents entry 'event.select-default-cube.policy' references unknown refId 'missing-ref'."
    )));
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
        .join("piloting-target-descriptor.schema.json")
        .is_file());
    assert!(output_path
        .join("piloting-surface-descriptor.schema.json")
        .is_file());
    assert!(output_path
        .join("piloting-observation-frame.schema.json")
        .is_file());
    assert!(output_path
        .join("piloting-action-intent.schema.json")
        .is_file());
    assert!(output_path
        .join("piloting-action-result.schema.json")
        .is_file());
    assert!(output_path
        .join("piloting-readiness-report.schema.json")
        .is_file());
    assert!(output_path
        .join("piloting-adapter-manifest.schema.json")
        .is_file());
    assert!(output_path
        .join("piloting-fixture-pack.schema.json")
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
        .join("elegy-plugin-package-v2.piloting-blender.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("piloting-target-descriptor.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("piloting-surface-descriptor.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("piloting-observation-frame.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("piloting-action-intent.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("piloting-action-result.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("piloting-readiness-report.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("piloting-adapter-manifest.minimal.json")
        .is_file());
    assert!(output_path
        .join("fixtures")
        .join("piloting-fixture-pack.minimal.json")
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
            .by_name("piloting-target-descriptor.schema.json")
            .is_ok());
        assert!(archive
            .by_name("piloting-surface-descriptor.schema.json")
            .is_ok());
        assert!(archive
            .by_name("piloting-observation-frame.schema.json")
            .is_ok());
        assert!(archive
            .by_name("piloting-action-intent.schema.json")
            .is_ok());
        assert!(archive
            .by_name("piloting-action-result.schema.json")
            .is_ok());
        assert!(archive
            .by_name("piloting-readiness-report.schema.json")
            .is_ok());
        assert!(archive
            .by_name("piloting-adapter-manifest.schema.json")
            .is_ok());
        assert!(archive.by_name("piloting-fixture-pack.schema.json").is_ok());
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
            .by_name("fixtures/elegy-plugin-package-v2.piloting-blender.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/piloting-target-descriptor.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/piloting-surface-descriptor.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/piloting-observation-frame.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/piloting-action-intent.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/piloting-action-result.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/piloting-readiness-report.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/piloting-adapter-manifest.minimal.json")
            .is_ok());
        assert!(archive
            .by_name("fixtures/piloting-fixture-pack.minimal.json")
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
