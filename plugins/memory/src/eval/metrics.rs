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

/// Fraction of contradiction-bearing queries where a contradicting memory
/// outranks the correct one — or where the correct memory is missing from
/// the results entirely while a contradicting memory is present.
///
/// This is rank-sensitive by design: a contradicting memory merely
/// *co-occurring* in top-k alongside the correct answer is not counted,
/// since a correctly-functioning system still surfaces the right answer
/// first. Only queries that declare at least one `contradicts` key are
/// counted in the denominator — a query with no known contradiction has
/// nothing to hallucinate, so it cannot dilute or inflate the rate.
///
/// `retrieved_per_query[i]`, `relevant_per_query[i]`, and
/// `contradicting_per_query[i]` all describe the same query `i`, in the
/// order results were actually returned (rank = index).
pub(crate) fn hallucination_rate(
    retrieved_per_query: &[Vec<MemoryId>],
    relevant_per_query: &[HashSet<MemoryId>],
    contradicting_per_query: &[HashSet<MemoryId>],
) -> f64 {
    let mut applicable: u64 = 0;
    let mut hallucinated: u64 = 0;

    for ((retrieved, relevant), contradicting) in retrieved_per_query
        .iter()
        .zip(relevant_per_query)
        .zip(contradicting_per_query)
    {
        if contradicting.is_empty() {
            continue;
        }
        applicable += 1;

        let Some(contradicting_rank) = retrieved.iter().position(|id| contradicting.contains(id))
        else {
            continue; // no contradicting memory surfaced at all: not a hallucination
        };
        let relevant_rank = retrieved.iter().position(|id| relevant.contains(id));
        let outranked = match relevant_rank {
            Some(relevant_rank) => contradicting_rank < relevant_rank,
            None => true, // correct answer missing entirely while a contradiction is present
        };
        if outranked {
            hallucinated += 1;
        }
    }

    ratio(hallucinated, applicable)
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
    fn hallucination_rate_is_zero_when_the_correct_answer_outranks_the_contradiction() {
        let items = ids(4);
        // Query 0: correct (items[0]) ranks ahead of the contradiction (items[1]).
        let retrieved_per_query = vec![vec![items[0], items[1]]];
        let relevant_per_query = vec![HashSet::from([items[0]])];
        let contradicting_per_query = vec![HashSet::from([items[1]])];
        assert_eq!(
            hallucination_rate(
                &retrieved_per_query,
                &relevant_per_query,
                &contradicting_per_query
            ),
            0.0,
            "co-occurrence alone must not count when the correct answer still ranks first"
        );
    }

    #[test]
    fn hallucination_rate_is_one_when_the_contradiction_outranks_the_correct_answer() {
        let items = ids(4);
        let retrieved_per_query = vec![vec![items[1], items[0]]];
        let relevant_per_query = vec![HashSet::from([items[0]])];
        let contradicting_per_query = vec![HashSet::from([items[1]])];
        assert_eq!(
            hallucination_rate(
                &retrieved_per_query,
                &relevant_per_query,
                &contradicting_per_query
            ),
            1.0
        );
    }

    #[test]
    fn hallucination_rate_counts_a_missing_correct_answer_as_hallucinated() {
        let items = ids(4);
        // items[0] (correct) is never retrieved; items[1] (contradicting) is.
        let retrieved_per_query = vec![vec![items[1], items[2]]];
        let relevant_per_query = vec![HashSet::from([items[0]])];
        let contradicting_per_query = vec![HashSet::from([items[1]])];
        assert_eq!(
            hallucination_rate(
                &retrieved_per_query,
                &relevant_per_query,
                &contradicting_per_query
            ),
            1.0
        );
    }

    #[test]
    fn hallucination_rate_excludes_queries_with_no_declared_contradiction() {
        let items = ids(2);
        let retrieved_per_query = vec![vec![items[0]], vec![items[1]]];
        let relevant_per_query = vec![HashSet::from([items[0]]), HashSet::new()];
        // Only query 0 declares a contradiction; query 1 has none and must not
        // appear in the denominator at all.
        let contradicting_per_query = vec![HashSet::new(), HashSet::new()];
        assert_eq!(
            hallucination_rate(
                &retrieved_per_query,
                &relevant_per_query,
                &contradicting_per_query
            ),
            0.0,
            "denominator with zero applicable queries defaults to a perfect (zero) rate"
        );
    }

    #[test]
    fn hallucination_rate_is_zero_when_the_contradiction_never_surfaces() {
        let items = ids(3);
        let retrieved_per_query = vec![vec![items[0]]];
        let relevant_per_query = vec![HashSet::from([items[0]])];
        // items[1] is a known contradiction for this query but never retrieved.
        let contradicting_per_query = vec![HashSet::from([items[1]])];
        assert_eq!(
            hallucination_rate(
                &retrieved_per_query,
                &relevant_per_query,
                &contradicting_per_query
            ),
            0.0
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
