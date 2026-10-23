"""Run two fresh, bounded answer repetitions for five frozen recall cases.

Output contract (``--output`` must not exist before a non-dry run)::

    output/
      run.json                 # execution manifest and per-case records
      trial-2/raw/q08_with.json
      trial-3/raw/q18_with.json

The two trials contain exactly q08, q09, q11, q17 and q18, each with arm
``with``.  Every invocation uses a new ephemeral Codex process and therefore
must produce a fresh thread ID.  ``run.json`` has ``status``
``execution-valid`` when transport and integrity checks pass, or
``inconclusive`` when they do not.  Lexical scores are retained as review
signals and never determine the execution status.

The runner deliberately requires the already completed replay and primary
campaign as inputs.  ``--dry-run`` validates their inventories and hashes
without looking up an executable, checking authentication, creating output,
or making model calls.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
from typing import Any

from answer_runner import configuration, environment, invoke
from grade import parse_stdout_details, reconstruct_prompt, score_answer
from run_replay import save, sha


ROOT = Path(__file__).resolve().parent
TARGET_CASES = ("q08", "q09", "q11", "q17", "q18")
TRIALS = ("trial-2", "trial-3")


def now() -> str:
    return datetime.now(timezone.utc).isoformat()


def load(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def fail(errors: list[str], message: str) -> None:
    errors.append(message)


def inputs(replay: Path, primary: Path) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any], dict[str, dict[str, Any]], dict[str, Any]]:
    """Load and validate the immutable replay, primary, protocol and corpus."""
    errors: list[str] = []
    protocol_path, corpus_path = ROOT / "protocol.json", ROOT / "corpus.json"
    protocol, corpus = load(protocol_path), load(corpus_path)
    cases = {case["id"]: case for episode in corpus["episodes"] for case in episode["cases"]}
    if set(TARGET_CASES) - set(cases):
        fail(errors, "target cases are absent from corpus")

    replay_manifest_path = replay / "run.json"
    primary_manifest_path = primary / "run.json"
    if not replay_manifest_path.is_file():
        fail(errors, "replay run.json is missing")
        replay_manifest: dict[str, Any] = {}
    else:
        replay_manifest = load(replay_manifest_path)
    if not primary_manifest_path.is_file():
        fail(errors, "primary run.json is missing")
        primary_manifest: dict[str, Any] = {}
    else:
        primary_manifest = load(primary_manifest_path)
    if replay_manifest.get("status") != "completed":
        fail(errors, "replay is not completed")
    if primary_manifest.get("status") != "completed":
        fail(errors, "primary is not completed")
    if primary_manifest.get("replayRunSha256") != sha(replay_manifest_path):
        fail(errors, "primary replay run hash does not match supplied replay")
    for label, manifest in (("replay", replay_manifest), ("primary", primary_manifest)):
        if manifest.get("corpusSha256") != sha(corpus_path):
            fail(errors, f"{label} corpus hash mismatch")
        if manifest.get("protocolSha256") != sha(protocol_path):
            fail(errors, f"{label} protocol hash mismatch")

    packets: dict[str, dict[str, Any]] = {}
    for episode in corpus["episodes"]:
        episode_run = replay / episode["id"] / "run.json"
        if not episode_run.is_file():
            fail(errors, f"replay episode missing: {episode['id']}")
            continue
        report = load(episode_run)
        if report.get("status") != "completed":
            fail(errors, f"replay episode is not completed: {episode['id']}")
        for stage in report.get("stages", []):
            packet = stage.get("packet")
            if packet:
                packets[stage["caseId"]] = packet
    if set(packets) != set(cases):
        fail(errors, "replay packet inventory differs from corpus")
    for case_id in TARGET_CASES:
        packet = packets.get(case_id)
        if not packet or packet.get("response", {}).get("status") != "ok":
            fail(errors, f"target replay packet unavailable: {case_id}")

    expected_pairs = {(case_id, arm) for case_id in cases for arm in ("with", "without")}
    primary_raw = sorted((primary / "raw").glob("*.json"))
    seen_pairs: set[tuple[str, str]] = set()
    primary_threads: set[str] = set()
    primary_target_threads: set[str] = set()
    if len(primary_raw) != 48:
        fail(errors, "primary must contain exactly 48 raw answers")
    for path in primary_raw:
        raw = load(path)
        pair = (raw.get("caseId"), raw.get("arm"))
        seen_pairs.add(pair)
        if pair not in expected_pairs:
            fail(errors, f"unexpected primary pair: {pair[0]}/{pair[1]}")
        if raw.get("valid") is not True or raw.get("exitCode") != 0:
            fail(errors, f"primary answer is not valid: {path.name}")
        thread = raw.get("threadId")
        if isinstance(thread, str):
            primary_threads.add(thread)
            if raw.get("caseId") in TARGET_CASES and raw.get("arm") == "with":
                primary_target_threads.add(thread)
    if seen_pairs != expected_pairs:
        fail(errors, "primary answer inventory does not equal all case/arm pairs")
    if len(primary_threads) != 48:
        fail(errors, "primary thread IDs are not all distinct")
    if len(primary_target_threads) != len(TARGET_CASES):
        fail(errors, "primary target thread inventory is incomplete")
    return protocol, corpus, primary_manifest, packets, {"errors": errors, "primaryThreads": primary_threads,
                                                         "primaryTargetThreads": primary_target_threads}


def verify_raw(raw_path: Path, case: dict[str, Any], packet: dict[str, Any], protocol: dict[str, Any], manifest: dict[str, Any], primary_threads: set[str]) -> dict[str, Any]:
    """Validate one saved invocation and return its review record."""
    raw = load(raw_path)
    answer, thread, usage, errors = parse_stdout_details(raw.get("stdout", ""))
    if raw.get("answer") != answer:
        errors.append("raw answer differs from final JSONL message")
    if raw.get("threadId") != thread:
        errors.append("raw threadId differs from JSONL thread id")
    if raw.get("usage") != usage:
        errors.append("raw usage differs from JSONL usage")
    if raw.get("valid") is not True:
        errors.append("valid must be true")
    if raw.get("exitCode") != 0:
        errors.append("exitCode must be zero")
    if raw.get("caseId") != case["id"] or raw.get("arm") != "with":
        errors.append("raw case or arm differs from requested repetition")
    if not thread or thread in primary_threads:
        errors.append("thread ID is missing or reused from primary")
    expected_prompt = reconstruct_prompt(protocol, case, "with", packet)
    if raw.get("promptSha256") != hashlib.sha256(expected_prompt.encode()).hexdigest():
        errors.append("prompt hash mismatch")
    if raw.get("command") != manifest.get("configuration", {}).get("argv"):
        errors.append("command differs from manifest configuration argv")
    if raw.get("configurationSha256") != manifest.get("configurationSha256"):
        errors.append("configuration hash mismatch")
    scored = score_answer(answer, case, execution_errors=errors)
    return {
        "caseId": case["id"], "trial": raw.get("trial"), "arm": raw.get("arm"),
        "question": case["question"], "kind": case.get("kind"), "answer": answer,
        "threadId": thread, "rawPath": str(raw_path.relative_to(manifest["output"])),
        "rawSha256": sha(raw_path), "scoreLexical": scored["score"],
        "missingGroups": scored["missingGroups"], "forbiddenMentions": scored["forbiddenMentions"],
        "executionErrors": scored["executionErrors"], "formatErrors": scored["formatErrors"],
        "usage": usage, "elapsedMs": raw.get("elapsedMs"),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--replay", type=Path, required=True)
    parser.add_argument("--primary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    replay, primary, output = (p.resolve() for p in (args.replay, args.primary, args.output))
    protocol, corpus, primary_manifest, packets, checks = inputs(replay, primary)
    errors = checks["errors"]
    if output.exists():
        errors.append("output must be a new directory")
    if args.dry_run:
        print(json.dumps({"status": "execution-valid" if not errors else "inconclusive", "errors": errors,
                          "targetCases": list(TARGET_CASES), "trials": list(TRIALS), "modelCalls": 0}, ensure_ascii=False))
        return 0 if not errors else 1
    if errors:
        print(json.dumps({"status": "inconclusive", "errors": errors}, ensure_ascii=False))
        return 1

    exe = Path(shutil.which("codex") or "")
    if not exe.is_file():
        raise RuntimeError("Installed Codex CLI unavailable")
    login = subprocess.run([str(exe), "login", "status"], capture_output=True, encoding="utf-8",
                           env=environment(), timeout=30)
    if login.returncode != 0 or "chatgpt" not in (login.stdout + login.stderr).lower():
        raise RuntimeError("Existing ChatGPT authentication required; never use API-key fallback")
    config, config_sha = configuration(exe, protocol)
    if config_sha != primary_manifest.get("configurationSha256"):
        raise RuntimeError("Client configuration differs from primary; do not mix trials")
    if primary_manifest.get("runnerSha256") != sha(ROOT / "answer_runner.py"):
        raise RuntimeError("Answer runner changed after primary")
    output.mkdir()
    for trial in TRIALS:
        (output / trial).mkdir()
    manifest: dict[str, Any] = {
        "schemaVersion": "memory-realistic-repetitions/v1", "status": "running", "startedAt": now(),
        "completedAt": None, "output": str(output), "model": protocol["model"],
        "reasoningEffort": protocol["reasoningEffort"], "configuration": config,
        "configurationSha256": config_sha, "codexSha256": sha(exe), "codexVersion": subprocess.run(
            [str(exe), "--version"], capture_output=True, encoding="utf-8", timeout=30).stdout.strip(),
        "corpusSha256": sha(ROOT / "corpus.json"), "protocolSha256": sha(ROOT / "protocol.json"),
        "replayRunSha256": sha(replay / "run.json"), "primaryRunSha256": sha(primary / "run.json"),
        "runnerSha256": sha(ROOT / "answer_runner.py"), "selfSha256": sha(__file__),
        "params": {"cases": list(TARGET_CASES), "trials": list(TRIALS), "arm": "with", "maxWorkers": 3,
                   "timeoutSeconds": 240, "freshContexts": 10},
        "primaryTargetThreadIds": sorted(checks["primaryTargetThreads"]),
        "paidApiCalls": 0, "records": [], "errors": [],
    }
    # Keep the public output field available to verify_raw without exposing it in packets.
    manifest["output"] = output
    cases = {case["id"]: case for episode in corpus["episodes"] for case in episode["cases"]}
    jobs = [(trial, case_id) for trial in TRIALS for case_id in TARGET_CASES]
    with ThreadPoolExecutor(max_workers=3) as pool:
        futures = {}
        for trial, case_id in jobs:
            trial_output = output / trial
            packet = packets[case_id]
            context = packet["response"]["data"]["additionalContext"]
            futures[pool.submit(invoke, exe, protocol, trial_output, case_id, "with",
                                 cases[case_id]["question"],
                                 context, config_sha)] = (trial, case_id)
        for future in as_completed(futures):
            trial, case_id = futures[future]
            try:
                future.result()
            except Exception as exc:
                manifest["errors"].append(f"{trial}/{case_id}: {type(exc).__name__}: {exc}")
    for trial, case_id in jobs:
        raw_path = output / trial / "raw" / f"{case_id}_with.json"
        if not raw_path.is_file():
            manifest["errors"].append(f"missing raw output: {trial}/{case_id}")
            continue
        record = verify_raw(raw_path, cases[case_id], packets[case_id], protocol, manifest, checks["primaryThreads"])
        record["trial"] = trial
        manifest["records"].append(record)
    manifest["records"].sort(key=lambda record: (record["trial"], record["caseId"]))
    threads = [r["threadId"] for r in manifest["records"] if r["threadId"]]
    if len(manifest["records"]) != 10:
        manifest["errors"].append("expected exactly 10 repetition records")
    if len(threads) != len(set(threads)):
        manifest["errors"].append("repetition thread IDs are not all distinct")
    all_target_threads = set(checks["primaryTargetThreads"]) | set(threads)
    manifest["threadInventory"] = {"primaryTargetCount": len(checks["primaryTargetThreads"]),
                                    "repetitionCount": len(threads), "distinctCount": len(all_target_threads),
                                    "expectedDistinctCount": 15}
    if len(all_target_threads) != 15:
        manifest["errors"].append("primary and repetition target threads are not 15 distinct IDs")
    if any(record["executionErrors"] for record in manifest["records"]):
        manifest["errors"].append("one or more repetition records failed integrity checks")
    manifest["output"] = "<output>"
    manifest["completedAt"] = now()
    manifest["status"] = "execution-valid" if not manifest["errors"] else "inconclusive"
    save(output / "run.json", manifest)
    return 0 if manifest["status"] == "execution-valid" else 1


if __name__ == "__main__":
    raise SystemExit(main())
