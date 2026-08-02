use elegy_plugin_sdk::capability_package::{
    canonical_json_sha256, validate_elegy_lock_v1, verify_elegy_package_v1, CapabilityPackageError,
    ElegyCapabilityReferenceV1, ElegyLockV1, ElegyPackageV1,
};
use elegy_plugin_sdk::{
    load_capability_catalog, validate_elegy_capability_catalog_v2, ElegyCapabilityCatalog,
    ElegyCapabilityInvocationV2, ElegyCapabilityV2, ElegySideEffectClass, ToolingError,
};
use jsonschema::Validator;
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResult, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool, ToolAnnotations,
    },
    transport::stdio,
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
};
use serde_json::{json, Value};
use sha2::Digest;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_MAX_OUTPUT_BYTES: usize = 1_048_576;

#[derive(Clone, Debug)]
pub struct BridgeOptions {
    pub package_root: PathBuf,
    pub lock_path: Option<PathBuf>,
    pub target: Option<String>,
    pub allow_side_effects: bool,
    pub allow_non_routable: bool,
    pub allowed_capabilities: Option<BTreeSet<String>>,
    pub timeout: Duration,
    pub max_output_bytes: usize,
}

impl BridgeOptions {
    pub fn new(package_root: impl Into<PathBuf>) -> Self {
        Self {
            package_root: package_root.into(),
            lock_path: None,
            target: None,
            allow_side_effects: false,
            allow_non_routable: false,
            allowed_capabilities: None,
            timeout: DEFAULT_TIMEOUT,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        }
    }

    pub fn with_side_effects(mut self, allow: bool) -> Self {
        self.allow_side_effects = allow;
        self
    }

    pub fn with_lock(mut self, lock_path: impl AsRef<Path>) -> Self {
        self.lock_path = Some(lock_path.as_ref().to_path_buf());
        self
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    pub fn with_non_routable(mut self, allow: bool) -> Self {
        self.allow_non_routable = allow;
        self
    }

    pub fn with_allowed_capabilities(
        mut self,
        allowed_capabilities: impl IntoIterator<Item = String>,
    ) -> Self {
        self.allowed_capabilities = Some(allowed_capabilities.into_iter().collect());
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_max_output_bytes(mut self, max_output_bytes: usize) -> Self {
        self.max_output_bytes = max_output_bytes;
        self
    }
}

impl Default for BridgeOptions {
    fn default() -> Self {
        Self::new(".")
    }
}

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error(transparent)]
    Package(#[from] CapabilityPackageError),
    #[error(transparent)]
    Catalog(#[from] ToolingError),
    #[error("I/O error while {operation} {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error while {operation}: {source}")]
    Json {
        operation: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid capability bridge configuration: {0}")]
    Invalid(String),
    #[error("capability '{capability}' {kind} schema validation failed: {message}")]
    Schema {
        capability: String,
        kind: &'static str,
        message: String,
    },
    #[error("capability '{capability}' timed out after {timeout_seconds} seconds")]
    Timeout {
        capability: String,
        timeout_seconds: u64,
    },
    #[error("capability '{capability}' exited unsuccessfully ({status}): {stderr}")]
    ProcessFailed {
        capability: String,
        status: String,
        stderr: String,
    },
    #[error("capability '{capability}' produced more than {max_output_bytes} output bytes")]
    OutputLimit {
        capability: String,
        max_output_bytes: usize,
    },
}

#[derive(Clone, Debug)]
struct CliCapability {
    id: String,
    description: String,
    side_effect_class: ElegySideEffectClass,
    invocation: ElegyCapabilityInvocationV2,
    input_schema: Value,
    output_schema: Value,
}

#[derive(Debug)]
pub struct CapabilityMcpBridge {
    package_root: PathBuf,
    package: ElegyPackageV1,
    capabilities: BTreeMap<String, CliCapability>,
    options: BridgeOptions,
}

impl CapabilityMcpBridge {
    pub fn load(options: BridgeOptions) -> Result<Self, BridgeError> {
        if options.max_output_bytes == 0 {
            return Err(BridgeError::Invalid(
                "max_output_bytes must be greater than zero".to_string(),
            ));
        }
        if options.timeout.is_zero() {
            return Err(BridgeError::Invalid(
                "timeout must be greater than zero".to_string(),
            ));
        }

        let root =
            std::fs::canonicalize(&options.package_root).map_err(|source| BridgeError::Io {
                operation: "canonicalize",
                path: options.package_root.clone(),
                source,
            })?;
        let package = verify_elegy_package_v1(&root)?;
        let locked_reference = if let Some(lock_path) = &options.lock_path {
            let lock = read_lock(lock_path)?;
            let reference = lock_reference(&lock, &package.name)?;
            if reference.version != package.version {
                return Err(BridgeError::Invalid(
                    "locked package version does not match package manifest".to_string(),
                ));
            }
            if reference.publisher != package.publisher.repository {
                return Err(BridgeError::Invalid(
                    "locked publisher does not match package publisher repository".to_string(),
                ));
            }
            if let Some(target) = &options.target {
                if reference.target != *target {
                    return Err(BridgeError::Invalid(format!(
                        "locked target is {}, requested target is {target}",
                        reference.target
                    )));
                }
            }
            if !package
                .targets
                .iter()
                .any(|target| target == &reference.target || target == "any")
            {
                return Err(BridgeError::Invalid(format!(
                    "package does not support locked target {}",
                    reference.target
                )));
            }
            let manifest_sha256 = canonical_json_sha256(&package)
                .map_err(|error| BridgeError::Invalid(format!("manifest digest: {error}")))?;
            if manifest_sha256 != reference.manifest_sha256.to_ascii_lowercase() {
                return Err(BridgeError::Invalid(
                    "package manifest digest does not match exact lock".to_string(),
                ));
            }
            verify_receipt(&root, &package, reference)?;
            Some(reference.clone())
        } else {
            None
        };
        let catalog_path = package_path(&root, &package.capability_catalog)?;
        let catalog_raw = std::fs::read(&catalog_path).map_err(|source| BridgeError::Io {
            operation: "read capability catalog",
            path: catalog_path.clone(),
            source,
        })?;
        let catalog_value: Value =
            serde_json::from_slice(&catalog_raw).map_err(|source| BridgeError::Json {
                operation: "parse capability catalog for digest",
                source,
            })?;
        let catalog = load_capability_catalog(&catalog_path)?;
        let catalog = match catalog {
            ElegyCapabilityCatalog::V2(catalog) => catalog,
            ElegyCapabilityCatalog::V1(_) => {
                return Err(BridgeError::Invalid(
                    "capability bridge requires capability-catalog/v2".to_string(),
                ));
            }
        };
        let validation = validate_elegy_capability_catalog_v2(&catalog);
        if !validation.is_valid() {
            return Err(BridgeError::Invalid(validation.issues.join("; ")));
        }
        if catalog.plugin != package.name || catalog.plugin_version != package.version {
            return Err(BridgeError::Invalid(
                "capability catalog identity does not match package identity".to_string(),
            ));
        }
        if let Some(reference) = &locked_reference {
            let catalog_sha256 = canonical_json_sha256(&catalog_value)
                .map_err(|error| BridgeError::Invalid(format!("catalog digest: {error}")))?;
            if catalog_sha256 != reference.capability_catalog_sha256.to_ascii_lowercase() {
                return Err(BridgeError::Invalid(
                    "capability catalog digest does not match exact lock".to_string(),
                ));
            }
        }

        for capability in &catalog.capabilities {
            if !capability.common().readiness.is_agent_routable()
                || options
                    .allowed_capabilities
                    .as_ref()
                    .is_some_and(|allowed| !allowed.contains(&capability.common().id))
                || locked_reference.as_ref().is_some_and(|reference| {
                    !reference
                        .allowed_capabilities
                        .contains(&capability.common().id)
                })
            {
                continue;
            }
            if !matches!(capability, ElegyCapabilityV2::Cli { .. }) {
                return Err(BridgeError::Invalid(format!(
                    "capability '{}' is a native MCP surface; generic CLI bridge would be lossy",
                    capability.common().id
                )));
            }
        }

        let declared_files = package
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<BTreeSet<_>>();
        let mut capabilities = BTreeMap::new();
        for capability in catalog.capabilities {
            let ElegyCapabilityV2::Cli { common, invocation } = capability else {
                continue;
            };
            if !options.allow_non_routable && !common.readiness.is_agent_routable() {
                continue;
            }
            if !options.allow_side_effects
                && matches!(
                    common.side_effect_class,
                    ElegySideEffectClass::Mutation | ElegySideEffectClass::FencedMutation
                )
            {
                continue;
            }
            if let Some(allowed) = &options.allowed_capabilities {
                if !allowed.contains(&common.id) {
                    continue;
                }
            }
            if let Some(reference) = &locked_reference {
                if !reference.allowed_capabilities.contains(&common.id) {
                    continue;
                }
            }

            let input_schema = invocation.input_schema.clone().ok_or_else(|| {
                BridgeError::Invalid(format!(
                    "capabilities.{}.invocation.inputSchema is required for CLI projection",
                    common.id
                ))
            })?;
            let output_schema = invocation.output_schema.clone().ok_or_else(|| {
                BridgeError::Invalid(format!(
                    "capabilities.{}.invocation.outputSchema is required for CLI projection",
                    common.id
                ))
            })?;
            if !input_schema.is_object() || !output_schema.is_object() {
                return Err(BridgeError::Invalid(format!(
                    "capabilities.{} CLI inputSchema and outputSchema must be JSON objects",
                    common.id
                )));
            }
            if !declared_files.contains(invocation.executable.as_str()) {
                return Err(BridgeError::Invalid(format!(
                    "capabilities.{} executable '{}' must be declared in package files",
                    common.id, invocation.executable
                )));
            }
            let executable = package_path(&root, &invocation.executable)?;
            let metadata =
                std::fs::symlink_metadata(&executable).map_err(|source| BridgeError::Io {
                    operation: "inspect",
                    path: executable.clone(),
                    source,
                })?;
            if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
                return Err(BridgeError::Invalid(format!(
                    "capabilities.{} executable must be a regular file",
                    common.id
                )));
            }
            capabilities.insert(
                common.id.clone(),
                CliCapability {
                    id: common.id,
                    description: common.description,
                    side_effect_class: common.side_effect_class,
                    invocation,
                    input_schema,
                    output_schema,
                },
            );
        }

        Ok(Self {
            package_root: root,
            package,
            capabilities,
            options: BridgeOptions {
                package_root: options.package_root,
                ..options
            },
        })
    }

    pub fn package(&self) -> &ElegyPackageV1 {
        &self.package
    }

    pub fn tools(&self) -> Vec<Tool> {
        self.capabilities
            .values()
            .map(|capability| {
                let input_schema = capability
                    .input_schema
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                let output_schema = capability
                    .output_schema
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                Tool::new(
                    capability.id.clone(),
                    capability.description.clone(),
                    Arc::new(input_schema),
                )
                .with_raw_output_schema(Arc::new(output_schema))
                .with_annotations(tool_annotations(capability.side_effect_class))
            })
            .collect()
    }

    pub async fn invoke(&self, capability_id: &str, input: Value) -> Result<Value, BridgeError> {
        let capability = self.capabilities.get(capability_id).ok_or_else(|| {
            BridgeError::Invalid(format!(
                "capability '{capability_id}' is not declared in the active package projection"
            ))
        })?;
        validate_schema(capability_id, "input", &capability.input_schema, &input)?;

        let executable = package_path(&self.package_root, &capability.invocation.executable)?;
        let mut command = Command::new(executable);
        command
            .args(&capability.invocation.command)
            .current_dir(&self.package_root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|source| BridgeError::Io {
            operation: "start capability",
            path: self.package_root.join(&capability.invocation.executable),
            source,
        })?;
        let input = serde_json::to_vec(&input).map_err(|source| BridgeError::Json {
            operation: "serialize CLI input",
            source,
        })?;
        let execution = run_child(&mut child, input, self.options.max_output_bytes);
        let process = match tokio::time::timeout(self.options.timeout, execution).await {
            Ok(result) => result?,
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(BridgeError::Timeout {
                    capability: capability_id.to_string(),
                    timeout_seconds: self.options.timeout.as_secs(),
                });
            }
        };
        if process.stdout.len() > self.options.max_output_bytes
            || process.stderr.len() > self.options.max_output_bytes
        {
            return Err(BridgeError::OutputLimit {
                capability: capability_id.to_string(),
                max_output_bytes: self.options.max_output_bytes,
            });
        }
        if !process.status.success() {
            return Err(BridgeError::ProcessFailed {
                capability: capability_id.to_string(),
                status: process.status.to_string(),
                stderr: String::from_utf8_lossy(&process.stderr).trim().to_string(),
            });
        }
        let output: Value =
            serde_json::from_slice(&process.stdout).map_err(|source| BridgeError::Json {
                operation: "parse CLI output",
                source,
            })?;
        validate_schema(capability_id, "output", &capability.output_schema, &output)?;
        Ok(output)
    }

    pub async fn serve_stdio(self) -> Result<(), BridgeError> {
        let server = ServiceExt::<RoleServer>::serve(self, stdio())
            .await
            .map_err(|error| BridgeError::Invalid(format!("start MCP stdio transport: {error}")))?;
        server
            .waiting()
            .await
            .map_err(|error| BridgeError::Invalid(format!("MCP server stopped: {error}")))?;
        Ok(())
    }
}

impl ServerHandler for CapabilityMcpBridge {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions(
                "Elegy exposes locked, schema-validated CLI capabilities through a generic MCP bridge."
                    .to_string(),
            )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: self.tools(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let input = Value::Object(request.arguments.unwrap_or_default());
        match self.invoke(request.name.as_ref(), input).await {
            Ok(output) => Ok(CallToolResult::structured(output)),
            Err(error) => Ok(CallToolResult::structured_error(json!({
                "error": error.to_string()
            }))),
        }
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools()
            .into_iter()
            .find(|tool| tool.name.as_ref() == name)
    }
}

struct ProcessOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

async fn run_child(
    child: &mut Child,
    input: Vec<u8>,
    max_output_bytes: usize,
) -> Result<ProcessOutput, BridgeError> {
    let mut stdin = child.stdin.take().ok_or_else(|| {
        BridgeError::Invalid("capability process did not expose stdin".to_string())
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        BridgeError::Invalid("capability process did not expose stdout".to_string())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        BridgeError::Invalid("capability process did not expose stderr".to_string())
    })?;

    let write_input = async move {
        stdin
            .write_all(&input)
            .await
            .map_err(|source| BridgeError::Io {
                operation: "write CLI input",
                path: PathBuf::from("<capability stdin>"),
                source,
            })
    };
    let read_stdout = read_limited(stdout, max_output_bytes);
    let read_stderr = read_limited(stderr, max_output_bytes);
    let wait = async {
        child.wait().await.map_err(|source| BridgeError::Io {
            operation: "wait for capability",
            path: PathBuf::from("<capability process>"),
            source,
        })
    };
    let ((), stdout, stderr, status) =
        tokio::try_join!(write_input, read_stdout, read_stderr, wait)?;
    Ok(ProcessOutput {
        status,
        stdout,
        stderr,
    })
}

async fn read_limited<R>(reader: R, max_output_bytes: usize) -> Result<Vec<u8>, BridgeError>
where
    R: AsyncRead + Unpin,
{
    let limit = max_output_bytes.saturating_add(1) as u64;
    let mut reader = reader.take(limit);
    let mut output = Vec::new();
    reader
        .read_to_end(&mut output)
        .await
        .map_err(|source| BridgeError::Io {
            operation: "read capability output",
            path: PathBuf::from("<capability output>"),
            source,
        })?;
    Ok(output)
}

fn validate_schema(
    capability_id: &str,
    kind: &'static str,
    schema: &Value,
    value: &Value,
) -> Result<(), BridgeError> {
    let validator: Validator = jsonschema::validator_for(schema).map_err(|error| {
        BridgeError::Invalid(format!(
            "capability '{capability_id}' {kind} schema is invalid: {error}"
        ))
    })?;
    if let Some(error) = validator.iter_errors(value).next() {
        return Err(BridgeError::Schema {
            capability: capability_id.to_string(),
            kind,
            message: error.to_string(),
        });
    }
    Ok(())
}

fn tool_annotations(side_effect_class: ElegySideEffectClass) -> ToolAnnotations {
    let read_only = matches!(
        side_effect_class,
        ElegySideEffectClass::Pure | ElegySideEffectClass::Query
    );
    ToolAnnotations::new()
        .read_only(read_only)
        .destructive(!read_only)
        .idempotent(matches!(side_effect_class, ElegySideEffectClass::Pure))
}

fn package_path(root: &Path, relative: &str) -> Result<PathBuf, BridgeError> {
    let normalized = relative
        .strip_prefix("./")
        .ok_or_else(|| BridgeError::Invalid(format!("path '{relative}' must start with './'")))?;
    if normalized.is_empty()
        || normalized
            .split('/')
            .any(|part| part.is_empty() || part == "..")
    {
        return Err(BridgeError::Invalid(format!("path '{relative}' is unsafe")));
    }
    Ok(root.join(normalized))
}

fn read_lock(path: &Path) -> Result<ElegyLockV1, BridgeError> {
    let raw = std::fs::read(path).map_err(|source| BridgeError::Io {
        operation: "read lock",
        path: path.to_path_buf(),
        source,
    })?;
    let lock: ElegyLockV1 = serde_json::from_slice(&raw).map_err(|source| BridgeError::Json {
        operation: "parse lock",
        source,
    })?;
    let issues = validate_elegy_lock_v1(&lock);
    if !issues.is_empty() {
        return Err(BridgeError::Invalid(issues.join("; ")));
    }
    Ok(lock)
}

fn lock_reference<'a>(
    lock: &'a ElegyLockV1,
    package_name: &str,
) -> Result<&'a ElegyCapabilityReferenceV1, BridgeError> {
    lock.packages
        .iter()
        .find(|reference| reference.name == package_name)
        .ok_or_else(|| {
            BridgeError::Invalid(format!(
                "package '{package_name}' is not present in exact lock"
            ))
        })
}

fn verify_receipt(
    root: &Path,
    package: &ElegyPackageV1,
    reference: &ElegyCapabilityReferenceV1,
) -> Result<(), BridgeError> {
    let receipt_file = root.join("capability-install-receipt.json");
    let raw = std::fs::read(&receipt_file).map_err(|source| BridgeError::Io {
        operation: "read install receipt for lock-backed bridge",
        path: receipt_file.clone(),
        source,
    })?;
    let receipt: Value = serde_json::from_slice(&raw).map_err(|source| BridgeError::Json {
        operation: "parse install receipt",
        source,
    })?;
    if receipt["schemaVersion"] != "elegy-capability-installer/v1"
        || receipt["name"] != package.name
        || receipt["version"] != package.version
        || receipt["publisher"] != package.publisher.repository
        || receipt["target"] != reference.target
        || receipt["archiveSha256"]
            .as_str()
            .is_none_or(|value| !value.eq_ignore_ascii_case(&reference.archive_sha256))
        || receipt["manifestSha256"]
            .as_str()
            .is_none_or(|value| !value.eq_ignore_ascii_case(&reference.manifest_sha256))
        || receipt["capabilityCatalogSha256"]
            .as_str()
            .is_none_or(|value| !value.eq_ignore_ascii_case(&reference.capability_catalog_sha256))
    {
        return Err(BridgeError::Invalid(
            "install receipt does not match exact lock".to_string(),
        ));
    }
    let files = receipt["files"].as_object().ok_or_else(|| {
        BridgeError::Invalid("lock-backed install receipt must contain file hashes".to_string())
    })?;
    let mut required_files = BTreeSet::from(["elegy-package.json".to_string()]);
    required_files.extend(
        package
            .files
            .iter()
            .map(|file| file.path.trim_start_matches("./").to_string()),
    );
    if let Some(missing) = required_files
        .iter()
        .find(|path| !files.contains_key(*path))
    {
        return Err(BridgeError::Invalid(format!(
            "install receipt is missing a digest for '{missing}'"
        )));
    }
    let package_executable_paths = package
        .entrypoints
        .iter()
        .map(|entrypoint| entrypoint.executable.clone())
        .collect::<BTreeSet<_>>();
    let locked_executable_paths = reference
        .executable_digests
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if package_executable_paths != locked_executable_paths {
        return Err(BridgeError::Invalid(
            "locked executable digest paths do not match package entrypoints".to_string(),
        ));
    }
    for (relative, expected) in files {
        let Some(expected) = expected.as_str() else {
            return Err(BridgeError::Invalid(format!(
                "install receipt hash for '{relative}' is not a string"
            )));
        };
        let path = receipt_path(root, relative)?;
        let metadata = std::fs::symlink_metadata(&path).map_err(|source| BridgeError::Io {
            operation: "inspect installed file for lock-backed bridge",
            path: path.clone(),
            source,
        })?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(BridgeError::Invalid(format!(
                "installed receipt file '{relative}' is not a regular file"
            )));
        }
        let bytes = std::fs::read(&path).map_err(|source| BridgeError::Io {
            operation: "read installed file for lock-backed bridge",
            path: path.clone(),
            source,
        })?;
        let actual = format!("{:x}", sha2::Sha256::digest(bytes));
        if actual != expected.to_ascii_lowercase() {
            return Err(BridgeError::Invalid(format!(
                "installed file '{relative}' digest does not match receipt"
            )));
        }
    }
    for (path, expected) in &reference.executable_digests {
        let relative = path.trim_start_matches("./");
        let Some(receipt_expected) = files.get(relative).and_then(Value::as_str) else {
            return Err(BridgeError::Invalid(format!(
                "install receipt is missing executable digest for '{path}'"
            )));
        };
        if !receipt_expected.eq_ignore_ascii_case(expected) {
            return Err(BridgeError::Invalid(format!(
                "install receipt executable digest for '{path}' does not match exact lock"
            )));
        }
    }
    Ok(())
}

fn receipt_path(root: &Path, relative: &str) -> Result<PathBuf, BridgeError> {
    let normalized = relative.replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.contains(':')
        || normalized
            .split('/')
            .any(|part| part.is_empty() || part == "..")
    {
        return Err(BridgeError::Invalid(format!(
            "install receipt path '{relative}' is unsafe"
        )));
    }
    Ok(root.join(normalized))
}
