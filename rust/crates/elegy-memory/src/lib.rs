use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub const SUMMARY_ONLY_SESSION_CONTEXT_ARTIFACT_KIND: &str =
    "summary-only-session-context-envelope";
pub const SUMMARY_ONLY_REPRESENTATION: &str = "summary-only";
pub const MAX_SUMMARY_LENGTH: usize = 4_000;
pub const MAX_SALIENT_FACTS: usize = 16;
pub const MAX_INSTRUCTION_CONTEXT_ITEMS: usize = 8;
pub const MAX_CONTEXT_ITEM_LENGTH: usize = 280;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryArtifactKind {
    #[default]
    #[serde(rename = "summary-only-session-context-envelope")]
    SummaryOnlySessionContextEnvelope,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionContextScope {
    Run,
    Session,
    Workspace,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionContextRepresentation {
    #[default]
    #[serde(rename = "summary-only")]
    SummaryOnly,
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
    #[error(
        "sessionContext.rawTranscriptPersisted must be false for portable summary-only artifacts"
    )]
    RawTranscriptPersistedMustBeFalse,
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

fn validate_optional_rfc3339(
    field: &'static str,
    value: &Option<String>,
) -> Result<(), MemoryValidationError> {
    if let Some(value) = value {
        OffsetDateTime::parse(value, &Rfc3339)
            .map_err(|_| MemoryValidationError::InvalidDateTime { field })?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_shape_deserializes_and_validates() {
        let envelope: SummaryOnlySessionContextEnvelope = match serde_json::from_str(FIXTURE_JSON) {
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
            Some(SUMMARY_ONLY_SESSION_CONTEXT_ARTIFACT_KIND)
        );
        assert_eq!(
            value
                .get("sessionContext")
                .and_then(|entry| entry.get("representation"))
                .and_then(serde_json::Value::as_str),
            Some(SUMMARY_ONLY_REPRESENTATION)
        );
        assert!(value.get("requestId").is_none());
    }

    #[test]
    fn validation_rejects_durable_transcript_flags_and_oversized_lists() {
        let invalid_context = SummaryOnlySessionContext {
            scope: SessionContextScope::Run,
            representation: SessionContextRepresentation::SummaryOnly,
            summary: "short summary".to_string(),
            salient_facts: vec!["fact".to_string(); MAX_SALIENT_FACTS + 1],
            instruction_context: Vec::new(),
            raw_transcript_persisted: true,
        };

        assert_eq!(
            invalid_context.validate(),
            Err(MemoryValidationError::TooManyItems {
                field: "sessionContext.salientFacts",
                max: MAX_SALIENT_FACTS,
                actual: MAX_SALIENT_FACTS + 1,
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
            top_level_error.to_string().contains("unknown field `rawTranscript`"),
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
            nested_error.to_string().contains("unknown field `transcript`"),
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
    fn deserialization_rejects_schema_invalid_payloads() {
        let overlong_summary = "x".repeat(MAX_SUMMARY_LENGTH + 1);
        let too_many_salient_facts = (0..=MAX_SALIENT_FACTS)
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
                .unwrap_err();

            assert!(
                error.to_string().contains(expected_message),
                "{name} should fail during deserialization with `{expected_message}`, got `{error}`"
            );
        }
    }

    const FIXTURE_JSON: &str = r#"{
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
}"#;
}
