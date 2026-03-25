use std::process::ExitCode;

fn main() -> ExitCode {
    match elegy_memory::cli::run_from_env() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
