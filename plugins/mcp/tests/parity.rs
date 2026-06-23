use std::fs;
use std::path::{Path, PathBuf};

use elegy_contracts::{resolve_upstream_contracts_dir, McpAnalysisResult, McpServerDescriptor};
use elegy_mcp::McpToolAnalyzer;
use serde::de::DeserializeOwned;

#[test]
fn analyze_shared_fixture_matches_expected_analysis_golden() {
    let contracts_dir = contracts_dir();
    let descriptor = load_json::<McpServerDescriptor>(
        &contracts_dir
            .join("fixtures")
            .join("mcp-server-descriptor.parity.json"),
    );
    let expected = load_json::<McpAnalysisResult>(
        &contracts_dir
            .join("fixtures")
            .join("mcp-analysis-result.parity.json"),
    );

    let actual = McpToolAnalyzer.analyze(&descriptor);

    assert_eq!(expected, actual);
}

fn contracts_dir() -> PathBuf {
    resolve_upstream_contracts_dir()
}

fn load_json<T>(path: &Path) -> T
where
    T: DeserializeOwned,
{
    let json = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&json)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}
