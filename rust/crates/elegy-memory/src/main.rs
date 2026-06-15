use std::process::ExitCode;

fn main() -> ExitCode {
    match elegy_memory::cli::run_from_env() {
        Ok(code) => code,
        Err(error) => {
            if elegy_memory::cli::has_machine_context()
                && elegy_memory::cli::emit_machine_failure(&error).is_ok()
            {
                return ExitCode::from(1);
            }
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
