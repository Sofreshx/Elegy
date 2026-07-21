use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use elegy_plugin_sdk::generate_plugin_schema_artifacts;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mode = env::args().nth(1).unwrap_or_else(|| "--check".to_string());
    if !matches!(mode.as_str(), "--check" | "--write") {
        return Err("usage: elegy-plugin-schemas [--check|--write]".to_string());
    }

    let schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas");
    let artifacts = generate_plugin_schema_artifacts().map_err(|error| error.to_string())?;
    let mut drifted = Vec::new();

    for (file_name, expected) in artifacts {
        let path = schema_dir.join(file_name);
        if mode == "--write" {
            write_schema(&path, &expected)?;
            continue;
        }
        match fs::read_to_string(&path) {
            Ok(actual) if generated_content_matches(&actual, &expected) => {}
            _ => drifted.push(path),
        }
    }

    if drifted.is_empty() {
        return Ok(());
    }

    let paths = drifted
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "plugin schema artifacts are missing or stale: {paths}; run `cargo run -p elegy-plugin-sdk --bin elegy-plugin-schemas -- --write`"
    ))
}

fn write_schema(path: &Path, content: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("schema path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("create {}: {error}", parent.display()))?;
    fs::write(path, content).map_err(|error| format!("write {}: {error}", path.display()))
}

fn generated_content_matches(actual: &str, expected: &str) -> bool {
    actual.replace("\r\n", "\n") == expected
}
