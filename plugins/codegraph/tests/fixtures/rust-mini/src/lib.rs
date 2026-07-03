//! A minimal Rust library for extractor testing.
//!
//! This crate exercises: public functions, private helpers, modules,
//! structs, impl blocks, re-exports, and doc comments.

mod helper;

/// Add two integers together.
///
/// # Examples
///
/// ```
/// let result = rust_mini::add(2, 3);
/// assert_eq!(result, 5);
/// ```
pub fn add(a: i32, b: i32) -> i32 {
    helper::validate_input(a);
    helper::validate_input(b);
    a + b
}

/// A simple counter with increment and value retrieval.
pub struct Counter {
    count: i32,
}

impl Counter {
    /// Create a new counter starting at zero.
    pub fn new() -> Self {
        Self { count: 0 }
    }

    /// Increment the counter by one.
    pub fn increment(&mut self) {
        self.count += 1;
    }

    /// Get the current count.
    pub fn value(&self) -> i32 {
        self.count
    }
}

// Re-export helper validation for convenience
pub use helper::validate_input;
