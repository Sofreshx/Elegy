#!/usr/bin/env python3
"""Offline old/new CLI compatibility probe for memory retrieval fixes.

The probe is intentionally self-contained and uses only the Python standard
library.  It records every child process result, including failures, but does
not contact a provider other than the loopback fixture started below.
"""

from __future__ import annotations

import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import shutil
import socketserver
import sqlite3
import subprocess
import tempfile
import threading
import time
from typing import Any

from rpc_support import source_snapshot
from run_replay import invariant


TIMEOUT_SECONDS = 90
DIMENSIONS = 768
CLI_LABELS: dict[str, str] = {}


def child_env() -> dict[str, str]:
    """Return a child environment with ambient provider/proxy settings removed."""
    env = dict(os.environ)
    for key in list(env):
        upper = key.upper()
        if any(token in upper for token in ("API_KEY", "OPENAI", "OLLAMA", "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY")):
            env.pop(key, None)
    env["NO_PROXY"] = "127.0.0.1,localhost"
    env["no_proxy"] = env["NO_PROXY"]
    return env


def answer_runner_environment() -> dict[str, str]:
    """Compatibility alias used by the answer runner contract."""
    return child_env()


class FixtureHandler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    requests: list[dict[str, Any]] = []

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def do_POST(self) -> None:  # noqa: N802 - stdlib HTTP handler API
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length)
        if self.path != "/v1/embeddings":
            self.send_error(404)
            return
        try:
            payload = json.loads(body.decode("utf-8"))
            inputs = payload.get("input")
            if isinstance(inputs, str):
                inputs = [inputs]
            if not isinstance(inputs, list) or not all(isinstance(item, str) for item in inputs):
                raise ValueError("embedding input must be a list of strings")
        except (ValueError, json.JSONDecodeError) as exc:
            self.send_error(400, str(exc))
            return
        vectors = [{"object": "embedding", "index": i, "embedding": vector_for(text)} for i, text in enumerate(inputs)]
        FixtureHandler.requests.append({"path": self.path, "count": len(inputs)})
        response = json.dumps({"object": "list", "data": vectors, "model": "fixture"}).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(response)))
        self.end_headers()
        self.wfile.write(response)


def vector_for(_text: str) -> list[float]:
    values = [0.0] * DIMENSIONS
    values[0] = 1.0
    return values


def run_child(executable: Path, args: list[str], cwd: Path) -> dict[str, Any]:
    command = [str(executable), *args]
    started = time.monotonic()
    try:
        result = subprocess.run(
            command,
            cwd=str(cwd),
            env=answer_runner_environment(),
            capture_output=True,
            text=True,
            timeout=TIMEOUT_SECONDS,
            check=False,
        )
        cli_label = CLI_LABELS[str(executable.resolve())]
        return {
            "command": [cli_label, *[normalize_path(item, cwd) for item in args]],
            "exitCode": result.returncode,
            "stdout": result.stdout,
            "stderr": result.stderr,
            "elapsedMs": round((time.monotonic() - started) * 1000, 3),
        }
    except subprocess.TimeoutExpired as exc:
        return {
            "command": [CLI_LABELS[str(executable.resolve())], *[normalize_path(item, cwd) for item in args]],
            "exitCode": 124,
            "stdout": decode_output(exc.stdout),
            "stderr": decode_output(exc.stderr) + "\nsubprocess timeout",
            "elapsedMs": round((time.monotonic() - started) * 1000, 3),
        }


def decode_output(value: bytes | str | None) -> str:
    if value is None:
        return ""
    return value.decode("utf-8", errors="replace") if isinstance(value, bytes) else value


def normalize_path(value: str, cwd: Path) -> str:
    try:
        path = Path(value)
        if path.is_absolute():
            return "<path>"
    except Exception:
        pass
    return value


def parse_json(stdout: str) -> dict[str, Any]:
    return json.loads(stdout)


def data_of(result: dict[str, Any]) -> Any:
    return parse_json(result["stdout"])["data"]


def search_projection(result: dict[str, Any]) -> list[dict[str, Any]]:
    rows = data_of(result)["results"]
    return [
        {key: row.get(key) for key in ("id", "preview", "similarity", "score")}
        for row in rows
    ]


def recall_projection(result: dict[str, Any]) -> dict[str, Any]:
    data = data_of(result)
    return {
        "status": data.get("status"),
        "mode": data.get("mode"),
        "recalls": [
            {key: item.get(key) for key in ("id", "content", "similarity", "score", "scope", "provenance")}
            for item in data.get("recalls", [])
        ],
    }


def check(condition: bool, message: str) -> dict[str, Any]:
    return {"passed": bool(condition), "message": message}


def write_config(path: Path, db: Path, journal: Path, project_root: Path, mode: str = "inject") -> None:
    config = {
        "version": 1,
        "projectRoot": str(project_root),
        "dbPath": str(db),
        "statePath": str(journal),
        "domain": "compatibility-probe",
        "scopes": ["Workspace"],
        "mode": mode,
        "agentId": None,
        "maxSensitivity": "Low",
        "maxContextTokens": 600,
        "minSimilarity": 0.2,
        "learningEnabled": False,
        "includeRecentContext": False,
    }
    path.write_text(json.dumps(config, indent=2), encoding="utf-8")


def _main_impl(args: argparse.Namespace, output: Path, report: dict[str, Any]) -> int:
    output.mkdir(parents=True, exist_ok=True)
    runtime = output / "runtime"
    runtime.mkdir(exist_ok=True)
    report["binaries"] = {name: {"path": f"<{name}-cli>", "sha256": hashlib.sha256(path.read_bytes()).hexdigest()} for name, path in (("old", args.old_cli), ("new", args.new_cli))}

    with tempfile.TemporaryDirectory(prefix="compatibility-probe-", dir=runtime) as temp_name:
        seed_root = Path(temp_name)
        seed_db = seed_root / "seed.sqlite3"
        seed_cwd = seed_root / "seed-cwd"
        seed_cwd.mkdir()
        facts = [
            ("Apollo uses SQLite for local persistence.", "workspace"),
            ("Cobalt is the review codename.", "workspace"),
            ("Private user memory must stay outside workspace recall.", "user"),
        ]
        add_results = []
        ids: list[str] = []
        for content, scope in facts:
            result = run_child(args.old_cli, ["--json", "add", "--db", str(seed_db), "--scope", scope, "--importance", "0.8", content], seed_cwd)
            add_results.append(result)
            if result["exitCode"] == 0:
                ids.append(data_of(result)["memory"]["id"])
        report["probes"]["seed"] = add_results
        if len(ids) != len(facts):
            report["errors"].append("seed failed; no provider fallback was attempted")
        else:
            with sqlite3.connect(seed_db) as connection:
                connection.execute("UPDATE memories SET agent_id='other-agent' WHERE content LIKE 'Cobalt %'")
                connection.commit()
            connection.close()

        with socketserver.ThreadingTCPServer(("127.0.0.1", 0), FixtureHandler) as server:
            server.daemon_threads = True
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                port = server.server_address[1]
                for label, executable in (("old", args.old_cli), ("new", args.new_cli)):
                    root = runtime / label
                    root.mkdir(exist_ok=True)
                    db = root / "memory.sqlite3"
                    shutil.copy2(seed_db, db)
                    cwd = root / "cwd"
                    cwd.mkdir()
                    reembed = run_child(executable, ["--json", "reembed", "--db", str(db), "--scope", "workspace", "--embedding-provider", "openai", "--openai-api-key", "synthetic-fixture", "--openai-model", "fixture", "--openai-url", f"http://127.0.0.1:{port}", "--openai-dimensions", str(DIMENSIONS)], cwd)
                    report["probes"].setdefault("reembed", {})[label] = reembed
                    report["checks"].append(check(reembed["exitCode"] == 0, f"{label} reembed succeeded against loopback fixture"))
                    reembed_user = run_child(executable, ["--json", "reembed", "--db", str(db), "--scope", "user", "--embedding-provider", "openai", "--openai-api-key", "synthetic-fixture", "--openai-model", "fixture", "--openai-url", f"http://127.0.0.1:{port}", "--openai-dimensions", str(DIMENSIONS)], cwd)
                    report["probes"].setdefault("reembedUser", {})[label] = reembed_user
                    report["checks"].append(check(reembed_user["exitCode"] == 0, f"{label} user reembed succeeded against loopback fixture"))
                    with sqlite3.connect(db) as connection:
                        connection.execute("UPDATE memories SET created_at='2099-01-01T00:00:00+00:00', updated_at='2099-01-01T00:00:00+00:00'")
                        connection.commit()

                    search = run_child(executable, ["--json", "search", "--db", str(db), "--scope", "workspace", "Apollo", "--limit", "10"], cwd)
                    report["probes"].setdefault("ordinarySearch", {})[label] = search
                    report["probes"].setdefault("ordinarySearchProjection", {})[label] = search_projection(search) if search["exitCode"] == 0 else None

                    config = root / "contextual-vector.json"
                    journal = root / "recall-vector-journal.sqlite3"
                    write_config(config, db, journal, root)
                    vector_request = {"cwd": str(cwd), "sessionId": "compatibility-session", "turnId": "vector-only", "prompt": "", "recentContext": [], "embedding": vector_for(facts[0][0])}
                    vector_before = source_snapshot(db)
                    contextual = run_with_stdin(executable, ["--json", "--non-interactive", "contextual-recall", "--config", str(config)], cwd, vector_request)
                    vector_after = source_snapshot(db)
                    report["probes"].setdefault("contextualRecall", {})[label] = contextual
                    report["probes"].setdefault("contextualRecallProjection", {})[label] = recall_projection(contextual) if contextual["exitCode"] == 0 else None
                    report["checks"].append(check(contextual["exitCode"] == 0, f"{label} contextual vector-only succeeded"))
                    if contextual["exitCode"] == 0:
                        report["checks"].append(check(bool(report["probes"]["contextualRecallProjection"][label]["recalls"]), f"{label} contextual vector-only returned results"))
                    report["checks"].append(check(invariant(vector_before, vector_after), f"{label} vector-only leaves source DB and WAL unchanged"))
                    hybrid_config = root / "contextual-hybrid.json"
                    hybrid_journal = root / "recall-hybrid-journal.sqlite3"
                    write_config(hybrid_config, db, hybrid_journal, root)
                    before = source_snapshot(db)
                    hybrid_request = {"cwd": str(cwd), "sessionId": "hybrid-session", "turnId": "hybrid", "prompt": "Apollo", "recentContext": [], "embedding": vector_for(facts[0][0])}
                    hybrid = run_with_stdin(executable, ["--json", "--non-interactive", "contextual-recall", "--config", str(hybrid_config)], cwd, hybrid_request)
                    after = source_snapshot(db)
                    report["probes"].setdefault("hybrid", {})[label] = hybrid
                    report["checks"].append(check(hybrid["exitCode"] == 0, f"{label} hybrid contextual recall succeeded"))
                    report["checks"].append(check(invariant(before, after), f"{label} hybrid leaves source DB and WAL unchanged"))
                    if label == "new":
                        if hybrid["exitCode"] == 0:
                            projection = recall_projection(hybrid)
                            report["checks"].append(check(bool(projection["recalls"]), "new hybrid returns non-empty results"))
                            report["checks"].append(check(any(item["id"] == ids[0] for item in projection["recalls"]), "hybrid keeps vector-selected workspace memory"))
                            report["checks"].append(check(all(item["id"] != ids[2] for item in projection["recalls"]), "hybrid preserves workspace scope filter"))
                            report["checks"].append(check(all(item["id"] != ids[1] for item in projection["recalls"]), "hybrid preserves agent ownership filter"))
                        report["checks"].append(check(invariant(before, after), "contextual recall leaves source DB and WAL unchanged"))

                old_search = report["probes"]["ordinarySearchProjection"].get("old")
                new_search = report["probes"]["ordinarySearchProjection"].get("new")
                report["checks"].append(check(report["probes"]["ordinarySearch"]["old"]["exitCode"] == 0 and report["probes"]["ordinarySearch"]["new"]["exitCode"] == 0, "ordinary search old/new exit successfully"))
                report["checks"].append(check(bool(old_search) and bool(new_search), "ordinary search returns non-empty results"))
                report["checks"].append(check(old_search == new_search, "ordinary search IDs/content/similarity/score match"))
                old_recall = report["probes"]["contextualRecallProjection"].get("old")
                new_recall = report["probes"]["contextualRecallProjection"].get("new")
                report["checks"].append(check(report["probes"]["contextualRecall"]["old"]["exitCode"] == 0 and report["probes"]["contextualRecall"]["new"]["exitCode"] == 0, "contextual vector-only old/new exit successfully"))
                report["checks"].append(check(bool(old_recall and old_recall["recalls"]) and bool(new_recall and new_recall["recalls"]), "contextual vector-only returns non-empty results for both CLIs"))
                report["checks"].append(check(old_recall == new_recall, "contextual vector-only IDs/content/similarity/score match"))
            finally:
                server.shutdown()
                thread.join(timeout=5)

    report["fixture"] = {"requests": FixtureHandler.requests, "provider": "loopback-only", "dimensions": DIMENSIONS}
    report["status"] = "passed" if report["checks"] and all(item["passed"] for item in report["checks"]) and not report["errors"] else "failed"
    (output / "run.json").write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
    return 0 if report["status"] == "passed" else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old-cli", type=Path, required=True)
    parser.add_argument("--new-cli", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        raise SystemExit(f"refusing existing output directory: {output}")
    report = {"status": "failed", "checks": [], "probes": {}, "binaries": {}, "errors": []}
    CLI_LABELS.update({str(path.resolve()): f"<{name}-cli>" for name, path in (("old", args.old_cli), ("new", args.new_cli))})
    try:
        return _main_impl(args, output, report)
    except BaseException as exc:
        output.mkdir(parents=True, exist_ok=True)
        report["status"] = "failed"
        report["errors"].append(f"probe aborted: {type(exc).__name__}: {exc}")
        (output / "run.json").write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
        return 1


def run_with_stdin(executable: Path, args: list[str], cwd: Path, payload: dict[str, Any]) -> dict[str, Any]:
    command = [str(executable), *args]
    started = time.monotonic()
    try:
        result = subprocess.run(command, cwd=str(cwd), env=answer_runner_environment(), input=json.dumps(payload), capture_output=True, text=True, timeout=TIMEOUT_SECONDS, check=False)
        cli_label = CLI_LABELS[str(executable.resolve())]
        return {"command": [cli_label, *[normalize_path(item, cwd) for item in args]], "exitCode": result.returncode, "stdout": result.stdout, "stderr": result.stderr, "elapsedMs": round((time.monotonic() - started) * 1000, 3)}
    except subprocess.TimeoutExpired as exc:
        cli_label = CLI_LABELS[str(executable.resolve())]
        return {"command": [cli_label, *[normalize_path(item, cwd) for item in args]], "exitCode": 124, "stdout": decode_output(exc.stdout), "stderr": decode_output(exc.stderr) + "\nsubprocess timeout", "elapsedMs": round((time.monotonic() - started) * 1000, 3)}


if __name__ == "__main__":
    raise SystemExit(main())
