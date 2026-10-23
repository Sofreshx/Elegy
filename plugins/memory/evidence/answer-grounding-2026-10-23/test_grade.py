import json
import unittest
from pathlib import Path

import grade

ROOT = Path(__file__).resolve().parent


class GradeTests(unittest.TestCase):
    def test_scores_list_groups_and_partial_facts(self):
        case = {"expectedGroups": [["6117"], ["mTLS"]], "forbidden": []}
        self.assertEqual(grade.score_answer("port 6117, transport mTLS", case)["score"], 1.0)
        self.assertEqual(grade.score_answer("port 6117", case)["score"], 0.5)

    def test_accent_abstention_and_numeric_boundaries(self):
        unknown = {"expectedGroups": [["inconnu", "inconnue", "pas disponible"]], "forbidden": []}
        self.assertEqual(grade.score_answer("Information inconnue et non documentée.", unknown)["score"], 1.0)
        numeric = {"expectedGroups": [["17"]], "forbidden": []}
        self.assertEqual(grade.score_answer("La valeur est 117.", numeric)["missingGroups"], [["17"]])

    def test_forbidden_negation_and_long_body(self):
        case = {"expectedGroups": [["7223"]], "forbidden": ["6117"]}
        result = grade.score_answer("Je ne dois pas répéter 6117.", case)
        self.assertEqual(result["score"], 0.0)
        self.assertEqual(result["forbiddenMentions"], ["6117"])
        long_result = grade.score_answer(" ".join(["mot"] * 101), {"expectedGroups": [["mot"]], "forbidden": []})
        self.assertEqual(long_result["score"], 0.0)

    def test_invalid_execution_is_inconclusive(self):
        result = grade.score_answer("inconnu", {"expectedGroups": [["inconnu"]], "forbidden": []}, ["exitCode=1"])
        self.assertIsNone(result["score"])
        self.assertEqual(result["status"], "INCONCLUSIVE")

    def test_reconstruct_prompt_exact_accented_header(self):
        protocol = {"instruction": "Instruction"}
        packet = {"response": {"data": {"additionalContext": "mémoire"}}}
        case = {"question": "Pourquoi ?"}
        self.assertEqual(grade.reconstruct_prompt(protocol, case, "with", packet), "Instruction\n\nDonnées historiques :\nmémoire\n\nQuestion actuelle :\nPourquoi ?")

    def test_parse_authoritative_codex_fixture(self):
        raw = json.loads((ROOT / "parser-fixture.json").read_text(encoding="utf-8"))
        answer, thread, usage, errors = grade.parse_stdout_details(raw["stdout"])
        self.assertEqual(answer, raw["answer"])
        self.assertEqual(thread, raw["threadId"])
        self.assertEqual(usage, raw["usage"])
        self.assertEqual(errors, [])

    def test_parse_rejects_tool_item(self):
        stdout = "\n".join([
            json.dumps({"type": "thread.started", "thread_id": "t"}),
            json.dumps({"type": "item.completed", "item": {"type": "command_execution"}}),
            json.dumps({"type": "turn.completed", "usage": {}}),
        ])
        _, _, _, errors = grade.parse_stdout_details(stdout)
        self.assertTrue(any("unexpected item type" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
