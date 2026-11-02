//! Stderr-only structured logging for the `elegy-run` MCP stdio host.
//!
//! stdout is reserved for the MCP transport (`rmcp::transport::stdio()`), so
//! this must never write there. Mirrors the stderr-only pattern already used
//! by `hosts/memory-mcp/src/observability.rs`, simplified to a single text
//! format since this host has no JSON/text toggle requirement.

use std::env;

use tracing_subscriber::EnvFilter;

const RUST_LOG: &str = "RUST_LOG";

/// Initialize a stderr-only tracing subscriber. Safe to call once per
/// process; a second call is a no-op (errors from `try_init` are ignored).
pub fn init_logging() {
    let filter = build_env_filter();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init();
}

fn build_env_filter() -> EnvFilter {
    match env::var(RUST_LOG) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                EnvFilter::new("info")
            } else {
                match EnvFilter::try_new(trimmed) {
                    Ok(filter) => filter,
                    Err(parse_error) => {
                        eprintln!(
                            "warning: {RUST_LOG}={trimmed:?} is invalid ({parse_error}); defaulting to info"
                        );
                        EnvFilter::new("info")
                    }
                }
            }
        }
        Err(env::VarError::NotPresent) => EnvFilter::new("info"),
        Err(env::VarError::NotUnicode(_)) => {
            eprintln!("warning: {RUST_LOG} must be valid Unicode; defaulting to info");
            EnvFilter::new("info")
        }
    }
}
