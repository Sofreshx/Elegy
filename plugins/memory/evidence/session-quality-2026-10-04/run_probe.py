#!/usr/bin/env python3
"""Bounded, local qualification probe for the memory MCP and contextual recall CLI.

The probe deliberately uses only synthetic corpus data and child processes.  It
does not contact a provider; embedding boot is disabled in every child.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import threading
import time
from queue import Empty, Full, Queue

RPC_TIMEOUT = 30.0
MAX_CAPTURE = 64 * 1024
DROP_ENV = {
    "OPENAI_API_KEY", "OPENAI_BASE_URL", "OPENAI_URL", "OPENAI_API_BASE",
    "OLLAMA_URL", "OLLAMA_HOST", "ELEGY_OPENAI_API_KEY", "ELEGY_OPENAI_URL",
    "ELEGY_LLM_API_KEY", "ELEGY_LLM_URL", "ELEGY_EMBEDDING_URL",
    "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy",
}


def digest(value: object) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def exe_info(path: Path) -> dict[str, object]:
    return {"name": path.name, "sha256": sha256_file(path)}


def public_config(value: object) -> object:
    """Replace path-bearing values before hashing a config for public evidence."""
    if isinstance(value, dict):
        return {k: ("<path>" if k.lower().endswith(("path", "root", "cwd")) else public_config(v)) for k, v in value.items()}
    if isinstance(value, list):
        return [public_config(v) for v in value]
    return value


def redact_paths(value: object, paths: list[str]) -> object:
    if isinstance(value, str):
        result = value
        for path in paths:
            if path:
                result = result.replace(path, "<local-path>")
        return result
    if isinstance(value, dict):
        return {key: redact_paths(child, paths) for key, child in value.items()}
    if isinstance(value, list):
        return [redact_paths(child, paths) for child in value]
    return value


def child_env(db: Path, agent: str) -> dict[str, str]:
    env = {k: v for k, v in os.environ.items() if k not in DROP_ENV}
    env.update({
        "ELEGY_DB_PATH": str(db), "ELEGY_MCP_AGENT_ID": agent,
        "ELEGY_MCP_READ_SCOPE": "agent", "ELEGY_EMBEDDING_BOOT_POLICY": "off",
        "ELEGY_ALLOW_NO_EMBEDDINGS": "true",
    })
    return env


class RpcClient:
    def __init__(self, executable: Path, env: dict[str, str], record: list[dict[str, object]], label: str):
        self.label, self.record, self.next_id = label, record, 1
        self.proc = subprocess.Popen([str(executable)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, env=env, text=True, bufsize=1)
        self.stdout_queue: Queue[str | None] = Queue(maxsize=256)
        self.stderr_queue: Queue[str | None] = Queue(maxsize=256)
        threading.Thread(target=self._reader, args=(self.proc.stdout, self.stdout_queue), daemon=True).start()
        threading.Thread(target=self._reader, args=(self.proc.stderr, self.stderr_queue), daemon=True).start()

    @staticmethod
    def _reader(stream: object, queue: Queue[str | None]) -> None:
        try:
            for line in stream:
                try:
                    queue.put_nowait(line)
                except Full:
                    try:
                        queue.get_nowait()
                    except Empty:
                        pass
                    queue.put_nowait(line)
        finally:
            try:
                queue.put_nowait(None)
            except Full:
                pass

    def call(self, method: str, params: object | None = None, notification: bool = False) -> object:
        ident = None if notification else self.next_id
        if not notification:
            self.next_id += 1
        request = {"jsonrpc": "2.0", "method": method}
        if ident is not None:
            request["id"] = ident
        if params is not None:
            request["params"] = params
        started = time.monotonic()
        self.proc.stdin.write(json.dumps(request, separators=(",", ":")) + "\n")
        self.proc.stdin.flush()
        if notification:
            self.record.append({"client": self.label, "timestamp": datetime.now(timezone.utc).isoformat(), "request": request, "response": None, "elapsedMs": 0.0})
            return {}
        deadline = started + RPC_TIMEOUT
        response = None
        while time.monotonic() < deadline:
            try:
                line = self.stdout_queue.get(timeout=max(0.01, deadline - time.monotonic()))
            except Empty:
                break
            if line is None:
                break
            try:
                candidate = json.loads(line)
            except json.JSONDecodeError:
                continue
            if candidate.get("id") == ident:
                response = candidate
                break
        elapsed = round((time.monotonic() - started) * 1000, 3)
        item = {"client": self.label, "timestamp": datetime.now(timezone.utc).isoformat(), "request": request, "response": response, "elapsedMs": elapsed}
        self.record.append(item)
        if response is None:
            raise RuntimeError(f"MCP {self.label} timed out or exited during {method}")
        if "error" in response or response.get("result", {}).get("isError") is True:
            raise RuntimeError(f"MCP {method} returned an error")
        result = response.get("result", {})
        if method == "tools/call":
            for part in result.get("content", []):
                if part.get("type") == "text":
                    try:
                        return json.loads(part["text"])
                    except (TypeError, json.JSONDecodeError):
                        pass
        return result

    def start(self) -> None:
        self.call("initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "qualification-probe", "version": "1"}})
        self.call("notifications/initialized", notification=True)

    def tool(self, name: str, arguments: dict[str, object]) -> object:
        return self.call("tools/call", {"name": name, "arguments": arguments})

    def close(self) -> dict[str, object]:
        graceful = False
        forced = False
        if self.proc.poll() is None:
            try:
                self.proc.stdin.close()
            except (OSError, ValueError):
                pass
            try:
                self.proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                forced = True
                self.proc.terminate()
                try:
                    self.proc.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    self.proc.kill()
                    self.proc.wait(timeout=3)
            else:
                graceful = True
        else:
            graceful = True
        stderr_parts = []
        while True:
            try:
                line = self.stderr_queue.get_nowait()
            except Empty:
                break
            if line is None:
                break
            stderr_parts.append(line)
        stderr = "".join(stderr_parts)
        result = {"exitCode": self.proc.returncode, "stderr": stderr[-MAX_CAPTURE:], "graceful": graceful, "forced": forced}
        self.record.append({"client": self.label, "timestamp": datetime.now(timezone.utc).isoformat(), "process": result})
        return result


def extract_id(value: object) -> str:
    if isinstance(value, dict):
        for key in ("id", "memoryId"):
            if isinstance(value.get(key), str):
                return value[key]
        for child in value.values():
            found = extract_id(child)
            if found:
                return found
    if isinstance(value, list):
        for child in value:
            found = extract_id(child)
            if found:
                return found
    return ""


def memory_items(value: object) -> list[dict[str, object]]:
    if isinstance(value, dict):
        for key in ("memories", "results", "items"):
            if isinstance(value.get(key), list):
                return [x for x in value[key] if isinstance(x, dict)]
    return []


def recalled_memory(value: object, identifier: str, content: str) -> bool:
    return isinstance(value, dict) and value.get("found") is True and isinstance(value.get("memory"), dict) and value["memory"].get("id") == identifier and value["memory"].get("content") == content


def search_contains(value: object, identifier: str, content: str) -> bool:
    return any(row.get("id") == identifier and (content in str(row.get("preview", "")) or content == row.get("content")) for row in memory_items(value))


def find_field(value: object, field: str) -> object | None:
    if isinstance(value, dict):
        if field in value:
            return value[field]
        for child in value.values():
            found = find_field(child, field)
            if found is not None:
                return found
    if isinstance(value, list):
        for child in value:
            found = find_field(child, field)
            if found is not None:
                return found
    return None


def validate_corpus(raw: object) -> dict[str, object]:
    if not isinstance(raw, dict) or not isinstance(raw.get("memories"), list) or not isinstance(raw.get("cases"), list):
        raise ValueError("corpus must contain memories and cases arrays")
    seen = set()
    for memory in raw["memories"]:
        if not isinstance(memory, dict) or not isinstance(memory.get("key"), str) or not isinstance(memory.get("content"), str):
            raise ValueError("each memory needs string key and content")
        if memory["key"] in seen or memory.get("agent") not in {"qual-a", "qual-b"}:
            raise ValueError("memory keys must be unique and agent must be qual-a or qual-b")
        seen.add(memory["key"])
    for case in raw["cases"]:
        if not isinstance(case, dict) or not all(isinstance(case.get(k), str) for k in ("id", "question", "expected")):
            raise ValueError("each case needs string id, question, and expected")
        if "forbidden" in case and not isinstance(case["forbidden"], list):
            raise ValueError("case forbidden must be an array")
    return raw


def source_files(db: Path) -> list[Path]:
    return [p for p in (db, Path(str(db) + "-wal"), Path(str(db) + "-shm")) if p.exists()]


def source_snapshot(db: Path) -> dict[str, object]:
    """Report physical files while comparing only durable DB/WAL data."""
    states = {}
    for label, path in (("main", db), ("wal", Path(str(db) + "-wal")), ("shm", Path(str(db) + "-shm"))):
        if not path.exists():
            states[label] = {"present": False, "size": 0, "sha256": None}
        else:
            size = path.stat().st_size
            states[label] = {"present": True, "size": size, "sha256": sha256_file(path)}
    # SQLite may create an empty WAL while opening a read-only database.  An
    # absent WAL and a zero-length WAL carry the same durable data.
    durable_wal = states["wal"]["sha256"] if states["wal"]["size"] else None
    states["durableData"] = {"mainSha256": states["main"]["sha256"], "walSha256": durable_wal}
    return states


def source_data_unchanged(before: dict[str, object], after: dict[str, object]) -> bool:
    return before.get("durableData") == after.get("durableData")


def sqlite_snapshot(db: Path) -> dict[str, object]:
    if not db.exists():
        return {"available": False}
    try:
        uri = f"file:{db.as_posix()}?mode=ro"
        with sqlite3.connect(uri, uri=True, timeout=2) as con:
            tables = [row[0] for row in con.execute("select name from sqlite_master where type='table' order by name")]
            counts = {name: con.execute(f'SELECT count(*) FROM "{name}"').fetchone()[0] for name in tables}
        return {"available": True, "tables": counts}
    except Exception as exc:
        return {"available": False, "error": type(exc).__name__}


def run(args: argparse.Namespace) -> int:
    output = Path(args.output).absolute()
    if output.exists():
        raise ValueError("output must be a new, non-existent directory")
    corpus_path = Path(args.corpus).absolute()
    corpus = validate_corpus(json.loads(corpus_path.read_text(encoding="utf-8")))
    mcp = Path(args.mcp).absolute()
    cli = Path(args.cli).absolute()
    if not mcp.is_file() or not cli.is_file():
        raise ValueError("--mcp and --cli must name existing executable files")
    output.mkdir(parents=True)
    runtime = output / "runtime"
    runtime.mkdir()
    db = runtime / "probe.sqlite"
    records: list[dict[str, object]] = []
    run_doc: dict[str, object] = {"schemaVersion": "memory-session-quality/v1", "status": "running", "protocol": "local-stdio-child-process", "synthetic": True, "createdAt": datetime.now(timezone.utc).isoformat(), "runnerSha256": sha256_file(Path(__file__).absolute()), "corpusDigest": digest(corpus), "executables": {"mcp": exe_info(mcp), "cli": exe_info(cli)}, "configDigest": digest(public_config({"agent": "qual-a", "scope": "agent", "embeddingBootPolicy": "off", "allowNoEmbeddings": True})), "rpc": records, "checks": [], "mapping": {}, "cases": []}
    run_path = output / "run.json"

    def save(status: str | None = None) -> None:
        if status:
            run_doc["status"] = status
        safe = redact_paths(run_doc, [str(runtime), str(output), str(mcp), str(cli), str(corpus_path)])
        run_path.write_text(json.dumps(safe, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    save()
    clients: list[RpcClient] = []
    seeded: dict[str, str] = {}
    try:
        for agent in ("qual-a", "qual-b"):
            client = RpcClient(mcp, child_env(db, agent), records, agent)
            clients.append(client)
            client.start()
            for memory in [m for m in corpus["memories"] if m["agent"] == agent]:
                response = client.tool("memory_store", {"content": memory["content"], "memoryType": "fact", "provenance": "user-stated", "importance": 0.9})
                identifier = extract_id(response)
                if not identifier:
                    raise RuntimeError(f"memory_store did not return an id for {memory['key']}")
                seeded[memory["key"]] = identifier
                run_doc["mapping"][memory["key"]] = {"id": identifier, "agent": agent}
                if memory.get("correction"):
                    client.tool("memory_correct", {"id": identifier, "content": memory["correction"], "reason": "synthetic qualification correction"})
        # Close both seed clients before the fresh-session checks.
        run_doc["processes"] = [client.close() for client in clients]
        clients.clear()
        fresh_a = RpcClient(mcp, child_env(db, "qual-a"), records, "qual-a-reopen")
        clients.append(fresh_a)
        fresh_a.start()
        a_memory = next((m for m in corpus["memories"] if m["agent"] == "qual-a"), None)
        if a_memory:
            by_id = fresh_a.tool("memory_recall", {"id": seeded[a_memory["key"]]})
            search = fresh_a.tool("memory_search", {"query": a_memory["content"], "limit": 10})
            run_doc["checks"].append({"name": "qual-a-persistence", "passed": recalled_memory(by_id, seeded[a_memory["key"]], a_memory["content"]) and search_contains(search, seeded[a_memory["key"]], a_memory["content"])})
        fresh_b = RpcClient(mcp, child_env(db, "qual-b"), records, "qual-b-reopen")
        clients.append(fresh_b)
        fresh_b.start()
        b_memory = next((m for m in corpus["memories"] if m["agent"] == "qual-b"), None)
        if a_memory:
            b_id = fresh_b.tool("memory_recall", {"id": seeded[a_memory["key"]]})
            b_search = fresh_b.tool("memory_search", {"query": a_memory["content"], "limit": 20})
            b_list = fresh_b.tool("memory_list", {"limit": 100})
            visible = json.dumps([b_id, b_search, b_list])
            run_doc["checks"].append({"name": "qual-b-isolation", "passed": b_id.get("found") is False and not search_contains(b_search, seeded[a_memory["key"]], a_memory["content"]) and not any(row.get("id") == seeded[a_memory["key"]] for row in memory_items(b_list)), "candidateForbiddenTerms": [a_memory["content"]] if a_memory["content"] in visible else []})
        if b_memory:
            own = fresh_b.tool("memory_search", {"query": b_memory["content"], "limit": 10})
            run_doc["checks"].append({"name": "qual-b-positive-control", "passed": search_contains(own, seeded[b_memory["key"]], b_memory["content"])})
        for client in clients:
            client.close()
        clients.clear()

        packets: list[dict[str, object]] = []
        case_failure = False
        for case in corpus["cases"]:
            config = {"version": 1, "projectRoot": str(runtime), "dbPath": str(db), "statePath": str(runtime / f"journal-{case['id']}.json"), "domain": "qualification", "scopes": ["Agent"], "mode": "inject", "agentId": "qual-a", "maxSensitivity": "Low", "maxContextTokens": 600, "minSimilarity": 0.2, "learningEnabled": False, "includeRecentContext": False}
            config_path = runtime / f"config-{case['id']}.json"
            config_path.write_text(json.dumps(config), encoding="utf-8")
            before = source_snapshot(db)
            request = {"cwd": str(runtime), "sessionId": f"session-{case['id']}", "turnId": f"turn-{case['id']}", "prompt": case["question"], "recentContext": []}
            public_request = {**request, "cwd": "<runtime>"}
            started = time.monotonic()
            proc = subprocess.run([str(cli), "--json", "--non-interactive", "contextual-recall", "--config", str(config_path)], input=json.dumps(request), text=True, capture_output=True, timeout=RPC_TIMEOUT, env=child_env(db, "qual-a"))
            elapsed = round((time.monotonic() - started) * 1000, 3)
            after = source_snapshot(db)
            try:
                parsed = json.loads(proc.stdout)
            except json.JSONDecodeError:
                parsed = {"raw": proc.stdout[-MAX_CAPTURE:]}
            packet = {"caseId": case["id"], "request": public_request, "response": parsed, "exitCode": proc.returncode, "stderr": proc.stderr[-MAX_CAPTURE:], "timestamp": datetime.now(timezone.utc).isoformat(), "elapsedMs": elapsed, "sourceHashesBefore": before, "sourceHashesAfter": after, "sourceUnchanged": source_data_unchanged(before, after), "sidecarChanged": before != after}
            recalls = find_field(parsed, "recalls") or []
            context = find_field(parsed, "additionalContext") or ""
            recalled_text = json.dumps({"recalls": recalls, "additionalContext": context}, ensure_ascii=False)
            expected_id = seeded.get(case.get("expectedMemoryKey")) if case.get("expectedMemoryKey") else None
            packet["checks"] = {
                "expectedTextFound": case["expected"] in recalled_text,
                "expectedConfigured": bool(case["expected"]),
                "forbiddenAbsent": all(term not in recalled_text for term in case.get("forbidden", [])),
                "expectedMemoryIdPresent": expected_id is None or expected_id in recalled_text,
            }
            case_failure = case_failure or proc.returncode != 0 or not source_data_unchanged(before, after)
            packets.append(packet)
            run_doc["cases"].append({"id": case["id"], "exitCode": proc.returncode, "sourceUnchanged": packet["sourceUnchanged"], "elapsedMs": elapsed, "checks": packet["checks"]})
        safe_packets = redact_paths(packets, [str(runtime), str(output), str(mcp), str(cli), str(corpus_path)])
        (output / "packets.json").write_text(json.dumps(safe_packets, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        run_doc["sqliteSnapshot"] = sqlite_snapshot(db)
        run_doc["status"] = "failed" if case_failure or any(not check.get("passed", False) for check in run_doc["checks"]) else "completed"
    except Exception as exc:
        run_doc["error"] = {"type": type(exc).__name__, "message": str(exc)[:500]}
        run_doc["status"] = "failed"
        raise
    finally:
        for client in clients:
            try:
                client.close()
            except Exception as exc:
                run_doc.setdefault("cleanupFailures", []).append(type(exc).__name__)
        cleanup = []
        if seeded:
            for agent in ("qual-a", "qual-b"):
                cleanup_client = None
                try:
                    cleanup_client = RpcClient(mcp, child_env(db, agent), records, f"{agent}-cleanup")
                    cleanup_client.start()
                    for key, identifier in seeded.items():
                        if run_doc["mapping"].get(key, {}).get("agent") != agent:
                            continue
                        try:
                            cleanup.append({"agent": agent, "key": key, "deleted": cleanup_client.tool("memory_delete", {"id": identifier})})
                        except Exception as exc:
                            cleanup.append({"agent": agent, "key": key, "error": type(exc).__name__})
                    try:
                        remaining = cleanup_client.tool("memory_list", {"limit": 100})
                        cleanup.append({"agent": agent, "remaining": len(memory_items(remaining)), "verifiedEmpty": not memory_items(remaining)})
                    except Exception as exc:
                        cleanup.append({"agent": agent, "verificationError": type(exc).__name__})
                    cleanup_client.close()
                except Exception as exc:
                    cleanup.append({"agent": agent, "error": type(exc).__name__})
                    if cleanup_client:
                        cleanup_client.close()
        run_doc["cleanup"] = cleanup
        if any(item.get("error") or item.get("verificationError") or item.get("verifiedEmpty") is False for item in cleanup):
            run_doc["status"] = "failed"
        save(run_doc.get("status", "failed"))
    return 1 if run_doc.get("status") == "failed" else 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Run an isolated synthetic memory qualification probe.")
    parser.add_argument("--mcp", required=True, help="absolute path to the memory MCP executable")
    parser.add_argument("--cli", required=True, help="absolute path to the memory CLI executable")
    parser.add_argument("--corpus", required=True, help="corpus JSON file")
    parser.add_argument("--output", required=True, help="new output directory")
    args = parser.parse_args()
    try:
        return run(args)
    except Exception as exc:
        print(f"probe failed: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
