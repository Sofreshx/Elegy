use chrono::{DateTime, Utc};

use crate::{Memory, MemoryType, ScopeConfig};

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

/// Returns the type-modulated decay multiplier for a given [`MemoryType`].
///
/// The multiplier is applied to the base decay lambda.  Lower values mean
/// slower decay (longer retention), higher values mean faster decay:
///
/// | Type          | Multiplier | Rationale                          |
/// |---------------|------------|------------------------------------|
/// | `Procedure`   | 0.70       | Long-lived procedural knowledge    |
/// | `Fact`        | 0.80       | Stable factual information         |
/// | `Decision`    | 0.85       | Intentional choices, fairly stable |
/// | `Preference`  | 0.90       | Preferences may evolve over time   |
/// | `Observation` | 1.20       | Transient observations decay fast  |
#[must_use]
pub fn type_decay_multiplier(memory_type: MemoryType) -> f64 {
    match memory_type {
        MemoryType::Procedure => 0.7,
        MemoryType::Fact => 0.8,
        MemoryType::Decision => 0.85,
        MemoryType::Preference => 0.9,
        MemoryType::Observation => 1.2,
    }
}

/// Compute adaptive retention that accounts for memory type, store activity,
/// importance, and access frequency.
///
/// The effective lambda is computed as:
///
/// ```text
/// effective_lambda = decay_lambda_base × type_multiplier × (1.0 + 0.5 × activity_rate)
/// ```
///
/// where `activity_rate = recent_writes_30d / max(total_memories, 1)`.
///
/// High store activity increases lambda (faster decay — more memories compete
/// for the budget), while low activity decreases it (fewer memories, keep them
/// longer).
///
/// The final retention score also incorporates importance and access count:
///
/// ```text
/// retention = importance × e^(-effective_lambda × days) × (1 + 0.2 × access_count)
/// ```
#[must_use]
pub fn adaptive_retention(
    memory: &Memory,
    now: DateTime<Utc>,
    scope_config: &ScopeConfig,
    total_memories: u64,
    recent_writes_30d: u64,
) -> f64 {
    let reference_time = memory.last_accessed_at.unwrap_or(memory.updated_at);
    let elapsed_seconds = (now - reference_time).num_seconds().max(0) as f64;
    let elapsed_days = elapsed_seconds / 86_400.0;

    let type_mult = type_decay_multiplier(memory.memory_type);

    let activity_rate = recent_writes_30d as f64 / total_memories.max(1) as f64;
    let effective_lambda =
        f64::from(scope_config.decay_lambda_base.max(0.0)) * type_mult * (1.0 + 0.5 * activity_rate);

    let decay = (-effective_lambda * elapsed_days).exp();
    let importance = f64::from(memory.importance_score);
    let access_boost = 1.0 + 0.2 * f64::from(memory.access_count);

    importance * decay * access_boost
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use super::{adaptive_retention, retention, retention_with_lambda, type_decay_multiplier};
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

    // ── Type-modulated decay multiplier tests ──────────────────────────

    #[test]
    fn type_multiplier_procedure_is_slowest() {
        assert!((type_decay_multiplier(MemoryType::Procedure) - 0.7).abs() < 1e-12);
    }

    #[test]
    fn type_multiplier_fact() {
        assert!((type_decay_multiplier(MemoryType::Fact) - 0.8).abs() < 1e-12);
    }

    #[test]
    fn type_multiplier_decision() {
        assert!((type_decay_multiplier(MemoryType::Decision) - 0.85).abs() < 1e-12);
    }

    #[test]
    fn type_multiplier_preference() {
        assert!((type_decay_multiplier(MemoryType::Preference) - 0.9).abs() < 1e-12);
    }

    #[test]
    fn type_multiplier_observation_is_fastest() {
        assert!((type_decay_multiplier(MemoryType::Observation) - 1.2).abs() < 1e-12);
    }

    #[test]
    fn type_multiplier_ordering() {
        let proc = type_decay_multiplier(MemoryType::Procedure);
        let fact = type_decay_multiplier(MemoryType::Fact);
        let decision = type_decay_multiplier(MemoryType::Decision);
        let pref = type_decay_multiplier(MemoryType::Preference);
        let obs = type_decay_multiplier(MemoryType::Observation);

        assert!(proc < fact);
        assert!(fact < decision);
        assert!(decision < pref);
        assert!(pref < obs);
    }

    // ── Adaptive retention tests ───────────────────────────────────────

    #[test]
    fn adaptive_retention_lower_for_high_activity() {
        let now = Utc::now();
        let memory = sample_memory(now - Duration::days(3));
        let scope_config = ScopeConfig {
            decay_lambda_base: 0.1,
            ..ScopeConfig::default()
        };

        let low_activity = adaptive_retention(&memory, now, &scope_config, 100, 5);
        let high_activity = adaptive_retention(&memory, now, &scope_config, 100, 80);

        assert!(
            high_activity < low_activity,
            "high activity ({high_activity}) should yield lower retention than low activity ({low_activity})"
        );
    }

    #[test]
    fn adaptive_retention_accounts_for_type_differences() {
        let now = Utc::now();
        let scope_config = ScopeConfig {
            decay_lambda_base: 0.1,
            ..ScopeConfig::default()
        };

        let mut procedure_mem = sample_memory(now - Duration::days(5));
        procedure_mem.memory_type = MemoryType::Procedure;

        let observation_mem = sample_memory(now - Duration::days(5));
        // observation_mem defaults to MemoryType::Observation via sample_memory

        let proc_ret = adaptive_retention(&procedure_mem, now, &scope_config, 50, 10);
        let obs_ret = adaptive_retention(&observation_mem, now, &scope_config, 50, 10);

        assert!(
            proc_ret > obs_ret,
            "procedures ({proc_ret}) should retain longer than observations ({obs_ret})"
        );
    }

    #[test]
    fn adaptive_retention_access_count_boost() {
        let now = Utc::now();
        let scope_config = ScopeConfig {
            decay_lambda_base: 0.1,
            ..ScopeConfig::default()
        };

        let mut no_access = sample_memory(now - Duration::days(2));
        no_access.access_count = 0;

        let mut many_access = sample_memory(now - Duration::days(2));
        many_access.access_count = 10;

        let ret_none = adaptive_retention(&no_access, now, &scope_config, 50, 10);
        let ret_many = adaptive_retention(&many_access, now, &scope_config, 50, 10);

        // With 10 accesses the boost is (1 + 0.2 * 10) = 3.0
        assert!(
            ret_many > ret_none,
            "high access_count ({ret_many}) should boost retention above zero access ({ret_none})"
        );

        // Verify the ratio matches the expected access boost formula
        let expected_ratio = (1.0 + 0.2 * 10.0) / 1.0;
        let actual_ratio = ret_many / ret_none;
        assert!(
            (actual_ratio - expected_ratio).abs() < 1e-9,
            "access boost ratio should be {expected_ratio}, got {actual_ratio}"
        );
    }

    #[test]
    fn adaptive_retention_zero_total_memories() {
        let now = Utc::now();
        let memory = sample_memory(now - Duration::days(1));
        let scope_config = ScopeConfig {
            decay_lambda_base: 0.1,
            ..ScopeConfig::default()
        };

        // total_memories = 0 should not panic; max(0,1) = 1 is used
        let ret = adaptive_retention(&memory, now, &scope_config, 0, 5);
        assert!(ret > 0.0, "retention should be positive even with zero total memories");
        assert!(ret.is_finite(), "retention should be finite");
    }

    #[test]
    fn adaptive_retention_zero_recent_writes() {
        let now = Utc::now();
        let memory = sample_memory(now - Duration::days(1));
        let scope_config = ScopeConfig {
            decay_lambda_base: 0.1,
            ..ScopeConfig::default()
        };

        // recent_writes_30d = 0 → activity_rate = 0 → scaling factor = 1.0
        let ret = adaptive_retention(&memory, now, &scope_config, 100, 0);

        // With zero activity the effective lambda = base × type_mult × 1.0
        // For Observation: 0.1 × 1.2 × 1.0 = 0.12, days = 1
        // decay = e^(-0.12) ≈ 0.8869
        // importance = 0.8, access_count = 0 → boost = 1.0
        // expected ≈ 0.8 × 0.8869 × 1.0 ≈ 0.7095
        let expected = 0.8 * (-0.12_f64).exp();
        assert!(
            (ret - expected).abs() < 1e-6,
            "expected {expected}, got {ret}"
        );
    }

    #[test]
    fn adaptive_retention_importance_scaling() {
        let now = Utc::now();
        let scope_config = ScopeConfig {
            decay_lambda_base: 0.1,
            ..ScopeConfig::default()
        };

        let mut low_importance = sample_memory(now - Duration::days(2));
        low_importance.importance_score = 0.2;

        let mut high_importance = sample_memory(now - Duration::days(2));
        high_importance.importance_score = 1.0;

        let ret_low = adaptive_retention(&low_importance, now, &scope_config, 50, 10);
        let ret_high = adaptive_retention(&high_importance, now, &scope_config, 50, 10);

        assert!(
            ret_high > ret_low,
            "higher importance ({ret_high}) should yield greater retention than lower ({ret_low})"
        );

        // The ratio should match importance_high / importance_low = 5.0
        let expected_ratio = f64::from(1.0_f32) / f64::from(0.2_f32);
        let actual_ratio = ret_high / ret_low;
        assert!(
            (actual_ratio - expected_ratio).abs() < 1e-6,
            "importance ratio should be {expected_ratio}, got {actual_ratio}"
        );
    }

    #[test]
    fn adaptive_retention_fresh_memory_returns_importance() {
        let now = Utc::now();
        let memory = sample_memory(now); // zero elapsed time
        let scope_config = ScopeConfig {
            decay_lambda_base: 0.1,
            ..ScopeConfig::default()
        };

        let ret = adaptive_retention(&memory, now, &scope_config, 50, 10);

        // With 0 elapsed days: decay = 1.0, access_count = 0 → boost = 1.0
        // expected = importance × 1.0 × 1.0 = 0.8
        assert!(
            (ret - f64::from(0.8_f32)).abs() < 1e-9,
            "fresh memory adaptive retention should equal importance, got {ret}"
        );
    }
}
