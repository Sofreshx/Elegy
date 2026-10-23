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
        self.assertEqual(result["productSourceRevision"], "95bf15ba2afa8e7655c45a8127b6a060893b695a")
        self.assertEqual(result["cliSourceSha256"], "1fe80e6ccdd04e20e64d1647758b1fd5209f81a1ae1afc779e1d5c2ffea598bc")

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
