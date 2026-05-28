use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    path::Path,
    sync::{Arc, Mutex},
    thread,
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, types::Type, Connection, OptionalExtension, Row};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::schema::init_database;
use crate::{
    decay,
    embedding::{prepare_embedding_input, EmbeddingTask},
    gate::DefaultSalienceGate,
    similarity::cosine_similarity,
    traits::{EmbeddingProvider, MemoryObservability, MemoryStore, SalienceGate},
    types::{
        ContradictionEntry, CorrectionDisposition, CorrectionRecord, ElegyArchive, ExportFormat,
        GraphNode, GraphTraversalResult, Memory, MemoryCandidate, MemoryContextConfig,
        MemoryHealthReport, MemoryId, MemoryLink, MemoryScope, MemoryState, MemoryType,
        MemoryVersion, PoisoningAlert, PoisoningAlertType, ProvenanceLevel, PurgeReport,
        ResolutionStatus, RetrievalFeedback, ScopeConfig, ScoredMemory, SearchQuery,
        SensitivityLevel, ShareConfig,
    },
    EmbeddingError, GateDecision, GateError, MemoryFilter, MetadataUpdate, ObservabilityError,
    OptionalFieldUpdate, StoreError,
};

const MEMORY_SELECT_COLUMNS: &str = r#"
    id,
    content,
    summary,
    scope,
    memory_type,
    provenance,
    importance_score,
    reliability_score,
    sensitivity,
    state,
    tags,
    status,
    custom_metadata,
    access_count,
    corroboration_count,
    embedding_stale,
    created_at,
    updated_at,
    last_accessed_at,
    tenant_id,
    user_id,
    agent_id
"#;

const DEFAULT_EMBEDDING_DIMENSIONS: usize = 768;
const DEFAULT_WORKSPACE_BUDGET: f32 = 500.0;
const DEFAULT_USER_BUDGET: f32 = 1_000.0;
const DEFAULT_AGENT_BUDGET: f32 = 200.0;
const VECTOR_SIMILARITY_BLEND_WEIGHT: f32 = 0.7;
const KEYWORD_SIMILARITY_BLEND_WEIGHT: f32 = 0.3;
const ACCESS_SIGNAL_HALF_SATURATION: f32 = 8.0;
// WU15: secondaries may only refine ranking inside a narrow similarity band. A gap above 0.03
// must keep the semantic winner ahead; this protects the observed fr_q07 hot canary (~0.03147).
// The threshold is canary-fitted and should be revisited against the observed distribution of
// similarity gaps; only gaps strictly above T get full structural protection, while quasi-equalities
// inside the band still resolve through the secondary blend.
const RETRIEVAL_SIMILARITY_TIE_THRESHOLD: f32 = 0.03;
const RETRIEVAL_BAND_REFINEMENT_RATIO: f32 = 0.49;
const RETRIEVAL_SCORING_MODE_ENV: &str = "ELEGY_RETRIEVAL_SCORING_MODE";
const EXPLAIN_RETRIEVAL_SCORING_ENV: &str = "ELEGY_EXPLAIN_RETRIEVAL_SCORING";
const ESTIMATED_CHARS_PER_TOKEN: usize = 4;
const BASE_MEMORY_TOKEN_OVERHEAD: u32 = 16;
const QUARANTINED_STATUS: &str = "quarantined";
const SHARED_REVIEW_STATUS: &str = "shared_review";
const SHARED_IMPORT_SOURCE_METADATA_KEY: &str = "shared_import_source";
const SHARED_IMPORT_DISPOSITION_METADATA_KEY: &str = "shared_import_disposition";
const SHARED_IMPORT_REASON_METADATA_KEY: &str = "shared_import_reason";
const SHARED_IMPORT_REFERENCED_MEMORY_METADATA_KEY: &str = "shared_import_referenced_memory_id";
const SHARED_IMPORT_ORIGINAL_ID_METADATA_KEY: &str = "shared_import_original_id";
const POISONING_QUARANTINED_AT_METADATA_KEY: &str = "poisoning_quarantined_at";
const POISONING_ALERT_TYPES_METADATA_KEY: &str = "poisoning_alert_types";
const POISONING_ALERT_IDS_METADATA_KEY: &str = "poisoning_alert_ids";
const POISONING_REMEDIATION_METADATA_KEY: &str = "poisoning_remediation";
const LEARNING_MIN_TOTAL_FEEDBACK: usize = 12;
const LEARNING_MIN_CLASS_FEEDBACK: usize = 3;
const LEARNING_FULL_CONFIDENCE_FEEDBACK: usize = 48;
const LEARNING_WEIGHT_FLOOR: f64 = 0.05;
const LEARNING_SIMILARITY_WEIGHT_CEILING: f64 = 0.70;
const LEARNING_RECENCY_WEIGHT_CEILING: f64 = 0.45;
const LEARNING_ACCESS_WEIGHT_CEILING: f64 = 0.05;
const LEARNING_PRIORITY_WEIGHT_CEILING: f64 = 0.45;

/// SQLite-backed [`MemoryStore`] implementation for the MVP memory schema.
#[derive(Clone)]
pub struct SqliteMemoryStore {
    connection: Arc<Mutex<Connection>>,
    scope: MemoryScope,
    embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SharedImportReport {
    pub(crate) new_ids: Vec<MemoryId>,
    pub(crate) review_ids: Vec<MemoryId>,
    pub(crate) quarantined_ids: Vec<MemoryId>,
    pub(crate) skipped_reasons: Vec<String>,
    pub(crate) outcomes: Vec<SharedImportOutcome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SharedImportAction {
    Review,
    Quarantine,
    Skip,
}

impl SharedImportAction {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Quarantine => "quarantine",
            Self::Skip => "skip",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharedImportOutcome {
    pub(crate) memory_id: Option<MemoryId>,
    pub(crate) disposition: SharedImportAction,
    pub(crate) reason: String,
    pub(crate) related_memory_id: Option<MemoryId>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LearnedWeightValues {
    pub(crate) similarity_weight: f64,
    pub(crate) recency_weight: f64,
    pub(crate) access_weight: f64,
    pub(crate) priority_weight: f64,
}

impl LearnedWeightValues {
    fn defaults() -> Self {
        let defaults = ScopeConfig::default();
        Self::from_scope_config(&defaults)
    }

    fn from_scope_config(config: &ScopeConfig) -> Self {
        Self {
            similarity_weight: f64::from(config.similarity_weight),
            recency_weight: f64::from(config.recency_weight),
            access_weight: f64::from(config.access_weight),
            priority_weight: f64::from(config.priority_weight),
        }
    }

    fn clamp(self) -> Self {
        let floors = [LEARNING_WEIGHT_FLOOR; 4];
        let ceilings = [
            LEARNING_SIMILARITY_WEIGHT_CEILING,
            LEARNING_RECENCY_WEIGHT_CEILING,
            LEARNING_ACCESS_WEIGHT_CEILING,
            LEARNING_PRIORITY_WEIGHT_CEILING,
        ];
        let normalized = normalize_weight_vector_with_bounds(
            [
                self.similarity_weight,
                self.recency_weight,
                self.access_weight,
                self.priority_weight,
            ],
            floors,
            ceilings,
        )
        .unwrap_or_else(|| {
            let defaults = Self::defaults();
            [
                defaults.similarity_weight,
                defaults.recency_weight,
                defaults.access_weight,
                defaults.priority_weight,
            ]
        });
        Self {
            similarity_weight: normalized[0],
            recency_weight: normalized[1],
            access_weight: normalized[2],
            priority_weight: normalized[3],
        }
    }

    fn blend(self, other: Self, factor: f64) -> Self {
        let factor = factor.clamp(0.0, 1.0);
        Self {
            similarity_weight: (self.similarity_weight * (1.0 - factor))
                + (other.similarity_weight * factor),
            recency_weight: (self.recency_weight * (1.0 - factor))
                + (other.recency_weight * factor),
            access_weight: (self.access_weight * (1.0 - factor)) + (other.access_weight * factor),
            priority_weight: (self.priority_weight * (1.0 - factor))
                + (other.priority_weight * factor),
        }
        .clamp()
    }

    fn with_multipliers(
        self,
        similarity_multiplier: f64,
        recency_multiplier: f64,
        access_multiplier: f64,
        priority_multiplier: f64,
    ) -> Self {
        Self {
            similarity_weight: self.similarity_weight * similarity_multiplier,
            recency_weight: self.recency_weight * recency_multiplier,
            access_weight: self.access_weight * access_multiplier,
            priority_weight: self.priority_weight * priority_multiplier,
        }
        .clamp()
    }

    fn to_hash_map(self) -> HashMap<String, f64> {
        HashMap::from([
            ("similarity_weight".to_string(), self.similarity_weight),
            ("recency_weight".to_string(), self.recency_weight),
            ("access_weight".to_string(), self.access_weight),
            ("priority_weight".to_string(), self.priority_weight),
        ])
    }

    pub(crate) fn to_btree_map(self) -> BTreeMap<String, f64> {
        BTreeMap::from([
            ("access_weight".to_string(), self.access_weight),
            ("priority_weight".to_string(), self.priority_weight),
            ("recency_weight".to_string(), self.recency_weight),
            ("similarity_weight".to_string(), self.similarity_weight),
        ])
    }
}

fn normalize_weight_vector_with_bounds(
    mut values: [f64; 4],
    floors: [f64; 4],
    ceilings: [f64; 4],
) -> Option<[f64; 4]> {
    for index in 0..values.len() {
        if !values[index].is_finite() {
            return None;
        }
        values[index] = values[index].clamp(floors[index], ceilings[index]);
    }

    for _ in 0..8 {
        let total = values.iter().sum::<f64>();
        let delta = 1.0 - total;
        if delta.abs() <= 1.0e-9 {
            return Some(values);
        }

        if delta > 0.0 {
            let slack = (0..values.len())
                .map(|index| (ceilings[index] - values[index]).max(0.0))
                .sum::<f64>();
            if slack <= f64::EPSILON {
                break;
            }

            for index in 0..values.len() {
                let local_slack = (ceilings[index] - values[index]).max(0.0);
                if local_slack <= f64::EPSILON {
                    continue;
                }
                values[index] =
                    (values[index] + (delta * (local_slack / slack))).min(ceilings[index]);
            }
        } else {
            let reducible = (0..values.len())
                .map(|index| (values[index] - floors[index]).max(0.0))
                .sum::<f64>();
            if reducible <= f64::EPSILON {
                break;
            }

            let excess = -delta;
            for index in 0..values.len() {
                let local_reducible = (values[index] - floors[index]).max(0.0);
                if local_reducible <= f64::EPSILON {
                    continue;
                }
                values[index] =
                    (values[index] - (excess * (local_reducible / reducible))).max(floors[index]);
            }
        }
    }

    let total = values.iter().sum::<f64>();
    if (total - 1.0).abs() <= 1.0e-6 {
        return Some(values);
    }

    None
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LearnedWeightsReport {
    pub(crate) sample_size: usize,
    pub(crate) relevant_samples: usize,
    pub(crate) irrelevant_samples: usize,
    pub(crate) using_defaults: bool,
    pub(crate) confidence: f64,
    pub(crate) status_detail: String,
    pub(crate) effective_weights: LearnedWeightValues,
    pub(crate) default_weights: LearnedWeightValues,
}

#[derive(Debug, Clone, Copy)]
struct RetrievalFeedbackSample {
    relevant: bool,
    similarity_signal: f64,
    recency_signal: f64,
    access_signal: f64,
    priority_signal: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PoisoningRemediationReport {
    pub(crate) quarantined_ids: Vec<MemoryId>,
    pub(crate) skipped_ids: Vec<MemoryId>,
    pub(crate) actions: Vec<PoisoningRemediationAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PoisoningRemediationAction {
    pub(crate) memory_id: MemoryId,
    pub(crate) action: PoisoningRemediationDisposition,
    pub(crate) reason: String,
    pub(crate) alert_ids: Vec<String>,
    pub(crate) alert_types: Vec<PoisoningAlertType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PoisoningRemediationDisposition {
    Quarantined,
    Skipped,
}

impl PoisoningRemediationDisposition {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Quarantined => "quarantined",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone)]
struct PoisoningConfig {
    frequency_hourly_threshold: u32,
    frequency_scope_ratio: f32,
    frequency_burst_ratio: f32,
    frequency_burst_min_hourly: u32,
    trust_mismatch_importance_threshold: f32,
    trust_mismatch_count_threshold: u32,
    trust_mismatch_scope_ratio: f32,
    bulk_overwrite_count_threshold: u32,
    bulk_overwrite_scope_ratio: f32,
    mass_contradiction_per_memory_threshold: u32,
    mass_contradiction_scope_ratio: f32,
    remediation_reliability_ceiling: f32,
}

#[derive(Debug, Clone)]
enum SharedImportDisposition {
    Review {
        reason: String,
        related_memory_id: Option<MemoryId>,
    },
    Quarantine {
        reason: String,
        related_memory_id: Option<MemoryId>,
    },
    Skip {
        reason: String,
    },
}

impl std::fmt::Debug for SqliteMemoryStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteMemoryStore")
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

impl SqliteMemoryStore {
    /// Open or create a SQLite-backed store at `path` for a single logical scope.
    pub fn new(path: impl AsRef<Path>, scope: MemoryScope) -> Result<Self, StoreError> {
        Self::new_with_optional_embedding_provider(path, scope, None)
    }

    /// Open or create a SQLite-backed store with an embedding provider for automatic embedding flows.
    pub fn new_with_embedding_provider(
        path: impl AsRef<Path>,
        scope: MemoryScope,
        embedding_provider: Arc<dyn EmbeddingProvider>,
    ) -> Result<Self, StoreError> {
        Self::new_with_optional_embedding_provider(path, scope, Some(embedding_provider))
    }

    /// Open or create a SQLite-backed store with an optional embedding provider.
    pub fn new_with_optional_embedding_provider(
        path: impl AsRef<Path>,
        scope: MemoryScope,
        embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
    ) -> Result<Self, StoreError> {
        let connection = init_database(path.as_ref())?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            scope,
            embedding_provider,
        })
    }

    /// Returns the scope this store instance is responsible for.
    #[must_use]
    pub const fn scope(&self) -> MemoryScope {
        self.scope
    }

    pub(crate) fn import_shared_with_report(
        &self,
        memories: &[Memory],
    ) -> Result<SharedImportReport, StoreError> {
        let gate = DefaultSalienceGate::new_with_optional_embedding_provider(
            self.scope_config()?,
            self.embedding_provider.clone(),
        );
        let mut report = SharedImportReport::default();

        for source in memories {
            let candidate = build_shared_import_candidate(source);
            let gate_decision = evaluate_gate_sync(gate.clone(), self.clone(), candidate)?;
            let decision =
                exact_shared_import_duplicate_decision(self, source)?.unwrap_or(gate_decision);
            let disposition = shared_import_disposition(&decision);

            match disposition {
                SharedImportDisposition::Skip { reason } => {
                    report.skipped_reasons.push(reason);
                    report.outcomes.push(SharedImportOutcome {
                        memory_id: None,
                        disposition: SharedImportAction::Skip,
                        reason: report.skipped_reasons.last().cloned().unwrap_or_default(),
                        related_memory_id: None,
                    });
                }
                SharedImportDisposition::Review {
                    reason,
                    related_memory_id,
                } => {
                    let imported = prepare_shared_import_memory(
                        source,
                        self.scope,
                        SHARED_REVIEW_STATUS,
                        "review",
                        &reason,
                        related_memory_id.as_ref(),
                    );
                    let imported_id = imported.id;
                    insert_memory_without_embedding(self, &imported)?;
                    report.new_ids.push(imported_id);
                    report.review_ids.push(imported_id);
                    report.outcomes.push(SharedImportOutcome {
                        memory_id: Some(imported_id),
                        disposition: SharedImportAction::Review,
                        reason,
                        related_memory_id,
                    });
                }
                SharedImportDisposition::Quarantine {
                    reason,
                    related_memory_id,
                } => {
                    let imported = prepare_shared_import_memory(
                        source,
                        self.scope,
                        QUARANTINED_STATUS,
                        "quarantine",
                        &reason,
                        related_memory_id.as_ref(),
                    );
                    let imported_id = imported.id;
                    insert_memory_without_embedding(self, &imported)?;

                    if let GateDecision::Contradiction {
                        conflicting_id,
                        description,
                    } = &decision
                    {
                        self.with_connection(|connection| {
                            let transaction = connection.transaction()?;
                            record_contradiction_row(
                                &transaction,
                                conflicting_id,
                                &imported_id,
                                description,
                            )?;
                            transaction.commit()?;
                            Ok(())
                        })?;
                    }

                    report.new_ids.push(imported_id);
                    report.quarantined_ids.push(imported_id);
                    report.outcomes.push(SharedImportOutcome {
                        memory_id: Some(imported_id),
                        disposition: SharedImportAction::Quarantine,
                        reason,
                        related_memory_id,
                    });
                }
            }
        }

        Ok(report)
    }

    pub(crate) fn remediate_poisoning(
        &self,
        alerts: &[PoisoningAlert],
    ) -> Result<PoisoningRemediationReport, StoreError> {
        let mut alert_types_by_id: HashMap<MemoryId, Vec<PoisoningAlertType>> = HashMap::new();
        let mut alert_ids_by_id: HashMap<MemoryId, Vec<String>> = HashMap::new();
        for alert in alerts {
            for memory_id in &alert.memory_ids {
                alert_types_by_id
                    .entry(*memory_id)
                    .or_default()
                    .push(alert.alert_type);
                alert_ids_by_id
                    .entry(*memory_id)
                    .or_default()
                    .push(alert.id.clone());
            }
        }

        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let config = load_poisoning_config(&transaction)?;
            let now = Utc::now();
            let mut report = PoisoningRemediationReport::default();
            let mut memory_ids: Vec<MemoryId> = alert_types_by_id.keys().copied().collect();
            sort_memory_ids(&mut memory_ids);

            for memory_id in memory_ids {
                let mut alert_types = alert_types_by_id.remove(&memory_id).unwrap_or_default();
                alert_types.sort_by_key(display_poisoning_alert_type);
                alert_types.dedup();
                let mut alert_ids = alert_ids_by_id.remove(&memory_id).unwrap_or_default();
                alert_ids.sort();
                alert_ids.dedup();
                let Some(mut memory) = require_memory(&transaction, &memory_id)? else {
                    report.skipped_ids.push(memory_id);
                    report.actions.push(PoisoningRemediationAction {
                        memory_id,
                        action: PoisoningRemediationDisposition::Skipped,
                        reason: "memory no longer exists; remediation skipped".to_string(),
                        alert_ids,
                        alert_types,
                    });
                    continue;
                };

                if memory.state != MemoryState::Active {
                    report.skipped_ids.push(memory_id);
                    report.actions.push(PoisoningRemediationAction {
                        memory_id,
                        action: PoisoningRemediationDisposition::Skipped,
                        reason: format!(
                            "memory is {:?}; remediation only quarantines active memories",
                            memory.state
                        ),
                        alert_ids,
                        alert_types,
                    });
                    continue;
                }

                if !is_low_trust_memory(&memory, config.remediation_reliability_ceiling) {
                    report.skipped_ids.push(memory_id);
                    report.actions.push(PoisoningRemediationAction {
                        memory_id,
                        action: PoisoningRemediationDisposition::Skipped,
                        reason: format!(
                            "memory remained active because reliability {:.2} and provenance {:?} exceed the low-trust ceiling {:.2}",
                            memory.reliability_score,
                            memory.provenance,
                            config.remediation_reliability_ceiling
                        ),
                        alert_ids,
                        alert_types,
                    });
                    continue;
                }

                let previous_memory = memory.clone();
                memory.state = MemoryState::Dormant;
                memory.status = Some(QUARANTINED_STATUS.to_string());
                memory.updated_at = now;
                memory.custom_metadata.insert(
                    POISONING_QUARANTINED_AT_METADATA_KEY.to_string(),
                    format_timestamp(now),
                );
                memory.custom_metadata.insert(
                    POISONING_ALERT_TYPES_METADATA_KEY.to_string(),
                    alert_types
                        .iter()
                        .map(display_poisoning_alert_type)
                        .collect::<Vec<_>>()
                        .join(","),
                );
                memory.custom_metadata.insert(
                    POISONING_ALERT_IDS_METADATA_KEY.to_string(),
                    alert_ids.join(","),
                );
                memory.custom_metadata.insert(
                    POISONING_REMEDIATION_METADATA_KEY.to_string(),
                    "dormant quarantine".to_string(),
                );

                persist_memory(&transaction, &memory)?;
                let row_id = require_memory_rowid(&transaction, &memory_id)?;
                sync_fts_entry(&transaction, row_id, Some(&previous_memory), &memory)?;
                report.quarantined_ids.push(memory_id);
                report.actions.push(PoisoningRemediationAction {
                    memory_id,
                    action: PoisoningRemediationDisposition::Quarantined,
                    reason: format!(
                        "quarantined low-trust active memory after alerts: {}",
                        alert_types
                            .iter()
                            .map(display_poisoning_alert_type)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    alert_ids,
                    alert_types,
                });
            }

            sort_memory_ids(&mut report.quarantined_ids);
            sort_memory_ids(&mut report.skipped_ids);
            report.actions.sort_by_key(|left| left.memory_id);
            transaction.commit()?;
            Ok(report)
        })
    }

    /// Promote a memory to a broader scope and record promotion provenance.
    pub fn promote_memory_to(
        &self,
        id: &MemoryId,
        to_scope: MemoryScope,
        changed_by: &str,
        reason: &str,
        trigger_session_id: Option<&str>,
    ) -> Result<Option<Memory>, StoreError> {
        self.with_connection(|connection| {
            let scope_config = load_scope_config(connection)?;
            let transaction = connection.transaction()?;
            let Some(mut memory) = require_memory(&transaction, id)? else {
                return Ok(None);
            };

            if memory.scope == to_scope {
                return Ok(Some(memory));
            }
            if !memory.scope.can_promote_to(to_scope) {
                return Err(StoreError::Validation(format!(
                    "cannot promote memory {} from {} to {}",
                    memory.id,
                    scope_to_db(memory.scope),
                    scope_to_db(to_scope)
                )));
            }

            record_promotion(
                &transaction,
                &mut memory,
                to_scope,
                reason,
                changed_by,
                trigger_session_id,
                &scope_config,
            )?;
            transaction.commit()?;
            Ok(Some(memory))
        })
    }

    /// Evaluate automatic promotion criteria and apply promotions for the visible scopes.
    pub fn run_promotion_pass(
        &self,
        limit: Option<usize>,
        trigger_session_id: Option<&str>,
    ) -> Result<Vec<Memory>, StoreError> {
        self.with_connection(|connection| {
            let scope_config = load_scope_config(connection)?;
            let mut memories = load_search_memories(
                connection,
                self.scope.visible_scopes(),
                MemoryState::Active,
                None,
                None,
            )?;
            memories.sort_by(|left, right| {
                right
                    .updated_at
                    .cmp(&left.updated_at)
                    .then_with(|| right.id.cmp(&left.id))
            });
            if let Some(limit) = limit {
                memories.truncate(limit);
            }

            let mut promoted = Vec::new();
            for memory in memories {
                if let Some(to_scope) =
                    promotion_target(connection, &memory, &scope_config, trigger_session_id)?
                {
                    let transaction = connection.transaction()?;
                    let Some(mut latest) = require_memory(&transaction, &memory.id)? else {
                        continue;
                    };
                    record_promotion(
                        &transaction,
                        &mut latest,
                        to_scope,
                        "automatic promotion pass",
                        "system:promotion",
                        trigger_session_id,
                        &scope_config,
                    )?;
                    transaction.commit()?;
                    promoted.push(latest);
                }
            }

            Ok(promoted)
        })
    }

    /// Load active memories plus their stored embeddings for consolidation.
    pub fn list_consolidation_candidates(
        &self,
        scopes: &[MemoryScope],
        limit: Option<usize>,
    ) -> Result<Vec<crate::ConsolidationCandidate>, StoreError> {
        self.with_connection(|connection| {
            let mut memories =
                load_search_memories(connection, scopes, MemoryState::Active, None, None)?;
            memories.sort_by(|left, right| {
                right
                    .updated_at
                    .cmp(&left.updated_at)
                    .then_with(|| right.id.cmp(&left.id))
            });
            if let Some(limit) = limit {
                memories.truncate(limit);
            }

            let expected_dimensions = load_embedding_dimensions(connection)?;
            let mut candidates = Vec::with_capacity(memories.len());
            for memory in memories {
                let embedding = load_stored_embedding(connection, &memory.id, expected_dimensions)?;
                candidates.push(crate::ConsolidationCandidate { memory, embedding });
            }
            Ok(candidates)
        })
    }

    /// Record the timestamp of the latest consolidation pass.
    pub fn mark_consolidation_run(&self) -> Result<(), StoreError> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO scope_config(key, value) VALUES ('last_consolidation_at', ?1) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [format_timestamp(Utc::now())],
            )?;
            Ok(())
        })
    }

    async fn generate_embedding(
        &self,
        text: &str,
        task: EmbeddingTask,
    ) -> Result<Option<Vec<f32>>, EmbeddingError> {
        let trimmed_text = text.trim();
        if trimmed_text.is_empty() {
            return Ok(None);
        }

        let Some(embedding_provider) = self.embedding_provider.as_ref() else {
            return Ok(None);
        };

        let prepared_input =
            prepare_embedding_input(embedding_provider.as_ref(), task, trimmed_text);
        embedding_provider
            .embed(prepared_input.as_ref())
            .await
            .map(Some)
    }

    async fn reuse_cached_embedding(
        &self,
        id: &MemoryId,
        content_sha256: &str,
    ) -> Result<bool, StoreError> {
        let Some(encoded_embedding) = self.with_connection(|connection| {
            load_cached_embedding_blob(connection, self.scope, content_sha256)
        })?
        else {
            return Ok(false);
        };

        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            if require_memory(&transaction, id)?.is_none() {
                return Err(StoreError::NotFound(*id));
            }

            let expected_dimensions = load_embedding_dimensions(&transaction)?;
            decode_embedding(&encoded_embedding, expected_dimensions)?;
            upsert_encoded_embedding(&transaction, id, &encoded_embedding, content_sha256)?;
            transaction.commit()?;

            Ok(true)
        })
    }

    fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| StoreError::Sqlite("sqlite connection lock poisoned".to_string()))?;
        operation(&mut connection)
    }

    /// Load the effective scope configuration used by this store.
    pub fn scope_config(&self) -> Result<ScopeConfig, StoreError> {
        self.with_connection(|connection| load_scope_config(connection))
    }

    /// Load version-history rows for a single memory without mutating access tracking.
    pub fn list_versions(&self, id: &MemoryId) -> Result<Vec<MemoryVersion>, StoreError> {
        self.with_connection(|connection| {
            if require_memory(connection, id)?.is_none() {
                return Err(StoreError::NotFound(*id));
            }

            let mut statement = connection.prepare(
                r#"
                SELECT id, memory_id, version_number, content, changed_by, change_reason, changed_at
                FROM memory_versions
                WHERE memory_id = ?1
                ORDER BY version_number DESC, changed_at DESC, id DESC
                "#,
            )?;
            let rows = statement.query_map([id.to_string()], map_memory_version_row)?;
            let mut versions = Vec::new();
            for row in rows {
                versions.push(row?);
            }
            Ok(versions)
        })
    }

    /// Load correction-history rows, optionally filtered to a single memory.
    pub fn list_corrections(
        &self,
        memory_id: Option<&MemoryId>,
        limit: usize,
    ) -> Result<Vec<CorrectionRecord>, StoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        self.with_connection(|connection| {
            if let Some(memory_id) = memory_id {
                if require_memory(connection, memory_id)?.is_none() {
                    return Err(StoreError::NotFound(*memory_id));
                }
            }

            let mut params: Vec<rusqlite::types::Value> = Vec::new();
            let mut sql = String::from(
                r#"
                SELECT id, memory_id, previous_content, corrected_content, corrected_by, reason,
                       disposition, related_memory_id, corrected_at
                FROM memory_corrections
                "#,
            );

            if let Some(memory_id) = memory_id {
                sql.push_str(" WHERE memory_id = ?1");
                params.push(rusqlite::types::Value::from(memory_id.to_string()));
            }

            sql.push_str(&format!(
                " ORDER BY corrected_at DESC, id DESC LIMIT ?{}",
                params.len() + 1
            ));
            params.push(rusqlite::types::Value::from(
                i64::try_from(limit).unwrap_or(i64::MAX),
            ));

            let mut statement = connection.prepare(&sql)?;
            let rows =
                statement.query_map(rusqlite::params_from_iter(params), map_correction_row)?;
            let mut corrections = Vec::new();
            for row in rows {
                corrections.push(row?);
            }
            Ok(corrections)
        })
    }

    /// Record a directional link between two memories in the proto-graph.
    pub fn record_link(
        &self,
        source_id: &MemoryId,
        target_id: &MemoryId,
        relation_type: &str,
    ) -> Result<(), StoreError> {
        if source_id == target_id {
            return Err(StoreError::Validation(
                "link source and target must be different memories".to_string(),
            ));
        }
        let relation = relation_type.trim();
        if relation.is_empty() {
            return Err(StoreError::Validation(
                "relation_type must not be empty".to_string(),
            ));
        }
        self.with_connection(|connection| {
            record_link_row(connection, source_id, target_id, relation)
        })
    }

    /// Query all links involving a given memory (as source or target).
    pub fn list_links(&self, memory_id: &MemoryId) -> Result<Vec<MemoryLink>, StoreError> {
        self.with_connection(|connection| load_links(connection, memory_id))
    }

    /// Record a corroboration between two memories, boosting the target's reliability.
    ///
    /// Increments the target memory's `corroboration_count` and adjusts its
    /// `reliability_score` by `+0.05` per corroboration, capped at
    /// `base_reliability + 0.2` where `base_reliability` is the provenance-level default.
    /// Also records a `"corroborates"` link from `source_id` to `target_id`.
    pub fn corroborate(
        &self,
        source_id: &MemoryId,
        target_id: &MemoryId,
    ) -> Result<(), StoreError> {
        if source_id == target_id {
            return Err(StoreError::Validation(
                "corroboration source and target must be different memories".to_string(),
            ));
        }
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;

            // Verify source exists
            require_memory(&transaction, source_id)?.ok_or(StoreError::NotFound(*source_id))?;

            // Load target for mutation
            let mut target =
                require_memory(&transaction, target_id)?.ok_or(StoreError::NotFound(*target_id))?;

            // Increment corroboration count
            target.corroboration_count += 1;

            // Calculate new reliability: min(current + 0.05, base + 0.2)
            let base = target.provenance.base_reliability();
            let cap = base + 0.2;
            let new_reliability = (target.reliability_score + 0.05).min(cap);
            target.reliability_score = new_reliability;
            target.updated_at = Utc::now();

            // Persist updated target
            persist_memory(&transaction, &target)?;

            // Record corroborates link
            record_link_row(&transaction, source_id, target_id, "corroborates")?;

            transaction.commit()?;
            Ok(())
        })
    }

    /// Restore a memory's content to a specific previous version.
    ///
    /// Loads the specified version from the `memory_versions` table and applies
    /// it as a new content update, creating a new version entry that records
    /// the rollback.
    pub fn rollback_to_version(
        &self,
        id: &MemoryId,
        version_number: u32,
    ) -> Result<(), StoreError> {
        self.with_connection(|connection| {
            let version_content: String = connection
                .query_row(
                    "SELECT content FROM memory_versions WHERE memory_id = ?1 AND version_number = ?2",
                    params![id.to_string(), version_number],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or_else(|| {
                    StoreError::Validation(format!(
                        "version {version_number} not found for memory {id}"
                    ))
                })?;

            let transaction = connection.transaction()?;
            let memory =
                require_memory(&transaction, id)?.ok_or(StoreError::NotFound(*id))?;

            if memory.content == version_content {
                return Ok(());
            }

            let row_id = require_memory_rowid(&transaction, id)?;
            let next_version_number = load_next_version_number(&transaction, id)?;
            let now = Utc::now();

            transaction.execute(
                r#"
                INSERT INTO memory_versions(
                    id,
                    memory_id,
                    version_number,
                    content,
                    changed_at,
                    changed_by,
                    change_reason
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    Uuid::new_v4().to_string(),
                    id.to_string(),
                    i64::from(next_version_number),
                    memory.content,
                    format_timestamp(now),
                    "system:rollback",
                    format!("rollback to version {version_number}"),
                ],
            )?;

            let mut updated = memory.clone();
            updated.content = version_content;
            updated.embedding_stale = true;
            updated.updated_at = now;

            persist_memory(&transaction, &updated)?;
            sync_fts_entry(&transaction, row_id, Some(&memory), &updated)?;

            transaction.commit()?;
            Ok(())
        })
    }

    /// Enforce the active memory budget and storage cap for this scope.
    ///
    /// When the number of active memories exceeds `budget_active_max` from the
    /// scope configuration, the lowest-scoring active memories are transitioned
    /// to dormant. When total storage exceeds the storage cap, the lowest-scoring
    /// dormant memories are hard-deleted.
    ///
    /// Returns the number of memories made dormant and the number hard-deleted.
    pub fn enforce_budget(&self) -> Result<(u64, u64), StoreError> {
        self.with_connection(|connection| {
            let scope = self.scope;

            // Load budget_active_max with scope-appropriate defaults
            let configured_budget = load_config_value(connection, "budget_active_max")?;
            let budget_active_max: u64 = match configured_budget {
                Some(value) => value.parse::<u64>().map_err(|error| {
                    StoreError::Serialization(format!(
                        "invalid budget_active_max config `{value}`: {error}"
                    ))
                })?,
                None => match scope {
                    MemoryScope::Workspace => DEFAULT_WORKSPACE_BUDGET as u64,
                    MemoryScope::User => DEFAULT_USER_BUDGET as u64,
                    MemoryScope::Agent => DEFAULT_AGENT_BUDGET as u64,
                    MemoryScope::Session => 0,
                },
            };

            // Load storage_cap_mb (default 512 MB)
            let configured_cap = load_config_value(connection, "storage_cap_mb")?;
            let storage_cap_mb: u64 = match configured_cap {
                Some(value) => value.parse::<u64>().map_err(|error| {
                    StoreError::Serialization(format!(
                        "invalid storage_cap_mb config `{value}`: {error}"
                    ))
                })?,
                None => 512,
            };

            let mut dormant_count: u64 = 0;
            let mut deleted_count: u64 = 0;

            // Phase 1: Enforce active budget
            if budget_active_max > 0 {
                let active_count = count_memories_by_state(connection, scope, MemoryState::Active)?;
                if active_count > 0 {
                    let active_u64 = active_count as u64;
                    if active_u64 > budget_active_max {
                        let excess = active_u64 - budget_active_max;
                        let mut active_memories = load_search_memories(
                            connection,
                            &[scope],
                            MemoryState::Active,
                            None,
                            None,
                        )?;
                        // Sort ascending by composite score (lowest first = eviction candidates)
                        active_memories.sort_by(|a, b| {
                            let score_a = a.importance_score * a.reliability_score;
                            let score_b = b.importance_score * b.reliability_score;
                            score_a
                                .partial_cmp(&score_b)
                                .unwrap_or(std::cmp::Ordering::Equal)
                                .then_with(|| a.id.cmp(&b.id))
                        });

                        let to_demote = excess.min(active_memories.len() as u64) as usize;
                        let now = Utc::now();
                        let transaction = connection.transaction()?;
                        for memory in active_memories.iter().take(to_demote) {
                            let mut dormant = memory.clone();
                            dormant.state = MemoryState::Dormant;
                            dormant.updated_at = now;
                            persist_memory(&transaction, &dormant)?;
                            dormant_count += 1;
                        }
                        transaction.commit()?;
                    }
                }
            }

            // Phase 2: Enforce storage cap
            let page_count: u64 =
                connection.query_row("SELECT page_count FROM pragma_page_count", [], |row| {
                    row.get(0)
                })?;
            let page_size: u64 =
                connection.query_row("SELECT page_size FROM pragma_page_size", [], |row| {
                    row.get(0)
                })?;
            let current_bytes = page_count * page_size;
            let cap_bytes = storage_cap_mb * 1024 * 1024;

            if current_bytes > cap_bytes {
                let mut dormant_memories =
                    load_search_memories(connection, &[scope], MemoryState::Dormant, None, None)?;
                // Sort ascending by composite score (lowest first = deletion candidates)
                dormant_memories.sort_by(|a, b| {
                    let score_a = a.importance_score * a.reliability_score;
                    let score_b = b.importance_score * b.reliability_score;
                    score_a
                        .partial_cmp(&score_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.id.cmp(&b.id))
                });

                for memory in &dormant_memories {
                    let transaction = connection.transaction()?;
                    let row_id = require_memory_rowid(&transaction, &memory.id)?;

                    let vec_rowid: Option<i64> = transaction
                        .query_row(
                            "SELECT vec_rowid FROM memory_embeddings WHERE memory_id = ?1",
                            [memory.id.to_string()],
                            |row| row.get(0),
                        )
                        .optional()?;

                    delete_fts_entry(&transaction, row_id, memory)?;

                    if let Some(vec_rowid) = vec_rowid {
                        transaction
                            .execute("DELETE FROM vec_memories WHERE rowid = ?1", [vec_rowid])?;
                    }
                    transaction.execute(
                        "DELETE FROM memory_embeddings WHERE memory_id = ?1",
                        [memory.id.to_string()],
                    )?;
                    transaction.execute(
                        "DELETE FROM memory_versions WHERE memory_id = ?1",
                        [memory.id.to_string()],
                    )?;
                    transaction.execute(
                        "DELETE FROM memory_links WHERE source_id = ?1 OR target_id = ?1",
                        [memory.id.to_string()],
                    )?;
                    transaction.execute(
                        "DELETE FROM contradictions WHERE memory_a_id = ?1 OR memory_b_id = ?1",
                        [memory.id.to_string()],
                    )?;
                    transaction.execute(
                        "DELETE FROM memories WHERE id = ?1",
                        [memory.id.to_string()],
                    )?;

                    transaction.commit()?;
                    deleted_count += 1;

                    // Re-check storage after each deletion
                    let new_page_count: u64 = connection.query_row(
                        "SELECT page_count FROM pragma_page_count",
                        [],
                        |row| row.get(0),
                    )?;
                    let new_bytes = new_page_count * page_size;
                    if new_bytes <= cap_bytes {
                        break;
                    }
                }
            }

            Ok((dormant_count, deleted_count))
        })
    }

    /// Delete a link between two memories by its unique identifier.
    ///
    /// Returns `Ok(true)` if a link was deleted, `Ok(false)` if no link matched.
    pub fn delete_link(&self, link_id: &str) -> Result<bool, StoreError> {
        let trimmed = link_id.trim();
        if trimmed.is_empty() {
            return Err(StoreError::Validation(
                "link_id must not be empty".to_string(),
            ));
        }
        self.with_connection(|connection| {
            let deleted_rows =
                connection.execute("DELETE FROM memory_links WHERE id = ?1", [trimmed])?;
            Ok(deleted_rows > 0)
        })
    }

    /// Traverse the memory link graph starting from a given memory using BFS.
    ///
    /// Discovers connected memories up to `max_depth` hops away, optionally
    /// filtering by relation type. Returns a [`GraphTraversalResult`] with
    /// all discovered nodes ordered by depth.
    pub fn traverse_links(
        &self,
        start_id: &MemoryId,
        max_depth: u32,
        relation_filter: Option<&str>,
    ) -> Result<GraphTraversalResult, StoreError> {
        self.with_connection(|connection| {
            // Validate start memory exists.
            let start_memory =
                require_memory(connection, start_id)?.ok_or(StoreError::NotFound(*start_id))?;

            let mut visited = HashSet::new();
            visited.insert(*start_id);

            let mut queue: VecDeque<(MemoryId, u32)> = VecDeque::new();
            queue.push_back((*start_id, 0));

            // Root node at depth 0 with no incoming links.
            let mut nodes = vec![GraphNode {
                memory: start_memory,
                depth: 0,
                incoming_links: Vec::new(),
            }];

            while let Some((current_id, current_depth)) = queue.pop_front() {
                if current_depth >= max_depth {
                    continue;
                }

                let links = load_links(connection, &current_id)?;

                for link in links {
                    // Apply optional relation-type filter.
                    if let Some(filter) = relation_filter {
                        if link.relation_type != filter {
                            continue;
                        }
                    }

                    // Determine the neighbor (the other end of the link).
                    let neighbor_id = if link.source_id == current_id {
                        link.target_id
                    } else {
                        link.source_id
                    };

                    if visited.contains(&neighbor_id) {
                        continue;
                    }
                    visited.insert(neighbor_id);

                    // Only include neighbors whose memory record still exists.
                    if let Some(neighbor_memory) = require_memory(connection, &neighbor_id)? {
                        nodes.push(GraphNode {
                            memory: neighbor_memory,
                            depth: current_depth + 1,
                            incoming_links: vec![link],
                        });
                        queue.push_back((neighbor_id, current_depth + 1));
                    }
                }
            }

            Ok(GraphTraversalResult {
                start_id: *start_id,
                max_depth,
                nodes,
            })
        })
    }

    /// Run heuristic-based poisoning detection on the memory store.
    ///
    /// Checks for:
    /// - **Frequency anomaly**: unusually high write frequency in recent time windows
    /// - **Trust mismatch**: low-provenance memories with high importance scores
    /// - **Bulk overwrite**: many memory updates in a short window
    /// - **Mass contradiction**: memories that contradict many existing memories
    ///
    /// Returns a list of [`PoisoningAlert`] values ordered by severity descending.
    pub fn detect_poisoning(&self) -> Result<Vec<PoisoningAlert>, StoreError> {
        self.with_connection(|connection| {
            let mut alerts: Vec<PoisoningAlert> = Vec::new();
            let poisoning_config = load_poisoning_config(connection)?;
            let now = Utc::now();
            let scope_str = scope_to_db(self.scope);
            let one_hour_ago = format_timestamp(now - chrono::Duration::hours(1));
            let one_day_ago = format_timestamp(now - chrono::Duration::hours(24));
            let scope_total: i64 = connection.query_row(
                "SELECT COUNT(*) FROM memories WHERE scope = ?1 AND state != 'deleted'",
                params![scope_str],
                |row| row.get(0),
            )?;
            let scope_active: i64 = connection.query_row(
                "SELECT COUNT(*) FROM memories WHERE scope = ?1 AND state = 'active'",
                params![scope_str],
                |row| row.get(0),
            )?;

            // ── Frequency anomaly ──────────────────────────────────────
            let mut hourly_stmt = connection.prepare(
                "SELECT id \
                 FROM memories \
                 WHERE scope = ?1 \
                   AND state = 'active' \
                   AND created_at > ?2 \
                 ORDER BY created_at DESC, id ASC",
            )?;
            let mut hourly_ids: Vec<MemoryId> = hourly_stmt
                .query_map(params![scope_str, one_hour_ago], |row| {
                    let raw: String = row.get(0)?;
                    parse_uuid_for_sqlite(&raw)
                })?
                .collect::<Result<Vec<_>, _>>()?;
            sort_memory_ids(&mut hourly_ids);
            let hourly_count = i64::try_from(hourly_ids.len()).unwrap_or(i64::MAX);

            let daily_count: i64 = connection.query_row(
                "SELECT COUNT(*) \
                 FROM memories \
                 WHERE scope = ?1 \
                   AND state = 'active' \
                   AND created_at > ?2",
                params![scope_str, one_day_ago],
                |row| row.get(0),
            )?;

            let frequency_threshold = scaled_threshold(
                scope_total,
                poisoning_config.frequency_hourly_threshold,
                poisoning_config.frequency_scope_ratio,
                1,
            );
            let burst_detected = daily_count > 0
                && usize::try_from(hourly_count).unwrap_or(usize::MAX)
                    >= usize::try_from(poisoning_config.frequency_burst_min_hourly).unwrap_or(usize::MAX)
                && ((hourly_count as f32) / (daily_count as f32))
                    >= poisoning_config.frequency_burst_ratio;

            if usize::try_from(hourly_count).unwrap_or(usize::MAX) >= frequency_threshold
                || burst_detected
            {
                let severity = compute_poisoning_severity(
                    compute_threshold_pressure(
                        usize::try_from(hourly_count).unwrap_or(usize::MAX),
                        frequency_threshold,
                    )
                    .max(if daily_count > 0 && poisoning_config.frequency_burst_ratio > 0.0 {
                        ((hourly_count as f32) / (daily_count as f32))
                            / poisoning_config.frequency_burst_ratio
                    } else {
                        0.0
                    }),
                    hourly_ids.len(),
                    scope_total,
                    0.45,
                );

                alerts.push(PoisoningAlert {
                    id: Uuid::new_v4().to_string(),
                    alert_type: PoisoningAlertType::FrequencyAnomaly,
                    description: format!(
                        "Unusually high write frequency in scope `{scope_str}`: {hourly_count} memories \
                         created in the last hour ({daily_count} in the last 24 h; alert threshold \
                         {frequency_threshold}, burst ratio {:.2}).",
                        poisoning_config.frequency_burst_ratio
                    ),
                    severity,
                    memory_ids: hourly_ids,
                    detected_at: now,
                });
            }

            // ── Trust mismatch ─────────────────────────────────────────
            {
                let mut stmt = connection.prepare(
                    "SELECT id FROM memories \
                     WHERE scope = ?1 \
                       AND provenance IN ('agent_inferred', 'imported') \
                        AND importance_score >= ?2 \
                        AND state = 'active'",
                )?;
                let mut ids: Vec<MemoryId> = stmt
                    .query_map(
                        params![
                            scope_str,
                            f64::from(poisoning_config.trust_mismatch_importance_threshold)
                        ],
                        |row| {
                            let raw: String = row.get(0)?;
                            parse_uuid_for_sqlite(&raw)
                        },
                    )?
                    .collect::<Result<Vec<_>, _>>()?;
                sort_memory_ids(&mut ids);

                let trust_mismatch_threshold = scaled_threshold(
                    scope_active,
                    poisoning_config.trust_mismatch_count_threshold,
                    poisoning_config.trust_mismatch_scope_ratio,
                    1,
                );

                if ids.len() >= trust_mismatch_threshold {
                    let average_importance: f32 = connection.query_row(
                        "SELECT COALESCE(AVG(importance_score), 0.0) FROM memories \
                         WHERE scope = ?1 \
                           AND provenance IN ('agent_inferred', 'imported') \
                           AND importance_score >= ?2 \
                           AND state = 'active'",
                        params![
                            scope_str,
                            f64::from(poisoning_config.trust_mismatch_importance_threshold)
                        ],
                        |row| row.get(0),
                    )?;
                    let severity = compute_poisoning_severity(
                        compute_threshold_pressure(ids.len(), trust_mismatch_threshold).max(
                            average_importance
                                / poisoning_config
                                    .trust_mismatch_importance_threshold
                                    .max(f32::EPSILON),
                        ),
                        ids.len(),
                        scope_active,
                        0.35,
                    );

                    alerts.push(PoisoningAlert {
                        id: Uuid::new_v4().to_string(),
                        alert_type: PoisoningAlertType::TrustMismatch,
                        description: format!(
                            "{} low-provenance memories in scope `{scope_str}` have importance >= {:.2} \
                             (alert threshold {}).",
                            ids.len(),
                            poisoning_config.trust_mismatch_importance_threshold,
                            trust_mismatch_threshold
                        ),
                        severity,
                        memory_ids: ids,
                        detected_at: now,
                    });
                }
            }

            // ── Bulk overwrite ─────────────────────────────────────────
            {
                let mut stmt = connection.prepare(
                    "SELECT v.memory_id, v.changed_by \
                     FROM memory_versions v \
                     JOIN memories m ON m.id = v.memory_id \
                     WHERE m.scope = ?1 \
                       AND m.state = 'active' \
                       AND v.changed_at > ?2 \
                     ORDER BY v.changed_at DESC, v.memory_id ASC",
                )?;
                let version_rows = stmt
                    .query_map(params![scope_str, one_hour_ago], |row| {
                        let raw_id: String = row.get(0)?;
                        Ok((parse_uuid_for_sqlite(&raw_id)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                let mut ids: Vec<MemoryId> = version_rows
                    .into_iter()
                    .filter_map(|(memory_id, changed_by)| {
                        (!is_benign_poisoning_update(&changed_by)).then_some(memory_id)
                    })
                    .collect();
                sort_memory_ids(&mut ids);
                ids.dedup();

                let bulk_overwrite_threshold = scaled_threshold(
                    scope_active,
                    poisoning_config.bulk_overwrite_count_threshold,
                    poisoning_config.bulk_overwrite_scope_ratio,
                    1,
                );

                if ids.len() >= bulk_overwrite_threshold {
                    let severity = compute_poisoning_severity(
                        compute_threshold_pressure(ids.len(), bulk_overwrite_threshold),
                        ids.len(),
                        scope_active,
                        0.4,
                    );

                    alerts.push(PoisoningAlert {
                        id: Uuid::new_v4().to_string(),
                        alert_type: PoisoningAlertType::BulkOverwrite,
                        description: format!(
                            "{} distinct active memories in scope `{scope_str}` were updated \
                             in the last hour by non-system actors (alert threshold {bulk_overwrite_threshold}).",
                            ids.len()
                        ),
                        severity,
                        memory_ids: ids,
                        detected_at: now,
                    });
                }
            }

            // ── Mass contradiction ─────────────────────────────────────
            {
                let mass_contradiction_population_threshold = scaled_threshold(
                    scope_active,
                    1,
                    poisoning_config.mass_contradiction_scope_ratio,
                    1,
                );
                let mut stmt = connection.prepare(
                    "SELECT \
                        c.memory_a_id, ma.scope, ma.state, ma.reliability_score, ma.provenance, \
                        c.memory_b_id, mb.scope, mb.state, mb.reliability_score, mb.provenance \
                     FROM contradictions c \
                     JOIN memories ma ON ma.id = c.memory_a_id \
                     JOIN memories mb ON mb.id = c.memory_b_id \
                     WHERE c.resolution_status = 'unresolved' \
                       AND ( \
                           (ma.scope = ?1 AND ma.state = 'active') \
                           OR (mb.scope = ?1 AND mb.state = 'active') \
                       )",
                )?;
                let contradiction_pairs = stmt
                    .query_map(params![scope_str], |row| {
                        Ok((
                            load_poisoning_memory_snapshot(row, 0)?,
                            load_poisoning_memory_snapshot(row, 5)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                let mut contradiction_counts: HashMap<MemoryId, u32> = HashMap::new();
                for (memory_a, memory_b) in contradiction_pairs {
                    for memory_id in select_mass_contradiction_candidates(
                        self.scope,
                        &memory_a,
                        &memory_b,
                        poisoning_config.remediation_reliability_ceiling,
                    ) {
                        *contradiction_counts.entry(memory_id).or_default() += 1;
                    }
                }
                let mut rows: Vec<(MemoryId, u32)> = contradiction_counts
                    .into_iter()
                    .filter(|(_, total)| {
                        *total >= poisoning_config.mass_contradiction_per_memory_threshold
                    })
                    .collect();
                rows.sort_by(|(left_id, left_total), (right_id, right_total)| {
                    right_total.cmp(left_total).then_with(|| left_id.cmp(right_id))
                });

                if rows.len() >= mass_contradiction_population_threshold {
                    let ids: Vec<MemoryId> = rows.iter().map(|(id, _)| *id).collect();
                    let max_total = rows.iter().map(|(_, total)| *total).max().unwrap_or(0);
                    let severity = compute_poisoning_severity(
                        compute_threshold_pressure(
                            usize::try_from(max_total).unwrap_or(usize::MAX),
                            usize::try_from(
                                poisoning_config.mass_contradiction_per_memory_threshold,
                            )
                            .unwrap_or(usize::MAX),
                        )
                        .max(compute_threshold_pressure(
                            ids.len(),
                            mass_contradiction_population_threshold,
                        )),
                        ids.len(),
                        scope_active,
                        0.5,
                    );

                    alerts.push(PoisoningAlert {
                        id: Uuid::new_v4().to_string(),
                        alert_type: PoisoningAlertType::MassContradiction,
                        description: format!(
                            "{} low-trust active memories in scope `{scope_str}` are on the weaker \
                             side of {} or more unresolved contradictions (alert threshold {}).",
                            rows.len(),
                            poisoning_config.mass_contradiction_per_memory_threshold,
                            mass_contradiction_population_threshold
                        ),
                        severity,
                        memory_ids: ids,
                        detected_at: now,
                    });
                }
            }

            // Sort by severity descending.
            alerts.sort_by(|a, b| {
                b.severity
                    .partial_cmp(&a.severity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            Ok(alerts)
        })
    }

    /// Apply a user-supplied correction to an existing memory.
    ///
    /// Routes the corrected content back through the store's write-time
    /// duplicate / contradiction safety model, records the outcome in
    /// `memory_corrections`, creates version entries, and refreshes any
    /// affected embeddings immediately when a provider or cache is available.
    pub fn correct_memory(
        &self,
        id: &MemoryId,
        corrected_content: &str,
        corrected_by: &str,
        reason: Option<&str>,
    ) -> Result<CorrectionRecord, StoreError> {
        let trimmed_content = corrected_content.trim();
        if trimmed_content.is_empty() {
            return Err(StoreError::Validation(
                "corrected content must not be empty".to_string(),
            ));
        }
        let corrected_by = corrected_by.trim();
        if corrected_by.is_empty() {
            return Err(StoreError::Validation(
                "corrected_by must not be empty".to_string(),
            ));
        }

        let existing_memory = self.with_connection(|connection| {
            require_memory(connection, id)?.ok_or(StoreError::NotFound(*id))
        })?;
        if existing_memory.content.trim() == trimmed_content {
            return Err(StoreError::Validation(
                "corrected content must differ from the current memory content".to_string(),
            ));
        }

        let decision = self.evaluate_correction_decision(id, &existing_memory, trimmed_content)?;
        let reason_text = reason.unwrap_or("").trim().to_string();
        let mut embeddings_to_refresh = vec![(*id, trimmed_content.to_string())];

        let (correction, target_embedding_refresh) = self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let mut memory = require_memory(&transaction, id)?.ok_or(StoreError::NotFound(*id))?;
            let previous_content = memory.content.clone();
            let previous_memory = memory.clone();
            let row_id = require_memory_rowid(&transaction, id)?;
            let correction_id = Uuid::new_v4().to_string();
            let now = Utc::now();
            let mut disposition = CorrectionDisposition::Applied;
            let mut related_memory_id = None;
            let mut target_embedding_refresh = None;

            insert_memory_version_row(
                &transaction,
                id,
                &previous_content,
                now,
                corrected_by,
                &format!("user correction: {reason_text}"),
            )?;

            memory.content = trimmed_content.to_string();
            memory.reliability_score = (memory.reliability_score + 0.1).min(1.0);
            memory.embedding_stale = true;
            memory.updated_at = now;

            match &decision {
                GateDecision::Accept { similar_to, .. } => {
                    related_memory_id = *similar_to;
                }
                GateDecision::Archive => {
                    disposition = CorrectionDisposition::Archived;
                    memory.state = MemoryState::Dormant;
                }
                GateDecision::Merge {
                    target_id,
                    enriched_content,
                    ..
                } => {
                    disposition = CorrectionDisposition::Merged;
                    related_memory_id = Some(*target_id);
                    memory.state = MemoryState::Dormant;

                    let mut target = require_memory(&transaction, target_id)?
                        .ok_or(StoreError::NotFound(*target_id))?;
                    let target_previous = target.clone();
                    let target_row_id = require_memory_rowid(&transaction, target_id)?;
                    let target_content_changed = target.content != *enriched_content;

                    if target_content_changed {
                        insert_memory_version_row(
                            &transaction,
                            target_id,
                            &target.content,
                            now,
                            corrected_by,
                            &format!("merged from correction on {id}: {reason_text}"),
                        )?;
                        target.content = enriched_content.clone();
                        target.embedding_stale = true;
                        target.updated_at = now;
                    }

                    target.reliability_score = (target.reliability_score + 0.1).min(1.0);
                    persist_memory(&transaction, &target)?;
                    sync_fts_entry(&transaction, target_row_id, Some(&target_previous), &target)?;

                    if target_content_changed {
                        target_embedding_refresh = Some((*target_id, enriched_content.clone()));
                    }
                }
                GateDecision::Contradiction { conflicting_id, .. } => {
                    disposition = CorrectionDisposition::Contradiction;
                    related_memory_id = Some(*conflicting_id);
                }
                GateDecision::Reject { reason } => {
                    return Err(StoreError::Validation(format!(
                        "correction rejected by safety gate: {reason}"
                    )));
                }
            }

            persist_memory(&transaction, &memory)?;
            sync_fts_entry(&transaction, row_id, Some(&previous_memory), &memory)?;

            if let GateDecision::Contradiction {
                conflicting_id,
                description,
            } = &decision
            {
                record_contradiction_row(&transaction, conflicting_id, id, description)?;
            }

            transaction.execute(
                r#"
                INSERT INTO memory_corrections(
                    id,
                    memory_id,
                    previous_content,
                    corrected_content,
                    corrected_by,
                    reason,
                    disposition,
                    related_memory_id,
                    corrected_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
                params![
                    correction_id,
                    id.to_string(),
                    previous_content,
                    trimmed_content,
                    corrected_by,
                    &reason_text,
                    correction_disposition_to_db(disposition),
                    related_memory_id.map(|memory_id| memory_id.to_string()),
                    format_timestamp(now),
                ],
            )?;

            transaction.commit()?;

            Ok((
                CorrectionRecord {
                    id: correction_id,
                    memory_id: *id,
                    previous_content,
                    corrected_content: trimmed_content.to_string(),
                    corrected_by: corrected_by.to_string(),
                    reason: reason_text.clone(),
                    disposition,
                    related_memory_id,
                    corrected_at: now,
                },
                target_embedding_refresh,
            ))
        })?;

        if let Some(target_embedding_refresh) = target_embedding_refresh {
            embeddings_to_refresh.push(target_embedding_refresh);
        }
        for (memory_id, content) in embeddings_to_refresh {
            self.refresh_embedding_after_content_change(&memory_id, &content)?;
        }

        Ok(correction)
    }

    fn evaluate_correction_decision(
        &self,
        id: &MemoryId,
        memory: &Memory,
        corrected_content: &str,
    ) -> Result<GateDecision, StoreError> {
        if let Some(decision) = exact_correction_duplicate_decision(self, id, corrected_content)? {
            return Ok(decision);
        }

        let gate = DefaultSalienceGate::new_with_optional_embedding_provider(
            self.scope_config()?,
            self.embedding_provider.clone(),
        );
        let candidate = MemoryCandidate {
            content: corrected_content.to_string(),
            summary: memory.summary.clone(),
            memory_type: memory.memory_type,
            provenance: memory.provenance,
            importance_score: memory.importance_score,
            sensitivity: memory.sensitivity,
            tags: memory.tags.clone(),
            custom_metadata: memory.custom_metadata.clone(),
            embedding: None,
        };

        run_store_future({
            let store = self.clone();
            let candidate = candidate.clone();
            let excluded_id = *id;
            async move {
                gate.evaluate_excluding(&candidate, &store, excluded_id)
                    .await
                    .map_err(|error| match error {
                        GateError::Store(store_error) => store_error,
                        other => StoreError::Validation(format!(
                            "correction gate evaluation failed: {other}"
                        )),
                    })
            }
        })
    }

    fn refresh_embedding_after_content_change(
        &self,
        id: &MemoryId,
        content: &str,
    ) -> Result<(), StoreError> {
        let trimmed_content = content.trim();
        if trimmed_content.is_empty() {
            return Ok(());
        }

        run_store_future({
            let store = self.clone();
            let memory_id = *id;
            let content_sha = content_sha256(trimmed_content);
            let content = trimmed_content.to_string();
            async move {
                if store
                    .reuse_cached_embedding(&memory_id, &content_sha)
                    .await?
                {
                    return Ok(());
                }

                let embedding = match store
                    .generate_embedding(&content, EmbeddingTask::Document)
                    .await
                {
                    Ok(Some(embedding)) => embedding,
                    Ok(None) => return Ok(()),
                    Err(error) => {
                        if let Some(warning) = embedding_degradation_warning(&error) {
                            eprintln!("warning: {warning}");
                        }
                        return Ok(());
                    }
                };

                match store.store_embedding(&memory_id, &embedding).await {
                    Ok(()) | Err(StoreError::Validation(_)) => Ok(()),
                    Err(error) => Err(error),
                }
            }
        })
    }

    /// Record relevance feedback for a memory that was returned by a search.
    ///
    /// This feedback accumulates in `retrieval_feedback` for future scoring
    /// weight adjustments.  When a memory is marked relevant, its access
    /// count is incremented; when irrelevant, its importance score is nudged
    /// downward by 0.02 (floored at 0.0).
    pub fn record_feedback(
        &self,
        memory_id: &MemoryId,
        query_text: &str,
        was_relevant: bool,
    ) -> Result<RetrievalFeedback, StoreError> {
        self.with_connection(|connection| {
            let mut memory =
                require_memory(connection, memory_id)?.ok_or(StoreError::NotFound(*memory_id))?;

            let transaction = connection.transaction()?;

            let feedback_id = Uuid::new_v4().to_string();
            let now = Utc::now();

            transaction.execute(
                r#"
                INSERT INTO retrieval_feedback (id, memory_id, query_text, relevant, recorded_at)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                params![
                    feedback_id,
                    memory_id.to_string(),
                    query_text,
                    i64::from(was_relevant),
                    format_timestamp(now),
                ],
            )?;

            if was_relevant {
                memory.access_count += 1;
            } else {
                memory.importance_score = (memory.importance_score - 0.02).max(0.0);
            }
            persist_memory(&transaction, &memory)?;

            let learning_report = compute_learned_weights_report(&transaction)?;
            persist_learned_weights(&transaction, learning_report.effective_weights)?;

            transaction.commit()?;

            Ok(RetrievalFeedback {
                id: feedback_id,
                memory_id: *memory_id,
                relevant: was_relevant,
                query_text: Some(query_text.to_string()),
                recorded_at: now,
            })
        })
    }

    /// Compute adjusted scoring weights based on accumulated feedback.
    ///
    /// Analyzes the `retrieval_feedback` table and derives effective
    /// `scope_config` weights keyed exactly the way live search reads them.
    pub fn compute_learned_weights(&self) -> Result<HashMap<String, f64>, StoreError> {
        self.learned_weights_report()
            .map(|report| report.effective_weights.to_hash_map())
    }

    pub(crate) fn learned_weights_report(&self) -> Result<LearnedWeightsReport, StoreError> {
        self.with_connection(|connection| compute_learned_weights_report(connection))
    }

    /// Export memories suitable for sharing with other agents.
    ///
    /// Filters active memories by the criteria in `config` (sensitivity ceiling,
    /// minimum reliability, optional type and tag filters), then returns sanitized
    /// copies with provenance reset to [`ProvenanceLevel::Imported`] and identity
    /// fields (`tenant_id`, `user_id`, `agent_id`) cleared.
    pub fn export_for_sharing(&self, config: &ShareConfig) -> Result<Vec<Memory>, StoreError> {
        self.with_connection(|connection| {
            let type_filter = config.type_filter.as_deref();
            let all = load_search_memories(
                connection,
                &[self.scope],
                MemoryState::Active,
                type_filter,
                None,
            )?;
            let max_ord = sensitivity_ord(config.max_sensitivity);
            let shared: Vec<Memory> = all
                .into_iter()
                .filter(|m| {
                    sensitivity_ord(m.sensitivity) <= max_ord
                        && m.reliability_score >= config.min_reliability
                })
                .filter(|m| {
                    config
                        .tag_filter
                        .as_ref()
                        .is_none_or(|required| required.iter().all(|t| m.tags.contains(t)))
                })
                .map(|mut m| {
                    m.provenance = ProvenanceLevel::Imported;
                    m.tenant_id = None;
                    m.user_id = None;
                    m.agent_id = None;
                    m
                })
                .collect();
            Ok(shared)
        })
    }

    /// Import memories shared from another agent.
    ///
    /// Each memory receives a fresh identifier, is assigned
    /// [`ProvenanceLevel::Imported`] provenance, has its reliability capped at
    /// `0.6`, and is marked with a stale embedding flag.  Identity fields are
    /// cleared and the scope is set to the store's own scope.
    ///
    /// Returns the newly assigned [`MemoryId`] values.
    pub fn import_shared(&self, memories: &[Memory]) -> Result<Vec<MemoryId>, StoreError> {
        Ok(self.import_shared_with_report(memories)?.new_ids)
    }
}

#[async_trait]
impl MemoryStore for SqliteMemoryStore {
    fn scope(&self) -> MemoryScope {
        self.scope
    }

    async fn store(&self, memory: Memory) -> Result<MemoryId, StoreError> {
        let should_attempt_embedding =
            self.embedding_provider.is_some() && !memory.content.trim().is_empty();
        let content_sha256 = should_attempt_embedding.then(|| content_sha256(&memory.content));
        let mut memory = memory;
        if should_attempt_embedding {
            memory.embedding_stale = true;
        }

        validate_memory_for_store(&memory, self.scope)?;

        let id = self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            transaction.execute(
                r#"
                INSERT INTO memories(
                    id,
                    content,
                    summary,
                    scope,
                    memory_type,
                    provenance,
                    importance_score,
                    reliability_score,
                    sensitivity,
                    state,
                    tags,
                    status,
                    custom_metadata,
                    access_count,
                    corroboration_count,
                    embedding_stale,
                    created_at,
                    updated_at,
                    last_accessed_at,
                    tenant_id,
                    user_id,
                    agent_id
                )
                VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                    ?18, ?19, ?20, ?21, ?22
                )
                "#,
                rusqlite::params_from_iter(memory_insert_params(&memory)?),
            )?;

            let row_id = require_memory_rowid(&transaction, &memory.id)?;
            sync_fts_entry(&transaction, row_id, None, &memory)?;
            transaction.commit()?;

            Ok(memory.id)
        })?;

        if !should_attempt_embedding {
            return Ok(id);
        }

        if let Some(content_sha256) = content_sha256.as_deref() {
            if self.reuse_cached_embedding(&id, content_sha256).await? {
                return Ok(id);
            }
        }

        let embedding: Vec<f32> = match self
            .generate_embedding(&memory.content, EmbeddingTask::Document)
            .await
        {
            Ok(Some(embedding)) => embedding,
            Ok(None) => return Ok(id),
            Err(error) => {
                if let Some(warning) = embedding_degradation_warning(&error) {
                    eprintln!("warning: {warning}");
                }
                return Ok(id);
            }
        };

        match self.store_embedding(&id, &embedding).await {
            Ok(()) | Err(StoreError::Validation(_)) => Ok(id),
            Err(error) => Err(error),
        }
    }

    async fn update_content(
        &self,
        id: &MemoryId,
        new_content: &str,
        changed_by: &str,
        reason: &str,
    ) -> Result<(), StoreError> {
        let trimmed_content = new_content.trim();
        if trimmed_content.is_empty() {
            return Err(StoreError::Validation(
                "memory content must not be empty".to_string(),
            ));
        }
        if changed_by.trim().is_empty() {
            return Err(StoreError::Validation(
                "changed_by must not be empty".to_string(),
            ));
        }

        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let mut memory = require_memory(&transaction, id)?.ok_or(StoreError::NotFound(*id))?;
            let previous_memory = memory.clone();

            if memory.content == trimmed_content {
                return Ok(());
            }

            let row_id = require_memory_rowid(&transaction, id)?;
            let next_version_number = load_next_version_number(&transaction, id)?;
            let changed_at = Utc::now();

            transaction.execute(
                r#"
                INSERT INTO memory_versions(
                    id,
                    memory_id,
                    version_number,
                    content,
                    changed_at,
                    changed_by,
                    change_reason
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    Uuid::new_v4().to_string(),
                    id.to_string(),
                    i64::from(next_version_number),
                    memory.content,
                    format_timestamp(changed_at),
                    changed_by.trim(),
                    reason,
                ],
            )?;

            memory.content = trimmed_content.to_string();
            memory.embedding_stale = true;
            memory.updated_at = changed_at;

            persist_memory(&transaction, &memory)?;
            sync_fts_entry(&transaction, row_id, Some(&previous_memory), &memory)?;
            transaction.commit()?;

            Ok(())
        })
    }

    async fn update_metadata(
        &self,
        id: &MemoryId,
        updates: MetadataUpdate,
    ) -> Result<(), StoreError> {
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let mut memory = require_memory(&transaction, id)?.ok_or(StoreError::NotFound(*id))?;
            let row_id = require_memory_rowid(&transaction, id)?;
            let previous_memory = memory.clone();
            let mut changed = false;

            if let Some(tags) = updates.tags {
                memory.tags = tags;
                changed = true;
            }

            if let Some(status) = updates.status {
                memory.status = match status {
                    OptionalFieldUpdate::Set(value) => Some(value),
                    OptionalFieldUpdate::Clear => None,
                };
                changed = true;
            }

            if let Some(custom_metadata) = updates.custom_metadata {
                memory.custom_metadata = custom_metadata;
                changed = true;
            }

            if let Some(importance_score) = updates.importance_score {
                validate_unit_interval("importance_score", importance_score)?;
                memory.importance_score = importance_score;
                changed = true;
            }

            if let Some(reliability_score) = updates.reliability_score {
                validate_unit_interval("reliability_score", reliability_score)?;
                memory.reliability_score = reliability_score;
                changed = true;
            }

            if let Some(state) = updates.state {
                memory.state = state;
                changed = true;
            }

            if !changed {
                return Ok(());
            }

            memory.updated_at = Utc::now();
            persist_memory(&transaction, &memory)?;
            sync_fts_entry(&transaction, row_id, Some(&previous_memory), &memory)?;
            transaction.commit()?;

            Ok(())
        })
    }

    async fn get(&self, id: &MemoryId) -> Result<Option<Memory>, StoreError> {
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let memory = require_memory(&transaction, id)?;

            let Some(mut memory) = memory else {
                return Ok(None);
            };

            memory.access_count = memory
                .access_count
                .checked_add(1)
                .ok_or(StoreError::Validation("access_count overflow".to_string()))?;
            memory.last_accessed_at = Some(Utc::now());

            transaction.execute(
                r#"
                UPDATE memories
                SET access_count = ?2,
                    last_accessed_at = ?3
                WHERE id = ?1
                "#,
                params![
                    id.to_string(),
                    i64::from(memory.access_count),
                    memory.last_accessed_at.map(format_timestamp),
                ],
            )?;

            transaction.commit()?;
            Ok(Some(memory))
        })
    }

    async fn get_raw(&self, id: &MemoryId) -> Result<Option<Memory>, StoreError> {
        self.with_connection(|connection| require_memory(connection, id))
    }

    async fn list(&self, filter: MemoryFilter) -> Result<Vec<Memory>, StoreError> {
        if matches!(filter.limit, Some(0)) {
            return Ok(Vec::new());
        }
        let list_scope = filter.scope.unwrap_or(self.scope);

        self.with_connection(|connection| {
            let mut sql = format!("SELECT {MEMORY_SELECT_COLUMNS} FROM memories WHERE scope = ?1");
            let mut params: Vec<rusqlite::types::Value> = vec![rusqlite::types::Value::from(
                scope_to_db(list_scope).to_string(),
            )];

            if let Some(state) = filter.state {
                sql.push_str(" AND state = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(rusqlite::types::Value::from(state_to_db(state).to_string()));
            }

            if let Some(status) = &filter.status {
                sql.push_str(" AND status = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(rusqlite::types::Value::from(status.clone()));
            }

            if let Some(tenant_id) = &filter.tenant_id {
                sql.push_str(" AND tenant_id = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(rusqlite::types::Value::from(tenant_id.clone()));
            }

            if let Some(user_id) = &filter.user_id {
                sql.push_str(" AND user_id = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(rusqlite::types::Value::from(user_id.clone()));
            }

            if let Some(agent_id) = &filter.agent_id {
                sql.push_str(" AND agent_id = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(rusqlite::types::Value::from(agent_id.clone()));
            }

            sql.push_str(" ORDER BY created_at ASC");

            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(rusqlite::params_from_iter(params), map_memory_row)?;

            let mut memories = Vec::new();
            for row in rows {
                let memory = row?;
                if !matches_filter(&memory, &filter) {
                    continue;
                }
                memories.push(memory);
                if filter.limit.is_some_and(|limit| memories.len() >= limit) {
                    break;
                }
            }

            Ok(memories)
        })
    }

    async fn search(&self, query: SearchQuery) -> Result<Vec<ScoredMemory>, StoreError> {
        let trimmed_text = query.text.trim().to_string();
        if query.max_results == 0 {
            return Ok(Vec::new());
        }
        if trimmed_text.is_empty() && query.embedding.is_none() {
            return Err(StoreError::Validation(
                "search requires non-empty text or a query embedding".to_string(),
            ));
        }

        let requested_state = query.state_filter.unwrap_or(MemoryState::Active);
        if requested_state == MemoryState::Deleted {
            return Ok(Vec::new());
        }

        let derived_query_embedding = if query.embedding.is_none() {
            self.generate_embedding(&trimmed_text, EmbeddingTask::Query)
                .await
                .unwrap_or_default()
        } else {
            None
        };
        let scoring_mode = RetrievalScoringMode::from_env();
        let explain_retrieval_scoring = explain_retrieval_scoring_enabled();

        self.with_connection(|connection| {
            let scope_config = load_scope_config(connection)?;
            let ranked_candidates = rank_search_candidates(
                connection,
                &query,
                &trimmed_text,
                derived_query_embedding.as_deref(),
                scoring_mode,
            )?;
            if explain_retrieval_scoring {
                emit_search_score_explanations(
                    &trimmed_text,
                    scoring_mode,
                    &ranked_candidates,
                    query.max_results,
                );
            }

            let mut results = ranked_candidates
                .into_iter()
                .map(|(scored_memory, _)| scored_memory)
                .collect::<Vec<_>>();
            results.truncate(query.max_results);
            let mut results = trim_results_to_context_budget(
                results,
                query.context_config.as_ref(),
                &scope_config,
            );
            touch_scored_memories(connection, &mut results)?;
            auto_promote_scored_memories(
                connection,
                &mut results,
                query.session_id.as_deref(),
                &scope_config,
            )?;
            Ok(results)
        })
    }

    async fn find_similar(
        &self,
        embedding: &[f32],
        threshold: f32,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>, StoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        validate_similarity_threshold(threshold)?;

        self.with_connection(|connection| {
            let expected_dimensions = load_embedding_dimensions(connection)?;
            validate_query_embedding(embedding, expected_dimensions)?;
            let visible_scopes = self.scope.visible_scopes();

            let similarity_scores = load_vector_similarity_scores(
                connection,
                visible_scopes,
                MemoryState::Active,
                None,
                None,
                embedding,
                threshold,
            )?;
            if similarity_scores.is_empty() {
                return Ok(Vec::new());
            }

            let memories_by_id: HashMap<MemoryId, Memory> =
                load_search_memories(connection, visible_scopes, MemoryState::Active, None, None)?
                    .into_iter()
                    .filter_map(|memory| {
                        similarity_scores
                            .get(&memory.id)
                            .copied()
                            .map(|_| (memory.id, memory))
                    })
                    .collect();

            let mut results = similarity_scores
                .into_iter()
                .filter_map(|(id, similarity)| {
                    memories_by_id.get(&id).cloned().map(|memory| ScoredMemory {
                        memory,
                        score: similarity,
                        similarity,
                    })
                })
                .collect::<Vec<_>>();

            results.sort_by(|left, right| {
                right
                    .similarity
                    .total_cmp(&left.similarity)
                    .then_with(|| right.memory.updated_at.cmp(&left.memory.updated_at))
                    .then_with(|| right.memory.id.cmp(&left.memory.id))
            });
            results.truncate(limit);
            Ok(results)
        })
    }

    async fn store_embedding(&self, id: &MemoryId, embedding: &[f32]) -> Result<(), StoreError> {
        if embedding.is_empty() {
            return Err(StoreError::Validation(
                "embedding vector must not be empty".to_string(),
            ));
        }

        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let memory = require_memory(&transaction, id)?.ok_or(StoreError::NotFound(*id))?;

            let expected_dimensions = load_embedding_dimensions(&transaction)?;
            if embedding.len() != expected_dimensions {
                return Err(StoreError::Validation(format!(
                    "embedding dimension mismatch: expected {expected_dimensions}, got {}",
                    embedding.len()
                )));
            }

            let content_sha256 = content_sha256(&memory.content);
            let encoded_embedding = encode_embedding(embedding);
            upsert_encoded_embedding(&transaction, id, &encoded_embedding, &content_sha256)?;
            transaction.commit()?;

            Ok(())
        })
    }

    async fn get_stale_embeddings(&self, limit: usize) -> Result<Vec<MemoryId>, StoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                r#"
                SELECT id
                FROM memories
                WHERE scope = ?1
                  AND embedding_stale = 1
                ORDER BY updated_at ASC, created_at ASC
                LIMIT ?2
                "#,
            )?;
            let rows = statement.query_map(
                params![
                    scope_to_db(self.scope),
                    i64::try_from(limit).unwrap_or(i64::MAX)
                ],
                |row| row.get::<_, String>(0),
            )?;

            let mut ids = Vec::new();
            for row in rows {
                let raw_id = row?;
                ids.push(parse_uuid(&raw_id)?);
            }

            Ok(ids)
        })
    }

    async fn make_dormant(&self, id: &MemoryId) -> Result<(), StoreError> {
        transition_state(self, id, MemoryState::Dormant).await
    }

    async fn reactivate(&self, id: &MemoryId) -> Result<(), StoreError> {
        transition_state(self, id, MemoryState::Active).await
    }

    async fn hard_delete(&self, id: &MemoryId) -> Result<(), StoreError> {
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let memory = require_memory(&transaction, id)?.ok_or(StoreError::NotFound(*id))?;
            let row_id = require_memory_rowid(&transaction, id)?;
            let vec_rowid: Option<i64> = transaction
                .query_row(
                    "SELECT vec_rowid FROM memory_embeddings WHERE memory_id = ?1",
                    [id.to_string()],
                    |row| row.get(0),
                )
                .optional()?;

            delete_fts_entry(&transaction, row_id, &memory)?;

            if let Some(vec_rowid) = vec_rowid {
                transaction.execute("DELETE FROM vec_memories WHERE rowid = ?1", [vec_rowid])?;
            }

            let deleted_rows = transaction.execute(
                "DELETE FROM memories WHERE id = ?1",
                [memory.id.to_string()],
            )?;
            if deleted_rows == 0 {
                return Err(StoreError::NotFound(*id));
            }

            transaction.commit()?;
            Ok(())
        })
    }

    async fn purge_user(&self, user_id: &str) -> Result<PurgeReport, StoreError> {
        let user_id = user_id.trim().to_string();
        if user_id.is_empty() {
            return Err(StoreError::Validation(
                "user_id must not be empty".to_string(),
            ));
        }

        self.with_connection(|connection| {
            let transaction = connection.transaction()?;

            // Count affected rows before deletion
            let memories_deleted: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM memories WHERE user_id = ?1",
                params![user_id],
                |row| row.get(0),
            )?;
            let versions_deleted: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM memory_versions WHERE memory_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
                |row| row.get(0),
            )?;
            let links_deleted: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM memory_links WHERE source_id IN (SELECT id FROM memories WHERE user_id = ?1) OR target_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
                |row| row.get(0),
            )?;
            let contradictions_deleted: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM contradictions WHERE memory_a_id IN (SELECT id FROM memories WHERE user_id = ?1) OR memory_b_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
                |row| row.get(0),
            )?;
            let embeddings_deleted: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM memory_embeddings WHERE memory_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
                |row| row.get(0),
            )?;

            // Delete in dependency order: dependents first, then memories
            transaction.execute(
                "DELETE FROM contradictions WHERE memory_a_id IN (SELECT id FROM memories WHERE user_id = ?1) OR memory_b_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
            )?;
            transaction.execute(
                "DELETE FROM memory_links WHERE source_id IN (SELECT id FROM memories WHERE user_id = ?1) OR target_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
            )?;
            transaction.execute(
                "DELETE FROM memory_versions WHERE memory_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
            )?;

            // Delete vec_memories rows BEFORE deleting memory_embeddings
            transaction.execute(
                "DELETE FROM vec_memories WHERE rowid IN (SELECT vec_rowid FROM memory_embeddings WHERE memory_id IN (SELECT id FROM memories WHERE user_id = ?1))",
                params![user_id],
            )?;
            transaction.execute(
                "DELETE FROM memory_embeddings WHERE memory_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
            )?;

            // Delete the memories themselves
            transaction.execute(
                "DELETE FROM memories WHERE user_id = ?1",
                params![user_id],
            )?;

            // Rebuild FTS index
            transaction.execute(
                "INSERT INTO memories_fts(memories_fts) VALUES('rebuild')",
                [],
            )?;

            transaction.commit()?;

            Ok(PurgeReport {
                memories_deleted: i64_to_u64(memories_deleted, "memories_deleted")?,
                versions_deleted: i64_to_u64(versions_deleted, "versions_deleted")?,
                links_deleted: i64_to_u64(links_deleted, "links_deleted")?,
                contradictions_deleted: i64_to_u64(
                    contradictions_deleted,
                    "contradictions_deleted",
                )?,
                embeddings_deleted: i64_to_u64(embeddings_deleted, "embeddings_deleted")?,
            })
        })
    }

    async fn purge_all(&self) -> Result<PurgeReport, StoreError> {
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let memories_deleted = count_table_rows(&transaction, "memories")?;
            let versions_deleted = count_table_rows(&transaction, "memory_versions")?;
            let links_deleted = count_table_rows(&transaction, "memory_links")?;
            let contradictions_deleted = count_table_rows(&transaction, "contradictions")?;
            let embeddings_deleted = count_table_rows(&transaction, "memory_embeddings")?;

            transaction.execute("DELETE FROM contradictions", [])?;
            transaction.execute("DELETE FROM memory_links", [])?;
            transaction.execute("DELETE FROM memory_versions", [])?;
            transaction.execute("DELETE FROM memory_embeddings", [])?;
            transaction.execute("DELETE FROM vec_memories", [])?;
            transaction.execute("DELETE FROM memories", [])?;
            transaction.execute(
                "INSERT INTO memories_fts(memories_fts) VALUES('rebuild')",
                [],
            )?;
            transaction.commit()?;

            Ok(PurgeReport {
                memories_deleted: i64_to_u64(memories_deleted, "memories_deleted")?,
                versions_deleted: i64_to_u64(versions_deleted, "versions_deleted")?,
                links_deleted: i64_to_u64(links_deleted, "links_deleted")?,
                contradictions_deleted: i64_to_u64(
                    contradictions_deleted,
                    "contradictions_deleted",
                )?,
                embeddings_deleted: i64_to_u64(embeddings_deleted, "embeddings_deleted")?,
            })
        })
    }

    async fn health_report(&self) -> Result<MemoryHealthReport, StoreError> {
        self.with_connection(|connection| {
            let active_count =
                count_memories_by_state(connection, self.scope, MemoryState::Active)?;
            let dormant_count =
                count_memories_by_state(connection, self.scope, MemoryState::Dormant)?;
            let stale_embeddings_count = connection.query_row(
                "SELECT COUNT(*) FROM memories WHERE scope = ?1 AND embedding_stale = 1",
                [scope_to_db(self.scope)],
                |row| row.get::<_, i64>(0),
            )?;
            let unresolved_contradictions = connection.query_row(
                "SELECT COUNT(*) FROM contradictions WHERE resolution_status = 'unresolved'",
                [],
                |row| row.get::<_, i64>(0),
            )?;
            let page_count =
                connection.query_row("PRAGMA page_count", [], |row| row.get::<_, i64>(0))?;
            let page_size =
                connection.query_row("PRAGMA page_size", [], |row| row.get::<_, i64>(0))?;
            let oldest_active_memory = connection
                .query_row(
                    "SELECT MIN(created_at) FROM memories WHERE scope = ?1 AND state = 'active'",
                    [scope_to_db(self.scope)],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()?
                .flatten()
                .map(|value| parse_datetime(&value))
                .transpose()?;
            let newest_memory = connection
                .query_row(
                    "SELECT MAX(created_at) FROM memories WHERE scope = ?1",
                    [scope_to_db(self.scope)],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()?
                .flatten()
                .map(|value| parse_datetime(&value))
                .transpose()?;
            let last_consolidation = connection
                .query_row(
                    "SELECT value FROM scope_config WHERE key = 'last_consolidation_at'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .map(|value| parse_datetime(&value))
                .transpose()?;

            Ok(MemoryHealthReport {
                scope: self.scope,
                active_count: i64_to_u64(active_count, "active_count")?,
                dormant_count: i64_to_u64(dormant_count, "dormant_count")?,
                total_storage_bytes: i64_to_u64(page_count, "page_count")?
                    .saturating_mul(i64_to_u64(page_size, "page_size")?),
                budget_usage_ratio: compute_budget_usage_ratio(
                    connection,
                    self.scope,
                    active_count,
                )?,
                unresolved_contradictions: i64_to_u64(
                    unresolved_contradictions,
                    "unresolved_contradictions",
                )?,
                stale_embeddings_count: i64_to_u64(
                    stale_embeddings_count,
                    "stale_embeddings_count",
                )?,
                last_consolidation,
                oldest_active_memory,
                newest_memory,
            })
        })
    }

    async fn list_contradictions(
        &self,
        status: Option<ResolutionStatus>,
    ) -> Result<Vec<ContradictionEntry>, StoreError> {
        self.with_connection(|connection| {
            let mut sql = String::from(
                "SELECT c.id, c.memory_a_id, c.memory_b_id, c.detected_at, c.description, c.resolution_status, c.resolved_at, c.resolution_note \
                 FROM contradictions c \
                 JOIN memories a ON a.id = c.memory_a_id \
                 JOIN memories b ON b.id = c.memory_b_id \
                 WHERE ",
            );
            let visible_scopes = self.scope.visible_scopes();
            let mut params: Vec<rusqlite::types::Value> = Vec::new();
            sql.push('(');
            sql.push_str(&scope_in_clause("a.scope", visible_scopes, &mut params));
            sql.push_str(") AND (");
            sql.push_str(&scope_in_clause("b.scope", visible_scopes, &mut params));
            sql.push(')');
            if let Some(status) = status {
                sql.push_str(" AND c.resolution_status = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(rusqlite::types::Value::from(
                    resolution_status_to_db(status).to_string(),
                ));
            }
            sql.push_str(" ORDER BY c.detected_at DESC, c.id ASC");

            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(rusqlite::params_from_iter(params), map_contradiction_row)?;
            let mut contradictions = Vec::new();
            for row in rows {
                contradictions.push(row?);
            }
            Ok(contradictions)
        })
    }

    async fn record_contradiction(
        &self,
        a_id: &MemoryId,
        b_id: &MemoryId,
        description: &str,
    ) -> Result<(), StoreError> {
        if a_id == b_id {
            return Err(StoreError::Validation(
                "a contradiction requires two distinct memory ids".to_string(),
            ));
        }
        if description.trim().is_empty() {
            return Err(StoreError::Validation(
                "contradiction description must not be empty".to_string(),
            ));
        }

        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            record_contradiction_row(&transaction, a_id, b_id, description)?;
            transaction.commit()?;
            Ok(())
        })
    }

    async fn update_contradiction_status(
        &self,
        contradiction_id: &str,
        status: ResolutionStatus,
        note: Option<&str>,
    ) -> Result<(), StoreError> {
        let trimmed_id = contradiction_id.trim();
        if trimmed_id.is_empty() {
            return Err(StoreError::Validation(
                "contradiction id must not be empty".to_string(),
            ));
        }

        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let normalized_note = note.and_then(|value| {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then_some(trimmed)
            });
            let resolved_at = if status == ResolutionStatus::Unresolved {
                None
            } else {
                Some(format_timestamp(Utc::now()))
            };
            let updated_rows = transaction.execute(
                r#"
                UPDATE contradictions
                SET resolution_status = ?2,
                    resolved_at = ?3,
                    resolution_note = ?4
                WHERE id = ?1
                "#,
                params![
                    trimmed_id,
                    resolution_status_to_db(status),
                    resolved_at,
                    normalized_note,
                ],
            )?;
            if updated_rows == 0 {
                return Err(StoreError::Validation(format!(
                    "contradiction not found: {trimmed_id}"
                )));
            }

            transaction.commit()?;
            Ok(())
        })
    }
}

impl MemoryObservability for SqliteMemoryStore {
    /// Produce a health report for the given scope.
    ///
    /// Queries the backing store for active/dormant counts, stale embeddings,
    /// unresolved contradictions, storage size, and temporal bounds, then
    /// assembles a [`MemoryHealthReport`].
    fn health_report(&self, scope: MemoryScope) -> Result<MemoryHealthReport, ObservabilityError> {
        self.with_connection(|connection| {
            let active_count = count_memories_by_state(connection, scope, MemoryState::Active)?;
            let dormant_count = count_memories_by_state(connection, scope, MemoryState::Dormant)?;
            let stale_embeddings_count = connection.query_row(
                "SELECT COUNT(*) FROM memories WHERE scope = ?1 AND embedding_stale = 1",
                [scope_to_db(scope)],
                |row| row.get::<_, i64>(0),
            )?;
            let unresolved_contradictions = connection.query_row(
                "SELECT COUNT(*) FROM contradictions WHERE resolution_status = 'unresolved'",
                [],
                |row| row.get::<_, i64>(0),
            )?;
            let page_count =
                connection.query_row("PRAGMA page_count", [], |row| row.get::<_, i64>(0))?;
            let page_size =
                connection.query_row("PRAGMA page_size", [], |row| row.get::<_, i64>(0))?;
            let oldest_active_memory = connection
                .query_row(
                    "SELECT MIN(created_at) FROM memories WHERE scope = ?1 AND state = 'active'",
                    [scope_to_db(scope)],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()?
                .flatten()
                .map(|value| parse_datetime(&value))
                .transpose()?;
            let newest_memory = connection
                .query_row(
                    "SELECT MAX(created_at) FROM memories WHERE scope = ?1",
                    [scope_to_db(scope)],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()?
                .flatten()
                .map(|value| parse_datetime(&value))
                .transpose()?;
            let last_consolidation = connection
                .query_row(
                    "SELECT value FROM scope_config WHERE key = 'last_consolidation_at'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .map(|value| parse_datetime(&value))
                .transpose()?;

            Ok(MemoryHealthReport {
                scope,
                active_count: i64_to_u64(active_count, "active_count")?,
                dormant_count: i64_to_u64(dormant_count, "dormant_count")?,
                total_storage_bytes: i64_to_u64(page_count, "page_count")?
                    .saturating_mul(i64_to_u64(page_size, "page_size")?),
                budget_usage_ratio: compute_budget_usage_ratio(connection, scope, active_count)?,
                unresolved_contradictions: i64_to_u64(
                    unresolved_contradictions,
                    "unresolved_contradictions",
                )?,
                stale_embeddings_count: i64_to_u64(
                    stale_embeddings_count,
                    "stale_embeddings_count",
                )?,
                last_consolidation,
                oldest_active_memory,
                newest_memory,
            })
        })
        .map_err(ObservabilityError::Store)
    }

    /// List contradictions visible from the store's configured scope.
    ///
    /// Optionally filters by [`ResolutionStatus`].  Results are ordered by
    /// detection time descending, then by contradiction id ascending.
    fn list_contradictions(
        &self,
        status: Option<ResolutionStatus>,
    ) -> Result<Vec<ContradictionEntry>, ObservabilityError> {
        self.with_connection(|connection| {
            let mut sql = String::from(
                "SELECT c.id, c.memory_a_id, c.memory_b_id, c.detected_at, c.description, \
                 c.resolution_status, c.resolved_at, c.resolution_note \
                 FROM contradictions c \
                 JOIN memories a ON a.id = c.memory_a_id \
                 JOIN memories b ON b.id = c.memory_b_id \
                 WHERE ",
            );
            let visible_scopes = self.scope.visible_scopes();
            let mut params: Vec<rusqlite::types::Value> = Vec::new();
            sql.push('(');
            sql.push_str(&scope_in_clause("a.scope", visible_scopes, &mut params));
            sql.push_str(") AND (");
            sql.push_str(&scope_in_clause("b.scope", visible_scopes, &mut params));
            sql.push(')');
            if let Some(status) = status {
                sql.push_str(" AND c.resolution_status = ?");
                sql.push_str(&(params.len() + 1).to_string());
                params.push(rusqlite::types::Value::from(
                    resolution_status_to_db(status).to_string(),
                ));
            }
            sql.push_str(" ORDER BY c.detected_at DESC, c.id ASC");

            let mut statement = connection.prepare(&sql)?;
            let rows =
                statement.query_map(rusqlite::params_from_iter(params), map_contradiction_row)?;
            let mut contradictions = Vec::new();
            for row in rows {
                contradictions.push(row?);
            }
            Ok(contradictions)
        })
        .map_err(ObservabilityError::Store)
    }

    /// Export memories for the given scope in the requested format.
    ///
    /// Currently supports [`ExportFormat::Json`].  The SQLite portable export
    /// format is planned for a future work unit and returns an error.
    fn export_memories(
        &self,
        scope: MemoryScope,
        format: ExportFormat,
    ) -> Result<Vec<u8>, ObservabilityError> {
        match format {
            ExportFormat::Json => {
                let memories = self
                    .with_connection(|connection| {
                        load_search_memories(connection, &[scope], MemoryState::Active, None, None)
                    })
                    .map_err(ObservabilityError::Store)?;
                serde_json::to_vec(&memories).map_err(|error| {
                    ObservabilityError::Operation(format!(
                        "failed to serialize memories to JSON: {error}"
                    ))
                })
            }
            ExportFormat::Sqlite => {
                let (memories, links, versions) = self
                    .with_connection(|connection| load_export_payload(connection, scope))
                    .map_err(ObservabilityError::Store)?;

                let temp_path =
                    std::env::temp_dir().join(format!("elegy-export-{}.sqlite3", Uuid::new_v4()));
                let result = export_to_sqlite_file(&temp_path, &memories, &links, &versions);
                let _ = std::fs::remove_file(&temp_path);
                result.map_err(|error| {
                    ObservabilityError::Operation(format!("sqlite export failed: {error}"))
                })
            }
            ExportFormat::Elegy => {
                let (memories, links, versions) = self
                    .with_connection(|connection| load_export_payload(connection, scope))
                    .map_err(ObservabilityError::Store)?;

                let archive = ElegyArchive {
                    format_version: "1".to_string(),
                    exported_at: Utc::now(),
                    scope,
                    memories,
                    links,
                    versions,
                };
                serde_json::to_vec(&archive).map_err(|error| {
                    ObservabilityError::Operation(format!(
                        "failed to serialize .elegy archive: {error}"
                    ))
                })
            }
        }
    }

    /// Purge all data associated with a specific user.
    ///
    /// Delegates to the internal purge logic used by the async
    /// [`MemoryStore::purge_user`] implementation, performing the operation
    /// synchronously.
    fn purge_user(&self, user_id: &str) -> Result<PurgeReport, ObservabilityError> {
        let user_id = user_id.trim().to_string();
        if user_id.is_empty() {
            return Err(ObservabilityError::Operation(
                "user_id must not be empty".to_string(),
            ));
        }

        self.with_connection(|connection| {
            let transaction = connection.transaction()?;

            // Count affected rows before deletion
            let memories_deleted: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM memories WHERE user_id = ?1",
                params![user_id],
                |row| row.get(0),
            )?;
            let versions_deleted: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM memory_versions WHERE memory_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
                |row| row.get(0),
            )?;
            let links_deleted: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM memory_links WHERE source_id IN (SELECT id FROM memories WHERE user_id = ?1) OR target_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
                |row| row.get(0),
            )?;
            let contradictions_deleted: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM contradictions WHERE memory_a_id IN (SELECT id FROM memories WHERE user_id = ?1) OR memory_b_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
                |row| row.get(0),
            )?;
            let embeddings_deleted: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM memory_embeddings WHERE memory_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
                |row| row.get(0),
            )?;

            // Delete in dependency order: dependents first, then memories
            transaction.execute(
                "DELETE FROM contradictions WHERE memory_a_id IN (SELECT id FROM memories WHERE user_id = ?1) OR memory_b_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
            )?;
            transaction.execute(
                "DELETE FROM memory_links WHERE source_id IN (SELECT id FROM memories WHERE user_id = ?1) OR target_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
            )?;
            transaction.execute(
                "DELETE FROM memory_versions WHERE memory_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
            )?;

            // Delete vec_memories rows BEFORE deleting memory_embeddings
            transaction.execute(
                "DELETE FROM vec_memories WHERE rowid IN (SELECT vec_rowid FROM memory_embeddings WHERE memory_id IN (SELECT id FROM memories WHERE user_id = ?1))",
                params![user_id],
            )?;
            transaction.execute(
                "DELETE FROM memory_embeddings WHERE memory_id IN (SELECT id FROM memories WHERE user_id = ?1)",
                params![user_id],
            )?;

            // Delete the memories themselves
            transaction.execute(
                "DELETE FROM memories WHERE user_id = ?1",
                params![user_id],
            )?;

            // Rebuild FTS index
            transaction.execute(
                "INSERT INTO memories_fts(memories_fts) VALUES('rebuild')",
                [],
            )?;

            transaction.commit()?;

            Ok(PurgeReport {
                memories_deleted: i64_to_u64(memories_deleted, "memories_deleted")?,
                versions_deleted: i64_to_u64(versions_deleted, "versions_deleted")?,
                links_deleted: i64_to_u64(links_deleted, "links_deleted")?,
                contradictions_deleted: i64_to_u64(
                    contradictions_deleted,
                    "contradictions_deleted",
                )?,
                embeddings_deleted: i64_to_u64(embeddings_deleted, "embeddings_deleted")?,
            })
        })
        .map_err(ObservabilityError::Store)
    }

    /// Purge all data for a specific scope.
    ///
    /// Deletes every memory whose scope matches the given value, along with
    /// associated versions, links, contradictions, and embeddings.
    fn purge_scope(&self, scope: MemoryScope) -> Result<PurgeReport, ObservabilityError> {
        self.with_connection(|connection| {
            let transaction = connection.transaction()?;
            let scope_db = scope_to_db(scope);

            // Count affected rows before deletion
            let memories_deleted: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM memories WHERE scope = ?1",
                params![scope_db],
                |row| row.get(0),
            )?;
            let versions_deleted: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM memory_versions WHERE memory_id IN (SELECT id FROM memories WHERE scope = ?1)",
                params![scope_db],
                |row| row.get(0),
            )?;
            let links_deleted: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM memory_links WHERE source_id IN (SELECT id FROM memories WHERE scope = ?1) OR target_id IN (SELECT id FROM memories WHERE scope = ?1)",
                params![scope_db],
                |row| row.get(0),
            )?;
            let contradictions_deleted: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM contradictions WHERE memory_a_id IN (SELECT id FROM memories WHERE scope = ?1) OR memory_b_id IN (SELECT id FROM memories WHERE scope = ?1)",
                params![scope_db],
                |row| row.get(0),
            )?;
            let embeddings_deleted: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM memory_embeddings WHERE memory_id IN (SELECT id FROM memories WHERE scope = ?1)",
                params![scope_db],
                |row| row.get(0),
            )?;

            // Delete in dependency order: dependents first, then memories
            transaction.execute(
                "DELETE FROM contradictions WHERE memory_a_id IN (SELECT id FROM memories WHERE scope = ?1) OR memory_b_id IN (SELECT id FROM memories WHERE scope = ?1)",
                params![scope_db],
            )?;
            transaction.execute(
                "DELETE FROM memory_links WHERE source_id IN (SELECT id FROM memories WHERE scope = ?1) OR target_id IN (SELECT id FROM memories WHERE scope = ?1)",
                params![scope_db],
            )?;
            transaction.execute(
                "DELETE FROM memory_versions WHERE memory_id IN (SELECT id FROM memories WHERE scope = ?1)",
                params![scope_db],
            )?;

            // Delete vec_memories rows BEFORE deleting memory_embeddings
            transaction.execute(
                "DELETE FROM vec_memories WHERE rowid IN (SELECT vec_rowid FROM memory_embeddings WHERE memory_id IN (SELECT id FROM memories WHERE scope = ?1))",
                params![scope_db],
            )?;
            transaction.execute(
                "DELETE FROM memory_embeddings WHERE memory_id IN (SELECT id FROM memories WHERE scope = ?1)",
                params![scope_db],
            )?;

            // Delete the memories themselves
            transaction.execute(
                "DELETE FROM memories WHERE scope = ?1",
                params![scope_db],
            )?;

            // Rebuild FTS index
            transaction.execute(
                "INSERT INTO memories_fts(memories_fts) VALUES('rebuild')",
                [],
            )?;

            transaction.commit()?;

            Ok(PurgeReport {
                memories_deleted: i64_to_u64(memories_deleted, "memories_deleted")?,
                versions_deleted: i64_to_u64(versions_deleted, "versions_deleted")?,
                links_deleted: i64_to_u64(links_deleted, "links_deleted")?,
                contradictions_deleted: i64_to_u64(
                    contradictions_deleted,
                    "contradictions_deleted",
                )?,
                embeddings_deleted: i64_to_u64(embeddings_deleted, "embeddings_deleted")?,
            })
        })
        .map_err(ObservabilityError::Store)
    }
}

fn embedding_degradation_warning(error: &EmbeddingError) -> Option<String> {
    let EmbeddingError::Provider(message) = error else {
        return None;
    };

    provider_not_reachable_warning(message, "ollama not reachable at ", "Ollama")
        .or_else(|| provider_not_reachable_warning(message, "openai not reachable at ", "OpenAI"))
        .or_else(|| openai_degradation_warning(message))
}

fn provider_not_reachable_warning(
    message: &str,
    prefix: &str,
    display_name: &str,
) -> Option<String> {
    message
        .strip_prefix(prefix)
        .and_then(|remainder| remainder.split_once(": ").map(|(url, _)| url.trim()))
        .map(|url| {
            format!(
                "{display_name} not reachable at {url}, storing without embeddings. Run reembed later.",
            )
        })
}

fn openai_degradation_warning(message: &str) -> Option<String> {
    if let Some(remainder) = message.strip_prefix("openai returned ") {
        let (status, detail) = remainder
            .split_once(": ")
            .map_or((remainder.trim(), None), |(status, detail)| {
                (status.trim(), summarize_openai_error_detail(detail))
            });
        let context = detail.map_or_else(
            || status.to_string(),
            |detail| format!("{status}: {detail}"),
        );
        return Some(format!(
            "OpenAI embeddings unavailable ({context}), storing without embeddings. Run reembed later.",
        ));
    }

    if let Some(remainder) = message.strip_prefix("openai embeddings request returned ") {
        let status = remainder
            .split_once(": ")
            .map_or(remainder.trim(), |(status, _)| status.trim());
        return Some(format!(
            "OpenAI embeddings unavailable ({status}), storing without embeddings. Run reembed later.",
        ));
    }

    None
}

fn summarize_openai_error_detail(detail: &str) -> Option<&str> {
    let detail = detail.trim();
    if detail.is_empty() {
        return None;
    }

    let summary = detail.split(" (").next().unwrap_or(detail).trim();
    if summary.is_empty() {
        None
    } else {
        Some(summary)
    }
}

async fn transition_state(
    store: &SqliteMemoryStore,
    id: &MemoryId,
    target_state: MemoryState,
) -> Result<(), StoreError> {
    store.with_connection(|connection| {
        let transaction = connection.transaction()?;
        let mut memory = require_memory(&transaction, id)?.ok_or(StoreError::NotFound(*id))?;

        if memory.state == MemoryState::Deleted {
            return Err(StoreError::Validation(format!(
                "memory {id} is logically deleted and cannot transition to {}",
                state_to_db(target_state)
            )));
        }

        if memory.state == target_state {
            return Ok(());
        }

        memory.state = target_state;
        memory.updated_at = Utc::now();
        persist_memory(&transaction, &memory)?;
        transaction.commit()?;

        Ok(())
    })
}

fn validate_memory_for_store(
    memory: &Memory,
    expected_scope: MemoryScope,
) -> Result<(), StoreError> {
    if memory.scope != expected_scope {
        return Err(StoreError::Validation(format!(
            "memory scope {} does not match store scope {}",
            scope_to_db(memory.scope),
            scope_to_db(expected_scope)
        )));
    }
    if memory.content.trim().is_empty() {
        return Err(StoreError::Validation(
            "memory content must not be empty".to_string(),
        ));
    }
    validate_unit_interval("importance_score", memory.importance_score)?;
    validate_unit_interval("reliability_score", memory.reliability_score)?;
    Ok(())
}

fn validate_unit_interval(field: &str, value: f32) -> Result<(), StoreError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        return Ok(());
    }

    Err(StoreError::Validation(format!(
        "{field} must be a finite value in the inclusive range 0.0..=1.0"
    )))
}

fn memory_insert_params(memory: &Memory) -> Result<Vec<rusqlite::types::Value>, StoreError> {
    Ok(vec![
        rusqlite::types::Value::from(memory.id.to_string()),
        rusqlite::types::Value::from(memory.content.clone()),
        optional_string_value(memory.summary.clone()),
        rusqlite::types::Value::from(scope_to_db(memory.scope).to_string()),
        rusqlite::types::Value::from(memory_type_to_db(memory.memory_type).to_string()),
        rusqlite::types::Value::from(provenance_to_db(memory.provenance).to_string()),
        rusqlite::types::Value::from(f64::from(memory.importance_score)),
        rusqlite::types::Value::from(f64::from(memory.reliability_score)),
        rusqlite::types::Value::from(sensitivity_to_db(memory.sensitivity).to_string()),
        rusqlite::types::Value::from(state_to_db(memory.state).to_string()),
        rusqlite::types::Value::from(serialize_json(&memory.tags)?),
        optional_string_value(memory.status.clone()),
        rusqlite::types::Value::from(serialize_json(&memory.custom_metadata)?),
        rusqlite::types::Value::from(i64::from(memory.access_count)),
        rusqlite::types::Value::from(i64::from(memory.corroboration_count)),
        rusqlite::types::Value::from(i64::from(memory.embedding_stale as u8)),
        rusqlite::types::Value::from(format_timestamp(memory.created_at)),
        rusqlite::types::Value::from(format_timestamp(memory.updated_at)),
        optional_string_value(memory.last_accessed_at.map(format_timestamp)),
        optional_string_value(memory.tenant_id.clone()),
        optional_string_value(memory.user_id.clone()),
        optional_string_value(memory.agent_id.clone()),
    ])
}

fn optional_string_value(value: Option<String>) -> rusqlite::types::Value {
    match value {
        Some(value) => rusqlite::types::Value::from(value),
        None => rusqlite::types::Value::Null,
    }
}

fn persist_memory(connection: &Connection, memory: &Memory) -> Result<(), StoreError> {
    connection.execute(
        r#"
        UPDATE memories
        SET content = ?2,
            summary = ?3,
            scope = ?4,
            memory_type = ?5,
            provenance = ?6,
            importance_score = ?7,
            reliability_score = ?8,
            sensitivity = ?9,
            state = ?10,
            tags = ?11,
            status = ?12,
            custom_metadata = ?13,
            access_count = ?14,
            corroboration_count = ?15,
            embedding_stale = ?16,
            created_at = ?17,
            updated_at = ?18,
            last_accessed_at = ?19,
            tenant_id = ?20,
            user_id = ?21,
            agent_id = ?22
        WHERE id = ?1
        "#,
        rusqlite::params_from_iter(memory_insert_params(memory)?),
    )?;
    Ok(())
}

fn sync_fts_entry(
    connection: &Connection,
    row_id: i64,
    previous_memory: Option<&Memory>,
    memory: &Memory,
) -> Result<(), StoreError> {
    if let Some(previous_memory) = previous_memory {
        delete_fts_entry(connection, row_id, previous_memory)?;
    }
    let indexed_fields = indexed_fts_fields(memory);
    connection.execute(
        "INSERT INTO memories_fts(rowid, content, summary, tags) VALUES (?1, ?2, ?3, ?4)",
        params![
            row_id,
            indexed_fields.content,
            indexed_fields.summary.as_deref(),
            indexed_fields.tags
        ],
    )?;
    Ok(())
}

fn delete_fts_entry(
    connection: &Connection,
    row_id: i64,
    memory: &Memory,
) -> Result<(), StoreError> {
    let indexed_fields = indexed_fts_fields(memory);
    connection.execute(
        "INSERT INTO memories_fts(memories_fts, rowid, content, summary, tags) VALUES ('delete', ?1, ?2, ?3, ?4)",
        params![
            row_id,
            indexed_fields.content,
            indexed_fields.summary.as_deref(),
            indexed_fields.tags
        ],
    )?;
    Ok(())
}

struct IndexedFtsFields {
    content: String,
    summary: Option<String>,
    tags: String,
}

fn indexed_fts_fields(memory: &Memory) -> IndexedFtsFields {
    IndexedFtsFields {
        content: expand_compound_words(&memory.content),
        summary: memory.summary.as_deref().map(expand_compound_words),
        tags: indexed_tags(memory),
    }
}

fn indexed_tags(memory: &Memory) -> String {
    expand_compound_words(&memory.tags.join(" "))
}

fn expand_compound_words(text: &str) -> String {
    let mut expansions = Vec::new();
    let mut seen_expansions = HashSet::new();
    let mut token = String::new();

    for character in text.chars() {
        if character.is_alphanumeric() || character == '_' {
            token.push(character);
        } else {
            collect_compound_word_expansion(&token, &mut expansions, &mut seen_expansions);
            token.clear();
        }
    }

    collect_compound_word_expansion(&token, &mut expansions, &mut seen_expansions);

    if expansions.is_empty() {
        return text.to_string();
    }

    let expansion_length = expansions.iter().map(String::len).sum::<usize>();
    let mut expanded = String::with_capacity(text.len() + expansion_length + expansions.len());
    expanded.push_str(text);

    for expansion in expansions {
        expanded.push(' ');
        expanded.push_str(&expansion);
    }

    expanded
}

fn collect_compound_word_expansion(
    token: &str,
    expansions: &mut Vec<String>,
    seen_expansions: &mut HashSet<String>,
) {
    let Some(expansion) = split_compound_word(token) else {
        return;
    };

    if seen_expansions.insert(expansion.clone()) {
        expansions.push(expansion);
    }
}

fn split_compound_word(token: &str) -> Option<String> {
    let characters = token.chars().collect::<Vec<_>>();
    if characters.len() < 2 {
        return None;
    }

    let mut parts = Vec::new();
    let mut current_part = String::new();

    for (index, character) in characters.iter().copied().enumerate() {
        if index > 0 {
            let previous = characters[index - 1];
            let next = characters.get(index + 1).copied();
            let has_boundary = (previous.is_lowercase() && character.is_uppercase())
                || (previous.is_uppercase()
                    && character.is_uppercase()
                    && next.is_some_and(|next_character| next_character.is_lowercase()))
                || (previous.is_ascii_digit() && character.is_alphabetic())
                || (previous.is_alphabetic() && character.is_ascii_digit());

            if has_boundary && !current_part.is_empty() {
                parts.push(std::mem::take(&mut current_part));
            }
        }

        current_part.push(character);
    }

    if !current_part.is_empty() {
        parts.push(current_part);
    }

    if parts.len() > 1 {
        Some(parts.join(" "))
    } else {
        None
    }
}

fn require_memory_rowid(connection: &Connection, id: &MemoryId) -> Result<i64, StoreError> {
    connection
        .query_row(
            "SELECT rowid FROM memories WHERE id = ?1",
            [id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .ok_or(StoreError::NotFound(*id))
}

fn require_memory(connection: &Connection, id: &MemoryId) -> Result<Option<Memory>, StoreError> {
    connection
        .query_row(
            &format!("SELECT {MEMORY_SELECT_COLUMNS} FROM memories WHERE id = ?1"),
            [id.to_string()],
            map_memory_row,
        )
        .optional()
        .map_err(StoreError::from)
}

fn map_memory_row(row: &Row<'_>) -> rusqlite::Result<Memory> {
    let raw_id: String = row.get(0)?;
    let raw_scope: String = row.get(3)?;
    let raw_memory_type: String = row.get(4)?;
    let raw_provenance: String = row.get(5)?;
    let raw_sensitivity: String = row.get(8)?;
    let raw_state: String = row.get(9)?;
    let raw_tags: Option<String> = row.get(10)?;
    let raw_custom_metadata: Option<String> = row.get(12)?;
    let raw_access_count: i64 = row.get(13)?;
    let raw_corroboration_count: i64 = row.get(14)?;
    let raw_embedding_stale: i64 = row.get(15)?;
    let raw_created_at: String = row.get(16)?;
    let raw_updated_at: String = row.get(17)?;
    let raw_last_accessed_at: Option<String> = row.get(18)?;

    Ok(Memory {
        id: parse_uuid_for_sqlite(&raw_id)?,
        content: row.get(1)?,
        summary: row.get(2)?,
        scope: parse_scope_for_sqlite(&raw_scope)?,
        memory_type: parse_memory_type_for_sqlite(&raw_memory_type)?,
        provenance: parse_provenance_for_sqlite(&raw_provenance)?,
        importance_score: row.get(6)?,
        reliability_score: row.get(7)?,
        sensitivity: parse_sensitivity_for_sqlite(&raw_sensitivity)?,
        state: parse_state_for_sqlite(&raw_state)?,
        tags: parse_json_for_sqlite(raw_tags.as_deref().unwrap_or("[]"))?,
        status: row.get(11)?,
        custom_metadata: parse_json_for_sqlite(raw_custom_metadata.as_deref().unwrap_or("{}"))?,
        access_count: i64_to_u32_for_sqlite(raw_access_count, "access_count")?,
        corroboration_count: i64_to_u32_for_sqlite(raw_corroboration_count, "corroboration_count")?,
        embedding_stale: raw_embedding_stale != 0,
        created_at: parse_datetime_for_sqlite(&raw_created_at)?,
        updated_at: parse_datetime_for_sqlite(&raw_updated_at)?,
        last_accessed_at: raw_last_accessed_at
            .as_deref()
            .map(parse_datetime_for_sqlite)
            .transpose()?,
        tenant_id: row.get(19)?,
        user_id: row.get(20)?,
        agent_id: row.get(21)?,
    })
}

fn map_contradiction_row(row: &Row<'_>) -> rusqlite::Result<ContradictionEntry> {
    let raw_id: String = row.get(0)?;
    let raw_memory_a_id: String = row.get(1)?;
    let raw_memory_b_id: String = row.get(2)?;
    let raw_detected_at: String = row.get(3)?;
    let raw_resolution_status: String = row.get(5)?;
    let raw_resolved_at: Option<String> = row.get(6)?;

    Ok(ContradictionEntry {
        id: raw_id,
        memory_a_id: parse_uuid_for_sqlite(&raw_memory_a_id)?,
        memory_b_id: parse_uuid_for_sqlite(&raw_memory_b_id)?,
        detected_at: parse_datetime_for_sqlite(&raw_detected_at)?,
        description: row.get(4)?,
        resolution_status: parse_resolution_status_for_sqlite(&raw_resolution_status)?,
        resolved_at: raw_resolved_at
            .as_deref()
            .map(parse_datetime_for_sqlite)
            .transpose()?,
        resolution_note: row.get(7)?,
    })
}

fn map_memory_version_row(row: &Row<'_>) -> rusqlite::Result<MemoryVersion> {
    let raw_memory_id: String = row.get(1)?;
    let raw_changed_at: String = row.get(6)?;

    Ok(MemoryVersion {
        id: row.get(0)?,
        memory_id: parse_uuid_for_sqlite(&raw_memory_id)?,
        version_number: i64_to_u32_for_sqlite(row.get(2)?, "version_number")?,
        content: row.get(3)?,
        changed_by: row.get(4)?,
        change_reason: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
        changed_at: parse_datetime_for_sqlite(&raw_changed_at)?,
    })
}

fn map_correction_row(row: &Row<'_>) -> rusqlite::Result<CorrectionRecord> {
    let raw_memory_id: String = row.get(1)?;
    let raw_disposition: String = row.get(6)?;
    let raw_related_memory_id: Option<String> = row.get(7)?;
    let raw_corrected_at: String = row.get(8)?;

    Ok(CorrectionRecord {
        id: row.get(0)?,
        memory_id: parse_uuid_for_sqlite(&raw_memory_id)?,
        previous_content: row.get(2)?,
        corrected_content: row.get(3)?,
        corrected_by: row.get(4)?,
        reason: row.get(5)?,
        disposition: parse_correction_disposition_for_sqlite(&raw_disposition)?,
        related_memory_id: raw_related_memory_id
            .as_deref()
            .map(parse_uuid_for_sqlite)
            .transpose()?,
        corrected_at: parse_datetime_for_sqlite(&raw_corrected_at)?,
    })
}

fn parse_json<T>(raw: &str) -> Result<T, StoreError>
where
    T: DeserializeOwned,
{
    serde_json::from_str(raw).map_err(|error| {
        StoreError::Serialization(format!("failed to decode JSON `{raw}`: {error}"))
    })
}

fn parse_correction_disposition_for_sqlite(raw: &str) -> rusqlite::Result<CorrectionDisposition> {
    match raw {
        "applied" => Ok(CorrectionDisposition::Applied),
        "archived" => Ok(CorrectionDisposition::Archived),
        "merged" => Ok(CorrectionDisposition::Merged),
        "contradiction" => Ok(CorrectionDisposition::Contradiction),
        other => Err(rusqlite::Error::FromSqlConversionFailure(
            0,
            Type::Text,
            format!("invalid correction disposition `{other}`").into(),
        )),
    }
}

fn serialize_json<T>(value: &T) -> Result<String, StoreError>
where
    T: Serialize,
{
    serde_json::to_string(value)
        .map_err(|error| StoreError::Serialization(format!("failed to encode JSON: {error}")))
}

fn parse_json_for_sqlite<T>(raw: &str) -> rusqlite::Result<T>
where
    T: DeserializeOwned,
{
    parse_json(raw).map_err(sqlite_conversion_error)
}

fn count_table_rows(connection: &Connection, table: &str) -> Result<i64, StoreError> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    connection
        .query_row(&sql, [], |row| row.get::<_, i64>(0))
        .map_err(StoreError::from)
}

fn parse_uuid(raw: &str) -> Result<MemoryId, StoreError> {
    Uuid::parse_str(raw)
        .map_err(|error| StoreError::Serialization(format!("invalid memory id `{raw}`: {error}")))
}

fn parse_uuid_for_sqlite(raw: &str) -> rusqlite::Result<MemoryId> {
    parse_uuid(raw).map_err(sqlite_conversion_error)
}

fn parse_datetime(raw: &str) -> Result<DateTime<Utc>, StoreError> {
    DateTime::parse_from_rfc3339(raw)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| {
            StoreError::Serialization(format!("invalid RFC3339 timestamp `{raw}`: {error}"))
        })
}

fn parse_datetime_for_sqlite(raw: &str) -> rusqlite::Result<DateTime<Utc>> {
    parse_datetime(raw).map_err(sqlite_conversion_error)
}

fn i64_to_u32_for_sqlite(value: i64, field: &str) -> rusqlite::Result<u32> {
    u32::try_from(value).map_err(|_| {
        sqlite_conversion_error(StoreError::Serialization(format!(
            "{field} value `{value}` does not fit into u32"
        )))
    })
}

fn i64_to_u64(value: i64, field: &str) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| {
        StoreError::Serialization(format!("{field} value `{value}` does not fit into u64"))
    })
}

fn sqlite_conversion_error(error: StoreError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error))
}

fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339()
}

fn scope_to_db(scope: MemoryScope) -> &'static str {
    match scope {
        MemoryScope::Session => "session",
        MemoryScope::Workspace => "workspace",
        MemoryScope::User => "user",
        MemoryScope::Agent => "agent",
    }
}

fn parse_scope(raw: &str) -> Result<MemoryScope, StoreError> {
    match raw {
        "session" => Ok(MemoryScope::Session),
        "workspace" => Ok(MemoryScope::Workspace),
        "user" => Ok(MemoryScope::User),
        "agent" => Ok(MemoryScope::Agent),
        _ => Err(StoreError::Serialization(format!(
            "unknown memory scope `{raw}`"
        ))),
    }
}

fn parse_scope_for_sqlite(raw: &str) -> rusqlite::Result<MemoryScope> {
    parse_scope(raw).map_err(sqlite_conversion_error)
}

fn memory_type_to_db(memory_type: MemoryType) -> &'static str {
    match memory_type {
        MemoryType::Fact => "fact",
        MemoryType::Preference => "preference",
        MemoryType::Decision => "decision",
        MemoryType::Procedure => "procedure",
        MemoryType::Observation => "observation",
    }
}

fn parse_memory_type(raw: &str) -> Result<MemoryType, StoreError> {
    match raw {
        "fact" => Ok(MemoryType::Fact),
        "preference" => Ok(MemoryType::Preference),
        "decision" => Ok(MemoryType::Decision),
        "procedure" => Ok(MemoryType::Procedure),
        "observation" => Ok(MemoryType::Observation),
        _ => Err(StoreError::Serialization(format!(
            "unknown memory type `{raw}`"
        ))),
    }
}

fn parse_memory_type_for_sqlite(raw: &str) -> rusqlite::Result<MemoryType> {
    parse_memory_type(raw).map_err(sqlite_conversion_error)
}

fn provenance_to_db(provenance: ProvenanceLevel) -> &'static str {
    match provenance {
        ProvenanceLevel::UserStated => "user_stated",
        ProvenanceLevel::AgentObserved => "agent_observed",
        ProvenanceLevel::Consolidated => "consolidated",
        ProvenanceLevel::Imported => "imported",
        ProvenanceLevel::AgentInferred => "agent_inferred",
    }
}

fn parse_provenance(raw: &str) -> Result<ProvenanceLevel, StoreError> {
    match raw {
        "user_stated" => Ok(ProvenanceLevel::UserStated),
        "agent_observed" => Ok(ProvenanceLevel::AgentObserved),
        "consolidated" => Ok(ProvenanceLevel::Consolidated),
        "imported" => Ok(ProvenanceLevel::Imported),
        "agent_inferred" => Ok(ProvenanceLevel::AgentInferred),
        _ => Err(StoreError::Serialization(format!(
            "unknown provenance level `{raw}`"
        ))),
    }
}

fn parse_provenance_for_sqlite(raw: &str) -> rusqlite::Result<ProvenanceLevel> {
    parse_provenance(raw).map_err(sqlite_conversion_error)
}

fn sensitivity_to_db(sensitivity: SensitivityLevel) -> &'static str {
    match sensitivity {
        SensitivityLevel::Low => "low",
        SensitivityLevel::Medium => "medium",
        SensitivityLevel::High => "high",
        SensitivityLevel::Critical => "critical",
    }
}

fn parse_sensitivity(raw: &str) -> Result<SensitivityLevel, StoreError> {
    match raw {
        "low" => Ok(SensitivityLevel::Low),
        "medium" => Ok(SensitivityLevel::Medium),
        "high" => Ok(SensitivityLevel::High),
        "critical" => Ok(SensitivityLevel::Critical),
        _ => Err(StoreError::Serialization(format!(
            "unknown sensitivity level `{raw}`"
        ))),
    }
}

fn parse_sensitivity_for_sqlite(raw: &str) -> rusqlite::Result<SensitivityLevel> {
    parse_sensitivity(raw).map_err(sqlite_conversion_error)
}

fn state_to_db(state: MemoryState) -> &'static str {
    match state {
        MemoryState::Active => "active",
        MemoryState::Dormant => "dormant",
        MemoryState::Deleted => "deleted",
    }
}

fn parse_state(raw: &str) -> Result<MemoryState, StoreError> {
    match raw {
        "active" => Ok(MemoryState::Active),
        "dormant" => Ok(MemoryState::Dormant),
        "deleted" => Ok(MemoryState::Deleted),
        _ => Err(StoreError::Serialization(format!(
            "unknown memory state `{raw}`"
        ))),
    }
}

fn parse_state_for_sqlite(raw: &str) -> rusqlite::Result<MemoryState> {
    parse_state(raw).map_err(sqlite_conversion_error)
}

fn resolution_status_to_db(status: ResolutionStatus) -> &'static str {
    match status {
        ResolutionStatus::Unresolved => "unresolved",
        ResolutionStatus::ResolvedByUser => "resolved_by_user",
        ResolutionStatus::ResolvedBySystem => "resolved_by_system",
        ResolutionStatus::Dismissed => "dismissed",
    }
}

fn parse_resolution_status(raw: &str) -> Result<ResolutionStatus, StoreError> {
    match raw {
        "unresolved" => Ok(ResolutionStatus::Unresolved),
        "resolved_by_user" => Ok(ResolutionStatus::ResolvedByUser),
        "resolved_by_system" => Ok(ResolutionStatus::ResolvedBySystem),
        "dismissed" => Ok(ResolutionStatus::Dismissed),
        _ => Err(StoreError::Serialization(format!(
            "unknown resolution status `{raw}`"
        ))),
    }
}

fn parse_resolution_status_for_sqlite(raw: &str) -> rusqlite::Result<ResolutionStatus> {
    parse_resolution_status(raw).map_err(sqlite_conversion_error)
}

fn scope_in_clause(
    column_name: &str,
    scopes: &[MemoryScope],
    params: &mut Vec<rusqlite::types::Value>,
) -> String {
    let mut clause = String::new();
    clause.push_str(column_name);
    clause.push_str(" IN (");
    for (index, scope) in scopes.iter().enumerate() {
        if index > 0 {
            clause.push_str(", ");
        }
        clause.push('?');
        clause.push_str(&(params.len() + 1).to_string());
        params.push(rusqlite::types::Value::from(
            scope_to_db(*scope).to_string(),
        ));
    }
    clause.push(')');
    clause
}

fn auto_promote_scored_memories(
    connection: &mut Connection,
    results: &mut [ScoredMemory],
    session_id: Option<&str>,
    scope_config: &ScopeConfig,
) -> Result<(), StoreError> {
    if results.is_empty() {
        return Ok(());
    }

    if let Some(session_id) = session_id {
        validate_session_id(session_id)?;
        record_session_accesses(
            connection,
            results.iter().map(|result| result.memory.id),
            session_id,
        )?;
    }

    for result in results {
        let Some(to_scope) =
            promotion_target(connection, &result.memory, scope_config, session_id)?
        else {
            continue;
        };
        let transaction = connection.transaction()?;
        let Some(mut latest) = require_memory(&transaction, &result.memory.id)? else {
            continue;
        };
        record_promotion(
            &transaction,
            &mut latest,
            to_scope,
            "automatic promotion during search",
            "system:promotion",
            session_id,
            scope_config,
        )?;
        transaction.commit()?;
        result.memory = latest;
    }

    Ok(())
}

fn validate_session_id(session_id: &str) -> Result<(), StoreError> {
    Uuid::parse_str(session_id).map(|_| ()).map_err(|error| {
        StoreError::Validation(format!("invalid session_id `{session_id}`: {error}"))
    })
}

fn record_session_accesses(
    connection: &Connection,
    memory_ids: impl IntoIterator<Item = MemoryId>,
    session_id: &str,
) -> Result<(), StoreError> {
    let now = format_timestamp(Utc::now());
    for memory_id in memory_ids {
        connection.execute(
            r#"
            INSERT INTO memory_session_accesses(memory_id, session_id, first_accessed_at, last_accessed_at)
            VALUES (?1, ?2, ?3, ?3)
            ON CONFLICT(memory_id, session_id) DO UPDATE
            SET last_accessed_at = excluded.last_accessed_at
            "#,
            params![memory_id.to_string(), session_id, now],
        )?;
    }
    Ok(())
}

fn promotion_target(
    connection: &Connection,
    memory: &Memory,
    scope_config: &ScopeConfig,
    _trigger_session_id: Option<&str>,
) -> Result<Option<MemoryScope>, StoreError> {
    let Some(next_scope) = memory.scope.next() else {
        return Ok(None);
    };

    if memory.scope == MemoryScope::Session {
        let distinct_sessions = connection.query_row(
            "SELECT COUNT(DISTINCT session_id) FROM memory_session_accesses WHERE memory_id = ?1",
            [memory.id.to_string()],
            |row| row.get::<_, i64>(0),
        )?;
        if distinct_sessions >= 3 {
            return Ok(Some(MemoryScope::Workspace));
        }
    }

    if memory.corroboration_count >= 2 {
        return Ok(Some(next_scope));
    }

    let age_days = (Utc::now() - memory.updated_at).num_days();
    if age_days >= 7
        && (memory.importance_score as f64) * decay::retention(memory, Utc::now(), scope_config)
            >= 0.4
    {
        return Ok(Some(next_scope));
    }

    Ok(None)
}

fn record_promotion(
    connection: &Connection,
    memory: &mut Memory,
    to_scope: MemoryScope,
    reason: &str,
    changed_by: &str,
    trigger_session_id: Option<&str>,
    _scope_config: &ScopeConfig,
) -> Result<(), StoreError> {
    let from_scope = memory.scope;
    if from_scope == to_scope {
        return Ok(());
    }

    let previous_memory = memory.clone();
    let now = Utc::now();
    let next_version = load_next_version_number(connection, &memory.id)?;
    connection.execute(
        r#"
        INSERT INTO memory_versions(id, memory_id, version_number, content, changed_at, changed_by, change_reason)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        params![
            Uuid::new_v4().to_string(),
            memory.id.to_string(),
            i64::from(next_version),
            memory.content.clone(),
            format_timestamp(now),
            changed_by,
            format!("scope promotion: {} -> {} ({reason})", scope_to_db(from_scope), scope_to_db(to_scope)),
        ],
    )?;
    connection.execute(
        r#"
        INSERT INTO memory_promotions(id, memory_id, from_scope, to_scope, reason, trigger_session_id, promoted_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        params![
            Uuid::new_v4().to_string(),
            memory.id.to_string(),
            scope_to_db(from_scope),
            scope_to_db(to_scope),
            reason,
            trigger_session_id,
            format_timestamp(now),
        ],
    )?;

    memory.scope = to_scope;
    memory.updated_at = now;
    persist_memory(connection, memory)?;
    let row_id = require_memory_rowid(connection, &memory.id)?;
    sync_fts_entry(connection, row_id, Some(&previous_memory), memory)?;
    Ok(())
}

fn matches_filter(memory: &Memory, filter: &MemoryFilter) -> bool {
    if let Some(memory_types) = &filter.memory_types {
        if !memory_types.contains(&memory.memory_type) {
            return false;
        }
    }

    if let Some(provenance_levels) = &filter.provenance_levels {
        if !provenance_levels.contains(&memory.provenance) {
            return false;
        }
    }

    if let Some(tags) = &filter.tags {
        if !tags
            .iter()
            .all(|tag| memory.tags.iter().any(|existing| existing == tag))
        {
            return false;
        }
    }

    true
}

fn load_next_version_number(connection: &Connection, id: &MemoryId) -> Result<u32, StoreError> {
    let max_version: Option<i64> = connection.query_row(
        "SELECT MAX(version_number) FROM memory_versions WHERE memory_id = ?1",
        [id.to_string()],
        |row| row.get(0),
    )?;

    let next = max_version.unwrap_or(0) + 1;
    u32::try_from(next).map_err(|_| {
        StoreError::Serialization(format!(
            "next version number `{next}` does not fit into u32"
        ))
    })
}

fn load_embedding_dimensions(connection: &Connection) -> Result<usize, StoreError> {
    let configured_value = connection
        .query_row(
            "SELECT value FROM scope_config WHERE key = 'embedding_dimensions'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    match configured_value {
        Some(value) => value.parse::<usize>().map_err(|error| {
            StoreError::Serialization(format!(
                "invalid embedding_dimensions config `{value}`: {error}"
            ))
        }),
        None => Ok(DEFAULT_EMBEDDING_DIMENSIONS),
    }
}

fn load_cached_embedding_blob(
    connection: &Connection,
    scope: MemoryScope,
    content_sha256: &str,
) -> Result<Option<Vec<u8>>, StoreError> {
    connection
        .query_row(
            r#"
            SELECT v.embedding
            FROM memory_embeddings me
            JOIN memories m ON m.id = me.memory_id
            JOIN vec_memories v ON v.rowid = me.vec_rowid
            WHERE me.content_sha256 = ?1
              AND m.scope = ?2
              AND m.embedding_stale = 0
            ORDER BY m.updated_at DESC, m.created_at DESC
            LIMIT 1
            "#,
            params![content_sha256, scope_to_db(scope)],
            |row| row.get(0),
        )
        .optional()
        .map_err(StoreError::from)
}

fn load_stored_embedding(
    connection: &Connection,
    id: &MemoryId,
    expected_dimensions: usize,
) -> Result<Option<Vec<f32>>, StoreError> {
    let encoded: Option<Vec<u8>> = connection
        .query_row(
            r#"
            SELECT v.embedding
            FROM memory_embeddings me
            JOIN vec_memories v ON v.rowid = me.vec_rowid
            WHERE me.memory_id = ?1
            "#,
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    encoded
        .map(|bytes| decode_embedding(&bytes, expected_dimensions))
        .transpose()
}

fn load_scope_config(connection: &Connection) -> Result<ScopeConfig, StoreError> {
    let defaults = ScopeConfig::default();
    Ok(ScopeConfig {
        similarity_weight: load_f32_config(
            connection,
            "similarity_weight",
            defaults.similarity_weight,
        )?,
        recency_weight: load_f32_config(connection, "recency_weight", defaults.recency_weight)?,
        access_weight: load_f32_config(connection, "access_weight", defaults.access_weight)?,
        priority_weight: load_f32_config(connection, "priority_weight", defaults.priority_weight)?,
        memory_context_ratio: load_f32_config(
            connection,
            "memory_context_ratio",
            defaults.memory_context_ratio,
        )?,
        decay_lambda_base: load_f32_config(
            connection,
            "decay_lambda_base",
            defaults.decay_lambda_base,
        )?,
        response_reserve: load_u32_config(
            connection,
            "response_reserve",
            defaults.response_reserve,
        )?,
        salience_threshold: load_f32_config(
            connection,
            "salience_threshold",
            defaults.salience_threshold,
        )?,
        novelty_doubt_threshold: load_f32_config(
            connection,
            "novelty_doubt_threshold",
            defaults.novelty_doubt_threshold,
        )?,
        merge_similarity_threshold: load_f32_config(
            connection,
            "merge_similarity_threshold",
            defaults.merge_similarity_threshold,
        )?,
        duplicate_similarity_threshold: load_f32_config(
            connection,
            "duplicate_similarity_threshold",
            defaults.duplicate_similarity_threshold,
        )?,
        agent_inferred_importance_threshold: load_f32_config(
            connection,
            "agent_inferred_importance_threshold",
            defaults.agent_inferred_importance_threshold,
        )?,
    })
}

fn load_f32_config(connection: &Connection, key: &str, default: f32) -> Result<f32, StoreError> {
    let raw_value = load_config_value(connection, key)?;
    match raw_value {
        Some(raw_value) => raw_value.parse::<f32>().map_err(|error| {
            StoreError::Serialization(format!("invalid {key} config `{raw_value}`: {error}"))
        }),
        None => Ok(default),
    }
}

fn load_u32_config(connection: &Connection, key: &str, default: u32) -> Result<u32, StoreError> {
    let raw_value = load_config_value(connection, key)?;
    match raw_value {
        Some(raw_value) => raw_value.parse::<u32>().map_err(|error| {
            StoreError::Serialization(format!("invalid {key} config `{raw_value}`: {error}"))
        }),
        None => Ok(default),
    }
}

fn load_config_value(connection: &Connection, key: &str) -> Result<Option<String>, StoreError> {
    connection
        .query_row(
            "SELECT value FROM scope_config WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(StoreError::from)
}

fn encode_embedding(embedding: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(embedding));
    for component in embedding {
        bytes.extend_from_slice(&component.to_le_bytes());
    }
    bytes
}

fn content_sha256(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";

    for byte in digest {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }

    encoded
}

fn upsert_encoded_embedding(
    connection: &Connection,
    id: &MemoryId,
    encoded_embedding: &[u8],
    content_sha256: &str,
) -> Result<(), StoreError> {
    let existing_vec_rowid: Option<i64> = connection
        .query_row(
            "SELECT vec_rowid FROM memory_embeddings WHERE memory_id = ?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()?;

    match existing_vec_rowid {
        Some(vec_rowid) => {
            connection.execute(
                "UPDATE vec_memories SET embedding = ?1 WHERE rowid = ?2",
                params![encoded_embedding, vec_rowid],
            )?;
            connection.execute(
                "UPDATE memory_embeddings SET content_sha256 = ?1 WHERE memory_id = ?2",
                params![content_sha256, id.to_string()],
            )?;
        }
        None => {
            connection.execute(
                "INSERT INTO vec_memories(embedding) VALUES (?1)",
                params![encoded_embedding],
            )?;
            let vec_rowid = connection.last_insert_rowid();
            connection.execute(
                "INSERT INTO memory_embeddings(memory_id, vec_rowid, content_sha256) VALUES (?1, ?2, ?3)",
                params![id.to_string(), vec_rowid, content_sha256],
            )?;
        }
    }

    connection.execute(
        "UPDATE memories SET embedding_stale = 0 WHERE id = ?1",
        [id.to_string()],
    )?;

    Ok(())
}

fn decode_embedding(bytes: &[u8], expected_dimensions: usize) -> Result<Vec<f32>, StoreError> {
    if !bytes.len().is_multiple_of(std::mem::size_of::<f32>()) {
        return Err(StoreError::Serialization(format!(
            "embedding blob length {} is not aligned to f32 components",
            bytes.len()
        )));
    }

    let actual_dimensions = bytes.len() / std::mem::size_of::<f32>();
    if actual_dimensions != expected_dimensions {
        return Err(StoreError::Serialization(format!(
            "stored embedding dimension mismatch: expected {expected_dimensions}, got {actual_dimensions}"
        )));
    }

    let mut embedding = Vec::with_capacity(actual_dimensions);
    for chunk in bytes.chunks_exact(std::mem::size_of::<f32>()) {
        embedding.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }

    Ok(embedding)
}

fn validate_query_embedding(
    embedding: &[f32],
    expected_dimensions: usize,
) -> Result<(), StoreError> {
    if embedding.is_empty() {
        return Err(StoreError::Validation(
            "embedding vector must not be empty".to_string(),
        ));
    }

    if embedding.len() != expected_dimensions {
        return Err(StoreError::Validation(format!(
            "embedding dimension mismatch: expected {expected_dimensions}, got {}",
            embedding.len()
        )));
    }

    Ok(())
}

fn validate_similarity_threshold(threshold: f32) -> Result<(), StoreError> {
    if threshold.is_finite() && (0.0..=1.0).contains(&threshold) {
        return Ok(());
    }

    Err(StoreError::Validation(
        "similarity threshold must be a finite value in the inclusive range 0.0..=1.0".to_string(),
    ))
}

fn load_search_memories(
    connection: &Connection,
    scopes: &[MemoryScope],
    state: MemoryState,
    type_filter: Option<&[MemoryType]>,
    agent_id_filter: Option<&str>,
) -> Result<Vec<Memory>, StoreError> {
    if scopes.is_empty() {
        return Ok(Vec::new());
    }
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    let scope_clause = scope_in_clause("scope", scopes, &mut params);
    let mut sql = format!(
        "SELECT {MEMORY_SELECT_COLUMNS} FROM memories WHERE {scope_clause} AND state = ?{}",
        params.len() + 1
    );
    params.push(rusqlite::types::Value::from(state_to_db(state).to_string()));

    if let Some(type_filter) = type_filter {
        if type_filter.is_empty() {
            return Ok(Vec::new());
        }

        sql.push_str(" AND memory_type IN (");
        for (index, memory_type) in type_filter.iter().enumerate() {
            if index > 0 {
                sql.push_str(", ");
            }
            sql.push('?');
            sql.push_str(&(params.len() + 1).to_string());
            params.push(rusqlite::types::Value::from(
                memory_type_to_db(*memory_type).to_string(),
            ));
        }
        sql.push(')');
    }

    if let Some(agent_id) = agent_id_filter {
        sql.push_str(" AND agent_id = ?");
        sql.push_str(&(params.len() + 1).to_string());
        params.push(rusqlite::types::Value::from(agent_id.to_string()));
    }

    sql.push_str(" ORDER BY updated_at DESC, rowid DESC");

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(params), map_memory_row)?;
    let mut memories = Vec::new();
    for row in rows {
        memories.push(row?);
    }
    Ok(memories)
}

fn load_keyword_scores(
    connection: &Connection,
    scopes: &[MemoryScope],
    state: MemoryState,
    type_filter: Option<&[MemoryType]>,
    agent_id_filter: Option<&str>,
    text: &str,
) -> Result<HashMap<MemoryId, f32>, StoreError> {
    let Some(fts_query) = build_fts_query(text) else {
        return Ok(HashMap::new());
    };
    if scopes.is_empty() {
        return Ok(HashMap::new());
    }

    let mut params: Vec<rusqlite::types::Value> = vec![rusqlite::types::Value::from(fts_query)];
    let scope_clause = scope_in_clause("m.scope", scopes, &mut params);
    let mut sql = format!(
        r#"
        SELECT m.id, bm25(memories_fts) AS bm25_score
        FROM memories_fts
        JOIN memories m ON m.rowid = memories_fts.rowid
        WHERE memories_fts MATCH ?1
          AND {scope_clause}
          AND m.state = ?{}
        "#,
        params.len() + 1
    );
    params.push(rusqlite::types::Value::from(state_to_db(state).to_string()));

    if let Some(type_filter) = type_filter {
        if type_filter.is_empty() {
            return Ok(HashMap::new());
        }

        sql.push_str(" AND m.memory_type IN (");
        for (index, memory_type) in type_filter.iter().enumerate() {
            if index > 0 {
                sql.push_str(", ");
            }
            sql.push('?');
            sql.push_str(&(params.len() + 1).to_string());
            params.push(rusqlite::types::Value::from(
                memory_type_to_db(*memory_type).to_string(),
            ));
        }
        sql.push(')');
    }

    if let Some(agent_id) = agent_id_filter {
        sql.push_str(" AND m.agent_id = ?");
        sql.push_str(&(params.len() + 1).to_string());
        params.push(rusqlite::types::Value::from(agent_id.to_string()));
    }

    sql.push_str(" ORDER BY bm25_score ASC, m.updated_at DESC");

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(params), |row| {
        let raw_id: String = row.get(0)?;
        Ok((parse_uuid_for_sqlite(&raw_id)?, row.get::<_, f64>(1)?))
    })?;

    let mut raw_matches = Vec::new();
    for row in rows {
        raw_matches.push(row?);
    }
    if raw_matches.is_empty() {
        return Ok(HashMap::new());
    }

    let best_score = raw_matches
        .iter()
        .map(|(_, score)| *score)
        .fold(f64::INFINITY, f64::min);
    let worst_score = raw_matches
        .iter()
        .map(|(_, score)| *score)
        .fold(f64::NEG_INFINITY, f64::max);

    let mut normalized_scores = HashMap::with_capacity(raw_matches.len());
    for (id, raw_score) in raw_matches {
        let normalized = if (worst_score - best_score).abs() < f64::EPSILON {
            1.0
        } else {
            ((worst_score - raw_score) / (worst_score - best_score)).clamp(0.0, 1.0)
        } as f32;
        normalized_scores.insert(id, normalized);
    }

    Ok(normalized_scores)
}

fn build_fts_query(text: &str) -> Option<String> {
    let terms = text
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>();

    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" AND "))
    }
}

fn load_vector_similarity_scores(
    connection: &Connection,
    scopes: &[MemoryScope],
    state: MemoryState,
    type_filter: Option<&[MemoryType]>,
    agent_id_filter: Option<&str>,
    query_embedding: &[f32],
    threshold: f32,
) -> Result<HashMap<MemoryId, f32>, StoreError> {
    let expected_dimensions = load_embedding_dimensions(connection)?;
    validate_query_embedding(query_embedding, expected_dimensions)?;
    if scopes.is_empty() {
        return Ok(HashMap::new());
    }

    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    let scope_clause = scope_in_clause("m.scope", scopes, &mut params);
    let mut sql = format!(
        r#"
        SELECT m.id, v.embedding
        FROM memories m
        JOIN memory_embeddings me ON me.memory_id = m.id
        JOIN vec_memories v ON v.rowid = me.vec_rowid
        WHERE {scope_clause}
          AND m.state = ?{}
          AND m.embedding_stale = 0
        "#,
        params.len() + 1
    );
    params.push(rusqlite::types::Value::from(state_to_db(state).to_string()));

    if let Some(type_filter) = type_filter {
        if type_filter.is_empty() {
            return Ok(HashMap::new());
        }

        sql.push_str(" AND m.memory_type IN (");
        for (index, memory_type) in type_filter.iter().enumerate() {
            if index > 0 {
                sql.push_str(", ");
            }
            sql.push('?');
            sql.push_str(&(params.len() + 1).to_string());
            params.push(rusqlite::types::Value::from(
                memory_type_to_db(*memory_type).to_string(),
            ));
        }
        sql.push(')');
    }

    if let Some(agent_id) = agent_id_filter {
        sql.push_str(" AND m.agent_id = ?");
        sql.push_str(&(params.len() + 1).to_string());
        params.push(rusqlite::types::Value::from(agent_id.to_string()));
    }

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(params), |row| {
        let raw_id: String = row.get(0)?;
        Ok((parse_uuid_for_sqlite(&raw_id)?, row.get::<_, Vec<u8>>(1)?))
    })?;

    let mut similarity_scores = HashMap::new();
    for row in rows {
        let (id, encoded_embedding) = row?;
        let stored_embedding = decode_embedding(&encoded_embedding, expected_dimensions)?;
        let similarity = cosine_similarity(query_embedding, &stored_embedding)?;
        if similarity >= threshold && similarity > 0.0 {
            similarity_scores.insert(id, similarity);
        }
    }

    Ok(similarity_scores)
}

fn combine_similarity_signals(
    vector_similarity: Option<f32>,
    keyword_similarity: Option<f32>,
) -> f32 {
    match (vector_similarity, keyword_similarity) {
        (Some(vector_similarity), Some(keyword_similarity)) => {
            (VECTOR_SIMILARITY_BLEND_WEIGHT * vector_similarity)
                + (KEYWORD_SIMILARITY_BLEND_WEIGHT * keyword_similarity)
        }
        (Some(vector_similarity), None) => vector_similarity,
        (None, Some(keyword_similarity)) => keyword_similarity,
        (None, None) => 0.0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetrievalScoringMode {
    Default,
    SimilarityOnly,
}

impl RetrievalScoringMode {
    fn from_env() -> Self {
        match std::env::var(RETRIEVAL_SCORING_MODE_ENV) {
            Ok(value)
                if value.eq_ignore_ascii_case("similarity-only")
                    || value.eq_ignore_ascii_case("similarity_only")
                    || value.eq_ignore_ascii_case("similarityonly") =>
            {
                Self::SimilarityOnly
            }
            _ => Self::Default,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::SimilarityOnly => "similarity-only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RetrievalScoreBreakdown {
    vector_similarity: Option<f32>,
    keyword_similarity: Option<f32>,
    blended_similarity: f32,
    recency_signal: f32,
    access_signal: f32,
    priority_signal: f32,
    weighted_similarity: f32,
    weighted_recency: f32,
    weighted_access: f32,
    weighted_priority: f32,
    total_score: f32,
    ranking_score: f32,
    similarity_band_index: usize,
    similarity_band_leader: f32,
}

type RankedSearchCandidate = (ScoredMemory, RetrievalScoreBreakdown);

fn rank_search_candidates(
    connection: &Connection,
    query: &SearchQuery,
    trimmed_text: &str,
    derived_query_embedding: Option<&[f32]>,
    scoring_mode: RetrievalScoringMode,
) -> Result<Vec<RankedSearchCandidate>, StoreError> {
    let scope_config = load_scope_config(connection)?;
    let visible_scopes = query.scope.visible_scopes();
    let requested_state = query.state_filter.unwrap_or(MemoryState::Active);
    let query_embedding = match query.embedding.as_deref() {
        Some(embedding) => {
            let expected_dimensions = load_embedding_dimensions(connection)?;
            validate_query_embedding(embedding, expected_dimensions)?;
            Some(embedding)
        }
        None => match derived_query_embedding {
            Some(embedding) => {
                let expected_dimensions = load_embedding_dimensions(connection)?;
                match validate_query_embedding(embedding, expected_dimensions) {
                    Ok(()) => Some(embedding),
                    Err(StoreError::Validation(_)) => None,
                    Err(error) => return Err(error),
                }
            }
            None => None,
        },
    };

    let mut keyword_scores = if trimmed_text.is_empty() {
        HashMap::new()
    } else {
        load_keyword_scores(
            connection,
            visible_scopes,
            requested_state,
            query.type_filter.as_deref(),
            query.agent_id.as_deref(),
            trimmed_text,
        )?
    };

    let mut vector_scores = match query_embedding {
        Some(embedding) => load_vector_similarity_scores(
            connection,
            visible_scopes,
            requested_state,
            query.type_filter.as_deref(),
            query.agent_id.as_deref(),
            embedding,
            0.0,
        )?,
        None => HashMap::new(),
    };

    let candidate_ids: HashSet<MemoryId> = keyword_scores
        .keys()
        .copied()
        .chain(vector_scores.keys().copied())
        .collect();
    if candidate_ids.is_empty() {
        return Ok(Vec::new());
    }

    let candidate_memories = load_search_memories(
        connection,
        visible_scopes,
        requested_state,
        query.type_filter.as_deref(),
        query.agent_id.as_deref(),
    )?;
    let candidate_memories_by_id: HashMap<MemoryId, Memory> = candidate_memories
        .into_iter()
        .filter(|memory| candidate_ids.contains(&memory.id))
        .map(|memory| (memory.id, memory))
        .collect();

    let scoring_now = Utc::now();
    let mut ranked_candidates = Vec::with_capacity(candidate_memories_by_id.len());
    for id in candidate_ids {
        let Some(memory) = candidate_memories_by_id.get(&id).cloned() else {
            continue;
        };

        let keyword_similarity = keyword_scores.remove(&id);
        let vector_similarity = vector_scores.remove(&id);
        let breakdown = compute_retrieval_score_breakdown_with_mode(
            &memory,
            vector_similarity,
            keyword_similarity,
            &scope_config,
            scoring_now,
            scoring_mode,
        );
        ranked_candidates.push((
            ScoredMemory {
                memory,
                score: breakdown.total_score,
                similarity: breakdown.blended_similarity,
            },
            breakdown,
        ));
    }

    assign_similarity_protected_ranking(&mut ranked_candidates, &scope_config, scoring_mode);

    Ok(ranked_candidates)
}

fn assign_similarity_protected_ranking(
    ranked_candidates: &mut [RankedSearchCandidate],
    scope_config: &ScopeConfig,
    scoring_mode: RetrievalScoringMode,
) {
    if ranked_candidates.is_empty() {
        return;
    }

    ranked_candidates.sort_by(compare_ranked_candidates_by_similarity_then_total);
    let max_total_score = maximum_possible_retrieval_score(scope_config, scoring_mode);

    if scoring_mode == RetrievalScoringMode::SimilarityOnly {
        for (scored_memory, breakdown) in ranked_candidates.iter_mut() {
            breakdown.ranking_score = breakdown.total_score;
            breakdown.similarity_band_index = 0;
            breakdown.similarity_band_leader = breakdown.blended_similarity;
            scored_memory.score = breakdown.ranking_score;
        }
        return;
    }

    let mut band_start = 0;
    let mut band_index = 0;
    while band_start < ranked_candidates.len() {
        let band_leader_similarity = ranked_candidates[band_start].0.similarity;
        let mut band_end = band_start + 1;
        while band_end < ranked_candidates.len()
            && (band_leader_similarity - ranked_candidates[band_end].0.similarity)
                <= RETRIEVAL_SIMILARITY_TIE_THRESHOLD
        {
            band_end += 1;
        }

        ranked_candidates[band_start..band_end].sort_by(compare_ranked_candidates_by_total);
        for (scored_memory, breakdown) in ranked_candidates[band_start..band_end].iter_mut() {
            breakdown.ranking_score = band_leader_similarity
                + normalize_retrieval_score_for_band(breakdown.total_score, max_total_score);
            breakdown.similarity_band_index = band_index;
            breakdown.similarity_band_leader = band_leader_similarity;
            scored_memory.score = breakdown.ranking_score;
        }

        band_start = band_end;
        band_index += 1;
    }
}

fn maximum_possible_retrieval_score(
    scope_config: &ScopeConfig,
    scoring_mode: RetrievalScoringMode,
) -> f32 {
    let similarity_weight = scope_config.similarity_weight.max(0.0);
    let secondary_weights = match scoring_mode {
        RetrievalScoringMode::Default => {
            scope_config.recency_weight.max(0.0)
                + scope_config.access_weight.max(0.0)
                + scope_config.priority_weight.max(0.0)
        }
        RetrievalScoringMode::SimilarityOnly => 0.0,
    };
    (similarity_weight + secondary_weights).max(f32::EPSILON)
}

fn normalize_retrieval_score_for_band(total_score: f32, max_total_score: f32) -> f32 {
    ((total_score.max(0.0) / max_total_score.max(f32::EPSILON)).clamp(0.0, 1.0))
        * RETRIEVAL_SIMILARITY_TIE_THRESHOLD
        * RETRIEVAL_BAND_REFINEMENT_RATIO
}

fn compare_ranked_candidates_by_similarity_then_total(
    left: &RankedSearchCandidate,
    right: &RankedSearchCandidate,
) -> std::cmp::Ordering {
    right
        .0
        .similarity
        .total_cmp(&left.0.similarity)
        .then_with(|| compare_ranked_candidates_by_total(left, right))
}

fn compare_ranked_candidates_by_total(
    left: &RankedSearchCandidate,
    right: &RankedSearchCandidate,
) -> std::cmp::Ordering {
    right
        .1
        .total_score
        .total_cmp(&left.1.total_score)
        .then_with(|| right.0.similarity.total_cmp(&left.0.similarity))
        .then_with(|| right.0.memory.updated_at.cmp(&left.0.memory.updated_at))
        .then_with(|| right.0.memory.id.cmp(&left.0.memory.id))
}

fn compute_retrieval_score_breakdown_with_mode(
    memory: &Memory,
    vector_similarity: Option<f32>,
    keyword_similarity: Option<f32>,
    scope_config: &ScopeConfig,
    now: DateTime<Utc>,
    scoring_mode: RetrievalScoringMode,
) -> RetrievalScoreBreakdown {
    let blended_similarity = combine_similarity_signals(vector_similarity, keyword_similarity);
    let recency = decay::retention(memory, now, scope_config) as f32;
    let access = compute_access_signal(memory.access_count);
    let priority = blended_similarity * memory.importance_score * memory.reliability_score;

    let (recency_weight, access_weight, priority_weight) = match scoring_mode {
        RetrievalScoringMode::Default => (
            scope_config.recency_weight,
            scope_config.access_weight,
            scope_config.priority_weight,
        ),
        RetrievalScoringMode::SimilarityOnly => (0.0, 0.0, 0.0),
    };

    let weighted_similarity = scope_config.similarity_weight * blended_similarity;
    let weighted_recency = recency_weight * recency;
    let weighted_access = access_weight * access;
    let weighted_priority = priority_weight * priority;

    RetrievalScoreBreakdown {
        vector_similarity,
        keyword_similarity,
        blended_similarity,
        recency_signal: recency,
        access_signal: access,
        priority_signal: priority,
        weighted_similarity,
        weighted_recency,
        weighted_access,
        weighted_priority,
        total_score: weighted_similarity + weighted_recency + weighted_access + weighted_priority,
        ranking_score: weighted_similarity + weighted_recency + weighted_access + weighted_priority,
        similarity_band_index: 0,
        similarity_band_leader: blended_similarity,
    }
}

fn compute_access_signal(access_count: u32) -> f32 {
    let access_count = access_count as f32;
    if access_count <= 0.0 {
        return 0.0;
    }

    access_count / (access_count + ACCESS_SIGNAL_HALF_SATURATION)
}

fn explain_retrieval_scoring_enabled() -> bool {
    env_flag_enabled(EXPLAIN_RETRIEVAL_SCORING_ENV)
}

fn env_flag_enabled(key: &str) -> bool {
    match std::env::var(key) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

fn emit_search_score_explanations(
    query_text: &str,
    scoring_mode: RetrievalScoringMode,
    ranked_candidates: &[RankedSearchCandidate],
    limit: usize,
) {
    let query_label = if query_text.is_empty() {
        "<embedding-only>"
    } else {
        query_text
    };
    eprintln!(
        "search scoring mode={} query=\"{}\" candidates={}",
        scoring_mode.as_str(),
        compact_retrieval_log_value(query_label, 120),
        ranked_candidates.len()
    );

    for (rank, (scored_memory, breakdown)) in ranked_candidates.iter().take(limit).enumerate() {
        eprintln!(
            "  rank={} id={} score={:.3} raw_total_score={:.3} similarity={:.3} band={} band_leader_similarity={:.3} vector_similarity={} keyword_similarity={} weighted_similarity={:.3} weighted_recency={:.3} weighted_access={:.3} weighted_priority={:.3} recency_signal={:.3} access_signal={:.3} priority_signal={:.3} importance={:.3} reliability={:.3} preview=\"{}\"",
            rank + 1,
            scored_memory.memory.id,
            scored_memory.score,
            breakdown.total_score,
            scored_memory.similarity,
            breakdown.similarity_band_index,
            breakdown.similarity_band_leader,
            format_optional_retrieval_score(breakdown.vector_similarity),
            format_optional_retrieval_score(breakdown.keyword_similarity),
            breakdown.weighted_similarity,
            breakdown.weighted_recency,
            breakdown.weighted_access,
            breakdown.weighted_priority,
            breakdown.recency_signal,
            breakdown.access_signal,
            breakdown.priority_signal,
            scored_memory.memory.importance_score,
            scored_memory.memory.reliability_score,
            compact_retrieval_log_value(&scored_memory.memory.content, 80),
        );
    }
}

fn format_optional_retrieval_score(value: Option<f32>) -> String {
    value
        .map(|score| format!("{score:.3}"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn compact_retrieval_log_value(value: &str, limit: usize) -> String {
    let compacted = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut compacted_chars = compacted.chars();
    let preview = compacted_chars.by_ref().take(limit).collect::<String>();
    if compacted_chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

fn compute_learned_weights_report(
    connection: &Connection,
) -> Result<LearnedWeightsReport, StoreError> {
    let default_weights = LearnedWeightValues::defaults();
    let scope_config = load_scope_config(connection)?;
    let samples = load_feedback_learning_samples(connection, &scope_config)?;
    let sample_size = samples.len();
    let relevant_samples = samples.iter().filter(|sample| sample.relevant).count();
    let irrelevant_samples = sample_size.saturating_sub(relevant_samples);

    if sample_size < LEARNING_MIN_TOTAL_FEEDBACK {
        return Ok(LearnedWeightsReport {
            sample_size,
            relevant_samples,
            irrelevant_samples,
            using_defaults: true,
            confidence: 0.0,
            status_detail: format!(
                "using defaults until at least {LEARNING_MIN_TOTAL_FEEDBACK} feedback rows exist"
            ),
            effective_weights: default_weights,
            default_weights,
        });
    }

    if relevant_samples < LEARNING_MIN_CLASS_FEEDBACK
        || irrelevant_samples < LEARNING_MIN_CLASS_FEEDBACK
    {
        return Ok(LearnedWeightsReport {
            sample_size,
            relevant_samples,
            irrelevant_samples,
            using_defaults: true,
            confidence: 0.0,
            status_detail: format!(
                "using defaults until feedback contains at least {LEARNING_MIN_CLASS_FEEDBACK} relevant and {LEARNING_MIN_CLASS_FEEDBACK} irrelevant judgments"
            ),
            effective_weights: default_weights,
            default_weights,
        });
    }

    let similarity_signal = feature_separation(&samples, |sample| sample.similarity_signal);
    let recency_signal = feature_separation(&samples, |sample| sample.recency_signal);
    let access_signal = feature_separation(&samples, |sample| sample.access_signal);
    let priority_signal = feature_separation(&samples, |sample| sample.priority_signal);

    let strongest_signal = similarity_signal
        .abs()
        .max(recency_signal.abs())
        .max(access_signal.abs())
        .max(priority_signal.abs());
    if strongest_signal < 0.10 {
        return Ok(LearnedWeightsReport {
            sample_size,
            relevant_samples,
            irrelevant_samples,
            using_defaults: true,
            confidence: 0.0,
            status_detail:
                "feedback has not separated any scoring component strongly enough; keeping defaults"
                    .to_string(),
            effective_weights: default_weights,
            default_weights,
        });
    }

    let confidence = learning_confidence(sample_size, relevant_samples, irrelevant_samples);
    let learned_weights = default_weights.with_multipliers(
        feature_multiplier(similarity_signal),
        feature_multiplier(recency_signal),
        feature_multiplier(access_signal),
        feature_multiplier(priority_signal),
    );
    let effective_weights = default_weights.blend(learned_weights, confidence);

    Ok(LearnedWeightsReport {
        sample_size,
        relevant_samples,
        irrelevant_samples,
        using_defaults: false,
        confidence,
        status_detail: "live scoring weights are being updated from retrieval feedback".to_string(),
        effective_weights,
        default_weights,
    })
}

fn load_feedback_learning_samples(
    connection: &Connection,
    scope_config: &ScopeConfig,
) -> Result<Vec<RetrievalFeedbackSample>, StoreError> {
    let mut statement = connection.prepare(
        r#"
        SELECT
            rf.relevant,
            rf.query_text,
            rf.recorded_at,
            m.content,
            m.summary,
            m.tags,
            m.access_count,
            m.importance_score,
            m.reliability_score,
            m.updated_at,
            m.last_accessed_at
        FROM retrieval_feedback rf
        JOIN memories m ON m.id = rf.memory_id
        ORDER BY rf.recorded_at ASC, rf.id ASC
        "#,
    )?;

    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)? != 0,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, u32>(6)?,
            row.get::<_, f32>(7)?,
            row.get::<_, f32>(8)?,
            row.get::<_, String>(9)?,
            row.get::<_, Option<String>>(10)?,
        ))
    })?;

    let mut samples = Vec::new();
    for row in rows {
        let (
            relevant,
            query_text,
            raw_recorded_at,
            content,
            summary,
            raw_tags,
            access_count,
            importance_score,
            reliability_score,
            raw_updated_at,
            raw_last_accessed_at,
        ) = row?;

        let recorded_at = parse_datetime_for_sqlite(&raw_recorded_at)?;
        let updated_at = parse_datetime_for_sqlite(&raw_updated_at)?;
        let last_accessed_at = raw_last_accessed_at
            .as_deref()
            .map(parse_datetime_for_sqlite)
            .transpose()?;
        let tags = parse_json::<Vec<String>>(&raw_tags)?;

        let similarity_signal = compute_feedback_similarity_signal(
            query_text.as_deref(),
            &content,
            summary.as_deref(),
            &tags,
        );
        let recency_signal = compute_feedback_recency_signal(
            updated_at,
            last_accessed_at,
            recorded_at,
            scope_config,
        );
        let access_signal = f64::from(compute_access_signal(access_count));
        let priority_signal =
            similarity_signal * f64::from(importance_score.max(0.0) * reliability_score.max(0.0));

        samples.push(RetrievalFeedbackSample {
            relevant,
            similarity_signal,
            recency_signal,
            access_signal,
            priority_signal,
        });
    }

    Ok(samples)
}

fn compute_feedback_similarity_signal(
    query_text: Option<&str>,
    content: &str,
    summary: Option<&str>,
    tags: &[String],
) -> f64 {
    let query_text = query_text.unwrap_or("").trim();
    if query_text.is_empty() {
        return 0.0;
    }

    let query_lower = query_text.to_lowercase();
    let memory_text = if let Some(summary) = summary {
        format!("{content} {summary} {}", tags.join(" "))
    } else {
        format!("{content} {}", tags.join(" "))
    };
    let memory_lower = memory_text.to_lowercase();
    if memory_lower.contains(&query_lower) {
        return 1.0;
    }

    let query_tokens = tokenize_learning_text(query_text);
    let memory_tokens = tokenize_learning_text(&memory_text);
    if query_tokens.is_empty() || memory_tokens.is_empty() {
        return 0.0;
    }

    let overlap = query_tokens.intersection(&memory_tokens).count() as f64;
    let coverage = overlap / query_tokens.len() as f64;
    let union = query_tokens.union(&memory_tokens).count() as f64;
    let jaccard = if union <= f64::EPSILON {
        0.0
    } else {
        overlap / union
    };

    (0.7 * coverage + 0.3 * jaccard).clamp(0.0, 1.0)
}

fn tokenize_learning_text(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|token| token.len() >= 2)
        .map(ToOwned::to_owned)
        .collect()
}

fn compute_feedback_recency_signal(
    updated_at: DateTime<Utc>,
    last_accessed_at: Option<DateTime<Utc>>,
    recorded_at: DateTime<Utc>,
    scope_config: &ScopeConfig,
) -> f64 {
    let reference_time = last_accessed_at.unwrap_or(updated_at);
    let elapsed_seconds = (recorded_at - reference_time).num_seconds().max(0) as f64;
    let elapsed_days = elapsed_seconds / 86_400.0;
    (-f64::from(scope_config.decay_lambda_base.max(0.0)) * elapsed_days).exp()
}

fn feature_separation(
    samples: &[RetrievalFeedbackSample],
    selector: impl Fn(&RetrievalFeedbackSample) -> f64,
) -> f64 {
    let mut relevant_values = Vec::new();
    let mut irrelevant_values = Vec::new();
    let mut all_values = Vec::with_capacity(samples.len());

    for sample in samples {
        let value = selector(sample);
        all_values.push(value);
        if sample.relevant {
            relevant_values.push(value);
        } else {
            irrelevant_values.push(value);
        }
    }

    let relevant_mean = mean(&relevant_values);
    let irrelevant_mean = mean(&irrelevant_values);
    let spread = standard_deviation(&all_values).max(0.05);
    ((relevant_mean - irrelevant_mean) / spread).clamp(-2.0, 2.0)
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn standard_deviation(values: &[f64]) -> f64 {
    if values.len() <= 1 {
        return 0.0;
    }

    let average = mean(values);
    let variance = values
        .iter()
        .map(|value| {
            let delta = value - average;
            delta * delta
        })
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt()
}

fn learning_confidence(
    sample_size: usize,
    relevant_samples: usize,
    irrelevant_samples: usize,
) -> f64 {
    let balance = if relevant_samples == 0 || irrelevant_samples == 0 {
        0.0
    } else {
        (relevant_samples.min(irrelevant_samples) as f64
            / relevant_samples.max(irrelevant_samples) as f64)
            .sqrt()
    };
    let sample_progress = if sample_size <= LEARNING_MIN_TOTAL_FEEDBACK {
        0.0
    } else {
        ((sample_size - LEARNING_MIN_TOTAL_FEEDBACK) as f64
            / (LEARNING_FULL_CONFIDENCE_FEEDBACK - LEARNING_MIN_TOTAL_FEEDBACK) as f64)
            .clamp(0.0, 1.0)
    };

    ((0.35 + (0.65 * sample_progress)) * balance).clamp(0.0, 1.0)
}

fn feature_multiplier(signal: f64) -> f64 {
    (1.0 + (0.45 * signal)).clamp(0.30, 2.50)
}

fn persist_learned_weights(
    connection: &Connection,
    weights: LearnedWeightValues,
) -> Result<(), StoreError> {
    for (key, value) in weights.to_btree_map() {
        connection.execute(
            "INSERT INTO scope_config(key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, format!("{value:.6}")],
        )?;
    }
    Ok(())
}

fn trim_results_to_context_budget(
    results: Vec<ScoredMemory>,
    context_config: Option<&MemoryContextConfig>,
    scope_config: &ScopeConfig,
) -> Vec<ScoredMemory> {
    let Some(context_config) = context_config else {
        return results;
    };

    let memory_context_ratio = if context_config.memory_context_ratio.is_finite() {
        context_config.memory_context_ratio.clamp(0.0, 1.0)
    } else {
        scope_config.memory_context_ratio
    };
    let response_reserve = context_config
        .response_reserve
        .unwrap_or(scope_config.response_reserve);
    let available_for_memory = context_config.model_max_tokens.saturating_sub(
        context_config
            .effective_already_used_tokens()
            .saturating_add(response_reserve),
    );
    let max_memory_tokens = ((available_for_memory as f32) * memory_context_ratio).floor() as u32;

    if max_memory_tokens == 0 {
        return Vec::new();
    }

    let mut retained = Vec::new();
    let mut used_tokens = 0_u32;
    for result in results {
        let estimated_tokens = estimate_memory_tokens(&result.memory);
        if retained.is_empty() || used_tokens.saturating_add(estimated_tokens) <= max_memory_tokens
        {
            used_tokens = used_tokens.saturating_add(estimated_tokens);
            retained.push(result);
        } else {
            break;
        }
    }

    retained
}

fn estimate_memory_tokens(memory: &Memory) -> u32 {
    let tags_length = if memory.tags.is_empty() {
        0
    } else {
        memory.tags.iter().map(String::len).sum::<usize>() + memory.tags.len().saturating_sub(1)
    };
    let raw_characters = memory.content.len()
        + memory.summary.as_ref().map_or(0, String::len)
        + memory.status.as_ref().map_or(0, String::len)
        + tags_length;

    let content_tokens = raw_characters.div_ceil(ESTIMATED_CHARS_PER_TOKEN) as u32;
    content_tokens.saturating_add(BASE_MEMORY_TOKEN_OVERHEAD)
}

fn touch_scored_memories(
    connection: &mut Connection,
    results: &mut [ScoredMemory],
) -> Result<(), StoreError> {
    if results.is_empty() {
        return Ok(());
    }

    let now = Utc::now();
    let transaction = connection.transaction()?;
    for result in results {
        result.memory.access_count = result
            .memory
            .access_count
            .checked_add(1)
            .ok_or(StoreError::Validation("access_count overflow".to_string()))?;
        result.memory.last_accessed_at = Some(now);

        transaction.execute(
            r#"
            UPDATE memories
            SET access_count = ?2,
                last_accessed_at = ?3
            WHERE id = ?1
            "#,
            params![
                result.memory.id.to_string(),
                i64::from(result.memory.access_count),
                format_timestamp(now),
            ],
        )?;
    }
    transaction.commit()?;

    Ok(())
}

fn count_memories_by_state(
    connection: &Connection,
    scope: MemoryScope,
    state: MemoryState,
) -> Result<i64, StoreError> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM memories WHERE scope = ?1 AND state = ?2",
            params![scope_to_db(scope), state_to_db(state)],
            |row| row.get::<_, i64>(0),
        )
        .map_err(StoreError::from)
}

fn compute_budget_usage_ratio(
    connection: &Connection,
    scope: MemoryScope,
    active_count: i64,
) -> Result<f32, StoreError> {
    let configured_budget = connection
        .query_row(
            "SELECT value FROM scope_config WHERE key = 'budget_active_max'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    let budget = match configured_budget {
        Some(value) => value.parse::<f32>().map_err(|error| {
            StoreError::Serialization(format!(
                "invalid budget_active_max config `{value}`: {error}"
            ))
        })?,
        None => match scope {
            MemoryScope::Workspace => DEFAULT_WORKSPACE_BUDGET,
            MemoryScope::User => DEFAULT_USER_BUDGET,
            MemoryScope::Agent => DEFAULT_AGENT_BUDGET,
            MemoryScope::Session => 0.0,
        },
    };

    if budget <= 0.0 {
        return Ok(0.0);
    }

    Ok((active_count as f32) / budget)
}

fn load_poisoning_config(connection: &Connection) -> Result<PoisoningConfig, StoreError> {
    Ok(PoisoningConfig {
        frequency_hourly_threshold: load_u32_config(
            connection,
            "poison_frequency_hourly_threshold",
            50,
        )?,
        frequency_scope_ratio: load_f32_config(connection, "poison_frequency_scope_ratio", 0.30)?,
        frequency_burst_ratio: load_f32_config(connection, "poison_frequency_burst_ratio", 0.25)?,
        frequency_burst_min_hourly: load_u32_config(
            connection,
            "poison_frequency_burst_min_hourly",
            12,
        )?,
        trust_mismatch_importance_threshold: load_f32_config(
            connection,
            "poison_trust_mismatch_importance_threshold",
            0.80,
        )?,
        trust_mismatch_count_threshold: load_u32_config(
            connection,
            "poison_trust_mismatch_count_threshold",
            5,
        )?,
        trust_mismatch_scope_ratio: load_f32_config(
            connection,
            "poison_trust_mismatch_scope_ratio",
            0.10,
        )?,
        bulk_overwrite_count_threshold: load_u32_config(
            connection,
            "poison_bulk_overwrite_count_threshold",
            20,
        )?,
        bulk_overwrite_scope_ratio: load_f32_config(
            connection,
            "poison_bulk_overwrite_scope_ratio",
            0.15,
        )?,
        mass_contradiction_per_memory_threshold: load_u32_config(
            connection,
            "poison_mass_contradiction_per_memory_threshold",
            3,
        )?,
        mass_contradiction_scope_ratio: load_f32_config(
            connection,
            "poison_mass_contradiction_scope_ratio",
            0.05,
        )?,
        remediation_reliability_ceiling: load_f32_config(
            connection,
            "poison_remediation_reliability_ceiling",
            0.60,
        )?,
    })
}

fn scaled_threshold(population: i64, absolute: u32, ratio: f32, minimum: u32) -> usize {
    let population = usize::try_from(population.max(0)).unwrap_or(usize::MAX);
    let ratio_count = ((population as f64) * f64::from(ratio.clamp(0.0, 1.0))).ceil() as usize;
    usize::try_from(absolute)
        .unwrap_or(usize::MAX)
        .max(ratio_count)
        .max(usize::try_from(minimum).unwrap_or(1))
}

fn compute_threshold_pressure(actual: usize, threshold: usize) -> f32 {
    if threshold == 0 {
        return 1.0;
    }

    (actual as f32) / (threshold as f32)
}

fn compute_poisoning_severity(
    pressure: f32,
    impacted_count: usize,
    population: i64,
    baseline: f32,
) -> f32 {
    let normalized_pressure = ((pressure - 1.0).max(0.0) / 2.0).clamp(0.0, 1.0);
    let impact_ratio = if population <= 0 {
        0.0
    } else {
        (impacted_count as f32 / population as f32).clamp(0.0, 1.0)
    };

    (baseline + (normalized_pressure * 0.35) + (impact_ratio * 0.30)).clamp(0.0, 1.0)
}

pub(crate) fn display_poisoning_alert_type(alert_type: &PoisoningAlertType) -> &'static str {
    match alert_type {
        PoisoningAlertType::FrequencyAnomaly => "frequency_anomaly",
        PoisoningAlertType::TrustMismatch => "trust_mismatch",
        PoisoningAlertType::BulkOverwrite => "bulk_overwrite",
        PoisoningAlertType::MassContradiction => "mass_contradiction",
    }
}

fn is_low_trust_memory(memory: &Memory, reliability_ceiling: f32) -> bool {
    memory.reliability_score <= reliability_ceiling
        || memory.provenance.base_reliability() <= reliability_ceiling
}

#[derive(Debug, Clone, Copy)]
struct PoisoningMemorySnapshot {
    id: MemoryId,
    scope: MemoryScope,
    state: MemoryState,
    reliability_score: f32,
    provenance: ProvenanceLevel,
}

fn sort_memory_ids(ids: &mut [MemoryId]) {
    ids.sort();
}

fn is_benign_poisoning_update(changed_by: &str) -> bool {
    changed_by.trim_start().starts_with("system:")
}

fn load_poisoning_memory_snapshot(
    row: &Row<'_>,
    offset: usize,
) -> rusqlite::Result<PoisoningMemorySnapshot> {
    let raw_id: String = row.get(offset)?;
    let raw_scope: String = row.get(offset + 1)?;
    let raw_state: String = row.get(offset + 2)?;
    let raw_provenance: String = row.get(offset + 4)?;
    Ok(PoisoningMemorySnapshot {
        id: parse_uuid_for_sqlite(&raw_id)?,
        scope: parse_scope_for_sqlite(&raw_scope)?,
        state: parse_state_for_sqlite(&raw_state)?,
        reliability_score: row.get(offset + 3)?,
        provenance: parse_provenance_for_sqlite(&raw_provenance)?,
    })
}

fn is_low_trust_snapshot(snapshot: &PoisoningMemorySnapshot, reliability_ceiling: f32) -> bool {
    snapshot.reliability_score <= reliability_ceiling
        || snapshot.provenance.base_reliability() <= reliability_ceiling
}

fn compare_poisoning_trust(
    left: &PoisoningMemorySnapshot,
    right: &PoisoningMemorySnapshot,
) -> std::cmp::Ordering {
    left.reliability_score
        .partial_cmp(&right.reliability_score)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| {
            left.provenance
                .base_reliability()
                .partial_cmp(&right.provenance.base_reliability())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn select_mass_contradiction_candidates(
    scope: MemoryScope,
    left: &PoisoningMemorySnapshot,
    right: &PoisoningMemorySnapshot,
    reliability_ceiling: f32,
) -> Vec<MemoryId> {
    let left_in_scope = left.scope == scope && left.state == MemoryState::Active;
    let right_in_scope = right.scope == scope && right.state == MemoryState::Active;
    let left_low_trust = is_low_trust_snapshot(left, reliability_ceiling);
    let right_low_trust = is_low_trust_snapshot(right, reliability_ceiling);

    match compare_poisoning_trust(left, right) {
        std::cmp::Ordering::Less if left_in_scope && left_low_trust => vec![left.id],
        std::cmp::Ordering::Greater if right_in_scope && right_low_trust => vec![right.id],
        std::cmp::Ordering::Equal => {
            let mut ids = Vec::new();
            if left_in_scope && left_low_trust {
                ids.push(left.id);
            }
            if right_in_scope && right_low_trust {
                ids.push(right.id);
            }
            ids
        }
        _ => Vec::new(),
    }
}

fn build_shared_import_candidate(source: &Memory) -> MemoryCandidate {
    MemoryCandidate {
        content: source.content.clone(),
        summary: source.summary.clone(),
        memory_type: source.memory_type,
        provenance: ProvenanceLevel::Imported,
        importance_score: source.importance_score,
        sensitivity: source.sensitivity,
        tags: source.tags.clone(),
        custom_metadata: source.custom_metadata.clone(),
        embedding: None,
    }
}

fn prepare_shared_import_memory(
    source: &Memory,
    scope: MemoryScope,
    status: &str,
    disposition: &str,
    reason: &str,
    related_memory_id: Option<&MemoryId>,
) -> Memory {
    let now = Utc::now();
    let mut imported = source.clone();
    imported.id = Uuid::new_v4();
    imported.scope = scope;
    imported.provenance = ProvenanceLevel::Imported;
    imported.reliability_score = imported.reliability_score.min(0.6);
    imported.embedding_stale = true;
    imported.state = MemoryState::Dormant;
    imported.status = Some(status.to_string());
    imported.tenant_id = None;
    imported.user_id = None;
    imported.agent_id = None;
    imported.created_at = now;
    imported.updated_at = now;
    imported.last_accessed_at = None;
    imported.access_count = 0;
    imported.custom_metadata.insert(
        SHARED_IMPORT_SOURCE_METADATA_KEY.to_string(),
        "cross_agent".to_string(),
    );
    imported.custom_metadata.insert(
        SHARED_IMPORT_DISPOSITION_METADATA_KEY.to_string(),
        disposition.to_string(),
    );
    imported.custom_metadata.insert(
        SHARED_IMPORT_REASON_METADATA_KEY.to_string(),
        reason.to_string(),
    );
    imported.custom_metadata.insert(
        SHARED_IMPORT_ORIGINAL_ID_METADATA_KEY.to_string(),
        source.id.to_string(),
    );
    if let Some(related_memory_id) = related_memory_id {
        imported.custom_metadata.insert(
            SHARED_IMPORT_REFERENCED_MEMORY_METADATA_KEY.to_string(),
            related_memory_id.to_string(),
        );
    }
    imported
}

fn exact_shared_import_duplicate_decision(
    store: &SqliteMemoryStore,
    source: &Memory,
) -> Result<Option<GateDecision>, StoreError> {
    let normalized_source = normalize_shared_import_content(&source.content);
    if normalized_source.is_empty() {
        return Ok(None);
    }

    let exact_match = store.with_connection(|connection| {
        let visible = load_search_memories(
            connection,
            store.scope.visible_scopes(),
            MemoryState::Active,
            None,
            None,
        )?;
        Ok(visible
            .into_iter()
            .filter(|memory| normalize_shared_import_content(&memory.content) == normalized_source)
            .max_by_key(|memory| memory.scope.rank()))
    })?;

    Ok(exact_match.map(|memory| {
        if memory.scope.rank() > store.scope.rank() {
            GateDecision::Reject {
                reason: "near-duplicate already exists in a higher visible scope".to_string(),
            }
        } else {
            GateDecision::Accept {
                similar_to: Some(memory.id),
                similarity: Some(1.0),
            }
        }
    }))
}

fn normalize_shared_import_content(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn exact_correction_duplicate_decision(
    store: &SqliteMemoryStore,
    excluded_id: &MemoryId,
    corrected_content: &str,
) -> Result<Option<GateDecision>, StoreError> {
    let normalized = normalize_shared_import_content(corrected_content);
    if normalized.is_empty() {
        return Ok(None);
    }

    let exact_match = store.with_connection(|connection| {
        let visible = load_search_memories(
            connection,
            store.scope.visible_scopes(),
            MemoryState::Active,
            None,
            None,
        )?;
        Ok(visible
            .into_iter()
            .filter(|memory| memory.id != *excluded_id)
            .filter(|memory| normalize_shared_import_content(&memory.content) == normalized)
            .max_by_key(|memory| memory.scope.rank()))
    })?;

    Ok(exact_match.map(|memory| GateDecision::Merge {
        target_id: memory.id,
        enriched_content: memory.content,
        promote_to: None,
    }))
}

fn insert_memory_version_row(
    connection: &Connection,
    memory_id: &MemoryId,
    previous_content: &str,
    changed_at: DateTime<Utc>,
    changed_by: &str,
    change_reason: &str,
) -> Result<(), StoreError> {
    let next_version_number = load_next_version_number(connection, memory_id)?;
    connection.execute(
        r#"
        INSERT INTO memory_versions(
            id,
            memory_id,
            version_number,
            content,
            changed_at,
            changed_by,
            change_reason
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        params![
            Uuid::new_v4().to_string(),
            memory_id.to_string(),
            i64::from(next_version_number),
            previous_content,
            format_timestamp(changed_at),
            changed_by,
            change_reason,
        ],
    )?;
    Ok(())
}

fn correction_disposition_to_db(disposition: CorrectionDisposition) -> &'static str {
    match disposition {
        CorrectionDisposition::Applied => "applied",
        CorrectionDisposition::Archived => "archived",
        CorrectionDisposition::Merged => "merged",
        CorrectionDisposition::Contradiction => "contradiction",
    }
}

fn shared_import_disposition(decision: &GateDecision) -> SharedImportDisposition {
    match decision {
        GateDecision::Accept {
            similar_to: Some(memory_id),
            ..
        } => SharedImportDisposition::Quarantine {
            reason: "near-duplicate shared memory requires review".to_string(),
            related_memory_id: Some(*memory_id),
        },
        GateDecision::Accept { .. } | GateDecision::Archive => SharedImportDisposition::Review {
            reason: "shared memory imported as dormant review-only entry".to_string(),
            related_memory_id: None,
        },
        GateDecision::Merge { target_id, .. } => SharedImportDisposition::Quarantine {
            reason: "shared memory would have merged with an existing memory; quarantined instead"
                .to_string(),
            related_memory_id: Some(*target_id),
        },
        GateDecision::Contradiction { conflicting_id, .. } => SharedImportDisposition::Quarantine {
            reason: "shared memory contradicts an existing memory and was quarantined".to_string(),
            related_memory_id: Some(*conflicting_id),
        },
        GateDecision::Reject { reason } => SharedImportDisposition::Skip {
            reason: format!("skipped shared memory: {reason}"),
        },
    }
}

fn evaluate_gate_sync(
    gate: DefaultSalienceGate,
    store: SqliteMemoryStore,
    candidate: MemoryCandidate,
) -> Result<GateDecision, StoreError> {
    run_store_future(async move {
        gate.evaluate(&candidate, &store)
            .await
            .map_err(|error| match error {
                GateError::Store(store_error) => store_error,
                other => {
                    StoreError::Validation(format!("shared import gate evaluation failed: {other}"))
                }
            })
    })
}

fn run_store_future<T, F>(future: F) -> Result<T, StoreError>
where
    T: Send + 'static,
    F: std::future::Future<Output = Result<T, StoreError>> + Send + 'static,
{
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                StoreError::Validation(format!("failed to build async runtime: {error}"))
            })?;
        runtime.block_on(future)
    })
    .join()
    .map_err(|_| StoreError::Validation("shared import worker thread panicked".to_string()))?
}

fn insert_memory_without_embedding(
    store: &SqliteMemoryStore,
    memory: &Memory,
) -> Result<(), StoreError> {
    validate_memory_for_store(memory, store.scope)?;
    store.with_connection(|connection| {
        let transaction = connection.transaction()?;
        transaction.execute(
            r#"
            INSERT INTO memories(
                id,
                content,
                summary,
                scope,
                memory_type,
                provenance,
                importance_score,
                reliability_score,
                sensitivity,
                state,
                tags,
                status,
                custom_metadata,
                access_count,
                corroboration_count,
                embedding_stale,
                created_at,
                updated_at,
                last_accessed_at,
                tenant_id,
                user_id,
                agent_id
            )
            VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
            )
            "#,
            rusqlite::params_from_iter(memory_insert_params(memory)?),
        )?;

        let row_id = require_memory_rowid(&transaction, &memory.id)?;
        sync_fts_entry(&transaction, row_id, None, memory)?;
        transaction.commit()?;
        Ok(())
    })
}

fn lower_reliability(
    connection: &Connection,
    memory: &Memory,
    now: DateTime<Utc>,
) -> Result<(), StoreError> {
    let adjusted_reliability = (memory.reliability_score - 0.3).max(0.0);
    connection.execute(
        "UPDATE memories SET reliability_score = ?2, updated_at = ?3 WHERE id = ?1",
        params![
            memory.id.to_string(),
            f64::from(adjusted_reliability),
            format_timestamp(now),
        ],
    )?;
    Ok(())
}

fn record_contradiction_row(
    connection: &Connection,
    a_id: &MemoryId,
    b_id: &MemoryId,
    description: &str,
) -> Result<(), StoreError> {
    let memory_a = require_memory(connection, a_id)?.ok_or(StoreError::NotFound(*a_id))?;
    let memory_b = require_memory(connection, b_id)?.ok_or(StoreError::NotFound(*b_id))?;
    let now = Utc::now();

    connection.execute(
        r#"
        INSERT INTO contradictions(
            id,
            memory_a_id,
            memory_b_id,
            detected_at,
            description,
            resolution_status,
            resolved_at,
            resolution_note
        )
        VALUES (?1, ?2, ?3, ?4, ?5, 'unresolved', NULL, NULL)
        "#,
        params![
            Uuid::new_v4().to_string(),
            a_id.to_string(),
            b_id.to_string(),
            format_timestamp(now),
            description.trim(),
        ],
    )?;

    if memory_a.provenance.base_reliability() > memory_b.provenance.base_reliability() {
        lower_reliability(connection, &memory_b, now)?;
    } else if memory_b.provenance.base_reliability() > memory_a.provenance.base_reliability() {
        lower_reliability(connection, &memory_a, now)?;
    }

    Ok(())
}

fn record_link_row(
    connection: &Connection,
    source_id: &MemoryId,
    target_id: &MemoryId,
    relation_type: &str,
) -> Result<(), StoreError> {
    connection.execute(
        r#"
        INSERT OR IGNORE INTO memory_links(id, source_id, target_id, relation_type, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        params![
            Uuid::new_v4().to_string(),
            source_id.to_string(),
            target_id.to_string(),
            relation_type,
            format_timestamp(Utc::now()),
        ],
    )?;
    Ok(())
}

fn load_links(
    connection: &Connection,
    memory_id: &MemoryId,
) -> Result<Vec<MemoryLink>, StoreError> {
    let id_str = memory_id.to_string();
    let mut statement = connection.prepare(
        r#"
        SELECT id, source_id, target_id, relation_type, weight, created_at
        FROM memory_links
        WHERE source_id = ?1 OR target_id = ?1
        ORDER BY created_at
        "#,
    )?;
    let links = statement
        .query_map([&id_str], |row| {
            let raw_source_id: String = row.get(1)?;
            let raw_target_id: String = row.get(2)?;
            let raw_created_at: String = row.get(5)?;
            Ok(MemoryLink {
                id: row.get(0)?,
                source_id: parse_uuid_for_sqlite(&raw_source_id)?,
                target_id: parse_uuid_for_sqlite(&raw_target_id)?,
                relation_type: row.get(3)?,
                weight: row.get(4)?,
                created_at: parse_datetime_for_sqlite(&raw_created_at)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(links)
}

/// Map a [`SensitivityLevel`] variant to a comparable ordinal.
fn sensitivity_ord(level: SensitivityLevel) -> u8 {
    match level {
        SensitivityLevel::Low => 0,
        SensitivityLevel::Medium => 1,
        SensitivityLevel::High => 2,
        SensitivityLevel::Critical => 3,
    }
}

/// Payload returned by [`load_export_payload`]: memories, links, and versions.
type ExportPayload = (Vec<Memory>, Vec<MemoryLink>, Vec<MemoryVersion>);

/// Load all active and dormant memories, their links, and version history for
/// a single scope.  Used by the SQLite and `.elegy` export paths.
fn load_export_payload(
    connection: &Connection,
    scope: MemoryScope,
) -> Result<ExportPayload, StoreError> {
    let mut memories = load_search_memories(connection, &[scope], MemoryState::Active, None, None)?;
    memories.extend(load_search_memories(
        connection,
        &[scope],
        MemoryState::Dormant,
        None,
        None,
    )?);

    let scope_str = scope_to_db(scope);
    let links = load_scope_links(connection, scope_str)?;
    let versions = load_scope_versions(connection, scope_str)?;

    Ok((memories, links, versions))
}

/// Load all links where **both** endpoints belong to the given scope.
fn load_scope_links(
    connection: &Connection,
    scope_str: &str,
) -> Result<Vec<MemoryLink>, StoreError> {
    let mut statement = connection.prepare(
        r#"
        SELECT ml.id, ml.source_id, ml.target_id, ml.relation_type, ml.weight, ml.created_at
        FROM memory_links ml
        JOIN memories ms ON ml.source_id = ms.id
        JOIN memories mt ON ml.target_id = mt.id
        WHERE ms.scope = ?1 AND mt.scope = ?1
        ORDER BY ml.created_at
        "#,
    )?;
    let links = statement
        .query_map([scope_str], |row| {
            let raw_source_id: String = row.get(1)?;
            let raw_target_id: String = row.get(2)?;
            let raw_created_at: String = row.get(5)?;
            Ok(MemoryLink {
                id: row.get(0)?,
                source_id: parse_uuid_for_sqlite(&raw_source_id)?,
                target_id: parse_uuid_for_sqlite(&raw_target_id)?,
                relation_type: row.get(3)?,
                weight: row.get(4)?,
                created_at: parse_datetime_for_sqlite(&raw_created_at)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(links)
}

/// Load all version-history rows for memories that belong to the given scope.
fn load_scope_versions(
    connection: &Connection,
    scope_str: &str,
) -> Result<Vec<MemoryVersion>, StoreError> {
    let mut statement = connection.prepare(
        r#"
        SELECT mv.id, mv.memory_id, mv.version_number, mv.content,
               mv.changed_by, mv.change_reason, mv.changed_at
        FROM memory_versions mv
        JOIN memories m ON mv.memory_id = m.id
        WHERE m.scope = ?1
        ORDER BY mv.memory_id, mv.version_number DESC
        "#,
    )?;
    let versions = statement
        .query_map([scope_str], map_memory_version_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(versions)
}

/// Create a self-contained SQLite file containing the given memories, links,
/// and version history.  Returns the raw file bytes.
fn export_to_sqlite_file(
    path: &Path,
    memories: &[Memory],
    links: &[MemoryLink],
    versions: &[MemoryVersion],
) -> Result<Vec<u8>, StoreError> {
    let mut connection = init_database(path)?;

    let transaction = connection.transaction()?;

    for memory in memories {
        transaction.execute(
            r#"
            INSERT INTO memories(
                id, content, summary, scope, memory_type, provenance,
                importance_score, reliability_score, sensitivity, state,
                tags, status, custom_metadata, access_count,
                corroboration_count, embedding_stale, created_at,
                updated_at, last_accessed_at, tenant_id, user_id, agent_id
            )
            VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
            )
            "#,
            rusqlite::params_from_iter(memory_insert_params(memory)?),
        )?;
    }

    for link in links {
        transaction.execute(
            r#"
            INSERT INTO memory_links(id, source_id, target_id, relation_type, weight, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                link.id,
                link.source_id.to_string(),
                link.target_id.to_string(),
                link.relation_type,
                link.weight,
                format_timestamp(link.created_at),
            ],
        )?;
    }

    for version in versions {
        transaction.execute(
            r#"
            INSERT INTO memory_versions(id, memory_id, version_number, content, changed_by, change_reason, changed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                version.id,
                version.memory_id.to_string(),
                version.version_number,
                version.content,
                version.changed_by,
                version.change_reason,
                format_timestamp(version.changed_at),
            ],
        )?;
    }

    // Rebuild FTS index so full-text search works in the exported DB
    transaction.execute_batch("INSERT INTO memories_fts(memories_fts) VALUES('rebuild')")?;

    transaction.commit()?;
    drop(connection);

    std::fs::read(path).map_err(|error| {
        StoreError::Migration(format!("failed to read exported SQLite file: {error}"))
    })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        env, fs,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use chrono::Utc;
    use rusqlite::params;
    use tokio;
    use uuid::Uuid;

    use super::{
        expand_compound_words, format_timestamp, split_compound_word, SqliteMemoryStore,
        POISONING_QUARANTINED_AT_METADATA_KEY, POISONING_REMEDIATION_METADATA_KEY,
        QUARANTINED_STATUS, SHARED_REVIEW_STATUS,
    };
    use crate::{
        CorrectionDisposition, ElegyArchive, EmbeddingError, EmbeddingProvider, ExportFormat,
        Memory, MemoryFilter, MemoryId, MemoryScope, MemoryState, MemoryStore, MemoryType,
        MetadataUpdate, OptionalFieldUpdate, PoisoningAlert, PoisoningAlertType, ProvenanceLevel,
        ResolutionStatus, ScopeConfig, SearchQuery, SensitivityLevel, ShareConfig,
    };

    #[derive(Debug, Clone)]
    enum StubEmbeddingResponse {
        Embedding(Vec<f32>),
        Failure(String),
    }

    #[derive(Debug)]
    struct StubEmbeddingProvider {
        responses: HashMap<String, StubEmbeddingResponse>,
        calls: Mutex<Vec<String>>,
    }

    impl StubEmbeddingProvider {
        fn new<I, S>(responses: I) -> Self
        where
            I: IntoIterator<Item = (S, StubEmbeddingResponse)>,
            S: Into<String>,
        {
            Self {
                responses: responses
                    .into_iter()
                    .map(|(text, response)| (text.into(), response))
                    .collect(),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("stub provider calls lock").clone()
        }

        fn call_count(&self) -> usize {
            self.calls.lock().expect("stub provider calls lock").len()
        }
    }

    #[async_trait]
    impl EmbeddingProvider for StubEmbeddingProvider {
        async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
            let trimmed = text.trim().to_string();
            self.calls
                .lock()
                .expect("stub provider calls lock")
                .push(trimmed.clone());

            match self.responses.get(&trimmed) {
                Some(StubEmbeddingResponse::Embedding(embedding)) => Ok(embedding.clone()),
                Some(StubEmbeddingResponse::Failure(message)) => {
                    Err(EmbeddingError::Provider(message.clone()))
                }
                None => Err(EmbeddingError::Provider(format!(
                    "missing stub embedding for `{trimmed}`"
                ))),
            }
        }

        fn dimensions(&self) -> usize {
            768
        }

        fn model_id(&self) -> &str {
            "stub-embedding-provider"
        }
    }

    #[test]
    fn split_compound_word_handles_camel_case_and_acronym_boundaries() {
        assert_eq!(
            split_compound_word("JavaScript").as_deref(),
            Some("Java Script")
        );
        assert_eq!(
            split_compound_word("ProtonVPN").as_deref(),
            Some("Proton VPN")
        );
        assert_eq!(
            split_compound_word("XMLParser").as_deref(),
            Some("XML Parser")
        );
        assert_eq!(split_compound_word("VPN"), None);
    }

    #[test]
    fn expand_compound_words_preserves_original_text_and_appends_split_forms() {
        assert_eq!(
            expand_compound_words("ProtonVPN avec WireGuard et JavaScript"),
            "ProtonVPN avec WireGuard et JavaScript Proton VPN Wire Guard Java Script"
        );
    }

    #[tokio::test]
    async fn store_and_get_round_trip_updates_access_tracking() {
        let fixture = test_fixture();
        let memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let id = memory.id;

        fixture
            .store
            .store(memory.clone())
            .await
            .expect("store memory");

        let raw = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get raw memory")
            .expect("memory exists");
        assert_eq!(raw.content, memory.content);
        assert_eq!(raw.access_count, 0);
        assert!(raw.last_accessed_at.is_none());

        let fetched = fixture
            .store
            .get(&id)
            .await
            .expect("get memory")
            .expect("memory exists");
        assert_eq!(fetched.access_count, 1);
        assert!(fetched.last_accessed_at.is_some());

        let persisted = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get raw persisted memory")
            .expect("memory exists");
        assert_eq!(persisted.access_count, 1);
        assert!(persisted.last_accessed_at.is_some());
    }

    #[tokio::test]
    async fn update_content_creates_version_and_marks_embedding_stale() {
        let fixture = test_fixture();
        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        memory.embedding_stale = false;
        let id = memory.id;
        let original_updated_at = memory.updated_at;

        fixture
            .store
            .store(memory.clone())
            .await
            .expect("store memory");

        fixture
            .store
            .update_content(&id, "updated content", "agent:test", "manual enrichment")
            .await
            .expect("update content");

        let updated = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get updated memory")
            .expect("memory exists");
        assert_eq!(updated.content, "updated content");
        assert!(updated.embedding_stale);
        assert!(updated.updated_at >= original_updated_at);

        let version_row = fixture
            .store
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT version_number, content, changed_by, change_reason FROM memory_versions WHERE memory_id = ?1",
                    [id.to_string()],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, Option<String>>(3)?,
                        ))
                    },
                )
                .map_err(crate::StoreError::from)
            })
            .expect("load version row");

        assert_eq!(version_row.0, 1);
        assert_eq!(version_row.1, memory.content);
        assert_eq!(version_row.2, "agent:test");
        assert_eq!(version_row.3.as_deref(), Some("manual enrichment"));
    }

    #[tokio::test]
    async fn lifecycle_and_hard_delete_remove_related_rows() {
        let fixture = test_fixture();
        let memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let id = memory.id;

        fixture.store.store(memory).await.expect("store memory");
        fixture
            .store
            .store_embedding(&id, &[0.5; 768])
            .await
            .expect("store embedding");

        fixture.store.make_dormant(&id).await.expect("make dormant");
        let dormant = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get dormant memory")
            .expect("memory exists");
        assert_eq!(dormant.state, MemoryState::Dormant);

        fixture.store.reactivate(&id).await.expect("reactivate");
        let active = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get active memory")
            .expect("memory exists");
        assert_eq!(active.state, MemoryState::Active);

        fixture.store.hard_delete(&id).await.expect("hard delete");
        assert!(fixture
            .store
            .get_raw(&id)
            .await
            .expect("get deleted memory")
            .is_none());

        let (embedding_rows, vector_rows) = fixture
            .store
            .with_connection(|connection| {
                let embedding_rows = connection.query_row(
                    "SELECT COUNT(*) FROM memory_embeddings WHERE memory_id = ?1",
                    [id.to_string()],
                    |row| row.get::<_, i64>(0),
                )?;
                let vector_rows =
                    connection.query_row("SELECT COUNT(*) FROM vec_memories", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                Ok((embedding_rows, vector_rows))
            })
            .expect("load cascade counts");

        assert_eq!(embedding_rows, 0);
        assert_eq!(vector_rows, 0);
    }

    #[tokio::test]
    async fn list_applies_filters_and_metadata_updates() {
        let fixture = test_fixture();
        let memory_a = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let id_a = memory_a.id;
        let mut memory_b = sample_memory(MemoryScope::Workspace, ProvenanceLevel::Imported);
        memory_b.memory_type = MemoryType::Decision;
        memory_b.tags = vec!["project".to_string(), "shipping".to_string()];
        let id_b = memory_b.id;

        fixture.store.store(memory_a).await.expect("store memory a");
        fixture.store.store(memory_b).await.expect("store memory b");

        fixture
            .store
            .update_metadata(
                &id_a,
                MetadataUpdate {
                    tags: Some(vec!["project".to_string(), "important".to_string()]),
                    status: Some(OptionalFieldUpdate::Set("open".to_string())),
                    custom_metadata: Some(HashMap::from([(
                        "source".to_string(),
                        "notes".to_string(),
                    )])),
                    importance_score: Some(0.9),
                    reliability_score: Some(0.95),
                    state: Some(MemoryState::Dormant),
                },
            )
            .await
            .expect("update metadata");

        let dormant = fixture
            .store
            .list(MemoryFilter {
                state: Some(MemoryState::Dormant),
                tags: Some(vec!["project".to_string(), "important".to_string()]),
                status: Some("open".to_string()),
                limit: Some(5),
                ..MemoryFilter::default()
            })
            .await
            .expect("list dormant memories");
        assert_eq!(dormant.len(), 1);
        assert_eq!(dormant[0].id, id_a);
        assert_eq!(
            dormant[0].custom_metadata.get("source").map(String::as_str),
            Some("notes")
        );

        let active_decisions = fixture
            .store
            .list(MemoryFilter {
                state: Some(MemoryState::Active),
                memory_types: Some(vec![MemoryType::Decision]),
                tags: Some(vec!["project".to_string()]),
                limit: Some(5),
                ..MemoryFilter::default()
            })
            .await
            .expect("list active decision memories");
        assert_eq!(active_decisions.len(), 1);
        assert_eq!(active_decisions[0].id, id_b);
    }

    #[tokio::test]
    async fn health_report_and_contradictions_reflect_store_state() {
        let fixture = test_fixture();
        let trusted = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let trusted_id = trusted.id;
        let mut less_trusted =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::AgentInferred);
        less_trusted.reliability_score = 0.7;
        less_trusted.embedding_stale = true;
        let less_trusted_id = less_trusted.id;

        fixture
            .store
            .store(trusted)
            .await
            .expect("store trusted memory");
        fixture
            .store
            .store(less_trusted)
            .await
            .expect("store less trusted memory");
        fixture
            .store
            .store_embedding(&trusted_id, &[0.25; 768])
            .await
            .expect("store trusted embedding");
        fixture
            .store
            .make_dormant(&less_trusted_id)
            .await
            .expect("make less trusted memory dormant");
        fixture
            .store
            .record_contradiction(&trusted_id, &less_trusted_id, "conflicting delivery date")
            .await
            .expect("record contradiction");

        let report = fixture.store.health_report().await.expect("health report");
        assert_eq!(report.scope, MemoryScope::Workspace);
        assert_eq!(report.active_count, 1);
        assert_eq!(report.dormant_count, 1);
        assert_eq!(report.unresolved_contradictions, 1);
        assert_eq!(report.stale_embeddings_count, 1);
        assert!(report.total_storage_bytes > 0);
        assert!(report.budget_usage_ratio > 0.0);
        assert!(report.newest_memory.is_some());

        let contradictions = fixture
            .store
            .list_contradictions(Some(ResolutionStatus::Unresolved))
            .await
            .expect("list contradictions");
        assert_eq!(contradictions.len(), 1);
        assert_eq!(contradictions[0].memory_a_id, trusted_id);
        assert_eq!(contradictions[0].memory_b_id, less_trusted_id);

        let downgraded = fixture
            .store
            .get_raw(&less_trusted_id)
            .await
            .expect("get downgraded memory")
            .expect("memory exists");
        assert!((downgraded.reliability_score - 0.4).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn search_uses_keyword_fts_and_updates_access_tracking() {
        let fixture = test_fixture();

        let mut active_match = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        active_match.content = "Apollo launch checklist and mission notes".to_string();
        active_match.tags = vec!["apollo".to_string(), "launch".to_string()];
        let active_match_id = active_match.id;

        let mut dormant_match = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        dormant_match.content = "Apollo archive notes".to_string();
        dormant_match.state = MemoryState::Dormant;

        let mut active_miss = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        active_miss.content = "Garden irrigation instructions".to_string();

        fixture
            .store
            .store(active_match)
            .await
            .expect("store active keyword match");
        fixture
            .store
            .store(dormant_match)
            .await
            .expect("store dormant keyword match");
        fixture
            .store
            .store(active_miss)
            .await
            .expect("store active non-match");

        let results = fixture
            .store
            .search(SearchQuery {
                text: "apollo launch".to_string(),
                embedding: None,
                scope: MemoryScope::Workspace,
                state_filter: None,
                type_filter: None,
                max_results: 5,
                context_config: None,
                session_id: None,
                agent_id: None,
            })
            .await
            .expect("run keyword search");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memory.id, active_match_id);
        assert!(results[0].similarity > 0.0);
        assert_eq!(results[0].memory.access_count, 1);
        assert!(results[0].memory.last_accessed_at.is_some());
    }

    #[tokio::test]
    async fn find_similar_returns_active_embedding_matches_without_touching_access() {
        let fixture = test_fixture();

        let active_match = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let active_match_id = active_match.id;
        let mut dormant_match = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        dormant_match.state = MemoryState::Dormant;
        let dormant_match_id = dormant_match.id;
        let active_far = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let active_far_id = active_far.id;

        fixture
            .store
            .store(active_match)
            .await
            .expect("store active vector match");
        fixture
            .store
            .store(dormant_match)
            .await
            .expect("store dormant vector match");
        fixture
            .store
            .store(active_far)
            .await
            .expect("store active far vector");

        fixture
            .store
            .store_embedding(&active_match_id, &[1.0; 768])
            .await
            .expect("store active embedding");
        fixture
            .store
            .store_embedding(&dormant_match_id, &[1.0; 768])
            .await
            .expect("store dormant embedding");
        fixture
            .store
            .store_embedding(&active_far_id, &[0.0; 768])
            .await
            .expect("store far embedding");

        let results = fixture
            .store
            .find_similar(&[1.0; 768], 0.95, 5)
            .await
            .expect("find similar");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memory.id, active_match_id);
        assert!((results[0].similarity - 1.0).abs() < f32::EPSILON);

        let persisted = fixture
            .store
            .get_raw(&active_match_id)
            .await
            .expect("reload active match")
            .expect("active match exists");
        assert_eq!(persisted.access_count, 0);
        assert!(persisted.last_accessed_at.is_none());
    }

    #[tokio::test]
    async fn search_combines_semantic_and_priority_signals_for_ordering() {
        let fixture = test_fixture();

        let mut keyword_only = sample_memory(MemoryScope::Workspace, ProvenanceLevel::Imported);
        keyword_only.content = "release checklist for apollo deployment".to_string();
        keyword_only.importance_score = 0.2;
        keyword_only.reliability_score = 0.2;
        let keyword_only_id = keyword_only.id;

        let mut semantic_priority =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        semantic_priority.content = "deployment runbook for launch window".to_string();
        semantic_priority.importance_score = 1.0;
        semantic_priority.reliability_score = 1.0;
        semantic_priority.access_count = 5;
        let semantic_priority_id = semantic_priority.id;

        fixture
            .store
            .store(keyword_only)
            .await
            .expect("store keyword-only memory");
        fixture
            .store
            .store(semantic_priority)
            .await
            .expect("store semantic-priority memory");

        let mut weak_embedding = vec![1.0_f32; 384];
        weak_embedding.extend(vec![-1.0_f32; 384]);
        fixture
            .store
            .store_embedding(&keyword_only_id, &weak_embedding)
            .await
            .expect("store weak semantic embedding");
        fixture
            .store
            .store_embedding(&semantic_priority_id, &[1.0; 768])
            .await
            .expect("store strong semantic embedding");

        fixture
            .store
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE memories SET access_count = 5, last_accessed_at = ?2 WHERE id = ?1",
                    [
                        semantic_priority_id.to_string(),
                        super::format_timestamp(Utc::now()),
                    ],
                )?;
                connection.execute(
                    "UPDATE memories SET access_count = 0, last_accessed_at = ?2 WHERE id = ?1",
                    [
                        keyword_only_id.to_string(),
                        super::format_timestamp(Utc::now()),
                    ],
                )?;
                Ok(())
            })
            .expect("seed deterministic access counts");

        let results = fixture
            .store
            .search(SearchQuery {
                text: "apollo deployment".to_string(),
                embedding: Some(vec![1.0; 768]),
                scope: MemoryScope::Workspace,
                state_filter: None,
                type_filter: None,
                max_results: 5,
                context_config: None,
                session_id: None,
                agent_id: None,
            })
            .await
            .expect("run hybrid search");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].memory.id, semantic_priority_id);
        assert_eq!(results[1].memory.id, keyword_only_id);
        assert!(results[0].score > results[1].score);
        assert!(results[0].similarity >= results[1].similarity);
    }

    #[tokio::test]
    async fn search_prefers_higher_similarity_over_higher_importance() {
        let fixture = test_fixture();

        let mut higher_similarity =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        higher_similarity.content = "higher semantic match".to_string();
        higher_similarity.importance_score = 0.5;
        higher_similarity.reliability_score = 1.0;
        let higher_similarity_id = higher_similarity.id;

        let mut higher_importance =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        higher_importance.content = "lower semantic match".to_string();
        higher_importance.importance_score = 0.8;
        higher_importance.reliability_score = 1.0;
        let higher_importance_id = higher_importance.id;

        fixture
            .store
            .store(higher_similarity)
            .await
            .expect("store higher-similarity memory");
        fixture
            .store
            .store(higher_importance)
            .await
            .expect("store higher-importance memory");

        fixture
            .store
            .store_embedding(&higher_similarity_id, &embedding_with_similarity(0.9))
            .await
            .expect("store higher-similarity embedding");
        fixture
            .store
            .store_embedding(&higher_importance_id, &embedding_with_similarity(0.5))
            .await
            .expect("store higher-importance embedding");

        fixture
            .store
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE scope_config SET value = '0.0' WHERE key IN ('recency_weight', 'access_weight')",
                    [],
                )?;
                connection.execute(
                    "UPDATE memories SET access_count = 0, last_accessed_at = NULL WHERE id IN (?1, ?2)",
                    [higher_similarity_id.to_string(), higher_importance_id.to_string()],
                )?;
                Ok(())
            })
            .expect("isolate similarity-vs-priority scoring");

        let results = fixture
            .store
            .search(SearchQuery {
                text: String::new(),
                embedding: Some(query_embedding()),
                scope: MemoryScope::Workspace,
                state_filter: None,
                type_filter: None,
                max_results: 5,
                context_config: None,
                session_id: None,
                agent_id: None,
            })
            .await
            .expect("run similarity-priority ordering search");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].memory.id, higher_similarity_id);
        assert_eq!(results[1].memory.id, higher_importance_id);
        assert!((results[0].similarity - 0.9).abs() < 1e-5);
        assert!((results[1].similarity - 0.5).abs() < 1e-5);
        assert!(results[0].score > results[1].score);
    }

    #[tokio::test]
    async fn sequential_search_dampens_access_driven_hubness() {
        let fixture = test_fixture();

        let mut m02 = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        m02.content = "alternate Windows target path".to_string();
        let m02_id = m02.id;

        let mut m05 = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        m05.content = "audit JSONL log location".to_string();
        let m05_id = m05.id;

        let mut m07 = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        m07.content = "system docs authority".to_string();
        let m07_id = m07.id;

        let mut m08 = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        m08.content = "git identity hooks".to_string();
        let m08_id = m08.id;

        for memory in [m02, m05, m07, m08] {
            fixture.store.store(memory).await.expect("store memory");
        }

        fixture
            .store
            .store_embedding(
                &m02_id,
                &embedding_with_query_similarities(&[0.65, 0.25, 0.40, 0.35]),
            )
            .await
            .expect("store m02 embedding");
        fixture
            .store
            .store_embedding(
                &m05_id,
                &embedding_with_query_similarities(&[0.25, 0.65, 0.40, 0.30]),
            )
            .await
            .expect("store m05 embedding");
        fixture
            .store
            .store_embedding(
                &m07_id,
                &embedding_with_query_similarities(&[0.20, 0.20, 0.75, 0.20]),
            )
            .await
            .expect("store m07 embedding");
        fixture
            .store
            .store_embedding(
                &m08_id,
                &embedding_with_query_similarities(&[0.20, 0.20, 0.20, 0.70]),
            )
            .await
            .expect("store m08 embedding");

        for query_index in 0..3 {
            let results = fixture
                .store
                .search(SearchQuery {
                    text: String::new(),
                    embedding: Some(query_basis_embedding(query_index, 4)),
                    scope: MemoryScope::Workspace,
                    state_filter: None,
                    type_filter: None,
                    max_results: 3,
                    context_config: None,
                    session_id: None,
                    agent_id: None,
                })
                .await
                .expect("run warm-up search");
            assert_eq!(results.len(), 3);
        }

        let final_results = fixture
            .store
            .search(SearchQuery {
                text: String::new(),
                embedding: Some(query_basis_embedding(3, 4)),
                scope: MemoryScope::Workspace,
                state_filter: None,
                type_filter: None,
                max_results: 4,
                context_config: None,
                session_id: None,
                agent_id: None,
            })
            .await
            .expect("run target search");

        assert_eq!(final_results[0].memory.id, m08_id);
        assert_eq!(final_results[1].memory.id, m02_id);
        assert_eq!(final_results[2].memory.id, m05_id);
        assert!(
            final_results[0].score > final_results[1].score,
            "semantic target should stay ahead of warmed-up hub candidates"
        );

        let warmed_hub = fixture
            .store
            .get_raw(&m02_id)
            .await
            .expect("reload m02")
            .expect("m02 exists");
        assert_eq!(warmed_hub.access_count, 4);
    }

    #[tokio::test]
    async fn fr_q07_hot_canary_keeps_semantic_winner_ahead_outside_similarity_band() {
        let fixture = test_fixture();
        set_scope_config(&fixture.store, "access_weight", "0.45");

        let mut target = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        target.content =
            "Une grande ouverture a f1.8 garde le visage net et floute le fond.".to_string();
        let target_id = target.id;

        let mut hub = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        hub.content = "Le cafe filtre V60 coule lentement a travers un papier rince.".to_string();
        let hub_id = hub.id;

        let mut portrait_alt = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        portrait_alt.content = "Le 85 mm isole le sujet pour un portrait flatteur.".to_string();
        let portrait_alt_id = portrait_alt.id;

        let mut soup = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        soup.content = "La soupe miso du soir melange dashi, tofu et algues.".to_string();
        let soup_id = soup.id;

        for memory in [target, hub, portrait_alt, soup] {
            fixture
                .store
                .store(memory)
                .await
                .expect("store canary memory");
        }

        fixture
            .store
            .store_embedding(
                &target_id,
                &embedding_with_query_similarities(&[
                    0.05,
                    0.05,
                    0.05,
                    0.05,
                    0.05,
                    0.05,
                    0.614_608_9,
                ]),
            )
            .await
            .expect("store target embedding");
        fixture
            .store
            .store_embedding(
                &hub_id,
                &embedding_with_query_similarities(&[
                    0.30,
                    0.29,
                    0.28,
                    0.27,
                    0.26,
                    0.25,
                    0.583_138_6,
                ]),
            )
            .await
            .expect("store hub embedding");
        fixture
            .store
            .store_embedding(
                &portrait_alt_id,
                &embedding_with_query_similarities(&[
                    0.10,
                    0.10,
                    0.10,
                    0.10,
                    0.10,
                    0.10,
                    0.594_978_6,
                ]),
            )
            .await
            .expect("store alternate portrait embedding");
        fixture
            .store
            .store_embedding(
                &soup_id,
                &embedding_with_query_similarities(&[
                    0.08,
                    0.08,
                    0.08,
                    0.08,
                    0.08,
                    0.08,
                    0.563_142_36,
                ]),
            )
            .await
            .expect("store soup embedding");

        for query_index in 0..6 {
            let warm_results = fixture
                .store
                .search(SearchQuery {
                    text: String::new(),
                    embedding: Some(query_basis_embedding(query_index, 7)),
                    scope: MemoryScope::Workspace,
                    state_filter: None,
                    type_filter: None,
                    max_results: 4,
                    context_config: None,
                    session_id: None,
                    agent_id: None,
                })
                .await
                .expect("run warm-up search");
            assert_eq!(warm_results[0].memory.id, hub_id);
        }

        fixture
            .store
            .with_connection(|connection| {
                let now = super::format_timestamp(Utc::now());
                connection.execute(
                    "UPDATE memories SET access_count = 6, last_accessed_at = ?2 WHERE id = ?1",
                    [hub_id.to_string(), now.clone()],
                )?;
                for memory_id in [target_id, portrait_alt_id, soup_id] {
                    connection.execute(
                        "UPDATE memories SET access_count = 0, last_accessed_at = ?2 WHERE id = ?1",
                        [memory_id.to_string(), now.clone()],
                    )?;
                }
                Ok(())
            })
            .expect("freeze hot canary access state");

        let ranked = rank_search_with_embedding(&fixture.store, query_basis_embedding(6, 7), 5);
        let target_ranked = find_ranked_candidate(&ranked, &target_id);
        let hub_ranked = find_ranked_candidate(&ranked, &hub_id);

        assert_eq!(ranked[0].0.memory.id, target_id);
        assert!(target_ranked.0.similarity > hub_ranked.0.similarity);
        assert!(
            (target_ranked.0.similarity - hub_ranked.0.similarity)
                > super::RETRIEVAL_SIMILARITY_TIE_THRESHOLD
        );
        assert!(
            target_ranked.1.total_score < hub_ranked.1.total_score,
            "raw score should still favor the warmed hub to prove the band guard is active"
        );
        assert!(target_ranked.0.score > hub_ranked.0.score);
        assert_eq!(target_ranked.1.similarity_band_index, 0);
        assert!(hub_ranked.1.similarity_band_index > target_ranked.1.similarity_band_index);
    }

    #[tokio::test]
    async fn similarity_band_blocks_recency_only_overrides_outside_threshold() {
        let fixture = test_fixture();
        set_scope_config(&fixture.store, "similarity_weight", "0.4");
        set_scope_config(&fixture.store, "recency_weight", "1.0");
        set_scope_config(&fixture.store, "access_weight", "0.0");
        set_scope_config(&fixture.store, "priority_weight", "0.0");

        let mut semantic_winner =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        semantic_winner.content = "semantic winner".to_string();
        let semantic_winner_id = semantic_winner.id;

        let mut fresh_runner_up =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        fresh_runner_up.content = "fresh runner up".to_string();
        let fresh_runner_up_id = fresh_runner_up.id;

        fixture
            .store
            .store(semantic_winner)
            .await
            .expect("store semantic winner");
        fixture
            .store
            .store(fresh_runner_up)
            .await
            .expect("store fresh runner up");

        fixture
            .store
            .store_embedding(&semantic_winner_id, &embedding_with_similarity(0.64))
            .await
            .expect("store semantic winner embedding");
        fixture
            .store
            .store_embedding(&fresh_runner_up_id, &embedding_with_similarity(0.60))
            .await
            .expect("store fresh runner up embedding");

        fixture
            .store
            .with_connection(|connection| {
                let now = Utc::now();
                let stale_time = now - chrono::Duration::days(30);
                connection.execute(
                    "UPDATE memories SET updated_at = ?2, last_accessed_at = ?2, access_count = 0 WHERE id = ?1",
                    [semantic_winner_id.to_string(), super::format_timestamp(stale_time)],
                )?;
                connection.execute(
                    "UPDATE memories SET updated_at = ?2, last_accessed_at = ?2, access_count = 0 WHERE id = ?1",
                    [fresh_runner_up_id.to_string(), super::format_timestamp(now)],
                )?;
                Ok(())
            })
            .expect("seed recency inversion fixture");

        let ranked = rank_search_with_embedding(&fixture.store, query_embedding(), 5);
        let semantic_ranked = find_ranked_candidate(&ranked, &semantic_winner_id);
        let fresh_ranked = find_ranked_candidate(&ranked, &fresh_runner_up_id);

        assert_eq!(ranked[0].0.memory.id, semantic_winner_id);
        assert!(semantic_ranked.0.similarity > fresh_ranked.0.similarity);
        assert!(
            (semantic_ranked.0.similarity - fresh_ranked.0.similarity)
                > super::RETRIEVAL_SIMILARITY_TIE_THRESHOLD
        );
        assert!(
            semantic_ranked.1.total_score < fresh_ranked.1.total_score,
            "raw score should favor recency to prove the structural guard is active"
        );
        assert!(semantic_ranked.0.score > fresh_ranked.0.score);
    }

    #[tokio::test]
    async fn similarity_band_still_allows_recency_refinement_inside_threshold() {
        let fixture = test_fixture();
        set_scope_config(&fixture.store, "similarity_weight", "0.4");
        set_scope_config(&fixture.store, "recency_weight", "1.0");
        set_scope_config(&fixture.store, "access_weight", "0.0");
        set_scope_config(&fixture.store, "priority_weight", "0.0");

        let mut slightly_better_but_old =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        slightly_better_but_old.content = "slightly better but old".to_string();
        let slightly_better_but_old_id = slightly_better_but_old.id;

        let mut fresh_quasi_tie =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        fresh_quasi_tie.content = "fresh quasi tie".to_string();
        let fresh_quasi_tie_id = fresh_quasi_tie.id;

        fixture
            .store
            .store(slightly_better_but_old)
            .await
            .expect("store old candidate");
        fixture
            .store
            .store(fresh_quasi_tie)
            .await
            .expect("store fresh candidate");

        fixture
            .store
            .store_embedding(
                &slightly_better_but_old_id,
                &embedding_with_similarity(0.62),
            )
            .await
            .expect("store old candidate embedding");
        fixture
            .store
            .store_embedding(&fresh_quasi_tie_id, &embedding_with_similarity(0.605))
            .await
            .expect("store fresh candidate embedding");

        fixture
            .store
            .with_connection(|connection| {
                let now = Utc::now();
                let stale_time = now - chrono::Duration::days(30);
                connection.execute(
                    "UPDATE memories SET updated_at = ?2, last_accessed_at = ?2, access_count = 0 WHERE id = ?1",
                    [
                        slightly_better_but_old_id.to_string(),
                        super::format_timestamp(stale_time),
                    ],
                )?;
                connection.execute(
                    "UPDATE memories SET updated_at = ?2, last_accessed_at = ?2, access_count = 0 WHERE id = ?1",
                    [fresh_quasi_tie_id.to_string(), super::format_timestamp(now)],
                )?;
                Ok(())
            })
            .expect("seed quasi-tie recency fixture");

        let ranked = rank_search_with_embedding(&fixture.store, query_embedding(), 5);
        let old_ranked = find_ranked_candidate(&ranked, &slightly_better_but_old_id);
        let fresh_ranked = find_ranked_candidate(&ranked, &fresh_quasi_tie_id);

        assert_eq!(ranked[0].0.memory.id, fresh_quasi_tie_id);
        assert!(
            (old_ranked.0.similarity - fresh_ranked.0.similarity)
                < super::RETRIEVAL_SIMILARITY_TIE_THRESHOLD
        );
        assert_eq!(
            old_ranked.1.similarity_band_index,
            fresh_ranked.1.similarity_band_index
        );
    }

    #[tokio::test]
    async fn similarity_band_blocks_priority_only_overrides_outside_threshold() {
        let fixture = test_fixture();
        set_scope_config(&fixture.store, "similarity_weight", "0.1");
        set_scope_config(&fixture.store, "recency_weight", "0.0");
        set_scope_config(&fixture.store, "access_weight", "0.0");
        set_scope_config(&fixture.store, "priority_weight", "0.9");

        let mut semantic_winner =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        semantic_winner.content = "semantic winner".to_string();
        semantic_winner.importance_score = 0.5;
        semantic_winner.reliability_score = 1.0;
        let semantic_winner_id = semantic_winner.id;

        let mut boosted_runner_up =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        boosted_runner_up.content = "boosted runner up".to_string();
        boosted_runner_up.importance_score = 1.0;
        boosted_runner_up.reliability_score = 1.0;
        let boosted_runner_up_id = boosted_runner_up.id;

        fixture
            .store
            .store(semantic_winner)
            .await
            .expect("store semantic winner");
        fixture
            .store
            .store(boosted_runner_up)
            .await
            .expect("store boosted runner up");

        fixture
            .store
            .store_embedding(&semantic_winner_id, &embedding_with_similarity(0.64))
            .await
            .expect("store semantic winner embedding");
        fixture
            .store
            .store_embedding(&boosted_runner_up_id, &embedding_with_similarity(0.60))
            .await
            .expect("store boosted runner up embedding");

        let ranked = rank_search_with_embedding(&fixture.store, query_embedding(), 5);
        let semantic_ranked = find_ranked_candidate(&ranked, &semantic_winner_id);
        let boosted_ranked = find_ranked_candidate(&ranked, &boosted_runner_up_id);

        assert_eq!(ranked[0].0.memory.id, semantic_winner_id);
        assert!(semantic_ranked.0.similarity > boosted_ranked.0.similarity);
        assert!(
            (semantic_ranked.0.similarity - boosted_ranked.0.similarity)
                > super::RETRIEVAL_SIMILARITY_TIE_THRESHOLD
        );
        assert!(
            semantic_ranked.1.total_score < boosted_ranked.1.total_score,
            "raw score should favor the priority-heavy runner up to prove the structural guard is active"
        );
        assert!(semantic_ranked.0.score > boosted_ranked.0.score);
    }

    #[test]
    fn learned_weight_clamp_keeps_access_within_safe_ceiling_after_normalization() {
        let clamped = super::LearnedWeightValues {
            similarity_weight: 0.01,
            recency_weight: 0.01,
            access_weight: 10.0,
            priority_weight: 0.01,
        }
        .clamp();

        assert!(
            (clamped.similarity_weight
                + clamped.recency_weight
                + clamped.access_weight
                + clamped.priority_weight
                - 1.0)
                .abs()
                < 1.0e-9
        );
        assert!(clamped.access_weight <= super::LEARNING_ACCESS_WEIGHT_CEILING + 1.0e-9);
    }

    #[test]
    fn retrieval_score_breakdown_can_neutralize_non_similarity_contributions() {
        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let now = Utc::now();
        memory.importance_score = 0.9;
        memory.reliability_score = 0.95;
        memory.access_count = 9;
        memory.updated_at = now;
        memory.last_accessed_at = Some(now);

        let default_breakdown = super::compute_retrieval_score_breakdown_with_mode(
            &memory,
            Some(0.8),
            Some(0.2),
            &ScopeConfig::default(),
            now,
            super::RetrievalScoringMode::Default,
        );
        let similarity_only_breakdown = super::compute_retrieval_score_breakdown_with_mode(
            &memory,
            Some(0.8),
            Some(0.2),
            &ScopeConfig::default(),
            now,
            super::RetrievalScoringMode::SimilarityOnly,
        );

        assert!(default_breakdown.weighted_similarity > 0.0);
        assert!(default_breakdown.weighted_recency > 0.0);
        assert!(default_breakdown.weighted_access > 0.0);
        assert!(default_breakdown.weighted_priority > 0.0);
        assert_eq!(
            similarity_only_breakdown.blended_similarity,
            default_breakdown.blended_similarity
        );
        assert_eq!(
            similarity_only_breakdown.weighted_similarity,
            default_breakdown.weighted_similarity
        );
        assert_eq!(similarity_only_breakdown.weighted_recency, 0.0);
        assert_eq!(similarity_only_breakdown.weighted_access, 0.0);
        assert_eq!(similarity_only_breakdown.weighted_priority, 0.0);
        assert_eq!(
            similarity_only_breakdown.total_score,
            similarity_only_breakdown.weighted_similarity
        );
    }

    #[tokio::test]
    async fn realistic_benchmark_similarity_only_mode_surfaces_expected_targets() {
        for case in realistic_benchmark_query_cases() {
            let fixture = test_fixture();
            let labels_by_id = seed_realistic_benchmark_case(&fixture, case).await;

            let default_results = rank_realistic_benchmark_case(
                &fixture.store,
                case.query,
                super::RetrievalScoringMode::Default,
            );
            let similarity_only_results = rank_realistic_benchmark_case(
                &fixture.store,
                case.query,
                super::RetrievalScoringMode::SimilarityOnly,
            );

            let similarity_only_top_label =
                label_for_scored_memory(&labels_by_id, &similarity_only_results[0].0);

            assert_eq!(
                similarity_only_top_label, case.target_label,
                "similarity-only mode should surface the target for {}",
                case.label
            );

            let default_target_rank =
                rank_for_label(&default_results, &labels_by_id, case.target_label);
            let similarity_only_target_rank =
                rank_for_label(&similarity_only_results, &labels_by_id, case.target_label);
            assert!(
                similarity_only_target_rank <= default_target_rank,
                "expected target rank to improve or stay stable for {} (default={}, similarity-only={})",
                case.label,
                default_target_rank,
                similarity_only_target_rank
            );

            if default_target_rank > 1 {
                let default_top_similarity = default_results[0].0.similarity;
                let target_similarity =
                    similarity_for_label(&default_results, &labels_by_id, case.target_label);
                assert!(
                    target_similarity > default_top_similarity,
                    "expected target similarity to beat the default top score for {}",
                    case.label
                );
            }
        }
    }

    #[tokio::test]
    async fn search_uses_scope_configured_fixed_decay_for_recency_ordering() {
        let fixture = test_fixture();

        let mut recent = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        recent.content = "apollo recency note".to_string();
        let recent_id = recent.id;

        let mut stale = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        stale.content = "apollo recency note".to_string();
        let stale_id = stale.id;

        fixture
            .store
            .store(recent)
            .await
            .expect("store recent memory");
        fixture
            .store
            .store(stale)
            .await
            .expect("store stale memory");

        fixture
            .store
            .with_connection(|connection| {
                let now = Utc::now();
                let stale_time = now - chrono::Duration::days(10);

                connection.execute(
                    "UPDATE scope_config SET value = '0.0' WHERE key IN ('similarity_weight', 'access_weight', 'priority_weight')",
                    [],
                )?;
                connection.execute(
                    "UPDATE scope_config SET value = '1.0' WHERE key = 'recency_weight'",
                    [],
                )?;
                connection.execute(
                    "UPDATE scope_config SET value = '0.5' WHERE key = 'decay_lambda_base'",
                    [],
                )?;
                connection.execute(
                    "UPDATE memories SET last_accessed_at = ?2, updated_at = ?2, access_count = 0 WHERE id = ?1",
                    [recent_id.to_string(), super::format_timestamp(now)],
                )?;
                connection.execute(
                    "UPDATE memories SET last_accessed_at = ?2, updated_at = ?2, access_count = 0 WHERE id = ?1",
                    [stale_id.to_string(), super::format_timestamp(stale_time)],
                )?;
                Ok(())
            })
            .expect("seed recency ordering fixture");

        let results = fixture
            .store
            .search(SearchQuery {
                text: "apollo recency".to_string(),
                embedding: None,
                scope: MemoryScope::Workspace,
                state_filter: None,
                type_filter: None,
                max_results: 5,
                context_config: None,
                session_id: None,
                agent_id: None,
            })
            .await
            .expect("run recency-focused search");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].memory.id, recent_id);
        assert_eq!(results[1].memory.id, stale_id);
        assert!(results[0].score > results[1].score);
    }

    #[tokio::test]
    async fn search_respects_type_and_state_filters() {
        let fixture = test_fixture();

        let mut active_decision =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        active_decision.content = "migration decision for apollo workspace".to_string();
        active_decision.memory_type = MemoryType::Decision;
        let active_decision_id = active_decision.id;

        let mut active_fact = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        active_fact.content = "apollo workspace fact sheet".to_string();
        active_fact.memory_type = MemoryType::Fact;

        let mut dormant_decision =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        dormant_decision.content = "old apollo decision archive".to_string();
        dormant_decision.memory_type = MemoryType::Decision;
        dormant_decision.state = MemoryState::Dormant;
        let dormant_decision_id = dormant_decision.id;

        fixture
            .store
            .store(active_decision)
            .await
            .expect("store active decision");
        fixture
            .store
            .store(active_fact)
            .await
            .expect("store active fact");
        fixture
            .store
            .store(dormant_decision)
            .await
            .expect("store dormant decision");

        let active_results = fixture
            .store
            .search(SearchQuery {
                text: "apollo decision".to_string(),
                embedding: None,
                scope: MemoryScope::Workspace,
                state_filter: None,
                type_filter: Some(vec![MemoryType::Decision]),
                max_results: 5,
                context_config: None,
                session_id: None,
                agent_id: None,
            })
            .await
            .expect("run active decision search");
        assert_eq!(active_results.len(), 1);
        assert_eq!(active_results[0].memory.id, active_decision_id);

        let dormant_results = fixture
            .store
            .search(SearchQuery {
                text: "apollo decision".to_string(),
                embedding: None,
                scope: MemoryScope::Workspace,
                state_filter: Some(MemoryState::Dormant),
                type_filter: Some(vec![MemoryType::Decision]),
                max_results: 5,
                context_config: None,
                session_id: None,
                agent_id: None,
            })
            .await
            .expect("run dormant decision search");
        assert_eq!(dormant_results.len(), 1);
        assert_eq!(dormant_results[0].memory.id, dormant_decision_id);
    }

    #[tokio::test]
    async fn store_with_embedding_provider_persists_embedding_automatically() {
        let memory_content = "provider-backed storage memory";
        let provider = Arc::new(StubEmbeddingProvider::new([(
            memory_content,
            StubEmbeddingResponse::Embedding(vec![1.0; 768]),
        )]));
        let fixture = test_fixture_with_provider(provider.clone());

        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        memory.content = memory_content.to_string();
        let id = memory.id;

        fixture
            .store
            .store(memory)
            .await
            .expect("store memory with automatic embedding");

        let persisted = fixture
            .store
            .get_raw(&id)
            .await
            .expect("reload stored memory")
            .expect("memory exists");
        assert!(!persisted.embedding_stale);

        let (embedding_rows, vector_rows) = fixture
            .store
            .with_connection(|connection| {
                let embedding_rows = connection.query_row(
                    "SELECT COUNT(*) FROM memory_embeddings WHERE memory_id = ?1",
                    [id.to_string()],
                    |row| row.get::<_, i64>(0),
                )?;
                let vector_rows =
                    connection.query_row("SELECT COUNT(*) FROM vec_memories", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                Ok((embedding_rows, vector_rows))
            })
            .expect("load automatic embedding counts");

        assert_eq!(embedding_rows, 1);
        assert_eq!(vector_rows, 1);
        assert_eq!(provider.calls(), vec![memory_content.to_string()]);
    }

    #[tokio::test]
    async fn store_with_duplicate_content_reuses_cached_embedding_without_reembedding() {
        let memory_content = "provider-backed cached storage memory";
        let provider = Arc::new(StubEmbeddingProvider::new([(
            memory_content,
            StubEmbeddingResponse::Embedding(vec![1.0; 768]),
        )]));
        let fixture = test_fixture_with_provider(provider.clone());

        let mut first = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        first.content = memory_content.to_string();
        let first_id = first.id;

        fixture
            .store
            .store(first)
            .await
            .expect("store first cached memory");

        let mut second = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        second.content = memory_content.to_string();
        let second_id = second.id;

        fixture
            .store
            .store(second)
            .await
            .expect("store second cached memory");

        let (embedding_rows, vector_rows, cached_hashes) = fixture
            .store
            .with_connection(|connection| {
                let embedding_rows =
                    connection.query_row("SELECT COUNT(*) FROM memory_embeddings", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                let vector_rows =
                    connection.query_row("SELECT COUNT(*) FROM vec_memories", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                let mut statement = connection.prepare(
                    "SELECT content_sha256 FROM memory_embeddings WHERE memory_id IN (?1, ?2) ORDER BY memory_id",
                )?;
                let hashes = statement
                    .query_map(params![first_id.to_string(), second_id.to_string()], |row| {
                        row.get::<_, Option<String>>(0)
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((embedding_rows, vector_rows, hashes))
            })
            .expect("load duplicate cached embedding state");

        assert_eq!(provider.call_count(), 1);
        assert_eq!(provider.calls(), vec![memory_content.to_string()]);
        assert_eq!(embedding_rows, 2);
        assert_eq!(vector_rows, 2);
        assert_eq!(cached_hashes.len(), 2);
        assert_eq!(cached_hashes[0], cached_hashes[1]);
        assert!(cached_hashes[0].is_some());
        assert!(
            !fixture
                .store
                .get_raw(&second_id)
                .await
                .expect("reload cached memory")
                .expect("cached memory exists")
                .embedding_stale
        );
    }

    #[tokio::test]
    async fn store_ignores_stale_cached_embeddings_for_changed_content() {
        let original_content = "cached content before update";
        let updated_content = "changed content after update";
        let provider = Arc::new(StubEmbeddingProvider::new([
            (
                original_content,
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
            (
                updated_content,
                StubEmbeddingResponse::Embedding(vec![0.5; 768]),
            ),
        ]));
        let fixture = test_fixture_with_provider(provider.clone());

        let mut first = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        first.content = original_content.to_string();
        let first_id = first.id;

        fixture
            .store
            .store(first)
            .await
            .expect("store original memory");
        fixture
            .store
            .update_content(&first_id, updated_content, "editor", "content changed")
            .await
            .expect("update original memory content");

        let mut second = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        second.content = original_content.to_string();

        fixture
            .store
            .store(second)
            .await
            .expect("store second memory with original content");

        assert_eq!(
            provider.calls(),
            vec![original_content.to_string(), original_content.to_string(),]
        );
    }

    #[tokio::test]
    async fn search_with_provider_auto_generates_query_embedding_when_missing() {
        let semantic_query = "semantic probe";
        let semantic_match_content = "release readiness checklist";
        let non_match_content = "garden watering schedule";
        let mut weak_embedding = vec![1.0_f32; 384];
        weak_embedding.extend(vec![-1.0_f32; 384]);
        let provider = Arc::new(StubEmbeddingProvider::new([
            (
                semantic_match_content,
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
            (
                non_match_content,
                StubEmbeddingResponse::Embedding(weak_embedding),
            ),
            (
                semantic_query,
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
        ]));
        let fixture = test_fixture_with_provider(provider.clone());

        let mut semantic_match = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        semantic_match.content = semantic_match_content.to_string();
        let semantic_match_id = semantic_match.id;

        let mut non_match = sample_memory(MemoryScope::Workspace, ProvenanceLevel::Imported);
        non_match.content = non_match_content.to_string();

        fixture
            .store
            .store(semantic_match)
            .await
            .expect("store semantic match");
        fixture
            .store
            .store(non_match)
            .await
            .expect("store semantic non-match");

        let results = fixture
            .store
            .search(SearchQuery {
                text: semantic_query.to_string(),
                embedding: None,
                scope: MemoryScope::Workspace,
                state_filter: None,
                type_filter: None,
                max_results: 5,
                context_config: None,
                session_id: None,
                agent_id: None,
            })
            .await
            .expect("run provider-backed semantic search");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memory.id, semantic_match_id);
        assert!(results[0].similarity > 0.0);
        assert_eq!(
            provider.calls(),
            vec![
                semantic_match_content.to_string(),
                non_match_content.to_string(),
                semantic_query.to_string()
            ]
        );
    }

    #[tokio::test]
    async fn ollama_offline_store_keeps_memory_and_preserves_keyword_search_fallback() {
        let fallback_content = "apollo keyword fallback";
        let provider = Arc::new(StubEmbeddingProvider::new([(
            fallback_content,
            StubEmbeddingResponse::Failure(
                "ollama not reachable at http://127.0.0.1:11434: connection failed".to_string(),
            ),
        )]));
        let fixture = test_fixture_with_provider(provider.clone());

        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        memory.content = fallback_content.to_string();
        let id = memory.id;

        fixture
            .store
            .store(memory)
            .await
            .expect("store memory despite provider failure");

        let persisted = fixture
            .store
            .get_raw(&id)
            .await
            .expect("reload stored fallback memory")
            .expect("memory exists");
        assert!(persisted.embedding_stale);

        let (embedding_rows, vector_rows) = fixture
            .store
            .with_connection(|connection| {
                let embedding_rows = connection.query_row(
                    "SELECT COUNT(*) FROM memory_embeddings WHERE memory_id = ?1",
                    [id.to_string()],
                    |row| row.get::<_, i64>(0),
                )?;
                let vector_rows =
                    connection.query_row("SELECT COUNT(*) FROM vec_memories", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                Ok((embedding_rows, vector_rows))
            })
            .expect("load failed automatic embedding counts");

        assert_eq!(embedding_rows, 0);
        assert_eq!(vector_rows, 0);

        let results = fixture
            .store
            .search(SearchQuery {
                text: fallback_content.to_string(),
                embedding: None,
                scope: MemoryScope::Workspace,
                state_filter: None,
                type_filter: None,
                max_results: 5,
                context_config: None,
                session_id: None,
                agent_id: None,
            })
            .await
            .expect("run keyword fallback search");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memory.id, id);
        assert!(results[0].similarity > 0.0);
        assert_eq!(
            provider.calls(),
            vec![fallback_content.to_string(), fallback_content.to_string()]
        );
    }

    #[test]
    fn ollama_offline_errors_map_to_user_facing_degradation_warning() {
        let warning = super::embedding_degradation_warning(&EmbeddingError::Provider(
            "ollama not reachable at http://127.0.0.1:11434: request timed out after 30s"
                .to_string(),
        ))
        .expect("offline provider errors should produce a degradation warning");

        assert_eq!(
            warning,
            "Ollama not reachable at http://127.0.0.1:11434, storing without embeddings. Run reembed later."
        );

        let non_offline_warning = super::embedding_degradation_warning(&EmbeddingError::Provider(
            "ollama embeddings request returned 500 Internal Server Error: boom".to_string(),
        ));
        assert!(non_offline_warning.is_none());
    }

    #[test]
    fn openai_offline_errors_map_to_user_facing_degradation_warning() {
        let warning = super::embedding_degradation_warning(&EmbeddingError::Provider(
            "openai not reachable at https://api.openai.com: request timed out after 30s"
                .to_string(),
        ))
        .expect("offline openai errors should produce a degradation warning");

        assert_eq!(
            warning,
            "OpenAI not reachable at https://api.openai.com, storing without embeddings. Run reembed later."
        );

        let invalid_api_key_warning =
            super::embedding_degradation_warning(&EmbeddingError::Provider(
                "openai returned 401 Unauthorized: invalid API key (...)".to_string(),
            ))
            .expect("openai auth errors should produce a degradation warning");

        assert_eq!(
            invalid_api_key_warning,
            "OpenAI embeddings unavailable (401 Unauthorized: invalid API key), storing without embeddings. Run reembed later."
        );
    }

    #[test]
    fn openai_http_errors_map_to_user_facing_degradation_warning() {
        let rate_limit_warning = super::embedding_degradation_warning(&EmbeddingError::Provider(
            "openai returned 429 Too Many Requests: rate limited, try again later (...)"
                .to_string(),
        ))
        .expect("rate limit errors should produce a degradation warning");

        assert_eq!(
            rate_limit_warning,
            "OpenAI embeddings unavailable (429 Too Many Requests: rate limited, try again later), storing without embeddings. Run reembed later."
        );

        let server_error_warning = super::embedding_degradation_warning(&EmbeddingError::Provider(
            "openai embeddings request returned 500 Internal Server Error: boom".to_string(),
        ))
        .expect("openai http status errors should produce a degradation warning");

        assert_eq!(
            server_error_warning,
            "OpenAI embeddings unavailable (500 Internal Server Error), storing without embeddings. Run reembed later."
        );
    }

    #[tokio::test]
    async fn search_cascades_upward_while_list_stays_exact_scope() {
        let fixture = test_fixture();
        let session_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Session).expect("session store");
        let workspace_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Workspace).expect("workspace store");
        let user_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::User).expect("user store");
        let agent_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Agent).expect("agent store");

        let mut session = sample_memory(MemoryScope::Session, ProvenanceLevel::UserStated);
        session.content = "shared scope note session".to_string();
        let session_id = session.id;
        let mut workspace = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        workspace.content = "shared scope note workspace".to_string();
        let workspace_id = workspace.id;
        let mut user = sample_memory(MemoryScope::User, ProvenanceLevel::UserStated);
        user.content = "shared scope note user".to_string();
        let user_id = user.id;
        let mut agent = sample_memory(MemoryScope::Agent, ProvenanceLevel::UserStated);
        agent.content = "shared scope note agent".to_string();
        let agent_id = agent.id;

        session_store
            .store(session)
            .await
            .expect("store session memory");
        workspace_store
            .store(workspace)
            .await
            .expect("store workspace memory");
        user_store.store(user).await.expect("store user memory");
        agent_store.store(agent).await.expect("store agent memory");

        let search_results = session_store
            .search(SearchQuery {
                text: "shared scope note".to_string(),
                embedding: None,
                scope: MemoryScope::Session,
                state_filter: None,
                type_filter: None,
                max_results: 10,
                context_config: None,
                session_id: None,
                agent_id: None,
            })
            .await
            .expect("search visible scopes");
        let ids = search_results
            .iter()
            .map(|result| result.memory.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&session_id));
        assert!(ids.contains(&workspace_id));
        assert!(ids.contains(&user_id));
        assert!(ids.contains(&agent_id));

        let listed = session_store
            .list(MemoryFilter {
                scope: Some(MemoryScope::Session),
                ..MemoryFilter::default()
            })
            .await
            .expect("list exact session scope");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, session_id);
    }

    #[tokio::test]
    async fn find_similar_cascades_to_higher_visible_scopes() {
        let fixture = test_fixture();
        let session_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Session).expect("session store");
        let workspace_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Workspace).expect("workspace store");

        let workspace = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let workspace_id = workspace.id;
        workspace_store
            .store(workspace)
            .await
            .expect("store workspace memory");
        workspace_store
            .store_embedding(&workspace_id, &[1.0; 768])
            .await
            .expect("store workspace embedding");

        let matches = session_store
            .find_similar(&[1.0; 768], 0.95, 5)
            .await
            .expect("find visible similar memories");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].memory.id, workspace_id);
        assert_eq!(matches[0].memory.scope, MemoryScope::Workspace);
    }

    #[tokio::test]
    async fn search_promotes_session_memory_after_three_distinct_sessions_and_records_provenance() {
        let fixture = test_fixture();
        let session_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Session).expect("session store");
        let workspace_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Workspace).expect("workspace store");

        let mut memory = sample_memory(MemoryScope::Session, ProvenanceLevel::UserStated);
        memory.content = "promotion candidate memory".to_string();
        let id = memory.id;
        session_store
            .store(memory)
            .await
            .expect("store session memory");

        for session_id in [
            "00000000-0000-0000-0000-000000000001",
            "00000000-0000-0000-0000-000000000002",
            "00000000-0000-0000-0000-000000000003",
        ] {
            let _ = session_store
                .search(SearchQuery {
                    text: "promotion candidate".to_string(),
                    embedding: None,
                    scope: MemoryScope::Session,
                    state_filter: None,
                    type_filter: None,
                    max_results: 5,
                    context_config: None,
                    session_id: Some(session_id.to_string()),
                    agent_id: None,
                })
                .await
                .expect("search session memory");
        }

        let promoted = workspace_store
            .get_raw(&id)
            .await
            .expect("reload promoted memory")
            .expect("promoted memory exists");
        assert_eq!(promoted.scope, MemoryScope::Workspace);

        let (promotion_rows, version_rows) = workspace_store
            .with_connection(|connection| {
                let promotion_rows = connection.query_row(
                    "SELECT COUNT(*) FROM memory_promotions WHERE memory_id = ?1",
                    [id.to_string()],
                    |row| row.get::<_, i64>(0),
                )?;
                let version_rows = connection.query_row(
                    "SELECT COUNT(*) FROM memory_versions WHERE memory_id = ?1",
                    [id.to_string()],
                    |row| row.get::<_, i64>(0),
                )?;
                Ok((promotion_rows, version_rows))
            })
            .expect("load promotion provenance rows");
        assert_eq!(promotion_rows, 1);
        assert_eq!(version_rows, 1);
    }

    #[tokio::test]
    async fn promotion_pass_advances_corroborated_and_durable_memories_one_scope_only() {
        let fixture = test_fixture();
        let workspace_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Workspace).expect("workspace store");
        let user_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::User).expect("user store");
        let agent_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Agent).expect("agent store");

        let mut corroborated = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        corroborated.content = "corroborated promotion candidate".to_string();
        corroborated.corroboration_count = 2;
        let corroborated_id = corroborated.id;

        let mut durable = sample_memory(MemoryScope::User, ProvenanceLevel::UserStated);
        durable.content = "durable promotion candidate".to_string();
        durable.importance_score = 0.9;
        durable.updated_at = Utc::now() - chrono::Duration::days(8);
        durable.last_accessed_at = Some(Utc::now() - chrono::Duration::days(8));
        let durable_id = durable.id;

        let mut top_scope = sample_memory(MemoryScope::Agent, ProvenanceLevel::UserStated);
        top_scope.content = "top scope remains agent".to_string();
        top_scope.corroboration_count = 5;
        let top_scope_id = top_scope.id;

        workspace_store
            .store(corroborated)
            .await
            .expect("store corroborated memory");
        user_store
            .store(durable)
            .await
            .expect("store durable memory");
        agent_store
            .store(top_scope)
            .await
            .expect("store top-scope memory");

        let promoted = workspace_store
            .run_promotion_pass(None, None)
            .expect("run promotion pass");
        let promoted_ids = promoted.iter().map(|memory| memory.id).collect::<Vec<_>>();
        assert!(promoted_ids.contains(&corroborated_id));
        assert!(promoted_ids.contains(&durable_id));
        assert!(!promoted_ids.contains(&top_scope_id));

        let corroborated_promoted = user_store
            .get_raw(&corroborated_id)
            .await
            .expect("load corroborated promoted memory")
            .expect("corroborated memory exists");
        assert_eq!(corroborated_promoted.scope, MemoryScope::User);

        let durable_promoted = agent_store
            .get_raw(&durable_id)
            .await
            .expect("load durable promoted memory")
            .expect("durable memory exists");
        assert_eq!(durable_promoted.scope, MemoryScope::Agent);
    }

    struct TestFixture {
        store: SqliteMemoryStore,
        path: PathBuf,
    }

    impl Drop for TestFixture {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    fn test_fixture() -> TestFixture {
        let path = env::temp_dir().join(format!("elegy-memory-store-{}.sqlite3", Uuid::new_v4()));
        let store =
            SqliteMemoryStore::new(&path, MemoryScope::Workspace).expect("create sqlite store");
        TestFixture { store, path }
    }

    fn test_fixture_with_provider(provider: Arc<dyn EmbeddingProvider>) -> TestFixture {
        let path = env::temp_dir().join(format!("elegy-memory-store-{}.sqlite3", Uuid::new_v4()));
        let store =
            SqliteMemoryStore::new_with_embedding_provider(&path, MemoryScope::Workspace, provider)
                .expect("create sqlite store with embedding provider");
        TestFixture { store, path }
    }

    fn sample_memory(scope: MemoryScope, provenance: ProvenanceLevel) -> Memory {
        let now = Utc::now();
        Memory {
            id: Uuid::new_v4(),
            content: format!("memory {}", Uuid::new_v4()),
            summary: Some("summary".to_string()),
            scope,
            memory_type: MemoryType::Fact,
            provenance,
            importance_score: 0.8,
            reliability_score: provenance.base_reliability(),
            sensitivity: SensitivityLevel::Low,
            state: MemoryState::Active,
            tags: vec!["baseline".to_string()],
            status: None,
            custom_metadata: HashMap::new(),
            access_count: 0,
            corroboration_count: 0,
            embedding_stale: true,
            created_at: now,
            updated_at: now,
            last_accessed_at: None,
            tenant_id: None,
            user_id: Some("user-1".to_string()),
            agent_id: Some("agent-1".to_string()),
        }
    }

    fn set_scope_config(store: &SqliteMemoryStore, key: &str, value: &str) {
        store
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE scope_config SET value = ?2 WHERE key = ?1",
                    params![key, value],
                )?;
                Ok(())
            })
            .expect("update scope config");
    }

    #[derive(Clone, Copy)]
    struct BenchmarkMemorySpec {
        label: &'static str,
        content: &'static str,
        importance: f32,
        reliability: f32,
        access_count: u32,
    }

    #[derive(Clone, Copy)]
    struct BenchmarkQueryCase {
        label: &'static str,
        query: &'static str,
        target_label: &'static str,
    }

    async fn seed_realistic_benchmark_case(
        fixture: &TestFixture,
        case: BenchmarkQueryCase,
    ) -> HashMap<MemoryId, &'static str> {
        let mut labels_by_id = HashMap::new();
        let seeded_at = Utc::now();

        for spec in realistic_benchmark_memory_specs() {
            let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
            memory.content = spec.content.to_string();
            memory.summary = Some(format!("{} realistic benchmark note", spec.label));
            memory.importance_score = spec.importance;
            memory.reliability_score = spec.reliability;
            memory.access_count = spec.access_count;
            memory.updated_at = seeded_at;
            memory.last_accessed_at = Some(seeded_at);
            memory.tags = vec!["benchmark".to_string(), case.label.to_string()];

            let id = memory.id;
            fixture
                .store
                .store(memory)
                .await
                .expect("store realistic benchmark memory");
            fixture
                .store
                .store_embedding(
                    &id,
                    &embedding_with_similarity(realistic_benchmark_similarity(
                        case.label, spec.label,
                    )),
                )
                .await
                .expect("store realistic benchmark embedding");
            labels_by_id.insert(id, spec.label);
        }

        labels_by_id
    }

    fn rank_realistic_benchmark_case(
        store: &SqliteMemoryStore,
        query: &str,
        scoring_mode: super::RetrievalScoringMode,
    ) -> Vec<super::RankedSearchCandidate> {
        let search_query = SearchQuery {
            text: query.to_string(),
            embedding: Some(query_embedding()),
            scope: MemoryScope::Workspace,
            state_filter: None,
            type_filter: None,
            max_results: 13,
            context_config: None,
            session_id: None,
            agent_id: None,
        };

        store
            .with_connection(|connection| {
                super::rank_search_candidates(connection, &search_query, query, None, scoring_mode)
            })
            .expect("rank realistic benchmark case")
    }

    fn rank_search_with_embedding(
        store: &SqliteMemoryStore,
        embedding: Vec<f32>,
        max_results: usize,
    ) -> Vec<super::RankedSearchCandidate> {
        let search_query = SearchQuery {
            text: String::new(),
            embedding: Some(embedding),
            scope: MemoryScope::Workspace,
            state_filter: None,
            type_filter: None,
            max_results,
            context_config: None,
            session_id: None,
            agent_id: None,
        };

        store
            .with_connection(|connection| {
                super::rank_search_candidates(
                    connection,
                    &search_query,
                    "",
                    None,
                    super::RetrievalScoringMode::Default,
                )
            })
            .expect("rank search with embedding")
    }

    fn find_ranked_candidate<'a>(
        ranked: &'a [super::RankedSearchCandidate],
        id: &MemoryId,
    ) -> &'a super::RankedSearchCandidate {
        ranked
            .iter()
            .find(|(scored_memory, _)| scored_memory.memory.id == *id)
            .expect("candidate should exist in ranked results")
    }

    fn label_for_scored_memory(
        labels_by_id: &HashMap<MemoryId, &'static str>,
        scored_memory: &crate::ScoredMemory,
    ) -> &'static str {
        labels_by_id
            .get(&scored_memory.memory.id)
            .copied()
            .expect("label should exist for scored memory")
    }

    fn rank_for_label(
        results: &[super::RankedSearchCandidate],
        labels_by_id: &HashMap<MemoryId, &'static str>,
        label: &str,
    ) -> usize {
        results
            .iter()
            .position(|(scored_memory, _)| {
                label_for_scored_memory(labels_by_id, scored_memory) == label
            })
            .map(|index| index + 1)
            .expect("label should appear in ranked results")
    }

    fn similarity_for_label(
        results: &[super::RankedSearchCandidate],
        labels_by_id: &HashMap<MemoryId, &'static str>,
        label: &str,
    ) -> f32 {
        results
            .iter()
            .find_map(|(scored_memory, _)| {
                (label_for_scored_memory(labels_by_id, scored_memory) == label)
                    .then_some(scored_memory.similarity)
            })
            .expect("label should appear in ranked results")
    }

    fn realistic_benchmark_memory_specs() -> [BenchmarkMemorySpec; 13] {
        [
            BenchmarkMemorySpec {
                label: "M01",
                content: "Run cargo test -p elegy-memory-mcp --test wu13_repro -- --nocapture before opening the pull request when retrieval behavior changes.",
                importance: 0.78,
                reliability: 0.9,
                access_count: 0,
            },
            BenchmarkMemorySpec {
                label: "M02",
                content: "On Romain's Windows setup, release binaries may land in D:\\cargo-targets\\elegy\\release instead of rust\\target\\release.",
                importance: 0.78,
                reliability: 0.9,
                access_count: 0,
            },
            BenchmarkMemorySpec {
                label: "M03",
                content: "The local stdio MCP binary does not need OAuth; OAuth only applies to the remote HTTP server.",
                importance: 0.78,
                reliability: 0.9,
                access_count: 0,
            },
            BenchmarkMemorySpec {
                label: "M04",
                content: "During benchmarks, never wipe the whole namespace. Delete only the IDs created by the current run.",
                importance: 0.78,
                reliability: 0.9,
                access_count: 0,
            },
            BenchmarkMemorySpec {
                label: "M05",
                content: "Deterministic hook audit events are appended as JSONL under .instructions-output\\hooks\\*.jsonl.",
                importance: 0.78,
                reliability: 0.9,
                access_count: 0,
            },
            BenchmarkMemorySpec {
                label: "M06",
                content: "Retrieval ranking blends similarity, recency, access frequency, and similarity-weighted priority from scope_config.",
                importance: 0.78,
                reliability: 0.9,
                access_count: 0,
            },
            BenchmarkMemorySpec {
                label: "M07",
                content: "When docs disagree, docs/system/** is authoritative over README.md and local overlays.",
                importance: 0.78,
                reliability: 0.9,
                access_count: 0,
            },
            BenchmarkMemorySpec {
                label: "M08",
                content: "Git hooks enforce the correct RomainROCH identity for protected git and gh operations in this repository.",
                importance: 1.0,
                reliability: 1.0,
                access_count: 20,
            },
            BenchmarkMemorySpec {
                label: "M09",
                content: "The dashboard development server listens on port 3000 unless the local config overrides it.",
                importance: 0.78,
                reliability: 0.9,
                access_count: 0,
            },
            BenchmarkMemorySpec {
                label: "M10",
                content: "Mulch the tomato beds after watering so the garden keeps moisture through the afternoon heat.",
                importance: 0.25,
                reliability: 0.8,
                access_count: 0,
            },
            BenchmarkMemorySpec {
                label: "M11",
                content: "After changing the site's CSS bundle, do a hard refresh or clear the browser cache to see the new styles.",
                importance: 0.78,
                reliability: 0.9,
                access_count: 0,
            },
            BenchmarkMemorySpec {
                label: "M12",
                content: "For posture work, start with light weights: two sets of twelve goblet squats at eight kilograms.",
                importance: 0.55,
                reliability: 0.85,
                access_count: 0,
            },
            BenchmarkMemorySpec {
                label: "M13",
                content: "Before release day, run smoke tests, update the changelog, and verify the release notes.",
                importance: 0.95,
                reliability: 1.0,
                access_count: 18,
            },
        ]
    }

    fn realistic_benchmark_query_cases() -> [BenchmarkQueryCase; 6] {
        [
            BenchmarkQueryCase {
                label: "R2",
                query: "alt Windows release path",
                target_label: "M02",
            },
            BenchmarkQueryCase {
                label: "R4",
                query: "wipe whole namespace?",
                target_label: "M04",
            },
            BenchmarkQueryCase {
                label: "R6",
                query: "README vs system docs",
                target_label: "M07",
            },
            BenchmarkQueryCase {
                label: "R7",
                query: "wrong git identity",
                target_label: "M08",
            },
            BenchmarkQueryCase {
                label: "R8",
                query: "audit log location",
                target_label: "M05",
            },
            BenchmarkQueryCase {
                label: "L3",
                query: "smoke tests before changelog",
                target_label: "M13",
            },
        ]
    }

    fn realistic_benchmark_similarity(case_label: &str, memory_label: &str) -> f32 {
        match (case_label, memory_label) {
            ("R1", "M01") => 0.97,
            ("R1", "M08") => 0.45,
            ("R1", "M13") => 0.42,
            ("R2", "M02") => 0.96,
            ("R2", "M08") => 0.58,
            ("R2", "M13") => 0.55,
            ("R4", "M04") => 0.95,
            ("R4", "M08") => 0.60,
            ("R4", "M13") => 0.57,
            ("R6", "M07") => 0.94,
            ("R6", "M08") => 0.59,
            ("R6", "M13") => 0.54,
            ("R7", "M08") => 0.98,
            ("R7", "M13") => 0.50,
            ("R8", "M05") => 0.95,
            ("R8", "M08") => 0.56,
            ("R8", "M13") => 0.55,
            ("L3", "M13") => 0.97,
            ("L3", "M08") => 0.52,
            (_, "M10") => 0.06,
            _ => 0.18,
        }
    }

    fn insert_version_snapshot(store: &SqliteMemoryStore, memory_id: &MemoryId) {
        store
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO memory_versions(id, memory_id, version_number, content, changed_by, change_reason, changed_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        Uuid::new_v4().to_string(),
                        memory_id.to_string(),
                        1_i64,
                        "before update",
                        "test",
                        "test snapshot",
                        format_timestamp(Utc::now()),
                    ],
                )?;
                Ok(())
            })
            .expect("insert version snapshot");
    }

    #[test]
    fn poisoning_severity_increases_with_pressure_and_scope_impact() {
        let mild = super::compute_poisoning_severity(1.1, 1, 20, 0.35);
        let severe = super::compute_poisoning_severity(3.0, 10, 20, 0.35);

        assert!(severe > mild);
        assert!((0.0..=1.0).contains(&mild));
        assert!((0.0..=1.0).contains(&severe));
    }

    #[tokio::test]
    async fn detect_poisoning_flags_frequency_anomaly_from_scope_config() {
        let fixture = test_fixture();
        set_scope_config(&fixture.store, "poison_frequency_hourly_threshold", "2");
        set_scope_config(&fixture.store, "poison_frequency_scope_ratio", "0.0");
        set_scope_config(&fixture.store, "poison_frequency_burst_ratio", "1.0");
        set_scope_config(&fixture.store, "poison_frequency_burst_min_hourly", "99");

        let mut ids = Vec::new();
        for idx in 0..3 {
            let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::Imported);
            memory.content = format!("frequency anomaly memory {idx}");
            ids.push(memory.id);
            fixture.store.store(memory).await.expect("store memory");
        }

        let alerts = fixture.store.detect_poisoning().expect("detect poisoning");
        let alert = alerts
            .into_iter()
            .find(|alert| alert.alert_type == PoisoningAlertType::FrequencyAnomaly)
            .expect("frequency anomaly alert");

        assert_eq!(alert.memory_ids.len(), ids.len());
        for id in ids {
            assert!(alert.memory_ids.contains(&id));
        }
        assert!(alert.description.contains("alert threshold 2"));
        assert!(alert.severity > 0.45);
    }

    #[tokio::test]
    async fn detect_poisoning_flags_trust_mismatch_for_low_trust_active_memories() {
        let fixture = test_fixture();
        set_scope_config(&fixture.store, "poison_trust_mismatch_count_threshold", "2");
        set_scope_config(&fixture.store, "poison_trust_mismatch_scope_ratio", "0.0");
        set_scope_config(
            &fixture.store,
            "poison_trust_mismatch_importance_threshold",
            "0.75",
        );

        let mut imported_a = sample_memory(MemoryScope::Workspace, ProvenanceLevel::Imported);
        imported_a.content = "imported trust mismatch a".to_string();
        imported_a.importance_score = 0.95;
        let imported_a_id = imported_a.id;
        fixture
            .store
            .store(imported_a)
            .await
            .expect("store imported a");

        let mut imported_b = sample_memory(MemoryScope::Workspace, ProvenanceLevel::AgentInferred);
        imported_b.content = "imported trust mismatch b".to_string();
        imported_b.importance_score = 0.85;
        let imported_b_id = imported_b.id;
        fixture
            .store
            .store(imported_b)
            .await
            .expect("store imported b");

        let mut trusted = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        trusted.content = "trusted high importance".to_string();
        trusted.importance_score = 0.99;
        fixture.store.store(trusted).await.expect("store trusted");

        let alerts = fixture.store.detect_poisoning().expect("detect poisoning");
        let alert = alerts
            .into_iter()
            .find(|alert| alert.alert_type == PoisoningAlertType::TrustMismatch)
            .expect("trust mismatch alert");

        assert_eq!(alert.memory_ids.len(), 2);
        assert!(alert.memory_ids.contains(&imported_a_id));
        assert!(alert.memory_ids.contains(&imported_b_id));
        assert!(!alert.description.is_empty());
    }

    #[tokio::test]
    async fn detect_poisoning_bulk_overwrite_stays_within_scope() {
        let fixture = test_fixture();
        let user_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::User).expect("open user store");
        set_scope_config(&fixture.store, "poison_bulk_overwrite_count_threshold", "2");
        set_scope_config(&fixture.store, "poison_bulk_overwrite_scope_ratio", "0.0");

        let workspace_a = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let workspace_a_id = workspace_a.id;
        fixture
            .store
            .store(workspace_a)
            .await
            .expect("store workspace a");
        insert_version_snapshot(&fixture.store, &workspace_a_id);

        let workspace_b = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let workspace_b_id = workspace_b.id;
        fixture
            .store
            .store(workspace_b)
            .await
            .expect("store workspace b");
        insert_version_snapshot(&fixture.store, &workspace_b_id);

        let user = sample_memory(MemoryScope::User, ProvenanceLevel::UserStated);
        let user_id = user.id;
        user_store.store(user).await.expect("store user");
        insert_version_snapshot(&user_store, &user_id);

        let alerts = fixture.store.detect_poisoning().expect("detect poisoning");
        let alert = alerts
            .into_iter()
            .find(|alert| alert.alert_type == PoisoningAlertType::BulkOverwrite)
            .expect("bulk overwrite alert");

        assert_eq!(alert.memory_ids.len(), 2);
        assert!(alert.memory_ids.contains(&workspace_a_id));
        assert!(alert.memory_ids.contains(&workspace_b_id));
        assert!(!alert.memory_ids.contains(&user_id));
    }

    #[tokio::test]
    async fn detect_poisoning_mass_contradiction_stays_within_scope() {
        let fixture = test_fixture();
        let user_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::User).expect("open user store");
        set_scope_config(
            &fixture.store,
            "poison_mass_contradiction_per_memory_threshold",
            "2",
        );
        set_scope_config(
            &fixture.store,
            "poison_mass_contradiction_scope_ratio",
            "0.0",
        );

        let focus = sample_memory(MemoryScope::Workspace, ProvenanceLevel::Imported);
        let focus_id = focus.id;
        fixture.store.store(focus).await.expect("store focus");

        let peer_a = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let peer_a_id = peer_a.id;
        fixture.store.store(peer_a).await.expect("store peer a");

        let peer_b = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let peer_b_id = peer_b.id;
        fixture.store.store(peer_b).await.expect("store peer b");

        fixture
            .store
            .record_contradiction(&focus_id, &peer_a_id, "workspace contradiction a")
            .await
            .expect("record contradiction a");
        fixture
            .store
            .record_contradiction(&focus_id, &peer_b_id, "workspace contradiction b")
            .await
            .expect("record contradiction b");

        let user_focus = sample_memory(MemoryScope::User, ProvenanceLevel::Imported);
        let user_focus_id = user_focus.id;
        user_store
            .store(user_focus)
            .await
            .expect("store user focus");
        let user_peer_a = sample_memory(MemoryScope::User, ProvenanceLevel::UserStated);
        let user_peer_a_id = user_peer_a.id;
        user_store
            .store(user_peer_a)
            .await
            .expect("store user peer a");
        let user_peer_b = sample_memory(MemoryScope::User, ProvenanceLevel::UserStated);
        let user_peer_b_id = user_peer_b.id;
        user_store
            .store(user_peer_b)
            .await
            .expect("store user peer b");
        user_store
            .record_contradiction(&user_focus_id, &user_peer_a_id, "user contradiction a")
            .await
            .expect("record user contradiction a");
        user_store
            .record_contradiction(&user_focus_id, &user_peer_b_id, "user contradiction b")
            .await
            .expect("record user contradiction b");

        let alerts = fixture.store.detect_poisoning().expect("detect poisoning");
        let alert = alerts
            .into_iter()
            .find(|alert| alert.alert_type == PoisoningAlertType::MassContradiction)
            .expect("mass contradiction alert");

        assert_eq!(alert.memory_ids, vec![focus_id]);
        assert!(!alert.memory_ids.contains(&user_focus_id));
    }

    #[tokio::test]
    async fn remediate_poisoning_quarantines_only_low_trust_active_memories() {
        let fixture = test_fixture();
        set_scope_config(
            &fixture.store,
            "poison_remediation_reliability_ceiling",
            "0.60",
        );

        let low_trust = sample_memory(MemoryScope::Workspace, ProvenanceLevel::Imported);
        let low_trust_id = low_trust.id;
        fixture
            .store
            .store(low_trust)
            .await
            .expect("store low trust");

        let trusted = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let trusted_id = trusted.id;
        fixture.store.store(trusted).await.expect("store trusted");

        let mut dormant = sample_memory(MemoryScope::Workspace, ProvenanceLevel::Imported);
        dormant.state = MemoryState::Dormant;
        let dormant_id = dormant.id;
        fixture.store.store(dormant).await.expect("store dormant");

        let alert = PoisoningAlert {
            id: Uuid::new_v4().to_string(),
            alert_type: PoisoningAlertType::TrustMismatch,
            description: "test remediation".to_string(),
            severity: 0.8,
            memory_ids: vec![low_trust_id, trusted_id, dormant_id],
            detected_at: Utc::now(),
        };

        let report = fixture
            .store
            .remediate_poisoning(&[alert])
            .expect("remediate poisoning");

        assert_eq!(report.quarantined_ids, vec![low_trust_id]);
        assert_eq!(report.skipped_ids.len(), 2);

        let remediated = fixture
            .store
            .get_raw(&low_trust_id)
            .await
            .expect("load remediated")
            .expect("remediated memory exists");
        assert_eq!(remediated.state, MemoryState::Dormant);
        assert_eq!(remediated.status.as_deref(), Some(QUARANTINED_STATUS));
        assert_eq!(
            remediated
                .custom_metadata
                .get(POISONING_REMEDIATION_METADATA_KEY)
                .map(String::as_str),
            Some("dormant quarantine")
        );
        assert!(remediated
            .custom_metadata
            .contains_key(POISONING_QUARANTINED_AT_METADATA_KEY));

        let trusted = fixture
            .store
            .get_raw(&trusted_id)
            .await
            .expect("load trusted")
            .expect("trusted memory exists");
        assert_eq!(trusted.state, MemoryState::Active);
    }

    fn query_embedding() -> Vec<f32> {
        embedding_with_similarity(1.0)
    }

    fn query_basis_embedding(index: usize, total_queries: usize) -> Vec<f32> {
        assert!(total_queries > 0);
        assert!(index < total_queries);

        let mut embedding = vec![0.0; 768];
        embedding[index] = 1.0;
        embedding
    }

    fn embedding_with_query_similarities(similarities: &[f32]) -> Vec<f32> {
        assert!(!similarities.is_empty());
        assert!(similarities.len() + 1 < 768);

        let sum_of_squares = similarities
            .iter()
            .map(|value| {
                let clamped = value.clamp(0.0, 1.0);
                clamped * clamped
            })
            .sum::<f32>();
        assert!(
            sum_of_squares <= 1.0,
            "similarities must fit inside a normalized embedding"
        );

        let mut embedding = vec![0.0; 768];
        for (index, similarity) in similarities.iter().enumerate() {
            embedding[index] = similarity.clamp(0.0, 1.0);
        }
        embedding[similarities.len()] = (1.0 - sum_of_squares).sqrt();
        embedding
    }

    fn embedding_with_similarity(similarity: f32) -> Vec<f32> {
        let clamped_similarity = similarity.clamp(0.0, 1.0);
        let orthogonal_component = (1.0 - (clamped_similarity * clamped_similarity)).sqrt();
        let mut embedding = vec![0.0; 768];
        embedding[0] = clamped_similarity;
        embedding[1] = orthogonal_component;
        embedding
    }

    #[tokio::test]
    async fn correct_memory_updates_content_and_reliability() {
        let fixture = test_fixture();
        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        memory.content = "old content".to_string();
        memory.reliability_score = 0.7;
        let id = memory.id;
        let old_reliability = memory.reliability_score;

        fixture.store.store(memory).await.expect("store memory");

        let correction = fixture
            .store
            .correct_memory(&id, "corrected content", "test-user", Some("was wrong"))
            .expect("correction should succeed");

        assert_eq!(correction.previous_content, "old content");
        assert_eq!(correction.corrected_content, "corrected content");
        assert_eq!(correction.corrected_by, "test-user");
        assert_eq!(correction.reason, "was wrong");
        assert_eq!(correction.memory_id, id);

        let updated = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get")
            .expect("exists");
        assert_eq!(updated.content, "corrected content");
        assert!(
            updated.reliability_score > old_reliability,
            "reliability should have increased: {} vs {}",
            updated.reliability_score,
            old_reliability,
        );
        assert!(updated.embedding_stale);
    }

    #[tokio::test]
    async fn correct_memory_archives_low_salience_memory() {
        let fixture = test_fixture();
        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        memory.content = "old content".to_string();
        memory.importance_score = 0.1;
        let id = memory.id;

        fixture.store.store(memory).await.expect("store memory");

        let correction = fixture
            .store
            .correct_memory(
                &id,
                "still low salience",
                "test-user",
                Some("archive expected"),
            )
            .expect("correction should succeed");

        assert_eq!(correction.disposition, CorrectionDisposition::Archived);
        assert_eq!(correction.related_memory_id, None);

        let updated = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get")
            .expect("exists");
        assert_eq!(updated.state, MemoryState::Dormant);
    }

    #[tokio::test]
    async fn correct_memory_merges_into_existing_memory_and_archives_source() {
        let fixture = test_fixture();

        let mut target = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        target.content = "Canonical backend is Rust with Axum".to_string();
        target.embedding_stale = false;
        let target_id = target.id;
        fixture.store.store(target).await.expect("store target");

        let mut source = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        source.content = "Legacy backend is Ruby on Rails".to_string();
        let source_id = source.id;
        fixture.store.store(source).await.expect("store source");

        let correction = fixture
            .store
            .correct_memory(
                &source_id,
                "Canonical backend is Rust with Axum",
                "test-user",
                Some("merge into canonical memory"),
            )
            .expect("correction should succeed");

        assert_eq!(correction.disposition, CorrectionDisposition::Merged);
        assert_eq!(correction.related_memory_id, Some(target_id));

        let source_after = fixture
            .store
            .get_raw(&source_id)
            .await
            .expect("get source")
            .expect("source exists");
        assert_eq!(source_after.state, MemoryState::Dormant);

        let target_after = fixture
            .store
            .get_raw(&target_id)
            .await
            .expect("get target")
            .expect("target exists");
        assert_eq!(target_after.content, "Canonical backend is Rust with Axum");
        assert!(target_after.reliability_score >= 1.0);
    }

    #[tokio::test]
    async fn correct_memory_records_contradiction_disposition() {
        let provider = Arc::new(StubEmbeddingProvider::new([
            (
                "Backend is C# with gRPC",
                StubEmbeddingResponse::Embedding(query_embedding()),
            ),
            (
                "Legacy backend note",
                StubEmbeddingResponse::Embedding(embedding_with_similarity(0.2)),
            ),
            (
                "Backend is Python with Flask",
                StubEmbeddingResponse::Embedding(query_embedding()),
            ),
        ]));
        let fixture = test_fixture_with_provider(provider);

        let mut existing = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        existing.content = "Backend is C# with gRPC".to_string();
        let existing_id = existing.id;
        fixture.store.store(existing).await.expect("store existing");

        let mut source = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        source.content = "Legacy backend note".to_string();
        let source_id = source.id;
        fixture.store.store(source).await.expect("store source");

        let correction = fixture
            .store
            .correct_memory(
                &source_id,
                "Backend is Python with Flask",
                "test-user",
                Some("contradiction expected"),
            )
            .expect("correction should succeed");

        assert_eq!(correction.disposition, CorrectionDisposition::Contradiction);
        assert_eq!(correction.related_memory_id, Some(existing_id));

        let contradictions = fixture
            .store
            .list_contradictions(None)
            .await
            .expect("list contradictions");
        assert_eq!(contradictions.len(), 1);
        assert_eq!(contradictions[0].memory_a_id, existing_id);
        assert_eq!(contradictions[0].memory_b_id, source_id);

        let source_after = fixture
            .store
            .get_raw(&source_id)
            .await
            .expect("get source")
            .expect("source exists");
        assert_eq!(source_after.state, MemoryState::Active);
        assert_eq!(source_after.content, "Backend is Python with Flask");
    }

    #[tokio::test]
    async fn correct_memory_excludes_stale_vectors_from_similarity_search() {
        let fixture = test_fixture();
        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        memory.content = "vector-backed memory".to_string();
        let id = memory.id;

        fixture.store.store(memory).await.expect("store memory");
        fixture
            .store
            .store_embedding(&id, &query_embedding())
            .await
            .expect("store embedding");

        let before = fixture
            .store
            .find_similar(&query_embedding(), 0.8, 5)
            .await
            .expect("find similar before correction");
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].memory.id, id);

        fixture
            .store
            .correct_memory(
                &id,
                "corrected content",
                "test-user",
                Some("stale vector check"),
            )
            .expect("correction should succeed");

        let updated = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get")
            .expect("exists");
        assert!(updated.embedding_stale);

        let after = fixture
            .store
            .find_similar(&query_embedding(), 0.8, 5)
            .await
            .expect("find similar after correction");
        assert!(after.is_empty(), "stale embeddings should be excluded");
    }

    #[tokio::test]
    async fn correct_memory_creates_version_entry() {
        let fixture = test_fixture();
        let memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let id = memory.id;

        fixture.store.store(memory).await.expect("store memory");

        fixture
            .store
            .correct_memory(&id, "v2 content", "corrector", None)
            .expect("correction should succeed");

        let versions = fixture.store.list_versions(&id).expect("list versions");
        assert!(
            !versions.is_empty(),
            "should have at least one version entry after correction"
        );
    }

    #[tokio::test]
    async fn correct_memory_not_found() {
        let fixture = test_fixture();
        let missing_id = Uuid::new_v4();

        let result = fixture
            .store
            .correct_memory(&missing_id, "new", "user", None);
        assert!(result.is_err(), "correcting missing memory should fail");
    }

    #[tokio::test]
    async fn correct_memory_caps_reliability_at_one() {
        let fixture = test_fixture();
        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        memory.reliability_score = 0.95;
        let id = memory.id;

        fixture.store.store(memory).await.expect("store memory");

        fixture
            .store
            .correct_memory(&id, "better", "user", None)
            .expect("correction should succeed");

        let updated = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get")
            .expect("exists");
        assert!(
            (updated.reliability_score - 1.0).abs() < f32::EPSILON,
            "reliability should be capped at 1.0, got {}",
            updated.reliability_score,
        );
    }

    #[tokio::test]
    async fn record_feedback_relevant_increments_access_count() {
        let fixture = test_fixture();
        let memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let id = memory.id;
        let original_access = memory.access_count;

        fixture.store.store(memory).await.expect("store memory");

        let feedback = fixture
            .store
            .record_feedback(&id, "test query", true)
            .expect("feedback should succeed");

        assert!(feedback.relevant);
        assert_eq!(feedback.memory_id, id);
        assert_eq!(feedback.query_text.as_deref(), Some("test query"));

        let updated = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get")
            .expect("exists");
        assert_eq!(
            updated.access_count,
            original_access + 1,
            "access count should be incremented for relevant feedback"
        );
    }

    #[tokio::test]
    async fn record_feedback_irrelevant_reduces_importance() {
        let fixture = test_fixture();
        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        memory.importance_score = 0.5;
        let id = memory.id;

        fixture.store.store(memory).await.expect("store memory");

        let feedback = fixture
            .store
            .record_feedback(&id, "bad query", false)
            .expect("feedback should succeed");

        assert!(!feedback.relevant);

        let updated = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get")
            .expect("exists");
        assert!(
            (updated.importance_score - 0.48).abs() < f32::EPSILON,
            "importance should be reduced by 0.02, got {}",
            updated.importance_score,
        );
    }

    #[tokio::test]
    async fn record_feedback_irrelevant_floors_importance_at_zero() {
        let fixture = test_fixture();
        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        memory.importance_score = 0.01;
        let id = memory.id;

        fixture.store.store(memory).await.expect("store memory");

        fixture
            .store
            .record_feedback(&id, "q", false)
            .expect("feedback should succeed");

        let updated = fixture
            .store
            .get_raw(&id)
            .await
            .expect("get")
            .expect("exists");
        assert!(
            updated.importance_score >= 0.0,
            "importance should be floored at 0.0, got {}",
            updated.importance_score,
        );
    }

    #[tokio::test]
    async fn record_feedback_not_found() {
        let fixture = test_fixture();
        let missing_id = Uuid::new_v4();

        let result = fixture.store.record_feedback(&missing_id, "q", true);
        assert!(
            result.is_err(),
            "recording feedback for missing memory should fail"
        );
    }

    #[test]
    fn compute_learned_weights_defaults_with_insufficient_data() {
        let fixture = test_fixture();

        let weights = fixture
            .store
            .compute_learned_weights()
            .expect("compute weights");

        assert_eq!(weights.len(), 4);
        assert!(
            (weights["similarity_weight"] - f64::from(crate::types::DEFAULT_SIMILARITY_WEIGHT))
                .abs()
                < f64::EPSILON
        );
        assert!(
            (weights["recency_weight"] - f64::from(crate::types::DEFAULT_RECENCY_WEIGHT)).abs()
                < f64::EPSILON
        );
        assert!(
            (weights["access_weight"] - f64::from(crate::types::DEFAULT_ACCESS_WEIGHT)).abs()
                < f64::EPSILON
        );
        assert!(
            (weights["priority_weight"] - f64::from(crate::types::DEFAULT_PRIORITY_WEIGHT)).abs()
                < f64::EPSILON
        );
    }

    #[tokio::test]
    async fn record_feedback_persists_live_learned_weights_after_balanced_feedback() {
        let fixture = test_fixture();

        for idx in 0..6 {
            let mut relevant_memory =
                sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
            relevant_memory.content = format!("apollo rollout checklist canonical {idx}");
            relevant_memory.importance_score = 0.35;
            let relevant_id = relevant_memory.id;

            let mut irrelevant_memory =
                sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
            irrelevant_memory.content = format!("apollo archive reference {idx}");
            irrelevant_memory.importance_score = 0.95;
            let irrelevant_id = irrelevant_memory.id;

            fixture
                .store
                .store(relevant_memory)
                .await
                .expect("store relevant memory");
            fixture
                .store
                .store(irrelevant_memory)
                .await
                .expect("store irrelevant memory");

            fixture
                .store
                .record_feedback(&relevant_id, "apollo rollout checklist", true)
                .expect("record relevant feedback");
            fixture
                .store
                .record_feedback(&irrelevant_id, "apollo rollout checklist", false)
                .expect("record irrelevant feedback");
        }

        let report = fixture
            .store
            .learned_weights_report()
            .expect("load learned weights report");
        let scope_config = fixture.store.scope_config().expect("scope config");

        assert_eq!(report.sample_size, 12);
        assert_eq!(report.relevant_samples, 6);
        assert_eq!(report.irrelevant_samples, 6);
        assert!(!report.using_defaults, "report should be in learned mode");
        assert!(
            report.effective_weights.similarity_weight
                > f64::from(crate::types::DEFAULT_SIMILARITY_WEIGHT),
            "similarity weight should be boosted, got {}",
            report.effective_weights.similarity_weight,
        );
        assert!(
            (f64::from(scope_config.similarity_weight)
                - report.effective_weights.similarity_weight)
                .abs()
                < 1e-6
        );
        assert!(
            (f64::from(scope_config.recency_weight) - report.effective_weights.recency_weight)
                .abs()
                < 1e-6
        );
        assert!(
            (f64::from(scope_config.access_weight) - report.effective_weights.access_weight).abs()
                < 1e-6
        );
        assert!(
            (f64::from(scope_config.priority_weight) - report.effective_weights.priority_weight)
                .abs()
                < 1e-6
        );
    }

    #[tokio::test]
    async fn feedback_learning_updates_live_search_scoring_via_scope_config() {
        let fixture = test_fixture();

        let mut similarity_favored =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        similarity_favored.content = "apollo rollout checklist canonical".to_string();
        similarity_favored.importance_score = 0.35;
        similarity_favored.reliability_score = 1.0;
        let similarity_favored_id = similarity_favored.id;

        let mut priority_favored =
            sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        priority_favored.content = "apollo archive reference".to_string();
        priority_favored.importance_score = 0.9;
        priority_favored.reliability_score = 1.0;
        let priority_favored_id = priority_favored.id;

        fixture
            .store
            .store(similarity_favored)
            .await
            .expect("store high-similarity memory");
        fixture
            .store
            .store(priority_favored)
            .await
            .expect("store high-priority memory");

        fixture
            .store
            .store_embedding(&similarity_favored_id, &embedding_with_similarity(0.75))
            .await
            .expect("store high-similarity embedding");
        fixture
            .store
            .store_embedding(&priority_favored_id, &embedding_with_similarity(0.40))
            .await
            .expect("store high-priority embedding");

        fixture
            .store
            .with_connection(|connection| {
                let now = super::format_timestamp(Utc::now());
                connection.execute(
                    "UPDATE memories SET access_count = 0, importance_score = 0.30, updated_at = ?2, last_accessed_at = NULL WHERE id = ?1",
                    params![similarity_favored_id.to_string(), now.clone()],
                )?;
                connection.execute(
                    "UPDATE memories SET access_count = 64, importance_score = 1.0, updated_at = ?2, last_accessed_at = NULL WHERE id = ?1",
                    params![priority_favored_id.to_string(), now],
                )?;
                Ok(())
            })
            .expect("seed deterministic ranking baseline");

        let before = rank_search_with_embedding(&fixture.store, query_embedding(), 5);

        assert_eq!(before.len(), 2);
        assert_eq!(before[0].0.memory.id, similarity_favored_id);
        assert_eq!(before[1].0.memory.id, priority_favored_id);
        let before_similarity = find_ranked_candidate(&before, &similarity_favored_id);
        let before_priority = find_ranked_candidate(&before, &priority_favored_id);
        assert_eq!(before_similarity.1.similarity_band_index, 0);
        assert!(
            before_priority.1.similarity_band_index > before_similarity.1.similarity_band_index
        );

        fixture
            .store
            .with_connection(|connection| {
                let now = super::format_timestamp(Utc::now());
                for _ in 0..24 {
                    connection.execute(
                        "INSERT INTO retrieval_feedback (id, memory_id, query_text, relevant, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            Uuid::new_v4().to_string(),
                            similarity_favored_id.to_string(),
                            "apollo rollout checklist",
                            1_i64,
                            now.clone(),
                        ],
                    )?;
                    connection.execute(
                        "INSERT INTO retrieval_feedback (id, memory_id, query_text, relevant, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            Uuid::new_v4().to_string(),
                            priority_favored_id.to_string(),
                            "apollo rollout checklist",
                            0_i64,
                            now.clone(),
                        ],
                    )?;
                }

                let report = super::compute_learned_weights_report(connection)?;
                super::persist_learned_weights(connection, report.effective_weights)?;

                connection.execute(
                    "UPDATE memories SET access_count = 0, importance_score = 0.30, updated_at = ?2, last_accessed_at = NULL WHERE id = ?1",
                    params![similarity_favored_id.to_string(), now.clone()],
                )?;
                connection.execute(
                    "UPDATE memories SET access_count = 64, importance_score = 1.0, updated_at = ?2, last_accessed_at = NULL WHERE id = ?1",
                    params![priority_favored_id.to_string(), now],
                )?;
                Ok(())
            })
            .expect("restore memory fields so ranking shift comes from learned weights");

        let learned_scope_config = fixture.store.scope_config().expect("scope config");
        assert!(
            learned_scope_config.similarity_weight > crate::types::DEFAULT_SIMILARITY_WEIGHT,
            "similarity weight should be learned upward, got {}",
            learned_scope_config.similarity_weight,
        );

        let after = rank_search_with_embedding(&fixture.store, query_embedding(), 5);

        assert_eq!(after.len(), 2);
        assert_eq!(after[0].0.memory.id, similarity_favored_id);
        assert_eq!(after[1].0.memory.id, priority_favored_id);
        let after_similarity = find_ranked_candidate(&after, &similarity_favored_id);
        let after_priority = find_ranked_candidate(&after, &priority_favored_id);
        assert!(
            after_similarity.1.weighted_similarity > before_similarity.1.weighted_similarity,
            "persisted learned weights should raise the live similarity contribution"
        );
        assert!(
            after_priority.1.weighted_priority < before_priority.1.weighted_priority,
            "persisted learned weights should reduce the live priority contribution"
        );
    }

    #[tokio::test]
    async fn export_sqlite_round_trips_memories() {
        let fixture = test_fixture();

        let mut m = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        m.content = "sqlite-export-test".to_string();
        let id = m.id;
        fixture.store.store(m).await.expect("store");

        let bytes = crate::MemoryObservability::export_memories(
            &fixture.store,
            MemoryScope::Workspace,
            ExportFormat::Sqlite,
        )
        .expect("sqlite export");
        assert!(!bytes.is_empty(), "exported bytes must be non-empty");

        // Open the exported DB and verify the memory is there
        let export_path = fixture.path.with_extension("export.sqlite3");
        std::fs::write(&export_path, &bytes).expect("write export");
        let conn = rusqlite::Connection::open(&export_path).expect("open export");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM memories WHERE id = ?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .expect("count");
        let _ = std::fs::remove_file(&export_path);
        assert_eq!(count, 1, "exported DB should contain the stored memory");
    }

    #[tokio::test]
    async fn export_elegy_round_trips_memories() {
        let fixture = test_fixture();

        let mut m = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        m.content = "elegy-export-test".to_string();
        fixture.store.store(m).await.expect("store");

        let bytes = crate::MemoryObservability::export_memories(
            &fixture.store,
            MemoryScope::Workspace,
            ExportFormat::Elegy,
        )
        .expect("elegy export");
        let archive: ElegyArchive = serde_json::from_slice(&bytes).expect("deserialize archive");
        assert_eq!(archive.format_version, "1");
        assert_eq!(archive.scope, MemoryScope::Workspace);
        assert_eq!(archive.memories.len(), 1);
        assert_eq!(archive.memories[0].content, "elegy-export-test");
    }

    #[tokio::test]
    async fn export_sqlite_includes_links_and_versions() {
        let fixture = test_fixture();

        let mut m1 = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        m1.content = "linked-a".to_string();
        let mut m2 = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        m2.content = "linked-b".to_string();
        let id1 = m1.id;
        let id2 = m2.id;
        fixture.store.store(m1).await.expect("store m1");
        fixture.store.store(m2).await.expect("store m2");
        fixture
            .store
            .record_link(&id1, &id2, "related")
            .expect("link");

        // Create a version by correcting m1
        fixture
            .store
            .correct_memory(&id1, "linked-a-v2", "tester", Some("test reason"))
            .expect("correct");

        let bytes = crate::MemoryObservability::export_memories(
            &fixture.store,
            MemoryScope::Workspace,
            ExportFormat::Sqlite,
        )
        .expect("sqlite export");
        let export_path = fixture.path.with_extension("check-links.sqlite3");
        std::fs::write(&export_path, &bytes).expect("write");
        let conn = rusqlite::Connection::open(&export_path).expect("open");

        let link_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM memory_links", [], |r| r.get(0))
            .expect("count links");
        let _ = std::fs::remove_file(&export_path);
        assert!(link_count >= 1, "should export at least one link");

        let version_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM memory_versions", [], |r| r.get(0))
            .expect("count versions");
        assert!(version_count >= 1, "should export at least one version");
    }

    #[tokio::test]
    async fn export_for_sharing_filters_by_config() {
        let fixture = test_fixture();

        // Store a Low-sensitivity memory
        let mut low = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        low.content = "shareable".to_string();
        low.sensitivity = SensitivityLevel::Low;
        low.reliability_score = 0.8;
        fixture.store.store(low).await.expect("store low");

        // Store a Critical-sensitivity memory
        let mut crit = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        crit.content = "secret".to_string();
        crit.sensitivity = SensitivityLevel::Critical;
        crit.reliability_score = 0.9;
        fixture.store.store(crit).await.expect("store critical");

        // Store a low-reliability memory
        let mut unreliable = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        unreliable.content = "unreliable".to_string();
        unreliable.reliability_score = 0.2;
        fixture
            .store
            .store(unreliable)
            .await
            .expect("store unreliable");

        let config = ShareConfig {
            max_sensitivity: SensitivityLevel::Medium,
            min_reliability: 0.5,
            type_filter: None,
            tag_filter: None,
        };
        let shared = fixture.store.export_for_sharing(&config).expect("export");
        assert_eq!(shared.len(), 1, "only the low-sensitivity reliable memory");
        assert_eq!(shared[0].content, "shareable");
        assert_eq!(shared[0].provenance, ProvenanceLevel::Imported);
        assert!(shared[0].tenant_id.is_none());
        assert!(shared[0].user_id.is_none());
        assert!(shared[0].agent_id.is_none());
    }

    #[tokio::test]
    async fn export_for_sharing_applies_tag_filter() {
        let fixture = test_fixture();

        let mut tagged = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        tagged.content = "tagged".to_string();
        tagged.tags = vec!["rust".to_string(), "memory".to_string()];
        tagged.reliability_score = 0.8;
        fixture.store.store(tagged).await.expect("store tagged");

        let mut untagged = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        untagged.content = "untagged".to_string();
        untagged.reliability_score = 0.8;
        fixture.store.store(untagged).await.expect("store untagged");

        let config = ShareConfig {
            max_sensitivity: SensitivityLevel::Critical,
            min_reliability: 0.0,
            type_filter: None,
            tag_filter: Some(vec!["rust".to_string()]),
        };
        let shared = fixture.store.export_for_sharing(&config).expect("export");
        assert_eq!(shared.len(), 1);
        assert_eq!(shared[0].content, "tagged");
    }

    #[tokio::test]
    async fn import_shared_creates_new_memories() {
        let fixture = test_fixture();

        let mut source = sample_memory(MemoryScope::Agent, ProvenanceLevel::UserStated);
        source.content = "imported-content".to_string();
        source.reliability_score = 0.9;
        source.provenance = ProvenanceLevel::UserStated;
        source.tenant_id = Some("other-tenant".to_string());
        source.user_id = Some("other-user".to_string());
        source.agent_id = Some("other-agent".to_string());

        let ids = fixture.store.import_shared(&[source]).expect("import");
        assert_eq!(ids.len(), 1);

        let imported = fixture
            .store
            .get_raw(&ids[0])
            .await
            .expect("get")
            .expect("exists");
        assert_eq!(imported.content, "imported-content");
        assert_eq!(imported.provenance, ProvenanceLevel::Imported);
        assert!(
            imported.reliability_score <= 0.6,
            "reliability should be capped at 0.6"
        );
        assert_eq!(
            imported.state,
            MemoryState::Dormant,
            "shared imports stay dormant"
        );
        assert_eq!(imported.scope, MemoryScope::Workspace, "scope rewritten");
        assert!(imported.tenant_id.is_none(), "tenant cleared");
        assert!(imported.user_id.is_none(), "user cleared");
        assert!(imported.agent_id.is_none(), "agent cleared");
        assert!(imported.embedding_stale, "embedding marked stale");
    }

    #[tokio::test]
    async fn import_shared_assigns_fresh_ids() {
        let fixture = test_fixture();

        let m1 = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let m2 = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        let original_id1 = m1.id;
        let original_id2 = m2.id;

        let ids = fixture.store.import_shared(&[m1, m2]).expect("import");
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], original_id1, "should be fresh ID");
        assert_ne!(ids[1], original_id2, "should be fresh ID");
        assert_ne!(ids[0], ids[1], "each import gets unique ID");
    }

    #[tokio::test]
    async fn detect_poisoning_uses_scope_ratio_for_frequency_alerts() {
        let fixture = test_fixture();
        fixture
            .store
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE scope_config SET value = '4' WHERE key = 'poison_frequency_hourly_threshold'",
                    [],
                )?;
                connection.execute(
                    "UPDATE scope_config SET value = '0.75' WHERE key = 'poison_frequency_scope_ratio'",
                    [],
                )?;
                Ok(())
            })
            .expect("configure poisoning thresholds");

        for index in 0..4 {
            let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::Imported);
            memory.content = format!("frequency-memory-{index}");
            fixture
                .store
                .store(memory)
                .await
                .expect("store frequency memory");
        }

        fixture
            .store
            .with_connection(|connection| {
                connection.execute(
                    "DELETE FROM memories WHERE content = 'frequency-memory-3'",
                    [],
                )?;
                Ok(())
            })
            .expect("delete one memory to drop below ratio threshold");

        let no_alerts = fixture
            .store
            .detect_poisoning()
            .expect("detect without threshold hit");
        assert!(no_alerts
            .iter()
            .all(|alert| alert.alert_type != PoisoningAlertType::FrequencyAnomaly));

        let mut restored = sample_memory(MemoryScope::Workspace, ProvenanceLevel::Imported);
        restored.content = "frequency-memory-restored".to_string();
        fixture
            .store
            .store(restored)
            .await
            .expect("restore ratio threshold");

        let alerts = fixture.store.detect_poisoning().expect("detect poisoning");
        let frequency = alerts
            .iter()
            .find(|alert| alert.alert_type == PoisoningAlertType::FrequencyAnomaly)
            .expect("frequency alert");
        assert_eq!(frequency.memory_ids.len(), 4);
    }

    #[tokio::test]
    async fn detect_poisoning_flags_trust_mismatch_and_remediates_only_low_trust_memories() {
        let fixture = test_fixture();
        fixture
            .store
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE scope_config SET value = '1' WHERE key = 'poison_trust_mismatch_count_threshold'",
                    [],
                )?;
                connection.execute(
                    "UPDATE scope_config SET value = '0.95' WHERE key = 'poison_trust_mismatch_importance_threshold'",
                    [],
                )?;
                Ok(())
            })
            .expect("configure trust mismatch thresholds");

        let mut imported = sample_memory(MemoryScope::Workspace, ProvenanceLevel::Imported);
        imported.content = "suspicious imported memory".to_string();
        imported.importance_score = 0.96;
        let imported_id = fixture
            .store
            .store(imported)
            .await
            .expect("store imported memory");

        let mut trusted = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        trusted.content = "trusted memory".to_string();
        let trusted_id = fixture
            .store
            .store(trusted)
            .await
            .expect("store trusted memory");

        let alerts = fixture.store.detect_poisoning().expect("detect poisoning");
        let trust_mismatch = alerts
            .iter()
            .find(|alert| alert.alert_type == PoisoningAlertType::TrustMismatch)
            .expect("trust mismatch alert");
        assert_eq!(trust_mismatch.memory_ids, vec![imported_id]);

        let remediation = fixture
            .store
            .remediate_poisoning(&alerts)
            .expect("remediate poisoning");
        assert_eq!(remediation.quarantined_ids, vec![imported_id]);
        assert!(remediation.skipped_ids.is_empty());

        let imported = fixture
            .store
            .get_raw(&imported_id)
            .await
            .expect("reload imported")
            .expect("imported exists");
        assert_eq!(imported.state, MemoryState::Dormant);
        assert_eq!(imported.status.as_deref(), Some(QUARANTINED_STATUS));

        let trusted = fixture
            .store
            .get_raw(&trusted_id)
            .await
            .expect("reload trusted")
            .expect("trusted exists");
        assert_eq!(trusted.state, MemoryState::Active);
    }

    #[tokio::test]
    async fn detect_poisoning_bulk_overwrite_stays_scoped() {
        let fixture = test_fixture();
        let agent_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::Agent).expect("open agent store");
        fixture
            .store
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE scope_config SET value = '1' WHERE key = 'poison_bulk_overwrite_count_threshold'",
                    [],
                )?;
                connection.execute(
                    "UPDATE scope_config SET value = '0.0' WHERE key = 'poison_bulk_overwrite_scope_ratio'",
                    [],
                )?;
                Ok(())
            })
            .expect("configure bulk overwrite thresholds");

        let agent_id = agent_store
            .store(sample_memory(
                MemoryScope::Agent,
                ProvenanceLevel::UserStated,
            ))
            .await
            .expect("store agent memory");
        agent_store
            .update_content(
                &agent_id,
                "updated agent memory",
                "test",
                "scope-only agent edit",
            )
            .await
            .expect("update agent memory");
        let no_workspace_alert = fixture
            .store
            .detect_poisoning()
            .expect("detect poisoning without workspace updates");
        assert!(no_workspace_alert
            .iter()
            .all(|alert| alert.alert_type != PoisoningAlertType::BulkOverwrite));

        let workspace_id = fixture
            .store
            .store(sample_memory(
                MemoryScope::Workspace,
                ProvenanceLevel::UserStated,
            ))
            .await
            .expect("store workspace memory");
        fixture
            .store
            .update_content(
                &workspace_id,
                "updated workspace memory",
                "test",
                "workspace edit",
            )
            .await
            .expect("update workspace memory");

        let alerts = fixture.store.detect_poisoning().expect("detect poisoning");
        let bulk = alerts
            .iter()
            .find(|alert| alert.alert_type == PoisoningAlertType::BulkOverwrite)
            .expect("bulk overwrite alert");
        assert_eq!(bulk.memory_ids, vec![workspace_id]);
    }

    #[tokio::test]
    async fn detect_poisoning_mass_contradiction_stays_scoped() {
        let fixture = test_fixture();
        fixture
            .store
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE scope_config SET value = '2' WHERE key = 'poison_mass_contradiction_per_memory_threshold'",
                    [],
                )?;
                connection.execute(
                    "UPDATE scope_config SET value = '0.0' WHERE key = 'poison_mass_contradiction_scope_ratio'",
                    [],
                )?;
                Ok(())
            })
            .expect("configure mass contradiction thresholds");

        let target_id = fixture
            .store
            .store(sample_memory(
                MemoryScope::Workspace,
                ProvenanceLevel::Imported,
            ))
            .await
            .expect("store contradiction target");
        for content in ["counter-a", "counter-b"] {
            let mut other = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
            other.content = content.to_string();
            let other_id = fixture
                .store
                .store(other)
                .await
                .expect("store contradiction peer");
            fixture
                .store
                .record_contradiction(&target_id, &other_id, "contradiction")
                .await
                .expect("record contradiction");
        }

        let alerts = fixture.store.detect_poisoning().expect("detect poisoning");
        let contradiction_alert = alerts
            .iter()
            .find(|alert| alert.alert_type == PoisoningAlertType::MassContradiction)
            .expect("mass contradiction alert");
        assert_eq!(contradiction_alert.memory_ids, vec![target_id]);
    }

    #[tokio::test]
    async fn import_shared_uses_gate_and_keeps_novel_content_dormant() {
        let content = "novel shared import content";
        let provider = Arc::new(StubEmbeddingProvider::new([(
            content,
            StubEmbeddingResponse::Embedding(vec![1.0; 768]),
        )]));
        let fixture = test_fixture_with_provider(provider.clone());

        let mut source = sample_memory(MemoryScope::Agent, ProvenanceLevel::UserStated);
        source.content = content.to_string();

        let report = fixture
            .store
            .import_shared_with_report(&[source])
            .expect("import shared memory");
        assert_eq!(report.review_ids.len(), 1);
        assert_eq!(provider.call_count(), 1, "gate should request an embedding");

        let imported = fixture
            .store
            .get_raw(&report.review_ids[0])
            .await
            .expect("get imported memory")
            .expect("imported memory exists");
        assert_eq!(imported.state, MemoryState::Dormant);
        assert_eq!(imported.status.as_deref(), Some(SHARED_REVIEW_STATUS));
    }

    #[tokio::test]
    async fn import_shared_never_merges_into_existing_trusted_memories() {
        let content = "existing trusted memory";
        let provider = Arc::new(StubEmbeddingProvider::new([(
            content,
            StubEmbeddingResponse::Embedding(vec![1.0; 768]),
        )]));
        let fixture = test_fixture_with_provider(provider.clone());

        let mut existing = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        existing.content = content.to_string();
        let existing_id = fixture.store.store(existing).await.expect("store existing");

        let mut source = sample_memory(MemoryScope::Agent, ProvenanceLevel::UserStated);
        source.content = content.to_string();

        let report = fixture
            .store
            .import_shared_with_report(&[source])
            .expect("import shared duplicate");
        assert_eq!(report.quarantined_ids.len(), 1);

        let existing = fixture
            .store
            .get_raw(&existing_id)
            .await
            .expect("reload existing")
            .expect("existing memory exists");
        assert_eq!(existing.content, content);
        assert_eq!(existing.state, MemoryState::Active);

        let imported = fixture
            .store
            .get_raw(&report.quarantined_ids[0])
            .await
            .expect("reload imported")
            .expect("imported memory exists");
        assert_eq!(imported.state, MemoryState::Dormant);
        assert_eq!(imported.status.as_deref(), Some(QUARANTINED_STATUS));
    }

    #[tokio::test]
    async fn import_shared_exact_duplicate_without_provider_still_quarantines() {
        let fixture = test_fixture();

        let mut existing = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        existing.content = "existing trusted memory".to_string();
        let existing_id = fixture.store.store(existing).await.expect("store existing");

        let mut source = sample_memory(MemoryScope::Agent, ProvenanceLevel::UserStated);
        source.content = " existing   trusted memory ".to_string();

        let report = fixture
            .store
            .import_shared_with_report(&[source])
            .expect("import shared duplicate without provider");
        assert_eq!(report.review_ids.len(), 0);
        assert_eq!(report.quarantined_ids.len(), 1);

        let existing = fixture
            .store
            .get_raw(&existing_id)
            .await
            .expect("reload existing")
            .expect("existing memory exists");
        assert_eq!(existing.state, MemoryState::Active);

        let imported = fixture
            .store
            .get_raw(&report.quarantined_ids[0])
            .await
            .expect("reload imported")
            .expect("imported memory exists");
        assert_eq!(imported.state, MemoryState::Dormant);
        assert_eq!(imported.status.as_deref(), Some(QUARANTINED_STATUS));
    }

    #[tokio::test]
    async fn import_shared_quarantines_contradictions_and_records_journal() {
        let existing_content = "Cap RTSS 120fps";
        let candidate_content = "Cap RTSS 60fps";
        let provider = Arc::new(StubEmbeddingProvider::new([
            (
                existing_content,
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
            (
                candidate_content,
                StubEmbeddingResponse::Embedding(vec![1.0; 768]),
            ),
        ]));
        let fixture = test_fixture_with_provider(provider.clone());

        let mut existing = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        existing.content = existing_content.to_string();
        let existing_id = fixture.store.store(existing).await.expect("store existing");

        let mut source = sample_memory(MemoryScope::Agent, ProvenanceLevel::UserStated);
        source.content = candidate_content.to_string();

        let report = fixture
            .store
            .import_shared_with_report(&[source])
            .expect("import contradictory shared memory");
        assert_eq!(report.quarantined_ids.len(), 1);

        let contradictions = fixture
            .store
            .list_contradictions(Some(ResolutionStatus::Unresolved))
            .await
            .expect("list contradictions");
        assert_eq!(contradictions.len(), 1);
        assert_eq!(contradictions[0].memory_a_id, existing_id);
        assert_eq!(contradictions[0].memory_b_id, report.quarantined_ids[0]);
    }

    #[tokio::test]
    async fn import_shared_skips_higher_scope_near_duplicates() {
        let content = "higher scope canonical memory";
        let provider = Arc::new(StubEmbeddingProvider::new([(
            content,
            StubEmbeddingResponse::Embedding(vec![1.0; 768]),
        )]));
        let fixture = test_fixture_with_provider(provider.clone());
        let user_store = SqliteMemoryStore::new_with_embedding_provider(
            &fixture.path,
            MemoryScope::User,
            provider,
        )
        .expect("open user store");

        let mut higher_scope = sample_memory(MemoryScope::User, ProvenanceLevel::UserStated);
        higher_scope.content = content.to_string();
        user_store
            .store(higher_scope)
            .await
            .expect("store higher scope memory");

        let mut source = sample_memory(MemoryScope::Agent, ProvenanceLevel::UserStated);
        source.content = content.to_string();

        let report = fixture
            .store
            .import_shared_with_report(&[source])
            .expect("import shared duplicate");
        assert!(report.new_ids.is_empty());
        assert_eq!(report.skipped_reasons.len(), 1);
    }

    #[tokio::test]
    async fn import_shared_skips_higher_scope_exact_duplicate_without_provider() {
        let fixture = test_fixture();
        let user_store =
            SqliteMemoryStore::new(&fixture.path, MemoryScope::User).expect("open user store");

        let mut higher_scope = sample_memory(MemoryScope::User, ProvenanceLevel::UserStated);
        higher_scope.content = "higher scope canonical memory".to_string();
        user_store
            .store(higher_scope)
            .await
            .expect("store higher scope memory");

        let mut source = sample_memory(MemoryScope::Agent, ProvenanceLevel::UserStated);
        source.content = "  Higher   scope canonical memory ".to_string();

        let report = fixture
            .store
            .import_shared_with_report(&[source])
            .expect("import shared duplicate without provider");
        assert!(report.new_ids.is_empty());
        assert_eq!(report.skipped_reasons.len(), 1);
        assert!(
            report.skipped_reasons[0].contains("higher visible scope"),
            "expected skip reason to mention higher visible scope, got {:?}",
            report.skipped_reasons
        );
    }
}
