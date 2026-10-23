import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import verify_source


class ImmutableSourceTests(unittest.TestCase):
    def test_committed_source_restores_exact_compiled_bytes(self):
        result = verify_source.verify()
        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["productSourceRevision"], "2a351a33124cd87d479f8f44ee274a131c84dff9")
        self.assertEqual(result["cliSourceSha256"], "a0c84881c88fb048d80539b3773453b636ba4e6798c3aa9a44cb976d72b05bcd")

    def test_mutated_git_and_checkout_bindings_are_rejected(self):
        original = json.loads((verify_source.ROOT / "source-lock.json").read_text(encoding="utf-8"))
        mutations = [
            ("gitBlobSha1", "0" * 40),
            ("gitContentSha256", "0" * 64),
            ("sha256", "0" * 64),
            ("gitToCheckout", "arbitrary-transform"),
            ("path", "../outside"),
        ]
        with tempfile.TemporaryDirectory(dir=verify_source.ROOT) as temp:
            root = Path(temp)
            for field, value in mutations:
                with self.subTest(field=field):
                    lock = copy.deepcopy(original)
                    lock["sourceFiles"][0][field] = value
                    (root / "source-lock.json").write_text(json.dumps(lock), encoding="utf-8")
                    with patch.object(verify_source, "ROOT", root), self.assertRaises(ValueError):
                        verify_source.verify()
            lock = copy.deepcopy(original)
            lock["cliSourceSha256"] = "0" * 64
            (root / "source-lock.json").write_text(json.dumps(lock), encoding="utf-8")
            with patch.object(verify_source, "ROOT", root), self.assertRaises(ValueError):
                verify_source.verify()


if __name__ == "__main__":
    unittest.main()
