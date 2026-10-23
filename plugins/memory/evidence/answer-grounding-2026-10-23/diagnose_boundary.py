"""Probe the boundary between program policy and unchanged historical data."""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import json
from pathlib import Path
import shutil
import subprocess

from answer_runner import ROOT, configuration, environment, now, save, sha
from diagnose_layout import CASE_TRIALS, load_replay_contexts, run_one


CONDITIONS = ("data-labeled", "separate-policy")
MAX_CONTEXT_BYTES = 600


def body_and_prefix(context: str) -> tuple[str, str]:
    index = context.find("\n")
    if index < 0 or index == len(context) - 1:
        raise ValueError("context must contain a prefix LF and JSON body")
    prefix_with_lf = context[:index + 1]
    body = context[index + 1:]
    if "\n" in prefix_with_lf[:-1] or not prefix_with_lf.endswith("\n"):
        raise ValueError("prefix must contain no internal LF and exactly one final LF")
    if len(context.encode("utf-8")) > MAX_CONTEXT_BYTES:
        raise ValueError("original context exceeds 600 bytes")
    if json.loads(body) is None:
        raise ValueError("historical JSON body is null")
    return prefix_with_lf, body


def failed_record(case: dict, condition: str, trial: int, error: Exception) -> dict:
    return {
        "caseId": case["id"], "condition": condition, "trial": trial,
        "answer": "", "score": 0, "missingGroups": [], "forbiddenMentions": [],
        "formatErrors": [], "executionErrors": [{"type": type(error).__name__, "detail": str(error)}],
        "threadId": None, "rawPath": None, "rawSha256": None, "promptSha256": None,
        "configurationSha256": None, "contextSha256": None, "usage": {}, "elapsedMs": None,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        raise RuntimeError("output must be a new directory; prior attempts are retained")

    design_path = ROOT / "boundary-protocol.json"
    protocol_path = ROOT / "protocol.json"
    corpus_path = ROOT / "corpus.json"
    design = json.loads(design_path.read_text(encoding="utf-8"))
    protocol = json.loads(protocol_path.read_text(encoding="utf-8"))
    corpus = json.loads(corpus_path.read_text(encoding="utf-8"))
    if tuple(design.get("conditions", ())) != CONDITIONS or design.get("totalContexts") != 22:
        raise ValueError("boundary protocol conditions or totalContexts mismatch")
    if {item.get("id"): item.get("trials") for item in design.get("cases", [])} != CASE_TRIALS:
        raise ValueError("boundary protocol case/trial matrix mismatch")
    cases = {case["id"]: case for episode in corpus["episodes"] for case in episode["cases"]}
    selected = set(CASE_TRIALS)
    replay_root = (ROOT.parent / "recall-fixes-2026-10-23" / "replay-01").resolve()
    originals = load_replay_contexts(replay_root, selected)
    prefixes = {body_and_prefix(originals[case_id])[0] for case_id in selected}
    if len(prefixes) != 1:
        raise ValueError("selected replay contexts do not share one exact prefix")
    prefix_with_lf = next(iter(prefixes))
    bodies = {case_id: body_and_prefix(originals[case_id])[1] for case_id in selected}
    policy_instruction = design["separatePolicyInstruction"]
    effective_instructions = {
        "data-labeled": protocol["instruction"],
        "separate-policy": protocol["instruction"] + policy_instruction + prefix_with_lf,
    }
    contexts = {
        case_id: {"data-labeled": originals[case_id], "separate-policy": bodies[case_id]}
        for case_id in selected
    }
    for case_id in selected:
        for condition in CONDITIONS:
            if len(contexts[case_id][condition].encode("utf-8")) > MAX_CONTEXT_BYTES:
                raise ValueError(f"{condition}/{case_id} context exceeds 600 bytes")
            if condition == "separate-policy" and contexts[case_id][condition] != bodies[case_id]:
                raise ValueError("separate-policy body changed")
    exe = Path(shutil.which("codex") or "")
    if not exe.is_file():
        raise RuntimeError("installed Codex CLI required; no API-key fallback")
    login = subprocess.run([str(exe), "login", "status"], env=environment(), capture_output=True, encoding="utf-8", timeout=30)
    if login.returncode or "chatgpt" not in (login.stdout + login.stderr).lower():
        raise RuntimeError("existing ChatGPT authentication required; no API-key fallback")
    config, config_sha = configuration(exe, protocol)
    client_before = sha(exe)
    output.mkdir()
    manifest = {
        "schemaVersion": "memory-boundary-diagnostic/v1", "status": "running", "startedAt": now(),
        "configuration": config, "configurationSha256": config_sha, "codexSha256": client_before,
        "codexVersion": subprocess.run([str(exe), "--version"], env=environment(), capture_output=True, encoding="utf-8", timeout=30).stdout.strip(),
        "clientSha256Before": client_before, "protocolSha256": sha(protocol_path), "corpusSha256": sha(corpus_path),
        "boundaryProtocolSha256": sha(design_path), "runnerSha256": sha(__file__),
        "answerRunnerSha256": sha(ROOT / "answer_runner.py"), "diagnoseLayoutSha256": sha(ROOT / "diagnose_layout.py"),
        "oldReplaySha256": sha(replay_root / "run.json"), "ordinaryChatGPTAuth": True, "paidApiCalls": 0,
        "contexts": contexts, "effectiveInstructions": effective_instructions, "records": [], "errors": [],
        "limits": {"maxContextBytes": MAX_CONTEXT_BYTES},
    }
    save(output / "run.json", manifest)
    jobs = [(trial, case_id) for case_id, max_trial in CASE_TRIALS.items() for trial in range(1, max_trial + 1)]

    def group(trial: int, case_id: str) -> list[dict]:
        ordered = list(CONDITIONS)
        if (trial + int(case_id[1:])) % 2:
            ordered.reverse()
        records = []
        for condition in ordered:
            condition_protocol = dict(protocol)
            condition_protocol["instruction"] = effective_instructions[condition]
            try:
                records.append(run_one(exe, condition_protocol, cases[case_id], condition, trial,
                                       contexts[case_id][condition], config_sha, output))
            except Exception as exc:
                records.append(failed_record(cases[case_id], condition, trial, exc))
        return records

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
    expected = {(case_id, trial, condition) for case_id, max_trial in CASE_TRIALS.items()
                for trial in range(1, max_trial + 1) for condition in CONDITIONS}
    actual = {(row.get("caseId"), row.get("trial"), row.get("condition")) for row in manifest["records"]}
    threads = [row.get("threadId") for row in manifest["records"]]
    if len(manifest["records"]) != 22 or actual != expected:
        manifest["errors"].append("boundary inventory mismatch")
    if len(threads) != 22 or None in threads or len(set(threads)) != 22:
        manifest["errors"].append("boundary threads are missing or duplicated")
    if any(row["executionErrors"] or row["formatErrors"] for row in manifest["records"]):
        manifest["errors"].append("execution or format errors recorded")
    manifest["clientSha256After"] = sha(exe)
    if manifest["clientSha256After"] != client_before:
        manifest["errors"].append("client executable changed during the diagnostic")
    manifest["status"] = "execution-valid" if not manifest["errors"] else "inconclusive"
    manifest["completedAt"] = now()
    save(output / "run.json", manifest)
    return 0 if manifest["status"] == "execution-valid" else 1


if __name__ == "__main__":
    raise SystemExit(main())
