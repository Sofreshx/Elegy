mod config;
mod memory_tools;
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
use memory_tools::{
    map_store_error, parse_tool_arguments, ClaudeRemoteMemoryRepository, MemoryCorrectArgs,
    MemoryCorrectResponse, MemoryDeleteArgs, MemoryDeleteResponse, MemoryListArgs,
    MemoryListResponse, MemoryRecallArgs, MemoryRecallResponse, MemorySearchArgs,
    MemorySearchResponse, MemoryStatsArgs, MemoryStatsResponse, MemoryStoreArgs,
    MemoryStoreResponse, MemoryUpdateArgs, MemoryUpdateResponse,
};
use rmcp::{
    handler::server::router::tool::ToolRouter,
    model::{Implementation, ServerCapabilities, ServerInfo},
    service::RequestContext,
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    },
    Json, RoleServer, ServerHandler,
};
use tokio::net::TcpListener;
use tracing::{error, info};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::oauth::{
    authorization_server_metadata, authorize_get, authorize_post, protected_resource_metadata,
    register_client, token, AppState, OAuthService,
};

#[derive(Clone)]
struct ElegyMemoryMcpServer {
    memory_repository: Arc<ClaudeRemoteMemoryRepository>,
    oauth: Arc<OAuthService>,
    tool_router: ToolRouter<Self>,
}

impl ElegyMemoryMcpServer {
    fn new(memory_repository: Arc<ClaudeRemoteMemoryRepository>, oauth: Arc<OAuthService>) -> Self {
        Self {
            memory_repository,
            oauth,
            tool_router: Self::tool_router(),
        }
    }

    fn audit_jti(&self, request_context: &RequestContext<RoleServer>) -> Option<String> {
        let parts = request_context
            .extensions
            .get::<axum::http::request::Parts>()?;
        let token = parts
            .headers
            .get(AUTHORIZATION)?
            .to_str()
            .ok()
            .and_then(extract_bearer_token)?;

        self.oauth
            .validate_access_token(token)
            .ok()
            .map(|claims| claims.jti)
    }
}

#[tool_router]
impl ElegyMemoryMcpServer {
    #[tool(
        name = "memory_search",
        description = "Search memories inside the fixed claude-ai-remote namespace",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemorySearchArgs>()
    )]
    async fn memory_search(
        &self,
        raw_arguments: rmcp::model::JsonObject,
    ) -> Result<Json<MemorySearchResponse>, rmcp::ErrorData> {
        let args = parse_tool_arguments::<MemorySearchArgs>(raw_arguments)?;
        let matches = self
            .memory_repository
            .search(&args)
            .await
            .map_err(map_store_error)?;
        Ok(Json(MemorySearchResponse::new(&args, matches)))
    }

    #[tool(
        name = "memory_recall",
        description = "Recall a single memory by id inside the fixed claude-ai-remote namespace",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemoryRecallArgs>()
    )]
    async fn memory_recall(
        &self,
        raw_arguments: rmcp::model::JsonObject,
    ) -> Result<Json<MemoryRecallResponse>, rmcp::ErrorData> {
        let args = parse_tool_arguments::<MemoryRecallArgs>(raw_arguments)?;
        let memory = self
            .memory_repository
            .recall(&args.id)
            .await
            .map_err(map_store_error)?;
        Ok(Json(MemoryRecallResponse::from_memory(memory)))
    }

    #[tool(
        name = "memory_list",
        description = "List memories inside the fixed claude-ai-remote namespace",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemoryListArgs>()
    )]
    async fn memory_list(
        &self,
        raw_arguments: rmcp::model::JsonObject,
    ) -> Result<Json<MemoryListResponse>, rmcp::ErrorData> {
        let args = parse_tool_arguments::<MemoryListArgs>(raw_arguments)?;
        let memories = self
            .memory_repository
            .list(&args)
            .await
            .map_err(map_store_error)?;
        Ok(Json(MemoryListResponse::new(&args, memories)))
    }

    #[tool(
        name = "memory_stats",
        description = "Report namespace-local memory stats for the fixed claude-ai-remote namespace",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemoryStatsArgs>()
    )]
    async fn memory_stats(
        &self,
        raw_arguments: rmcp::model::JsonObject,
    ) -> Result<Json<MemoryStatsResponse>, rmcp::ErrorData> {
        let _: MemoryStatsArgs = parse_tool_arguments(raw_arguments)?;
        let stats = self
            .memory_repository
            .stats()
            .await
            .map_err(map_store_error)?;
        Ok(Json(MemoryStatsResponse::from(stats)))
    }

    #[tool(
        name = "memory_store",
        description = "Store a memory inside the fixed claude-ai-remote namespace",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemoryStoreArgs>()
    )]
    async fn memory_store(
        &self,
        raw_arguments: rmcp::model::JsonObject,
        request_context: RequestContext<RoleServer>,
    ) -> Result<Json<MemoryStoreResponse>, rmcp::ErrorData> {
        let args = parse_tool_arguments::<MemoryStoreArgs>(raw_arguments)?;
        let response = self
            .memory_repository
            .store_memory(&args)
            .await
            .map_err(map_store_error)?;
        let jti = self.audit_jti(&request_context);
        audit_write("memory_store", &response.memory.id, jti.as_deref());
        Ok(Json(response))
    }

    #[tool(
        name = "memory_update",
        description = "Update an existing memory inside the fixed claude-ai-remote namespace",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemoryUpdateArgs>()
    )]
    async fn memory_update(
        &self,
        raw_arguments: rmcp::model::JsonObject,
        request_context: RequestContext<RoleServer>,
    ) -> Result<Json<MemoryUpdateResponse>, rmcp::ErrorData> {
        let args = parse_tool_arguments::<MemoryUpdateArgs>(raw_arguments)?;
        let response = self
            .memory_repository
            .update_memory(&args)
            .await
            .map_err(map_store_error)?;
        let jti = self.audit_jti(&request_context);
        audit_write("memory_update", &response.memory.id, jti.as_deref());
        Ok(Json(response))
    }

    #[tool(
        name = "memory_correct",
        description = "Correct a memory through the underlying gate-aware correction path",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemoryCorrectArgs>()
    )]
    async fn memory_correct(
        &self,
        raw_arguments: rmcp::model::JsonObject,
        request_context: RequestContext<RoleServer>,
    ) -> Result<Json<MemoryCorrectResponse>, rmcp::ErrorData> {
        let args = parse_tool_arguments::<MemoryCorrectArgs>(raw_arguments)?;
        let response = self
            .memory_repository
            .correct_memory(&args)
            .await
            .map_err(map_store_error)?;
        let jti = self.audit_jti(&request_context);
        audit_write("memory_correct", &response.memory.id, jti.as_deref());
        Ok(Json(response))
    }

    #[tool(
        name = "memory_delete",
        description = "Delete a memory inside the fixed claude-ai-remote namespace",
        input_schema = rmcp::handler::server::tool::schema_for_type::<MemoryDeleteArgs>()
    )]
    async fn memory_delete(
        &self,
        raw_arguments: rmcp::model::JsonObject,
        request_context: RequestContext<RoleServer>,
    ) -> Result<Json<MemoryDeleteResponse>, rmcp::ErrorData> {
        let args = parse_tool_arguments::<MemoryDeleteArgs>(raw_arguments)?;
        let response = self
            .memory_repository
            .delete_memory(&args)
            .await
            .map_err(map_store_error)?;
        let jti = self.audit_jti(&request_context);
        audit_write("memory_delete", &response.id, jti.as_deref());
        Ok(Json(response))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ElegyMemoryMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
                    .with_title("Elegy Memory MCP")
                    .with_description("OAuth-protected MCP transport endpoint for Elegy Memory."),
            )
            .with_instructions(
                "This server requires a valid claude-ai-remote bearer token on /mcp and exposes read/write memory tools inside the fixed claude-ai-remote namespace.",
            )
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
    let oauth = OAuthService::new(config.clone()).context("initializing OAuth service")?;
    let memory_repository = Arc::new(
        ClaudeRemoteMemoryRepository::new(&config.db_path)
            .context("initializing claude-ai-remote memory repository")?,
    );
    let state = AppState {
        oauth: Arc::new(oauth),
        memory_repository,
    };

    info!(
        admin_password_configured = !config.admin_password_verifier.is_empty(),
        port = config.port,
        bind_address = %bind_address,
        mcp_path = "/mcp",
        memory_namespace = memory_tools::FIXED_NAMESPACE,
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
        build_router(state, StreamableHttpServerConfig::default())
            .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("serving MCP and OAuth endpoints")?;

    Ok(())
}

fn build_router(state: AppState, transport_config: StreamableHttpServerConfig) -> Router {
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
            build_mcp_service(
                Arc::clone(&state.memory_repository),
                Arc::clone(&state.oauth),
                transport_config,
            ),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_mcp_bearer,
        ));

    public_routes.merge(mcp_routes).with_state(state)
}

fn build_mcp_service(
    memory_repository: Arc<ClaudeRemoteMemoryRepository>,
    oauth: Arc<OAuthService>,
    transport_config: StreamableHttpServerConfig,
) -> StreamableHttpService<ElegyMemoryMcpServer, LocalSessionManager> {
    StreamableHttpService::new(
        move || {
            Ok(ElegyMemoryMcpServer::new(
                Arc::clone(&memory_repository),
                Arc::clone(&oauth),
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

fn audit_write(tool: &'static str, id: &str, jti: Option<&str>) {
    let timestamp = time::OffsetDateTime::now_utc().unix_timestamp();
    info!(
        tool,
        id,
        scope = memory_tools::FIXED_NAMESPACE,
        timestamp,
        jti = jti.unwrap_or_default(),
        "memory write audit"
    );
}
