"""Recheck every diagnostic answer and the predeclared prose gate offline."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys

from probe import ROOT, Q3, expected_inputs, load, sha

sys.path.insert(0, str(Q3))
from assess import check_raw


def assess() -> dict:
    errors, failed, rows = [], [], []
    inputs = expected_inputs(); protocol = load(Q3 / "protocol.json"); run = load(ROOT / "diagnostic-01/run.json")
    expected_config = load(Q3 / "answers-primary/run.json")
    if (run["status"] != "execution-valid" or run["errors"] or run["protocolSha256"] != sha(ROOT / "protocol.json")
            or run["inputsSha256"] != sha(ROOT / "inputs.json") or run["configuration"] != expected_config["configuration"]
            or run["configurationSha256"] != expected_config["configurationSha256"] or run["ordinaryChatGPTAuth"] is not True or run["paidApiCalls"] != 0):
        errors.append("run inputs/configuration/execution binding")
    subject = load(Q3 / "subject.json")
    if run["codexSha256Before"] != subject["codexSha256"] or run["codexSha256After"] != subject["codexSha256"]:
        errors.append("client changed")
    if run["selfSha256Before"] != sha(ROOT / "probe.py") or run["selfSha256After"] != sha(ROOT / "probe.py"): errors.append("probe source changed")
    for when in ("preflightBefore", "preflightAfter"):
        state = run[when]
        if any(value is not True for value in state["checks"].values() if isinstance(value, bool)): errors.append(f"{when}: preflight")
        if state["subjectSha256"] != sha(Q3 / "subject.json") or state["protocolSha256"] != sha(Q3 / "protocol.json"): errors.append(f"{when}: subject")
        if state["binaries"]["cli"]["sha256"] != subject["cliSha256"] or state["binaries"]["mcp"]["sha256"] != subject["mcpSha256"]: errors.append(f"{when}: binaries")
    jobs = {(j["caseId"], j["trial"]): j for j in inputs["jobs"]}
    expected = {(condition, case_id, trial) for case_id, trial in jobs for condition in ("current", "candidate")}
    actual = {(r["condition"], r["caseId"], r["trial"]) for r in run["records"]}
    if actual != expected or len(run["records"]) != 12: errors.append("record inventory")
    review_file = load(ROOT / "independent-review.json"); review_inputs = load(ROOT / "review-inputs.json")
    if review_file["inputSha256"] != sha(ROOT / "review-inputs.json"): errors.append("independent review input binding")
    reviews = {r["id"]: r for r in review_file["records"]}; masked = {r["id"]: r for r in review_inputs["records"]}
    if len(reviews) != 12 or len(masked) != 12: errors.append("review inventory")
    threads = set(); ids = set()
    old_threads = {r["threadId"] for phase in ("answers-primary", "answers-transfer", "repeats") for r in load(Q3 / phase / "run.json")["records"]}
    old_threads.update(r["threadId"] for r in load(ROOT.parent / "sol-version-label-2026-10-23" / "diagnostic-01/run.json")["records"])
    for record in run["records"]:
        key = [record["condition"], record["caseId"], record["trial"]]
        opaque = hashlib.sha256(json.dumps(key).encode()).hexdigest()[:16]; ids.add(opaque)
        job = jobs[(key[1], key[2])]; context = job["contexts"][key[0]]
        raw_name = f"{key[0]}/trial-{key[2]}/raw/{key[1]}_with.json"
        raw_path = ROOT / "diagnostic-01" / raw_name
        if record["rawPath"] != raw_name or record["rawSha256"] != sha(raw_path): errors.append(f"{key}: raw path/hash")
        check_raw(record, load(raw_path), job["case"], context, {"protocol": protocol}, run["configurationSha256"], errors)
        if record["bodySha256"] != job["bodySha256"]: errors.append(f"{key}: JSON body binding")
        thread = record["threadId"]
        if not thread or thread in threads or thread in old_threads: errors.append(f"{key}: reused/missing thread")
        threads.add(thread)
        review = reviews[opaque]; item = masked[opaque]
        for name, expected_value in (("answer", record["answer"]), ("question", job["case"]["question"]), ("context", context), ("rawSha256", record["rawSha256"]), ("answerSha256", hashlib.sha256(record["answer"].encode()).hexdigest())):
            if item.get(name) != expected_value: errors.append(f"{key}: masked input {name}")
            if name.endswith("Sha256") and review.get(name) != expected_value: errors.append(f"{key}: review {name}")
        flags = ["grounded", "taskComplete", "controlPass", "formatPass"]
        if any(type(review.get(flag)) is not bool for flag in flags) or not review.get("reason"): errors.append(f"{key}: incomplete prose review")
        passed = all(review.get(flag) is True for flag in flags) and not record["forbiddenMentions"] and not record["formatErrors"] and not record["executionErrors"]
        if not passed: failed.append({"condition": key[0], "caseId": key[1], "trial": key[2], "reason": review["reason"]})
        rows.append({**record, "semantic": review, "availableDataGatePassed": passed})
    if set(reviews) != ids or set(masked) != ids: errors.append("review identity inventory")
    counts = {condition: sum(r["condition"] == condition and r["availableDataGatePassed"] for r in rows) for condition in ("current", "candidate")}
    accepted = not errors and counts["candidate"] == 6
    return {"schemaVersion": "memory-fixture-adequacy-assessment/v1", "evidenceConsistent": not errors,
            "candidateAccepted": accepted, "status": "blocked-evidence" if errors else "accepted-diagnostic-only" if accepted else "rejected-as-predeclared",
            "errors": errors, "passedPer6": counts, "failedAnswers": failed, "records": rows,
            "protocolSha256": sha(ROOT / "protocol.json"), "inputsSha256": sha(ROOT / "inputs.json"),
            "runSha256": sha(ROOT / "diagnostic-01/run.json"), "reviewSha256": sha(ROOT / "independent-review.json"), "assessorSha256": sha(__file__),
            "parentAcceptance": "All12 actual paragraphs read; independent agent review accepted by parent. q20v2 retrieves stored region/tag, q25 explicitly abstains on undocumented approval. No old Q3/Q4 failure is rerated; this is fixture adequacy only, not rebuilt-binary qualification."}


def main() -> int:
    parser = argparse.ArgumentParser(); parser.add_argument("--output", type=Path, required=True); args = parser.parse_args()
    if args.output.exists(): raise RuntimeError("preserve prior assessments")
    result = assess(); args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in result.items() if k != "records"}, ensure_ascii=True))
    return 0 if result["candidateAccepted"] else 1


if __name__ == "__main__": raise SystemExit(main())
