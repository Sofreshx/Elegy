use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

struct Fixture {
    evidence: TempDir,
    evidence_path: PathBuf,
    project: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let evidence = tempfile::tempdir().expect("evidence directory");
        let evidence_path = evidence.path().canonicalize().expect("evidence path");
        Self {
            evidence,
            evidence_path,
            project: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .expect("repository"),
        }
    }

    fn call(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
            .arg("eval")
            .args(args)
            .arg("--json")
            .arg("--project")
            .arg(&self.project)
            .arg("--evidence-dir")
            .arg(&self.evidence_path)
            .output()
            .expect("qualification process")
    }

    fn ok(&self, args: &[&str]) -> Value {
        let result = self.call(args);
        assert!(
            result.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        data(&result)
    }

    fn observation(&self) -> Value {
        let bytes = b"Synthetic observation fixture: session A wrote marker; session B recalled marker; marker removed.\n";
        fs::write(self.evidence_path.join("observation.txt"), bytes).expect("evidence");
        let checks = ["installed-client", "stored-via-mcp", "fresh-session-recall", "same-namespace", "cleanup"]
            .into_iter()
            .map(|name| json!({"name":name,"passed":true,"detail":"Synthetic fixture observation, not installed-host proof."}))
            .collect::<Vec<_>>();
        json!({
            "schemaVersion":"memory-observation/v1", "claimId":"host.cross-session", "protocolVersion":1,
            "subject": {"artifactSha256": format!("{:x}", Sha256::digest(b"synthetic installed artifact")), "host":"synthetic-test-host", "hostVersion":"1.0", "configurationSha256": format!("{:x}", Sha256::digest(b"synthetic configuration"))},
            "startedAt":"2026-01-01T00:00:00Z", "completedAt":"2026-01-01T00:01:00Z",
            "sessions":["synthetic-session-a", "synthetic-session-b"],
            "checks":checks,
            "evidence":[{"path":"observation.txt", "sha256":format!("{:x}", Sha256::digest(bytes))}]
        })
    }

    fn record(&self, observation: &Value) -> Output {
        let path = self.evidence_path.join("input.json");
        fs::write(
            &path,
            serde_json::to_vec(observation).expect("observation JSON"),
        )
        .expect("observation input");
        self.call(&[
            "record",
            "--claim",
            "host.cross-session",
            "--observation",
            path.to_str().expect("path"),
        ])
    }
}

fn data(output: &Output) -> Value {
    serde_json::from_slice::<Value>(&output.stdout).expect("JSON envelope")["data"].clone()
}

fn claim_status<'a>(status: &'a Value, id: &str) -> &'a Value {
    status["claims"]
        .as_array()
        .expect("claims")
        .iter()
        .find(|entry| entry["claim"]["id"] == id)
        .expect("claim")
}

#[test]
fn inventory_is_complete_and_next_is_deterministic_without_writing_evidence() {
    let f = Fixture::new();
    let status = f.ok(&["status"]);
    let claims = status["claims"].as_array().expect("claims");
    assert_eq!(claims.len(), 5);
    assert!(claims
        .iter()
        .all(|c| c["status"] == "unverified" && c["receiptCount"] == 0));
    assert_eq!(status["readinessPromotion"], false);
    let next = f.ok(&["next"]);
    assert_eq!(next, f.ok(&["next"]));
    assert_eq!(next["next"]["claim"]["id"], "forgetting.retention");
    assert!(!f.evidence_path.join("receipts").exists());
}

#[test]
fn executed_scenarios_survive_process_restart_without_promoting_readiness() {
    let f = Fixture::new();
    let readiness = f.project.join("docs/readiness.md");
    let before = fs::read(&readiness).expect("readiness");
    for id in ["recall.contract", "forgetting.retention"] {
        let receipt = f.ok(&["run", "--claim", id]);
        assert_eq!(receipt["outcome"], "satisfied");
        assert_eq!(receipt["provenance"], "local-scenario");
        assert!(!receipt["checks"].as_array().expect("checks").is_empty());
        let history = f.ok(&["history", "--claim", id]);
        assert_eq!(history.as_array().expect("history").len(), 1);
        assert_eq!(history[0], receipt);
        let saved = fs::read(
            f.evidence
                .path()
                .join("receipts")
                .join(format!("{}.json", receipt["id"].as_str().expect("id"))),
        )
        .expect("saved receipt");
        let status = f.ok(&["status"]);
        assert_eq!(claim_status(&status, id)["status"], "satisfied");
        assert_eq!(status["readinessPromotion"], false);
        assert_eq!(
            saved,
            fs::read(
                f.evidence
                    .path()
                    .join("receipts")
                    .join(format!("{}.json", receipt["id"].as_str().expect("id")))
            )
            .expect("unchanged receipt")
        );
    }
    assert_eq!(f.ok(&["history"]).as_array().expect("history").len(), 2);
    assert_eq!(before, fs::read(readiness).expect("readiness"));
}

#[test]
fn unknown_claims_external_execution_and_mixed_legacy_flags_are_rejected() {
    let f = Fixture::new();
    for args in [
        vec!["run", "--claim", "missing.claim"],
        vec!["history", "--claim", "missing.claim"],
        vec!["run", "--claim", "host.cross-session"],
        vec![
            "run",
            "--claim",
            "recall.contract",
            "--corpus",
            "missing.json",
        ],
        vec![
            "run",
            "--claim",
            "recall.contract",
            "--thresholds",
            "missing.json",
        ],
        vec![
            "run",
            "--claim",
            "recall.contract",
            "--output",
            "unused.json",
        ],
    ] {
        assert!(!f.call(&args).status.success(), "{args:?}");
    }
    assert!(!f.evidence_path.join("receipts").exists());
}

#[test]
fn external_success_is_reported_with_provenance_and_remains_review_required() {
    let f = Fixture::new();
    let observation = f.observation();
    let output = f.record(&observation);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt = data(&output);
    assert_eq!(receipt["provenance"], "external-observation");
    assert_eq!(receipt["observation"]["subject"], observation["subject"]);
    assert_eq!(receipt["observation"]["sessions"], observation["sessions"]);
    let status = f.ok(&["status"]);
    let entry = claim_status(&status, "host.cross-session");
    assert_eq!(entry["status"], "reported-satisfied");
    assert_eq!(entry["reviewRequired"], true);
    assert_eq!(status["readinessPromotion"], false);
    assert_eq!(
        f.ok(&["history", "--claim", "host.cross-session"])[0],
        receipt
    );
    fs::write(f.evidence_path.join("observation.txt"), b"changed evidence")
        .expect("change fixture");
    for args in [vec!["status"], vec!["next"], vec!["history"]] {
        assert!(
            !f.call(&args).status.success(),
            "changed evidence accepted: {args:?}"
        );
    }
}

#[test]
fn external_failure_and_interrupted_experiments_cannot_become_success() {
    for (error, expected) in [
        (None, "refuted"),
        (Some("synthetic experiment interrupted"), "inconclusive"),
    ] {
        let f = Fixture::new();
        let mut observation = f.observation();
        if let Some(error) = error {
            observation["error"] = json!(error);
        } else {
            observation["checks"][2]["passed"] = json!(false);
        }
        let output = f.record(&observation);
        assert!(!output.status.success());
        assert_eq!(data(&output)["outcome"], expected);
        let history = f.ok(&["history", "--claim", "host.cross-session"]);
        assert_eq!(history.as_array().expect("history").len(), 1);
        assert_eq!(history[0]["outcome"], expected);
    }
}

#[test]
fn incomplete_or_misattributed_observations_cannot_create_receipts() {
    let f = Fixture::new();
    let baseline = f.observation();
    let mut invalid = Vec::new();
    let mut value = baseline.clone();
    value["claimId"] = json!("host.agent-isolation");
    invalid.push(value);
    let mut value = baseline.clone();
    value["protocolVersion"] = json!(999);
    invalid.push(value);
    let mut value = baseline.clone();
    value["checks"].as_array_mut().expect("checks").pop();
    invalid.push(value);
    let mut value = baseline.clone();
    value["subject"]["artifactSha256"] = json!("not-a-hash");
    invalid.push(value);
    let mut value = baseline.clone();
    value["sessions"] = json!(["same", "same"]);
    invalid.push(value);
    let mut value = baseline.clone();
    value["evidence"][0]["sha256"] = json!("0".repeat(64));
    invalid.push(value);
    let mut value = baseline.clone();
    value["evidence"][0]["path"] = json!("../outside.txt");
    invalid.push(value);
    let mut value = baseline.clone();
    value["unrecognized"] = json!(true);
    invalid.push(value);
    for value in invalid {
        assert!(
            !f.record(&value).status.success(),
            "invalid observation accepted: {value}"
        );
    }
    assert!(!f.evidence_path.join("receipts").exists());
}

#[test]
fn corrupted_or_deleted_receipts_fail_closed_instead_of_restoring_a_pass() {
    for damage in ["malformed", "changed-outcome", "deleted"] {
        let f = Fixture::new();
        let observation = f.observation();
        assert!(f.record(&observation).status.success());
        let mut failed = observation.clone();
        failed["checks"][2]["passed"] = json!(false);
        let result = f.record(&failed);
        assert!(!result.status.success());
        let receipt = data(&result);
        assert_eq!(receipt["outcome"], "refuted");
        let path = f
            .evidence
            .path()
            .join("receipts")
            .join(format!("{}.json", receipt["id"].as_str().expect("id")));
        match damage {
            "malformed" => fs::write(&path, b"{broken").expect("malformed fixture"),
            "changed-outcome" => {
                let mut stored: Value =
                    serde_json::from_slice(&fs::read(&path).expect("receipt")).expect("stored");
                stored["receipt"]["outcome"] = json!("satisfied");
                fs::write(&path, serde_json::to_vec(&stored).expect("tampered JSON"))
                    .expect("tamper fixture");
            }
            "deleted" => fs::remove_file(&path).expect("delete fixture receipt"),
            _ => unreachable!(),
        }
        for args in [vec!["status"], vec!["next"], vec!["history"]] {
            assert!(
                !f.call(&args).status.success(),
                "{damage} accepted by {args:?}"
            );
        }
        assert!(
            !f.record(&observation).status.success(),
            "corrupt history must block new receipts"
        );
    }
}

#[test]
fn observation_schema_enforces_claim_specific_experiment_requirements() {
    let schema: Value = serde_json::from_str(include_str!(
        "../schemas/qualification-observation.schema.json"
    ))
    .expect("observation schema");
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(&schema)
        .expect("valid JSON Schema");
    let f = Fixture::new();
    let base = f.observation();
    let mut cases = vec![("valid cross-session", base.clone(), true)];

    let mut value = base.clone();
    value["claimId"] = json!("unknown");
    cases.push(("unknown claim", value, false));
    let mut value = base.clone();
    value["unknownField"] = json!(true);
    cases.push(("unknown field", value, false));

    let pair = json!({
        "id":"pair", "scoreWith":0.8, "scoreWithout":0.5,
        "latencyWithMs":3, "latencyWithoutMs":2, "costWith":1, "costWithout":1
    });
    let mut quality = base.clone();
    quality["claimId"] = json!("recall.answer-quality");
    let quality_checks = [
        "paired-inputs",
        "fixed-model-config",
        "predeclared-rubric",
        "independent-scoring",
        "negative-controls",
    ]
    .into_iter()
    .map(|name| json!({"name":name,"passed":true,"detail":"Synthetic schema fixture."}))
    .collect::<Vec<_>>();
    quality["checks"] = json!(quality_checks);
    cases.push(("quality missing pairs and cost", quality.clone(), false));
    quality["pairedCases"] = Value::Array(
        (0..10)
            .map(|i| {
                let mut item = pair.clone();
                item["id"] = json!(format!("case-{i}"));
                item
            })
            .collect(),
    );
    cases.push(("quality missing cost", quality.clone(), false));
    quality["costUnit"] = json!("synthetic-units");
    cases.push(("quality ten pairs plus cost", quality.clone(), true));
    let mut value = quality.clone();
    value["pairedCases"].as_array_mut().expect("pairs").pop();
    cases.push(("quality nine pairs", value, false));
    let mut value = quality;
    value["costUnit"] = Value::Null;
    cases.push(("quality null cost", value, false));

    let mut value = base.clone();
    value["pairedCases"] = json!([pair]);
    cases.push(("host carrying paired cases", value, false));
    let mut value = base;
    value["costUnit"] = json!("units");
    cases.push(("host carrying cost unit", value, false));

    for (name, observation, expected) in cases {
        assert_eq!(validator.is_valid(&observation), expected, "{name}");
    }
}

#[test]
fn mutable_bookkeeping_cannot_be_registered_as_external_evidence() {
    for name in ["ledger.json", ".writer.lock"] {
        let f = Fixture::new();
        let mut observation = f.observation();
        fs::write(f.evidence_path.join(name), b"[]").expect("bookkeeping fixture");
        observation["evidence"] =
            json!([{"path":name,"sha256":format!("{:x}", Sha256::digest(b"[]"))}]);
        let result = f.record(&observation);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stdout).contains("bookkeeping"));
        assert!(!f.evidence_path.join("receipts").exists());
        assert_eq!(
            fs::read(f.evidence_path.join(name)).expect("unchanged bookkeeping"),
            b"[]"
        );
    }
}
