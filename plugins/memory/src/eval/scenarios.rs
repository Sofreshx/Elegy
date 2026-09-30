//! Bounded, provider-free qualification experiments over disposable synthetic stores.
use std::{collections::HashMap, path::PathBuf};

use chrono::{Duration, TimeZone, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    recall::{
        contextual_recall, record_recall_feedback, RecallConfig, RecallFeedbackRequest,
        RecallJudgment, RecallMode, RecallRequest,
    },
    Memory, MemoryScope, MemoryState, MemoryStore, MemoryType, ProvenanceLevel, SensitivityLevel,
    SqliteMemoryStore,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ScenarioReport {
    pub checks: Vec<ScenarioCheck>,
    pub measurements: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ScenarioCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

fn check(name: &str, passed: bool, detail: impl Into<String>) -> ScenarioCheck {
    ScenarioCheck {
        name: name.into(),
        passed,
        detail: detail.into(),
    }
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!("elegy-qualification-{}", Uuid::new_v4()));
        std::fs::create_dir(&path).map_err(|e| e.to_string())?;
        Ok(Self(path))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only this freshly-created, uniquely-owned directory is removed.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn memory(index: u128, text: &str) -> Result<Memory, String> {
    let time = Utc
        .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
        .single()
        .ok_or("invalid fixture timestamp")?
        + Duration::days(index as i64);
    Ok(Memory {
        id: Uuid::from_u128(index),
        content: text.into(),
        summary: None,
        scope: MemoryScope::Workspace,
        memory_type: MemoryType::Observation,
        provenance: ProvenanceLevel::UserStated,
        importance_score: 0.5,
        reliability_score: 1.0,
        sensitivity: SensitivityLevel::Low,
        state: MemoryState::Active,
        tags: vec![],
        status: None,
        custom_metadata: HashMap::new(),
        access_count: 0,
        corroboration_count: 0,
        embedding_stale: true,
        created_at: time,
        updated_at: time,
        last_accessed_at: None,
        tenant_id: None,
        user_id: None,
        agent_id: None,
    })
}

fn insert(store: &SqliteMemoryStore, memory: Memory) -> Result<(), String> {
    crate::runtime::block_on(store.store(memory))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn run(claim: &str) -> Result<ScenarioReport, String> {
    match claim {
        "recall.contract" => recall_contract(),
        "forgetting.retention" => forgetting_retention(),
        _ => Err(format!("unsupported qualification claim: {claim}")),
    }
}

fn recall_contract() -> Result<ScenarioReport, String> {
    let fixture = Fixture::new()?;
    let db = fixture.0.join("source.sqlite3");
    let store = SqliteMemoryStore::new(&db, MemoryScope::Workspace).map_err(|e| e.to_string())?;
    insert(&store, memory(1, "Cobalt uses blue.")?)?;
    let mut owned = memory(2, "Violet belongs to agent alpha.")?;
    owned.agent_id = Some("alpha".into());
    insert(&store, owned)?;
    let mut sensitive = memory(3, "Scarlet is confidential.")?;
    sensitive.sensitivity = SensitivityLevel::High;
    insert(&store, sensitive)?;
    let mut tenant = memory(4, "Amber belongs to tenant alpha.")?;
    tenant.tenant_id = Some("alpha".into());
    insert(&store, tenant)?;
    let mut user = memory(5, "Indigo belongs to user alpha.")?;
    user.user_id = Some("alpha".into());
    insert(&store, user)?;
    drop(store);
    let broader = SqliteMemoryStore::new(&db, MemoryScope::User).map_err(|e| e.to_string())?;
    let mut outside = memory(6, "Emerald belongs to user scope.")?;
    outside.scope = MemoryScope::User;
    insert(&broader, outside)?;
    drop(broader);
    // Closed writers checkpoint WAL before the byte comparison.
    let before = std::fs::read(&db).map_err(|e| e.to_string())?;
    let config = RecallConfig {
        version: 1,
        project_root: fixture.0.clone(),
        db_path: db.clone(),
        state_path: fixture.0.join("journal.sqlite3"),
        domain: "synthetic-qualification".into(),
        scopes: vec![MemoryScope::Workspace],
        mode: RecallMode::Inject,
        agent_id: None,
        max_sensitivity: SensitivityLevel::Low,
        max_context_tokens: 600,
        min_similarity: 0.0,
        learning_enabled: false,
        include_recent_context: false,
        transcript_root: None,
    };
    let request = |turn: &str, prompt: &str| RecallRequest {
        cwd: fixture.0.clone(),
        session_id: "synthetic-session".into(),
        turn_id: turn.into(),
        prompt: prompt.into(),
        recent_context: vec![],
        embedding: None,
    };
    let first_request = request("first", "Cobalt");
    let first = contextual_recall(&config, &first_request).map_err(|e| e.to_string())?;
    let mut checks = vec![check(
        "positive-control",
        first.recalls.len() == 1 && first.recalls[0].id == Uuid::from_u128(1).to_string(),
        "Synthetic cobalt fact is recalled.",
    )];
    checks.push(check(
        "context-budget",
        !first.additional_context.is_empty() && first.additional_context.len() <= 600,
        format!("{} UTF-8 bytes", first.additional_context.len()),
    ));
    for (name, keyword) in [
        ("agent-owner-isolation", "Violet"),
        ("sensitivity-isolation", "Scarlet"),
        ("tenant-owner-isolation", "Amber"),
        ("user-owner-isolation", "Indigo"),
        ("scope-isolation", "Emerald"),
    ] {
        let response =
            contextual_recall(&config, &request(name, keyword)).map_err(|e| e.to_string())?;
        checks.push(check(
            name,
            response.recalls.is_empty(),
            format!("{} selected records", response.recalls.len()),
        ));
    }
    let retry = contextual_recall(&config, &first_request).map_err(|e| e.to_string())?;
    checks.push(check(
        "turn-retry",
        retry.event_id == first.event_id
            && retry.recalls.is_empty()
            && retry.additional_context.is_empty(),
        "Same turn retains event identity without duplicate injection.",
    ));
    if let (Some(event), Some(item)) = (first.event_id.as_ref(), first.recalls.first()) {
        let mut feedback = RecallFeedbackRequest {
            cwd: fixture.0.clone(),
            session_id: "synthetic-session".into(),
            event_id: event.clone(),
            memory_id: item.id.clone(),
            judgment: RecallJudgment::Dismiss,
        };
        // Injection already suppresses this version. Remove only this disposable
        // fixture's automatic suppression to isolate the feedback write below.
        let journal = Connection::open(&config.state_path).map_err(|e| e.to_string())?;
        journal
            .execute("DELETE FROM suppressed", [])
            .map_err(|e| e.to_string())?;
        drop(journal);
        let recorded = record_recall_feedback(&config, &feedback).map_err(|e| e.to_string())?;
        let repeated = record_recall_feedback(&config, &feedback).map_err(|e| e.to_string())?;
        checks.push(check(
            "feedback-retry",
            recorded.recorded && !repeated.recorded,
            "Dismiss recorded once.",
        ));
        feedback.judgment = RecallJudgment::Useful;
        checks.push(check(
            "conflicting-feedback",
            matches!(record_recall_feedback(&config, &feedback),
                Err(crate::StoreError::Validation(message)) if message == "recall feedback already has a different judgment"),
            "A useful judgment cannot replace the recorded dismiss.",
        ));
        let next = contextual_recall(&config, &request("after-dismiss", "Cobalt"))
            .map_err(|e| e.to_string())?;
        checks.push(check("injected-dismiss-suppression", next.recalls.is_empty(),
            "Dismiss restores suppression after removing automatic injection suppression in the disposable fixture."));
        let mut other_session = request("other-session", "Cobalt");
        other_session.session_id = "fresh-synthetic-session".into();
        let fresh = contextual_recall(&config, &other_session).map_err(|e| e.to_string())?;
        checks.push(check(
            "suppression-session-boundary",
            fresh.recalls.iter().any(|r| r.id == item.id),
            "A new session can recall the fact again.",
        ));
    } else {
        for name in [
            "feedback-retry",
            "conflicting-feedback",
            "injected-dismiss-suppression",
            "suppression-session-boundary",
        ] {
            checks.push(check(
                name,
                false,
                "Positive control produced no item to exercise feedback.",
            ));
        }
    }
    let journal = Connection::open(&config.state_path).map_err(|e| e.to_string())?;
    let identity: i32 = journal
        .pragma_query_value(None, "application_id", |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let version: i32 = journal
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|e| e.to_string())?;
    checks.push(check(
        "journal-header",
        identity == 0x45475243 && version == 1,
        format!("applicationId={identity}; userVersion={version}"),
    ));
    journal
        .pragma_update(None, "user_version", 99)
        .map_err(|e| e.to_string())?;
    checks.push(check(
        "journal-version-rejection",
        matches!(contextual_recall(&config, &request("bad-version", "Cobalt")),
            Err(crate::StoreError::Validation(message)) if message == "unsupported recall journal schema"),
        "Unsupported journal version rejected.",
    ));
    journal
        .pragma_update(None, "user_version", 1)
        .map_err(|e| e.to_string())?;
    journal
        .pragma_update(None, "application_id", 123)
        .map_err(|e| e.to_string())?;
    checks.push(check(
        "journal-identity-rejection",
        matches!(contextual_recall(&config, &request("bad-identity", "Cobalt")),
            Err(crate::StoreError::Validation(message)) if message == "unsupported recall journal schema"),
        "Foreign journal identity rejected.",
    ));
    drop(journal);
    checks.push(check(
        "source-unchanged",
        before == std::fs::read(&db).map_err(|e| e.to_string())?,
        "Closed source database bytes unchanged by recall and feedback.",
    ));
    Ok(ScenarioReport {
        checks,
        measurements: json!({"fixture": "recall-contract-v1", "journalApplicationId": identity,
        "journalVersion": version, "contextBytes": first.additional_context.len(), "sourceBytes": before.len(),
        "scope": "synthetic library contract; does not attest a host installation or cross-client persistence"}),
    })
}

fn forgetting_retention() -> Result<ScenarioReport, String> {
    let fixture = Fixture::new()?;
    let mut checks = Vec::new();
    let mut observations = Vec::new();
    let labelled = [
        Uuid::from_u128(1).to_string(),
        Uuid::from_u128(2).to_string(),
    ];
    for policy in [
        "importance-reliability",
        "fifo",
        "lru",
        "priority-decay",
        "random-drop",
    ] {
        let db = fixture.0.join(format!("{policy}.sqlite3"));
        let store =
            SqliteMemoryStore::new(&db, MemoryScope::Workspace).map_err(|e| e.to_string())?;
        for index in 1..=6 {
            let important = index <= 2;
            let mut item = memory(
                index,
                if important {
                    "Keep the signed release requirement."
                } else {
                    "Temporary draft detail."
                },
            )?;
            item.importance_score = if important { 0.95 } else { 0.1 };
            // Reverse access order deliberately distinguishes FIFO and LRU.
            item.last_accessed_at = Some(item.created_at + Duration::days(20 - 2 * index as i64));
            insert(&store, item)?;
        }
        let connection = Connection::open(&db).map_err(|e| e.to_string())?;
        for (key, value) in [("budget_active_max", "2"), ("forgetting_policy", policy)] {
            // Exercise the real configured default for the default-policy claim.
            if key == "forgetting_policy" && policy == "importance-reliability" {
                continue;
            }
            connection
                .execute(
                    "INSERT OR REPLACE INTO scope_config(key,value) VALUES(?1,?2)",
                    [key, value],
                )
                .map_err(|e| e.to_string())?;
        }
        let execution_started_at = Utc::now();
        let resolved_policy = store
            .configured_forgetting_policy_name()
            .map_err(|e| e.to_string())?;
        let (demoted, deleted) = store.enforce_budget().map_err(|e| e.to_string())?;
        let execution_completed_at = Utc::now();
        let mut statement = connection
            .prepare("SELECT id FROM memories WHERE state='active' ORDER BY id")
            .map_err(|e| e.to_string())?;
        let retained: Vec<String> = statement
            .query_map([], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        let relevant = retained.iter().filter(|id| labelled.contains(id)).count();
        checks.push(check(
            &format!("{policy}.count-budget"),
            retained.len() == 2 && demoted == 4 && deleted == 0,
            format!(
                "{} retained, {demoted} demoted, {deleted} deleted",
                retained.len()
            ),
        ));
        if policy == "importance-reliability" {
            checks.push(check(
                "default-policy-identity",
                resolved_policy == "importance-reliability",
                format!("Unconfigured store resolves to {resolved_policy}."),
            ));
            checks.push(check(
                "default-labelled-retention",
                relevant == labelled.len(),
                "Default retains both labelled important facts in this fixture.",
            ));
        }
        observations.push(
            json!({"policy":policy,"resolvedPolicy":resolved_policy,"retainedIds":retained,"demoted":demoted,"deleted":deleted,
            "executionStartedAt":execution_started_at,"executionCompletedAt":execution_completed_at,
            "precision":if retained.is_empty() {0.0} else {relevant as f64 / retained.len() as f64},
            "recall":relevant as f64 / labelled.len() as f64}),
        );
    }
    Ok(ScenarioReport {
        checks,
        measurements: json!({"fixture":"forgetting-retention-v1", "labelledImportantIds":labelled,
        "activeBudget":2,"policies":observations,"scope":"fixed synthetic fixture; no universal policy winner",
        "clockLimitation":"priority-decay uses the store's execution clock with fixed fixture timestamps"}),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recall_contract_exercises_real_store_boundaries() {
        let report = run("recall.contract").expect("scenario infrastructure");
        assert!(
            report.checks.iter().all(|c| c.passed),
            "{:#?}",
            report.checks
        );
        assert!(report
            .checks
            .iter()
            .any(|c| c.name == "journal-version-rejection"));
        assert!(report
            .checks
            .iter()
            .any(|c| c.name == "conflicting-feedback"));
    }

    #[test]
    fn forgetting_records_all_policies_and_counterexamples() {
        let report = run("forgetting.retention").expect("scenario infrastructure");
        assert!(
            report.checks.iter().all(|c| c.passed),
            "{:#?}",
            report.checks
        );
        let policies = report.measurements["policies"]
            .as_array()
            .expect("observations");
        assert_eq!(policies.len(), 5);
        let fifo = policies
            .iter()
            .find(|p| p["policy"] == "fifo")
            .expect("fifo");
        assert_eq!(fifo["recall"], json!(0.0));
        let default = policies
            .iter()
            .find(|p| p["policy"] == "importance-reliability")
            .expect("default");
        assert_eq!(default["recall"], json!(1.0));
    }

    #[test]
    fn unknown_claim_is_not_executed() {
        assert!(run("arbitrary.command").is_err());
    }
}
