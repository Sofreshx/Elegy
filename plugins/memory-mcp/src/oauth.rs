use std::{
    collections::{HashMap, VecDeque},
    fs,
    net::IpAddr,
    path::Path,
    sync::{Arc, Mutex, RwLock},
};

use anyhow::{anyhow, Context};
use argon2::{password_hash::PasswordHash, Argon2, PasswordVerifier};
use axum::{
    extract::{ConnectInfo, Form, Query, State},
    http::{
        header::{LOCATION, RETRY_AFTER},
        HeaderMap, HeaderValue, StatusCode,
    },
    response::{Html, IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use elegy_memory_mcp::config::Config;

pub const ACCESS_TOKEN_TTL_SECONDS: i64 = 60 * 60;
pub const AUTHORIZATION_CODE_TTL_SECONDS: i64 = 60;
pub const REFRESH_TOKEN_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;
pub const OAUTH_SCOPE: &str = "claude-ai-remote";

const REGISTER_RATE_LIMIT_PER_MINUTE: usize = 10;
const TOKEN_RATE_LIMIT_PER_MINUTE: usize = 10;
const AUTHORIZE_RATE_LIMIT_PER_MINUTE: usize = 20;
const RATE_LIMIT_WINDOW_SECONDS: i64 = 60;
const SIGNING_KEY_FILENAME: &str = "signing-key";
const CLIENTS_FILENAME: &str = "clients.json";
const REFRESH_TOKENS_FILENAME: &str = "refresh-tokens.json";

type NowFn = Arc<dyn Fn() -> i64 + Send + Sync>;

#[derive(Clone)]
pub struct AppState {
    pub oauth: Arc<OAuthService>,
}

pub struct OAuthService {
    config: Config,
    now: NowFn,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    clients: RwLock<HashMap<String, RegisteredClient>>,
    refresh_tokens: RwLock<HashMap<String, PersistedRefreshToken>>,
    authorization_codes: Mutex<HashMap<String, AuthorizationCode>>,
    rate_limiter: Mutex<RateLimiter>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisteredClient {
    pub client_id: String,
    pub metadata: ClientMetadata,
    pub registered_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientMetadata {
    pub redirect_uris: Vec<String>,
    pub client_name: Option<String>,
    pub grant_types: Vec<String>,
    pub response_types: Vec<String>,
    pub token_endpoint_auth_method: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedRefreshToken {
    pub token_hash: String,
    pub client_id: String,
    pub scope: String,
    pub expires_at: i64,
}

#[derive(Clone, Debug)]
struct AuthorizationCode {
    client_id: String,
    redirect_uri: String,
    code_challenge: String,
    scope: String,
    expires_at: i64,
}

#[derive(Default)]
struct RateLimiter {
    buckets: HashMap<(String, String), VecDeque<i64>>,
}

#[derive(Clone, Copy)]
enum RateLimitedEndpoint {
    Register,
    Authorize,
    Token,
}

impl RateLimitedEndpoint {
    fn as_key(self) -> &'static str {
        match self {
            Self::Register => "/oauth/register",
            Self::Authorize => "/oauth/authorize",
            Self::Token => "/oauth/token",
        }
    }

    fn limit(self) -> usize {
        match self {
            Self::Register => REGISTER_RATE_LIMIT_PER_MINUTE,
            Self::Authorize => AUTHORIZE_RATE_LIMIT_PER_MINUTE,
            Self::Token => TOKEN_RATE_LIMIT_PER_MINUTE,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ClientRegistrationRequest {
    pub redirect_uris: Vec<String>,
    pub client_name: Option<String>,
    pub grant_types: Option<Vec<String>>,
    pub response_types: Option<Vec<String>>,
    pub token_endpoint_auth_method: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ClientRegistrationResponse {
    pub client_id: String,
    pub client_id_issued_at: i64,
    pub redirect_uris: Vec<String>,
    pub client_name: Option<String>,
    pub grant_types: Vec<String>,
    pub response_types: Vec<String>,
    pub token_endpoint_auth_method: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AuthorizeRequest {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AuthorizeForm {
    #[serde(flatten)]
    pub request: AuthorizeRequest,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: Option<String>,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub refresh_token: String,
    pub scope: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AccessTokenClaims {
    pub(crate) iss: String,
    pub(crate) aud: String,
    pub(crate) sub: String,
    pub(crate) client_id: String,
    pub(crate) scope: String,
    pub(crate) iat: usize,
    pub(crate) exp: usize,
    pub(crate) jti: String,
}

#[derive(Debug, Error)]
pub enum AccessTokenValidationError {
    #[error("access token is invalid")]
    Invalid,
    #[error("access token expired")]
    Expired,
    #[error("access token scope is invalid")]
    InvalidScope,
}

#[derive(Debug)]
pub enum OAuthError {
    BadRequest(String),
    Html(StatusCode, String),
    OAuth {
        status: StatusCode,
        error: &'static str,
        description: String,
    },
    Internal(String),
}

impl IntoResponse for OAuthError {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message).into_response(),
            Self::Html(status, body) => (status, Html(body)).into_response(),
            Self::OAuth {
                status,
                error,
                description,
            } => (
                status,
                Json(json!({
                    "error": error,
                    "error_description": description,
                })),
            )
                .into_response(),
            Self::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message).into_response(),
        }
    }
}

impl OAuthService {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        Self::with_now(
            config,
            Arc::new(|| OffsetDateTime::now_utc().unix_timestamp()),
        )
    }

    pub fn with_now(config: Config, now: NowFn) -> anyhow::Result<Self> {
        fs::create_dir_all(&config.data_dir)
            .with_context(|| format!("creating data dir {}", config.data_dir.display()))?;

        let signing_key = load_or_create_signing_key(&config.data_dir)?;
        let clients = load_clients(&config.data_dir)?;
        let refresh_tokens = load_refresh_tokens(&config.data_dir)?;

        Ok(Self {
            config,
            now,
            encoding_key: EncodingKey::from_secret(&signing_key),
            decoding_key: DecodingKey::from_secret(&signing_key),
            clients: RwLock::new(clients),
            refresh_tokens: RwLock::new(refresh_tokens),
            authorization_codes: Mutex::new(HashMap::new()),
            rate_limiter: Mutex::new(RateLimiter::default()),
        })
    }

    pub fn protected_resource_metadata(&self) -> Value {
        json!({
            "resource": self.mcp_url().as_str(),
            "authorization_servers": [self.config.public_url.as_str()],
            "scopes_supported": [OAUTH_SCOPE],
            "bearer_methods_supported": ["header"],
        })
    }

    pub fn authorization_server_metadata(&self) -> Value {
        json!({
            "issuer": self.config.public_url.as_str(),
            "authorization_endpoint": self.oauth_url("oauth/authorize"),
            "token_endpoint": self.oauth_url("oauth/token"),
            "registration_endpoint": self.oauth_url("oauth/register"),
            "response_types_supported": ["code"],
            "grant_types_supported": ["authorization_code", "refresh_token"],
            "code_challenge_methods_supported": ["S256"],
            "token_endpoint_auth_methods_supported": ["none"],
            "scopes_supported": [OAUTH_SCOPE],
        })
    }

    pub fn mcp_bearer_challenge(&self) -> String {
        format!(
            "Bearer realm=\"elegy-mcp\", resource_metadata=\"{}\"",
            self.oauth_url(".well-known/oauth-protected-resource")
        )
    }

    pub fn register_client(
        &self,
        request: ClientRegistrationRequest,
    ) -> Result<ClientRegistrationResponse, OAuthError> {
        if request.redirect_uris.is_empty() {
            return Err(OAuthError::BadRequest(
                "redirect_uris must contain at least one URI".to_owned(),
            ));
        }

        if let Some(method) = &request.token_endpoint_auth_method {
            if method != "none" {
                return Err(OAuthError::OAuth {
                    status: StatusCode::BAD_REQUEST,
                    error: "invalid_client_metadata",
                    description: "token_endpoint_auth_method must be \"none\"".to_owned(),
                });
            }
        }

        let redirect_uris = request
            .redirect_uris
            .iter()
            .map(|uri| validate_redirect_uri(uri))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|url| url.to_string())
            .collect::<Vec<_>>();

        let grant_types = request
            .grant_types
            .unwrap_or_else(|| vec!["authorization_code".to_owned(), "refresh_token".to_owned()]);
        let response_types = request
            .response_types
            .unwrap_or_else(|| vec!["code".to_owned()]);

        let now = (self.now)();
        let client_id = Uuid::new_v4().to_string();
        let client = RegisteredClient {
            client_id: client_id.clone(),
            metadata: ClientMetadata {
                redirect_uris: redirect_uris.clone(),
                client_name: request.client_name.clone(),
                grant_types: grant_types.clone(),
                response_types: response_types.clone(),
                token_endpoint_auth_method: "none".to_owned(),
            },
            registered_at: now,
        };

        {
            let mut clients = self
                .clients
                .write()
                .map_err(|_| OAuthError::Internal("clients lock poisoned".to_owned()))?;
            clients.insert(client_id.clone(), client.clone());
            persist_clients(&self.config.data_dir, &clients).map_err(internal_error)?;
        }

        Ok(ClientRegistrationResponse {
            client_id,
            client_id_issued_at: now,
            redirect_uris,
            client_name: request.client_name,
            grant_types,
            response_types,
            token_endpoint_auth_method: "none".to_owned(),
        })
    }

    pub fn render_authorize_page(&self, request: &AuthorizeRequest) -> Result<String, OAuthError> {
        let validated = self.validate_authorize_request(request)?;
        Ok(render_authorize_html(&validated))
    }

    pub fn authorize(&self, form: AuthorizeForm) -> Result<String, OAuthError> {
        let validated = self.validate_authorize_request(&form.request)?;

        let password_hash =
            PasswordHash::new(&self.config.admin_password_verifier).map_err(internal_error)?;
        let verified = Argon2::default()
            .verify_password(form.password.as_bytes(), &password_hash)
            .is_ok();
        if !verified {
            return Err(OAuthError::Html(
                StatusCode::UNAUTHORIZED,
                render_html_error("Mot de passe invalide."),
            ));
        }

        let code = random_token()?;
        let expires_at = (self.now)() + AUTHORIZATION_CODE_TTL_SECONDS;
        let auth_code = AuthorizationCode {
            client_id: validated.client_id.clone(),
            redirect_uri: validated.redirect_uri.clone(),
            code_challenge: validated.code_challenge.clone(),
            scope: OAUTH_SCOPE.to_owned(),
            expires_at,
        };

        let mut authorization_codes = self
            .authorization_codes
            .lock()
            .map_err(|_| OAuthError::Internal("authorization code lock poisoned".to_owned()))?;
        authorization_codes.insert(code.clone(), auth_code);

        build_redirect_uri(&validated.redirect_uri, &code, validated.state.as_deref())
    }

    pub fn exchange_token(&self, request: TokenRequest) -> Result<TokenResponse, OAuthError> {
        match request.grant_type.as_str() {
            "authorization_code" => self.exchange_authorization_code(request),
            "refresh_token" => self.exchange_refresh_token(request),
            _ => Err(OAuthError::OAuth {
                status: StatusCode::BAD_REQUEST,
                error: "unsupported_grant_type",
                description: "grant_type must be authorization_code or refresh_token".to_owned(),
            }),
        }
    }

    fn exchange_authorization_code(
        &self,
        request: TokenRequest,
    ) -> Result<TokenResponse, OAuthError> {
        let client_id = required_field(request.client_id, "client_id")?;
        let redirect_uri_raw = required_field(request.redirect_uri, "redirect_uri")?;
        let redirect_uri = validate_redirect_uri(&redirect_uri_raw)?.to_string();
        self.ensure_known_client(&client_id)?;
        validate_scope(request.scope.as_deref())?;

        let code = required_field(request.code, "code")?;
        let code_verifier = required_field(request.code_verifier, "code_verifier")?;

        let auth_code = {
            let mut authorization_codes = self
                .authorization_codes
                .lock()
                .map_err(|_| OAuthError::Internal("authorization code lock poisoned".to_owned()))?;
            authorization_codes.remove(&code)
        }
        .ok_or_else(|| invalid_grant("authorization code is invalid or already used"))?;

        let now = (self.now)();
        if auth_code.expires_at < now {
            return Err(invalid_grant("authorization code expired"));
        }
        if auth_code.client_id != client_id {
            return Err(invalid_grant("authorization code client mismatch"));
        }
        if auth_code.redirect_uri != redirect_uri {
            return Err(invalid_grant("redirect_uri mismatch"));
        }
        if auth_code.code_challenge != pkce_s256(&code_verifier) {
            return Err(invalid_grant("PKCE verification failed"));
        }

        self.issue_tokens(&client_id, &auth_code.scope)
    }

    fn exchange_refresh_token(&self, request: TokenRequest) -> Result<TokenResponse, OAuthError> {
        let client_id = required_field(request.client_id, "client_id")?;
        self.ensure_known_client(&client_id)?;
        validate_scope(request.scope.as_deref())?;

        let refresh_token = required_field(request.refresh_token, "refresh_token")?;
        let refresh_token_hash = hash_token(&refresh_token);
        let now = (self.now)();

        let mut refresh_tokens = self
            .refresh_tokens
            .write()
            .map_err(|_| OAuthError::Internal("refresh token lock poisoned".to_owned()))?;

        let existing = refresh_tokens
            .remove(&refresh_token_hash)
            .ok_or_else(|| invalid_grant("refresh token invalid"))?;

        if existing.client_id != client_id {
            return Err(invalid_grant("refresh token client mismatch"));
        }
        if existing.expires_at < now {
            persist_refresh_tokens(&self.config.data_dir, &refresh_tokens)
                .map_err(internal_error)?;
            return Err(invalid_grant("refresh token expired"));
        }

        let scope = existing.scope;
        let new_refresh_token = random_token()?;
        let new_refresh_token_hash = hash_token(&new_refresh_token);
        let new_record = PersistedRefreshToken {
            token_hash: new_refresh_token_hash.clone(),
            client_id: client_id.clone(),
            scope: scope.clone(),
            expires_at: now + REFRESH_TOKEN_TTL_SECONDS,
        };
        refresh_tokens.insert(new_refresh_token_hash, new_record);
        persist_refresh_tokens(&self.config.data_dir, &refresh_tokens).map_err(internal_error)?;
        drop(refresh_tokens);

        self.issue_access_and_refresh_response(&client_id, &scope, new_refresh_token, now)
    }

    fn issue_tokens(&self, client_id: &str, scope: &str) -> Result<TokenResponse, OAuthError> {
        let now = (self.now)();
        let refresh_token = random_token()?;
        let refresh_record = PersistedRefreshToken {
            token_hash: hash_token(&refresh_token),
            client_id: client_id.to_owned(),
            scope: scope.to_owned(),
            expires_at: now + REFRESH_TOKEN_TTL_SECONDS,
        };

        {
            let mut refresh_tokens = self
                .refresh_tokens
                .write()
                .map_err(|_| OAuthError::Internal("refresh token lock poisoned".to_owned()))?;
            refresh_tokens.insert(refresh_record.token_hash.clone(), refresh_record);
            persist_refresh_tokens(&self.config.data_dir, &refresh_tokens)
                .map_err(internal_error)?;
        }

        self.issue_access_and_refresh_response(client_id, scope, refresh_token, now)
    }

    fn issue_access_and_refresh_response(
        &self,
        client_id: &str,
        scope: &str,
        refresh_token: String,
        now: i64,
    ) -> Result<TokenResponse, OAuthError> {
        let claims = AccessTokenClaims {
            iss: self.config.public_url.to_string(),
            aud: self.mcp_url().to_string(),
            sub: client_id.to_owned(),
            client_id: client_id.to_owned(),
            scope: scope.to_owned(),
            iat: now as usize,
            exp: (now + ACCESS_TOKEN_TTL_SECONDS) as usize,
            jti: Uuid::new_v4().to_string(),
        };

        let access_token =
            jsonwebtoken::encode(&Header::new(Algorithm::HS256), &claims, &self.encoding_key)
                .map_err(internal_error)?;

        Ok(TokenResponse {
            access_token,
            token_type: "Bearer",
            expires_in: ACCESS_TOKEN_TTL_SECONDS,
            refresh_token,
            scope: scope.to_owned(),
        })
    }

    fn validate_authorize_request(
        &self,
        request: &AuthorizeRequest,
    ) -> Result<ValidatedAuthorizeRequest, OAuthError> {
        if request.response_type != "code" {
            return Err(OAuthError::Html(
                StatusCode::BAD_REQUEST,
                render_html_error("response_type must be code"),
            ));
        }
        if request.code_challenge_method != "S256" {
            return Err(OAuthError::Html(
                StatusCode::BAD_REQUEST,
                render_html_error("code_challenge_method must be S256"),
            ));
        }
        if request.code_challenge.trim().is_empty() {
            return Err(OAuthError::Html(
                StatusCode::BAD_REQUEST,
                render_html_error("code_challenge is required"),
            ));
        }
        validate_scope(request.scope.as_deref()).map_err(|error| match error {
            OAuthError::OAuth { description, .. } => {
                OAuthError::Html(StatusCode::BAD_REQUEST, render_html_error(&description))
            }
            other => other,
        })?;

        let redirect_uri = validate_redirect_uri(&request.redirect_uri)?.to_string();
        let client = self.lookup_client(&request.client_id)?;
        if !client.metadata.redirect_uris.contains(&redirect_uri) {
            return Err(OAuthError::Html(
                StatusCode::BAD_REQUEST,
                render_html_error("redirect_uri is not registered for this client"),
            ));
        }

        Ok(ValidatedAuthorizeRequest {
            client_id: request.client_id.clone(),
            redirect_uri,
            state: request.state.clone(),
            code_challenge: request.code_challenge.clone(),
        })
    }

    fn lookup_client(&self, client_id: &str) -> Result<RegisteredClient, OAuthError> {
        let clients = self
            .clients
            .read()
            .map_err(|_| OAuthError::Internal("clients lock poisoned".to_owned()))?;
        clients.get(client_id).cloned().ok_or_else(|| {
            OAuthError::Html(
                StatusCode::BAD_REQUEST,
                render_html_error("client_id is not registered"),
            )
        })
    }

    fn ensure_known_client(&self, client_id: &str) -> Result<(), OAuthError> {
        let clients = self
            .clients
            .read()
            .map_err(|_| OAuthError::Internal("clients lock poisoned".to_owned()))?;
        if clients.contains_key(client_id) {
            Ok(())
        } else {
            Err(OAuthError::OAuth {
                status: StatusCode::BAD_REQUEST,
                error: "invalid_client",
                description: "unknown client_id".to_owned(),
            })
        }
    }

    fn check_rate_limit(&self, endpoint: RateLimitedEndpoint, ip: &str) -> Option<u64> {
        let now = (self.now)();
        let mut limiter = self.rate_limiter.lock().ok()?;
        limiter.check(endpoint, ip, now)
    }

    fn oauth_url(&self, path: &str) -> String {
        self.config
            .public_url
            .join(path)
            .map(|url| url.to_string())
            .unwrap_or_else(|_| format!("{}{}", self.config.public_url, path))
    }

    fn mcp_url(&self) -> Url {
        self.config
            .public_url
            .join("mcp")
            .unwrap_or_else(|_| self.config.public_url.clone())
    }

    pub fn validate_access_token(
        &self,
        token: &str,
    ) -> Result<AccessTokenClaims, AccessTokenValidationError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = false;
        validation.validate_aud = false;
        let decoded =
            jsonwebtoken::decode::<AccessTokenClaims>(token, &self.decoding_key, &validation)
                .map_err(|_| AccessTokenValidationError::Invalid)?;

        if decoded.claims.exp <= (self.now)() as usize {
            return Err(AccessTokenValidationError::Expired);
        }
        if decoded.claims.scope != OAUTH_SCOPE {
            return Err(AccessTokenValidationError::InvalidScope);
        }

        Ok(decoded.claims)
    }

    #[cfg(test)]
    pub fn decode_access_token(&self, token: &str) -> anyhow::Result<AccessTokenClaims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = false;
        validation.validate_aud = false;
        let decoded =
            jsonwebtoken::decode::<AccessTokenClaims>(token, &self.decoding_key, &validation)?;
        Ok(decoded.claims)
    }

    #[cfg(test)]
    pub fn sign_access_token_for_tests(
        &self,
        claims: &AccessTokenClaims,
    ) -> anyhow::Result<String> {
        Ok(jsonwebtoken::encode(
            &Header::new(Algorithm::HS256),
            claims,
            &self.encoding_key,
        )?)
    }
}

#[derive(Clone)]
struct ValidatedAuthorizeRequest {
    client_id: String,
    redirect_uri: String,
    state: Option<String>,
    code_challenge: String,
}

impl RateLimiter {
    fn check(&mut self, endpoint: RateLimitedEndpoint, ip: &str, now: i64) -> Option<u64> {
        let key = (endpoint.as_key().to_owned(), ip.to_owned());
        let bucket = self.buckets.entry(key).or_default();
        while let Some(oldest) = bucket.front().copied() {
            if now - oldest >= RATE_LIMIT_WINDOW_SECONDS {
                let _ = bucket.pop_front();
            } else {
                break;
            }
        }

        if bucket.len() >= endpoint.limit() {
            let retry_after = bucket
                .front()
                .map(|oldest| (RATE_LIMIT_WINDOW_SECONDS - (now - *oldest)).max(1) as u64)
                .unwrap_or(1);
            return Some(retry_after);
        }

        bucket.push_back(now);
        None
    }
}

pub async fn protected_resource_metadata(State(state): State<AppState>) -> Response {
    Json(state.oauth.protected_resource_metadata()).into_response()
}

pub async fn authorization_server_metadata(State(state): State<AppState>) -> Response {
    Json(state.oauth.authorization_server_metadata()).into_response()
}

pub async fn register_client(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(peer_addr): ConnectInfo<std::net::SocketAddr>,
    Json(request): Json<ClientRegistrationRequest>,
) -> Response {
    if let Some(response) = maybe_rate_limited(
        &state,
        RateLimitedEndpoint::Register,
        &headers,
        peer_addr.ip(),
    ) {
        return response;
    }

    match state.oauth.register_client(request) {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn authorize_get(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(peer_addr): ConnectInfo<std::net::SocketAddr>,
    Query(request): Query<AuthorizeRequest>,
) -> Response {
    if let Some(response) = maybe_rate_limited(
        &state,
        RateLimitedEndpoint::Authorize,
        &headers,
        peer_addr.ip(),
    ) {
        return response;
    }

    match state.oauth.render_authorize_page(&request) {
        Ok(html) => Html(html).into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn authorize_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(peer_addr): ConnectInfo<std::net::SocketAddr>,
    Form(form): Form<AuthorizeForm>,
) -> Response {
    if let Some(response) = maybe_rate_limited(
        &state,
        RateLimitedEndpoint::Authorize,
        &headers,
        peer_addr.ip(),
    ) {
        return response;
    }

    match state.oauth.authorize(form) {
        Ok(location) => redirect_response(&location),
        Err(error) => error.into_response(),
    }
}

pub async fn token(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(peer_addr): ConnectInfo<std::net::SocketAddr>,
    Form(request): Form<TokenRequest>,
) -> Response {
    if let Some(response) =
        maybe_rate_limited(&state, RateLimitedEndpoint::Token, &headers, peer_addr.ip())
    {
        return response;
    }

    match state.oauth.exchange_token(request) {
        Ok(response) => Json(response).into_response(),
        Err(error) => error.into_response(),
    }
}

fn maybe_rate_limited(
    state: &AppState,
    endpoint: RateLimitedEndpoint,
    headers: &HeaderMap,
    peer_ip: IpAddr,
) -> Option<Response> {
    let ip = client_ip(headers, peer_ip);
    state
        .oauth
        .check_rate_limit(endpoint, &ip)
        .map(rate_limited_response)
}

fn client_ip(headers: &HeaderMap, peer_ip: IpAddr) -> String {
    headers
        .get("CF-Connecting-IP")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| peer_ip.to_string())
}

fn rate_limited_response(retry_after: u64) -> Response {
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({
            "error": "rate_limited",
            "error_description": "too many requests",
        })),
    )
        .into_response();

    if let Ok(value) = HeaderValue::from_str(&retry_after.to_string()) {
        response.headers_mut().insert(RETRY_AFTER, value);
    }

    response
}

fn redirect_response(location: &str) -> Response {
    let mut response = StatusCode::FOUND.into_response();
    if let Ok(value) = HeaderValue::from_str(location) {
        response.headers_mut().insert(LOCATION, value);
    }
    response
}

fn render_authorize_html(request: &ValidatedAuthorizeRequest) -> String {
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Elegy Memory OAuth</title></head><body><h1>Autorisation Elegy Memory</h1><p>Claude demande accès à ta mémoire (scope: {scope})</p><form method=\"post\" action=\"/oauth/authorize\"><input type=\"hidden\" name=\"response_type\" value=\"code\"><input type=\"hidden\" name=\"client_id\" value=\"{client_id}\"><input type=\"hidden\" name=\"redirect_uri\" value=\"{redirect_uri}\"><input type=\"hidden\" name=\"scope\" value=\"{scope}\"><input type=\"hidden\" name=\"state\" value=\"{state}\"><input type=\"hidden\" name=\"code_challenge\" value=\"{challenge}\"><input type=\"hidden\" name=\"code_challenge_method\" value=\"S256\"><label>Mot de passe <input type=\"password\" name=\"password\" autocomplete=\"current-password\"></label><button type=\"submit\">Authoriser Claude</button></form></body></html>",
        scope = OAUTH_SCOPE,
        client_id = escape_html(&request.client_id),
        redirect_uri = escape_html(&request.redirect_uri),
        state = escape_html(request.state.as_deref().unwrap_or("")),
        challenge = escape_html(&request.code_challenge),
    )
}

fn render_html_error(message: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Erreur OAuth</title></head><body><p>{}</p></body></html>",
        escape_html(message)
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn build_redirect_uri(
    redirect_uri: &str,
    code: &str,
    state: Option<&str>,
) -> Result<String, OAuthError> {
    let mut url =
        Url::parse(redirect_uri).map_err(|error| OAuthError::Internal(error.to_string()))?;
    {
        let mut query_pairs = url.query_pairs_mut();
        query_pairs.append_pair("code", code);
        if let Some(state) = state {
            query_pairs.append_pair("state", state);
        }
    }
    Ok(url.to_string())
}

fn validate_redirect_uri(value: &str) -> Result<Url, OAuthError> {
    let url = Url::parse(value)
        .map_err(|_| OAuthError::BadRequest("redirect_uri must be an absolute URL".to_owned()))?;

    match (url.scheme(), url.host_str()) {
        ("https", Some("claude.ai" | "claude.com")) => Ok(url),
        ("http", Some("127.0.0.1" | "localhost")) if url.port().is_some() => Ok(url),
        _ => Err(OAuthError::BadRequest(
            "redirect_uri is not in the allowlist".to_owned(),
        )),
    }
}

fn validate_scope(scope: Option<&str>) -> Result<(), OAuthError> {
    match scope {
        None | Some(OAUTH_SCOPE) => Ok(()),
        Some(_) => Err(OAuthError::OAuth {
            status: StatusCode::BAD_REQUEST,
            error: "invalid_scope",
            description: format!("scope must be {OAUTH_SCOPE}"),
        }),
    }
}

fn required_field(value: Option<String>, name: &'static str) -> Result<String, OAuthError> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| OAuthError::OAuth {
            status: StatusCode::BAD_REQUEST,
            error: "invalid_request",
            description: format!("{name} is required"),
        })
}

fn invalid_grant(description: &str) -> OAuthError {
    OAuthError::OAuth {
        status: StatusCode::BAD_REQUEST,
        error: "invalid_grant",
        description: description.to_owned(),
    }
}

fn internal_error(error: impl std::fmt::Display) -> OAuthError {
    OAuthError::Internal(error.to_string())
}

fn random_token() -> Result<String, OAuthError> {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn pkce_s256(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn load_or_create_signing_key(data_dir: &Path) -> anyhow::Result<Vec<u8>> {
    let path = data_dir.join(SIGNING_KEY_FILENAME);
    if path.exists() {
        let encoded = fs::read_to_string(&path)
            .with_context(|| format!("reading signing key {}", path.display()))?;
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded.trim())
            .context("decoding persisted signing key")?;
        if bytes.len() != 32 {
            return Err(anyhow!("persisted signing key must decode to 32 bytes"));
        }
        return Ok(bytes);
    }

    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    fs::write(&path, URL_SAFE_NO_PAD.encode(bytes))
        .with_context(|| format!("writing signing key {}", path.display()))?;
    restrict_permissions(&path).context("restricting signing key permissions")?;
    Ok(bytes.to_vec())
}

fn load_clients(data_dir: &Path) -> anyhow::Result<HashMap<String, RegisteredClient>> {
    let path = data_dir.join(CLIENTS_FILENAME);
    load_json_vec::<RegisteredClient>(&path).map(|clients| {
        clients
            .into_iter()
            .map(|client| (client.client_id.clone(), client))
            .collect()
    })
}

fn load_refresh_tokens(data_dir: &Path) -> anyhow::Result<HashMap<String, PersistedRefreshToken>> {
    let path = data_dir.join(REFRESH_TOKENS_FILENAME);
    load_json_vec::<PersistedRefreshToken>(&path).map(|tokens| {
        tokens
            .into_iter()
            .map(|token| (token.token_hash.clone(), token))
            .collect()
    })
}

fn load_json_vec<T>(path: &Path) -> anyhow::Result<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    if !path.exists() {
        return Ok(Vec::new());
    }

    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
}

fn persist_clients(
    data_dir: &Path,
    clients: &HashMap<String, RegisteredClient>,
) -> anyhow::Result<()> {
    let mut values = clients.values().cloned().collect::<Vec<_>>();
    values.sort_by(|left, right| left.client_id.cmp(&right.client_id));
    persist_json_vec(&data_dir.join(CLIENTS_FILENAME), &values)
}

fn persist_refresh_tokens(
    data_dir: &Path,
    refresh_tokens: &HashMap<String, PersistedRefreshToken>,
) -> anyhow::Result<()> {
    let mut values = refresh_tokens.values().cloned().collect::<Vec<_>>();
    values.sort_by(|left, right| left.token_hash.cmp(&right.token_hash));
    persist_json_vec(&data_dir.join(REFRESH_TOKENS_FILENAME), &values)
}

fn persist_json_vec<T>(path: &Path, values: &[T]) -> anyhow::Result<()>
where
    T: Serialize,
{
    let payload = serde_json::to_vec_pretty(values)?;
    fs::write(path, payload).with_context(|| format!("writing {}", path.display()))
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let permissions = fs::Permissions::from_mode(0o600);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("setting permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn restrict_permissions(path: &Path) -> anyhow::Result<()> {
    let identity_output = std::process::Command::new("whoami")
        .output()
        .with_context(|| format!("resolving current Windows identity for {}", path.display()))?;
    if !identity_output.status.success() {
        return Err(anyhow!(
            "whoami failed while resolving current Windows identity"
        ));
    }

    let identity = String::from_utf8(identity_output.stdout)
        .context("decoding whoami output")?
        .trim()
        .to_owned();
    if identity.is_empty() {
        return Err(anyhow!("whoami returned an empty Windows identity"));
    }

    let icacls_output = std::process::Command::new("icacls")
        .arg(path)
        .arg("/inheritance:r")
        .arg("/grant:r")
        .arg(format!("{identity}:F"))
        .output()
        .with_context(|| format!("restricting ACLs on {}", path.display()))?;
    if !icacls_output.status.success() {
        return Err(anyhow!("icacls failed while restricting ACLs"));
    }

    Ok(())
}
