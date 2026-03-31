use clap::{Parser, Subcommand, ValueEnum};
use elegy_contracts::{McpAnalysisResult, McpTransportKind};
use elegy_mcp::{
    analyze_mcp_descriptor_file, author_mcp_descriptor_to_path, AuthorMcpDescriptorRequest,
    AuthorMcpToolRequest,
};
use serde::Serialize;
use std::path::PathBuf;
use std::process::ExitCode;

const EXIT_CODE_INVALID_INPUT: u8 = 1;
const EXIT_CODE_RUNTIME_FAILURE: u8 = 2;

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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MachineEnvelope<T>
where
    T: Serialize,
{
    #[serde(skip_serializing_if = "Option::is_none")]
    correlation_id: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    non_interactive: bool,
    command: Vec<String>,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Clone, Debug)]
struct MachineContext {
    format: OutputFormat,
    non_interactive: bool,
    correlation_id: Option<String>,
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

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("unexpected CLI failure: {error}");
            exit_runtime()
        }
    }
}

fn run() -> Result<ExitCode, serde_json::Error> {
    let cli = Cli::parse();
    let context = MachineContext {
        format: resolve_output_format(cli.json, cli.format),
        non_interactive: cli.non_interactive,
        correlation_id: cli.correlation_id,
    };

    match cli.command {
        Command::Author {
            server_name,
            output,
            transport,
            tools,
            force,
        } => execute_author_command(server_name, output, transport, tools, force, &context),
        Command::Analyze { descriptor } => execute_analyze_command(descriptor, &context),
    }
}

fn execute_author_command(
    server_name: String,
    output: PathBuf,
    transport: CliTransport,
    tools: Vec<String>,
    force: bool,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    let tools = match parse_tool_specs(&tools) {
        Ok(tools) => tools,
        Err(message) => return emit_error(context, "author", message, exit_invalid()),
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
            match context.format {
                OutputFormat::Text => print_authored_mcp_text(&result),
                OutputFormat::Json => print_json(&build_success_envelope(
                    context,
                    ["author"],
                    result,
                ))?,
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_error(context, "author", error.to_string(), exit_invalid()),
    }
}

fn execute_analyze_command(
    descriptor: PathBuf,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    match analyze_mcp_descriptor_file(&descriptor) {
        Ok(analysis) => {
            match context.format {
                OutputFormat::Text => print_mcp_analysis_text(&analysis),
                OutputFormat::Json => print_json(&build_success_envelope(
                    context,
                    ["analyze"],
                    analysis,
                ))?,
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_error(context, "analyze", error.to_string(), exit_invalid()),
    }
}

fn emit_error(
    context: &MachineContext,
    command: &str,
    message: String,
    code: ExitCode,
) -> Result<ExitCode, serde_json::Error> {
    match context.format {
        OutputFormat::Text => eprintln!("{message}"),
        OutputFormat::Json => print_json(&MachineEnvelope::<serde_json::Value> {
            correlation_id: context.correlation_id.clone(),
            non_interactive: context.non_interactive,
            command: vec![command.to_string()],
            status: "error",
            data: None,
            error: Some(message),
        })?,
    }

    Ok(code)
}

fn resolve_output_format(json: bool, format: OutputFormat) -> OutputFormat {
    if json {
        OutputFormat::Json
    } else {
        format
    }
}

fn build_success_envelope<T, S>(
    context: &MachineContext,
    command: impl IntoIterator<Item = S>,
    data: T,
) -> MachineEnvelope<T>
where
    T: Serialize,
    S: Into<String>,
{
    MachineEnvelope {
        correlation_id: context.correlation_id.clone(),
        non_interactive: context.non_interactive,
        command: command.into_iter().map(Into::into).collect(),
        status: "ok",
        data: Some(data),
        error: None,
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn exit_invalid() -> ExitCode {
    ExitCode::from(EXIT_CODE_INVALID_INPUT)
}

fn exit_runtime() -> ExitCode {
    ExitCode::from(EXIT_CODE_RUNTIME_FAILURE)
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

fn print_json<T>(value: &T) -> Result<(), serde_json::Error>
where
    T: Serialize,
{
    let text = serde_json::to_string_pretty(value)?;
    println!("{text}");
    Ok(())
}
