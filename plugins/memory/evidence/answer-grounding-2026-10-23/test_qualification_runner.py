import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))

import qualification_runner as runner


def case(case_id):
    return {"id": case_id, "agent": "qual-a", "question": "Question?", "expectedGroups": [["answer"]], "forbidden": [], "actions": [{"agent": "qual-a", "op": "store", "key": "x", "text": "fact"}]}


class QualificationRunnerTests(unittest.TestCase):
    def test_validate_corpus_scales_to_twelve_and_twenty_four_cases(self):
        for count in (12, 24):
            corpus = {"episodes": [{"id": "episode", "cases": [case(f"q{i}") for i in range(1, count + 1)]}]}
            self.assertEqual(len(runner.validate_corpus(corpus)), count)

    def test_validate_corpus_accepts_both_frozen_real_corpora_and_empty_actions(self):
        for filename, count in (("corpus.json", 24), ("transfer-corpus.json", 12)):
            corpus = json.loads((HERE / filename).read_text(encoding="utf-8"))
            self.assertEqual(len(runner.validate_corpus(corpus)), count)
        empty_actions = case("q99")
        empty_actions["actions"] = []
        self.assertEqual(len(runner.validate_corpus({"episodes": [{"id": "episode", "cases": [empty_actions]}]})), 1)

    def test_parse_trials_supports_explicit_unique_two_and_three(self):
        self.assertEqual(runner.parse_trials("2,3"), [2, 3])
        with self.assertRaises(ValueError):
            runner.parse_trials("2,2")
        with self.assertRaises(ValueError):
            runner.parse_trials("0,3")

    def test_installed_runner_root_resolves_actual_frozen_inputs_and_helpers(self):
        hashes = runner.campaign_hashes(runner.ROOT, HERE / "transfer-corpus.json", HERE / "protocol.json")
        self.assertEqual(hashes["runnerSha256"], runner.sha(HERE / "qualification_runner.py"))
        self.assertEqual(hashes["corpusSha256"], runner.sha(HERE / "transfer-corpus.json"))
        self.assertIn("rpcSupportSha256", hashes)

    def test_validate_corpus_rejects_duplicate_or_invalid_ids_and_empty_episode(self):
        with self.assertRaises(ValueError):
            runner.validate_corpus({"episodes": [{"id": "x", "cases": [case("q1"), case("q1")]}]})
        with self.assertRaises(ValueError):
            runner.validate_corpus({"episodes": [{"id": "x", "cases": [case("x1")]}]})
        with self.assertRaises(ValueError):
            runner.validate_corpus({"episodes": [{"id": "x", "cases": []}]})

    def test_replay_contexts_rejects_protocol_or_corpus_hash_mismatch(self):
        with tempfile.TemporaryDirectory(dir=HERE) as directory:
            root = Path(directory)
            (root / "run.json").write_text(json.dumps({"status": "completed", "protocolSha256": "wrong", "corpusSha256": "wrong"}), encoding="utf-8")
            protocol = root / "protocol.json"
            corpus_path = root / "corpus.json"
            protocol.write_text("protocol", encoding="utf-8")
            corpus_path.write_text("corpus", encoding="utf-8")
            corpus = {"episodes": [{"id": "episode", "cases": [case("q1")]}]}
            with self.assertRaises(ValueError):
                runner.replay_contexts(root, corpus, protocol, corpus_path)

    def test_validate_records_rejects_missing_and_duplicate_threads(self):
        rows = [
            {"caseId": "q1", "arm": "with", "trial": 1, "threadId": "same", "executionErrors": [], "formatErrors": []},
            {"caseId": "q1", "arm": "without", "trial": 1, "threadId": "same", "executionErrors": [], "formatErrors": []},
        ]
        errors = runner.validate_records(rows, {("q1", "with", 1), ("q1", "without", 1)})
        self.assertIn("thread IDs are missing or not globally unique", errors)
        self.assertIn("result inventory mismatch", runner.validate_records(rows[:1], {("q1", "with", 1), ("q1", "without", 1)}))

    def test_record_answer_prompt_hash_is_byte_stable_and_linked(self):
        protocol = {"instruction": "Use exact facts.", "model": "m", "reasoningEffort": "medium"}
        selected = case("q1")
        context = "Historique\n{\"records\":[]}"
        config_sha = "config-sha"
        with tempfile.TemporaryDirectory(dir=HERE) as directory:
            destination = Path(directory)
            def fake_invoke(_exe, _protocol, out, case_id, arm, _question, _context, _config):
                raw = {"answer": "answer", "valid": True, "exitCode": 0,
                       "stdout": "{}\n", "promptSha256": hashlib.sha256((protocol["instruction"] + "\n\nDonnées historiques :\n" + context + "\n\nQuestion actuelle :\n" + selected["question"]).encode()).hexdigest(),
                       "configurationSha256": config_sha, "elapsedMs": 1.0, "caseId": case_id, "arm": arm,
                       "command": runner.command("<codex>", "<scratch-case>", protocol)}
                (out / "raw").mkdir()
                (out / "raw" / f"{case_id}_{arm}.json").write_text(json.dumps(raw), encoding="utf-8")
            with patch.object(runner, "invoke", fake_invoke), patch.object(runner, "parse_stdout_details", return_value=("answer", "thread-1", {"output_tokens": 1}, [])):
                result = runner.record_answer(Path("codex"), protocol, selected, "with", context, config_sha, destination, destination)
            self.assertEqual(result["promptSha256"], hashlib.sha256((protocol["instruction"] + "\n\nDonnées historiques :\n" + context + "\n\nQuestion actuelle :\n" + selected["question"]).encode()).hexdigest())
            self.assertEqual(result["configurationSha256"], config_sha)
            self.assertEqual(result["threadId"], "thread-1")
            self.assertEqual(result["rawPath"], "raw/q1_with.json")

    def test_replay_episode_report_hash_is_required_and_binds_source(self):
        with tempfile.TemporaryDirectory(dir=HERE) as directory:
            root = Path(directory)
            protocol = root / "protocol.json"; corpus_path = root / "corpus.json"
            protocol.write_text("protocol", encoding="utf-8"); corpus_path.write_text("corpus", encoding="utf-8")
            episode = {"id": "episode", "cases": [case("q1")]}
            episode_dir = root / "episode"; episode_dir.mkdir()
            episode_report = {"status": "completed", "stages": [{"caseId": "q1", "packet": {"response": {"data": {"additionalContext": "ctx"}}}}]}
            episode_path = episode_dir / "run.json"; episode_path.write_text(json.dumps(episode_report), encoding="utf-8")
            manifest = {"status": "completed", "protocolSha256": runner.sha(protocol), "corpusSha256": runner.sha(corpus_path),
                        "episodes": [{"id": "episode", "status": "completed", "stageCount": 1, "reportSha256": runner.sha(episode_path)}]}
            (root / "run.json").write_text(json.dumps(manifest), encoding="utf-8")
            corpus = {"episodes": [episode]}
            self.assertEqual(runner.replay_contexts(root, corpus, protocol, corpus_path)[0]["q1"], "ctx")
            episode_path.write_text(json.dumps({**episode_report, "stages": []}), encoding="utf-8")
            with self.assertRaises(ValueError): runner.replay_contexts(root, corpus, protocol, corpus_path)

    def test_main_rejects_protocol_limits_before_action_dispatch(self):
        with tempfile.TemporaryDirectory(dir=HERE) as directory:
            root = Path(directory)
            corpus_path = root / "corpus.json"; protocol_path = root / "protocol.json"
            corpus_path.write_text(json.dumps({"episodes": [{"id": "episode", "cases": [case("q1")]}]}), encoding="utf-8")
            protocol_path.write_text(json.dumps({"memory": {"maxContextTokens": 601, "scopes": ["Agent"]}}), encoding="utf-8")
            with patch.object(sys, "argv", ["qualification_runner.py", "replay", "--corpus", str(corpus_path), "--protocol", str(protocol_path), "--output", str(root / "out"), "--cli", str(root / "cli"), "--mcp", str(root / "mcp")]), patch.object(runner, "run_replay", side_effect=AssertionError("dispatch occurred")):
                with self.assertRaises(AssertionError): runner.main()

    def test_record_answer_rejects_mutated_raw_identity_fields(self):
        protocol = {"instruction": "Use exact facts.", "model": "m", "reasoningEffort": "medium"}
        selected = case("q1"); context = "Historique\n{}"
        with tempfile.TemporaryDirectory(dir=HERE) as directory:
            destination = Path(directory)
            def fake_invoke(_exe, _protocol, out, case_id, arm, _question, _context, _config):
                prompt = protocol["instruction"] + "\n\nDonnées historiques :\n" + context + "\n\nQuestion actuelle :\n" + selected["question"]
                raw = {"answer": "answer", "valid": True, "exitCode": 0, "stdout": "{}\n",
                       "promptSha256": hashlib.sha256(prompt.encode()).hexdigest(), "configurationSha256": "cfg",
                       "caseId": "wrong", "arm": "without", "command": ["wrong"]}
                (out / "raw").mkdir(); (out / "raw" / f"{case_id}_{arm}.json").write_text(json.dumps(raw), encoding="utf-8")
            with patch.object(runner, "invoke", fake_invoke), patch.object(runner, "parse_stdout_details", return_value=("answer", "thread-1", {"output_tokens": 1}, [])):
                result = runner.record_answer(Path("codex"), protocol, selected, "with", context, "cfg", destination, destination)
            kinds = {item["type"] for item in result["executionErrors"]}
            self.assertTrue({"case-id-mismatch", "arm-mismatch", "command-mismatch"}.issubset(kinds))


if __name__ == "__main__":
    unittest.main()
