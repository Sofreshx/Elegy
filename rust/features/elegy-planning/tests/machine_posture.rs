use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_nanos();
    let pid = std::process::id();
    let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("{prefix}-{pid}-{unique}-{counter}"));
    fs::create_dir_all(&dir).expect("create temp directory");
    dir
}

fn command_json(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args(args)
        .output()
        .expect("run elegy-planning command");

    assert!(
        output.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("stdout should be valid json")
}

#[test]
fn capabilities_reports_lease_contract_without_initializing_a_database() {
    let temp_dir = unique_temp_dir("elegy-planning-capabilities");
    let db_path = temp_dir.join("missing-parent").join("planning.db");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "capabilities",
        ])
        .output()
        .expect("run elegy-planning capabilities");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !db_path.exists(),
        "capability discovery must not initialize storage"
    );

    let envelope: Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be valid json");
    assert_eq!(envelope["schemaVersion"], "planning-result/v1");
    assert_eq!(envelope["command"], serde_json::json!(["capabilities"]));
    assert_eq!(envelope["status"], "ok");
    assert_eq!(envelope["data"]["planningSchemaVersion"], "10");
    assert_eq!(
        envelope["data"]["capabilities"],
        serde_json::json!([
            "project-run.claim.v2",
            "project-run.activate.fenced.v1",
            "project-run.heartbeat.v1",
            "project-run.release.fenced.v1",
            "project-run.add-evidence.fenced.v1"
        ])
    );
}

#[test]
fn goal_create_supports_machine_flags_and_correlation_id() {
    let temp_dir = unique_temp_dir("elegy-planning-machine-goal");
    let db_path = temp_dir.join("planning.db");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-plan-machine-1",
            "goal",
            "create",
            "--id",
            "goal-machine-1",
            "--title",
            "Ship planning CLI",
            "--description",
            "Create the first planning authority.",
            "--acceptance",
            "crate exists",
            "--rejection",
            "planning remains memory-only",
        ])
        .output()
        .expect("run elegy-planning goal create");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"ok\""));
    assert!(stdout.contains("\"correlationId\": \"corr-plan-machine-1\""));
    assert!(stdout.contains("\"nonInteractive\": true"));
    assert!(stdout.contains("\"goal-machine-1\""));
}

#[test]
fn missing_parent_emits_structured_invalid_error_with_machine_flags() {
    let temp_dir = unique_temp_dir("elegy-planning-machine-invalid");
    let db_path = temp_dir.join("planning.db");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-plan-invalid-1",
            "roadmap",
            "create",
            "--id",
            "roadmap-invalid-1",
            "--goal-id",
            "missing-goal",
            "--title",
            "Roadmap",
            "--summary",
            "Summary",
        ])
        .output()
        .expect("run elegy-planning roadmap create invalid parent");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("\"correlationId\": \"corr-plan-invalid-1\""));
    assert!(stdout.contains("goalId references missing goal `missing-goal`"));
}

#[test]
fn project_render_uses_projection_format_without_colliding_with_global_format() {
    let temp_dir = unique_temp_dir("elegy-planning-machine-render");
    let db_path = temp_dir.join("planning.db");
    let output_path = temp_dir.join("roadmap.md");

    let create_goal = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-plan-machine-2",
            "goal",
            "create",
            "--id",
            "goal-machine-2",
            "--title",
            "Ship planning renderer",
            "--description",
            "Render projections without flag collisions.",
            "--acceptance",
            "markdown renders",
            "--rejection",
            "clap panics",
        ])
        .output()
        .expect("create goal for render test");
    assert!(create_goal.status.success());

    let create_roadmap = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-plan-machine-2",
            "roadmap",
            "create",
            "--id",
            "roadmap-machine-2",
            "--goal-id",
            "goal-machine-2",
            "--title",
            "Planning renderer roadmap",
            "--summary",
            "Ensure projection rendering works.",
        ])
        .output()
        .expect("create roadmap for render test");
    assert!(create_roadmap.status.success());

    let render = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-plan-machine-2",
            "project",
            "render",
            "--entity-type",
            "roadmap",
            "--entity-id",
            "roadmap-machine-2",
            "--projection-format",
            "markdown",
            "--output",
            output_path.to_str().expect("utf-8 output path"),
        ])
        .output()
        .expect("render roadmap projection");

    assert!(
        render.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&render.stderr)
    );
    let stdout = String::from_utf8(render.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"ok\""));
    assert!(stdout.contains("\"format\": \"markdown\""));

    let rendered = fs::read_to_string(output_path).expect("rendered roadmap markdown");
    assert!(rendered.contains("# Planning renderer roadmap"));
}

#[test]
fn parse_time_invalid_enum_emits_structured_invalid_json() {
    let temp_dir = unique_temp_dir("elegy-planning-machine-parse-invalid");
    let db_path = temp_dir.join("planning.db");

    let create_goal = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-plan-parse-1",
            "goal",
            "create",
            "--id",
            "goal-parse-1",
            "--title",
            "Goal",
            "--description",
            "Desc",
            "--acceptance",
            "ok",
            "--rejection",
            "no",
        ])
        .output()
        .expect("create goal for parse-time invalid test");
    assert!(create_goal.status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-plan-parse-1",
            "roadmap",
            "create",
            "--id",
            "roadmap-parse-1",
            "--goal-id",
            "goal-parse-1",
            "--title",
            "Roadmap",
            "--summary",
            "Summary",
            "--status",
            "done",
        ])
        .output()
        .expect("run elegy-planning roadmap create with invalid enum");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("\"correlationId\": \"corr-plan-parse-1\""));
    assert!(stdout.contains("\"command\": [\n    \"roadmap\",\n    \"create\""));
    assert!(stdout.contains("invalid value 'done' for '--status <STATUS>'"));
}

#[test]
fn parse_time_invalid_enum_emits_structured_invalid_json_with_format_flag() {
    let temp_dir = unique_temp_dir("elegy-planning-machine-parse-format-invalid");
    let db_path = temp_dir.join("planning.db");

    let create_goal = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--format",
            "json",
            "--non-interactive",
            "--correlation-id",
            "corr-plan-format-1",
            "goal",
            "create",
            "--id",
            "goal-format-1",
            "--title",
            "Goal",
            "--description",
            "Desc",
            "--acceptance",
            "ok",
            "--rejection",
            "no",
        ])
        .output()
        .expect("create goal for format-flag parse invalid test");
    assert!(create_goal.status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--format",
            "json",
            "--non-interactive",
            "--correlation-id",
            "corr-plan-format-1",
            "roadmap",
            "create",
            "--id",
            "roadmap-format-1",
            "--goal-id",
            "goal-format-1",
            "--title",
            "Roadmap",
            "--summary",
            "Summary",
            "--status",
            "done",
        ])
        .output()
        .expect("run elegy-planning roadmap create with invalid enum under --format json");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("\"correlationId\": \"corr-plan-format-1\""));
    assert!(stdout.contains("invalid value 'done' for '--status <STATUS>'"));
}

#[test]
fn out_of_scope_update_status_returns_structured_invalid_json() {
    let temp_dir = unique_temp_dir("elegy-planning-machine-scope-invalid");
    let db_path = temp_dir.join("planning.db");

    let create_scope = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "scope",
            "create",
            "--scope-key",
            "workspace-a",
            "--scope-type",
            "workspace",
        ])
        .output()
        .expect("create workspace-a scope");
    assert!(create_scope.status.success());

    let create_goal = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "workspace-a",
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-scope-invalid-1",
            "goal",
            "create",
            "--id",
            "goal-scope-invalid-1",
            "--title",
            "Scoped goal",
            "--description",
            "Workspace goal",
            "--acceptance",
            "ok",
            "--rejection",
            "no",
        ])
        .output()
        .expect("create scoped goal");
    assert!(create_goal.status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "default",
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-scope-invalid-2",
            "goal",
            "update-status",
            "--goal-id",
            "goal-scope-invalid-1",
            "--status",
            "validated",
        ])
        .output()
        .expect("run out-of-scope update-status");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("goal `goal-scope-invalid-1` is in scope `workspace-a`"));
    assert!(stdout.contains("\"correlationId\": \"corr-scope-invalid-2\""));
}

#[test]
fn plan_revise_rejects_conflicting_clear_flags() {
    let temp_dir = unique_temp_dir("elegy-planning-machine-plan-revise-conflict");
    let db_path = temp_dir.join("planning.db");
    let db = db_path.to_str().expect("utf-8 db path");

    let _ = command_json(&[
        "--db",
        db,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-plan-conflict-1",
        "goal",
        "create",
        "--id",
        "goal-plan-conflict-1",
        "--title",
        "Goal",
        "--description",
        "Desc",
        "--acceptance",
        "ok",
        "--rejection",
        "no",
    ]);

    let _ = command_json(&[
        "--db",
        db,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-plan-conflict-1",
        "roadmap",
        "create",
        "--id",
        "roadmap-plan-conflict-1",
        "--goal-id",
        "goal-plan-conflict-1",
        "--title",
        "Roadmap",
        "--summary",
        "Summary",
    ]);

    let _ = command_json(&[
        "--db",
        db,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-plan-conflict-1",
        "plan",
        "create",
        "--id",
        "plan-conflict-1",
        "--goal-id",
        "goal-plan-conflict-1",
        "--roadmap-id",
        "roadmap-plan-conflict-1",
        "--title",
        "Plan",
        "--summary",
        "Summary",
        "--scope",
        "Execution",
    ]);

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-plan-conflict-2",
            "plan",
            "revise",
            "--plan-id",
            "plan-conflict-1",
            "--clear-routing-hint",
            "--routing-hint",
            "flash-lane",
        ])
        .output()
        .expect("run plan revise with conflicting clear routing hint flags");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("--clear-routing-hint cannot be combined with --routing-hint"));
}

#[test]
fn events_are_isolated_by_scope_in_machine_mode() {
    let temp_dir = unique_temp_dir("elegy-planning-machine-events-scope");
    let db_path = temp_dir.join("planning.db");
    let db = db_path.to_str().expect("utf-8 db path");

    let _ = command_json(&[
        "--db",
        db,
        "--json",
        "--non-interactive",
        "scope",
        "create",
        "--scope-key",
        "workspace-a",
        "--scope-type",
        "workspace",
    ]);

    let _ = command_json(&[
        "--db",
        db,
        "--scope",
        "default",
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-events-default",
        "goal",
        "create",
        "--id",
        "goal-events-default",
        "--title",
        "Default goal",
        "--description",
        "Default scope goal",
        "--acceptance",
        "ok",
        "--rejection",
        "no",
    ]);

    let _ = command_json(&[
        "--db",
        db,
        "--scope",
        "workspace-a",
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-events-custom",
        "goal",
        "create",
        "--id",
        "goal-events-custom",
        "--title",
        "Custom goal",
        "--description",
        "Custom scope goal",
        "--acceptance",
        "ok",
        "--rejection",
        "no",
    ]);

    let default_events = command_json(&[
        "--db",
        db,
        "--scope",
        "default",
        "--json",
        "--non-interactive",
        "events",
    ]);
    let workspace_events = command_json(&[
        "--db",
        db,
        "--scope",
        "workspace-a",
        "--json",
        "--non-interactive",
        "events",
    ]);

    let default_events = default_events["data"]["events"]
        .as_array()
        .expect("default events array");
    let workspace_events = workspace_events["data"]["events"]
        .as_array()
        .expect("workspace events array");

    assert!(default_events
        .iter()
        .any(|event| { event["entityId"].as_str() == Some("goal-events-default") }));
    assert!(!default_events
        .iter()
        .any(|event| { event["entityId"].as_str() == Some("goal-events-custom") }));
    assert!(workspace_events
        .iter()
        .any(|event| { event["entityId"].as_str() == Some("goal-events-custom") }));
    assert!(!workspace_events
        .iter()
        .any(|event| { event["entityId"].as_str() == Some("goal-events-default") }));
}

#[test]
fn out_of_scope_project_render_returns_structured_invalid_json() {
    let temp_dir = unique_temp_dir("elegy-planning-machine-render-scope-invalid");
    let db_path = temp_dir.join("planning.db");
    let output_path = temp_dir.join("goal.json");

    let create_scope = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "scope",
            "create",
            "--scope-key",
            "workspace-a",
            "--scope-type",
            "workspace",
        ])
        .output()
        .expect("create workspace-a scope");
    assert!(create_scope.status.success());

    let create_goal = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "workspace-a",
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-render-scope-1",
            "goal",
            "create",
            "--id",
            "goal-render-scope-1",
            "--title",
            "Scoped goal",
            "--description",
            "Workspace goal",
            "--acceptance",
            "ok",
            "--rejection",
            "no",
        ])
        .output()
        .expect("create scoped goal");
    assert!(create_goal.status.success());

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "default",
            "--json",
            "--non-interactive",
            "project",
            "render",
            "--entity-type",
            "goal",
            "--entity-id",
            "goal-render-scope-1",
            "--projection-format",
            "json",
            "--output",
            output_path.to_str().expect("utf-8 output path"),
        ])
        .output()
        .expect("run out-of-scope project render");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("goal `goal-render-scope-1` is in scope `workspace-a`"));
    assert!(!output_path.exists());
}

#[test]
fn scoped_validate_all_excludes_findings_from_other_scopes() {
    let temp_dir = unique_temp_dir("elegy-planning-scoped-validate");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    // Create scope A with a goal (no acceptance criteria -> validation finding)
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c1",
        "--scope",
        "scope-a",
        "scope",
        "create",
        "--scope-key",
        "scope-a",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        "scope-a",
        "goal",
        "create",
        "--id",
        "goal-a",
        "--title",
        "Goal A",
        "--description",
        "Test",
    ]);

    // Create scope B with a goal (no acceptance criteria -> validation finding)
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c3",
        "--scope",
        "scope-b",
        "scope",
        "create",
        "--scope-key",
        "scope-b",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c4",
        "--scope",
        "scope-b",
        "goal",
        "create",
        "--id",
        "goal-b",
        "--title",
        "Goal B",
        "--description",
        "Test",
    ]);

    // Validate scope A only
    let result_a = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c5",
        "--scope",
        "scope-a",
        "validate",
        "all",
    ]);

    assert_eq!(result_a["status"], "ok");
    assert_eq!(result_a["data"]["scopeMode"], "single");
    assert_eq!(result_a["data"]["scopeKey"], "scope-a");

    // All findings should be for scope-a entities
    if let Some(findings) = result_a["data"]["findings"].as_array() {
        for finding in findings {
            assert_eq!(
                finding["scopeKey"], "scope-a",
                "finding {:?} should be in scope-a",
                finding["code"]
            );
            assert!(
                finding["fingerprint"]
                    .as_str()
                    .unwrap_or("")
                    .contains("scope-a"),
                "fingerprint should contain scope key"
            );
        }
    }

    // Validate all scopes
    let result_all = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c6",
        "--scope",
        "scope-a",
        "validate",
        "all",
        "--all-scopes",
    ]);

    assert_eq!(result_all["status"], "ok");
    assert_eq!(result_all["data"]["scopeMode"], "all");
    assert_eq!(result_all["data"]["scopeKey"], "all");

    // Should have findings from both scopes
    if let Some(findings) = result_all["data"]["findings"].as_array() {
        let has_scope_a = findings.iter().any(|f| f["scopeKey"] == "scope-a");
        let has_scope_b = findings.iter().any(|f| f["scopeKey"] == "scope-b");
        assert!(has_scope_a, "should have scope-a findings");
        assert!(has_scope_b, "should have scope-b findings");
    }
}

#[test]
fn cross_roadmap_work_point_dependency_rejected_at_write_time() {
    let temp_dir = unique_temp_dir("elegy-planning-cross-roadmap");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    // Setup: goal + two roadmaps
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c1",
        "--scope",
        "scope-a",
        "scope",
        "create",
        "--scope-key",
        "scope-a",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        "scope-a",
        "goal",
        "create",
        "--id",
        "goal-x",
        "--title",
        "Goal X",
        "--description",
        "Test",
        "--acceptance",
        "done",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c3",
        "--scope",
        "scope-a",
        "roadmap",
        "create",
        "--id",
        "rm-a",
        "--goal-id",
        "goal-x",
        "--title",
        "Roadmap A",
        "--summary",
        "First",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c4",
        "--scope",
        "scope-a",
        "roadmap",
        "create",
        "--id",
        "rm-b",
        "--goal-id",
        "goal-x",
        "--title",
        "Roadmap B",
        "--summary",
        "Second",
    ]);

    // Add work point to rm-a
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c5",
        "--scope",
        "scope-a",
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-a",
        "--id",
        "wp-a",
        "--title",
        "WP A",
        "--summary",
        "First work point",
        "--effort-tier",
        "fast",
    ]);

    // Try to add wp-b to rm-b with dependency on wp-a (cross-roadmap) — should fail
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c6",
            "--scope",
            "scope-a",
            "roadmap",
            "add-work-point",
            "--roadmap-id",
            "rm-b",
            "--id",
            "wp-b",
            "--title",
            "WP B",
            "--summary",
            "Second work point",
            "--dependency-id",
            "wp-a",
            "--effort-tier",
            "fast",
        ])
        .output()
        .expect("run add-work-point");

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(result["status"], "invalid");
    assert!(
        result["error"]
            .as_str()
            .unwrap_or("")
            .contains("Cross-roadmap"),
        "error should mention cross-roadmap: {}",
        result["error"]
    );
}

#[test]
fn work_point_revise_clear_dependencies() {
    let temp_dir = unique_temp_dir("elegy-planning-clear-deps");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    // Setup
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c1",
        "--scope",
        "scope-a",
        "scope",
        "create",
        "--scope-key",
        "scope-a",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        "scope-a",
        "goal",
        "create",
        "--id",
        "goal-x",
        "--title",
        "Goal X",
        "--description",
        "Test",
        "--acceptance",
        "done",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c3",
        "--scope",
        "scope-a",
        "roadmap",
        "create",
        "--id",
        "rm-a",
        "--goal-id",
        "goal-x",
        "--title",
        "Roadmap A",
        "--summary",
        "First",
    ]);

    // Add two work points, wp-b depends on wp-a
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c4",
        "--scope",
        "scope-a",
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-a",
        "--id",
        "wp-a",
        "--title",
        "WP A",
        "--summary",
        "First",
        "--effort-tier",
        "fast",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c5",
        "--scope",
        "scope-a",
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-a",
        "--id",
        "wp-b",
        "--title",
        "WP B",
        "--summary",
        "Second",
        "--dependency-id",
        "wp-a",
        "--effort-tier",
        "fast",
    ]);

    // Verify wp-b has dependency on wp-a
    let show1 = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        "scope-a",
        "work-point",
        "show",
        "--work-point-id",
        "wp-b",
    ]);
    let deps1 = show1["data"]["workPoint"]["dependencyIds"]
        .as_array()
        .expect("deps array");
    assert!(!deps1.is_empty(), "wp-b should have dependencies");
    assert!(deps1.iter().any(|d| d.as_str() == Some("wp-a")));

    // Revise wp-b to clear dependencies
    let revise = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c6",
        "--scope",
        "scope-a",
        "work-point",
        "revise",
        "--work-point-id",
        "wp-b",
        "--clear-dependencies",
    ]);
    assert_eq!(revise["status"], "ok");

    // Verify wp-b has no dependencies now
    let show2 = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        "scope-a",
        "work-point",
        "show",
        "--work-point-id",
        "wp-b",
    ]);
    let deps2 = show2["data"]["workPoint"]["dependencyIds"]
        .as_array()
        .expect("deps array");
    assert!(
        deps2.is_empty(),
        "wp-b should have no dependencies after clear: {:?}",
        deps2
    );
}

#[test]
fn work_point_revise_rejects_conflicting_clear_flags() {
    let temp_dir = unique_temp_dir("elegy-planning-revise-conflict");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    // Setup
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c1",
        "--scope",
        "scope-a",
        "scope",
        "create",
        "--scope-key",
        "scope-a",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        "scope-a",
        "goal",
        "create",
        "--id",
        "goal-x",
        "--title",
        "Goal X",
        "--description",
        "Test",
        "--acceptance",
        "done",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c3",
        "--scope",
        "scope-a",
        "roadmap",
        "create",
        "--id",
        "rm-a",
        "--goal-id",
        "goal-x",
        "--title",
        "Roadmap A",
        "--summary",
        "First",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c4",
        "--scope",
        "scope-a",
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-a",
        "--id",
        "wp-a",
        "--title",
        "WP A",
        "--summary",
        "First",
        "--effort-tier",
        "fast",
    ]);

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c5",
            "--scope",
            "scope-a",
            "work-point",
            "revise",
            "--work-point-id",
            "wp-a",
            "--clear-dependencies",
            "--dependency-id",
            "other",
        ])
        .output()
        .expect("run revise");

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(result["status"], "invalid");
}

#[test]
fn scope_create_metadata_file() {
    let temp_dir = unique_temp_dir("elegy-planning-metadata-file");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    // Write a metadata JSON file
    let meta_path = temp_dir.join("meta.json");
    fs::write(&meta_path, r#"{"key": "value", "count": 42}"#).expect("write metadata file");

    let result = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c1",
        "--scope",
        "scope-a",
        "scope",
        "create",
        "--scope-key",
        "scope-a",
        "--metadata-file",
        meta_path.to_str().expect("utf-8 path"),
    ]);

    assert_eq!(result["status"], "ok");

    // Verify metadata was stored
    let show = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        "scope-a",
        "scope",
        "show",
        "--scope-key",
        "scope-a",
    ]);
    let metadata = &show["data"]["scope"]["metadata"];
    assert_eq!(metadata["key"], "value");
    assert_eq!(metadata["count"], 42);
}

#[test]
fn scope_create_metadata_file_rejects_bad_json_with_path_aware_error() {
    let temp_dir = unique_temp_dir("elegy-planning-meta-err");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    let meta_path = temp_dir.join("bad.json");
    fs::write(&meta_path, "not valid json!!!").expect("write bad metadata");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c1",
            "--scope",
            "scope-a",
            "scope",
            "create",
            "--scope-key",
            "scope-a",
            "--metadata-file",
            meta_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run scope create");

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(result["status"], "invalid");
    let error = result["error"].as_str().unwrap_or("");
    assert!(
        error.contains("bad.json"),
        "error should mention file path: {}",
        error
    );
}

#[test]
fn insight_list_all_lists_only_active_scope_insights() {
    let temp_dir = unique_temp_dir("elegy-planning-insight-all");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    // Create scope A and scope B
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c1",
        "--scope",
        "scope-a",
        "scope",
        "create",
        "--scope-key",
        "scope-a",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        "scope-b",
        "scope",
        "create",
        "--scope-key",
        "scope-b",
    ]);

    // Create goals in each scope (needed as parent for insights)
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c3",
        "--scope",
        "scope-a",
        "goal",
        "create",
        "--id",
        "goal-a",
        "--title",
        "Goal A",
        "--description",
        "Test",
        "--acceptance",
        "done",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c4",
        "--scope",
        "scope-b",
        "goal",
        "create",
        "--id",
        "goal-b",
        "--title",
        "Goal B",
        "--description",
        "Test",
        "--acceptance",
        "done",
    ]);

    // Record insights in both scopes
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c5",
        "--scope",
        "scope-a",
        "insight",
        "record",
        "--id",
        "insight-a",
        "--title",
        "Insight A",
        "--content",
        "Scope A content",
        "--insight-type",
        "context",
        "--parent-type",
        "goal",
        "--parent-id",
        "goal-a",
        "--tag",
        "test",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c6",
        "--scope",
        "scope-b",
        "insight",
        "record",
        "--id",
        "insight-b",
        "--title",
        "Insight B",
        "--content",
        "Scope B content",
        "--insight-type",
        "context",
        "--parent-type",
        "goal",
        "--parent-id",
        "goal-b",
        "--tag",
        "test",
    ]);

    // List all insights in scope-a
    let result = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        "scope-a",
        "insight",
        "list",
        "--all",
    ]);

    assert_eq!(result["status"], "ok");
    let insights = result["data"]["insights"]
        .as_array()
        .expect("insights array");
    assert_eq!(insights.len(), 1);
    assert_eq!(insights[0]["title"], "Insight A");

    // List all insights in scope-b
    let result_b = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        "scope-b",
        "insight",
        "list",
        "--all",
    ]);
    let insights_b = result_b["data"]["insights"]
        .as_array()
        .expect("insights array");
    assert_eq!(insights_b.len(), 1);
    assert_eq!(insights_b[0]["title"], "Insight B");
}

#[test]
fn machine_output_conforms_to_planning_result_schema() {
    let temp_dir = unique_temp_dir("elegy-planning-schema-validate");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    // Load the schema
    let schema_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR should have a parent (crate root)")
        .parent()
        .expect("crate root should have a parent (workspace root)")
        .parent()
        .expect("workspace root should have a parent (repo root)")
        .join("contracts")
        .join("schemas")
        .join("planning-result.schema.json");
    let schema_json: Value =
        serde_json::from_str(&std::fs::read_to_string(&schema_path).expect("read schema file"))
            .expect("parse schema");
    let schema = jsonschema::validator_for(&schema_json).expect("compile schema");

    // Test 1: goal create output
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c1",
        "--scope",
        "default",
        "scope",
        "create",
        "--scope-key",
        "default",
    ]);

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c2",
            "--scope",
            "default",
            "goal",
            "create",
            "--id",
            "goal-schema",
            "--title",
            "Schema Goal",
            "--description",
            "Test",
            "--acceptance",
            "done",
        ])
        .output()
        .expect("run");
    let instance: Value = serde_json::from_slice(&output.stdout).expect("parse output");

    if let Err(error) = schema.validate(&instance) {
        eprintln!("Schema validation error: {}", error);
        panic!("output does not conform to planning-result schema");
    }

    // Test 2: validate all output with new fields
    let validate_output = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c3",
        "--scope",
        "default",
        "validate",
        "all",
    ]);
    if let Err(error) = schema.validate(&validate_output) {
        eprintln!("Schema validation error on validate output: {}", error);
        panic!("validate output does not conform to schema");
    }

    // Test 3: error output (invalid command)
    let err_output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c4",
            "goal",
            "create",
            "--id",
            "nonexistent",
            "--title",
            "X",
            "--description",
            "X",
        ])
        .output()
        .expect("run");
    // Might succeed or fail - if it has json output, validate it
    if let Ok(instance) = serde_json::from_slice::<Value>(&err_output.stdout) {
        if instance.get("status").is_some() {
            let validation = schema.validate(&instance);
            if let Err(errors) = validation {
                eprintln!("Error output schema error: {}", errors);
                panic!("error output does not conform to schema");
            }
        }
    }
}

// ===================================================================
// FIX 4: Worktree scope isolation tests
// ===================================================================

#[test]
fn worktree_scope_isolation_list() {
    let temp_dir = unique_temp_dir("elegy-planning-wt-scope-list");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    // Create two scopes
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c1",
        "--scope",
        "scope-a",
        "scope",
        "create",
        "--scope-key",
        "scope-a",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        "scope-b",
        "scope",
        "create",
        "--scope-key",
        "scope-b",
    ]);

    // Attach worktree in scope-a with ID "wt-1"
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c3",
        "--scope",
        "scope-a",
        "worktree",
        "attach",
        "--id",
        "wt-1",
        "--repo-uri",
        "https://example.com/repo.git",
    ]);

    // Attach worktree in scope-b with ID "wt-2"
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c4",
        "--scope",
        "scope-b",
        "worktree",
        "attach",
        "--id",
        "wt-2",
        "--repo-uri",
        "https://example.com/other.git",
    ]);

    // List in scope-a — should see wt-1 but not wt-2
    let list_a = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        "scope-a",
        "worktree",
        "list",
    ]);
    let wt_a_ids: Vec<&str> = list_a["data"]["worktrees"]
        .as_array()
        .expect("worktrees array")
        .iter()
        .filter_map(|w| w["id"].as_str())
        .collect();
    assert!(
        wt_a_ids.contains(&"wt-1"),
        "scope-a should contain wt-1: {:?}",
        wt_a_ids
    );
    assert!(
        !wt_a_ids.contains(&"wt-2"),
        "scope-a should NOT contain wt-2: {:?}",
        wt_a_ids
    );

    // List in scope-b — should see wt-2 but not wt-1
    let list_b = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        "scope-b",
        "worktree",
        "list",
    ]);
    let wt_b_ids: Vec<&str> = list_b["data"]["worktrees"]
        .as_array()
        .expect("worktrees array")
        .iter()
        .filter_map(|w| w["id"].as_str())
        .collect();
    assert!(
        wt_b_ids.contains(&"wt-2"),
        "scope-b should contain wt-2: {:?}",
        wt_b_ids
    );
    assert!(
        !wt_b_ids.contains(&"wt-1"),
        "scope-b should NOT contain wt-1: {:?}",
        wt_b_ids
    );
}

#[test]
fn worktree_scope_show_rejects_wrong_scope() {
    let temp_dir = unique_temp_dir("elegy-planning-wt-scope-show");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c1",
        "--scope",
        "scope-a",
        "scope",
        "create",
        "--scope-key",
        "scope-a",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        "scope-a",
        "worktree",
        "attach",
        "--id",
        "wt-1",
        "--repo-uri",
        "https://example.com/repo.git",
    ]);

    // Try show in scope-b — expect status "invalid"
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--scope",
            "scope-b",
            "worktree",
            "show",
            "--id",
            "wt-1",
        ])
        .output()
        .expect("run worktree show in wrong scope");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(
        result["status"], "invalid",
        "show in wrong scope should be invalid: {}",
        stdout
    );
}

#[test]
fn worktree_scope_archive_rejects_wrong_scope() {
    let temp_dir = unique_temp_dir("elegy-planning-wt-scope-archive");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c1",
        "--scope",
        "scope-a",
        "scope",
        "create",
        "--scope-key",
        "scope-a",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        "scope-a",
        "worktree",
        "attach",
        "--id",
        "wt-1",
        "--repo-uri",
        "https://example.com/repo.git",
    ]);

    // Try archive in scope-b — expect status "invalid"
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c3",
            "--scope",
            "scope-b",
            "worktree",
            "archive",
            "--id",
            "wt-1",
        ])
        .output()
        .expect("run worktree archive in wrong scope");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(
        result["status"], "invalid",
        "archive in wrong scope should be invalid: {}",
        stdout
    );
}

#[test]
fn worktree_scope_cleanup_intent_rejects_wrong_scope() {
    let temp_dir = unique_temp_dir("elegy-planning-wt-scope-cleanup");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c1",
        "--scope",
        "scope-a",
        "scope",
        "create",
        "--scope-key",
        "scope-a",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        "scope-a",
        "worktree",
        "attach",
        "--id",
        "wt-1",
        "--repo-uri",
        "https://example.com/repo.git",
    ]);

    // Try cleanup-intent in scope-b
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c3",
            "--scope",
            "scope-b",
            "worktree",
            "cleanup-intent",
            "--id",
            "wt-1",
        ])
        .output()
        .expect("run worktree cleanup-intent in wrong scope");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(
        result["status"], "invalid",
        "cleanup-intent in wrong scope should be invalid: {}",
        stdout
    );
}

#[test]
fn worktree_reattach_cross_scope_rejected() {
    let temp_dir = unique_temp_dir("elegy-planning-wt-reattach");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c1",
        "--scope",
        "scope-a",
        "scope",
        "create",
        "--scope-key",
        "scope-a",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        "scope-a",
        "worktree",
        "attach",
        "--id",
        "wt-1",
        "--repo-uri",
        "https://example.com/repo.git",
    ]);

    // Try to attach same ID from scope-b
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c3",
            "--scope",
            "scope-b",
            "worktree",
            "attach",
            "--id",
            "wt-1",
            "--repo-uri",
            "https://example.com/other.git",
        ])
        .output()
        .expect("run worktree attach cross-scope");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(
        result["status"], "invalid",
        "cross-scope reattach should be invalid: {}",
        stdout
    );
    let error = result["error"].as_str().unwrap_or("");
    assert!(
        error.contains("CROSS_SCOPE_MUTATION") || error.contains("scope"),
        "error should mention cross-scope: {}",
        error
    );

    // Verify scope-a worktree still intact
    let show = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        "scope-a",
        "worktree",
        "show",
        "--id",
        "wt-1",
    ]);
    assert_eq!(show["status"], "ok");
    assert_eq!(show["data"]["repoUri"], "https://example.com/repo.git");
}

// ===================================================================
// FIX 4: Project run graph consistency tests
// ===================================================================

#[test]
fn project_run_claim_rejects_wrong_goal_roadmap() {
    let temp_dir = unique_temp_dir("elegy-planning-pr-wrong-goal");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    // Setup: scope, goal-g1, goal-g2, roadmap-r1 (under g1), wp-1 in r1
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c1",
        "--scope",
        "scope-a",
        "scope",
        "create",
        "--scope-key",
        "scope-a",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        "scope-a",
        "goal",
        "create",
        "--id",
        "goal-g1",
        "--title",
        "Goal G1",
        "--description",
        "Test",
        "--acceptance",
        "done",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c3",
        "--scope",
        "scope-a",
        "goal",
        "create",
        "--id",
        "goal-g2",
        "--title",
        "Goal G2",
        "--description",
        "Test",
        "--acceptance",
        "done",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c4",
        "--scope",
        "scope-a",
        "roadmap",
        "create",
        "--id",
        "roadmap-r1",
        "--goal-id",
        "goal-g1",
        "--title",
        "RM R1",
        "--summary",
        "Test",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c5",
        "--scope",
        "scope-a",
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "roadmap-r1",
        "--id",
        "wp-1",
        "--title",
        "WP 1",
        "--summary",
        "Test",
        "--effort-tier",
        "fast",
    ]);

    // Try claim with goal-g2 + roadmap-r1 + wp-1 — should fail (roadmap belongs to goal-g1)
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c6",
            "--scope",
            "scope-a",
            "project-run",
            "claim",
            "--goal-id",
            "goal-g2",
            "--roadmap-id",
            "roadmap-r1",
            "--work-point-id",
            "wp-1",
        ])
        .output()
        .expect("run claim with wrong goal");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(
        result["status"], "invalid",
        "claim with wrong goal should fail: {}",
        stdout
    );
    let error = result["error"].as_str().unwrap_or("");
    assert!(
        error.contains("PROJECT-RUN-GOAL-ROADMAP-MISMATCH") || error.contains("MISMATCH"),
        "error should mention mismatch: {}",
        error
    );
}

#[test]
fn project_run_claim_rejects_wrong_work_point_roadmap() {
    let temp_dir = unique_temp_dir("elegy-planning-pr-wrong-wp");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    // Setup
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c1",
        "--scope",
        "scope-a",
        "scope",
        "create",
        "--scope-key",
        "scope-a",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        "scope-a",
        "goal",
        "create",
        "--id",
        "goal-g1",
        "--title",
        "Goal G1",
        "--description",
        "Test",
        "--acceptance",
        "done",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c3",
        "--scope",
        "scope-a",
        "roadmap",
        "create",
        "--id",
        "roadmap-r1",
        "--goal-id",
        "goal-g1",
        "--title",
        "RM R1",
        "--summary",
        "Test",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c4",
        "--scope",
        "scope-a",
        "roadmap",
        "create",
        "--id",
        "roadmap-r2",
        "--goal-id",
        "goal-g1",
        "--title",
        "RM R2",
        "--summary",
        "Test",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c5",
        "--scope",
        "scope-a",
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "roadmap-r1",
        "--id",
        "wp-1",
        "--title",
        "WP 1",
        "--summary",
        "Test",
        "--effort-tier",
        "fast",
    ]);

    // Try claim with goal-g1 + roadmap-r2 + wp-1 — should fail (wp-1 belongs to roadmap-r1)
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c6",
            "--scope",
            "scope-a",
            "project-run",
            "claim",
            "--goal-id",
            "goal-g1",
            "--roadmap-id",
            "roadmap-r2",
            "--work-point-id",
            "wp-1",
        ])
        .output()
        .expect("run claim with wrong roadmap");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(
        result["status"], "invalid",
        "claim with wrong roadmap should fail: {}",
        stdout
    );
    let error = result["error"].as_str().unwrap_or("");
    assert!(
        error.contains("PROJECT-RUN-WORK-POINT-ROADMAP-MISMATCH") || error.contains("MISMATCH"),
        "error should mention mismatch: {}",
        error
    );
}

// ===================================================================
// Graph CLI machine posture tests
// ===================================================================

#[test]
fn graph_node_create_supports_machine_flags() {
    let temp_dir = unique_temp_dir("elegy-planning-machine-graph-node");
    let db_path = temp_dir.join("planning.db");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-graph-node-1",
            "graph",
            "node",
            "create",
            "--id",
            "gn-machine-1",
            "--kind",
            "work",
            "--title",
            "Machine Test Node",
            "--summary",
            "Testing graph node create via CLI",
            "--status",
            "active",
            "--tag",
            "cli-test",
        ])
        .output()
        .expect("run graph node create");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(result["status"], "ok", "should succeed: {}", stdout);
    assert_eq!(result["correlationId"], "corr-graph-node-1");
    assert_eq!(result["nonInteractive"], true);
    assert!(result["command"]
        .as_array()
        .expect("command is array")
        .iter()
        .any(|c| c == "graph"));
    let record = &result["data"]["record"];
    assert_eq!(record["id"], "gn-machine-1");
    assert_eq!(record["title"], "Machine Test Node");
    assert_eq!(record["kind"], "work");
    assert_eq!(record["status"], "active");
}

#[test]
fn graph_edge_create_supports_machine_flags() {
    let temp_dir = unique_temp_dir("elegy-planning-machine-graph-edge");
    let db_path = temp_dir.join("planning.db");

    // Create two nodes first
    for (id, title) in &[("gn-edge-src", "Source"), ("gn-edge-tgt", "Target")] {
        let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
            .args([
                "--db",
                db_path.to_str().expect("utf-8 db path"),
                "--json",
                "--non-interactive",
                "--correlation-id",
                "corr-graph-edge-setup",
                "graph",
                "node",
                "create",
                "--id",
                id,
                "--kind",
                "work",
                "--title",
                title,
                "--summary",
                "Node for edge test",
                "--status",
                "active",
            ])
            .output()
            .expect("create node for edge test");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let result: Value = serde_json::from_str(&stdout).expect("valid json");
        assert_eq!(result["status"], "ok", "node create failed: {}", stdout);
    }

    // Create edge
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-graph-edge-1",
            "graph",
            "edge",
            "create",
            "--id",
            "ge-machine-1",
            "--kind",
            "depends-on",
            "--source-node-id",
            "gn-edge-src",
            "--target-node-id",
            "gn-edge-tgt",
            "--status",
            "active",
        ])
        .output()
        .expect("run graph edge create");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(result["status"], "ok", "should succeed: {}", stdout);
    assert_eq!(result["correlationId"], "corr-graph-edge-1");
    let record = &result["data"]["record"];
    assert_eq!(record["id"], "ge-machine-1");
    assert_eq!(record["kind"], "depends-on");
    assert_eq!(record["sourceNodeId"], "gn-edge-src");
    assert_eq!(record["targetNodeId"], "gn-edge-tgt");
}

#[test]
fn graph_node_show_returns_correct_record() {
    let temp_dir = unique_temp_dir("elegy-planning-gn-show");
    let db_path = temp_dir.join("planning.db");

    // Create node
    Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "graph",
            "node",
            "create",
            "--id",
            "gn-show-1",
            "--kind",
            "milestone",
            "--title",
            "Show Test",
            "--summary",
            "Node for show test",
            "--status",
            "in-progress",
        ])
        .output()
        .expect("create node");

    // Show node
    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "graph",
        "node",
        "show",
        "--node-id",
        "gn-show-1",
    ]);
    assert_eq!(result["status"], "ok");
    let data = &result["data"];
    assert_eq!(data["node"]["id"], "gn-show-1");
    assert_eq!(data["node"]["kind"], "milestone");
    assert_eq!(data["node"]["title"], "Show Test");
    assert_eq!(data["node"]["status"], "in-progress");
    assert!(data["incomingEdges"].is_array());
    assert!(data["outgoingEdges"].is_array());
    assert!(data["connectedNodes"].is_array());
    assert!(data["tags"].is_array());
    assert!(data["validation"]["status"].is_string());
}

#[test]
fn graph_node_list_filters_by_kind() {
    let temp_dir = unique_temp_dir("elegy-planning-gn-list");
    let db_path = temp_dir.join("planning.db");

    // Create nodes of different kinds
    for (id, kind, title) in &[
        ("gn-list-w1", "work", "Work Node"),
        ("gn-list-w2", "work", "Another Work"),
        ("gn-list-m1", "milestone", "Milestone Node"),
    ] {
        Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
            .args([
                "--db",
                db_path.to_str().expect("utf-8 db path"),
                "--json",
                "--non-interactive",
                "graph",
                "node",
                "create",
                "--id",
                id,
                "--kind",
                kind,
                "--title",
                title,
                "--summary",
                "List test",
                "--status",
                "active",
            ])
            .output()
            .expect("create node");
    }

    // List all
    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "graph",
        "node",
        "list",
    ]);
    assert_eq!(result["status"], "ok");
    assert_eq!(
        result["data"]["nodes"]
            .as_array()
            .expect("nodes is array")
            .len(),
        3
    );

    // List only work nodes
    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "graph",
        "node",
        "list",
        "--kind",
        "work",
    ]);
    assert_eq!(result["status"], "ok");
    let nodes = result["data"]["nodes"].as_array().expect("nodes is array");
    assert_eq!(nodes.len(), 2);
    for node in nodes {
        assert_eq!(node["kind"], "work");
    }

    // List with limit
    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "graph",
        "node",
        "list",
        "--limit",
        "1",
    ]);
    assert_eq!(result["status"], "ok");
    assert_eq!(
        result["data"]["nodes"]
            .as_array()
            .expect("nodes is array")
            .len(),
        1
    );
}

#[test]
fn graph_edge_incoming_and_outgoing() {
    let temp_dir = unique_temp_dir("elegy-planning-ge-dir");
    let db_path = temp_dir.join("planning.db");

    // Create three nodes: A -> B -> C
    for (id, title) in &[
        ("ge-dir-a", "Node A"),
        ("ge-dir-b", "Node B"),
        ("ge-dir-c", "Node C"),
    ] {
        Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
            .args([
                "--db",
                db_path.to_str().expect("utf-8 db path"),
                "--json",
                "--non-interactive",
                "graph",
                "node",
                "create",
                "--id",
                id,
                "--kind",
                "work",
                "--title",
                title,
                "--summary",
                "Direction test",
                "--status",
                "active",
            ])
            .output()
            .expect("create node");
    }

    // A depends-on B
    Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "graph",
            "edge",
            "create",
            "--id",
            "ge-dir-ab",
            "--kind",
            "depends-on",
            "--source-node-id",
            "ge-dir-a",
            "--target-node-id",
            "ge-dir-b",
            "--status",
            "active",
        ])
        .output()
        .expect("create A->B edge");

    // B depends-on C
    Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "graph",
            "edge",
            "create",
            "--id",
            "ge-dir-bc",
            "--kind",
            "depends-on",
            "--source-node-id",
            "ge-dir-b",
            "--target-node-id",
            "ge-dir-c",
            "--status",
            "active",
        ])
        .output()
        .expect("create B->C edge");

    // B should have 1 outgoing (B->C) and 1 incoming (A->B)
    let outgoing = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "graph",
        "edge",
        "outgoing",
        "--node-id",
        "ge-dir-b",
    ]);
    assert_eq!(outgoing["status"], "ok");
    assert_eq!(
        outgoing["data"]["edges"]
            .as_array()
            .expect("edges is array")
            .len(),
        1
    );
    assert_eq!(outgoing["data"]["edges"][0]["targetNodeId"], "ge-dir-c");

    let incoming = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "graph",
        "edge",
        "incoming",
        "--node-id",
        "ge-dir-b",
    ]);
    assert_eq!(incoming["status"], "ok");
    assert_eq!(
        incoming["data"]["edges"]
            .as_array()
            .expect("edges is array")
            .len(),
        1
    );
    assert_eq!(incoming["data"]["edges"][0]["sourceNodeId"], "ge-dir-a");
}

#[test]
fn graph_node_out_of_scope_show_returns_structured_invalid_json() {
    let temp_dir = unique_temp_dir("elegy-planning-gn-scope");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    // Create a scope
    let create_scope = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "scope",
            "create",
            "--scope-key",
            "workspace-a",
            "--scope-type",
            "workspace",
        ])
        .output()
        .expect("create workspace-a scope");
    assert!(create_scope.status.success());

    // Create a graph node in workspace-a
    let create_node = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--scope",
            "workspace-a",
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-gn-scope-1",
            "graph",
            "node",
            "create",
            "--id",
            "gn-scope-a",
            "--kind",
            "work",
            "--title",
            "Scoped Node",
            "--summary",
            "Node in workspace-a",
            "--status",
            "active",
        ])
        .output()
        .expect("create scoped node");
    assert!(create_node.status.success());

    // Try to show the node from scope "default" — should fail
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--scope",
            "default",
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-gn-scope-2",
            "graph",
            "node",
            "show",
            "--node-id",
            "gn-scope-a",
        ])
        .output()
        .expect("run out-of-scope graph node show");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("graph node `gn-scope-a` is in scope `workspace-a`"));
    assert!(stdout.contains("\"correlationId\": \"corr-gn-scope-2\""));
}

#[test]
fn graph_node_create_with_payload_json() {
    let temp_dir = unique_temp_dir("elegy-planning-gn-payload");
    let db_path = temp_dir.join("planning.db");

    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "graph",
        "node",
        "create",
        "--id",
        "gn-payload-1",
        "--kind",
        "work",
        "--title",
        "Payload Test",
        "--summary",
        "Testing payload",
        "--status",
        "active",
        "--payload-json",
        r#"{"key": "value", "num": 42}"#,
    ]);
    assert_eq!(result["status"], "ok");
    assert_eq!(result["data"]["record"]["payload"]["key"], "value");
    assert_eq!(result["data"]["record"]["payload"]["num"], 42);
}

#[test]
fn graph_node_create_with_payload_file() {
    let temp_dir = unique_temp_dir("elegy-planning-gn-payload-file");
    let db_path = temp_dir.join("planning.db");
    let payload_path = temp_dir.join("payload.json");
    std::fs::write(&payload_path, r#"{"file_key": "file_value"}"#).expect("write payload file");

    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "graph",
        "node",
        "create",
        "--id",
        "gn-payload-file-1",
        "--kind",
        "work",
        "--title",
        "Payload File Test",
        "--summary",
        "Testing payload file",
        "--status",
        "active",
        "--payload-file",
        payload_path.to_str().expect("utf-8 path"),
    ]);
    assert_eq!(result["status"], "ok");
    assert_eq!(
        result["data"]["record"]["payload"]["file_key"],
        "file_value"
    );
}

#[test]
fn graph_node_show_returns_view_shaped_json() {
    let temp_dir = unique_temp_dir("elegy-planning-gn-view-show");
    let db_path = temp_dir.join("planning.db");

    // Create node
    Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "graph",
            "node",
            "create",
            "--id",
            "gn-view-show",
            "--kind",
            "work",
            "--title",
            "View Show Node",
            "--summary",
            "Testing view output",
            "--status",
            "active",
        ])
        .output()
        .expect("create node");

    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "graph",
        "node",
        "show",
        "--node-id",
        "gn-view-show",
    ]);
    assert_eq!(result["status"], "ok");
    // Should have view-shaped response (node, incomingEdges, outgoingEdges, connectedNodes, tags, validation)
    let data = &result["data"];
    assert!(data["node"].is_object(), "view should have node field");
    assert_eq!(data["node"]["id"], "gn-view-show");
    assert!(
        data["incomingEdges"].is_array(),
        "view should have incomingEdges array"
    );
    assert!(
        data["outgoingEdges"].is_array(),
        "view should have outgoingEdges array"
    );
    assert!(
        data["connectedNodes"].is_array(),
        "view should have connectedNodes array"
    );
    assert!(data["tags"].is_array(), "view should have tags array");
    assert!(
        data["validation"].is_object(),
        "view should have validation object"
    );
    assert_eq!(data["validation"]["status"], "valid");
}

#[test]
fn graph_edge_show_returns_view_shaped_json() {
    let temp_dir = unique_temp_dir("elegy-planning-ge-view-show");
    let db_path = temp_dir.join("planning.db");

    // Create 2 nodes + edge
    for (id, title) in &[("ge-view-src", "VSource"), ("ge-view-tgt", "VTarget")] {
        Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
            .args([
                "--db",
                db_path.to_str().expect("utf-8 db path"),
                "--json",
                "--non-interactive",
                "graph",
                "node",
                "create",
                "--id",
                id,
                "--kind",
                "work",
                "--title",
                title,
                "--summary",
                "test",
                "--status",
                "active",
            ])
            .output()
            .expect("create node");
    }
    Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "graph",
            "edge",
            "create",
            "--id",
            "ge-view-show",
            "--kind",
            "depends-on",
            "--source-node-id",
            "ge-view-src",
            "--target-node-id",
            "ge-view-tgt",
            "--status",
            "active",
        ])
        .output()
        .expect("create edge");

    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "graph",
        "edge",
        "show",
        "--edge-id",
        "ge-view-show",
    ]);
    assert_eq!(result["status"], "ok");
    let data = &result["data"];
    assert!(data["edge"].is_object(), "view should have edge field");
    assert_eq!(data["edge"]["id"], "ge-view-show");
    assert!(
        data["sourceNode"].is_object(),
        "view should have sourceNode field"
    );
    assert!(
        data["targetNode"].is_object(),
        "view should have targetNode field"
    );
    assert!(
        data["validation"].is_object(),
        "view should have validation object"
    );
}

#[test]
fn graph_node_status_appends_event() {
    let temp_dir = unique_temp_dir("elegy-planning-gn-status");
    let db_path = temp_dir.join("planning.db");

    // Create node
    Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-status-node",
            "graph",
            "node",
            "create",
            "--id",
            "gn-status-cli",
            "--kind",
            "work",
            "--title",
            "Status CLI Node",
            "--summary",
            "Testing status CLI",
            "--status",
            "active",
        ])
        .output()
        .expect("create node");

    // Change status
    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-status-change",
        "graph",
        "node",
        "status",
        "--node-id",
        "gn-status-cli",
        "--status",
        "completed",
    ]);
    assert_eq!(result["status"], "ok");
    assert_eq!(result["data"]["record"]["status"], "completed");
    assert_eq!(result["data"]["record"]["revision"], 2);
    assert!(
        result["data"]["validation"]["status"] == "valid"
            || result["data"]["validation"]["status"] == "warning"
    );
}

#[test]
fn graph_edge_status_requires_scope_in_machine_mode() {
    let temp_dir = unique_temp_dir("elegy-planning-ge-status-scope");
    let db_path = temp_dir.join("planning.db");

    // Create scope + node
    let db_arg = db_path.to_str().expect("utf-8 db path");
    Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "scope",
            "create",
            "--scope-key",
            "workspace-b",
            "--scope-type",
            "workspace",
        ])
        .output()
        .expect("create scope");

    // Create node in workspace-b
    Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--scope",
            "workspace-b",
            "--json",
            "--non-interactive",
            "graph",
            "node",
            "create",
            "--id",
            "gn-scope-b",
            "--kind",
            "work",
            "--title",
            "Scoped B",
            "--summary",
            "In workspace-b",
            "--status",
            "active",
        ])
        .output()
        .expect("create scoped node");

    // Try to change status from default scope — should fail
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--scope",
            "default",
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-status-out",
            "graph",
            "node",
            "status",
            "--node-id",
            "gn-scope-b",
            "--status",
            "completed",
        ])
        .output()
        .expect("run out-of-scope status");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(result["status"], "invalid", "should be invalid: {}", stdout);
}

#[test]
fn graph_node_revise_requires_scope_in_machine_mode() {
    let temp_dir = unique_temp_dir("elegy-planning-gn-revise-scope");
    let db_path = temp_dir.join("planning.db");

    let db_arg = db_path.to_str().expect("utf-8 db path");
    Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "scope",
            "create",
            "--scope-key",
            "workspace-c",
            "--scope-type",
            "workspace",
        ])
        .output()
        .expect("create scope");

    Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--scope",
            "workspace-c",
            "--json",
            "--non-interactive",
            "graph",
            "node",
            "create",
            "--id",
            "gn-revise-scope",
            "--kind",
            "work",
            "--title",
            "Revise Scope",
            "--summary",
            "Test revise scope",
            "--status",
            "active",
        ])
        .output()
        .expect("create scoped node");

    // Try to revise from default scope — should fail
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--scope",
            "default",
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-revise-out",
            "graph",
            "node",
            "revise",
            "--node-id",
            "gn-revise-scope",
            "--title",
            "Out of Scope",
        ])
        .output()
        .expect("run out-of-scope revise");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(result["status"], "invalid", "should be invalid: {}", stdout);
}

#[test]
fn graph_edge_revise_rejects_out_of_scope() {
    let temp_dir = unique_temp_dir("elegy-planning-ge-revise-scope");
    let db_path = temp_dir.join("planning.db");

    let db_arg = db_path.to_str().expect("utf-8 db path");
    Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "scope",
            "create",
            "--scope-key",
            "workspace-d",
            "--scope-type",
            "workspace",
        ])
        .output()
        .expect("create scope");

    // Create nodes + edge in workspace-d
    for id in &["ge-rv-src", "ge-rv-tgt"] {
        Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
            .args([
                "--db",
                db_arg,
                "--scope",
                "workspace-d",
                "--json",
                "--non-interactive",
                "graph",
                "node",
                "create",
                "--id",
                id,
                "--kind",
                "work",
                "--title",
                &format!("Rev {id}"),
                "--summary",
                "test",
                "--status",
                "active",
            ])
            .output()
            .expect("create scoped node");
    }
    Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--scope",
            "workspace-d",
            "--json",
            "--non-interactive",
            "graph",
            "edge",
            "create",
            "--id",
            "ge-rv-scope",
            "--kind",
            "depends-on",
            "--source-node-id",
            "ge-rv-src",
            "--target-node-id",
            "ge-rv-tgt",
            "--status",
            "active",
        ])
        .output()
        .expect("create scoped edge");

    // Try to revise from default scope
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--scope",
            "default",
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-edge-rev-out",
            "graph",
            "edge",
            "revise",
            "--edge-id",
            "ge-rv-scope",
            "--status",
            "completed",
        ])
        .output()
        .expect("run out-of-scope edge revise");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(result["status"], "invalid", "should be invalid: {}", stdout);
}

#[test]
fn graph_node_status_preserves_correlation_id_in_events() {
    let temp_dir = unique_temp_dir("elegy-planning-gn-corr");
    let db_path = temp_dir.join("planning.db");

    // Create node
    Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-global-1",
            "graph",
            "node",
            "create",
            "--id",
            "gn-corr-cli",
            "--kind",
            "work",
            "--title",
            "Corr CLI",
            "--summary",
            "Testing correlation",
            "--status",
            "active",
        ])
        .output()
        .expect("create node");

    // Change status with explicit correlation-id
    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-global-2",
        "graph",
        "node",
        "status",
        "--node-id",
        "gn-corr-cli",
        "--status",
        "completed",
    ]);
    assert_eq!(result["status"], "ok");
    assert_eq!(result["correlationId"], "corr-global-2");
}

// ===================================================================
// Phase 6: Acceptance graph machine posture tests
// ===================================================================

#[test]
fn graph_acceptance_create_and_show_json_envelope() {
    let temp_dir = unique_temp_dir("elegy-planning-acc-create-show");
    let db_path = temp_dir.join("planning.db");

    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-acc-create",
        "graph",
        "acceptance",
        "create",
        "--id",
        "acc-show-test",
        "--acceptance-kind",
        "abstract",
        "--title",
        "System must be reliable",
        "--summary",
        "Abstract acceptance for reliability",
        "--status",
        "active",
        "--description",
        "The system must maintain 99.9% uptime",
        "--verification-policy",
        "automated",
    ]);
    assert_eq!(result["status"], "ok");
    assert_eq!(result["correlationId"], "corr-acc-create");
    let record = &result["data"]["record"];
    assert_eq!(record["id"], "acc-show-test");
    assert_eq!(record["kind"], "acceptance");

    // Show the acceptance
    let show = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "graph",
        "acceptance",
        "show",
        "--node-id",
        "acc-show-test",
    ]);
    assert_eq!(show["status"], "ok");
    let node = &show["data"]["node"];
    assert_eq!(node["id"], "acc-show-test");
    assert_eq!(node["kind"], "acceptance");
    assert!(show["data"]["requiredBy"].is_array());
    assert!(show["data"]["satisfiedAbstracts"].is_array());
    assert!(show["data"]["satisfyingConcretes"].is_array());
    assert!(show["data"]["attachedEvidence"].is_array());
}

#[test]
fn graph_acceptance_satisfy_preserves_correlation_id() {
    let temp_dir = unique_temp_dir("elegy-planning-acc-satisfy");
    let db_path = temp_dir.join("planning.db");

    // Create abstract acceptance
    command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-sat-1",
        "graph",
        "acceptance",
        "create",
        "--id",
        "abs-sat",
        "--acceptance-kind",
        "abstract",
        "--title",
        "Abstract requirement",
        "--summary",
        "Abstract acceptance for satisfy test",
        "--description",
        "Must be satisfiable",
    ]);

    // Create concrete acceptance
    command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-sat-2",
        "graph",
        "acceptance",
        "create",
        "--id",
        "conc-sat",
        "--acceptance-kind",
        "concrete",
        "--title",
        "Concrete check",
        "--summary",
        "Concrete acceptance for satisfy test",
        "--description",
        "Verifies the abstract",
    ]);

    // Satisfy the abstract
    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-sat-3",
        "graph",
        "acceptance",
        "satisfy",
        "--id",
        "sat-edge-1",
        "--concrete-id",
        "conc-sat",
        "--abstract-id",
        "abs-sat",
        "--rationale",
        "This concrete check verifies the abstract requirement",
    ]);
    assert_eq!(result["status"], "ok");
    assert_eq!(result["correlationId"], "corr-sat-3");
    let edge_record = &result["data"]["record"];
    assert_eq!(edge_record["kind"], "satisfies");
    assert_eq!(edge_record["sourceNodeId"], "conc-sat");
    assert_eq!(edge_record["targetNodeId"], "abs-sat");
}

#[test]
fn graph_evidence_create_and_show_json_envelope() {
    let temp_dir = unique_temp_dir("elegy-planning-ev-create-show");
    let db_path = temp_dir.join("planning.db");

    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-ev-create",
        "graph",
        "evidence",
        "create",
        "--id",
        "ev-show-test",
        "--evidence-kind",
        "test-result",
        "--title",
        "Login test suite results",
        "--summary",
        "All login tests passed",
        "--status",
        "active",
        "--reference",
        "ci/build-42",
        "--content",
        "42 passed, 0 failed",
        "--captured-at",
        "2026-06-01T12:00:00Z",
    ]);
    assert_eq!(result["status"], "ok");
    assert_eq!(result["correlationId"], "corr-ev-create");
    let record = &result["data"]["record"];
    assert_eq!(record["id"], "ev-show-test");
    assert_eq!(record["kind"], "evidence");

    // Show the evidence
    let show = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "graph",
        "evidence",
        "show",
        "--node-id",
        "ev-show-test",
    ]);
    assert_eq!(show["status"], "ok");
    let node = &show["data"]["node"];
    assert_eq!(node["id"], "ev-show-test");
    assert_eq!(node["kind"], "evidence");
    assert!(show["data"]["attachedTo"].is_array());
}

#[test]
fn graph_evidence_attach_to_acceptance() {
    let temp_dir = unique_temp_dir("elegy-planning-ev-attach");
    let db_path = temp_dir.join("planning.db");

    // Create concrete acceptance
    command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-ea-1",
        "graph",
        "acceptance",
        "create",
        "--id",
        "acc-ea-target",
        "--acceptance-kind",
        "concrete",
        "--title",
        "Target acceptance for evidence",
        "--summary",
        "Concrete acceptance for evidence attach test",
        "--description",
        "Must be verified by evidence",
    ]);

    // Create evidence
    command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-ea-2",
        "graph",
        "evidence",
        "create",
        "--id",
        "ev-ea-source",
        "--evidence-kind",
        "review",
        "--title",
        "Peer review result",
        "--summary",
        "Code reviewed and approved",
        "--reference",
        "",
        "--content",
        "",
        "--captured-at",
        "",
    ]);

    // Attach evidence to acceptance
    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-ea-3",
        "graph",
        "evidence",
        "attach",
        "--id",
        "ev-attach-edge",
        "--evidence-id",
        "ev-ea-source",
        "--target-id",
        "acc-ea-target",
        "--rationale",
        "Code review confirms acceptance criteria met",
    ]);
    assert_eq!(result["status"], "ok");
    let edge_record = &result["data"]["record"];
    assert_eq!(edge_record["kind"], "evidenced-by");
    assert_eq!(edge_record["sourceNodeId"], "acc-ea-target");
    assert_eq!(edge_record["targetNodeId"], "ev-ea-source");

    // Verify evidence appears in acceptance view
    let view = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "graph",
        "acceptance",
        "show",
        "--node-id",
        "acc-ea-target",
    ]);
    assert_eq!(view["status"], "ok");
    let attached = &view["data"]["attachedEvidence"];
    assert_eq!(
        attached
            .as_array()
            .expect("attachedEvidence is array")
            .len(),
        1
    );
    assert_eq!(attached[0]["id"], "ev-ea-source");
}

#[test]
fn graph_acceptance_create_rejects_invalid_status() {
    let temp_dir = unique_temp_dir("elegy-planning-acc-bad-status");
    let db_path = temp_dir.join("planning.db");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-bad-status",
            "graph",
            "acceptance",
            "create",
            "--id",
            "acc-bad-status",
            "--acceptance-kind",
            "abstract",
            "--title",
            "Invalid status test",
            "--summary",
            "Test invalid status rejection",
            "--description",
            "This should fail with a non-kebab status",
            "--status",
            "InvalidStatus",
        ])
        .output()
        .expect("run acceptance create with invalid status");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let result: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(result["status"], "invalid");
    assert!(
        result["error"].as_str().unwrap_or("").contains("kebab"),
        "should mention kebab: {}",
        result
    );
}

#[test]
fn graph_acceptance_out_of_scope_show_rejected() {
    let temp_dir = unique_temp_dir("elegy-planning-acc-oos");
    let db_path = temp_dir.join("planning.db");

    // Create a separate scope
    Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-oos",
            "scope",
            "create",
            "--scope-key",
            "other-workspace",
            "--scope-type",
            "workspace",
        ])
        .output()
        .expect("create scope");

    // Create acceptance in other scope
    Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-oos",
            "--scope",
            "other-workspace",
            "graph",
            "acceptance",
            "create",
            "--id",
            "acc-oos",
            "--acceptance-kind",
            "abstract",
            "--title",
            "Out of scope acceptance",
            "--summary",
            "Acceptance in other workspace",
            "--description",
            "Should not be visible from default scope",
        ])
        .output()
        .expect("create acceptance in other scope");

    // Try to show from default scope (no --scope flag)
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "graph",
            "acceptance",
            "show",
            "--node-id",
            "acc-oos",
        ])
        .output()
        .expect("run out-of-scope acceptance show");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let result: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(result["status"], "invalid");
    assert!(
        result["error"].as_str().unwrap_or("").contains("not"),
        "should mention scope mismatch: {}",
        result
    );
}

#[test]
fn graph_node_finalize_success_json() {
    let temp_dir = unique_temp_dir("elegy-planning-gn-finalize");
    let db_path = temp_dir.join("planning.db");

    // Create a work node
    command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-cli-fin",
        "graph",
        "node",
        "create",
        "--id",
        "gn-fin",
        "--kind",
        "work",
        "--title",
        "Finalize test node",
        "--summary",
        "Testing finalize",
        "--status",
        "active",
    ]);

    // Finalize it
    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-cli-fin-2",
        "graph",
        "node",
        "finalize",
        "--node-id",
        "gn-fin",
        "--status",
        "completed",
    ]);
    assert_eq!(result["status"], "ok");
    assert_eq!(result["correlationId"], "corr-cli-fin-2");
    let record = &result["data"]["record"];
    assert_eq!(record["status"], "completed");
}

#[test]
fn graph_node_finalize_rejection_structured_json() {
    let temp_dir = unique_temp_dir("elegy-planning-gn-fin-rej");
    let db_path = temp_dir.join("planning.db");

    // Create abstract acceptance without coverage
    command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-fin-rej",
        "graph",
        "acceptance",
        "create",
        "--id",
        "abs-fin-rej",
        "--acceptance-kind",
        "abstract",
        "--title",
        "Uncovered abstract for finalize rejection",
        "--summary",
        "No coverage",
        "--description",
        "",
    ]);

    // Try to finalize — should fail with invalid status (InvalidInput -> status "invalid", exit code 1)
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--json",
            "--non-interactive",
            "--correlation-id",
            "corr-fin-rej-2",
            "graph",
            "node",
            "finalize",
            "--node-id",
            "abs-fin-rej",
            "--status",
            "validated",
        ])
        .output()
        .expect("run finalize command");
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(result["status"], "invalid");
    assert!(
        result["error"]
            .as_str()
            .unwrap_or("")
            .contains("ACCEPTANCE-COVERAGE-MISSING"),
        "should mention coverage: {result}"
    );
}

#[test]
fn graph_node_finalize_accepted_risk_in_event() {
    let temp_dir = unique_temp_dir("elegy-planning-gn-fin-risk");
    let db_path = temp_dir.join("planning.db");

    // Create abstract acceptance without coverage
    command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-risk-cli",
        "graph",
        "acceptance",
        "create",
        "--id",
        "abs-risk-cli",
        "--acceptance-kind",
        "abstract",
        "--title",
        "Risk-based finalization",
        "--summary",
        "Accepted risk",
        "--description",
        "",
    ]);

    // Finalize with accepted-risk
    let result = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "--correlation-id",
        "corr-risk-cli-2",
        "graph",
        "node",
        "finalize",
        "--node-id",
        "abs-risk-cli",
        "--status",
        "validated",
        "--accepted-risk",
        "Deferred to Q3 per team decision",
    ]);
    assert_eq!(result["status"], "ok");
    assert_eq!(result["correlationId"], "corr-risk-cli-2");

    // Verify event contains accepted risk
    let events = command_json(&[
        "--db",
        db_path.to_str().expect("utf-8 db path"),
        "--json",
        "--non-interactive",
        "events",
    ]);
    let events_arr = events["data"]["events"].as_array().expect("events array");
    let finalize_event = events_arr
        .iter()
        .find(|e| {
            e["eventType"].as_str().unwrap_or("") == "graph-node.finalized-with-accepted-risk"
        })
        .expect("finalize event should exist");
    assert!(finalize_event["payload"]["acceptedRisk"]
        .as_str()
        .unwrap_or("")
        .contains("Deferred to Q3"));
}

#[test]
fn graph_acceptance_evidence_finalize_help() {
    let bin = env!("CARGO_BIN_EXE_elegy-planning");

    // graph acceptance --help
    let output = std::process::Command::new(bin)
        .args(["graph", "acceptance", "--help"])
        .output()
        .expect("run graph acceptance --help");
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("stdout utf-8");
    assert!(
        help.contains("create"),
        "acceptance help should include create"
    );
    assert!(
        help.contains("satisfy"),
        "acceptance help should include satisfy"
    );
    assert!(help.contains("show"), "acceptance help should include show");
    assert!(help.contains("list"), "acceptance help should include list");

    // graph evidence --help
    let output = std::process::Command::new(bin)
        .args(["graph", "evidence", "--help"])
        .output()
        .expect("run graph evidence --help");
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("stdout utf-8");
    assert!(
        help.contains("create"),
        "evidence help should include create"
    );
    assert!(
        help.contains("attach"),
        "evidence help should include attach"
    );

    // graph node --help (should include finalize)
    let output = std::process::Command::new(bin)
        .args(["graph", "node", "--help"])
        .output()
        .expect("run graph node --help");
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("stdout utf-8");
    assert!(
        help.contains("finalize"),
        "graph node help should include finalize"
    );
}
