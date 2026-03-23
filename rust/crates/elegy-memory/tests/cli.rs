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
fn validate_command_defaults_to_text_output() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-validate-text");
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

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "validate",
            "--input",
            input_path.to_str().expect("utf-8 input path"),
        ])
        .output()
        .expect("run elegy-memory validate text mode");

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
fn top_level_local_commands_derive_state_from_artifacts() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-local");
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

    let init = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "init",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy-memory init");
    assert!(
        init.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(!root.join("state").join("catalog.json").exists());

    for (record_id, imported_at_utc, input_path) in [
        ("record-a", "2026-03-23T00:00:00Z", &input_a),
        ("record-b", "2026-03-23T01:00:00Z", &input_b),
        ("record-c", "2026-03-23T02:00:00Z", &input_c),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
            .args([
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
            .expect("run elegy-memory import");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let supersede = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
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
        .expect("run elegy-memory supersede");
    assert!(
        supersede.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&supersede.stderr)
    );

    let tombstone = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
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
        .expect("run elegy-memory tombstone");
    assert!(
        tombstone.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&tombstone.stderr)
    );

    let default_list = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "list",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy-memory list default");
    assert!(
        default_list.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&default_list.stderr)
    );
    let default_stdout = String::from_utf8(default_list.stdout).expect("stdout should be utf-8");
    assert!(default_stdout.contains("\"recordId\": \"record-b\""));
    assert!(!default_stdout.contains("\"recordId\": \"record-a\""));
    assert!(!default_stdout.contains("\"recordId\": \"record-c\""));
    assert!(!default_stdout.contains("catalogPath"));

    let list_all = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "list",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--include-superseded",
            "--include-tombstoned",
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy-memory list all");
    assert!(
        list_all.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&list_all.stderr)
    );
    let list_all_stdout = String::from_utf8(list_all.stdout).expect("stdout should be utf-8");
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

    let show = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "show",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--record-id",
            "record-b",
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy-memory show");
    assert!(
        show.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&show.stderr)
    );
    let show_stdout = String::from_utf8(show.stdout).expect("stdout should be utf-8");
    assert!(show_stdout.contains("\"recordId\": \"record-b\""));

    let export = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "export",
            "--root",
            root.to_str().expect("utf-8 root path"),
            "--record-id",
            "record-b",
            "--format",
            "json",
        ])
        .output()
        .expect("run elegy-memory export");
    assert!(
        export.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&export.stderr)
    );

    let export_path = root
        .join("exports")
        .join("record-b.summary-only-session-context-envelope.json");
    let exported_contents = fs::read_to_string(export_path).expect("read exported artifact");
    assert!(exported_contents.contains("summary-only-session-context-envelope"));
}
