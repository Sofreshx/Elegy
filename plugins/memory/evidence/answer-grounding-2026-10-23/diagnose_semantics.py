"""Diagnostic of semantic-prefix effects over retained replay contexts.

This is an evidence probe only. It changes the envelope prefix and preserves
the JSON body byte-for-byte; it emits no product recommendation or pass flag.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import json
from pathlib import Path
import shutil
import subprocess

from answer_runner import ROOT, configuration, environment, now, save, sha
from diagnose_layout import CASE_TRIALS, load_replay_contexts, run_one


MAX_CONTEXT_BYTES = 600


def replace_prefix(original: str, prefix: str) -> str:
    separator = original.find("\n")
    if separator < 0:
        raise ValueError("replay context has no prefix/body LF separator")
    body = original[separator + 1:]
    if not prefix.endswith("\n") or "\n" in prefix[:-1]:
        raise ValueError("semantic prefix must end with exactly one LF")
    context = prefix + body
    if context.count("\n", 0, len(prefix)) != 1:
        raise ValueError("semantic context does not have exactly one prefix LF")
    if json.loads(context.split("\n", 1)[1]) != json.loads(body):
        raise ValueError("semantic prefix replacement changed JSON meaning")
    if context[context.find("\n") + 1:] != body:
        raise ValueError("semantic prefix replacement changed JSON body bytes")
    if len(context.encode("utf-8")) > MAX_CONTEXT_BYTES:
        raise ValueError("semantic context exceeds 600 bytes")
    return context


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        raise RuntimeError("output must be a new directory; prior attempts are retained")

    design = json.loads((ROOT / "semantics-protocol.json").read_text(encoding="utf-8"))
    protocol = json.loads((ROOT / "protocol.json").read_text(encoding="utf-8"))
    corpus = json.loads((ROOT / "corpus.json").read_text(encoding="utf-8"))
    cases = {case["id"]: case for episode in corpus["episodes"] for case in episode["cases"]}
    declared_conditions = design.get("conditions")
    if not isinstance(declared_conditions, dict) or not declared_conditions:
        raise ValueError("semantics protocol must declare a non-empty condition mapping")
    conditions = [{"name": name, "prefix": prefix} for name, prefix in declared_conditions.items()]
    condition_names = [item.get("name") for item in conditions]
    if any(not isinstance(name, str) or not name for name in condition_names) or len(set(condition_names)) != len(condition_names):
        raise ValueError("semantic condition names must be unique non-empty strings")
    if design.get("totalContexts") != sum(CASE_TRIALS.values()) * len(condition_names):
        raise ValueError("semantics protocol totalContexts does not match declared matrix")
    selected = set(CASE_TRIALS)
    if {item.get("id"): item.get("trials") for item in design.get("cases", [])} != CASE_TRIALS:
        raise ValueError("semantics protocol cases do not match the fixed diagnostic cases")
    replay_root = (ROOT.parent / "recall-fixes-2026-10-23" / "replay-01").resolve()
    originals = load_replay_contexts(replay_root, selected)
    contexts = {
        case_id: {item["name"]: replace_prefix(originals[case_id], item["prefix"])
                  for item in conditions}
        for case_id in selected
    }
    exe = Path(shutil.which("codex") or "")
    if not exe.is_file():
        raise RuntimeError("installed Codex CLI required; no API-key fallback")
    login = subprocess.run([str(exe), "login", "status"], env=environment(), capture_output=True, encoding="utf-8", timeout=30)
    if login.returncode or "chatgpt" not in (login.stdout + login.stderr).lower():
        raise RuntimeError("existing ChatGPT authentication required; no API-key fallback")
    config, config_sha = configuration(exe, protocol)
    output.mkdir()
    manifest = {
        "schemaVersion": "memory-semantic-prefix-diagnostic/v1",
        "status": "running",
        "startedAt": now(),
        "configuration": config,
        "configurationSha256": config_sha,
        "codexSha256": sha(exe),
        "protocolSha256": sha(ROOT / "protocol.json"),
        "corpusSha256": sha(ROOT / "corpus.json"),
        "semanticsProtocolSha256": sha(ROOT / "semantics-protocol.json"),
        "runnerSha256": sha(__file__),
        "answerRunnerSha256": sha(ROOT / "answer_runner.py"),
        "diagnoseLayoutSha256": sha(ROOT / "diagnose_layout.py"),
        "oldReplaySha256": sha(replay_root / "run.json"),
        "ordinaryChatGPTAuth": True,
        "paidApiCalls": 0,
        "contexts": contexts,
        "records": [],
        "errors": [],
        "limits": {"maxContextBytes": MAX_CONTEXT_BYTES},
    }
    save(output / "run.json", manifest)
    jobs = [(trial, case_id) for case_id, max_trial in CASE_TRIALS.items() for trial in range(1, max_trial + 1)]

    def group(trial: int, case_id: str) -> list[dict]:
        ordered = list(conditions)
        offset = (trial + int(case_id[1:])) % len(ordered)
        ordered = ordered[offset:] + ordered[:offset]
        return [run_one(exe, protocol, cases[case_id], item["name"], trial,
                        contexts[case_id][item["name"]], config_sha, output)
                for item in ordered]

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
    expected = sum(CASE_TRIALS.values()) * len(condition_names)
    expected_keys = {(case_id, trial, name)
                     for case_id, max_trial in CASE_TRIALS.items()
                     for trial in range(1, max_trial + 1)
                     for name in condition_names}
    actual_keys = {(row.get("caseId"), row.get("trial"), row.get("condition"))
                   for row in manifest["records"]}
    threads = [row.get("threadId") for row in manifest["records"]]
    if len(manifest["records"]) != expected or actual_keys != expected_keys:
        manifest["errors"].append("semantic diagnostic inventory is incomplete or duplicated")
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
