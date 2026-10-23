"""Generic replay/answer/repeat qualification runner for frozen corpora."""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

from answer_runner import command, configuration, environment, invoke, now, save, sha
from grade import parse_stdout_details, score_answer
from run_replay import episode, validate_protocol


CASE_RE = re.compile(r"^[qh][0-9]+$")
NAMESPACES = {"qual-a", "qual-b"}
ROOT = Path(__file__).resolve().parent


def load_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def validate_corpus(corpus: dict) -> list[dict]:
    episodes = corpus.get("episodes")
    if not isinstance(episodes, list) or not episodes:
        raise ValueError("corpus must contain non-empty episodes")
    seen: set[str] = set()
    episode_ids: set[str] = set()
    cases: list[dict] = []
    for item in episodes:
        episode_id = item.get("id") if isinstance(item, dict) else None
        if not isinstance(episode_id, str) or not re.fullmatch(r"^[a-z][a-z0-9-]*$", episode_id) or episode_id in episode_ids:
            raise ValueError(f"invalid or duplicate episode id: {episode_id}")
        episode_ids.add(episode_id)
        if not isinstance(item, dict) or not item.get("cases"):
            raise ValueError("each corpus episode must have an id and non-empty cases")
        for case in item["cases"]:
            case_id = case.get("id")
            if not isinstance(case_id, str) or not CASE_RE.fullmatch(case_id) or case_id in seen:
                raise ValueError(f"invalid or duplicate case id: {case_id}")
            if case.get("agent") not in NAMESPACES:
                raise ValueError(f"case {case_id} has invalid namespace")
            if not case.get("question") or not isinstance(case.get("actions"), list):
                raise ValueError(f"case {case_id} is incomplete")
            for action in case["actions"]:
                if action.get("agent") not in NAMESPACES:
                    raise ValueError(f"case {case_id} action has invalid namespace")
            seen.add(case_id)
            cases.append(case)
    return cases


def campaign_hashes(root: Path, corpus_path: Path, protocol_path: Path, replay_path: Path | None = None) -> dict[str, str]:
    hashes = {
        "protocolSha256": sha(protocol_path),
        "corpusSha256": sha(corpus_path),
        "runnerSha256": sha(root / "qualification_runner.py"),
        "answerRunnerSha256": sha(root / "answer_runner.py"),
        "runReplaySha256": sha(root / "run_replay.py"),
        "gradeSha256": sha(root / "grade.py"),
        "rpcSupportSha256": sha(root / "rpc_support.py"),
    }
    if replay_path is not None:
        hashes["replayRunSha256"] = sha(replay_path)
    return hashes


def replay_contexts(replay: Path, corpus: dict, protocol_path: Path, corpus_path: Path) -> tuple[dict[str, str], dict]:
    report = load_json(replay / "run.json")
    if report.get("status") != "completed":
        raise ValueError("replay is not completed")
    if report.get("protocolSha256") != sha(protocol_path) or report.get("corpusSha256") != sha(corpus_path):
        raise ValueError("replay protocol/corpus hashes do not match requested inputs")
    expected_episodes = {item["id"]: item for item in corpus["episodes"]}
    declared = report.get("episodes")
    if not isinstance(declared, list) or {item.get("id") for item in declared} != set(expected_episodes) or len(declared) != len(expected_episodes):
        raise ValueError("replay episode manifest inventory mismatch")
    for item in declared:
        episode_dir = replay / item["id"]
        episode_path = episode_dir / "run.json"
        if item.get("status") != "completed" or item.get("stageCount") != len(expected_episodes[item["id"]]["cases"]):
            raise ValueError(f"replay episode manifest incomplete: {item['id']}")
        if item.get("reportSha256") != sha(episode_path):
            raise ValueError(f"replay episode report hash mismatch: {item['id']}")
    contexts: dict[str, str] = {}
    for episode_spec in corpus["episodes"]:
        episode_report = load_json(replay / episode_spec["id"] / "run.json")
        if episode_report.get("status") != "completed" or len(episode_report.get("stages", [])) != len(episode_spec["cases"]):
            raise ValueError(f"replay episode incomplete: {episode_spec['id']}")
        episode_ids = [stage.get("caseId") for stage in episode_report["stages"]]
        declared_ids = [case["id"] for case in episode_spec["cases"]]
        if set(episode_ids) != set(declared_ids) or len(episode_ids) != len(set(episode_ids)):
            raise ValueError(f"replay case inventory mismatch: {episode_spec['id']}")
        for stage in episode_report["stages"]:
            packet = stage.get("packet", {})
            contexts[stage["caseId"]] = packet["response"]["data"]["additionalContext"]
    if set(contexts) != {case["id"] for episode in corpus["episodes"] for case in episode["cases"]}:
        raise ValueError("replay contexts contain duplicate or missing case IDs")
    return contexts, report


def record_answer(exe: Path, protocol: dict, case: dict, arm: str, context: str,
                  config_sha: str, destination: Path, output_root: Path) -> dict:
    destination.mkdir(parents=True, exist_ok=True)
    invoke(exe, protocol, destination, case["id"], arm, case["question"], context, config_sha)
    raw_path = destination / "raw" / f'{case["id"]}_{arm}.json'
    raw = load_json(raw_path)
    answer, thread, usage, parse_errors = parse_stdout_details(raw.get("stdout", ""))
    execution_errors = list(parse_errors)
    expected_command = command("<codex>", "<scratch-case>", protocol)
    if raw.get("caseId") != case["id"]:
        execution_errors.append({"type": "case-id-mismatch"})
    if raw.get("arm") != arm:
        execution_errors.append({"type": "arm-mismatch"})
    if raw.get("command") != expected_command:
        execution_errors.append({"type": "command-mismatch"})
    if raw.get("exitCode") != 0:
        execution_errors.append({"type": "process-exit", "exitCode": raw.get("exitCode")})
    if not raw.get("valid") or raw.get("answer") != answer:
        execution_errors.append({"type": "invalid-recorded-output"})
    prompt = protocol["instruction"] + "\n\nDonnées historiques :\n" + context + "\n\nQuestion actuelle :\n" + case["question"]
    prompt_sha = hashlib.sha256(prompt.encode()).hexdigest()
    if raw.get("promptSha256") != prompt_sha:
        execution_errors.append({"type": "prompt-hash-mismatch"})
    if raw.get("configurationSha256") != config_sha:
        execution_errors.append({"type": "configuration-hash-mismatch"})
    scored = score_answer(answer, case, execution_errors=execution_errors)
    return {
        "caseId": case["id"], "arm": arm, "answer": answer, "score": scored["score"],
        "missingGroups": scored["missingGroups"], "forbiddenMentions": scored["forbiddenMentions"],
        "formatErrors": scored["formatErrors"], "executionErrors": scored["executionErrors"],
        "threadId": thread, "rawPath": raw_path.relative_to(output_root).as_posix(),
        "rawSha256": sha(raw_path), "promptSha256": prompt_sha,
        "configurationSha256": config_sha, "contextSha256": hashlib.sha256(context.encode()).hexdigest(),
        "usage": usage, "elapsedMs": raw.get("elapsedMs"),
    }


def validate_records(records: list[dict], expected_keys: set[tuple]) -> list[str]:
    errors: list[str] = []
    actual = {(r.get("caseId"), r.get("arm", "with"), r.get("trial", 1)) for r in records}
    if actual != expected_keys or len(records) != len(expected_keys):
        errors.append("result inventory mismatch")
    threads = [r.get("threadId") for r in records]
    if None in threads or len(set(threads)) != len(expected_keys):
        errors.append("thread IDs are missing or not globally unique")
    if any(r.get("executionErrors") or r.get("formatErrors") for r in records):
        errors.append("execution or format errors recorded")
    return errors


def parse_trials(value: str) -> list[int]:
    values = [part.strip() for part in value.split(",") if part.strip()]
    if not values:
        raise ValueError("--trials must contain one or more integers")
    trials = [int(part) for part in values]
    if any(trial < 1 for trial in trials) or len(set(trials)) != len(trials):
        raise ValueError("--trials must contain unique integers >= 1")
    return sorted(trials)


def failed_record(case: dict, arm: str, trial: int | None, error: Exception) -> dict:
    return {"caseId": case["id"], "arm": arm, "trial": trial or 1, "answer": "", "score": 0,
            "missingGroups": [], "forbiddenMentions": [], "formatErrors": [],
            "executionErrors": [{"type": type(error).__name__, "detail": str(error)}],
            "threadId": None, "rawPath": None, "rawSha256": None, "promptSha256": None,
            "configurationSha256": None, "contextSha256": None, "usage": {}, "elapsedMs": None}


def run_replay(args, root: Path, corpus: dict, protocol: dict, corpus_path: Path, protocol_path: Path) -> int:
    validate_protocol(protocol)
    cli, mcp = args.cli.resolve(), args.mcp.resolve()
    before = {"cliSha256": sha(cli), "mcpSha256": sha(mcp)}
    args.output.mkdir()
    start = now()
    manifest = {"schemaVersion": "memory-generic-replay/v1", "startedAt": start, "status": "running",
                "sourceRevision": protocol["sourceRevision"], **before, "corpusSha256": sha(corpus_path),
                "protocolSha256": sha(protocol_path), **campaign_hashes(root, corpus_path, protocol_path),
                "episodes": []}
    save(args.output / "run.json", manifest)
    episodes = []
    for item in corpus["episodes"]:
        report = episode(protocol, cli, mcp, item, args.output.resolve())
        report_path = args.output / item["id"] / "run.json"
        episodes.append({"id": item["id"], "status": report["status"], "stageCount": len(report["stages"]),
                         "reportSha256": sha(report_path), "error": report.get("error")})
        manifest["episodes"] = episodes
        save(args.output / "run.json", manifest)
    after = {"cliSha256": sha(cli), "mcpSha256": sha(mcp)}
    expected = {item["id"]: len(item["cases"]) for item in corpus["episodes"]}
    integrity = all(e["status"] == "completed" and e["stageCount"] == expected[e["id"]] for e in episodes) and before == after
    result = {**manifest, "completedAt": now(), "cliSha256After": after["cliSha256"],
              "mcpSha256After": after["mcpSha256"], "episodes": episodes,
              "status": "completed" if integrity else "failed"}
    save(args.output / "run.json", result)
    return 0 if integrity else 1


def run_answers(args, root: Path, corpus: dict, protocol: dict, corpus_path: Path, protocol_path: Path) -> int:
    replay = args.replay.resolve()
    contexts, replay_report = replay_contexts(replay, corpus, protocol_path, corpus_path)
    exe = Path(shutil.which("codex") or "")
    if not exe.is_file():
        raise RuntimeError("installed Codex CLI unavailable")
    login = subprocess.run([str(exe), "login", "status"], env=environment(), capture_output=True, encoding="utf-8", timeout=30)
    if login.returncode or "chatgpt" not in (login.stdout + login.stderr).lower():
        raise RuntimeError("existing ChatGPT authentication required; no API-key fallback")
    config, config_sha = configuration(exe, protocol)
    client_before = sha(exe)
    version = subprocess.run([str(exe), "--version"], env=environment(), capture_output=True, encoding="utf-8", timeout=30).stdout.strip()
    args.output.mkdir()
    manifest = {"schemaVersion": "memory-generic-answers/v1", "status": "running", "startedAt": now(),
                **campaign_hashes(root, corpus_path, protocol_path, replay / "run.json"), "configuration": config,
                "configurationSha256": config_sha, "codexSha256": client_before, "codexVersion": version,
                "clientSha256Before": client_before, "ordinaryChatGPTAuth": True,
                "paidApiCalls": 0, "records": [], "errors": []}
    save(args.output / "run.json", manifest)
    cases = validate_corpus(corpus)
    def group(case: dict) -> list[dict]:
        arms = ("without", "with") if int(case["id"][1:]) % 2 else ("with", "without")
        records = []
        for arm in arms:
            try:
                records.append(record_answer(exe, protocol, case, arm, contexts[case["id"]] if arm == "with" else "", config_sha, args.output, args.output))
            except Exception as exc:
                records.append(failed_record(case, arm, 1, exc))
        return records
    with ThreadPoolExecutor(max_workers=3) as pool:
        futures = [pool.submit(group, case) for case in cases]
        for future in as_completed(futures):
            try:
                manifest["records"].extend(future.result())
            except Exception as exc:
                manifest["errors"].append({"type": type(exc).__name__, "detail": str(exc)})
            save(args.output / "run.json", manifest)
    expected = {(case["id"], arm, 1) for case in cases for arm in ("with", "without")}
    manifest["errors"].extend(validate_records(manifest["records"], expected))
    manifest["clientSha256After"] = sha(exe)
    if manifest["clientSha256After"] != client_before:
        manifest["errors"].append("client executable changed during the campaign")
    manifest["status"] = "execution-valid" if not manifest["errors"] else "inconclusive"
    manifest["completedAt"] = now()
    save(args.output / "run.json", manifest)
    return 0 if manifest["status"] == "execution-valid" else 1


def run_repeats(args, root: Path, corpus: dict, protocol: dict, corpus_path: Path, protocol_path: Path) -> int:
    contexts, replay_report = replay_contexts(args.replay.resolve(), corpus, protocol_path, corpus_path)
    requested_cases = [value.strip() for value in args.cases.split(",") if value.strip()]
    if len(set(requested_cases)) != len(requested_cases):
        raise ValueError("--cases must not contain duplicates")
    selected = set(requested_cases)
    cases = [case for case in validate_corpus(corpus) if case["id"] in selected]
    if len(cases) != len(selected) or not selected:
        raise ValueError("--cases must name existing unique case IDs")
    trials = parse_trials(args.trials)
    exe = Path(shutil.which("codex") or "")
    if not exe.is_file():
        raise RuntimeError("installed Codex CLI unavailable")
    login = subprocess.run([str(exe), "login", "status"], env=environment(), capture_output=True, encoding="utf-8", timeout=30)
    if login.returncode or "chatgpt" not in (login.stdout + login.stderr).lower():
        raise RuntimeError("existing ChatGPT authentication required; no API-key fallback")
    config, config_sha = configuration(exe, protocol)
    client_before = sha(exe)
    version = subprocess.run([str(exe), "--version"], env=environment(), capture_output=True, encoding="utf-8", timeout=30).stdout.strip()
    args.output.mkdir()
    replay_path = args.replay.resolve() / "run.json"
    manifest = {"schemaVersion": "memory-generic-repeats/v1", "status": "running", "startedAt": now(),
                **campaign_hashes(root, corpus_path, protocol_path, replay_path), "configuration": config,
                "configurationSha256": config_sha, "codexSha256": client_before, "codexVersion": version,
                "clientSha256Before": client_before, "ordinaryChatGPTAuth": True,
                "paidApiCalls": 0, "cases": sorted(selected), "trials": trials, "records": [], "errors": []}
    save(args.output / "run.json", manifest)
    jobs = [(trial, case) for trial in trials for case in cases]
    def group(trial: int, case: dict) -> list[dict]:
        destination = args.output / f"trial-{trial}"
        destination.mkdir(parents=True, exist_ok=True)
        try:
            return [dict(record_answer(exe, protocol, case, "with", contexts[case["id"]], config_sha, destination, args.output), trial=trial)]
        except Exception as exc:
            return [failed_record(case, "with", trial, exc)]
    with ThreadPoolExecutor(max_workers=3) as pool:
        futures = [pool.submit(group, trial, case) for trial, case in jobs]
        for future in as_completed(futures):
            try:
                manifest["records"].extend(future.result())
            except Exception as exc:
                manifest["errors"].append({"type": type(exc).__name__, "detail": str(exc)})
            save(args.output / "run.json", manifest)
    expected = {(case["id"], "with", trial) for case in cases for trial in trials}
    manifest["errors"].extend(validate_records(manifest["records"], expected))
    manifest["clientSha256After"] = sha(exe)
    if manifest["clientSha256After"] != client_before:
        manifest["errors"].append("client executable changed during the campaign")
    manifest["status"] = "execution-valid" if not manifest["errors"] else "inconclusive"
    manifest["completedAt"] = now()
    save(args.output / "run.json", manifest)
    return 0 if manifest["status"] == "execution-valid" else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("replay", "answers", "repeats"))
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--protocol", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cli", type=Path)
    parser.add_argument("--mcp", type=Path)
    parser.add_argument("--replay", type=Path)
    parser.add_argument("--cases", default="")
    parser.add_argument("--trials", default="1")
    args = parser.parse_args()
    if args.output.exists():
        raise RuntimeError("output must be new; prior attempts are retained")
    root = ROOT
    corpus_path, protocol_path = args.corpus.resolve(), args.protocol.resolve()
    corpus, protocol = load_json(corpus_path), load_json(protocol_path)
    validate_corpus(corpus)
    validate_protocol(protocol)
    if args.action == "replay":
        if not args.cli or not args.mcp:
            parser.error("replay requires --cli and --mcp")
        return run_replay(args, root, corpus, protocol, corpus_path, protocol_path)
    if not args.replay:
        parser.error(f"{args.action} requires --replay")
    if args.action == "answers":
        return run_answers(args, root, corpus, protocol, corpus_path, protocol_path)
    if not args.cases:
        parser.error("repeats requires --cases")
    return run_repeats(args, root, corpus, protocol, corpus_path, protocol_path)


if __name__ == "__main__":
    raise SystemExit(main())
