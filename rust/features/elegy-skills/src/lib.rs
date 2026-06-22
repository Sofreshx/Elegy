use elegy_contracts::{
    parse_agent_skill_frontmatter, validate_agent_skill_frontmatter, AgentSkillFrontmatter,
    ContractsError,
};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkillsSurfaceError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse YAML frontmatter in {path}: {details}")]
    Yaml { path: PathBuf, details: String },
    #[error("skill registry contract error: {0}")]
    Contracts(#[from] ContractsError),
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistrySkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
    pub lifecycle_state: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistrySearchMatch {
    pub matched: bool,
    pub score: f64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub matched_capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub match_reasons: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistrySkillEntry {
    #[serde(flatten)]
    pub summary: RegistrySkillSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_result: Option<RegistrySearchMatch>,
}

/// A loaded skill from the skills/ directory with parsed frontmatter.
#[derive(Clone, Debug)]
struct LoadedSkill {
    summary: RegistrySkillSummary,
    frontmatter: AgentSkillFrontmatter,
    path: PathBuf,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryResolveResult {
    pub query: String,
    pub top_skill: Option<RegistrySkillEntry>,
    pub results: Vec<RegistrySkillEntry>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryValidationIssue {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryValidationReport {
    pub valid: bool,
    pub source: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    pub issues: Vec<RegistryValidationIssue>,
}

#[derive(Clone, Debug, Default)]
pub struct SkillRegistryQuery {
    pub lifecycle: Option<String>,
    pub include_detail: bool,
}

#[derive(Clone, Debug)]
pub struct SkillRegistry {
    skills: Vec<LoadedSkill>,
}

impl SkillRegistry {
    /// Build registry by scanning `skills/` directory at the repo root.
    pub fn builtin() -> Result<Self, SkillsSurfaceError> {
        // Look for skills/ relative to the manifest directory or CWD
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // Go up from rust/features/elegy-skills to repo root
        // rust/features/elegy-skills -> rust/features -> rust -> repo root
        let repo_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("../../.."));
        let skills_dir = repo_root.join("skills");

        Self::from_skills_dir(&skills_dir)
    }

    /// Build registry from a specific `skills/` directory.
    pub fn from_skills_dir(skills_dir: &Path) -> Result<Self, SkillsSurfaceError> {
        let mut skills = Vec::new();

        if !skills_dir.exists() || !skills_dir.is_dir() {
            return Ok(Self { skills });
        }

        let entries = fs::read_dir(skills_dir).map_err(|e| SkillsSurfaceError::Io {
            path: skills_dir.to_path_buf(),
            source: e,
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_md = path.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }

            let content = fs::read_to_string(&skill_md).map_err(|e| SkillsSurfaceError::Io {
                path: skill_md.clone(),
                source: e,
            })?;

            let (frontmatter, _body) =
                parse_agent_skill_frontmatter(&content).map_err(|e| SkillsSurfaceError::Yaml {
                    path: skill_md.clone(),
                    details: e,
                })?;

            let validation_issues = validate_agent_skill_frontmatter(&frontmatter);
            if !validation_issues.is_empty() {
                // Skip invalid skills
                continue;
            }

            let summary = RegistrySkillSummary {
                id: frontmatter.name.clone(),
                name: frontmatter.name.clone(),
                description: frontmatter.description.clone(),
                aliases: Vec::new(),
                lifecycle_state: "active".to_string(),
            };

            skills.push(LoadedSkill {
                summary,
                frontmatter,
                path,
            });
        }

        Ok(Self { skills })
    }

    pub fn from_sources(paths: &[PathBuf]) -> Result<Self, SkillsSurfaceError> {
        let mut registry = Self::builtin()?;
        for path in paths {
            if path.is_dir() {
                let sub = Self::from_skills_dir(path)?;
                registry.skills.extend(sub.skills);
            }
        }
        Ok(registry)
    }

    pub fn list(&self, query: &SkillRegistryQuery) -> Vec<RegistrySkillEntry> {
        self.skills
            .iter()
            .filter(|skill| matches_query(skill, query))
            .map(|skill| skill_entry(skill, None))
            .collect()
    }

    pub fn search(&self, query: &str, _include_detail: bool) -> Vec<RegistrySkillEntry> {
        let query_lower = query.to_ascii_lowercase();
        let mut results = self
            .skills
            .iter()
            .filter_map(|skill| {
                let match_result = score_skill_match(skill, &query_lower);
                if match_result.matched {
                    Some(skill_entry(skill, Some(match_result)))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        results.sort_by(compare_match_score);
        results
    }

    pub fn resolve(&self, query: &str, include_detail: bool) -> RegistryResolveResult {
        let results = self.search(query, include_detail);
        let top_skill = results.first().cloned();

        RegistryResolveResult {
            query: query.to_string(),
            top_skill,
            results,
        }
    }

    pub fn skill(&self, skill_id: &str) -> Option<RegistrySkillEntry> {
        self.skills
            .iter()
            .find(|skill| skill_matches_id(skill, skill_id))
            .map(|skill| skill_entry(skill, None))
    }
}

// ── Validators ─────────────────────────────────────────────────────────

pub fn validate_skill_file(path: &Path) -> Result<RegistryValidationReport, SkillsSurfaceError> {
    let content = fs::read_to_string(path).map_err(|e| SkillsSurfaceError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    match parse_agent_skill_frontmatter(&content) {
        Ok((frontmatter, _body)) => {
            let issues = validate_agent_skill_frontmatter(&frontmatter);
            Ok(RegistryValidationReport {
                valid: issues.is_empty(),
                source: path.display().to_string(),
                skills: vec![frontmatter.name],
                issues: issues
                    .into_iter()
                    .map(|msg| RegistryValidationIssue {
                        code: "REGISTRY-SKILL-001".to_string(),
                        message: msg,
                        path: Some(path.display().to_string()),
                        skill_id: None,
                    })
                    .collect(),
            })
        }
        Err(e) => Ok(RegistryValidationReport {
            valid: false,
            source: path.display().to_string(),
            skills: Vec::new(),
            issues: vec![RegistryValidationIssue {
                code: "REGISTRY-SKILL-001".to_string(),
                message: e,
                path: Some(path.display().to_string()),
                skill_id: None,
            }],
        }),
    }
}

pub fn validate_skill_directory(
    path: &Path,
) -> Result<RegistryValidationReport, SkillsSurfaceError> {
    let registry = SkillRegistry::from_skills_dir(path)?;
    let mut issues = Vec::new();
    let mut skills = Vec::new();

    for skill in &registry.skills {
        skills.push(skill.summary.id.clone());
        let frontmatter_issues = validate_agent_skill_frontmatter(&skill.frontmatter);
        for issue in frontmatter_issues {
            issues.push(RegistryValidationIssue {
                code: "REGISTRY-SKILL-001".to_string(),
                message: issue,
                path: Some(skill.path.display().to_string()),
                skill_id: Some(skill.summary.id.clone()),
            });
        }
    }

    Ok(RegistryValidationReport {
        valid: issues.is_empty(),
        source: path.display().to_string(),
        skills,
        issues,
    })
}

// ── Internal helpers ────────────────────────────────────────────────

fn matches_query(skill: &LoadedSkill, query: &SkillRegistryQuery) -> bool {
    query.lifecycle.as_ref().is_none_or(|lifecycle| {
        skill
            .summary
            .lifecycle_state
            .eq_ignore_ascii_case(lifecycle)
    })
}

fn skill_entry(
    skill: &LoadedSkill,
    match_result: Option<RegistrySearchMatch>,
) -> RegistrySkillEntry {
    RegistrySkillEntry {
        summary: skill.summary.clone(),
        match_result,
    }
}

fn skill_matches_id(skill: &LoadedSkill, skill_id: &str) -> bool {
    skill.summary.id == skill_id
        || skill
            .summary
            .aliases
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(skill_id))
}

fn score_skill_match(skill: &LoadedSkill, query_lower: &str) -> RegistrySearchMatch {
    let mut score = 0.0;
    let mut match_reasons = Vec::new();
    let mut field_hits = 0u32;
    let total_possible_fields = 4u32;

    let id_lower = skill.summary.id.to_ascii_lowercase();
    let name_lower = skill.summary.name.to_ascii_lowercase();
    let desc_lower = skill.summary.description.to_ascii_lowercase();

    if id_lower.contains(query_lower) {
        score += 0.9;
        match_reasons.push("skill-id".to_string());
        field_hits += 1;
    }
    if name_lower.contains(query_lower) {
        score += 0.9;
        match_reasons.push("skill-name".to_string());
        field_hits += 1;
    }
    if desc_lower.contains(query_lower) {
        score += 0.5;
        if !match_reasons.iter().any(|reason| reason == "description") {
            match_reasons.push("description".to_string());
        }
    }

    let query_tokens = query_lower.split_whitespace().collect::<Vec<_>>();
    if query_tokens.len() > 1 {
        let mut token_hits = 0u32;
        for token in &query_tokens {
            if id_lower.contains(token) || name_lower.contains(token) {
                token_hits += 1;
            } else if desc_lower.contains(token) {
                token_hits += 1;
            }
        }
        let token_ratio = token_hits as f64 / query_tokens.len() as f64;
        score += token_ratio * 0.3;
    }

    let normalized = if score > 0.0 {
        let field_coverage = field_hits as f64 / total_possible_fields as f64;
        let raw = (score / 3.0).min(1.0);
        (raw * 0.7 + field_coverage * 0.3).min(1.0)
    } else {
        0.0
    };

    RegistrySearchMatch {
        matched: score > 0.0,
        score: (normalized * 100.0).round() / 100.0,
        matched_capabilities: Vec::new(),
        match_reasons,
    }
}

fn compare_match_score(a: &RegistrySkillEntry, b: &RegistrySkillEntry) -> std::cmp::Ordering {
    let a_score = a
        .match_result
        .as_ref()
        .map(|result| result.score)
        .unwrap_or(0.0);
    let b_score = b
        .match_result
        .as_ref()
        .map(|result| result.score)
        .unwrap_or(0.0);
    b_score
        .partial_cmp(&a_score)
        .unwrap_or(std::cmp::Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_search_finds_repo_status() {
        let registry = SkillRegistry::builtin().expect("builtin registry should load");
        let results = registry.search("repo", false);
        assert!(!results.is_empty());
    }
}
