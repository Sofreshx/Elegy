import copy
import unittest

import assess


class EvidenceBindingTests(unittest.TestCase):
    def test_review_binds_the_actual_question_and_context(self):
        question, context = "Question courante ?", "Contexte exact."
        record = {"answer": "Une réponse.", "rawSha256": "a" * 64}
        item = {"rawSha256": record["rawSha256"], "answerSha256": assess.sha(record["answer"].encode()),
                "questionSha256": assess.sha(question.encode()), "contextSha256": assess.sha(context.encode())}
        key = ("answers-primary", "q01", "with", 1)
        errors = []
        assess.check_review_binding(item, record, key, errors, question, context)
        self.assertFalse(errors)
        for field in ("questionSha256", "contextSha256"):
            changed = dict(item)
            changed[field] = "0" * 64
            errors = []
            assess.check_review_binding(changed, record, key, errors, question, context)
            self.assertTrue(errors, field)

    def test_all_normalized_recall_configuration_fields_are_bound(self):
        protocol = {"memory": {"maxContextTokens": 600, "minSimilarity": 0.2}}
        case = {"agent": "qual-a"}
        request = {"cwd": "<runtime>"}
        config = {"version": 1, "projectRoot": "<runtime>", "dbPath": "<source.sqlite>",
                  "statePath": "<journal-qual-a>", "domain": "realistic-qualification", "scopes": ["Agent"],
                  "mode": "inject", "agentId": "qual-a", "maxSensitivity": "Low", "maxContextTokens": 600,
                  "minSimilarity": 0.2, "learningEnabled": False, "includeRecentContext": False}
        self.assertTrue(assess.recall_configuration_matches(config, request, case, protocol))
        for field in config:
            changed = copy.deepcopy(config)
            changed[field] = "wrong"
            self.assertFalse(assess.recall_configuration_matches(changed, request, case, protocol), field)
        self.assertFalse(assess.recall_configuration_matches(config, {"cwd": "outside"}, case, protocol))


if __name__ == "__main__":
    unittest.main()
