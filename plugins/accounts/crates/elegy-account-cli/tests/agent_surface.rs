use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use std::fs;
use std::process::{Command as StdCommand, Stdio};
use std::sync::Arc;
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
        "account_attention_list",
        "account_present",
        "account_cancel_request",
        "account_resume_request",
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
            "account_attention_list",
            "account_audit_list",
            "account_cancel_request",
            "account_discover",
            "account_list",
            "account_open_center",
            "account_present",
            "account_request_access",
            "account_request_creation",
            "account_request_status",
            "account_require",
            "account_resume_request",
            "account_revoke_grant",
        ]
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn action_mcp_advertises_only_the_bundled_typed_read_operations() {
    let local_data = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_elegy-accounts"));
    command
        .arg("actions-mcp")
        .env("LOCALAPPDATA", local_data.path());
    let client = ()
        .serve(
            TokioChildProcess::new(command.configure(|child| {
                child.kill_on_drop(true);
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    let mut names: Vec<_> = client
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "cloudflare_dns_records_read",
            "cloudflare_zones_read",
            "github_profile_read",
            "github_repositories_read",
        ]
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
#[cfg(windows)]
async fn action_mcp_executes_a_typed_read_through_the_running_broker() {
    use axum::{Json, Router, http::HeaderMap, routing::get};
    use elegy_accountd::{BrokerStore, DpapiProtector, NewAccessRequest, Vault};
    use serde_json::json;

    let app = Router::new().route(
        "/user",
        get(|headers: HeaderMap| async move {
            assert_eq!(
                headers.get("authorization").expect("authorization"),
                "Bearer action-secret-canary"
            );
            Json(json!({"login":"action-user"}))
        }),
    );
    let provider_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider_base = format!("http://{}", provider_listener.local_addr().unwrap());
    let provider_task = tokio::spawn(async move {
        axum::serve(provider_listener, app).await.unwrap();
    });

    let local_data = tempfile::tempdir().unwrap();
    let provider_dir = tempfile::tempdir().unwrap();
    let manifest = format!(
        r#"{{
          "schema_version":"elegy-account-provider/v2",
          "id":"github","display_name":"GitHub","version":"2.0.0","publisher":"test",
          "browser_origins":["{provider_base}"],
          "auth_profiles":[{{
            "id":"device","method":"api_token","audience":"{provider_base}",
            "identity":{{"url":"{provider_base}/user","selectors":["/login"]}},
            "client":{{"mode":"user_provided"}},"scopes":["read:user"]
          }}],
          "operations":{{
            "profile.read":{{
              "description":"Read profile.","risk":"read","scopes":["read:user"],
              "input_schema":{{"type":"object","additionalProperties":false}},
              "result_schema":{{"type":"object"}},
              "executor":{{"kind":"http","profile":"device","method":"GET","path":"/user"}}
            }}
          }}
        }}"#
    );
    fs::write(provider_dir.path().join("github.json"), manifest).unwrap();

    let database = local_data
        .path()
        .join("Elegy")
        .join("Accounts")
        .join("accounts.sqlite");
    fs::create_dir_all(database.parent().unwrap()).unwrap();
    let broker_store = BrokerStore::new(Vault::open(&database, Arc::new(DpapiProtector)).unwrap());
    let account = broker_store
        .vault()
        .store_account(
            "github",
            "action-user",
            "api_token",
            b"action-secret-canary",
        )
        .unwrap();
    let access = broker_store
        .request_access(NewAccessRequest {
            account_id: account.id.clone(),
            client_id: "codex-actions".into(),
            purpose: "github.profile.read".into(),
            operations: vec!["profile.read".into()],
            duration_minutes: 43_200,
        })
        .unwrap();
    broker_store.approve_access(&access.id).unwrap();
    drop(broker_store);

    let port_probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = port_probe.local_addr().unwrap().port();
    drop(port_probe);
    let port_text = port.to_string();
    let pipe_name = format!(r"\\.\pipe\elegy-accounts-test-{}", std::process::id());
    let mut broker = Command::new(env!("CARGO_BIN_EXE_elegy-accounts"));
    broker
        .arg("broker")
        .env("LOCALAPPDATA", local_data.path())
        .env("ELEGY_ACCOUNTS_PROVIDER_DIR", provider_dir.path())
        .env("ELEGY_ACCOUNTS_TRUST_LOCAL_PACKS", "1")
        .env("ELEGY_ACCOUNT_CENTER_PORT", &port_text)
        .env("ELEGY_ACCOUNTS_PIPE_NAME", &pipe_name)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut broker = broker.spawn().unwrap();
    let health = format!("http://127.0.0.1:{port}/api/state");
    let http = reqwest::Client::new();
    for _ in 0..50 {
        if http
            .get(&health)
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let mut action = Command::new(env!("CARGO_BIN_EXE_elegy-accounts"));
    action
        .arg("actions-mcp")
        .env("LOCALAPPDATA", local_data.path())
        .env("ELEGY_ACCOUNTS_PROVIDER_DIR", provider_dir.path())
        .env("ELEGY_ACCOUNTS_TRUST_LOCAL_PACKS", "1")
        .env("ELEGY_ACCOUNT_CENTER_PORT", &port_text)
        .env("ELEGY_ACCOUNTS_PIPE_NAME", &pipe_name);
    let client = ()
        .serve(
            TokioChildProcess::new(action.configure(|child| {
                child.kill_on_drop(true);
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    let result = client
        .call_tool(
            CallToolRequestParams::new("github_profile_read").with_arguments(
                json!({"account_id":account.id})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    let public = serde_json::to_value(result).unwrap();
    let text = public
        .pointer("/content/0/text")
        .and_then(|value| value.as_str())
        .unwrap();
    assert!(text.contains("action-user"), "{text}");
    assert!(!text.contains("action-secret-canary"));
    assert!(!text.contains("ela_"));

    client.cancel().await.unwrap();
    broker.kill().await.unwrap();
    provider_task.abort();
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

#[test]
fn open_can_target_a_durable_request_without_putting_secrets_in_the_url() {
    let local_data = tempfile::tempdir().expect("temp data directory");
    let output = StdCommand::new(env!("CARGO_BIN_EXE_elegy-accounts"))
        .args(["open", "--print-url", "--request", "auth_fixture-1"])
        .env("LOCALAPPDATA", local_data.path())
        .output()
        .expect("open command should run");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "http://127.0.0.1:43119/?request=auth_fixture-1"
    );

    let rejected = StdCommand::new(env!("CARGO_BIN_EXE_elegy-accounts"))
        .args(["open", "--print-url", "--request", "unsafe&token=secret"])
        .env("LOCALAPPDATA", local_data.path())
        .output()
        .expect("open command should run");
    assert!(!rejected.status.success());
}
