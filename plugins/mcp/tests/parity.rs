use std::fs;
use std::path::{Path, PathBuf};

use elegy_mcp::{McpAnalysisResult, McpServerDescriptor, McpToolAnalyzer};
use serde::de::DeserializeOwned;

#[test]
fn analyze_shared_fixture_matches_expected_analysis_golden() {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let descriptor = load_json::<McpServerDescriptor>(
        &fixture_dir.join("mcp-server-descriptor.parity.json"),
    );
    let expected = load_json::<McpAnalysisResult>(
        &fixture_dir.join("mcp-analysis-result.parity.json"),
    );

    let actual = McpToolAnalyzer.analyze(&descriptor);

    assert_eq!(expected, actual);
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
