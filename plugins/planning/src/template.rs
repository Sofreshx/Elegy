use crate::PlanningStoreError;

const TEMPLATE_DIR: &str = "../templates";

const TEMPLATES: &[(&str, &str)] = &[
    ("phase-gate", include_str!("../templates/phase-gate.yaml")),
    (
        "implementation-slice",
        include_str!("../templates/implementation-slice.yaml"),
    ),
    (
        "research-decision",
        include_str!("../templates/research-decision.yaml"),
    ),
    ("migration", include_str!("../templates/migration.yaml")),
    (
        "production-readiness",
        include_str!("../templates/production-readiness.yaml"),
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
