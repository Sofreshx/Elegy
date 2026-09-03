//! Deterministic, network-free embedding vectors for the eval harness.
//!
//! Every corpus entity gets a unique "home" axis. A vector at an exact
//! cosine similarity to a home axis is expressed as a two-axis unit vector,
//! generalizing the `axis_embedding`/`cosine_embedding` pattern already used
//! by `tests/cli.rs`. No `EmbeddingProvider`, no network, no randomness.

/// Must match `storage::schema::EMBEDDING_DIMENSIONS`. Not imported directly: that
/// constant is private to `schema.rs`, and this crate's storage schema requires
/// explicit human confirmation to change (`.claude/rules/storage.md`), so the eval
/// harness keeps its own copy rather than depending on schema internals.
pub(crate) const EVAL_EMBEDDING_DIMENSIONS: usize = 768;

/// Returns a unit vector with `1.0` at `axis` and `0.0` elsewhere.
///
/// # Panics
/// Panics if `axis >= EVAL_EMBEDDING_DIMENSIONS`.
pub(crate) fn axis_vector(axis: usize) -> Vec<f32> {
    assert!(
        axis < EVAL_EMBEDDING_DIMENSIONS,
        "axis {axis} out of range for {EVAL_EMBEDDING_DIMENSIONS}-dimensional eval embeddings"
    );
    let mut vector = vec![0.0_f32; EVAL_EMBEDDING_DIMENSIONS];
    vector[axis] = 1.0;
    vector
}

/// Returns a unit vector with cosine similarity `target_cosine` to `axis_vector(axis)`,
/// using `companion_axis` to carry the orthogonal remainder.
///
/// `target_cosine` is clamped to `-1.0..=1.0`. `axis` and `companion_axis` must differ.
///
/// # Panics
/// Panics if `axis == companion_axis` or either is out of range.
pub(crate) fn vector_at_similarity(
    axis: usize,
    companion_axis: usize,
    target_cosine: f32,
) -> Vec<f32> {
    assert!(
        axis != companion_axis,
        "axis and companion_axis must differ"
    );
    assert!(
        axis < EVAL_EMBEDDING_DIMENSIONS && companion_axis < EVAL_EMBEDDING_DIMENSIONS,
        "axis indices out of range for {EVAL_EMBEDDING_DIMENSIONS}-dimensional eval embeddings"
    );
    let cosine = target_cosine.clamp(-1.0, 1.0);
    let mut vector = vec![0.0_f32; EVAL_EMBEDDING_DIMENSIONS];
    vector[axis] = cosine;
    vector[companion_axis] = (1.0_f32 - cosine * cosine).sqrt();
    vector
}

/// Returns the L2-normalized sum of the home-axis vectors for `axes`.
///
/// Used to build a query embedding that is deliberately similar to several
/// target memories at once. Empty `axes` returns a zero vector (callers must
/// not feed this to a similarity search, which would be meaningless).
pub(crate) fn blended_vector(axes: &[usize]) -> Vec<f32> {
    let mut vector = vec![0.0_f32; EVAL_EMBEDDING_DIMENSIONS];
    for &axis in axes {
        assert!(
            axis < EVAL_EMBEDDING_DIMENSIONS,
            "axis {axis} out of range for {EVAL_EMBEDDING_DIMENSIONS}-dimensional eval embeddings"
        );
        vector[axis] += 1.0;
    }
    let norm = vector
        .iter()
        .map(|component| component * component)
        .sum::<f32>()
        .sqrt();
    if norm > f32::EPSILON {
        for component in &mut vector {
            *component /= norm;
        }
    }
    vector
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cosine(left: &[f32], right: &[f32]) -> f32 {
        let dot: f32 = left.iter().zip(right).map(|(l, r)| l * r).sum();
        let left_norm = left.iter().map(|v| v * v).sum::<f32>().sqrt();
        let right_norm = right.iter().map(|v| v * v).sum::<f32>().sqrt();
        dot / (left_norm * right_norm)
    }

    #[test]
    fn axis_vectors_are_orthogonal_unit_vectors() {
        let a = axis_vector(0);
        let b = axis_vector(1);
        assert!((cosine(&a, &a) - 1.0).abs() < 1e-6);
        assert!(cosine(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn vector_at_similarity_hits_the_target_cosine_exactly() {
        let target_axis_vector = axis_vector(5);
        for target in [0.0_f32, 0.25, 0.5, 0.85, 0.99, 1.0] {
            let produced = vector_at_similarity(5, 6, target);
            let achieved = cosine(&target_axis_vector, &produced);
            assert!(
                (achieved - target).abs() < 1e-5,
                "target={target} achieved={achieved}"
            );
            let norm = produced.iter().map(|v| v * v).sum::<f32>().sqrt();
            assert!(
                (norm - 1.0).abs() < 1e-5,
                "produced vector must be unit length"
            );
        }
    }

    #[test]
    fn vector_at_similarity_clamps_out_of_range_targets() {
        let target_axis_vector = axis_vector(0);
        let produced = vector_at_similarity(0, 1, 2.0);
        assert!((cosine(&target_axis_vector, &produced) - 1.0).abs() < 1e-5);
        let produced = vector_at_similarity(0, 1, -2.0);
        assert!((cosine(&target_axis_vector, &produced) - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn blended_vector_is_more_similar_to_its_components_than_to_an_outsider() {
        let blended = blended_vector(&[0, 1, 2]);
        let outsider = axis_vector(9);
        let component = axis_vector(0);
        assert!(cosine(&blended, &component) > cosine(&blended, &outsider));
        let norm = blended.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn blended_vector_of_empty_axes_is_the_zero_vector() {
        let blended = blended_vector(&[]);
        assert!(blended.iter().all(|component| *component == 0.0));
    }
}
