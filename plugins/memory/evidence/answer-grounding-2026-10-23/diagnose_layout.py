"""Run a bounded, causal layout diagnostic over retained replay contexts.

This is an evidence probe only.  ``text-only`` is an intentionally ineligible
ablation because it removes record references; no product recommendation is
emitted by this script.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

from answer_runner import ROOT, configuration, environment, invoke, now, save, sha
from grade import parse_stdout_details, score_answer


CONDITIONS = ("current", "text-first", "text-only")
CASE_TRIALS = {"q08": 3, "q09": 3, "q11": 3, "q17": 1, "q18": 1}
MAX_CONTEXT_BYTES = 600


def compact_json(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def context_layout(original: str) -> dict[str, str]:
    prefix, separator, body = original.partition("\n")
    if not separator or not body:
        raise ValueError("replay context lacks exact prefix and JSON body")
    parsed = json.loads(body)
    records = parsed.get("records")
    if not isinstance(records, list):
        raise ValueError("replay context JSON lacks records list")

    first_records = []
    text_records = []
    for record in records:
        if not isinstance(record, dict) or not isinstance(record.get("text"), str):
            raise ValueError("record lacks exact text field")
        first_records.append({"text": record["text"], **{k: v for k, v in record.items() if k != "text"}})
        text_records.append({"text": record["text"]})

    first_json = dict(parsed)
    first_json["records"] = first_records
    text_json = {"records": text_records}
    layouts = {
        "current": original,
        "text-first": prefix + "\n" + compact_json(first_json),
        "text-only": prefix + "\n" + compact_json(text_json),
    }
    if json.loads(layouts["text-first"].split("\n", 1)[1]) != parsed:
        raise ValueError("text-first JSON is not semantically identical")
    if json.loads(layouts["text-only"].split("\n", 1)[1]) != text_json:
        raise ValueError("text-only JSON roundtrip mismatch")
    for condition, context in layouts.items():
        if not context.startswith(prefix + "\n"):
            raise ValueError(f"{condition} changed the original prefix")
        if len(context.encode("utf-8")) > MAX_CONTEXT_BYTES:
            raise ValueError(f"{condition} context exceeds 600 bytes")
    return layouts


def load_replay_contexts(replay_root: Path, case_ids: set[str]) -> dict[str, str]:
    contexts: dict[str, str] = {}
    for run_path in sorted(replay_root.glob("*/run.json")):
        report = json.loads(run_path.read_text(encoding="utf-8"))
        for stage in report.get("stages", []):
            case_id = stage.get("caseId")
            if case_id in case_ids:
                contexts[case_id] = stage["packet"]["response"]["data"]["additionalContext"]
    missing = case_ids - contexts.keys()
    if missing:
        raise ValueError(f"old replay lacks cases: {sorted(missing)}")
    return contexts


def run_one(exe: Path, protocol: dict, case: dict, condition: str, trial: int,
            context: str, config_sha: str, output: Path) -> dict:
    destination = output / condition / f"trial-{trial}"
    destination.mkdir(parents=True, exist_ok=True)
    invoke(exe, protocol, destination, case["id"], "with", case["question"], context, config_sha)
    raw_path = destination / "raw" / f'{case["id"]}_with.json'
    raw = json.loads(raw_path.read_text(encoding="utf-8"))
    answer, thread, usage, parse_errors = parse_stdout_details(raw.get("stdout", ""))
    errors = list(parse_errors)
    if raw.get("exitCode") != 0:
        errors.append({"type": "process-exit", "exitCode": raw.get("exitCode")})
    if raw.get("answer") != answer or not raw.get("valid"):
        errors.append({"type": "invalid-recorded-execution"})
    prompt = protocol["instruction"] + "\n\nDonnées historiques :\n" + context + "\n\nQuestion actuelle :\n" + case["question"]
    prompt_sha = hashlib.sha256(prompt.encode("utf-8")).hexdigest()
    if raw.get("promptSha256") != prompt_sha:
        errors.append({"type": "prompt-hash-mismatch"})
    if raw.get("configurationSha256") != config_sha:
        errors.append({"type": "configuration-hash-mismatch"})
    scored = score_answer(answer, case, execution_errors=errors)
    return {
        "caseId": case["id"],
        "condition": condition,
        "trial": trial,
        "answer": answer,
        "score": scored["score"],
        "missingGroups": scored["missingGroups"],
        "forbiddenMentions": scored["forbiddenMentions"],
        "formatErrors": scored["formatErrors"],
        "executionErrors": scored["executionErrors"],
        "threadId": thread,
        "rawPath": raw_path.relative_to(output).as_posix(),
        "rawSha256": sha(raw_path),
        "promptSha256": prompt_sha,
        "contextSha256": hashlib.sha256(context.encode("utf-8")).hexdigest(),
        "usage": usage,
        "elapsedMs": raw.get("elapsedMs"),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        raise RuntimeError("output must be a new directory; prior attempts are retained")

    design = json.loads((ROOT / "layout-protocol.json").read_text(encoding="utf-8"))
    protocol = json.loads((ROOT / "protocol.json").read_text(encoding="utf-8"))
    corpus = json.loads((ROOT / "corpus.json").read_text(encoding="utf-8"))
    cases = {case["id"]: case for episode in corpus["episodes"] for case in episode["cases"]}
    if tuple(design.get("conditions", CONDITIONS)) != CONDITIONS:
        raise ValueError("layout protocol conditions must be current, text-first, text-only")
    if {row["id"]: row["trials"] for row in design["cases"]} != CASE_TRIALS or design["totalContexts"] != 33:
        raise ValueError("layout protocol case/trial inventory differs from the runner")
    selected = set(CASE_TRIALS)
    replay_root = (ROOT.parent / "recall-fixes-2026-10-23" / "replay-01").resolve()
    original_contexts = load_replay_contexts(replay_root, selected)
    layouts = {case_id: context_layout(original_contexts[case_id]) for case_id in selected}
    exe = Path(shutil.which("codex") or "")
    if not exe.is_file():
        raise RuntimeError("installed Codex CLI required; no API-key fallback")
    login = subprocess.run([str(exe), "login", "status"], env=environment(), capture_output=True, encoding="utf-8", timeout=30)
    if login.returncode or "chatgpt" not in (login.stdout + login.stderr).lower():
        raise RuntimeError("existing ChatGPT authentication required; no API-key fallback")
    config, config_sha = configuration(exe, protocol)
    output.mkdir()
    manifest = {
        "schemaVersion": "memory-layout-diagnostic/v1",
        "status": "running",
        "startedAt": now(),
        "configuration": config,
        "configurationSha256": config_sha,
        "codexSha256": sha(exe),
        "protocolSha256": sha(ROOT / "protocol.json"),
        "corpusSha256": sha(ROOT / "corpus.json"),
        "layoutProtocolSha256": sha(ROOT / "layout-protocol.json"),
        "runnerSha256": sha(__file__),
        "answerRunnerSha256": sha(ROOT / "answer_runner.py"),
        "oldReplaySha256": sha(replay_root / "run.json"),
        "ordinaryChatGPTAuth": True,
        "paidApiCalls": 0,
        "contexts": layouts,
        "records": [],
        "errors": [],
        "limits": {"maxContextBytes": MAX_CONTEXT_BYTES, "textOnlyProductionEligible": False},
    }
    save(output / "run.json", manifest)
    jobs = [(trial, case_id) for case_id, max_trial in CASE_TRIALS.items() for trial in range(1, max_trial + 1)]

    def group(trial: int, case_id: str) -> list[dict]:
        conditions = list(CONDITIONS)
        offset = (trial + int(case_id[1:])) % len(conditions)
        conditions = conditions[offset:] + conditions[:offset]
        return [run_one(exe, protocol, cases[case_id], condition, trial, layouts[case_id][condition], config_sha, output)
                for condition in conditions]

    try:
        with ThreadPoolExecutor(max_workers=3) as pool:
            futures = [pool.submit(group, trial, case_id) for trial, case_id in jobs]
            for future in as_completed(futures):
                try:
                    manifest["records"].extend(future.result())
                except Exception as exc:
                    manifest["errors"].append({"type": type(exc).__name__, "detail": str(exc)})
                save(output / "run.json", manifest)
    except Exception as exc:
        manifest["errors"].append({"type": type(exc).__name__, "detail": str(exc)})
    manifest["records"].sort(key=lambda row: (row["condition"], row["trial"], row["caseId"]))
    expected = sum(CASE_TRIALS.values()) * len(CONDITIONS)
    expected_keys = {(case_id, trial, condition)
                     for case_id, max_trial in CASE_TRIALS.items()
                     for trial in range(1, max_trial + 1)
                     for condition in CONDITIONS}
    actual_keys = {(row.get("caseId"), row.get("trial"), row.get("condition"))
                   for row in manifest["records"]}
    threads = [row.get("threadId") for row in manifest["records"]]
    if len(manifest["records"]) != expected:
        manifest["errors"].append(f"expected {expected} records, got {len(manifest['records'])}")
    if actual_keys != expected_keys:
        manifest["errors"].append("record inventory differs from the declared case/trial/condition matrix")
    if len(threads) != expected or None in threads or len(set(threads)) != expected:
        manifest["errors"].append(f"expected {expected} distinct threads")
    if any(row["executionErrors"] or row["formatErrors"] for row in manifest["records"]):
        manifest["errors"].append("recorded execution or format error")
    manifest["status"] = "execution-valid" if not manifest["errors"] else "inconclusive"
    manifest["completedAt"] = now()
    save(output / "run.json", manifest)
    print(json.dumps({"status": manifest["status"], "recordCount": len(manifest["records"])}, ensure_ascii=False))
    return 0 if manifest["status"] == "execution-valid" else 1


if __name__ == "__main__":
    raise SystemExit(main())
