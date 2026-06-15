//! Rust syntax extractor using tree-sitter and cargo metadata.
//!
//! Always-on layer providing syntax-level facts: modules, functions, structs,
//! enums, traits, impls, crate graph. Augmented by the SCIP layer when available.
//!
//! ## Known gaps (v0)
//! - `#[cfg]` branches other than `test` are flattened.
//! - Macros are recorded as entities but not expanded.
//! - `async fn` is treated as a regular function.

use crate::ir::Graph;

/// Extract a syntax-level graph from a Rust workspace at the given path.
pub fn extract(_repo_path: &str) -> crate::error::Result<Graph> {
    unimplemented!("Rust tree-sitter + cargo metadata extraction (wp-rust-tree-sitter)")
}
