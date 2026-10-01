//! Eval harness v1 (`docs/specs/eval-harness-v1/spec.md`): regression-tests the
//! write→store→retrieve pipeline against an embedded, network-free corpus and
//! gates every metric in the spec's Metrics table.
//!
//! Internal to the crate — the CLI's `eval` subcommand (`src/cli.rs`) is the only
//! consumer. `tests/eval.rs` drives this module indirectly, as a subprocess
//! against the built `elegy-memory` binary, matching the existing convention in
//! `tests/cli.rs` rather than exposing a second, test-only public API surface.

mod corpus;
mod embedding;
mod fingerprint;
mod gates;
mod metrics;
pub(crate) mod qualification;
mod runner;
mod scenarios;
mod synthetic;

// Consumed by the `eval` CLI subcommand in `src/cli.rs`.
#[allow(unused_imports)]
pub(crate) use runner::{
    list_corpora, run_eval, sweep_threshold, CorpusDescriptor, EvalError, EvalMetricResult,
    EvalReport, EvalRunOptions, SweepPoint, SweepReport,
};
