"""Single-factor prefix diagnostic using exact Q3 record bodies, not rebuilt recall."""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent
Q3 = ROOT.parent / "codex-qualification-2026-10-23"
Q2 = ROOT.parent / "answer-grounding-2026-10-23"
sys.path.insert(0, str(Q3)); sys.path.insert(0, str(Q2))
from answer_runner import configuration, environment, now
from assess import check_replay
from campaign import preflight, public
from qualification_runner import record_answer, replay_contexts

Q3_MANIFEST_SHA = "8398e6fa5246337c2cea8b530cf7d0d8e1042141aae02e408617a1dd432946bc"


def load(path): return json.loads(Path(path).read_text(encoding="utf-8"))
def sha(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def digest(text): return hashlib.sha256(text.encode()).hexdigest()
def save(path, value): Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def expected_inputs() -> dict:
    protocol = load(ROOT / "protocol.json"); base = load(Q3 / "protocol.json")
    manifest_path = Q3 / "evidence-manifest.json"
    if sha(manifest_path) != Q3_MANIFEST_SHA: raise ValueError("Q3 sealed evidence manifest changed")
    for item in load(manifest_path)["files"]:
        if sha(Q3 / item["path"]) != item["sha256"]: raise ValueError("Q3 sealed evidence changed")
    if sha(Q3 / "protocol.json") != protocol["baseProtocolSha256"]: raise ValueError("base protocol changed")
    if any(protocol[k] != base[k] for k in ("model", "reasoningEffort", "instruction", "sourceRevision")): raise ValueError("base configuration changed")
    if len(protocol["labelBefore"].encode()) != 22 or len(protocol["labelAfter"].encode()) != 22: raise ValueError("labels must be22bytes")
    contexts, cases, proofs = {}, {}, {}
    for phase, corpus_name in (("replay-primary", "corpus.json"), ("replay-transfer", "transfer-corpus.json")):
        corpus = load(Q3 / corpus_name)
        found, _ = replay_contexts(Q3 / phase, corpus, Q3 / "protocol.json", Q3 / corpus_name)
        contexts.update(found)
        replay_errors = []; check_replay(phase, corpus, base, replay_errors)
        if replay_errors: raise ValueError("Q3 replay invariants failed")
        cases.update({case["id"]: case for ep in corpus["episodes"] for case in ep["cases"]})
        for ep in corpus["episodes"]:
            path = Q3 / phase / ep["id"] / "run.json"
            for case in ep["cases"]: proofs[case["id"]] = {"path": path.relative_to(Q3).as_posix(), "sha256": sha(path)}
    jobs = []
    for declared in protocol["cases"]:
        case_id = declared["id"]; current = contexts[case_id]
        if not current.startswith(protocol["labelBefore"]): raise ValueError("unexpected current prefix")
        candidate = protocol["labelAfter"] + current[len(protocol["labelBefore"]):]
        if candidate[22:] != current[22:] or len(candidate.encode()) != len(current.encode()): raise ValueError("more than label changed")
        body = current.split("\n", 1)[1]
        for trial in declared["trials"]:
            jobs.append({"caseId": case_id, "trial": trial, "case": cases[case_id], "contexts": {"current": current, "candidate": candidate},
                         "bodySha256": digest(body), "contextSha256": {"current": digest(current), "candidate": digest(candidate)}, "proof": proofs[case_id]})
    if len(jobs) != 21 or len({(x["caseId"], x["trial"]) for x in jobs}) != 21: raise ValueError("expected21groups")
    return {"schemaVersion": "memory-sol-version-label-inputs/v1", "protocolSha256": sha(ROOT / "protocol.json"),
            "baseSubjectSha256": sha(Q3 / "subject.json"), "jobs": jobs}


def run(output: Path) -> int:
    if output.exists(): raise RuntimeError("preserve all prior outputs")
    inputs = expected_inputs()
    if load(ROOT / "inputs.json") != inputs: raise ValueError("frozen inputs changed")
    protocol = load(Q3 / "protocol.json"); exe = Path(shutil.which("codex") or "")
    before = preflight(); client = sha(exe); self_before = sha(__file__)
    login = subprocess.run([str(exe), "login", "status"], env=environment(), capture_output=True, encoding="utf-8", timeout=30)
    if login.returncode or "chatgpt" not in (login.stdout + login.stderr).lower(): raise RuntimeError("existing ChatGPT authentication required, no API fallback")
    config, config_sha = configuration(exe, protocol)
    output.mkdir()
    result = {"schemaVersion": "memory-sol-version-label-run/v1", "status": "running", "startedAt": now(),
              "protocolSha256": sha(ROOT / "protocol.json"), "inputsSha256": sha(ROOT / "inputs.json"),
              "selfSha256Before": self_before, "configuration": config, "configurationSha256": config_sha,
              "codexSha256Before": client, "ordinaryChatGPTAuth": True, "paidApiCalls": 0,
              "preflightBefore": public(before), "records": [], "errors": []}
    save(output / "run.json", result)
    def group(index, job):
        records = []
        conditions = ("current", "candidate") if index % 2 == 0 else ("candidate", "current")
        for condition in conditions:
            destination = output / condition / f"trial-{job['trial']}"
            record = record_answer(exe, protocol, job["case"], "with", job["contexts"][condition], config_sha, destination, output)
            records.append({**record, "condition": condition, "trial": job["trial"], "bodySha256": job["bodySha256"]})
        return records
    with ThreadPoolExecutor(max_workers=3) as pool:
        futures = [pool.submit(group, index, job) for index, job in enumerate(inputs["jobs"])]
        for future in as_completed(futures):
            try: result["records"].extend(future.result())
            except Exception as exc: result["errors"].append({"type": type(exc).__name__, "detail": str(exc)})
            save(output / "run.json", result)
    expected = {(job["caseId"], condition, job["trial"]) for job in inputs["jobs"] for condition in ("current", "candidate")}
    actual = {(r["caseId"], r["condition"], r["trial"]) for r in result["records"]}
    if actual != expected or len(result["records"]) != 42: result["errors"].append("record inventory mismatch")
    threads = [r["threadId"] for r in result["records"]]
    if any(not t for t in threads) or len(set(threads)) != 42: result["errors"].append("thread identity mismatch")
    if any(r["executionErrors"] or r["formatErrors"] for r in result["records"]): result["errors"].append("execution/format errors")
    finish_checks(result, client, self_before)
    save(output / "run.json", result)
    print(json.dumps({"status": result["status"], "recordCount": len(result["records"]), "uniqueThreads": len(set(threads)), "errors": result["errors"]}, ensure_ascii=True))
    return 0 if not result["errors"] else 1


def finish_checks(result: dict, client: str, self_before: str) -> None:
    result["completedAt"] = now()
    try:
        result["preflightAfter"] = public(preflight())
        result["codexSha256After"] = sha(Path(shutil.which("codex") or ""))
        result["selfSha256After"] = sha(__file__)
        if result["codexSha256After"] != client or result["selfSha256After"] != self_before or load(ROOT / "inputs.json") != expected_inputs():
            result["errors"].append("inputs/client/source changed")
    except Exception as exc:
        result["errors"].append({"type": "postflight-failed", "detail": str(exc)})
    result["status"] = "execution-valid" if not result["errors"] else "inconclusive"


def main() -> int:
    parser = argparse.ArgumentParser(); parser.add_argument("action", choices=("prepare", "run")); parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.action == "prepare":
        target = ROOT / "inputs.json"
        if target.exists(): raise RuntimeError("preserve frozen inputs")
        save(target, expected_inputs()); print(json.dumps({"groups": 21, "contexts": 42, "labelBytes": 22})); return 0
    if not args.output: parser.error("run requires a new output directory")
    return run(args.output.resolve())


if __name__ == "__main__": raise SystemExit(main())
