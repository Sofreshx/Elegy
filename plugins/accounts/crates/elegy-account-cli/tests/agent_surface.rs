use rmcp::{
    ServiceExt,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use std::fs;
use std::process::Command as StdCommand;
use tokio::process::Command;

#[test]
fn agent_surface_has_account_tools_but_no_secret_or_raw_execution_tool() {
    let source = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs")).unwrap();
    for tool in [
        "account_list",
        "account_discover",
        "account_require",
        "account_request_access",
        "account_request_creation",
        "account_request_status",
        "account_open_center",
        "account_revoke_grant",
        "account_audit_list",
    ] {
        assert!(source.contains(&format!("fn {tool}")), "missing {tool}");
    }
    for forbidden in [
        "fn secret_read",
        "fn credential_get",
        "fn execute_http",
        "fn spawn_process",
        "fn browser_cookie",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden agent tool: {forbidden}"
        );
    }
}

#[tokio::test]
async fn mcp_server_advertises_only_the_bounded_account_tools() {
    let local_data = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_elegy-accounts"));
    command.env("LOCALAPPDATA", local_data.path());
    let client = ()
        .serve(
            TokioChildProcess::new(command.configure(|child| {
                child.kill_on_drop(true);
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    let tools = client.list_all_tools().await.unwrap();
    let access = tools
        .iter()
        .find(|tool| tool.name == "account_request_access")
        .unwrap();
    let access_schema = serde_json::to_value(&access.input_schema)
        .unwrap()
        .to_string();
    assert!(
        !access_schema.contains("client_id"),
        "transport identity must not be agent-selectable"
    );
    let mut names: Vec<_> = tools
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "account_audit_list",
            "account_discover",
            "account_list",
            "account_open_center",
            "account_request_access",
            "account_request_creation",
            "account_request_status",
            "account_require",
            "account_revoke_grant",
        ]
    );
    client.cancel().await.unwrap();
}

#[test]
fn status_is_machine_readable_and_secret_free() {
    let local_data = tempfile::tempdir().expect("temp data directory");
    let output = StdCommand::new(env!("CARGO_BIN_EXE_elegy-accounts"))
        .args(["status", "--json"])
        .env("LOCALAPPDATA", local_data.path())
        .output()
        .expect("status command should run");

    assert!(output.status.success());
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("status should emit JSON");
    assert_eq!(value["schemaVersion"], "elegy-accounts-status/v1");
    assert_eq!(value["localOnly"], true);
    assert!(value["connectedAccounts"].is_number());
    let serialized = value.to_string().to_ascii_lowercase();
    for forbidden in ["access_token", "refresh_token", "client_secret", "password"] {
        assert!(!serialized.contains(forbidden));
    }
}

#[test]
fn unknown_commands_fail_closed() {
    let local_data = tempfile::tempdir().expect("temp data directory");
    let output = StdCommand::new(env!("CARGO_BIN_EXE_elegy-accounts"))
        .arg("definitely-not-a-command")
        .env("LOCALAPPDATA", local_data.path())
        .output()
        .expect("command should run");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown command"));
}

#[test]
fn open_can_return_the_local_center_url_without_launching() {
    let local_data = tempfile::tempdir().expect("temp data directory");
    let output = StdCommand::new(env!("CARGO_BIN_EXE_elegy-accounts"))
        .args(["open", "--print-url"])
        .env("LOCALAPPDATA", local_data.path())
        .output()
        .expect("open command should run");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "http://127.0.0.1:43119/"
    );
}
