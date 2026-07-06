//! Internal helper module.

/// Validate that the input is non-negative.
pub fn validate_input(n: i32) {
    if n < 0 {
        panic!("Input must be non-negative, got {}", n);
    }
}
