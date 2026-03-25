use chrono::{DateTime, Utc};

use crate::{Memory, ScopeConfig};

/// Compute MVP recency retention using the scope-configured fixed lambda.
#[must_use]
pub fn retention(memory: &Memory, now: DateTime<Utc>, scope_config: &ScopeConfig) -> f64 {
    retention_with_lambda(memory, now, scope_config.decay_lambda_base)
}

/// Compute MVP recency retention using an explicit fixed lambda.
#[must_use]
pub fn retention_with_lambda(memory: &Memory, now: DateTime<Utc>, decay_lambda_base: f32) -> f64 {
    let reference_time = memory.last_accessed_at.unwrap_or(memory.updated_at);
    let elapsed_seconds = (now - reference_time).num_seconds().max(0) as f64;
    let elapsed_days = elapsed_seconds / 86_400.0;

    (-f64::from(decay_lambda_base.max(0.0)) * elapsed_days).exp()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use super::{retention, retention_with_lambda};
    use crate::{
        Memory, MemoryScope, MemoryState, MemoryType, ProvenanceLevel, ScopeConfig,
        SensitivityLevel,
    };

    #[test]
    fn fixed_lambda_matches_expected_exponential_decay() {
        let now = Utc::now();
        let memory = sample_memory(now - Duration::hours(12));

        let actual = retention_with_lambda(&memory, now, 1.0);
        let expected = (-0.5_f64).exp();

        assert!((actual - expected).abs() < 1e-9);
    }

    #[test]
    fn retention_decreases_as_time_since_access_increases() {
        let now = Utc::now();
        let recent = sample_memory(now - Duration::hours(6));
        let older = sample_memory(now - Duration::days(5));

        let recent_retention = retention_with_lambda(&recent, now, 0.1);
        let older_retention = retention_with_lambda(&older, now, 0.1);

        assert!(recent_retention > older_retention);
    }

    #[test]
    fn access_count_does_not_change_mvp_retention() {
        let now = Utc::now();
        let mut low_access = sample_memory(now - Duration::days(2));
        let mut high_access = low_access.clone();
        low_access.access_count = 1;
        high_access.access_count = 100;

        let scope_config = ScopeConfig {
            decay_lambda_base: 0.25,
            ..ScopeConfig::default()
        };

        let low_access_retention = retention(&low_access, now, &scope_config);
        let high_access_retention = retention(&high_access, now, &scope_config);

        assert!((low_access_retention - high_access_retention).abs() < 1e-12);
    }

    fn sample_memory(reference_time: chrono::DateTime<Utc>) -> Memory {
        Memory {
            id: Uuid::new_v4(),
            content: "sample memory".to_string(),
            summary: None,
            scope: MemoryScope::Workspace,
            memory_type: MemoryType::Observation,
            provenance: ProvenanceLevel::UserStated,
            importance_score: 0.8,
            reliability_score: 1.0,
            sensitivity: SensitivityLevel::Low,
            state: MemoryState::Active,
            tags: Vec::new(),
            status: None,
            custom_metadata: HashMap::new(),
            access_count: 0,
            corroboration_count: 0,
            embedding_stale: false,
            created_at: reference_time,
            updated_at: reference_time,
            last_accessed_at: Some(reference_time),
            tenant_id: None,
            user_id: None,
            agent_id: None,
        }
    }
}
