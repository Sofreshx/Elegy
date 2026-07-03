use crate::StoreError;

/// Compute cosine similarity for two non-empty vectors with equal dimensions.
pub(crate) fn cosine_similarity(left: &[f32], right: &[f32]) -> Result<f32, StoreError> {
    if left.len() != right.len() {
        return Err(StoreError::Validation(format!(
            "cosine similarity requires equal vector dimensions, got {} and {}",
            left.len(),
            right.len()
        )));
    }
    if left.is_empty() {
        return Err(StoreError::Validation(
            "cosine similarity requires non-empty vectors".to_string(),
        ));
    }

    let mut dot_product = 0.0_f32;
    let mut left_norm = 0.0_f32;
    let mut right_norm = 0.0_f32;
    for (left_component, right_component) in left.iter().zip(right.iter()) {
        dot_product += left_component * right_component;
        left_norm += left_component * left_component;
        right_norm += right_component * right_component;
    }

    if left_norm <= f32::EPSILON || right_norm <= f32::EPSILON {
        return Ok(0.0);
    }

    Ok((dot_product / (left_norm.sqrt() * right_norm.sqrt())).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::cosine_similarity;

    #[test]
    fn returns_expected_cosine_for_matching_dimensions() {
        let similarity = cosine_similarity(&[1.0, 0.0], &[0.8, 0.6]).expect("compute similarity");
        assert!((similarity - 0.8).abs() < 1.0e-6);
    }

    #[test]
    fn zero_norm_vectors_return_zero_similarity() {
        let similarity = cosine_similarity(&[0.0, 0.0], &[1.0, 0.0]).expect("compute similarity");
        assert_eq!(similarity, 0.0);
    }
}
