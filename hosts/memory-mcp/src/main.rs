#[cfg(test)]
mod tests;

use std::{net::SocketAddr, sync::Arc};

use anyhow::Context;
use axum::{
    extract::{Request, State},
    http::{
        header::{AUTHORIZATION, WWW_AUTHENTICATE},
        HeaderValue, StatusCode,
    },
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use elegy_memory_mcp::{
    config::{AuthMode, Config},
    memory_tools::{MemoryBinding, MemoryRepository},
    resource_auth::{ExternalTokenValidator, ValidatedTokenClaims},
    server::{ElegyMemoryMcpServer, WriteAuditor},
};
use rmcp::{
    service::RequestContext,
    transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    },
    RoleServer,
};
use tokio::net::TcpListener;
use tracing::{error, info};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};
use url::Url;

#[derive(Clone)]
enum HttpAuth {
    LocalNone,
    External(Arc<ExternalTokenValidator>),
}

#[derive(Clone)]
struct AppState {
    auth: HttpAuth,
    public_url: Url,
}

#[derive(Clone, Default)]
struct HttpWriteAuditor;

impl WriteAuditor for HttpWriteAuditor {
    fn audit_write(
        &self,
        request_context: &RequestContext<RoleServer>,
        tool: &'static str,
        id: &str,
        memory_repository: &MemoryRepository,
    ) {
        let jti = request_context
            .extensions
            .get::<axum::http::request::Parts>()
            .and_then(|parts| parts.extensions.get::<ValidatedTokenClaims>())
            .and_then(|claims| claims.jti.as_deref());
        let timestamp = time::OffsetDateTime::now_utc().unix_timestamp();
        info!(
            tool,
            id,
            scope = memory_repository.namespace(),
            agent_id = memory_repository.agent_id(),
            timestamp,
            jti = jti.unwrap_or_default(),
            "memory write audit"
        );
    }
}

#[tokio::main]
async fn main() {
    init_logging();

    if let Err(error) = run().await {
        let error_message = format!("{error:#}");
        error!(error = %error_message, "startup failed");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let config = Config::from_env().context("loading startup configuration")?;
    let bind_address = SocketAddr::from((config.bind_ip, config.port));
    let auth = match config.auth {
        AuthMode::LocalNone => HttpAuth::LocalNone,
        AuthMode::ExternalOAuth(external) => HttpAuth::External(Arc::new(
            ExternalTokenValidator::load(external)
                .await
                .context("loading external identity-provider JWKS")?,
        )),
    };
    let auth_mode = match auth {
        HttpAuth::LocalNone => "local-none",
        HttpAuth::External(_) => "external-oauth",
    };
    let memory_repository = Arc::new(
        MemoryRepository::new(&config.db_path, MemoryBinding::default())
            .context("initializing claude-ai-remote memory repository")?,
    );

    info!(
        auth_mode,
        port = config.port,
        bind_address = %bind_address,
        mcp_path = "/mcp",
        memory_namespace = memory_repository.namespace(),
        memory_agent_id = memory_repository.agent_id(),
        public_url = %config.public_url,
        db_path = %config.db_path.display(),
        log_content = config.log_content,
        "elegy-memory-mcp starting"
    );

    let listener = TcpListener::bind(bind_address)
        .await
        .with_context(|| format!("binding elegy-memory-mcp to {bind_address}"))?;

    axum::serve(
        listener,
        build_router(
            AppState {
                auth,
                public_url: config.public_url,
            },
            memory_repository,
            StreamableHttpServerConfig::default(),
        )
        .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("serving MCP resource endpoint")?;

    Ok(())
}

fn build_router(
    state: AppState,
    memory_repository: Arc<MemoryRepository>,
    transport_config: StreamableHttpServerConfig,
) -> Router {
    let mcp_routes = Router::new()
        .nest_service(
            "/mcp",
            build_mcp_service(memory_repository, transport_config),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_mcp_bearer,
        ));

    let router = match state.auth {
        HttpAuth::LocalNone => Router::new(),
        HttpAuth::External(_) => Router::new().route(
            "/.well-known/oauth-protected-resource",
            get(protected_resource_metadata),
        ),
    };
    router.merge(mcp_routes).with_state(state)
}

fn build_mcp_service(
    memory_repository: Arc<MemoryRepository>,
    transport_config: StreamableHttpServerConfig,
) -> StreamableHttpService<ElegyMemoryMcpServer, LocalSessionManager> {
    let write_auditor: Arc<dyn WriteAuditor> = Arc::new(HttpWriteAuditor);

    StreamableHttpService::new(
        move || {
            Ok(ElegyMemoryMcpServer::new(
                Arc::clone(&memory_repository),
                Arc::clone(&write_auditor),
            ))
        },
        Default::default(),
        transport_config,
    )
}

fn init_logging() {
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .json()
                .with_writer(std::io::stdout)
                .with_ansi(false)
                .with_current_span(false)
                .with_span_list(false),
        )
        .init();
}

async fn protected_resource_metadata(State(state): State<AppState>) -> Response {
    match state.auth {
        HttpAuth::External(validator) => {
            Json(validator.protected_resource_metadata(&state.public_url)).into_response()
        }
        HttpAuth::LocalNone => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn require_mcp_bearer(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let HttpAuth::External(validator) = &state.auth else {
        return next.run(request).await;
    };
    let token = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(extract_bearer_token)
        .map(str::to_owned);

    let claims = match token {
        Some(token) => validator.validate_with_refresh(&token).await.ok(),
        None => None,
    };
    match claims {
        Some(claims) => {
            request.extensions_mut().insert(claims);
            next.run(request).await
        }
        None => unauthorized_mcp_response(&state, validator),
    }
}

fn extract_bearer_token(value: &str) -> Option<&str> {
    let (scheme, token) = value.trim().split_once(char::is_whitespace)?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then_some(token.trim())
        .filter(|token| !token.is_empty() && !token.chars().any(char::is_whitespace))
}

#[cfg(test)]
mod auth_header_tests {
    use super::extract_bearer_token;

    #[test]
    fn bearer_scheme_is_case_insensitive_but_token_shape_is_strict() {
        assert_eq!(extract_bearer_token("bearer token"), Some("token"));
        assert_eq!(extract_bearer_token("  BEARER   token  "), Some("token"));
        assert_eq!(extract_bearer_token("Basic token"), None);
        assert_eq!(extract_bearer_token("Bearer"), None);
        assert_eq!(extract_bearer_token("Bearer token extra"), None);
    }
}

fn unauthorized_mcp_response(state: &AppState, validator: &ExternalTokenValidator) -> Response {
    let mut response = StatusCode::UNAUTHORIZED.into_response();
    let challenge = validator.bearer_challenge(&state.public_url);
    if let Ok(value) = HeaderValue::from_str(&challenge) {
        response.headers_mut().insert(WWW_AUTHENTICATE, value);
    }
    response
}
