mod error;
mod host;
pub mod logging;
mod preflight;

pub use error::HostError;
pub use host::{serve_stdio, serve_stdio_with_options, ElegyMcpHost, HostOptions};
pub use preflight::{run_preflight, PreflightOutcome};
