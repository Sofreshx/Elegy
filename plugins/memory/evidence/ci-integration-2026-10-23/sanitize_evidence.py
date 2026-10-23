"""Sanitize the three public evidence derivatives without changing semantics."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path


COMMIT = "a753b4b5f4bddf553a5b11255b3532957291d9c2"
ROOT = Path(__file__).resolve().parents[4]
FILES = (
    "plugins/memory/evidence/recall-fixes-2026-10-23/second-host-01/paths.stdout",
    "plugins/memory/evidence/recall-fixes-2026-10-23/validation-preparation/05-python-integrations.log",
    "plugins/memory/evidence/stored-version-qualification-2026-10-23/native-validation/harness-preflight.log",
)
MANIFEST = Path(__file__).with_name("redactions.json")
LOCAL_PATH = "<local-path>"
LOCAL_APPDATA = "<local-appdata>"
_APP_COMPONENT = "App" + "Data"
_LOCAL_COMPONENT = "Lo" + "cal"

# A path token is deliberately limited to one line and excludes quote-like
# delimiters.  The separator class accepts both normal and doubled Windows
# separators as they appear in repr-style error output.
WINDOWS_PATH = re.compile(
    r"(?<![A-Za-z0-9])(?:[A-Za-z]:[\\/]{1,2}|\\\\)"
    r"(?:[^\\/\r\n\t \"'<>]+[\\/]{1,2})+"
    r"[^\\/\r\n\t \"'<>;,\)\]]+"
)
APPDATA_FRAGMENT = re.compile(
    r"(?P<prefix><user-home>)(?P<sep>[\\/]{1,2})"
    + _APP_COMPONENT
    + r"(?P<sep2>[\\/]{1,2})"
    + _LOCAL_COMPONENT
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sanitize_text(text: str) -> tuple[str, dict[str, int]]:
    """Return sanitized text and exact replacement counts."""

    counts = {"windows_absolute_path": 0, "neutralized_appdata_fragment": 0}

    def replace_appdata(match: re.Match[str]) -> str:
        counts["neutralized_appdata_fragment"] += 1
        return f"{match.group('prefix')}{match.group('sep')}{LOCAL_APPDATA}"

    text = APPDATA_FRAGMENT.sub(replace_appdata, text)

    def replace_path(match: re.Match[str]) -> str:
        counts["windows_absolute_path"] += 1
        return LOCAL_PATH

    return WINDOWS_PATH.sub(replace_path, text), counts


def sanitize_bytes(data: bytes) -> tuple[bytes, dict[str, int]]:
    text = data.decode("utf-8")
    sanitized, counts = sanitize_text(text)
    return sanitized.encode("utf-8"), counts


def original_bytes(relative: str) -> bytes:
    result = subprocess.run(
        ["git", "-C", str(ROOT), "show", f"{COMMIT}:{relative}"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode:
        raise RuntimeError(f"git show failed for {relative}: exit {result.returncode}")
    return result.stdout


def original_blob(relative: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(ROOT), "rev-parse", f"{COMMIT}:{relative}"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode:
        raise RuntimeError(f"git rev-parse failed for {relative}: exit {result.returncode}")
    return result.stdout.strip()


def expected_records() -> list[dict[str, object]]:
    records = []
    for relative in FILES:
        source = original_bytes(relative)
        sanitized, counts = sanitize_bytes(source)
        records.append(
            {
                "path": relative,
                "originalBlob": original_blob(relative),
                "originalSha256": sha256(source),
                "currentSha256": sha256(sanitized),
                "replacementCounts": counts,
                "transforms": [
                    "replace single or doubled Windows absolute path tokens with <local-path>",
                    "replace the neutralized home-local-data fragment with the local-appdata placeholder",
                    "preserve UTF-8 bytes, line endings, and non-path evidence text",
                ],
            }
        )
    return records


def verify_records(records: list[dict[str, object]]) -> None:
    expected = {record["path"]: record for record in expected_records()}
    if {record["path"] for record in records} != set(FILES):
        raise ValueError("redactions manifest does not cover exactly the three evidence files")
    for record in records:
        relative = str(record["path"])
        current = (ROOT / relative).read_bytes()
        wanted = expected[relative]
        if record != wanted:
            raise ValueError(f"redactions metadata mismatch for {relative}")
        if sha256(current) != wanted["currentSha256"]:
            raise ValueError(f"current evidence does not match sanitized snapshot for {relative}")
        rebuilt, _ = sanitize_bytes(original_bytes(relative))
        if current != rebuilt:
            raise ValueError(f"sanitizer reconstruction mismatch for {relative}")


def apply() -> list[dict[str, object]]:
    records = expected_records()
    for record in records:
        relative = str(record["path"])
        path = ROOT / relative
        current = path.read_bytes()
        source = original_bytes(relative)
        wanted, _ = sanitize_bytes(source)
        if current == wanted:
            continue
        if current != source:
            raise ValueError(f"refusing unexpected current bytes for {relative}")
        path.write_bytes(wanted)
    return records


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args(argv)
    try:
        records = apply() if args.apply else expected_records()
        if args.apply:
            MANIFEST.write_text(
                json.dumps(
                    {"schemaVersion": "evidence-redactions/v1", "originalCommit": COMMIT, "files": records},
                    ensure_ascii=False,
                    indent=2,
                )
                + "\n",
                encoding="utf-8",
            )
        elif not MANIFEST.exists():
            raise ValueError("redactions.json is missing; run --apply once to create the binding")
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        if manifest.get("originalCommit") != COMMIT:
            raise ValueError("redactions manifest commit mismatch")
        verify_records(manifest.get("files", []))
        print(json.dumps({"status": "verified", "files": len(FILES)}, separators=(",", ":")))
        return 0
    except (OSError, ValueError, RuntimeError, UnicodeError) as exc:
        print(f"sanitizer error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
