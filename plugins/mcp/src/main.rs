use clap::{Parser, Subcommand, ValueEnum};
use elegy_mcp::{
    analyze_mcp_descriptor_file, author_mcp_descriptor_to_path, AuthorMcpDescriptorRequest,
    AuthorMcpToolRequest, McpAnalysisResult, McpTransportKind,
};
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

const EXIT_CODE_INVALID_INPUT: u8 = 1;

const CLI_SCHEMA_VERSION: &str = "elegy.cli/v1";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliEnvelope<T: Serialize> {
    schema_version: &'static str,
    correlation_id: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    non_interactive: bool,
    command: Vec<String>,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data_schema: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<StructuredFailure>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StructuredFailure {
    code: String,
    message: String,
    #[serde(default)]
    category: String,
    retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<StructuredFailureCause>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StructuredFailureCause {
    code: String,
    message: String,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Parser, Debug)]
#[command(name = "elegy-mcp")]
#[command(about = "Dedicated MCP authoring and analysis CLI for Elegy")]
struct Cli {
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    format: OutputFormat,
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    non_interactive: bool,
    #[arg(long, global = true)]
    correlation_id: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Subcommand, Debug)]
enum Command {
    Author {
        #[arg(long)]
        server_name: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, value_enum, default_value_t = CliTransport::Stdio)]
        transport: CliTransport,
        #[arg(long = "tool", value_name = "NAME[=DESCRIPTION]")]
        tools: Vec<String>,
        #[arg(long)]
        force: bool,
    },
    Analyze {
        #[arg(long)]
        descriptor: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CliTransport {
    Stdio,
    Http,
}

impl From<CliTransport> for McpTransportKind {
    fn from(value: CliTransport) -> Self {
        match value {
            CliTransport::Stdio => McpTransportKind::Stdio,
            CliTransport::Http => McpTransportKind::Http,
        }
    }
}

struct MachineContext {
    format: OutputFormat,
    non_interactive: bool,
    correlation_id: String,
    command: Vec<String>,
}

fn resolve_correlation_id(correlation_id: Option<String>, prefix: &str) -> String {
    correlation_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            format!("{prefix}-{timestamp}")
        })
}

fn build_success_envelope<T: Serialize>(
    context: &MachineContext,
    data: T,
) -> CliEnvelope<T> {
    CliEnvelope {
        schema_version: CLI_SCHEMA_VERSION,
        correlation_id: context.correlation_id.clone(),
        non_interactive: context.non_interactive,
        command: context.command.clone(),
        status: "ok".to_string(),
        data_schema: None,
        data: Some(data),
        failure: None,
    }
}

fn build_failure_envelope<T: Serialize>(
    context: &MachineContext,
    kind: &str,
    message: String,
    correlation_id: String,
) -> CliEnvelope<T> {
    let (status, code, category) = match kind {
        "invalid_input" => ("invalid", "CLI-INVALID-INPUT", "invalidInput"),
        "runtime" => ("error", "CLI-RUNTIME-FAILURE", "internal"),
        _ => ("error", "CLI-RUNTIME-FAILURE", "internal"),
    };
    CliEnvelope {
        schema_version: CLI_SCHEMA_VERSION,
        correlation_id: correlation_id.clone(),
        non_interactive: context.non_interactive,
        command: context.command.clone(),
        status: status.to_string(),
        data_schema: None,
        data: None,
        failure: Some(StructuredFailure {
            code: code.to_string(),
            message,
            category: category.to_string(),
            retryable: false,
            correlation_id: Some(correlation_id),
            details: None,
            cause: None,
        }),
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let output_format = if cli.json {
        OutputFormat::Json
    } else {
        cli.format
    };
    let correlation_id = resolve_correlation_id(cli.correlation_id, "elegy-mcp");

    match cli.command {
        Command::Author {
            server_name,
            output,
            transport,
            tools,
            force,
        } => {
            let ctx = MachineContext {
                format: output_format,
                non_interactive: cli.non_interactive,
                correlation_id: correlation_id.clone(),
                command: vec!["author".to_string()],
            };
            execute_author_command(server_name, output, transport, tools, force, &ctx)
        }
        Command::Analyze { descriptor } => {
            let ctx = MachineContext {
                format: output_format,
                non_interactive: cli.non_interactive,
                correlation_id: correlation_id.clone(),
                command: vec!["analyze".to_string()],
            };
            execute_analyze_command(descriptor, &ctx)
        }
    }
}

fn execute_author_command(
    server_name: String,
    output: PathBuf,
    transport: CliTransport,
    tools: Vec<String>,
    force: bool,
    ctx: &MachineContext,
) -> ExitCode {
    let tools = match parse_tool_specs(&tools) {
        Ok(tools) => tools,
        Err(message) => {
            if ctx.format == OutputFormat::Json {
                print_json_failure(ctx, "invalid_input", message);
            } else {
                eprintln!("{message}");
            }
            return ExitCode::from(EXIT_CODE_INVALID_INPUT);
        }
    };

    match author_mcp_descriptor_to_path(
        AuthorMcpDescriptorRequest {
            server_name,
            transport: transport.into(),
            tools,
        },
        &output,
        force,
    ) {
        Ok(result) => {
            match ctx.format {
                OutputFormat::Text => print_authored_mcp_text(&result),
                OutputFormat::Json => {
                    if let Err(e) = print_json(&build_success_envelope(ctx, result)) {
                        eprintln!("json serialization error: {e}");
                    }
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            if ctx.format == OutputFormat::Json {
                print_json_failure(ctx, "invalid_input", error.to_string());
            } else {
                eprintln!("{error}");
            }
            ExitCode::from(EXIT_CODE_INVALID_INPUT)
        }
    }
}

fn execute_analyze_command(
    descriptor: PathBuf,
    ctx: &MachineContext,
) -> ExitCode {
    match analyze_mcp_descriptor_file(&descriptor) {
        Ok(analysis) => {
            match ctx.format {
                OutputFormat::Text => print_mcp_analysis_text(&analysis),
                OutputFormat::Json => {
                    if let Err(e) = print_json(&build_success_envelope(ctx, analysis)) {
                        eprintln!("json serialization error: {e}");
                    }
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            if ctx.format == OutputFormat::Json {
                print_json_failure(ctx, "invalid_input", error.to_string());
            } else {
                eprintln!("{error}");
            }
            ExitCode::from(EXIT_CODE_INVALID_INPUT)
        }
    }
}

fn print_json_failure(ctx: &MachineContext, kind: &str, message: String) {
    let envelope = build_failure_envelope::<Value>(ctx, kind, message, ctx.correlation_id.clone());
    if let Err(e) = print_json(&envelope) {
        eprintln!("json serialization error: {e}");
    }
}

fn print_authored_mcp_text(result: &elegy_mcp::AuthoredMcpDescriptor) {
    println!("authored MCP descriptor");
    println!("server: {}", result.descriptor.server_name);
    println!(
        "transport: {}",
        format_transport(result.descriptor.transport)
    );
    println!("tools: {}", result.descriptor.tools.len());
    println!("output: {}", result.output_path);
}

fn print_mcp_analysis_text(analysis: &McpAnalysisResult) {
    println!("analyzed MCP descriptor");
    println!("server: {}", analysis.server_name);
    println!("tools: {}", analysis.analyses.len());
    for tool in &analysis.analyses {
        let schema_status = if tool.has_valid_schema {
            "valid_schema"
        } else {
            "missing_schema"
        };
        println!("- {} [{}]", tool.tool.name, schema_status);
    }
}

fn format_transport(transport: McpTransportKind) -> &'static str {
    match transport {
        McpTransportKind::Stdio => "stdio",
        McpTransportKind::Http => "http",
    }
}

fn parse_tool_specs(values: &[String]) -> Result<Vec<AuthorMcpToolRequest>, String> {
    values
        .iter()
        .map(|value| {
            let (name, description) = match value.split_once('=') {
                Some((name, description)) => (name.trim(), Some(description.trim())),
                None => (value.trim(), None),
            };

            if name.is_empty() {
                return Err(format!(
                    "tool specification `{value}` is invalid; expected NAME or NAME=DESCRIPTION"
                ));
            }

            Ok(AuthorMcpToolRequest {
                name: name.to_string(),
                description: description
                    .filter(|description| !description.is_empty())
                    .map(str::to_string),
            })
        })
        .collect()
}

fn print_json(value: &impl Serialize) -> Result<(), serde_json::Error> {
    let text = serde_json::to_string_pretty(value)?;
    println!("{text}");
    Ok(())
}
