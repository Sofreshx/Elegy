"""Strict offline scorer for the frozen realistic memory campaign."""
from __future__ import annotations
import argparse, hashlib, json, re, unicodedata
import math
import statistics
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
ALLOWED_ITEM_TYPES = {"reasoning", "agent_message", "assistant_message"}
ABSTENTION = ("inconnu", "inconnue", "pas renseigné", "pas renseignée", "pas indiqué", "pas indiquée", "pas disponible", "ne sais pas", "non documenté", "non documentée", "non fourni", "non fournie", "aucune information", "impossible de déterminer")

def canonical(value: str) -> str:
    value = unicodedata.normalize("NFKD", str(value)).casefold()
    return "".join(ch for ch in value if not unicodedata.combining(ch))

def contains(text: str, alias: str) -> bool:
    return re.search(r"(?<![\w])" + re.escape(canonical(alias).strip()) + r"(?![\w])", canonical(text), re.UNICODE) is not None

def aliases_for(group: list[str] | str) -> list[str]:
    if isinstance(group, list): return group
    normalized = canonical(group)
    if "impossible de determiner" in normalized: return list(ABSTENTION)
    if normalized == "21 vingt et une vingt-et-une": return ["21", "vingt et une", "vingt-et-une"]
    return [group]

def word_count(text: str) -> int:
    return len(re.findall(r"[^\W_]+(?:['’][^\W_]+)*", text, re.UNICODE))

def score_answer(answer: str, case: dict[str, Any], execution_errors: list[str] | None = None, format_errors: list[str] | None = None) -> dict[str, Any]:
    execution_errors, format_errors = list(execution_errors or []), list(format_errors or [])
    if word_count(answer) > 100: format_errors.append("response exceeds 100 words")
    groups = case.get("expectedGroups", [])
    missing = [group for group in groups if not any(contains(answer, alias) for alias in aliases_for(group))]
    forbidden = [token for token in case.get("forbidden", []) if contains(answer, token)]
    if execution_errors: value, status = None, "INCONCLUSIVE"
    else:
        value = sum(group not in missing for group in groups) / len(groups) if groups else 0.0
        if forbidden or format_errors: value = 0.0
        status = "SCORED"
    return {"status": status, "score": value, "missingGroups": missing, "forbiddenMentions": forbidden, "formatErrors": format_errors, "executionErrors": execution_errors}

def reconstruct_prompt(protocol: dict[str, Any], case: dict[str, Any], arm: str, packet: dict[str, Any] | None) -> str:
    context = packet.get("response", {}).get("data", {}).get("additionalContext", "") if arm == "with" and packet else ""
    return protocol["instruction"] + "\n\nDonnées historiques :\n" + (context or "") + "\n\nQuestion actuelle :\n" + case["question"]

def parse_stdout_details(stdout: str) -> tuple[str, str | None, dict[str, Any], list[str]]:
    errors, events = [], []
    for line in stdout.splitlines():
        if not line.strip(): continue
        try:
            event = json.loads(line)
            if not isinstance(event, dict): raise ValueError
            events.append(event)
        except (json.JSONDecodeError, ValueError): errors.append("stdout contains non-JSONL output")
    messages, threads, usage, completed = [], [], {}, 0
    allowed_events = {"thread.started", "turn.started", "turn.completed", "item.completed", "item.started"}
    for event in events:
        kind = event.get("type")
        if kind not in allowed_events: errors.append(f"unexpected event type: {kind}")
        if kind == "thread.started":
            if event.get("thread_id"): threads.append(event["thread_id"])
            else: errors.append("thread.started missing thread_id")
        if kind in {"item.completed", "item.started"}:
            item = event.get("item") or {}
            if item.get("type") not in ALLOWED_ITEM_TYPES: errors.append(f"unexpected item type: {item.get('type')}")
            elif kind == "item.completed" and item.get("type") in {"agent_message", "assistant_message"}:
                if isinstance(item.get("text"), str): messages.append(item["text"])
                else: errors.append("assistant item missing text")
        if kind == "turn.completed":
            completed += 1
            if not isinstance(event.get("usage"), dict): errors.append("turn.completed missing usage")
            else: usage = event["usage"]
        if kind in {"turn.failed", "error"} or event.get("error"): errors.append("model/provider error event")
    if len(messages) != 1: errors.append("expected exactly one final assistant message")
    if len(threads) != 1: errors.append("expected exactly one thread id")
    if completed != 1: errors.append("expected exactly one completed turn")
    return (messages[0] if len(messages) == 1 else "", threads[0] if len(threads) == 1 else None, usage, errors)

def parse_stdout(stdout: str) -> tuple[str, list[str]]:
    answer, _, _, errors = parse_stdout_details(stdout)
    return answer, errors

def sha(path: Path) -> str: return hashlib.sha256(path.read_bytes()).hexdigest()
def load(path: Path) -> Any: return json.loads(path.read_text(encoding="utf-8"))

def find_packets(replay_dir: Path) -> tuple[dict[str, dict[str, Any]], dict[str, str]]:
    packets, hashes = {}, {"run.json": sha(replay_dir / 'run.json')}
    for path in sorted(replay_dir.glob("*/run.json")):
        hashes[str(path.relative_to(replay_dir))] = sha(path)
        for stage in load(path).get("stages", []):
            if stage.get("packet"): packets[stage["caseId"]] = stage["packet"]
    return packets, hashes

def grade_campaign(answers: Path, replay: Path, output: Path) -> dict[str, Any]:
    answers = answers if answers.is_absolute() else ROOT / answers
    replay = replay if replay.is_absolute() else ROOT / replay
    protocol_path, corpus_path, runner_path, scorer_path = (ROOT / n for n in ("protocol.json", "corpus.json", "answer_runner.py", "grade.py"))
    protocol, corpus, manifest = load(protocol_path), load(corpus_path), load(answers / "run.json")
    replay_manifest = load(replay / 'run.json')
    packets, replay_hashes = find_packets(replay)
    cases = {case["id"]: case for ep in corpus["episodes"] for case in ep["cases"]}
    expected_pairs = {(cid, arm) for cid in cases for arm in ("with", "without")}
    records, raw_hashes, inventory_errors = [], {}, []
    raw_paths = sorted((answers / "raw").glob("*.json")); seen_pairs, threads = set(), []
    required = ("model", "reasoningEffort", "configurationSha256", "corpusSha256", "protocolSha256", "replayRunSha256", "runnerSha256", "status")
    inventory_errors.extend(f"manifest missing {key}" for key in required if key not in manifest)
    if manifest.get('status') != 'completed' or replay_manifest.get('status') != 'completed':
        inventory_errors.append('answer or replay execution incomplete')
    if set(packets) != set(cases): inventory_errors.append('replay packet inventory differs from corpus')
    if manifest.get('model') != protocol['model'] or manifest.get('reasoningEffort') != protocol['reasoningEffort']:
        inventory_errors.append('requested model or effort differs from protocol')
    expected_command = ['<codex>', 'exec', '--ignore-user-config', '--ephemeral', '--sandbox', 'read-only',
        '--skip-git-repo-check', '--json', '-C', '<scratch-case>', '-m', protocol['model'],
        '-c', f'model_reasoning_effort="{protocol["reasoningEffort"]}"', '-c', 'project_doc_max_bytes=0', '-']
    configuration = manifest.get('configuration', {})
    if configuration.get('argv') != expected_command: inventory_errors.append('unexpected command configuration')
    actual_config_sha = hashlib.sha256(json.dumps(configuration, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    if manifest.get('configurationSha256') != actual_config_sha: inventory_errors.append('configuration payload digest mismatch')
    expected_hashes = {"corpusSha256": sha(corpus_path), "protocolSha256": sha(protocol_path), "runnerSha256": sha(runner_path), "replayRunSha256": replay_hashes.get("run.json")}
    for key, value in expected_hashes.items():
        if value and manifest.get(key) != value: inventory_errors.append(f"manifest {key} mismatch")
    for path in raw_paths:
        raw = load(path); cid, arm = raw.get("caseId"), raw.get("arm"); seen_pairs.add((cid, arm)); rel = str(path.relative_to(answers)); raw_hashes[rel] = sha(path)
        case = cases.get(cid, {"id": cid, "question": "", "expectedGroups": [], "forbidden": []}); packet = packets.get(cid)
        answer, thread, usage, errors = parse_stdout_details(raw.get("stdout", ""))
        if raw.get("answer") != answer: errors.append("raw answer differs from final JSONL message")
        if raw.get("threadId") != thread: errors.append("raw threadId differs from JSONL thread id")
        if raw.get("usage") != usage: errors.append("raw usage differs from JSONL usage")
        if raw.get("valid") is not True: errors.append("valid must be true")
        if raw.get("exitCode") != 0: errors.append("exitCode must be zero")
        elapsed = raw.get('elapsedMs')
        if not isinstance(elapsed, (int, float)) or not math.isfinite(elapsed) or elapsed < 0: errors.append('invalid elapsedMs')
        if not packet: errors.append('missing replay packet')
        elif (packet.get('agent') != case.get('agent') or packet['response'].get('status') != 'ok'
              or not packet['sourceBefore']['main']['present'] or not packet['sourceAfter']['main']['present']
              or packet['sourceBefore']['durableData'] != packet['sourceAfter']['durableData']):
            errors.append('replay binding or source invariant mismatch')
        if not raw.get("promptSha256") or not raw.get("configurationSha256"): errors.append("missing required hash")
        prompt = reconstruct_prompt(protocol, case, arm, packet)
        if raw.get("promptSha256") != hashlib.sha256(prompt.encode()).hexdigest(): errors.append("prompt hash mismatch")
        expected_argv = manifest.get("configuration", {}).get("argv")
        if raw.get("command") != expected_argv: errors.append("command differs from manifest configuration argv")
        if raw.get("configurationSha256") != manifest.get("configurationSha256"): errors.append("configuration hash mismatch")
        if thread: threads.append(thread)
        scored = score_answer(answer, case, execution_errors=errors)
        retrieval = [x.get("content", "") for x in (packet or {}).get("response", {}).get("data", {}).get("recalls", [])]
        coverage = {str(group): any(any(contains(text, alias) for alias in aliases_for(group)) for text in retrieval) for group in case.get("expectedGroups", [])}
        records.append({"caseId": cid, "arm": arm, "question": case.get("question"), "kind": case.get("kind"), "expectedGroups": case.get("expectedGroups", []), "actual": answer, "rawPath": rel, "rawSha256": raw_hashes[rel], "retrievalTexts": retrieval, "expectedAtomCoverage": coverage, 'elapsedMs': elapsed, 'usage': usage, **scored})
    if len(raw_paths) != 48: inventory_errors.append("expected exactly 48 raw answers")
    if seen_pairs != expected_pairs: inventory_errors.append("answer inventory does not equal all case/arm pairs")
    if len(threads) != len(set(threads)): inventory_errors.append("thread IDs are not distinct")
    per_arm = {}
    for arm in ("with", "without"):
        rows = [r for r in records if r["arm"] == arm]; vals = [r["score"] for r in rows if r["score"] is not None]
        per_arm[arm] = {"count": len(rows), "meanScore": sum(vals) / len(vals) if vals else None, "elapsedMsTotal": sum(load(answers / r["rawPath"]).get("elapsedMs", 0) for r in rows), "usage": [load(answers / r["rawPath"]).get("usage", {}) for r in rows], "errorCount": sum(bool(r["executionErrors"]) for r in rows)}
        times = [r['elapsedMs'] for r in rows if isinstance(r['elapsedMs'], (float,int)) and math.isfinite(r['elapsedMs'])]
        per_arm[arm]['timing'] = {'meanWholeProcessMs': statistics.fmean(times) if times else None, 'medianWholeProcessMs': statistics.median(times) if times else None}
        per_arm[arm]['fullyCorrectCount'] = sum(r['score'] == 1 for r in rows)
        per_arm[arm]['forbiddenMentionCount'] = sum(bool(r['forbiddenMentions']) for r in rows)
    assessment = {"schemaVersion": "memory-realistic-assessment/v2", "records": records, "inventory": {"rawCount": len(raw_paths), "expectedCount": 48, "distinctCaseArms": len(seen_pairs), "distinctThreadIds": len(set(threads))}, "inventoryErrors": inventory_errors, "perArm": per_arm, "rawHashes": raw_hashes, "replayHashes": replay_hashes, "hashInputs": {"corpusSha256": sha(corpus_path), "protocolSha256": sha(protocol_path), "runnerSha256": sha(runner_path), "scorerSha256": sha(scorer_path)}, "limitations": protocol.get("limits"), "monetaryCostMeasured": False}
    assessment['status'] = 'inconclusive' if inventory_errors or any(r['executionErrors'] for r in records) else 'valid'
    assessment['meanPairedDifference'] = per_arm['with']['meanScore'] - per_arm['without']['meanScore'] if all(per_arm[a]['meanScore'] is not None for a in ('with','without')) else None
    output.parent.mkdir(parents=True, exist_ok=True); output.write_text(json.dumps(assessment, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return assessment

def main() -> int:
    parser = argparse.ArgumentParser(); parser.add_argument("--answers", required=True); parser.add_argument("--replay", required=True); parser.add_argument("--output", required=True); args = parser.parse_args()
    output = Path(args.output)
    if not output.is_absolute(): output = ROOT / output
    result = grade_campaign(Path(args.answers), Path(args.replay), output)
    return 1 if result["inventoryErrors"] or any(r["executionErrors"] for r in result["records"]) else 0

if __name__ == "__main__": raise SystemExit(main())
