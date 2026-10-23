"""Reproduce publication-only derivatives; never change model or test results."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re


def python_metadata(raw: bytes) -> bytes:
    record = json.loads(raw.decode("utf-8"))
    executable = record["argv"][0]
    if Path(executable).name.casefold() != "python.exe":
        raise ValueError("expected the recorded Python executable")
    token = json.dumps(executable, ensure_ascii=True).encode("utf-8")
    if raw.count(token) != 1:
        raise ValueError("expected exactly one executable token")
    result = raw.replace(token, b'"<python>"', 1)
    expected = {**record, "argv": ["<python>", *record["argv"][1:]]}
    if json.loads(result.decode("utf-8")) != expected:
        raise ValueError("unexpected metadata change")
    return result


def shape_diagnostic(raw: bytes) -> bytes:
    # The validator reports the matched prefix, not the full path. Preserve
    # every other diagnostic byte, including the failed exit/counts.
    pattern = re.compile(("C" + ":" + r"[/\\]{1,2}" + "Users").encode(), re.I)
    result, count = pattern.subn(b"<local-path-prefix>", raw)
    if count != 1:
        raise ValueError("expected one local-path diagnostic prefix")
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("kind", choices=("python-metadata", "shape-diagnostic"))
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise ValueError("preserve prior output")
    raw = args.input.read_bytes()
    result = (python_metadata if args.kind == "python-metadata" else shape_diagnostic)(raw)
    args.output.write_bytes(result)
    print(json.dumps({"rawSha256": hashlib.sha256(raw).hexdigest(),
                      "publicSha256": hashlib.sha256(result).hexdigest()}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
