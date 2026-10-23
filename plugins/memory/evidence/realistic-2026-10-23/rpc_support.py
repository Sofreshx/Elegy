"""Reusable MCP transport and byte snapshots from qualification commit 4ab2b26.
Only support functions are retained; this module never executes a campaign.
"""

from __future__ import annotations

from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
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

def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()

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
