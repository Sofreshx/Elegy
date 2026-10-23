"""Controlled envelope experiment; simulated candidate, never product evidence.

Uses only the installed Codex CLI's existing ChatGPT login. No real Memory
access, paid API fallback, persistent configuration or previous model answers.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import json
from pathlib import Path
import shutil
import subprocess

from answer_runner import ROOT, configuration, environment, invoke, now, save, sha
from grade import parse_stdout_details, score_answer


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        raise RuntimeError("Output must be new; retain every attempt")
    design = json.loads((ROOT / "diagnostic-protocol.json").read_text(encoding="utf-8"))
    protocol = json.loads((ROOT / "protocol.json").read_text(encoding="utf-8"))
    corpus = json.loads((ROOT / "corpus.json").read_text(encoding="utf-8"))
    cases = {case["id"]: case for episode in corpus["episodes"] for case in episode["cases"]}
    old_root = (ROOT / design["oldEvidence"]).resolve()
    packets = {stage["caseId"]: stage["packet"] for path in sorted((old_root / "replay-01").glob("*/run.json"))
               for stage in json.loads(path.read_text(encoding="utf-8"))["stages"]}
    exe = Path(shutil.which("codex") or "")
    if not exe.is_file():
        raise RuntimeError("Installed Codex CLI required; no installation or substitute")
    login = subprocess.run([str(exe), "login", "status"], env=environment(), capture_output=True, encoding="utf-8", timeout=30)
    if login.returncode or "chatgpt" not in (login.stdout + login.stderr).lower():
        raise RuntimeError("Existing ChatGPT login required; no API-key fallback")
    config, config_sha = configuration(exe, protocol)
    contexts = {}
    for case_id in design["cases"]:
        original = packets[case_id]["response"]["data"]["additionalContext"]
        prefix, separator, body = original.partition("\n")
        if not separator or not body or len(original.encode()) > 600:
            raise RuntimeError("Missing or oversized original context")
        candidate = design["candidateEnvelope"] + body
        if json.loads(body) != json.loads(candidate.partition("\n")[2]) or len(candidate.encode()) > 600:
            raise RuntimeError("Candidate changes data or exceeds product budget")
        contexts[case_id] = {"current": original, "candidate": candidate}
    output.mkdir()
    manifest = {"schemaVersion": "memory-grounding-diagnostic-run/v1", "status": "running", "startedAt": now(),
                "candidateIsProductOutput": False, "configuration": config, "configurationSha256": config_sha,
                "protocolSha256": sha(ROOT / "protocol.json"), "designSha256": sha(ROOT / "diagnostic-protocol.json"),
                "corpusSha256": sha(ROOT / "corpus.json"), "runnerSha256": sha(__file__),
                "answerRunnerSha256": sha(ROOT / "answer_runner.py"), "paidApiCalls": 0,
                "originalReplaySha256": sha(old_root / "replay-01/run.json"), "contexts": contexts,
                "records": [], "errors": []}
    save(output / "run.json", manifest)

    def pair(trial, case_id):
        conditions = list(design["conditions"])
        if (trial + int(case_id[1:])) % 2:
            conditions.reverse()
        for condition in conditions:
            destination = output / condition / f"trial-{trial}"
            destination.mkdir(parents=True, exist_ok=True)
            invoke(exe, protocol, destination, case_id, "with", cases[case_id]["question"], contexts[case_id][condition], config_sha)

    jobs = [(trial, case) for trial in range(1, design["trials"] + 1) for case in design["cases"]]
    with ThreadPoolExecutor(max_workers=3) as pool:
        for future in as_completed([pool.submit(pair, *job) for job in jobs]):
            try:
                future.result()
            except Exception as exc:
                manifest["errors"].append({"type": type(exc).__name__, "detail": str(exc)})
    threads = []
    for condition in design["conditions"]:
        for trial, case in jobs:
            path = output / condition / f"trial-{trial}" / "raw" / f"{case}_with.json"
            raw = json.loads(path.read_text(encoding="utf-8"))
            answer, thread, usage, errors = parse_stdout_details(raw.get("stdout", ""))
            if not raw.get("valid") or raw.get("exitCode") != 0 or raw.get("answer") != answer:
                errors.append("Invalid recorded execution")
            prompt = protocol["instruction"] + "\n\nDonnées historiques :\n" + contexts[case][condition] + "\n\nQuestion actuelle :\n" + cases[case]["question"]
            import hashlib
            if hashlib.sha256(prompt.encode()).hexdigest() != raw["promptSha256"] or raw["configurationSha256"] != config_sha:
                errors.append("Prompt or configuration hash mismatch")
            score = score_answer(answer, cases[case], execution_errors=errors)
            threads.append(thread)
            manifest["records"].append({"caseId": case, "condition": condition, "trial": trial, "answer": answer,
                "threadId": thread, "score": score["score"], "missingGroups": score["missingGroups"],
                "forbiddenMentions": score["forbiddenMentions"], "formatErrors": score["formatErrors"],
                "executionErrors": score["executionErrors"], "rawPath": path.relative_to(output).as_posix(),
                "rawSha256": sha(path), "usage": usage, "elapsedMs": raw["elapsedMs"]})
    if len(threads) != 30 or None in threads or len(set(threads)) != 30:
        manifest["errors"].append("Expected 30 distinct contexts")
    if any(r["executionErrors"] for r in manifest["records"]):
        manifest["errors"].append("Invalid execution record")
    manifest["status"] = "execution-valid" if not manifest["errors"] else "inconclusive"
    manifest["candidatePasses"] = manifest["status"] == "execution-valid" and all(r["score"] == 1 and not r["formatErrors"] for r in manifest["records"] if r["condition"] == "candidate")
    manifest["freshControlFailure"] = any(r["score"] != 1 for r in manifest["records"] if r["condition"] == "current")
    manifest["completedAt"] = now()
    save(output / "run.json", manifest)
    print(json.dumps({k: manifest[k] for k in ["status", "candidatePasses", "freshControlFailure"]}))
    return 0 if manifest["status"] == "execution-valid" else 1


if __name__ == "__main__":
    raise SystemExit(main())
