use elegy_contracts::{
    builtin_capability_definitions, export_contract_bundle,
    load_agent_capability_profile_fixture_from_dir, load_capability_definition_fixture_from_dir,
    load_elegy_configuration_profile_fixture_from_dir,
    load_elegy_configuration_receipt_fixture_from_dir,
    load_elegy_configuration_template_fixture_from_dir, load_elegy_plugin_package_fixture_from_dir,
    load_execution_event_fixture_from_dir, load_invocation_request_fixture_from_dir,
    load_invocation_response_fixture_from_dir, load_mcp_analysis_result_fixture_from_dir,
    load_mcp_server_descriptor_fixture_from_dir, load_observation_event_fixture_from_dir,
    load_observation_session_fixture_from_dir, load_observation_summary_fixture_from_dir,
    load_skill_definition_v2_fixture_from_dir, load_structured_failure_fixture_from_dir,
    resolve_upstream_contracts_dir, validate_agent_capability_profile,
    validate_capability_definition, validate_elegy_configuration_profile,
    validate_elegy_configuration_receipt, validate_elegy_configuration_template,
    validate_elegy_plugin_package, validate_execution_event, validate_invocation_request,
    validate_invocation_response, validate_mcp_analysis_result, validate_mcp_server_descriptor,
    validate_observation_event, validate_observation_session, validate_observation_summary,
    validate_skill_definition_v2, validate_structured_failure, CapabilityApprovalRequirement,
    CapabilityDefinition, CapabilityGovernance, CapabilitySource, CapabilitySourceKind,
    ExecutionEvent, ExecutionEventStatus, ExecutionEventType, InvocationRequest,
    InvocationResponse, InvocationStatus, McpAnalysisResult, McpServerDescriptor, McpToolAnalysis,
    McpToolDefinition, SkillDefinitionV2, SkillGovernance, SkillIdentityV2, SkillImplementation,
    SkillOriginV2, StructuredFailure, StructuredFailureCategory, StructuredFailureCause,
};
use std::env;
use std::fs;
use std::path::Path;
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
fn all_plugin_package_fixtures_match_current_schema() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let fixtures_dir = contracts_dir.join("fixtures");

    let mut tested = 0;
    for entry in fs::read_dir(&fixtures_dir).expect("read fixtures directory") {
        let entry = entry.expect("read entry");
        let path = entry.path();
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("valid filename");

        if !filename.starts_with("elegy-plugin-package.") || !filename.ends_with(".json") {
            continue;
        }

        if filename.contains(".negative-") {
            continue;
        }

        if filename == "elegy-plugin-package.template.json" {
            continue;
        }

        let content = fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {filename}"));

        let package: elegy_contracts::ElegyPluginPackage =
            serde_json::from_str(&content).unwrap_or_else(|e| panic!("parse {filename}: {e}"));

        let validation = validate_elegy_plugin_package(&package);
        assert!(
            validation.is_valid(),
            "{filename} validation failed: {:?}",
            validation.issues
        );

        assert_eq!(
            package.schema_version, "elegy-plugin-package/v1",
            "{filename} must use elegy-plugin-package/v1"
        );

        tested += 1;
    }

    assert!(
        tested >= 5,
        "expected at least 5 non-negative plugin package fixtures, found {tested}"
    );
}

#[test]
fn plugin_package_fixtures_with_tool_requirements_have_publishing_metadata() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let fixtures_dir = contracts_dir.join("fixtures");
    let mut offenders: Vec<String> = Vec::new();
    let mut published_with_tools: usize = 0;

    for entry in fs::read_dir(&fixtures_dir).expect("read fixtures directory") {
        let entry = entry.expect("read entry");
        let path = entry.path();
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("valid filename");

        if !filename.starts_with("elegy-plugin-package.") || !filename.ends_with(".json") {
            continue;
        }
        if filename.contains(".negative-") || filename == "elegy-plugin-package.template.json" {
            continue;
        }

        let content = fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {filename}"));
        let package: elegy_contracts::ElegyPluginPackage =
            serde_json::from_str(&content).unwrap_or_else(|e| panic!("parse {filename}: {e}"));

        let has_tools = !package.components.tool_requirements.is_empty();

        if !has_tools {
            continue;
        }

        published_with_tools += 1;

        let Some(publishing) = package.publishing.as_ref() else {
            offenders.push(format!(
                "{filename}: has toolRequirements but no publishing block; the per-feature publish workflow cannot derive archive family or asset prefix without it"
            ));
            continue;
        };

        if publishing
            .archive_family
            .as_deref()
            .unwrap_or("")
            .is_empty()
        {
            offenders.push(format!(
                "{filename}: has toolRequirements but publishing.archiveFamily is missing or empty"
            ));
        }
    }

    assert!(
        offenders.is_empty(),
        "per-feature publish metadata contract violations:\n  - {}",
        offenders.join("\n  - ")
    );
    assert!(
        published_with_tools >= 5,
        "expected at least 5 plugin package fixtures with toolRequirements + publishing, found {published_with_tools}"
    );
}

#[test]
fn plugin_package_publishing_blocks_have_orchestrator_required_fields() {
    // The central publish orchestrator (`.github/workflows/publish-orchestrator.yml`)
    // discovers surfaces by walking `contracts/fixtures/elegy-plugin-package.*.json`
    // and reading the `publishing` block. Every fixture with a `publishing` block
    // must declare enough metadata for the orchestrator to drive a build without
    // any per-feature workflow file:
    //   - `cratePath`    : workspace-relative path to the Rust crate (or absent
    //                      for skill-only / external-CLI surfaces).
    //   - `assetKind`    : "cli" or "wrapper" — drives archive staging contents.
    //   - `archiveFamily`: per-feature asset family; required when publishing is set.
    // Fixtures without a `publishing` block are out of scope (skill-only or demo).
    let contracts_dir = resolve_upstream_contracts_dir();
    let fixtures_dir = contracts_dir.join("fixtures");
    let mut offenders: Vec<String> = Vec::new();
    let mut tested = 0;

    for entry in fs::read_dir(&fixtures_dir).expect("read fixtures directory") {
        let entry = entry.expect("read entry");
        let path = entry.path();
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("valid filename")
            .to_string();

        if !filename.starts_with("elegy-plugin-package.") || !filename.ends_with(".json") {
            continue;
        }
        if filename.contains(".negative-")
            || filename == "elegy-plugin-package.template.json"
            || filename == "elegy-plugin-package.demo-config.json"
            || filename == "elegy-plugin-package.minimal.json"
        {
            continue;
        }

        let content = fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {filename}"));
        let package: elegy_contracts::ElegyPluginPackage =
            serde_json::from_str(&content).unwrap_or_else(|e| panic!("parse {filename}: {e}"));

        let Some(publishing) = package.publishing.as_ref() else {
            continue;
        };
        tested += 1;

        if publishing
            .archive_family
            .as_deref()
            .unwrap_or("")
            .is_empty()
        {
            offenders.push(format!(
                "{filename}: publishing.archiveFamily is required for the central orchestrator"
            ));
        }

        let asset_kind = publishing.asset_kind.as_deref().unwrap_or("");
        if asset_kind.is_empty() {
            offenders.push(format!(
                "{filename}: publishing.assetKind is required (cli | wrapper) for the central orchestrator"
            ));
        } else if !matches!(asset_kind, "cli" | "wrapper") {
            offenders.push(format!(
                "{filename}: publishing.assetKind must be 'cli' or 'wrapper', got '{asset_kind}'"
            ));
        }

        if matches!(asset_kind, "wrapper") {
            if publishing.skill_bridge.as_deref().unwrap_or("").is_empty() {
                offenders.push(format!(
                    "{filename}: assetKind=wrapper requires publishing.skillBridge"
                ));
            }
            if publishing.installer.as_deref().unwrap_or("").is_empty() {
                offenders.push(format!(
                    "{filename}: assetKind=wrapper requires publishing.installer"
                ));
            }
        }

        if !matches!(asset_kind, "cli" | "wrapper") {
            // Already reported above; skip cratePath requirement check.
            continue;
        }

        if matches!(asset_kind, "cli") && publishing.crate_path.as_deref().unwrap_or("").is_empty()
        {
            offenders.push(format!(
                "{filename}: assetKind=cli requires publishing.cratePath"
            ));
        }
    }

    assert!(
        offenders.is_empty(),
        "central-orchestrator contract violations:\n  - {}",
        offenders.join("\n  - ")
    );
    assert!(
        tested >= 5,
        "expected at least 5 publishable plugin package fixtures, found {tested}"
    );
}

#[test]
fn plugin_package_crate_paths_resolve_in_workspace() {
    // For every publishable fixture, when `cratePath` is set, it must point at a
    // crate that exists in the Rust workspace. The orchestrator builds the crate
    // via `cargo build -p <cratePath>`; a stale path would fail the build matrix.
    let contracts_dir = resolve_upstream_contracts_dir();
    let fixtures_dir = contracts_dir.join("fixtures");
    let workspace_members = collect_workspace_member_names();
    let mut offenders: Vec<String> = Vec::new();

    for entry in fs::read_dir(&fixtures_dir).expect("read fixtures directory") {
        let entry = entry.expect("read entry");
        let path = entry.path();
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("valid filename")
            .to_string();

        if !filename.starts_with("elegy-plugin-package.") || !filename.ends_with(".json") {
            continue;
        }
        if filename.contains(".negative-")
            || filename == "elegy-plugin-package.template.json"
            || filename == "elegy-plugin-package.demo-config.json"
            || filename == "elegy-plugin-package.minimal.json"
        {
            continue;
        }

        let content = fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {filename}"));
        let package: elegy_contracts::ElegyPluginPackage =
            serde_json::from_str(&content).unwrap_or_else(|e| panic!("parse {filename}: {e}"));

        let Some(crate_path) = package
            .publishing
            .as_ref()
            .and_then(|p| p.crate_path.as_deref())
        else {
            continue;
        };
        if !workspace_members.contains(crate_path) {
            offenders.push(format!(
                "{filename}: publishing.cratePath '{crate_path}' is not a workspace member"
            ));
        }
    }

    assert!(
        offenders.is_empty(),
        "publishing.cratePath must resolve to a workspace member:\n  - {}",
        offenders.join("\n  - ")
    );
}

fn collect_workspace_member_names() -> std::collections::HashSet<String> {
    // The workspace's `members = [...]` list is paths like "core/elegy-contracts".
    // The orchestrator passes the fixture's `cratePath` to `cargo build -p`, so
    // it must be the crate NAME (the `name` field in each member's Cargo.toml),
    // not the path. We resolve each member path to its crate name by reading
    // its Cargo.toml.
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let workspace_toml = workspace_root.join("Cargo.toml");
    let content = fs::read_to_string(&workspace_toml).expect("read workspace Cargo.toml");
    let mut paths: Vec<String> = Vec::new();
    let mut in_members = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("members") {
            if rest.trim_start().starts_with('=') {
                in_members = true;
                continue;
            }
        }
        if !in_members {
            continue;
        }
        for token in trimmed.split(',') {
            let cleaned = token.trim().trim_matches('"').trim_matches('\'');
            if cleaned.is_empty() || cleaned == "]" {
                continue;
            }
            if cleaned.starts_with('[') {
                in_members = !cleaned.contains(']');
                continue;
            }
            if cleaned.starts_with("//") {
                continue;
            }
            paths.push(cleaned.to_string());
        }
        if trimmed.contains(']') {
            in_members = false;
        }
    }
    let mut names = std::collections::HashSet::new();
    for path in paths {
        let cargo_toml = workspace_root.join(&path).join("Cargo.toml");
        let Ok(content) = fs::read_to_string(&cargo_toml) else {
            continue;
        };
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("name") {
                let rest = rest
                    .trim_start()
                    .trim_start_matches('=')
                    .trim()
                    .trim_matches('"');
                if !rest.is_empty() {
                    names.insert(rest.to_string());
                    break;
                }
            }
        }
    }
    names
}

#[test]
fn all_plugin_package_negative_fixtures_are_rejected() {
    let contracts_dir = resolve_upstream_contracts_dir();
    let fixtures_dir = contracts_dir.join("fixtures");

    let mut tested = 0;
    for entry in fs::read_dir(&fixtures_dir).expect("read fixtures directory") {
        let entry = entry.expect("read entry");
        let path = entry.path();
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("valid filename");

        if !filename.starts_with("elegy-plugin-package.negative-") || !filename.ends_with(".json") {
            continue;
        }

        // Subset-of coverage is validated at the readiness (verify) level, not in structural validation.
        if filename.contains("missing-subset-marker") {
            continue;
        }

        let content = fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {filename}"));

        let package: elegy_contracts::ElegyPluginPackage =
            serde_json::from_str(&content).unwrap_or_else(|e| panic!("parse {filename}: {e}"));

        let validation = validate_elegy_plugin_package(&package);
        assert!(
            !validation.is_valid(),
            "{filename} must be rejected, but validator accepted it: {:?}",
            validation.issues
        );

        if filename.contains("phantom-capability") {
            assert!(
                validation
                    .issues
                    .iter()
                    .any(|i| i.contains("unknown capability")),
                "{filename} must report an 'unknown capability' issue; got {:?}",
                validation.issues
            );
        }

        tested += 1;
    }

    assert!(
        tested >= 2,
        "expected at least 3 plugin package negative fixtures, found {tested}"
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

    assert_eq!(template.template_id, "repo-opencode-agentic-minimal");
    assert_eq!(profile.profile_id, "repo-opencode-minimal");
    assert_eq!(
        receipt.mode,
        elegy_contracts::ElegyConfigurationReceiptMode::DryRun
    );
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
    assert!(output_path
        .join("schemas")
        .join("canonical-workflow.schema.json")
        .is_file());
    assert!(output_path
        .join("schemas")
        .join("agent-manifest.schema.json")
        .is_file());
    assert!(output_path
        .join("schemas")
        .join("agent-check.schema.json")
        .is_file());
    assert!(output_path
        .join("schemas")
        .join("agent-discovery.schema.json")
        .is_file());
    assert!(output_path
        .join("schemas")
        .join("capability-definition.schema.json")
        .is_file());
    assert!(output_path
        .join("schemas")
        .join("elegy-plugin-package.schema.json")
        .is_file());
    assert!(output_path
        .join("schemas")
        .join("structured-failure.schema.json")
        .is_file());
    assert!(output_path
        .join("schemas")
        .join("invocation-request.schema.json")
        .is_file());
    assert!(output_path
        .join("schemas")
        .join("invocation-response.schema.json")
        .is_file());
    assert!(output_path
        .join("schemas")
        .join("execution-event.schema.json")
        .is_file());
    assert!(output_path
        .join("schemas")
        .join("observation-event.schema.json")
        .is_file());
    assert!(output_path
        .join("schemas")
        .join("observation-session.schema.json")
        .is_file());
    assert!(output_path
        .join("schemas")
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
        assert!(archive
            .by_name("schemas/agent-manifest.schema.json")
            .is_ok());
        assert!(archive.by_name("schemas/agent-check.schema.json").is_ok());
        assert!(archive
            .by_name("schemas/agent-discovery.schema.json")
            .is_ok());
        assert!(archive
            .by_name("schemas/capability-definition.schema.json")
            .is_ok());
        assert!(archive
            .by_name("schemas/elegy-plugin-package.schema.json")
            .is_ok());
        assert!(archive
            .by_name("schemas/structured-failure.schema.json")
            .is_ok());
        assert!(archive
            .by_name("schemas/invocation-request.schema.json")
            .is_ok());
        assert!(archive
            .by_name("schemas/invocation-response.schema.json")
            .is_ok());
        assert!(archive
            .by_name("schemas/execution-event.schema.json")
            .is_ok());
        assert!(archive
            .by_name("schemas/observation-event.schema.json")
            .is_ok());
        assert!(archive
            .by_name("schemas/observation-session.schema.json")
            .is_ok());
        assert!(archive
            .by_name("schemas/observation-summary.schema.json")
            .is_ok());
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

#[test]
fn plugin_package_with_elegy_compatibility_parses() {
    let json = r#"{
        "schemaVersion": "elegy-plugin-package/v1",
        "identity": { "packageId": "test.plugin", "name": "test-plugin", "version": "0.1.0" },
        "components": {},
        "elegyCompatibility": {
            "contractBundleVersion": "1.8.0",
            "schemaLine": "1.x",
            "minimumElegyToolingVersion": "1.8.0",
            "contractsSource": "https://example.com/contracts/bundle-v1.8.0.zip"
        }
    }"#;

    let package: elegy_contracts::ElegyPluginPackage =
        serde_json::from_str(json).expect("parse package with elegyCompatibility");

    let compat = package
        .elegy_compatibility
        .expect("elegyCompatibility should be present");
    assert_eq!(compat.contract_bundle_version, "1.8.0");
    assert_eq!(compat.schema_line, "1.x");
    assert_eq!(
        compat.minimum_elegy_tooling_version.as_deref(),
        Some("1.8.0")
    );
    assert!(compat.contracts_source.is_some());
}

#[test]
fn plugin_package_without_elegy_compatibility_still_parses() {
    let json = r#"{
        "schemaVersion": "elegy-plugin-package/v1",
        "identity": {
            "packageId": "elegy.compatibility-omitted",
            "name": "compatibility-omitted",
            "version": "0.1.0"
        },
        "components": {}
    }"#;
    let package: elegy_contracts::ElegyPluginPackage =
        serde_json::from_str(json).expect("parse package without elegyCompatibility");

    assert!(package.elegy_compatibility.is_none());

    let validation = validate_elegy_plugin_package(&package);
    assert!(
        validation.is_valid(),
        "existing packages without elegyCompatibility should still validate: {:?}",
        validation.issues
    );
}

#[test]
fn elegy_compatibility_uri_validation() {
    let json = r#"{
        "schemaVersion": "elegy-plugin-package/v1",
        "identity": { "packageId": "test.plugin", "name": "test-plugin", "version": "0.1.0" },
        "components": {},
        "elegyCompatibility": {
            "contractBundleVersion": "1.8.0",
            "schemaLine": "1.x",
            "contractsSource": "not-a-uri"
        }
    }"#;

    let package: elegy_contracts::ElegyPluginPackage =
        serde_json::from_str(json).expect("parse package with invalid contractsSource");

    let validation = validate_elegy_plugin_package(&package);
    assert!(validation
        .issues
        .contains(&"elegyCompatibility.contractsSource must be a valid URI.".to_string()));
}

#[test]
fn plugin_lock_v1_parses_valid_lock() {
    let json = r#"{
        "schemaVersion": "elegy-plugin-lock/v1",
        "lockVersion": 1,
        "elegyCompatibility": {
            "contractBundleVersion": "1.8.0",
            "schemaLine": "1.x",
            "sourceAsset": "https://example.com/bundles/elegy-contracts-1.8.0.zip",
            "checksum": "sha256:abc123def456"
        },
        "generatedAt": "2026-06-12T10:00:00Z",
        "generatedBy": "elegy-cli/1.8.0",
        "pluginPackageRef": "elegy-plugin-package.json"
    }"#;

    let lock: elegy_contracts::ElegyPluginLockV1 =
        serde_json::from_str(json).expect("parse valid lock file");

    assert_eq!(lock.schema_version, "elegy-plugin-lock/v1");
    assert_eq!(lock.lock_version, 1);
    assert_eq!(lock.elegy_compatibility.contract_bundle_version, "1.8.0");
    assert_eq!(lock.elegy_compatibility.schema_line, "1.x");
    assert!(lock.elegy_compatibility.source_asset.is_some());
    assert!(lock.elegy_compatibility.checksum.is_some());
    assert_eq!(lock.generated_at, "2026-06-12T10:00:00Z");
}

#[test]
fn plugin_lock_v1_rejects_missing_required_fields() {
    // Missing contractBundleVersion
    let json_missing_version = r#"{
        "schemaVersion": "elegy-plugin-lock/v1",
        "lockVersion": 1,
        "elegyCompatibility": {
            "schemaLine": "1.x"
        },
        "generatedAt": "2026-06-12T10:00:00Z"
    }"#;

    let result: Result<elegy_contracts::ElegyPluginLockV1, _> =
        serde_json::from_str(json_missing_version);
    assert!(
        result.is_err(),
        "lock file with missing required fields should fail to parse"
    );

    // Missing checksum is optional - should parse fine
    let json_no_checksum = r#"{
        "schemaVersion": "elegy-plugin-lock/v1",
        "lockVersion": 1,
        "elegyCompatibility": {
            "contractBundleVersion": "1.8.0",
            "schemaLine": "1.x"
        },
        "generatedAt": "2026-06-12T10:00:00Z"
    }"#;

    let lock: elegy_contracts::ElegyPluginLockV1 =
        serde_json::from_str(json_no_checksum).expect("parse lock without checksum");
    assert!(lock.elegy_compatibility.checksum.is_none());
}

#[test]
fn plugin_lock_v1_round_trips() {
    let lock = elegy_contracts::ElegyPluginLockV1 {
        schema_version: "elegy-plugin-lock/v1".to_string(),
        lock_version: 1,
        elegy_compatibility: elegy_contracts::ElegyPluginLockCompatibility {
            contract_bundle_version: "1.8.0".to_string(),
            schema_line: "1.x".to_string(),
            source_asset: Some("https://example.com/bundle.zip".to_string()),
            checksum: Some("sha256:abc".to_string()),
        },
        generated_at: "2026-06-12T10:00:00Z".to_string(),
        generated_by: Some("elegy-cli/1.8.0".to_string()),
        plugin_package_ref: Some("elegy-plugin-package.json".to_string()),
    };

    let json = serde_json::to_string(&lock).expect("serialize lock");
    let parsed: elegy_contracts::ElegyPluginLockV1 =
        serde_json::from_str(&json).expect("deserialize lock");
    assert_eq!(parsed.schema_version, lock.schema_version);
    assert_eq!(parsed.lock_version, lock.lock_version);
    assert_eq!(
        parsed.elegy_compatibility.contract_bundle_version,
        lock.elegy_compatibility.contract_bundle_version
    );
    assert_eq!(
        parsed.elegy_compatibility.schema_line,
        lock.elegy_compatibility.schema_line
    );
    assert_eq!(
        parsed.elegy_compatibility.checksum,
        lock.elegy_compatibility.checksum
    );
}
