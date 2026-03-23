use elegy_contracts::{
    validate_mcp_analysis_result, validate_mcp_server_descriptor, validate_skill_definition,
    McpAnalysisResult, McpServerDescriptor, McpToolDefinition, McpTransportKind, SkillDefinition,
};
use elegy_mcp::{generated_skill_id, McpSkillGenerator, McpToolAnalyzer};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
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

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct GeneratedSkillArtifacts {
    pub source_descriptor: String,
    pub analysis: McpAnalysisResult,
    pub generated_skills: Vec<SkillDefinition>,
    pub skipped_tools: Vec<McpToolDefinition>,
    pub written_files: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ToolingError {
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
    #[error("generated skill definition {skill_id} is invalid")]
    InvalidSkillDefinition {
        skill_id: String,
        issues: Vec<String>,
    },
    #[error("duplicate generated skill ID: {skill_id}")]
    DuplicateSkillId { skill_id: String },
    #[error("output file already exists: {path}")]
    OutputExists { path: PathBuf },
}

pub fn author_mcp_descriptor_to_path(
    request: AuthorMcpDescriptorRequest,
    output_path: &Path,
    overwrite: bool,
) -> Result<AuthoredMcpDescriptor, ToolingError> {
    let descriptor = build_mcp_descriptor(request)?;
    write_json_file(output_path, &descriptor, overwrite)?;

    Ok(AuthoredMcpDescriptor {
        output_path: display_path(output_path),
        descriptor,
    })
}

pub fn analyze_mcp_descriptor_file(path: &Path) -> Result<McpAnalysisResult, ToolingError> {
    let descriptor = load_mcp_descriptor_file(path)?;
    let analysis = analyze_descriptor(&descriptor);
    let validation = validate_mcp_analysis_result(&analysis);

    if !validation.is_valid() {
        return Err(ToolingError::InvalidMcpAnalysis {
            path: path.to_path_buf(),
            issues: validation.issues,
        });
    }

    Ok(analysis)
}

pub fn generate_skills_from_descriptor_file(
    descriptor_path: &Path,
    output_dir: Option<&Path>,
    overwrite: bool,
) -> Result<GeneratedSkillArtifacts, ToolingError> {
    let analysis = analyze_mcp_descriptor_file(descriptor_path)?;
    let generation = McpSkillGenerator.generate(&analysis);

    validate_generated_skills(&generation.generated_skills)?;

    let written_files = match output_dir {
        Some(output_dir) => write_skill_files(output_dir, &generation.generated_skills, overwrite)?
            .into_iter()
            .map(|path| display_path(&path))
            .collect(),
        None => Vec::new(),
    };

    Ok(GeneratedSkillArtifacts {
        source_descriptor: display_path(descriptor_path),
        analysis,
        generated_skills: generation.generated_skills,
        skipped_tools: generation.skipped_tools,
        written_files,
    })
}

fn build_mcp_descriptor(
    request: AuthorMcpDescriptorRequest,
) -> Result<McpServerDescriptor, ToolingError> {
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
        return Err(ToolingError::InvalidMcpDescriptor {
            path: PathBuf::from("<in-memory>"),
            issues,
        });
    }

    Ok(descriptor)
}

fn load_mcp_descriptor_file(path: &Path) -> Result<McpServerDescriptor, ToolingError> {
    let content = fs::read_to_string(path).map_err(|source| ToolingError::Io {
        operation: "read",
        path: path.to_path_buf(),
        source,
    })?;

    let descriptor = serde_json::from_str::<McpServerDescriptor>(&content).map_err(|source| {
        ToolingError::Json {
            path: path.to_path_buf(),
            source,
        }
    })?;

    let issues = descriptor_validation_issues(&descriptor);
    if !issues.is_empty() {
        return Err(ToolingError::InvalidMcpDescriptor {
            path: path.to_path_buf(),
            issues,
        });
    }

    Ok(descriptor)
}

fn descriptor_validation_issues(descriptor: &McpServerDescriptor) -> Vec<String> {
    let mut issues = validate_mcp_server_descriptor(descriptor).issues;
    issues.extend(generator_collision_issues(descriptor));
    issues
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

fn generator_collision_issues(descriptor: &McpServerDescriptor) -> Vec<String> {
    let mut distinct_ids = BTreeSet::new();
    let mut issues = Vec::new();

    for tool in &descriptor.tools {
        let Some(schema) = tool.input_schema.as_ref() else {
            continue;
        };

        if !is_supported_input_schema(schema) {
            continue;
        }

        let skill_id = generated_skill_id(&descriptor.server_name, &tool.name);
        let normalized_skill_id = skill_id.to_ascii_lowercase();
        if !distinct_ids.insert(normalized_skill_id) {
            issues.push(format!(
                "MCP descriptor tools must not collapse to the same generated skill ID; {skill_id} is duplicated."
            ));
        }
    }

    issues
}

fn validate_generated_skills(skills: &[SkillDefinition]) -> Result<(), ToolingError> {
    let mut distinct_ids = BTreeSet::new();

    for skill in skills {
        let skill_id = skill.effective_id().trim();
        let normalized_skill_id = skill_id.to_ascii_lowercase();
        if !distinct_ids.insert(normalized_skill_id) {
            return Err(ToolingError::DuplicateSkillId {
                skill_id: skill_id.to_string(),
            });
        }

        let validation = validate_skill_definition(skill);
        if !validation.is_valid() {
            return Err(ToolingError::InvalidSkillDefinition {
                skill_id: skill.effective_id().to_string(),
                issues: validation.issues,
            });
        }
    }

    Ok(())
}

fn write_skill_files(
    output_dir: &Path,
    skills: &[SkillDefinition],
    overwrite: bool,
) -> Result<Vec<PathBuf>, ToolingError> {
    fs::create_dir_all(output_dir).map_err(|source| ToolingError::Io {
        operation: "create directory",
        path: output_dir.to_path_buf(),
        source,
    })?;

    let target_paths = skills
        .iter()
        .map(|skill| output_dir.join(format!("{}.json", skill.effective_id())))
        .collect::<Vec<_>>();

    if !overwrite {
        for target_path in &target_paths {
            if target_path.exists() {
                return Err(ToolingError::OutputExists {
                    path: target_path.clone(),
                });
            }
        }
    }

    let mut written_files = Vec::with_capacity(skills.len());
    for (skill, file_path) in skills.iter().zip(target_paths.iter()) {
        if let Err(error) = write_json_file(file_path, skill, overwrite) {
            if !overwrite {
                cleanup_written_files(&written_files);
            }
            return Err(error);
        }

        written_files.push(file_path.clone());
    }

    Ok(written_files)
}

fn write_json_file<T: Serialize>(
    output_path: &Path,
    value: &T,
    overwrite: bool,
) -> Result<(), ToolingError> {
    if output_path.exists() && !overwrite {
        return Err(ToolingError::OutputExists {
            path: output_path.to_path_buf(),
        });
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| ToolingError::Io {
            operation: "create directory",
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let mut content = serde_json::to_string_pretty(value).map_err(|source| ToolingError::Json {
        path: output_path.to_path_buf(),
        source,
    })?;
    content.push('\n');

    fs::write(output_path, content).map_err(|source| ToolingError::Io {
        operation: "write",
        path: output_path.to_path_buf(),
        source,
    })
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

fn cleanup_written_files(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        analyze_mcp_descriptor_file, author_mcp_descriptor_to_path,
        generate_skills_from_descriptor_file, AuthorMcpDescriptorRequest, AuthorMcpToolRequest,
        ToolingError,
    };
    use elegy_contracts::{validate_mcp_server_descriptor, McpTransportKind};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&dir).expect("create temp directory");
        dir
    }

    #[test]
    fn author_mcp_descriptor_writes_valid_json() {
        let temp_dir = unique_temp_dir("elegy-tooling-author");
        let output_path = temp_dir.join("weather-mcp.json");

        let result = author_mcp_descriptor_to_path(
            AuthorMcpDescriptorRequest {
                server_name: "weather-server".to_string(),
                transport: McpTransportKind::Stdio,
                tools: vec![
                    AuthorMcpToolRequest {
                        name: "get-weather".to_string(),
                        description: Some("Look up a weather report".to_string()),
                    },
                    AuthorMcpToolRequest {
                        name: "list-alerts".to_string(),
                        description: None,
                    },
                ],
            },
            &output_path,
            false,
        )
        .expect("authoring should succeed");

        assert_eq!(result.descriptor.server_name, "weather-server");
        assert_eq!(result.descriptor.tools.len(), 2);
        assert!(output_path.is_file());

        let persisted = fs::read_to_string(&output_path).expect("read descriptor file");
        let parsed = serde_json::from_str(&persisted).expect("parse descriptor file");
        let validation = validate_mcp_server_descriptor(&parsed);
        assert!(
            validation.is_valid(),
            "unexpected issues: {:?}",
            validation.issues
        );
        assert!(
            parsed.tools.iter().all(|tool| tool.input_schema.is_none()),
            "authored MCP descriptors should not fabricate tool schemas"
        );
    }

    #[test]
    fn analyze_and_generate_skills_from_descriptor_file() {
        let temp_dir = unique_temp_dir("elegy-tooling-generate");
        let descriptor_path = temp_dir.join("weather-mcp.json");
        let output_dir = temp_dir.join("generated-skills");

        fs::write(
            &descriptor_path,
            r#"{
    "serverName": "weather-server",
    "transport": "stdio",
    "tools": [
        {
            "name": "get-weather",
            "description": "Look up a weather report",
            "inputSchema": { "type": "object" }
        },
        {
            "name": "list-alerts",
            "description": "List active weather alerts"
        }
    ]
}
"#,
        )
        .expect("write descriptor fixture");

        let analysis = analyze_mcp_descriptor_file(&descriptor_path)
            .expect("analysis should succeed for valid descriptor");
        assert_eq!(analysis.server_name, "weather-server");
        assert_eq!(analysis.analyses.len(), 2);

        let generated =
            generate_skills_from_descriptor_file(&descriptor_path, Some(&output_dir), false)
                .expect("skill generation should succeed");
        assert_eq!(generated.generated_skills.len(), 1);
        assert_eq!(
            generated.generated_skills[0].effective_id(),
            "mcp-weather-server-get-weather"
        );
        assert_eq!(generated.skipped_tools.len(), 1);
        assert_eq!(generated.written_files.len(), 1);
        assert!(output_dir
            .join("mcp-weather-server-get-weather.json")
            .is_file());
    }

    #[test]
    fn analyze_and_generate_skip_present_but_invalid_schemas() {
        let temp_dir = unique_temp_dir("elegy-tooling-invalid-schema");
        let descriptor_path = temp_dir.join("weather-mcp.json");

        fs::write(
            &descriptor_path,
            r#"{
  "serverName": "weather-server",
  "transport": "stdio",
  "tools": [
    {
      "name": "get-weather",
      "description": "Look up a weather report",
      "inputSchema": "not-a-schema-object"
    }
  ]
}
"#,
        )
        .expect("write descriptor fixture");

        let analysis = analyze_mcp_descriptor_file(&descriptor_path)
            .expect("analysis should still succeed for structurally invalid tool schema values");
        assert_eq!(analysis.analyses.len(), 1);
        assert!(
            !analysis.analyses[0].has_valid_schema,
            "non-object schemas should not be treated as valid for skill generation"
        );

        let generated = generate_skills_from_descriptor_file(&descriptor_path, None, false)
            .expect("generation should succeed while skipping invalid-schema tools");
        assert!(generated.generated_skills.is_empty());
        assert_eq!(generated.skipped_tools.len(), 1);
        assert_eq!(generated.skipped_tools[0].name, "get-weather");
    }

    #[test]
    fn authoring_refuses_to_overwrite_existing_file_without_force() {
        let temp_dir = unique_temp_dir("elegy-tooling-overwrite");
        let output_path = temp_dir.join("weather-mcp.json");
        fs::write(&output_path, "{}\n").expect("seed existing file");

        let error = author_mcp_descriptor_to_path(
            AuthorMcpDescriptorRequest {
                server_name: "weather-server".to_string(),
                transport: McpTransportKind::Stdio,
                tools: Vec::new(),
            },
            &output_path,
            false,
        )
        .expect_err("existing file should be rejected without force");

        assert!(matches!(error, ToolingError::OutputExists { .. }));
    }

    #[test]
    fn generation_preflights_existing_outputs_before_writing_any_files() {
        let temp_dir = unique_temp_dir("elegy-tooling-preflight");
        let descriptor_path = temp_dir.join("weather-mcp.json");
        let output_dir = temp_dir.join("generated-skills");
        fs::create_dir_all(&output_dir).expect("create output directory");
        fs::write(
            output_dir.join("mcp-weather-server-list-alerts.json"),
            "{}\n",
        )
        .expect("seed colliding output file");

        fs::write(
            &descriptor_path,
            r#"{
    "serverName": "weather-server",
    "transport": "stdio",
    "tools": [
        {
            "name": "get-weather",
            "description": "Look up a weather report",
            "inputSchema": { "type": "object" }
        },
        {
            "name": "list-alerts",
            "description": "List active weather alerts",
            "inputSchema": { "type": "object" }
        }
    ]
}
"#,
        )
        .expect("write descriptor fixture");

        let error =
            generate_skills_from_descriptor_file(&descriptor_path, Some(&output_dir), false)
                .expect_err("colliding output should fail before any write occurs");

        assert!(matches!(error, ToolingError::OutputExists { .. }));
        assert!(
            !output_dir
                .join("mcp-weather-server-get-weather.json")
                .exists(),
            "preflight should block all writes when a collision is detected"
        );
    }

    #[test]
    fn analyze_rejects_generator_id_collisions_for_valid_schema_tools() {
        let temp_dir = unique_temp_dir("elegy-tooling-collision");
        let descriptor_path = temp_dir.join("weather-mcp.json");

        fs::write(
            &descriptor_path,
            r#"{
          "serverName": "weather-server",
          "transport": "stdio",
          "tools": [
            {
              "name": "get-user",
              "description": "Get a user",
              "inputSchema": { "type": "object" }
            },
            {
              "name": "get_user",
              "description": "Get a user through another alias",
              "inputSchema": { "type": "object" }
            }
          ]
        }
        "#,
        )
        .expect("write descriptor fixture");

        let error = analyze_mcp_descriptor_file(&descriptor_path)
            .expect_err("colliding generated skill IDs should be rejected during analysis");

        match error {
            ToolingError::InvalidMcpDescriptor { issues, .. } => {
                assert!(issues
                    .iter()
                    .any(|issue| issue.contains("generated skill ID")));
            }
            other => panic!("unexpected error: {other}"),
        }
    }
}
