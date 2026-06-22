use elegy_contracts::{
    validate_mcp_analysis_result, validate_mcp_server_descriptor, McpAnalysisResult,
    McpServerDescriptor, McpToolAnalysis, McpToolDefinition, McpTransportKind, SkillTrigger,
};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorMcpDescriptorRequest {
    pub server_name: String,
    pub transport: McpTransportKind,
    pub tools: Vec<AuthorMcpToolRequest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorMcpToolRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct AuthoredMcpDescriptor {
    pub output_path: String,
    pub descriptor: McpServerDescriptor,
}

#[derive(Debug, Error)]
pub enum McpSurfaceError {
    #[error("failed to {operation} {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid MCP descriptor in {path}")]
    InvalidMcpDescriptor { path: PathBuf, issues: Vec<String> },
    #[error("invalid MCP analysis result for {path}")]
    InvalidMcpAnalysis { path: PathBuf, issues: Vec<String> },
    #[error("output file already exists: {path}")]
    OutputExists { path: PathBuf },
}

pub fn author_mcp_descriptor_to_path(
    request: AuthorMcpDescriptorRequest,
    output_path: &Path,
    overwrite: bool,
) -> Result<AuthoredMcpDescriptor, McpSurfaceError> {
    let descriptor = build_mcp_descriptor(request)?;
    write_json_file(output_path, &descriptor, overwrite)?;

    Ok(AuthoredMcpDescriptor {
        output_path: display_path(output_path),
        descriptor,
    })
}

pub fn analyze_mcp_descriptor_file(path: &Path) -> Result<McpAnalysisResult, McpSurfaceError> {
    let descriptor = load_mcp_descriptor_file(path)?;
    let analysis = analyze_descriptor(&descriptor);
    let validation = validate_mcp_analysis_result(&analysis);

    if !validation.is_valid() {
        return Err(McpSurfaceError::InvalidMcpAnalysis {
            path: path.to_path_buf(),
            issues: validation.issues,
        });
    }

    Ok(analysis)
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

fn build_mcp_descriptor(
    request: AuthorMcpDescriptorRequest,
) -> Result<McpServerDescriptor, McpSurfaceError> {
    let descriptor = McpServerDescriptor {
        server_name: request.server_name,
        transport: request.transport,
        tools: request
            .tools
            .into_iter()
            .map(|tool| McpToolDefinition {
                name: tool.name,
                description: tool.description,
                input_schema: None,
            })
            .collect(),
    };

    let issues = descriptor_validation_issues(&descriptor);
    if !issues.is_empty() {
        return Err(McpSurfaceError::InvalidMcpDescriptor {
            path: PathBuf::from("<in-memory>"),
            issues,
        });
    }

    Ok(descriptor)
}

fn load_mcp_descriptor_file(path: &Path) -> Result<McpServerDescriptor, McpSurfaceError> {
    let content = fs::read_to_string(path).map_err(|source| McpSurfaceError::Io {
        operation: "read",
        path: path.to_path_buf(),
        source,
    })?;

    let descriptor = serde_json::from_str::<McpServerDescriptor>(&content).map_err(|source| {
        McpSurfaceError::Json {
            path: path.to_path_buf(),
            source,
        }
    })?;

    let issues = descriptor_validation_issues(&descriptor);
    if !issues.is_empty() {
        return Err(McpSurfaceError::InvalidMcpDescriptor {
            path: path.to_path_buf(),
            issues,
        });
    }

    Ok(descriptor)
}

fn descriptor_validation_issues(descriptor: &McpServerDescriptor) -> Vec<String> {
    validate_mcp_server_descriptor(descriptor).issues
}

fn analyze_descriptor(descriptor: &McpServerDescriptor) -> McpAnalysisResult {
    let mut analysis = McpToolAnalyzer.analyze(descriptor);
    for tool_analysis in &mut analysis.analyses {
        tool_analysis.has_valid_schema = tool_analysis
            .tool
            .input_schema
            .as_ref()
            .is_some_and(is_supported_input_schema);
    }

    analysis
}

fn is_supported_input_schema(value: &Value) -> bool {
    matches!(value, Value::Object(_))
}

fn write_json_file<T: Serialize>(
    output_path: &Path,
    value: &T,
    overwrite: bool,
) -> Result<(), McpSurfaceError> {
    if output_path.exists() && !overwrite {
        return Err(McpSurfaceError::OutputExists {
            path: output_path.to_path_buf(),
        });
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| McpSurfaceError::Io {
            operation: "create directory",
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let mut content =
        serde_json::to_string_pretty(value).map_err(|source| McpSurfaceError::Json {
            path: output_path.to_path_buf(),
            source,
        })?;
    content.push('\n');

    fs::write(output_path, content).map_err(|source| McpSurfaceError::Io {
        operation: "write",
        path: output_path.to_path_buf(),
        source,
    })
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
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

#[cfg(test)]
mod tests {
    use super::McpToolAnalyzer;
    use elegy_contracts::{McpServerDescriptor, McpToolDefinition};
    use serde_json::json;

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
}
