use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}-{unique}"));
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
