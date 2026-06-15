//! Rust semantic augmentation via SCIP from rust-analyzer.
//!
//! Opt-in layer invoked when `--use-scip` is passed to `extract`.
//! Spawns `rust-analyzer scip` per workspace member, parses the SCIP
//! protobuf, and merges `calls` and `references` edges with confidence:exact.
//!
//! Graceful degradation: when rust-analyzer is not on PATH, emits a warning
//! but does not fail the extraction.

use crate::ir::Graph;

/// Augment a syntax-level graph with SCIP semantic edges.
pub fn augment(_graph: &mut Graph, _repo_path: &str) -> crate::error::Result<()> {
    unimplemented!("SCIP protobuf ingestion from rust-analyzer (wp-rust-scip)")
}
