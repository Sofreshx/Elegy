use std::process::ExitCode;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(true)
        .with_writer(std::io::stderr)
        .init();

    match elegy_memory::cli::run_from_env() {
        Ok(code) => code,
        Err(error) => {
            if elegy_memory::cli::has_machine_context()
                && elegy_memory::cli::emit_machine_failure(&error).is_ok()
            {
                return ExitCode::from(1);
            }
            tracing::error!("{error}");
            ExitCode::from(1)
        }
    }
}
