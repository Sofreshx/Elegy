use axum::{
    Json, Router,
    body::Bytes,
    http::HeaderMap,
    routing::{get, post},
};
use elegy_accountd::{
    AuthenticatedRequest, BrokerStore, DpapiProtector, NewAccessRequest, OAuthCredential,
    ProviderCatalog, Vault,
};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[tokio::test]
#[cfg(windows)]
async fn broker_refreshes_an_expired_oauth_credential_before_execution() {
    let refresh_calls = Arc::new(AtomicUsize::new(0));
    let observed_refresh_calls = refresh_calls.clone();
    let app = Router::new()
        .route(
            "/token",
            post(move |body: Bytes| {
                let observed_refresh_calls = observed_refresh_calls.clone();
                async move {
                    let body = String::from_utf8(body.to_vec()).unwrap();
                    assert!(body.contains("grant_type=refresh_token"));
                    assert!(body.contains("client_id=fixture-client"));
                    assert!(body.contains("refresh_token=refresh-original"));
                    observed_refresh_calls.fetch_add(1, Ordering::SeqCst);
                    Json(json!({
                        "access_token":"access-rotated",
                        "refresh_token":"refresh-rotated",
                        "scope":"profile.read",
                        "expires_in":3600
                    }))
                }
            }),
        )
        .route(
            "/profile",
            get(|headers: HeaderMap| async move {
                assert_eq!(
                    headers.get("authorization").unwrap(),
                    "Bearer access-rotated"
                );
                Json(json!({"identity":"safe-result"}))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let manifest = format!(
        r#"{{
          "schema_version":"elegy-account-provider/v3",
          "id":"refreshable",
          "display_name":"Refreshable",
          "version":"3.0.0",
          "publisher":"test",
          "browser_origins":["{base}"],
          "auth_profiles":[{{
            "id":"oauth",
            "method":"oauth_pkce",
            "audience":"{base}",
            "authorization_url":"{base}/authorize",
            "token_url":"{base}/token",
            "identity":{{"url":"{base}/profile","selectors":["/identity"]}},
            "client":{{"mode":"public","client_id":"fixture-client"}},
            "scopes":["profile.read"],
            "oauth_lifecycle":{{
              "refresh_url":"{base}/token",
              "revocation_url":"{base}/revoke"
            }}
          }}],
          "operations":{{
            "profile.read":{{
              "description":"Read profile.",
              "risk":"read",
              "scopes":["profile.read"],
              "input_schema":{{"type":"object","additionalProperties":false}},
              "result_schema":{{"type":"object"}},
              "executor":{{"kind":"http","profile":"oauth","method":"GET","path":"/profile"}}
            }}
          }}
        }}"#
    );
    let catalog = ProviderCatalog::from_json_documents([manifest.as_str()]).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let broker = BrokerStore::new(
        Vault::open(
            directory.path().join("accounts.sqlite"),
            Arc::new(DpapiProtector),
        )
        .unwrap(),
    );
    let credential = OAuthCredential {
        version: "elegy-oauth-credential/v1".into(),
        access_token: "access-expired".into(),
        refresh_token: "refresh-original".into(),
        scopes: vec!["profile.read".into()],
        expires_at: "2020-01-01T00:00:00Z".into(),
    };
    let secret = credential.to_secret().unwrap();
    let account = broker
        .vault()
        .store_account("refreshable", "owner", "oauth_pkce", secret.as_bytes())
        .unwrap();
    let request = broker
        .request_access(NewAccessRequest {
            account_id: account.id.clone(),
            client_id: "fixture-tool".into(),
            purpose: "read profile".into(),
            operations: vec!["profile.read".into()],
            duration_minutes: 5,
        })
        .unwrap();
    let grant = broker.approve_access(&request.id).unwrap();
    let lease = broker.issue_lease(&grant.id, 5).unwrap();

    let response = broker
        .execute_authenticated(
            &reqwest::Client::new(),
            &catalog,
            AuthenticatedRequest {
                lease: &lease.token,
                client_id: "fixture-tool",
                purpose: "read profile",
                provider: "refreshable",
                operation: "profile.read",
                method: "GET",
                url: &format!("{base}/profile"),
                body: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(response.body["identity"], "safe-result");
    assert_eq!(refresh_calls.load(Ordering::SeqCst), 1);
    let stored = broker.vault().load_secret(&account.id).unwrap();
    let stored = OAuthCredential::from_secret(std::str::from_utf8(&stored).unwrap()).unwrap();
    assert_eq!(stored.access_token, "access-rotated");
    assert_eq!(stored.refresh_token, "refresh-rotated");
}

#[tokio::test]
#[cfg(windows)]
async fn broker_injects_auth_for_an_approved_operation_without_returning_the_secret() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let app = Router::new().route(
        "/profile",
        get(move |headers: HeaderMap| {
            let observed = observed.clone();
            async move {
                assert_eq!(
                    headers.get("authorization").unwrap(),
                    "Bearer proxy-secret-canary"
                );
                observed.fetch_add(1, Ordering::SeqCst);
                Json(json!({"identity":"safe-result"}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let manifest = format!(
        r#"{{
      "schema_version":"elegy-account-provider/v1","id":"synthetic","display_name":"Synthetic","version":"1.0.0","publisher":"test",
      "browser_origins":["{base}"],
      "auth_profiles":[{{"id":"token","method":"api_token","audience":"{base}","identity":{{"url":"{base}/profile","selectors":["/identity"]}},"client":{{"mode":"user_provided"}},"scopes":["profile.read"],"credential_header":"authorization"}}],
      "operations":{{"profile.read":["profile.read"]}}
    }}"#
    );
    let catalog = ProviderCatalog::from_json_documents([manifest.as_str()]).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let broker = BrokerStore::new(
        Vault::open(
            directory.path().join("accounts.sqlite"),
            Arc::new(DpapiProtector),
        )
        .unwrap(),
    );
    let account = broker
        .vault()
        .store_account("synthetic", "owner", "api_token", b"proxy-secret-canary")
        .unwrap();
    let request = broker
        .request_access(NewAccessRequest {
            account_id: account.id,
            client_id: "fixture-tool".into(),
            purpose: "read profile".into(),
            operations: vec!["profile.read".into()],
            duration_minutes: 5,
        })
        .unwrap();
    let grant = broker.approve_access(&request.id).unwrap();
    let lease = broker.issue_single_use_lease(&grant.id, 5).unwrap();

    let response = broker
        .execute_authenticated(
            &reqwest::Client::new(),
            &catalog,
            AuthenticatedRequest {
                lease: &lease.token,
                client_id: "fixture-tool",
                purpose: "read profile",
                provider: "synthetic",
                operation: "profile.read",
                method: "GET",
                url: &format!("{base}/profile"),
                body: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body["identity"], "safe-result");
    assert!(!response.body.to_string().contains("proxy-secret-canary"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
#[cfg(windows)]
async fn broker_rejects_a_destination_outside_the_provider_audience() {
    let manifest = r#"{"schema_version":"elegy-account-provider/v1","id":"synthetic","display_name":"Synthetic","version":"1.0.0","publisher":"test","browser_origins":["https://accounts.example.test"],"auth_profiles":[{"id":"token","method":"api_token","audience":"https://api.example.test","identity":{"url":"https://api.example.test/me","selectors":["/id"]},"client":{"mode":"user_provided"},"scopes":["profile.read"]}],"operations":{"profile.read":["profile.read"]}}"#;
    let catalog = ProviderCatalog::from_json_documents([manifest]).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let broker = BrokerStore::new(
        Vault::open(
            directory.path().join("accounts.sqlite"),
            Arc::new(DpapiProtector),
        )
        .unwrap(),
    );
    let account = broker
        .vault()
        .store_account("synthetic", "owner", "api_token", b"secret")
        .unwrap();
    let request = broker
        .request_access(NewAccessRequest {
            account_id: account.id,
            client_id: "fixture-tool".into(),
            purpose: "read profile".into(),
            operations: vec!["profile.read".into()],
            duration_minutes: 5,
        })
        .unwrap();
    let grant = broker.approve_access(&request.id).unwrap();
    let lease = broker.issue_lease(&grant.id, 5).unwrap();
    let result = broker
        .execute_authenticated(
            &reqwest::Client::new(),
            &catalog,
            AuthenticatedRequest {
                lease: &lease.token,
                client_id: "fixture-tool",
                purpose: "read profile",
                provider: "synthetic",
                operation: "profile.read",
                method: "GET",
                url: "https://evil.example/steal",
                body: None,
            },
        )
        .await;
    assert!(result.unwrap_err().to_string().contains("destination"));
}

#[tokio::test]
#[cfg(windows)]
async fn broker_injects_structured_basic_credentials_without_exposing_the_envelope() {
    let app = Router::new().route(
        "/mail",
        get(|headers: HeaderMap| async move {
            assert_eq!(
                headers.get("authorization").unwrap(),
                "Basic dXNlcjphcHAtcGFzcw=="
            );
            Json(json!({"messages":1}))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let manifest = format!(
        r#"{{"schema_version":"elegy-account-provider/v1","id":"mail","display_name":"Mail","version":"1.0.0","publisher":"test","browser_origins":["{base}"],"auth_profiles":[{{"id":"basic","method":"http_basic","audience":"{base}","identity":{{"url":"{base}/mail","selectors":["/messages"]}},"client":{{"mode":"user_provided"}},"scopes":[]}}],"operations":{{"mail.read":[]}}}}"#
    );
    let catalog = ProviderCatalog::from_json_documents([manifest.as_str()]).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let broker = BrokerStore::new(
        Vault::open(
            directory.path().join("accounts.sqlite"),
            Arc::new(DpapiProtector),
        )
        .unwrap(),
    );
    let envelope = br#"{"version":"elegy-credential/v1","kind":"http_basic","fields":{"username":"user","password":"app-pass"}}"#;
    let account = broker
        .vault()
        .store_account("mail", "user", "http_basic", envelope)
        .unwrap();
    let request = broker
        .request_access(NewAccessRequest {
            account_id: account.id,
            client_id: "mail-tool".into(),
            purpose: "read mail".into(),
            operations: vec!["mail.read".into()],
            duration_minutes: 5,
        })
        .unwrap();
    let grant = broker.approve_access(&request.id).unwrap();
    let lease = broker.issue_lease(&grant.id, 5).unwrap();
    let response = broker
        .execute_authenticated(
            &reqwest::Client::new(),
            &catalog,
            AuthenticatedRequest {
                lease: &lease.token,
                client_id: "mail-tool",
                purpose: "read mail",
                provider: "mail",
                operation: "mail.read",
                method: "GET",
                url: &format!("{base}/mail"),
                body: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(response.body["messages"], 1);
}

#[tokio::test]
#[cfg(windows)]
async fn broker_exchanges_client_credentials_and_only_injects_the_short_lived_token() {
    let app = Router::new()
        .route("/token", post(|body: Bytes| async move {
            let body = String::from_utf8(body.to_vec()).unwrap();
            assert!(body.contains("grant_type=client_credentials"));
            assert!(body.contains("client_id=fixture-client"));
            assert!(body.contains("client_secret=fixture-secret"));
            Json(json!({"access_token":"ephemeral-access-token","token_type":"Bearer","expires_in":300}))
        }))
        .route("/resource", get(|headers: HeaderMap| async move {
            assert_eq!(headers.get("authorization").unwrap(), "Bearer ephemeral-access-token");
            Json(json!({"result":"safe"}))
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let manifest = format!(
        r#"{{"schema_version":"elegy-account-provider/v1","id":"machine-api","display_name":"Machine API","version":"1.0.0","publisher":"test","browser_origins":["{base}"],"auth_profiles":[{{"id":"client","method":"client_credentials","audience":"{base}","token_url":"{base}/token","identity":{{"url":"{base}/resource","selectors":["/result"]}},"client":{{"mode":"user_provided"}},"scopes":["resource.read"]}}],"operations":{{"resource.read":["resource.read"]}}}}"#
    );
    let catalog = ProviderCatalog::from_json_documents([manifest.as_str()]).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let broker = BrokerStore::new(
        Vault::open(
            directory.path().join("accounts.sqlite"),
            Arc::new(DpapiProtector),
        )
        .unwrap(),
    );
    let envelope = br#"{"version":"elegy-credential/v1","kind":"client_credentials","fields":{"client_id":"fixture-client","client_secret":"fixture-secret"}}"#;
    let account = broker
        .vault()
        .store_account(
            "machine-api",
            "fixture-client",
            "client_credentials",
            envelope,
        )
        .unwrap();
    let request = broker
        .request_access(NewAccessRequest {
            account_id: account.id,
            client_id: "machine-tool".into(),
            purpose: "read resource".into(),
            operations: vec!["resource.read".into()],
            duration_minutes: 5,
        })
        .unwrap();
    let grant = broker.approve_access(&request.id).unwrap();
    let lease = broker.issue_single_use_lease(&grant.id, 5).unwrap();
    let response = broker
        .execute_authenticated(
            &reqwest::Client::new(),
            &catalog,
            AuthenticatedRequest {
                lease: &lease.token,
                client_id: "machine-tool",
                purpose: "read resource",
                provider: "machine-api",
                operation: "resource.read",
                method: "GET",
                url: &format!("{base}/resource"),
                body: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(response.body["result"], "safe");
    assert!(!response.body.to_string().contains("fixture-secret"));
    assert!(!response.body.to_string().contains("ephemeral-access-token"));
}
