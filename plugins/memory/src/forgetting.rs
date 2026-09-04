//! Pluggable eviction ranking policies for [`crate::storage::SqliteMemoryStore::enforce_budget`].
//!
//! Each policy implements [`ForgettingPolicy`] and is a pure function over an
//! already-loaded [`Memory`] plus a [`RetentionContext`] — no I/O. `enforce_budget`
//! sorts ascending by `retention_score` and evicts (demotes or deletes) from the
//! low end, so a *lower* score means "forgotten sooner."

use sha2::{Digest, Sha256};

use crate::decay::adaptive_retention;
use crate::traits::{ForgettingPolicy, RetentionContext};
use crate::types::Memory;

/// Reproduces today's eviction ranking exactly: `importance_score * reliability_score`.
///
/// This is the default policy — selecting it (or no policy at all) changes
/// nothing about `enforce_budget`'s observable behavior.
#[derive(Debug, Clone, Copy, Default)]
pub struct ImportanceReliability;

impl ForgettingPolicy for ImportanceReliability {
    fn retention_score(&self, memory: &Memory, _context: &RetentionContext<'_>) -> f64 {
        f64::from(memory.importance_score) * f64::from(memory.reliability_score)
    }

    fn name(&self) -> &'static str {
        "importance-reliability"
    }
}

/// First in, first out: evicts the oldest memories by `created_at` first.
#[derive(Debug, Clone, Copy, Default)]
pub struct Fifo;

impl ForgettingPolicy for Fifo {
    fn retention_score(&self, memory: &Memory, _context: &RetentionContext<'_>) -> f64 {
        memory.created_at.timestamp() as f64
    }

    fn name(&self) -> &'static str {
        "fifo"
    }
}

/// Least recently used: evicts by `last_accessed_at`, falling back to
/// `updated_at` for memories never accessed — the same reference-time
/// convention [`crate::decay`] uses.
#[derive(Debug, Clone, Copy, Default)]
pub struct Lru;

impl ForgettingPolicy for Lru {
    fn retention_score(&self, memory: &Memory, _context: &RetentionContext<'_>) -> f64 {
        let reference_time = memory.last_accessed_at.unwrap_or(memory.updated_at);
        reference_time.timestamp() as f64
    }

    fn name(&self) -> &'static str {
        "lru"
    }
}

/// MaRS-style priority decay: direct reuse of [`adaptive_retention`], which
/// combines memory type, store activity, importance, and access frequency
/// into a single decaying score.
#[derive(Debug, Clone, Copy, Default)]
pub struct PriorityDecay;

impl ForgettingPolicy for PriorityDecay {
    fn retention_score(&self, memory: &Memory, context: &RetentionContext<'_>) -> f64 {
        adaptive_retention(
            memory,
            context.now,
            context.scope_config,
            context.total_memories,
            context.recent_writes_30d,
        )
    }

    fn name(&self) -> &'static str {
        "priority-decay"
    }
}

/// Deterministic random eviction: scores by a stable hash of `memory.id`
/// rather than a random-number generator, so results are reproducible
/// across runs given the same candidate set.
#[derive(Debug, Clone, Copy, Default)]
pub struct RandomDrop;

impl ForgettingPolicy for RandomDrop {
    fn retention_score(&self, memory: &Memory, _context: &RetentionContext<'_>) -> f64 {
        let digest = Sha256::digest(memory.id.as_bytes());
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&digest[..8]);
        (u64::from_be_bytes(bytes) as f64) / (u64::MAX as f64)
    }

    fn name(&self) -> &'static str {
        "random-drop"
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use super::{Fifo, ImportanceReliability, Lru, PriorityDecay, RandomDrop};
    use crate::traits::{ForgettingPolicy, RetentionContext};
    use crate::types::{
        Memory, MemoryScope, MemoryState, MemoryType, ProvenanceLevel, ScopeConfig,
        SensitivityLevel,
    };

    fn sample_memory(created_at: chrono::DateTime<Utc>) -> Memory {
        Memory {
            id: Uuid::new_v4(),
            content: "sample memory".to_string(),
            summary: None,
            scope: MemoryScope::Workspace,
            memory_type: MemoryType::Observation,
            provenance: ProvenanceLevel::UserStated,
            importance_score: 0.5,
            reliability_score: 1.0,
            sensitivity: SensitivityLevel::Low,
            state: MemoryState::Active,
            tags: Vec::new(),
            status: None,
            custom_metadata: HashMap::new(),
            access_count: 0,
            corroboration_count: 0,
            embedding_stale: false,
            created_at,
            updated_at: created_at,
            last_accessed_at: None,
            tenant_id: None,
            user_id: None,
            agent_id: None,
        }
    }

    fn context(now: chrono::DateTime<Utc>, scope_config: &ScopeConfig) -> RetentionContext<'_> {
        RetentionContext {
            now,
            scope_config,
            total_memories: 10,
            recent_writes_30d: 2,
        }
    }

    #[test]
    fn importance_reliability_matches_the_product() {
        let now = Utc::now();
        let mut memory = sample_memory(now);
        memory.importance_score = 0.4;
        memory.reliability_score = 0.5;
        let scope_config = ScopeConfig::default();

        let score = ImportanceReliability.retention_score(&memory, &context(now, &scope_config));
        assert!((score - 0.2).abs() < 1e-6);
    }

    #[test]
    fn fifo_ranks_older_memories_lower() {
        let now = Utc::now();
        let scope_config = ScopeConfig::default();
        let older = sample_memory(now - Duration::days(10));
        let newer = sample_memory(now - Duration::days(1));

        let older_score = Fifo.retention_score(&older, &context(now, &scope_config));
        let newer_score = Fifo.retention_score(&newer, &context(now, &scope_config));
        assert!(older_score < newer_score);
    }

    #[test]
    fn lru_falls_back_to_updated_at_when_never_accessed() {
        let now = Utc::now();
        let scope_config = ScopeConfig::default();
        let mut memory = sample_memory(now - Duration::days(5));
        memory.updated_at = now - Duration::days(2);
        memory.last_accessed_at = None;

        let score = Lru.retention_score(&memory, &context(now, &scope_config));
        assert!((score - memory.updated_at.timestamp() as f64).abs() < 1e-9);
    }

    #[test]
    fn lru_prefers_last_accessed_at_over_updated_at() {
        let now = Utc::now();
        let scope_config = ScopeConfig::default();
        let mut memory = sample_memory(now - Duration::days(5));
        memory.updated_at = now - Duration::days(5);
        memory.last_accessed_at = Some(now - Duration::hours(1));

        let score = Lru.retention_score(&memory, &context(now, &scope_config));
        assert!((score - memory.last_accessed_at.unwrap().timestamp() as f64).abs() < 1e-9);
    }

    #[test]
    fn priority_decay_matches_adaptive_retention_directly() {
        let now = Utc::now();
        let scope_config = ScopeConfig {
            decay_lambda_base: 0.1,
            ..ScopeConfig::default()
        };
        let memory = sample_memory(now - Duration::days(3));

        let expected = crate::decay::adaptive_retention(&memory, now, &scope_config, 10, 2);
        let actual = PriorityDecay.retention_score(&memory, &context(now, &scope_config));
        assert!((actual - expected).abs() < 1e-12);
    }

    #[test]
    fn random_drop_is_deterministic_for_the_same_id() {
        let now = Utc::now();
        let scope_config = ScopeConfig::default();
        let memory = sample_memory(now);

        let first = RandomDrop.retention_score(&memory, &context(now, &scope_config));
        let second = RandomDrop.retention_score(&memory, &context(now, &scope_config));
        assert_eq!(first, second);
    }

    #[test]
    fn random_drop_differs_across_ids() {
        let now = Utc::now();
        let scope_config = ScopeConfig::default();
        let a = sample_memory(now);
        let b = sample_memory(now);

        let score_a = RandomDrop.retention_score(&a, &context(now, &scope_config));
        let score_b = RandomDrop.retention_score(&b, &context(now, &scope_config));
        assert_ne!(score_a, score_b);
    }

    #[test]
    fn every_policy_reports_a_stable_name() {
        assert_eq!(ImportanceReliability.name(), "importance-reliability");
        assert_eq!(Fifo.name(), "fifo");
        assert_eq!(Lru.name(), "lru");
        assert_eq!(PriorityDecay.name(), "priority-decay");
        assert_eq!(RandomDrop.name(), "random-drop");
    }
}
