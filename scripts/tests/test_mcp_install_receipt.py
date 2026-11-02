"""Offline receipt checks against fictional executable bytes (no installation)."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


BASH = shutil.which("bash")
if os.name == "nt":
    candidate = Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "Git/bin/bash.exe"
    BASH = str(candidate) if candidate.exists() else None
SCRIPT = Path(__file__).resolve().parents[1] / "mcp-install-receipt.sh"


@unittest.skipUnless(BASH, "Bash is required for the optional install recipe")
class ReceiptTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="mcp-receipt-")
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        self.fresh = self.root / "build with spaces" / "elegy-run.exe"
        self.fresh.parent.mkdir()
        self.fresh.write_bytes(b"fictional executable version one")
        self.install = self.root / "install with spaces"
        self.install.mkdir()
        self.installed = self.install / self.fresh.name
        shutil.copyfile(self.fresh, self.installed)

    def run_receipt(self, mode):
        return subprocess.run(
            [BASH, SCRIPT.as_posix(), mode, self.install.as_posix(), self.fresh.as_posix()],
            capture_output=True, timeout=20, check=False,
        )

    def test_matching_source_receipt_and_installed_bytes_pass(self):
        self.assertEqual(self.run_receipt("write").returncode, 0)
        self.assertEqual(self.run_receipt("check").returncode, 0)

    def test_changed_installed_bytes_are_rejected(self):
        self.assertEqual(self.run_receipt("write").returncode, 0)
        self.installed.write_bytes(b"unexpected installed executable")
        self.assertNotEqual(self.run_receipt("check").returncode, 0)

    def test_missing_installed_binary_is_rejected(self):
        self.assertEqual(self.run_receipt("write").returncode, 0)
        self.installed.unlink()
        self.assertNotEqual(self.run_receipt("check").returncode, 0)

    def test_write_cannot_attest_different_installed_bytes(self):
        self.installed.write_bytes(b"old executable")
        self.assertNotEqual(self.run_receipt("write").returncode, 0)
        self.assertFalse((self.install / "install-receipt.json").exists())

    def test_new_source_build_is_detected(self):
        self.assertEqual(self.run_receipt("write").returncode, 0)
        self.fresh.write_bytes(b"fictional executable version two")
        self.assertNotEqual(self.run_receipt("check").returncode, 0)


if __name__ == "__main__":
    unittest.main()
