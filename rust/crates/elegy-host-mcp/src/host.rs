use crate::HostError;
use base64::Engine as _;
use elegy_core::{
    compose_runtime_state, CatalogResource, ProjectLocator, ReadResourceError, ResourceReadResult,
    RuntimeState,
};
use rmcp::{
    model::*, transport::stdio, ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
};
use serde_json::json;
use std::sync::{Arc, OnceLock};
use tokio::task;

// ---------------------------------------------------------------------------
// Embedded v2 skill definitions (compile-time)
// ---------------------------------------------------------------------------
const EMBEDDED_SKILL_DEFINITIONS: &[(&str, &str)] = &[(
    "diagram",
    include_str!("../../../../contracts/fixtures/skill-definition-v2.elegy-diagram.json"),
)];

// ---------------------------------------------------------------------------
// Cached tool list built from the embedded skill definitions
// ---------------------------------------------------------------------------
fn cached_tools() -> Vec<Tool> {
    static TOOLS: OnceLock<Vec<Tool>> = OnceLock::new();
    TOOLS
        .get_or_init(build_tools_from_skill_definitions)
        .clone()
}

/// Parse every embedded v2 skill-definition JSON and convert each capability
/// entry into an rmcp `Tool`.
fn build_tools_from_skill_definitions() -> Vec<Tool> {
    let mut tools = Vec::new();

    for &(_skill_name, json_text) in EMBEDDED_SKILL_DEFINITIONS {
        let def: serde_json::Value = match serde_json::from_str(json_text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let capabilities = match def.get("capabilities").and_then(|c| c.as_array()) {
            Some(arr) => arr,
            None => continue,
        };

        for cap in capabilities {
            let id = match cap.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            let description = cap
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            // Build JSON-Schema input_schema from parameters
            let input_schema = build_input_schema(cap);

            // Build annotations from execution metadata
            let annotations = build_annotations(cap);

            let mut tool = Tool::new(id, description, input_schema);
            tool.annotations = Some(annotations);
            tools.push(tool);
        }
    }

    tools
}

/// Construct a JSON-Schema `{ "type": "object", "properties": {...}, "required": [...] }`
/// from the capability's `input.parameters` array.
fn build_input_schema(capability: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    let mut properties = serde_json::Map::new();
    let mut required: Vec<serde_json::Value> = Vec::new();

    if let Some(params) = capability
        .get("input")
        .and_then(|i| i.get("parameters"))
        .and_then(|p| p.as_array())
    {
        for param in params {
            let name = match param.get("name").and_then(|n| n.as_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let mut prop = serde_json::Map::new();

            // Map skill-definition type strings to JSON-Schema types
            let param_type = param
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("string");
            let schema_type = match param_type {
                "boolean" => "boolean",
                "integer" | "number" => param_type,
                // path, path-or-stdin, string, and anything else map to string
                _ => "string",
            };
            prop.insert("type".to_string(), json!(schema_type));

            if let Some(desc) = param.get("description").and_then(|d| d.as_str()) {
                prop.insert("description".to_string(), json!(desc));
            }
            if let Some(default) = param.get("default") {
                prop.insert("default".to_string(), default.clone());
            }

            if param.get("required").and_then(|r| r.as_bool()).unwrap_or(false) {
                required.push(json!(name));
            }

            properties.insert(name, serde_json::Value::Object(prop));
        }
    }

    // Also expose a synthetic `stdin` parameter when the capability declares stdinFormat
    if let Some(stdin_fmt) = capability
        .get("input")
        .and_then(|i| i.get("stdinFormat"))
        .and_then(|f| f.as_str())
    {
        let mut prop = serde_json::Map::new();
        prop.insert("type".to_string(), json!("string"));
        prop.insert(
            "description".to_string(),
            json!(format!(
                "Data to pipe to the process via stdin (format: {stdin_fmt})."
            )),
        );
        properties.insert("stdin".to_string(), serde_json::Value::Object(prop));
    }

    let mut schema = serde_json::Map::new();
    schema.insert("type".to_string(), json!("object"));
    schema.insert(
        "properties".to_string(),
        serde_json::Value::Object(properties),
    );
    if !required.is_empty() {
        schema.insert("required".to_string(), serde_json::Value::Array(required));
    }
    schema
}

/// Derive `ToolAnnotations` from the capability's `execution` block.
fn build_annotations(capability: &serde_json::Value) -> ToolAnnotations {
    let exec = capability.get("execution");

    let has_side_effects = exec
        .and_then(|e| e.get("hasSideEffects"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let is_deterministic = exec
        .and_then(|e| e.get("isDeterministic"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    ToolAnnotations::new()
        .read_only(!has_side_effects)
        .destructive(false)
        .idempotent(is_deterministic)
        .open_world(false)
}

// ---------------------------------------------------------------------------
// Capability lookup + argument substitution helpers (used by call_tool)
// ---------------------------------------------------------------------------

/// Locate the raw capability JSON and its parent skill definition for a given
/// tool name (= capability id).
fn find_capability(tool_name: &str) -> Option<(serde_json::Value, serde_json::Value)> {
    for &(_skill_name, json_text) in EMBEDDED_SKILL_DEFINITIONS {
        let def: serde_json::Value = match serde_json::from_str(json_text) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(caps) = def.get("capabilities").and_then(|c| c.as_array()) {
            for cap in caps {
                if cap.get("id").and_then(|v| v.as_str()) == Some(tool_name) {
                    return Some((cap.clone(), def.clone()));
                }
            }
        }
    }
    None
}

/// Build the final CLI argument vector, correctly dropping `--flag ${param}`
/// pairs when the parameter is not supplied by the caller.
fn build_cli_arguments(
    template_args: &[serde_json::Value],
    params: &serde_json::Map<String, serde_json::Value>,
) -> Vec<String> {
    let mut result = Vec::new();
    let mut skip_next = false;

    for (i, arg) in template_args.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }
        let s = arg.as_str().unwrap_or_default();

        // Look ahead: if the *next* element is a placeholder, decide now
        if let Some(next) = template_args.get(i + 1) {
            let next_s = next.as_str().unwrap_or_default();
            if next_s.starts_with("${") && next_s.ends_with('}') {
                let key = &next_s[2..next_s.len() - 1];
                if let Some(val) = params.get(key) {
                    result.push(s.to_string());
                    result.push(value_as_string(val));
                    skip_next = true;
                    continue;
                }
                // Parameter not provided — skip both the flag and placeholder
                skip_next = true;
                continue;
            }
        }

        // Plain argument (e.g. subcommand token, `--json`, `--patch-stdin`)
        result.push(s.to_string());
    }

    result
}

fn value_as_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Resolve the path for an executable name.  If it matches the current binary
/// stem, returns the current exe path so the MCP host can self-dispatch even
/// when the binary is not on `PATH`.
fn which_executable(name: &str) -> String {
    // Fast path: if the name is the same as our own binary, use current_exe
    if let Ok(current) = std::env::current_exe() {
        let stem = current
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if stem == name {
            return current.to_string_lossy().into_owned();
        }
    }
    // Otherwise assume the executable is on PATH
    name.to_string()
}

pub struct ElegyMcpHost {
    state: Arc<RuntimeState>,
}

impl ElegyMcpHost {
    pub fn new(state: RuntimeState) -> Self {
        Self {
            state: Arc::new(state),
        }
    }
}

pub async fn serve_stdio(locator: ProjectLocator) -> Result<(), HostError> {
    let state = compose_runtime_state(locator)?;
    let server = ElegyMcpHost::new(state).serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}

impl ServerHandler for ElegyMcpHost {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_resources()
                .enable_tools()
                .build(),
        )
        .with_server_info(Implementation::from_build_env())
        .with_instructions(
            "Elegy exposes runtime-composed MCP resources and skill-backed tools over stdio. \
             This host supports resources/list, resources/read, tools/list, and tools/call."
                .to_string(),
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: self
                .state
                .catalog()
                .resources
                .iter()
                .map(resource_to_mcp)
                .collect(),
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
        let state = Arc::clone(&self.state);
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
        let tools = cached_tools();
        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let tool_name = request.name.as_ref();
        let arguments = request.arguments.unwrap_or_default();

        // 1. Find the matching capability
        let (capability, _def) = find_capability(tool_name).ok_or_else(|| {
            McpError::invalid_params(
                format!("unknown tool: {tool_name}"),
                Some(json!({ "tool": tool_name })),
            )
        })?;

        // 2. Extract implementation details
        let impl_block = capability.get("implementation").ok_or_else(|| {
            McpError::internal_error(
                "capability has no implementation block",
                Some(json!({ "tool": tool_name })),
            )
        })?;

        let executable = impl_block
            .get("executableName")
            .and_then(|v| v.as_str())
            .unwrap_or("elegy");

        let template_args = impl_block
            .get("arguments")
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();

        // 3. Build CLI arguments with placeholder substitution
        let cli_args = build_cli_arguments(&template_args, &arguments);

        // 4. Detect whether stdin piping is required
        let stdin_data = capability
            .get("input")
            .and_then(|i| i.get("stdinFormat"))
            .and_then(|_| arguments.get("stdin"))
            .map(value_as_string);

        // 5. Resolve the executable — try PATH first, fall back to current_exe
        let exe_path = which_executable(executable);

        // 6. Spawn the subprocess
        let mut cmd = tokio::process::Command::new(&exe_path);
        cmd.args(&cli_args);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        if stdin_data.is_some() {
            cmd.stdin(std::process::Stdio::piped());
        } else {
            cmd.stdin(std::process::Stdio::null());
        }

        let mut child = cmd.spawn().map_err(|e| {
            McpError::internal_error(
                format!("failed to spawn {exe_path}: {e}"),
                Some(json!({ "tool": tool_name })),
            )
        })?;

        // 7. Pipe stdin if needed
        if let Some(data) = stdin_data {
            if let Some(mut stdin_handle) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                let _ = stdin_handle.write_all(data.as_bytes()).await;
                drop(stdin_handle);
            }
        }

        // 8. Await completion
        let output = child.wait_with_output().await.map_err(|e| {
            McpError::internal_error(
                format!("subprocess I/O error: {e}"),
                Some(json!({ "tool": tool_name })),
            )
        })?;

        // 9. Return result
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if output.status.success() {
            Ok(CallToolResult::success(vec![Content::text(stdout)]))
        } else {
            let message = if stderr.is_empty() {
                format!("tool {tool_name} failed with exit code {}", output.status)
            } else {
                stderr
            };
            Ok(CallToolResult::error(vec![Content::text(message)]))
        }
    }
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

    // -----------------------------------------------------------------------
    // Tool-related tests
    // -----------------------------------------------------------------------

    #[test]
    fn build_tools_parses_expected_capabilities() {
        let tools = build_tools_from_skill_definitions();

        // The diagram skill definition has 4 capabilities
        assert_eq!(tools.len(), 4, "expected 4 tools from diagram skill def");

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"diagram-create"), "missing diagram-create");
        assert!(names.contains(&"diagram-patch"), "missing diagram-patch");
        assert!(names.contains(&"diagram-narrate"), "missing diagram-narrate");
        assert!(names.contains(&"diagram-render"), "missing diagram-render");
    }

    #[test]
    fn build_tools_includes_correct_input_schema() {
        let tools = build_tools_from_skill_definitions();

        let create_tool = tools
            .iter()
            .find(|t| t.name.as_ref() == "diagram-create")
            .expect("diagram-create tool should exist");

        let schema = &*create_tool.input_schema;
        assert_eq!(schema.get("type").and_then(|v| v.as_str()), Some("object"));

        let props = schema
            .get("properties")
            .and_then(|v| v.as_object())
            .expect("schema should have properties");
        assert!(props.contains_key("type"), "should have 'type' property");
    }

    #[test]
    fn build_tools_marks_required_params() {
        let tools = build_tools_from_skill_definitions();

        let patch_tool = tools
            .iter()
            .find(|t| t.name.as_ref() == "diagram-patch")
            .expect("diagram-patch tool should exist");

        let schema = &*patch_tool.input_schema;
        let required = schema
            .get("required")
            .and_then(|v| v.as_array())
            .expect("patch schema should have required array");
        let required_strs: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            required_strs.contains(&"inputPath"),
            "inputPath should be required"
        );
    }

    #[test]
    fn build_tools_adds_stdin_synthetic_param_for_stdin_capable_tools() {
        let tools = build_tools_from_skill_definitions();

        let patch_tool = tools
            .iter()
            .find(|t| t.name.as_ref() == "diagram-patch")
            .expect("diagram-patch tool should exist");

        let props = patch_tool
            .input_schema
            .get("properties")
            .and_then(|v| v.as_object())
            .expect("schema should have properties");
        assert!(
            props.contains_key("stdin"),
            "patch tool should expose synthetic stdin param"
        );
    }

    #[test]
    fn build_tools_annotations_reflect_execution_metadata() {
        let tools = build_tools_from_skill_definitions();

        // diagram-create: no side effects, deterministic
        let create = tools
            .iter()
            .find(|t| t.name.as_ref() == "diagram-create")
            .expect("diagram-create");
        let ann = create.annotations.as_ref().expect("should have annotations");
        assert_eq!(ann.read_only_hint, Some(true));
        assert_eq!(ann.idempotent_hint, Some(true));

        // diagram-patch: has side effects, deterministic
        let patch = tools
            .iter()
            .find(|t| t.name.as_ref() == "diagram-patch")
            .expect("diagram-patch");
        let ann = patch.annotations.as_ref().expect("should have annotations");
        assert_eq!(ann.read_only_hint, Some(false));
        assert_eq!(ann.idempotent_hint, Some(true));
    }

    #[test]
    fn build_cli_arguments_substitutes_provided_params() {
        let template: Vec<serde_json::Value> = vec![
            json!("diagram"),
            json!("create"),
            json!("--diagram-type"),
            json!("${type}"),
            json!("--json"),
        ];
        let mut params = serde_json::Map::new();
        params.insert("type".to_string(), json!("architecture"));

        let result = build_cli_arguments(&template, &params);
        assert_eq!(
            result,
            vec!["diagram", "create", "--diagram-type", "architecture", "--json"]
        );
    }

    #[test]
    fn build_cli_arguments_drops_flag_and_placeholder_when_param_missing() {
        let template: Vec<serde_json::Value> = vec![
            json!("diagram"),
            json!("render"),
            json!("--render-format"),
            json!("${format}"),
            json!("--json"),
        ];
        let params = serde_json::Map::new(); // no params provided

        let result = build_cli_arguments(&template, &params);
        assert_eq!(result, vec!["diagram", "render", "--json"]);
    }

    #[test]
    fn find_capability_returns_matching_capability() {
        let (cap, _def) =
            find_capability("diagram-create").expect("should find diagram-create capability");
        assert_eq!(cap.get("id").and_then(|v| v.as_str()), Some("diagram-create"));
    }

    #[test]
    fn find_capability_returns_none_for_unknown_tool() {
        assert!(find_capability("nonexistent-tool").is_none());
    }

    #[tokio::test]
    async fn tools_list_returns_expected_capabilities() {
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
        let tools = client_service
            .list_all_tools()
            .await
            .expect("client should list tools");

        assert_eq!(tools.len(), 4, "expected 4 tools from diagram skill def");

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"diagram-create"));
        assert!(names.contains(&"diagram-patch"));
        assert!(names.contains(&"diagram-narrate"));
        assert!(names.contains(&"diagram-render"));

        client_service.cancel().await.expect("client should cancel");
        server_task.await.expect("server task should join");
    }
}
