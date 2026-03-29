use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const CANONICAL_WORKFLOW_FIXTURE: &str =
    include_str!("../../../../contracts/fixtures/canonical-workflow.minimal.json");
const CANONICAL_WORKFLOW_GRAPH_FIXTURE: &str =
    include_str!("../../../../contracts/fixtures/canonical-workflow-graph.minimal.json");
const RENDERED_WORKFLOW_MERMAID: &str = concat!(
    "flowchart TD\n",
    "    step_step_fulfill[\"Fulfill Order\"]\n",
    "    step_step_review[\"Review Order\"]\n",
    "    trigger_trigger_order_created((\"Order Created\"))\n",
    "    trigger_trigger_order_created --> step_step_review\n",
    "    step_step_review -->|approved| step_step_fulfill"
);

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}-{unique}"));
    fs::create_dir_all(&dir).expect("create temp directory");
    dir
}

#[test]
fn mermaid_render_command_renders_canonical_workflow_fixture_from_file() {
    let temp_dir = unique_temp_dir("elegy-cli-mermaid-workflow");
    let input_path = temp_dir.join("canonical-workflow.json");
    fs::write(&input_path, CANONICAL_WORKFLOW_FIXTURE).expect("write workflow fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "mermaid",
            "render",
            "--input",
            input_path.to_str().expect("utf-8 workflow input path"),
        ])
        .output()
        .expect("run elegy mermaid render with workflow file input");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert_eq!(
        stdout.trim_end(),
        concat!(
            "flowchart TD\n",
            "    step_step_fulfill[\"Fulfill Order\"]\n",
            "    step_step_review[\"Review Order\"]\n",
            "    trigger_trigger_order_created((\"Order Created\"))\n",
            "    trigger_trigger_order_created --> step_step_review\n",
            "    step_step_review -->|approved| step_step_fulfill"
        )
    );
}

#[test]
fn mermaid_render_command_reads_canonical_workflow_graph_from_stdin() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args(["mermaid", "render"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn elegy mermaid render for stdin input");

    child
        .stdin
        .as_mut()
        .expect("stdin pipe should be available")
        .write_all(CANONICAL_WORKFLOW_GRAPH_FIXTURE.as_bytes())
        .expect("write workflow graph fixture to stdin");

    let output = child
        .wait_with_output()
        .expect("wait for elegy mermaid render stdin run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert_eq!(
        stdout.trim_end(),
        concat!(
            "flowchart TD\n",
            "    node_step_collect[\"Collect\"]\n",
            "    trigger_contract_updated((\"contract.updated\"))\n",
            "    trigger_contract_updated --> node_step_collect"
        )
    );
}

#[test]
fn mermaid_render_command_rejects_unsupported_canonical_json() {
    let temp_dir = unique_temp_dir("elegy-cli-mermaid-invalid");
    let input_path = temp_dir.join("unsupported.json");
    fs::write(
        &input_path,
        r#"{
  "artifactKind": "summary-only-session-context-envelope",
  "sessionContext": {
    "scope": "workspace",
    "representation": "summary-only",
    "summary": "Not a Mermaid renderable canonical workflow shape.",
    "rawTranscriptPersisted": false
  }
}
"#,
    )
    .expect("write unsupported JSON fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "mermaid",
            "render",
            "--input",
            input_path.to_str().expect("utf-8 unsupported input path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy mermaid render with unsupported JSON input");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("CLI-MERMAID-003"));
    assert!(stdout.contains("expected canonical workflow or canonical workflow graph JSON"));
}

#[test]
fn mermaid_render_command_rejects_undeclared_workflow_step_reference() {
    let temp_dir = unique_temp_dir("elegy-cli-mermaid-invalid-reference");
    let input_path = temp_dir.join("invalid-workflow.json");
    let workflow = r#"{
    "id": "wf.order-approval",
    "name": "Order Approval",
    "specVersion": "1.0",
    "canonicalAuthority": "blueprint",
    "conflictPolicy": "reconcile",
    "blueprint": {
        "blueprintId": "bp.sales.order-approval",
        "version": "2026.01",
        "isPinned": true
    },
    "triggers": [
        {
            "id": "trigger.order-created",
            "name": "Order Created",
            "type": "event",
            "targetStepId": "step.review"
        }
    ],
    "steps": [
        {
            "id": "step.review",
            "name": "Review Order",
            "type": "human-task"
        }
    ],
    "connections": [
        {
            "id": "conn.review-to-missing",
            "fromStepId": "step.review",
            "toStepId": "step.missing",
            "label": "approved"
        }
    ],
    "layout": {
        "groups": [
            {
                "id": "group.main",
                "name": "Main Lane",
                "x": 0,
                "y": 0,
                "width": 1200,
                "height": 400
            }
        ],
        "positions": [
            {
                "stepId": "step.review",
                "x": 200,
                "y": 120
            }
        ]
    }
}
"#;
    fs::write(&input_path, workflow).expect("write invalid workflow fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "mermaid",
            "render",
            "--input",
            input_path
                .to_str()
                .expect("utf-8 invalid workflow input path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy mermaid render with invalid workflow input");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("CLI-MERMAID-005"));
    assert!(stdout.contains("connections.toStepId"));
    assert!(stdout.contains("step.missing"));
}

#[test]
fn mermaid_render_command_rejects_duplicate_workflow_step_ids() {
    let temp_dir = unique_temp_dir("elegy-cli-mermaid-duplicate-steps");
    let input_path = temp_dir.join("duplicate-workflow-steps.json");
    fs::write(
        &input_path,
        r#"{
    "id": "wf.order-approval",
    "name": "Order Approval",
    "specVersion": "1.0",
    "canonicalAuthority": "blueprint",
    "conflictPolicy": "reconcile",
    "blueprint": {
        "blueprintId": "bp.sales.order-approval",
        "version": "2026.01",
        "isPinned": true
    },
    "triggers": [
        {
            "id": "trigger.order-created",
            "name": "Order Created",
            "type": "event",
            "targetStepId": "step.review"
        }
    ],
    "steps": [
        {
            "id": "step.review",
            "name": "Review Order",
            "type": "human-task"
        },
        {
            "id": "step.review",
            "name": "Review Order Duplicate",
            "type": "service-task"
        }
    ],
    "connections": [],
    "layout": {
        "groups": [
            {
                "id": "group.main",
                "name": "Main Lane",
                "x": 0,
                "y": 0,
                "width": 1200,
                "height": 400
            }
        ],
        "positions": [
            {
                "stepId": "step.review",
                "x": 200,
                "y": 120
            }
        ]
    }
}
"#,
    )
    .expect("write duplicate workflow step fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "mermaid",
            "render",
            "--input",
            input_path
                .to_str()
                .expect("utf-8 duplicate workflow step input path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy mermaid render with duplicate workflow steps");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("CLI-MERMAID-005"));
    assert!(stdout.contains("steps.id"));
    assert!(stdout.contains("step.review"));
}

#[test]
fn mermaid_reverse_command_projects_mermaid_from_file() {
    let temp_dir = unique_temp_dir("elegy-cli-mermaid-reverse");
    let input_path = temp_dir.join("workflow.mmd");
    fs::write(&input_path, RENDERED_WORKFLOW_MERMAID).expect("write Mermaid fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "mermaid",
            "reverse",
            "--input",
            input_path.to_str().expect("utf-8 Mermaid input path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy mermaid reverse with Mermaid file input");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"ok\""));
    assert!(stdout.contains("\"projectionKind\": \"workflow-graph-semantics\""));
    assert!(stdout.contains("\"sourceKind\": \"mermaidFlowchartTd\""));
    assert!(stdout.contains("\"entryNodeIds\": ["));
    assert!(stdout.contains("step_step_review"));
    assert!(stdout.contains("\"input_source\": \"file\""));
}

#[test]
fn mermaid_narrate_command_accepts_mermaid_from_stdin() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args(["mermaid", "narrate", "--format", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn elegy mermaid narrate for stdin input");

    child
        .stdin
        .as_mut()
        .expect("stdin pipe should be available")
        .write_all(RENDERED_WORKFLOW_MERMAID.as_bytes())
        .expect("write Mermaid fixture to stdin");

    let output = child
        .wait_with_output()
        .expect("wait for elegy mermaid narrate stdin run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"ok\""));
    assert!(stdout.contains("\"sourceKind\": \"mermaidFlowchartTd\""));
    assert!(stdout.contains(
        "derived Mermaid projection only; canonical workflow authority remains outside Mermaid"
    ));
    assert!(stdout.contains("Order Created activates Review Order."));
}

#[test]
fn mermaid_reverse_command_rejects_unsupported_mermaid_direction() {
    let temp_dir = unique_temp_dir("elegy-cli-mermaid-reverse-invalid");
    let input_path = temp_dir.join("workflow-invalid.mmd");
    fs::write(&input_path, "flowchart LR\n    step_a[\"A\"]")
        .expect("write invalid Mermaid fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy"))
        .args([
            "mermaid",
            "reverse",
            "--input",
            input_path
                .to_str()
                .expect("utf-8 invalid Mermaid input path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy mermaid reverse with invalid Mermaid input");

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("\"status\": \"invalid\""));
    assert!(stdout.contains("CLI-MERMAID-006"));
    assert!(stdout.contains("flowchart LR"));
}
