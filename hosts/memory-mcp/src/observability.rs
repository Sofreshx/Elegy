use std::env;
use std::future::Future;
use std::sync::OnceLock;
use std::time::Instant;

use rmcp::ErrorData;
use tracing::Instrument;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

const ELEGY_MCP_LOG_CONTENT: &str = "ELEGY_MCP_LOG_CONTENT";
const ELEGY_MCP_LOG_FORMAT: &str = "ELEGY_MCP_LOG_FORMAT";
const RUST_LOG: &str = "RUST_LOG";

static CONTENT_LOGGING: OnceLock<bool> = OnceLock::new();

pub fn content_logging_enabled() -> bool {
    *CONTENT_LOGGING.get_or_init(|| {
        let raw = env::var(ELEGY_MCP_LOG_CONTENT).ok();
        parse_bool_env(raw.as_deref())
    })
}

fn parse_bool_env(raw: Option<&str>) -> bool {
    match raw {
        Some(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        None => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Text,
}

impl LogFormat {
    pub fn from_env() -> LogFormat {
        let raw = env::var(ELEGY_MCP_LOG_FORMAT).ok();
        parse_log_format(raw.as_deref())
    }
}

fn parse_log_format(raw: Option<&str>) -> LogFormat {
    match raw {
        Some(value) => match value.trim().to_ascii_lowercase().as_str() {
            "json" => LogFormat::Json,
            "text" => LogFormat::Text,
            other => {
                eprintln!(
                    "warning: {ELEGY_MCP_LOG_FORMAT}={other:?} is invalid; defaulting to json"
                );
                LogFormat::Json
            }
        },
        None => LogFormat::Json,
    }
}

pub fn init_logging(format: LogFormat) {
    let filter = build_env_filter();
    match format {
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .with_writer(std::io::stderr)
                        .with_ansi(false),
                )
                .init();
        }
        LogFormat::Text => {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(std::io::stderr)
                        .with_ansi(false),
                )
                .init();
        }
    }
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

pub async fn instrumented_tool<F, T>(tool: &'static str, future: F) -> Result<T, ErrorData>
where
    F: Future<Output = Result<T, ErrorData>>,
{
    let start = Instant::now();
    let span = tracing::info_span!("mcp.tool", tool);
    let result = future.instrument(span).await;
    let elapsed = start.elapsed();
    match &result {
        Ok(_) => {
            tracing::info!(
                tool,
                duration_ms = elapsed.as_millis() as u64,
                outcome = "ok",
                "mcp tool call completed"
            );
        }
        Err(error) => {
            tracing::warn!(
                tool,
                duration_ms = elapsed.as_millis() as u64,
                outcome = "error",
                error_code = ?error.code,
                error = %error.message,
                "mcp tool call failed"
            );
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bool_env_true_values() {
        assert!(parse_bool_env(Some("true")));
        assert!(parse_bool_env(Some("TRUE")));
        assert!(parse_bool_env(Some("1")));
        assert!(parse_bool_env(Some("yes")));
        assert!(parse_bool_env(Some("YES")));
        assert!(parse_bool_env(Some("on")));
        assert!(parse_bool_env(Some("ON")));
        assert!(parse_bool_env(Some("  true  ")));
    }

    #[test]
    fn parse_bool_env_false_values() {
        assert!(!parse_bool_env(Some("false")));
        assert!(!parse_bool_env(Some("0")));
        assert!(!parse_bool_env(Some("no")));
        assert!(!parse_bool_env(Some("off")));
        assert!(!parse_bool_env(Some("anything")));
        assert!(!parse_bool_env(Some("")));
    }

    #[test]
    fn parse_bool_env_unset_is_false() {
        assert!(!parse_bool_env(None));
    }

    #[test]
    fn content_logging_enabled_defaults_to_false_when_unset() {
        let raw = env::var(ELEGY_MCP_LOG_CONTENT).ok();
        assert!(!parse_bool_env(raw.as_deref()));
    }

    #[test]
    fn parse_log_format_json_variants() {
        assert_eq!(parse_log_format(Some("json")), LogFormat::Json);
        assert_eq!(parse_log_format(Some("JSON")), LogFormat::Json);
        assert_eq!(parse_log_format(Some("Json")), LogFormat::Json);
    }

    #[test]
    fn parse_log_format_text_variants() {
        assert_eq!(parse_log_format(Some("text")), LogFormat::Text);
        assert_eq!(parse_log_format(Some("TEXT")), LogFormat::Text);
        assert_eq!(parse_log_format(Some("Text")), LogFormat::Text);
    }

    #[test]
    fn parse_log_format_unset_defaults_to_json() {
        assert_eq!(parse_log_format(None), LogFormat::Json);
    }

    #[test]
    fn parse_log_format_garbage_defaults_to_json() {
        assert_eq!(parse_log_format(Some("xml")), LogFormat::Json);
        assert_eq!(parse_log_format(Some("yaml")), LogFormat::Json);
    }

    #[tokio::test]
    async fn instrumented_tool_returns_ok_unchanged() {
        let result = instrumented_tool("test_tool", async { Ok::<_, ErrorData>(42) }).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn instrumented_tool_returns_err_unchanged() {
        let err = ErrorData::internal_error("test error".to_string(), None);
        let result: Result<(), ErrorData> =
            instrumented_tool("test_tool", async { Err(err) }).await;
        assert!(result.is_err());
    }
}
