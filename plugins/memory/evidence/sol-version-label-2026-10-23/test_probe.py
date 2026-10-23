import copy
import unittest
from unittest.mock import patch

import probe


class ProbeInputsTests(unittest.TestCase):
    def test_frozen_inputs_preserve_every_json_body_and_suffix(self):
        actual = probe.expected_inputs()
        self.assertEqual(actual, probe.load(probe.ROOT / "inputs.json"))
        self.assertEqual(len(actual["jobs"]), 21)
        for job in actual["jobs"]:
            before, after = job["contexts"]["current"], job["contexts"]["candidate"]
            self.assertNotEqual(before, after)
            self.assertEqual(before.encode()[22:], after.encode()[22:])
            self.assertEqual(len(before.encode()), len(after.encode()))
            self.assertEqual(probe.digest(before.split("\n", 1)[1]), job["bodySha256"])

    def test_rejects_changed_configuration_and_label_length(self):
        original = probe.load
        for field, value in (("reasoningEffort", "low"), ("labelAfter", "changed-label"), ("baseProtocolSha256", "bad")):
            def changed(path):
                data = original(path)
                if path == probe.ROOT / "protocol.json": data[field] = value
                return data
            with patch.object(probe, "load", side_effect=changed):
                with self.assertRaises(ValueError): probe.expected_inputs()

    def test_sealed_replay_evidence_is_required(self):
        original = probe.sha
        with patch.object(probe, "sha", side_effect=lambda p: "bad" if p == probe.Q3 / "evidence-manifest.json" else original(p)):
            with self.assertRaises(ValueError): probe.expected_inputs()

    def test_postflight_failure_is_preserved_as_inconclusive(self):
        result = {"errors": []}
        with patch.object(probe, "preflight", side_effect=ValueError("source changed")):
            probe.finish_checks(result, "client", "code")
        self.assertEqual(result["status"], "inconclusive")
        self.assertTrue(result["errors"])


if __name__ == "__main__": unittest.main()
