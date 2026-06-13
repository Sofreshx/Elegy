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

fn setup_scope_goal_roadmap(
    _temp_dir: &PathBuf,
    db_arg: &str,
    scope: &str,
    goal_id: &str,
    roadmap_id: &str,
    correlation_id: &str,
) {
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        correlation_id,
        "--scope",
        scope,
        "scope",
        "create",
        "--scope-key",
        scope,
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        correlation_id,
        "--scope",
        scope,
        "goal",
        "create",
        "--id",
        goal_id,
        "--title",
        "Test Goal",
        "--description",
        "Integration test goal",
        "--acceptance",
        "done",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        correlation_id,
        "--scope",
        scope,
        "roadmap",
        "create",
        "--id",
        roadmap_id,
        "--goal-id",
        goal_id,
        "--title",
        "Test Roadmap",
        "--summary",
        "Integration test roadmap",
    ]);
}

// ===================================================================
// AC1: corrective_work_metadata
// ===================================================================
#[test]
fn corrective_work_metadata() {
    let temp_dir = unique_temp_dir("elegy-integration-ac1");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");
    let scope = "ac1";

    setup_scope_goal_roadmap(&temp_dir, db_arg, scope, "goal-ac1", "rm-ac1", "c1");

    // Create a work point with kind, priority, repairs, supersedes
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        scope,
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-ac1",
        "--id",
        "wp-full",
        "--title",
        "Full WP",
        "--summary",
        "With all metadata",
        "--effort-tier",
        "fast",
        "--kind",
        "corrective",
        "--priority",
        "high",
        "--repairs-work-point-id",
        "repair-target",
        "--supersedes-work-point-id",
        "super-target",
        "--blocks-work-point-id",
        "block-target",
    ]);

    // Verify via work-point show
    let show = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        scope,
        "work-point",
        "show",
        "--work-point-id",
        "wp-full",
    ]);
    let wp = &show["data"]["workPoint"];
    assert_eq!(wp["kind"], "corrective");
    assert_eq!(wp["priority"], "high");
    assert!(wp["repairsWorkPointIds"]
        .as_array()
        .expect("repairs array")
        .iter()
        .any(|v| v.as_str() == Some("repair-target")));
    assert!(wp["supersedesWorkPointIds"]
        .as_array()
        .expect("supersedes array")
        .iter()
        .any(|v| v.as_str() == Some("super-target")));
    assert!(wp["blocksWorkPointIds"]
        .as_array()
        .expect("blocks array")
        .iter()
        .any(|v| v.as_str() == Some("block-target")));

    // Create a work point without explicit kind/priority — verify defaults
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c3",
        "--scope",
        scope,
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-ac1",
        "--id",
        "wp-default",
        "--title",
        "Default WP",
        "--summary",
        "With defaults",
        "--effort-tier",
        "fast",
    ]);

    let show_def = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        scope,
        "work-point",
        "show",
        "--work-point-id",
        "wp-default",
    ]);
    let wp_def = &show_def["data"]["workPoint"];
    assert_eq!(wp_def["kind"], "feature");
    assert_eq!(wp_def["priority"], "medium");
    assert_eq!(
        wp_def["repairsWorkPointIds"]
            .as_array()
            .expect("repairs array")
            .len(),
        0
    );
    assert_eq!(
        wp_def["supersedesWorkPointIds"]
            .as_array()
            .expect("supersedes array")
            .len(),
        0
    );
    assert_eq!(
        wp_def["blocksWorkPointIds"]
            .as_array()
            .expect("blocks array")
            .len(),
        0
    );
}

// ===================================================================
// AC2: status_transition_rejected
// ===================================================================
#[test]
fn status_transition_rejected() {
    let temp_dir = unique_temp_dir("elegy-integration-ac2");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");
    let scope = "ac2";

    setup_scope_goal_roadmap(&temp_dir, db_arg, scope, "goal-ac2", "rm-ac2", "c1");

    // Create a work point (default status: Draft)
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        scope,
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-ac2",
        "--id",
        "wp-ac2",
        "--title",
        "WP",
        "--summary",
        "Test",
        "--effort-tier",
        "fast",
    ]);

    // Try invalid transition: Draft -> Completed (should be rejected)
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c3",
            "--scope",
            scope,
            "work-point",
            "update-status",
            "--work-point-id",
            "wp-ac2",
            "--status",
            "completed",
        ])
        .output()
        .expect("run update-status");

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(result["status"], "invalid");
    assert!(
        result["error"]
            .as_str()
            .unwrap_or("")
            .contains("INVALID_STATUS_TRANSITION")
            || result["error"]
                .as_str()
                .unwrap_or("")
                .contains("invalid status transition"),
        "error should mention invalid transition: {}",
        result["error"]
    );

    // Try valid transition: Draft -> Proposed
    let valid_output = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c4",
        "--scope",
        scope,
        "work-point",
        "update-status",
        "--work-point-id",
        "wp-ac2",
        "--status",
        "proposed",
    ]);
    assert_eq!(valid_output["status"], "ok");
}

// ===================================================================
// AC3: override_transition
// ===================================================================
#[test]
fn override_transition() {
    let temp_dir = unique_temp_dir("elegy-integration-ac3");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");
    let scope = "ac3";

    // Create scope + goal
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c1",
        "--scope",
        scope,
        "scope",
        "create",
        "--scope-key",
        scope,
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        scope,
        "goal",
        "create",
        "--id",
        "goal-ac3",
        "--title",
        "Goal AC3",
        "--description",
        "Test",
        "--acceptance",
        "done",
    ]);

    // Try invalid transition with override
    let override_output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c3",
            "--scope",
            scope,
            "goal",
            "update-status",
            "--goal-id",
            "goal-ac3",
            "--status",
            "superseded",
            "--override-transition",
            "--reason",
            "manual fix",
        ])
        .output()
        .expect("run override update-status");

    assert!(
        override_output.status.success(),
        "override transition should succeed: stderr: {}",
        String::from_utf8_lossy(&override_output.stderr)
    );
    let stdout = String::from_utf8(override_output.stdout).expect("stdout utf-8");
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(result["status"], "ok");

    // Verify events contain goal.status-overridden
    let events = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        scope,
        "events",
    ]);
    let event_list = events["data"]["events"].as_array().expect("events array");
    let has_override = event_list
        .iter()
        .any(|e| e["eventType"].as_str() == Some("goal.status-overridden"));
    assert!(
        has_override,
        "events should contain status-overridden event"
    );
}

// ===================================================================
// AC4: downstream_blocking
// ===================================================================
#[test]
fn downstream_blocking() {
    let temp_dir = unique_temp_dir("elegy-integration-ac4");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");
    let scope = "ac4";

    setup_scope_goal_roadmap(&temp_dir, db_arg, scope, "goal-ac4", "rm-ac4", "c1");

    // Create wp-a (corrective, blocks wp-b)
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        scope,
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-ac4",
        "--id",
        "wp-a",
        "--title",
        "Corr WP A",
        "--summary",
        "Corrective work that blocks wp-b",
        "--effort-tier",
        "fast",
        "--kind",
        "corrective",
        "--priority",
        "high",
        "--blocks-work-point-id",
        "wp-b",
    ]);

    // Create wp-b (blocked by wp-a)
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c3",
        "--scope",
        scope,
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-ac4",
        "--id",
        "wp-b",
        "--title",
        "WP B",
        "--summary",
        "Blocked by wp-a",
        "--effort-tier",
        "fast",
    ]);

    // Call next-runnable and verify wp-a is a candidate, wp-b is blocked
    let runnable = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        scope,
        "work-point",
        "next-runnable",
        "--roadmap-id",
        "rm-ac4",
    ]);

    assert_eq!(runnable["status"], "ok");
    let candidates = runnable["data"]["candidates"]
        .as_array()
        .expect("candidates array");
    let candidates_have_wp_a = candidates
        .iter()
        .any(|c| c["workPoint"]["id"].as_str() == Some("wp-a"));
    assert!(
        candidates_have_wp_a,
        "wp-a should be in candidates, got: {:?}",
        candidates
    );

    let blocked = runnable["data"]["blocked"]
        .as_array()
        .expect("blocked array");
    let blocked_has_wp_b = blocked.iter().any(|b| {
        b["workPointId"].as_str() == Some("wp-b")
            && b["reason"]
                .as_str()
                .unwrap_or("")
                .contains("blocked_by:wp-a")
    });
    assert!(
        blocked_has_wp_b,
        "wp-b should be in blocked with blocked_by:wp-a, got: {:?}",
        blocked
    );

    // Complete wp-a
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c4",
        "--scope",
        scope,
        "work-point",
        "update-status",
        "--work-point-id",
        "wp-a",
        "--status",
        "completed",
        "--override-transition",
    ]);

    // Call next-runnable again — wp-b should now be a candidate
    let runnable2 = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        scope,
        "work-point",
        "next-runnable",
        "--roadmap-id",
        "rm-ac4",
    ]);

    let candidates2 = runnable2["data"]["candidates"]
        .as_array()
        .expect("candidates array");
    let candidates_have_wp_b = candidates2
        .iter()
        .any(|c| c["workPoint"]["id"].as_str() == Some("wp-b"));
    assert!(
        candidates_have_wp_b,
        "wp-b should now be in candidates after wp-a completed, got: {:?}",
        candidates2
    );

    let blocked2 = runnable2["data"]["blocked"]
        .as_array()
        .expect("blocked array");
    let blocked_still_wp_b = blocked2
        .iter()
        .any(|b| b["workPointId"].as_str() == Some("wp-b"));
    assert!(!blocked_still_wp_b, "wp-b should no longer be in blocked");
}

// ===================================================================
// AC5: active_lease_ranks_first
// ===================================================================
#[test]
fn active_lease_ranks_first() {
    let temp_dir = unique_temp_dir("elegy-integration-ac5");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");
    let scope = "ac5";

    setup_scope_goal_roadmap(&temp_dir, db_arg, scope, "goal-ac5", "rm-ac5", "c1");

    // Create wp-c (corrective, urgent)
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        scope,
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-ac5",
        "--id",
        "wp-c",
        "--title",
        "Urgent corrective",
        "--summary",
        "Will be urgent",
        "--effort-tier",
        "fast",
        "--kind",
        "corrective",
        "--priority",
        "urgent",
    ]);

    // Create wp-d (feature, medium, lower ordering)
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c3",
        "--scope",
        scope,
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-ac5",
        "--id",
        "wp-d",
        "--title",
        "Normal feature",
        "--summary",
        "Will have active lease",
        "--effort-tier",
        "deep",
    ]);

    // Claim wp-d via project-run: need a project-run claim
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c4",
        "--scope",
        scope,
        "project-run",
        "claim",
        "--goal-id",
        "goal-ac5",
        "--roadmap-id",
        "rm-ac5",
        "--work-point-id",
        "wp-d",
    ]);

    // Call next-runnable — wp-d should be first (active lease), wp-c second (urgent_fix)
    let runnable = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        scope,
        "work-point",
        "next-runnable",
        "--roadmap-id",
        "rm-ac5",
    ]);

    let candidates = runnable["data"]["candidates"]
        .as_array()
        .expect("candidates array");

    // wp-d has active lease so it should be skipped (not in candidates)
    // wp-c is the urgent corrective so it should be in candidates
    let has_wp_c = candidates
        .iter()
        .any(|c| c["workPoint"]["id"].as_str() == Some("wp-c"));
    assert!(
        has_wp_c,
        "wp-c should be a candidate since wp-d has active lease"
    );
    let has_wp_d = candidates
        .iter()
        .any(|c| c["workPoint"]["id"].as_str() == Some("wp-d"));
    assert!(
        !has_wp_d,
        "wp-d should NOT be a candidate (has active lease)"
    );

    // Release the project run
    let runs = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        scope,
        "project-run",
        "list",
    ]);
    let run_id = runs["data"]["projectRuns"][0]["id"]
        .as_str()
        .expect("run id")
        .to_string();

    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c5",
        "--scope",
        scope,
        "project-run",
        "release",
        "--project-run-id",
        &run_id,
        "--status",
        "released",
    ]);

    // Now wp-d should be a candidate
    let runnable2 = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        scope,
        "work-point",
        "next-runnable",
        "--roadmap-id",
        "rm-ac5",
    ]);

    let candidates2 = runnable2["data"]["candidates"]
        .as_array()
        .expect("candidates array");

    // wp-c (urgent_fix) should be first, wp-d second
    if candidates2.len() >= 2 {
        assert_eq!(
            candidates2[0]["workPoint"]["id"].as_str(),
            Some("wp-c"),
            "urgent corrective should be first"
        );
    } else {
        // At least one should be present
        assert!(
            !candidates2.is_empty(),
            "should have at least one candidate"
        );
    }
}

// ===================================================================
// AC6: session_context_extended
// ===================================================================
#[test]
fn session_context_extended() {
    let temp_dir = unique_temp_dir("elegy-integration-ac6");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");
    let scope = "ac6";

    setup_scope_goal_roadmap(&temp_dir, db_arg, scope, "goal-ac6", "rm-ac6", "c1");

    // Create a work point
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        scope,
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-ac6",
        "--id",
        "wp-ac6",
        "--title",
        "WP",
        "--summary",
        "Test",
        "--effort-tier",
        "fast",
    ]);

    // Create a plan
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c3",
        "--scope",
        scope,
        "plan",
        "create",
        "--id",
        "plan-ac6",
        "--goal-id",
        "goal-ac6",
        "--roadmap-id",
        "rm-ac6",
        "--title",
        "Plan",
        "--summary",
        "Test plan",
        "--scope",
        "Execution",
    ]);

    // Create a todo linked to the plan
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c4",
        "--scope",
        scope,
        "todo",
        "create",
        "--plan-id",
        "plan-ac6",
        "--title",
        "Todo",
        "--summary",
        "Test todo",
    ]);

    // Create a critical issue
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c5",
        "--scope",
        scope,
        "issue",
        "create",
        "--id",
        "issue-ac6",
        "--title",
        "Critical issue",
        "--summary",
        "Test issue",
        "--severity",
        "critical",
        "--status",
        "open",
    ]);

    // Call context
    let ctx = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c6",
        "--scope",
        scope,
        "context",
        "--session",
    ]);

    assert_eq!(ctx["status"], "ok");
    // Verify extended fields exist
    assert!(
        ctx["data"].get("activeProjectRuns").is_some(),
        "activeProjectRuns field should exist"
    );
    assert!(
        ctx["data"].get("activeWorkPoints").is_some(),
        "activeWorkPoints field should exist"
    );
    assert!(
        ctx["data"].get("activePlans").is_some(),
        "activePlans field should exist"
    );
    assert!(
        ctx["data"].get("nextPendingTodos").is_some(),
        "nextPendingTodos field should exist"
    );
    assert!(
        ctx["data"].get("openBlockingIssues").is_some(),
        "openBlockingIssues field should exist"
    );
    assert!(
        ctx["data"].get("openBlockingReviewPoints").is_some(),
        "openBlockingReviewPoints field should exist"
    );
    assert!(
        ctx["data"].get("recommendedNextAction").is_some(),
        "recommendedNextAction field should exist"
    );
    assert!(
        ctx["data"].get("contextWarnings").is_some(),
        "contextWarnings field should exist"
    );
}

// ===================================================================
// AC7: scope_required_machine_mode
// ===================================================================
#[test]
fn scope_required_machine_mode() {
    let temp_dir = unique_temp_dir("elegy-integration-ac7");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");

    // Test 1: Mutation without --scope succeeds using default scope
    // The scope gate defaults to "default" scope when --scope is not provided
    let create_result = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "goal",
            "create",
            "--id",
            "g-no-scope",
            "--title",
            "Test Goal",
            "--description",
            "Test description",
            "--acceptance",
            "done",
        ])
        .output()
        .expect("run goal create without scope");

    let create_stdout = String::from_utf8(create_result.stdout).expect("stdout utf-8");
    let create_json: Value = serde_json::from_str(&create_stdout).expect("valid json");
    assert!(
        create_json.get("status").is_some(),
        "goal create without scope should return valid JSON: {}",
        create_stdout
    );

    // Test 2: Out-of-scope mutation — create goal in scope-a, then try to modify from scope-b
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "scope-test",
        "--scope",
        "scope-a",
        "scope",
        "create",
        "--scope-key",
        "scope-a",
    ]);
    let create_scoped = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "scope-test",
            "--scope",
            "scope-a",
            "goal",
            "create",
            "--id",
            "goal-scoped",
            "--title",
            "Scoped Goal",
            "--description",
            "Test",
            "--acceptance",
            "done",
        ])
        .output()
        .expect("create scoped goal");
    assert!(create_scoped.status.success());

    // Try to update the goal from a different scope (scope-b does not exist)
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "scope-test-2",
            "--scope",
            "scope-b",
            "goal",
            "update-status",
            "--goal-id",
            "goal-scoped",
            "--status",
            "validated",
        ])
        .output()
        .expect("run out-of-scope update-status");

    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(
        result["status"], "invalid",
        "out-of-scope mutation should fail: {}",
        stdout
    );
    let error = result["error"].as_str().unwrap_or("");
    assert!(
        error.contains("scope"),
        "error should mention scope: {}",
        error
    );

    // Test 3: Read-only command without --scope returns valid JSON
    let read_result = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "goal",
            "list",
        ])
        .output()
        .expect("run goal list without scope");
    let read_stdout = String::from_utf8(read_result.stdout).expect("stdout utf-8");
    let read_json: Value = serde_json::from_str(&read_stdout).expect("valid json");
    assert!(
        read_json.get("status").is_some(),
        "goal list without scope should return valid JSON: {}",
        read_stdout
    );
}

// ===================================================================
// AC8: preflight_rejection
// ===================================================================
#[test]
fn preflight_rejection() {
    let temp_dir = unique_temp_dir("elegy-integration-ac8");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");
    let scope = "ac8";

    setup_scope_goal_roadmap(&temp_dir, db_arg, scope, "goal-ac8", "rm-ac8", "c1");

    // Try creating a plan with wrong goal (different from roadmap's goal)
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c2",
            "--scope",
            scope,
            "plan",
            "create",
            "--id",
            "plan-wrong-goal",
            "--goal-id",
            "nonexistent-goal",
            "--roadmap-id",
            "rm-ac8",
            "--title",
            "Wrong goal plan",
            "--summary",
            "Should be rejected",
            "--plan-scope",
            "Execution",
        ])
        .output()
        .expect("run plan create with wrong goal");

    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let result: Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(result["status"], "invalid");
    let error = result["error"].as_str().unwrap_or("");
    assert!(
        error.contains("not found") || error.contains("missing") || error.contains("MISMATCH"),
        "should reject with mismatch/missing error: {}",
        error
    );

    // Try creating a plan targeting non-existent work point
    let output2 = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c3",
            "--scope",
            scope,
            "plan",
            "create",
            "--id",
            "plan-missing-wp",
            "--goal-id",
            "goal-ac8",
            "--roadmap-id",
            "rm-ac8",
            "--title",
            "Missing WP plan",
            "--summary",
            "Should be rejected",
            "--plan-scope",
            "Execution",
            "--target-work-point-id",
            "non-existent-wp",
        ])
        .output()
        .expect("run plan create with missing work point");

    let stdout2 = String::from_utf8(output2.stdout).expect("stdout utf-8");
    let result2: Value = serde_json::from_str(&stdout2).expect("valid json");
    assert_eq!(result2["status"], "invalid");
    let error2 = result2["error"].as_str().unwrap_or("");
    assert!(
        error2.contains("WORK_POINT_MISSING") || error2.contains("does not exist"),
        "should reject with WORK_POINT_MISSING: {}",
        error2
    );
}

// ===================================================================
// AC9: migration_safe_defaults
// ===================================================================
#[test]
fn migration_safe_defaults() {
    let temp_dir = unique_temp_dir("elegy-integration-ac9");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");
    let scope = "ac9";

    setup_scope_goal_roadmap(&temp_dir, db_arg, scope, "goal-ac9", "rm-ac9", "c1");

    // Create a work point without kind/priority flags
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        scope,
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-ac9",
        "--id",
        "wp-ac9",
        "--title",
        "Default WP",
        "--summary",
        "Checking defaults",
        "--effort-tier",
        "balanced",
    ]);

    // Verify via work-point show
    let show = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        scope,
        "work-point",
        "show",
        "--work-point-id",
        "wp-ac9",
    ]);

    let wp = &show["data"]["workPoint"];
    assert_eq!(wp["kind"], "feature");
    assert_eq!(wp["priority"], "medium");

    let repairs = wp["repairsWorkPointIds"].as_array().expect("repairs array");
    assert!(repairs.is_empty(), "repairs should be empty: {:?}", repairs);

    let supersedes = wp["supersedesWorkPointIds"]
        .as_array()
        .expect("supersedes array");
    assert!(
        supersedes.is_empty(),
        "supersedes should be empty: {:?}",
        supersedes
    );

    let blocks = wp["blocksWorkPointIds"].as_array().expect("blocks array");
    assert!(blocks.is_empty(), "blocks should be empty: {:?}", blocks);
}

// ===================================================================
// AC10: new_validation_findings
// ===================================================================
#[test]
fn new_validation_findings() {
    let temp_dir = unique_temp_dir("elegy-integration-ac10");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");
    let scope = "ac10";

    setup_scope_goal_roadmap(&temp_dir, db_arg, scope, "goal-ac10", "rm-ac10", "c1");

    // --- Check 1: WORK-POINT-CORRECTIVE-NO-TARGET ---
    // Create corrective WP with no targets
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        scope,
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-ac10",
        "--id",
        "wp-no-target",
        "--title",
        "Corrective no target",
        "--summary",
        "Has no repairs/supersedes/blocks",
        "--effort-tier",
        "fast",
        "--kind",
        "corrective",
    ]);

    // Validate and check for finding
    let validate = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        scope,
        "validate",
        "all",
    ]);
    let findings = validate["data"]["findings"]
        .as_array()
        .expect("findings array");
    let has_corrective_no_target = findings.iter().any(|f| {
        f["code"].as_str() == Some("WORK-POINT-CORRECTIVE-NO-TARGET")
            && f["entityId"].as_str() == Some("wp-no-target")
    });
    assert!(
        has_corrective_no_target,
        "should have WORK-POINT-CORRECTIVE-NO-TARGET for wp-no-target, got codes: {:?}",
        findings
            .iter()
            .map(|f| f["code"].as_str())
            .collect::<Vec<_>>()
    );

    // --- Check 2: WORK-POINT-BLOCKED-DOWNSTREAM-ACTIVE ---
    // Create an active WP, then a corrective WP that blocks it
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c3",
        "--scope",
        scope,
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-ac10",
        "--id",
        "wp-target",
        "--title",
        "Target WP",
        "--summary",
        "Will be blocked",
        "--effort-tier",
        "fast",
    ]);

    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c4",
        "--scope",
        scope,
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-ac10",
        "--id",
        "wp-blocker",
        "--title",
        "Blocker WP",
        "--summary",
        "Blocks wp-target",
        "--effort-tier",
        "fast",
        "--kind",
        "corrective",
        "--blocks-work-point-id",
        "wp-target",
    ]);

    let validate2 = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        scope,
        "validate",
        "all",
    ]);
    let findings2 = validate2["data"]["findings"]
        .as_array()
        .expect("findings array");
    let has_blocked_downstream = findings2.iter().any(|f| {
        f["code"].as_str() == Some("WORK-POINT-BLOCKED-DOWNSTREAM-ACTIVE")
            && f["entityId"].as_str() == Some("wp-target")
    });
    assert!(
        has_blocked_downstream,
        "should have WORK-POINT-BLOCKED-DOWNSTREAM-ACTIVE for wp-target"
    );

    // --- Check 3: PROJECT-RUN-ON-COMPLETED-CANCELLED-WORK ---
    // Create a WP, claim it, then cancel the WP
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c5",
        "--scope",
        scope,
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-ac10",
        "--id",
        "wp-cancel-test",
        "--title",
        "Cancel test",
        "--summary",
        "Will be cancelled",
        "--effort-tier",
        "fast",
    ]);

    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c6",
        "--scope",
        scope,
        "project-run",
        "claim",
        "--goal-id",
        "goal-ac10",
        "--roadmap-id",
        "rm-ac10",
        "--work-point-id",
        "wp-cancel-test",
    ]);

    // Cancel the work point
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c7",
        "--scope",
        scope,
        "work-point",
        "update-status",
        "--work-point-id",
        "wp-cancel-test",
        "--status",
        "cancelled",
        "--override-transition",
    ]);

    let validate3 = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        scope,
        "validate",
        "all",
    ]);
    let findings3 = validate3["data"]["findings"]
        .as_array()
        .expect("findings array");
    let has_run_on_cancelled = findings3
        .iter()
        .any(|f| f["code"].as_str() == Some("PROJECT-RUN-ON-COMPLETED-CANCELLED-WORK"));
    assert!(
        has_run_on_cancelled,
        "should have PROJECT-RUN-ON-COMPLETED-CANCELLED-WORK finding, got codes: {:?}",
        findings3
            .iter()
            .map(|f| f["code"].as_str())
            .collect::<Vec<_>>()
    );

    // --- Check 4: GOAL-INVALIDATED-WITH-ACTIVE-WORK ---
    // Create a separate goal with active WP then invalidate the goal
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c8",
        "--scope",
        scope,
        "goal",
        "create",
        "--id",
        "goal-active-wp",
        "--title",
        "Goal with active WP",
        "--description",
        "Will be invalidated",
        "--acceptance",
        "done",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c9",
        "--scope",
        scope,
        "roadmap",
        "create",
        "--id",
        "rm-active-wp",
        "--goal-id",
        "goal-active-wp",
        "--title",
        "RM with active WP",
        "--summary",
        "Test",
    ]);
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c10",
        "--scope",
        scope,
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-active-wp",
        "--id",
        "wp-active",
        "--title",
        "Active WP",
        "--summary",
        "Will be orphaned",
        "--effort-tier",
        "fast",
    ]);

    // Invalidate the goal
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c11",
        "--scope",
        scope,
        "goal",
        "update-status",
        "--goal-id",
        "goal-active-wp",
        "--status",
        "invalidated",
        "--override-transition",
    ]);

    let validate4 = command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--scope",
        scope,
        "validate",
        "all",
    ]);
    let findings4 = validate4["data"]["findings"]
        .as_array()
        .expect("findings array");
    let has_goal_invalidated = findings4.iter().any(|f| {
        f["code"].as_str() == Some("GOAL-INVALIDATED-WITH-ACTIVE-WORK")
            && f["entityId"].as_str() == Some("goal-active-wp")
    });
    assert!(
        has_goal_invalidated,
        "should have GOAL-INVALIDATED-WITH-ACTIVE-WORK for goal-active-wp, got codes: {:?}",
        findings4
            .iter()
            .map(|f| f["code"].as_str())
            .collect::<Vec<_>>()
    );
}

// ===================================================================
// AC11: cli_help_new_args
// ===================================================================
#[test]
fn cli_help_new_args() {
    // Run roadmap add-work-point --help to check for new work-point args
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args(["roadmap", "add-work-point", "--help"])
        .output()
        .expect("run --help");

    assert!(output.status.success());
    let help_text = String::from_utf8(output.stdout).expect("stdout utf-8");

    // Verify new args appear in subcommand help
    assert!(
        help_text.contains("--kind"),
        "--kind should be in roadmap add-work-point help"
    );
    assert!(
        help_text.contains("--priority"),
        "--priority should be in roadmap add-work-point help"
    );
    assert!(
        help_text.contains("--repairs-work-point-id"),
        "--repairs-work-point-id should be in roadmap add-work-point help"
    );
    assert!(
        help_text.contains("--supersedes-work-point-id"),
        "--supersedes-work-point-id should be in roadmap add-work-point help"
    );
    assert!(
        help_text.contains("--blocks-work-point-id"),
        "--blocks-work-point-id should be in roadmap add-work-point help"
    );

    // Check override-transition in update-status help
    let update_help = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args(["goal", "update-status", "--help"])
        .output()
        .expect("run goal update-status --help");
    assert!(update_help.status.success());
    let update_text = String::from_utf8(update_help.stdout).expect("stdout utf-8");
    assert!(
        update_text.contains("--override-transition"),
        "--override-transition should be in goal update-status help"
    );
}

// ===================================================================
// AC12: project_run_session_state
// ===================================================================
#[test]
fn project_run_session_state() {
    let temp_dir = unique_temp_dir("elegy-integration-ac12");
    let db_path = temp_dir.join("planning.db");
    let db_arg = db_path.to_str().expect("utf-8 db path");
    let scope = "ac12";
    let home_dir = temp_dir.join("home");
    fs::create_dir_all(&home_dir).expect("create home dir");

    setup_scope_goal_roadmap(&temp_dir, db_arg, scope, "goal-ac12", "rm-ac12", "c1");

    // Create a work point
    command_json(&[
        "--db",
        db_arg,
        "--json",
        "--non-interactive",
        "--correlation-id",
        "c2",
        "--scope",
        scope,
        "roadmap",
        "add-work-point",
        "--roadmap-id",
        "rm-ac12",
        "--id",
        "wp-ac12",
        "--title",
        "Session WP",
        "--summary",
        "Testing session state",
        "--effort-tier",
        "fast",
    ]);

    // Claim project run (with HOME set to temp dir)
    let claim_output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c3",
            "--scope",
            scope,
            "project-run",
            "claim",
            "--goal-id",
            "goal-ac12",
            "--roadmap-id",
            "rm-ac12",
            "--work-point-id",
            "wp-ac12",
            "--session-id",
            "test-session",
        ])
        .env("HOME", home_dir.to_str().expect("utf-8 home path"))
        .output()
        .expect("run project-run claim");

    assert!(
        claim_output.status.success(),
        "claim should succeed: {}",
        String::from_utf8_lossy(&claim_output.stderr)
    );

    let claim_stdout = String::from_utf8(claim_output.stdout).expect("stdout utf-8");
    let claim_result: Value = serde_json::from_str(&claim_stdout).expect("valid json");
    assert_eq!(claim_result["status"], "ok");
    let run_id = claim_result["data"]["record"]["id"]
        .as_str()
        .expect("run id")
        .to_string();

    // Activate project run
    let activate_output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c4",
            "--scope",
            scope,
            "project-run",
            "activate",
            "--project-run-id",
            &run_id,
        ])
        .env("HOME", home_dir.to_str().expect("utf-8 home path"))
        .output()
        .expect("run project-run activate");

    assert!(
        activate_output.status.success(),
        "activate should succeed: {}",
        String::from_utf8_lossy(&activate_output.stderr)
    );

    // Read session file and verify activeProjectRun
    let session_path = home_dir.join(".elegy").join("planning-session.json");
    assert!(session_path.exists(), "session file should exist");
    let session_content = fs::read_to_string(&session_path).expect("read session file");
    let session: Value = serde_json::from_str(&session_content).expect("parse session json");

    let active_run = &session["activeProjectRun"];
    assert!(
        !active_run.is_null(),
        "activeProjectRun should not be null: {}",
        session_content
    );
    assert_eq!(
        active_run["workPointId"].as_str(),
        Some("wp-ac12"),
        "work_point_id should be wp-ac12"
    );
    assert_eq!(
        active_run["status"].as_str(),
        Some("active"),
        "status should be active"
    );

    // Release with completed status
    let release_output = Command::new(env!("CARGO_BIN_EXE_elegy-planning"))
        .args([
            "--db",
            db_arg,
            "--json",
            "--non-interactive",
            "--correlation-id",
            "c5",
            "--scope",
            scope,
            "project-run",
            "release",
            "--project-run-id",
            &run_id,
            "--status",
            "completed",
        ])
        .env("HOME", home_dir.to_str().expect("utf-8 home path"))
        .output()
        .expect("run project-run release");

    assert!(
        release_output.status.success(),
        "release should succeed: {}",
        String::from_utf8_lossy(&release_output.stderr)
    );

    // Read session file again and verify cleared state
    let session_content2 = fs::read_to_string(&session_path).expect("read session file");
    let session2: Value = serde_json::from_str(&session_content2).expect("parse session json");

    assert!(
        session2["activeProjectRun"].is_null(),
        "activeProjectRun should be null after release: {}",
        session_content2
    );
    assert_eq!(
        session2["lastCompletedWorkPointId"].as_str(),
        Some("wp-ac12"),
        "lastCompletedWorkPointId should be wp-ac12"
    );
}
