//! Gate thresholds for the eval harness. `eval-harness-v1.json` is the single
//! source shared by `eval run --ci` and `eval sweep-threshold`
//! (spec: "once implemented, they move to `eval-harness-v1.json` ... so CI and
//! `eval sweep-threshold` share one source").

use std::{collections::BTreeMap, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum EvalGateError {
    #[error("failed to read threshold file {0}: {1}")]
    Io(String, String),
    #[error("failed to parse threshold file {0}: {1}")]
    Parse(String, String),
    #[error("threshold file is missing a required metric gate: {0}")]
    MissingMetric(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Comparison {
    Gte,
    Lte,
}

impl Comparison {
    fn passes(self, value: f64, threshold: f64) -> bool {
        match self {
            Comparison::Gte => value >= threshold,
            Comparison::Lte => value <= threshold,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ThresholdSpec {
    pub comparison: Comparison,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvalGateThresholds {
    pub version: String,
    pub thresholds: BTreeMap<String, ThresholdSpec>,
}

/// Every metric name the runner computes. `require_all` fails closed if any of
/// these is missing from a threshold file, satisfying the spec's Acceptance
/// Criteria: "no metric is computed without a gate."
pub(crate) const REQUIRED_METRIC_NAMES: &[&str] = &[
    "recall_at_10",
    "precision_at_10",
    "ndcg_at_10",
    "hallucination_rate",
    "gate_accuracy_8to1",
    "write_latency_p50_ms",
    "write_latency_p95_ms",
    "retrieval_latency_p50_ms",
    "retrieval_latency_p95_ms",
    "storage_efficiency_bytes_per_memory",
    "stale_embedding_ratio",
];

impl EvalGateThresholds {
    /// Fails closed if any `REQUIRED_METRIC_NAMES` entry has no threshold.
    pub fn require_all(&self) -> Result<(), EvalGateError> {
        for name in REQUIRED_METRIC_NAMES {
            if !self.thresholds.contains_key(*name) {
                return Err(EvalGateError::MissingMetric((*name).to_string()));
            }
        }
        Ok(())
    }

    /// Returns the threshold spec and pass/fail verdict for a computed metric
    /// value, or `None` if `name` has no configured gate.
    pub fn evaluate(&self, name: &str, value: f64) -> Option<(ThresholdSpec, bool)> {
        self.thresholds
            .get(name)
            .map(|spec| (spec.clone(), spec.comparison.passes(value, spec.value)))
    }
}

/// Loads the embedded default gate thresholds shipped with the crate.
///
/// # Panics
/// Panics if the embedded `eval-harness-v1.json` fails to parse or does not gate
/// every required metric — a release-blocking authoring bug, not a runtime error.
pub(crate) fn embedded_default_thresholds() -> EvalGateThresholds {
    let thresholds: EvalGateThresholds =
        serde_json::from_str(include_str!("../../eval-harness-v1.json"))
            .expect("embedded eval-harness-v1.json must deserialize");
    thresholds
        .require_all()
        .expect("embedded eval-harness-v1.json must gate every required metric");
    thresholds
}

/// Loads gate thresholds from a caller-supplied file path.
pub(crate) fn load_thresholds(path: &Path) -> Result<EvalGateThresholds, EvalGateError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| EvalGateError::Io(path.display().to_string(), error.to_string()))?;
    let thresholds: EvalGateThresholds = serde_json::from_str(&raw)
        .map_err(|error| EvalGateError::Parse(path.display().to_string(), error.to_string()))?;
    thresholds.require_all()?;
    Ok(thresholds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_default_thresholds_gate_every_required_metric() {
        let thresholds = embedded_default_thresholds();
        assert!(thresholds.require_all().is_ok());
        for name in REQUIRED_METRIC_NAMES {
            assert!(
                thresholds.thresholds.contains_key(*name),
                "missing gate for {name}"
            );
        }
    }

    #[test]
    fn comparison_gte_and_lte_pass_correctly() {
        assert!(Comparison::Gte.passes(0.9, 0.9));
        assert!(Comparison::Gte.passes(0.95, 0.9));
        assert!(!Comparison::Gte.passes(0.8, 0.9));
        assert!(Comparison::Lte.passes(0.05, 0.05));
        assert!(!Comparison::Lte.passes(0.06, 0.05));
    }

    #[test]
    fn require_all_fails_closed_on_a_missing_metric() {
        let mut thresholds = embedded_default_thresholds();
        thresholds.thresholds.remove("hallucination_rate");
        let error = thresholds.require_all().expect_err("must fail closed");
        assert!(
            matches!(error, EvalGateError::MissingMetric(name) if name == "hallucination_rate")
        );
    }

    #[test]
    fn evaluate_returns_none_for_an_unconfigured_metric() {
        let thresholds = embedded_default_thresholds();
        assert!(thresholds.evaluate("not_a_real_metric", 1.0).is_none());
    }
}
