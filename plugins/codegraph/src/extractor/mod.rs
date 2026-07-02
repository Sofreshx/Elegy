//! Language-specific extractors.
//!
//! Each extractor produces a normalized [Graph](crate::ir::Graph) from source code.

pub mod rust_lang;
pub mod rust_scip;
pub mod ts;
pub mod ts_script;
