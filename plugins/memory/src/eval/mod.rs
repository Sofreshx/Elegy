//! Eval harness v1 (`docs/specs/eval-harness-v1/spec.md`): regression-tests the
//! write→store→retrieve pipeline against an embedded, network-free corpus and
//! gates all nine metrics from the spec's Metrics table.
//!
//! Internal to the crate — the CLI's `eval` subcommand (`src/cli.rs`) is the only
//! consumer. `tests/eval.rs` drives this module indirectly, as a subprocess
//! against the built `elegy-memory` binary, matching the existing convention in
//! `tests/cli.rs` rather than exposing a second, test-only public API surface.

mod corpus;
mod embedding;
mod gates;
mod metrics;
mod runner;
mod synthetic;

// Consumed by the `eval` CLI subcommand in `src/cli.rs`.
#[allow(unused_imports)]
pub(crate) use runner::{
    list_corpora, run_eval, sweep_threshold, CorpusDescriptor, EvalError, EvalMetricResult,
    EvalReport, EvalRunOptions, SweepPoint, SweepReport,
};
