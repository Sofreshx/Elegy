use std::process::ExitCode;

fn main() -> ExitCode {
    match elegy_memory::cli::run_from_env() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("unexpected CLI failure: {error}");
            ExitCode::from(2)
        }
    }
}
