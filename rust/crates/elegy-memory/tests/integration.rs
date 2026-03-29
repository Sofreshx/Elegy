use std::collections::HashMap;

use chrono::{Duration, Utc};
use elegy_memory::{
    retention, DefaultSalienceGate, GateDecision, Memory, MemoryCandidate, MemoryScope,
    MemoryState, MemoryStore, MemoryType, MetadataUpdate, ProvenanceLevel, SalienceGate,
    ScopeConfig, SearchQuery, SensitivityLevel, SqliteMemoryStore,
};
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn full_lifecycle_covers_versioning_keyword_search_and_dormant_exclusion() {
    let (_temp_dir, store) = test_store("full-lifecycle");
    let memory = sample_memory(
        "Apollo launch checklist",
        MemoryType::Observation,
        ProvenanceLevel::UserStated,
        0.7,
        Utc::now(),
    );

    let id = store.store(memory.clone()).await.expect("store memory");

    let retrieved = store
        .get(&id)
        .await
        .expect("retrieve memory")
        .expect("stored memory should exist");
    assert_eq!(retrieved.content, memory.content);
    assert_eq!(retrieved.access_count, 1);

    let updated_content = "Apollo launch checklist with contingency rollback notes";
    store
        .update_content(
            &id,
            updated_content,
            "integration:test",
            "expanded checklist",
        )
        .await
        .expect("update memory content");

    let versions = store.list_versions(&id).expect("load version history");
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version_number, 1);
    assert_eq!(versions[0].content, memory.content);

    let updated = store
        .get_raw(&id)
        .await
        .expect("load raw updated memory")
        .expect("updated memory should exist");
    assert_eq!(updated.content, updated_content);
    assert!(updated.embedding_stale);

    let active_results = store
        .search(SearchQuery {
            text: "rollback".to_string(),
            embedding: None,
            scope: MemoryScope::Workspace,
            state_filter: None,
            type_filter: None,
            max_results: 10,
            context_config: None,
        })
        .await
        .expect("search active memories");
    assert_eq!(active_results.len(), 1);
    assert_eq!(active_results[0].memory.id, id);

    store.make_dormant(&id).await.expect("archive memory");

    let excluded_from_default_search = store
        .search(SearchQuery {
            text: "rollback".to_string(),
            embedding: None,
            scope: MemoryScope::Workspace,
            state_filter: None,
            type_filter: None,
            max_results: 10,
            context_config: None,
        })
        .await
        .expect("search active memories after archival");
    assert!(excluded_from_default_search.is_empty());

    let dormant_results = store
        .search(SearchQuery {
            text: "rollback".to_string(),
            embedding: None,
            scope: MemoryScope::Workspace,
            state_filter: Some(MemoryState::Dormant),
            type_filter: None,
            max_results: 10,
            context_config: None,
        })
        .await
        .expect("search dormant memories");
    assert_eq!(dormant_results.len(), 1);
    assert_eq!(dormant_results[0].memory.state, MemoryState::Dormant);
}

#[tokio::test]
async fn gate_integration_merges_near_duplicates_and_preserves_version_history() {
    let (_temp_dir, store) = test_store("gate-merge");
    let gate = DefaultSalienceGate::new(store.scope_config().expect("load scope config"));
    let now = Utc::now();
    let original = sample_memory(
        "Remember the Apollo launch checklist",
        MemoryType::Observation,
        ProvenanceLevel::UserStated,
        0.6,
        now,
    );

    let original_id = store.store(original.clone()).await.expect("store original");
    store
        .store_embedding(&original_id, &axis_embedding())
        .await
        .expect("store original embedding");

    let candidate = sample_candidate(
        "Remember the Apollo launch checklist with contingency notes",
        0.9,
        ProvenanceLevel::UserStated,
        Some(cosine_embedding(0.95)),
    );

    let decision = gate
        .evaluate(&candidate, &store)
        .await
        .expect("evaluate merge candidate");

    let GateDecision::Merge {
        target_id,
        enriched_content,
    } = decision
    else {
        panic!("expected merge decision");
    };
    assert_eq!(target_id, original_id);

    store
        .update_content(
            &target_id,
            &enriched_content,
            "integration:test",
            "gate merge",
        )
        .await
        .expect("apply merged content");
    store
        .update_metadata(
            &target_id,
            MetadataUpdate {
                importance_score: Some(candidate.importance_score),
                ..MetadataUpdate::default()
            },
        )
        .await
        .expect("keep higher importance after merge");

    let merged = store
        .get_raw(&target_id)
        .await
        .expect("load merged memory")
        .expect("merged memory should exist");
    assert_eq!(merged.content, candidate.content);
    assert!((merged.importance_score - candidate.importance_score).abs() < f32::EPSILON);

    let versions = store
        .list_versions(&target_id)
        .expect("load merge version history");
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].content, original.content);
}

#[tokio::test]
async fn gate_safety_yields_only_accept_merge_or_archive_and_accepts_doubt_zone() {
    let (_temp_dir, store) = test_store("gate-safety");
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let existing = sample_memory(
        "Known launch preference",
        MemoryType::Preference,
        ProvenanceLevel::UserStated,
        0.8,
        Utc::now(),
    );
    let existing_id = store.store(existing).await.expect("store existing memory");
    store
        .store_embedding(&existing_id, &axis_embedding())
        .await
        .expect("store similarity baseline");

    let decisions = [
        gate.evaluate(
            &sample_candidate(
                "Distinct memory with no embedding",
                0.9,
                ProvenanceLevel::UserStated,
                None,
            ),
            &store,
        )
        .await
        .expect("accept candidate"),
        gate.evaluate(
            &sample_candidate(
                "Known launch preference with a reversible merge candidate",
                0.9,
                ProvenanceLevel::UserStated,
                Some(cosine_embedding(0.95)),
            ),
            &store,
        )
        .await
        .expect("merge candidate"),
        gate.evaluate(
            &sample_candidate("Low-value aside", 0.1, ProvenanceLevel::UserStated, None),
            &store,
        )
        .await
        .expect("archive low-salience candidate"),
        gate.evaluate(
            &sample_candidate(
                "The user might prefer evening launches",
                0.45,
                ProvenanceLevel::AgentInferred,
                None,
            ),
            &store,
        )
        .await
        .expect("archive low-confidence inference"),
    ];

    assert!(decisions.iter().all(|decision| {
        matches!(
            decision,
            GateDecision::Accept | GateDecision::Archive | GateDecision::Merge { .. }
        )
    }));
    assert!(decisions
        .iter()
        .all(|decision| !matches!(decision, GateDecision::Reject { .. })));

    let doubt_zone = gate
        .evaluate(
            &sample_candidate(
                "Known launch preference but with a distinct rider",
                0.8,
                ProvenanceLevel::UserStated,
                Some(cosine_embedding(0.90)),
            ),
            &store,
        )
        .await
        .expect("evaluate doubt-zone candidate");
    assert_eq!(doubt_zone, GateDecision::Accept);
}

#[tokio::test]
async fn search_orders_results_by_combined_scoring_signals() {
    let (_temp_dir, store) = test_store("search-scoring");
    let now = Utc::now();

    let mut strongest = sample_memory(
        "mission ranking alpha",
        MemoryType::Fact,
        ProvenanceLevel::UserStated,
        1.0,
        now,
    );
    strongest.access_count = 10;
    strongest.last_accessed_at = Some(now);
    let strongest_id = strongest.id;

    let mut balanced = sample_memory(
        "mission ranking beta",
        MemoryType::Fact,
        ProvenanceLevel::UserStated,
        0.9,
        now - Duration::days(2),
    );
    balanced.access_count = 3;
    balanced.last_accessed_at = Some(now - Duration::days(2));
    let balanced_id = balanced.id;

    let mut weakest = sample_memory(
        "mission ranking gamma",
        MemoryType::Fact,
        ProvenanceLevel::UserStated,
        0.2,
        now - Duration::days(20),
    );
    weakest.last_accessed_at = Some(now - Duration::days(20));
    let weakest_id = weakest.id;

    for memory in [strongest, balanced, weakest] {
        let id = memory.id;
        store.store(memory).await.expect("store ranking memory");
        store
            .store_embedding(&id, &axis_embedding())
            .await
            .expect("store ranking embedding");
    }

    let results = store
        .search(SearchQuery {
            text: String::new(),
            embedding: Some(axis_embedding()),
            scope: MemoryScope::Workspace,
            state_filter: None,
            type_filter: None,
            max_results: 10,
            context_config: None,
        })
        .await
        .expect("search by vector similarity");

    let ordered_ids = results
        .iter()
        .map(|result| result.memory.id)
        .collect::<Vec<_>>();
    assert_eq!(ordered_ids, vec![strongest_id, balanced_id, weakest_id]);
    assert!(results[0].score > results[1].score);
    assert!(results[1].score > results[2].score);
}

#[tokio::test]
async fn decay_integration_uses_age_and_fixed_lambda_consistently() {
    let (_temp_dir, store) = test_store("decay");
    let now = Utc::now();
    let scope_config = store.scope_config().expect("load scope config");

    let mut recent = sample_memory(
        "recent memory",
        MemoryType::Observation,
        ProvenanceLevel::UserStated,
        0.7,
        now - Duration::days(1),
    );
    recent.last_accessed_at = Some(now - Duration::days(1));

    let mut older = sample_memory(
        "older memory",
        MemoryType::Observation,
        ProvenanceLevel::UserStated,
        0.7,
        now - Duration::days(12),
    );
    older.last_accessed_at = Some(now - Duration::days(12));

    let mut same_age_fact = sample_memory(
        "same age fact",
        MemoryType::Fact,
        ProvenanceLevel::UserStated,
        0.7,
        now - Duration::days(5),
    );
    same_age_fact.last_accessed_at = Some(now - Duration::days(5));

    let mut same_age_preference = sample_memory(
        "same age preference",
        MemoryType::Preference,
        ProvenanceLevel::UserStated,
        0.7,
        now - Duration::days(5),
    );
    same_age_preference.last_accessed_at = Some(now - Duration::days(5));

    for memory in [
        recent.clone(),
        older.clone(),
        same_age_fact.clone(),
        same_age_preference.clone(),
    ] {
        store.store(memory).await.expect("store decay test memory");
    }

    let recent_retention = retention(&recent, now, &scope_config);
    let older_retention = retention(&older, now, &scope_config);
    let fact_retention = retention(&same_age_fact, now, &scope_config);
    let preference_retention = retention(&same_age_preference, now, &scope_config);

    assert!(recent_retention > older_retention);
    assert!((fact_retention - preference_retention).abs() < 1.0e-12);
}

fn test_store(prefix: &str) -> (TempDir, SqliteMemoryStore) {
    let temp_dir = TempDir::new().expect("create temp directory");
    let db_path = temp_dir.path().join(format!("{prefix}.sqlite3"));
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");
    (temp_dir, store)
}

fn sample_memory(
    content: &str,
    memory_type: MemoryType,
    provenance: ProvenanceLevel,
    importance_score: f32,
    timestamp: chrono::DateTime<Utc>,
) -> Memory {
    Memory {
        id: Uuid::new_v4(),
        content: content.to_string(),
        summary: None,
        scope: MemoryScope::Workspace,
        memory_type,
        provenance,
        importance_score,
        reliability_score: provenance.base_reliability(),
        sensitivity: SensitivityLevel::Low,
        state: MemoryState::Active,
        tags: Vec::new(),
        status: None,
        custom_metadata: HashMap::new(),
        access_count: 0,
        corroboration_count: 0,
        embedding_stale: false,
        created_at: timestamp,
        updated_at: timestamp,
        last_accessed_at: Some(timestamp),
        tenant_id: None,
        user_id: None,
        agent_id: None,
    }
}

fn sample_candidate(
    content: &str,
    importance_score: f32,
    provenance: ProvenanceLevel,
    embedding: Option<Vec<f32>>,
) -> MemoryCandidate {
    MemoryCandidate {
        content: content.to_string(),
        summary: None,
        memory_type: MemoryType::Observation,
        provenance,
        importance_score,
        sensitivity: SensitivityLevel::Low,
        tags: Vec::new(),
        custom_metadata: HashMap::new(),
        embedding,
    }
}

fn axis_embedding() -> Vec<f32> {
    let mut embedding = vec![0.0; 768];
    embedding[0] = 1.0;
    embedding
}

fn cosine_embedding(target_cosine: f32) -> Vec<f32> {
    assert!((0.0..=1.0).contains(&target_cosine));

    let mut embedding = vec![0.0; 768];
    embedding[0] = target_cosine;
    embedding[1] = (1.0 - (target_cosine * target_cosine)).sqrt();
    embedding
}
