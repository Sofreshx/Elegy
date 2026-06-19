use std::fs;
use std::path::{Path, PathBuf};

use elegy_contracts::{
    resolve_upstream_contracts_dir, McpAnalysisResult, McpServerDescriptor, McpToolDefinition,
    SkillDefinitionV2,
};
use elegy_mcp::{McpSkillGenerator, McpToolAnalyzer, McpToolResolveService, McpToolSearchService};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct McpParityExpectation {
    #[serde(default)]
    generated_skills: Vec<SkillDefinitionV2>,
    #[serde(default)]
    skipped_tool_names: Vec<String>,
    #[serde(default)]
    search: McpSearchExpectation,
    #[serde(default)]
    resolve: McpResolveExpectation,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct McpSearchExpectation {
    query: String,
    #[serde(default)]
    results: Vec<McpToolSummaryExpectation>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct McpToolSummaryExpectation {
    name: String,
    description: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct McpResolveExpectation {
    tool_name: String,
    #[serde(default)]
    result: McpToolDefinition,
}

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

    assert_eq!(canonical_json(&expected), canonical_json(&actual));
}

#[test]
fn generate_and_discovery_services_match_shared_parity_golden() {
    let contracts_dir = contracts_dir();
    let descriptor = load_json::<McpServerDescriptor>(
        &contracts_dir
            .join("fixtures")
            .join("mcp-server-descriptor.parity.json"),
    );
    let expected = load_json::<McpParityExpectation>(
        &contracts_dir
            .join("fixtures")
            .join("mcp-parity-expected.json"),
    );

    let analysis = McpToolAnalyzer.analyze(&descriptor);
    let generation = McpSkillGenerator.generate(&analysis);
    let search_results =
        McpToolSearchService.search(&descriptor, Some(expected.search.query.as_str()));
    let resolved = McpToolResolveService.resolve(&descriptor, &expected.resolve.tool_name);

    let actual = McpParityExpectation {
        generated_skills: generation.generated_skills,
        skipped_tool_names: generation
            .skipped_tools
            .into_iter()
            .map(|tool| tool.name)
            .collect(),
        search: McpSearchExpectation {
            query: expected.search.query.clone(),
            results: search_results
                .into_iter()
                .map(|summary| McpToolSummaryExpectation {
                    name: summary.name,
                    description: summary.description,
                })
                .collect(),
        },
        resolve: McpResolveExpectation {
            tool_name: expected.resolve.tool_name.clone(),
            result: resolved.unwrap_or_else(|| {
                panic!("shared parity fixture expected tool to resolve successfully")
            }),
        },
    };

    assert_eq!(canonical_json(&expected), canonical_json(&actual));
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

fn canonical_json<T>(value: &T) -> String
where
    T: Serialize,
{
    serde_json::to_string(value).unwrap_or_else(|error| panic!("failed to serialize JSON: {error}"))
}
