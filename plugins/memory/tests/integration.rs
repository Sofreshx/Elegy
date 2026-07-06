use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use elegy_memory::{
    retention, DefaultSalienceGate, EmbeddingError, EmbeddingProvider, GateDecision, Memory,
    MemoryCandidate, MemoryScope, MemoryState, MemoryStore, MemoryType, MetadataUpdate,
    ProvenanceLevel, SalienceGate, ScopeConfig, SearchQuery, SensitivityLevel, SqliteMemoryStore,
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
            session_id: None,
            agent_id: None,
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
            session_id: None,
            agent_id: None,
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
            session_id: None,
            agent_id: None,
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
        ..
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
async fn gate_integration_avoids_concatenation_for_moderate_similarity_merges() {
    let (_temp_dir, store) = test_store("gate-moderate-merge");
    let gate = DefaultSalienceGate::new(store.scope_config().expect("load scope config"));
    let original = sample_memory(
        "Remember the Apollo launch checklist with rollback owners",
        MemoryType::Observation,
        ProvenanceLevel::UserStated,
        0.6,
        Utc::now(),
    );

    let original_id = store.store(original.clone()).await.expect("store original");
    store
        .store_embedding(&original_id, &axis_embedding())
        .await
        .expect("store original embedding");

    let candidate = sample_candidate(
        "Remember the Apollo launch checklist with fallback owners",
        0.9,
        ProvenanceLevel::UserStated,
        Some(cosine_embedding(0.94)),
    );

    let decision = gate
        .evaluate(&candidate, &store)
        .await
        .expect("evaluate merge candidate");

    let GateDecision::Merge {
        target_id,
        enriched_content,
        ..
    } = decision
    else {
        panic!("expected merge decision");
    };
    assert_eq!(target_id, original_id);
    assert_eq!(enriched_content, original.content);
    assert!(!enriched_content.contains("\n\n"));

    store
        .update_content(
            &target_id,
            &enriched_content,
            "integration:test",
            "gate merge without concatenation",
        )
        .await
        .expect("apply merge result");

    let merged = store
        .get_raw(&target_id)
        .await
        .expect("load merged memory")
        .expect("merged memory should exist");
    assert_eq!(merged.content, original.content);

    let versions = store
        .list_versions(&target_id)
        .expect("load merge version history");
    assert!(versions.is_empty());
}

#[tokio::test]
async fn gate_integration_merges_rust_and_tauri_near_duplicates_at_lowered_threshold() {
    let (_temp_dir, store) = test_store("gate-rust-tauri-merge");
    let gate = DefaultSalienceGate::new(store.scope_config().expect("load scope config"));
    let original = sample_memory(
        "Project uses Rust",
        MemoryType::Fact,
        ProvenanceLevel::UserStated,
        0.7,
        Utc::now(),
    );

    let original_id = store.store(original.clone()).await.expect("store original");
    store
        .store_embedding(&original_id, &axis_embedding())
        .await
        .expect("store original embedding");

    let candidate = sample_candidate(
        "Project uses Rust and Tauri",
        0.9,
        ProvenanceLevel::UserStated,
        Some(cosine_embedding(0.86)),
    );

    let decision = gate
        .evaluate(&candidate, &store)
        .await
        .expect("evaluate near-duplicate candidate");

    let GateDecision::Merge {
        target_id,
        enriched_content,
        ..
    } = decision
    else {
        panic!("expected merge decision");
    };
    assert_eq!(target_id, original_id);
    assert_eq!(enriched_content, candidate.content);
}

#[tokio::test]
async fn gate_safety_yields_only_accept_merge_or_archive_and_warns_in_likely_duplicate_band() {
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
            GateDecision::Accept { .. } | GateDecision::Archive | GateDecision::Merge { .. }
        )
    }));
    assert!(decisions
        .iter()
        .all(|decision| !matches!(decision, GateDecision::Reject { .. })));

    let likely_duplicate = gate
        .evaluate(
            &sample_candidate(
                "Known launch preference but with a distinct rider",
                0.8,
                ProvenanceLevel::UserStated,
                Some(cosine_embedding(0.82)),
            ),
            &store,
        )
        .await
        .expect("evaluate likely-duplicate candidate");
    assert_eq!(
        likely_duplicate,
        GateDecision::Accept {
            similar_to: Some(existing_id),
            similarity: Some(0.82),
        }
    );
}

#[tokio::test]
async fn provider_backed_search_derives_query_embedding_without_explicit_vector() {
    let semantic_query = "orbital prep semantic probe";
    let semantic_match_content = "release readiness checklist";
    let non_match_content = "garden watering schedule";
    let provider = Arc::new(StubEmbeddingProvider::new([
        (semantic_match_content, axis_embedding()),
        (non_match_content, negative_axis_embedding()),
        (semantic_query, axis_embedding()),
    ]));
    let (_temp_dir, store) = test_store_with_provider("provider-search-derived", provider.clone());

    let semantic_match = sample_memory(
        semantic_match_content,
        MemoryType::Fact,
        ProvenanceLevel::UserStated,
        0.8,
        Utc::now(),
    );
    let semantic_match_id = semantic_match.id;
    let non_match = sample_memory(
        non_match_content,
        MemoryType::Fact,
        ProvenanceLevel::Imported,
        0.8,
        Utc::now(),
    );

    store
        .store(semantic_match)
        .await
        .expect("store semantic match");
    store.store(non_match).await.expect("store non-match");

    let results = store
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
            semantic_query.to_string(),
        ]
    );
}

#[tokio::test]
async fn nomic_embeddings_apply_document_and_query_task_prefixes() {
    let semantic_query = "orbital prep semantic probe";
    let semantic_match_content = "release readiness checklist";
    let non_match_content = "garden watering schedule";
    let provider = Arc::new(StubEmbeddingProvider::new_with_model(
        "nomic-embed-text:latest",
        [
            (
                "search_document: release readiness checklist",
                axis_embedding(),
            ),
            (
                "search_document: garden watering schedule",
                negative_axis_embedding(),
            ),
            (
                "search_query: orbital prep semantic probe",
                axis_embedding(),
            ),
        ],
    ));
    let (_temp_dir, store) =
        test_store_with_provider("nomic-provider-search-derived", provider.clone());

    let semantic_match = sample_memory(
        semantic_match_content,
        MemoryType::Fact,
        ProvenanceLevel::UserStated,
        0.8,
        Utc::now(),
    );
    let semantic_match_id = semantic_match.id;
    let non_match = sample_memory(
        non_match_content,
        MemoryType::Fact,
        ProvenanceLevel::Imported,
        0.8,
        Utc::now(),
    );

    store
        .store(semantic_match)
        .await
        .expect("store semantic match");
    store.store(non_match).await.expect("store non-match");

    let results = store
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
            "search_document: release readiness checklist".to_string(),
            "search_document: garden watering schedule".to_string(),
            "search_query: orbital prep semantic probe".to_string(),
        ]
    );
}

#[tokio::test]
async fn agent_scoped_search_can_filter_to_a_single_agent_id() {
    let temp_dir = TempDir::new().expect("create temp directory");
    let db_path = temp_dir.path().join("agent-filter.sqlite3");
    let store = SqliteMemoryStore::new(&db_path, MemoryScope::Agent).expect("create agent store");
    let now = Utc::now();

    let mut visible = sample_memory(
        "Visible semantic memory",
        MemoryType::Fact,
        ProvenanceLevel::UserStated,
        0.8,
        now,
    );
    visible.scope = MemoryScope::Agent;
    visible.agent_id = Some("agent-a".to_string());
    let visible_id = visible.id;

    let mut hidden = sample_memory(
        "Hidden semantic memory",
        MemoryType::Fact,
        ProvenanceLevel::UserStated,
        0.8,
        now,
    );
    hidden.scope = MemoryScope::Agent;
    hidden.agent_id = Some("agent-b".to_string());
    let hidden_id = hidden.id;

    store.store(visible).await.expect("store visible memory");
    store.store(hidden).await.expect("store hidden memory");
    store
        .store_embedding(&visible_id, &axis_embedding())
        .await
        .expect("store visible embedding");
    store
        .store_embedding(&hidden_id, &axis_embedding())
        .await
        .expect("store hidden embedding");

    let results = store
        .search(SearchQuery {
            text: String::new(),
            embedding: Some(axis_embedding()),
            scope: MemoryScope::Agent,
            state_filter: None,
            type_filter: None,
            max_results: 5,
            context_config: None,
            session_id: None,
            agent_id: Some("agent-a".to_string()),
        })
        .await
        .expect("search filtered agent memories");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].memory.id, visible_id);
}

#[tokio::test]
async fn text_only_search_without_provider_remains_keyword_driven() {
    let (_temp_dir, store) = test_store("keyword-only-search");
    let memory = sample_memory(
        "apollo fallback keyword memory",
        MemoryType::Observation,
        ProvenanceLevel::UserStated,
        0.7,
        Utc::now(),
    );
    let id = memory.id;

    store.store(memory).await.expect("store keyword memory");

    let results = store
        .search(SearchQuery {
            text: "fallback keyword".to_string(),
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
        .expect("search without provider");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].memory.id, id);
    assert!(results[0].similarity > 0.0);
}

#[tokio::test]
async fn text_only_search_matches_compound_words_via_fts_index_expansion() {
    let (_temp_dir, store) = test_store("compound-word-search");
    let memory = sample_memory(
        "ProtonVPN avec WireGuard et JavaScript",
        MemoryType::Observation,
        ProvenanceLevel::UserStated,
        0.7,
        Utc::now(),
    );
    let id = memory.id;
    let original_content = memory.content.clone();

    store
        .store(memory)
        .await
        .expect("store compound word memory");

    let stored = store
        .get_raw(&id)
        .await
        .expect("load raw stored memory")
        .expect("compound word memory should exist");
    assert_eq!(stored.content, original_content);

    for query_text in ["VPN", "VPN WireGuard", "Script"] {
        let results = store
            .search(SearchQuery {
                text: query_text.to_string(),
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
            .expect("search compound word memory");

        assert_eq!(results.len(), 1, "query `{query_text}` should match");
        assert_eq!(
            results[0].memory.id, id,
            "query `{query_text}` should return the indexed memory"
        );
    }

    store
        .update_content(
            &id,
            "OpenSSH tunnel notes",
            "integration:test",
            "exercise FTS delete/insert",
        )
        .await
        .expect("update compound word memory");

    let removed_results = store
        .search(SearchQuery {
            text: "VPN".to_string(),
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
        .expect("search for removed compound token");
    assert!(removed_results.is_empty());

    let updated_results = store
        .search(SearchQuery {
            text: "SSH".to_string(),
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
        .expect("search updated compound token");
    assert_eq!(updated_results.len(), 1);
    assert_eq!(updated_results[0].memory.id, id);
}

#[tokio::test]
async fn gate_merge_preserves_searchable_compound_enrichment_for_keyword_search() {
    let (_temp_dir, store) = test_store("compound-word-merge-search");
    let gate = DefaultSalienceGate::new(store.scope_config().expect("load scope config"));
    let original = sample_memory(
        "ProtonVPN avec WireGuard protege tout le trafic reseau",
        MemoryType::Preference,
        ProvenanceLevel::UserStated,
        0.5,
        Utc::now(),
    );

    let original_id = store.store(original.clone()).await.expect("store original");
    store
        .store_embedding(&original_id, &axis_embedding())
        .await
        .expect("store original embedding");

    let candidate = sample_candidate(
        "ProtonVPN avec WireGuard et JavaScript protegent le reseau",
        0.5,
        ProvenanceLevel::UserStated,
        Some(cosine_embedding(0.94)),
    );

    let decision = gate
        .evaluate(&candidate, &store)
        .await
        .expect("evaluate merge candidate");

    let GateDecision::Merge {
        target_id,
        enriched_content,
        ..
    } = decision
    else {
        panic!("expected merge decision");
    };
    assert_eq!(target_id, original_id);
    assert_eq!(enriched_content, candidate.content);

    store
        .update_content(
            &target_id,
            &enriched_content,
            "integration:test",
            "preserve searchable compound enrichment",
        )
        .await
        .expect("apply merge result");

    let merged = store
        .get_raw(&target_id)
        .await
        .expect("load merged memory")
        .expect("merged memory should exist");
    assert_eq!(merged.content, candidate.content);

    for query_text in ["VPN", "Script"] {
        let results = store
            .search(SearchQuery {
                text: query_text.to_string(),
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
            .expect("search merged memory");

        assert_eq!(results.len(), 1, "query `{query_text}` should match");
        assert_eq!(
            results[0].memory.id, target_id,
            "query `{query_text}` should return the merged memory"
        );
        assert_eq!(
            results[0].memory.content, candidate.content,
            "query `{query_text}` should surface the enriched merged content"
        );
    }
}

#[tokio::test]
async fn reembed_flow_recomputes_oldest_stale_embeddings_with_stub_provider() {
    let provider = Arc::new(StubEmbeddingProvider::new([
        ("older stale memory", axis_embedding()),
        ("newer stale memory", cosine_embedding(0.5)),
    ]));
    let (_temp_dir, store) = test_store("reembed-flow");
    let now = Utc::now();

    let mut older = sample_memory(
        "older stale memory",
        MemoryType::Observation,
        ProvenanceLevel::UserStated,
        0.7,
        now - Duration::days(1),
    );
    older.embedding_stale = true;
    let older_id = older.id;

    let mut newer = sample_memory(
        "newer stale memory",
        MemoryType::Observation,
        ProvenanceLevel::UserStated,
        0.7,
        now,
    );
    newer.embedding_stale = true;
    let newer_id = newer.id;

    store.store(older).await.expect("store older stale memory");
    store.store(newer).await.expect("store newer stale memory");

    let reembedded_ids = reembed_stale_memories(&store, provider.as_ref(), 1)
        .await
        .expect("reembed oldest stale memory");

    assert_eq!(reembedded_ids, vec![older_id]);
    assert_eq!(provider.calls(), vec!["older stale memory".to_string()]);

    let older_reloaded = store
        .get_raw(&older_id)
        .await
        .expect("reload older memory")
        .expect("older memory exists");
    let newer_reloaded = store
        .get_raw(&newer_id)
        .await
        .expect("reload newer memory")
        .expect("newer memory exists");
    assert!(!older_reloaded.embedding_stale);
    assert!(newer_reloaded.embedding_stale);
}

#[tokio::test]
async fn gate_uses_provider_embedding_when_candidate_embedding_is_missing() {
    let provider = Arc::new(StubEmbeddingProvider::new([(
        "Remember the Apollo launch checklist with contingency notes",
        cosine_embedding(0.95),
    )]));
    let (_temp_dir, store) = test_store("gate-provider-fallback");
    let gate = DefaultSalienceGate::new_with_embedding_provider(
        store.scope_config().expect("load scope config"),
        provider.clone(),
    );
    let original = sample_memory(
        "Remember the Apollo launch checklist",
        MemoryType::Observation,
        ProvenanceLevel::UserStated,
        0.6,
        Utc::now(),
    );

    let original_id = store.store(original).await.expect("store original");
    store
        .store_embedding(&original_id, &axis_embedding())
        .await
        .expect("store original embedding");

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Remember the Apollo launch checklist with contingency notes",
                0.9,
                ProvenanceLevel::UserStated,
                None,
            ),
            &store,
        )
        .await
        .expect("evaluate candidate with provider-backed embedding");

    let GateDecision::Merge {
        target_id,
        enriched_content,
        ..
    } = decision
    else {
        panic!("expected merge decision");
    };
    assert_eq!(target_id, original_id);
    assert_eq!(
        enriched_content,
        "Remember the Apollo launch checklist with contingency notes"
    );
    assert_eq!(
        provider.calls(),
        vec!["Remember the Apollo launch checklist with contingency notes".to_string()]
    );
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
            session_id: None,
            agent_id: None,
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

#[tokio::test]
async fn record_link_and_list_links_round_trip() {
    let (_temp_dir, store) = test_store("link-round-trip");

    let mem_a = sample_memory(
        "Source memory",
        MemoryType::Fact,
        ProvenanceLevel::UserStated,
        0.7,
        Utc::now(),
    );
    let mem_b = sample_memory(
        "Target memory",
        MemoryType::Fact,
        ProvenanceLevel::UserStated,
        0.7,
        Utc::now(),
    );

    let source_id = mem_a.id;
    let target_id = mem_b.id;

    store.store(mem_a).await.expect("store source memory");
    store.store(mem_b).await.expect("store target memory");

    store
        .record_link(&source_id, &target_id, "supersedes")
        .expect("record_link should succeed");

    // list_links from source side
    let links_from_source = store.list_links(&source_id).expect("list_links for source");
    assert_eq!(
        links_from_source.len(),
        1,
        "source should see exactly 1 link"
    );
    let link = &links_from_source[0];
    assert_eq!(link.source_id, source_id);
    assert_eq!(link.target_id, target_id);
    assert_eq!(link.relation_type, "supersedes");
    assert!(
        (link.weight - 1.0).abs() < f32::EPSILON,
        "default weight should be 1.0"
    );

    // list_links from target side — same link should appear
    let links_from_target = store.list_links(&target_id).expect("list_links for target");
    assert_eq!(
        links_from_target.len(),
        1,
        "target should see exactly 1 link"
    );
    assert_eq!(links_from_target[0].source_id, source_id);
    assert_eq!(links_from_target[0].target_id, target_id);
}

#[tokio::test]
async fn record_link_rejects_self_link() {
    let (_temp_dir, store) = test_store("link-self-reject");

    let mem = sample_memory(
        "Self-referencing memory",
        MemoryType::Fact,
        ProvenanceLevel::UserStated,
        0.7,
        Utc::now(),
    );
    let id = mem.id;

    store.store(mem).await.expect("store memory");

    let result = store.record_link(&id, &id, "supersedes");
    assert!(
        result.is_err(),
        "self-link should be rejected with a validation error"
    );
}

#[tokio::test]
async fn record_link_ignores_duplicate_link() {
    let (_temp_dir, store) = test_store("link-dup-ignore");

    let mem_a = sample_memory(
        "First memory",
        MemoryType::Fact,
        ProvenanceLevel::UserStated,
        0.7,
        Utc::now(),
    );
    let mem_b = sample_memory(
        "Second memory",
        MemoryType::Fact,
        ProvenanceLevel::UserStated,
        0.7,
        Utc::now(),
    );

    let source_id = mem_a.id;
    let target_id = mem_b.id;

    store.store(mem_a).await.expect("store first memory");
    store.store(mem_b).await.expect("store second memory");

    store
        .record_link(&source_id, &target_id, "related")
        .expect("first record_link should succeed");
    store
        .record_link(&source_id, &target_id, "related")
        .expect("duplicate record_link should succeed (INSERT OR IGNORE)");

    let links = store
        .list_links(&source_id)
        .expect("list_links after duplicate insert");
    assert_eq!(
        links.len(),
        1,
        "duplicate link should be silently ignored, only 1 link expected"
    );
}

fn test_store(prefix: &str) -> (TempDir, SqliteMemoryStore) {
    let temp_dir = TempDir::new().expect("create temp directory");
    let db_path = temp_dir.path().join(format!("{prefix}.sqlite3"));
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");
    (temp_dir, store)
}

fn test_store_with_provider(
    prefix: &str,
    provider: Arc<dyn EmbeddingProvider>,
) -> (TempDir, SqliteMemoryStore) {
    let temp_dir = TempDir::new().expect("create temp directory");
    let db_path = temp_dir.path().join(format!("{prefix}.sqlite3"));
    let store =
        SqliteMemoryStore::new_with_embedding_provider(&db_path, MemoryScope::Workspace, provider)
            .expect("create sqlite store with embedding provider");
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

fn negative_axis_embedding() -> Vec<f32> {
    let mut embedding = vec![0.0; 768];
    embedding[0] = -1.0;
    embedding
}

fn cosine_embedding(target_cosine: f32) -> Vec<f32> {
    assert!((0.0..=1.0).contains(&target_cosine));

    let mut embedding = vec![0.0; 768];
    embedding[0] = target_cosine;
    embedding[1] = (1.0 - (target_cosine * target_cosine)).sqrt();
    embedding
}

async fn reembed_stale_memories(
    store: &SqliteMemoryStore,
    provider: &dyn EmbeddingProvider,
    limit: usize,
) -> Result<Vec<Uuid>, String> {
    let stale_ids = store
        .get_stale_embeddings(limit)
        .await
        .map_err(|error| format!("load stale ids: {error}"))?;
    let mut reembedded_ids = Vec::with_capacity(stale_ids.len());

    for id in &stale_ids {
        let memory = store
            .get_raw(id)
            .await
            .map_err(|error| format!("load memory {id}: {error}"))?
            .ok_or_else(|| format!("load memory {id}: not found"))?;
        let embedding = provider
            .embed(&memory.content)
            .await
            .map_err(|error| format!("embed memory {id}: {error}"))?;
        store
            .store_embedding(id, &embedding)
            .await
            .map_err(|error| format!("store embedding for {id}: {error}"))?;
        reembedded_ids.push(*id);
    }

    Ok(reembedded_ids)
}

#[derive(Debug)]
struct StubEmbeddingProvider {
    model_id: &'static str,
    responses: HashMap<String, Vec<f32>>,
    calls: Mutex<Vec<String>>,
}

impl StubEmbeddingProvider {
    fn new<I, S>(responses: I) -> Self
    where
        I: IntoIterator<Item = (S, Vec<f32>)>,
        S: Into<String>,
    {
        Self::new_with_model("integration-stub", responses)
    }

    fn new_with_model<I, S>(model_id: &'static str, responses: I) -> Self
    where
        I: IntoIterator<Item = (S, Vec<f32>)>,
        S: Into<String>,
    {
        Self {
            model_id,
            responses: responses
                .into_iter()
                .map(|(text, embedding)| (text.into(), embedding))
                .collect(),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("stub provider calls lock").clone()
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

        self.responses.get(&trimmed).cloned().ok_or_else(|| {
            EmbeddingError::Provider(format!("missing stub embedding for `{trimmed}`"))
        })
    }

    fn dimensions(&self) -> usize {
        768
    }

    fn model_id(&self) -> &str {
        self.model_id
    }
}
