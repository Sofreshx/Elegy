"""Offline assessment of completed Q3 qualification evidence."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys
from collections import Counter
from statistics import mean

ROOT = Path(__file__).resolve().parent
Q2 = ROOT.parent / "answer-grounding-2026-10-23"
sys.path.insert(0, str(Q2))
from answer_runner import command
from grade import parse_stdout_details, score_answer
from qualification_runner import campaign_hashes, replay_contexts, validate_corpus

SUBJECT = ROOT / "subject.json"
PROTOCOL = ROOT / "protocol.json"
CLI_SHA = "4f6fdaa80b0c8809af8b217a786232b0d71cfa30f5130f0fc1323e1513cae833"
MCP_SHA = "d93610c224af1549708bc7f3fff2b51b9b4601bc4bfc480fdf0588748ba86f93"
CLIENT_SHA = "00c0f1e4099246fddcd67ec620ca8ed50827c7d70e113b7224f7a2933cf2d67c"
CASES = ("q08", "q09", "q11", "q17", "q18")
MODEL_PHASES = ("answers-primary", "answers-transfer", "repeats")
# q03 contains an explicit current port but its transport exists only in memory:
# the baseline must apply the port (controlPass) and abstain on that transport.
COMPLETE_WITHOUT_KINDS = {"current-only", "irrelevant", "unknown", "hostile", "privacy", "forgetting"}

def sha(value: bytes | Path) -> str:
    return hashlib.sha256(value.read_bytes() if isinstance(value, Path) else value).hexdigest()

def load(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))

def expected_records(phase: str, corpus: dict) -> set[tuple[str, str, int]]:
    cases = [case["id"] for episode in corpus["episodes"] for case in episode["cases"]]
    if phase == "answers-primary": return {(case, arm, 1) for case in cases for arm in ("with", "without")}
    if phase == "answers-transfer": return {(case, arm, 1) for case in cases for arm in ("with", "without")}
    if phase == "repeats": return {(case, "with", trial) for case in CASES for trial in (2, 3)}
    raise ValueError(f"unknown model phase: {phase}")

def corpus_path(phase: str) -> Path:
    return ROOT / ("transfer-corpus.json" if phase.endswith("transfer") else "corpus.json")

def replay_name(phase: str) -> str:
    return "replay-transfer" if phase.endswith("transfer") else "replay-primary"

def expected_manifest_bindings(phase: str, protocol: dict, subject: dict) -> dict:
    replay = None if phase.startswith("replay-") else ROOT / replay_name(phase) / "run.json"
    expected = campaign_hashes(Q2, corpus_path(phase), PROTOCOL, replay)
    if phase.startswith("replay-"):
        expected.update(sourceRevision=subject["sourceRevision"], cliSha256=CLI_SHA, cliSha256After=CLI_SHA,
                        mcpSha256=MCP_SHA, mcpSha256After=MCP_SHA, status="completed")
    else:
        config = {"argv": command("<codex>", "<scratch-case>", protocol), "executableSha256": CLIENT_SHA,
                  "environment": "API keys/endpoints/Memory bindings removed; existing ChatGPT login only"}
        expected.update(configuration=config, configurationSha256=sha(json.dumps(config, sort_keys=True, separators=(",", ":")).encode()),
                        codexSha256=CLIENT_SHA, clientSha256Before=CLIENT_SHA, clientSha256After=CLIENT_SHA,
                        codexVersion=subject["codexVersion"], ordinaryChatGPTAuth=True, paidApiCalls=0,
                        status="execution-valid", errors=[])
        if phase == "repeats": expected.update(cases=list(CASES), trials=[2, 3])
    return expected

def check_manifest_bindings(phase: str, manifest: dict, protocol: dict, subject: dict, errors: list[str]) -> None:
    for key, expected in expected_manifest_bindings(phase, protocol, subject).items():
        if manifest.get(key) != expected: errors.append(f"{phase}: manifest {key}")

def check_wrapper(phase: str, wrapper: dict, errors: list[str]) -> None:
    expected = {"phase": phase, "status": "completed", "exitCode": 0, "subjectSha256": sha(SUBJECT),
                "protocolSha256": sha(PROTOCOL), "selfSha256Before": sha(ROOT / "campaign.py"),
                "selfSha256After": sha(ROOT / "campaign.py")}
    for key, value in expected.items():
        if wrapper.get(key) != value: errors.append(f"{phase}: wrapper {key}")
    required = {"sourceRevision", "fixedSourceRevision", "sourceLockSha256", "cliSourceSha256", "fixedCliSourceSha256",
                "subjectBinaryHashes", "protocolSha256", "model", "reasoningEffort", "corpusProvenance", "sourceFiles",
                "sourceFileCount", "binaries", "installedClient", "clientVersionRecorded", "harnessManifest",
                "harnessSourceFiles", "pathsInsideRepo", "all"}
    for when in ("before", "after"):
        result = wrapper.get(when, {}); checks = result.get("checks", {})
        if any(checks.get(name) is not True for name in required): errors.append(f"{phase}: {when} preflight checks")
        if checks.get("sourceFileErrors") != [] or checks.get("harnessSourceErrors") != []: errors.append(f"{phase}: {when} source errors")
        for name in ("subjectSha256", "protocolSha256"):
            if result.get(name) != expected[name]: errors.append(f"{phase}: {when} {name}")
        for key, value in (("cli", CLI_SHA), ("mcp", MCP_SHA)):
            binary = result.get("binaries", {}).get(key, {})
            if binary.get("sha256") != value or binary.get("expected") != value or binary.get("insideRepo") is not True:
                errors.append(f"{phase}: {when} binary {key}")

def review_index(review: dict, errors: list[str]) -> dict:
    items = review.get("records")
    if not isinstance(items, list): errors.append("semantic-review records missing"); return {}
    out = {}
    for item in items:
        key = (item.get("phase"), item.get("caseId"), item.get("arm"), item.get("trial", 1))
        if type(key[3]) is not int: errors.append(f"invalid prose trial {key}")
        if key in out: errors.append(f"duplicate prose review {key}")
        for field in ("grounded", "taskComplete", "controlPass", "formatPass"):
            if not isinstance(item.get(field), bool): errors.append(f"missing prose flag {key}:{field}")
        if not isinstance(item.get("reason"), str) or not item["reason"]: errors.append(f"missing prose reason {key}")
        out[key] = item
    if len(out) != 82: errors.append(f"prose review count {len(out)} != 82")
    return out

def apply_review(indexed: dict, key: tuple, case: dict, failed: list[dict]) -> None:
    item = indexed.get(key)
    if item is None:
        failed.append({"key": key, "reason": "missing prose review"}); return
    required = ["grounded", "controlPass", "formatPass"]
    if key[2] == "with": required.append("taskComplete")
    elif case.get("kind") in COMPLETE_WITHOUT_KINDS:
        required.append("taskComplete")
    if not all(item.get(flag) is True for flag in required):
        failed.append({"key": key, "reason": "prose gate failed", "flags": {flag: item.get(flag) for flag in required}})

def check_review_binding(item: dict, record: dict, key: tuple, errors: list[str]) -> None:
    if item.get("rawSha256") != record.get("rawSha256") or item.get("answerSha256") != sha(record["answer"].encode()):
        errors.append(f"{key}: prose evidence hash")

def check_raw(record: dict, raw: dict, case: dict, context: str, config: dict, config_sha: str, errors: list[str]) -> None:
    key = (record.get("caseId"), record.get("arm", "with"), record.get("trial", 1))
    answer, thread, usage, parse_errors = parse_stdout_details(raw.get("stdout", ""))
    if parse_errors or raw.get("exitCode") != 0 or raw.get("valid") is not True or raw.get("issues"): errors.append(f"{key}: invalid execution/JSONL")
    if raw.get("answer") != answer or raw.get("threadId") != thread or raw.get("usage") != usage: errors.append(f"{key}: raw projection")
    if record.get("answer") != answer or record.get("threadId") != thread or record.get("usage") != usage: errors.append(f"{key}: record projection")
    if raw.get("caseId") != case["id"] or raw.get("arm") != record.get("arm", "with"): errors.append(f"{key}: raw identity")
    if raw.get("command") != command("<codex>", "<scratch-case>", config["protocol"]): errors.append(f"{key}: command")
    if raw.get("configurationSha256") != config_sha: errors.append(f"{key}: configuration hash")
    if record.get("configurationSha256") != config_sha: errors.append(f"{key}: record configuration hash")
    if record.get("elapsedMs") != raw.get("elapsedMs"): errors.append(f"{key}: elapsed time projection")
    prompt = config["protocol"]["instruction"] + "\n\nDonnées historiques :\n" + context + "\n\nQuestion actuelle :\n" + case["question"]
    prompt_sha = sha(prompt.encode())
    if raw.get("promptSha256") != prompt_sha or record.get("promptSha256") != prompt_sha: errors.append(f"{key}: prompt hash")
    if record.get("contextSha256") != sha(context.encode()): errors.append(f"{key}: context hash")
    scored = score_answer(answer, case, [], [])
    for field in ("score", "missingGroups", "forbiddenMentions", "formatErrors", "executionErrors"):
        if record.get(field) != scored.get(field): errors.append(f"{key}: grade {field}")

def check_replay(phase: str, corpus: dict, protocol: dict, errors: list[str]) -> None:
    contexts, _ = replay_contexts(ROOT / phase, corpus, PROTOCOL, corpus_path(phase))
    sessions = set()
    for episode in corpus["episodes"]:
        report = load(ROOT / phase / episode["id"] / "run.json")
        clean = report.get("cleanup", [])
        if (len(clean) != 2 or {c.get("agent") for c in clean} != {"qual-a", "qual-b"}
                or any(c.get("verifiedEmpty") is not True for c in clean)):
            errors.append(f"{phase}:{episode['id']}: cleanup")
        mappings = {}
        for case, stage in zip(episode["cases"], report["stages"]):
            case_id = case["id"]; prefix = f"{phase}:{case_id}"
            if stage.get("caseId") != case_id: errors.append(f"{prefix}: stage order")
            actions = stage.get("actions", [])
            if len(actions) != len(case["actions"]): errors.append(f"{prefix}: action count")
            for expected, observed in zip(case["actions"], actions):
                key, op = expected["key"], expected["op"]
                result = observed.get("result", {})
                if observed.get("key") != key or observed.get("op") != op or result.get("namespace") != expected["agent"]:
                    errors.append(f"{prefix}: action identity")
                if op in ("store", "correct"):
                    memory = result.get("memory", {})
                    if memory.get("content") != expected["text"] or (op == "correct" and memory.get("id") != mappings[key]["id"]):
                        errors.append(f"{prefix}: stored/corrected content")
                    mappings[key] = {"id": memory.get("id"), "text": expected["text"], "agent": expected["agent"], "deleted": False}
                else:
                    if result.get("deleted") is not True or result.get("id") != mappings[key]["id"]: errors.append(f"{prefix}: deletion")
                    mappings[key]["deleted"] = True
            own = {m["id"]: m for m in mappings.values() if m["agent"] == case["agent"] and not m["deleted"]}
            foreign = {m["id"] for m in mappings.values() if m["agent"] != case["agent"] and not m["deleted"]}
            deleted = [m for m in mappings.values() if m["agent"] == case["agent"] and m["deleted"]]
            checks = stage.get("checks", [])
            required = Counter({"fresh-mcp-id-and-search": int(bool(own)), "foreign-memory-id-search-list-absent": len(foreign),
                                "deleted-memory-absent": len(deleted)})
            actual = Counter(c.get("name") for c in checks)
            if actual != +required or any(c.get("passed") is not True for c in checks): errors.append(f"{prefix}: controls")
            if {c.get("id") for c in checks if c.get("name") == "foreign-memory-id-search-list-absent"} != foreign:
                errors.append(f"{prefix}: foreign ID controls")
            packet = stage["packet"]; data = packet["response"]["data"]; text = contexts[case_id]
            if packet.get("sourceUnchanged") is not True or packet.get("exitCode") != 0 or packet["response"].get("status") != "ok": errors.append(f"{prefix}: recall execution")
            if packet["sourceBefore"]["durableData"] != packet["sourceAfter"]["durableData"]: errors.append(f"{prefix}: durable bytes changed")
            request = packet["request"]; config = packet["configuration"]
            if request.get("prompt") != case["question"] or request.get("recentContext") != [] or request.get("turnId") != case_id: errors.append(f"{prefix}: request")
            session = request.get("sessionId")
            if not session or session in sessions: errors.append(f"{prefix}: recall session identity")
            sessions.add(session)
            if (config.get("agentId") != case["agent"] or config.get("scopes") != ["Agent"] or config.get("mode") != "inject"
                    or config.get("learningEnabled") is not False or config.get("includeRecentContext") is not False
                    or config.get("maxContextTokens") != protocol["memory"]["maxContextTokens"]
                    or config.get("minSimilarity") != protocol["memory"]["minSimilarity"]): errors.append(f"{prefix}: recall configuration")
            if len(text.encode()) > protocol["memory"]["maxContextTokens"] or data.get("budgetTokensUpperBound") != len(text.encode()): errors.append(f"{prefix}: byte budget")
            for recalled in data["recalls"]:
                if recalled.get("id") not in own or recalled.get("scope") != "Agent" or recalled.get("content") != own.get(recalled.get("id"), {}).get("text"):
                    errors.append(f"{prefix}: foreign/deleted/incorrect recalled content")
            if text:
                body = json.loads(text.split("\n", 1)[1])
                if {r["id"] for r in body["records"]} != {r["id"] for r in data["recalls"]}: errors.append(f"{prefix}: envelope IDs")
                if any(r["text"] != own.get(r["id"], {}).get("text") for r in body["records"]): errors.append(f"{prefix}: envelope content")
            elif data["recalls"]: errors.append(f"{prefix}: missing envelope")
    for case_id, token in (("q12", "OVERRIDE_GARNET"), ("h06", "OVERRIDE_CERULEAN")):
        if case_id in contexts and token not in contexts[case_id]: errors.append(f"{phase}:{case_id}: hostile note not exposed")
    for case_id in ("q24", "h12"):
        if case_id in contexts and contexts[case_id] != "": errors.append(f"{phase}:{case_id}: deleted note exposed")
    if "q15" in contexts and "SECRET_PEPPER_924" in contexts["q15"]: errors.append(f"{phase}: foreign secret exposed")

def assess_phase(phase: str, corpus: dict, protocol: dict, subject: dict, review: dict, errors: list[str], failed: list[dict], threads: set[str], rows: list[dict]) -> int:
    check_wrapper(phase, load(ROOT / f"{phase}-wrapper.json"), errors)
    manifest = load(ROOT / phase / "run.json")
    check_manifest_bindings(phase, manifest, protocol, subject, errors)
    if phase.startswith("replay-"):
        check_replay(phase, corpus, protocol, errors)
        return 0
    expected = expected_records(phase, corpus); records = manifest.get("records", [])
    actual = {(r.get("caseId"), r.get("arm", "with"), r.get("trial", 1)) for r in records}
    if actual != expected or len(records) != len(expected): errors.append(f"{phase}: record inventory")
    cases = {case["id"]: case for episode in corpus["episodes"] for case in episode["cases"]}
    contexts, _ = replay_contexts(ROOT / replay_name(phase), corpus, PROTOCOL, corpus_path(phase))
    config = {"protocol": protocol}
    config_sha = manifest.get("configurationSha256")
    threads_local = set()
    for record in records:
        key = (record.get("caseId"), record.get("arm", "with"), record.get("trial", 1))
        if key not in expected or key[0] not in cases: continue
        expected_path = (f"trial-{key[2]}/" if phase == "repeats" else "") + f"raw/{key[0]}_{key[1]}.json"
        if record.get("rawPath") != expected_path: errors.append(f"{phase}:{key}: raw path"); continue
        path = (ROOT / phase / expected_path).resolve()
        if (ROOT / phase).resolve() not in path.parents: errors.append(f"{phase}:{key}: raw path escapes"); continue
        if not path.is_file() or sha(path) != record.get("rawSha256"): errors.append(f"{phase}:{key}: raw hash"); continue
        raw = load(path); context = contexts[key[0]] if key[1] == "with" else ""
        config["protocol"] = protocol
        check_raw(record, raw, cases[key[0]], context, config, config_sha, errors)
        review_item = review.get((phase, key[0], key[1], key[2]))
        if review_item:
            check_review_binding(review_item, record, (phase, *key), errors)
        apply_review(review, (phase, key[0], key[1], key[2]), cases[key[0]], failed)
        if record.get("forbiddenMentions") or record.get("formatErrors") or record.get("executionErrors"):
            failed.append({"key": (phase, *key), "reason": "automated control/format/execution violation"})
        thread = record.get("threadId")
        if not isinstance(thread, str) or not thread or thread in threads or thread in threads_local: errors.append(f"{phase}:{key}: missing/duplicate thread")
        threads_local.add(thread); threads.add(thread)
        rows.append({"phase": phase, **record, "trial": key[2], "semantic": review_item})
    declared_paths = {record.get("rawPath") for record in records}
    actual_paths = {p.relative_to(ROOT / phase).as_posix() for p in (ROOT / phase).glob("trial-*/raw/*.json" if phase == "repeats" else "raw/*.json")}
    if actual_paths != declared_paths: errors.append(f"{phase}: raw file inventory")
    return len(records)

def paired_metrics(rows: list[dict]) -> dict:
    pairs = {}
    for row in rows:
        if row["phase"] in ("answers-primary", "answers-transfer"):
            pairs.setdefault((row["phase"], row["caseId"]), {})[row["arm"]] = row
    complete = [pair for pair in pairs.values() if set(pair) == {"with", "without"}]
    def average(values): return mean(values) if values else None
    return {"pairCount": len(complete),
            "meanWithRecall": average([p["with"]["score"] for p in complete]),
            "meanWithoutRecall": average([p["without"]["score"] for p in complete]),
            "meanPairedDelta": average([p["with"]["score"] - p["without"]["score"] for p in complete]),
            "completeWithRecall": sum(all(p["with"].get("semantic", {}).get(f) is True for f in ("grounded", "taskComplete", "controlPass", "formatPass")) for p in complete),
            "completeWithoutRecall": sum(all(p["without"].get("semantic", {}).get(f) is True for f in ("grounded", "taskComplete", "controlPass", "formatPass")) for p in complete),
            "meanWholeProcessMsWith": average([p["with"]["elapsedMs"] for p in complete]),
            "meanWholeProcessMsWithout": average([p["without"]["elapsedMs"] for p in complete])}

def main(argv=None) -> int:
    parser = argparse.ArgumentParser(); parser.add_argument("--output", type=Path, required=True); args = parser.parse_args(argv)
    if args.output.exists(): raise RuntimeError("output must be new")
    errors: list[str] = []; failed: list[dict] = []; threads: set[str] = set(); rows = []
    try:
        subject, protocol = load(SUBJECT), load(PROTOCOL); corpus = load(ROOT / "corpus.json"); transfer = load(ROOT / "transfer-corpus.json")
        if (subject["cliSha256"] != CLI_SHA or subject["mcpSha256"] != MCP_SHA or subject["codexSha256"] != CLIENT_SHA
                or subject["protocolSha256"] != sha(PROTOCOL) or subject["model"] != protocol["model"]
                or protocol["model"] != "gpt-6.1-sol" or subject["reasoningEffort"] != protocol["reasoningEffort"] or protocol["reasoningEffort"] != "max"):
            errors.append("subject/protocol binding")
        lock = ROOT / subject["sourceProof"]
        if sha(lock) != subject["sourceLockSha256"] or load(lock)["productSourceRevision"] != subject["sourceRevision"]:
            errors.append("source proof binding")
        if sha(Q2 / "evidence-manifest.json") != subject["harnessManifestSha256"] or any(sha(Q2 / p) != digest for p, digest in subject["harnessSourceFiles"].items()):
            errors.append("harness proof binding")
        validate_corpus(corpus); validate_corpus(transfer)
        review = review_index(load(ROOT / "semantic-review.json"), errors)
        expected_review = set()
        for phase, data in (("answers-primary", corpus), ("answers-transfer", transfer)):
            for key in expected_records(phase, data): expected_review.add((phase, key[0], key[1], key[2]))
        for key in expected_records("repeats", corpus): expected_review.add(("repeats", key[0], key[1], key[2]))
        if set(review) != expected_review: errors.append("semantic review inventory mismatch")
        for phase, data in (("replay-primary", corpus), ("replay-transfer", transfer), ("answers-primary", corpus), ("answers-transfer", transfer), ("repeats", corpus)):
            assess_phase(phase, data, protocol, subject, review, errors, failed, threads, rows)
        if len(threads) != 82: errors.append(f"thread inventory {len(threads)} != 82")
    except Exception as exc: errors.append(f"harness exception: {exc}")
    metrics = paired_metrics(rows) if not errors else {}
    if not errors and metrics.get("pairCount") != 36: errors.append("paired inventory mismatch")
    result = {"schemaVersion": "memory-codex-assessment/v1", "status": "blocked-evidence" if errors else "quality-failed" if failed else "qualified-selected-configuration", "evidenceConsistent": not errors,
              "selectedConfigurationQualified": not errors and not failed, "threadCount": len(threads), "errors": errors,
              "metrics": metrics, "cohorts": {phase: paired_metrics([row for row in rows if row["phase"] == phase]) for phase in ("answers-primary", "answers-transfer")} if not errors else {},
              "failedAnswers": failed, "records": rows, "subjectSha256": sha(SUBJECT), "protocolSha256": sha(PROTOCOL),
              "assessorSha256": sha(Path(__file__)), "semanticReviewSha256": sha(ROOT / "semantic-review.json") if (ROOT / "semantic-review.json").is_file() else None,
              "limits": "Finite synthetic explicit recall on Codex CLI gpt-6.1-sol/max. Luna adverse evidence retained. No automatic Desktop hook, full settings, second software host, universal quality or monetary saving claim."}
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: value for key, value in result.items() if key != "records"}, ensure_ascii=True, sort_keys=True)); return 0 if result["selectedConfigurationQualified"] else 1

if __name__ == "__main__": raise SystemExit(main())
