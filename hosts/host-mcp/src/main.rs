use clap::Parser;
use elegy_core::ProjectLocator;
use elegy_host_mcp::{serve_stdio_with_options, HostOptions};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "elegy-run", about = "Run Elegy as an MCP stdio host")]
struct Cli {
    /// Path to the project root or elegy.toml file
    #[arg(long, default_value = ".")]
    project: PathBuf,

    /// Allow side-effecting tools (default: false)
    #[arg(long, default_value_t = false)]
    allow_side_effects: bool,

    /// Default tool timeout in seconds
    #[arg(long, default_value_t = 30)]
    tool_timeout: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let locator = ProjectLocator::Path(cli.project);
    let options = HostOptions {
        allow_side_effects: cli.allow_side_effects,
        default_tool_timeout_seconds: cli.tool_timeout,
        max_tool_output_bytes: 1_048_576,
    };

    serve_stdio_with_options(locator, options).await?;
    Ok(())
}
