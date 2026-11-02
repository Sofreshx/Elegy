use clap::Parser;
use elegy_core::ProjectLocator;
use elegy_host_mcp::{logging, run_preflight, serve_stdio_with_options, HostOptions};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "elegy-run",
    about = "Run Elegy as an MCP stdio host",
    version = env!("CARGO_PKG_VERSION")
)]
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

    /// Resolve and compose the project, report the result, and exit without
    /// opening the MCP transport. Exits 0 when the project loads cleanly, 1
    /// when descriptor configuration fails to load.
    #[arg(long, default_value_t = false)]
    preflight: bool,
}

#[tokio::main]
async fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    logging::init_logging();
    let cli = Cli::parse();

    let locator = ProjectLocator::Path(cli.project);

    if cli.preflight {
        let outcome = run_preflight(locator);
        let code = outcome.report();
        return Ok(ExitCode::from(code as u8));
    }

    let options = HostOptions {
        allow_side_effects: cli.allow_side_effects,
        default_tool_timeout_seconds: cli.tool_timeout,
        max_tool_output_bytes: 1_048_576,
    };

    serve_stdio_with_options(locator, options).await?;
    Ok(ExitCode::SUCCESS)
}
