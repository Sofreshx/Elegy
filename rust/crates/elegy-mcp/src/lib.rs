use elegy_contracts::{
    McpAnalysisResult, McpServerDescriptor, McpToolAnalysis, McpToolDefinition, SkillDefinition,
    SkillDiscoveryMetadata, SkillGovernanceMetadata, SkillIdentity, SkillInputContract,
    SkillLifecycleState, SkillMaterializationKind, SkillMetadata, SkillOrigin, SkillSourceKind,
    SkillTrigger,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct McpSkillGenerationResult {
    pub generated_skills: Vec<SkillDefinition>,
    pub skipped_tools: Vec<McpToolDefinition>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct McpToolSummary {
    pub name: String,
    pub description: Option<String>,
}

pub struct McpToolAnalyzer;

impl McpToolAnalyzer {
    pub fn analyze(&self, descriptor: &McpServerDescriptor) -> McpAnalysisResult {
        McpAnalysisResult {
            server_name: descriptor.server_name.clone(),
            analyses: descriptor
                .tools
                .iter()
                .cloned()
                .map(|tool| McpToolAnalysis {
                    extracted_triggers: extract_triggers(&tool.name),
                    has_valid_schema: tool.input_schema.is_some(),
                    tool,
                })
                .collect(),
        }
    }
}

pub struct McpSkillGenerator;

impl McpSkillGenerator {
    pub fn generate(&self, analysis_result: &McpAnalysisResult) -> McpSkillGenerationResult {
        let mut generated = Vec::new();
        let mut skipped = Vec::new();

        for analysis in &analysis_result.analyses {
            if !analysis.has_valid_schema {
                skipped.push(analysis.tool.clone());
                continue;
            }

            let skill_id = generated_skill_id(&analysis_result.server_name, &analysis.tool.name);
            let slug = build_slug(&analysis_result.server_name, &analysis.tool.name);
            let source_ref = format!("mcp://{}/tools/{slug}", analysis_result.server_name);

            generated.push(SkillDefinition {
                id: skill_id.clone(),
                name: analysis.tool.name.clone(),
                description: analysis.tool.description.clone(),
                triggers: analysis.extracted_triggers.clone(),
                constraints: Vec::new(),
                identity: SkillIdentity {
                    definition_id: skill_id,
                    display_name: analysis.tool.name.clone(),
                    namespace: Some(analysis_result.server_name.clone()),
                    ..SkillIdentity::default()
                },
                metadata: SkillMetadata {
                    summary: analysis.tool.description.clone(),
                    category: Some("mcp".to_string()),
                    tags: build_keywords(&analysis_result.server_name, &analysis.tool.name),
                    ..SkillMetadata::default()
                },
                input: SkillInputContract {
                    schema_ref: analysis
                        .tool
                        .input_schema
                        .as_ref()
                        .map(|_| format!("{source_ref}/input-schema")),
                    ..SkillInputContract::default()
                },
                governance: SkillGovernanceMetadata {
                    allowed_contexts: vec!["mcp".to_string()],
                    ..SkillGovernanceMetadata::default()
                },
                discovery: SkillDiscoveryMetadata {
                    keywords: build_keywords(&analysis_result.server_name, &analysis.tool.name),
                    capability_hints: analysis
                        .extracted_triggers
                        .iter()
                        .map(|trigger| trigger.pattern.clone())
                        .collect(),
                    ..SkillDiscoveryMetadata::default()
                },
                origin: SkillOrigin {
                    materialization_kind: SkillMaterializationKind::Declared,
                    source_kind: SkillSourceKind::Generated,
                    source_ref: Some(source_ref),
                    ..SkillOrigin::default()
                },
                lifecycle_state: SkillLifecycleState::Draft,
                ..SkillDefinition::default()
            });
        }

        McpSkillGenerationResult {
            generated_skills: generated,
            skipped_tools: skipped,
        }
    }
}

pub fn generated_skill_id(server_name: &str, tool_name: &str) -> String {
    let slug = build_slug(server_name, tool_name);
    format!("mcp-{slug}")
}

pub struct McpToolSearchService;

impl McpToolSearchService {
    pub fn search(
        &self,
        descriptor: &McpServerDescriptor,
        query: Option<&str>,
    ) -> Vec<McpToolSummary> {
        match query.map(str::trim).filter(|query| !query.is_empty()) {
            None => descriptor
                .tools
                .iter()
                .map(|tool| McpToolSummary {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                })
                .collect(),
            Some(query) => descriptor
                .tools
                .iter()
                .filter(|tool| {
                    tool.name.contains(query)
                        || tool
                            .description
                            .as_ref()
                            .is_some_and(|description| contains_ignore_case(description, query))
                        || contains_ignore_case(&tool.name, query)
                })
                .map(|tool| McpToolSummary {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                })
                .collect(),
        }
    }
}

pub struct McpToolResolveService;

impl McpToolResolveService {
    pub fn resolve(
        &self,
        descriptor: &McpServerDescriptor,
        tool_name: &str,
    ) -> Option<McpToolDefinition> {
        descriptor
            .tools
            .iter()
            .find(|tool| tool.name == tool_name)
            .cloned()
    }
}

fn extract_triggers(tool_name: &str) -> Vec<SkillTrigger> {
    if tool_name.trim().is_empty() {
        return Vec::new();
    }

    let mut words = Vec::new();
    for part in tool_name.split(['-', '_']) {
        if part.is_empty() {
            continue;
        }

        words.extend(split_camel_case(part));
    }

    let pattern = words
        .into_iter()
        .map(|word| word.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ");

    vec![SkillTrigger {
        pattern,
        description: Some("Extracted from MCP tool name".to_string()),
    }]
}

fn split_camel_case(part: &str) -> Vec<String> {
    let chars = part.chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return Vec::new();
    }

    let mut words = Vec::new();
    let mut current = String::new();

    for (index, character) in chars.iter().enumerate() {
        if index > 0 {
            let previous = chars[index - 1];
            let next = chars.get(index + 1).copied();
            let boundary = (previous.is_ascii_lowercase() && character.is_ascii_uppercase())
                || (previous.is_ascii_uppercase()
                    && character.is_ascii_uppercase()
                    && next.is_some_and(|next| next.is_ascii_lowercase()));

            if boundary && !current.is_empty() {
                words.push(current);
                current = String::new();
            }
        }

        current.push(*character);
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
}

fn build_slug(server_name: &str, tool_name: &str) -> String {
    let combined = format!("{server_name}-{tool_name}");
    let mut slug = String::new();

    for character in combined.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if matches!(character, '-' | '_') {
            slug.push('-');
        }
    }

    slug.trim_matches('-').to_string()
}

fn build_keywords(server_name: &str, tool_name: &str) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut keywords = Vec::new();

    for keyword in std::iter::once(server_name)
        .chain(tool_name.split(['-', '_', ' ']))
        .filter(|keyword| !keyword.trim().is_empty())
        .map(|keyword| keyword.to_ascii_lowercase())
    {
        if seen.insert(keyword.clone()) {
            keywords.push(keyword);
        }
    }

    keywords
}

fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::{
        generated_skill_id, McpSkillGenerator, McpToolAnalyzer, McpToolResolveService,
        McpToolSearchService,
    };
    use elegy_contracts::{
        McpServerDescriptor, McpToolAnalysis, McpToolDefinition, SkillMaterializationKind,
        SkillSourceKind,
    };
    use serde_json::json;

    fn create_test_descriptor() -> McpServerDescriptor {
        McpServerDescriptor {
            server_name: "test-server".to_string(),
            tools: vec![
                McpToolDefinition {
                    name: "list-files".to_string(),
                    description: Some("List directory contents".to_string()),
                    input_schema: Some(json!({})),
                },
                McpToolDefinition {
                    name: "read-file".to_string(),
                    description: Some("Read a file by path".to_string()),
                    input_schema: Some(json!({})),
                },
                McpToolDefinition {
                    name: "search-code".to_string(),
                    description: Some("Search code with regex".to_string()),
                    input_schema: Some(json!({})),
                },
            ],
            ..McpServerDescriptor::default()
        }
    }

    fn create_test_analysis() -> elegy_contracts::McpAnalysisResult {
        elegy_contracts::McpAnalysisResult {
            server_name: "test-server".to_string(),
            analyses: vec![
                create_analysis("list-files", "List all files", true),
                create_analysis("read-content", "Read file contents", true),
                create_analysis("no-schema-tool", "Tool without schema", false),
            ],
        }
    }

    fn create_analysis(name: &str, description: &str, valid_schema: bool) -> McpToolAnalysis {
        McpToolAnalysis {
            tool: McpToolDefinition {
                name: name.to_string(),
                description: Some(description.to_string()),
                input_schema: valid_schema.then(|| json!({})),
            },
            extracted_triggers: vec![elegy_contracts::SkillTrigger {
                pattern: name.replace('-', " "),
                description: None,
            }],
            has_valid_schema: valid_schema,
        }
    }

    #[test]
    fn analyze_tool_with_valid_schema_extracts_triggers_and_marks_valid() {
        let analyzer = McpToolAnalyzer;
        let descriptor = McpServerDescriptor {
            server_name: "test-server".to_string(),
            tools: vec![McpToolDefinition {
                name: "get-user".to_string(),
                description: Some("Gets a user".to_string()),
                input_schema: Some(json!({ "type": "object" })),
            }],
            ..McpServerDescriptor::default()
        };

        let result = analyzer.analyze(&descriptor);

        assert_eq!(result.server_name, "test-server");
        assert_eq!(result.analyses.len(), 1);
        assert!(result.analyses[0].has_valid_schema);
        assert_eq!(result.analyses[0].extracted_triggers.len(), 1);
        assert_eq!(result.analyses[0].extracted_triggers[0].pattern, "get user");
        assert_eq!(
            result.analyses[0].extracted_triggers[0]
                .description
                .as_deref(),
            Some("Extracted from MCP tool name")
        );
    }

    #[test]
    fn analyze_tool_without_schema_marks_invalid() {
        let analyzer = McpToolAnalyzer;
        let descriptor = McpServerDescriptor {
            server_name: "no-schema-server".to_string(),
            tools: vec![McpToolDefinition {
                name: "listItems".to_string(),
                description: Some("Lists items".to_string()),
                ..McpToolDefinition::default()
            }],
            ..McpServerDescriptor::default()
        };

        let result = analyzer.analyze(&descriptor);

        assert!(!result.analyses[0].has_valid_schema);
        assert_eq!(
            result.analyses[0].extracted_triggers[0].pattern,
            "list items"
        );
    }

    #[test]
    fn generated_skill_id_and_source_ref_preserve_mcp_prefixed_server_names() {
        let generator = McpSkillGenerator;
        let analysis = elegy_contracts::McpAnalysisResult {
            server_name: "mcp-server".to_string(),
            analyses: vec![create_analysis("list-items", "List items", true)],
        };

        let generated = generator.generate(&analysis);

        assert_eq!(generated.generated_skills.len(), 1);
        assert_eq!(
            generated_skill_id("mcp-server", "list-items"),
            "mcp-mcp-server-list-items"
        );
        assert_eq!(
            generated.generated_skills[0].origin.source_ref.as_deref(),
            Some("mcp://mcp-server/tools/mcp-server-list-items")
        );
        assert_eq!(
            generated.generated_skills[0].input.schema_ref.as_deref(),
            Some("mcp://mcp-server/tools/mcp-server-list-items/input-schema")
        );
    }

    #[test]
    fn analyze_mixed_tools_returns_correct_count_and_results() {
        let analyzer = McpToolAnalyzer;
        let descriptor = McpServerDescriptor {
            server_name: "mixed-server".to_string(),
            tools: vec![
                McpToolDefinition {
                    name: "get-user".to_string(),
                    input_schema: Some(json!({ "type": "object" })),
                    ..McpToolDefinition::default()
                },
                McpToolDefinition {
                    name: "create_item".to_string(),
                    description: Some("Creates an item".to_string()),
                    ..McpToolDefinition::default()
                },
                McpToolDefinition {
                    name: "fetchOrderDetails".to_string(),
                    input_schema: Some(json!({ "type": "object" })),
                    ..McpToolDefinition::default()
                },
            ],
            ..McpServerDescriptor::default()
        };

        let result = analyzer.analyze(&descriptor);

        assert_eq!(result.server_name, "mixed-server");
        assert_eq!(result.analyses.len(), 3);
        assert!(result.analyses[0].has_valid_schema);
        assert_eq!(result.analyses[0].extracted_triggers[0].pattern, "get user");
        assert!(!result.analyses[1].has_valid_schema);
        assert_eq!(
            result.analyses[1].extracted_triggers[0].pattern,
            "create item"
        );
        assert!(result.analyses[2].has_valid_schema);
        assert_eq!(
            result.analyses[2].extracted_triggers[0].pattern,
            "fetch order details"
        );
    }

    #[test]
    fn generate_valid_tools_creates_skills_with_mcp_prefix() {
        let generator = McpSkillGenerator;
        let result = generator.generate(&create_test_analysis());

        assert_eq!(result.generated_skills.len(), 2);
        assert!(result
            .generated_skills
            .iter()
            .all(|skill| skill.effective_id().starts_with("mcp-")));
    }

    #[test]
    fn generate_invalid_schema_skips_tools() {
        let generator = McpSkillGenerator;
        let result = generator.generate(&create_test_analysis());

        assert_eq!(result.skipped_tools.len(), 1);
        assert_eq!(result.skipped_tools[0].name, "no-schema-tool");
    }

    #[test]
    fn generate_uses_canonical_origin_and_discovery_metadata() {
        let generator = McpSkillGenerator;
        let result = generator.generate(&create_test_analysis());

        for skill in result.generated_skills {
            assert_eq!(skill.origin.source_kind, SkillSourceKind::Generated);
            assert_eq!(
                skill.origin.materialization_kind,
                SkillMaterializationKind::Declared
            );
            assert!(skill
                .origin
                .source_ref
                .as_deref()
                .is_some_and(|value| value.starts_with("mcp://test-server/tools/")));
            assert!(skill
                .discovery
                .keywords
                .contains(&"test-server".to_string()));
        }
    }

    #[test]
    fn generate_lifecycle_state_is_draft() {
        let generator = McpSkillGenerator;
        let result = generator.generate(&create_test_analysis());

        assert!(result
            .generated_skills
            .iter()
            .all(|skill| skill.lifecycle_state == elegy_contracts::SkillLifecycleState::Draft));
    }

    #[test]
    fn generate_input_schema_ref_present_when_tool_has_schema() {
        let generator = McpSkillGenerator;
        let result = generator.generate(&create_test_analysis());

        assert!(result.generated_skills.iter().all(|skill| skill
            .input
            .schema_ref
            .as_deref()
            .is_some_and(|value| value.contains("/input-schema"))));
    }

    #[test]
    fn search_null_query_returns_all() {
        let search = McpToolSearchService;
        let results = search.search(&create_test_descriptor(), None);

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|result| !result.name.is_empty()));
    }

    #[test]
    fn search_filter_query_narrows_results() {
        let search = McpToolSearchService;
        let results = search.search(&create_test_descriptor(), Some("file"));

        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|result| result.name == "list-files"));
        assert!(results.iter().any(|result| result.name == "read-file"));
    }

    #[test]
    fn resolve_existing_tool_returns_full_definition() {
        let resolve = McpToolResolveService;
        let result = resolve.resolve(&create_test_descriptor(), "search-code");

        assert!(result.is_some());
        let result = result.expect("existing tool");
        assert_eq!(result.name, "search-code");
        assert!(result.input_schema.is_some());
    }

    #[test]
    fn resolve_missing_tool_returns_none() {
        let resolve = McpToolResolveService;
        let result = resolve.resolve(&create_test_descriptor(), "nonexistent");

        assert!(result.is_none());
    }
}
