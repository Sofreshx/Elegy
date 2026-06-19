use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    embedding::{prepare_embedding_input, EmbeddingTask},
    similarity::cosine_similarity,
    ConsolidationAction, ConsolidationCandidate, ConsolidationError, EmbeddingProvider,
    LlmProvider, MemoryConsolidator, MemoryState, ScopeConfig,
};

/// Simple consolidator that reports high-similarity active-memory dedup actions.
#[derive(Debug, Clone)]
pub struct SimpleConsolidator {
    similarity_threshold: f32,
    cross_scope: bool,
    pair_limit: Option<usize>,
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
            cross_scope: false,
            pair_limit: None,
        }
    }

    /// Enable or disable cross-scope consolidation.
    #[must_use]
    pub fn with_cross_scope(mut self, cross_scope: bool) -> Self {
        self.cross_scope = cross_scope;
        self
    }

    /// Cap the number of qualifying candidate pairs processed in one pass.
    #[must_use]
    pub fn with_pair_limit(mut self, pair_limit: Option<usize>) -> Self {
        self.pair_limit = pair_limit.filter(|limit| *limit > 0);
        self
    }
}

impl Default for SimpleConsolidator {
    fn default() -> Self {
        Self::new(ScopeConfig::default())
    }
}

/// LLM-backed consolidator with per-pair graceful fallback to [`SimpleConsolidator`] semantics.
#[derive(Clone)]
pub struct LlmConsolidator {
    similarity_threshold: f32,
    cross_scope: bool,
    pair_limit: Option<usize>,
    llm_provider: Arc<dyn LlmProvider>,
    embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
}

impl std::fmt::Debug for LlmConsolidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmConsolidator")
            .field("similarity_threshold", &self.similarity_threshold)
            .field("cross_scope", &self.cross_scope)
            .field("pair_limit", &self.pair_limit)
            .field("llm_provider", &self.llm_provider.name())
            .field("llm_model", &self.llm_provider.model())
            .field("has_embedding_provider", &self.embedding_provider.is_some())
            .finish()
    }
}

impl LlmConsolidator {
    /// Create an LLM consolidator using the scope-configured merge similarity threshold.
    #[must_use]
    pub fn new(scope_config: ScopeConfig, llm_provider: Arc<dyn LlmProvider>) -> Self {
        Self::with_threshold(llm_provider, scope_config.merge_similarity_threshold)
    }

    /// Create an LLM consolidator with an explicit cosine threshold.
    #[must_use]
    pub fn with_threshold(llm_provider: Arc<dyn LlmProvider>, similarity_threshold: f32) -> Self {
        Self {
            similarity_threshold: normalize_threshold(similarity_threshold),
            cross_scope: false,
            pair_limit: None,
            llm_provider,
            embedding_provider: None,
        }
    }

    /// Enable or disable cross-scope consolidation.
    #[must_use]
    pub fn with_cross_scope(mut self, cross_scope: bool) -> Self {
        self.cross_scope = cross_scope;
        self
    }

    /// Cap the number of qualifying candidate pairs processed in one pass.
    #[must_use]
    pub fn with_pair_limit(mut self, pair_limit: Option<usize>) -> Self {
        self.pair_limit = pair_limit.filter(|limit| *limit > 0);
        self
    }

    /// Attach an embedding provider for candidates whose stored embeddings are missing.
    #[must_use]
    pub fn with_embedding_provider(
        mut self,
        embedding_provider: Arc<dyn EmbeddingProvider>,
    ) -> Self {
        self.embedding_provider = Some(embedding_provider);
        self
    }

    async fn prepared_embeddings(
        &self,
        memories: &[ConsolidationCandidate],
    ) -> Vec<Option<Vec<f32>>> {
        let mut prepared = Vec::with_capacity(memories.len());
        for candidate in memories {
            prepared.push(self.embedding_for(candidate).await);
        }
        prepared
    }

    async fn embedding_for(&self, candidate: &ConsolidationCandidate) -> Option<Vec<f32>> {
        if candidate.memory.state != MemoryState::Active {
            return None;
        }

        if let Some(embedding) = candidate
            .embedding
            .as_ref()
            .filter(|embedding| !embedding.is_empty())
        {
            return Some(embedding.clone());
        }

        let provider = self.embedding_provider.as_ref()?;
        let prepared_input = prepare_embedding_input(
            provider.as_ref(),
            EmbeddingTask::Document,
            &candidate.memory.content,
        );
        match provider.embed(prepared_input.as_ref()).await {
            Ok(embedding) if !embedding.is_empty() => Some(embedding),
            Ok(_) => {
                eprintln!(
                    "warning: consolidation skipped memory {} because the embedding provider returned an empty vector",
                    candidate.memory.id
                );
                None
            }
            Err(error) => {
                eprintln!(
                    "warning: consolidation skipped memory {} because embeddings could not be generated: {error}",
                    candidate.memory.id
                );
                None
            }
        }
    }
}

#[async_trait]
impl MemoryConsolidator for SimpleConsolidator {
    async fn consolidate(
        &self,
        memories: &[ConsolidationCandidate],
    ) -> Result<Vec<ConsolidationAction>, ConsolidationError> {
        consolidate_with_simple_strategy(
            memories,
            self.similarity_threshold,
            self.cross_scope,
            self.pair_limit,
        )
    }
}

#[async_trait]
impl MemoryConsolidator for LlmConsolidator {
    async fn consolidate(
        &self,
        memories: &[ConsolidationCandidate],
    ) -> Result<Vec<ConsolidationAction>, ConsolidationError> {
        let prepared_embeddings = self.prepared_embeddings(memories).await;
        let candidate_order = prioritized_candidates(memories, &prepared_embeddings);
        let mut consumed = vec![false; memories.len()];
        let mut processed_pairs = 0usize;
        let mut limit_reached = false;
        let mut actions = Vec::new();

        'survivors: for (position, survivor_index) in candidate_order.iter().enumerate() {
            let survivor_index = *survivor_index;
            if consumed[survivor_index] {
                continue;
            }

            let Some(survivor_embedding) = prepared_embeddings[survivor_index].as_deref() else {
                continue;
            };
            let survivor_candidate = &memories[survivor_index];
            let mut merged_source_ids = Vec::new();
            let mut highest_scope = survivor_candidate.memory.scope;
            let mut result_content = survivor_candidate.memory.content.clone();

            for other_index in candidate_order.iter().skip(position + 1) {
                let other_index = *other_index;
                if consumed[other_index] {
                    continue;
                }

                let other_candidate = &memories[other_index];
                if !self.cross_scope
                    && survivor_candidate.memory.scope != other_candidate.memory.scope
                {
                    continue;
                }

                let Some(other_embedding) = prepared_embeddings[other_index].as_deref() else {
                    continue;
                };
                let similarity = cosine_similarity(survivor_embedding, other_embedding)?;
                if similarity < self.similarity_threshold {
                    continue;
                }

                if self
                    .pair_limit
                    .is_some_and(|limit| processed_pairs >= limit)
                {
                    limit_reached = true;
                    break;
                }
                processed_pairs += 1;

                let prompt =
                    build_consolidation_prompt(&result_content, &other_candidate.memory.content);
                match self.llm_provider.complete(&prompt).await {
                    Ok(response) => match parse_consolidation_response(&response) {
                        Some(ConsolidationVerdict::Merged(content)) => {
                            result_content = content;
                            consumed[other_index] = true;
                            merged_source_ids.push(other_candidate.memory.id);
                            highest_scope = highest_scope.max(other_candidate.memory.scope);
                        }
                        Some(ConsolidationVerdict::Contradiction(description)) => {
                            actions.push(ConsolidationAction::Contradiction {
                                memory_a_id: survivor_candidate.memory.id,
                                memory_b_id: other_candidate.memory.id,
                                description,
                            });
                        }
                        None => {
                            eprintln!(
                                "warning: {} ({}) returned an unusable consolidation response; falling back to simple consolidation for {} and {}",
                                self.llm_provider.name(),
                                self.llm_provider.model(),
                                survivor_candidate.memory.id,
                                other_candidate.memory.id
                            );
                            consumed[other_index] = true;
                            merged_source_ids.push(other_candidate.memory.id);
                            highest_scope = highest_scope.max(other_candidate.memory.scope);
                            result_content = simple_merge_content(
                                &result_content,
                                &other_candidate.memory.content,
                                similarity,
                            );
                        }
                    },
                    Err(error) => {
                        eprintln!(
                            "warning: {} ({}) failed during consolidation for {} and {}: {error}. Falling back to simple consolidation.",
                            self.llm_provider.name(),
                            self.llm_provider.model(),
                            survivor_candidate.memory.id,
                            other_candidate.memory.id
                        );
                        consumed[other_index] = true;
                        merged_source_ids.push(other_candidate.memory.id);
                        highest_scope = highest_scope.max(other_candidate.memory.scope);
                        result_content = simple_merge_content(
                            &result_content,
                            &other_candidate.memory.content,
                            similarity,
                        );
                    }
                }
            }

            if !merged_source_ids.is_empty() {
                let mut result = survivor_candidate.memory.clone();
                result.content = result_content;
                if self.cross_scope {
                    result.scope = highest_scope;
                }
                actions.push(ConsolidationAction::Merged {
                    source_ids: merged_source_ids,
                    result,
                });
            }

            if limit_reached {
                break 'survivors;
            }
        }

        Ok(actions)
    }
}

fn consolidate_with_simple_strategy(
    memories: &[ConsolidationCandidate],
    similarity_threshold: f32,
    cross_scope: bool,
    pair_limit: Option<usize>,
) -> Result<Vec<ConsolidationAction>, ConsolidationError> {
    let candidate_order = prioritized_candidates(memories, &prepare_stored_embeddings(memories));
    let mut consumed = vec![false; memories.len()];
    let mut processed_pairs = 0usize;
    let mut limit_reached = false;
    let mut actions = Vec::new();

    'survivors: for (position, survivor_index) in candidate_order.iter().enumerate() {
        let survivor_index = *survivor_index;
        if consumed[survivor_index] {
            continue;
        }

        let survivor_candidate = &memories[survivor_index];
        let Some(survivor_embedding) = survivor_candidate.embedding.as_deref() else {
            continue;
        };
        let mut merged_source_ids = Vec::new();
        let mut highest_scope = survivor_candidate.memory.scope;

        for other_index in candidate_order.iter().skip(position + 1) {
            let other_index = *other_index;
            if consumed[other_index] {
                continue;
            }

            let other_candidate = &memories[other_index];
            if !cross_scope && survivor_candidate.memory.scope != other_candidate.memory.scope {
                continue;
            }

            let Some(other_embedding) = other_candidate.embedding.as_deref() else {
                continue;
            };
            let similarity = cosine_similarity(survivor_embedding, other_embedding)?;
            if similarity < similarity_threshold {
                continue;
            }

            if pair_limit.is_some_and(|limit| processed_pairs >= limit) {
                limit_reached = true;
                break;
            }
            processed_pairs += 1;

            consumed[other_index] = true;
            merged_source_ids.push(other_candidate.memory.id);
            highest_scope = highest_scope.max(other_candidate.memory.scope);
        }

        if !merged_source_ids.is_empty() {
            let mut result = survivor_candidate.memory.clone();
            if cross_scope {
                result.scope = highest_scope;
            }
            actions.push(ConsolidationAction::Merged {
                source_ids: merged_source_ids,
                result,
            });
        }

        if limit_reached {
            break 'survivors;
        }
    }

    Ok(actions)
}

fn prepare_stored_embeddings(memories: &[ConsolidationCandidate]) -> Vec<Option<Vec<f32>>> {
    memories
        .iter()
        .map(|candidate| {
            (candidate.memory.state == MemoryState::Active)
                .then_some(candidate.embedding.as_ref())
                .flatten()
                .filter(|embedding| !embedding.is_empty())
                .cloned()
        })
        .collect()
}

fn prioritized_candidates(
    memories: &[ConsolidationCandidate],
    embeddings: &[Option<Vec<f32>>],
) -> Vec<usize> {
    let mut candidate_order = memories
        .iter()
        .enumerate()
        .filter(|(index, candidate)| {
            candidate.memory.state == MemoryState::Active
                && embeddings
                    .get(*index)
                    .and_then(|embedding| embedding.as_ref())
                    .is_some()
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    candidate_order.sort_by(|left_index, right_index| {
        memories[*right_index]
            .memory
            .importance_score
            .total_cmp(&memories[*left_index].memory.importance_score)
            .then_with(|| left_index.cmp(right_index))
    });

    candidate_order
}

fn simple_merge_content(
    existing_content: &str,
    candidate_content: &str,
    similarity: f32,
) -> String {
    let existing_content = existing_content.trim();
    let candidate_content = candidate_content.trim();
    let normalized_existing = normalize_for_merge(existing_content);
    let normalized_candidate = normalize_for_merge(candidate_content);

    if similarity >= 0.95 {
        return candidate_content.to_string();
    }
    if normalized_existing == normalized_candidate {
        return existing_content.to_string();
    }
    if normalized_candidate.contains(&normalized_existing)
        || candidate_content.chars().count() > (existing_content.chars().count() * 6 / 5)
    {
        return candidate_content.to_string();
    }
    existing_content.to_string()
}

fn normalize_for_merge(content: &str) -> String {
    content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn build_consolidation_prompt(memory_a_content: &str, memory_b_content: &str) -> String {
    format!(
        "You are a memory consolidation agent. You receive two related memories and must produce a single consolidated version.\n\nRules:\n- Include ALL facts from both memories. Do not drop information.\n- If the memories agree, synthesize into one clear statement.\n- If the memories contradict, respond with CONTRADICTION and explain the conflict.\n- Do not add any information not present in the original memories.\n- Be concise. One to three sentences maximum.\n- Respond ONLY with the consolidated memory text, or CONTRADICTION: <explanation>.\n\nMemory A:\n{memory_a_content}\n\nMemory B:\n{memory_b_content}\n\nConsolidated:"
    )
}

enum ConsolidationVerdict {
    Merged(String),
    Contradiction(String),
}

fn parse_consolidation_response(response: &str) -> Option<ConsolidationVerdict> {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.len() >= "CONTRADICTION".len()
        && trimmed[.."CONTRADICTION".len()].eq_ignore_ascii_case("CONTRADICTION")
    {
        let description = trimmed.split_once(':').map_or(
            "llm consolidation marked the pair as contradictory",
            |(_, detail)| detail.trim(),
        );
        let description = if description.is_empty() {
            "llm consolidation marked the pair as contradictory"
        } else {
            description
        };
        return Some(ConsolidationVerdict::Contradiction(description.to_string()));
    }

    let stripped = trimmed
        .strip_prefix("Consolidated:")
        .or_else(|| trimmed.strip_prefix("consolidated:"))
        .map(str::trim)
        .unwrap_or(trimmed);
    if stripped.is_empty() {
        return None;
    }
    let lower = stripped.to_ascii_lowercase();
    if lower.contains("memory a:")
        || lower.contains("memory b:")
        || lower.contains("rules:")
        || lower.contains("you are a memory consolidation agent")
    {
        return None;
    }
    Some(ConsolidationVerdict::Merged(stripped.to_string()))
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
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use chrono::Utc;
    use uuid::Uuid;

    use super::{LlmConsolidator, SimpleConsolidator};
    use crate::{
        ConsolidationAction, ConsolidationCandidate, EmbeddingError, EmbeddingProvider, LlmError,
        LlmProvider, Memory, MemoryConsolidator, MemoryScope, MemoryState, MemoryType,
        ProvenanceLevel, ScopeConfig, SensitivityLevel,
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

    #[tokio::test]
    async fn merges_near_duplicates_at_the_lowered_default_threshold() {
        let survivor = candidate(
            sample_memory("Project uses Rust", 0.9, MemoryState::Active),
            Some(vec![1.0, 0.0]),
        );
        let duplicate = candidate(
            sample_memory("Project uses Rust and Tauri", 0.6, MemoryState::Active),
            Some(vec![0.86, (1.0_f32 - 0.86_f32.powi(2)).sqrt()]),
        );

        let actions = SimpleConsolidator::default()
            .consolidate(&[survivor.clone(), duplicate.clone()])
            .await
            .expect("consolidate near-duplicate candidates");

        assert_eq!(
            actions,
            vec![ConsolidationAction::Merged {
                source_ids: vec![duplicate.memory.id],
                result: survivor.memory,
            }]
        );
    }

    #[tokio::test]
    async fn cross_scope_consolidation_promotes_result_to_highest_scope_in_pair() {
        let lower = candidate(
            sample_memory("Workspace duplicate", 0.9, MemoryState::Active),
            Some(vec![1.0, 0.0]),
        );
        let mut higher_memory = sample_memory("User duplicate", 0.4, MemoryState::Active);
        higher_memory.scope = MemoryScope::User;
        let higher = candidate(higher_memory.clone(), Some(vec![0.95, 0.05]));

        let actions = SimpleConsolidator::default()
            .with_cross_scope(true)
            .consolidate(&[lower, higher])
            .await
            .expect("consolidate cross-scope candidates");

        assert_eq!(actions.len(), 1);
        let ConsolidationAction::Merged { result, .. } = &actions[0] else {
            panic!("expected merged action");
        };
        assert_eq!(result.scope, MemoryScope::User);
    }

    #[tokio::test]
    async fn consolidate_limit_caps_simple_pair_processing() {
        let survivor = candidate(
            sample_memory("Canonical memory", 0.9, MemoryState::Active),
            Some(vec![1.0, 0.0]),
        );
        let first_duplicate = candidate(
            sample_memory("First duplicate", 0.5, MemoryState::Active),
            Some(vec![0.97, 0.03]),
        );
        let second_duplicate = candidate(
            sample_memory("Second duplicate", 0.4, MemoryState::Active),
            Some(vec![0.96, 0.04]),
        );

        let actions = SimpleConsolidator::default()
            .with_pair_limit(Some(1))
            .consolidate(&[
                survivor.clone(),
                first_duplicate.clone(),
                second_duplicate.clone(),
            ])
            .await
            .expect("consolidate with pair limit");

        assert_eq!(
            actions,
            vec![ConsolidationAction::Merged {
                source_ids: vec![first_duplicate.memory.id],
                result: survivor.memory,
            }]
        );
    }

    #[tokio::test]
    async fn llm_consolidator_uses_provider_merged_text() {
        let llm = Arc::new(StubLlmProvider::new([(
            "High priority plan\n---\nNear duplicate plan",
            Ok("Merged plan with both facts".to_string()),
        )]));
        let actions = LlmConsolidator::new(ScopeConfig::default(), llm)
            .consolidate(&[
                candidate(
                    sample_memory("High priority plan", 0.9, MemoryState::Active),
                    Some(vec![1.0, 0.0]),
                ),
                candidate(
                    sample_memory("Near duplicate plan", 0.3, MemoryState::Active),
                    Some(vec![0.95, 0.05]),
                ),
            ])
            .await
            .expect("llm consolidate");

        assert_eq!(actions.len(), 1);
        let ConsolidationAction::Merged { result, .. } = &actions[0] else {
            panic!("expected merged action");
        };
        assert_eq!(result.content, "Merged plan with both facts");
    }

    #[tokio::test]
    async fn llm_consolidator_records_contradiction_actions() {
        let llm = Arc::new(StubLlmProvider::new([(
            "Backend is C# with gRPC\n---\nBackend is Python with Flask",
            Ok("CONTRADICTION: incompatible backend stack".to_string()),
        )]));
        let first = candidate(
            sample_memory("Backend is C# with gRPC", 0.9, MemoryState::Active),
            Some(vec![1.0, 0.0]),
        );
        let second = candidate(
            sample_memory("Backend is Python with Flask", 0.5, MemoryState::Active),
            Some(vec![0.95, 0.05]),
        );

        let actions = LlmConsolidator::new(ScopeConfig::default(), llm)
            .consolidate(&[first.clone(), second.clone()])
            .await
            .expect("llm consolidate");

        assert_eq!(
            actions,
            vec![ConsolidationAction::Contradiction {
                memory_a_id: first.memory.id,
                memory_b_id: second.memory.id,
                description: "incompatible backend stack".to_string(),
            }]
        );
    }

    #[tokio::test]
    async fn llm_consolidator_falls_back_to_simple_merge_for_empty_response() {
        let llm = Arc::new(StubLlmProvider::new([(
            "Project uses Rust\n---\nProject uses Rust and Tauri",
            Ok(String::new()),
        )]));
        let survivor = candidate(
            sample_memory("Project uses Rust", 0.9, MemoryState::Active),
            Some(vec![1.0, 0.0]),
        );
        let duplicate = candidate(
            sample_memory("Project uses Rust and Tauri", 0.6, MemoryState::Active),
            Some(vec![0.96, 0.04]),
        );

        let actions = LlmConsolidator::new(ScopeConfig::default(), llm)
            .consolidate(&[survivor, duplicate.clone()])
            .await
            .expect("llm consolidate");

        assert_eq!(actions.len(), 1);
        let ConsolidationAction::Merged { result, source_ids } = &actions[0] else {
            panic!("expected merged action");
        };
        assert_eq!(source_ids, &vec![duplicate.memory.id]);
        assert_eq!(result.content, "Project uses Rust and Tauri");
    }

    #[tokio::test]
    async fn llm_consolidator_can_backfill_missing_embeddings_with_provider() {
        let llm = Arc::new(StubLlmProvider::new([(
            "Canonical memory\n---\nMissing embedding duplicate",
            Ok("Canonical memory with duplicate detail".to_string()),
        )]));
        let embeddings = Arc::new(StubEmbeddingProvider::new([(
            "Missing embedding duplicate",
            Ok(vec![0.97, 0.03]),
        )]));
        let actions = LlmConsolidator::new(ScopeConfig::default(), llm)
            .with_embedding_provider(embeddings)
            .consolidate(&[
                candidate(
                    sample_memory("Canonical memory", 0.9, MemoryState::Active),
                    Some(vec![1.0, 0.0]),
                ),
                candidate(
                    sample_memory("Missing embedding duplicate", 0.4, MemoryState::Active),
                    None,
                ),
            ])
            .await
            .expect("llm consolidate");

        assert_eq!(actions.len(), 1);
        let ConsolidationAction::Merged { result, .. } = &actions[0] else {
            panic!("expected merged action");
        };
        assert_eq!(result.content, "Canonical memory with duplicate detail");
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
                    .map(|(prompt, response)| (prompt.into(), response))
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
                .and_then(|tail| tail.split("\n\nConsolidated:").next())
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
            "stub-model"
        }
    }

    #[derive(Debug)]
    struct StubEmbeddingProvider {
        responses: HashMap<String, Result<Vec<f32>, EmbeddingError>>,
        calls: Mutex<Vec<String>>,
    }

    impl StubEmbeddingProvider {
        fn new<I, S>(responses: I) -> Self
        where
            I: IntoIterator<Item = (S, Result<Vec<f32>, EmbeddingError>)>,
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
    }

    #[async_trait]
    impl EmbeddingProvider for StubEmbeddingProvider {
        async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
            let trimmed = text.trim().to_string();
            self.calls
                .lock()
                .expect("stub embedding calls lock")
                .push(trimmed.clone());
            match self.responses.get(&trimmed) {
                Some(Ok(embedding)) => Ok(embedding.clone()),
                Some(Err(error)) => Err(EmbeddingError::Provider(error.to_string())),
                None => Err(EmbeddingError::Provider(format!(
                    "missing embedding for `{trimmed}`"
                ))),
            }
        }

        fn dimensions(&self) -> usize {
            2
        }

        fn model_id(&self) -> &str {
            "stub-embedding-provider"
        }
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
