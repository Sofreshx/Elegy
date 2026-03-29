// Re-export public APIs from elegy-tooling for use by other crates
pub use elegy_tooling::{
    analyze_mcp_descriptor_file, author_mcp_descriptor_to_path,
    AuthorMcpDescriptorRequest, AuthorMcpToolRequest, AuthoredMcpDescriptor, GeneratedSkillArtifacts,
    ToolingError,
};

use elegy_mcp::McpSurfaceError;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkillsSurfaceError {
    #[error("MCP surface error")]
    Mcp(#[from] McpSurfaceError),
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

impl From<ToolingError> for SkillsSurfaceError {
    fn from(error: ToolingError) -> Self {
        match error {
            ToolingError::Io {
                operation,
                path,
                source,
            } => SkillsSurfaceError::Io {
                operation,
                path,
                source,
            },
            ToolingError::Json { path, source } => SkillsSurfaceError::Json { path, source },
            ToolingError::InvalidMcpDescriptor { path, issues } => {
                // Map descriptor validation errors to MCP surface error
                // For now, treat as InvalidSkillDefinition since it's related to skill generation
                SkillsSurfaceError::InvalidSkillDefinition {
                    skill_id: path.display().to_string(),
                    issues,
                }
            }
            ToolingError::InvalidMcpAnalysis { path, issues } => {
                SkillsSurfaceError::InvalidSkillDefinition {
                    skill_id: path.display().to_string(),
                    issues,
                }
            }
            ToolingError::InvalidSkillDefinition { skill_id, issues } => {
                SkillsSurfaceError::InvalidSkillDefinition { skill_id, issues }
            }
            ToolingError::DuplicateSkillId { skill_id } => {
                SkillsSurfaceError::DuplicateSkillId { skill_id }
            }
            ToolingError::OutputExists { path } => SkillsSurfaceError::OutputExists { path },
        }
    }
}

// Public wrapper function that returns SkillsSurfaceError
pub fn generate_skills_from_descriptor_file(
    descriptor_path: &Path,
    output_dir: Option<&Path>,
    overwrite: bool,
) -> Result<GeneratedSkillArtifacts, SkillsSurfaceError> {
    elegy_tooling::generate_skills_from_descriptor_file(descriptor_path, output_dir, overwrite)
        .map_err(SkillsSurfaceError::from)
}
