use axum::{
    Json, Router,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use elegy_accountd::{
    AuthMethod, AuthProfile, ClientRegistration, ClientRegistrationMode, IdentitySpec,
    OAuthAdapterConfig, OAuthCredential, OAuthLifecycle, TokenAdapterConfig, exchange_and_verify,
    refresh_oauth_credential, revoke_oauth_credential, verify_credentials, verify_token,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[tokio::test]
async fn oauth_exchange_primitive_uses_pkce_and_verifies_identity() {
    let app = Router::new()
        .route(
            "/token",
            post(|headers: HeaderMap, body: String| async move {
                assert_eq!(headers.get("accept").unwrap(), "application/json");
                assert!(body.contains("code_verifier=verifier-canary"));
                Json(json!({
                    "access_token":"SYNTHETIC_SECRET_CANARY",
                    "refresh_token":"SYNTHETIC_REFRESH_CANARY",
                    "scope":"profile.read",
                    "expires_in":3600
                }))
            }),
        )
        .route(
            "/identity",
            get(|headers: HeaderMap| async move {
                assert_eq!(
                    headers.get("authorization").unwrap(),
                    "Bearer SYNTHETIC_SECRET_CANARY"
                );
                Json(json!({"email":"verified@example.test"}))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = reqwest::Client::new();

    for provider in ["synthetic-oauth"] {
        let config = OAuthAdapterConfig {
            provider: provider.into(),
            client_id: "local-test-client".into(),
            token_url: format!("http://{address}/token"),
            identity: IdentitySpec {
                url: format!("http://{address}/identity"),
                selectors: vec!["/email".into()],
                required: BTreeMap::new(),
            },
            required_scopes: vec!["profile.read".into()],
        };
        let verified = exchange_and_verify(
            &client,
            &config,
            "authorization-code",
            "verifier-canary",
            "http://127.0.0.1/callback",
        )
        .await
        .unwrap();
        assert_eq!(verified.provider, provider);
        assert_eq!(verified.identity, "verified@example.test");
        let stored = elegy_accountd::OAuthCredential::from_secret(verified.secret.as_str())
            .expect("OAuth envelope");
        assert_eq!(stored.access_token, "SYNTHETIC_SECRET_CANARY");
        assert_eq!(stored.refresh_token, "SYNTHETIC_REFRESH_CANARY");
    }
}

#[tokio::test]
async fn declarative_token_profile_must_satisfy_identity_assertions_before_storage() {
    let app = Router::new().route(
        "/verify",
        get(|headers: HeaderMap| async move {
            assert_eq!(
                headers.get("authorization").unwrap(),
                "Bearer scoped-canary"
            );
            Json(json!({"success":true,"result":{"id":"verified-token-id","status":"active"}}))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let verified = verify_token(
        &reqwest::Client::new(),
        &TokenAdapterConfig {
            provider: "synthetic-edge".into(),
            identity: IdentitySpec {
                url: format!("http://{address}/verify"),
                selectors: vec!["/result/id".into()],
                required: BTreeMap::from([
                    ("/success".into(), json!(true)),
                    ("/result/status".into(), json!("active")),
                ]),
            },
            header: "authorization".into(),
            prefix: "Bearer ".into(),
        },
        "scoped-canary",
    )
    .await
    .unwrap();
    assert_eq!(verified.provider, "synthetic-edge");
    assert_eq!(verified.identity, "verified-token-id");
    assert_eq!(verified.secret.as_str(), "scoped-canary");
}

#[tokio::test]
async fn adapter_fails_closed_on_provider_rejection() {
    let app = Router::new().route(
        "/token",
        post(|| async {
            (
                StatusCode::UNAUTHORIZED,
                Json::<Value>(json!({"error":"invalid_grant"})),
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let config = OAuthAdapterConfig {
        provider: "generic".into(),
        client_id: "test".into(),
        token_url: format!("http://{address}/token"),
        identity: IdentitySpec {
            url: format!("http://{address}/identity"),
            selectors: vec!["/id".into()],
            required: BTreeMap::new(),
        },
        required_scopes: vec![],
    };
    assert!(
        exchange_and_verify(
            &reqwest::Client::new(),
            &config,
            "bad",
            "verifier",
            "http://127.0.0.1/callback"
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn refresh_preserves_an_unrotated_refresh_token_and_rejects_reduced_scopes() {
    let app = Router::new()
        .route(
            "/refresh",
            post(|body: String| async move {
                assert!(body.contains("refresh_token=refresh-original"));
                Json(json!({
                    "access_token":"access-rotated",
                    "scope":"profile.read mail.read",
                    "expires_in":3600
                }))
            }),
        )
        .route(
            "/reduced",
            post(|| async {
                Json(json!({
                    "access_token":"access-reduced",
                    "scope":"profile.read",
                    "expires_in":3600
                }))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let config = OAuthAdapterConfig {
        provider: "synthetic".into(),
        client_id: "public-client".into(),
        token_url: format!("http://{address}/token"),
        identity: IdentitySpec {
            url: format!("http://{address}/identity"),
            selectors: vec!["/id".into()],
            required: BTreeMap::new(),
        },
        required_scopes: vec!["profile.read".into(), "mail.read".into()],
    };
    let current = OAuthCredential {
        version: "elegy-oauth-credential/v1".into(),
        access_token: "access-original".into(),
        refresh_token: "refresh-original".into(),
        scopes: vec!["profile.read".into(), "mail.read".into()],
        expires_at: "2026-01-01T00:00:00Z".into(),
    };
    let lifecycle = OAuthLifecycle {
        refresh_url: format!("http://{address}/refresh"),
        revocation_url: format!("http://{address}/revoke"),
        revocation_token_type_hint: Some("refresh_token".into()),
    };

    let refreshed =
        refresh_oauth_credential(&reqwest::Client::new(), &config, &lifecycle, &current)
            .await
            .expect("refresh");
    assert_eq!(refreshed.access_token, "access-rotated");
    assert_eq!(refreshed.refresh_token, "refresh-original");

    let reduced = OAuthLifecycle {
        refresh_url: format!("http://{address}/reduced"),
        ..lifecycle
    };
    assert!(matches!(
        refresh_oauth_credential(&reqwest::Client::new(), &config, &reduced, &current).await,
        Err(elegy_accountd::AdapterError::ReducedScopes)
    ));
}

#[tokio::test]
async fn provider_revocation_uses_the_declared_endpoint_and_reports_pending_cleanup() {
    let app = Router::new()
        .route(
            "/revoke",
            post(|body: String| async move {
                assert!(body.contains("token=refresh-secret"));
                assert!(body.contains("token_type_hint=refresh_token"));
                StatusCode::OK
            }),
        )
        .route(
            "/unavailable",
            post(|| async { StatusCode::SERVICE_UNAVAILABLE }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let credential = OAuthCredential {
        version: "elegy-oauth-credential/v1".into(),
        access_token: "access-secret".into(),
        refresh_token: "refresh-secret".into(),
        scopes: vec!["profile.read".into()],
        expires_at: "2026-01-01T00:00:00Z".into(),
    };
    let mut lifecycle = OAuthLifecycle {
        refresh_url: format!("http://{address}/refresh"),
        revocation_url: format!("http://{address}/revoke"),
        revocation_token_type_hint: Some("refresh_token".into()),
    };
    revoke_oauth_credential(&reqwest::Client::new(), &lifecycle, &credential)
        .await
        .expect("provider revocation");
    lifecycle.revocation_url = format!("http://{address}/unavailable");
    assert!(matches!(
        revoke_oauth_credential(&reqwest::Client::new(), &lifecycle, &credential).await,
        Err(elegy_accountd::AdapterError::RevocationPending)
    ));
}

#[tokio::test]
async fn common_basic_credentials_are_verified_and_serialized_as_an_encrypted_envelope_payload() {
    let app = Router::new().route(
        "/identity",
        get(|headers: HeaderMap| async move {
            assert_eq!(
                headers.get("authorization").unwrap(),
                "Basic dXNlcjphcHAtcGFzcw=="
            );
            Json(json!({"username":"verified-user"}))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let profile = AuthProfile {
        id: "app-password".into(),
        method: AuthMethod::HttpBasic,
        audience: format!("http://{address}"),
        issuer: None,
        authorization_url: None,
        authorization_parameters: BTreeMap::new(),
        token_url: None,
        device_authorization_url: None,
        identity: IdentitySpec {
            url: format!("http://{address}/identity"),
            selectors: vec!["/username".into()],
            required: BTreeMap::new(),
        },
        client: ClientRegistration {
            mode: ClientRegistrationMode::UserProvided,
            client_id: None,
            client_id_env: None,
        },
        scopes: vec![],
        credential_header: None,
        creation_url: None,
        credential_fields: vec![],
        oauth_lifecycle: None,
    };
    let fields = BTreeMap::from([
        ("username".into(), "user".into()),
        ("password".into(), "app-pass".into()),
    ]);
    let verified = verify_credentials(&reqwest::Client::new(), "synthetic-basic", &profile, fields)
        .await
        .unwrap();
    assert_eq!(verified.identity, "verified-user");
    let envelope: Value = serde_json::from_str(verified.secret.as_str()).unwrap();
    assert_eq!(envelope["version"], "elegy-credential/v1");
    assert_eq!(envelope["fields"]["username"], "user");
    assert_eq!(envelope["fields"]["password"], "app-pass");
}

#[tokio::test]
async fn client_credentials_are_verified_but_only_the_long_lived_inputs_are_stored() {
    let app = Router::new()
        .route(
            "/token",
            post(|body: String| async move {
                assert!(body.contains("client_id=fixture-client"));
                assert!(body.contains("client_secret=fixture-secret"));
                Json(json!({"access_token":"short-lived-token"}))
            }),
        )
        .route(
            "/identity",
            get(|headers: HeaderMap| async move {
                assert_eq!(
                    headers.get("authorization").unwrap(),
                    "Bearer short-lived-token"
                );
                Json(json!({"client":"fixture-client"}))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let profile = AuthProfile {
        id: "client".into(),
        method: AuthMethod::ClientCredentials,
        audience: format!("http://{address}"),
        issuer: None,
        authorization_url: None,
        authorization_parameters: BTreeMap::new(),
        token_url: Some(format!("http://{address}/token")),
        device_authorization_url: None,
        identity: IdentitySpec {
            url: format!("http://{address}/identity"),
            selectors: vec!["/client".into()],
            required: BTreeMap::new(),
        },
        client: ClientRegistration {
            mode: ClientRegistrationMode::UserProvided,
            client_id: None,
            client_id_env: None,
        },
        scopes: vec!["resource.read".into()],
        credential_header: None,
        creation_url: None,
        credential_fields: vec![],
        oauth_lifecycle: None,
    };
    let fields = BTreeMap::from([
        ("client_id".into(), "fixture-client".into()),
        ("client_secret".into(), "fixture-secret".into()),
    ]);
    let verified = verify_credentials(&reqwest::Client::new(), "machine-api", &profile, fields)
        .await
        .unwrap();
    assert_eq!(verified.identity, "fixture-client");
    assert!(!verified.secret.contains("short-lived-token"));
    assert!(verified.secret.contains("fixture-secret"));
}
