pub use elegy_contracts::{
    parse_builtin_skill_definitions, AgentCapabilityProfile, CapabilityApprovalRequirement,
    CapabilityAuthMode, CapabilityCostHint, CapabilityDefinition, CapabilityExecutionContract,
    CapabilityFamily, CapabilityGovernance, CapabilityIdempotenceHint, CapabilityLatencyHint,
    CapabilityLifecycleState, CapabilityObservability, CapabilitySchemaReference,
    CapabilitySideEffectClass, CapabilitySource, CapabilitySourceKind, CapabilityTrustLevel,
    ContractsError, SkillCapability, SkillCapabilityExecution, SkillCapabilityInput,
    SkillDefinitionV2, AGENT_CAPABILITY_PROFILE_SCHEMA_VERSION,
};
use elegy_contracts::{
    project_skill_capability_definition, validate_agent_capability_profile,
    validate_skill_definition_v2_strict,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
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
    #[error("failed to parse JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("skill registry contract error: {0}")]
    Contracts(#[from] ContractsError),
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegistrySkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub aliases: Vec<String>,
    pub capabilities_count: usize,
    pub lifecycle_state: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryContextCostEstimate {
    pub index_tokens: usize,
    pub detail_tokens: usize,
    pub full_tokens: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryCapabilityDetail {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<RegistryParameterDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<RegistryExecutionDetail>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub typical_next: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pipeable_to: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub output_consumed_by: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryParameterDetail {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub description: String,
    pub required: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryExecutionDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_side_effects: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_deterministic: Option<bool>,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub trigger_keywords: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub capability_hints: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_cost_estimate: Option<RegistryContextCostEstimate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<RegistryCapabilityDetail>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_result: Option<RegistrySearchMatch>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryCapabilityCard {
    pub skill_id: String,
    pub capability_id: String,
    pub capability_name: String,
    pub skill_name: String,
    pub description: String,
    pub has_side_effects: bool,
    pub is_deterministic: bool,
    pub approval_requirement: String,
    pub risk_level: Option<String>,
    pub invocation: Vec<String>,
    pub capability_definition: CapabilityDefinition,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryResolveResult {
    pub query: String,
    pub top_skill: Option<RegistrySkillEntry>,
    pub top_capability: Option<RegistryCapabilityCard>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_id: Option<String>,
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
    pub category: Option<String>,
    pub lifecycle: Option<String>,
    pub include_detail: bool,
}

#[derive(Clone, Debug)]
pub struct SkillRegistry {
    skills: Vec<LoadedSkill>,
}

#[derive(Clone, Debug)]
struct LoadedSkill {
    summary: RegistrySkillSummary,
    definition: SkillDefinitionV2,
    raw: Value,
}

impl SkillRegistry {
    pub fn builtin() -> Result<Self, SkillsSurfaceError> {
        let skills = parse_builtin_skill_definitions()?
            .into_iter()
            .map(LoadedSkill::from_definition)
            .collect::<Result<Vec<_>, _>>()?;
        validate_registry_invariants(&skills)?;
        Ok(Self { skills })
    }

    pub fn from_sources(paths: &[PathBuf]) -> Result<Self, SkillsSurfaceError> {
        let mut definitions = parse_builtin_skill_definitions()?;
        for path in paths {
            if path.is_dir() {
                definitions.extend(load_skill_definitions_from_dir(path)?);
            } else {
                definitions.push(load_skill_definition_from_file(path)?);
            }
        }
        let skills = definitions
            .into_iter()
            .map(LoadedSkill::from_definition)
            .collect::<Result<Vec<_>, _>>()?;
        validate_registry_invariants(&skills)?;
        Ok(Self { skills })
    }

    pub fn list(&self, query: &SkillRegistryQuery) -> Vec<RegistrySkillEntry> {
        self.skills
            .iter()
            .filter(|skill| matches_query(skill, query))
            .map(|skill| skill_entry(skill, query.include_detail, None))
            .collect()
    }

    pub fn search(&self, query: &str, include_detail: bool) -> Vec<RegistrySkillEntry> {
        let query_lower = query.to_ascii_lowercase();
        let mut results = self
            .skills
            .iter()
            .filter_map(|skill| {
                let match_result = score_skill_match(skill, &query_lower);
                if match_result.matched {
                    Some(skill_entry(skill, include_detail, Some(match_result)))
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
        let top_capability = top_skill
            .as_ref()
            .and_then(|skill| skill.match_result.as_ref())
            .and_then(|match_result| match_result.matched_capabilities.first())
            .and_then(|capability_id| self.capability(capability_id));

        RegistryResolveResult {
            query: query.to_string(),
            top_skill,
            top_capability,
            results,
        }
    }

    pub fn skill(&self, skill_id: &str) -> Option<RegistrySkillEntry> {
        self.skills
            .iter()
            .find(|skill| skill_matches_id(skill, skill_id))
            .map(|skill| skill_entry(skill, true, None))
    }

    pub fn skill_definition(&self, skill_id: &str) -> Option<SkillDefinitionV2> {
        self.skills
            .iter()
            .find(|skill| skill_matches_id(skill, skill_id))
            .map(|skill| skill.definition.clone())
    }

    pub fn capability(&self, capability_id: &str) -> Option<RegistryCapabilityCard> {
        self.skills.iter().find_map(|skill| {
            skill
                .definition
                .capabilities
                .iter()
                .find(|capability| capability.id == capability_id)
                .map(|capability| capability_card(skill, capability))
        })
    }

    pub fn capability_definition(&self, capability_id: &str) -> Option<CapabilityDefinition> {
        self.capability(capability_id)
            .map(|capability| capability.capability_definition)
    }

    pub fn profile_selection(
        &self,
        profile: Option<&AgentCapabilityProfile>,
    ) -> RegistryProfileSelection {
        build_profile_selection(self, profile)
    }

    pub fn filtered_by_profile(
        &self,
        selection: &RegistryProfileSelection,
    ) -> Vec<RegistrySkillEntry> {
        self.skills
            .iter()
            .filter_map(|skill| {
                if !selection.selected_skill_ids.contains(&skill.summary.id) {
                    return None;
                }
                let capabilities = skill
                    .definition
                    .capabilities
                    .iter()
                    .filter(|capability| selection.selected_capability_ids.contains(&capability.id))
                    .cloned()
                    .collect::<Vec<_>>();
                if capabilities.is_empty() {
                    return None;
                }

                let mut filtered_definition = skill.definition.clone();
                filtered_definition.capabilities = capabilities;
                let filtered = LoadedSkill::from_definition(filtered_definition).ok()?;
                Some(skill_entry(&filtered, true, None))
            })
            .collect()
    }

    pub fn search_filtered(
        &self,
        filtered_skills: &[RegistrySkillEntry],
        query: &str,
        include_detail: bool,
    ) -> Vec<RegistrySkillEntry> {
        let query_lower = query.to_ascii_lowercase();
        let mut results = filtered_skills
            .iter()
            .filter_map(|skill| {
                let match_result = score_filtered_skill_match(skill, &query_lower);
                if match_result.matched {
                    let mut skill = skill.clone();
                    if !include_detail {
                        skill.capabilities = None;
                    }
                    skill.match_result = Some(match_result);
                    Some(skill)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        results.sort_by(compare_match_score);
        results
    }

    pub fn resolve_filtered(
        &self,
        filtered_skills: &[RegistrySkillEntry],
        query: &str,
        include_detail: bool,
    ) -> RegistryResolveResult {
        let results = self.search_filtered(filtered_skills, query, include_detail);
        let top_skill = results.first().cloned();
        let top_capability = top_skill
            .as_ref()
            .and_then(|skill| skill.match_result.as_ref())
            .and_then(|match_result| match_result.matched_capabilities.first())
            .and_then(|capability_id| self.capability(capability_id));

        RegistryResolveResult {
            query: query.to_string(),
            top_skill,
            top_capability,
            results,
        }
    }

    pub fn build_mcp_tools(&self) -> Vec<RegistryMcpTool> {
        self.build_mcp_tool_bindings()
            .into_iter()
            .map(|binding| RegistryMcpTool {
                capability_id: binding.capability_id,
                description: binding.description,
                input_schema: binding.input_schema,
                read_only_hint: binding.read_only_hint,
                idempotent_hint: binding.idempotent_hint,
            })
            .collect()
    }

    pub fn build_mcp_tool_bindings(&self) -> Vec<RegistryMcpToolBinding> {
        self.skills
            .iter()
            .flat_map(|skill| {
                skill
                    .definition
                    .capabilities
                    .iter()
                    .filter_map(mcp_tool_binding)
            })
            .collect()
    }

    pub fn mcp_tool_binding(&self, capability_id: &str) -> Option<RegistryMcpToolBinding> {
        self.skills.iter().find_map(|skill| {
            skill
                .definition
                .capabilities
                .iter()
                .find(|capability| capability.id == capability_id)
                .and_then(mcp_tool_binding)
        })
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryProfileSelection {
    pub profile_provided: bool,
    pub profile_id: Option<String>,
    pub schema_version: Option<String>,
    pub always_include_router: bool,
    pub router_available: bool,
    pub selected_skill_ids: BTreeSet<String>,
    pub selected_capability_ids: BTreeSet<String>,
    pub total_skill_count: usize,
    pub total_capability_count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<RegistryValidationIssue>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryMcpTool {
    pub capability_id: String,
    pub description: String,
    pub input_schema: Value,
    pub read_only_hint: Option<bool>,
    pub idempotent_hint: Option<bool>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryMcpToolBinding {
    pub capability_id: String,
    pub description: String,
    pub executable_name: String,
    pub execution_type: String,
    pub argument_template: Vec<String>,
    pub input_schema: Value,
    pub stdin_format: Option<String>,
    pub timeout_seconds: Option<u64>,
    pub has_side_effects: bool,
    pub supports_dry_run: bool,
    pub read_only_hint: Option<bool>,
    pub idempotent_hint: Option<bool>,
}

impl RegistryProfileSelection {
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.code.starts_with("REGISTRY-PROFILE-E"))
    }
}

pub fn validate_skill_file(path: &Path) -> Result<RegistryValidationReport, SkillsSurfaceError> {
    match load_skill_definition_from_file(path) {
        Ok(definition) => Ok(validate_definition_report(
            &definition,
            path.display().to_string(),
        )),
        Err(SkillsSurfaceError::Json { source, .. }) => Ok(RegistryValidationReport {
            valid: false,
            source: path.display().to_string(),
            skills: Vec::new(),
            issues: vec![RegistryValidationIssue {
                code: "REGISTRY-SKILL-001".to_string(),
                message: format!("failed to parse JSON in {}: {source}", path.display()),
                path: Some(path.display().to_string()),
                skill_id: None,
                capability_id: None,
            }],
        }),
        Err(error) => Err(error),
    }
}

pub fn validate_skill_directory(
    path: &Path,
) -> Result<RegistryValidationReport, SkillsSurfaceError> {
    let mut issues = Vec::new();
    let mut skills = Vec::new();
    let mut definitions = Vec::new();

    for entry in walk_skill_definition_files(path)? {
        match load_skill_definition_from_file(&entry) {
            Ok(definition) => {
                skills.push(definition.identity.name.clone());
                issues.extend(
                    validate_definition_report(&definition, entry.display().to_string()).issues,
                );
                definitions.push(definition);
            }
            Err(SkillsSurfaceError::Json { source, .. }) => {
                issues.push(RegistryValidationIssue {
                    code: "REGISTRY-SKILL-001".to_string(),
                    message: format!("failed to parse JSON in {}: {source}", entry.display()),
                    path: Some(entry.display().to_string()),
                    skill_id: None,
                    capability_id: None,
                });
            }
            Err(error) => return Err(error),
        }
    }

    if issues.is_empty() {
        let loaded = skills_from_definitions(definitions)?;
        if let Err(error) = validate_registry_invariants(&loaded) {
            issues.push(RegistryValidationIssue {
                code: "REGISTRY-SKILL-001".to_string(),
                message: error.to_string(),
                path: Some(path.display().to_string()),
                skill_id: None,
                capability_id: None,
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

fn validate_definition_report(
    definition: &SkillDefinitionV2,
    source: String,
) -> RegistryValidationReport {
    let mut issues = Vec::new();

    if let Err(error) = validate_skill_definition_v2_strict(definition) {
        issues.push(RegistryValidationIssue {
            code: "REGISTRY-SKILL-001".to_string(),
            message: error.to_string(),
            path: Some(source.clone()),
            skill_id: Some(definition.identity.name.clone()),
            capability_id: None,
        });
    }

    let known_parameters = definition
        .capabilities
        .iter()
        .map(|capability| {
            (
                capability.id.clone(),
                capability
                    .input
                    .as_ref()
                    .map(|input| {
                        input
                            .parameters
                            .iter()
                            .map(|parameter| parameter.name.to_ascii_lowercase())
                            .collect::<BTreeSet<_>>()
                    })
                    .unwrap_or_default(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    for capability in &definition.capabilities {
        if let Some(implementation) = &capability.implementation {
            for argument in &implementation.arguments {
                if let Some(placeholder) = placeholder_name(argument) {
                    if !known_parameters
                        .get(&capability.id)
                        .is_some_and(|parameters| parameters.contains(&placeholder))
                    {
                        issues.push(RegistryValidationIssue {
                            code: "REGISTRY-SKILL-002".to_string(),
                            message: format!(
                                "capability '{}' references undeclared input parameter '{}' in implementation.arguments",
                                capability.id, placeholder
                            ),
                            path: Some(source.clone()),
                            skill_id: Some(definition.identity.name.clone()),
                            capability_id: Some(capability.id.clone()),
                        });
                    }
                }
            }
        }
    }

    RegistryValidationReport {
        valid: issues.is_empty(),
        source,
        skills: vec![definition.identity.name.clone()],
        issues,
    }
}

fn validate_registry_invariants(skills: &[LoadedSkill]) -> Result<(), ContractsError> {
    let mut skill_ids = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    let mut capability_ids = BTreeSet::new();

    for skill in skills {
        let normalized_skill_id = skill.summary.id.to_ascii_lowercase();
        if !skill_ids.insert(normalized_skill_id.clone()) {
            return Err(ContractsError::Compatibility(format!(
                "duplicate skill id '{}' in registry",
                skill.summary.id
            )));
        }
        for alias in &skill.summary.aliases {
            let normalized_alias = alias.to_ascii_lowercase();
            if normalized_alias == normalized_skill_id {
                continue;
            }
            if skill_ids.contains(&normalized_alias) {
                return Err(ContractsError::Compatibility(format!(
                    "duplicate skill alias '{}' in registry",
                    alias
                )));
            }
            if !aliases.insert(normalized_alias.clone()) {
                return Err(ContractsError::Compatibility(format!(
                    "duplicate skill alias '{}' in registry",
                    alias
                )));
            }
        }
        for capability in &skill.definition.capabilities {
            let normalized_capability_id = capability.id.to_ascii_lowercase();
            if !capability_ids.insert(normalized_capability_id) {
                return Err(ContractsError::Compatibility(format!(
                    "duplicate capability id '{}' in registry",
                    capability.id
                )));
            }
        }
    }

    Ok(())
}

fn skills_from_definitions(
    definitions: Vec<SkillDefinitionV2>,
) -> Result<Vec<LoadedSkill>, SkillsSurfaceError> {
    definitions
        .into_iter()
        .map(LoadedSkill::from_definition)
        .collect::<Result<Vec<_>, _>>()
}

fn load_skill_definitions_from_dir(
    path: &Path,
) -> Result<Vec<SkillDefinitionV2>, SkillsSurfaceError> {
    walk_skill_definition_files(path)?
        .into_iter()
        .map(|entry| load_skill_definition_from_file_strict(&entry))
        .collect()
}

fn walk_skill_definition_files(path: &Path) -> Result<Vec<PathBuf>, SkillsSurfaceError> {
    let mut files = Vec::new();
    for entry in fs::read_dir(path).map_err(|source| SkillsSurfaceError::Io {
        path: path.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| SkillsSurfaceError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let entry_path = entry.path();
        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            files.extend(walk_skill_definition_files(&entry_path)?);
            continue;
        }
        if entry_path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        files.push(entry_path);
    }
    Ok(files)
}

fn load_skill_definition_from_file(path: &Path) -> Result<SkillDefinitionV2, SkillsSurfaceError> {
    let contents = fs::read_to_string(path).map_err(|source| SkillsSurfaceError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let definition = serde_json::from_str::<SkillDefinitionV2>(&contents).map_err(|source| {
        SkillsSurfaceError::Json {
            path: path.to_path_buf(),
            source,
        }
    })?;
    Ok(definition)
}

fn load_skill_definition_from_file_strict(
    path: &Path,
) -> Result<SkillDefinitionV2, SkillsSurfaceError> {
    let definition = load_skill_definition_from_file(path)?;
    validate_skill_definition_v2_strict(&definition)?;
    Ok(definition)
}

impl LoadedSkill {
    fn from_definition(definition: SkillDefinitionV2) -> Result<Self, SkillsSurfaceError> {
        let raw = serde_json::to_value(&definition).map_err(|source| SkillsSurfaceError::Json {
            path: PathBuf::from("<in-memory>"),
            source,
        })?;
        let metadata = definition.metadata.as_ref();
        Ok(Self {
            summary: RegistrySkillSummary {
                id: definition.identity.name.clone(),
                name: metadata
                    .and_then(|metadata| metadata.display_name.clone())
                    .or_else(|| definition.identity.display_name.clone())
                    .unwrap_or_else(|| definition.identity.name.clone()),
                description: metadata
                    .and_then(|metadata| metadata.summary.clone())
                    .or_else(|| metadata.and_then(|metadata| metadata.description.clone()))
                    .unwrap_or_default(),
                category: metadata
                    .and_then(|metadata| metadata.category.clone())
                    .unwrap_or_default(),
                aliases: definition.identity.aliases.clone(),
                capabilities_count: definition.capabilities.len(),
                lifecycle_state: definition.lifecycle_state.clone(),
            },
            definition,
            raw,
        })
    }
}

fn matches_query(skill: &LoadedSkill, query: &SkillRegistryQuery) -> bool {
    let category_ok = query
        .category
        .as_ref()
        .is_none_or(|category| skill.summary.category.eq_ignore_ascii_case(category));
    let lifecycle_ok = query.lifecycle.as_ref().is_none_or(|lifecycle| {
        skill
            .summary
            .lifecycle_state
            .eq_ignore_ascii_case(lifecycle)
    });
    category_ok && lifecycle_ok
}

fn skill_entry(
    skill: &LoadedSkill,
    include_detail: bool,
    match_result: Option<RegistrySearchMatch>,
) -> RegistrySkillEntry {
    let capabilities = capability_details(skill);
    let context_cost_estimate = Some(estimate_context_cost(
        &skill.summary,
        &capabilities,
        &skill.raw,
    ));
    RegistrySkillEntry {
        summary: skill.summary.clone(),
        trigger_keywords: trigger_keywords(skill),
        capability_hints: skill
            .definition
            .discovery
            .as_ref()
            .map(|discovery| discovery.capability_hints.clone())
            .unwrap_or_default(),
        context_cost_estimate,
        capabilities: include_detail.then_some(capabilities),
        match_result,
    }
}

fn capability_details(skill: &LoadedSkill) -> Vec<RegistryCapabilityDetail> {
    skill
        .definition
        .capabilities
        .iter()
        .map(|capability| RegistryCapabilityDetail {
            id: capability.id.clone(),
            name: capability.name.clone(),
            description: capability.description.clone(),
            parameters: capability
                .input
                .as_ref()
                .map(|input| {
                    input
                        .parameters
                        .iter()
                        .map(|parameter| RegistryParameterDetail {
                            name: parameter.name.clone(),
                            param_type: parameter.param_type.clone(),
                            description: parameter.description.clone().unwrap_or_default(),
                            required: parameter.required,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            execution: capability
                .execution
                .as_ref()
                .map(|execution| RegistryExecutionDetail {
                    mode: execution.mode.clone(),
                    has_side_effects: Some(execution.has_side_effects),
                    is_deterministic: Some(execution.is_deterministic),
                }),
            typical_next: capability
                .composes_well
                .as_ref()
                .map(|composition| composition.typical_next.clone())
                .unwrap_or_default(),
            pipeable_to: capability
                .composes_well
                .as_ref()
                .map(|composition| composition.pipeable_to.clone())
                .unwrap_or_default(),
            output_consumed_by: capability
                .composes_well
                .as_ref()
                .map(|composition| composition.output_consumed_by.clone())
                .unwrap_or_default(),
        })
        .collect()
}

fn trigger_keywords(skill: &LoadedSkill) -> Vec<String> {
    skill
        .definition
        .discovery
        .as_ref()
        .map(|discovery| discovery.keywords.clone())
        .unwrap_or_default()
}

fn estimate_context_cost(
    summary: &RegistrySkillSummary,
    capabilities: &[RegistryCapabilityDetail],
    raw: &Value,
) -> RegistryContextCostEstimate {
    let index_bytes = serde_json::to_vec(summary)
        .map(|value| value.len())
        .unwrap_or(0);
    let detail_bytes = index_bytes
        + serde_json::to_vec(capabilities)
            .map(|value| value.len())
            .unwrap_or(0);
    let full_bytes = serde_json::to_vec(raw)
        .map(|value| value.len())
        .unwrap_or(0);

    RegistryContextCostEstimate {
        index_tokens: index_bytes.div_ceil(4),
        detail_tokens: detail_bytes.div_ceil(4),
        full_tokens: full_bytes.div_ceil(4),
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
    let mut matched_capabilities = Vec::new();
    let mut match_reasons = Vec::new();
    let mut field_hits = 0u32;
    let total_possible_fields = 5u32;

    let id_lower = skill.summary.id.to_ascii_lowercase();
    let name_lower = skill.summary.name.to_ascii_lowercase();
    let desc_lower = skill.summary.description.to_ascii_lowercase();
    let category_lower = skill.summary.category.to_ascii_lowercase();

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
    if category_lower.contains(query_lower) {
        score += 0.5;
        if !match_reasons.iter().any(|reason| reason == "category") {
            match_reasons.push("category".to_string());
        }
    }
    if desc_lower.contains(query_lower) {
        score += 0.5;
        if !match_reasons.iter().any(|reason| reason == "description") {
            match_reasons.push("description".to_string());
        }
    }

    let mut keyword_phrase_hit = false;
    if let Some(discovery) = &skill.definition.discovery {
        for keyword in &discovery.keywords {
            if keyword.to_ascii_lowercase().contains(query_lower) {
                score += 0.8;
                keyword_phrase_hit = true;
                if !match_reasons
                    .iter()
                    .any(|reason| reason == "discovery-keyword")
                {
                    match_reasons.push("discovery-keyword".to_string());
                    field_hits += 1;
                }
                break;
            }
        }

        for trigger in &discovery.triggers {
            if trigger.pattern.to_ascii_lowercase().contains(query_lower) {
                score += 0.7;
                if !match_reasons
                    .iter()
                    .any(|reason| reason == "discovery-trigger")
                {
                    match_reasons.push("discovery-trigger".to_string());
                    field_hits += 1;
                }
                break;
            }
        }
    }

    for capability in &skill.definition.capabilities {
        let capability_id = capability.id.to_ascii_lowercase();
        let capability_name = capability.name.to_ascii_lowercase();
        let capability_description = capability.description.to_ascii_lowercase();

        let mut matched = false;
        if capability_id.contains(query_lower) || capability_name.contains(query_lower) {
            score += 1.0;
            matched = true;
        } else if capability_description.contains(query_lower) {
            score += 0.5;
            matched = true;
        }

        if matched {
            matched_capabilities.push(capability.id.clone());
            if !match_reasons.iter().any(|reason| reason == "capability") {
                match_reasons.push("capability".to_string());
                field_hits += 1;
            }
        }
    }

    let query_tokens = query_lower.split_whitespace().collect::<Vec<_>>();
    if query_tokens.len() > 1 {
        let mut token_hits = 0u32;
        for token in &query_tokens {
            if id_lower.contains(token) || name_lower.contains(token) {
                token_hits += 1;
            } else if keyword_phrase_hit {
            } else if desc_lower.contains(token) || category_lower.contains(token) {
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
        matched_capabilities,
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

fn score_filtered_skill_match(
    skill: &RegistrySkillEntry,
    query_lower: &str,
) -> RegistrySearchMatch {
    let mut score = 0.0;
    let mut matched_capabilities = Vec::new();
    let mut match_reasons = Vec::new();
    let mut field_hits = 0u32;
    let total_possible_fields = 5u32;

    let id_lower = skill.summary.id.to_ascii_lowercase();
    let name_lower = skill.summary.name.to_ascii_lowercase();
    let desc_lower = skill.summary.description.to_ascii_lowercase();
    let category_lower = skill.summary.category.to_ascii_lowercase();

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
    if category_lower.contains(query_lower) {
        score += 0.5;
        if !match_reasons.iter().any(|reason| reason == "category") {
            match_reasons.push("category".to_string());
        }
    }
    if desc_lower.contains(query_lower) {
        score += 0.5;
        if !match_reasons.iter().any(|reason| reason == "description") {
            match_reasons.push("description".to_string());
        }
    }

    let mut keyword_phrase_hit = false;
    for keyword in &skill.trigger_keywords {
        if keyword.to_ascii_lowercase().contains(query_lower) {
            score += 0.8;
            keyword_phrase_hit = true;
            if !match_reasons
                .iter()
                .any(|reason| reason == "discovery-keyword")
            {
                match_reasons.push("discovery-keyword".to_string());
                field_hits += 1;
            }
            break;
        }
    }

    if let Some(capabilities) = &skill.capabilities {
        for capability in capabilities {
            let capability_id = capability.id.to_ascii_lowercase();
            let capability_name = capability.name.to_ascii_lowercase();
            let capability_description = capability.description.to_ascii_lowercase();

            let matched = capability_id.contains(query_lower)
                || capability_name.contains(query_lower)
                || capability_description.contains(query_lower);
            if matched {
                matched_capabilities.push(capability.id.clone());
                score += if capability_id.contains(query_lower)
                    || capability_name.contains(query_lower)
                {
                    1.0
                } else {
                    0.5
                };
                if !match_reasons.iter().any(|reason| reason == "capability") {
                    match_reasons.push("capability".to_string());
                    field_hits += 1;
                }
            }
        }
    }

    let query_tokens = query_lower.split_whitespace().collect::<Vec<_>>();
    if query_tokens.len() > 1 {
        let mut token_hits = 0u32;
        for token in &query_tokens {
            if id_lower.contains(token) || name_lower.contains(token) {
                token_hits += 1;
            } else if keyword_phrase_hit {
            } else if desc_lower.contains(token) || category_lower.contains(token) {
                token_hits += 1;
            }
        }
        score += (token_hits as f64 / query_tokens.len() as f64) * 0.3;
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
        matched_capabilities,
        match_reasons,
    }
}

fn capability_card(skill: &LoadedSkill, capability: &SkillCapability) -> RegistryCapabilityCard {
    RegistryCapabilityCard {
        skill_id: skill.summary.id.clone(),
        capability_id: capability.id.clone(),
        capability_name: capability.name.clone(),
        skill_name: skill.summary.name.clone(),
        description: capability.description.clone(),
        has_side_effects: capability_has_side_effects(capability),
        is_deterministic: capability_is_deterministic(capability),
        approval_requirement: skill
            .definition
            .governance
            .as_ref()
            .and_then(|governance| governance.approval_requirement.clone())
            .unwrap_or_else(|| "none".to_string()),
        risk_level: skill
            .definition
            .governance
            .as_ref()
            .and_then(|governance| governance.risk_level.clone()),
        invocation: capability
            .implementation
            .as_ref()
            .map(|implementation| implementation.arguments.clone())
            .unwrap_or_default(),
        capability_definition: project_skill_capability_definition(&skill.definition, capability),
    }
}

fn build_profile_selection(
    registry: &SkillRegistry,
    profile: Option<&AgentCapabilityProfile>,
) -> RegistryProfileSelection {
    let mut issues = Vec::new();

    if let Some(profile) = profile {
        for issue in validate_agent_capability_profile(profile).issues {
            issues.push(RegistryValidationIssue {
                code: "REGISTRY-PROFILE-E001".to_string(),
                message: issue,
                path: None,
                skill_id: None,
                capability_id: None,
            });
        }
    }

    let include_skills = profile
        .map(|profile| {
            profile
                .include_skills
                .iter()
                .map(|skill| skill.to_ascii_lowercase())
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let include_capabilities = profile
        .map(|profile| {
            profile
                .include_capabilities
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let exclude_capabilities = profile
        .map(|profile| {
            profile
                .exclude_capabilities
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();

    let known_skill_keys = registry
        .skills
        .iter()
        .flat_map(|skill| {
            std::iter::once(skill.summary.id.to_ascii_lowercase()).chain(
                skill
                    .summary
                    .aliases
                    .iter()
                    .map(|alias| alias.to_ascii_lowercase()),
            )
        })
        .collect::<BTreeSet<_>>();
    let known_capabilities = registry
        .skills
        .iter()
        .flat_map(|skill| {
            skill
                .definition
                .capabilities
                .iter()
                .map(|capability| capability.id.clone())
        })
        .collect::<BTreeSet<_>>();

    if let Some(profile) = profile {
        for skill in &profile.include_skills {
            if !known_skill_keys.contains(&skill.to_ascii_lowercase()) {
                issues.push(RegistryValidationIssue {
                    code: "REGISTRY-PROFILE-E002".to_string(),
                    message: format!("agent profile references unknown skill '{skill}'"),
                    path: None,
                    skill_id: Some(skill.clone()),
                    capability_id: None,
                });
            }
        }
        for capability in profile
            .include_capabilities
            .iter()
            .chain(profile.exclude_capabilities.iter())
        {
            if !known_capabilities.contains(capability) {
                issues.push(RegistryValidationIssue {
                    code: "REGISTRY-PROFILE-E003".to_string(),
                    message: format!("agent profile references unknown capability '{capability}'"),
                    path: None,
                    skill_id: None,
                    capability_id: Some(capability.clone()),
                });
            }
        }
    }

    let mut selected_skill_ids = BTreeSet::new();
    let mut selected_capability_ids = BTreeSet::new();
    let profile_provided = profile.is_some();

    for skill in &registry.skills {
        let is_active = skill.summary.lifecycle_state.eq_ignore_ascii_case("active");
        let skill_requested = !profile_provided
            || (is_active
                && std::iter::once(skill.summary.id.as_str())
                    .chain(skill.summary.aliases.iter().map(String::as_str))
                    .any(|key| include_skills.contains(&key.to_ascii_lowercase())));
        let router_requested = profile.is_some_and(|profile| profile.always_include_router)
            && skill.summary.id == "skill-router";

        let mut selected_for_skill = false;
        for capability in &skill.definition.capabilities {
            let capability_requested = skill_requested
                || router_requested
                || include_capabilities.contains(&capability.id);
            if capability_requested && !exclude_capabilities.contains(&capability.id) {
                selected_for_skill = true;
                selected_capability_ids.insert(capability.id.clone());
            }
        }

        if selected_for_skill {
            selected_skill_ids.insert(skill.summary.id.clone());
        }
    }

    if profile_provided && selected_capability_ids.is_empty() {
        issues.push(RegistryValidationIssue {
            code: "REGISTRY-PROFILE-E004".to_string(),
            message: "agent profile selects no capabilities".to_string(),
            path: None,
            skill_id: None,
            capability_id: None,
        });
    }

    let router_available = selected_skill_ids.contains("skill-router")
        && selected_capability_ids
            .iter()
            .any(|capability| capability.starts_with("router-"));
    if profile_provided && !router_available {
        issues.push(RegistryValidationIssue {
            code: "REGISTRY-PROFILE-W001".to_string(),
            message: "agent profile does not expose the skill registry router; progressive discovery may be limited".to_string(),
            path: None,
            skill_id: Some("skill-router".to_string()),
            capability_id: None,
        });
    }

    RegistryProfileSelection {
        profile_provided,
        profile_id: profile.map(|profile| profile.profile_id.clone()),
        schema_version: profile.map(|profile| profile.schema_version.clone()),
        always_include_router: profile
            .map(|profile| profile.always_include_router)
            .unwrap_or(true),
        router_available,
        selected_skill_ids,
        selected_capability_ids,
        total_skill_count: registry.skills.len(),
        total_capability_count: registry
            .skills
            .iter()
            .map(|skill| skill.definition.capabilities.len())
            .sum(),
        issues,
    }
}

fn capability_input_schema(capability: &SkillCapability) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    if let Some(input) = &capability.input {
        for parameter in &input.parameters {
            let schema_type = match parameter.param_type.as_str() {
                "boolean" => "boolean",
                "integer" | "number" => parameter.param_type.as_str(),
                "array" => "array",
                _ => "string",
            };

            let mut property = serde_json::Map::new();
            property.insert("type".to_string(), Value::String(schema_type.to_string()));
            if schema_type == "array" {
                property.insert("items".to_string(), serde_json::json!({ "type": "string" }));
            }
            if let Some(description) = &parameter.description {
                property.insert(
                    "description".to_string(),
                    Value::String(description.clone()),
                );
            }
            if let Some(default) = &parameter.default {
                property.insert("default".to_string(), default.clone());
            }
            if parameter.required {
                required.push(Value::String(parameter.name.clone()));
            }
            properties.insert(parameter.name.clone(), Value::Object(property));
        }

        if let Some(stdin_format) = &input.stdin_format {
            properties.insert(
                "stdin".to_string(),
                serde_json::json!({
                    "type": "string",
                    "description": format!("Data to pipe to stdin (format: {stdin_format}).")
                }),
            );
        }
    }

    let mut schema = serde_json::Map::new();
    schema.insert("type".to_string(), Value::String("object".to_string()));
    schema.insert("properties".to_string(), Value::Object(properties));
    if !required.is_empty() {
        schema.insert("required".to_string(), Value::Array(required));
    }
    Value::Object(schema)
}

fn capability_supports_dry_run(capability: &SkillCapability) -> bool {
    let declares_parameter = capability.input.as_ref().is_some_and(|input| {
        input
            .parameters
            .iter()
            .any(|parameter| parameter.name == "dry_run" || parameter.name == "dryRun")
    });

    let template_uses_parameter =
        capability
            .implementation
            .as_ref()
            .is_some_and(|implementation| {
                implementation.arguments.iter().any(|argument| {
                    argument.contains("${dry_run}") || argument.contains("${dryRun}")
                })
            });

    declares_parameter && template_uses_parameter
}

fn capability_has_side_effects(capability: &SkillCapability) -> bool {
    capability
        .execution
        .as_ref()
        .map(|execution| execution.has_side_effects)
        .unwrap_or(false)
}

fn capability_is_deterministic(capability: &SkillCapability) -> bool {
    capability
        .execution
        .as_ref()
        .map(|execution| execution.is_deterministic)
        .unwrap_or(false)
}

fn mcp_tool_binding(capability: &SkillCapability) -> Option<RegistryMcpToolBinding> {
    let implementation = capability.implementation.as_ref()?;

    Some(RegistryMcpToolBinding {
        capability_id: capability.id.clone(),
        description: capability.description.clone(),
        executable_name: implementation.executable_name.clone(),
        execution_type: implementation.execution_type.clone(),
        argument_template: implementation.arguments.clone(),
        input_schema: capability_input_schema(capability),
        stdin_format: capability
            .input
            .as_ref()
            .and_then(|input| input.stdin_format.clone()),
        timeout_seconds: capability
            .execution
            .as_ref()
            .and_then(|execution| execution.timeout_seconds)
            .map(u64::from),
        has_side_effects: capability_has_side_effects(capability),
        supports_dry_run: capability_supports_dry_run(capability),
        read_only_hint: Some(!capability_has_side_effects(capability)),
        idempotent_hint: Some(capability_is_deterministic(capability)),
    })
}

fn placeholder_name(argument: &str) -> Option<String> {
    if argument.starts_with("${") && argument.ends_with('}') {
        Some(argument[2..argument.len() - 1].to_ascii_lowercase())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elegy_contracts::validate_capability_definition;

    #[test]
    fn builtin_registry_search_finds_repo_status() {
        let registry = SkillRegistry::builtin().expect("builtin registry should load");
        let results = registry.search("repo status", false);
        assert!(!results.is_empty());
        assert_eq!(results[0].summary.id, "repo");
        assert!(
            results[0]
                .match_result
                .as_ref()
                .expect("match result")
                .score
                > 0.0
        );
    }

    #[test]
    fn builtin_registry_capability_projection_works() {
        let registry = SkillRegistry::builtin().expect("builtin registry should load");
        let capability = registry
            .capability("diagram-create")
            .expect("diagram-create capability should exist");
        assert_eq!(capability.capability_definition.id, "diagram-create");
        assert_eq!(
            capability.capability_definition.family,
            CapabilityFamily::Skill
        );
    }

    #[test]
    fn all_builtin_capability_projections_validate_as_capability_definitions() {
        let registry = SkillRegistry::builtin().expect("builtin registry should load");

        for capability in registry.build_mcp_tool_bindings() {
            let definition = registry
                .capability_definition(&capability.capability_id)
                .expect("capability definition should exist");
            let validation = validate_capability_definition(&definition);
            assert!(
                validation.is_valid(),
                "unexpected capability-definition issues for {}: {:?}",
                capability.capability_id,
                validation.issues
            );
        }
    }

    #[test]
    fn builtin_registry_profile_selection_keeps_router_when_requested() {
        let registry = SkillRegistry::builtin().expect("builtin registry should load");
        let profile = AgentCapabilityProfile {
            schema_version: AGENT_CAPABILITY_PROFILE_SCHEMA_VERSION.to_string(),
            profile_id: "repo-host".to_string(),
            include_skills: vec!["repo".to_string()],
            include_capabilities: Vec::new(),
            exclude_capabilities: Vec::new(),
            always_include_router: true,
        };
        let selection = registry.profile_selection(Some(&profile));
        assert!(selection.selected_skill_ids.contains("repo"));
        assert!(selection.selected_skill_ids.contains("skill-router"));
        assert!(selection
            .selected_capability_ids
            .contains("router-skill-search"));
    }
}
