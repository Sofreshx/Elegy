"""Probe the installed second software host without credentials or real memory.

Requires a prior `opencode --pure models opencode --verbose` catalog in
runtime/opencode-inspection/catalog.stdout. Only its explicitly zero-cost model
is eligible. This is a provider preflight, not a Memory integration verdict.
"""
import hashlib
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    exe = Path(shutil.which("opencode") or "")
    if not exe.is_file():
        raise RuntimeError("Installed OpenCode required; never install a substitute")
    catalog_path = ROOT / "runtime/opencode-inspection/catalog.stdout"
    catalog = catalog_path.read_text(encoding="utf-8")
    start = catalog.index("opencode/big-pickle\n") + len("opencode/big-pickle\n")
    model, _ = json.JSONDecoder().raw_decode(catalog[start:])
    assert model["id"] == "big-pickle" and model["providerID"] == "opencode"
    assert model["status"] == "active"
    assert model["cost"]["input"] == model["cost"]["output"] == 0
    assert all(v == 0 for v in model["cost"].get("cache", {}).values())
    output = args.output.resolve()
    output.mkdir(exist_ok=False)
    runtime = (ROOT / "runtime" / output.name).resolve()
    runtime.mkdir()
    keep = {"PATH", "PATHEXT", "SYSTEMROOT", "WINDIR", "COMSPEC", "USERPROFILE",
            "APPDATA", "LOCALAPPDATA", "PROGRAMFILES", "PROGRAMFILES(X86)",
            "PROGRAMDATA", "TEMP", "TMP", "HOMEDRIVE", "HOMEPATH"}
    env = {k: v for k, v in os.environ.items() if k.upper() in keep}
    (runtime / "temp").mkdir()
    env["TEMP"] = env["TMP"] = str(runtime / "temp")
    for name in ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME"]:
        path = runtime / name.lower()
        path.mkdir()
        env[name] = str(path)
    for name in ["OPENCODE_DISABLE_AUTOUPDATE", "OPENCODE_DISABLE_PRUNE",
                 "OPENCODE_DISABLE_DEFAULT_PLUGINS", "OPENCODE_DISABLE_LSP_DOWNLOAD",
                 "OPENCODE_DISABLE_CLAUDE_CODE"]:
        env[name] = "true"
    env["OPENCODE_CONFIG_DIR"] = str(runtime / "config")
    Path(env["OPENCODE_CONFIG_DIR"]).mkdir()
    config = {"autoupdate": False, "share": "disabled", "snapshot": False,
              "enabled_providers": ["opencode"], "model": "opencode/big-pickle",
              "small_model": "opencode/big-pickle", "plugin": [], "mcp": {},
              "permission": "deny", "lsp": False, "formatter": False}
    env["OPENCODE_CONFIG_CONTENT"] = json.dumps(config)
    report = {"schemaVersion": "memory-second-host-preflight/v1",
              "startedAt": datetime.now(timezone.utc).isoformat(),
              "host": "OpenCode", "binarySha256": hashlib.sha256(exe.read_bytes()).hexdigest(),
              "catalogSha256": hashlib.sha256(catalog_path.read_bytes()).hexdigest(),
              "runnerSha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "modelMetadata": model, "configuration": config, "commands": [],
              "memoryIntegrationExercised": False, "paidApiCalls": 0,
              "status": "inconclusive"}
    def save():
        (output / "run.json").write_text(json.dumps(report, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
    def redact(text, cwd):
        for value, alias in [(str(exe), "<installed-opencode>"), (str(runtime), "<isolated-host-state>"),
                             (str(cwd), "<isolated-cwd>"), (str(Path.home()), "<user-home>")]:
            text = text.replace(value, alias).replace(value.replace("\\", "/"), alias)
        return text
    save()
    with tempfile.TemporaryDirectory(prefix="elegy-second-host-") as cwd:
        for name, args in [("version", ["--version"]), ("paths", ["--pure", "debug", "paths"]),
                           ("config", ["--pure", "debug", "config"]),
                           ("preflight", ["--pure", "run", "--dir", cwd, "--format", "json",
                                          "--model", "opencode/big-pickle",
                                          "Do not use tools. Reply with exactly READY."])]:
            result = subprocess.run([str(exe), *args], cwd=cwd, env=env, capture_output=True,
                                    encoding="utf-8", timeout=90)
            stdout, stderr = redact(result.stdout, cwd), redact(result.stderr, cwd)
            (output / (name+".stdout")).write_text(stdout, encoding="utf-8")
            (output / (name+".stderr")).write_text(stderr, encoding="utf-8")
            report["commands"].append({"name": name, "argv": ["<installed-opencode>", *[redact(a, cwd) for a in args]],
                                       "exitCode": result.returncode,
                                       "stdoutSha256": hashlib.sha256(stdout.encode()).hexdigest(),
                                       "stderrSha256": hashlib.sha256(stderr.encode()).hexdigest()})
            save()
            if name == "paths":
                paths = dict(line.split(None, 1) for line in result.stdout.splitlines() if line.strip())
                for key in ["data", "config", "cache", "state", "tmp", "bin", "log", "repos"]:
                    path = Path(paths[key]).resolve()
                    if not path.is_relative_to(runtime):
                        raise RuntimeError("Host state path escapes scratch; stop before model call")
            if name == "config":
                observed = json.loads(result.stdout)
                if observed.get("mcp") or observed.get("plugin") or observed.get("permission") not in ["deny", {"*": "deny"}]:
                    raise RuntimeError("Unexpected host capabilities; stop before model call")
            if name == "preflight":
                # Some host versions report provider failures as JSON with exit 0.
                events = [json.loads(line) for line in result.stdout.splitlines() if line.strip().startswith("{")]
                errors = [e for e in events if e.get("type") == "error"]
                texts = [e.get("part", {}).get("text", "") for e in events if e.get("type") == "text"]
                report["providerErrors"] = errors
                report["answer"] = "\n".join(texts).strip()
                report["status"] = "provider-ready" if result.returncode == 0 and not errors and report["answer"] == "READY" else "blocked"
                report["memoryIntegrationExercised"] = False
                save()
    print(json.dumps({"status": report["status"], "memoryIntegrationExercised": False}))


if __name__ == "__main__":
    main()
