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
    POISONING_QUARANTINED_AT_METADATA_KEY, POISONING_REMEDIATION_METADATA_KEY, QUARANTINED_STATUS,
    SHARED_REVIEW_STATUS,
};
use crate::{
    CorrectionDisposition, ElegyArchive, EmbeddingError, EmbeddingProvider, ExportFormat, Memory,
    MemoryFilter, MemoryId, MemoryScope, MemoryState, MemoryStore, MemoryType, MetadataUpdate,
    OptionalFieldUpdate, PoisoningAlert, PoisoningAlertType, ProvenanceLevel, ResolutionStatus,
    ScopeConfig, SearchQuery, SensitivityLevel, ShareConfig,
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
                custom_metadata: Some(HashMap::from([("source".to_string(), "notes".to_string())])),
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
    let mut less_trusted = sample_memory(MemoryScope::Workspace, ProvenanceLevel::AgentInferred);
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

    let mut semantic_priority = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
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

    let mut higher_similarity = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    higher_similarity.content = "higher semantic match".to_string();
    higher_similarity.importance_score = 0.5;
    higher_similarity.reliability_score = 1.0;
    let higher_similarity_id = higher_similarity.id;

    let mut higher_importance = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
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
            &embedding_with_query_similarities(&[0.05, 0.05, 0.05, 0.05, 0.05, 0.05, 0.614_608_9]),
        )
        .await
        .expect("store target embedding");
    fixture
        .store
        .store_embedding(
            &hub_id,
            &embedding_with_query_similarities(&[0.30, 0.29, 0.28, 0.27, 0.26, 0.25, 0.583_138_6]),
        )
        .await
        .expect("store hub embedding");
    fixture
        .store
        .store_embedding(
            &portrait_alt_id,
            &embedding_with_query_similarities(&[0.10, 0.10, 0.10, 0.10, 0.10, 0.10, 0.594_978_6]),
        )
        .await
        .expect("store alternate portrait embedding");
    fixture
        .store
        .store_embedding(
            &soup_id,
            &embedding_with_query_similarities(&[0.08, 0.08, 0.08, 0.08, 0.08, 0.08, 0.563_142_36]),
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
            > super::RETRIEVAL_SECONDARY_FADE_THRESHOLD
    );
    assert!(
        target_ranked.1.total_score < hub_ranked.1.total_score,
        "raw score should still favor the warmed hub to prove the fade guard is active"
    );
    assert!(target_ranked.0.score > hub_ranked.0.score);
    assert_eq!(target_ranked.1.secondary_fade_factor, 1.0);
    assert_eq!(hub_ranked.1.secondary_fade_factor, 0.0);
}

#[tokio::test]
async fn continuous_fade_neutralizes_recency_only_overrides_past_threshold() {
    let fixture = test_fixture();
    set_scope_config(&fixture.store, "similarity_weight", "0.4");
    set_scope_config(&fixture.store, "recency_weight", "1.0");
    set_scope_config(&fixture.store, "access_weight", "0.0");
    set_scope_config(&fixture.store, "priority_weight", "0.0");

    let mut semantic_winner = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    semantic_winner.content = "semantic winner".to_string();
    let semantic_winner_id = semantic_winner.id;

    let mut fresh_runner_up = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
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
            > super::RETRIEVAL_SECONDARY_FADE_THRESHOLD
    );
    assert!(
        semantic_ranked.1.total_score < fresh_ranked.1.total_score,
        "raw score should favor recency to prove the structural guard is active"
    );
    assert!(semantic_ranked.0.score > fresh_ranked.0.score);
    assert_eq!(fresh_ranked.1.secondary_fade_factor, 0.0);
}

#[tokio::test]
async fn continuous_fade_still_allows_recency_refinement_inside_threshold() {
    let fixture = test_fixture();
    set_scope_config(&fixture.store, "similarity_weight", "0.4");
    set_scope_config(&fixture.store, "recency_weight", "1.0");
    set_scope_config(&fixture.store, "access_weight", "0.0");
    set_scope_config(&fixture.store, "priority_weight", "0.0");

    let mut slightly_better_but_old =
        sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    slightly_better_but_old.content = "slightly better but old".to_string();
    let slightly_better_but_old_id = slightly_better_but_old.id;

    let mut fresh_quasi_tie = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
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
        .store_embedding(&fresh_quasi_tie_id, &embedding_with_similarity(0.615))
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
            < super::RETRIEVAL_SECONDARY_FADE_THRESHOLD
    );
    assert_eq!(old_ranked.1.secondary_fade_factor, 1.0);
    assert!(fresh_ranked.1.secondary_fade_factor > 0.0);
}

#[tokio::test]
async fn continuous_fade_neutralizes_priority_only_overrides_past_threshold() {
    let fixture = test_fixture();
    set_scope_config(&fixture.store, "similarity_weight", "0.1");
    set_scope_config(&fixture.store, "recency_weight", "0.0");
    set_scope_config(&fixture.store, "access_weight", "0.0");
    set_scope_config(&fixture.store, "priority_weight", "0.9");

    let mut semantic_winner = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    semantic_winner.content = "semantic winner".to_string();
    semantic_winner.importance_score = 0.5;
    semantic_winner.reliability_score = 1.0;
    let semantic_winner_id = semantic_winner.id;

    let mut boosted_runner_up = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
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
            > super::RETRIEVAL_SECONDARY_FADE_THRESHOLD
    );
    assert!(
        semantic_ranked.1.total_score < boosted_ranked.1.total_score,
        "raw score should favor the priority-heavy runner up to prove the structural guard is active"
    );
    assert!(semantic_ranked.0.score > boosted_ranked.0.score);
    assert_eq!(boosted_ranked.1.secondary_fade_factor, 0.0);
}

#[test]
fn continuous_secondary_fade_is_monotonic_and_zero_at_threshold() {
    let quarter =
        super::compute_secondary_fade_factor(super::RETRIEVAL_SECONDARY_FADE_THRESHOLD * 0.25);
    let half =
        super::compute_secondary_fade_factor(super::RETRIEVAL_SECONDARY_FADE_THRESHOLD * 0.5);
    let three_quarters =
        super::compute_secondary_fade_factor(super::RETRIEVAL_SECONDARY_FADE_THRESHOLD * 0.75);

    assert_eq!(super::compute_secondary_fade_factor(0.0), 1.0);
    assert!(quarter < 1.0 && quarter > half);
    assert!(half > three_quarters);
    assert!(three_quarters > 0.0);
    assert_eq!(
        super::compute_secondary_fade_factor(super::RETRIEVAL_SECONDARY_FADE_THRESHOLD),
        0.0
    );
    assert_eq!(
        super::compute_secondary_fade_factor(super::RETRIEVAL_SECONDARY_FADE_THRESHOLD * 1.5),
        0.0
    );
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

    let mut active_decision = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    active_decision.content = "migration decision for apollo workspace".to_string();
    active_decision.memory_type = MemoryType::Decision;
    let active_decision_id = active_decision.id;

    let mut active_fact = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    active_fact.content = "apollo workspace fact sheet".to_string();
    active_fact.memory_type = MemoryType::Fact;

    let mut dormant_decision = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
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
        "ollama not reachable at http://127.0.0.1:11434: request timed out after 30s".to_string(),
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
        "openai not reachable at https://api.openai.com: request timed out after 30s".to_string(),
    ))
    .expect("offline openai errors should produce a degradation warning");

    assert_eq!(
        warning,
        "OpenAI not reachable at https://api.openai.com, storing without embeddings. Run reembed later."
    );

    let invalid_api_key_warning = super::embedding_degradation_warning(&EmbeddingError::Provider(
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
        "openai returned 429 Too Many Requests: rate limited, try again later (...)".to_string(),
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
    let user_store = SqliteMemoryStore::new(&fixture.path, MemoryScope::User).expect("user store");
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
    let user_store = SqliteMemoryStore::new(&fixture.path, MemoryScope::User).expect("user store");
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
    let store = SqliteMemoryStore::new(&path, MemoryScope::Workspace).expect("create sqlite store");
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
                &embedding_with_similarity(realistic_benchmark_similarity(case.label, spec.label)),
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
        (weights["similarity_weight"] - f64::from(crate::types::DEFAULT_SIMILARITY_WEIGHT)).abs()
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
        (f64::from(scope_config.similarity_weight) - report.effective_weights.similarity_weight)
            .abs()
            < 1e-6
    );
    assert!(
        (f64::from(scope_config.recency_weight) - report.effective_weights.recency_weight).abs()
            < 1e-6
    );
    assert!(
        (f64::from(scope_config.access_weight) - report.effective_weights.access_weight).abs()
            < 1e-6
    );
    assert!(
        (f64::from(scope_config.priority_weight) - report.effective_weights.priority_weight).abs()
            < 1e-6
    );
}

#[tokio::test]
async fn feedback_learning_updates_live_search_scoring_via_scope_config() {
    let fixture = test_fixture();

    let mut similarity_favored = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    similarity_favored.content = "apollo rollout checklist canonical".to_string();
    similarity_favored.importance_score = 0.35;
    similarity_favored.reliability_score = 1.0;
    let similarity_favored_id = similarity_favored.id;

    let mut priority_favored = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
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
    assert_eq!(before_similarity.1.secondary_fade_factor, 1.0);
    assert_eq!(before_priority.1.secondary_fade_factor, 0.0);

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
    let user_store =
        SqliteMemoryStore::new_with_embedding_provider(&fixture.path, MemoryScope::User, provider)
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

// ── enforce_budget characterization tests ──────────────────────────
//
// enforce_budget had zero test coverage before this pass. These tests
// pin its behavior — including the Phase 2 over-deletion bug — so a
// later change to make eviction ranking pluggable (and to fix the
// storage-cap loop) shows up as an intentional, reviewable diff rather
// than silent drift.

#[tokio::test]
async fn enforce_budget_phase1_demotes_lowest_scoring_active_memories_first() {
    let fixture = test_fixture();
    set_scope_config(&fixture.store, "budget_active_max", "3");

    let mut scored_ids = Vec::new();
    for i in 0..5u32 {
        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        memory.importance_score = 0.1 * (i + 1) as f32;
        memory.reliability_score = 1.0;
        let id = fixture
            .store
            .store(memory)
            .await
            .expect("store active memory");
        scored_ids.push((id, 0.1 * (i + 1) as f32));
    }

    let (dormanted, deleted) = fixture.store.enforce_budget().expect("enforce budget");
    assert_eq!(dormanted, 2, "excess over budget_active_max=3 is 2");
    assert_eq!(deleted, 0, "storage cap was not exceeded");

    scored_ids.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    for (id, score) in scored_ids.iter().take(2) {
        let memory = fixture
            .store
            .get(id)
            .await
            .expect("get memory")
            .expect("memory exists");
        assert_eq!(
            memory.state,
            MemoryState::Dormant,
            "lowest-scoring memory (score={score}) should have been demoted"
        );
    }
    for (id, score) in scored_ids.iter().skip(2) {
        let memory = fixture
            .store
            .get(id)
            .await
            .expect("get memory")
            .expect("memory exists");
        assert_eq!(
            memory.state,
            MemoryState::Active,
            "higher-scoring memory (score={score}) should remain active"
        );
    }
}

#[tokio::test]
async fn enforce_budget_phase1_is_a_noop_when_active_count_is_within_budget() {
    let fixture = test_fixture();
    set_scope_config(&fixture.store, "budget_active_max", "10");

    for _ in 0..3 {
        let memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        fixture
            .store
            .store(memory)
            .await
            .expect("store active memory");
    }

    let (dormanted, deleted) = fixture.store.enforce_budget().expect("enforce budget");
    assert_eq!(dormanted, 0);
    assert_eq!(deleted, 0);
}

#[tokio::test]
async fn enforce_budget_phase1_ranks_by_importance_times_reliability_not_importance_alone() {
    let fixture = test_fixture();
    set_scope_config(&fixture.store, "budget_active_max", "1");

    // Higher importance but much lower reliability should score lower
    // than the reverse, under today's importance * reliability ranking.
    let mut low_reliability = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    low_reliability.importance_score = 0.9;
    low_reliability.reliability_score = 0.1;
    let low_reliability_id = fixture
        .store
        .store(low_reliability)
        .await
        .expect("store low reliability memory");

    let mut high_reliability = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    high_reliability.importance_score = 0.3;
    high_reliability.reliability_score = 0.9;
    let high_reliability_id = fixture
        .store
        .store(high_reliability)
        .await
        .expect("store high reliability memory");

    let (dormanted, _deleted) = fixture.store.enforce_budget().expect("enforce budget");
    assert_eq!(dormanted, 1);

    let demoted = fixture
        .store
        .get(&low_reliability_id)
        .await
        .expect("get memory")
        .expect("memory exists");
    assert_eq!(
        demoted.state,
        MemoryState::Dormant,
        "0.9*0.1=0.09 should score lower than 0.3*0.9=0.27 and be demoted"
    );
    let kept = fixture
        .store
        .get(&high_reliability_id)
        .await
        .expect("get memory")
        .expect("memory exists");
    assert_eq!(kept.state, MemoryState::Active);
}

#[tokio::test]
async fn enforce_budget_phase2_deletes_every_dormant_memory_when_cap_is_unreachably_low() {
    // A storage_cap_mb of 0 is unreachable regardless of ranking or
    // measurement strategy: live bytes are never <= 0 while any schema
    // or data exists. Both the pre-fix (raw page_count) and post-fix
    // (live, freelist-adjusted) measurements delete the entire dormant
    // set here, so this scenario does NOT distinguish the two — see
    // `enforce_budget_phase2_stops_deleting_once_live_bytes_are_under_the_cap`
    // for the test that actually exercises the fix.
    let fixture = test_fixture();
    set_scope_config(&fixture.store, "storage_cap_mb", "0");

    let mut dormant_ids = Vec::new();
    for _ in 0..5 {
        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        memory.state = MemoryState::Dormant;
        let id = fixture
            .store
            .store(memory)
            .await
            .expect("store dormant memory");
        dormant_ids.push(id);
    }

    let (dormanted, deleted) = fixture.store.enforce_budget().expect("enforce budget");
    assert_eq!(dormanted, 0, "no active memories were seeded");
    assert_eq!(
        deleted, 5,
        "current implementation deletes the entire dormant set once over cap"
    );

    for id in &dormant_ids {
        let memory = fixture.store.get(id).await.expect("get memory");
        assert!(
            memory.is_none(),
            "every dormant memory should have been hard-deleted"
        );
    }
}

#[tokio::test]
async fn enforce_budget_phase2_stops_deleting_once_live_bytes_are_under_the_cap() {
    // This is the assertion that changes from the pre-fix baseline: with
    // a *reachable* cap, Phase 2 must stop once live bytes (page_count -
    // freelist_count, not raw page_count) drop under it, rather than
    // deleting the entire dormant set. Each memory's content is made
    // large enough (well beyond storage_cap_mb's whole-MB resolution)
    // that deleting one measurably frees pages back to the freelist.
    let fixture = test_fixture();

    const MEMORY_COUNT: u32 = 4;
    const CONTENT_BYTES: usize = 3 * 1024 * 1024;

    let mut scored_ids = Vec::new();
    for i in 0..MEMORY_COUNT {
        let mut memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
        memory.state = MemoryState::Dormant;
        memory.importance_score = 0.1 * (i + 1) as f32;
        memory.reliability_score = 1.0;
        memory.content = "x".repeat(CONTENT_BYTES);
        let id = fixture
            .store
            .store(memory)
            .await
            .expect("store dormant memory");
        scored_ids.push((id, 0.1 * (i + 1) as f32));
    }

    let live_bytes_before = fixture
        .store
        .with_connection(|connection| super::live_storage_bytes(connection))
        .expect("measure live bytes before enforcement");

    // Set a cap comfortably below current usage but well above zero, so
    // reaching it does not require deleting the whole dormant set.
    let headroom_bytes = 2 * CONTENT_BYTES as u64;
    assert!(
        live_bytes_before > headroom_bytes,
        "test data (before={live_bytes_before}) must exceed the chosen headroom ({headroom_bytes}) \
         for the cap to be reachable without deleting everything"
    );
    let cap_mb = ((live_bytes_before - headroom_bytes) / (1024 * 1024)).max(1);
    set_scope_config(&fixture.store, "storage_cap_mb", &cap_mb.to_string());

    let (dormanted, deleted) = fixture.store.enforce_budget().expect("enforce budget");
    assert_eq!(dormanted, 0, "no active memories were seeded");
    assert!(
        deleted > 0,
        "cap was exceeded, so at least one deletion is expected"
    );
    assert!(
        deleted < u64::from(MEMORY_COUNT),
        "a reachable cap must not require deleting the entire dormant set \
         (got deleted={deleted} of {MEMORY_COUNT}); this is the Phase 2 bug this test guards against"
    );

    let live_bytes_after = fixture
        .store
        .with_connection(|connection| super::live_storage_bytes(connection))
        .expect("measure live bytes after enforcement");
    assert!(
        live_bytes_after <= cap_mb * 1024 * 1024,
        "live bytes after enforcement ({live_bytes_after}) must be under the cap ({} bytes)",
        cap_mb * 1024 * 1024
    );

    // Lowest-scoring memories are deleted first; the highest-scoring
    // survivors must be exactly the top `MEMORY_COUNT - deleted` by score.
    scored_ids.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let deleted_count = deleted as usize;
    for (id, score) in scored_ids.iter().take(deleted_count) {
        let memory = fixture.store.get(id).await.expect("get memory");
        assert!(
            memory.is_none(),
            "lowest-scoring dormant memory (score={score}) should have been deleted first"
        );
    }
    for (id, score) in scored_ids.iter().skip(deleted_count) {
        let memory = fixture
            .store
            .get(id)
            .await
            .expect("get memory")
            .expect("higher-scoring memory should survive");
        assert_eq!(
            memory.state,
            MemoryState::Dormant,
            "higher-scoring memory (score={score}) should not have been deleted"
        );
    }
}

// ── ForgettingPolicy selection ──────────────────────────────────────

#[tokio::test]
async fn enforce_budget_default_reads_forgetting_policy_from_scope_config() {
    let fixture = test_fixture();
    set_scope_config(&fixture.store, "budget_active_max", "1");
    set_scope_config(&fixture.store, "forgetting_policy", "fifo");

    let mut older = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    older.importance_score = 0.9;
    older.reliability_score = 1.0;
    older.created_at = Utc::now() - chrono::Duration::days(10);
    older.updated_at = older.created_at;
    let older_id = fixture
        .store
        .store(older)
        .await
        .expect("store older memory");

    let mut newer = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    newer.importance_score = 0.1;
    newer.reliability_score = 0.1;
    newer.created_at = Utc::now();
    newer.updated_at = newer.created_at;
    let newer_id = fixture
        .store
        .store(newer)
        .await
        .expect("store newer memory");

    // Under fifo, the older memory is demoted first even though it has
    // the higher importance*reliability score.
    let (dormanted, _deleted) = fixture.store.enforce_budget().expect("enforce budget");
    assert_eq!(dormanted, 1);

    let older_memory = fixture
        .store
        .get(&older_id)
        .await
        .expect("get memory")
        .expect("memory exists");
    assert_eq!(older_memory.state, MemoryState::Dormant);
    let newer_memory = fixture
        .store
        .get(&newer_id)
        .await
        .expect("get memory")
        .expect("memory exists");
    assert_eq!(newer_memory.state, MemoryState::Active);
}

#[tokio::test]
async fn enforce_budget_rejects_unknown_configured_forgetting_policy() {
    let fixture = test_fixture();
    set_scope_config(&fixture.store, "forgetting_policy", "not-a-real-policy");

    let memory = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    fixture.store.store(memory).await.expect("store memory");

    let error = fixture
        .store
        .enforce_budget()
        .expect_err("unknown policy name must fail loudly");
    assert!(
        matches!(error, crate::StoreError::Serialization(_)),
        "expected a Serialization error for an unknown policy, got {error:?}"
    );
}

#[tokio::test]
async fn enforce_budget_with_policy_overrides_the_configured_default_for_this_call_only() {
    let fixture = test_fixture();
    set_scope_config(&fixture.store, "budget_active_max", "1");
    // Configured default stays importance-reliability; the call below
    // overrides it with Fifo for this invocation only.

    let mut older = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    older.importance_score = 0.9;
    older.reliability_score = 1.0;
    older.created_at = Utc::now() - chrono::Duration::days(10);
    older.updated_at = older.created_at;
    let older_id = fixture
        .store
        .store(older)
        .await
        .expect("store older memory");

    let mut newer = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    newer.importance_score = 0.1;
    newer.reliability_score = 0.1;
    newer.created_at = Utc::now();
    newer.updated_at = newer.created_at;
    let newer_id = fixture
        .store
        .store(newer)
        .await
        .expect("store newer memory");

    let (dormanted, _deleted) = fixture
        .store
        .enforce_budget_with_policy(&crate::Fifo)
        .expect("enforce budget with fifo override");
    assert_eq!(dormanted, 1);

    let older_memory = fixture
        .store
        .get(&older_id)
        .await
        .expect("get memory")
        .expect("memory exists");
    assert_eq!(
        older_memory.state,
        MemoryState::Dormant,
        "fifo override should demote the older memory despite its higher importance*reliability"
    );
    let newer_memory = fixture
        .store
        .get(&newer_id)
        .await
        .expect("get memory")
        .expect("memory exists");
    assert_eq!(newer_memory.state, MemoryState::Active);
}

#[tokio::test]
async fn enforce_budget_with_priority_decay_differs_from_default_on_disagreement_corpus() {
    // A corpus where recency and importance disagree: one memory is
    // high-importance but stale and type-fast-decaying (Observation), the
    // other is low-importance but freshly accessed. importance*reliability
    // ranks the stale memory higher; priority-decay's exponential recency
    // term should invert that ranking.
    let fixture = test_fixture();
    set_scope_config(&fixture.store, "budget_active_max", "1");
    set_scope_config(&fixture.store, "decay_lambda_base", "0.5");

    let now = Utc::now();

    let mut stale_but_important =
        sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    stale_but_important.memory_type = MemoryType::Observation;
    stale_but_important.importance_score = 0.9;
    stale_but_important.reliability_score = 1.0;
    stale_but_important.updated_at = now - chrono::Duration::days(60);
    stale_but_important.last_accessed_at = Some(now - chrono::Duration::days(60));
    let stale_id = fixture
        .store
        .store(stale_but_important)
        .await
        .expect("store stale memory");

    let mut fresh_but_minor = sample_memory(MemoryScope::Workspace, ProvenanceLevel::UserStated);
    fresh_but_minor.memory_type = MemoryType::Observation;
    fresh_but_minor.importance_score = 0.3;
    fresh_but_minor.reliability_score = 1.0;
    fresh_but_minor.updated_at = now;
    fresh_but_minor.last_accessed_at = Some(now);
    let fresh_id = fixture
        .store
        .store(fresh_but_minor)
        .await
        .expect("store fresh memory");

    let default_result = fixture
        .store
        .enforce_budget_with_policy(&crate::ImportanceReliability)
        .expect("enforce budget with default policy");
    assert_eq!(default_result.0, 1);
    let demoted_by_default = fixture
        .store
        .get(&fresh_id)
        .await
        .expect("get memory")
        .expect("memory exists");
    assert_eq!(
        demoted_by_default.state,
        MemoryState::Dormant,
        "importance*reliability demotes the lower-importance fresh memory"
    );

    // Reset both back to active for a clean second run.
    set_scope_config(&fixture.store, "budget_active_max", "1");
    fixture
        .store
        .with_connection(|connection| {
            connection.execute(
                "UPDATE memories SET state = 'active' WHERE id = ?1",
                params![fresh_id.to_string()],
            )?;
            Ok(())
        })
        .expect("reset fresh memory to active");

    let decay_result = fixture
        .store
        .enforce_budget_with_policy(&crate::PriorityDecay)
        .expect("enforce budget with priority-decay policy");
    assert_eq!(decay_result.0, 1);
    let demoted_by_decay = fixture
        .store
        .get(&stale_id)
        .await
        .expect("get memory")
        .expect("memory exists");
    assert_eq!(
        demoted_by_decay.state,
        MemoryState::Dormant,
        "priority-decay demotes the stale memory despite its higher importance, \
         inverting the default policy's choice"
    );
}

#[tokio::test]
async fn enforce_budget_returns_zero_zero_when_scope_is_empty() {
    let fixture = test_fixture();
    let (dormanted, deleted) = fixture.store.enforce_budget().expect("enforce budget");
    assert_eq!(dormanted, 0);
    assert_eq!(deleted, 0);
}
