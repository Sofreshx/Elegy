"""One predeclared prompt-overlay experiment; no native Memory changes."""
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import sys
import threading

ROOT = Path(__file__).resolve().parent
BASE = ROOT.parent / "current-replay"


def load(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def save(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def now():
    return datetime.now(timezone.utc).isoformat()


def prepare():
    candidate = (ROOT / "candidate.txt").read_text(encoding="utf-8")
    assert len(candidate.encode()) == 200
    corpus = load(BASE / "harness/corpus.json")
    protocol = load(BASE / "harness/protocol.json")
    original_packets = {}
    for path in sorted((BASE / "replay-01").glob("*/run.json")):
        for stage in load(path)["stages"]:
            original_packets[stage["caseId"]] = (stage["packet"], path)
    assert len(original_packets) == 24
    packets = {}
    for episode in corpus["episodes"]:
        for case in episode["cases"]:
            packet, source = original_packets[case["id"]]
            original = packet["response"]["data"]["additionalContext"]
            if original:
                split = original.index("{")
                prefix, payload = original[:split], original[split:]
                assert len(prefix.encode()) == 202
                overlay = candidate + payload
                assert overlay[len(candidate):] == payload
                assert len(overlay.encode()) <= len(original.encode()) <= 600
            else:
                prefix, payload, overlay = "", "", ""
            packets[case["id"]] = {"caseId": case["id"], "originalEpisodeRun": str(source.relative_to(BASE)),
                "originalEpisodeRunSha256": sha(source),
                "sourcePacketSha256": hashlib.sha256(json.dumps(packet, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).hexdigest(),
                "originalPrefixSha256": hashlib.sha256(prefix.encode()).hexdigest(),
                "recordPayloadSha256": hashlib.sha256(payload.encode()).hexdigest(),
                "originalAdditionalContextSha256": hashlib.sha256(original.encode()).hexdigest(),
                "overlayAdditionalContextSha256": hashlib.sha256(overlay.encode()).hexdigest(),
                "originalContextBytes": len(original.encode()), "overlayContextBytes": len(overlay.encode()),
                "originalBudgetTokensUpperBound": packet["response"]["data"]["budgetTokensUpperBound"],
                "configuredBudget": packet["configuration"]["maxContextTokens"],
                "originalPacket": packet, "overlayAdditionalContext": overlay}
    assert not (ROOT / "predeclaration.json").exists(), "Preserve prior experiment"
    declaration = {"schemaVersion": "memory-prompt-overlay-experiment/v1", "label": "PROMPT-OVERLAY EXPERIMENT",
        "declaredAt": now(), "modelCallsAtDeclaration": 0,
        "change": "Replace only the 202-byte opening prefix with the candidate 200-byte prefix; preserve record payload, IDs, versions, contents, and configured budget. Empty recall remains empty.",
        "candidateSha256": sha(ROOT / "candidate.txt"), "candidateBytes": len(candidate.encode()),
        "sourceRevision": load(BASE / "subject.json")["sourceRevision"], "sourceNativeQualificationSha256": sha(BASE / "qualification-summary.json"),
        "protocolSha256": sha(BASE / "harness/protocol.json"), "corpusSha256": sha(BASE / "harness/corpus.json"),
        "frozenScorerSha256": sha(BASE / "harness/grade.py"), "invocationRunnerSha256": sha(BASE / "harness/answer_runner.py"),
        "orchestratorSha256": sha(__file__), "model": protocol["model"], "reasoningEffort": protocol["reasoningEffort"],
        "sessionCount": 24, "arms": ["with"], "maxConcurrentSessions": 3,
        "criteria": {"all24ValidNoTools": True, "q08ExplicitRetention": "11 days", "q09": "Backup/snapshot identifier is unknown; do not claim the runbook as that identifier", "q17AndQ18": "75 minutes and REPLAY_PINE", "allNegativeControlsContinuePass": True,
            "semanticCompleteCount": 24, "noNewSemanticErrorsAmongPreviouslyCompleteCases": [f"q{i:02}" for i in range(1, 25) if i not in (8, 9)],
            "frozenMeanMinimum": 0.8402777778, "meanTolerance": 1e-6, "lexicalFullyCorrectMinimum": 19},
        "failurePolicy": "Any failed criterion refutes this candidate. No second prefix, retry, model substitution, threshold or rubric change.",
        "baselineUse": "Existing no-recall results are context only; no new without calls and no fresh paired causal A/B claim.",
        "limits": "Prompt overlay, not native-binary requalification; fixed known corpus, one realization, hosted alias, no guarantee of production semantic reliability."}
    assert declaration["model"] == "gpt-5.6-luna" and declaration["reasoningEffort"] == "medium"
    save(ROOT / "source-and-overlay-packets.json", packets)
    save(ROOT / "predeclaration.json", declaration)
    print(json.dumps({"stage": "predeclared", "modelCalls": 0, "candidateBytes": 200, "cases": len(packets), "criteriaWritten": True}), flush=True)


def run():
    declaration = load(ROOT / "predeclaration.json")
    assert declaration["orchestratorSha256"] == sha(__file__), "Declared harness changed"
    assert declaration["candidateSha256"] == sha(ROOT / "candidate.txt"), "Candidate changed"
    spec = importlib.util.spec_from_file_location("current_campaign", BASE / "current_campaign.py")
    current = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(current)
    current.clean_environment()
    current.guard()
    sys.path.insert(0, str(BASE / "harness"))
    import answer_runner
    import grade
    protocol = load(BASE / "harness/protocol.json")
    exe = Path(shutil.which("codex") or "")
    assert exe.is_file()
    configuration, config_sha = answer_runner.configuration(exe, protocol)
    out = ROOT / "answers-01"
    assert not out.exists(), "Preserve prior attempt"
    out.mkdir()
    packets = load(ROOT / "source-and-overlay-packets.json")
    cases = [c for ep in load(BASE / "harness/corpus.json")["episodes"] for c in ep["cases"]]
    report = {"schemaVersion": "memory-prompt-overlay-answers/v1", "label": "PROMPT-OVERLAY EXPERIMENT", "startedAt": now(),
        "status": "running", "predeclarationSha256": sha(ROOT / "predeclaration.json"), "candidateSha256": declaration["candidateSha256"],
        "model": protocol["model"], "reasoningEffort": protocol["reasoningEffort"],
        "configuration": configuration, "configurationSha256": config_sha,
        "codexSha256": sha(exe), "runnerSha256": sha(BASE / "harness/answer_runner.py"), "orchestratorSha256": sha(__file__),
        "loginStatusQueried": False, "quotaQueried": False, "paidApiFallback": False, "answers": []}
    stopped = threading.Event()

    def call(case):
        if stopped.is_set():
            return None
        record = answer_runner.invoke(exe, protocol, out, case["id"], "with", case["question"], packets[case["id"]]["overlayAdditionalContext"], config_sha)
        raw = load(out / "raw" / f'{case["id"]}_with.json')
        _, _, _, issues = grade.parse_stdout_details(raw["stdout"])
        if not record["valid"] or issues:
            stopped.set()
        record["strictProtocolIssues"] = issues
        return record

    save(out / "run.json", report)
    with ThreadPoolExecutor(max_workers=3) as pool:
        for future in as_completed([pool.submit(call, case) for case in cases]):
            item = future.result()
            if item:
                report["answers"].append(item)
                save(out / "run.json", report)
    report["answers"].sort(key=lambda x: x["caseId"])
    report["status"] = "completed" if len(report["answers"]) == 24 and all(x["valid"] and not x["strictProtocolIssues"] for x in report["answers"]) else "inconclusive"
    report.update(completedAt=now(), pendingCallsStopped=stopped.is_set())
    save(out / "run.json", report)
    current.guard()
    print(json.dumps({"stage": "experiment-model-calls", "status": report["status"], "answerCount": len(report["answers"])}), flush=True)
    return 0 if report["status"] == "completed" else 1


if __name__ == "__main__":
    if sys.argv[1:] == ["prepare"]:
        prepare()
    elif sys.argv[1:] == ["run"]:
        raise SystemExit(run())
    else:
        raise SystemExit("Choose prepare or run")
