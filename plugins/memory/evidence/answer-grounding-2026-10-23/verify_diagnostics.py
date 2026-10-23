#!/usr/bin/env python3
"""Offline integrity checker for the retained answer-grounding diagnostics."""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
CASE_TRIALS = {"q08": 3, "q09": 3, "q11": 3, "q17": 1, "q18": 1}
DESIGNS = {
    "diagnostic-01": ("diagnostic-protocol.json", "diagnose_grounding.py", ["current", "candidate"]),
    "layout-01": ("layout-protocol.json", "diagnose_layout.py", ["current", "text-first", "text-only"]),
    "semantics-01": ("semantics-protocol.json", "diagnose_semantics.py", ["historical-generic", "historical-typed", "latest-generic", "latest-typed"]),
    "boundary-01": ("boundary-protocol.json", "diagnose_boundary.py", ["data-labeled", "separate-policy"]),
}

def sha(data: bytes | Path) -> str:
    return hashlib.sha256(data.read_bytes() if isinstance(data, Path) else data).hexdigest()

def canon(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()

def load(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))

def fail(errors: list[str], msg: str) -> None:
    errors.append(msg)

def corpus_cases(corpus: dict[str, Any]) -> dict[str, dict[str, Any]]:
    out = {}
    for episode in corpus.get("episodes", []):
        for case in episode.get("cases", []):
            if case.get("id") in out: raise ValueError(f"duplicate corpus case {case.get('id')}")
            out[case["id"]] = case
    return out

def replay_contexts(replay: Path, corpus: dict[str, Any], errors: list[str]) -> dict[str, str]:
    manifest = replay / "run.json"
    if not manifest.is_file(): fail(errors, "missing replay run.json"); return {}
    try: run = load(manifest)
    except Exception as exc: fail(errors, f"invalid replay manifest: {exc}"); return {}
    if run.get("status") != "completed": fail(errors, "replay is not completed")
    expected_episodes = {e["id"] for e in corpus.get("episodes", [])}
    reports = run.get("episodes", run.get("episodeReports", []))
    got = {x.get("episodeId", x.get("id")) for x in reports if isinstance(x, dict)}
    if got != expected_episodes: fail(errors, "replay episode inventory mismatch")
    contexts: dict[str, str] = {}
    for p in replay.glob("*/run.json"):
        if p == manifest: continue
        try: x = load(p)
        except Exception as exc: fail(errors, f"invalid replay episode {p.name}: {exc}"); continue
        for stage in x.get("stages", []):
            packet = stage.get("packet", {}).get("response", {}).get("data", {})
            case = packet.get("caseId") or stage.get("caseId")
            context = packet.get("additionalContext")
            if not isinstance(case, str) or not isinstance(context, str): continue
            if case in contexts: fail(errors, f"duplicate replay context {case}")
            contexts[case] = context
    missing = set(CASE_TRIALS) - set(contexts)
    if missing: fail(errors, f"missing replay contexts: {sorted(missing)}")
    return contexts

def split_context(context: str) -> tuple[str, str]:
    pos = context.find("\n")
    if pos < 0: raise ValueError("context has no prefix/body separator")
    return context[:pos + 1], context[pos + 1:]

def layout_context(context: str, condition: str) -> str:
    prefix, body = split_context(context)
    value = json.loads(body)
    if condition == "current": return context
    records = value.get("records")
    if not isinstance(records, list): raise ValueError("records missing")
    if condition == "text-first":
        for record in records:
            if not isinstance(record, dict) or "text" not in record: raise ValueError("record text missing")
            text = record["text"]
            reordered = {"text": text}
            reordered.update({key: value for key, value in record.items() if key != "text"})
            record.clear(); record.update(reordered)
    elif condition == "text-only":
        value = {"records": [{"text": r["text"]} for r in records]}
    else: raise ValueError(condition)
    return prefix + json.dumps(value, ensure_ascii=False, separators=(",", ":"))

def transform_context(base: str, design: dict[str, Any], condition: str, name: str) -> str:
    prefix, body = split_context(base)
    if name == "diagnostic-01":
        return base if condition == "current" else design["candidateEnvelope"] + body
    if name == "layout-01": return layout_context(base, condition)
    if name == "semantics-01": return design["conditions"][condition] + body
    if name == "boundary-01": return base if condition == "data-labeled" else body
    raise ValueError(name)

def effective_instruction(protocol: dict[str, Any], design: dict[str, Any], run: dict[str, Any], name: str, condition: str, base: str) -> str:
    if name != "boundary-01":
        return protocol["instruction"]
    prefix, _ = split_context(base)
    if condition == "data-labeled":
        expected = protocol["instruction"]
    elif condition == "separate-policy":
        expected = protocol["instruction"] + design["separatePolicyInstruction"] + prefix
    else:
        raise ValueError(condition)
    if run.get("effectiveInstructions", {}).get(condition) != expected:
        raise ValueError(f"boundary effective instruction mismatch: {condition}")
    return expected

def expected_keys(conditions: list[str], trials: dict[str, int] | None = None) -> set[tuple[str, int, str]]:
    trials = trials or CASE_TRIALS
    return {(case, trial, condition) for case, count in trials.items() for trial in range(1, count + 1) for condition in conditions}

def safe_raw(run_dir: Path, rel: str) -> Path:
    p = Path(rel)
    if p.is_absolute() or any(part in ("", ".", "..") for part in p.parts) or "\\" in rel:
        raise ValueError("unsafe rawPath")
    resolved = (run_dir / p).resolve()
    if run_dir.resolve() not in resolved.parents: raise ValueError("rawPath escapes run")
    return resolved


def proof_files(root: Path) -> dict[str, Path]:
    return {path.relative_to(root).as_posix(): path for path in root.rglob("*")
            if path.is_file() and path.name != "evidence-manifest.json"
            and not {"runtime", "__pycache__"}.intersection(path.relative_to(root).parts)}


def verify_file_manifest(root: Path, errors: list[str]) -> None:
    manifest = load(root / "evidence-manifest.json")
    entries = manifest["files"]
    current = proof_files(root)
    declared = [entry["path"] for entry in entries]
    if len(declared) != len(set(declared)) or set(declared) != set(current):
        fail(errors, "file manifest inventory mismatch")
    for entry in entries:
        path = safe_raw(root, entry["path"])
        if not path.is_file() or sha(path) != entry["sha256"]:
            fail(errors, "file manifest hash mismatch: " + entry["path"])


def verify_source_inputs(root: Path, corpus: dict[str, Any], errors: list[str]) -> None:
    source_root = root.parent / "recall-fixes-2026-10-23"
    anchors = load(root / "local-checks.json")["priorEvidence"]
    manifest_path = source_root / "evidence-manifest.json"
    if sha(manifest_path) != anchors["evidenceManifestSha256"]:
        fail(errors, "source evidence manifest differs from its pinned anchor")
    entries = load(manifest_path)["files"]
    declared = {entry["path"]: entry["sha256"] for entry in entries}
    if len(declared) != len(entries):
        fail(errors, "source evidence manifest has duplicate paths")
    required = {"grade.py", "corpus.json", "replay-01/run.json"}
    required.update("replay-01/" + episode["id"] + "/run.json" for episode in corpus["episodes"])
    if not required.issubset(declared):
        fail(errors, "source replay/scorer inputs missing from the pinned manifest")
    for entry in entries:
        path = safe_raw(source_root, entry["path"])
        if not path.is_file() or sha(path) != entry["sha256"]:
            fail(errors, "source evidence file differs: " + entry["path"])
    if declared.get("grade.py") != sha(root / "grade.py"):
        fail(errors, "diagnostic scorer differs from the frozen source scorer")
    if declared.get("corpus.json") != sha(root / "corpus.json"):
        fail(errors, "diagnostic corpus differs from the frozen source corpus")
    for field, name in (("subjectSha256", "subject.json"), ("sourceLockSha256", "source-lock.json")):
        if anchors[field] != sha(source_root / name):
            fail(errors, "source binding differs: " + name)
    scorer = load(root / "evidence-manifest.json")["scorerAnchor"]
    if scorer["sha256"] != sha(root / "grade.py"):
        fail(errors, "scorer differs from the pretrial source anchor")
    blob = subprocess.run(["git", "cat-file", "blob", scorer["revision"] + ":" + scorer["path"]],
                          cwd=root.parents[3], capture_output=True, timeout=30)
    if blob.returncode or sha(blob.stdout) != scorer["sha256"]:
        fail(errors, "scorer does not match its committed pretrial source")

def verify_raw_record(record: dict[str, Any], raw: dict[str, Any], case: dict[str, Any], context: str,
                      instruction: str, configuration_sha: str, seen_threads: set[str] | None = None,
                      require_record_prompt_hash: bool = True, expected_command: list[str] | None = None) -> str:
    """Verify one stored raw response and its manifest projection."""
    import grade
    if raw.get("exitCode") != 0 or raw.get("valid") is not True:
        raise ValueError("raw execution invalid")
    if raw.get("caseId") != case["id"] or raw.get("arm") != "with":
        raise ValueError("raw case/arm mismatch")
    if expected_command is not None and raw.get("command") != expected_command:
        raise ValueError("raw command mismatch")
    if raw.get("configurationSha256") != configuration_sha:
        raise ValueError("raw configuration hash mismatch")
    if "configurationSha256" in record and record["configurationSha256"] != configuration_sha:
        raise ValueError("record configuration hash mismatch")
    answer, thread, usage, parse_errors = grade.parse_stdout_details(raw.get("stdout", ""))
    if parse_errors or not answer or not thread or not usage:
        raise ValueError(f"stdout parse errors: {parse_errors}")
    if raw.get("answer") != answer or raw.get("threadId") != thread or raw.get("usage") != usage:
        raise ValueError("raw JSON projection mismatch")
    if record.get("answer") != answer or record.get("threadId") != thread or record.get("usage") != usage:
        raise ValueError("record JSON projection mismatch")
    prompt = instruction + "\n\nDonnées historiques :\n" + context + "\n\nQuestion actuelle :\n" + case["question"]
    prompt_sha = sha(prompt.encode())
    if not raw.get("promptSha256") or (require_record_prompt_hash and not record.get("promptSha256")):
        raise ValueError("prompt hash missing")
    if raw["promptSha256"] != prompt_sha or ("promptSha256" in record and record["promptSha256"] != prompt_sha):
        raise ValueError("prompt hash mismatch")
    if seen_threads is not None and thread in seen_threads:
        raise ValueError("duplicate thread")
    score = grade.score_answer(answer, case, [], [])
    for field in ("score", "missingGroups", "forbiddenMentions", "formatErrors", "executionErrors"):
        if record.get(field) != score.get(field):
            raise ValueError(f"grade mismatch: {field}")
    if seen_threads is not None:
        seen_threads.add(thread)
    return thread

def verify_reviews(run_dir: Path, run: dict[str, Any], records: list[dict[str, Any]], errors: list[str]) -> None:
    p = run_dir / "review.json"
    if not p.is_file():
        fail(errors, "required prose review is missing")
        return
    try: review = load(p)
    except Exception as exc: fail(errors, f"invalid review: {exc}"); return
    if review.get("runSha256") and review["runSha256"] != sha(run_dir / "run.json"): fail(errors, "review run hash mismatch")
    by_key = {(r.get("caseId"), r.get("trial"), r.get("condition")): r for r in records}
    items = review.get("records", review.get("failures", []))
    if not isinstance(items, list):
        fail(errors, "prose review records/failures is not a list")
        return
    if "records" not in review and "failures" not in review:
        # The original layout schema retains a reviewed aggregate plus exceptions.
        if review.get("schemaVersion") != "memory-layout-review/v1":
            fail(errors, "unrecognized aggregate prose review")
        for condition, counts in review.get("counts", {}).items():
            selected = [record for record in records if record["condition"] == condition]
            if counts != {"lexicalFull": sum(record["score"] == 1 for record in selected), "total": len(selected)}:
                fail(errors, "layout review count mismatch")
    if isinstance(items, list):
        if "records" in review:
            if len(items) != len(records): fail(errors, "review record count mismatch")
            review_keys = set()
        else:
            review_keys = None
        for item in items:
            key = (item.get("caseId"), item.get("trial"), item.get("condition"))
            if key not in by_key: fail(errors, f"review references absent record {key}"); continue
            if review_keys is not None:
                if key in review_keys: fail(errors, f"duplicate review record {key}")
                review_keys.add(key)
                if not isinstance(item.get("prosePass"), bool): fail(errors, f"missing prosePass {key}")
            record = by_key[key]
            if item.get("rawSha256") and item["rawSha256"] != record.get("rawSha256"): fail(errors, "review raw hash mismatch")
            if item.get("answerSha256") and item["answerSha256"] != sha(record.get("answer", "").encode()): fail(errors, "review answer hash mismatch")
        if "records" in review:
            if review_keys != set(by_key):
                fail(errors, "review inventory mismatch")
            if review.get("prosePassed") != sum(item.get("prosePass") is True for item in items):
                fail(errors, "review prose count mismatch")

def verify_run(name: str, replay: Path, corpus: dict[str, Any], protocol: dict[str, Any], source: dict[str, str], errors: list[str], global_threads: set[str], shared: dict[str, Any]) -> int:
    run_dir = ROOT / name; manifest = run_dir / "run.json"
    run = load(manifest)
    design_file, runner_file, conditions = DESIGNS[name]; design = load(ROOT / design_file)
    trials = {case: 3 for case in CASE_TRIALS} if name == "diagnostic-01" else CASE_TRIALS
    expected = expected_keys(conditions, trials); records = run.get("records", [])
    if len(records) != len(expected): fail(errors, f"{name}: record count")
    if run.get("status") != "execution-valid": fail(errors, f"{name}: status")
    hash_fields = {"protocolSha256": ROOT / "protocol.json", "corpusSha256": ROOT / "corpus.json", "answerRunnerSha256": ROOT / "answer_runner.py", "runnerSha256": ROOT / runner_file}
    design_field = {"diagnostic-01":"designSha256","layout-01":"layoutProtocolSha256","semantics-01":"semanticsProtocolSha256","boundary-01":"boundaryProtocolSha256"}[name]
    hash_fields[design_field] = ROOT / design_file
    for field, path in hash_fields.items():
        if run.get(field) != sha(path): fail(errors, f"{name}: {field} mismatch")
    if run.get("configurationSha256") != hashlib.sha256(canon(run.get("configuration", {}))).hexdigest(): fail(errors, f"{name}: configuration hash")
    from answer_runner import command
    expected_command = command("<codex>", "<scratch-case>", protocol)
    if run.get("configuration", {}).get("argv") != expected_command:
        fail(errors, f"{name}: configured command differs from the frozen protocol")
    if (run.get("configuration", {}).get("executableSha256") and run.get("codexSha256")
            and run["configuration"]["executableSha256"] != run.get("codexSha256")):
        fail(errors, f"{name}: executable/client hash mismatch")
    if run.get("errors"): fail(errors, f"{name}: non-empty run errors")
    for field in ("configurationSha256", "codexSha256", "clientSha256Before", "clientSha256After"):
        value = run.get(field)
        if value is not None:
            prior = shared.setdefault(field, value)
            if value != prior: fail(errors, f"{name}: shared {field} mismatch")
    if run.get("clientSha256Before") and run.get("clientSha256After") and run["clientSha256Before"] != run["clientSha256After"]:
        fail(errors, f"{name}: client changed")
    for field in ("clientSha256Before", "clientSha256After"):
        if run.get(field) and run[field] != run.get("codexSha256"):
            fail(errors, f"{name}: {field} differs from the recorded client")
    helper = run.get("diagnoseLayoutSha256")
    if helper and helper != sha(ROOT / "diagnose_layout.py"): fail(errors, f"{name}: diagnoseLayout hash")
    replay_field = "originalReplaySha256" if name == "diagnostic-01" else "oldReplaySha256"
    if run.get(replay_field) != sha(replay / "run.json"): fail(errors, f"{name}: replay hash")
    if run.get("paidApiCalls") != 0: fail(errors, f"{name}: paid calls")
    seen, threads = set(), set()
    cases = corpus_cases(corpus)
    for record in records:
        key = (record.get("caseId"), record.get("trial"), record.get("condition")); seen.add(key)
        if key not in expected: fail(errors, f"{name}: unexpected record {key}")
        case = cases.get(record.get("caseId"))
        if not case: fail(errors, f"{name}: unknown case"); continue
        try:
            context = run["contexts"][record["caseId"]][record["condition"]]
            if context != transform_context(source[record["caseId"]], design, record["condition"], name): fail(errors, f"{name}: context transform {key}")
            if len(context.encode("utf-8")) > 600: fail(errors, f"{name}: context exceeds 600 bytes {key}")
            raw_path = safe_raw(run_dir, record["rawPath"]); raw = load(raw_path)
            if record.get("rawSha256") != sha(raw_path): fail(errors, f"{name}: raw hash {key}")
            if record.get("contextSha256") and record["contextSha256"] != sha(context.encode()): fail(errors, f"{name}: context hash {key}")
            instruction = effective_instruction(protocol, design, run, name, record["condition"], source[record["caseId"]])
            # The first immutable schema stores the prompt hash in raw only.
            # Do not retrofit evidence; rederive and validate that raw hash.
            legacy_raw_only = name == "diagnostic-01" and run.get("schemaVersion") == "memory-grounding-diagnostic-run/v1"
            thread = verify_raw_record(record, raw, case, context, instruction, run["configurationSha256"], threads,
                                       require_record_prompt_hash=not legacy_raw_only, expected_command=expected_command)
            if thread in global_threads: fail(errors, f"global duplicate thread {thread}")
            global_threads.add(thread)
        except Exception as exc: fail(errors, f"{name}: {key}: {exc}")
    if seen != expected: fail(errors, f"{name}: inventory mismatch")
    verify_reviews(run_dir, run, records, errors)
    return len(records)

def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(); ap.add_argument("--output"); args = ap.parse_args(argv)
    errors: list[str] = []
    try:
        verify_file_manifest(ROOT, errors)
        corpus = load(ROOT / "corpus.json"); protocol = load(ROOT / "protocol.json")
        verify_source_inputs(ROOT, corpus, errors)
        source = replay_contexts(ROOT.parent / "recall-fixes-2026-10-23" / "replay-01", corpus, errors)
        global_threads: set[str] = set()
        shared: dict[str, Any] = {}
        total = sum(verify_run(n, ROOT.parent / "recall-fixes-2026-10-23" / "replay-01", corpus, protocol, source, errors, global_threads, shared) for n in DESIGNS)
        if len(global_threads) != 129: fail(errors, f"global thread inventory is {len(global_threads)}")
    except Exception as exc:
        errors.append(f"harness exception: {exc}"); total = 0
    result = {"schemaVersion":"memory-grounding-diagnostics-verification/v1", "status":"evidence-consistent" if not errors and total == 129 else "failed", "productQualified":False, "records":total, "errors":errors}
    if args.output:
        out = Path(args.output); out.mkdir(parents=True, exist_ok=False); (out / "verification.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0 if result["status"] == "evidence-consistent" else 1

if __name__ == "__main__": sys.exit(main())
