"""Verify compiled checkout bytes from immutable Git objects, including EOL conversion."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify(with_binaries: bool = False) -> dict:
    lock = json.loads((ROOT / "source-lock.json").read_text(encoding="utf-8"))
    groups = [lock["sourceFiles"], lock["mcpSourceFiles"]]
    if [len(group) for group in groups] != [149, 18]: raise ValueError("source inventory changed")
    entries = {entry["path"]: entry for group in groups for entry in group}
    for path in entries:
        if Path(path).is_absolute() or ".." in Path(path).parts or "\n" in path: raise ValueError("invalid source path")
    paths = sorted(entries)
    query = "\n".join(f"{lock['productSourceRevision']}:{path}" for path in paths) + "\n"
    batch = subprocess.check_output(["git", "cat-file", "--batch"], input=query.encode(), cwd=REPO)
    offset = 0; restored = {}
    for path in paths:
        end = batch.index(b"\n", offset); header = batch[offset:end].split()
        if len(header) != 3 or header[1] != b"blob": raise ValueError("missing Git source object")
        size = int(header[2]); data = batch[end + 1:end + 1 + size]; offset = end + size + 2
        entry = entries[path]
        if header[0].decode() != entry["gitBlobSha1"] or sha(data) != entry["gitContentSha256"]:
            raise ValueError("Git source binding differs")
        relation = entry["gitToCheckout"]
        if relation == "LF-to-CRLF": data = data.replace(b"\r\n", b"\n").replace(b"\n", b"\r\n")
        elif relation == "CRLF-to-LF": data = data.replace(b"\r\n", b"\n")
        elif relation != "exact": raise ValueError("unsupported source transformation")
        if sha(data) != entry["sha256"]: raise ValueError("compiled checkout bytes differ")
        restored[path] = data
    if offset != len(batch): raise ValueError("extra Git batch output")
    digest = hashlib.sha256()
    for entry in sorted(lock["sourceFiles"], key=lambda item: item["path"]):
        name = entry["path"].encode(); content = restored[entry["path"]]
        digest.update(struct.pack("<Q", len(name))); digest.update(name)
        digest.update(struct.pack("<Q", len(content))); digest.update(content)
    if digest.hexdigest() != lock["cliSourceSha256"]: raise ValueError("source fingerprint differs")
    binary_status = "not-requested"
    if with_binaries:
        subject = json.loads((ROOT / "subject.json").read_text(encoding="utf-8"))
        for key in ("cli", "mcp"):
            path = (ROOT / subject["binaryPaths"][key]).resolve()
            if REPO.resolve() not in path.parents or not path.is_file() or sha(path.read_bytes()) != subject[f"{key}Sha256"]:
                raise ValueError("qualified binary bytes differ")
        binary_status = "passed"
    return {"status": "passed", "productSourceRevision": lock["productSourceRevision"],
            "cliSourceSha256": digest.hexdigest(), "sourceFiles": 149, "mcpSourceFiles": 18, "binaries": binary_status}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(); parser.add_argument("--with-binaries", action="store_true")
    print(json.dumps(verify(parser.parse_args().with_binaries), sort_keys=True))
