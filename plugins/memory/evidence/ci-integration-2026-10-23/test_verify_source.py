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
        self.assertEqual(result["productSourceRevision"], "5eb34e0b32034a4bd254c6d775e5179dd2e4a1e1")
        self.assertEqual(result["cliSourceSha256"], "1742beff51d26ca32749e7b70ef7cce365f6ed4d3636f5c5e7116a4ecd0783ba")

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
