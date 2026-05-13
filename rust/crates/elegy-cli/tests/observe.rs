use serde_json::Value;
use std::process::Command;

fn elegy() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elegy"))
}

#[cfg(windows)]
#[test]
fn observe_record_emits_observation_session_json() {
    let output = elegy()
        .args([
            "--json",
            "observe",
            "record",
            "--duration-seconds",
            "1",
            "--poll-interval-ms",
            "50",
        ])
        .output()
        .expect("run elegy observe record");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body: Value = serde_json::from_slice(&output.stdout).expect("valid JSON output");
    assert_eq!(body["status"], "ok");
    assert_eq!(
        body["dataSchema"],
        "https://elegy/contracts/schemas/observation-session.schema.json"
    );
    assert_eq!(body["data"]["artifactKind"], "observation-session");
    assert!(body["data"]["sessionId"].is_string());
    assert!(body["data"]["summary"]["summary"].is_string());
}
