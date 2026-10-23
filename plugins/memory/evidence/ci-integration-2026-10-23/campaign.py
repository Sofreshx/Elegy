"""Governed phase wrapper for the frozen Codex qualification campaign."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import shutil
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]
Q2 = ROOT.parent / "answer-grounding-2026-10-23"
Q1 = ROOT  # Current immutable source proof; old campaigns stay sealed.
SUBJECT = ROOT / "subject.json"
PROTOCOL = ROOT / "protocol.json"

EXPECTED_CLI = "55b76918b44dc1e556a027f444d0c47c8e1175bae6d3d7a59df1fc274917afd6"
EXPECTED_MCP = "ad5e338e9733a0dd1516e9fec28b99f625fe63246c0b52cf929ddc9b1cf83c7b"
EXPECTED_SOURCE_REVISION = "5eb34e0b32034a4bd254c6d775e5179dd2e4a1e1"
EXPECTED_CLI_SOURCE = "1742beff51d26ca32749e7b70ef7cce365f6ed4d3636f5c5e7116a4ecd0783ba"
EXPECTED_CLIENT = "00c0f1e4099246fddcd67ec620ca8ed50827c7d70e113b7224f7a2933cf2d67c"
QUALIFICATION_SOURCES = ("campaign.py", "assess.py", "prepare_review.py", "verify_source.py",
                         "test_campaign.py", "test_assess.py", "test_verify_source.py", "test_evidence_binding.py")
sys.path.insert(0, str(Q2))

def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()

def load(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))

def now() -> str:
    return datetime.now(timezone.utc).isoformat()

def public(value):
    if isinstance(value, dict): return {key: public(item) for key, item in value.items()}
    if isinstance(value, list): return [public(item) for item in value]
    if isinstance(value, str):
        if value == str(Path(sys.executable)): return "<python>"
        if value.startswith(str(REPO)): return str(Path(value).resolve().relative_to(REPO)).replace("\\", "/")
        if value.startswith(str(Path.home())): return "<home>" + value[len(str(Path.home())):]
    return value

def preflight() -> dict:
    subject, protocol = load(SUBJECT), load(PROTOCOL)
    source_lock_path = (ROOT / subject["sourceProof"]).resolve()
    lock = load(source_lock_path)
    checks: dict[str, object] = {}
    checks["sourceRevision"] = subject["sourceRevision"] == lock["productSourceRevision"] == protocol["sourceRevision"]
    checks["fixedSourceRevision"] = subject.get("sourceRevision") == EXPECTED_SOURCE_REVISION
    checks["sourceLockSha256"] = sha(source_lock_path) == subject["sourceLockSha256"]
    checks["cliSourceSha256"] = subject["cliSourceSha256"] == lock["cliSourceSha256"]
    checks["fixedCliSourceSha256"] = subject.get("cliSourceSha256") == EXPECTED_CLI_SOURCE
    checks["subjectBinaryHashes"] = subject.get("cliSha256") == EXPECTED_CLI and subject.get("mcpSha256") == EXPECTED_MCP
    checks["protocolSha256"] = sha(PROTOCOL) == subject["protocolSha256"]
    checks["model"] = protocol.get("model") == subject["model"] == "gpt-6.1-sol"
    checks["reasoningEffort"] = protocol.get("reasoningEffort") == subject["reasoningEffort"] == "max"
    checks["corpusProvenance"] = all(sha(ROOT / Path(item["source"]).name) == item["sha256"] for item in protocol["corpusProvenance"].values())
    source_errors = []
    for group in ("sourceFiles", "mcpSourceFiles"):
        for item in lock.get(group, []):
            path = (REPO / item["path"]).resolve()
            if REPO.resolve() not in path.parents or not path.is_file() or sha(path) != item["sha256"]:
                source_errors.append(item["path"])
    checks["sourceFiles"] = not source_errors
    from verify_source import verify
    checks["sourceGitObjects"] = verify()["status"] == "passed"
    checks["sourceFileCount"] = len(lock.get("sourceFiles", [])) == 149 and len(lock.get("mcpSourceFiles", [])) == 18
    checks["sourceFileErrors"] = source_errors
    binaries = {}
    for key, expected in (("cli", EXPECTED_CLI), ("mcp", EXPECTED_MCP)):
        path = (ROOT / subject["binaryPaths"][key]).resolve()
        binaries[key] = {"path": path, "sha256": sha(path) if path.is_file() else None,
                         "insideRepo": REPO.resolve() in path.parents, "expected": expected}
    checks["binaries"] = all(v["sha256"] == v["expected"] and v["insideRepo"] for v in binaries.values())
    installed = shutil.which("codex")
    checks["installedClient"] = bool(installed and sha(Path(installed)) == subject.get("codexSha256", EXPECTED_CLIENT))
    checks["clientVersionRecorded"] = bool(subject.get("codexVersion"))
    checks["harnessManifest"] = sha(Q2 / "evidence-manifest.json") == subject["harnessManifestSha256"] if (Q2 / "evidence-manifest.json").is_file() else False
    harness_errors = []
    for filename, expected in subject["harnessSourceFiles"].items():
        path = Q2 / filename
        if not path.is_file() or sha(path) != expected: harness_errors.append(filename)
    checks["harnessSourceFiles"] = not harness_errors
    checks["harnessSourceErrors"] = harness_errors
    qualification_sources = subject.get("qualificationSourceFiles", {})
    checks["qualificationSourceFiles"] = (set(qualification_sources) == set(QUALIFICATION_SOURCES)
        and all((ROOT / name).is_file() and sha(ROOT / name) == digest for name, digest in qualification_sources.items()))
    checks["pathsInsideRepo"] = all(REPO.resolve() in path.resolve().parents for path in [source_lock_path, *(ROOT / Path(item["source"]).name for item in protocol["corpusProvenance"].values())])
    checks["all"] = all(value is True for value in checks.values() if isinstance(value, bool))
    if not checks["all"]:
        raise ValueError(json.dumps({"checks": checks}, ensure_ascii=False))
    return {"checks": checks, "binaries": {key: {**value, "path": str(value["path"])} for key, value in binaries.items()},
            "subjectSha256": sha(SUBJECT), "protocolSha256": sha(PROTOCOL)}

def phase_spec(phase: str) -> tuple[str, str, list[str]]:
    if phase == "replay-primary": return "replay", "corpus.json", ["--cli", "--mcp"]
    if phase == "replay-transfer": return "replay", "transfer-corpus.json", ["--cli", "--mcp"]
    if phase == "answers-primary": return "answers", "corpus.json", ["--replay", "replay-primary"]
    if phase == "answers-transfer": return "answers", "transfer-corpus.json", ["--replay", "replay-transfer"]
    if phase == "repeats": return "repeats", "corpus.json", ["--replay", "replay-primary", "--cases", "q08,q09,q11,q17,q18", "--trials", "2,3"]
    raise ValueError(phase)

def run_phase(phase: str) -> int:
    wrapper_path = ROOT / f"{phase}-wrapper.json"
    if wrapper_path.exists(): raise RuntimeError("phase wrapper output already exists")
    log_path = ROOT / "runtime" / f"{phase}.log"
    if log_path.exists(): raise RuntimeError("phase log already exists")
    started = now()
    self_before = sha(Path(__file__))
    try:
        before = preflight()
    except Exception as exc:
        report = {"schemaVersion": "memory-codex-campaign-wrapper/v1", "phase": phase, "startedAt": started, "endedAt": now(), "exitCode": 1, "status": "failed", "error": str(exc)}
        wrapper_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        return 1
    action, corpus_name, extra = phase_spec(phase)
    output = ROOT / phase
    if output.exists(): raise RuntimeError("phase output already exists")
    binary = before["binaries"]
    args = [sys.executable, str(Q2 / "qualification_runner.py"), action, "--corpus", str(ROOT / corpus_name), "--protocol", str(PROTOCOL), "--output", str(output)]
    if action == "replay": args += ["--cli", binary["cli"]["path"], "--mcp", binary["mcp"]["path"]]
    else: args += [extra[0], str(ROOT / extra[1])]
    if phase == "repeats": args += extra[2:]
    runtime = ROOT / "runtime"; runtime.mkdir(exist_ok=True)
    try:
        proc = subprocess.run(args, cwd=REPO, env=__import__("answer_runner").environment(), capture_output=True, text=True, encoding="utf-8", timeout=7200)
        if log_path.exists(): raise RuntimeError("phase log already exists")
        log_path.write_text(proc.stdout + "\n--- stderr ---\n" + proc.stderr, encoding="utf-8")
        manifest = load(output / "run.json") if (output / "run.json").is_file() else {}
        after = preflight()
        expected_status = "completed" if action == "replay" else "execution-valid"
        status = "completed" if proc.returncode == 0 and manifest.get("status") == expected_status else "failed"
        if action == "replay":
            if manifest.get("cliSha256") != before["binaries"]["cli"]["sha256"] or manifest.get("mcpSha256") != before["binaries"]["mcp"]["sha256"]:
                status = "failed"
            if manifest.get("cliSha256After") != manifest.get("cliSha256") or manifest.get("mcpSha256After") != manifest.get("mcpSha256"):
                status = "failed"
        else:
            from answer_runner import command
            expected_command = command("<codex>", "<scratch-case>", load(PROTOCOL))
            expected_count = {"answers-primary": 50, "answers-transfer": 24, "repeats": 10}[phase]
            if (manifest.get("configuration", {}).get("argv") != expected_command
                    or manifest.get("codexSha256") != load(SUBJECT)["codexSha256"]
                    or manifest.get("errors") or len(manifest.get("records", [])) != expected_count):
                status = "failed"
            for record in manifest.get("records", []):
                raw_path = (output / record["rawPath"]).resolve()
                if output.resolve() not in raw_path.parents:
                    raise ValueError("raw path escapes phase output")
                raw = load(raw_path)
                if (sha(raw_path) != record["rawSha256"] or raw.get("command") != expected_command
                        or raw.get("caseId") != record["caseId"] or raw.get("arm") != record["arm"]
                        or raw.get("configurationSha256") != manifest["configurationSha256"]):
                    status = "failed"
        self_after = sha(Path(__file__))
        if self_after != self_before or before["subjectSha256"] != sha(SUBJECT) or before["protocolSha256"] != sha(PROTOCOL): status = "failed"
        report = {"schemaVersion": "memory-codex-campaign-wrapper/v1", "phase": phase, "startedAt": started, "endedAt": now(),
                  "exitCode": proc.returncode, "status": status, "before": before, "after": after,
                  "selfSha256Before": self_before, "selfSha256After": self_after,
                  "subjectSha256": sha(SUBJECT), "protocolSha256": sha(PROTOCOL), "argv": public(["<python>" if x == sys.executable else x for x in args])}
    except Exception as exc:
        log_path.write_text(str(exc), encoding="utf-8")
        report = {"schemaVersion": "memory-codex-campaign-wrapper/v1", "phase": phase, "startedAt": started, "endedAt": now(), "exitCode": 1, "status": "failed", "error": str(exc), "before": before, "selfSha256Before": self_before}
    wrapper_path.write_text(json.dumps(public(report), ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return 0 if report["status"] == "completed" else 1

def main() -> int:
    parser = argparse.ArgumentParser(); parser.add_argument("--phase", choices=("preflight", "replay-primary", "replay-transfer", "answers-primary", "answers-transfer", "repeats"), required=True)
    args = parser.parse_args()
    if args.phase == "preflight": print(json.dumps(public(preflight()), ensure_ascii=False, sort_keys=True)); return 0
    return run_phase(args.phase)

if __name__ == "__main__": raise SystemExit(main())
