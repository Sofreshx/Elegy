use elegy_memory::{
    GovernedMemoryRecord, GovernedMemoryRecordImportOptions, GovernedMemoryRecordProjection,
    LocalMemoryLifecycleState, MemoryArtifactKind, MemoryValidationError,
    SessionContextRepresentation, SessionContextScope, SummaryOnlySessionContext,
    SummaryOnlySessionContextEnvelope,
};
use serde::Serialize;
use serde_json::json;

const SUMMARY_ONLY_FIXTURE_JSON: &str =
    include_str!("../../../contracts/fixtures/summary-only-session-context-envelope.minimal.json");
const GOVERNED_RECORD_FIXTURE_JSON: &str = r#"{
  "artifactKind": "governed-memory-record",
  "recordId": "memory-record-1",
  "sessionContext": {
    "scope": "workspace",
    "representation": "summary-only",
    "summary": "Workspace context persists only bounded summaries for instruction assembly and follow-on agent runs.",
    "salientFacts": [
      "Persist summary and context artifacts only.",
      "Raw execution logs remain transient and are not stored durably."
    ],
    "instructionContext": [
      "Use this summary context when assembling workspace-level instructions."
    ],
    "rawTranscriptPersisted": false
  },
  "provenance": {
    "sourceArtifactKind": "summary-only-session-context-envelope",
    "requestId": "request-1",
    "runId": "run-1",
    "capturedAtUtc": "2026-03-22T00:00:00Z",
    "importedAtUtc": "2026-03-23T00:00:00Z",
    "projectionRulesVersion": "1.0.0"
  },
  "localLifecycle": { "state": "active" },
  "deterministicSortKeys": {
    "scopeCapturedAtRecordId": "workspace|2026-03-22T00:00:00Z|memory-record-1",
    "lifecycleStateRecordId": "active|workspace|memory-record-1",
    "supersededByRecordIdOrSelf": "workspace|memory-record-1|memory-record-1"
  }
}
"#;
const GOVERNED_PROJECTION_FIXTURE_JSON: &str = r#"{
  "artifactKind": "governed-memory-record-projection",
  "rulesVersion": "1.0.0",
  "sourceRecordId": "memory-record-1",
  "sourceArtifactKind": "governed-memory-record",
  "targetArtifactKind": "summary-only-session-context-envelope",
  "sourceLifecycleState": "active",
  "deterministicSortKey": "workspace|2026-03-22T00:00:00Z|memory-record-1",
  "projectedEnvelope": {
    "artifactKind": "summary-only-session-context-envelope",
    "requestId": "request-1",
    "runId": "run-1",
    "capturedAtUtc": "2026-03-22T00:00:00Z",
    "sessionContext": {
      "scope": "workspace",
      "representation": "summary-only",
      "summary": "Workspace context persists only bounded summaries for instruction assembly and follow-on agent runs.",
      "salientFacts": [
        "Persist summary and context artifacts only.",
        "Raw execution logs remain transient and are not stored durably."
      ],
      "instructionContext": [
        "Use this summary context when assembling workspace-level instructions."
      ],
      "rawTranscriptPersisted": false
    }
  }
}
"#;
const MAX_MEMORY_RECORD_ID_LENGTH: usize = 128;
const MAX_MEMORY_SORT_KEY_LENGTH: usize = 280;

#[test]
fn fixture_shape_deserializes_and_validates() {
    let envelope: SummaryOnlySessionContextEnvelope =
        match serde_json::from_str(SUMMARY_ONLY_FIXTURE_JSON) {
            Ok(value) => value,
            Err(error) => panic!("fixture should deserialize: {error}"),
        };

    assert_eq!(
        envelope.artifact_kind,
        MemoryArtifactKind::SummaryOnlySessionContextEnvelope
    );
    assert_eq!(
        envelope.session_context.scope,
        SessionContextScope::Workspace
    );
    assert!(envelope.validate().is_ok());
}

#[test]
fn constructor_serializes_canonical_strings() {
    let context = match SummaryOnlySessionContext::new(
        SessionContextScope::Session,
        "Bounded summary for handoff.",
    ) {
        Ok(value) => value,
        Err(error) => panic!("context should be valid: {error}"),
    };

    let envelope = match SummaryOnlySessionContextEnvelope::new(context) {
        Ok(value) => value,
        Err(error) => panic!("envelope should be valid: {error}"),
    };

    let value = match serde_json::to_value(&envelope) {
        Ok(value) => value,
        Err(error) => panic!("envelope should serialize: {error}"),
    };

    assert_eq!(
        value
            .get("artifactKind")
            .and_then(serde_json::Value::as_str),
        Some("summary-only-session-context-envelope")
    );
    assert_eq!(
        value
            .get("sessionContext")
            .and_then(|entry| entry.get("representation"))
            .and_then(serde_json::Value::as_str),
        Some("summary-only")
    );
    assert!(value.get("requestId").is_none());
}

#[test]
fn governed_memory_record_fixture_deserializes_and_validates() {
    let record: GovernedMemoryRecord = match serde_json::from_str(GOVERNED_RECORD_FIXTURE_JSON) {
        Ok(value) => value,
        Err(error) => panic!("record fixture should deserialize: {error}"),
    };

    assert_eq!(
        record.artifact_kind,
        MemoryArtifactKind::GovernedMemoryRecord
    );
    assert_eq!(
        record.local_lifecycle.state,
        LocalMemoryLifecycleState::Active
    );
    assert!(record.validate().is_ok());
}

#[test]
fn governed_projection_fixture_deserializes_and_validates() {
    let projection: GovernedMemoryRecordProjection =
        match serde_json::from_str(GOVERNED_PROJECTION_FIXTURE_JSON) {
            Ok(value) => value,
            Err(error) => panic!("projection fixture should deserialize: {error}"),
        };

    assert_eq!(
        projection.artifact_kind,
        MemoryArtifactKind::GovernedMemoryRecordProjection
    );
    assert_eq!(
        projection.target_artifact_kind,
        MemoryArtifactKind::SummaryOnlySessionContextEnvelope
    );
    assert!(projection.validate().is_ok());
}

#[test]
fn governed_import_and_projection_preserve_summary_only_compatibility() {
    let envelope = summary_only_fixture();
    let record = match GovernedMemoryRecord::import_summary_only_envelope(
        &envelope,
        GovernedMemoryRecordImportOptions {
            record_id: "memory-record-1".to_string(),
            imported_at_utc: "2026-03-23T00:00:00Z".to_string(),
        },
    ) {
        Ok(value) => value,
        Err(error) => panic!("summary-only import should succeed: {error}"),
    };

    assert_serialized_json_matches_fixture(&record, GOVERNED_RECORD_FIXTURE_JSON);

    let exported = match record.export_summary_only_envelope() {
        Ok(value) => value,
        Err(error) => panic!("summary-only export should succeed: {error}"),
    };
    assert_serialized_json_matches_fixture(&exported, SUMMARY_ONLY_FIXTURE_JSON);

    let projection = match record.project_summary_only_envelope() {
        Ok(value) => value,
        Err(error) => panic!("projection should succeed: {error}"),
    };
    assert_serialized_json_matches_fixture(&projection, GOVERNED_PROJECTION_FIXTURE_JSON);
}

#[test]
fn import_normalizes_offset_imported_at_for_deterministic_sort_keys() {
    let context = match SummaryOnlySessionContext::new(
        SessionContextScope::Workspace,
        "Portable summary only.",
    ) {
        Ok(value) => value,
        Err(error) => panic!("context should be valid: {error}"),
    };
    let envelope = match SummaryOnlySessionContextEnvelope::new(context) {
        Ok(value) => value,
        Err(error) => panic!("envelope should be valid: {error}"),
    };

    let record = match GovernedMemoryRecord::import_summary_only_envelope(
        &envelope,
        GovernedMemoryRecordImportOptions {
            record_id: "memory-record-offset-import".to_string(),
            imported_at_utc: "2026-03-23T01:00:00+01:00".to_string(),
        },
    ) {
        Ok(value) => value,
        Err(error) => panic!("offset import should succeed: {error}"),
    };

    assert_eq!(
        record.deterministic_sort_keys.scope_captured_at_record_id,
        "workspace|2026-03-23T00:00:00Z|memory-record-offset-import"
    );
}

#[test]
fn lifecycle_transitions_are_limited_to_the_governed_paths() {
    assert!(
        LocalMemoryLifecycleState::Active.can_transition_to(LocalMemoryLifecycleState::Superseded)
    );
    assert!(
        LocalMemoryLifecycleState::Active.can_transition_to(LocalMemoryLifecycleState::Tombstoned)
    );
    assert!(LocalMemoryLifecycleState::Superseded
        .can_transition_to(LocalMemoryLifecycleState::Tombstoned));
    assert!(!LocalMemoryLifecycleState::Active.can_transition_to(LocalMemoryLifecycleState::Active));
    assert!(
        !LocalMemoryLifecycleState::Superseded.can_transition_to(LocalMemoryLifecycleState::Active)
    );
    assert!(
        !LocalMemoryLifecycleState::Tombstoned.can_transition_to(LocalMemoryLifecycleState::Active)
    );

    let active_record = governed_record_fixture();
    let superseded = match active_record.supersede("memory-record-2") {
        Ok(value) => value,
        Err(error) => panic!("active -> superseded should succeed: {error}"),
    };
    assert_eq!(
        superseded
            .local_lifecycle
            .superseded_by_record_id
            .as_deref(),
        Some("memory-record-2")
    );

    let tombstoned_from_active = match active_record.tombstone(
        "2026-03-24T00:00:00Z",
        "Locally withdrawn after replacement review.",
    ) {
        Ok(value) => value,
        Err(error) => panic!("active -> tombstoned should succeed: {error}"),
    };
    assert_eq!(
        tombstoned_from_active.local_lifecycle.state,
        LocalMemoryLifecycleState::Tombstoned
    );

    let tombstoned_from_superseded = match superseded.tombstone(
        "2026-03-24T01:00:00Z",
        "Superseded record withdrawn from local circulation.",
    ) {
        Ok(value) => value,
        Err(error) => panic!("superseded -> tombstoned should succeed: {error}"),
    };
    assert_eq!(
        tombstoned_from_superseded.local_lifecycle.state,
        LocalMemoryLifecycleState::Tombstoned
    );
    assert_eq!(
        tombstoned_from_superseded
            .local_lifecycle
            .superseded_by_record_id
            .as_deref(),
        Some("memory-record-2")
    );

    assert_eq!(
        superseded.supersede("memory-record-3"),
        Err(MemoryValidationError::InvalidLifecycleTransition {
            from: "superseded",
            to: "superseded",
        })
    );
    assert_eq!(
        tombstoned_from_active.tombstone("2026-03-24T02:00:00Z", "Already tombstoned locally.",),
        Err(MemoryValidationError::InvalidLifecycleTransition {
            from: "tombstoned",
            to: "tombstoned",
        })
    );
}

#[test]
fn validation_rejects_durable_transcript_flags_and_oversized_lists() {
    let invalid_context = SummaryOnlySessionContext {
        scope: SessionContextScope::Run,
        representation: SessionContextRepresentation::SummaryOnly,
        summary: "short summary".to_string(),
        salient_facts: vec!["fact".to_string(); 17],
        instruction_context: Vec::new(),
        raw_transcript_persisted: true,
    };

    assert_eq!(
        invalid_context.validate(),
        Err(MemoryValidationError::TooManyItems {
            field: "sessionContext.salientFacts",
            max: 16,
            actual: 17,
        })
    );

    let transcript_flag_context = SummaryOnlySessionContext {
        scope: SessionContextScope::Run,
        representation: SessionContextRepresentation::SummaryOnly,
        summary: "short summary".to_string(),
        salient_facts: Vec::new(),
        instruction_context: Vec::new(),
        raw_transcript_persisted: true,
    };

    assert_eq!(
        transcript_flag_context.validate(),
        Err(MemoryValidationError::RawTranscriptPersistedMustBeFalse)
    );
}

#[test]
fn deserialization_rejects_unknown_transcript_fields() {
    let top_level_error = serde_json::from_str::<SummaryOnlySessionContextEnvelope>(
        r#"{
  "artifactKind": "summary-only-session-context-envelope",
  "capturedAtUtc": "2026-03-22T00:00:00Z",
  "rawTranscript": "forbidden",
  "sessionContext": {
    "scope": "workspace",
    "representation": "summary-only",
    "summary": "Portable summary only.",
    "rawTranscriptPersisted": false
  }
}"#,
    )
    .expect_err("top-level transcript property should be rejected");

    assert!(
        top_level_error
            .to_string()
            .contains("unknown field `rawTranscript`"),
        "unexpected error: {top_level_error}"
    );

    let nested_error = serde_json::from_str::<SummaryOnlySessionContextEnvelope>(
        r#"{
  "artifactKind": "summary-only-session-context-envelope",
  "capturedAtUtc": "2026-03-22T00:00:00Z",
  "sessionContext": {
    "scope": "workspace",
    "representation": "summary-only",
    "summary": "Portable summary only.",
    "rawTranscriptPersisted": false,
    "transcript": "forbidden"
  }
}"#,
    )
    .expect_err("nested transcript property should be rejected");

    assert!(
        nested_error
            .to_string()
            .contains("unknown field `transcript`"),
        "unexpected error: {nested_error}"
    );
}

#[test]
fn validation_rejects_invalid_captured_at_utc() {
    let envelope = SummaryOnlySessionContextEnvelope {
        artifact_kind: MemoryArtifactKind::SummaryOnlySessionContextEnvelope,
        request_id: None,
        run_id: None,
        captured_at_utc: Some("not-a-date-time".to_string()),
        session_context: SummaryOnlySessionContext {
            scope: SessionContextScope::Session,
            representation: SessionContextRepresentation::SummaryOnly,
            summary: "Portable summary only.".to_string(),
            salient_facts: Vec::new(),
            instruction_context: Vec::new(),
            raw_transcript_persisted: false,
        },
    };

    assert_eq!(
        envelope.validate(),
        Err(MemoryValidationError::InvalidDateTime {
            field: "capturedAtUtc",
        })
    );
}

#[test]
fn governed_record_deserialization_rejects_sort_key_drift() {
    let mut value: serde_json::Value = match serde_json::from_str(GOVERNED_RECORD_FIXTURE_JSON) {
        Ok(value) => value,
        Err(error) => panic!("fixture should parse: {error}"),
    };
    value["deterministicSortKeys"]["scopeCapturedAtRecordId"] =
        json!("workspace|2026-03-22T00:00:00Z|different-record");

    let error = serde_json::from_value::<GovernedMemoryRecord>(value)
        .expect_err("sort-key drift should be rejected");

    assert!(
        error
            .to_string()
            .contains("deterministicSortKeys.scopeCapturedAtRecordId"),
        "unexpected error: {error}"
    );
}

#[test]
fn governed_record_deserialization_accepts_equivalent_offset_timestamp_with_canonical_sort_key() {
    let mut value: serde_json::Value = match serde_json::from_str(GOVERNED_RECORD_FIXTURE_JSON) {
        Ok(value) => value,
        Err(error) => panic!("fixture should parse: {error}"),
    };
    value["provenance"]["capturedAtUtc"] = json!("2026-03-22T01:00:00+01:00");

    let record = match serde_json::from_value::<GovernedMemoryRecord>(value) {
        Ok(value) => value,
        Err(error) => panic!("canonical sort key should accept equivalent offset: {error}"),
    };

    assert_eq!(
        record.provenance.captured_at_utc.as_deref(),
        Some("2026-03-22T01:00:00+01:00")
    );
    assert!(record.validate().is_ok());
}

#[test]
fn import_and_deserialization_reject_oversized_record_ids() {
    let overlong_record_id = "r".repeat(MAX_MEMORY_RECORD_ID_LENGTH + 1);

    assert_eq!(
        GovernedMemoryRecord::import_summary_only_envelope(
            &summary_only_fixture(),
            GovernedMemoryRecordImportOptions {
                record_id: overlong_record_id.clone(),
                imported_at_utc: "2026-03-23T00:00:00Z".to_string(),
            },
        ),
        Err(MemoryValidationError::StringTooLong {
            field: "recordId",
            max: MAX_MEMORY_RECORD_ID_LENGTH,
            actual: MAX_MEMORY_RECORD_ID_LENGTH + 1,
        })
    );

    let mut projection_value: serde_json::Value =
        match serde_json::from_str(GOVERNED_PROJECTION_FIXTURE_JSON) {
            Ok(value) => value,
            Err(error) => panic!("projection fixture should parse: {error}"),
        };
    projection_value["sourceRecordId"] = json!(overlong_record_id);

    let error = serde_json::from_value::<GovernedMemoryRecordProjection>(projection_value)
        .expect_err("oversized sourceRecordId should be rejected");

    assert!(
        error
            .to_string()
            .contains("sourceRecordId exceeds max length 128"),
        "unexpected error: {error}"
    );
}

#[test]
fn deserialization_rejects_oversized_deterministic_sort_keys() {
    let overlong_sort_key = "s".repeat(MAX_MEMORY_SORT_KEY_LENGTH + 1);

    let mut record_value: serde_json::Value =
        match serde_json::from_str(GOVERNED_RECORD_FIXTURE_JSON) {
            Ok(value) => value,
            Err(error) => panic!("record fixture should parse: {error}"),
        };
    record_value["deterministicSortKeys"]["scopeCapturedAtRecordId"] =
        json!(overlong_sort_key.clone());

    let record_error = serde_json::from_value::<GovernedMemoryRecord>(record_value)
        .expect_err("oversized record sort key should be rejected");

    assert!(
        record_error
            .to_string()
            .contains("deterministicSortKeys.scopeCapturedAtRecordId exceeds max length 280"),
        "unexpected error: {record_error}"
    );

    let mut projection_value: serde_json::Value =
        match serde_json::from_str(GOVERNED_PROJECTION_FIXTURE_JSON) {
            Ok(value) => value,
            Err(error) => panic!("projection fixture should parse: {error}"),
        };
    projection_value["deterministicSortKey"] = json!(overlong_sort_key);

    let projection_error =
        serde_json::from_value::<GovernedMemoryRecordProjection>(projection_value)
            .expect_err("oversized projection sort key should be rejected");

    assert!(
        projection_error
            .to_string()
            .contains("deterministicSortKey exceeds max length 280"),
        "unexpected error: {projection_error}"
    );
}

#[test]
fn deserialization_rejects_schema_invalid_payloads() {
    let overlong_summary = "x".repeat(4_001);
    let too_many_salient_facts = (0..=16)
        .map(|index| format!("fact-{index}"))
        .collect::<Vec<_>>();

    let cases = vec![
        (
            "capturedAtUtc format",
            r#"{
  "artifactKind": "summary-only-session-context-envelope",
  "capturedAtUtc": "not-a-date-time",
  "sessionContext": {
    "scope": "workspace",
    "representation": "summary-only",
    "summary": "Portable summary only.",
    "rawTranscriptPersisted": false
  }
}"#
            .to_string(),
            "capturedAtUtc must be a valid RFC 3339 date-time",
        ),
        (
            "rawTranscriptPersisted const false",
            r#"{
  "artifactKind": "summary-only-session-context-envelope",
  "sessionContext": {
    "scope": "workspace",
    "representation": "summary-only",
    "summary": "Portable summary only.",
    "rawTranscriptPersisted": true
  }
}"#
            .to_string(),
            "sessionContext.rawTranscriptPersisted must be false",
        ),
        (
            "summary string bounds",
            format!(
                r#"{{
  "artifactKind": "summary-only-session-context-envelope",
  "sessionContext": {{
    "scope": "workspace",
    "representation": "summary-only",
    "summary": "{overlong_summary}",
    "rawTranscriptPersisted": false
  }}
}}"#,
            ),
            "sessionContext.summary exceeds max length",
        ),
        (
            "salientFacts array bounds",
            format!(
                r#"{{
  "artifactKind": "summary-only-session-context-envelope",
  "sessionContext": {{
    "scope": "workspace",
    "representation": "summary-only",
    "summary": "Portable summary only.",
    "salientFacts": {},
    "rawTranscriptPersisted": false
  }}
}}"#,
                serde_json::to_string(&too_many_salient_facts)
                    .expect("salientFacts fixture should serialize")
            ),
            "sessionContext.salientFacts exceeds max items",
        ),
    ];

    for (name, json, expected_message) in cases {
        let error = serde_json::from_str::<SummaryOnlySessionContextEnvelope>(&json)
            .expect_err("invalid payload should be rejected");

        assert!(
            error.to_string().contains(expected_message),
            "{name} should fail during deserialization with `{expected_message}`, got `{error}`"
        );
    }
}

fn summary_only_fixture() -> SummaryOnlySessionContextEnvelope {
    match serde_json::from_str(SUMMARY_ONLY_FIXTURE_JSON) {
        Ok(value) => value,
        Err(error) => panic!("summary-only fixture should deserialize: {error}"),
    }
}

fn governed_record_fixture() -> GovernedMemoryRecord {
    match serde_json::from_str(GOVERNED_RECORD_FIXTURE_JSON) {
        Ok(value) => value,
        Err(error) => panic!("governed record fixture should deserialize: {error}"),
    }
}

fn assert_serialized_json_matches_fixture<T>(value: &T, expected_fixture: &str)
where
    T: Serialize,
{
    let actual = match serde_json::to_value(value) {
        Ok(value) => value,
        Err(error) => panic!("value should serialize: {error}"),
    };
    let expected: serde_json::Value = match serde_json::from_str(expected_fixture) {
        Ok(value) => value,
        Err(error) => panic!("fixture should parse: {error}"),
    };

    assert_eq!(actual, expected);
}
