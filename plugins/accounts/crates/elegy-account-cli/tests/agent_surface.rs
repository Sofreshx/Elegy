use elegy_plugin_sdk::{PluginArchiveBinary, pack_plugin_v3_with_binary};
use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use std::fs;
use std::fs::File;
use std::path::PathBuf;
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
async fn packaged_accounts_archive_preserves_surface_and_advertises_all_tools() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .unwrap();
    let plugin_root = repo_root.join("plugins/accounts");
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("elegy-accounts.zip");
    let binary_source = PathBuf::from(env!("CARGO_BIN_EXE_elegy-accounts"));
    let binary_name = format!("bin/elegy-accounts{}", std::env::consts::EXE_SUFFIX);

    pack_plugin_v3_with_binary(
        &plugin_root,
        &archive,
        Some(PluginArchiveBinary {
            source_path: &binary_source,
            archive_path: binary_name.clone(),
        }),
    )
    .expect("Accounts archive should package successfully");

    let install = temp.path().join("installed");
    fs::create_dir_all(&install).unwrap();
    let file = File::open(&archive).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    zip.extract(&install).unwrap();
    for required in [
        "plugin.json",
        "capability-catalog.json",
        ".mcp.json",
        "readiness.json",
        binary_name.as_str(),
    ] {
        assert!(
            install.join(required).is_file(),
            "missing packaged {required}"
        );
    }
    let packaged_manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(install.join("plugin.json")).unwrap()).unwrap();
    assert_eq!(packaged_manifest["schemaVersion"], "elegy-plugin/v3");
    let packaged_catalog: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(install.join("capability-catalog.json")).unwrap())
            .unwrap();
    assert_eq!(
        packaged_catalog["schemaVersion"],
        "elegy-capability-catalog/v2"
    );

    let local_data = tempfile::tempdir().unwrap();
    for (argument, expected) in [
        (
            None,
            vec![
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
            ],
        ),
        (
            Some("actions-mcp"),
            vec![
                "cloudflare_dns_records_read",
                "cloudflare_zones_read",
                "github_profile_read",
                "github_repositories_read",
            ],
        ),
    ] {
        let binary = install.join(&binary_name);
        let mut command = Command::new(binary);
        if let Some(argument) = argument {
            command.arg(argument);
        }
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
        let mut names: Vec<_> = client
            .list_all_tools()
            .await
            .unwrap()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();
        names.sort();
        assert_eq!(names, expected);
        client.cancel().await.unwrap();
    }
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
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(200))
        .build()
        .unwrap();
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

#[tokio::test]
#[cfg(windows)]
async fn packaged_live_proof_calls_declared_mcp_and_proves_post_disconnect_failure() {
    use axum::{Json, Router, http::HeaderMap, routing::get};
    use elegy_accountd::{BrokerStore, DpapiProtector, NewAccessRequest, Vault};
    use serde_json::json;

    let app = Router::new().route(
        "/user",
        get(|headers: HeaderMap| async move {
            assert_eq!(
                headers.get("authorization").expect("authorization"),
                "Bearer packaged-live-secret-canary"
            );
            Json(json!({"login":"packaged-live-user","id":4242}))
        }),
    );
    let provider_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider_base = format!("http://{}", provider_listener.local_addr().unwrap());
    let provider_task = tokio::spawn(async move {
        axum::serve(provider_listener, app).await.unwrap();
    });

    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("elegy-accounts.zip");
    let binary_source = PathBuf::from(env!("CARGO_BIN_EXE_elegy-accounts"));
    let binary_name = format!("bin/elegy-accounts{}", std::env::consts::EXE_SUFFIX);
    pack_plugin_v3_with_binary(
        &repo_root.join("plugins/accounts"),
        &archive,
        Some(PluginArchiveBinary {
            source_path: &binary_source,
            archive_path: binary_name.clone(),
        }),
    )
    .unwrap();
    let install = temp.path().join("installed");
    fs::create_dir_all(&install).unwrap();
    zip::ZipArchive::new(File::open(&archive).unwrap())
        .unwrap()
        .extract(&install)
        .unwrap();
    fs::write(
        install.join("providers/github.json"),
        format!(
            r#"{{
              "schema_version":"elegy-account-provider/v2",
              "id":"github","display_name":"GitHub","version":"2.0.0","publisher":"test",
              "browser_origins":["{provider_base}"],
              "auth_profiles":[{{
                "id":"device","method":"api_token","audience":"{provider_base}",
                "identity":{{"url":"{provider_base}/user","selectors":["/login","/id"]}},
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
        ),
    )
    .unwrap();

    let proof_root = temp.path().join("elegy-accounts-live-proof-fixture");
    fs::create_dir_all(&proof_root).unwrap();
    fs::write(
        proof_root.join(".elegy-accounts-live-proof.json"),
        r#"{"schemaVersion":"elegy-accounts-live-proof-root/v1"}"#,
    )
    .unwrap();
    let database = proof_root
        .join("Elegy")
        .join("Accounts")
        .join("accounts.sqlite");
    fs::create_dir_all(database.parent().unwrap()).unwrap();
    let broker_store = BrokerStore::new(Vault::open(&database, Arc::new(DpapiProtector)).unwrap());
    let account = broker_store
        .vault()
        .store_account(
            "github",
            "packaged-live-user",
            "device_authorization",
            b"packaged-live-secret-canary",
        )
        .unwrap();
    let access = broker_store
        .request_access(NewAccessRequest {
            account_id: account.id.clone(),
            client_id: "codex-actions".into(),
            purpose: "github.profile.read".into(),
            operations: vec!["profile.read".into()],
            duration_minutes: 5,
        })
        .unwrap();
    broker_store.approve_access(&access.id).unwrap();
    drop(broker_store);

    let port_probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = port_probe.local_addr().unwrap().port();
    drop(port_probe);
    let pipe_name = format!(
        r"\\.\pipe\elegy-accounts-live-proof-test-{}",
        std::process::id()
    );
    let evidence = temp.path().join("proof.json");
    let output = Command::new(install.join(binary_name))
        .args([
            "proof-github",
            evidence.to_str().unwrap(),
            "--consent=github-device-read-only",
        ])
        .env("LOCALAPPDATA", &proof_root)
        .env("ELEGY_ACCOUNTS_PROOF_ROOT", &proof_root)
        .env("ELEGY_GITHUB_CLIENT_ID", "dedicated-device-flow-client")
        .env("ELEGY_ACCOUNT_CENTER_PORT", port.to_string())
        .env("ELEGY_ACCOUNTS_PIPE_NAME", pipe_name)
        .output()
        .await
        .unwrap();

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: serde_json::Value = serde_json::from_slice(&fs::read(&evidence).unwrap()).unwrap();
    assert_eq!(receipt["schemaVersion"], "elegy-accounts-live-proof/v1");
    assert_eq!(receipt["result"], "passed");
    assert_eq!(receipt["provider"], "github");
    assert_eq!(receipt["verifiedIdentity"], "packaged-live-user");
    assert_eq!(receipt["package"]["installed"], false);
    assert_eq!(
        receipt["nonFixture"], false,
        "a loopback provider pack must never qualify as non-fixture evidence"
    );
    assert_eq!(receipt["interface"]["kind"], "mcp");
    assert_eq!(receipt["interface"]["server"], "elegy-account-actions");
    assert_eq!(receipt["interface"]["tool"], "github_profile_read");
    assert_eq!(receipt["task"]["providerStatus"], 200);
    assert_eq!(receipt["task"]["remoteMutations"], 0);
    assert_eq!(receipt["revocation"]["localConnectionRemoved"], true);
    assert_eq!(
        receipt["revocation"]["postRevokeError"],
        "account_unavailable"
    );
    assert_eq!(receipt["redaction"]["passed"], true);
    let public = format!(
        "{}{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        receipt
    );
    assert!(!public.contains("packaged-live-secret-canary"));

    assert!(
        !database.exists(),
        "the disposable encrypted vault must be removed after the successful scan"
    );
    provider_task.abort();
}

#[tokio::test]
#[cfg(windows)]
async fn packaged_live_proof_waits_for_device_flow_and_matching_action_approval() {
    use axum::{Json, Router, http::HeaderMap, routing::get, routing::post};
    use serde_json::json;

    async fn device(headers: HeaderMap, body: String) -> Json<serde_json::Value> {
        assert_eq!(headers.get("accept").unwrap(), "application/json");
        assert!(body.contains("client_id=dedicated-device-flow-client"));
        Json(json!({
            "device_code":"device-secret-canary",
            "user_code":"ABCD-EFGH",
            "verification_uri":"https://github.com/login/device",
            "expires_in":900,
            "interval":1
        }))
    }
    async fn token(headers: HeaderMap, body: String) -> Json<serde_json::Value> {
        assert_eq!(headers.get("accept").unwrap(), "application/json");
        assert!(body.contains("device_code=device-secret-canary"));
        Json(json!({
            "access_token":"device-access-secret-canary",
            "token_type":"bearer",
            "scope":"read:user"
        }))
    }
    async fn identity(headers: HeaderMap) -> Json<serde_json::Value> {
        assert_eq!(
            headers.get("authorization").unwrap(),
            "Bearer device-access-secret-canary"
        );
        Json(json!({"login":"device-live-user","id":4343}))
    }
    let provider_app = Router::new()
        .route("/device/code", post(device))
        .route("/oauth/access_token", post(token))
        .route("/user", get(identity));
    let provider_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider_base = format!("http://{}", provider_listener.local_addr().unwrap());
    let provider_task = tokio::spawn(async move {
        axum::serve(provider_listener, provider_app).await.unwrap();
    });

    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("elegy-accounts.zip");
    let binary_source = PathBuf::from(env!("CARGO_BIN_EXE_elegy-accounts"));
    let binary_name = format!("bin/elegy-accounts{}", std::env::consts::EXE_SUFFIX);
    pack_plugin_v3_with_binary(
        &repo_root.join("plugins/accounts"),
        &archive,
        Some(PluginArchiveBinary {
            source_path: &binary_source,
            archive_path: binary_name.clone(),
        }),
    )
    .unwrap();
    let install = temp.path().join("installed");
    fs::create_dir_all(&install).unwrap();
    zip::ZipArchive::new(File::open(&archive).unwrap())
        .unwrap()
        .extract(&install)
        .unwrap();
    fs::write(
        install.join("providers/github.json"),
        format!(
            r#"{{
              "schema_version":"elegy-account-provider/v2",
              "id":"github","display_name":"GitHub","version":"2.0.0","publisher":"test",
              "browser_origins":["{provider_base}"],
              "auth_profiles":[{{
                "id":"device","method":"device_authorization","issuer":"{provider_base}",
                "audience":"{provider_base}","token_url":"{provider_base}/oauth/access_token",
                "device_authorization_url":"{provider_base}/device/code",
                "identity":{{"url":"{provider_base}/user","selectors":["/login","/id"]}},
                "client":{{"mode":"environment","client_id_env":"ELEGY_GITHUB_CLIENT_ID"}},
                "scopes":["read:user"]
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
        ),
    )
    .unwrap();

    let proof_root = temp.path().join("elegy-accounts-live-proof-device-fixture");
    fs::create_dir_all(&proof_root).unwrap();
    fs::write(
        proof_root.join(".elegy-accounts-live-proof.json"),
        r#"{"schemaVersion":"elegy-accounts-live-proof-root/v1"}"#,
    )
    .unwrap();
    let port_probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = port_probe.local_addr().unwrap().port();
    drop(port_probe);
    let pipe_name = format!(
        r"\\.\pipe\elegy-accounts-live-proof-device-test-{}",
        std::process::id()
    );
    let evidence = temp.path().join("device-proof.json");
    let mut command = Command::new(install.join(binary_name));
    command
        .args([
            "proof-github",
            evidence.to_str().unwrap(),
            "--consent=github-device-read-only",
        ])
        .env("LOCALAPPDATA", &proof_root)
        .env("ELEGY_ACCOUNTS_PROOF_ROOT", &proof_root)
        .env("ELEGY_GITHUB_CLIENT_ID", "dedicated-device-flow-client")
        .env("ELEGY_LIVE_PROOF_TIMEOUT_SECONDS", "20")
        .env("ELEGY_ACCOUNT_CENTER_PORT", port.to_string())
        .env("ELEGY_ACCOUNTS_PIPE_NAME", pipe_name)
        .kill_on_drop(true);
    let child = command.spawn().unwrap();

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(200))
        .build()
        .unwrap();
    let base = format!("http://127.0.0.1:{port}");
    let mut healthy = false;
    for _ in 0..100 {
        if http
            .get(format!("{base}/api/state"))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            healthy = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        healthy,
        "proof runner did not start packaged Account Center"
    );
    let start = http
        .post(format!("{base}/api/connections/github/start"))
        .header("x-elegy-intent", "user-action")
        .send()
        .await
        .unwrap();
    assert!(start.status().is_success());

    let mut approved = false;
    for _ in 0..200 {
        let state: serde_json::Value = http
            .get(format!("{base}/api/state"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if let Some(request_id) = state["requests"]
            .as_array()
            .and_then(|requests| {
                requests.iter().find(|request| {
                    request["status"] == "awaiting_user"
                        && request["client_id"] == "codex-actions"
                        && request["purpose"] == "github.profile.read"
                })
            })
            .and_then(|request| request["id"].as_str())
        {
            let response = http
                .post(format!("{base}/api/requests/{request_id}/approve"))
                .header("x-elegy-intent", "user-action")
                .send()
                .await
                .unwrap();
            assert!(response.status().is_success());
            approved = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(
        approved,
        "proof runner did not request the Actions MCP grant"
    );

    let output = tokio::time::timeout(std::time::Duration::from_secs(30), child.wait_with_output())
        .await
        .expect("proof runner timed out")
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: serde_json::Value = serde_json::from_slice(&fs::read(&evidence).unwrap()).unwrap();
    assert_eq!(receipt["verifiedIdentity"], "device-live-user");
    assert_eq!(
        receipt["revocation"]["postRevokeError"],
        "account_unavailable"
    );
    assert!(!receipt.to_string().contains("device-access-secret-canary"));
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
fn live_proof_rejects_a_source_binary_before_accessing_any_account() {
    let local_data = tempfile::tempdir().expect("temp data directory");
    let evidence = local_data.path().join("proof.json");
    let output = StdCommand::new(env!("CARGO_BIN_EXE_elegy-accounts"))
        .args([
            "proof-github",
            evidence.to_str().expect("evidence path"),
            "--consent=github-device-read-only",
        ])
        .env("LOCALAPPDATA", local_data.path())
        .env("PATH", "")
        .output()
        .expect("command should run");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("installed Accounts package"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!evidence.exists());
}

#[test]
fn packaged_live_proof_requires_a_dedicated_device_flow_client() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .unwrap();
    let plugin_root = repo_root.join("plugins/accounts");
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("elegy-accounts.zip");
    let binary_source = PathBuf::from(env!("CARGO_BIN_EXE_elegy-accounts"));
    let binary_name = format!("bin/elegy-accounts{}", std::env::consts::EXE_SUFFIX);
    pack_plugin_v3_with_binary(
        &plugin_root,
        &archive,
        Some(PluginArchiveBinary {
            source_path: &binary_source,
            archive_path: binary_name.clone(),
        }),
    )
    .unwrap();
    let install = temp.path().join("installed");
    fs::create_dir_all(&install).unwrap();
    zip::ZipArchive::new(File::open(&archive).unwrap())
        .unwrap()
        .extract(&install)
        .unwrap();

    let local_data = temp.path().join("isolated-local-data");
    fs::create_dir_all(&local_data).unwrap();
    let evidence = temp.path().join("proof.json");
    let output = StdCommand::new(install.join(binary_name))
        .args([
            "proof-github",
            evidence.to_str().unwrap(),
            "--consent=github-device-read-only",
        ])
        .env("LOCALAPPDATA", &local_data)
        .env_remove("ELEGY_GITHUB_CLIENT_ID")
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("ELEGY_GITHUB_CLIENT_ID"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!evidence.exists());
}

#[test]
fn packaged_live_proof_requires_an_explicit_isolated_data_root() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("elegy-accounts.zip");
    let binary_source = PathBuf::from(env!("CARGO_BIN_EXE_elegy-accounts"));
    let binary_name = format!("bin/elegy-accounts{}", std::env::consts::EXE_SUFFIX);
    pack_plugin_v3_with_binary(
        &repo_root.join("plugins/accounts"),
        &archive,
        Some(PluginArchiveBinary {
            source_path: &binary_source,
            archive_path: binary_name.clone(),
        }),
    )
    .unwrap();
    let install = temp.path().join("installed");
    fs::create_dir_all(&install).unwrap();
    zip::ZipArchive::new(File::open(&archive).unwrap())
        .unwrap()
        .extract(&install)
        .unwrap();

    let local_data = temp.path().join("isolated-local-data");
    fs::create_dir_all(&local_data).unwrap();
    let evidence = temp.path().join("proof.json");
    let output = StdCommand::new(install.join(binary_name))
        .args([
            "proof-github",
            evidence.to_str().unwrap(),
            "--consent=github-device-read-only",
        ])
        .env("LOCALAPPDATA", &local_data)
        .env("ELEGY_GITHUB_CLIENT_ID", "dedicated-device-flow-client")
        .env_remove("ELEGY_ACCOUNTS_PROOF_ROOT")
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("ELEGY_ACCOUNTS_PROOF_ROOT"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!evidence.exists());
}

#[test]
fn packaged_live_proof_never_falls_back_to_github_cli_credentials() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("elegy-accounts.zip");
    let binary_source = PathBuf::from(env!("CARGO_BIN_EXE_elegy-accounts"));
    let binary_name = format!("bin/elegy-accounts{}", std::env::consts::EXE_SUFFIX);
    pack_plugin_v3_with_binary(
        &repo_root.join("plugins/accounts"),
        &archive,
        Some(PluginArchiveBinary {
            source_path: &binary_source,
            archive_path: binary_name.clone(),
        }),
    )
    .unwrap();
    let install = temp.path().join("installed");
    fs::create_dir_all(&install).unwrap();
    zip::ZipArchive::new(File::open(&archive).unwrap())
        .unwrap()
        .extract(&install)
        .unwrap();
    fs::create_dir_all(install.join(".elegy-plugin")).unwrap();
    fs::rename(
        install.join("plugin.json"),
        install.join(".elegy-plugin/plugin.json"),
    )
    .unwrap();
    fs::write(
        install.join("install-receipt.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion":"elegy-installer/v1",
            "name":"elegy-accounts",
            "version":"0.1.0",
            "installedAt":"2026-07-31T00:00:00Z",
            "source":archive,
            "installDir":install,
            "files":[
                ".elegy-plugin/plugin.json",
                "capability-catalog.json",
                ".mcp.json",
                "readiness.json",
                "providers/github.json",
                binary_name
            ]
        }))
        .unwrap(),
    )
    .unwrap();

    let proof_root = temp.path().join("isolated-local-data");
    fs::create_dir_all(&proof_root).unwrap();
    let port_probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let proof_port = port_probe.local_addr().unwrap().port().to_string();
    drop(port_probe);
    let proof_pipe = format!(
        r"\\.\pipe\elegy-accounts-live-proof-no-cli-test-{}",
        std::process::id()
    );
    let evidence = temp.path().join("proof.json");
    let installed_binary = install.join(binary_name);
    let missing_marker = StdCommand::new(&installed_binary)
        .args([
            "proof-github",
            evidence.to_str().unwrap(),
            "--consent=github-device-read-only",
        ])
        .env("LOCALAPPDATA", &proof_root)
        .env("ELEGY_ACCOUNTS_PROOF_ROOT", &proof_root)
        .env("ELEGY_GITHUB_CLIENT_ID", "dedicated-device-flow-client")
        .env("ELEGY_ACCOUNT_CENTER_PORT", &proof_port)
        .env("ELEGY_ACCOUNTS_PIPE_NAME", &proof_pipe)
        .env("ELEGY_LIVE_PROOF_TIMEOUT_SECONDS", "0")
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(!missing_marker.status.success());
    let stderr = String::from_utf8_lossy(&missing_marker.stderr);
    assert!(stderr.contains("live-proof root marker"), "{stderr}");
    assert!(!stderr.contains("GitHub CLI"), "{stderr}");
    assert!(!evidence.exists());

    fs::write(
        proof_root.join(".elegy-accounts-live-proof.json"),
        r#"{"schemaVersion":"elegy-accounts-live-proof-root/v1"}"#,
    )
    .unwrap();
    let override_output = StdCommand::new(&installed_binary)
        .args([
            "proof-github",
            evidence.to_str().unwrap(),
            "--consent=github-device-read-only",
        ])
        .env("LOCALAPPDATA", &proof_root)
        .env("ELEGY_ACCOUNTS_PROOF_ROOT", &proof_root)
        .env("ELEGY_GITHUB_CLIENT_ID", "dedicated-device-flow-client")
        .env("ELEGY_ACCOUNT_CENTER_PORT", &proof_port)
        .env("ELEGY_ACCOUNTS_PIPE_NAME", &proof_pipe)
        .env("ELEGY_ACCOUNTS_PROVIDER_DIR", temp.path())
        .output()
        .unwrap();
    assert!(!override_output.status.success());
    assert!(
        String::from_utf8_lossy(&override_output.stderr)
            .contains("ELEGY_ACCOUNTS_PROVIDER_DIR must be unset"),
        "{}",
        String::from_utf8_lossy(&override_output.stderr)
    );

    let output = StdCommand::new(&installed_binary)
        .args([
            "proof-github",
            evidence.to_str().unwrap(),
            "--consent=github-device-read-only",
        ])
        .env("LOCALAPPDATA", &proof_root)
        .env("ELEGY_ACCOUNTS_PROOF_ROOT", &proof_root)
        .env("ELEGY_GITHUB_CLIENT_ID", "dedicated-device-flow-client")
        .env("ELEGY_ACCOUNT_CENTER_PORT", &proof_port)
        .env("ELEGY_ACCOUNTS_PIPE_NAME", &proof_pipe)
        .env("ELEGY_LIVE_PROOF_TIMEOUT_SECONDS", "0")
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("complete GitHub Device Flow in Account Center"),
        "{stderr}"
    );
    assert!(!stderr.contains("GitHub CLI"), "{stderr}");
    assert!(!evidence.exists());
    assert!(
        !proof_root.join("Elegy/Accounts").exists(),
        "failed proofs must remove the disposable vault directory"
    );
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
