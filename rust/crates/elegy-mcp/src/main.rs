use clap::{Parser, Subcommand, ValueEnum};
use elegy_contracts::{McpAnalysisResult, McpTransportKind};
use elegy_mcp::{
    analyze_mcp_descriptor_file, author_mcp_descriptor_to_path, AuthorMcpDescriptorRequest,
    AuthorMcpToolRequest,
};
use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "elegy-mcp")]
#[command(about = "Dedicated MCP authoring and analysis CLI for Elegy")]
struct Cli {
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    format: OutputFormat,
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

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("unexpected CLI failure: {error}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, serde_json::Error> {
    let cli = Cli::parse();

    match cli.command {
        Command::Author {
            server_name,
            output,
            transport,
            tools,
            force,
        } => execute_author_command(server_name, output, transport, tools, force, cli.format),
        Command::Analyze { descriptor } => execute_analyze_command(descriptor, cli.format),
    }
}

fn execute_author_command(
    server_name: String,
    output: PathBuf,
    transport: CliTransport,
    tools: Vec<String>,
    force: bool,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let tools = match parse_tool_specs(&tools) {
        Ok(tools) => tools,
        Err(message) => return emit_error(format, "author", message, ExitCode::from(1)),
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
            match format {
                OutputFormat::Text => print_authored_mcp_text(&result),
                OutputFormat::Json => print_json(&json!({
                    "command": ["author"],
                    "status": "ok",
                    "data": result,
                }))?,
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_error(format, "author", error.to_string(), ExitCode::from(1)),
    }
}

fn execute_analyze_command(
    descriptor: PathBuf,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    match analyze_mcp_descriptor_file(&descriptor) {
        Ok(analysis) => {
            match format {
                OutputFormat::Text => print_mcp_analysis_text(&analysis),
                OutputFormat::Json => print_json(&json!({
                    "command": ["analyze"],
                    "status": "ok",
                    "data": analysis,
                }))?,
            }

            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_error(format, "analyze", error.to_string(), ExitCode::from(1)),
    }
}

fn emit_error(
    format: OutputFormat,
    command: &str,
    message: String,
    code: ExitCode,
) -> Result<ExitCode, serde_json::Error> {
    match format {
        OutputFormat::Text => eprintln!("{message}"),
        OutputFormat::Json => print_json(&json!({
            "command": [command],
            "status": "error",
            "error": message,
        }))?,
    }

    Ok(code)
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
