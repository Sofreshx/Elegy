use async_trait::async_trait;

use crate::{
    similarity::cosine_similarity, ConsolidationAction, ConsolidationCandidate, ConsolidationError,
    MemoryConsolidator, MemoryState, ScopeConfig,
};

/// Simple MVP consolidator that reports high-similarity active-memory dedup actions.
#[derive(Debug, Clone)]
pub struct SimpleConsolidator {
    similarity_threshold: f32,
}

impl SimpleConsolidator {
    /// Create a consolidator using the scope-configured merge similarity threshold.
    #[must_use]
    pub fn new(scope_config: ScopeConfig) -> Self {
        Self::with_threshold(scope_config.merge_similarity_threshold)
    }

    /// Create a consolidator with an explicit cosine threshold.
    #[must_use]
    pub fn with_threshold(similarity_threshold: f32) -> Self {
        Self {
            similarity_threshold: normalize_threshold(similarity_threshold),
        }
    }

    fn is_eligible(candidate: &ConsolidationCandidate) -> bool {
        candidate.memory.state == MemoryState::Active
            && candidate
                .embedding
                .as_ref()
                .is_some_and(|embedding| !embedding.is_empty())
    }
}

impl Default for SimpleConsolidator {
    fn default() -> Self {
        Self::new(ScopeConfig::default())
    }
}

#[async_trait]
impl MemoryConsolidator for SimpleConsolidator {
    async fn consolidate(
        &self,
        memories: &[ConsolidationCandidate],
    ) -> Result<Vec<ConsolidationAction>, ConsolidationError> {
        let mut candidate_order = memories
            .iter()
            .enumerate()
            .filter(|(_, candidate)| Self::is_eligible(candidate))
            .collect::<Vec<_>>();

        candidate_order.sort_by(|(left_index, left), (right_index, right)| {
            right
                .memory
                .importance_score
                .total_cmp(&left.memory.importance_score)
                .then_with(|| left_index.cmp(right_index))
        });

        let mut consumed = vec![false; memories.len()];
        let mut actions = Vec::new();

        for (position, (survivor_index, survivor_candidate)) in candidate_order.iter().enumerate() {
            let survivor_index = *survivor_index;
            if consumed[survivor_index] {
                continue;
            }

            let survivor_embedding = survivor_candidate
                .embedding
                .as_deref()
                .expect("eligible candidates always have embeddings");
            let mut merged_source_ids = Vec::new();

            for (other_index, other_candidate) in candidate_order.iter().skip(position + 1) {
                let other_index = *other_index;
                if consumed[other_index] {
                    continue;
                }

                let other_embedding = other_candidate
                    .embedding
                    .as_deref()
                    .expect("eligible candidates always have embeddings");
                let similarity = cosine_similarity(survivor_embedding, other_embedding)?;

                if similarity > self.similarity_threshold {
                    consumed[other_index] = true;
                    merged_source_ids.push(other_candidate.memory.id);
                }
            }

            if !merged_source_ids.is_empty() {
                actions.push(ConsolidationAction::Merged {
                    source_ids: merged_source_ids,
                    result: survivor_candidate.memory.clone(),
                });
            }
        }

        Ok(actions)
    }
}

fn normalize_threshold(similarity_threshold: f32) -> f32 {
    if similarity_threshold.is_finite() {
        similarity_threshold.clamp(0.0, 1.0)
    } else {
        ScopeConfig::default().merge_similarity_threshold
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::Utc;
    use uuid::Uuid;

    use super::SimpleConsolidator;
    use crate::{
        ConsolidationAction, ConsolidationCandidate, Memory, MemoryConsolidator, MemoryScope,
        MemoryState, MemoryType, ProvenanceLevel, ScopeConfig, SensitivityLevel,
    };

    #[tokio::test]
    async fn reports_merge_for_active_candidates_above_threshold() {
        let survivor = candidate(
            sample_memory("High priority plan", 0.9, MemoryState::Active),
            Some(vec![1.0, 0.0]),
        );
        let duplicate = candidate(
            sample_memory("Near duplicate plan", 0.3, MemoryState::Active),
            Some(vec![0.95, 0.05]),
        );

        let actions = SimpleConsolidator::default()
            .consolidate(&[survivor.clone(), duplicate.clone()])
            .await
            .expect("consolidate candidates");

        assert_eq!(
            actions,
            vec![ConsolidationAction::Merged {
                source_ids: vec![duplicate.memory.id],
                result: survivor.memory,
            }]
        );
    }

    #[tokio::test]
    async fn keeps_higher_importance_memory_as_survivor_even_if_it_appears_later() {
        let lower_importance = candidate(
            sample_memory("Draft checklist", 0.2, MemoryState::Active),
            Some(vec![1.0, 0.0]),
        );
        let higher_importance = candidate(
            sample_memory("Canonical checklist", 0.8, MemoryState::Active),
            Some(vec![0.97, 0.03]),
        );

        let actions = SimpleConsolidator::default()
            .consolidate(&[lower_importance.clone(), higher_importance.clone()])
            .await
            .expect("consolidate candidates");

        assert_eq!(
            actions,
            vec![ConsolidationAction::Merged {
                source_ids: vec![lower_importance.memory.id],
                result: higher_importance.memory,
            }]
        );
    }

    #[tokio::test]
    async fn skips_candidates_without_embeddings_and_below_threshold_pairs() {
        let no_embedding = candidate(
            sample_memory("Needs re-embedding", 0.7, MemoryState::Active),
            None,
        );
        let below_threshold = candidate(
            sample_memory("Distinct memory", 0.6, MemoryState::Active),
            Some(vec![0.7, 0.7]),
        );
        let survivor = candidate(
            sample_memory("Primary memory", 0.9, MemoryState::Active),
            Some(vec![1.0, 0.0]),
        );

        let actions = SimpleConsolidator::default()
            .consolidate(&[no_embedding, below_threshold, survivor])
            .await
            .expect("consolidate candidates");

        assert!(actions.is_empty());
    }

    #[tokio::test]
    async fn ignores_dormant_candidates_even_with_high_similarity() {
        let active = candidate(
            sample_memory("Active memory", 0.8, MemoryState::Active),
            Some(vec![1.0, 0.0]),
        );
        let dormant = candidate(
            sample_memory("Dormant duplicate", 0.9, MemoryState::Dormant),
            Some(vec![0.99, 0.01]),
        );

        let actions = SimpleConsolidator::default()
            .consolidate(&[active, dormant])
            .await
            .expect("consolidate candidates");

        assert!(actions.is_empty());
    }

    #[tokio::test]
    async fn respects_custom_threshold_configuration() {
        let consolidator = SimpleConsolidator::new(ScopeConfig {
            merge_similarity_threshold: 0.98,
            ..ScopeConfig::default()
        });
        let first = candidate(
            sample_memory("High similarity A", 0.8, MemoryState::Active),
            Some(vec![1.0, 0.0]),
        );
        let second = candidate(
            sample_memory("High similarity B", 0.6, MemoryState::Active),
            Some(vec![0.8, 0.2]),
        );

        let actions = consolidator
            .consolidate(&[first, second])
            .await
            .expect("consolidate candidates");

        assert!(actions.is_empty());
    }

    fn candidate(memory: Memory, embedding: Option<Vec<f32>>) -> ConsolidationCandidate {
        ConsolidationCandidate { memory, embedding }
    }

    fn sample_memory(content: &str, importance_score: f32, state: MemoryState) -> Memory {
        let now = Utc::now();
        Memory {
            id: Uuid::new_v4(),
            content: content.to_string(),
            summary: None,
            scope: MemoryScope::Workspace,
            memory_type: MemoryType::Fact,
            provenance: ProvenanceLevel::UserStated,
            importance_score,
            reliability_score: ProvenanceLevel::UserStated.base_reliability(),
            sensitivity: SensitivityLevel::Low,
            state,
            tags: Vec::new(),
            status: None,
            custom_metadata: HashMap::new(),
            access_count: 0,
            corroboration_count: 0,
            embedding_stale: false,
            created_at: now,
            updated_at: now,
            last_accessed_at: None,
            tenant_id: None,
            user_id: None,
            agent_id: None,
        }
    }
}
