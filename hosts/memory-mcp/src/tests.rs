use std::sync::Arc;

use axum::http::StatusCode;
use elegy_memory_mcp::{
    config::ExternalOAuthConfig,
    memory_tools::{MemoryBinding, MemoryRepository},
    resource_auth::ExternalTokenValidator,
};
use jsonwebtoken::{encode, jwk::JwkSet, Algorithm, EncodingKey, Header};
use reqwest::{Client, Response};
use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use serde::Serialize;
use serde_json::json;
use tempfile::TempDir;
use tokio::{net::TcpListener, task::JoinHandle};
use url::Url;

use crate::{build_router, AppState, HttpAuth};

struct TestServer {
    _temp_dir: TempDir,
    base_url: String,
    task: JoinHandle<()>,
}

impl TestServer {
    async fn start(auth: HttpAuth) -> Self {
        let temp_dir = TempDir::new().expect("temp directory");
        let repository = Arc::new(
            MemoryRepository::new(&temp_dir.path().join("memory.db"), MemoryBinding::default())
                .expect("memory repository"),
        );
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("listener");
        let address = listener.local_addr().expect("listener address");
        let state = AppState {
            auth,
            public_url: Url::parse(&format!("http://{address}/")).expect("public URL"),
        };
        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                build_router(
                    state,
                    repository,
                    StreamableHttpServerConfig::default()
                        .with_sse_keep_alive(None)
                        .with_sse_retry(None),
                )
                .into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
            .expect("test server");
        });
        Self {
            _temp_dir: temp_dir,
            base_url: format!("http://{address}"),
            task,
        }
    }

    async fn post(&self, path: &str) -> Response {
        Client::new()
            .post(format!("{}{path}", self.base_url))
            .header("content-type", "application/json")
            .body("{}")
            .send()
            .await
            .expect("request")
    }

    async fn post_bearer(&self, path: &str, token: &str) -> Response {
        Client::new()
            .post(format!("{}{path}", self.base_url))
            .bearer_auth(token)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(
                r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"memory-auth-test","version":"1.0.0"}}}"#,
            )
            .send()
            .await
            .expect("request")
    }

    async fn get(&self, path: &str) -> Response {
        Client::new()
            .get(format!("{}{path}", self.base_url))
            .send()
            .await
            .expect("request")
    }
}

#[derive(Serialize)]
struct TestClaims<'a> {
    iss: &'a str,
    aud: &'a str,
    exp: usize,
    scope: &'a str,
}

fn valid_access_token() -> String {
    const PRIVATE_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgWTFfCGljY6aw3Hrt
kHmPRiazukxPLb6ilpRAewjW8nihRANCAATDskChT+Altkm9X7MI69T3IUmrQU0L
950IxEzvw/x5BMEINRMrXLBJhqzO9Bm+d6JbqA21YQmd1Kt4RzLJR1W+
-----END PRIVATE KEY-----"#;
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some("test-key".to_string());
    encode(
        &header,
        &TestClaims {
            iss: "https://identity.example.com/",
            aud: "https://memory.example.com/mcp",
            exp: (time::OffsetDateTime::now_utc().unix_timestamp() + 300) as usize,
            scope: "memory.read",
        },
        &EncodingKey::from_ec_pem(PRIVATE_KEY.as_bytes()).expect("test signing key"),
    )
    .expect("signed access token")
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn external_auth() -> HttpAuth {
    let jwks: JwkSet = serde_json::from_value(json!({
        "keys": [{
            "kty": "EC",
            "crv": "P-256",
            "x": "w7JAoU_gJbZJvV-zCOvU9yFJq0FNC_edCMRM78P8eQQ",
            "y": "wQg1EytcsEmGrM70Gb53oluoDbVhCZ3Uq3hHMslHVb4",
            "alg": "ES256",
            "kid": "test-key",
            "use": "sig"
        }]
    }))
    .expect("JWKS");
    HttpAuth::External(Arc::new(
        ExternalTokenValidator::from_jwks(
            ExternalOAuthConfig {
                issuer: Url::parse("https://identity.example.com").expect("issuer"),
                audience: "https://memory.example.com/mcp".to_string(),
                jwks_url: Url::parse("https://identity.example.com/jwks").expect("JWKS URL"),
                scopes: vec!["memory.read".to_string()],
            },
            jwks,
        )
        .expect("validator"),
    ))
}

#[tokio::test]
async fn local_mode_does_not_require_http_oauth() {
    let server = TestServer::start(HttpAuth::LocalNone).await;
    assert_ne!(server.post("/mcp").await.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        server
            .get("/.well-known/oauth-protected-resource")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn external_mode_challenges_missing_bearer_and_publishes_resource_metadata() {
    let server = TestServer::start(external_auth()).await;
    let response = server.post("/mcp").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response
        .headers()
        .get("www-authenticate")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("oauth-protected-resource")));

    let metadata = server.get("/.well-known/oauth-protected-resource").await;
    assert_eq!(metadata.status(), StatusCode::OK);
    let body = metadata
        .json::<serde_json::Value>()
        .await
        .expect("metadata");
    assert_eq!(
        body["authorization_servers"],
        json!(["https://identity.example.com/"])
    );
}

#[tokio::test]
async fn external_mode_accepts_a_valid_resource_token_at_the_mcp_boundary() {
    let server = TestServer::start(external_auth()).await;
    let response = server.post_bearer("/mcp", &valid_access_token()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.expect("initialize response body");
    assert!(
        body.contains(r#""serverInfo""#),
        "authenticated MCP initialize response did not contain server information: {body}"
    );
}

#[tokio::test]
async fn production_router_has_no_authorization_server_endpoints() {
    let server = TestServer::start(external_auth()).await;
    for path in [
        "/.well-known/oauth-authorization-server",
        "/oauth/register",
        "/oauth/authorize",
        "/oauth/token",
    ] {
        assert_eq!(server.get(path).await.status(), StatusCode::NOT_FOUND);
    }
}
