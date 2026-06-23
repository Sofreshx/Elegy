use crate::PlanningStoreError;

const TEMPLATE_DIR: &str = "contracts/templates/planning";

const TEMPLATES: &[(&str, &str)] = &[
    (
        "phase-gate",
        include_str!("../../../contracts/templates/planning/phase-gate.yaml"),
    ),
    (
        "implementation-slice",
        include_str!("../../../contracts/templates/planning/implementation-slice.yaml"),
    ),
    (
        "research-decision",
        include_str!("../../../contracts/templates/planning/research-decision.yaml"),
    ),
    (
        "migration",
        include_str!("../../../contracts/templates/planning/migration.yaml"),
    ),
    (
        "production-readiness",
        include_str!("../../../contracts/templates/planning/production-readiness.yaml"),
    ),
];

/// List available template names.
pub fn list_templates() -> Result<Vec<String>, PlanningStoreError> {
    Ok(TEMPLATES.iter().map(|(name, _)| name.to_string()).collect())
}

/// Render a template by name. Returns the YAML content.
pub fn render_template(name: &str) -> Result<String, PlanningStoreError> {
    TEMPLATES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, content)| content.to_string())
        .ok_or_else(|| {
            PlanningStoreError::InvalidInput(format!(
                "template `{name}` not found in {TEMPLATE_DIR}"
            ))
        })
}
