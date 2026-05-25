use std::{
    collections::HashMap,
    io,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHasher,
};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use elegy_memory::{
    Memory, MemoryScope, MemoryState, MemoryStore, MemoryType, ProvenanceLevel, SensitivityLevel,
    SqliteMemoryStore,
};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use reqwest::{redirect::Policy, Client};
use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokio::{net::TcpListener, task::JoinHandle};
use tracing_subscriber::fmt::MakeWriter;
use uuid::Uuid;

use crate::oauth::{AccessTokenClaims, AppState, OAuthService, OAUTH_SCOPE};
use elegy_memory_mcp::{
    config::Config,
    memory_tools::{
        MemoryBinding, MemoryRepository, DEFAULT_NAMESPACE, SCOPE_OVERRIDE_ERROR_MESSAGE,
    },
};

const TEST_PASSWORD: &str = "correct horse battery staple";
const TEST_PUBLIC_URL: &str = "https://elegy-memory.holon.it.com";
const INITIALIZE_REQUEST: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"wu4-test-client","version":"1.0.0"}}}"#;
const FIXED_NAMESPACE: &str = DEFAULT_NAMESPACE;

#[derive(Clone)]
struct TestClock {
    now: Arc<Mutex<i64>>,
}

impl TestClock {
    fn new(initial: i64) -> Self {
        Self {
            now: Arc::new(Mutex::new(initial)),
        }
    }

    fn now_fn(&self) -> Arc<dyn Fn() -> i64 + Send + Sync> {
        let now = Arc::clone(&self.now);
        Arc::new(move || *now.lock().unwrap_or_else(|_| panic!("clock poisoned")))
    }

    fn advance(&self, seconds: i64) {
        let mut now = self.now.lock().unwrap_or_else(|_| panic!("clock poisoned"));
        *now += seconds;
    }

    fn unix_timestamp(&self) -> i64 {
        *self.now.lock().unwrap_or_else(|_| panic!("clock poisoned"))
    }
}

struct TestHarness {
    _temp_dir: TempDir,
    data_dir: PathBuf,
    db_path: PathBuf,
    client: Client,
    server: JoinHandle<()>,
    base_url: String,
    oauth: Arc<OAuthService>,
    clock: TestClock,
}

#[derive(Clone)]
struct SharedLogWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

struct SharedLogGuard {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl<'a> MakeWriter<'a> for SharedLogWriter {
    type Writer = SharedLogGuard;

    fn make_writer(&'a self) -> Self::Writer {
        SharedLogGuard {
            buffer: Arc::clone(&self.buffer),
        }
    }
}

impl io::Write for SharedLogGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut buffer = self
            .buffer
            .lock()
            .unwrap_or_else(|_| panic!("log buffer should not be poisoned"));
        buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn install_test_tracing() -> Arc<Mutex<Vec<u8>>> {
    static TEST_LOG_BUFFER: OnceLock<Arc<Mutex<Vec<u8>>>> = OnceLock::new();
    static TEST_TRACING_INIT: OnceLock<()> = OnceLock::new();

    let buffer = Arc::clone(TEST_LOG_BUFFER.get_or_init(|| Arc::new(Mutex::new(Vec::new()))));
    TEST_TRACING_INIT.get_or_init(|| {
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_ansi(false)
            .with_current_span(false)
            .with_span_list(false)
            .with_writer(SharedLogWriter {
                buffer: Arc::clone(&buffer),
            })
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);
    });
    buffer
}

fn clear_test_logs(buffer: &Arc<Mutex<Vec<u8>>>) {
    buffer
        .lock()
        .unwrap_or_else(|_| panic!("log buffer should not be poisoned"))
        .clear();
}

fn read_test_logs(buffer: &Arc<Mutex<Vec<u8>>>) -> String {
    let bytes = buffer
        .lock()
        .unwrap_or_else(|_| panic!("log buffer should not be poisoned"))
        .clone();
    String::from_utf8(bytes).unwrap_or_else(|_| panic!("captured logs should be valid UTF-8"))
}

impl TestHarness {
    async fn new() -> Self {
        let temp_dir = TempDir::new().unwrap_or_else(|_| panic!("tempdir should create"));
        let data_dir = temp_dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap_or_else(|_| panic!("data dir should create"));
        let db_path = temp_dir.path().join("memory.db");
        std::fs::write(&db_path, b"").unwrap_or_else(|_| panic!("db placeholder should write"));
        let clock = TestClock::new(1_735_689_600);
        let config = test_config(&data_dir, &db_path);
        let oauth = Arc::new(
            OAuthService::with_now(config, clock.now_fn())
                .unwrap_or_else(|_| panic!("oauth service should initialize")),
        );
        let memory_repository = Arc::new(
            MemoryRepository::new(&db_path, MemoryBinding::default())
                .unwrap_or_else(|_| panic!("memory repository should initialize")),
        );
        let state = AppState {
            oauth: Arc::clone(&oauth),
        };

        let transport_config = StreamableHttpServerConfig::default()
            .with_sse_keep_alive(None)
            .with_sse_retry(None);

        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap_or_else(|_| panic!("test listener should bind"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|_| panic!("listener should expose local address"));
        let server_oauth = Arc::clone(&oauth);
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                crate::build_router(state, memory_repository, server_oauth, transport_config)
                    .into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
            .unwrap_or_else(|_| panic!("test server should run"));
        });

        Self {
            _temp_dir: temp_dir,
            data_dir,
            db_path,
            client: Client::builder()
                .redirect(Policy::none())
                .build()
                .unwrap_or_else(|_| panic!("http client should build")),
            server,
            base_url: format!("http://{address}"),
            oauth,
            clock,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

impl Drop for TestHarness {
    fn drop(&mut self) {
        self.server.abort();
    }
}

fn derive_admin_password_verifier(value: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(value.as_bytes(), &salt)
        .unwrap_or_else(|_| panic!("test password verifier should generate"))
        .to_string()
}

fn test_config(data_dir: &std::path::Path, db_path: &std::path::Path) -> Config {
    Config {
        admin_password_verifier: derive_admin_password_verifier(TEST_PASSWORD),
        db_path: db_path.to_path_buf(),
        public_url: url::Url::parse("https://elegy-memory.holon.it.com")
            .unwrap_or_else(|_| panic!("test public url should parse")),
        port: 8765,
        log_content: false,
        data_dir: data_dir.to_path_buf(),
    }
}

fn parse_query_parameter(location: &str, name: &str) -> String {
    let url = url::Url::parse(location).unwrap_or_else(|_| panic!("location should parse"));
    url.query_pairs()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| panic!("query parameter should exist"))
}

fn pkce_pair(verifier: &str) -> (String, String) {
    let digest = Sha256::digest(verifier.as_bytes());
    (verifier.to_owned(), URL_SAFE_NO_PAD.encode(digest))
}

async fn register_client(client: &Client, base_url: &str, redirect_uri: &str) -> Value {
    client
        .post(format!("{base_url}/oauth/register"))
        .header("CF-Connecting-IP", "198.51.100.1")
        .json(&json!({
            "redirect_uris": [redirect_uri],
            "client_name": "WU4 test client",
            "token_endpoint_auth_method": "none"
        }))
        .send()
        .await
        .unwrap_or_else(|_| panic!("register request should succeed"))
        .json::<Value>()
        .await
        .unwrap_or_else(|_| panic!("register response should parse"))
}

async fn initialize_request(
    client: &Client,
    url: String,
    authorization: Option<&str>,
) -> reqwest::Response {
    let mut request = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(
            reqwest::header::ACCEPT,
            "application/json, text/event-stream",
        );
    if let Some(authorization) = authorization {
        request = request.header(reqwest::header::AUTHORIZATION, authorization);
    }

    request
        .body(INITIALIZE_REQUEST)
        .send()
        .await
        .unwrap_or_else(|_| panic!("initialize request should return"))
}

fn mcp_www_authenticate() -> String {
    format!(
        "Bearer realm=\"elegy-mcp\", resource_metadata=\"{TEST_PUBLIC_URL}/.well-known/oauth-protected-resource\""
    )
}

fn access_token_claims(scope: &str, issued_at: i64, expires_at: i64) -> AccessTokenClaims {
    access_token_claims_with_jti(scope, issued_at, expires_at, "wu5-test-jti")
}

fn access_token_claims_with_jti(
    scope: &str,
    issued_at: i64,
    expires_at: i64,
    jti: &str,
) -> AccessTokenClaims {
    AccessTokenClaims {
        iss: format!("{TEST_PUBLIC_URL}/"),
        aud: format!("{TEST_PUBLIC_URL}/mcp"),
        sub: "wu5-test-client".to_owned(),
        client_id: "wu5-test-client".to_owned(),
        scope: scope.to_owned(),
        iat: issued_at as usize,
        exp: expires_at as usize,
        jti: jti.to_owned(),
    }
}

fn first_json_sse_payload(body: &str) -> Value {
    body.lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .find_map(|payload| serde_json::from_str(payload).ok())
        .unwrap_or_else(|| panic!("response should contain a JSON SSE payload: {body}"))
}

#[tokio::test]
async fn missing_token_returns_401_with_www_authenticate() {
    let harness = TestHarness::new().await;

    let response = initialize_request(&harness.client, harness.url("/mcp"), None).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::WWW_AUTHENTICATE)
            .and_then(|value| value.to_str().ok()),
        Some(mcp_www_authenticate().as_str())
    );
}

async fn authorize_code(
    client: &Client,
    base_url: &str,
    client_id: &str,
    redirect_uri: &str,
    code_challenge: &str,
    password: &str,
) -> reqwest::Response {
    client
        .post(format!("{base_url}/oauth/authorize"))
        .header("CF-Connecting-IP", "198.51.100.1")
        .form(&[
            ("response_type", "code"),
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
            ("scope", OAUTH_SCOPE),
            ("state", "wu4-state"),
            ("code_challenge", code_challenge),
            ("code_challenge_method", "S256"),
            ("password", password),
        ])
        .send()
        .await
        .unwrap_or_else(|_| panic!("authorize request should return"))
}

#[tokio::test]
async fn metadata_endpoints_are_conformant() {
    let harness = TestHarness::new().await;

    let protected_resource = harness
        .client
        .get(harness.url("/.well-known/oauth-protected-resource"))
        .send()
        .await
        .unwrap_or_else(|_| panic!("protected resource metadata should return"))
        .json::<Value>()
        .await
        .unwrap_or_else(|_| panic!("protected resource metadata should parse"));
    assert_eq!(
        protected_resource["authorization_servers"],
        json!(["https://elegy-memory.holon.it.com/"])
    );
    assert_eq!(
        protected_resource["resource"],
        json!("https://elegy-memory.holon.it.com/mcp")
    );

    let authorization_server = harness
        .client
        .get(harness.url("/.well-known/oauth-authorization-server"))
        .send()
        .await
        .unwrap_or_else(|_| panic!("auth metadata should return"))
        .json::<Value>()
        .await
        .unwrap_or_else(|_| panic!("auth metadata should parse"));
    assert_eq!(
        authorization_server["token_endpoint_auth_methods_supported"],
        json!(["none"])
    );
    assert_eq!(
        authorization_server["grant_types_supported"],
        json!(["authorization_code", "refresh_token"])
    );
}

#[tokio::test]
async fn persistence_survives_restart_for_clients_and_refresh_tokens() {
    let harness = TestHarness::new().await;
    let redirect_uri = "https://claude.ai/callback";
    let client_registration =
        register_client(&harness.client, &harness.base_url, redirect_uri).await;
    let client_id = client_registration["client_id"]
        .as_str()
        .unwrap_or_else(|| panic!("client_id should exist"))
        .to_owned();
    let (_, code_challenge) = pkce_pair("persistence-verifier");
    let authorize_response = authorize_code(
        &harness.client,
        &harness.base_url,
        &client_id,
        redirect_uri,
        &code_challenge,
        TEST_PASSWORD,
    )
    .await;
    let location = authorize_response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_else(|| panic!("authorize redirect should be present"))
        .to_owned();
    let code = parse_query_parameter(&location, "code");

    let token_response = harness
        .client
        .post(harness.url("/oauth/token"))
        .header("CF-Connecting-IP", "198.51.100.1")
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri),
            ("code", code.as_str()),
            ("code_verifier", "persistence-verifier"),
        ])
        .send()
        .await
        .unwrap_or_else(|_| panic!("token request should return"))
        .json::<Value>()
        .await
        .unwrap_or_else(|_| panic!("token response should parse"));
    let refresh_token = token_response["refresh_token"]
        .as_str()
        .unwrap_or_else(|| panic!("refresh token should exist"))
        .to_owned();
    assert!(harness.data_dir.join("signing-key").exists());
    assert!(harness.data_dir.join("clients.json").exists());
    assert!(harness.data_dir.join("refresh-tokens.json").exists());

    let reloaded = Arc::new(
        OAuthService::with_now(
            test_config(&harness.data_dir, &harness.data_dir.join("memory.db")),
            harness.clock.now_fn(),
        )
        .unwrap_or_else(|_| panic!("reloaded oauth service should initialize")),
    );

    let refresh_response = reloaded
        .exchange_token(crate::oauth::TokenRequest {
            grant_type: "refresh_token".to_owned(),
            code: None,
            redirect_uri: None,
            client_id: Some(client_id),
            code_verifier: None,
            refresh_token: Some(refresh_token.clone()),
            scope: Some(OAUTH_SCOPE.to_owned()),
        })
        .unwrap_or_else(|_| panic!("refresh token should survive restart"));
    assert_eq!(refresh_response.scope, OAUTH_SCOPE);
    let rotated_error = reloaded
        .exchange_token(crate::oauth::TokenRequest {
            grant_type: "refresh_token".to_owned(),
            code: None,
            redirect_uri: None,
            client_id: Some(
                client_registration["client_id"]
                    .as_str()
                    .unwrap_or_else(|| panic!("client id should exist"))
                    .to_owned(),
            ),
            code_verifier: None,
            refresh_token: Some(refresh_token),
            scope: Some(OAUTH_SCOPE.to_owned()),
        })
        .expect_err("rotated refresh token should be single-use");
    assert_eq!(
        rotated_error.into_response().status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn full_code_flow_issues_hs256_tokens() {
    let harness = TestHarness::new().await;
    let redirect_uri = "https://claude.ai/callback";
    let registration = register_client(&harness.client, &harness.base_url, redirect_uri).await;
    let client_id = registration["client_id"]
        .as_str()
        .unwrap_or_else(|| panic!("client_id should exist"))
        .to_owned();
    let (code_verifier, code_challenge) = pkce_pair("correct-verifier");

    let consent_page = harness
        .client
        .get(harness.url("/oauth/authorize"))
        .header("CF-Connecting-IP", "198.51.100.1")
        .query(&[
            ("response_type", "code"),
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri),
            ("scope", OAUTH_SCOPE),
            ("state", "abc123"),
            ("code_challenge", code_challenge.as_str()),
            ("code_challenge_method", "S256"),
        ])
        .send()
        .await
        .unwrap_or_else(|_| panic!("consent page should return"));
    assert_eq!(consent_page.status(), StatusCode::OK);
    let consent_html = consent_page
        .text()
        .await
        .unwrap_or_else(|_| panic!("consent html should be readable"));
    assert!(consent_html.contains("Claude demande accès à ta mémoire"));
    assert!(consent_html.contains("value=\"abc123\""));

    let authorize_response = authorize_code(
        &harness.client,
        &harness.base_url,
        &client_id,
        redirect_uri,
        &code_challenge,
        TEST_PASSWORD,
    )
    .await;
    assert_eq!(authorize_response.status(), StatusCode::FOUND);
    let location = authorize_response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_else(|| panic!("redirect location should exist"))
        .to_owned();
    let code = parse_query_parameter(&location, "code");
    let state = parse_query_parameter(&location, "state");
    assert_eq!(state, "wu4-state");

    let token_response = harness
        .client
        .post(harness.url("/oauth/token"))
        .header("CF-Connecting-IP", "198.51.100.1")
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri),
            ("code", code.as_str()),
            ("code_verifier", code_verifier.as_str()),
        ])
        .send()
        .await
        .unwrap_or_else(|_| panic!("token request should return"));
    assert_eq!(token_response.status(), StatusCode::OK);
    let token_payload = token_response
        .json::<Value>()
        .await
        .unwrap_or_else(|_| panic!("token payload should parse"));
    assert_eq!(token_payload["token_type"], json!("Bearer"));
    assert_eq!(token_payload["expires_in"], json!(3600));
    assert_eq!(token_payload["scope"], json!(OAUTH_SCOPE));

    let claims = harness
        .oauth
        .decode_access_token(
            token_payload["access_token"]
                .as_str()
                .unwrap_or_else(|| panic!("access token should exist")),
        )
        .unwrap_or_else(|_| panic!("access token should decode"));
    assert_eq!(claims.scope, OAUTH_SCOPE);
    assert_eq!(
        claims.client_id,
        registration["client_id"]
            .as_str()
            .unwrap_or_else(|| panic!("client id should exist"))
    );
    assert_eq!(claims.exp - claims.iat, 3600);

    let access_token = token_payload["access_token"]
        .as_str()
        .unwrap_or_else(|| panic!("access token should exist"))
        .to_owned();
    let mcp_response = initialize_request(
        &harness.client,
        harness.url("/mcp"),
        Some(&format!("Bearer {access_token}")),
    )
    .await;
    assert_eq!(mcp_response.status(), StatusCode::OK);

    let session_id = mcp_response
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_else(|| panic!("session id should be present"))
        .to_owned();
    let body = mcp_response
        .text()
        .await
        .unwrap_or_else(|_| panic!("initialize body should be readable"));
    let payload = first_json_sse_payload(&body);
    assert_eq!(
        payload["result"]["serverInfo"]["name"],
        json!(env!("CARGO_PKG_NAME"))
    );

    let delete_response = harness
        .client
        .delete(harness.url("/mcp"))
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {access_token}"),
        )
        .header("mcp-session-id", session_id)
        .header(
            "mcp-protocol-version",
            payload["result"]["protocolVersion"]
                .as_str()
                .unwrap_or_else(|| panic!("protocol version should exist")),
        )
        .send()
        .await
        .unwrap_or_else(|_| panic!("session delete should succeed"));
    assert_eq!(delete_response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn nominal_end_to_end_flow_exercises_oauth_and_all_memory_tools() {
    let harness = TestHarness::new().await;
    let base_url =
        url::Url::parse(&harness.base_url).unwrap_or_else(|_| panic!("test base url should parse"));
    assert_eq!(base_url.host_str(), Some("127.0.0.1"));
    assert!(base_url.port().unwrap_or_default() != 0);
    assert!(harness.db_path.exists());
    assert!(harness.data_dir.exists());
    let session = oauth_authenticated_mcp_session(&harness).await;

    let initial_search = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":600,
            "method":"tools/call",
            "params":{
                "name":"memory_search",
                "arguments":{"query":"wu8-missing-term","limit":5}
            }
        }),
    )
    .await;
    assert_eq!(
        initial_search["result"]["structuredContent"]["count"],
        json!(0)
    );
    assert_eq!(
        initial_search["result"]["structuredContent"]["results"],
        json!([])
    );

    let stored = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":601,
            "method":"tools/call",
            "params":{
                "name":"memory_store",
                "arguments":{
                    "content":"WU8 nominal memory mentions lighthouse alpha.",
                    "memoryType":"fact",
                    "importance":0.8,
                    "tags":["wu8","nominal"]
                }
            }
        }),
    )
    .await;
    let stored_memory = &stored["result"]["structuredContent"]["memory"];
    let memory_id = stored_memory["id"]
        .as_str()
        .unwrap_or_else(|| panic!("stored memory id should exist"))
        .to_owned();
    assert_eq!(
        stored["result"]["structuredContent"]["action"],
        json!("added")
    );
    assert_eq!(
        stored_memory["content"],
        json!("WU8 nominal memory mentions lighthouse alpha.")
    );

    let recalled = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":602,
            "method":"tools/call",
            "params":{
                "name":"memory_recall",
                "arguments":{"id": memory_id.clone()}
            }
        }),
    )
    .await;
    assert_eq!(
        recalled["result"]["structuredContent"]["found"],
        json!(true)
    );
    assert_eq!(
        recalled["result"]["structuredContent"]["memory"]["id"],
        json!(memory_id.clone())
    );

    let listed = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":603,
            "method":"tools/call",
            "params":{
                "name":"memory_list",
                "arguments":{"limit":10}
            }
        }),
    )
    .await;
    let listed_ids = listed["result"]["structuredContent"]["memories"]
        .as_array()
        .unwrap_or_else(|| panic!("listed memories should be an array"))
        .iter()
        .map(|memory| memory["id"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(listed_ids.len(), 1);
    assert!(listed_ids.contains(&memory_id.as_str()));

    let stats = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":604,
            "method":"tools/call",
            "params":{
                "name":"memory_stats",
                "arguments":{}
            }
        }),
    )
    .await;
    let stats_content = &stats["result"]["structuredContent"];
    assert_eq!(stats_content["totalCount"], json!(1));
    assert_eq!(stats_content["activeCount"], json!(1));
    assert_eq!(stats_content["typeCounts"]["fact"], json!(1));

    let updated = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":605,
            "method":"tools/call",
            "params":{
                "name":"memory_update",
                "arguments":{
                    "id": memory_id.clone(),
                    "content":"WU8 nominal memory now mentions observatory beta.",
                    "reason":"end-to-end update"
                }
            }
        }),
    )
    .await;
    assert_eq!(
        updated["result"]["structuredContent"]["updated"],
        json!(true)
    );
    assert_eq!(
        updated["result"]["structuredContent"]["memory"]["content"],
        json!("WU8 nominal memory now mentions observatory beta.")
    );

    let corrected = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":606,
            "method":"tools/call",
            "params":{
                "name":"memory_correct",
                "arguments":{
                    "id": memory_id.clone(),
                    "content":"WU8 nominal memory finally mentions aurora gamma.",
                    "reason":"end-to-end correction"
                }
            }
        }),
    )
    .await;
    assert_eq!(
        corrected["result"]["structuredContent"]["correction"]["disposition"],
        json!("applied")
    );
    assert_eq!(
        corrected["result"]["structuredContent"]["memory"]["content"],
        json!("WU8 nominal memory finally mentions aurora gamma.")
    );

    let final_search = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":607,
            "method":"tools/call",
            "params":{
                "name":"memory_search",
                "arguments":{"query":"aurora gamma","limit":5}
            }
        }),
    )
    .await;
    assert_eq!(
        final_search["result"]["structuredContent"]["count"],
        json!(1)
    );
    assert_eq!(
        final_search["result"]["structuredContent"]["results"][0]["id"],
        json!(memory_id.clone())
    );

    let deleted = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":608,
            "method":"tools/call",
            "params":{
                "name":"memory_delete",
                "arguments":{"id": memory_id.clone()}
            }
        }),
    )
    .await;
    assert_eq!(
        deleted["result"]["structuredContent"]["deleted"],
        json!(true)
    );
    assert_eq!(
        deleted["result"]["structuredContent"]["id"],
        json!(memory_id.clone())
    );

    let final_recall = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":609,
            "method":"tools/call",
            "params":{
                "name":"memory_recall",
                "arguments":{"id": memory_id}
            }
        }),
    )
    .await;
    assert_eq!(
        final_recall["result"]["structuredContent"]["found"],
        json!(false)
    );
    assert!(final_recall["result"]["structuredContent"]["memory"].is_null());

    let delete_response = close_mcp_session(&harness, &session).await;
    assert_eq!(delete_response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn expired_token_returns_401_with_www_authenticate() {
    let harness = TestHarness::new().await;
    let now = harness.clock.unix_timestamp();
    let token = harness
        .oauth
        .sign_access_token_for_tests(&access_token_claims(OAUTH_SCOPE, now - 120, now - 1))
        .unwrap_or_else(|_| panic!("expired token should sign"));

    let response = initialize_request(
        &harness.client,
        harness.url("/mcp"),
        Some(&format!("Bearer {token}")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::WWW_AUTHENTICATE)
            .and_then(|value| value.to_str().ok()),
        Some(mcp_www_authenticate().as_str())
    );
}

#[tokio::test]
async fn invalid_signature_returns_401_with_www_authenticate() {
    let harness = TestHarness::new().await;
    let now = harness.clock.unix_timestamp();
    let token = jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        &access_token_claims(OAUTH_SCOPE, now, now + 3600),
        &EncodingKey::from_secret(b"wu5-invalid-signature-secret"),
    )
    .unwrap_or_else(|_| panic!("token with wrong secret should sign"));

    let response = initialize_request(
        &harness.client,
        harness.url("/mcp"),
        Some(&format!("Bearer {token}")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::WWW_AUTHENTICATE)
            .and_then(|value| value.to_str().ok()),
        Some(mcp_www_authenticate().as_str())
    );
}

#[tokio::test]
async fn wrong_scope_returns_401_with_www_authenticate() {
    let harness = TestHarness::new().await;
    let now = harness.clock.unix_timestamp();
    let token = harness
        .oauth
        .sign_access_token_for_tests(&access_token_claims("memory-read", now, now + 3600))
        .unwrap_or_else(|_| panic!("wrong-scope token should sign"));

    let response = initialize_request(
        &harness.client,
        harness.url("/mcp"),
        Some(&format!("Bearer {token}")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::WWW_AUTHENTICATE)
            .and_then(|value| value.to_str().ok()),
        Some(mcp_www_authenticate().as_str())
    );
}

#[tokio::test]
async fn wrong_pkce_verifier_returns_invalid_grant() {
    let harness = TestHarness::new().await;
    let redirect_uri = "https://claude.ai/callback";
    let registration = register_client(&harness.client, &harness.base_url, redirect_uri).await;
    let client_id = registration["client_id"]
        .as_str()
        .unwrap_or_else(|| panic!("client_id should exist"))
        .to_owned();
    let (_, code_challenge) = pkce_pair("expected-verifier");
    let authorize_response = authorize_code(
        &harness.client,
        &harness.base_url,
        &client_id,
        redirect_uri,
        &code_challenge,
        TEST_PASSWORD,
    )
    .await;
    let location = authorize_response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_else(|| panic!("redirect should exist"))
        .to_owned();
    let code = parse_query_parameter(&location, "code");

    let response = harness
        .client
        .post(harness.url("/oauth/token"))
        .header("CF-Connecting-IP", "198.51.100.1")
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri),
            ("code", code.as_str()),
            ("code_verifier", "wrong-verifier"),
        ])
        .send()
        .await
        .unwrap_or_else(|_| panic!("token error response should return"));
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response
        .json::<Value>()
        .await
        .unwrap_or_else(|_| panic!("error payload should parse"));
    assert_eq!(payload["error"], json!("invalid_grant"));
}

#[tokio::test]
async fn invalid_redirect_uri_is_rejected() {
    let harness = TestHarness::new().await;

    let response = harness
        .client
        .post(harness.url("/oauth/register"))
        .header("CF-Connecting-IP", "198.51.100.1")
        .json(&json!({
            "redirect_uris": ["https://example.com/callback"],
            "token_endpoint_auth_method": "none"
        }))
        .send()
        .await
        .unwrap_or_else(|_| panic!("register request should return"));
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn loopback_redirect_uri_is_accepted() {
    let harness = TestHarness::new().await;

    let response = harness
        .client
        .post(harness.url("/oauth/register"))
        .header("CF-Connecting-IP", "198.51.100.1")
        .json(&json!({
            "redirect_uris": ["http://127.0.0.1:45789/oauth/callback"],
            "token_endpoint_auth_method": "none"
        }))
        .send()
        .await
        .unwrap_or_else(|_| panic!("register request should return"));
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn expired_code_returns_invalid_grant() {
    let harness = TestHarness::new().await;
    let redirect_uri = "https://claude.ai/callback";
    let registration = register_client(&harness.client, &harness.base_url, redirect_uri).await;
    let client_id = registration["client_id"]
        .as_str()
        .unwrap_or_else(|| panic!("client_id should exist"))
        .to_owned();
    let (code_verifier, code_challenge) = pkce_pair("expiry-verifier");
    let authorize_response = authorize_code(
        &harness.client,
        &harness.base_url,
        &client_id,
        redirect_uri,
        &code_challenge,
        TEST_PASSWORD,
    )
    .await;
    let location = authorize_response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_else(|| panic!("redirect should exist"))
        .to_owned();
    let code = parse_query_parameter(&location, "code");
    harness.clock.advance(61);

    let response = harness
        .client
        .post(harness.url("/oauth/token"))
        .header("CF-Connecting-IP", "198.51.100.1")
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri),
            ("code", code.as_str()),
            ("code_verifier", code_verifier.as_str()),
        ])
        .send()
        .await
        .unwrap_or_else(|_| panic!("token request should return"));
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response
        .json::<Value>()
        .await
        .unwrap_or_else(|_| panic!("error payload should parse"));
    assert_eq!(payload["error"], json!("invalid_grant"));
}

#[tokio::test]
async fn unknown_client_after_restart_returns_invalid_client() {
    let harness = TestHarness::new().await;
    let redirect_uri = "https://claude.ai/callback";
    let registration = register_client(&harness.client, &harness.base_url, redirect_uri).await;
    let client_id = registration["client_id"]
        .as_str()
        .unwrap_or_else(|| panic!("client id should exist"))
        .to_owned();
    std::fs::write(harness.data_dir.join("clients.json"), "[]")
        .unwrap_or_else(|_| panic!("clients file should overwrite"));

    let reloaded = Arc::new(
        OAuthService::with_now(
            test_config(&harness.data_dir, &harness.data_dir.join("memory.db")),
            harness.clock.now_fn(),
        )
        .unwrap_or_else(|_| panic!("reloaded oauth service should initialize")),
    );

    let error = reloaded
        .exchange_token(crate::oauth::TokenRequest {
            grant_type: "refresh_token".to_owned(),
            code: None,
            redirect_uri: None,
            client_id: Some(client_id),
            code_verifier: None,
            refresh_token: Some("bogus".to_owned()),
            scope: Some(OAUTH_SCOPE.to_owned()),
        })
        .expect_err("unknown client should fail");
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rate_limit_returns_429_with_retry_after() {
    let harness = TestHarness::new().await;

    for _ in 0..10 {
        let response = harness
            .client
            .post(harness.url("/oauth/register"))
            .header("CF-Connecting-IP", "203.0.113.7")
            .json(&json!({
                "redirect_uris": ["https://claude.ai/callback"],
                "token_endpoint_auth_method": "none"
            }))
            .send()
            .await
            .unwrap_or_else(|_| panic!("register request should return"));
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let response = harness
        .client
        .post(harness.url("/oauth/register"))
        .header("CF-Connecting-IP", "203.0.113.7")
        .json(&json!({
            "redirect_uris": ["https://claude.ai/callback"],
            "token_endpoint_auth_method": "none"
        }))
        .send()
        .await
        .unwrap_or_else(|_| panic!("rate limited request should return"));
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .is_some());
}

struct McpSession {
    authorization: String,
    session_id: String,
    protocol_version: String,
}

#[derive(Debug)]
struct SeededMemories {
    visible_rust_id: String,
    visible_cloudflare_id: String,
    hidden_other_agent_id: String,
}

struct TestMemorySpec<'a> {
    content: &'a str,
    summary: Option<String>,
    state: MemoryState,
    memory_type: MemoryType,
    agent_id: Option<String>,
    tags: Vec<String>,
    importance_score: f32,
    embedding_stale: bool,
}

async fn open_mcp_session(harness: &TestHarness, authorization: String) -> McpSession {
    let initialize_response =
        initialize_request(&harness.client, harness.url("/mcp"), Some(&authorization)).await;
    assert_eq!(initialize_response.status(), StatusCode::OK);

    let session_id = initialize_response
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_else(|| panic!("session id should exist"))
        .to_owned();
    let initialize_payload = parse_mcp_response(initialize_response).await;
    let protocol_version = initialize_payload["result"]["protocolVersion"]
        .as_str()
        .unwrap_or_else(|| panic!("protocol version should exist"))
        .to_owned();

    let initialized_response = harness
        .client
        .post(harness.url("/mcp"))
        .header(reqwest::header::AUTHORIZATION, authorization.clone())
        .header("mcp-session-id", &session_id)
        .header("mcp-protocol-version", &protocol_version)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(
            reqwest::header::ACCEPT,
            "application/json, text/event-stream",
        )
        .body(r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#)
        .send()
        .await
        .unwrap_or_else(|_| panic!("initialized notification should return"));
    assert!(matches!(
        initialized_response.status(),
        StatusCode::ACCEPTED | StatusCode::NO_CONTENT | StatusCode::OK
    ));

    McpSession {
        authorization,
        session_id,
        protocol_version,
    }
}

async fn authenticated_mcp_session(harness: &TestHarness) -> McpSession {
    authenticated_mcp_session_with_jti(harness, "wu5-test-jti").await
}

async fn authenticated_mcp_session_with_jti(harness: &TestHarness, jti: &str) -> McpSession {
    let now = harness.clock.unix_timestamp();
    let token = harness
        .oauth
        .sign_access_token_for_tests(&access_token_claims_with_jti(
            OAUTH_SCOPE,
            now,
            now + 3600,
            jti,
        ))
        .unwrap_or_else(|_| panic!("access token should sign"));
    open_mcp_session(harness, format!("Bearer {token}")).await
}

async fn oauth_authenticated_mcp_session(harness: &TestHarness) -> McpSession {
    let redirect_uri = "https://claude.ai/callback";
    let registration = register_client(&harness.client, &harness.base_url, redirect_uri).await;
    let client_id = registration["client_id"]
        .as_str()
        .unwrap_or_else(|| panic!("client_id should exist"))
        .to_owned();
    let (code_verifier, code_challenge) = pkce_pair("wu8-end-to-end-verifier");

    let authorize_response = authorize_code(
        &harness.client,
        &harness.base_url,
        &client_id,
        redirect_uri,
        &code_challenge,
        TEST_PASSWORD,
    )
    .await;
    assert_eq!(authorize_response.status(), StatusCode::FOUND);
    let location = authorize_response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_else(|| panic!("authorize redirect should exist"))
        .to_owned();
    let code = parse_query_parameter(&location, "code");

    let token_response = harness
        .client
        .post(harness.url("/oauth/token"))
        .header("CF-Connecting-IP", "198.51.100.1")
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri),
            ("code", code.as_str()),
            ("code_verifier", code_verifier.as_str()),
        ])
        .send()
        .await
        .unwrap_or_else(|_| panic!("token request should return"));
    assert_eq!(token_response.status(), StatusCode::OK);
    let access_token = token_response
        .json::<Value>()
        .await
        .unwrap_or_else(|_| panic!("token payload should parse"))["access_token"]
        .as_str()
        .unwrap_or_else(|| panic!("access token should exist"))
        .to_owned();

    open_mcp_session(harness, format!("Bearer {access_token}")).await
}

async fn close_mcp_session(harness: &TestHarness, session: &McpSession) -> reqwest::Response {
    harness
        .client
        .delete(harness.url("/mcp"))
        .header(reqwest::header::AUTHORIZATION, &session.authorization)
        .header("mcp-session-id", &session.session_id)
        .header("mcp-protocol-version", &session.protocol_version)
        .send()
        .await
        .unwrap_or_else(|_| panic!("session delete should succeed"))
}

async fn mcp_json_request(harness: &TestHarness, session: &McpSession, body: Value) -> Value {
    let response = harness
        .client
        .post(harness.url("/mcp"))
        .header(reqwest::header::AUTHORIZATION, &session.authorization)
        .header("mcp-session-id", &session.session_id)
        .header("mcp-protocol-version", &session.protocol_version)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(
            reqwest::header::ACCEPT,
            "application/json, text/event-stream",
        )
        .body(body.to_string())
        .send()
        .await
        .unwrap_or_else(|_| panic!("mcp request should return"));
    assert_eq!(response.status(), StatusCode::OK);
    parse_mcp_response(response).await
}

async fn parse_mcp_response(response: reqwest::Response) -> Value {
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| panic!("mcp body should be readable"));
    if body.trim_start().starts_with("data: ") {
        first_json_sse_payload(&body)
    } else {
        serde_json::from_str(&body).unwrap_or_else(|_| panic!("mcp JSON payload should parse"))
    }
}

async fn seed_memory_fixture(db_path: &std::path::Path) -> SeededMemories {
    let store = SqliteMemoryStore::new(db_path, MemoryScope::Agent)
        .unwrap_or_else(|_| panic!("fixture store should initialize"));
    let visible_rust = test_memory(TestMemorySpec {
        content: "Romain uses Rust for MCP work.",
        summary: Some("Rust MCP work".to_string()),
        state: MemoryState::Active,
        memory_type: MemoryType::Fact,
        agent_id: Some(FIXED_NAMESPACE.to_string()),
        tags: vec!["rust".to_string(), "mcp".to_string()],
        importance_score: 0.9,
        embedding_stale: false,
    });
    let visible_cloudflare = test_memory(TestMemorySpec {
        content: "Cloudflare tunnel elegy-memory is configured.",
        summary: Some("Cloudflare tunnel".to_string()),
        state: MemoryState::Dormant,
        memory_type: MemoryType::Procedure,
        agent_id: Some(FIXED_NAMESPACE.to_string()),
        tags: vec!["cloudflare".to_string(), "tunnel".to_string()],
        importance_score: 0.7,
        embedding_stale: true,
    });
    let visible_preference = test_memory(TestMemorySpec {
        content: "Romain prefers concise summaries.",
        summary: None,
        state: MemoryState::Active,
        memory_type: MemoryType::Preference,
        agent_id: Some(FIXED_NAMESPACE.to_string()),
        tags: vec!["style".to_string()],
        importance_score: 0.6,
        embedding_stale: false,
    });
    let hidden_other_agent = test_memory(TestMemorySpec {
        content: "Hidden other-agent memory should never be exposed.",
        summary: None,
        state: MemoryState::Active,
        memory_type: MemoryType::Fact,
        agent_id: Some("other-agent".to_string()),
        tags: vec!["hidden".to_string()],
        importance_score: 0.95,
        embedding_stale: false,
    });
    let hidden_other_agent_second = test_memory(TestMemorySpec {
        content: "Another hidden memory for contradiction counting.",
        summary: None,
        state: MemoryState::Active,
        memory_type: MemoryType::Decision,
        agent_id: Some("other-agent".to_string()),
        tags: vec!["hidden".to_string()],
        importance_score: 0.55,
        embedding_stale: false,
    });

    let visible_rust_id = store
        .store(visible_rust.clone())
        .await
        .unwrap_or_else(|_| panic!("visible rust memory should store"));
    let visible_cloudflare_id = store
        .store(visible_cloudflare.clone())
        .await
        .unwrap_or_else(|_| panic!("visible cloudflare memory should store"));
    let visible_preference_id = store
        .store(visible_preference.clone())
        .await
        .unwrap_or_else(|_| panic!("visible preference memory should store"));
    let hidden_other_agent_id = store
        .store(hidden_other_agent.clone())
        .await
        .unwrap_or_else(|_| panic!("hidden other-agent memory should store"));
    let hidden_other_agent_second_id = store
        .store(hidden_other_agent_second.clone())
        .await
        .unwrap_or_else(|_| panic!("second hidden other-agent memory should store"));

    store
        .record_contradiction(
            &visible_rust_id,
            &visible_preference_id,
            "fixture visible contradiction",
        )
        .await
        .unwrap_or_else(|_| panic!("visible contradiction should record"));
    store
        .record_contradiction(
            &hidden_other_agent_id,
            &hidden_other_agent_second_id,
            "fixture hidden contradiction",
        )
        .await
        .unwrap_or_else(|_| panic!("hidden contradiction should record"));

    SeededMemories {
        visible_rust_id: visible_rust_id.to_string(),
        visible_cloudflare_id: visible_cloudflare_id.to_string(),
        hidden_other_agent_id: hidden_other_agent_id.to_string(),
    }
}

fn test_memory(spec: TestMemorySpec<'_>) -> Memory {
    let now = "2026-04-21T12:00:00Z"
        .parse()
        .unwrap_or_else(|_| panic!("fixture timestamp should parse"));
    Memory {
        id: Uuid::new_v4(),
        content: spec.content.to_string(),
        summary: spec.summary,
        scope: MemoryScope::Agent,
        memory_type: spec.memory_type,
        provenance: ProvenanceLevel::UserStated,
        importance_score: spec.importance_score,
        reliability_score: ProvenanceLevel::UserStated.base_reliability(),
        sensitivity: SensitivityLevel::Low,
        state: spec.state,
        tags: spec.tags,
        status: None,
        custom_metadata: HashMap::new(),
        access_count: 0,
        corroboration_count: 0,
        embedding_stale: spec.embedding_stale,
        created_at: now,
        updated_at: now,
        last_accessed_at: None,
        tenant_id: None,
        user_id: None,
        agent_id: spec.agent_id,
    }
}

fn find_tool<'a>(tools: &'a [Value], name: &str) -> &'a Value {
    tools
        .iter()
        .find(|tool| tool["name"] == json!(name))
        .unwrap_or_else(|| panic!("tool `{name}` should exist"))
}

fn tool_property_schema<'a>(tool: &'a Value, property: &str) -> &'a Value {
    tool["inputSchema"]["properties"]
        .get(property)
        .unwrap_or_else(|| panic!("tool schema should expose property `{property}`"))
}

fn schema_allows_null(schema: &Value) -> bool {
    match schema {
        Value::Object(map) => {
            map.get("type").is_some_and(|value| match value {
                Value::String(kind) => kind == "null",
                Value::Array(kinds) => kinds.iter().any(|kind| kind == "null"),
                _ => false,
            }) || ["anyOf", "oneOf", "allOf"]
                .iter()
                .filter_map(|key| map.get(*key))
                .any(schema_allows_null)
        }
        Value::Array(items) => items.iter().any(schema_allows_null),
        _ => false,
    }
}

#[tokio::test]
async fn memory_search_and_tool_schema_stay_namespace_bound() {
    let harness = TestHarness::new().await;
    let seeded = seed_memory_fixture(&harness.db_path).await;
    let session = authenticated_mcp_session(&harness).await;

    let list_tools = mcp_json_request(
        &harness,
        &session,
        json!({"jsonrpc":"2.0","id":10,"method":"tools/list","params":{}}),
    )
    .await;
    let tools = list_tools["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("tools list should be an array"));
    let tool_names = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"memory_search"));
    assert!(tool_names.contains(&"memory_recall"));
    assert!(tool_names.contains(&"memory_list"));
    assert!(tool_names.contains(&"memory_stats"));
    assert!(tool_names.contains(&"memory_store"));
    assert!(tool_names.contains(&"memory_update"));
    assert!(tool_names.contains(&"memory_correct"));
    assert!(tool_names.contains(&"memory_delete"));
    for tool in tools {
        let schema = &tool["inputSchema"];
        assert!(
            !schema.to_string().contains("scope"),
            "tool schema leaked scope field: {schema}"
        );
        assert!(
            !schema.to_string().contains("namespace"),
            "tool schema leaked namespace field: {schema}"
        );
    }
    let memory_store = find_tool(tools, "memory_store");
    for property in [
        "memoryType",
        "importance",
        "provenance",
        "sensitivity",
        "tags",
        "customMetadata",
    ] {
        assert!(
            schema_allows_null(tool_property_schema(memory_store, property)),
            "memory_store schema should allow explicit null for `{property}`"
        );
    }
    let memory_search = find_tool(tools, "memory_search");
    for property in ["limit", "includeDormant", "memoryTypes"] {
        assert!(
            schema_allows_null(tool_property_schema(memory_search, property)),
            "memory_search schema should allow explicit null for `{property}`"
        );
    }
    let memory_list = find_tool(tools, "memory_list");
    for property in ["limit", "includeDormant", "state", "memoryTypes"] {
        assert!(
            schema_allows_null(tool_property_schema(memory_list, property)),
            "memory_list schema should allow explicit null for `{property}`"
        );
    }

    let search_response = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":11,
            "method":"tools/call",
            "params":{
                "name":"memory_search",
                "arguments":{
                    "query":"rust",
                    "limit":5
                }
            }
        }),
    )
    .await;
    let result = &search_response["result"]["structuredContent"];
    assert_eq!(result["namespace"], json!(FIXED_NAMESPACE));
    assert_eq!(result["count"], json!(1));
    assert_eq!(result["results"][0]["id"], json!(seeded.visible_rust_id));
    assert_ne!(
        result["results"][0]["id"],
        json!(seeded.hidden_other_agent_id)
    );
}

#[tokio::test]
async fn memory_recall_only_returns_memories_from_fixed_namespace() {
    let harness = TestHarness::new().await;
    let seeded = seed_memory_fixture(&harness.db_path).await;
    let session = authenticated_mcp_session(&harness).await;

    let visible_recall = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":20,
            "method":"tools/call",
            "params":{
                "name":"memory_recall",
                "arguments":{"id": seeded.visible_rust_id.clone()}
            }
        }),
    )
    .await;
    assert_eq!(
        visible_recall["result"]["structuredContent"]["found"],
        json!(true)
    );
    assert_eq!(
        visible_recall["result"]["structuredContent"]["memory"]["id"],
        json!(seeded.visible_rust_id)
    );

    let hidden_recall = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":21,
            "method":"tools/call",
            "params":{
                "name":"memory_recall",
                "arguments":{"id": seeded.hidden_other_agent_id.clone()}
            }
        }),
    )
    .await;
    assert_eq!(
        hidden_recall["result"]["structuredContent"]["found"],
        json!(false)
    );
    assert!(hidden_recall["result"]["structuredContent"]["memory"].is_null());
}

#[tokio::test]
async fn memory_list_defaults_to_active_and_filters_to_fixed_namespace() {
    let harness = TestHarness::new().await;
    let seeded = seed_memory_fixture(&harness.db_path).await;
    let session = authenticated_mcp_session(&harness).await;

    let default_list = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":30,
            "method":"tools/call",
            "params":{
                "name":"memory_list",
                "arguments":{"limit":10}
            }
        }),
    )
    .await;
    let memories = default_list["result"]["structuredContent"]["memories"]
        .as_array()
        .unwrap_or_else(|| panic!("memories should be an array"));
    assert_eq!(memories.len(), 2);
    assert!(memories.iter().all(|memory| {
        memory["id"] != json!(seeded.visible_cloudflare_id.clone())
            && memory["id"] != json!(seeded.hidden_other_agent_id.clone())
    }));

    let include_dormant = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":31,
            "method":"tools/call",
            "params":{
                "name":"memory_list",
                "arguments":{"limit":10, "includeDormant": true}
            }
        }),
    )
    .await;
    let listed_ids = include_dormant["result"]["structuredContent"]["memories"]
        .as_array()
        .unwrap_or_else(|| panic!("includeDormant memories should be an array"))
        .iter()
        .map(|memory| memory["id"].as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert!(listed_ids.contains(&seeded.visible_cloudflare_id));
    assert!(!listed_ids.contains(&seeded.hidden_other_agent_id));
}

#[tokio::test]
async fn memory_stats_report_only_fixed_namespace_counts() {
    let harness = TestHarness::new().await;
    let _seeded = seed_memory_fixture(&harness.db_path).await;
    let session = authenticated_mcp_session(&harness).await;

    let stats = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":40,
            "method":"tools/call",
            "params":{
                "name":"memory_stats",
                "arguments":{}
            }
        }),
    )
    .await;
    let structured = &stats["result"]["structuredContent"];
    assert_eq!(structured["namespace"], json!(FIXED_NAMESPACE));
    assert_eq!(structured["scope"], json!("agent"));
    assert_eq!(structured["agentId"], json!(FIXED_NAMESPACE));
    assert_eq!(structured["totalCount"], json!(3));
    assert_eq!(structured["activeCount"], json!(2));
    assert_eq!(structured["dormantCount"], json!(1));
    assert_eq!(structured["staleEmbeddingsCount"], json!(1));
    assert_eq!(structured["unresolvedContradictions"], json!(1));
    assert_eq!(structured["typeCounts"]["fact"], json!(1));
    assert_eq!(structured["typeCounts"]["preference"], json!(1));
    assert_eq!(structured["typeCounts"]["procedure"], json!(1));
}

#[tokio::test]
async fn scope_override_arguments_are_rejected_with_invalid_params() {
    let harness = TestHarness::new().await;
    let _seeded = seed_memory_fixture(&harness.db_path).await;
    let session = authenticated_mcp_session(&harness).await;

    let response = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":50,
            "method":"tools/call",
            "params":{
                "name":"memory_search",
                "arguments":{
                    "query":"rust",
                    "scope":"workspace"
                }
            }
        }),
    )
    .await;
    assert_eq!(response["error"]["code"], json!(-32602));
    assert_eq!(
        response["error"]["message"],
        json!(SCOPE_OVERRIDE_ERROR_MESSAGE)
    );
}

#[tokio::test]
async fn memory_store_then_read_back_and_list_are_visible() {
    let harness = TestHarness::new().await;
    let session = authenticated_mcp_session(&harness).await;

    let stored = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":60,
            "method":"tools/call",
            "params":{
                "name":"memory_store",
                "arguments":{
                    "content":"WU7 store keeps MCP writes visible in claude-ai-remote.",
                    "memoryType":"fact",
                    "importance":0.82,
                    "tags":["wu7","write-path"]
                }
            }
        }),
    )
    .await;
    let stored_memory = &stored["result"]["structuredContent"]["memory"];
    let stored_id = stored_memory["id"]
        .as_str()
        .unwrap_or_else(|| panic!("stored memory id should exist"))
        .to_string();
    assert_eq!(
        stored["result"]["structuredContent"]["namespace"],
        json!(FIXED_NAMESPACE)
    );
    assert_eq!(
        stored["result"]["structuredContent"]["action"],
        json!("added")
    );

    let recalled = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":61,
            "method":"tools/call",
            "params":{
                "name":"memory_recall",
                "arguments":{"id": stored_id}
            }
        }),
    )
    .await;
    assert_eq!(
        recalled["result"]["structuredContent"]["found"],
        json!(true)
    );
    assert_eq!(
        recalled["result"]["structuredContent"]["memory"]["content"],
        json!("WU7 store keeps MCP writes visible in claude-ai-remote.")
    );

    let listed = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":62,
            "method":"tools/call",
            "params":{
                "name":"memory_list",
                "arguments":{"limit":20}
            }
        }),
    )
    .await;
    let listed_ids = listed["result"]["structuredContent"]["memories"]
        .as_array()
        .unwrap_or_else(|| panic!("listed memories should be an array"))
        .iter()
        .map(|memory| memory["id"].as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert!(listed_ids.contains(&stored_memory["id"].as_str().unwrap_or_default().to_string()));
}

#[tokio::test]
async fn defaulted_memory_tool_fields_accept_explicit_null() {
    let harness = TestHarness::new().await;
    let session = authenticated_mcp_session(&harness).await;

    let stored = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":65,
            "method":"tools/call",
            "params":{
                "name":"memory_store",
                "arguments":{
                    "content":"Explicit null defaults now deserialize across MCP memory tools.",
                    "summary":null,
                    "memoryType":null,
                    "importance":null,
                    "provenance":null,
                    "sensitivity":null,
                    "tags":null,
                    "customMetadata":null
                }
            }
        }),
    )
    .await;
    let stored_memory = &stored["result"]["structuredContent"]["memory"];
    let stored_id = stored_memory["id"]
        .as_str()
        .unwrap_or_else(|| panic!("stored memory id should exist"))
        .to_owned();
    assert_eq!(
        stored["result"]["structuredContent"]["action"],
        json!("added")
    );
    assert_eq!(
        stored["result"]["structuredContent"]["embeddingStatus"],
        json!("skipped_no_provider")
    );
    assert_eq!(stored_memory["memoryType"], json!("observation"));
    assert_eq!(stored_memory["provenance"], json!("user-stated"));
    assert_eq!(stored_memory["importance"], json!(0.5));
    assert_eq!(stored_memory["tags"], json!([]));
    assert!(
        stored_memory.get("summary").is_none(),
        "null summary should normalize to omission"
    );

    let search = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":66,
            "method":"tools/call",
            "params":{
                "name":"memory_search",
                "arguments":{
                    "query":"deserialize across MCP",
                    "limit":null,
                    "includeDormant":null,
                    "memoryTypes":null
                }
            }
        }),
    )
    .await;
    assert_eq!(
        search["result"]["structuredContent"]["includeDormant"],
        json!(false)
    );
    let search_results = search["result"]["structuredContent"]["results"]
        .as_array()
        .unwrap_or_else(|| panic!("search results should be an array"));
    assert_eq!(search["result"]["structuredContent"]["count"], json!(1));
    assert_eq!(search_results[0]["id"], json!(stored_id.clone()));

    let listed = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":67,
            "method":"tools/call",
            "params":{
                "name":"memory_list",
                "arguments":{
                    "limit":null,
                    "includeDormant":null,
                    "state":null,
                    "memoryTypes":null
                }
            }
        }),
    )
    .await;
    assert_eq!(
        listed["result"]["structuredContent"]["includeDormant"],
        json!(false)
    );
    let memories = listed["result"]["structuredContent"]["memories"]
        .as_array()
        .unwrap_or_else(|| panic!("listed memories should be an array"));
    assert_eq!(listed["result"]["structuredContent"]["count"], json!(1));
    assert_eq!(memories[0]["id"], json!(stored_id));
}

#[tokio::test]
async fn memory_update_changes_existing_content() {
    let harness = TestHarness::new().await;
    let session = authenticated_mcp_session(&harness).await;

    let stored = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":70,
            "method":"tools/call",
            "params":{
                "name":"memory_store",
                "arguments":{"content":"Initial MCP update content."}
            }
        }),
    )
    .await;
    let stored_id = stored["result"]["structuredContent"]["memory"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("stored id should exist"))
        .to_string();

    let updated = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":71,
            "method":"tools/call",
            "params":{
                "name":"memory_update",
                "arguments":{
                    "id": stored_id,
                    "content":"Updated MCP content now mentions Cloudflare tunnel.",
                    "reason":"WU7 update path"
                }
            }
        }),
    )
    .await;
    assert_eq!(
        updated["result"]["structuredContent"]["updated"],
        json!(true)
    );
    assert_eq!(
        updated["result"]["structuredContent"]["memory"]["content"],
        json!("Updated MCP content now mentions Cloudflare tunnel.")
    );
}

#[tokio::test]
async fn memory_correct_respects_gate_aware_merge_behavior() {
    let harness = TestHarness::new().await;
    let session = authenticated_mcp_session(&harness).await;

    let primary = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":80,
            "method":"tools/call",
            "params":{
                "name":"memory_store",
                "arguments":{"content":"The canonical correction target."}
            }
        }),
    )
    .await;
    let primary_id = primary["result"]["structuredContent"]["memory"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("primary id should exist"))
        .to_string();

    let secondary = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":81,
            "method":"tools/call",
            "params":{
                "name":"memory_store",
                "arguments":{"content":"The memory that will be corrected."}
            }
        }),
    )
    .await;
    let secondary_id = secondary["result"]["structuredContent"]["memory"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("secondary id should exist"))
        .to_string();

    let corrected = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":82,
            "method":"tools/call",
            "params":{
                "name":"memory_correct",
                "arguments":{
                    "id": secondary_id,
                    "content":"The canonical correction target.",
                    "reason":"align duplicate content"
                }
            }
        }),
    )
    .await;
    assert_eq!(
        corrected["result"]["structuredContent"]["correction"]["disposition"],
        json!("merged")
    );
    assert_eq!(
        corrected["result"]["structuredContent"]["correction"]["relatedMemoryId"],
        json!(primary_id)
    );
    assert_eq!(
        corrected["result"]["structuredContent"]["memory"]["state"],
        json!("dormant")
    );
}

#[tokio::test]
async fn memory_delete_removes_memory_from_recall() {
    let harness = TestHarness::new().await;
    let session = authenticated_mcp_session(&harness).await;

    let stored = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":90,
            "method":"tools/call",
            "params":{
                "name":"memory_store",
                "arguments":{"content":"Delete this MCP memory."}
            }
        }),
    )
    .await;
    let stored_id = stored["result"]["structuredContent"]["memory"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("stored id should exist"))
        .to_string();

    let deleted = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":91,
            "method":"tools/call",
            "params":{
                "name":"memory_delete",
                "arguments":{"id": stored_id}
            }
        }),
    )
    .await;
    assert_eq!(
        deleted["result"]["structuredContent"]["deleted"],
        json!(true)
    );

    let recalled = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":92,
            "method":"tools/call",
            "params":{
                "name":"memory_recall",
                "arguments":{"id": deleted["result"]["structuredContent"]["id"].clone()}
            }
        }),
    )
    .await;
    assert_eq!(
        recalled["result"]["structuredContent"]["found"],
        json!(false)
    );
}

#[tokio::test]
async fn write_scope_override_arguments_are_rejected_with_invalid_params() {
    let harness = TestHarness::new().await;
    let session = authenticated_mcp_session(&harness).await;

    let response = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":100,
            "method":"tools/call",
            "params":{
                "name":"memory_store",
                "arguments":{
                    "content":"attempted override",
                    "namespace":"workspace"
                }
            }
        }),
    )
    .await;
    assert_eq!(response["error"]["code"], json!(-32602));
    assert_eq!(
        response["error"]["message"],
        json!(SCOPE_OVERRIDE_ERROR_MESSAGE)
    );
}

#[tokio::test]
async fn audit_logging_omits_content_and_includes_jti_for_write_tools() {
    let log_buffer = install_test_tracing();
    clear_test_logs(&log_buffer);

    let harness = TestHarness::new().await;
    let session = authenticated_mcp_session_with_jti(&harness, "wu7-audit-jti").await;
    let content = "WU7 audit content must never appear in logs.";

    let _stored = mcp_json_request(
        &harness,
        &session,
        json!({
            "jsonrpc":"2.0",
            "id":110,
            "method":"tools/call",
            "params":{
                "name":"memory_store",
                "arguments":{"content": content}
            }
        }),
    )
    .await;

    let logs = read_test_logs(&log_buffer);
    let audit_line = logs
        .lines()
        .find(|line| {
            line.contains("\"message\":\"memory write audit\"")
                && line.contains("\"jti\":\"wu7-audit-jti\"")
        })
        .unwrap_or_else(|| panic!("audit log line with jti should exist"));
    assert!(audit_line.contains("\"tool\":\"memory_store\""));
    assert!(audit_line.contains("\"scope\":\"claude-ai-remote\""));
    assert!(!audit_line.contains(content));
}
