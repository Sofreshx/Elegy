// ── MCP (Model Context Protocol) types ───────────────────────────────────

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerDescriptor {
    pub server_name: String,
    #[serde(default)]
    pub transport: McpTransportKind,
    #[serde(default)]
    pub tools: Vec<McpToolDefinition>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum McpTransportKind {
    #[default]
    Stdio,
    Http,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpAnalysisResult {
    pub server_name: String,
    #[serde(default)]
    pub analyses: Vec<McpToolAnalysis>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillTrigger {
    pub pattern: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpToolAnalysis {
    #[serde(default)]
    pub tool: McpToolDefinition,
    #[serde(default)]
    pub extracted_triggers: Vec<SkillTrigger>,
    pub has_valid_schema: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct McpValidationResult {
    pub issues: Vec<String>,
}

impl McpValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn has_duplicate_values<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut distinct = BTreeSet::new();

    for value in values {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            continue;
        }

        if !distinct.insert(normalized) {
            return true;
        }
    }

    false
}

// ── MCP validation ────────────────────────────────────────────────────────

pub fn validate_mcp_server_descriptor(descriptor: &McpServerDescriptor) -> McpValidationResult {
    let mut issues = Vec::new();

    if descriptor.server_name.trim().is_empty() {
        issues.push("MCP server descriptor must declare a server name.".to_string());
    }

    if descriptor
        .tools
        .iter()
        .any(|tool| tool.name.trim().is_empty())
    {
        issues.push("MCP server descriptor tools must define a non-empty name.".to_string());
    }

    if has_duplicate_values(descriptor.tools.iter().map(|tool| tool.name.as_str())) {
        issues.push("MCP server descriptor tool names must be unique.".to_string());
    }

    McpValidationResult { issues }
}

pub fn validate_mcp_analysis_result(result: &McpAnalysisResult) -> McpValidationResult {
    let mut issues = Vec::new();

    if result.server_name.trim().is_empty() {
        issues.push("MCP analysis result must declare a server name.".to_string());
    }

    if result
        .analyses
        .iter()
        .any(|analysis| analysis.tool.name.trim().is_empty())
    {
        issues.push("MCP analysis entries must define a non-empty tool name.".to_string());
    }

    if has_duplicate_values(
        result
            .analyses
            .iter()
            .map(|analysis| analysis.tool.name.as_str()),
    ) {
        issues.push("MCP analysis entries must be unique per tool name.".to_string());
    }

    if result.analyses.iter().any(|analysis| {
        analysis
            .extracted_triggers
            .iter()
            .any(|trigger| trigger.pattern.trim().is_empty())
    }) {
        issues.push("MCP analysis extracted triggers must define a non-empty pattern.".to_string());
    }

    if result
        .analyses
        .iter()
        .any(|analysis| analysis.has_valid_schema && analysis.tool.input_schema.is_none())
    {
        issues.push(
            "MCP analysis entries marked as having a valid schema must include an input schema."
                .to_string(),
        );
    }

    McpValidationResult { issues }
}
