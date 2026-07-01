mod error;
mod host;

pub use error::HostError;
pub use host::{serve_stdio, serve_stdio_with_options, ElegyMcpHost, HostOptions};
