import copy
import unittest
from unittest.mock import patch

import probe


class ProbeInputsTests(unittest.TestCase):
    def test_frozen_inputs_preserve_every_json_body_and_suffix(self):
        actual = probe.expected_inputs()
        self.assertEqual(actual, probe.load(probe.ROOT / "inputs.json"))
        self.assertEqual(len(actual["jobs"]), 6)
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


    def test_fixture_changes_only_ambiguous_question_and_adds_unknown_control(self):
        old = probe.load(probe.Q3 / "corpus.json")
        revised = probe.load(probe.ROOT / "corpus.json")
        old_cases = {c["id"]: c for e in old["episodes"] for c in e["cases"]}
        new_cases = {c["id"]: c for e in revised["episodes"] for c in e["cases"]}
        self.assertEqual(set(new_cases) - set(old_cases), {"q25"})
        self.assertEqual(len(new_cases), 25)
        for case_id, before in old_cases.items():
            after = dict(new_cases[case_id])
            if case_id == "q20":
                self.assertNotEqual(after["question"], before["question"])
                after["question"] = before["question"]
            self.assertEqual(after, before)
        self.assertEqual(new_cases["q25"]["actions"], [])
        self.assertEqual(new_cases["q25"]["kind"], "unknown")
        self.assertEqual(new_cases["q25"]["expectedGroups"], new_cases["q21"]["expectedGroups"])


if __name__ == "__main__": unittest.main()
