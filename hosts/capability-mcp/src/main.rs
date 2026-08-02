use clap::Parser;
use elegy_capability_mcp::{BridgeOptions, CapabilityMcpBridge};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "elegy-capability-mcp",
    about = "Expose an Elegy capability package through a generic MCP stdio bridge"
)]
struct Cli {
    /// Root directory containing elegy-package.json.
    #[arg(long, default_value = ".")]
    package: PathBuf,

    /// Exact elegy-lock/v1 file required for this bridge instance.
    #[arg(long)]
    lock: Option<PathBuf>,

    /// Target selected by the exact lock.
    #[arg(long)]
    target: Option<String>,

    /// Allow mutation and fenced-mutation capabilities to be exposed.
    #[arg(long, default_value_t = false)]
    allow_side_effects: bool,

    /// Include concept and implemented capabilities for maintainer inspection.
    #[arg(long, default_value_t = false)]
    allow_non_routable: bool,

    /// Expose only these capability IDs. Repeat the option for multiple IDs.
    #[arg(long = "capability")]
    capabilities: Vec<String>,

    /// Maximum execution time per CLI call.
    #[arg(long, default_value_t = 30)]
    timeout_seconds: u64,

    /// Maximum stdout or stderr bytes accepted from a CLI call.
    #[arg(long, default_value_t = 1_048_576)]
    max_output_bytes: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let allowed = (!cli.capabilities.is_empty())
        .then(|| cli.capabilities.into_iter().collect::<BTreeSet<_>>());
    let options = BridgeOptions {
        package_root: cli.package,
        lock_path: cli.lock,
        target: cli.target,
        allow_side_effects: cli.allow_side_effects,
        allow_non_routable: cli.allow_non_routable,
        allowed_capabilities: allowed,
        timeout: Duration::from_secs(cli.timeout_seconds),
        max_output_bytes: cli.max_output_bytes,
    };
    CapabilityMcpBridge::load(options)?.serve_stdio().await?;
    Ok(())
}
