pub mod cli;
pub mod error;
mod local_store;
pub mod traits;
pub mod types;

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};

pub use local_store::{
    LocalMemoryCatalog, LocalMemoryCatalogEntry, LocalMemoryExportResult, LocalMemoryPaths,
    LocalMemoryQueryOptions, LocalMemoryStore, LocalMemoryStoreError, LocalMemoryStoreInitResult,
    LocalMemoryStoredRecord, LOCAL_MEMORY_ARTIFACTS_DIR, LOCAL_MEMORY_AUTHORITY_POSTURE,
    LOCAL_MEMORY_DETERMINISTIC_ORDERING, LOCAL_MEMORY_EXPORTS_DIR,
    LOCAL_MEMORY_SINGLE_WRITER_POSTURE, LOCAL_MEMORY_STATE_DIR, LOCAL_MEMORY_STORE_KIND,
    LOCAL_MEMORY_WRITE_LOCK_RELATIVE_PATH,
};
pub use error::{
    ConsolidationError, EmbeddingError, GateError, ObservabilityError, StoreError,
};
pub use traits::{
    ConsolidationAction, EmbeddingProvider, GateDecision, MemoryConsolidator, MemoryFilter,
    MemoryObservability, MemoryStore, MetadataUpdate, OptionalFieldUpdate, SalienceGate,
};
pub use types::{
    ContradictionEntry, ContradictionRecord, ExportFormat, Memory, MemoryCandidate,
    MemoryContextConfig, MemoryHealthReport, MemoryId, MemoryScope, MemorySearchQuery,
    MemorySearchResult, MemoryState, MemoryType, MemoryVersion, ProvenanceLevel, PurgeReport,
    ResolutionStatus, ScopeConfig, ScoredMemory, SearchQuery, SensitivityLevel,
};

pub const SUMMARY_ONLY_SESSION_CONTEXT_ARTIFACT_KIND: &str =
    "summary-only-session-context-envelope";
pub const GOVERNED_MEMORY_RECORD_ARTIFACT_KIND: &str = "governed-memory-record";
pub const GOVERNED_MEMORY_RECORD_PROJECTION_ARTIFACT_KIND: &str =
    "governed-memory-record-projection";
pub const SUMMARY_ONLY_REPRESENTATION: &str = "summary-only";
pub const MEMORY_PROJECTION_RULES_VERSION: &str = "1.0.0";
pub const MAX_SUMMARY_LENGTH: usize = 4_000;
pub const MAX_SALIENT_FACTS: usize = 16;
pub const MAX_INSTRUCTION_CONTEXT_ITEMS: usize = 8;
pub const MAX_CONTEXT_ITEM_LENGTH: usize = 280;
pub const MAX_MEMORY_RECORD_ID_LENGTH: usize = 128;
pub const MAX_MEMORY_SORT_KEY_LENGTH: usize = 280;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryArtifactKind {
    #[default]
    #[serde(rename = "summary-only-session-context-envelope")]
    SummaryOnlySessionContextEnvelope,
    #[serde(rename = "governed-memory-record")]
    GovernedMemoryRecord,
    #[serde(rename = "governed-memory-record-projection")]
    GovernedMemoryRecordProjection,
}

impl MemoryArtifactKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SummaryOnlySessionContextEnvelope => SUMMARY_ONLY_SESSION_CONTEXT_ARTIFACT_KIND,
            Self::GovernedMemoryRecord => GOVERNED_MEMORY_RECORD_ARTIFACT_KIND,
            Self::GovernedMemoryRecordProjection => GOVERNED_MEMORY_RECORD_PROJECTION_ARTIFACT_KIND,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionContextScope {
    Run,
    Session,
    Workspace,
}

impl SessionContextScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Session => "session",
            Self::Workspace => "workspace",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionContextRepresentation {
    #[default]
    #[serde(rename = "summary-only")]
    SummaryOnly,
}

impl SessionContextRepresentation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SummaryOnly => SUMMARY_ONLY_REPRESENTATION,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LocalMemoryLifecycleState {
    #[default]
    Active,
    Superseded,
    Tombstoned,
}

impl LocalMemoryLifecycleState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Superseded => "superseded",
            Self::Tombstoned => "tombstoned",
        }
    }

    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Active, Self::Superseded)
                | (Self::Active, Self::Tombstoned)
                | (Self::Superseded, Self::Tombstoned)
        )
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SummaryOnlySessionContext {
    pub scope: SessionContextScope,
    pub representation: SessionContextRepresentation,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub salient_facts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instruction_context: Vec<String>,
    pub raw_transcript_persisted: bool,
}

impl SummaryOnlySessionContext {
    fn validated(self) -> Result<Self, MemoryValidationError> {
        self.validate()?;
        Ok(self)
    }

    pub fn new(
        scope: SessionContextScope,
        summary: impl Into<String>,
    ) -> Result<Self, MemoryValidationError> {
        Self {
            scope,
            representation: SessionContextRepresentation::SummaryOnly,
            summary: summary.into(),
            salient_facts: Vec::new(),
            instruction_context: Vec::new(),
            raw_transcript_persisted: false,
        }
        .validated()
    }

    pub fn validate(&self) -> Result<(), MemoryValidationError> {
        validate_constant_string(
            "sessionContext.representation",
            self.representation.as_str(),
            SUMMARY_ONLY_REPRESENTATION,
        )?;
        validate_bounded_string("sessionContext.summary", &self.summary, MAX_SUMMARY_LENGTH)?;
        validate_bounded_items(
            "sessionContext.salientFacts",
            &self.salient_facts,
            MAX_SALIENT_FACTS,
        )?;
        validate_bounded_items(
            "sessionContext.instructionContext",
            &self.instruction_context,
            MAX_INSTRUCTION_CONTEXT_ITEMS,
        )?;

        if self.raw_transcript_persisted {
            return Err(MemoryValidationError::RawTranscriptPersistedMustBeFalse);
        }

        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SummaryOnlySessionContextWire {
    scope: SessionContextScope,
    representation: SessionContextRepresentation,
    summary: String,
    #[serde(default)]
    salient_facts: Vec<String>,
    #[serde(default)]
    instruction_context: Vec<String>,
    raw_transcript_persisted: bool,
}

impl<'de> Deserialize<'de> for SummaryOnlySessionContext {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SummaryOnlySessionContextWire::deserialize(deserializer)?;
        Self {
            scope: wire.scope,
            representation: wire.representation,
            summary: wire.summary,
            salient_facts: wire.salient_facts,
            instruction_context: wire.instruction_context,
            raw_transcript_persisted: wire.raw_transcript_persisted,
        }
        .validated()
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SummaryOnlySessionContextEnvelope {
    pub artifact_kind: MemoryArtifactKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at_utc: Option<String>,
    pub session_context: SummaryOnlySessionContext,
}

impl SummaryOnlySessionContextEnvelope {
    fn validated(self) -> Result<Self, MemoryValidationError> {
        self.validate()?;
        Ok(self)
    }

    pub fn new(session_context: SummaryOnlySessionContext) -> Result<Self, MemoryValidationError> {
        Self {
            artifact_kind: MemoryArtifactKind::SummaryOnlySessionContextEnvelope,
            request_id: None,
            run_id: None,
            captured_at_utc: None,
            session_context,
        }
        .validated()
    }

    pub fn validate(&self) -> Result<(), MemoryValidationError> {
        validate_constant_string(
            "artifactKind",
            self.artifact_kind.as_str(),
            SUMMARY_ONLY_SESSION_CONTEXT_ARTIFACT_KIND,
        )?;
        validate_optional_non_empty("requestId", &self.request_id)?;
        validate_optional_non_empty("runId", &self.run_id)?;
        validate_optional_rfc3339("capturedAtUtc", &self.captured_at_utc)?;
        self.session_context.validate()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SummaryOnlySessionContextEnvelopeWire {
    artifact_kind: MemoryArtifactKind,
    #[serde(default)]
    request_id: Option<String>,
    #[serde(default)]
    run_id: Option<String>,
    #[serde(default)]
    captured_at_utc: Option<String>,
    session_context: SummaryOnlySessionContext,
}

impl<'de> Deserialize<'de> for SummaryOnlySessionContextEnvelope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SummaryOnlySessionContextEnvelopeWire::deserialize(deserializer)?;
        Self {
            artifact_kind: wire.artifact_kind,
            request_id: wire.request_id,
            run_id: wire.run_id,
            captured_at_utc: wire.captured_at_utc,
            session_context: wire.session_context,
        }
        .validated()
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRecordProvenance {
    pub source_artifact_kind: MemoryArtifactKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at_utc: Option<String>,
    pub imported_at_utc: String,
    pub projection_rules_version: String,
}

impl MemoryRecordProvenance {
    fn validated(self) -> Result<Self, MemoryValidationError> {
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), MemoryValidationError> {
        validate_constant_string(
            "provenance.sourceArtifactKind",
            self.source_artifact_kind.as_str(),
            SUMMARY_ONLY_SESSION_CONTEXT_ARTIFACT_KIND,
        )?;
        validate_optional_non_empty("provenance.requestId", &self.request_id)?;
        validate_optional_non_empty("provenance.runId", &self.run_id)?;
        validate_optional_rfc3339("provenance.capturedAtUtc", &self.captured_at_utc)?;
        validate_rfc3339("provenance.importedAtUtc", &self.imported_at_utc)?;
        validate_constant_string(
            "provenance.projectionRulesVersion",
            &self.projection_rules_version,
            MEMORY_PROJECTION_RULES_VERSION,
        )?;
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MemoryRecordProvenanceWire {
    source_artifact_kind: MemoryArtifactKind,
    #[serde(default)]
    request_id: Option<String>,
    #[serde(default)]
    run_id: Option<String>,
    #[serde(default)]
    captured_at_utc: Option<String>,
    imported_at_utc: String,
    projection_rules_version: String,
}

impl<'de> Deserialize<'de> for MemoryRecordProvenance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = MemoryRecordProvenanceWire::deserialize(deserializer)?;
        Self {
            source_artifact_kind: wire.source_artifact_kind,
            request_id: wire.request_id,
            run_id: wire.run_id,
            captured_at_utc: wire.captured_at_utc,
            imported_at_utc: wire.imported_at_utc,
            projection_rules_version: wire.projection_rules_version,
        }
        .validated()
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryTombstoneMetadata {
    pub tombstoned_at_utc: String,
    pub reason: String,
}

impl MemoryTombstoneMetadata {
    fn validated(self) -> Result<Self, MemoryValidationError> {
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), MemoryValidationError> {
        validate_rfc3339(
            "localLifecycle.tombstone.tombstonedAtUtc",
            &self.tombstoned_at_utc,
        )?;
        validate_bounded_string(
            "localLifecycle.tombstone.reason",
            &self.reason,
            MAX_CONTEXT_ITEM_LENGTH,
        )?;
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MemoryTombstoneMetadataWire {
    tombstoned_at_utc: String,
    reason: String,
}

impl<'de> Deserialize<'de> for MemoryTombstoneMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = MemoryTombstoneMetadataWire::deserialize(deserializer)?;
        Self {
            tombstoned_at_utc: wire.tombstoned_at_utc,
            reason: wire.reason,
        }
        .validated()
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalMemoryLifecycle {
    pub state: LocalMemoryLifecycleState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by_record_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tombstone: Option<MemoryTombstoneMetadata>,
}

impl LocalMemoryLifecycle {
    pub fn active() -> Self {
        Self::default()
    }

    fn validated(self) -> Result<Self, MemoryValidationError> {
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), MemoryValidationError> {
        validate_optional_bounded_string(
            "localLifecycle.supersededByRecordId",
            &self.superseded_by_record_id,
            MAX_MEMORY_RECORD_ID_LENGTH,
        )?;

        match self.state {
            LocalMemoryLifecycleState::Active => {
                if self.superseded_by_record_id.is_some() {
                    return Err(MemoryValidationError::UnexpectedLifecycleMetadata {
                        field: "localLifecycle.supersededByRecordId",
                        state: self.state.as_str(),
                    });
                }

                if self.tombstone.is_some() {
                    return Err(MemoryValidationError::UnexpectedLifecycleMetadata {
                        field: "localLifecycle.tombstone",
                        state: self.state.as_str(),
                    });
                }
            }
            LocalMemoryLifecycleState::Superseded => {
                if self.superseded_by_record_id.is_none() {
                    return Err(MemoryValidationError::MissingLifecycleMetadata {
                        field: "localLifecycle.supersededByRecordId",
                        state: self.state.as_str(),
                    });
                }

                if self.tombstone.is_some() {
                    return Err(MemoryValidationError::UnexpectedLifecycleMetadata {
                        field: "localLifecycle.tombstone",
                        state: self.state.as_str(),
                    });
                }
            }
            LocalMemoryLifecycleState::Tombstoned => {
                let tombstone = self.tombstone.as_ref().ok_or(
                    MemoryValidationError::MissingLifecycleMetadata {
                        field: "localLifecycle.tombstone",
                        state: self.state.as_str(),
                    },
                )?;
                tombstone.validate()?;
            }
        }

        Ok(())
    }

    pub fn transition_to_superseded(
        &self,
        superseded_by_record_id: impl Into<String>,
    ) -> Result<Self, MemoryValidationError> {
        let next_state = LocalMemoryLifecycleState::Superseded;
        if !self.state.can_transition_to(next_state) {
            return Err(MemoryValidationError::InvalidLifecycleTransition {
                from: self.state.as_str(),
                to: next_state.as_str(),
            });
        }

        Self {
            state: next_state,
            superseded_by_record_id: Some(superseded_by_record_id.into()),
            tombstone: None,
        }
        .validated()
    }

    pub fn transition_to_tombstoned(
        &self,
        tombstone: MemoryTombstoneMetadata,
    ) -> Result<Self, MemoryValidationError> {
        let next_state = LocalMemoryLifecycleState::Tombstoned;
        if !self.state.can_transition_to(next_state) {
            return Err(MemoryValidationError::InvalidLifecycleTransition {
                from: self.state.as_str(),
                to: next_state.as_str(),
            });
        }

        Self {
            state: next_state,
            superseded_by_record_id: self.superseded_by_record_id.clone(),
            tombstone: Some(tombstone),
        }
        .validated()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LocalMemoryLifecycleWire {
    state: LocalMemoryLifecycleState,
    #[serde(default)]
    superseded_by_record_id: Option<String>,
    #[serde(default)]
    tombstone: Option<MemoryTombstoneMetadata>,
}

impl<'de> Deserialize<'de> for LocalMemoryLifecycle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = LocalMemoryLifecycleWire::deserialize(deserializer)?;
        Self {
            state: wire.state,
            superseded_by_record_id: wire.superseded_by_record_id,
            tombstone: wire.tombstone,
        }
        .validated()
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRecordSortKeys {
    pub scope_captured_at_record_id: String,
    pub lifecycle_state_record_id: String,
    pub superseded_by_record_id_or_self: String,
}

impl MemoryRecordSortKeys {
    fn from_components(
        record_id: &str,
        scope: SessionContextScope,
        provenance: &MemoryRecordProvenance,
        local_lifecycle: &LocalMemoryLifecycle,
    ) -> Result<Self, MemoryValidationError> {
        let scope_value = scope.as_str();
        let captured_at = canonical_sort_key_timestamp(
            provenance.captured_at_utc.as_deref(),
            provenance.imported_at_utc.as_str(),
        )?;
        let superseded_by = local_lifecycle
            .superseded_by_record_id
            .as_deref()
            .unwrap_or(record_id);

        Ok(Self {
            scope_captured_at_record_id: format!("{scope_value}|{captured_at}|{record_id}"),
            lifecycle_state_record_id: format!(
                "{}|{scope_value}|{record_id}",
                local_lifecycle.state.as_str()
            ),
            superseded_by_record_id_or_self: format!("{scope_value}|{superseded_by}|{record_id}"),
        })
    }

    fn validate_matches(
        &self,
        record_id: &str,
        scope: SessionContextScope,
        provenance: &MemoryRecordProvenance,
        local_lifecycle: &LocalMemoryLifecycle,
    ) -> Result<(), MemoryValidationError> {
        let expected = Self::from_components(record_id, scope, provenance, local_lifecycle)?;

        validate_sort_key(
            "deterministicSortKeys.scopeCapturedAtRecordId",
            &self.scope_captured_at_record_id,
            &expected.scope_captured_at_record_id,
        )?;
        validate_sort_key(
            "deterministicSortKeys.lifecycleStateRecordId",
            &self.lifecycle_state_record_id,
            &expected.lifecycle_state_record_id,
        )?;
        validate_sort_key(
            "deterministicSortKeys.supersededByRecordIdOrSelf",
            &self.superseded_by_record_id_or_self,
            &expected.superseded_by_record_id_or_self,
        )?;

        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MemoryRecordSortKeysWire {
    scope_captured_at_record_id: String,
    lifecycle_state_record_id: String,
    superseded_by_record_id_or_self: String,
}

impl<'de> Deserialize<'de> for MemoryRecordSortKeys {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = MemoryRecordSortKeysWire::deserialize(deserializer)?;
        Ok(Self {
            scope_captured_at_record_id: wire.scope_captured_at_record_id,
            lifecycle_state_record_id: wire.lifecycle_state_record_id,
            superseded_by_record_id_or_self: wire.superseded_by_record_id_or_self,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GovernedMemoryRecordImportOptions {
    pub record_id: String,
    pub imported_at_utc: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GovernedMemoryRecord {
    pub artifact_kind: MemoryArtifactKind,
    pub record_id: String,
    pub session_context: SummaryOnlySessionContext,
    pub provenance: MemoryRecordProvenance,
    pub local_lifecycle: LocalMemoryLifecycle,
    pub deterministic_sort_keys: MemoryRecordSortKeys,
}

impl GovernedMemoryRecord {
    fn validated(self) -> Result<Self, MemoryValidationError> {
        self.validate()?;
        Ok(self)
    }

    pub fn import_summary_only_envelope(
        envelope: &SummaryOnlySessionContextEnvelope,
        options: GovernedMemoryRecordImportOptions,
    ) -> Result<Self, MemoryValidationError> {
        envelope.validate()?;
        validate_bounded_string("recordId", &options.record_id, MAX_MEMORY_RECORD_ID_LENGTH)?;
        validate_rfc3339("provenance.importedAtUtc", &options.imported_at_utc)?;
        let imported_at_utc =
            canonicalize_rfc3339_utc("provenance.importedAtUtc", &options.imported_at_utc)?;

        let provenance = MemoryRecordProvenance {
            source_artifact_kind: MemoryArtifactKind::SummaryOnlySessionContextEnvelope,
            request_id: envelope.request_id.clone(),
            run_id: envelope.run_id.clone(),
            captured_at_utc: envelope.captured_at_utc.clone(),
            imported_at_utc,
            projection_rules_version: MEMORY_PROJECTION_RULES_VERSION.to_string(),
        }
        .validated()?;
        let local_lifecycle = LocalMemoryLifecycle::active();
        let deterministic_sort_keys = MemoryRecordSortKeys::from_components(
            &options.record_id,
            envelope.session_context.scope,
            &provenance,
            &local_lifecycle,
        )?;

        Self {
            artifact_kind: MemoryArtifactKind::GovernedMemoryRecord,
            record_id: options.record_id,
            session_context: envelope.session_context.clone(),
            provenance,
            local_lifecycle,
            deterministic_sort_keys,
        }
        .validated()
    }

    pub fn validate(&self) -> Result<(), MemoryValidationError> {
        validate_constant_string(
            "artifactKind",
            self.artifact_kind.as_str(),
            GOVERNED_MEMORY_RECORD_ARTIFACT_KIND,
        )?;
        validate_bounded_string("recordId", &self.record_id, MAX_MEMORY_RECORD_ID_LENGTH)?;
        self.session_context.validate()?;
        self.provenance.validate()?;
        self.local_lifecycle.validate()?;
        self.deterministic_sort_keys.validate_matches(
            &self.record_id,
            self.session_context.scope,
            &self.provenance,
            &self.local_lifecycle,
        )
    }

    pub fn export_summary_only_envelope(
        &self,
    ) -> Result<SummaryOnlySessionContextEnvelope, MemoryValidationError> {
        self.validate()?;

        SummaryOnlySessionContextEnvelope {
            artifact_kind: MemoryArtifactKind::SummaryOnlySessionContextEnvelope,
            request_id: self.provenance.request_id.clone(),
            run_id: self.provenance.run_id.clone(),
            captured_at_utc: self.provenance.captured_at_utc.clone(),
            session_context: self.session_context.clone(),
        }
        .validated()
    }

    pub fn project_summary_only_envelope(
        &self,
    ) -> Result<GovernedMemoryRecordProjection, MemoryValidationError> {
        self.validate()?;

        GovernedMemoryRecordProjection {
            artifact_kind: MemoryArtifactKind::GovernedMemoryRecordProjection,
            rules_version: MEMORY_PROJECTION_RULES_VERSION.to_string(),
            source_record_id: self.record_id.clone(),
            source_artifact_kind: MemoryArtifactKind::GovernedMemoryRecord,
            target_artifact_kind: MemoryArtifactKind::SummaryOnlySessionContextEnvelope,
            source_lifecycle_state: self.local_lifecycle.state,
            deterministic_sort_key: self
                .deterministic_sort_keys
                .scope_captured_at_record_id
                .clone(),
            projected_envelope: self.export_summary_only_envelope()?,
        }
        .validated()
    }

    pub fn supersede(
        &self,
        superseded_by_record_id: impl Into<String>,
    ) -> Result<Self, MemoryValidationError> {
        self.with_local_lifecycle(
            self.local_lifecycle
                .transition_to_superseded(superseded_by_record_id)?,
        )
    }

    pub fn tombstone(
        &self,
        tombstoned_at_utc: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<Self, MemoryValidationError> {
        let tombstone = MemoryTombstoneMetadata {
            tombstoned_at_utc: tombstoned_at_utc.into(),
            reason: reason.into(),
        }
        .validated()?;

        self.with_local_lifecycle(self.local_lifecycle.transition_to_tombstoned(tombstone)?)
    }

    fn with_local_lifecycle(
        &self,
        local_lifecycle: LocalMemoryLifecycle,
    ) -> Result<Self, MemoryValidationError> {
        let deterministic_sort_keys = MemoryRecordSortKeys::from_components(
            &self.record_id,
            self.session_context.scope,
            &self.provenance,
            &local_lifecycle,
        )?;

        Self {
            artifact_kind: self.artifact_kind,
            record_id: self.record_id.clone(),
            session_context: self.session_context.clone(),
            provenance: self.provenance.clone(),
            local_lifecycle,
            deterministic_sort_keys,
        }
        .validated()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GovernedMemoryRecordWire {
    artifact_kind: MemoryArtifactKind,
    record_id: String,
    session_context: SummaryOnlySessionContext,
    provenance: MemoryRecordProvenance,
    local_lifecycle: LocalMemoryLifecycle,
    deterministic_sort_keys: MemoryRecordSortKeys,
}

impl<'de> Deserialize<'de> for GovernedMemoryRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GovernedMemoryRecordWire::deserialize(deserializer)?;
        Self {
            artifact_kind: wire.artifact_kind,
            record_id: wire.record_id,
            session_context: wire.session_context,
            provenance: wire.provenance,
            local_lifecycle: wire.local_lifecycle,
            deterministic_sort_keys: wire.deterministic_sort_keys,
        }
        .validated()
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GovernedMemoryRecordProjection {
    pub artifact_kind: MemoryArtifactKind,
    pub rules_version: String,
    pub source_record_id: String,
    pub source_artifact_kind: MemoryArtifactKind,
    pub target_artifact_kind: MemoryArtifactKind,
    pub source_lifecycle_state: LocalMemoryLifecycleState,
    pub deterministic_sort_key: String,
    pub projected_envelope: SummaryOnlySessionContextEnvelope,
}

impl GovernedMemoryRecordProjection {
    fn validated(self) -> Result<Self, MemoryValidationError> {
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), MemoryValidationError> {
        validate_constant_string(
            "artifactKind",
            self.artifact_kind.as_str(),
            GOVERNED_MEMORY_RECORD_PROJECTION_ARTIFACT_KIND,
        )?;
        validate_constant_string(
            "rulesVersion",
            &self.rules_version,
            MEMORY_PROJECTION_RULES_VERSION,
        )?;
        validate_bounded_string(
            "sourceRecordId",
            &self.source_record_id,
            MAX_MEMORY_RECORD_ID_LENGTH,
        )?;
        validate_constant_string(
            "sourceArtifactKind",
            self.source_artifact_kind.as_str(),
            GOVERNED_MEMORY_RECORD_ARTIFACT_KIND,
        )?;
        validate_constant_string(
            "targetArtifactKind",
            self.target_artifact_kind.as_str(),
            SUMMARY_ONLY_SESSION_CONTEXT_ARTIFACT_KIND,
        )?;
        validate_bounded_string(
            "deterministicSortKey",
            &self.deterministic_sort_key,
            MAX_MEMORY_SORT_KEY_LENGTH,
        )?;
        self.projected_envelope.validate()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GovernedMemoryRecordProjectionWire {
    artifact_kind: MemoryArtifactKind,
    rules_version: String,
    source_record_id: String,
    source_artifact_kind: MemoryArtifactKind,
    target_artifact_kind: MemoryArtifactKind,
    source_lifecycle_state: LocalMemoryLifecycleState,
    deterministic_sort_key: String,
    projected_envelope: SummaryOnlySessionContextEnvelope,
}

impl<'de> Deserialize<'de> for GovernedMemoryRecordProjection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GovernedMemoryRecordProjectionWire::deserialize(deserializer)?;
        Self {
            artifact_kind: wire.artifact_kind,
            rules_version: wire.rules_version,
            source_record_id: wire.source_record_id,
            source_artifact_kind: wire.source_artifact_kind,
            target_artifact_kind: wire.target_artifact_kind,
            source_lifecycle_state: wire.source_lifecycle_state,
            deterministic_sort_key: wire.deterministic_sort_key,
            projected_envelope: wire.projected_envelope,
        }
        .validated()
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum MemoryValidationError {
    #[error("{field} must not be empty")]
    EmptyString { field: &'static str },
    #[error("{field} exceeds max length {max} with {actual} character(s)")]
    StringTooLong {
        field: &'static str,
        max: usize,
        actual: usize,
    },
    #[error("{field} exceeds max items {max} with {actual} item(s)")]
    TooManyItems {
        field: &'static str,
        max: usize,
        actual: usize,
    },
    #[error("{field}[{index}] must not be empty")]
    EmptyItem { field: &'static str, index: usize },
    #[error("{field}[{index}] exceeds max length {max} with {actual} character(s)")]
    ItemTooLong {
        field: &'static str,
        index: usize,
        max: usize,
        actual: usize,
    },
    #[error("{field} must be a valid RFC 3339 date-time")]
    InvalidDateTime { field: &'static str },
    #[error("{field} must be `{expected}`")]
    ConstantMismatch {
        field: &'static str,
        expected: &'static str,
    },
    #[error(
        "sessionContext.rawTranscriptPersisted must be false for portable summary-only artifacts"
    )]
    RawTranscriptPersistedMustBeFalse,
    #[error("{field} metadata is required when localLifecycle.state is `{state}`")]
    MissingLifecycleMetadata {
        field: &'static str,
        state: &'static str,
    },
    #[error("{field} metadata is not allowed when localLifecycle.state is `{state}`")]
    UnexpectedLifecycleMetadata {
        field: &'static str,
        state: &'static str,
    },
    #[error("local lifecycle transition `{from}` -> `{to}` is not allowed")]
    InvalidLifecycleTransition {
        from: &'static str,
        to: &'static str,
    },
    #[error("{field} must match deterministic projection formula")]
    DeterministicSortKeyMismatch { field: &'static str },
}

fn validate_constant_string(
    field: &'static str,
    value: &str,
    expected: &'static str,
) -> Result<(), MemoryValidationError> {
    if value != expected {
        return Err(MemoryValidationError::ConstantMismatch { field, expected });
    }

    Ok(())
}

fn validate_optional_non_empty(
    field: &'static str,
    value: &Option<String>,
) -> Result<(), MemoryValidationError> {
    if let Some(value) = value {
        validate_bounded_string(field, value, usize::MAX)?;
    }

    Ok(())
}

fn validate_optional_bounded_string(
    field: &'static str,
    value: &Option<String>,
    max: usize,
) -> Result<(), MemoryValidationError> {
    if let Some(value) = value {
        validate_bounded_string(field, value, max)?;
    }

    Ok(())
}

fn parse_rfc3339(
    field: &'static str,
    value: &str,
) -> Result<OffsetDateTime, MemoryValidationError> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| MemoryValidationError::InvalidDateTime { field })
}

fn validate_rfc3339(field: &'static str, value: &str) -> Result<(), MemoryValidationError> {
    parse_rfc3339(field, value)?;

    Ok(())
}

fn canonical_sort_key_timestamp(
    captured_at_utc: Option<&str>,
    imported_at_utc: &str,
) -> Result<String, MemoryValidationError> {
    match captured_at_utc {
        Some(value) => canonicalize_rfc3339_utc("provenance.capturedAtUtc", value),
        None => canonicalize_rfc3339_utc("provenance.importedAtUtc", imported_at_utc),
    }
}

fn canonicalize_rfc3339_utc(
    field: &'static str,
    value: &str,
) -> Result<String, MemoryValidationError> {
    parse_rfc3339(field, value)?
        .to_offset(UtcOffset::UTC)
        .format(&Rfc3339)
        .map_err(|_| MemoryValidationError::InvalidDateTime { field })
}

fn validate_optional_rfc3339(
    field: &'static str,
    value: &Option<String>,
) -> Result<(), MemoryValidationError> {
    if let Some(value) = value {
        validate_rfc3339(field, value)?;
    }

    Ok(())
}

fn validate_bounded_string(
    field: &'static str,
    value: &str,
    max: usize,
) -> Result<(), MemoryValidationError> {
    let actual = value.chars().count();
    if actual == 0 {
        return Err(MemoryValidationError::EmptyString { field });
    }

    if actual > max {
        return Err(MemoryValidationError::StringTooLong { field, max, actual });
    }

    Ok(())
}

fn validate_bounded_items(
    field: &'static str,
    items: &[String],
    max_items: usize,
) -> Result<(), MemoryValidationError> {
    let actual_items = items.len();
    if actual_items > max_items {
        return Err(MemoryValidationError::TooManyItems {
            field,
            max: max_items,
            actual: actual_items,
        });
    }

    for (index, item) in items.iter().enumerate() {
        let actual = item.chars().count();
        if actual == 0 {
            return Err(MemoryValidationError::EmptyItem { field, index });
        }

        if actual > MAX_CONTEXT_ITEM_LENGTH {
            return Err(MemoryValidationError::ItemTooLong {
                field,
                index,
                max: MAX_CONTEXT_ITEM_LENGTH,
                actual,
            });
        }
    }

    Ok(())
}

fn validate_sort_key(
    field: &'static str,
    actual: &str,
    expected: &str,
) -> Result<(), MemoryValidationError> {
    validate_bounded_string(field, actual, MAX_MEMORY_SORT_KEY_LENGTH)?;
    if actual != expected {
        return Err(MemoryValidationError::DeterministicSortKeyMismatch { field });
    }

    Ok(())
}
