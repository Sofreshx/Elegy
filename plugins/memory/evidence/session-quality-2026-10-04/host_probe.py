#!/usr/bin/env python3
"""Fresh-session qualification harness for installed Codex and OpenCode hosts.

The harness is intentionally boring: it creates per-run directories, invokes
each host in a new process, records raw JSON/text output, and removes every
synthetic memory through the local MCP protocol in a finally block.  It never
touches a user's host configuration or credentials.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
import uuid

HERE = Path(__file__).resolve().parent
DROP = {"OPENAI_API_KEY", "OPENAI_BASE_URL", "OPENAI_URL", "OPENAI_API_BASE",
        "ELEGY_OPENAI_API_KEY", "ELEGY_LLM_API_KEY", "ELEGY_LLM_URL",
        "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"}


def now() -> str:
    return datetime.now(timezone.utc).isoformat()


def sha(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def redact(value: object, paths: list[str]) -> object:
    if isinstance(value, str):
        result = value
        for path in paths:
            if path:
                result = result.replace(path, "<local-path>")
        return result
    if isinstance(value, dict):
        return {key: redact(child, paths) for key, child in value.items()}
    if isinstance(value, list):
        return [redact(child, paths) for child in value]
    return value


def executable(name: str, supplied: str | None) -> Path:
    found = Path(supplied).resolve() if supplied else (Path(shutil.which(name)) if shutil.which(name) else None)
    if not found or not found.is_file():
        raise RuntimeError(f"{name} executable is unavailable")
    return found


def command_version(path: Path) -> dict[str, object]:
    try:
        result = subprocess.run([str(path), "--version"], capture_output=True, text=True, timeout=15)
        return {"exitCode": result.returncode, "stdout": result.stdout[-4000:], "stderr": result.stderr[-4000:]}
    except Exception as exc:
        return {"error": type(exc).__name__}


def base_env(root: Path, db: Path, agent: str) -> dict[str, str]:
    env = {k: v for k, v in os.environ.items()
           if k not in DROP and not k.endswith("_API_KEY") and not k.startswith("OPENCODE_")}
    env.update({"ELEGY_DB_PATH": str(db), "ELEGY_MCP_AGENT_ID": agent,
                "ELEGY_MCP_READ_SCOPE": "agent", "ELEGY_EMBEDDING_BOOT_POLICY": "off",
                "ELEGY_ALLOW_NO_EMBEDDINGS": "true", "OPENCODE_DISABLE_INSTALL": "true",
                "XDG_CONFIG_HOME": str(root / "xdg-config"), "XDG_DATA_HOME": str(root / "xdg-data"),
                "XDG_STATE_HOME": str(root / "xdg-state"), "OPENCODE_CONFIG_DIR": str(root / "opencode-config")})
    return env


def codex_overrides(mcp: Path, db: Path, agent: str) -> list[str]:
    # TOML values are passed as individual overrides; no user config is read.
    def toml_string(value: str) -> str:
        return json.dumps(value, ensure_ascii=True)
    env = "{" + ",".join(f"{key}={toml_string(value)}" for key, value in {
        "ELEGY_DB_PATH": str(db), "ELEGY_MCP_AGENT_ID": agent,
        "ELEGY_MCP_READ_SCOPE": "agent", "ELEGY_EMBEDDING_BOOT_POLICY": "off",
        "ELEGY_ALLOW_NO_EMBEDDINGS": "true"}.items()) + "}"
    overrides = ["-c", f'mcp_servers.memory.command={toml_string(str(mcp))}',
            "-c", f"mcp_servers.memory.env={env}",
            "-c", 'mcp_servers.memory.enabled_tools=["memory_store","memory_recall","memory_search","memory_list"]']
    # Per-process approval for the exact tools in the user-authorized synthetic
    # experiment. This neither modifies user config nor relaxes shell sandboxing.
    for tool in ("memory_store", "memory_recall", "memory_search", "memory_list"):
        overrides += ["-c", f'mcp_servers.memory.tools.{tool}.approval_mode="approve"']
    return overrides


def opencode_config(mcp: Path, db: Path, agent: str) -> dict[str, object]:
    return {"mcp": {"memory": {"type": "local", "command": [str(mcp)], "enabled": True,
                                  "environment": {"ELEGY_DB_PATH": str(db), "ELEGY_MCP_AGENT_ID": agent,
                                                   "ELEGY_MCP_READ_SCOPE": "agent", "ELEGY_EMBEDDING_BOOT_POLICY": "off",
                                                   "ELEGY_ALLOW_NO_EMBEDDINGS": "true"}}},
            "model": "opencode/big-pickle", "small_model": "opencode/big-pickle",
            "enabled_providers": ["opencode"], "share": "disabled", "autoupdate": False,
            "plugin": [], "permission": {"*": "deny", "memory_*": "allow"}}


def run_cmd(argv: list[str], env: dict[str, str], cwd: Path, timeout: int = 180) -> dict[str, object]:
    started = time.monotonic()
    try:
        proc = subprocess.run(argv, env=env, cwd=str(cwd), capture_output=True, text=True, timeout=timeout)
        return {"argv": argv, "exitCode": proc.returncode, "stdout": proc.stdout, "stderr": proc.stderr,
                "elapsedMs": round((time.monotonic() - started) * 1000, 3), "timedOut": False}
    except subprocess.TimeoutExpired as exc:
        return {"argv": argv, "exitCode": None, "stdout": exc.stdout or "", "stderr": exc.stderr or "",
                "elapsedMs": round((time.monotonic() - started) * 1000, 3), "timedOut": True}


def preflight(args: argparse.Namespace) -> int:
    root = Path(args.scratch).resolve() if args.scratch else Path.cwd() / ".host-probe-preflight"
    root.mkdir(parents=True, exist_ok=True)
    codex = executable("codex", args.codex)
    opencode = executable("opencode", args.opencode)
    mcp = Path(args.mcp).resolve()
    if not mcp.is_file():
        raise RuntimeError("memory MCP executable is unavailable")
    report: dict[str, object] = {"schemaVersion": "memory-host-preflight/v1", "timestamp": now(),
        "executables": {"codex": {"path": str(codex), "sha256": sha(codex), "version": command_version(codex)},
                        "opencode": {"path": str(opencode), "sha256": sha(opencode), "version": command_version(opencode)},
                        "memoryMcp": {"path": str(mcp), "sha256": sha(mcp)}},
        "models": {"codex": "gpt-5.6-luna", "opencode": "opencode/big-pickle"},
        "isolation": {"codexIgnoreUserConfig": True, "codexSandbox": "read-only", "opencodePure": True,
                       "opencodePlugins": [], "opencodeInstallDisabled": True}, "checks": []}
    config = opencode_config(mcp, root / "preflight.sqlite", "qual-a")
    config_text = json.dumps(config, sort_keys=True, separators=(",", ":"))
    report["configSha256"] = hashlib.sha256(config_text.encode()).hexdigest()
    report["checks"].append({"name": "opencode-config-memory-only", "passed": set(config["mcp"]) == {"memory"} and config["plugin"] == []})
    env = base_env(root, root / "preflight.sqlite", "qual-a")
    env["OPENCODE_CONFIG_CONTENT"] = config_text
    codex_help = run_cmd([str(codex), "exec", "--help"], env, root, timeout=30)
    report["checks"].append({"name": "codex-user-config-exclusion-supported",
        "passed": codex_help["exitCode"] == 0 and "--ignore-user-config" in codex_help["stdout"]
                  and "auth still uses" in codex_help["stdout"],
        "limitation": "Installed CLI help and authored overrides; actual startup MCP inventory must also be reviewed.",
        "result": codex_help})
    # This is a non-model invocation. It proves the installed command accepts
    # pure mode and the requested model; failure blocks run rather than falling back.
    debug = run_cmd([str(opencode), "--pure", "debug", "config"], env, root, timeout=30)
    debug_text = str(debug["stdout"])
    resolved = json.loads(debug_text) if debug["exitCode"] == 0 else {}
    resolved_ok = (set(resolved.get("mcp", {})) == {"memory"}
                   and resolved.get("mcp") == config["mcp"]
                   and resolved.get("plugin") == []
                   and resolved.get("permission") == config["permission"]
                   and resolved.get("enabled_providers") == ["opencode"]
                   and resolved.get("model") == "opencode/big-pickle"
                   and resolved.get("small_model") == "opencode/big-pickle")
    report["checks"].append({"name": "opencode-resolved-config", "passed": resolved_ok, "result": debug})
    probe = run_cmd([str(opencode), "--pure", "models", "opencode", "--verbose"], env, root, timeout=30)
    model_text = str(probe["stdout"]) + str(probe["stderr"])
    model = {}
    if "opencode/big-pickle" in model_text:
        tail = model_text.split("opencode/big-pickle", 1)[1].lstrip()
        model, _ = json.JSONDecoder().raw_decode(tail)
    free = (model.get("id") == "big-pickle" and model.get("providerID") == "opencode"
            and model.get("status") == "active"
            and model.get("cost") == {"input": 0, "output": 0, "cache": {"read": 0, "write": 0}})
    report["checks"].append({"name": "opencode-free-model-listed", "passed": probe["exitCode"] == 0 and free,
        "model": model, "result": {"exitCode": probe["exitCode"]}})
    login = run_cmd([str(codex), "login", "status"], env, root, timeout=30)
    report["checks"].append({"name": "codex-chatgpt-auth", "passed": login["exitCode"] == 0 and "chatgpt" in (str(login["stdout"]) + str(login["stderr"])).lower(), "result": login})
    out = Path(args.output).resolve() if args.output else root / "preflight.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(redact(report, [str(root), str(Path.home()), str(codex.parent), str(opencode.parent), str(mcp)]), indent=2, sort_keys=True) + "\n", encoding="utf-8")
    ok = all(bool(x.get("passed")) for x in report["checks"])
    print(json.dumps({"status": "ready" if ok else "blocked", "path": str(out)}))
    return 0 if ok else 2


def extract_id(text: str) -> str:
    patterns = [r'"(?:memoryId|id)"\s*:\s*"([A-Za-z0-9_-]{8,})"', r"memory[_ -]?id\s*[:=]\s*([A-Za-z0-9_-]{8,})"]
    for pattern in patterns:
        match = re.search(pattern, text, re.I)
        if match:
            return match.group(1)
    return ""


def cleanup(mcp: Path, db: Path, ids: dict[str, str], root: Path) -> dict[str, object]:
    sys.path.insert(0, str(HERE))
    from run_probe import RpcClient, child_env, memory_items
    result: dict[str, object] = {"deleted": [], "errors": []}
    for agent in ("qual-a", "qual-b"):
        client = None
        try:
            client = RpcClient(mcp, child_env(db, agent), [], f"{agent}-host-cleanup")
            client.start()
            for key, value in ids.items():
                if value.startswith(agent + ":"):
                    result["deleted"].append({"key": key, "agent": agent, "result": client.tool("memory_delete", {"id": value.split(":", 1)[1]})})
            listed = client.tool("memory_list", {"limit": 1000, "includeDormant": True})
            for item in memory_items(listed):
                item_id = item.get("id")
                if isinstance(item_id, str):
                    result["deleted"].append({"agent": agent, "id": item_id, "result": client.tool("memory_delete", {"id": item_id})})
            remaining = client.tool("memory_list", {"limit": 1000, "includeDormant": True})
            result.setdefault("verifiedEmpty", {})[agent] = len(memory_items(remaining)) == 0
            client.close()
        except Exception as exc:
            result["errors"].append({"agent": agent, "type": type(exc).__name__})
            if client:
                client.close()
    return result


def lookup_marker(mcp: Path, db: Path, agent: str, marker: str) -> str:
    """Resolve an id from the server's structured result, never host text."""
    from run_probe import RpcClient, memory_items
    client = RpcClient(mcp, __import__("run_probe").child_env(db, agent), [], f"{agent}-lookup")
    try:
        client.start()
        result = client.tool("memory_search", {"query": marker, "limit": 100})
        for item in memory_items(result):
            if marker in json.dumps(item, ensure_ascii=False):
                value = item.get("id")
                if isinstance(value, str):
                    return value
        return ""
    finally:
        client.close()


def run(args: argparse.Namespace) -> int:
    output = Path(args.output).resolve()
    if output.exists():
        raise RuntimeError("output must be a new directory")
    output.mkdir(parents=True)
    root = output / "runtime"
    root.mkdir()
    db = root / "probe.sqlite"
    codex, opencode, mcp = executable("codex", args.codex), executable("opencode", args.opencode), Path(args.mcp).resolve()
    if not mcp.is_file():
        raise RuntimeError("memory MCP executable is unavailable")
    report: dict[str, object] = {"schemaVersion": "memory-host-session/v1", "status": "running", "createdAt": now(),
        "executables": {x: {"path": str(p), "sha256": sha(p), "version": command_version(p)} for x, p in (("codex", codex), ("opencode", opencode), ("memoryMcp", mcp))},
        "model": {"codex": "gpt-5.6-luna", "opencode": "opencode/big-pickle"}, "sessions": [], "mapping": {}, "cleanup": {}}
    # Never invoke a model unless the no-model isolation checks pass.
    preflight_args = argparse.Namespace(mcp=str(mcp), codex=str(codex), opencode=str(opencode), scratch=str(root / "preflight"), output=str(output / "preflight.json"))
    if preflight(preflight_args) != 0:
        report["status"] = "blocked"
        report["error"] = {"type": "PreflightBlocked", "message": "host isolation or free-model evidence failed"}
        (output / "run.json").write_text(json.dumps(redact(report, [str(root), str(Path.home()), str(codex.parent), str(opencode.parent), str(mcp)]), indent=2, sort_keys=True) + "\n", encoding="utf-8")
        return 2
    marker_a = "QUALA-" + uuid.uuid4().hex[:12].upper()
    marker_b = "QUALB-" + uuid.uuid4().hex[:12].upper()
    label_a = "topic-" + uuid.uuid4().hex[:12]
    ids: dict[str, str] = {}
    try:
        prompts = [("codex", "qual-a", f"Use memory_store now. Store this exact synthetic fact: Topic {label_a} has recall code {marker_a}. Set importance 0.9 and provenance user-stated. Then answer with the returned memory id only."),
                   ("codex", "qual-a", 'Call memory_recall with id exactly "{id}". Also call memory_search with query exactly "{label}" (no extra prefix or words). Report whether both find the stored fact and return the exact stored marker.'),
                   ("opencode", "qual-b", "First attempt memory_recall by A's id {a_id}, memory_search for A's query label {a_label}, and memory_list; report whether any A record is visible. Then use memory_store to store this exact synthetic fact: " + marker_b + ". Return the new memory id only."),
                   ("opencode", "qual-b", "Use memory_recall by your id {id} and memory_search for your stored marker. Return the exact stored marker only.")]
        if args.only_codex:
            prompts = prompts[:2]
            report['secondHost'] = 'not-run: prior OpenCode provider 403 preserved in host-run-02'
        for index, (host, agent, prompt) in enumerate(prompts, 1):
            source = ids.get("qual-a:marker") if agent == "qual-a" else ids.get("qual-b:marker")
            prompt = prompt.format(id=(source or "UNKNOWN").split(":", 1)[-1], label=label_a,
                a_id=ids.get("qual-a:marker", "").split(":", 1)[-1], a_label=label_a)
            prompt += " Use only the memory MCP tools. Do not read files, use shell, network, other tools, skills or delegates. This is a synthetic isolated test; perform all requested memory tool calls, then finish."
            env = base_env(root, db, agent)
            if host == "opencode":
                env["OPENCODE_CONFIG_CONTENT"] = json.dumps(opencode_config(mcp, db, agent), separators=(",", ":"))
                argv = [str(opencode), "--pure", "run", "--format", "json", "--model", "opencode/big-pickle", prompt]
            else:
                argv = [str(codex), "exec", "--ignore-user-config", *codex_overrides(mcp, db, agent),
                    "-c", "project_doc_max_bytes=0", "-c", 'model_reasoning_effort="medium"',
                    "--ephemeral", "--sandbox", "read-only", "--skip-git-repo-check", "--json", "-C", str(root), "-m", "gpt-5.6-luna", prompt]
            packet = run_cmd(argv, env, root, timeout=None)
            packet.update({"index": index, "host": host, "agent": agent, "prompt": prompt, "timestamp": now()})
            (output / f"session-{index:02d}.json").write_text(json.dumps(redact(packet, [str(root), str(Path.home()), str(codex.parent), str(opencode.parent), str(mcp)]), indent=2) + "\n", encoding="utf-8")
            report["sessions"].append({k: packet[k] for k in ("index", "host", "agent", "exitCode", "elapsedMs", "timedOut")})
            if packet["exitCode"] != 0:
                raise RuntimeError(f"{host} session {index} failed; no fallback model authorized")
            if index == 1:
                resolved = lookup_marker(mcp, db, "qual-a", marker_a)
                if resolved:
                    ids["qual-a:marker"] = "qual-a:" + resolved
                else:
                    raise RuntimeError("Codex did not store the requested synthetic fact")
            if index == 3:
                resolved = lookup_marker(mcp, db, "qual-b", marker_b)
                if resolved:
                    ids["qual-b:marker"] = "qual-b:" + resolved
                else:
                    raise RuntimeError("OpenCode did not store its positive-control fact")
            packet["structuredIdCandidate"] = ids.get("qual-a:marker" if index <= 2 else "qual-b:marker")
        report["mapping"] = ids
        report["status"] = "completed" if all(s["exitCode"] == 0 and not s["timedOut"] for s in report["sessions"]) else "failed"
    except Exception as exc:
        report["status"] = "failed"
        report["error"] = {"type": type(exc).__name__, "message": str(exc)[:500]}
    finally:
        report["cleanup"] = cleanup(mcp, db, ids, root)
        if report["cleanup"].get("errors") or not all(report["cleanup"].get("verifiedEmpty", {}).values()):
            report["status"] = "failed"
        (output / "run.json").write_text(json.dumps(redact(report, [str(root), str(Path.home()), str(codex.parent), str(opencode.parent), str(mcp)]), indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"status": report["status"], "output": str(output)}))
    return 0 if report["status"] == "completed" else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    for name in ("preflight", "run"):
        p = sub.add_parser(name)
        p.add_argument("--mcp", required=True)
        p.add_argument("--codex")
        p.add_argument("--opencode")
        p.add_argument("--scratch")
        p.add_argument("--output", required=True)
        p.add_argument("--only-codex", action="store_true", help="Run only same-host persistence; never qualifies second-host isolation")
    args = parser.parse_args()
    try:
        return preflight(args) if args.command == "preflight" else run(args)
    except Exception as exc:
        print(f"host probe blocked: {type(exc).__name__}: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
