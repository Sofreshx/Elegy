use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use axum::{Json, Router, http::HeaderMap, routing::get};
use chrono::{Duration, Utc};
use elegy_accountd::{
    BrokerStore, DpapiProtector, ProviderCatalog, TypedExecutionOutcome, TypedExecutionRequest,
    Vault,
};
use serde_json::json;

#[tokio::test]
#[cfg(windows)]
async fn typed_read_requests_one_thirty_day_grant_then_reuses_it_without_exposing_a_lease() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let app = Router::new().route(
        "/profile",
        get(move |headers: HeaderMap| {
            let observed = observed.clone();
            async move {
                assert_eq!(
                    headers.get("authorization").expect("authorization header"),
                    "Bearer typed-secret-canary"
                );
                observed.fetch_add(1, Ordering::SeqCst);
                Json(json!({"identity":"safe-user","access_token":"must-redact"}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener");
    let base = format!(
        "http://{}",
        listener.local_addr().expect("listener address")
    );
    tokio::spawn(async move { axum::serve(listener, app).await.expect("test server") });

    let manifest = format!(
        r#"{{
          "schema_version":"elegy-account-provider/v2",
          "id":"synthetic",
          "display_name":"Synthetic",
          "version":"2.0.0",
          "publisher":"test",
          "browser_origins":["{base}"],
          "auth_profiles":[{{
            "id":"token",
            "method":"api_token",
            "audience":"{base}",
            "identity":{{"url":"{base}/profile","selectors":["/identity"]}},
            "client":{{"mode":"user_provided"}},
            "scopes":["profile.read"]
          }}],
          "operations":{{
            "profile.read":{{
              "description":"Read profile.",
              "risk":"read",
              "scopes":["profile.read"],
              "input_schema":{{"type":"object","additionalProperties":false}},
              "result_schema":{{"type":"object"}},
              "executor":{{"kind":"http","profile":"token","method":"GET","path":"/profile"}}
            }}
          }}
        }}"#
    );
    let catalog =
        ProviderCatalog::from_json_documents([manifest.as_str()]).expect("typed provider catalog");
    let directory = tempfile::tempdir().expect("temporary broker directory");
    let broker = BrokerStore::new(
        Vault::open(
            directory.path().join("accounts.sqlite"),
            Arc::new(DpapiProtector),
        )
        .expect("broker vault"),
    );
    let account = broker
        .vault()
        .store_account(
            "synthetic",
            "safe-user",
            "api_token",
            b"typed-secret-canary",
        )
        .expect("stored account");
    let request = TypedExecutionRequest {
        client_id: "codex-actions".into(),
        purpose_class: "synthetic.profile.read".into(),
        provider: "synthetic".into(),
        operation: "profile.read".into(),
        account_id: Some(account.id.clone()),
        arguments: json!({}),
    };

    let first = broker
        .execute_typed_operation(&reqwest::Client::new(), &catalog, request.clone())
        .await
        .expect("first execution outcome");
    let approval_request_id = match first {
        TypedExecutionOutcome::InteractionRequired {
            request_id,
            kind,
            duration_minutes,
        } => {
            assert_eq!(kind, "approve_read_access");
            assert_eq!(duration_minutes, 43_200);
            request_id
        }
        other => panic!("expected approval checkpoint, got {other:?}"),
    };
    broker
        .approve_access(&approval_request_id)
        .expect("approved read grant");

    for _ in 0..2 {
        let outcome = broker
            .execute_typed_operation(&reqwest::Client::new(), &catalog, request.clone())
            .await
            .expect("approved typed execution");
        match outcome {
            TypedExecutionOutcome::Completed {
                status,
                result,
                audit_id,
            } => {
                assert_eq!(status, 200);
                assert_eq!(result["identity"], "safe-user");
                assert_eq!(result["access_token"], "[REDACTED]");
                assert!(audit_id.starts_with("audit_"));
                let public = serde_json::to_string(&result).expect("serialized public result");
                assert!(!public.contains("typed-secret-canary"));
                assert!(!public.contains("ela_"));
            }
            other => panic!("expected completed operation, got {other:?}"),
        }
    }

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(broker.list_requests().expect("requests").len(), 1);
    let grant = broker.list_grants().expect("grants").pop().expect("grant");
    let expiry = chrono::DateTime::parse_from_rfc3339(&grant.expires_at)
        .expect("grant expiry")
        .with_timezone(&Utc);
    assert!(expiry > Utc::now() + Duration::days(29));
}

#[tokio::test]
#[cfg(windows)]
async fn typed_operation_rejects_missing_and_unsafe_path_arguments_before_network_access() {
    let manifest = r#"{
      "schema_version":"elegy-account-provider/v2",
      "id":"synthetic",
      "display_name":"Synthetic",
      "version":"2.0.0",
      "publisher":"test",
      "browser_origins":[],
      "auth_profiles":[{
        "id":"token",
        "method":"api_token",
        "audience":"https://api.example.test",
        "identity":{"url":"https://api.example.test/me","selectors":["/id"]},
        "client":{"mode":"user_provided"}
      }],
      "operations":{
        "dns.records.read":{
          "description":"Read records.",
          "risk":"read",
          "scopes":[],
          "input_schema":{
            "type":"object",
            "properties":{"zone_id":{"type":"string","pattern":"^[A-Za-z0-9_-]{1,128}$"}},
            "required":["zone_id"],
            "additionalProperties":false
          },
          "result_schema":{"type":"object"},
          "executor":{"kind":"http","profile":"token","method":"GET","path":"/zones/{zone_id}/records"}
        }
      }
    }"#;
    let catalog = ProviderCatalog::from_json_documents([manifest]).expect("typed catalog");
    let directory = tempfile::tempdir().expect("temporary broker directory");
    let broker = BrokerStore::new(
        Vault::open(
            directory.path().join("accounts.sqlite"),
            Arc::new(DpapiProtector),
        )
        .expect("broker vault"),
    );
    let account = broker
        .vault()
        .store_account("synthetic", "owner", "api_token", b"secret")
        .expect("stored account");

    for arguments in [json!({}), json!({"zone_id":"../../steal"})] {
        let error = broker
            .execute_typed_operation(
                &reqwest::Client::new(),
                &catalog,
                TypedExecutionRequest {
                    client_id: "codex-actions".into(),
                    purpose_class: "synthetic.dns.read".into(),
                    provider: "synthetic".into(),
                    operation: "dns.records.read".into(),
                    account_id: Some(account.id.clone()),
                    arguments,
                },
            )
            .await
            .expect_err("invalid arguments");
        assert_eq!(error.code(), "invalid_operation_arguments");
    }
    assert!(broker.list_requests().expect("requests").is_empty());
}

#[tokio::test]
#[cfg(windows)]
async fn typed_execution_rejects_write_operations_until_exact_action_confirmation_exists() {
    let manifest = r#"{
      "schema_version":"elegy-account-provider/v2",
      "id":"synthetic",
      "display_name":"Synthetic",
      "version":"2.0.0",
      "publisher":"test",
      "browser_origins":[],
      "auth_profiles":[{
        "id":"token",
        "method":"api_token",
        "audience":"https://api.example.test",
        "identity":{"url":"https://api.example.test/me","selectors":["/id"]},
        "client":{"mode":"user_provided"}
      }],
      "operations":{
        "record.delete":{
          "description":"Delete a record.",
          "risk":"write",
          "scopes":[],
          "input_schema":{"type":"object","additionalProperties":false},
          "result_schema":{"type":"object"},
          "executor":{"kind":"http","profile":"token","method":"DELETE","path":"/record"}
        }
      }
    }"#;
    let catalog = ProviderCatalog::from_json_documents([manifest]).expect("typed catalog");
    let directory = tempfile::tempdir().expect("temporary broker directory");
    let broker = BrokerStore::new(
        Vault::open(
            directory.path().join("accounts.sqlite"),
            Arc::new(DpapiProtector),
        )
        .expect("broker vault"),
    );

    let error = broker
        .execute_typed_operation(
            &reqwest::Client::new(),
            &catalog,
            TypedExecutionRequest {
                client_id: "codex-actions".into(),
                purpose_class: "synthetic.record.delete".into(),
                provider: "synthetic".into(),
                operation: "record.delete".into(),
                account_id: None,
                arguments: json!({}),
            },
        )
        .await
        .expect_err("write operation must not execute");

    assert_eq!(error.code(), "invalid_operation");
    assert!(broker.list_requests().expect("requests").is_empty());
}
