"""Fail-open Codex UserPromptSubmit adapter for contextual memory recall."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time


MAX_STDIN = 64 * 1024
MAX_CHILD_OUTPUT = 64 * 1024
MAX_CONFIG = 64 * 1024
MAX_CONTEXT_BYTES = 600
DEADLINE_SECONDS = 1.0


def _empty() -> None:
    sys.stdout.write("{}")


def _inside(path: Path, root: Path) -> bool:
    try:
        return os.path.commonpath((str(path), str(root))) == str(root)
    except (OSError, ValueError):
        return False


def _recent_context(event: dict, root: Path) -> list[str]:
    if not event.get("transcript_path") or not isinstance(event.get("transcript_path"), str):
        return []
    try:
        transcript = Path(event["transcript_path"]).resolve(strict=True)
        if not _inside(transcript, root) or not transcript.is_file():
            return []
        with transcript.open("rb") as handle:
            handle.seek(0, os.SEEK_END)
            size = handle.tell()
            start = max(0, size - MAX_STDIN)
            handle.seek(start)
            raw = handle.read(MAX_STDIN)
        values: list[str] = []
        current = event.get("prompt")
        lines = raw.decode("utf-8", errors="ignore").splitlines()
        if start > 0 and raw and not raw.startswith((b"\n", b"\r")):
            lines = lines[1:]
        for line in lines:
            try:
                record = json.loads(line)
                if record.get("type") != "response_item":
                    continue
                payload = record.get("payload")
                if not isinstance(payload, dict):
                    return []
                if payload.get("type") in {"tool_call", "function_call"}:
                    continue
                if payload.get("type") != "message" or payload.get("role") not in {"user", "assistant"}:
                    return []
                content = payload.get("content")
                if not isinstance(content, list):
                    return []
                for part in content:
                    if not isinstance(part, dict) or part.get("type") not in {"input_text", "output_text"}:
                        return []
                    text = part.get("text")
                    if not isinstance(text, str):
                        return []
                    if not text or text == current:
                        continue
                    values.append(text)
            except (TypeError, ValueError, UnicodeError):
                return []
        selected: list[str] = []
        total = 0
        for text in reversed(values):
            encoded_length = len(text.encode("utf-8"))
            if len(selected) >= 4 or total + encoded_length > 8000:
                continue
            selected.append(text)
            total += encoded_length
        return list(reversed(selected))
    except (OSError, ValueError):
        return []


def _valid_event(event: object) -> bool:
    if not isinstance(event, dict):
        return False
    if event.get("hook_event_name") != "UserPromptSubmit":
        return False
    return all(isinstance(event.get(key), str) for key in ("cwd", "session_id", "turn_id", "prompt"))


def _run_child(command: list[str], payload: bytes, deadline: float) -> bytes | None:
    process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    output = bytearray()
    overflow = threading.Event()

    def write_input() -> None:
        try:
            assert process.stdin is not None
            process.stdin.write(payload)
            process.stdin.close()
        except (BrokenPipeError, OSError):
            pass

    def read_output() -> None:
        try:
            assert process.stdout is not None
            while True:
                chunk = process.stdout.read(4096)
                if not chunk:
                    return
                output.extend(chunk)
                if len(output) > MAX_CHILD_OUTPUT:
                    overflow.set()
                    return
        except OSError:
            return

    writer = threading.Thread(target=write_input, daemon=True)
    reader = threading.Thread(target=read_output, daemon=True)
    writer.start()
    reader.start()
    while process.poll() is None:
        if overflow.is_set() or time.monotonic() >= deadline:
            process.kill()
            process.wait()
            return None
        time.sleep(min(0.01, max(0.001, deadline - time.monotonic())))
    process.wait()
    reader.join(timeout=max(0.0, deadline - time.monotonic()))
    writer.join(timeout=max(0.0, deadline - time.monotonic()))
    if overflow.is_set() or process.returncode != 0 or time.monotonic() >= deadline:
        return None
    return bytes(output)


def main() -> int:
    deadline = time.monotonic() + DEADLINE_SECONDS
    try:
        parser = argparse.ArgumentParser(add_help=False)
        parser.add_argument("--config", required=True)
        parser.add_argument("--executable", required=True)
        args = parser.parse_args()
        if not os.path.isabs(args.config) or not os.path.isabs(args.executable):
            _empty()
            return 0
        raw_event = sys.stdin.buffer.read(MAX_STDIN + 1)
        if len(raw_event) > MAX_STDIN:
            _empty()
            return 0
        event = json.loads(raw_event.decode("utf-8"))
        if not _valid_event(event):
            _empty()
            return 0
        with open(args.config, "rb") as handle:
            if os.fstat(handle.fileno()).st_size > MAX_CONFIG:
                _empty()
                return 0
            config = json.load(handle)
        if not isinstance(config, dict):
            _empty()
            return 0
        mode = config.get("mode", "off")
        if mode == "off" or mode not in {"inject", "observe"}:
            _empty()
            return 0
        project_root_value = config.get("projectRoot")
        if not isinstance(project_root_value, str) or not os.path.isabs(project_root_value):
            _empty()
            return 0
        root = Path(project_root_value).resolve(strict=True)
        transcript_root_value = config.get("transcriptRoot", project_root_value)
        if transcript_root_value is None:
            transcript_root_value = project_root_value
        if not isinstance(transcript_root_value, str) or not os.path.isabs(transcript_root_value):
            _empty()
            return 0
        transcript_root = Path(transcript_root_value).resolve(strict=True)
        cwd = Path(event["cwd"]).resolve(strict=True)
        if not cwd.is_dir() or not _inside(cwd, root):
            _empty()
            return 0
        max_context = config.get("maxContextTokens", 600)
        if not isinstance(max_context, int) or isinstance(max_context, bool):
            max_context = 600
        max_context = min(max(0, max_context), MAX_CONTEXT_BYTES)
        request = {
            "cwd": str(cwd),
            "sessionId": event["session_id"],
            "turnId": event["turn_id"],
            "prompt": event["prompt"],
            "recentContext": _recent_context(event, transcript_root) if config.get("includeRecentContext", False) is True else [],
        }
        if time.monotonic() >= deadline:
            _empty()
            return 0
        command = [args.executable, "contextual-recall", "--config", args.config, "--json", "--non-interactive"]
        if Path(args.executable).suffix.lower() == ".py":
            command = [sys.executable, args.executable, "contextual-recall", "--config", args.config, "--json", "--non-interactive"]
        child_raw = _run_child(command, json.dumps(request, ensure_ascii=False).encode("utf-8"), deadline)
        if child_raw is None:
            _empty()
            return 0
        envelope = json.loads(child_raw.decode("utf-8"))
        if time.monotonic() >= deadline:
            _empty()
            return 0
        data = envelope.get("data") if isinstance(envelope, dict) and envelope.get("status") == "ok" else None
        if mode != "inject" or not isinstance(data, dict) or data.get("schemaVersion") != "memory-contextual-recall/v1" or data.get("mode") != "inject" or data.get("status") != "selected":
            _empty()
            return 0
        context = data.get("additionalContext")
        if not isinstance(context, str) or not context or len(context.encode("utf-8")) > max_context:
            _empty()
            return 0
        sys.stdout.write(json.dumps({"hookSpecificOutput": {"hookEventName": "UserPromptSubmit", "additionalContext": context}}, ensure_ascii=False, separators=(",", ":")))
        return 0
    except Exception:
        _empty()
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
