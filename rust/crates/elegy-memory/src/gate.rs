use async_trait::async_trait;

use crate::{
    GateDecision, GateError, MemoryCandidate, MemoryStore, ProvenanceLevel, SalienceGate,
    ScopeConfig,
};

/// Default MVP salience gate using scope-configured novelty and salience thresholds.
#[derive(Debug, Clone)]
pub struct DefaultSalienceGate {
    scope_config: ScopeConfig,
}

impl DefaultSalienceGate {
    /// Create a new salience gate from an already-loaded scope configuration.
    #[must_use]
    pub fn new(scope_config: ScopeConfig) -> Self {
        Self { scope_config }
    }

    fn validate_candidate(&self, candidate: &MemoryCandidate) -> Result<(), GateError> {
        if candidate.content.trim().is_empty() {
            return Err(GateError::InvalidCandidate(
                "candidate content must not be empty".to_string(),
            ));
        }
        if !(0.0..=1.0).contains(&candidate.importance_score)
            || !candidate.importance_score.is_finite()
        {
            return Err(GateError::InvalidCandidate(
                "candidate importance_score must be finite and within 0.0..=1.0".to_string(),
            ));
        }
        if candidate.embedding.as_ref().is_some_and(Vec::is_empty) {
            return Err(GateError::InvalidCandidate(
                "candidate embedding must not be empty when provided".to_string(),
            ));
        }

        Ok(())
    }

    fn novelty_floor(&self) -> f32 {
        self.scope_config
            .novelty_doubt_threshold
            .clamp(0.0, 1.0)
            .min(self.scope_config.merge_similarity_threshold.clamp(0.0, 1.0))
    }

    fn should_merge(&self, similarity: f32) -> bool {
        similarity > self.scope_config.merge_similarity_threshold.clamp(0.0, 1.0)
    }

    fn merge_content(existing_content: &str, candidate_content: &str) -> String {
        let existing_content = existing_content.trim();
        let candidate_content = candidate_content.trim();

        if normalize_for_merge(existing_content) == normalize_for_merge(candidate_content) {
            return existing_content.to_string();
        }
        if candidate_content.contains(existing_content) {
            return candidate_content.to_string();
        }
        if existing_content.contains(candidate_content) {
            return existing_content.to_string();
        }

        format!("{existing_content}\n\n{candidate_content}")
    }
}

#[async_trait]
impl SalienceGate for DefaultSalienceGate {
    async fn evaluate(
        &self,
        candidate: &MemoryCandidate,
        store: &dyn MemoryStore,
    ) -> Result<GateDecision, GateError> {
        self.validate_candidate(candidate)?;

        if let Some(embedding) = candidate.embedding.as_deref() {
            let matches = store
                .find_similar(embedding, self.novelty_floor(), 1)
                .await?;
            if let Some(best_match) = matches.into_iter().next() {
                if self.should_merge(best_match.similarity) {
                    return Ok(GateDecision::Merge {
                        target_id: best_match.memory.id,
                        enriched_content: Self::merge_content(
                            &best_match.memory.content,
                            &candidate.content,
                        ),
                    });
                }
            }
        }

        if candidate.importance_score < self.scope_config.salience_threshold {
            return Ok(GateDecision::Archive);
        }

        if candidate.provenance == ProvenanceLevel::AgentInferred
            && candidate.importance_score < self.scope_config.agent_inferred_importance_threshold
        {
            return Ok(GateDecision::Archive);
        }

        Ok(GateDecision::Accept)
    }
}

fn normalize_for_merge(content: &str) -> String {
    content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use chrono::Utc;
    use uuid::Uuid;

    use super::DefaultSalienceGate;
    use crate::{
        ContradictionEntry, GateDecision, Memory, MemoryCandidate, MemoryFilter,
        MemoryHealthReport, MemoryId, MemoryScope, MemoryState, MemoryStore, MemoryType,
        MetadataUpdate, ProvenanceLevel, PurgeReport, ResolutionStatus, SalienceGate, ScopeConfig,
        ScoredMemory, SearchQuery, SensitivityLevel, StoreError,
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
            } => {
                assert_eq!(target_id, target.id);
                assert_eq!(enriched_content, "Launch plan with contingency checklist");
            }
            other => panic!("expected merge decision, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn accepts_candidates_in_the_novelty_doubt_zone() {
        let gate = DefaultSalienceGate::new(ScopeConfig::default());
        let store = MockStore::with_similar_results(vec![ScoredMemory {
            memory: sample_memory("Existing plan", ProvenanceLevel::UserStated),
            score: 0.90,
            similarity: 0.90,
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

        assert_eq!(decision, GateDecision::Accept);
        assert_eq!(store.find_similar_call_count(), 1);
        let calls = store.find_similar_calls();
        assert_eq!(calls[0].0, 4);
        assert!((calls[0].1 - 0.85).abs() < f32::EPSILON);
        assert_eq!(calls[0].2, 1);
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

        assert_eq!(decision, GateDecision::Accept);
        assert_eq!(store.find_similar_call_count(), 0);
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

    #[derive(Clone, Default)]
    struct MockStore {
        similar_results: Vec<ScoredMemory>,
        find_similar_calls: Arc<Mutex<Vec<(usize, f32, usize)>>>,
    }

    impl MockStore {
        fn with_similar_results(similar_results: Vec<ScoredMemory>) -> Self {
            Self {
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

    #[async_trait]
    impl MemoryStore for MockStore {
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

        async fn store_embedding(
            &self,
            _id: &MemoryId,
            _embedding: &[f32],
        ) -> Result<(), StoreError> {
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
    }

    fn unused_store_error() -> StoreError {
        StoreError::Validation("unused mock store method".to_string())
    }
}
