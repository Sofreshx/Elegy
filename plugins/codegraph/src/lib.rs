//! Elegy Codegraph — portable codebase graph extraction and query.
//!
//! Provides `extract` (build a normalized graph IR from TypeScript or Rust source)
//! and `query` (symbol lookup, neighbors, impact analysis, structural summary).
//! The graph is stored in a local [redb] database.

pub mod error;
pub mod extractor;
pub mod ir;
pub mod query;
pub mod store;
