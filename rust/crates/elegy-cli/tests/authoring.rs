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
