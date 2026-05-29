use crate::HostError;
use base64::Engine as _;
use elegy_contracts::{
    StructuredFailure, StructuredFailureCategory, CLI_SCHEMA_VERSION,
};
use elegy_core::{
    compose_runtime_state, CatalogResource, ProjectLocator, ReadResourceError, ResourceReadResult,
    RuntimeState,
};
use elegy_skills::{RegistryMcpToolBinding, SkillRegistry};
use rmcp::{
    model::*, transport::stdio, ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::task;

// ---------------------------------------------------------------------------
// Cached tool list built from the embedded skill definitions
// ---------------------------------------------------------------------------
fn cached_tool_bindings() -> Vec<RegistryMcpToolBinding> {
    static TOOL_BINDINGS: OnceLock<Vec<RegistryMcpToolBinding>> = OnceLock::new();
    TOOL_BINDINGS
        .get_or_init(build_tool_bindings_from_skill_definitions)
        .clone()
}

fn cached_tools() -> Vec<Tool> {
    static TOOLS: OnceLock<Vec<Tool>> = OnceLock::new();
    TOOLS
        .get_or_init(build_tools_from_skill_definitions)
        .clone()
}

fn tools_for_allowed_ids(allowed_tool_ids: Option<&BTreeSet<String>>) -> Vec<Tool> {
    cached_tools()
        .into_iter()
        .filter(|tool| allowed_tool_ids.is_none_or(|allowed| allowed.contains(tool.name.as_ref())))
        .collect()
}

/// Parse every embedded v2 skill-definition JSON and convert each capability
/// entry into an rmcp `Tool`.
fn build_tool_bindings_from_skill_definitions() -> Vec<RegistryMcpToolBinding> {
    let registry = match SkillRegistry::builtin() {
        Ok(registry) => registry,
        Err(_) => return Vec::new(),
    };

    registry.build_mcp_tool_bindings()
}

fn build_tools_from_skill_definitions() -> Vec<Tool> {
    build_tool_bindings_from_skill_definitions()
        .iter()
        .map(tool_from_binding)
        .collect()
}

fn tool_from_binding(binding: &RegistryMcpToolBinding) -> Tool {
    let mut mcp_tool = Tool::new(
        binding.capability_id.clone(),
        binding.description.clone(),
        match &binding.input_schema {
            Value::Object(map) => map.clone(),
            _ => serde_json::Map::new(),
        },
    );
    mcp_tool.annotations = Some(
        ToolAnnotations::new()
            .read_only(binding.read_only_hint.unwrap_or(false))
            .destructive(false)
            .idempotent(binding.idempotent_hint.unwrap_or(false))
            .open_world(false),
    );
    mcp_tool
}

// ---------------------------------------------------------------------------
// Capability lookup + argument substitution helpers (used by call_tool)
// ---------------------------------------------------------------------------

fn find_tool_binding(tool_name: &str) -> Option<RegistryMcpToolBinding> {
    cached_tool_bindings()
        .into_iter()
        .find(|binding| binding.capability_id == tool_name)
}

/// Build the final CLI argument vector, correctly dropping `--flag ${param}`
/// pairs when the parameter is not supplied by the caller.
fn build_cli_arguments(
    template_args: &[String],
    params: &serde_json::Map<String, serde_json::Value>,
) -> Vec<String> {
    let mut result = Vec::new();
    let mut skip_next = false;

    for (i, arg) in template_args.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }
        let s = arg.as_str();

        if s.starts_with("${") && s.ends_with('}') {
            let key = &s[2..s.len() - 1];
            if let Some(val) = params.get(key) {
                if let Some(values) = val.as_array() {
                    result.extend(values.iter().map(value_as_string));
                } else {
                    result.push(value_as_string(val));
                }
            }
            continue;
        }

        // Look ahead: if the *next* element is a placeholder, decide now
        if let Some(next) = template_args.get(i + 1) {
            let next_s = next.as_str();
            if next_s.starts_with("${") && next_s.ends_with('}') {
                let key = &next_s[2..next_s.len() - 1];
                if let Some(val) = params.get(key) {
                    if let Some(flag_value) = val.as_bool() {
                        if flag_value {
                            result.push(s.to_string());
                        }
                    } else if let Some(values) = val.as_array() {
                        for value in values {
                            result.push(s.to_string());
                            result.push(value_as_string(value));
                        }
                    } else {
                        result.push(s.to_string());
                        result.push(value_as_string(val));
                    }
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

fn dry_run_argument_value(arguments: &serde_json::Map<String, serde_json::Value>) -> Option<bool> {
    arguments
        .get("dryRun")
        .or_else(|| arguments.get("dry_run"))
        .and_then(|value| value.as_bool())
}

fn arguments_request_dry_run(arguments: &serde_json::Map<String, serde_json::Value>) -> bool {
    dry_run_argument_value(arguments).unwrap_or(false)
}

fn normalize_dry_run_argument(arguments: &mut serde_json::Map<String, serde_json::Value>) {
    let Some(value) = dry_run_argument_value(arguments) else {
        return;
    };

    arguments.insert("dry_run".to_string(), json!(value));
    arguments.insert("dryRun".to_string(), json!(value));
}

fn bytes_to_capped_string(bytes: &[u8], max_bytes: usize) -> String {
    if bytes.len() <= max_bytes {
        return String::from_utf8_lossy(bytes).into_owned();
    }

    let capped = bytes.get(..max_bytes).unwrap_or(bytes);
    let mut text = String::from_utf8_lossy(capped).into_owned();
    text.push_str("\n... [truncated by elegy-host-mcp]");
    text
}

fn executable_file_name(name: &str) -> String {
    if cfg!(windows) && !name.ends_with(".exe") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn workspace_target_dir() -> Option<std::path::PathBuf> {
    let current = std::env::current_exe().ok()?;
    let parent = current.parent()?;
    if parent.file_name().and_then(|name| name.to_str()) == Some("deps") {
        parent.parent().map(std::path::Path::to_path_buf)
    } else {
        Some(parent.to_path_buf())
    }
}

fn parse_cli_machine_envelope(stdout: &str) -> Option<Value> {
    let trimmed = stdout.trim();
    let json_slice = trimmed
        .find('{')
        .zip(trimmed.rfind('}'))
        .and_then(|(start, end)| trimmed.get(start..=end))
        .unwrap_or(trimmed);
    let value: Value = serde_json::from_str(json_slice).ok()?;
    let schema_version = value
        .get("schemaVersion")
        .or_else(|| value.get("schema_version"))
        .and_then(Value::as_str);
    if schema_version == Some(CLI_SCHEMA_VERSION) && value.is_object() {
        Some(value)
    } else {
        None
    }
}

fn cli_machine_result(envelope: Value) -> CallToolResult {
    let is_error = envelope.get("status").and_then(Value::as_str) != Some("ok");
    if is_error {
        CallToolResult::structured_error(envelope)
    } else {
        CallToolResult::structured(envelope)
    }
}

fn host_error_result(
    tool_name: &str,
    code: &str,
    category: StructuredFailureCategory,
    message: impl Into<String>,
    details: Option<Value>,
) -> CallToolResult {
    let failure = StructuredFailure {
        code: code.to_string(),
        message: message.into(),
        category,
        retryable: false,
        correlation_id: None,
        details,
        cause: None,
    };
    CallToolResult::structured_error(json!({
        "surface": "elegy-host-mcp",
        "tool": tool_name,
        "failure": failure,
    }))
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
    if let Some(target_dir) = workspace_target_dir() {
        let candidate = target_dir.join(executable_file_name(name));
        if candidate.is_file() {
            return candidate.to_string_lossy().into_owned();
        }
    }
    // Otherwise assume the executable is on PATH
    name.to_string()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostOptions {
    pub allow_side_effects: bool,
    pub default_tool_timeout_seconds: u64,
    pub max_tool_output_bytes: usize,
    pub allowed_tool_ids: Option<BTreeSet<String>>,
}

impl Default for HostOptions {
    fn default() -> Self {
        Self {
            allow_side_effects: false,
            default_tool_timeout_seconds: 30,
            max_tool_output_bytes: 1_048_576,
            allowed_tool_ids: None,
        }
    }
}

pub struct ElegyMcpHost {
    state: Arc<RuntimeState>,
    options: HostOptions,
}

impl ElegyMcpHost {
    pub fn new(state: RuntimeState) -> Self {
        Self::with_options(state, HostOptions::default())
    }

    pub fn with_options(state: RuntimeState, options: HostOptions) -> Self {
        Self {
            state: Arc::new(state),
            options,
        }
    }

    fn tool_allowed(&self, tool_name: &str) -> bool {
        self.options
            .allowed_tool_ids
            .as_ref()
            .is_none_or(|allowed| allowed.contains(tool_name))
    }
}

pub async fn serve_stdio(locator: ProjectLocator) -> Result<(), HostError> {
    serve_stdio_with_options(locator, HostOptions::default()).await
}

pub async fn serve_stdio_with_options(
    locator: ProjectLocator,
    options: HostOptions,
) -> Result<(), HostError> {
    let state = compose_runtime_state(locator)?;
    let server = ElegyMcpHost::with_options(state, options)
        .serve(stdio())
        .await?;
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
        let tools = tools_for_allowed_ids(self.options.allowed_tool_ids.as_ref());
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
        let mut arguments = request.arguments.unwrap_or_default();

        if !self.tool_allowed(tool_name) {
            return Err(McpError::invalid_params(
                format!("tool is not enabled by the active Elegy agent profile: {tool_name}"),
                Some(json!({ "tool": tool_name })),
            ));
        }

        // 1. Find the matching capability binding
        let binding = find_tool_binding(tool_name).ok_or_else(|| {
            McpError::invalid_params(
                format!("unknown tool: {tool_name}"),
                Some(json!({ "tool": tool_name })),
            )
        })?;

        let has_side_effects = binding.has_side_effects;
        let dry_run_requested = arguments_request_dry_run(&arguments);
        let dry_run_allowed = dry_run_requested && binding.supports_dry_run;
        if has_side_effects && !self.options.allow_side_effects && !dry_run_allowed {
            return Ok(host_error_result(
                tool_name,
                "MCP-POLICY-DENIED",
                StructuredFailureCategory::Policy,
                format!(
                    "tool {tool_name} has side effects; restart the host with side-effect execution enabled or call a capability that explicitly supports dry-run"
                ),
                Some(json!({
                    "tool": tool_name,
                    "dryRunRequested": dry_run_requested,
                    "supportsDryRun": binding.supports_dry_run,
                })),
            ));
        }
        if dry_run_allowed {
            normalize_dry_run_argument(&mut arguments);
        }

        // 2. Extract implementation details
        let execution_type = binding.execution_type.as_str();
        if execution_type != "subprocess" {
            return Ok(host_error_result(
                tool_name,
                "MCP-UNSUPPORTED-EXECUTION",
                StructuredFailureCategory::Unavailable,
                format!(
                    "tool {tool_name} uses unsupported executionType '{execution_type}' in this host"
                ),
                Some(json!({
                    "tool": tool_name,
                    "executionType": execution_type,
                })),
            ));
        }

        let executable = binding.executable_name.as_str();

        // 3. Build CLI arguments with placeholder substitution
        let cli_args = build_cli_arguments(&binding.argument_template, &arguments);

        // 4. Detect whether stdin piping is required
        let stdin_data = binding
            .stdin_format
            .as_ref()
            .and_then(|_| arguments.get("stdin"))
            .map(value_as_string);

        // 5. Resolve the executable — try PATH first, fall back to current_exe
        let exe_path = which_executable(executable);

        // 6. Spawn the subprocess
        let mut cmd = tokio::process::Command::new(&exe_path);
        cmd.args(&cli_args);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.kill_on_drop(true);

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
        let timeout_seconds = binding
            .timeout_seconds
            .unwrap_or(self.options.default_tool_timeout_seconds);
        let output = match tokio::time::timeout(
            Duration::from_secs(timeout_seconds),
            child.wait_with_output(),
        )
        .await
        {
            Ok(result) => result.map_err(|e| {
                McpError::internal_error(
                    format!("subprocess I/O error: {e}"),
                    Some(json!({ "tool": tool_name })),
                )
            })?,
            Err(_) => {
                return Ok(host_error_result(
                    tool_name,
                    "MCP-TIMEOUT",
                    StructuredFailureCategory::Timeout,
                    format!("tool {tool_name} timed out after {timeout_seconds}s"),
                    Some(json!({
                        "tool": tool_name,
                        "timeoutSeconds": timeout_seconds,
                    })),
                ));
            }
        };

        // 9. Return result
        let stdout = bytes_to_capped_string(&output.stdout, self.options.max_tool_output_bytes);
        let stderr = bytes_to_capped_string(&output.stderr, self.options.max_tool_output_bytes);

        if let Some(envelope) = parse_cli_machine_envelope(&stdout) {
            return Ok(cli_machine_result(envelope));
        }

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
    use serde::Deserialize;
    use serde_json::Value;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Default, Clone)]
    struct TestClient;

    impl ClientHandler for TestClient {}

    #[derive(Debug, Deserialize)]
    struct HostMachineEnvelope {
        #[serde(rename = "schema_version", alias = "schemaVersion")]
        schema_version: String,
        command: Vec<String>,
        status: String,
        data: Value,
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        std::fs::create_dir_all(&dir).expect("create temp directory");
        dir
    }

    fn ensure_elegy_binary_built() {
        static BUILT: OnceLock<()> = OnceLock::new();
        BUILT.get_or_init(|| {
            let status = std::process::Command::new("cargo")
                .args(["build", "-p", "elegy-cli", "--bin", "elegy"])
                .current_dir(repo_root())
                .status()
                .expect("build elegy binary for MCP host tests");
            assert!(status.success(), "failed to build elegy binary for MCP host tests");
        });
    }

    fn expect_structured_content(result: &CallToolResult) -> Value {
        result.structured_content.clone().unwrap_or_else(|| {
            let fallback_text = result
                .content
                .iter()
                .filter_map(|content| content.raw.as_text().map(|text| text.text.clone()))
                .collect::<Vec<_>>()
                .join("\n");
            panic!(
                "tool result should include structured content; is_error={:?}; content={fallback_text}",
                result.is_error
            );
        })
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
        let expected_count = SkillRegistry::builtin()
            .expect("built-in skill registry should load")
            .build_mcp_tools()
            .len();

        assert_eq!(
            tools.len(),
            expected_count,
            "expected tool count to match the built-in v2 skill registry"
        );

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"diagram-create"), "missing diagram-create");
        assert!(names.contains(&"memory-add"), "missing memory-add");
        assert!(names.contains(&"memory-search"), "missing memory-search");
        assert!(
            names.contains(&"mcp-analyze-descriptor"),
            "missing mcp-analyze-descriptor"
        );
        assert!(
            names.contains(&"skills-registry-search"),
            "missing skills-registry-search"
        );
        assert!(
            names.contains(&"skills-registry-resolve"),
            "missing skills-registry-resolve"
        );
        assert!(
            names.contains(&"skills-registry-validate"),
            "missing skills-registry-validate"
        );
        assert!(names.contains(&"mermaid-render"), "missing mermaid-render");
        assert!(names.contains(&"diagram-patch"), "missing diagram-patch");
        assert!(
            names.contains(&"diagram-narrate"),
            "missing diagram-narrate"
        );
        assert!(names.contains(&"diagram-render"), "missing diagram-render");
        assert!(
            names.contains(&"router-skill-search"),
            "missing router-skill-search"
        );
        assert!(
            names.contains(&"router-skill-describe"),
            "missing router-skill-describe"
        );
        assert!(
            names.contains(&"router-skill-list"),
            "missing router-skill-list"
        );
        assert!(
            names.contains(&"observe-processes"),
            "missing observe-processes"
        );
        assert!(names.contains(&"observe-window"), "missing observe-window");
        assert!(
            names.contains(&"observe-windows"),
            "missing observe-windows"
        );
        assert!(names.contains(&"observe-screen"), "missing observe-screen");
        assert!(
            names.contains(&"observe-clipboard"),
            "missing observe-clipboard"
        );
        assert!(
            names.contains(&"observe-filesystem"),
            "missing observe-filesystem"
        );
        assert!(names.contains(&"observe-system"), "missing observe-system");
        assert!(names.contains(&"observe-record"), "missing observe-record");
        assert!(names.contains(&"desktop-click"), "missing desktop-click");
        assert!(names.contains(&"desktop-type"), "missing desktop-type");
        assert!(names.contains(&"desktop-key"), "missing desktop-key");
        assert!(names.contains(&"desktop-focus"), "missing desktop-focus");
        assert!(names.contains(&"desktop-move"), "missing desktop-move");
        assert!(
            names.contains(&"desktop-minimize"),
            "missing desktop-minimize"
        );
        assert!(
            names.contains(&"desktop-maximize"),
            "missing desktop-maximize"
        );
        assert!(names.contains(&"repo-status"), "missing repo-status");
        assert!(names.contains(&"repo-log"), "missing repo-log");
        assert!(names.contains(&"web-fetch"), "missing web-fetch");
        assert!(names.contains(&"web-ping"), "missing web-ping");
        assert!(names.contains(&"data-convert"), "missing data-convert");
        assert!(names.contains(&"data-validate"), "missing data-validate");
        assert!(names.contains(&"notify-toast"), "missing notify-toast");
        assert!(names.contains(&"notify-webhook"), "missing notify-webhook");
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
    fn build_tools_preserves_array_param_types() {
        let tools = build_tools_from_skill_definitions();

        let fetch_tool = tools
            .iter()
            .find(|t| t.name.as_ref() == "web-fetch")
            .expect("web-fetch tool should exist");

        let props = fetch_tool
            .input_schema
            .get("properties")
            .and_then(|v| v.as_object())
            .expect("schema should have properties");
        assert_eq!(
            props
                .get("headers")
                .and_then(|value| value.get("type"))
                .and_then(|value| value.as_str()),
            Some("array")
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
        let ann = create
            .annotations
            .as_ref()
            .expect("should have annotations");
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
    fn tools_for_allowed_ids_filters_profile_projection() {
        let allowed =
            BTreeSet::from(["router-skill-search".to_string(), "repo-status".to_string()]);
        let tools = tools_for_allowed_ids(Some(&allowed));
        let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();

        assert_eq!(names.len(), 2);
        assert!(names.contains(&"router-skill-search"));
        assert!(names.contains(&"repo-status"));
        assert!(!names.contains(&"memory-add"));
    }

    #[test]
    fn build_cli_arguments_substitutes_provided_params() {
        let template = vec![
            "diagram".to_string(),
            "create".to_string(),
            "--diagram-type".to_string(),
            "${type}".to_string(),
            "--json".to_string(),
        ];
        let mut params = serde_json::Map::new();
        params.insert("type".to_string(), json!("architecture"));

        let result = build_cli_arguments(&template, &params);
        assert_eq!(
            result,
            vec![
                "diagram",
                "create",
                "--diagram-type",
                "architecture",
                "--json"
            ]
        );
    }

    #[test]
    fn build_cli_arguments_drops_flag_and_placeholder_when_param_missing() {
        let template = vec![
            "diagram".to_string(),
            "render".to_string(),
            "--render-format".to_string(),
            "${format}".to_string(),
            "--json".to_string(),
        ];
        let params = serde_json::Map::new(); // no params provided

        let result = build_cli_arguments(&template, &params);
        assert_eq!(result, vec!["diagram", "render", "--json"]);
    }

    #[test]
    fn build_cli_arguments_treats_boolean_placeholder_as_flag() {
        let template = vec![
            "desktop".to_string(),
            "click".to_string(),
            "--dry-run".to_string(),
            "${dry_run}".to_string(),
            "--json".to_string(),
        ];

        let mut true_params = serde_json::Map::new();
        true_params.insert("dry_run".to_string(), json!(true));
        assert_eq!(
            build_cli_arguments(&template, &true_params),
            vec!["desktop", "click", "--dry-run", "--json"]
        );

        let mut false_params = serde_json::Map::new();
        false_params.insert("dry_run".to_string(), json!(false));
        assert_eq!(
            build_cli_arguments(&template, &false_params),
            vec!["desktop", "click", "--json"]
        );
    }

    #[test]
    fn build_cli_arguments_expands_array_placeholder_per_flag() {
        let template = vec![
            "web".to_string(),
            "fetch".to_string(),
            "--header".to_string(),
            "${headers}".to_string(),
            "--json".to_string(),
        ];
        let mut params = serde_json::Map::new();
        params.insert(
            "headers".to_string(),
            json!(["Accept: application/json", "X-Test: true"]),
        );

        let result = build_cli_arguments(&template, &params);
        assert_eq!(
            result,
            vec![
                "web",
                "fetch",
                "--header",
                "Accept: application/json",
                "--header",
                "X-Test: true",
                "--json",
            ]
        );
    }

    #[test]
    fn build_cli_arguments_substitutes_standalone_placeholders() {
        let template = vec![
            "memory".to_string(),
            "add".to_string(),
            "${content}".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ];
        let mut params = serde_json::Map::new();
        params.insert("content".to_string(), json!("A distilled memory"));

        let result = build_cli_arguments(&template, &params);
        assert_eq!(
            result,
            vec!["memory", "add", "A distilled memory", "--format", "json"]
        );
    }

    #[test]
    fn side_effect_policy_helpers_require_explicit_dry_run_support() {
        let unsupported_capability = RegistryMcpToolBinding {
            capability_id: "desktop-click".to_string(),
            description: "click".to_string(),
            executable_name: "elegy".to_string(),
            execution_type: "subprocess".to_string(),
            argument_template: vec!["desktop".to_string(), "click".to_string()],
            input_schema: json!({}),
            stdin_format: None,
            timeout_seconds: None,
            has_side_effects: true,
            supports_dry_run: false,
            read_only_hint: Some(false),
            idempotent_hint: Some(false),
        };
        let mut args = serde_json::Map::new();

        assert!(unsupported_capability.has_side_effects);
        assert!(!unsupported_capability.supports_dry_run);
        assert!(!arguments_request_dry_run(&args));

        args.insert("dryRun".to_string(), json!(true));
        assert!(arguments_request_dry_run(&args));
        assert!(!unsupported_capability.supports_dry_run);

        let supported_capability = SkillRegistry::builtin()
            .expect("built-in skill registry should load")
            .mcp_tool_binding("desktop-click")
            .expect("desktop-click binding");
        assert!(supported_capability.supports_dry_run);

        normalize_dry_run_argument(&mut args);
        assert_eq!(args.get("dry_run"), Some(&json!(true)));
        assert_eq!(args.get("dryRun"), Some(&json!(true)));
    }

    #[test]
    fn find_tool_binding_returns_matching_capability() {
        let binding = find_tool_binding("diagram-create").expect("should find diagram-create capability");
        assert_eq!(binding.capability_id, "diagram-create");
        assert_eq!(binding.execution_type, "subprocess");
    }

    #[test]
    fn find_tool_binding_returns_none_for_unknown_tool() {
        assert!(find_tool_binding("nonexistent-tool").is_none());
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
        let expected_count = SkillRegistry::builtin()
            .expect("built-in skill registry should load")
            .build_mcp_tools()
            .len();

        assert_eq!(
            tools.len(),
            expected_count,
            "expected tool count to match the built-in v2 skill registry"
        );

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"diagram-create"));
        assert!(names.contains(&"memory-add"));
        assert!(names.contains(&"memory-search"));
        assert!(names.contains(&"mcp-analyze-descriptor"));
        assert!(names.contains(&"skills-registry-search"));
        assert!(names.contains(&"skills-registry-resolve"));
        assert!(names.contains(&"skills-registry-validate"));
        assert!(names.contains(&"mermaid-render"));
        assert!(names.contains(&"diagram-patch"));
        assert!(names.contains(&"diagram-narrate"));
        assert!(names.contains(&"diagram-render"));
        assert!(names.contains(&"router-skill-search"));
        assert!(names.contains(&"router-skill-describe"));
        assert!(names.contains(&"router-skill-list"));
        assert!(names.contains(&"observe-processes"));
        assert!(names.contains(&"observe-window"));
        assert!(names.contains(&"observe-windows"));
        assert!(names.contains(&"observe-screen"));
        assert!(names.contains(&"observe-clipboard"));
        assert!(names.contains(&"observe-filesystem"));
        assert!(names.contains(&"observe-system"));
        assert!(names.contains(&"observe-record"));
        assert!(names.contains(&"desktop-click"));
        assert!(names.contains(&"desktop-type"));
        assert!(names.contains(&"desktop-key"));
        assert!(names.contains(&"desktop-focus"));
        assert!(names.contains(&"desktop-move"));
        assert!(names.contains(&"desktop-minimize"));
        assert!(names.contains(&"desktop-maximize"));
        assert!(names.contains(&"repo-status"));
        assert!(names.contains(&"repo-log"));
        assert!(names.contains(&"web-fetch"));
        assert!(names.contains(&"web-ping"));
        assert!(names.contains(&"data-convert"));
        assert!(names.contains(&"data-validate"));
        assert!(names.contains(&"notify-toast"));
        assert!(names.contains(&"notify-webhook"));

        client_service.cancel().await.expect("client should cancel");
        server_task.await.expect("server task should join");
    }

    #[tokio::test]
    async fn host_call_tool_returns_structured_machine_success_for_elegy_cli() {
        ensure_elegy_binary_built();
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

        let schema_dir = unique_temp_dir("elegy-host-mcp-data-validate-success");
        let schema_path = schema_dir.join("schema.json");
        std::fs::write(
            &schema_path,
            r#"{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}"#,
        )
        .expect("write schema file");

        let client_service = client
            .serve(client_transport)
            .await
            .expect("client should initialize");
        let result = client_service
            .call_tool(
                CallToolRequestParams::new("data-validate").with_arguments(
                    json!({
                        "schema": schema_path.display().to_string(),
                        "stdin": { "name": "Elegy" }
                    })
                    .as_object()
                    .expect("tool arguments should be an object")
                    .clone(),
                ),
            )
            .await
            .expect("tool call should succeed");

        assert_eq!(result.is_error, Some(false));
        let structured = expect_structured_content(&result);
        let envelope: HostMachineEnvelope =
            serde_json::from_value(structured).expect("structured content should be an elegy envelope");
        assert_eq!(envelope.schema_version, CLI_SCHEMA_VERSION);
        assert_eq!(envelope.command, ["data", "validate"]);
        assert_eq!(envelope.status, "ok");
        assert_eq!(envelope.data["valid"], json!(true));

        client_service.cancel().await.expect("client should cancel");
        server_task.await.expect("server task should join");
    }

    #[tokio::test]
    async fn host_call_tool_returns_structured_machine_failure_for_elegy_cli() {
        ensure_elegy_binary_built();
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

        let missing_path = unique_temp_dir("elegy-host-mcp-data-validate-missing")
            .join("missing-schema.json");

        let client_service = client
            .serve(client_transport)
            .await
            .expect("client should initialize");
        let result = client_service
            .call_tool(
                CallToolRequestParams::new("data-validate").with_arguments(
                    json!({
                        "schema": missing_path.display().to_string(),
                        "stdin": { "name": "Elegy" }
                    })
                    .as_object()
                    .expect("tool arguments should be an object")
                    .clone(),
                ),
            )
            .await
            .expect("tool call should return structured failure");

        assert_eq!(result.is_error, Some(true));
        let structured = expect_structured_content(&result);
        assert_eq!(
            structured
                .get("schemaVersion")
                .or_else(|| structured.get("schema_version"))
                .cloned(),
            Some(json!(CLI_SCHEMA_VERSION))
        );
        assert_eq!(structured["command"], json!(["data", "validate"]));
        assert_eq!(structured["status"], json!("error"));
        let summary_text = structured["summary"]["text"]
            .as_str()
            .expect("summary text");
        assert!(!summary_text.trim().is_empty());

        client_service.cancel().await.expect("client should cancel");
        server_task.await.expect("server task should join");
    }

    #[tokio::test]
    async fn host_call_tool_returns_structured_policy_denial() {
        ensure_elegy_binary_built();
        let state = compose_runtime_state(ProjectLocator::Path(
            repo_root().join("examples/http-minimal"),
        ))
        .expect("example runtime should compose");
        let server = ElegyMcpHost::with_options(
            state,
            HostOptions {
                allow_side_effects: false,
                default_tool_timeout_seconds: 30,
                max_tool_output_bytes: 1_048_576,
                allowed_tool_ids: None,
            },
        );
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
            .call_tool(CallToolRequestParams::new("desktop-click"))
            .await
            .expect("tool call should return policy denial");

        assert_eq!(result.is_error, Some(true));
        let structured = expect_structured_content(&result);
        assert_eq!(structured["failure"]["code"], json!("MCP-POLICY-DENIED"));
        assert_eq!(structured["failure"]["category"], json!("policy"));

        client_service.cancel().await.expect("client should cancel");
        server_task.await.expect("server task should join");
    }
}
