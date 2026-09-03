//! Subprocess driver for the eval harness, matching the convention in
//! `tests/cli.rs`: spawn the built `elegy-memory` binary rather than reaching
//! into crate internals, since `crate::eval` is intentionally `pub(crate)` (the
//! eval harness is CLI-internal, not part of this crate's public API).
//!
//! Satisfies the spec's Validation clause: `cargo test -p elegy-memory --test
//! eval` runs the full harness against the embedded corpus with no network
//! access and no external download — every corpus and threshold used below is
//! either embedded in the binary or written to a local temp file by this test.

use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args(args)
        .output()
        .expect("spawn elegy-memory binary")
}

#[test]
fn eval_run_ci_passes_against_the_embedded_corpus_and_thresholds() {
    let output = run(&["eval", "run", "--ci"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "eval run --ci must exit 0 against the embedded corpus; stdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("PASSED"),
        "expected a PASSED summary line, got:\n{stdout}"
    );
}

#[test]
fn eval_run_ci_json_report_carries_all_eleven_gated_metrics() {
    let output = run(&["--format", "json", "eval", "run", "--ci"]);
    assert!(output.status.success(), "eval run --ci must exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: Value =
        serde_json::from_str(&stdout).expect("eval run --format json must emit a JSON envelope");
    assert_eq!(envelope["status"], "ok");
    let data = &envelope["data"];
    assert_eq!(data["passed"], true);
    let metrics = data["metrics"]
        .as_array()
        .expect("metrics must be an array");
    assert_eq!(
        metrics.len(),
        11,
        "every metric in the spec's Metrics table must be gated"
    );

    let required_names = [
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
    for name in required_names {
        let found = metrics.iter().any(|metric| metric["name"] == name);
        assert!(found, "missing gated metric `{name}` in eval run output");
    }
}

#[test]
fn eval_run_ci_fails_closed_when_a_threshold_is_impossible_to_meet() {
    let temp_dir = TempDir::new().expect("create temp directory");
    let thresholds_path = temp_dir.path().join("impossible-thresholds.json");
    std::fs::write(
        &thresholds_path,
        r#"{
  "version": "1",
  "thresholds": {
    "recall_at_10": { "comparison": "gte", "value": 1.1 },
    "precision_at_10": { "comparison": "gte", "value": 0.80 },
    "ndcg_at_10": { "comparison": "gte", "value": 0.85 },
    "hallucination_rate": { "comparison": "lte", "value": 0.15 },
    "gate_accuracy_8to1": { "comparison": "gte", "value": 0.95 },
    "write_latency_p50_ms": { "comparison": "lte", "value": 200.0 },
    "write_latency_p95_ms": { "comparison": "lte", "value": 500.0 },
    "retrieval_latency_p50_ms": { "comparison": "lte", "value": 450.0 },
    "retrieval_latency_p95_ms": { "comparison": "lte", "value": 600.0 },
    "storage_efficiency_bytes_per_memory": { "comparison": "lte", "value": 16000.0 },
    "stale_embedding_ratio": { "comparison": "lte", "value": 0.05 }
  }
}
"#,
    )
    .expect("write impossible thresholds fixture");

    let output = run(&[
        "eval",
        "run",
        "--ci",
        "--thresholds",
        thresholds_path.to_str().expect("utf8 temp path"),
    ]);
    assert!(
        !output.status.success(),
        "eval run --ci must exit non-zero when a metric misses its gate"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("FAILED"),
        "expected a FAILED summary line, got:\n{stdout}"
    );
    assert!(
        stdout.contains("[FAIL] recall_at_10"),
        "expected the impossible recall_at_10 gate to be reported as failing, got:\n{stdout}"
    );
}

#[test]
fn eval_run_ci_rejects_a_threshold_file_missing_a_required_metric() {
    let temp_dir = TempDir::new().expect("create temp directory");
    let thresholds_path = temp_dir.path().join("incomplete-thresholds.json");
    std::fs::write(
        &thresholds_path,
        r#"{"version":"1","thresholds":{"recall_at_10":{"comparison":"gte","value":0.9}}}"#,
    )
    .expect("write incomplete thresholds fixture");

    let output = run(&[
        "eval",
        "run",
        "--ci",
        "--thresholds",
        thresholds_path.to_str().expect("utf8 temp path"),
    ]);
    assert!(
        !output.status.success(),
        "a threshold file missing a required metric gate must fail closed, not silently skip it"
    );
}

#[test]
fn eval_list_corpora_reports_the_two_embedded_and_three_generated_corpora() {
    let output = run(&["--format", "json", "eval", "list-corpora"]);
    assert!(output.status.success(), "eval list-corpora must exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: Value =
        serde_json::from_str(&stdout).expect("eval list-corpora must emit a JSON envelope");
    let corpora = envelope["data"]
        .as_array()
        .expect("corpora must be an array");
    assert_eq!(corpora.len(), 5);

    let names: Vec<&str> = corpora
        .iter()
        .map(|corpus| {
            corpus["name"]
                .as_str()
                .expect("corpus name must be a string")
        })
        .collect();
    for expected in [
        "golden-v1",
        "gate-decisions-v1",
        "synthetic-distractors-1to1",
        "synthetic-distractors-4to1",
        "synthetic-distractors-8to1",
    ] {
        assert!(
            names.contains(&expected),
            "missing corpus `{expected}` in list-corpora output"
        );
    }
}

#[test]
fn eval_sweep_threshold_rejects_an_unknown_param_without_running_the_corpus() {
    let output = run(&[
        "eval",
        "sweep-threshold",
        "--param",
        "not_a_real_param",
        "--range",
        "0.1..0.2",
    ]);
    assert!(
        !output.status.success(),
        "sweep-threshold must reject an unknown parameter"
    );
}
