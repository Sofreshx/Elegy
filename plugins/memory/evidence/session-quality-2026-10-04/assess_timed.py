"""Deterministically assess the completed timed memory answer run offline."""

from __future__ import annotations

import argparse
import hashlib
import json
import statistics
from pathlib import Path


ARMS = ("with", "without")
USAGE_FIELDS = (
    "input_tokens",
    "cached_input_tokens",
    "cache_write_input_tokens",
    "output_tokens",
    "reasoning_output_tokens",
)


def load(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def normalize(value: str) -> str:
    value = value.strip().upper()
    return value[:-1] if value.endswith(".") else value


def fail(message: str) -> None:
    raise ValueError(message)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def parse_stdout_events(raw: dict, path: Path) -> list[dict]:
    events = []
    for line in raw.get("stdout", "").splitlines():
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError as exc:
            fail(f"{path}: invalid JSON event: {exc}")
    require(events, f"{path}: missing stdout events")
    allowed = {"thread.started", "turn.started", "item.completed", "turn.completed"}
    require(all(event.get("type") in allowed for event in events), f"{path}: tool event present")
    require(
        not any(
            event.get("type") == "item.completed"
            and event.get("item", {}).get("type") != "agent_message"
            for event in events
        ),
        f"{path}: non-agent item present",
    )
    return events


def validate_raw(raw: dict, case_id: str, arm: str, run: dict, path: Path) -> dict:
    require(raw.get("schemaVersion") == "memory-timed-answer-packet/v1", f"{path}: schema")
    require(raw.get("caseId") == case_id and raw.get("arm") == arm, f"{path}: case/arm")
    require(raw.get("valid") is True, f"{path}: invalid packet")
    require(raw.get("exitCode") == 0, f"{path}: non-zero exit")
    require(isinstance(raw.get("answer"), str), f"{path}: missing answer")
    require(raw.get("issues") == [], f"{path}: packet issues")
    require(raw.get("elapsedMs", 0) >= 0, f"{path}: invalid elapsedMs")
    usage = raw.get("usage")
    require(isinstance(usage, dict), f"{path}: missing usage")
    require(all(isinstance(usage.get(field), int) and usage[field] >= 0 for field in USAGE_FIELDS), f"{path}: invalid usage")
    require(raw.get("configArgHash") == run["configArgHash"], f"{path}: config hash differs")
    command = raw.get("command")
    require(isinstance(command, list), f"{path}: missing command")
    require(run["model"] in command, f"{path}: model differs")
    events = parse_stdout_events(raw, path)
    messages = [event["item"]["text"] for event in events if event.get("type") == "item.completed" and event.get("item", {}).get("type") == "agent_message"]
    completions = [event.get("usage") for event in events if event.get("type") == "turn.completed"]
    thread_ids = [event.get("thread_id") for event in events if event.get("type") == "thread.started"]
    require(len(messages) == 1 and messages[0] == raw["answer"], f"{path}: parsed agent message differs from answer")
    require(len(completions) == 1 and completions[0] == usage, f"{path}: turn usage differs from packet usage")
    metadata_id = raw.get("metadata", {}).get("ids", {}).get("thread_id")
    require(metadata_id is not None, f"{path}: missing raw session id")
    require(len(thread_ids) == 1 and thread_ids[0], f"{path}: missing stdout session id")
    require(metadata_id == hashlib.sha256(thread_ids[0].encode()).hexdigest()[:16], f"{path}: session id digest mismatch")
    return {"raw": raw, "metadataSessionId": metadata_id, "stdoutSessionId": thread_ids[0]}


def normalized_command(command: list[str]) -> list[str]:
    normalized = list(command)
    require(normalized, "empty command")
    normalized[0] = "<executable>"
    try:
        index = normalized.index("-C")
    except ValueError:
        fail("command missing -C")
    require(index + 1 < len(normalized), "command missing -C value")
    normalized[index + 1] = "<dedicated-runtime>"
    return normalized


def summarize(values: list[float | int]) -> dict:
    return {
        "count": len(values),
        "mean": statistics.fmean(values),
        "median": statistics.median(values),
        "total": sum(values),
    }


def assess(root: Path) -> dict:
    corpus_path = root / "corpus.json"
    protocol_path = root / "protocol.json"
    repeat_path = root / "timed-repeat-protocol.json"
    run_path = root / "timed-run-01" / "run.json"
    answers_path = root / "timed-run-01" / "answers.json"
    source_packets_path = root / "source-run-01" / "packets.json"
    raw_dir = root / "timed-run-01" / "raw"
    timed_script_path = root / "timed_answers.py"

    corpus, protocol, repeat, run, answers_doc = map(load, (corpus_path, protocol_path, repeat_path, run_path, answers_path))
    source_packets = {packet["caseId"]: packet for packet in load(source_packets_path)}
    require(len(corpus["cases"]) == 12, "corpus must contain 12 cases")
    require(run["status"] == "execution-completed", "timed run is not complete")
    require(run.get("failures") == [], "timed run reports failures")
    require(answers_doc["answers"] == run["answers"], "answers.json differs from run.json")
    expected_keys = {f"{case['id']}_{arm}" for case in corpus["cases"] for arm in ARMS}
    require(set(answers_doc["answers"]) == expected_keys and len(expected_keys) == 24, "answer inventory is not complete")

    input_hashes = {
        "corpus.json": sha256(corpus_path),
        "protocol.json": sha256(protocol_path),
        "timed-repeat-protocol.json": sha256(repeat_path),
        "source-run-01/packets.json": sha256(source_packets_path),
        "timed-run-01/run.json": sha256(run_path),
        "timed-run-01/answers.json": sha256(answers_path),
    }
    require(run["corpusSha256"] == input_hashes["corpus.json"], "frozen corpus hash mismatch")
    require(run["protocolSha256"] == input_hashes["protocol.json"], "frozen protocol hash mismatch")
    require(run["sourcePacketsSha256"] == input_hashes["source-run-01/packets.json"], "frozen source packet hash mismatch")
    require(run["scriptSha256"] == sha256(timed_script_path), "timed harness script hash mismatch")
    require(run["model"] == protocol["answerExperiment"]["model"] == repeat["model"], "model is inconsistent")
    require(run["reasoningEffort"] == protocol["answerExperiment"]["reasoningEffort"] == repeat["effort"], "reasoning effort is inconsistent")
    require(answers_doc["timingLimit"] == "spawn-to-reported-completion wall-clock interval", "timing limit changed")
    expected_command = ["<executable>", "exec", "--ignore-user-config", "--ephemeral", "--sandbox", "read-only", "--skip-git-repo-check", "--json", "-C", "<dedicated-runtime>", "-m", run["model"], "-c", 'model_reasoning_effort="medium"', "-c", "project_doc_max_bytes=0", "-"]

    packet_meta = {}
    session_ids = set()
    raw_packet_hashes = {}
    rows = []
    for case in corpus["cases"]:
        row = {"id": case["id"], "question": case["question"], "expected": case["expected"], "arms": {}}
        for arm in ARMS:
            key = f"{case['id']}_{arm}"
            raw_path = raw_dir / f"{key}.json"
            require(raw_path.exists(), f"missing raw packet: {key}")
            raw_packet_hashes[f"timed-run-01/raw/{key}.json"] = sha256(raw_path)
            raw_info = validate_raw(load(raw_path), case["id"], arm, run, raw_path)
            raw = raw_info["raw"]
            require(normalized_command(raw["command"]) == expected_command, f"{key}: command shape differs")
            context = source_packets[case["id"]]["response"]["data"].get("additionalContext", "") if arm == "with" else ""
            prompt = protocol["answerExperiment"]["instruction"] + "\n\nHistorical data:\n" + context + "\n\nQuestion:\n" + case["question"]
            require(raw["inputSha256"] == hashlib.sha256(prompt.encode()).hexdigest(), f"{key}: reconstructed prompt hash differs")
            for source_name, source in (("run", run["answers"]), ("answers", answers_doc["answers"])):
                require(
                    all(raw.get(field) == source[key].get(field) for field in source[key]),
                    f"{key}: {source_name} answer differs from raw packet",
                )
            session_id = raw_info["stdoutSessionId"]
            require(session_id not in session_ids, f"duplicate raw session id: {key}")
            session_ids.add(session_id)
            forbidden = [token for token in case.get("forbidden", []) if token.upper() in raw["answer"].upper()]
            row["arms"][arm] = {
                "actual": raw["answer"],
                "expected": case["expected"],
                "score": int(normalize(raw["answer"]) == normalize(case["expected"])),
                "forbiddenMatches": forbidden,
                "elapsedMs": raw["elapsedMs"],
                "usage": raw["usage"],
                "valid": raw["valid"],
                "exitCode": raw["exitCode"],
                "issues": raw["issues"],
                "rawPacket": f"timed-run-01/raw/{key}.json",
            }
        rows.append(row)

    counts = {arm: sum(row["arms"][arm]["score"] for row in rows) for arm in ARMS}
    timings = {arm: summarize([row["arms"][arm]["elapsedMs"] for row in rows]) for arm in ARMS}
    tokens = {
        arm: {field: summarize([row["arms"][arm]["usage"][field] for row in rows]) for field in USAGE_FIELDS}
        for arm in ARMS
    }
    errors = [
        {"caseId": row["id"], "arm": arm, "expected": row["expected"], "actual": row["arms"][arm]["actual"], "reason": "exact-match failure"}
        for row in rows for arm in ARMS if row["arms"][arm]["score"] == 0
    ]
    return {
        "schemaVersion": "memory-timed-assessment/v1",
        "status": "valid",
        "inventory": {"cases": len(rows), "arms": len(rows) * 2, "allValid": True},
        "inputs": input_hashes,
        "scriptSha256": sha256(timed_script_path),
        "rawPacketHashes": raw_packet_hashes,
        "frozenInputChecks": {"corpus": True, "protocol": True, "sourcePackets": True},
        "configuration": {"model": run["model"], "reasoningEffort": run["reasoningEffort"], "configArgHash": run["configArgHash"], "noToolsObserved": True, "distinctRawSessionIds": True},
        "scores": {"correctWith": counts["with"], "correctWithout": counts["without"], "meanPairedDifference": (counts["with"] - counts["without"]) / len(rows), "forbiddenAnswerCount": sum(bool(row["arms"][arm]["forbiddenMatches"]) for row in rows for arm in ARMS)},
        "timing": {"definition": "whole-process wall-clock spawn through reported JSON completion; includes setup, scheduling and network; not isolated inference latency", "byArm": timings},
        "tokens": {"definition": "raw provider usage fields reported by each process; descriptive only and not a billing or inference-latency measure", "byArm": tokens},
        "errors": errors,
        "cases": rows,
        "limits": ["One repeated synthetic corpus and one timed realization per case/arm; no holdout or statistical generalization.", "Whole-process elapsedMs includes authentication, setup, scheduling and network; it cannot establish inference latency or causal performance.", "Raw token fields are reported usage, not monetary cost or cost savings; paid API calls were not introduced.", "A model alias does not prove an immutable backend version; no savings, population effect, significance, or automatic hook activation is claimed."],
        "constraints": {"offline": True, "noModelCalls": True, "noToolsInChildOutput": True, "paidApiCalls": 0, "evaluatorMutations": "none; recorded experiment scope is taken from frozen run evidence"},
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, default=Path(__file__).parent)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = assess(args.directory)
    encoded = json.dumps(result, indent=2, ensure_ascii=False) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded)
