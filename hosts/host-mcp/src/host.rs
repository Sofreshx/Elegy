use crate::HostError;
use base64::Engine as _;
use elegy_core::{
    compose_runtime_state, CatalogResource, CoreError, ProjectLocator, ReadResourceError,
    ResourceReadResult, RuntimeState,
};
use rmcp::{
    model::*, service::ServerInitializeError, transport::stdio, ErrorData as McpError, RoleServer,
    ServerHandler, ServiceExt,
};
use serde_json::json;
use std::sync::Arc;
use tokio::task;

/// URI of the single diagnostic resource served when the host starts in
/// degraded mode (descriptor configuration failed to load).
const DIAGNOSTICS_URI: &str = "elegy://host/diagnostics";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostOptions {
    pub allow_side_effects: bool,
    pub default_tool_timeout_seconds: u64,
    pub max_tool_output_bytes: usize,
}

impl Default for HostOptions {
    fn default() -> Self {
        Self {
            allow_side_effects: false,
            default_tool_timeout_seconds: 30,
            max_tool_output_bytes: 1_048_576,
        }
    }
}

/// Runtime posture of the host: either a fully composed project, or a
/// degraded fallback carrying the diagnostics that explain why composition
/// failed. The host always serves one of these — it never dies before the
/// MCP transport opens.
enum HostState {
    Ready(Arc<RuntimeState>),
    Degraded(CoreError),
}

pub struct ElegyMcpHost {
    state: HostState,
    #[allow(dead_code)]
    options: HostOptions,
}

impl ElegyMcpHost {
    pub fn new(state: RuntimeState) -> Self {
        Self::with_options(state, HostOptions::default())
    }

    pub fn with_options(state: RuntimeState, options: HostOptions) -> Self {
        Self {
            state: HostState::Ready(Arc::new(state)),
            options,
        }
    }

    /// Build a host that serves no real resources because the project's
    /// descriptor configuration failed to load. It still answers `initialize`
    /// and exposes a single diagnostic resource describing `error`.
    pub fn degraded(error: CoreError, options: HostOptions) -> Self {
        Self {
            state: HostState::Degraded(error),
            options,
        }
    }
}

pub async fn serve_stdio(locator: ProjectLocator) -> Result<(), HostError> {
    serve_stdio_with_options(locator, HostOptions::default()).await
}

pub async fn serve_stdio_with_options(
    locator: ProjectLocator,
    options: HostOptions,
) -> Result<(), HostError> {
    let host = match compose_runtime_state(locator) {
        Ok(state) => {
            tracing::info!(
                project = %state.catalog().project_name,
                resources = state.catalog().resource_count,
                "elegy-run resolved project descriptors"
            );
            ElegyMcpHost::with_options(state, options)
        }
        Err(error) => {
            for diagnostic in error.diagnostics() {
                tracing::error!(
                    code = %diagnostic.code,
                    message = %diagnostic.message,
                    "elegy-run starting in degraded mode: descriptor configuration failed to load"
                );
            }
            ElegyMcpHost::degraded(error, options)
        }
    };

    let server = match host.serve(stdio()).await {
        Ok(server) => server,
        Err(ServerInitializeError::ConnectionClosed(reason)) => {
            // The client tore down the transport before/while initializing
            // (e.g. a host releasing a port during its own shutdown). This is
            // not a program error: log it and exit cleanly instead of
            // propagating a Debug-formatted panic-shaped error.
            tracing::info!(%reason, "client closed the connection during initialize; exiting");
            return Ok(());
        }
        Err(other) => return Err(other.into()),
    };
    server.waiting().await?;
    Ok(())
}

impl ServerHandler for ElegyMcpHost {
    fn get_info(&self) -> ServerInfo {
        let instructions = match &self.state {
            HostState::Ready(_) => "Elegy exposes runtime-composed MCP resources over stdio. \
                 This host supports resources/list and resources/read."
                .to_string(),
            HostState::Degraded(error) => degraded_instructions(error),
        };
        ServerInfo::new(ServerCapabilities::builder().enable_resources().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions(instructions)
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let resources = match &self.state {
            HostState::Ready(state) => state
                .catalog()
                .resources
                .iter()
                .map(resource_to_mcp)
                .collect(),
            HostState::Degraded(_) => vec![diagnostics_resource_descriptor()],
        };
        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
            meta: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: Vec::new(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri = request.uri;
        let state = match &self.state {
            HostState::Ready(state) => Arc::clone(state),
            HostState::Degraded(error) => {
                return if uri == DIAGNOSTICS_URI {
                    Ok(ReadResourceResult::new(vec![
                        diagnostics_resource_contents(error),
                    ]))
                } else {
                    Err(McpError::resource_not_found(
                        "resource_not_found",
                        Some(
                            json!({ "uri": uri, "reason": "host is running in degraded mode; only elegy://host/diagnostics is available" }),
                        ),
                    ))
                };
            }
        };
        let read_uri = uri.clone();
        let result = task::spawn_blocking(move || state.read_resource(read_uri.as_str()))
            .await
            .map_err(|_| {
                McpError::internal_error("resource read task failed", Some(json!({ "uri": uri })))
            })?;
        match result {
            Ok(result) => Ok(ReadResourceResult::new(vec![
                resource_contents_from_read_result(result),
            ])),
            Err(error) => Err(map_read_error(uri.as_str(), error)),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: Vec::new(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Err(McpError::internal_error(
            "MCP tool projection has been removed; use direct CLI invocation instead",
            Some(json!({"tool": request.name})),
        ))
    }
}

/// Human-readable explanation, appended to `get_info().instructions`, for why
/// the host is running with no real resources.
fn degraded_instructions(error: &CoreError) -> String {
    let mut text = String::from(
        "Elegy MCP host started in DEGRADED mode: this project's descriptor \
         configuration failed to load, so no real resources are served. \
         Read the elegy://host/diagnostics resource for the exact \
         diagnostic(s), fix the referenced elegy.toml/descriptor file, then \
         restart this host.",
    );
    for diagnostic in error.diagnostics() {
        text.push_str(&format!("\n- [{}] {}", diagnostic.code, diagnostic.message));
    }
    text
}

/// The single resource entry exposed by `resources/list` in degraded mode.
fn diagnostics_resource_descriptor() -> Resource {
    RawResource::new(DIAGNOSTICS_URI, "Elegy Host Diagnostics")
        .with_mime_type("application/json")
        .with_description(
            "Structured diagnostics explaining why this host started in degraded mode.",
        )
        .no_annotation()
}

/// The body returned by `resources/read` for `elegy://host/diagnostics`.
fn diagnostics_resource_contents(error: &CoreError) -> ResourceContents {
    let payload = json!({
        "status": "degraded",
        "diagnostics": error.diagnostics(),
    });
    ResourceContents::text(payload.to_string(), DIAGNOSTICS_URI).with_mime_type("application/json")
}

fn resource_to_mcp(resource: &CatalogResource) -> Resource {
    let mut raw = RawResource::new(
        resource.uri.clone(),
        resource
            .title
            .clone()
            .unwrap_or_else(|| resource.id.clone()),
    )
    .with_mime_type(resource.mime_type.clone());

    if let Some(title) = &resource.title {
        raw = raw.with_title(title.clone());
    }
    if let Some(description) = &resource.description {
        raw = raw.with_description(description.clone());
    }
    if let Ok(size) = u32::try_from(resource.limits.max_size_bytes) {
        raw = raw.with_size(size);
    }

    raw.no_annotation()
}

fn resource_contents_from_read_result(result: ResourceReadResult) -> ResourceContents {
    if mime_type_is_textual(&result.mime_type) {
        match String::from_utf8(result.bytes) {
            Ok(text) => ResourceContents::text(text, result.uri).with_mime_type(result.mime_type),
            Err(error) => ResourceContents::blob(
                base64::engine::general_purpose::STANDARD.encode(error.into_bytes()),
                result.uri,
            )
            .with_mime_type(result.mime_type),
        }
    } else {
        ResourceContents::blob(
            base64::engine::general_purpose::STANDARD.encode(result.bytes),
            result.uri,
        )
        .with_mime_type(result.mime_type)
    }
}

fn mime_type_is_textual(mime_type: &str) -> bool {
    let essence = mime_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    essence.starts_with("text/")
        || matches!(
            essence.as_str(),
            "application/json"
                | "application/xml"
                | "application/yaml"
                | "application/x-yaml"
                | "application/javascript"
                | "application/ecmascript"
                | "image/svg+xml"
        )
        || essence.ends_with("+json")
        || essence.ends_with("+xml")
}

fn map_read_error(uri: &str, error: ReadResourceError) -> McpError {
    match error {
        ReadResourceError::UnknownResource { .. } => {
            McpError::resource_not_found("resource_not_found", Some(json!({ "uri": uri })))
        }
        other => McpError::internal_error(
            read_error_message(&other),
            Some(json!({
                "uri": uri,
            })),
        ),
    }
}

fn read_error_message(error: &ReadResourceError) -> &'static str {
    match error {
        ReadResourceError::AccessDenied { .. } => "resource access denied",
        ReadResourceError::InvalidResourceState { .. } => "resource state is invalid",
        ReadResourceError::Io { .. } => "resource read failed",
        ReadResourceError::Http(_) => "resource HTTP read failed",
        ReadResourceError::NotYetSupported { .. } => {
            "resource family is not supported by this host"
        }
        ReadResourceError::UnknownResource { .. } => "resource not found",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elegy_core::ProjectLocator;
    use rmcp::{ClientHandler, ServiceExt};
    use std::path::PathBuf;

    #[derive(Default, Clone)]
    struct TestClient;

    impl ClientHandler for TestClient {}

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn read_result_uses_blob_for_binary_mime_even_when_bytes_are_utf8() {
        let result = resource_contents_from_read_result(ResourceReadResult {
            uri: "elegy://tests/resource/blob".to_string(),
            mime_type: "application/octet-stream".to_string(),
            bytes: b"plain text bytes".to_vec(),
            http_response: None,
        });

        assert_eq!(
            result,
            ResourceContents::blob(
                base64::engine::general_purpose::STANDARD.encode("plain text bytes"),
                "elegy://tests/resource/blob",
            )
            .with_mime_type("application/octet-stream")
        );
    }

    #[test]
    fn read_result_uses_text_for_utf8_json_payloads() {
        let result = resource_contents_from_read_result(ResourceReadResult {
            uri: "elegy://tests/resource/json".to_string(),
            mime_type: "application/json".to_string(),
            bytes: br#"{"status":"ok"}"#.to_vec(),
            http_response: None,
        });

        assert_eq!(
            result,
            ResourceContents::text(r#"{"status":"ok"}"#, "elegy://tests/resource/json")
                .with_mime_type("application/json")
        );
    }

    #[tokio::test]
    async fn host_lists_supported_resources_over_duplex_transport() {
        let state = compose_runtime_state(ProjectLocator::Path(
            repo_root().join("examples/http-minimal"),
        ))
        .expect("example runtime should compose");
        let server = ElegyMcpHost::new(state);
        let client = TestClient;
        let (server_transport, client_transport) = tokio::io::duplex(4096);

        let server_task = tokio::spawn(async move {
            let service = server
                .serve(server_transport)
                .await
                .expect("server should initialize");
            service.waiting().await.expect("server should run cleanly");
        });

        let client_service = client
            .serve(client_transport)
            .await
            .expect("client should initialize");
        let resources = client_service
            .list_all_resources()
            .await
            .expect("client should list resources");

        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].uri, "elegy://http-minimal/resource/status");
        assert_eq!(
            resources[0].mime_type.as_deref(),
            Some("application/octet-stream")
        );

        client_service.cancel().await.expect("client should cancel");
        server_task.await.expect("server task should join");
    }

    #[tokio::test]
    async fn host_reads_static_resource_over_duplex_transport() {
        let state = compose_runtime_state(ProjectLocator::Path(
            repo_root().join("examples/fs-static-minimal"),
        ))
        .expect("example runtime should compose");
        let server = ElegyMcpHost::new(state);
        let client = TestClient;
        let (server_transport, client_transport) = tokio::io::duplex(4096);

        let server_task = tokio::spawn(async move {
            let service = server
                .serve(server_transport)
                .await
                .expect("server should initialize");
            service.waiting().await.expect("server should run cleanly");
        });

        let client_service = client
            .serve(client_transport)
            .await
            .expect("client should initialize");
        let result = client_service
            .read_resource(ReadResourceRequestParams::new(
                "elegy://fs-static-minimal/resource/welcome",
            ))
            .await
            .expect("client should read static resource");

        assert_eq!(result.contents.len(), 1);
        assert_eq!(
            result.contents[0],
            ResourceContents::text(
                "Hello from Elegy.\n",
                "elegy://fs-static-minimal/resource/welcome"
            )
            .with_mime_type("text/plain; charset=utf-8")
        );

        client_service.cancel().await.expect("client should cancel");
        server_task.await.expect("server task should join");
    }
}
