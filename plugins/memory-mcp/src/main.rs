mod oauth;

#[cfg(test)]
mod tests;

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::Context;
use axum::{
    extract::{Request, State},
    http::{
        header::{AUTHORIZATION, WWW_AUTHENTICATE},
        HeaderValue, StatusCode,
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use elegy_memory_mcp::{
    config::Config,
    memory_tools::{MemoryBinding, MemoryRepository},
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

use crate::oauth::{
    authorization_server_metadata, authorize_get, authorize_post, protected_resource_metadata,
    register_client, token, AppState, OAuthService,
};

#[derive(Clone)]
struct HttpWriteAuditor {
    oauth: Arc<OAuthService>,
}

impl WriteAuditor for HttpWriteAuditor {
    fn audit_write(
        &self,
        request_context: &RequestContext<RoleServer>,
        tool: &'static str,
        id: &str,
        memory_repository: &MemoryRepository,
    ) {
        let jti = audit_jti(request_context, &self.oauth);
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
    let bind_address = SocketAddr::from((Ipv4Addr::LOCALHOST, config.port));
    let oauth = Arc::new(OAuthService::new(config.clone()).context("initializing OAuth service")?);
    let memory_repository = Arc::new(
        MemoryRepository::new(&config.db_path, MemoryBinding::default())
            .context("initializing claude-ai-remote memory repository")?,
    );

    info!(
        admin_password_configured = !config.admin_password_verifier.is_empty(),
        port = config.port,
        bind_address = %bind_address,
        mcp_path = "/mcp",
        memory_namespace = memory_repository.namespace(),
        memory_agent_id = memory_repository.agent_id(),
        public_url = %config.public_url,
        db_path = %config.db_path.display(),
        data_dir = %config.data_dir.display(),
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
                oauth: Arc::clone(&oauth),
            },
            memory_repository,
            oauth,
            StreamableHttpServerConfig::default(),
        )
        .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("serving MCP and OAuth endpoints")?;

    Ok(())
}

fn build_router(
    state: AppState,
    memory_repository: Arc<MemoryRepository>,
    oauth: Arc<OAuthService>,
    transport_config: StreamableHttpServerConfig,
) -> Router {
    let public_routes = Router::new()
        .route(
            "/.well-known/oauth-protected-resource",
            get(protected_resource_metadata),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            get(authorization_server_metadata),
        )
        .route("/oauth/register", post(register_client))
        .route("/oauth/authorize", get(authorize_get).post(authorize_post))
        .route("/oauth/token", post(token));
    let mcp_routes = Router::new()
        .nest_service(
            "/mcp",
            build_mcp_service(memory_repository, Arc::clone(&oauth), transport_config),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_mcp_bearer,
        ));

    public_routes.merge(mcp_routes).with_state(state)
}

fn build_mcp_service(
    memory_repository: Arc<MemoryRepository>,
    oauth: Arc<OAuthService>,
    transport_config: StreamableHttpServerConfig,
) -> StreamableHttpService<ElegyMemoryMcpServer, LocalSessionManager> {
    let write_auditor: Arc<dyn WriteAuditor> = Arc::new(HttpWriteAuditor { oauth });

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

async fn require_mcp_bearer(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let token = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(extract_bearer_token);

    match token {
        Some(token) if state.oauth.validate_access_token(token).is_ok() => next.run(request).await,
        Some(_) => unauthorized_mcp_response(&state),
        _ => unauthorized_mcp_response(&state),
    }
}

fn extract_bearer_token(value: &str) -> Option<&str> {
    value
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
}

fn unauthorized_mcp_response(state: &AppState) -> Response {
    let mut response = StatusCode::UNAUTHORIZED.into_response();
    let challenge = state.oauth.mcp_bearer_challenge();
    if let Ok(value) = HeaderValue::from_str(&challenge) {
        response.headers_mut().insert(WWW_AUTHENTICATE, value);
    }
    response
}

fn audit_jti(request_context: &RequestContext<RoleServer>, oauth: &OAuthService) -> Option<String> {
    let parts = request_context
        .extensions
        .get::<axum::http::request::Parts>()?;
    let token = parts
        .headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()
        .and_then(extract_bearer_token)?;

    oauth
        .validate_access_token(token)
        .ok()
        .map(|claims| claims.jti)
}
