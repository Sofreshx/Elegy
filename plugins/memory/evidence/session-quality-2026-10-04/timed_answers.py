"""Run the frozen paired native Codex answer experiment in fresh processes."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
from typing import Any


MODEL = "gpt-5.6-luna"
REASONING = "medium"
MAX_WORKERS = 3


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def redact(value: Any, paths: list[str]) -> Any:
    if isinstance(value, str):
        result = value
        for path in paths:
            if path:
                result = result.replace(path, "<local-path>")
        return result
    if isinstance(value, dict):
        return {key: redact(child, paths) for key, child in value.items()}
    if isinstance(value, list):
        return [redact(child, paths) for child in value]
    return value


def child_environment() -> dict[str, str]:
    blocked = {
        "ELEGY_DB_PATH", "OPENAI_BASE_URL", "OPENAI_API_BASE", "OPENAI_URL",
        "ELEGY_OPENAI_URL", "ELEGY_LLM_URL", "ELEGY_EMBEDDING_URL",
        "ELEGY_PROVIDER_ENDPOINT", "ELEGY_PROVIDER_URL", "OLLAMA_URL", "OLLAMA_HOST",
    }
    return {
        key: value for key, value in os.environ.items()
        if not key.endswith("_API_KEY") and key not in blocked
        and not (key.endswith(("_ENDPOINT", "_URL", "_BASE_URL", "_API_BASE"))
                 and (key.startswith("ELEGY_") or "PROVIDER" in key or "OPENAI" in key or "ANTHROPIC" in key or "OLLAMA" in key))
    }


def parse_jsonl(stdout: str) -> tuple[str, dict[str, Any], list[dict[str, Any]], list[str]]:
    events: list[dict[str, Any]] = []
    malformed: list[str] = []
    for line in stdout.splitlines():
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            malformed.append(line[:500])
            continue
        if isinstance(event, dict):
            events.append(event)
    messages: list[str] = []
    usage: dict[str, Any] = {}
    tool_calls: list[dict[str, Any]] = []
    errors: list[str] = []
    ids: dict[str, Any] = {}
    for event in events:
        kind = str(event.get("type", "")).lower()
        item = event.get("item") if isinstance(event.get("item"), dict) else event
        item_kind = str(item.get("type", "")).lower() if isinstance(item, dict) else ""
        if ("tool" in kind or "tool" in item_kind
                or (kind.startswith("item.") and item_kind not in {"agent_message", "assistant_message", "reasoning"})):
            tool_calls.append(event)
        if kind == "error" or item_kind == "error" or event.get("error"):
            errors.append(str(event.get("error") or item.get("message") or event))
        if kind == "item.completed" and item_kind in {"agent_message", "assistant_message"}:
            text = item.get("text")
            if isinstance(text, str):
                messages.append(text)
        if isinstance(event.get("usage"), dict):
            usage = event["usage"]
        for key in ("thread_id", "threadId", "session_id", "sessionId"):
            if key in event and isinstance(event[key], str):
                ids[key] = sha256_bytes(event[key].encode())[:16]
    if len(messages) == 1 and not malformed and not tool_calls and not errors:
        answer = messages[0]
    else:
        answer = ""
    return answer, {"events": len(events), "ids": ids}, usage, tool_calls + errors + malformed


def run_case(executable: str, runtime: Path, case: dict[str, Any], arm: str,
             instruction: str, context: str, output: Path, arg_hash: str) -> dict[str, Any]:
    case_id = case["id"]
    label = f"{case_id}_{arm}"
    case_runtime = runtime / label
    case_runtime.mkdir(parents=True, exist_ok=True)
    prompt = instruction + "\n\nHistorical data:\n" + (context if arm == "with" else "") + "\n\nQuestion:\n" + case["question"]
    command = [executable, "exec", "--ignore-user-config", "--ephemeral", "--sandbox", "read-only",
               "--skip-git-repo-check", "--json", "-C", str(case_runtime), "-m", MODEL,
               "-c", 'model_reasoning_effort="medium"', "-c", "project_doc_max_bytes=0", "-"]
    started_wall = time.time()
    started_perf = time.perf_counter()
    proc = subprocess.run(command, input=prompt, text=True, capture_output=True,
                          env=child_environment(), check=False)
    elapsed_ms = round((time.perf_counter() - started_perf) * 1000, 3)
    completed_wall = time.time()
    answer, metadata, usage, issues = parse_jsonl(proc.stdout)
    valid = proc.returncode == 0 and not issues and bool(answer)
    packet = {
        "schemaVersion": "memory-timed-answer-packet/v1", "caseId": case_id, "arm": arm,
        "valid": valid, "exitCode": proc.returncode, "startedAt": round(started_wall * 1000, 3),
        "observedCompletedAt": round(completed_wall * 1000, 3), "elapsedMs": elapsed_ms,
        "answer": answer, "usage": usage, "metadata": metadata, "issues": issues,
        "stdout": redact(proc.stdout, [str(runtime), str(output), str(Path.home())]),
        "stderr": redact(proc.stderr, [str(runtime), str(output), str(Path.home())]),
        "command": redact(command, [str(runtime), str(output), str(Path.home())]),
        "inputSha256": sha256_bytes(prompt.encode()), "configArgHash": arg_hash,
        "timestamp": utc_now(),
    }
    raw_path = output / "raw" / f"{label}.json"
    raw_path.parent.mkdir(parents=True, exist_ok=True)
    raw_path.write_text(json.dumps(packet, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return {key: packet[key] for key in ("valid", "answer", "startedAt", "observedCompletedAt", "elapsedMs", "exitCode", "usage", "issues", "inputSha256")}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path, help="new output directory")
    parser.add_argument("--directory", type=Path, default=Path(__file__).parent)
    args = parser.parse_args()
    root = args.directory.resolve()
    output = args.output.resolve()
    if output.exists():
        raise ValueError("--output must name a new directory")
    executable = shutil.which("codex")
    if not executable:
        raise RuntimeError("codex executable was not found on PATH")
    login = subprocess.run([executable, "login", "status"], capture_output=True, text=True, check=False)
    if login.returncode != 0 or "chatgpt" not in (login.stdout + login.stderr).lower():
        raise RuntimeError("codex login status did not report ChatGPT")
    corpus_path, protocol_path = root / "corpus.json", root / "protocol.json"
    source_packets = root / "source-run-01" / "packets.json"
    corpus = json.loads(corpus_path.read_text(encoding="utf-8"))
    protocol = json.loads(protocol_path.read_text(encoding="utf-8"))
    source = {packet["caseId"]: packet for packet in json.loads(source_packets.read_text(encoding="utf-8"))}
    instruction = protocol["answerExperiment"]["instruction"]
    cases = corpus["cases"]
    if len(cases) != 12 or any(case["id"] not in source for case in cases):
        raise ValueError("frozen corpus/source packets must contain all 12 cases")
    output.mkdir(parents=True)
    runtime = output / "runtime"
    runtime.mkdir()
    arg_shape = ["codex", "exec", "--ignore-user-config", "--ephemeral", "--sandbox", "read-only", "--skip-git-repo-check", "--json", "-C", "<dedicated-runtime>", "-m", MODEL, "-c", 'model_reasoning_effort="medium"', "-c", "project_doc_max_bytes=0", "-"]
    arg_hash = sha256_bytes(json.dumps(arg_shape, separators=(",", ":")).encode())
    run_doc: dict[str, Any] = {
        "schemaVersion": "memory-timed-answers/v1", "status": "running", "synthetic": True,
        "createdAt": utc_now(), "model": MODEL, "reasoningEffort": REASONING,
        "maxWorkers": MAX_WORKERS, "counterbalance": "odd without then with; even with then without",
        "corpusSha256": sha256_file(corpus_path), "protocolSha256": sha256_file(protocol_path),
        "sourcePacketsSha256": sha256_file(source_packets), "scriptSha256": sha256_file(Path(__file__).resolve()),
        "codex": {"path": "<codex>", "sha256": sha256_file(Path(executable).resolve()), "version": subprocess.run([executable, "--version"], capture_output=True, text=True, check=False).stdout.strip()},
        "configArgHash": arg_hash, "answers": {}, "failures": [],
    }
    (output / "run.json").write_text(json.dumps(run_doc, indent=2) + "\n", encoding="utf-8")

    def pair(case: dict[str, Any]) -> dict[str, Any]:
        order = ("without", "with") if int(case["id"][1:]) % 2 else ("with", "without")
        result: dict[str, Any] = {}
        for arm in order:
            context = source[case["id"]]["response"]["data"].get("additionalContext", "")
            label = f"{case['id']}_{arm}"
            try:
                result[label] = run_case(executable, runtime, case, arm, instruction, context, output, arg_hash)
            except Exception as exc:
                failure = {"valid": False, "answer": "", "startedAt": None, "observedCompletedAt": None,
                           "elapsedMs": None, "exitCode": None, "usage": {}, "issues": [type(exc).__name__ + ": " + str(exc)[:500]],
                           "inputSha256": sha256_bytes((instruction + "\n\nQuestion:\n" + case["question"]).encode())}
                result[label] = failure
                raw_path = output / "raw" / f"{label}.json"
                raw_path.parent.mkdir(parents=True, exist_ok=True)
                raw_path.write_text(json.dumps({"schemaVersion": "memory-timed-answer-packet/v1", "caseId": case["id"], "arm": arm, **failure}, indent=2) + "\n", encoding="utf-8")
        return result

    with ThreadPoolExecutor(max_workers=MAX_WORKERS) as pool:
        futures = [pool.submit(pair, case) for case in cases]
        for future in as_completed(futures):
            result = future.result()
            run_doc["answers"].update(result)
            run_doc["failures"] = sorted(key for key, row in run_doc["answers"].items() if not row["valid"])
            (output / "run.json").write_text(json.dumps(run_doc, indent=2) + "\n", encoding="utf-8")
    run_doc["status"] = "execution-completed" if len(run_doc["answers"]) == 24 and not run_doc["failures"] else "execution-failed"
    run_doc["completedAt"] = utc_now()
    (output / "answers.json").write_text(json.dumps({"schemaVersion": "memory-timed-answers/v1", "answers": run_doc["answers"], "timingLimit": "spawn-to-reported-completion wall-clock interval"}, indent=2) + "\n", encoding="utf-8")
    (output / "run.json").write_text(json.dumps(run_doc, indent=2) + "\n", encoding="utf-8")
    return 0 if run_doc["status"] == "execution-completed" else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"timed answer harness failed: {exc}", file=sys.stderr)
        raise SystemExit(2)
