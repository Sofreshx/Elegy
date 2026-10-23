"""Recheck retained evidence offline. Does not call models or open Memory stores.

Exit 0 means internally consistent evidence, including expected failed product
acceptance. It never means that answer quality passed.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

from check_results import verdict
from repeat_answers import inputs, verify_raw

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]


def load(path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify():
    runtime = ROOT / "runtime"
    runtime.mkdir(exist_ok=True)
    report = {"schemaVersion": "memory-recall-fix-evidence-check/v1",
              "verifiedAt": datetime.now(timezone.utc).isoformat(), "checks": []}

    def check(name, passed, detail=None):
        report["checks"].append({"name": name, "passed": bool(passed), "detail": detail})

    reproduced = runtime / "assessment-recheck.json"
    result = subprocess.run([sys.executable, str(ROOT / "grade.py"), "--answers", str(ROOT / "answers-01"),
                             "--replay", str(ROOT / "replay-01"), "--output", str(reproduced)],
                            cwd=REPO, capture_output=True, timeout=60)
    check("primary-48-reproduction", result.returncode == 0 and load(reproduced) == load(ROOT / "assessment.json"))
    protocol, corpus, primary, packets, prechecks = inputs(ROOT / "replay-01", ROOT / "answers-01")
    check("frozen-primary-inputs", not prechecks["errors"], prechecks["errors"])
    stages = {stage["caseId"]: stage for path in sorted((ROOT / "replay-01").glob("*/run.json"))
              for stage in load(path)["stages"]}
    cleanup = [row for path in sorted((ROOT / "replay-01").glob("*/run.json")) for row in load(path)["cleanup"]]
    source_checks = [packet["sourceUnchanged"] and packet["sourceBefore"]["durableData"] == packet["sourceAfter"]["durableData"]
                     for stage in stages.values() for packet in [stage["packet"], stage["baselinePacket"]]]
    fields = ["id", "version", "content", "scope", "provenance", "updatedAt"]
    def recalled(case, old=False):
        return stages[case]["baselinePacket" if old else "packet"]["response"]["data"]["recalls"]
    equal_rows = all([[r.get(k) for k in fields] for r in recalled(case)] == [[r.get(k) for k in fields] for r in recalled(case, True)] for case in ["q08", "q11"])
    corrected = all("REPLAY_PINE" in json.dumps(recalled(case)) and "75" in json.dumps(recalled(case))
                    and "REPLAY_BIRCH" not in json.dumps(recalled(case)) and "REPLAY_BIRCH" in json.dumps(recalled(case, True)) for case in ["q17", "q18"])
    check("packet-controls-reproduction", len(stages) == 24 and len(source_checks) == 48 and all(source_checks)
          and len(cleanup) == 16 and all(c["verifiedEmpty"] for c in cleanup) and equal_rows and corrected
          and "OVERRIDE_GARNET" in json.dumps(recalled("q12")) and "SECRET_PEPPER_924" not in json.dumps(recalled("q15"))
          and not recalled("q24"))
    repetitions = load(ROOT / "repeats-01/run.json")
    context = dict(repetitions, output=ROOT / "repeats-01")
    cases = {c["id"]: c for episode in corpus["episodes"] for c in episode["cases"]}
    repeated_errors = []
    threads = set(prechecks["primaryThreads"])
    for saved in repetitions["records"]:
        path = ROOT / "repeats-01" / saved["rawPath"]
        actual = verify_raw(path, cases[saved["caseId"]], packets[saved["caseId"]], protocol, context, prechecks["primaryThreads"])
        actual["trial"] = saved["trial"]
        if actual != saved or actual["executionErrors"] or actual["formatErrors"]:
            repeated_errors.append(saved["trial"] + "/" + saved["caseId"])
        threads.add(actual["threadId"])
    check("ten-repeat-raw-reproduction", not repeated_errors and len(repetitions["records"]) == 10, repeated_errors)
    check("58-distinct-model-contexts", len(threads) == 58, len(threads))
    expected_gate = verdict()
    check("adverse-acceptance-preserved", expected_gate == load(ROOT / "verdicts.json") and expected_gate["status"] == "blocked"
          and [c["caseId"] for c in expected_gate["cases"] if not c["passed"]] == ["q08", "q09", "q11"])
    subject, lock = load(ROOT / "subject.json"), load(ROOT / "source-lock.json")
    changed, revision_mismatch, checkout_eol_paths = [], [], []
    digest = hashlib.sha256()
    entries = lock["sourceFiles"] + lock["mcpSourceFiles"]
    batch = subprocess.run(["git", "cat-file", "--batch"],
                           input="".join(subject["sourceRevision"] + ":" + e["path"] + "\n" for e in entries).encode(),
                           cwd=REPO, capture_output=True, timeout=120)
    if batch.returncode:
        raise RuntimeError("Cannot read the committed source objects")
    offset = 0
    for entry in entries:
        path = REPO / entry["path"]
        if sha(path) != entry["sha256"]:
            changed.append(entry["path"])
        end = batch.stdout.index(b"\n", offset)
        header = batch.stdout[offset:end].split()
        if len(header) != 3 or header[1] != b"blob":
            raise RuntimeError("Missing source object in committed revision")
        size = int(header[2]); body = batch.stdout[end + 1:end + 1 + size]
        offset = end + 1 + size + 1
        if len(body) != size:
            revision_mismatch.append(entry["path"])
        elif hashlib.sha256(body).hexdigest() != entry["sha256"]:
            # Source locks bind exact executed checkout bytes. Git may store LF
            # for the Windows checkout's CRLF. Disclose this instead of changing
            # qualified files or pretending the raw Git blob hashes are equal.
            if body.replace(b"\r\n", b"\n") == path.read_bytes().replace(b"\r\n", b"\n"):
                checkout_eol_paths.append(entry["path"])
            else:
                revision_mismatch.append(entry["path"])
    for entry in lock["sourceFiles"]:
        path_bytes, body = entry["path"].encode("utf-8"), (REPO / entry["path"]).read_bytes()
        digest.update(len(path_bytes).to_bytes(8, "little")); digest.update(path_bytes)
        digest.update(len(body).to_bytes(8, "little")); digest.update(body)
    check("source-bytes-and-committed-revision", not changed and not revision_mismatch,
          {"cliFiles": len(lock["sourceFiles"]), "mcpFiles": len(lock["mcpSourceFiles"]), "changed": changed,
           "revisionMismatch": revision_mismatch, "checkoutEolOnlyPaths": checkout_eol_paths,
           "method": "Exact checkout SHA-256 for every locked input; committed blobs must be equal or differ only by CRLF/LF checkout conversion."})
    check("canonical-cli-source-fingerprint", digest.hexdigest() == subject["cliSourceSha256"] == lock["cliSourceSha256"], digest.hexdigest())
    check("source-lock-binding", sha(ROOT / "source-lock.json") == subject["sourceLockSha256"])
    for name, file in [("cli", "elegy-memory.exe"), ("mcp", "elegy-memory-mcp-stdio.exe")]:
        binary = runtime / "qualified-binaries" / file
        check(name + "-binary-binding", binary.is_file() and sha(binary) == subject[name + "Sha256"])
    compatibility = load(ROOT / "compatibility-02/run.json")
    expected_labels = {f"{label} {suffix}" for label in ["old", "new"] for suffix in [
        "reembed succeeded against loopback fixture", "user reembed succeeded against loopback fixture",
        "contextual vector-only succeeded", "contextual vector-only returned results",
        "vector-only leaves source DB and WAL unchanged", "hybrid contextual recall succeeded",
        "hybrid leaves source DB and WAL unchanged"]} | {
        "new hybrid returns non-empty results", "hybrid keeps vector-selected workspace memory",
        "hybrid preserves workspace scope filter", "hybrid preserves agent ownership filter",
        "contextual recall leaves source DB and WAL unchanged", "ordinary search old/new exit successfully",
        "ordinary search returns non-empty results", "ordinary search IDs/content/similarity/score match",
        "contextual vector-only old/new exit successfully", "contextual vector-only returns non-empty results for both CLIs",
        "contextual vector-only IDs/content/similarity/score match"}
    check("compatibility-controls", compatibility["status"] == "passed" and not compatibility["errors"]
          and len(compatibility["checks"]) == 25 and all(c["passed"] for c in compatibility["checks"])
          and {c["message"] for c in compatibility["checks"]} == expected_labels)
    baseline = load(ROOT / "baseline-subject.json")
    check("compatibility-binary-bindings", compatibility["binaries"]["new"]["sha256"] == subject["cliSha256"]
          and compatibility["binaries"]["old"]["sha256"] == baseline["cliSha256"]
          and compatibility["binaries"]["new"]["path"] == "<new-cli>"
          and compatibility["binaries"]["old"]["path"] == "<old-cli>")
    from compatibility_probe import search_projection, recall_projection
    probes = compatibility["probes"]
    all_success = all(probes[name][label]["exitCode"] == 0 and json.loads(probes[name][label]["stdout"])["status"] == "ok"
                      for name in ["ordinarySearch", "contextualRecall", "hybrid", "reembed", "reembedUser"] for label in ["old", "new"])
    search = {label: search_projection(probes["ordinarySearch"][label]) for label in ["old", "new"]}
    vector = {label: recall_projection(probes["contextualRecall"][label]) for label in ["old", "new"]}
    ids = [json.loads(row["stdout"])["data"]["memory"]["id"] for row in probes["seed"]]
    hybrid = recall_projection(probes["hybrid"]["new"])["recalls"]
    check("compatibility-raw-reproduction", all_success and bool(search["old"]) and search["old"] == search["new"] == probes["ordinarySearchProjection"]["new"]
          and search["old"] == probes["ordinarySearchProjection"]["old"] and bool(vector["old"]["recalls"])
          and vector["old"] == vector["new"] == probes["contextualRecallProjection"]["new"]
          and vector["old"] == probes["contextualRecallProjection"]["old"] and bool(hybrid)
          and {r["id"] for r in hybrid} == {ids[0]} and all(r["scope"] == "Workspace" for r in hybrid)
          and compatibility["fixture"]["provider"] == "loopback-only"
          and all(r["path"] == "/v1/embeddings" and r["count"] > 0 for r in compatibility["fixture"]["requests"]))
    host = load(ROOT / "second-host-02/run.json")
    check("second-host-block-preserved", host["status"] == "blocked" and not host["memoryIntegrationExercised"]
          and any(e.get("error", {}).get("data", {}).get("statusCode") == 403 for e in host["providerErrors"]))
    check("second-host-runner-and-catalog", sha(ROOT / "second-host-02/probe-source.py") == host["runnerSha256"]
          and sha(ROOT / "second-host-02/catalog.stdout") == host["catalogSha256"])
    # The host runner hashes captured UTF-8 text before Windows newline translation.
    for command in host["commands"]:
        for stream in ["stdout", "stderr"]:
            body = (ROOT / "second-host-02" / (command["name"] + "." + stream)).read_text(encoding="utf-8")
            check("second-host-text:" + command["name"] + ":" + stream,
                  hashlib.sha256(body.encode()).hexdigest() == command[stream + "Sha256"])
    host_summary = load(ROOT / "second-host-summary.json")
    check("second-host-summary", host_summary["status"] == host["status"]
          and not host_summary["memoryIntegrationExercised"] and host_summary["evidence"] == "second-host-02/run.json")
    ledger_path = ROOT / "ledger.json"
    if ledger_path.exists():
        for item in load(ledger_path):
            wrapper = load(ROOT / "receipts" / (item["id"] + ".json"))
            check("receipt-index:" + item["id"], item["sha256"] == wrapper["sha256"])
            receipt = wrapper["receipt"]
            check("receipt-inconclusive:" + item["id"], receipt["outcome"] == "inconclusive")
            for evidence in receipt["observation"]["evidence"]:
                check("receipt-file:" + evidence["path"], sha(ROOT / evidence["path"]) == evidence["sha256"])
    else:
        check("receipt-present", False)
    manifest = load(ROOT / "evidence-manifest.json")
    manifest_mismatch = [e["path"] for e in manifest["files"] if sha(ROOT / e["path"]) != e["sha256"]]
    check("immutable-experiment-manifest", len(manifest["files"]) == 108 and not manifest_mismatch, manifest_mismatch)
    final_status_path = ROOT / "final-status.json"
    if final_status_path.exists():
        status = load(final_status_path)["data"]
        quality = next(c for c in status["claims"] if c["claim"]["id"] == "recall.answer-quality")
        check("cli-receipt-readback", status["buildMatchesSource"] and quality["status"] == "reported-inconclusive"
              and quality["latestReceipt"] == load(ledger_path)[-1]["id"])
    else:
        check("cli-receipt-readback", False)
    validation_path = ROOT / "native-validation/run.json"
    if validation_path.exists():
        native = load(validation_path)
        check("native-validation-complete", len(native["results"]) == 10 and all(r["exitCode"] == 0 for r in native["results"]))
        for entry in native["results"]:
            check("native-log:" + entry["name"], sha(ROOT / "native-validation" / entry["log"]) == entry["logSha256"])
    else:
        check("native-validation-complete", False)
    broken, links = [], 0
    for name in ["README.md", "answers.md"]:
        for target in re.findall(r"\]\(([^)]+)\)", (ROOT / name).read_text(encoding="utf-8")):
            target = target.split("#")[0]
            if target and "://" not in target:
                links += 1
                if not (ROOT / target).exists():
                    broken.append({"document": name, "target": target})
    check("local-document-links", not broken, {"links": links, "broken": broken})
    diff = subprocess.run(["git", "diff", "--check"], cwd=REPO, capture_output=True, timeout=30)
    check("git-diff-check", diff.returncode == 0)
    report["status"] = "passed-evidence-checks" if all(c["passed"] for c in report["checks"]) else "failed"
    report["productAcceptance"] = "blocked"
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "runtime/evidence-recheck.json")
    args = parser.parse_args()
    try:
        report = verify()
    except Exception as exc:
        report = {"schemaVersion": "memory-recall-fix-evidence-check/v1", "status": "failed",
                  "productAcceptance": "blocked", "checks": [{"name": "verification-exception", "passed": False,
                  "detail": {"type": type(exc).__name__, "message": str(exc)}}]}
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": report["status"], "failed": [c["name"] for c in report["checks"] if not c["passed"]]}))
    raise SystemExit(0 if report["status"] == "passed-evidence-checks" else 1)
