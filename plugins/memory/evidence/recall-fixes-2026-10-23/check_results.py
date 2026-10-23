"""Apply the predeclared acceptance gate to retained trials, without model calls.

Exit 1 is the expected adverse product result for this campaign. Integrity of
the experiment is verified separately by verify_evidence.py.
"""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TARGETS = ("q08", "q09", "q11", "q17", "q18")


def load(path):
    return json.loads(path.read_text(encoding="utf-8"))


def verdict():
    primary = load(ROOT / "assessment.json")
    repeats = load(ROOT / "repeats-01/run.json")
    trials = []
    for record in primary["records"]:
        if record["arm"] == "with" and record["caseId"] in TARGETS:
            trials.append({"caseId": record["caseId"], "trial": "primary",
                           "score": record["score"], "answer": record["actual"],
                           "rawPath": "answers-01/" + record["rawPath"].replace("\\", "/"),
                           "rawSha256": record["rawSha256"],
                           "missingGroups": record["missingGroups"],
                           "forbiddenMentions": record["forbiddenMentions"],
                           "formatErrors": record["formatErrors"],
                           "executionErrors": record["executionErrors"]})
    for record in repeats["records"]:
        trials.append({"caseId": record["caseId"], "trial": record["trial"],
                       "score": record["scoreLexical"], "answer": record["answer"],
                       "rawPath": "repeats-01/" + record["rawPath"].replace("\\", "/"),
                       "rawSha256": record["rawSha256"],
                       "missingGroups": record["missingGroups"],
                       "forbiddenMentions": record["forbiddenMentions"],
                       "formatErrors": record["formatErrors"],
                       "executionErrors": record["executionErrors"]})
    trials.sort(key=lambda r: (r["caseId"], r["trial"]))
    errors = list(primary["inventoryErrors"]) + list(repeats["errors"])
    if primary["status"] != "valid" or repeats["status"] != "execution-valid":
        errors.append("invalid execution")
    expected = {(case, trial) for case in TARGETS for trial in ("primary", "trial-2", "trial-3")}
    if len(trials) != 15 or {(r["caseId"], r["trial"]) for r in trials} != expected:
        errors.append("target inventory differs from frozen 5 cases x 3 trials")
    for row in trials:
        row["passed"] = row["score"] == 1 and not row["forbiddenMentions"] and not row["executionErrors"] and not row["formatErrors"]
    cases = [{"caseId": case, "passedTrials": sum(r["passed"] for r in trials if r["caseId"] == case),
              "requiredTrials": 3, "passed": all(r["passed"] for r in trials if r["caseId"] == case)} for case in TARGETS]
    passed = not errors and all(r["passed"] for r in cases)
    return {"schemaVersion": "memory-recall-fix-acceptance/v1", "status": "passed" if passed else "blocked",
            "criterion": "Each of five target cases passes all three retained trials; no replacement or retuning.",
            "executionErrors": errors, "cases": cases, "trials": trials,
            "individualFailures": [r for r in trials if not r["passed"]],
            "assessmentSha256": hashlib.sha256((ROOT / "assessment.json").read_bytes()).hexdigest(),
            "repetitionsSha256": hashlib.sha256((ROOT / "repeats-01/run.json").read_bytes()).hexdigest(),
            "pushAndMergeAllowed": passed,
            "limits": "Finite synthetic acceptance; no universal reliability, cost, or daily-use claim."}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "verdicts.json")
    args = parser.parse_args()
    result = verdict()
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": result["status"], "failedCases": [r["caseId"] for r in result["cases"] if not r["passed"]]}))
    raise SystemExit(0 if result["status"] == "passed" else 1)
