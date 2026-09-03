//! Pure metric computations for the eval harness. No I/O, no store access —
//! every function here takes already-retrieved data and returns a number.
//! Each of the nine metrics in `docs/specs/eval-harness-v1/spec.md` has a
//! corresponding function; `runner.rs` is responsible for wiring a gate to
//! every one of them (no metric is computed without a gate).

use std::collections::{HashMap, HashSet};

use crate::MemoryId;

/// Fraction of `relevant` that appear anywhere in `retrieved` (already truncated to k).
pub(crate) fn recall_at_k(retrieved: &[MemoryId], relevant: &HashSet<MemoryId>) -> f64 {
    if relevant.is_empty() {
        return 1.0;
    }
    let retrieved_set: HashSet<MemoryId> = retrieved.iter().copied().collect();
    let hits = relevant.intersection(&retrieved_set).count();
    hits as f64 / relevant.len() as f64
}

/// Fraction of `retrieved` (already truncated to k) that are in `relevant`.
pub(crate) fn precision_at_k(retrieved: &[MemoryId], relevant: &HashSet<MemoryId>) -> f64 {
    if retrieved.is_empty() {
        return 1.0;
    }
    let hits = retrieved
        .iter()
        .filter(|memory_id| relevant.contains(memory_id))
        .count();
    hits as f64 / retrieved.len() as f64
}

/// Normalized discounted cumulative gain over `retrieved` (already truncated to k),
/// using `grades` for relevance (0 for any id not present). `k` bounds the ideal
/// ranking used to compute IDCG.
pub(crate) fn ndcg_at_k(retrieved: &[MemoryId], grades: &HashMap<MemoryId, u8>, k: usize) -> f64 {
    let retrieved_dcg = dcg(retrieved
        .iter()
        .map(|id| f64::from(grades.get(id).copied().unwrap_or(0))));

    let mut ideal_grades: Vec<u8> = grades.values().copied().collect();
    ideal_grades.sort_unstable_by(|left, right| right.cmp(left));
    ideal_grades.truncate(k);
    let ideal_dcg = dcg(ideal_grades.into_iter().map(f64::from));

    if ideal_dcg <= 0.0 {
        return if retrieved_dcg <= 0.0 { 1.0 } else { 0.0 };
    }
    (retrieved_dcg / ideal_dcg).min(1.0)
}

fn dcg(grades: impl Iterator<Item = f64>) -> f64 {
    grades
        .enumerate()
        .map(|(rank, grade)| {
            let discount = ((rank as f64) + 2.0).log2();
            grade / discount
        })
        .sum()
}

/// Fraction of retrieved memories, across all queries, that were a hallucinated
/// (contradictory) retrieval. `retrieved_per_query[i]` pairs with
/// `contradicting_per_query[i]`.
pub(crate) fn hallucination_rate(
    retrieved_per_query: &[Vec<MemoryId>],
    contradicting_per_query: &[HashSet<MemoryId>],
) -> f64 {
    let mut total_retrieved: u64 = 0;
    let mut total_hallucinated: u64 = 0;
    for (retrieved, contradicting) in retrieved_per_query.iter().zip(contradicting_per_query) {
        total_retrieved += retrieved.len() as u64;
        total_hallucinated += retrieved
            .iter()
            .filter(|memory_id| contradicting.contains(memory_id))
            .count() as u64;
    }
    ratio(total_hallucinated, total_retrieved)
}

/// Fraction of `correct` entries that are `true`. Used for write-time gate accuracy:
/// callers determine per-case correctness (expected outcome vs. actual `GateDecision`
/// discriminant) and pass the resulting booleans here.
pub(crate) fn accuracy(correct: &[bool]) -> f64 {
    if correct.is_empty() {
        return 1.0;
    }
    let hits = correct.iter().filter(|value| **value).count();
    hits as f64 / correct.len() as f64
}

/// Returns the `pct` percentile (0.0..=100.0) of `samples_ms` using nearest-rank
/// interpolation. Does not mutate the caller's slice.
pub(crate) fn percentile_ms(samples_ms: &[f64], pct: f64) -> f64 {
    if samples_ms.is_empty() {
        return 0.0;
    }
    let mut sorted = samples_ms.to_vec();
    sorted.sort_by(f64::total_cmp);
    let clamped_pct = pct.clamp(0.0, 100.0);
    let rank = ((clamped_pct / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

/// `numerator / denominator`, or `0.0` when `denominator` is zero.
pub(crate) fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        return 0.0;
    }
    numerator as f64 / denominator as f64
}

/// Average of `values`, or `0.0` for an empty slice.
pub(crate) fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn ids(n: usize) -> Vec<MemoryId> {
        (0..n).map(|_| Uuid::new_v4()).collect()
    }

    #[test]
    fn recall_and_precision_perfect_when_retrieved_equals_relevant() {
        let items = ids(3);
        let relevant: HashSet<MemoryId> = items.iter().copied().collect();
        assert_eq!(recall_at_k(&items, &relevant), 1.0);
        assert_eq!(precision_at_k(&items, &relevant), 1.0);
    }

    #[test]
    fn recall_counts_hits_out_of_all_relevant_even_if_not_all_retrieved() {
        let items = ids(4);
        let relevant: HashSet<MemoryId> = items.iter().copied().collect();
        let retrieved = vec![items[0], items[1]];
        assert_eq!(recall_at_k(&retrieved, &relevant), 0.5);
        assert_eq!(precision_at_k(&retrieved, &relevant), 1.0);
    }

    #[test]
    fn precision_penalizes_irrelevant_hits() {
        let relevant_items = ids(1);
        let noise = ids(3);
        let relevant: HashSet<MemoryId> = relevant_items.iter().copied().collect();
        let mut retrieved = relevant_items.clone();
        retrieved.extend(noise);
        assert_eq!(precision_at_k(&retrieved, &relevant), 0.25);
    }

    #[test]
    fn ndcg_is_one_for_a_perfectly_ranked_result() {
        let items = ids(3);
        let mut grades = HashMap::new();
        grades.insert(items[0], 3);
        grades.insert(items[1], 2);
        grades.insert(items[2], 1);
        assert!((ndcg_at_k(&items, &grades, 3) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn ndcg_penalizes_a_reversed_ranking() {
        let items = ids(3);
        let mut grades = HashMap::new();
        grades.insert(items[0], 1);
        grades.insert(items[1], 2);
        grades.insert(items[2], 3);
        let ndcg = ndcg_at_k(&items, &grades, 3);
        assert!(ndcg < 1.0);
        assert!(ndcg > 0.0);
    }

    #[test]
    fn ndcg_with_no_relevant_items_and_nothing_retrieved_is_one() {
        let grades: HashMap<MemoryId, u8> = HashMap::new();
        assert_eq!(ndcg_at_k(&[], &grades, 10), 1.0);
    }

    #[test]
    fn hallucination_rate_counts_contradicting_hits_across_queries() {
        let items = ids(4);
        let retrieved_per_query = vec![vec![items[0], items[1]], vec![items[2]]];
        let mut contradicting_a = HashSet::new();
        contradicting_a.insert(items[1]);
        let contradicting_per_query = vec![contradicting_a, HashSet::new()];
        // 1 hallucinated out of 3 total retrieved.
        assert!(
            (hallucination_rate(&retrieved_per_query, &contradicting_per_query) - (1.0 / 3.0))
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn accuracy_counts_true_fraction() {
        assert_eq!(accuracy(&[true, true, false, true]), 0.75);
        assert_eq!(accuracy(&[]), 1.0);
    }

    #[test]
    fn percentile_ms_matches_known_values_without_mutating_input() {
        let samples = vec![100.0, 200.0, 300.0, 400.0, 500.0];
        assert_eq!(percentile_ms(&samples, 0.0), 100.0);
        assert_eq!(percentile_ms(&samples, 100.0), 500.0);
        assert_eq!(percentile_ms(&samples, 50.0), 300.0);
        assert_eq!(samples[0], 100.0, "input slice must not be reordered");
    }

    #[test]
    fn percentile_ms_of_empty_slice_is_zero() {
        assert_eq!(percentile_ms(&[], 95.0), 0.0);
    }

    #[test]
    fn ratio_handles_zero_denominator() {
        assert_eq!(ratio(0, 0), 0.0);
        assert_eq!(ratio(5, 0), 0.0);
        assert_eq!(ratio(1, 4), 0.25);
    }
}
