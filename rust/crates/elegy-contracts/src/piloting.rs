use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::ContractsError;

/// Piloting schema version constants for each contract artifact type.
pub const PILOTING_TARGET_DESCRIPTOR_SCHEMA_VERSION: &str = "elegy-piloting-target-descriptor/v1";
pub const PILOTING_SURFACE_DESCRIPTOR_SCHEMA_VERSION: &str = "elegy-piloting-surface-descriptor/v1";
pub const PILOTING_OBSERVATION_FRAME_SCHEMA_VERSION: &str = "elegy-piloting-observation-frame/v1";
pub const PILOTING_ACTION_INTENT_SCHEMA_VERSION: &str = "elegy-piloting-action-intent/v1";
pub const PILOTING_ACTION_RESULT_SCHEMA_VERSION: &str = "elegy-piloting-action-result/v1";
pub const PILOTING_READINESS_REPORT_SCHEMA_VERSION: &str = "elegy-piloting-readiness-report/v1";
pub const PILOTING_ADAPTER_MANIFEST_SCHEMA_VERSION: &str = "elegy-piloting-adapter-manifest/v1";
pub const PILOTING_FIXTURE_PACK_SCHEMA_VERSION: &str = "elegy-piloting-fixture-pack/v1";
pub const PILOTING_POLICY_DECISION_SCHEMA_VERSION: &str = "elegy-piloting-policy-decision/v1";
pub const PILOTING_SIMULATION_RESULT_SCHEMA_VERSION: &str = "elegy-piloting-simulation-result/v1";
pub const PILOTING_REPLAY_CHECKPOINT_SCHEMA_VERSION: &str = "elegy-piloting-replay-checkpoint/v1";
pub const PILOTING_LIFECYCLE_EVENT_SCHEMA_VERSION: &str = "elegy-piloting-lifecycle-event/v1";

/// Result of validating a piloting contract artifact.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PilotingValidationResult {
    pub issues: Vec<String>,
}

impl PilotingValidationResult {
    /// Returns true if no validation issues were found.
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

/// Hints for launching a piloting target process.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingLaunchHints {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub executables: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<String>,
}

/// Hints for attaching to an existing piloting target process.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingAttachHints {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub process_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub window_titles: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surface_urls: Vec<String>,
}

/// Describes a piloting target software product.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingTargetDescriptor {
    pub schema_version: String,
    pub target_id: String,
    pub product_name: String,
    pub version_range: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub launch_hints: PilotingLaunchHints,
    #[serde(default)]
    pub attach_hints: PilotingAttachHints,
}

/// Selector strategy for locating a piloting surface element.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingSelector {
    pub strategy: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Describes a piloting surface within a target.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingSurfaceDescriptor {
    pub schema_version: String,
    pub surface_id: String,
    pub target_id: String,
    pub surface_kind: String,
    pub stability: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selectors: Vec<PilotingSelector>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub semantic_anchors: Vec<String>,
}

/// Snapshot of observed surface state at a point in time.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingObservationFrame {
    pub schema_version: String,
    pub frame_id: String,
    pub target_id: String,
    pub surface_id: String,
    pub observed_at_utc: String,
    pub redaction_class: String,
    pub source: String,
    pub confidence: f64,
    pub state: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
}

/// Declares an intended piloting action on a surface.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingActionIntent {
    pub schema_version: String,
    pub action_id: String,
    pub target_id: String,
    pub surface_id: String,
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
    pub side_effect_class: String,
    pub required_confirmation: String,
}

/// Records the outcome of a piloting action execution.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingActionResult {
    pub schema_version: String,
    pub action_id: String,
    pub status: String,
    pub message: String,
    pub observed_delta: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_hint: Option<String>,
}

/// Status of a single dependency in a readiness report.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingDependencyStatus {
    pub id: String,
    pub kind: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Reports target readiness and dependency status.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingReadinessReport {
    pub schema_version: String,
    pub report_id: String,
    pub target_id: String,
    pub generated_at_utc: String,
    pub availability: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<PilotingDependencyStatus>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_reasons: Vec<String>,
    pub drift_status: String,
}

/// References to piloting contract schema files.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingContractRefs {
    pub target_descriptor_schema_ref: String,
    pub surface_descriptor_schema_ref: String,
    pub observation_frame_schema_ref: String,
    pub action_intent_schema_ref: String,
    pub action_result_schema_ref: String,
    pub readiness_report_schema_ref: String,
    pub fixture_pack_schema_ref: String,
    pub policy_decision_schema_ref: String,
    pub simulation_result_schema_ref: String,
    pub replay_checkpoint_schema_ref: String,
    pub lifecycle_event_schema_ref: String,
}

/// Reference to a fixture pack within an adapter manifest.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingFixtureRef {
    pub fixture_pack_id: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Permissions declared by a piloting adapter.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingPermissions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub side_effect_classes: Vec<String>,
    pub requires_host_approval: bool,
}

/// Manifest declaring a piloting adapter's capabilities and fixtures.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingAdapterManifest {
    pub schema_version: String,
    pub adapter_id: String,
    pub display_name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub mode: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_software: Vec<PilotingTargetDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_surfaces: Vec<PilotingSurfaceDescriptor>,
    pub contracts: PilotingContractRefs,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixtures: Vec<PilotingFixtureRef>,
    pub permissions: PilotingPermissions,
}

/// Expected result checks for a piloting action.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingExpectedResultCheck {
    pub action_id: String,
    pub expected_status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
}

/// Records a policy decision for a piloting action.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingPolicyDecision {
    pub schema_version: String,
    pub decision_id: String,
    pub action_id: String,
    pub target_id: String,
    pub decision: String,
    pub side_effect_class: String,
    pub evaluated_at_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_requirement: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
}

/// Records the predicted outcome of a simulated piloting action.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingSimulationResult {
    pub schema_version: String,
    pub simulation_id: String,
    pub action_id: String,
    pub target_id: String,
    pub status: String,
    pub simulated_at_utc: String,
    pub predicted_outcome: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_decision_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
}

/// Checkpoint capturing action state for replay support.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingReplayCheckpoint {
    pub schema_version: String,
    pub checkpoint_id: String,
    pub action_id: String,
    pub target_id: String,
    pub stage: String,
    pub captured_at_utc: String,
    pub state_ref: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Records a lifecycle event during piloting action execution.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingLifecycleEvent {
    pub schema_version: String,
    pub event_id: String,
    pub action_id: String,
    pub target_id: String,
    pub event_type: String,
    pub recorded_at_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

/// Bundle of fixture data for piloting contract testing.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PilotingFixturePack {
    pub schema_version: String,
    pub fixture_pack_id: String,
    pub adapter_id: String,
    pub target_id: String,
    pub recorded_at_utc: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observations: Vec<PilotingObservationFrame>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_actions: Vec<PilotingActionIntent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_result_checks: Vec<PilotingExpectedResultCheck>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_decisions: Vec<PilotingPolicyDecision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub simulation_results: Vec<PilotingSimulationResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replay_checkpoints: Vec<PilotingReplayCheckpoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifecycle_events: Vec<PilotingLifecycleEvent>,
}

/// Validates a piloting target descriptor against its contract rules.
pub fn validate_piloting_target_descriptor(
    descriptor: &PilotingTargetDescriptor,
) -> PilotingValidationResult {
    let mut issues = Vec::new();

    if descriptor.schema_version != PILOTING_TARGET_DESCRIPTOR_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{PILOTING_TARGET_DESCRIPTOR_SCHEMA_VERSION}'."
        ));
    }
    validate_contract_identifier("targetId", &descriptor.target_id, &mut issues);
    validate_non_empty("productName", &descriptor.product_name, &mut issues);
    validate_non_empty("versionRange", &descriptor.version_range, &mut issues);
    if descriptor.platforms.is_empty() {
        issues.push("platforms must contain at least one platform entry.".to_string());
    }
    validate_unique_non_empty_strings("platforms", &descriptor.platforms, &mut issues);

    let has_launch_hint =
        !descriptor.launch_hints.executables.is_empty() || !descriptor.launch_hints.urls.is_empty();
    let has_attach_hint = !descriptor.attach_hints.process_names.is_empty()
        || !descriptor.attach_hints.window_titles.is_empty()
        || !descriptor.attach_hints.surface_urls.is_empty();
    if !has_launch_hint && !has_attach_hint {
        issues.push(
            "launchHints or attachHints must describe at least one portable target hint."
                .to_string(),
        );
    }

    PilotingValidationResult { issues }
}

/// Validates a piloting surface descriptor against its contract rules.
pub fn validate_piloting_surface_descriptor(
    descriptor: &PilotingSurfaceDescriptor,
) -> PilotingValidationResult {
    let mut issues = Vec::new();

    if descriptor.schema_version != PILOTING_SURFACE_DESCRIPTOR_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{PILOTING_SURFACE_DESCRIPTOR_SCHEMA_VERSION}'."
        ));
    }
    validate_contract_identifier("surfaceId", &descriptor.surface_id, &mut issues);
    validate_contract_identifier("targetId", &descriptor.target_id, &mut issues);
    if !matches!(
        descriptor.surface_kind.as_str(),
        "ui" | "api" | "browser" | "desktop"
    ) {
        issues.push("surfaceKind must be one of 'ui', 'api', 'browser', or 'desktop'.".to_string());
    }
    if !matches!(
        descriptor.stability.as_str(),
        "experimental" | "volatile" | "stable" | "pinned"
    ) {
        issues.push(
            "stability must be one of 'experimental', 'volatile', 'stable', or 'pinned'."
                .to_string(),
        );
    }
    if descriptor.selectors.is_empty() && descriptor.semantic_anchors.is_empty() {
        issues.push("selectors or semanticAnchors must contain at least one entry.".to_string());
    }
    for selector in &descriptor.selectors {
        validate_non_empty("selectors[].strategy", &selector.strategy, &mut issues);
        validate_non_empty("selectors[].value", &selector.value, &mut issues);
    }
    validate_unique_non_empty_strings("semanticAnchors", &descriptor.semantic_anchors, &mut issues);

    PilotingValidationResult { issues }
}

/// Validates a piloting observation frame against its contract rules.
pub fn validate_piloting_observation_frame(
    frame: &PilotingObservationFrame,
) -> PilotingValidationResult {
    let mut issues = Vec::new();

    if frame.schema_version != PILOTING_OBSERVATION_FRAME_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{PILOTING_OBSERVATION_FRAME_SCHEMA_VERSION}'."
        ));
    }
    validate_contract_identifier("frameId", &frame.frame_id, &mut issues);
    validate_contract_identifier("targetId", &frame.target_id, &mut issues);
    validate_contract_identifier("surfaceId", &frame.surface_id, &mut issues);
    validate_non_empty("observedAtUtc", &frame.observed_at_utc, &mut issues);
    validate_rfc3339_datetime("observedAtUtc", &frame.observed_at_utc, &mut issues);
    if !matches!(
        frame.redaction_class.as_str(),
        "none" | "internal" | "sensitive" | "restricted"
    ) {
        issues.push(
            "redactionClass must be one of 'none', 'internal', 'sensitive', or 'restricted'."
                .to_string(),
        );
    }
    if !matches!(
        frame.source.as_str(),
        "fixture" | "ui" | "api" | "browser" | "desktop" | "manual"
    ) {
        issues.push(
            "source must be one of 'fixture', 'ui', 'api', 'browser', 'desktop', or 'manual'."
                .to_string(),
        );
    }
    if !(0.0..=1.0).contains(&frame.confidence) {
        issues.push("confidence must be between 0.0 and 1.0 inclusive.".to_string());
    }
    if !frame.state.is_object() {
        issues.push("state must be a JSON object.".to_string());
    }
    validate_unique_non_empty_strings("evidenceRefs", &frame.evidence_refs, &mut issues);

    PilotingValidationResult { issues }
}

/// Validates a piloting action intent against its contract rules.
pub fn validate_piloting_action_intent(intent: &PilotingActionIntent) -> PilotingValidationResult {
    let mut issues = Vec::new();

    if intent.schema_version != PILOTING_ACTION_INTENT_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{PILOTING_ACTION_INTENT_SCHEMA_VERSION}'."
        ));
    }
    validate_contract_identifier("actionId", &intent.action_id, &mut issues);
    validate_contract_identifier("targetId", &intent.target_id, &mut issues);
    validate_contract_identifier("surfaceId", &intent.surface_id, &mut issues);
    validate_non_empty("operation", &intent.operation, &mut issues);
    if !intent.input_schema.is_boolean() && !intent.input_schema.is_object() {
        issues.push("inputSchema must be a JSON object or boolean schema.".to_string());
    }
    if !is_known_side_effect_class(&intent.side_effect_class) {
        issues.push(format!(
            "sideEffectClass '{}' is not supported.",
            intent.side_effect_class
        ));
    }
    if !matches!(
        intent.required_confirmation.as_str(),
        "none" | "advisory" | "explicit"
    ) {
        issues.push(
            "requiredConfirmation must be one of 'none', 'advisory', or 'explicit'.".to_string(),
        );
    }

    PilotingValidationResult { issues }
}

/// Validates a piloting action result against its contract rules.
pub fn validate_piloting_action_result(result: &PilotingActionResult) -> PilotingValidationResult {
    let mut issues = Vec::new();

    if result.schema_version != PILOTING_ACTION_RESULT_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{PILOTING_ACTION_RESULT_SCHEMA_VERSION}'."
        ));
    }
    validate_contract_identifier("actionId", &result.action_id, &mut issues);
    if !matches!(
        result.status.as_str(),
        "succeeded" | "failed" | "refused" | "retryable"
    ) {
        issues.push(
            "status must be one of 'succeeded', 'failed', 'refused', or 'retryable'.".to_string(),
        );
    }
    validate_non_empty("message", &result.message, &mut issues);
    if !result.observed_delta.is_object() {
        issues.push("observedDelta must be a JSON object.".to_string());
    }
    validate_unique_non_empty_strings("evidenceRefs", &result.evidence_refs, &mut issues);
    if result.status == "retryable"
        && result
            .retry_hint
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        issues.push("retryable results must include retryHint.".to_string());
    }
    if result.status == "refused"
        && result
            .refusal_hint
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        issues.push("refused results must include refusalHint.".to_string());
    }

    PilotingValidationResult { issues }
}

/// Validates a piloting readiness report against its contract rules.
pub fn validate_piloting_readiness_report(
    report: &PilotingReadinessReport,
) -> PilotingValidationResult {
    let mut issues = Vec::new();

    if report.schema_version != PILOTING_READINESS_REPORT_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{PILOTING_READINESS_REPORT_SCHEMA_VERSION}'."
        ));
    }
    validate_contract_identifier("reportId", &report.report_id, &mut issues);
    validate_contract_identifier("targetId", &report.target_id, &mut issues);
    validate_non_empty("generatedAtUtc", &report.generated_at_utc, &mut issues);
    validate_rfc3339_datetime("generatedAtUtc", &report.generated_at_utc, &mut issues);
    if !matches!(
        report.availability.as_str(),
        "available" | "unavailable" | "degraded"
    ) {
        issues.push(
            "availability must be one of 'available', 'unavailable', or 'degraded'.".to_string(),
        );
    }
    if !matches!(
        report.drift_status.as_str(),
        "aligned" | "drifted" | "unknown"
    ) {
        issues.push("driftStatus must be one of 'aligned', 'drifted', or 'unknown'.".to_string());
    }
    if report.availability == "available" && !report.blocked_reasons.is_empty() {
        issues.push("blockedReasons must be empty when availability is 'available'.".to_string());
    }

    let mut seen_dependencies = BTreeSet::new();
    for dependency in &report.dependencies {
        validate_non_empty("dependencies[].id", &dependency.id, &mut issues);
        validate_non_empty("dependencies[].kind", &dependency.kind, &mut issues);
        if !matches!(
            dependency.status.as_str(),
            "ready" | "missing" | "unsupported" | "degraded"
        ) {
            issues.push(format!(
                "dependency '{}' uses unsupported status '{}'.",
                dependency.id, dependency.status
            ));
        }
        let normalized = dependency.id.trim().to_ascii_lowercase();
        if !normalized.is_empty() && !seen_dependencies.insert(normalized) {
            issues.push(format!(
                "dependencies must not contain duplicate id '{}'.",
                dependency.id
            ));
        }
    }
    validate_unique_non_empty_strings("blockedReasons", &report.blocked_reasons, &mut issues);

    PilotingValidationResult { issues }
}

/// Validates a piloting policy decision against its contract rules.
pub fn validate_piloting_policy_decision(
    decision: &PilotingPolicyDecision,
) -> PilotingValidationResult {
    let mut issues = Vec::new();

    if decision.schema_version != PILOTING_POLICY_DECISION_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{PILOTING_POLICY_DECISION_SCHEMA_VERSION}'."
        ));
    }
    validate_contract_identifier("decisionId", &decision.decision_id, &mut issues);
    validate_contract_identifier("actionId", &decision.action_id, &mut issues);
    validate_contract_identifier("targetId", &decision.target_id, &mut issues);
    if !matches!(
        decision.decision.as_str(),
        "allow" | "deny" | "simulate" | "escalate"
    ) {
        issues.push(
            "decision must be one of 'allow', 'deny', 'simulate', or 'escalate'.".to_string(),
        );
    }
    if !is_known_side_effect_class(&decision.side_effect_class) {
        issues.push(format!(
            "sideEffectClass '{}' is not supported.",
            decision.side_effect_class
        ));
    }
    validate_non_empty("evaluatedAtUtc", &decision.evaluated_at_utc, &mut issues);
    validate_rfc3339_datetime("evaluatedAtUtc", &decision.evaluated_at_utc, &mut issues);
    if let Some(policy_ref) = &decision.policy_ref {
        validate_non_empty("policyRef", policy_ref, &mut issues);
    }
    if let Some(approval_requirement) = &decision.approval_requirement {
        if !matches!(
            approval_requirement.as_str(),
            "none" | "advisory" | "explicit"
        ) {
            issues.push(
                "approvalRequirement must be one of 'none', 'advisory', or 'explicit'.".to_string(),
            );
        }
    }
    if decision.decision == "escalate"
        && decision
            .approval_requirement
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        issues.push("escalate decisions must include approvalRequirement.".to_string());
    }
    validate_unique_non_empty_strings("reasons", &decision.reasons, &mut issues);
    validate_unique_non_empty_strings("evidenceRefs", &decision.evidence_refs, &mut issues);

    PilotingValidationResult { issues }
}

/// Validates a piloting simulation result against its contract rules.
pub fn validate_piloting_simulation_result(
    result: &PilotingSimulationResult,
) -> PilotingValidationResult {
    let mut issues = Vec::new();

    if result.schema_version != PILOTING_SIMULATION_RESULT_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{PILOTING_SIMULATION_RESULT_SCHEMA_VERSION}'."
        ));
    }
    validate_contract_identifier("simulationId", &result.simulation_id, &mut issues);
    validate_contract_identifier("actionId", &result.action_id, &mut issues);
    validate_contract_identifier("targetId", &result.target_id, &mut issues);
    if !matches!(
        result.status.as_str(),
        "predicted" | "blocked" | "inconclusive"
    ) {
        issues.push("status must be one of 'predicted', 'blocked', or 'inconclusive'.".to_string());
    }
    validate_non_empty("simulatedAtUtc", &result.simulated_at_utc, &mut issues);
    validate_rfc3339_datetime("simulatedAtUtc", &result.simulated_at_utc, &mut issues);
    if !result.predicted_outcome.is_object() {
        issues.push("predictedOutcome must be a JSON object.".to_string());
    }
    validate_unique_non_empty_strings("checks", &result.checks, &mut issues);
    validate_unique_non_empty_strings("evidenceRefs", &result.evidence_refs, &mut issues);
    if let Some(policy_decision_ref) = &result.policy_decision_ref {
        validate_contract_identifier("policyDecisionRef", policy_decision_ref, &mut issues);
    }

    PilotingValidationResult { issues }
}

/// Validates a piloting replay checkpoint against its contract rules.
pub fn validate_piloting_replay_checkpoint(
    checkpoint: &PilotingReplayCheckpoint,
) -> PilotingValidationResult {
    let mut issues = Vec::new();

    if checkpoint.schema_version != PILOTING_REPLAY_CHECKPOINT_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{PILOTING_REPLAY_CHECKPOINT_SCHEMA_VERSION}'."
        ));
    }
    validate_contract_identifier("checkpointId", &checkpoint.checkpoint_id, &mut issues);
    validate_contract_identifier("actionId", &checkpoint.action_id, &mut issues);
    validate_contract_identifier("targetId", &checkpoint.target_id, &mut issues);
    if !matches!(
        checkpoint.stage.as_str(),
        "before" | "predicted_after" | "after" | "rollback"
    ) {
        issues.push(
            "stage must be one of 'before', 'predicted_after', 'after', or 'rollback'.".to_string(),
        );
    }
    validate_non_empty("capturedAtUtc", &checkpoint.captured_at_utc, &mut issues);
    validate_rfc3339_datetime("capturedAtUtc", &checkpoint.captured_at_utc, &mut issues);
    validate_non_empty("stateRef", &checkpoint.state_ref, &mut issues);
    validate_unique_non_empty_strings("evidenceRefs", &checkpoint.evidence_refs, &mut issues);
    validate_unique_non_empty_strings("notes", &checkpoint.notes, &mut issues);

    PilotingValidationResult { issues }
}

/// Validates a piloting lifecycle event against its contract rules.
pub fn validate_piloting_lifecycle_event(
    event: &PilotingLifecycleEvent,
) -> PilotingValidationResult {
    let mut issues = Vec::new();

    if event.schema_version != PILOTING_LIFECYCLE_EVENT_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{PILOTING_LIFECYCLE_EVENT_SCHEMA_VERSION}'."
        ));
    }
    validate_contract_identifier("eventId", &event.event_id, &mut issues);
    validate_contract_identifier("actionId", &event.action_id, &mut issues);
    validate_contract_identifier("targetId", &event.target_id, &mut issues);
    if !matches!(
        event.event_type.as_str(),
        "intent_recorded"
            | "policy_evaluated"
            | "simulation_recorded"
            | "checkpoint_recorded"
            | "result_recorded"
    ) {
        issues.push(
            "eventType must be one of 'intent_recorded', 'policy_evaluated', 'simulation_recorded', 'checkpoint_recorded', or 'result_recorded'."
                .to_string(),
        );
    }
    validate_non_empty("recordedAtUtc", &event.recorded_at_utc, &mut issues);
    validate_rfc3339_datetime("recordedAtUtc", &event.recorded_at_utc, &mut issues);
    if let Some(ref_id) = &event.ref_id {
        validate_contract_identifier("refId", ref_id, &mut issues);
    }
    if let Some(message) = &event.message {
        validate_non_empty("message", message, &mut issues);
    }
    if let Some(metadata) = &event.metadata {
        if !metadata.is_object() {
            issues.push("metadata must be a JSON object when provided.".to_string());
        }
    }

    PilotingValidationResult { issues }
}

/// Validates a piloting adapter manifest against its contract rules.
pub fn validate_piloting_adapter_manifest(
    manifest: &PilotingAdapterManifest,
) -> PilotingValidationResult {
    let mut issues = Vec::new();

    if manifest.schema_version != PILOTING_ADAPTER_MANIFEST_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{PILOTING_ADAPTER_MANIFEST_SCHEMA_VERSION}'."
        ));
    }
    validate_contract_identifier("adapterId", &manifest.adapter_id, &mut issues);
    validate_non_empty("displayName", &manifest.display_name, &mut issues);
    validate_non_empty("version", &manifest.version, &mut issues);
    if manifest.mode != "contracts_only" {
        issues.push("mode must be 'contracts_only' in this first piloting slice.".to_string());
    }
    if manifest.supported_software.is_empty() {
        issues.push("supportedSoftware must contain at least one target descriptor.".to_string());
    }
    if manifest.supported_surfaces.is_empty() {
        issues.push("supportedSurfaces must contain at least one surface descriptor.".to_string());
    }
    if manifest.fixtures.is_empty() {
        issues.push("fixtures must contain at least one fixture pack reference.".to_string());
    }

    let mut target_ids = BTreeSet::new();
    for target in &manifest.supported_software {
        for issue in validate_piloting_target_descriptor(target).issues {
            issues.push(format!(
                "supportedSoftware entry '{}': {issue}",
                target.target_id
            ));
        }
        let normalized = target.target_id.trim().to_ascii_lowercase();
        if !normalized.is_empty() && !target_ids.insert(normalized) {
            issues.push(format!(
                "supportedSoftware must not contain duplicate targetId '{}'.",
                target.target_id
            ));
        }
    }

    let supported_target_ids = manifest
        .supported_software
        .iter()
        .map(|target| target.target_id.as_str())
        .filter(|target_id| !target_id.trim().is_empty())
        .collect::<BTreeSet<_>>();
    for surface in &manifest.supported_surfaces {
        for issue in validate_piloting_surface_descriptor(surface).issues {
            issues.push(format!(
                "supportedSurfaces entry '{}': {issue}",
                surface.surface_id
            ));
        }
        if !surface.target_id.trim().is_empty()
            && !supported_target_ids.contains(surface.target_id.as_str())
        {
            issues.push(format!(
                "supportedSurfaces entry '{}' references unknown targetId '{}'.",
                surface.surface_id, surface.target_id
            ));
        }
    }

    for (field, value) in [
        (
            "contracts.targetDescriptorSchemaRef",
            manifest.contracts.target_descriptor_schema_ref.as_str(),
        ),
        (
            "contracts.surfaceDescriptorSchemaRef",
            manifest.contracts.surface_descriptor_schema_ref.as_str(),
        ),
        (
            "contracts.observationFrameSchemaRef",
            manifest.contracts.observation_frame_schema_ref.as_str(),
        ),
        (
            "contracts.actionIntentSchemaRef",
            manifest.contracts.action_intent_schema_ref.as_str(),
        ),
        (
            "contracts.actionResultSchemaRef",
            manifest.contracts.action_result_schema_ref.as_str(),
        ),
        (
            "contracts.readinessReportSchemaRef",
            manifest.contracts.readiness_report_schema_ref.as_str(),
        ),
        (
            "contracts.fixturePackSchemaRef",
            manifest.contracts.fixture_pack_schema_ref.as_str(),
        ),
        (
            "contracts.policyDecisionSchemaRef",
            manifest.contracts.policy_decision_schema_ref.as_str(),
        ),
        (
            "contracts.simulationResultSchemaRef",
            manifest.contracts.simulation_result_schema_ref.as_str(),
        ),
        (
            "contracts.replayCheckpointSchemaRef",
            manifest.contracts.replay_checkpoint_schema_ref.as_str(),
        ),
        (
            "contracts.lifecycleEventSchemaRef",
            manifest.contracts.lifecycle_event_schema_ref.as_str(),
        ),
    ] {
        validate_contract_reference_path(field, value, &mut issues);
    }

    let mut fixture_ids = BTreeSet::new();
    for fixture in &manifest.fixtures {
        validate_contract_identifier(
            "fixtures[].fixturePackId",
            &fixture.fixture_pack_id,
            &mut issues,
        );
        validate_relative_path("fixtures[].path", &fixture.path, &mut issues);
        let normalized = fixture.fixture_pack_id.trim().to_ascii_lowercase();
        if !normalized.is_empty() && !fixture_ids.insert(normalized) {
            issues.push(format!(
                "fixtures must not contain duplicate fixturePackId '{}'.",
                fixture.fixture_pack_id
            ));
        }
    }

    if manifest.permissions.side_effect_classes.is_empty() {
        issues.push(
            "permissions.sideEffectClasses must declare at least one allowed side-effect class."
                .to_string(),
        );
    }
    let mut seen_side_effects = BTreeSet::new();
    for side_effect_class in &manifest.permissions.side_effect_classes {
        if !is_known_side_effect_class(side_effect_class) {
            issues.push(format!(
                "permissions.sideEffectClasses entry '{}' is not supported.",
                side_effect_class
            ));
        }
        let normalized = side_effect_class.trim().to_ascii_lowercase();
        if !normalized.is_empty() && !seen_side_effects.insert(normalized) {
            issues.push(format!(
                "permissions.sideEffectClasses must not contain duplicate entry '{}'.",
                side_effect_class
            ));
        }
    }

    PilotingValidationResult { issues }
}

/// Validates a piloting fixture pack against its contract rules.
pub fn validate_piloting_fixture_pack(pack: &PilotingFixturePack) -> PilotingValidationResult {
    let mut issues = Vec::new();

    if pack.schema_version != PILOTING_FIXTURE_PACK_SCHEMA_VERSION {
        issues.push(format!(
            "schemaVersion must be '{PILOTING_FIXTURE_PACK_SCHEMA_VERSION}'."
        ));
    }
    validate_contract_identifier("fixturePackId", &pack.fixture_pack_id, &mut issues);
    validate_contract_identifier("adapterId", &pack.adapter_id, &mut issues);
    validate_contract_identifier("targetId", &pack.target_id, &mut issues);
    validate_non_empty("recordedAtUtc", &pack.recorded_at_utc, &mut issues);
    validate_rfc3339_datetime("recordedAtUtc", &pack.recorded_at_utc, &mut issues);
    if pack.observations.is_empty() {
        issues.push("observations must contain at least one observation frame.".to_string());
    }
    if pack.allowed_actions.is_empty() {
        issues.push("allowedActions must contain at least one action intent.".to_string());
    }
    if pack.expected_result_checks.is_empty() {
        issues.push(
            "expectedResultChecks must contain at least one expected result check.".to_string(),
        );
    }
    if pack.policy_decisions.is_empty() {
        issues.push("policyDecisions must contain at least one policy decision.".to_string());
    }
    if pack.simulation_results.is_empty() {
        issues.push("simulationResults must contain at least one simulation result.".to_string());
    }
    if pack.replay_checkpoints.is_empty() {
        issues.push("replayCheckpoints must contain at least one replay checkpoint.".to_string());
    }
    if pack.lifecycle_events.is_empty() {
        issues.push("lifecycleEvents must contain at least one lifecycle event.".to_string());
    }

    for frame in &pack.observations {
        for issue in validate_piloting_observation_frame(frame).issues {
            issues.push(format!("observations entry '{}': {issue}", frame.frame_id));
        }
        if frame.target_id != pack.target_id {
            issues.push(format!(
                "observations entry '{}' must reuse fixture targetId '{}'.",
                frame.frame_id, pack.target_id
            ));
        }
    }

    let mut action_ids = BTreeSet::new();
    for action in &pack.allowed_actions {
        for issue in validate_piloting_action_intent(action).issues {
            issues.push(format!(
                "allowedActions entry '{}': {issue}",
                action.action_id
            ));
        }
        if action.target_id != pack.target_id {
            issues.push(format!(
                "allowedActions entry '{}' must reuse fixture targetId '{}'.",
                action.action_id, pack.target_id
            ));
        }
        let normalized = action.action_id.trim().to_ascii_lowercase();
        if !normalized.is_empty() && !action_ids.insert(normalized) {
            issues.push(format!(
                "allowedActions must not contain duplicate actionId '{}'.",
                action.action_id
            ));
        }
    }

    for check in &pack.expected_result_checks {
        validate_contract_identifier(
            "expectedResultChecks[].actionId",
            &check.action_id,
            &mut issues,
        );
        if !matches!(
            check.expected_status.as_str(),
            "succeeded" | "failed" | "refused" | "retryable"
        ) {
            issues.push(format!(
                "expectedResultChecks entry '{}' uses unsupported expectedStatus '{}'.",
                check.action_id, check.expected_status
            ));
        }
        if check.checks.is_empty() {
            issues.push(format!(
                "expectedResultChecks entry '{}' must declare at least one textual check.",
                check.action_id
            ));
        }
        if !action_ids.contains(&check.action_id.trim().to_ascii_lowercase()) {
            issues.push(format!(
                "expectedResultChecks entry '{}' must reference an allowed action.",
                check.action_id
            ));
        }
        validate_unique_non_empty_strings(
            "expectedResultChecks[].checks",
            &check.checks,
            &mut issues,
        );
        validate_unique_non_empty_strings(
            "expectedResultChecks[].evidenceRefs",
            &check.evidence_refs,
            &mut issues,
        );
    }

    let action_ids = pack
        .allowed_actions
        .iter()
        .map(|action| action.action_id.trim().to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut policy_decision_ids = BTreeSet::new();
    for decision in &pack.policy_decisions {
        for issue in validate_piloting_policy_decision(decision).issues {
            issues.push(format!(
                "policyDecisions entry '{}': {issue}",
                decision.decision_id
            ));
        }
        if decision.target_id != pack.target_id {
            issues.push(format!(
                "policyDecisions entry '{}' must reuse fixture targetId '{}'.",
                decision.decision_id, pack.target_id
            ));
        }
        if !action_ids.contains(&decision.action_id.trim().to_ascii_lowercase()) {
            issues.push(format!(
                "policyDecisions entry '{}' must reference an allowed action.",
                decision.decision_id
            ));
        }
        let normalized = decision.decision_id.trim().to_ascii_lowercase();
        if !normalized.is_empty() && !policy_decision_ids.insert(normalized) {
            issues.push(format!(
                "policyDecisions must not contain duplicate decisionId '{}'.",
                decision.decision_id
            ));
        }
    }

    let mut simulation_ids = BTreeSet::new();
    for result in &pack.simulation_results {
        for issue in validate_piloting_simulation_result(result).issues {
            issues.push(format!(
                "simulationResults entry '{}': {issue}",
                result.simulation_id
            ));
        }
        if result.target_id != pack.target_id {
            issues.push(format!(
                "simulationResults entry '{}' must reuse fixture targetId '{}'.",
                result.simulation_id, pack.target_id
            ));
        }
        if !action_ids.contains(&result.action_id.trim().to_ascii_lowercase()) {
            issues.push(format!(
                "simulationResults entry '{}' must reference an allowed action.",
                result.simulation_id
            ));
        }
        if let Some(policy_decision_ref) = &result.policy_decision_ref {
            if !policy_decision_ids.contains(&policy_decision_ref.trim().to_ascii_lowercase()) {
                issues.push(format!(
                    "simulationResults entry '{}' references unknown policyDecisionRef '{}'.",
                    result.simulation_id, policy_decision_ref
                ));
            }
        }
        let normalized = result.simulation_id.trim().to_ascii_lowercase();
        if !normalized.is_empty() && !simulation_ids.insert(normalized) {
            issues.push(format!(
                "simulationResults must not contain duplicate simulationId '{}'.",
                result.simulation_id
            ));
        }
    }

    let state_refs = pack
        .observations
        .iter()
        .map(|frame| frame.frame_id.trim().to_ascii_lowercase())
        .chain(
            pack.simulation_results
                .iter()
                .map(|result| result.simulation_id.trim().to_ascii_lowercase()),
        )
        .collect::<BTreeSet<_>>();
    let mut checkpoint_ids = BTreeSet::new();
    for checkpoint in &pack.replay_checkpoints {
        for issue in validate_piloting_replay_checkpoint(checkpoint).issues {
            issues.push(format!(
                "replayCheckpoints entry '{}': {issue}",
                checkpoint.checkpoint_id
            ));
        }
        if checkpoint.target_id != pack.target_id {
            issues.push(format!(
                "replayCheckpoints entry '{}' must reuse fixture targetId '{}'.",
                checkpoint.checkpoint_id, pack.target_id
            ));
        }
        if !action_ids.contains(&checkpoint.action_id.trim().to_ascii_lowercase()) {
            issues.push(format!(
                "replayCheckpoints entry '{}' must reference an allowed action.",
                checkpoint.checkpoint_id
            ));
        }
        if !state_refs.contains(&checkpoint.state_ref.trim().to_ascii_lowercase()) {
            issues.push(format!(
                "replayCheckpoints entry '{}' references unknown stateRef '{}'.",
                checkpoint.checkpoint_id, checkpoint.state_ref
            ));
        }
        let normalized = checkpoint.checkpoint_id.trim().to_ascii_lowercase();
        if !normalized.is_empty() && !checkpoint_ids.insert(normalized) {
            issues.push(format!(
                "replayCheckpoints must not contain duplicate checkpointId '{}'.",
                checkpoint.checkpoint_id
            ));
        }
    }

    let ref_ids = action_ids
        .iter()
        .cloned()
        .chain(policy_decision_ids.iter().cloned())
        .chain(simulation_ids.iter().cloned())
        .chain(checkpoint_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    let mut lifecycle_event_ids = BTreeSet::new();
    for event in &pack.lifecycle_events {
        for issue in validate_piloting_lifecycle_event(event).issues {
            issues.push(format!(
                "lifecycleEvents entry '{}': {issue}",
                event.event_id
            ));
        }
        if event.target_id != pack.target_id {
            issues.push(format!(
                "lifecycleEvents entry '{}' must reuse fixture targetId '{}'.",
                event.event_id, pack.target_id
            ));
        }
        if !action_ids.contains(&event.action_id.trim().to_ascii_lowercase()) {
            issues.push(format!(
                "lifecycleEvents entry '{}' must reference an allowed action.",
                event.event_id
            ));
        }
        if let Some(ref_id) = &event.ref_id {
            if !ref_ids.contains(&ref_id.trim().to_ascii_lowercase()) {
                issues.push(format!(
                    "lifecycleEvents entry '{}' references unknown refId '{}'.",
                    event.event_id, ref_id
                ));
            }
        }
        let normalized = event.event_id.trim().to_ascii_lowercase();
        if !normalized.is_empty() && !lifecycle_event_ids.insert(normalized) {
            issues.push(format!(
                "lifecycleEvents must not contain duplicate eventId '{}'.",
                event.event_id
            ));
        }
    }

    PilotingValidationResult { issues }
}

/// Validates a fixture pack against its adapter manifest constraints.
pub fn validate_piloting_fixture_pack_against_manifest(
    pack: &PilotingFixturePack,
    manifest: &PilotingAdapterManifest,
) -> PilotingValidationResult {
    let mut issues = Vec::new();

    if pack.adapter_id != manifest.adapter_id {
        issues.push(format!(
            "fixture pack adapterId '{}' does not match adapter manifest '{}'.",
            pack.adapter_id, manifest.adapter_id
        ));
    }

    let supported_targets = manifest
        .supported_software
        .iter()
        .map(|target| target.target_id.as_str())
        .collect::<BTreeSet<_>>();
    if !supported_targets.contains(pack.target_id.as_str()) {
        issues.push(format!(
            "fixture pack targetId '{}' is not declared by the adapter manifest.",
            pack.target_id
        ));
    }

    let fixture_ids = manifest
        .fixtures
        .iter()
        .map(|fixture| fixture.fixture_pack_id.as_str())
        .collect::<BTreeSet<_>>();
    if !fixture_ids.contains(pack.fixture_pack_id.as_str()) {
        issues.push(format!(
            "fixture pack id '{}' is not declared by the adapter manifest.",
            pack.fixture_pack_id
        ));
    }

    let supported_surfaces = manifest
        .supported_surfaces
        .iter()
        .map(|surface| surface.surface_id.as_str())
        .collect::<BTreeSet<_>>();
    let declared_side_effects = manifest
        .permissions
        .side_effect_classes
        .iter()
        .map(|value| value.as_str())
        .collect::<BTreeSet<_>>();

    for action in &pack.allowed_actions {
        if !supported_surfaces.contains(action.surface_id.as_str()) {
            issues.push(format!(
                "allowed action '{}' references undeclared surfaceId '{}'.",
                action.action_id, action.surface_id
            ));
        }
        if !declared_side_effects.contains(action.side_effect_class.as_str()) {
            issues.push(format!(
                "allowed action '{}' uses undeclared sideEffectClass '{}'.",
                action.action_id, action.side_effect_class
            ));
        }
    }

    PilotingValidationResult { issues }
}

/// Loads a piloting target descriptor fixture from a directory.
pub fn load_piloting_target_descriptor_fixture_from_dir(
    dir: &Path,
) -> Result<PilotingTargetDescriptor, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("piloting-target-descriptor.minimal.json"),
    )
}

/// Loads a piloting surface descriptor fixture from a directory.
pub fn load_piloting_surface_descriptor_fixture_from_dir(
    dir: &Path,
) -> Result<PilotingSurfaceDescriptor, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("piloting-surface-descriptor.minimal.json"),
    )
}

/// Loads a piloting observation frame fixture from a directory.
pub fn load_piloting_observation_frame_fixture_from_dir(
    dir: &Path,
) -> Result<PilotingObservationFrame, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("piloting-observation-frame.minimal.json"),
    )
}

/// Loads a piloting action intent fixture from a directory.
pub fn load_piloting_action_intent_fixture_from_dir(
    dir: &Path,
) -> Result<PilotingActionIntent, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("piloting-action-intent.minimal.json"),
    )
}

/// Loads a piloting action result fixture from a directory.
pub fn load_piloting_action_result_fixture_from_dir(
    dir: &Path,
) -> Result<PilotingActionResult, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("piloting-action-result.minimal.json"),
    )
}

/// Loads a piloting readiness report fixture from a directory.
pub fn load_piloting_readiness_report_fixture_from_dir(
    dir: &Path,
) -> Result<PilotingReadinessReport, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("piloting-readiness-report.minimal.json"),
    )
}

/// Loads a piloting adapter manifest fixture from a directory.
pub fn load_piloting_adapter_manifest_fixture_from_dir(
    dir: &Path,
) -> Result<PilotingAdapterManifest, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("piloting-adapter-manifest.minimal.json"),
    )
}

/// Loads a piloting fixture pack fixture from a directory.
pub fn load_piloting_fixture_pack_fixture_from_dir(
    dir: &Path,
) -> Result<PilotingFixturePack, ContractsError> {
    load_json_file(
        &dir.join("fixtures")
            .join("piloting-fixture-pack.minimal.json"),
    )
}

fn load_json_file<T>(path: &Path) -> Result<T, ContractsError>
where
    T: for<'de> Deserialize<'de>,
{
    let content = fs::read_to_string(path).map_err(|source| ContractsError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    serde_json::from_str(&content).map_err(|source| ContractsError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn validate_non_empty(field: &str, value: &str, issues: &mut Vec<String>) {
    if value.trim().is_empty() {
        issues.push(format!("{field} must not be empty."));
    }
}

fn validate_contract_identifier(field: &str, value: &str, issues: &mut Vec<String>) {
    validate_non_empty(field, value, issues);
    if !value.trim().is_empty()
        && !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        issues.push(format!(
            "{field} must contain only ASCII letters, digits, '.', '_' or '-'."
        ));
    }
}

fn validate_unique_non_empty_strings(field: &str, values: &[String], issues: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for value in values {
        if value.trim().is_empty() {
            issues.push(format!("{field} entries must not be empty."));
            continue;
        }
        let normalized = value.trim().to_ascii_lowercase();
        if !seen.insert(normalized) {
            issues.push(format!(
                "{field} must not contain duplicate entry '{}'.",
                value
            ));
        }
    }
}

fn validate_relative_path(field: &str, value: &str, issues: &mut Vec<String>) {
    let value = value.trim();
    if value.is_empty()
        || value.starts_with('/')
        || value.starts_with('\\')
        || value.contains(':')
        || value
            .split(['/', '\\'])
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        issues.push(format!(
            "{field} must be a portable relative path without traversal."
        ));
    }
}

fn validate_contract_reference_path(field: &str, value: &str, issues: &mut Vec<String>) {
    let value = value.trim();
    if value.is_empty()
        || value.starts_with('/')
        || value.starts_with('\\')
        || value.contains(':')
        || value
            .split(['/', '\\'])
            .any(|segment| segment.is_empty() || segment == ".")
    {
        issues.push(format!(
            "{field} must be a portable relative path without empty segments."
        ));
    }
}

fn validate_rfc3339_datetime(field: &str, value: &str, issues: &mut Vec<String>) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }

    if OffsetDateTime::parse(value, &Rfc3339).is_err() {
        issues.push(format!("{field} must be a valid RFC3339 date-time."));
    }
}

fn is_known_side_effect_class(value: &str) -> bool {
    matches!(
        value,
        "none"
            | "read_only"
            | "disk_read"
            | "disk_write"
            | "network_outbound"
            | "process_spawn"
            | "desktop_ui"
    )
}
