import copy
import hashlib
import json
import unittest
from unittest.mock import patch

import assess


class AssessTests(unittest.TestCase):
    def setUp(self):
        self.protocol = {"instruction": "Use facts.", "model": "m", "reasoningEffort": "x"}
        self.case = {"id": "q1", "question": "Question?", "expectedGroups": [["Apollo"]], "forbidden": []}
        self.context = "History\n{}"
        self.config = {"protocol": self.protocol}
        self.config_sha = "cfg"
        prompt = self.protocol["instruction"] + "\n\nDonnées historiques :\n" + self.context + "\n\nQuestion actuelle :\n" + self.case["question"]
        stdout = "\n".join([
            json.dumps({"type": "thread.started", "thread_id": "t1"}),
            json.dumps({"type": "item.completed", "item": {"type": "agent_message", "text": "Apollo"}}),
            json.dumps({"type": "turn.completed", "usage": {"output_tokens": 1}}),
        ])
        self.raw = {"stdout": stdout, "answer": "Apollo", "threadId": "t1", "usage": {"output_tokens": 1}, "exitCode": 0,
                    "valid": True, "caseId": "q1", "arm": "with", "command": assess.command("<codex>", "<scratch-case>", self.protocol),
                    "configurationSha256": self.config_sha, "promptSha256": hashlib.sha256(prompt.encode()).hexdigest()}
        self.record = {"caseId": "q1", "arm": "with", "trial": 1, "answer": "Apollo", "threadId": "t1", "usage": {"output_tokens": 1},
                       "configurationSha256": self.config_sha,
                       "promptSha256": self.raw["promptSha256"], "contextSha256": assess.sha(self.context.encode()), "score": 1.0,
                       "missingGroups": [], "forbiddenMentions": [], "formatErrors": [], "executionErrors": []}

    def test_valid_raw_passes_and_each_mutation_fails(self):
        errors = []; assess.check_raw(self.record, self.raw, self.case, self.context, self.config, self.config_sha, errors); self.assertFalse(errors)
        for field, value in (("answer", "changed"), ("configurationSha256", "bad"), ("promptSha256", "0" * 64), ("caseId", "q2"), ("arm", "without"), ("command", ["bad"])):
            raw = copy.deepcopy(self.raw); raw[field] = value; errors = []
            assess.check_raw(self.record, raw, self.case, self.context, self.config, self.config_sha, errors)
            self.assertTrue(errors, field)

    def test_tool_item_and_prose_flags_are_blocking(self):
        raw = copy.deepcopy(self.raw); raw["stdout"] = raw["stdout"].replace('"agent_message"', '"tool_call"')
        errors = []; assess.check_raw(self.record, raw, self.case, self.context, self.config, self.config_sha, errors); self.assertTrue(errors)
        errors = []; assess.review_index({"records": [{"phase": "x", "caseId": "q1"}]}, errors); self.assertTrue(errors)

    def test_inventory_sizes_are_fixed(self):
        corpus = {"episodes": [{"id": "e", "cases": [dict(self.case, id="q01"), dict(self.case, id="q02")]}]}
        self.assertEqual(len(assess.expected_records("answers-primary", corpus)), 4)
        self.assertEqual(len(assess.expected_records("repeats", {"episodes": [{"id": "e", "cases": [dict(self.case, id=x) for x in assess.CASES]}]})), 10)

    def test_false_review_cannot_be_accepted(self):
        errors = []
        review = {"records": [{"phase": "answers-primary", "caseId": "q1", "arm": "with", "trial": 1,
                                "grounded": False, "taskComplete": True, "controlPass": True, "formatPass": True,
                                "reason": "failed"}]}
        indexed = assess.review_index(review, errors)
        failed = []
        assess.apply_review(indexed, ("answers-primary", "q1", "with", 1), {"kind": "facts"}, failed)
        self.assertTrue(failed)

    def test_without_abstention_is_valid_only_for_allowed_kinds(self):
        review = {"records": [{"phase": "answers-primary", "caseId": "q1", "arm": "without", "trial": 1,
                                "grounded": True, "taskComplete": False, "controlPass": True, "formatPass": True,
                                "reason": "safe abstention"}]}
        errors = []; indexed = assess.review_index(review, errors); failed = []
        assess.apply_review(indexed, ("answers-primary", "q1", "without", 1), {"kind": "facts"}, failed)
        self.assertFalse(failed)
        assess.apply_review(indexed, ("answers-primary", "q1", "without", 1), {"kind": "unknown"}, failed)
        self.assertTrue(failed)

    def test_current_override_without_history_may_omit_unknown_transport(self):
        key = ("answers-primary", "q03", "without", 1)
        review = {key: {"grounded": True, "taskComplete": False, "controlPass": True,
                        "formatPass": True, "reason": "Current port applied; historical transport safely unknown."}}
        failed = []; assess.apply_review(review, key, {"kind": "current-override"}, failed)
        self.assertFalse(failed)

    def test_bindings_require_exact_inputs_and_unchanged_client(self):
        subject = assess.load(assess.SUBJECT)
        protocol = assess.load(assess.PROTOCOL)
        manifest = assess.expected_manifest_bindings("answers-primary", protocol, subject)
        errors = []
        assess.check_manifest_bindings("answers-primary", manifest, protocol, subject, errors)
        self.assertFalse(errors)
        for field in ("configurationSha256", "clientSha256After", "codexVersion", "replayRunSha256", "protocolSha256", "gradeSha256"):
            with self.subTest(field=field):
                changed = copy.deepcopy(manifest); changed[field] = "wrong"
                errors = []
                assess.check_manifest_bindings("answers-primary", changed, protocol, subject, errors)
                self.assertTrue(errors)
        changed = copy.deepcopy(manifest); changed["configuration"]["argv"][-1] = "changed"
        errors = []; assess.check_manifest_bindings("answers-primary", changed, protocol, subject, errors)
        self.assertTrue(errors)

    def test_review_must_bind_answer_and_raw_bytes(self):
        key = ("repeats", "q08", "with", 2)
        row = dict(self.record, rawSha256="a" * 64)
        review = {"rawSha256": "a" * 64, "answerSha256": assess.sha(b"Apollo")}
        errors = []; assess.check_review_binding(review, row, key, errors); self.assertFalse(errors)
        for field in ("rawSha256", "answerSha256"):
            changed = dict(review); changed[field] = "b" * 64
            errors = []; assess.check_review_binding(changed, row, key, errors); self.assertTrue(errors)

    def test_pair_metrics_use_both_arms(self):
        rows = [{"phase": "answers-primary", "caseId": "q01", "arm": arm, "trial": 1,
                 "score": score, "elapsedMs": 10, "usage": {}, "semantic": {"grounded": True,
                 "taskComplete": arm == "with", "controlPass": True, "formatPass": True}}
                for arm, score in (("with", 1), ("without", 0))]
        result = assess.paired_metrics(rows)
        self.assertEqual(result["pairCount"], 1)
        self.assertEqual(result["meanWithRecall"], 1)
        self.assertEqual(result["meanWithoutRecall"], 0)
        self.assertEqual(result["meanPairedDelta"], 1)

    def test_every_semantic_flag_blocks_with_recall_and_repeats(self):
        key = ("repeats", "q08", "with", 3)
        good = {flag: True for flag in ("grounded", "taskComplete", "controlPass", "formatPass")}
        good["reason"] = "Complete and supported."
        for flag in ("grounded", "taskComplete", "controlPass", "formatPass"):
            changed = dict(good); changed[flag] = False; failed = []
            assess.apply_review({key: changed}, key, {"kind": "correction"}, failed)
            self.assertTrue(failed, flag)

    def test_observed_wrapper_rejects_changed_subject_and_binary(self):
        original = assess.load(assess.ROOT / "replay-primary-wrapper.json")
        errors = []; assess.check_wrapper("replay-primary", original, errors); self.assertFalse(errors)
        for when in ("before", "after"):
            changed = copy.deepcopy(original); changed[when]["subjectSha256"] = "bad"
            errors = []; assess.check_wrapper("replay-primary", changed, errors); self.assertTrue(errors)
            changed = copy.deepcopy(original); changed[when]["binaries"]["cli"]["sha256"] = "bad"
            errors = []; assess.check_wrapper("replay-primary", changed, errors); self.assertTrue(errors)

    def test_observed_replay_requires_cleanup_controls_and_immutable_source(self):
        corpus = assess.load(assess.ROOT / "corpus.json"); protocol = assess.load(assess.PROTOCOL)
        load_original = assess.load
        errors = []; assess.check_replay("replay-primary", corpus, protocol, errors); self.assertFalse(errors)
        for mutation in ("source", "cleanup", "control"):
            def changed_load(path):
                data = load_original(path)
                if path == assess.ROOT / "replay-primary" / "cedar" / "run.json":
                    if mutation == "source": data["stages"][0]["packet"]["sourceAfter"]["durableData"]["mainSha256"] = "wrong"
                    elif mutation == "cleanup": data["cleanup"][0]["verifiedEmpty"] = False
                    else: data["stages"][0]["checks"] = []
                return data
            errors = []
            with patch.object(assess, "load", side_effect=changed_load): assess.check_replay("replay-primary", corpus, protocol, errors)
            self.assertTrue(errors, mutation)


if __name__ == "__main__": unittest.main()
