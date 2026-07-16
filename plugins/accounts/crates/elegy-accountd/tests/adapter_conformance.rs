use axum::{
    Json, Router,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use elegy_accountd::{OAuthAdapterConfig, exchange_and_verify, verify_cloudflare_token};
use serde_json::{Value, json};

#[tokio::test]
async fn oauth_exchange_primitive_uses_pkce_and_verifies_identity() {
    let app = Router::new()
        .route(
            "/token",
            post(|headers: HeaderMap, body: String| async move {
                assert_eq!(headers.get("accept").unwrap(), "application/json");
                assert!(body.contains("code_verifier=verifier-canary"));
                Json(json!({"access_token":"SYNTHETIC_SECRET_CANARY"}))
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
            identity_url: format!("http://{address}/identity"),
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
        assert_eq!(verified.secret.as_str(), "SYNTHETIC_SECRET_CANARY");
    }
}

#[tokio::test]
async fn cloudflare_scoped_token_must_be_active_before_storage() {
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

    let verified = verify_cloudflare_token(
        &reqwest::Client::new(),
        &format!("http://{address}/verify"),
        "scoped-canary",
    )
    .await
    .unwrap();
    assert_eq!(verified.provider, "cloudflare");
    assert_eq!(verified.identity, "token:verified-token-id");
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
        identity_url: format!("http://{address}/identity"),
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
