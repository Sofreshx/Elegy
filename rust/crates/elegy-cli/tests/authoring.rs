use elegy_contracts::{
    validate_execution_event, validate_invocation_request, validate_invocation_response,
    ExecutionEvent, ExecutionEventStatus, ExecutionEventType, InvocationContext, InvocationRequest,
    InvocationResponse, InvocationStatus,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}-{unique}"));
    fs::create_dir_all(&dir).expect("create temp directory");
    dir
}

fn rust_workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("rust workspace root")
        .to_path_buf()
}

#[test]
fn author_mcp_command_writes_descriptor_file() {
    let temp_dir = unique_temp_dir("elegy-cli-author");
    let output_path = temp_dir.join("weather-mcp.json");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "author",
            "mcp",
            "--server-name",
            "weather-server",
            "--tool",
            "get-weather=Look up a weather report",
            "--tool",
            "list-alerts",
            "--output",
            output_path.to_str().expect("utf-8 output path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy author mcp");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_path.is_file());

    let descriptor = fs::read_to_string(&output_path).expect("read authored descriptor");
    assert!(descriptor.contains("weather-server"));
    assert!(descriptor.contains("get-weather"));

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"ok\""));
    assert!(stdout.contains("\"output_path\""));
}

#[test]
fn author_mcp_command_supports_machine_flags_and_correlation_id() {
    let temp_dir = unique_temp_dir("elegy-cli-author-machine");
    let output_path = temp_dir.join("machine-weather-mcp.json");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-author-1",
            "author",
            "mcp",
            "--server-name",
            "weather-server",
            "--tool",
            "get-weather=Look up a weather report",
            "--output",
            output_path.to_str().expect("utf-8 output path"),
        ])
        .output()
        .expect("run elegy author mcp with machine flags");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_path.is_file());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"ok\""));
    assert!(stdout.contains("\"correlationId\": \"corr-author-1\""));
    assert!(stdout.contains("\"nonInteractive\": true"));
    assert!(stdout.contains("\"serverName\": \"weather-server\""));
}

#[test]
fn author_mcp_machine_output_maps_cleanly_to_invocation_contracts() {
    let temp_dir = unique_temp_dir("elegy-cli-author-invocation");
    let output_path = temp_dir.join("invocation-weather-mcp.json");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-author-map-1",
            "author",
            "mcp",
            "--server-name",
            "weather-server",
            "--tool",
            "get-weather=Look up a weather report",
            "--output",
            output_path.to_str().expect("utf-8 output path"),
        ])
        .output()
        .expect("run elegy author mcp for invocation mapping");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_path.is_file());

    let envelope: Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be valid machine json");
    assert_eq!(envelope["status"], "ok");
    assert_eq!(envelope["correlationId"], "corr-author-map-1");
    assert_eq!(envelope["nonInteractive"], true);
    assert_eq!(envelope["command"], json!(["author", "mcp"]));

    let request = InvocationRequest {
        request_id: "invoke-author-mcp-1".to_string(),
        capability_id: "elegy.author.mcp".to_string(),
        input: json!({
            "serverName": "weather-server",
            "toolSpecs": ["get-weather=Look up a weather report"],
            "outputPath": output_path.display().to_string()
        }),
        context: InvocationContext {
            correlation_id: envelope["correlationId"]
                .as_str()
                .expect("correlation id string")
                .to_string(),
            execution_id: "exec-author-mcp-1".to_string(),
            requested_at: "2026-03-31T18:00:00Z".to_string(),
            timeout_seconds: Some(30),
            caller_ref: Some("integration-test:elegy-cli-authoring".to_string()),
            policy_context: Some(BTreeMap::from([(
                "mode".to_string(),
                "non-interactive".to_string(),
            )])),
            trace_ref: Some("trace-author-mcp-1".to_string()),
            metadata: BTreeMap::from([(
                "surface".to_string(),
                "elegy-cli-machine-envelope".to_string(),
            )]),
        },
    };
    let request_validation = validate_invocation_request(&request);
    assert!(
        request_validation.is_valid(),
        "unexpected request issues: {:?}",
        request_validation.issues
    );

    let response = InvocationResponse {
        request_id: request.request_id.clone(),
        execution_id: request.context.execution_id.clone(),
        status: InvocationStatus::Completed,
        output: Some(json!({
            "command": envelope["command"].clone(),
            "summary": envelope["summary"].clone(),
            "descriptor": envelope["data"]["descriptor"].clone(),
            "outputPath": envelope["data"]["outputPath"].clone()
        })),
        failure: None,
        completed_at: Some("2026-03-31T18:00:01Z".to_string()),
        trace_ref: request.context.trace_ref.clone(),
        metadata: BTreeMap::from([
            ("surface".to_string(), "elegy-cli".to_string()),
            (
                "mappedFrom".to_string(),
                "author-mcp-machine-output".to_string(),
            ),
        ]),
    };
    let response_validation = validate_invocation_response(&response);
    assert!(
        response_validation.is_valid(),
        "unexpected response issues: {:?}",
        response_validation.issues
    );

    let event = ExecutionEvent {
        event_id: "exec-event-author-mcp-1".to_string(),
        request_id: request.request_id.clone(),
        execution_id: request.context.execution_id.clone(),
        sequence: 1,
        timestamp: "2026-03-31T18:00:00Z".to_string(),
        event_type: ExecutionEventType::Completed,
        status: ExecutionEventStatus::Completed,
        correlation_id: Some(request.context.correlation_id.clone()),
        trace_ref: request.context.trace_ref.clone(),
        capability_id: Some(request.capability_id.clone()),
        message: Some("author mcp completed successfully".to_string()),
        progress: None,
        failure: None,
        metadata: BTreeMap::from([
            ("surface".to_string(), "elegy-cli".to_string()),
            ("command".to_string(), "author mcp".to_string()),
        ]),
    };
    let event_validation = validate_execution_event(&event);
    assert!(
        event_validation.is_valid(),
        "unexpected event issues: {:?}",
        event_validation.issues
    );

    assert_eq!(
        response.output.as_ref().expect("completed output")["descriptor"]["serverName"],
        "weather-server"
    );
}

#[test]
fn analyze_and_generate_commands_use_same_descriptor_input() {
    let temp_dir = unique_temp_dir("elegy-cli-generate");
    let descriptor_path = temp_dir.join("weather-mcp.json");
    let output_dir = temp_dir.join("generated-skills");

    fs::write(
        &descriptor_path,
        r#"{
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
}
"#,
    )
    .expect("write descriptor fixture");

    let analysis = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "analyze",
            "mcp",
            "--descriptor",
            descriptor_path.to_str().expect("utf-8 descriptor path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy analyze mcp");

    assert!(
        analysis.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&analysis.stderr)
    );
    let analysis_stdout = String::from_utf8(analysis.stdout).expect("stdout should be utf-8");
    assert!(analysis_stdout.contains("weather-server"));
    assert!(analysis_stdout.contains("get-weather"));

    let generation = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "generate",
            "skills",
            "--descriptor",
            descriptor_path.to_str().expect("utf-8 descriptor path"),
            "--output-dir",
            output_dir.to_str().expect("utf-8 output dir"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy generate skills");

    assert!(
        generation.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&generation.stderr)
    );
    assert!(output_dir
        .join("mcp-weather-server-get-weather.json")
        .is_file());

    let generation_stdout = String::from_utf8(generation.stdout).expect("stdout should be utf-8");
    assert!(generation_stdout.contains("mcp-weather-server-get-weather"));
    assert!(generation_stdout.contains("list-alerts"));
}

#[test]
fn generate_codex_plugin_command_writes_projected_plugin_bundle() {
    let temp_dir = unique_temp_dir("elegy-cli-generate-codex-plugin");
    let package_path = temp_dir.join("demo-plugin-package.json");
    let output_dir = temp_dir.join("generated-codex-plugin");

    fs::write(
        &package_path,
        r#"{
  "schemaVersion": "elegy-plugin-package/v1",
  "identity": {
    "packageId": "elegy.demo-plugin",
    "name": "demo-plugin",
    "version": "0.1.0",
    "displayName": "Elegy Demo Plugin"
  },
  "metadata": {
    "description": "Portable package fixture for a governed skill definition and optional MCP projection metadata.",
    "tags": ["plugin", "demo"],
    "license": "MIT",
    "homepage": "https://example.com/demo-plugin"
  },
  "components": {
    "skillDefinitions": [
      {
        "id": "demo-skill",
        "definition": {
          "skillFormat": "elegy-skill-definition",
          "skillVersion": 2,
          "identity": {
            "namespace": "elegy",
            "name": "demo-plugin",
            "version": "0.1.0",
            "displayName": "Demo Plugin Skill"
          },
          "metadata": {
            "displayName": "Demo Plugin Skill",
            "description": "Demonstrates portable plugin package capability metadata.",
            "category": "demo",
            "author": "Elegy",
            "tags": ["plugin", "demo"],
            "documentationUri": "docs/architecture/codex-plugin-projection.md"
          },
          "capabilities": [
            {
              "id": "demo-search",
              "name": "Demo Search",
              "description": "Search demo package data.",
              "implementation": {
                "executionType": "mcp",
                "executableName": "elegy-demo-mcp",
                "arguments": ["search", "--query", "${query}", "--json"]
              },
              "input": {
                "parameters": [
                  {
                    "name": "query",
                    "type": "string",
                    "description": "Search query.",
                    "required": true
                  }
                ]
              },
              "execution": {
                "mode": "requestResponse",
                "isDeterministic": true,
                "hasSideEffects": false,
                "timeoutSeconds": 30
              }
            }
          ],
          "governance": {
            "riskLevel": "low",
            "approvalRequirement": "none",
            "policyRefs": []
          },
          "origin": {
            "materializationKind": "declared",
            "sourceKind": "manual",
            "sourceRef": "contracts/fixtures/elegy-plugin-package-v1.minimal.json"
          },
          "lifecycleState": "active"
        }
      }
    ],
    "instructionSkills": [
      {
        "id": "demo-instructions",
        "path": "skills/demo/SKILL.md",
        "description": "Optional instruction surface derived from the governed skill definition."
      }
    ],
    "mcpProjections": [
      {
        "id": "demo-mcp",
        "serverName": "elegy-demo-mcp",
        "capabilityRefs": [
          {
            "skill": "elegy.demo-plugin",
            "capability": "demo-search"
          }
        ]
      }
    ],
    "capabilityProjections": [
      {
        "id": "demo-search-mcp",
        "skill": "elegy.demo-plugin",
        "capability": "demo-search",
        "lane": "mcp",
        "supportsDryRun": true,
        "sideEffectClass": "none",
        "projection": {
          "projections": ["function_calling", "mcp"],
          "functionName": "demo_search",
          "mcpToolName": "demo.search"
        }
      }
    ]
  }
}
"#,
    )
    .expect("write package fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "generate",
            "codex-plugin",
            "--package",
            package_path.to_str().expect("utf-8 package path"),
            "--output-dir",
            output_dir.to_str().expect("utf-8 output dir"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy generate codex-plugin");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let plugin_root = output_dir.join("demo-plugin");
    assert!(plugin_root
        .join(".codex-plugin")
        .join("plugin.json")
        .is_file());
    assert!(plugin_root
        .join("skills")
        .join("skill-elegy_2edemo-plugin")
        .join("SKILL.md")
        .is_file());
    assert!(plugin_root
        .join("skills")
        .join("instruction-demo")
        .join("SKILL.md")
        .is_file());
    assert!(!plugin_root.join(".mcp.json").exists());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"ok\""));
    assert!(stdout.contains("\"command\": [\n    \"generate\",\n    \"codex-plugin\"\n  ]"));
    assert!(stdout.contains("\"pluginName\": \"demo-plugin\""));
    assert!(stdout.contains("\"mcpServersEmitted\": false"));
}

#[test]
fn generate_codex_plugin_command_rejects_unsafe_plugin_output_name() {
    let temp_dir = unique_temp_dir("elegy-cli-generate-codex-plugin-invalid-name");
    let package_path = temp_dir.join("demo-plugin-package.json");
    let output_dir = temp_dir.join("generated-codex-plugin");

    fs::write(
        &package_path,
        r#"{
  "schemaVersion": "elegy-plugin-package/v1",
  "identity": {
    "packageId": "elegy.demo-plugin",
    "name": "nested/name",
    "version": "0.1.0"
  },
  "components": {
    "skillDefinitions": [
      {
        "id": "demo-skill",
        "definition": {
          "skillFormat": "elegy-skill-definition",
          "skillVersion": 2,
          "identity": {
            "namespace": "elegy",
            "name": "demo-plugin",
            "version": "0.1.0"
          },
          "capabilities": [
            {
              "id": "demo-cap",
              "name": "Demo Cap",
              "description": "Demo capability",
              "implementation": {
                "executionType": "subprocess",
                "executableName": "demo",
                "arguments": []
              }
            }
          ],
          "lifecycleState": "active"
        }
      }
    ]
  }
}
"#,
    )
    .expect("write invalid package fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "generate",
            "codex-plugin",
            "--package",
            package_path.to_str().expect("utf-8 package path"),
            "--output-dir",
            output_dir.to_str().expect("utf-8 output dir"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy generate codex-plugin invalid package");

    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("CLI-PLUGIN-001"));
    assert!(stdout.contains("identity.name must be a Codex plugin slug"));
    assert!(!output_dir.exists());
}

#[test]
fn configuration_apply_command_supports_package_profiles() {
    let temp_dir = unique_temp_dir("elegy-cli-config-package-apply");
    let target_dir = temp_dir.join("target");
    let package_path = rust_workspace_root()
        .parent()
        .expect("repo root")
        .join("contracts")
        .join("fixtures")
        .join("elegy-plugin-package-v2.demo-config.json");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "configuration",
            "apply",
            "--package",
            package_path.to_str().expect("utf-8 package path"),
            "--profile-id",
            "demo-profile",
            "--target",
            target_dir.to_str().expect("utf-8 target path"),
            "--json",
        ])
        .output()
        .expect("run elegy configuration apply --package");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["command"], json!(["configuration", "apply"]));
    assert_eq!(body["data"]["sourceKind"], "package");
    assert_eq!(body["data"]["subjectKind"], "profile");
    assert_eq!(body["data"]["subjectId"], "demo-profile");
    assert_eq!(body["data"]["verified"], true);
    assert_eq!(body["data"]["summary"]["created"], 1);
    assert!(body["data"]["sourceRef"]
        .as_str()
        .expect("sourceRef string")
        .contains("#demo-profile"));
    assert_eq!(
        fs::read_to_string(target_dir.join("generated").join("demo.txt")).expect("generated file"),
        "demo\n"
    );
}

#[test]
fn configuration_verify_command_supports_package_profiles() {
    let temp_dir = unique_temp_dir("elegy-cli-config-package-verify");
    let target_dir = temp_dir.join("target");
    let package_path = rust_workspace_root()
        .parent()
        .expect("repo root")
        .join("contracts")
        .join("fixtures")
        .join("elegy-plugin-package-v2.demo-config.json");

    let apply_output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "configuration",
            "apply",
            "--package",
            package_path.to_str().expect("utf-8 package path"),
            "--profile-id",
            "demo-profile",
            "--target",
            target_dir.to_str().expect("utf-8 target path"),
            "--json",
        ])
        .output()
        .expect("run elegy configuration apply before verify");

    assert!(
        apply_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&apply_output.stderr)
    );

    let verify_output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "configuration",
            "verify",
            "--package",
            package_path.to_str().expect("utf-8 package path"),
            "--profile-id",
            "demo-profile",
            "--target",
            target_dir.to_str().expect("utf-8 target path"),
            "--json",
        ])
        .output()
        .expect("run elegy configuration verify --package");

    assert!(
        verify_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&verify_output.stderr)
    );

    let body: Value =
        serde_json::from_slice(&verify_output.stdout).expect("stdout should be valid json");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["command"], json!(["configuration", "verify"]));
    assert_eq!(body["data"]["sourceKind"], "package");
    assert_eq!(body["data"]["subjectKind"], "profile");
    assert_eq!(body["data"]["subjectId"], "demo-profile");
    assert_eq!(body["data"]["verified"], true);
    assert_eq!(body["data"]["summary"]["verified"], 1);
    assert_eq!(body["data"]["summary"]["mismatched"], 0);
}

#[test]
fn piloting_validate_adapter_accepts_valid_targeted_adapter_package() {
    let package_path = rust_workspace_root()
        .parent()
        .expect("repo root")
        .join("contracts")
        .join("fixtures")
        .join("elegy-plugin-package-v2.piloting-blender.json");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "piloting",
            "package",
            "--target",
            "holon",
            package_path.to_str().expect("utf-8 package path"),
            "--json",
        ])
        .output()
        .expect("run elegy piloting package --target holon");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["command"], json!(["piloting", "package"]));
    assert_eq!(body["data"]["target"], "holon");
    assert_eq!(body["data"]["packageId"], "elegy.blender-piloting");
}

#[test]
fn piloting_validate_adapter_rejects_missing_target_identity() {
    let temp_dir = unique_temp_dir("elegy-cli-piloting-missing-target");
    let input_path = temp_dir.join("adapter.json");

    fs::write(
        &input_path,
        r#"{
  "schemaVersion": "elegy-piloting-adapter-manifest/v1",
  "adapterId": "broken.piloting",
  "displayName": "Broken Piloting Adapter",
  "version": "0.1.0",
  "mode": "contracts_only",
  "supportedSoftware": [
    {
      "schemaVersion": "elegy-piloting-target-descriptor/v1",
      "targetId": "",
      "productName": "Broken",
      "versionRange": ">=1.0 <2.0",
      "platforms": ["windows"],
      "launchHints": { "executables": ["broken.exe"], "arguments": [], "urls": [] },
      "attachHints": { "processNames": ["broken.exe"], "windowTitles": [], "surfaceUrls": [] }
    }
  ],
  "supportedSurfaces": [
    {
      "schemaVersion": "elegy-piloting-surface-descriptor/v1",
      "surfaceId": "broken.main",
      "targetId": "broken.desktop",
      "surfaceKind": "desktop",
      "stability": "stable",
      "selectors": [{ "strategy": "windowTitle", "value": "Broken" }],
      "semanticAnchors": []
    }
  ],
  "contracts": {
    "targetDescriptorSchemaRef": "contracts/schemas/piloting-target-descriptor.schema.json",
    "surfaceDescriptorSchemaRef": "contracts/schemas/piloting-surface-descriptor.schema.json",
    "observationFrameSchemaRef": "contracts/schemas/piloting-observation-frame.schema.json",
    "actionIntentSchemaRef": "contracts/schemas/piloting-action-intent.schema.json",
    "actionResultSchemaRef": "contracts/schemas/piloting-action-result.schema.json",
    "readinessReportSchemaRef": "contracts/schemas/piloting-readiness-report.schema.json",
    "fixturePackSchemaRef": "contracts/schemas/piloting-fixture-pack.schema.json",
    "policyDecisionSchemaRef": "contracts/schemas/piloting-policy-decision.schema.json",
    "simulationResultSchemaRef": "contracts/schemas/piloting-simulation-result.schema.json",
    "replayCheckpointSchemaRef": "contracts/schemas/piloting-replay-checkpoint.schema.json",
    "lifecycleEventSchemaRef": "contracts/schemas/piloting-lifecycle-event.schema.json"
  },
  "fixtures": [{ "fixturePackId": "broken.fixtures", "path": "fixtures/broken.json" }],
  "permissions": {
    "sideEffectClasses": ["desktop_ui"],
    "requiresHostApproval": true
  }
}"#,
    )
    .expect("write invalid adapter fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "piloting",
            "validate-adapter",
            input_path.to_str().expect("utf-8 input path"),
            "--json",
        ])
        .output()
        .expect("run elegy piloting validate-adapter invalid target");

    assert_eq!(output.status.code(), Some(1));
    let body: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    assert_eq!(body["status"], "invalid");
    assert!(body["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .any(|entry| entry["code"] == "CLI-PILOT-003"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("targetId must not be empty"));
}

#[test]
fn piloting_package_rejects_undeclared_side_effects() {
    let temp_dir = unique_temp_dir("elegy-cli-piloting-side-effects");
    let input_path = temp_dir.join("package.json");
    let valid_package_path = rust_workspace_root()
        .parent()
        .expect("repo root")
        .join("contracts")
        .join("fixtures")
        .join("elegy-plugin-package-v2.piloting-blender.json");

    let mut package: Value = serde_json::from_str(
        &fs::read_to_string(valid_package_path).expect("read valid piloting package"),
    )
    .expect("parse valid piloting package");
    package["components"]["fixturePacks"][0]["fixturePack"]["allowedActions"][0]
        ["sideEffectClass"] = json!("process_spawn");

    fs::write(
        &input_path,
        serde_json::to_string_pretty(&package).expect("serialize broken package"),
    )
    .expect("write broken package");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "piloting",
            "package",
            "--target",
            "holon",
            input_path.to_str().expect("utf-8 input path"),
            "--json",
        ])
        .output()
        .expect("run elegy piloting package undeclared side effects");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("undeclared sideEffectClass 'process_spawn'"));
}

#[test]
fn piloting_package_rejects_live_actuation_fields_in_contracts_only_package() {
    let temp_dir = unique_temp_dir("elegy-cli-piloting-live-fields");
    let input_path = temp_dir.join("package.json");

    fs::write(
        &input_path,
        r#"{
  "schemaVersion": "elegy-plugin-package/v2",
  "identity": {
    "packageId": "elegy.bad-piloting",
    "name": "bad-piloting",
    "version": "0.1.0"
  },
  "metadata": {
    "description": "Broken piloting package",
    "license": "Apache-2.0"
  },
  "components": {
    "pilotingAdapters": [
      {
        "id": "bad-adapter",
        "manifest": {
          "schemaVersion": "elegy-piloting-adapter-manifest/v1",
          "adapterId": "bad.piloting",
          "displayName": "Bad Piloting Adapter",
          "version": "0.1.0",
          "mode": "live_runtime",
          "supportedSoftware": [
            {
              "schemaVersion": "elegy-piloting-target-descriptor/v1",
              "targetId": "bad.desktop",
              "productName": "Bad",
              "versionRange": ">=1.0 <2.0",
              "platforms": ["windows"],
              "launchHints": { "executables": ["bad.exe"], "arguments": [], "urls": [] },
              "attachHints": { "processNames": ["bad.exe"], "windowTitles": ["Bad"], "surfaceUrls": [] }
            }
          ],
          "supportedSurfaces": [
            {
              "schemaVersion": "elegy-piloting-surface-descriptor/v1",
              "surfaceId": "bad.desktop.main-window",
              "targetId": "bad.desktop",
              "surfaceKind": "desktop",
              "stability": "stable",
              "selectors": [{ "strategy": "windowTitle", "value": "Bad" }],
              "semanticAnchors": []
            }
          ],
          "contracts": {
            "targetDescriptorSchemaRef": "contracts/schemas/piloting-target-descriptor.schema.json",
            "surfaceDescriptorSchemaRef": "contracts/schemas/piloting-surface-descriptor.schema.json",
            "observationFrameSchemaRef": "contracts/schemas/piloting-observation-frame.schema.json",
            "actionIntentSchemaRef": "contracts/schemas/piloting-action-intent.schema.json",
            "actionResultSchemaRef": "contracts/schemas/piloting-action-result.schema.json",
            "readinessReportSchemaRef": "contracts/schemas/piloting-readiness-report.schema.json",
            "fixturePackSchemaRef": "contracts/schemas/piloting-fixture-pack.schema.json",
            "policyDecisionSchemaRef": "contracts/schemas/piloting-policy-decision.schema.json",
            "simulationResultSchemaRef": "contracts/schemas/piloting-simulation-result.schema.json",
            "replayCheckpointSchemaRef": "contracts/schemas/piloting-replay-checkpoint.schema.json",
            "lifecycleEventSchemaRef": "contracts/schemas/piloting-lifecycle-event.schema.json"
          },
          "fixtures": [{ "fixturePackId": "bad.fixtures", "path": "fixtures/bad.json" }],
          "permissions": {
            "sideEffectClasses": ["desktop_ui"],
            "requiresHostApproval": true
          }
        }
      }
    ],
    "fixturePacks": [
      {
        "id": "bad-fixtures",
        "fixturePack": {
          "schemaVersion": "elegy-piloting-fixture-pack/v1",
          "fixturePackId": "bad.fixtures",
          "adapterId": "bad.piloting",
          "targetId": "bad.desktop",
          "recordedAtUtc": "2026-05-28T10:05:00Z",
          "observations": [
            {
              "schemaVersion": "elegy-piloting-observation-frame/v1",
              "frameId": "obs.bad.1",
              "targetId": "bad.desktop",
              "surfaceId": "bad.desktop.main-window",
              "observedAtUtc": "2026-05-28T10:00:00Z",
              "redactionClass": "internal",
              "source": "fixture",
              "confidence": 1,
              "state": {},
              "evidenceRefs": []
            }
          ],
          "allowedActions": [
            {
              "schemaVersion": "elegy-piloting-action-intent/v1",
              "actionId": "click-anything",
              "targetId": "bad.desktop",
              "surfaceId": "bad.desktop.main-window",
              "operation": "click",
              "inputSchema": { "type": "object" },
              "sideEffectClass": "desktop_ui",
              "requiredConfirmation": "explicit"
            }
          ],
          "expectedResultChecks": [
            {
              "actionId": "click-anything",
              "expectedStatus": "succeeded",
              "checks": ["state changes"],
              "evidenceRefs": []
            }
          ],
          "policyDecisions": [
            {
              "schemaVersion": "elegy-piloting-policy-decision/v1",
              "decisionId": "policy.click-anything.1",
              "actionId": "click-anything",
              "targetId": "bad.desktop",
              "decision": "simulate",
              "sideEffectClass": "desktop_ui",
              "evaluatedAtUtc": "2026-05-28T10:01:00Z",
              "approvalRequirement": "explicit",
              "reasons": ["dry-run first"]
            }
          ],
          "simulationResults": [
            {
              "schemaVersion": "elegy-piloting-simulation-result/v1",
              "simulationId": "sim.click-anything.1",
              "actionId": "click-anything",
              "targetId": "bad.desktop",
              "status": "predicted",
              "simulatedAtUtc": "2026-05-28T10:02:00Z",
              "predictedOutcome": {},
              "checks": ["window receives focus"],
              "policyDecisionRef": "policy.click-anything.1"
            }
          ],
          "replayCheckpoints": [
            {
              "schemaVersion": "elegy-piloting-replay-checkpoint/v1",
              "checkpointId": "checkpoint.click-anything.before",
              "actionId": "click-anything",
              "targetId": "bad.desktop",
              "stage": "before",
              "capturedAtUtc": "2026-05-28T10:00:00Z",
              "stateRef": "obs.bad.1"
            }
          ],
          "lifecycleEvents": [
            {
              "schemaVersion": "elegy-piloting-lifecycle-event/v1",
              "eventId": "event.click-anything.intent",
              "actionId": "click-anything",
              "targetId": "bad.desktop",
              "eventType": "intent_recorded",
              "recordedAtUtc": "2026-05-28T10:00:30Z",
              "refId": "click-anything",
              "message": "intent recorded"
            }
          ]
        }
      }
    ]
  },
  "hostPolicyHints": {
    "sideEffectClass": "desktop_ui",
    "requiresApproval": true,
    "policyTags": ["contracts-only"]
  },
  "publishing": {
    "marketplaceTarget": "holon",
    "importMode": "package",
    "sourceRepository": "https://github.com/Sofreshx/Elegy.git",
    "sourceRef": "refs/tags/v0.1.0-bad",
    "sourceCommit": "1111111111111111111111111111111111111111",
    "changelogRef": "docs/changelog/piloting.md",
    "provenanceRef": "contracts/fixtures/piloting-readiness-report.minimal.json",
    "signatureRefs": ["signatures/bad.sig"],
    "compatibility": [{ "host": "holon", "versionRange": ">=0.1.0 <0.2.0" }]
  }
}"#,
    )
    .expect("write invalid live-runtime package fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "piloting",
            "package",
            "--target",
            "holon",
            input_path.to_str().expect("utf-8 input path"),
            "--json",
        ])
        .output()
        .expect("run elegy piloting package invalid live runtime package");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("CLI-PILOT-008") || stdout.contains("CLI-PILOT-012"));
    assert!(stdout.contains("contracts_only") || stdout.contains("live_runtime"));
}

#[test]
fn piloting_package_rejects_missing_holon_marketplace_metadata() {
    let temp_dir = unique_temp_dir("elegy-cli-piloting-missing-metadata");
    let input_path = temp_dir.join("package.json");

    let valid_package_path = rust_workspace_root()
        .parent()
        .expect("repo root")
        .join("contracts")
        .join("fixtures")
        .join("elegy-plugin-package-v2.piloting-blender.json");
    let mut package: Value = serde_json::from_str(
        &fs::read_to_string(valid_package_path).expect("read valid piloting package"),
    )
    .expect("parse valid piloting package");
    package["publishing"]
        .as_object_mut()
        .expect("publishing object")
        .remove("sourceRepository");
    package["publishing"]
        .as_object_mut()
        .expect("publishing object")
        .remove("provenanceRef");
    fs::write(
        &input_path,
        serde_json::to_string_pretty(&package).expect("serialize broken package"),
    )
    .expect("write broken package");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "piloting",
            "package",
            "--target",
            "holon",
            input_path.to_str().expect("utf-8 input path"),
            "--json",
        ])
        .output()
        .expect("run elegy piloting package missing metadata");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("Holon publishing requires publishing.sourceRepository"));
    assert!(stdout.contains("Holon publishing requires publishing.provenanceRef"));
}

#[test]
fn piloting_package_accepts_ref_backed_manifest_and_fixture_pack_components() {
    let temp_dir = unique_temp_dir("elegy-cli-piloting-ref-backed");
    let package_dir = temp_dir.join("package");
    fs::create_dir_all(package_dir.join("signatures")).expect("create signature dir");

    let repo_root = rust_workspace_root()
        .parent()
        .expect("repo root")
        .to_path_buf();
    fs::copy(
        repo_root
            .join("contracts")
            .join("fixtures")
            .join("piloting-adapter-manifest.minimal.json"),
        package_dir.join("adapter.json"),
    )
    .expect("copy adapter manifest");
    let mut adapter_manifest: Value = serde_json::from_str(
        &fs::read_to_string(package_dir.join("adapter.json")).expect("read copied adapter"),
    )
    .expect("parse copied adapter");
    adapter_manifest["fixtures"][0]["path"] = json!("fixture-pack.json");
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
    .expect("copy provenance");
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
    "compatibility": [{ "host": "holon", "versionRange": ">=0.1.0 <0.2.0" }]
  }
}"#,
    )
    .expect("write package");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "piloting",
            "package",
            "--target",
            "holon",
            package_path.to_str().expect("utf-8 package path"),
            "--json",
        ])
        .output()
        .expect("run elegy piloting package ref-backed package");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["data"]["packageId"], "elegy.ref-backed-piloting");
}

#[test]
fn piloting_validate_fixtures_reports_policy_and_replay_counts() {
    let fixture_path = rust_workspace_root()
        .parent()
        .expect("repo root")
        .join("contracts")
        .join("fixtures")
        .join("piloting-fixture-pack.minimal.json");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "piloting",
            "validate-fixtures",
            fixture_path.to_str().expect("utf-8 fixture path"),
            "--json",
        ])
        .output()
        .expect("run elegy piloting validate-fixtures");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["data"]["fixtureKind"], "fixture-pack");
    assert_eq!(body["data"]["policyDecisionCount"], 1);
    assert_eq!(body["data"]["simulationResultCount"], 1);
    assert_eq!(body["data"]["replayCheckpointCount"], 2);
    assert_eq!(body["data"]["lifecycleEventCount"], 4);
}

#[test]
fn piloting_validate_fixtures_rejects_unknown_policy_references() {
    let temp_dir = unique_temp_dir("elegy-cli-piloting-fixture-policy-ref");
    let fixture_path = temp_dir.join("fixture-pack.json");
    let repo_root = rust_workspace_root()
        .parent()
        .expect("repo root")
        .to_path_buf();
    let mut fixture_pack: Value = serde_json::from_str(
        &fs::read_to_string(
            repo_root
                .join("contracts")
                .join("fixtures")
                .join("piloting-fixture-pack.minimal.json"),
        )
        .expect("read piloting fixture pack"),
    )
    .expect("parse piloting fixture pack");
    fixture_pack["simulationResults"][0]["policyDecisionRef"] = json!("missing-policy");
    fs::write(
        &fixture_path,
        serde_json::to_string_pretty(&fixture_pack).expect("serialize modified fixture pack"),
    )
    .expect("write modified fixture pack");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "piloting",
            "validate-fixtures",
            fixture_path.to_str().expect("utf-8 fixture path"),
            "--json",
        ])
        .output()
        .expect("run elegy piloting validate-fixtures invalid policy ref");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("CLI-PILOT-006"));
    assert!(stdout.contains("unknown policyDecisionRef 'missing-policy'"));
}

#[test]
fn piloting_package_rejects_missing_holon_compatibility_entry() {
    let temp_dir = unique_temp_dir("elegy-cli-piloting-missing-holon-compat");
    let input_path = temp_dir.join("package.json");
    let valid_package_path = rust_workspace_root()
        .parent()
        .expect("repo root")
        .join("contracts")
        .join("fixtures")
        .join("elegy-plugin-package-v2.piloting-blender.json");

    let mut package: Value = serde_json::from_str(
        &fs::read_to_string(valid_package_path).expect("read valid piloting package"),
    )
    .expect("parse valid piloting package");
    package["publishing"]["compatibility"] = json!([
      { "host": "someone-else", "versionRange": ">=1.0.0 <2.0.0" }
    ]);

    fs::write(
        &input_path,
        serde_json::to_string_pretty(&package).expect("serialize broken package"),
    )
    .expect("write broken package");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "piloting",
            "package",
            "--target",
            "holon",
            input_path.to_str().expect("utf-8 input path"),
            "--json",
        ])
        .output()
        .expect("run elegy piloting package missing holon compatibility");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("publishing.compatibility entry for host 'holon'"));
}

#[test]
fn configuration_list_command_does_not_claim_missing_catalog_schema() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args(["configuration", "list", "--json"])
        .output()
        .expect("run elegy configuration list --json");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body: Value = serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["command"], json!(["configuration", "list"]));
    assert!(body.get("dataSchema").is_none());
}

#[test]
fn run_dry_run_command_matches_http_example_catalog() {
    let example = rust_workspace_root().join("examples/http-minimal");
    let expected: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(example.join("expected-resources.json"))
            .expect("read expected resources golden"),
    )
    .expect("parse expected resources golden");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "--project",
            example.to_str().expect("utf-8 example path"),
            "--format",
            "json",
            "run",
            "--dry-run",
        ])
        .output()
        .expect("run elegy dry-run against http example");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("dry-run stdout should be valid json");
    assert_eq!(stdout["status"], "ok");
    assert_eq!(stdout["command"], serde_json::json!(["run", "dry-run"]));
    assert_eq!(stdout["data"], expected);
}

#[test]
fn validate_session_context_command_reports_bounded_json_result() {
    let temp_dir = unique_temp_dir("elegy-cli-session-context-json");
    let input_path = temp_dir.join("session-context.json");

    fs::write(
        &input_path,
        r#"{
  "artifactKind": "summary-only-session-context-envelope",
  "requestId": "request-1",
  "runId": "run-1",
  "capturedAtUtc": "2026-03-22T00:00:00Z",
  "sessionContext": {
    "scope": "workspace",
    "representation": "summary-only",
    "summary": "Workspace context persists only bounded summaries for instruction assembly and follow-on agent runs.",
    "salientFacts": [
      "Persist summary and context artifacts only.",
      "Raw execution logs remain transient and are not stored durably."
    ],
    "instructionContext": [
      "Use this summary context when assembling workspace-level instructions."
    ],
    "rawTranscriptPersisted": false
  }
}
"#,
    )
    .expect("write session context fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "validate",
            "session-context",
            "--input",
            input_path.to_str().expect("utf-8 input path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy validate session-context");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"ok\""));
    assert!(stdout.contains("\"artifactKind\": \"summary-only-session-context-envelope\""));
    assert!(stdout.contains("\"scope\": \"workspace\""));
    assert!(stdout.contains("\"readOnly\": true"));
    assert!(stdout.contains("\"hostValidationOwner\": \"SAASTools\""));
}

#[test]
fn validate_session_context_command_reports_bounded_text_result() {
    let temp_dir = unique_temp_dir("elegy-cli-session-context-text");
    let input_path = temp_dir.join("session-context.json");

    fs::write(
        &input_path,
        r#"{
  "artifactKind": "summary-only-session-context-envelope",
  "sessionContext": {
    "scope": "session",
    "representation": "summary-only",
    "summary": "Short handoff summary.",
    "rawTranscriptPersisted": false
  }
}
"#,
    )
    .expect("write session context fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "validate",
            "session-context",
            "--input",
            input_path.to_str().expect("utf-8 input path"),
        ])
        .output()
        .expect("run elegy validate session-context text mode");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("summary-only session context artifact is valid"));
    assert!(stdout.contains("scope: session"));
    assert!(stdout.contains("read only: true"));
    assert!(stdout.contains("host validation owner: SAASTools"));
}

#[test]
fn validate_session_context_command_rejects_invalid_artifact() {
    let temp_dir = unique_temp_dir("elegy-cli-session-context-invalid");
    let input_path = temp_dir.join("invalid-session-context.json");

    fs::write(
        &input_path,
        r#"{
  "artifactKind": "summary-only-session-context-envelope",
  "sessionContext": {
    "scope": "workspace",
    "representation": "summary-only",
    "summary": "Portable summary only.",
    "rawTranscriptPersisted": true
  }
}
"#,
    )
    .expect("write invalid session context fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "validate",
            "session-context",
            "--input",
            input_path.to_str().expect("utf-8 input path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy validate session-context invalid artifact");

    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("CLI-LOCAL-002"));
    assert!(stdout.contains("rawTranscriptPersisted must be false"));
}

#[test]
fn local_cli_is_deterministic_and_hides_non_active_records_by_default() {
    let temp_dir = unique_temp_dir("elegy-cli-local-memory");
    let root = temp_dir.join("local-store");
    let input_a = temp_dir.join("record-a.json");
    let input_b = temp_dir.join("record-b.json");
    let input_c = temp_dir.join("record-c.json");

    fs::write(
        &input_a,
        r#"{
    "artifactKind": "summary-only-session-context-envelope",
    "requestId": "request-a",
    "runId": "run-a",
    "capturedAtUtc": "2026-03-22T00:00:00Z",
    "sessionContext": {
        "scope": "workspace",
        "representation": "summary-only",
        "summary": "First deterministic local summary.",
        "rawTranscriptPersisted": false
  }
}
"#,
    )
    .expect("write record-a fixture");
    fs::write(
        &input_b,
        r#"{
    "artifactKind": "summary-only-session-context-envelope",
    "requestId": "request-b",
    "runId": "run-b",
    "capturedAtUtc": "2026-03-22T01:00:00Z",
    "sessionContext": {
        "scope": "workspace",
        "representation": "summary-only",
        "summary": "Second deterministic local summary.",
        "rawTranscriptPersisted": false
  }
}
"#,
    )
    .expect("write record-b fixture");
    fs::write(
        &input_c,
        r#"{
    "artifactKind": "summary-only-session-context-envelope",
    "requestId": "request-c",
    "runId": "run-c",
    "capturedAtUtc": "2026-03-22T02:00:00Z",
    "sessionContext": {
        "scope": "workspace",
        "representation": "summary-only",
        "summary": "Third deterministic local summary.",
        "rawTranscriptPersisted": false
  }
}
"#,
    )
    .expect("write record-c fixture");

    let init = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "local",
            "init",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy local init");
    assert!(
        init.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&init.stderr)
    );

    let import_a = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "--correlation-id",
            "corr-local-import-repeat",
            "local",
            "import",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--input",
            input_a.to_str().expect("utf-8 input path"),
            "--record-id",
            "record-a",
            "--imported-at-utc",
            "2026-03-23T00:00:00Z",
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy local import record-a");
    assert!(
        import_a.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&import_a.stderr)
    );
    let import_a_repeat = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "--correlation-id",
            "corr-local-import-repeat",
            "local",
            "import",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--input",
            input_a.to_str().expect("utf-8 input path"),
            "--record-id",
            "record-a",
            "--imported-at-utc",
            "2026-03-23T00:00:00Z",
            "--format",
            "json",
        ])
        .output()
        .expect("repeat import record-a");
    assert_eq!(import_a.stdout, import_a_repeat.stdout);

    for (record_id, imported_at_utc, input_path) in [
        ("record-b", "2026-03-23T01:00:00Z", &input_b),
        ("record-c", "2026-03-23T02:00:00Z", &input_c),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
            .args([
                "local",
                "import",
                "--root",
                root.to_str().expect("utf-8 root path"),
                "--input",
                input_path.to_str().expect("utf-8 input path"),
                "--record-id",
                record_id,
                "--imported-at-utc",
                imported_at_utc,
                "--format",
                "json",
            ])
            .output()
            .expect("run local import");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let supersede = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "local",
            "supersede",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--record-id",
            "record-a",
            "--superseded-by-record-id",
            "record-b",
            "--format",
            "json",
        ])
        .output()
        .expect("run local supersede");
    assert!(
        supersede.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&supersede.stderr)
    );

    let tombstone = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "local",
            "tombstone",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--record-id",
            "record-c",
            "--tombstoned-at-utc",
            "2026-03-24T00:00:00Z",
            "--reason",
            "Local tombstone for deterministic test coverage.",
            "--format",
            "json",
        ])
        .output()
        .expect("run local tombstone");
    assert!(
        tombstone.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&tombstone.stderr)
    );

    let default_list = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "--correlation-id",
            "corr-local-list-repeat",
            "local",
            "list",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run local list default");
    assert!(
        default_list.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&default_list.stderr)
    );
    let default_stdout = String::from_utf8(default_list.stdout).expect("stdout should be utf-8");
    assert!(default_stdout.contains("\"recordId\": \"record-b\""));
    assert!(!default_stdout.contains("\"recordId\": \"record-a\""));
    assert!(!default_stdout.contains("\"recordId\": \"record-c\""));
    assert!(default_stdout.contains("local non-authoritative artifact management only"));

    let show_hidden = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "local",
            "show",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--record-id",
            "record-a",
            "--format",
            "json",
        ])
        .output()
        .expect("run local show hidden record");
    assert!(!show_hidden.status.success());
    let show_hidden_stdout = String::from_utf8(show_hidden.stdout).expect("stdout should be utf-8");
    assert!(show_hidden_stdout.contains("CLI-LOCAL-006"));

    let list_all_one = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "--correlation-id",
            "corr-local-list-repeat",
            "local",
            "list",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--include-superseded",
            "--include-tombstoned",
            "--format",
            "json",
        ])
        .output()
        .expect("run local list all one");
    let list_all_two = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "--correlation-id",
            "corr-local-list-repeat",
            "local",
            "list",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--include-superseded",
            "--include-tombstoned",
            "--format",
            "json",
        ])
        .output()
        .expect("run local list all two");
    assert_eq!(list_all_one.stdout, list_all_two.stdout);
    let list_all_stdout = String::from_utf8(list_all_one.stdout).expect("stdout should be utf-8");
    let index_a = list_all_stdout
        .find("\"recordId\": \"record-a\"")
        .expect("record-a in list");
    let index_b = list_all_stdout
        .find("\"recordId\": \"record-b\"")
        .expect("record-b in list");
    let index_c = list_all_stdout
        .find("\"recordId\": \"record-c\"")
        .expect("record-c in list");
    assert!(index_a < index_b && index_b < index_c);

    let show_one = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "--correlation-id",
            "corr-local-show-repeat",
            "local",
            "show",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--record-id",
            "record-b",
            "--format",
            "json",
        ])
        .output()
        .expect("run local show one");
    let show_two = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "--correlation-id",
            "corr-local-show-repeat",
            "local",
            "show",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--record-id",
            "record-b",
            "--format",
            "json",
        ])
        .output()
        .expect("run local show two");
    assert_eq!(show_one.stdout, show_two.stdout);

    let export_one = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "--correlation-id",
            "corr-local-export-repeat",
            "local",
            "export",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--record-id",
            "record-b",
            "--format",
            "json",
        ])
        .output()
        .expect("run local export one");
    let export_two = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "--correlation-id",
            "corr-local-export-repeat",
            "local",
            "export",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--record-id",
            "record-b",
            "--format",
            "json",
        ])
        .output()
        .expect("run local export two");
    assert_eq!(export_one.stdout, export_two.stdout);

    let export_path = root
        .join("exports")
        .join("record-b.summary-only-session-context-envelope.json");
    let exported_contents = fs::read_to_string(export_path).expect("read exported artifact");
    assert!(exported_contents.contains("summary-only-session-context-envelope"));
}
