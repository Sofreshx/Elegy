use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use super::{searchable_terms, DefaultSalienceGate};
use crate::{
    ContradictionEntry, EmbeddingError, EmbeddingProvider, GateDecision, LlmError, LlmProvider,
    Memory, MemoryCandidate, MemoryFilter, MemoryHealthReport, MemoryId, MemoryScope, MemoryState,
    MemoryStore, MemoryType, MetadataUpdate, ProvenanceLevel, PurgeReport, ResolutionStatus,
    SalienceGate, ScopeConfig, ScoredMemory, SearchQuery, SensitivityLevel, StoreError,
};

#[tokio::test]
async fn merges_when_similarity_exceeds_merge_threshold() {
    let target = sample_memory("Launch plan", ProvenanceLevel::UserStated);
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: target.clone(),
        score: 0.95,
        similarity: 0.95,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Launch plan with contingency checklist",
                0.9,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate candidate");

    match decision {
        GateDecision::Merge {
            target_id,
            enriched_content,
            ..
        } => {
            assert_eq!(target_id, target.id);
            assert_eq!(enriched_content, "Launch plan with contingency checklist");
        }
        other => panic!("expected merge decision, got {other:?}"),
    }
}

#[tokio::test]
async fn detects_contradiction_for_different_technology_values() {
    let target = sample_memory("Backend is C# with gRPC", ProvenanceLevel::UserStated);
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: target.clone(),
        score: 0.95,
        similarity: 0.95,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Backend is Python with Flask",
                0.9,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate contradictory candidate");

    match decision {
        GateDecision::Contradiction {
            conflicting_id,
            description,
        } => {
            assert_eq!(conflicting_id, target.id);
            assert!(description.contains("backend"));
            assert!(description.contains("c#"));
            assert!(description.contains("python"));
        }
        other => panic!("expected contradiction decision, got {other:?}"),
    }
}

#[tokio::test]
async fn detects_contradiction_for_different_numeric_values() {
    let target = sample_memory("Cap RTSS 120fps", ProvenanceLevel::UserStated);
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: target.clone(),
        score: 0.95,
        similarity: 0.95,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Cap RTSS 60fps",
                0.9,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate contradictory numeric candidate");

    match decision {
        GateDecision::Contradiction {
            conflicting_id,
            description,
        } => {
            assert_eq!(conflicting_id, target.id);
            assert!(description.contains("cap rtss"));
            assert!(description.contains("120fps"));
            assert!(description.contains("60fps"));
        }
        other => panic!("expected contradiction decision, got {other:?}"),
    }
}

#[tokio::test]
async fn llm_agree_verdict_keeps_merge_path() {
    let target = sample_memory("Project uses Rust", ProvenanceLevel::UserStated);
    let llm = Arc::new(StubLlmProvider::new([(
        "Project uses Rust\n---\nProject uses Rust and Tauri",
        Ok("AGREE".to_string()),
    )]));
    let gate = DefaultSalienceGate::new_with_llm_provider(ScopeConfig::default(), llm);
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: target.clone(),
        score: 0.95,
        similarity: 0.95,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Project uses Rust and Tauri",
                0.9,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate llm agree candidate");

    match decision {
        GateDecision::Merge { target_id, .. } => assert_eq!(target_id, target.id),
        other => panic!("expected merge decision, got {other:?}"),
    }
}

#[tokio::test]
async fn llm_contradict_verdict_records_contradiction() {
    let target = sample_memory("Backend is C# with gRPC", ProvenanceLevel::UserStated);
    let llm = Arc::new(StubLlmProvider::new([(
        "Backend is C# with gRPC\n---\nBackend is Python with Flask",
        Ok("CONTRADICT: incompatible backend stack".to_string()),
    )]));
    let gate = DefaultSalienceGate::new_with_llm_provider(ScopeConfig::default(), llm);
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: target.clone(),
        score: 0.95,
        similarity: 0.95,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Backend is Python with Flask",
                0.9,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate llm contradiction candidate");

    match decision {
        GateDecision::Contradiction {
            conflicting_id,
            description,
        } => {
            assert_eq!(conflicting_id, target.id);
            assert_eq!(description, "incompatible backend stack");
        }
        other => panic!("expected contradiction decision, got {other:?}"),
    }
}

#[tokio::test]
async fn llm_unrelated_verdict_accepts_new_memory() {
    let target = sample_memory("Project uses Rust", ProvenanceLevel::UserStated);
    let llm = Arc::new(StubLlmProvider::new([(
        "Project uses Rust\n---\nTeam meets on Fridays",
        Ok("UNRELATED".to_string()),
    )]));
    let gate = DefaultSalienceGate::new_with_llm_provider(ScopeConfig::default(), llm);
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: target,
        score: 0.95,
        similarity: 0.95,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Team meets on Fridays",
                0.9,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate llm unrelated candidate");

    assert_eq!(
        decision,
        GateDecision::Accept {
            similar_to: None,
            similarity: None,
        }
    );
}

#[tokio::test]
async fn llm_failure_falls_back_to_heuristic_contradiction_detection() {
    let target = sample_memory("Cap RTSS 120fps", ProvenanceLevel::UserStated);
    let llm = Arc::new(StubLlmProvider::new([(
        "Cap RTSS 120fps\n---\nCap RTSS 60fps",
        Err(LlmError::Provider("offline".to_string())),
    )]));
    let gate = DefaultSalienceGate::new_with_llm_provider(ScopeConfig::default(), llm);
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: target.clone(),
        score: 0.95,
        similarity: 0.95,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Cap RTSS 60fps",
                0.9,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate llm failure candidate");

    match decision {
        GateDecision::Contradiction { conflicting_id, .. } => {
            assert_eq!(conflicting_id, target.id);
        }
        other => panic!("expected heuristic contradiction decision, got {other:?}"),
    }
}

#[tokio::test]
async fn additive_information_still_merges_instead_of_flagging_contradiction() {
    let target = sample_memory("Project uses Rust", ProvenanceLevel::UserStated);
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: target.clone(),
        score: 0.95,
        similarity: 0.95,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Project uses Rust and Tauri",
                0.9,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate additive candidate");

    match decision {
        GateDecision::Merge {
            target_id,
            enriched_content,
            ..
        } => {
            assert_eq!(target_id, target.id);
            assert_eq!(enriched_content, "Project uses Rust and Tauri");
        }
        other => panic!("expected merge decision, got {other:?}"),
    }
}

#[tokio::test]
async fn rephrased_content_still_merges_instead_of_flagging_contradiction() {
    let target = sample_memory("Elegy is a memory system", ProvenanceLevel::UserStated);
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: target.clone(),
        score: 0.95,
        similarity: 0.95,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Elegy is a standalone memory system",
                0.9,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate rephrased candidate");

    match decision {
        GateDecision::Merge {
            target_id,
            enriched_content,
            ..
        } => {
            assert_eq!(target_id, target.id);
            assert_eq!(enriched_content, "Elegy is a standalone memory system");
        }
        other => panic!("expected merge decision, got {other:?}"),
    }
}

#[tokio::test]
async fn moderate_similarity_keeps_existing_content_instead_of_concatenating() {
    let target = sample_memory(
        "Launch plan with rollback checklist",
        ProvenanceLevel::UserStated,
    );
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: target.clone(),
        score: 0.94,
        similarity: 0.94,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Launch plan with fallback checklist",
                0.9,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate candidate");

    match decision {
        GateDecision::Merge {
            target_id,
            enriched_content,
            ..
        } => {
            assert_eq!(target_id, target.id);
            assert_eq!(enriched_content, target.content);
            assert!(!enriched_content.contains("\n\n"));
        }
        other => panic!("expected merge decision, got {other:?}"),
    }
}

#[tokio::test]
async fn moderate_similarity_replaces_with_more_detailed_candidate() {
    let target = sample_memory("Launch plan", ProvenanceLevel::UserStated);
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: target.clone(),
        score: 0.94,
        similarity: 0.94,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Launch plan with contingency checklist and rollback owner",
                0.9,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate candidate");

    match decision {
        GateDecision::Merge {
            target_id,
            enriched_content,
            ..
        } => {
            assert_eq!(target_id, target.id);
            assert_eq!(
                enriched_content,
                "Launch plan with contingency checklist and rollback owner"
            );
        }
        other => panic!("expected merge decision, got {other:?}"),
    }
}

#[test]
fn searchable_term_extraction_keeps_compound_word_expansions_for_merge_enrichment() {
    let terms = searchable_terms("ProtonVPN avec WireGuard et JavaScript");

    assert!(terms.contains("protonvpn"));
    assert!(terms.contains("vpn"));
    assert!(terms.contains("wireguard"));
    assert!(terms.contains("javascript"));
    assert!(terms.contains("script"));
}

#[tokio::test]
async fn moderate_similarity_replaces_when_candidate_adds_material_search_terms() {
    let target = sample_memory(
        "ProtonVPN avec WireGuard protege tout le trafic reseau",
        ProvenanceLevel::UserStated,
    );
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: target.clone(),
        score: 0.94,
        similarity: 0.94,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "ProtonVPN avec WireGuard et JavaScript protegent le reseau",
                0.9,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate candidate");

    match decision {
        GateDecision::Merge {
            target_id,
            enriched_content,
            ..
        } => {
            assert_eq!(target_id, target.id);
            assert_eq!(
                enriched_content,
                "ProtonVPN avec WireGuard et JavaScript protegent le reseau"
            );
        }
        other => panic!("expected merge decision, got {other:?}"),
    }
}

#[tokio::test]
async fn accepts_candidates_below_the_likely_duplicate_floor_without_warning() {
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: sample_memory("Existing plan", ProvenanceLevel::UserStated),
        score: 0.79,
        similarity: 0.79,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Existing plan with a distinct rollback path",
                0.8,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate candidate");

    assert_eq!(
        decision,
        GateDecision::Accept {
            similar_to: None,
            similarity: None,
        }
    );
    assert_eq!(store.find_similar_call_count(), 1);
    let calls = store.find_similar_calls();
    assert_eq!(calls[0].0, 4);
    assert!((calls[0].1 - 0.80).abs() < f32::EPSILON);
    assert_eq!(calls[0].2, 1);
}

#[tokio::test]
async fn accepts_candidates_in_the_likely_duplicate_warning_band() {
    let existing = sample_memory("Uses Rust", ProvenanceLevel::UserStated);
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: existing.clone(),
        score: 0.82,
        similarity: 0.82,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Uses Rust and Tauri",
                0.8,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate likely-duplicate candidate");

    assert_eq!(
        decision,
        GateDecision::Accept {
            similar_to: Some(existing.id),
            similarity: Some(0.82),
        }
    );
}

#[tokio::test]
async fn archives_low_salience_candidates() {
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let store = MockStore::default();

    let decision = gate
        .evaluate(
            &sample_candidate("Minor aside", 0.1, ProvenanceLevel::UserStated, None),
            &store,
        )
        .await
        .expect("evaluate low-salience candidate");

    assert_eq!(decision, GateDecision::Archive);
}

#[tokio::test]
async fn archives_low_confidence_inferences_using_architecture_threshold() {
    let gate = DefaultSalienceGate::new(ScopeConfig {
        salience_threshold: 0.2,
        agent_inferred_importance_threshold: 0.5,
        ..ScopeConfig::default()
    });
    let store = MockStore::default();

    let decision = gate
        .evaluate(
            &sample_candidate(
                "The user might prefer morning standups",
                0.45,
                ProvenanceLevel::AgentInferred,
                None,
            ),
            &store,
        )
        .await
        .expect("evaluate inferred candidate");

    assert_eq!(decision, GateDecision::Archive);
}

#[tokio::test]
async fn missing_embedding_skips_novelty_lookup() {
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: sample_memory("Should not be consulted", ProvenanceLevel::UserStated),
        score: 0.99,
        similarity: 0.99,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Important user preference",
                0.9,
                ProvenanceLevel::UserStated,
                None,
            ),
            &store,
        )
        .await
        .expect("evaluate candidate without embedding");

    assert_eq!(
        decision,
        GateDecision::Accept {
            similar_to: None,
            similarity: None,
        }
    );
    assert_eq!(store.find_similar_call_count(), 0);
}

#[tokio::test]
async fn provider_backed_gate_merges_when_candidate_embedding_is_missing() {
    let target = sample_memory("Launch plan", ProvenanceLevel::UserStated);
    let provider = Arc::new(StubEmbeddingProvider::new([(
        "Launch plan with contingency checklist",
        StubEmbeddingResponse::Embedding(vec![0.1, 0.2, 0.3, 0.4]),
    )]));
    let gate =
        DefaultSalienceGate::new_with_embedding_provider(ScopeConfig::default(), provider.clone());
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: target.clone(),
        score: 0.95,
        similarity: 0.95,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Launch plan with contingency checklist",
                0.9,
                ProvenanceLevel::UserStated,
                None,
            ),
            &store,
        )
        .await
        .expect("evaluate candidate with provider-backed novelty lookup");

    match decision {
        GateDecision::Merge {
            target_id,
            enriched_content,
            ..
        } => {
            assert_eq!(target_id, target.id);
            assert_eq!(enriched_content, "Launch plan with contingency checklist");
        }
        other => panic!("expected merge decision, got {other:?}"),
    }
    assert_eq!(
        provider.calls(),
        vec!["Launch plan with contingency checklist".to_string()]
    );
    assert_eq!(store.find_similar_call_count(), 1);
    let calls = store.find_similar_calls();
    assert_eq!(calls[0].0, 4);
}

#[tokio::test]
async fn provider_failure_gracefully_falls_back_to_archive_logic() {
    let provider = Arc::new(StubEmbeddingProvider::new([(
        "Minor aside",
        StubEmbeddingResponse::Failure("provider offline".to_string()),
    )]));
    let gate =
        DefaultSalienceGate::new_with_embedding_provider(ScopeConfig::default(), provider.clone());
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: sample_memory("Should not be consulted", ProvenanceLevel::UserStated),
        score: 0.99,
        similarity: 0.99,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate("Minor aside", 0.1, ProvenanceLevel::UserStated, None),
            &store,
        )
        .await
        .expect("evaluate candidate when provider embedding fails");

    assert_eq!(decision, GateDecision::Archive);
    assert_eq!(provider.calls(), vec!["Minor aside".to_string()]);
    assert_eq!(store.find_similar_call_count(), 0);
}

#[tokio::test]
async fn rejects_near_duplicate_when_match_exists_in_higher_scope() {
    let existing = sample_memory("Shared preference", ProvenanceLevel::UserStated);
    let mut higher = existing.clone();
    higher.scope = MemoryScope::User;
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let store = MockStore::with_scope_and_similar_results(
        MemoryScope::Workspace,
        vec![ScoredMemory {
            memory: higher.clone(),
            score: 0.95,
            similarity: 0.95,
        }],
    );

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Shared preference with tiny wording change",
                0.8,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate higher-scope duplicate");

    assert!(matches!(decision, GateDecision::Reject { .. }));
}

#[tokio::test]
async fn lower_scope_duplicate_requests_merge_and_promotion_to_current_scope() {
    let mut lower = sample_memory("Team procedure", ProvenanceLevel::UserStated);
    lower.scope = MemoryScope::Workspace;
    let gate = DefaultSalienceGate::new(ScopeConfig::default());
    let store = MockStore::with_scope_and_similar_results(
        MemoryScope::User,
        vec![ScoredMemory {
            memory: lower.clone(),
            score: 0.95,
            similarity: 0.95,
        }],
    );

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Team procedure with more detail",
                0.9,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate lower-scope duplicate");

    assert_eq!(
        decision,
        GateDecision::Merge {
            target_id: lower.id,
            enriched_content: "Team procedure with more detail".to_string(),
            promote_to: Some(MemoryScope::User),
        }
    );
}

#[tokio::test]
async fn explicit_candidate_embedding_still_takes_precedence_over_provider() {
    let target = sample_memory("Launch plan", ProvenanceLevel::UserStated);
    let provider = Arc::new(StubEmbeddingProvider::new([(
        "Launch plan with contingency checklist",
        StubEmbeddingResponse::Embedding(vec![9.0; 4]),
    )]));
    let gate =
        DefaultSalienceGate::new_with_embedding_provider(ScopeConfig::default(), provider.clone());
    let store = MockStore::with_similar_results(vec![ScoredMemory {
        memory: target.clone(),
        score: 0.95,
        similarity: 0.95,
    }]);

    let decision = gate
        .evaluate(
            &sample_candidate(
                "Launch plan with contingency checklist",
                0.9,
                ProvenanceLevel::UserStated,
                Some(vec![1.0; 4]),
            ),
            &store,
        )
        .await
        .expect("evaluate candidate with explicit embedding");

    match decision {
        GateDecision::Merge { target_id, .. } => assert_eq!(target_id, target.id),
        other => panic!("expected merge decision, got {other:?}"),
    }
    assert!(provider.calls().is_empty());
    assert_eq!(store.find_similar_call_count(), 1);
    let calls = store.find_similar_calls();
    assert_eq!(calls[0].0, 4);
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

fn sample_memory(content: &str, provenance: ProvenanceLevel) -> Memory {
    let now = Utc::now();
    Memory {
        id: Uuid::new_v4(),
        content: content.to_string(),
        summary: None,
        scope: MemoryScope::Workspace,
        memory_type: MemoryType::Observation,
        provenance,
        importance_score: 0.8,
        reliability_score: provenance.base_reliability(),
        sensitivity: SensitivityLevel::Low,
        state: MemoryState::Active,
        tags: Vec::new(),
        status: None,
        custom_metadata: HashMap::new(),
        access_count: 0,
        corroboration_count: 0,
        embedding_stale: false,
        created_at: now,
        updated_at: now,
        last_accessed_at: Some(now),
        tenant_id: None,
        user_id: None,
        agent_id: None,
    }
}

#[derive(Debug)]
struct StubLlmProvider {
    responses: HashMap<String, Result<String, LlmError>>,
}

impl StubLlmProvider {
    fn new<I, S>(responses: I) -> Self
    where
        I: IntoIterator<Item = (S, Result<String, LlmError>)>,
        S: Into<String>,
    {
        Self {
            responses: responses
                .into_iter()
                .map(|(pair, response)| (pair.into(), response))
                .collect(),
        }
    }
}

#[async_trait]
impl LlmProvider for StubLlmProvider {
    async fn complete(&self, prompt: &str) -> Result<String, LlmError> {
        let memory_a = prompt
            .split("\n\nMemory A:\n")
            .nth(1)
            .and_then(|tail| tail.split("\n\nMemory B:\n").next())
            .unwrap_or("")
            .trim();
        let memory_b = prompt
            .split("\n\nMemory B:\n")
            .nth(1)
            .and_then(|tail| tail.split("\n\nVerdict:").next())
            .unwrap_or("")
            .trim();
        let key = format!("{memory_a}\n---\n{memory_b}");
        match self.responses.get(&key) {
            Some(Ok(response)) => Ok(response.clone()),
            Some(Err(error)) => Err(LlmError::Provider(error.to_string())),
            None => Err(LlmError::Provider(format!(
                "missing llm response for `{key}`"
            ))),
        }
    }

    fn name(&self) -> &str {
        "stub-llm"
    }

    fn model(&self) -> &str {
        "stub-llm-model"
    }
}

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

#[derive(Clone)]
struct MockStore {
    scope: MemoryScope,
    similar_results: Vec<ScoredMemory>,
    find_similar_calls: Arc<Mutex<Vec<(usize, f32, usize)>>>,
}

impl MockStore {
    fn with_similar_results(similar_results: Vec<ScoredMemory>) -> Self {
        Self::with_scope_and_similar_results(MemoryScope::Workspace, similar_results)
    }

    fn with_scope_and_similar_results(
        scope: MemoryScope,
        similar_results: Vec<ScoredMemory>,
    ) -> Self {
        Self {
            scope,
            similar_results,
            find_similar_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn find_similar_call_count(&self) -> usize {
        self.find_similar_calls.lock().expect("lock call log").len()
    }

    fn find_similar_calls(&self) -> Vec<(usize, f32, usize)> {
        self.find_similar_calls
            .lock()
            .expect("lock call log")
            .clone()
    }
}

impl Default for MockStore {
    fn default() -> Self {
        Self::with_scope_and_similar_results(MemoryScope::Workspace, Vec::new())
    }
}

#[async_trait]
impl MemoryStore for MockStore {
    fn scope(&self) -> MemoryScope {
        self.scope
    }

    async fn store(&self, _memory: Memory) -> Result<MemoryId, StoreError> {
        Err(unused_store_error())
    }

    async fn update_content(
        &self,
        _id: &MemoryId,
        _new_content: &str,
        _changed_by: &str,
        _reason: &str,
    ) -> Result<(), StoreError> {
        Err(unused_store_error())
    }

    async fn update_metadata(
        &self,
        _id: &MemoryId,
        _updates: MetadataUpdate,
    ) -> Result<(), StoreError> {
        Err(unused_store_error())
    }

    async fn get(&self, _id: &MemoryId) -> Result<Option<Memory>, StoreError> {
        Err(unused_store_error())
    }

    async fn get_raw(&self, _id: &MemoryId) -> Result<Option<Memory>, StoreError> {
        Err(unused_store_error())
    }

    async fn list(&self, _filter: MemoryFilter) -> Result<Vec<Memory>, StoreError> {
        Err(unused_store_error())
    }

    async fn search(&self, _query: SearchQuery) -> Result<Vec<ScoredMemory>, StoreError> {
        Err(unused_store_error())
    }

    async fn find_similar(
        &self,
        embedding: &[f32],
        threshold: f32,
        limit: usize,
    ) -> Result<Vec<ScoredMemory>, StoreError> {
        self.find_similar_calls
            .lock()
            .expect("lock call log")
            .push((embedding.len(), threshold, limit));
        Ok(self.similar_results.iter().take(limit).cloned().collect())
    }

    async fn store_embedding(&self, _id: &MemoryId, _embedding: &[f32]) -> Result<(), StoreError> {
        Err(unused_store_error())
    }

    async fn get_stale_embeddings(&self, _limit: usize) -> Result<Vec<MemoryId>, StoreError> {
        Err(unused_store_error())
    }

    async fn make_dormant(&self, _id: &MemoryId) -> Result<(), StoreError> {
        Err(unused_store_error())
    }

    async fn reactivate(&self, _id: &MemoryId) -> Result<(), StoreError> {
        Err(unused_store_error())
    }

    async fn hard_delete(&self, _id: &MemoryId) -> Result<(), StoreError> {
        Err(unused_store_error())
    }

    async fn purge_user(&self, _user_id: &str) -> Result<PurgeReport, StoreError> {
        Err(unused_store_error())
    }

    async fn purge_all(&self) -> Result<PurgeReport, StoreError> {
        Err(unused_store_error())
    }

    async fn health_report(&self) -> Result<MemoryHealthReport, StoreError> {
        Err(unused_store_error())
    }

    async fn list_contradictions(
        &self,
        _status: Option<ResolutionStatus>,
    ) -> Result<Vec<ContradictionEntry>, StoreError> {
        Err(unused_store_error())
    }

    async fn record_contradiction(
        &self,
        _a_id: &MemoryId,
        _b_id: &MemoryId,
        _description: &str,
    ) -> Result<(), StoreError> {
        Err(unused_store_error())
    }

    async fn update_contradiction_status(
        &self,
        _contradiction_id: &str,
        _status: ResolutionStatus,
        _note: Option<&str>,
    ) -> Result<(), StoreError> {
        Err(unused_store_error())
    }
}

fn unused_store_error() -> StoreError {
    StoreError::Validation("unused mock store method".to_string())
}
