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
