//! Opt-in contextual recall. Historical records are data, never instructions.
//! The source database is read-only; delivery and feedback live in a separate journal.
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{MemoryScope, ProvenanceLevel, SensitivityLevel, StoreError};

pub const RECALL_SCHEMA: &str = "memory-contextual-recall/v1";
pub const MAX_RECALL_INPUT_BYTES: u64 = 65_536;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RecallMode {
    #[default]
    Off,
    Observe,
    Inject,
}

/// Trusted local binding, never derived from a prompt or a memory record.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecallConfig {
    pub version: u32,
    pub project_root: PathBuf,
    pub db_path: PathBuf,
    pub state_path: PathBuf,
    pub domain: String,
    pub scopes: Vec<MemoryScope>,
    #[serde(default)]
    pub mode: RecallMode,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default = "low_sensitivity")]
    pub max_sensitivity: SensitivityLevel,
    #[serde(default = "context_budget")]
    pub max_context_tokens: usize,
    #[serde(default = "similarity_floor")]
    pub min_similarity: f32,
    #[serde(default)]
    pub learning_enabled: bool,
    /// Host adapter option; the library only accepts already bounded recent context.
    #[serde(default)]
    pub include_recent_context: bool,
    /// Optional trusted transcript directory for the host adapter.
    #[serde(default)]
    pub transcript_root: Option<PathBuf>,
}
fn low_sensitivity() -> SensitivityLevel {
    SensitivityLevel::Low
}
fn context_budget() -> usize {
    600
}
fn similarity_floor() -> f32 {
    0.2
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecallRequest {
    pub cwd: PathBuf,
    pub session_id: String,
    pub turn_id: String,
    pub prompt: String,
    #[serde(default)]
    pub recent_context: Vec<String>,
    /// Optional caller-computed embedding. This path never contacts a provider.
    #[serde(default)]
    pub embedding: Option<Vec<f32>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallItem {
    pub id: String,
    pub version: String,
    pub content: String,
    pub scope: MemoryScope,
    pub provenance: ProvenanceLevel,
    pub updated_at: String,
    pub similarity: f32,
    pub score: f32,
    pub reason: String,
    pub contradicted: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallResponse {
    pub schema_version: &'static str,
    pub mode: RecallMode,
    pub status: &'static str,
    pub event_id: Option<String>,
    pub recalls: Vec<RecallItem>,
    pub additional_context: String,
    /// Conservative UTF-8 byte upper bound, including the complete envelope.
    pub budget_tokens_upper_bound: usize,
    pub learning_samples: usize,
    pub elapsed_ms: u128,
}
impl RecallResponse {
    pub(crate) fn empty(mode: RecallMode, status: &'static str) -> Self {
        Self {
            schema_version: RECALL_SCHEMA,
            mode,
            status,
            event_id: None,
            recalls: Vec::new(),
            additional_context: String::new(),
            budget_tokens_upper_bound: 0,
            learning_samples: 0,
            elapsed_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecallJudgment {
    Useful,
    Irrelevant,
    Dismiss,
    NeedsCorrection,
}
impl RecallJudgment {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Useful => "useful",
            Self::Irrelevant => "irrelevant",
            Self::Dismiss => "dismiss",
            Self::NeedsCorrection => "needs_correction",
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecallFeedbackRequest {
    pub cwd: PathBuf,
    pub session_id: String,
    pub event_id: String,
    pub memory_id: String,
    pub judgment: RecallJudgment,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallFeedbackResponse {
    pub recorded: bool,
    pub correction_required: bool,
    pub learning_samples: usize,
    pub learning_active: bool,
}

pub fn contextual_recall(
    config: &RecallConfig,
    request: &RecallRequest,
) -> Result<RecallResponse, StoreError> {
    crate::storage::contextual_recall(config, request)
}
pub fn record_recall_feedback(
    config: &RecallConfig,
    request: &RecallFeedbackRequest,
) -> Result<RecallFeedbackResponse, StoreError> {
    crate::storage::record_recall_feedback(config, request)
}
