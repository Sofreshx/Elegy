import json
import unittest
import tempfile
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import grade
import verify_diagnostics as v


class VerifyDiagnosticsTests(unittest.TestCase):
    def test_source_anchor_rejects_episode_and_scorer_drift(self):
        (v.ROOT / "runtime").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=v.ROOT / "runtime") as directory:
            base = Path(directory).resolve()
            self.assertTrue(base.is_relative_to(v.ROOT / "runtime"))
            root = base / "diagnostic"
            source = base / "recall-fixes-2026-10-23"
            root.mkdir()
            (source / "replay-01" / "fixture").mkdir(parents=True)
            corpus = {"episodes": [{"id": "fixture"}]}
            for owner in (root, source):
                (owner / "grade.py").write_bytes(b"scorer")
                (owner / "corpus.json").write_text(json.dumps(corpus), encoding="utf-8")
            for name in ("subject.json", "source-lock.json", "replay-01/run.json", "replay-01/fixture/run.json"):
                (source / name).write_text("{}", encoding="utf-8")
            paths = ("grade.py", "corpus.json", "replay-01/run.json", "replay-01/fixture/run.json")
            (source / "evidence-manifest.json").write_text(json.dumps({"files": [
                {"path": name, "sha256": v.sha(source / name)} for name in paths]}), encoding="utf-8")
            anchors = {"evidenceManifestSha256": v.sha(source / "evidence-manifest.json"),
                       "subjectSha256": v.sha(source / "subject.json"),
                       "sourceLockSha256": v.sha(source / "source-lock.json")}
            (root / "local-checks.json").write_text(json.dumps({"priorEvidence": anchors}), encoding="utf-8")
            (root / "evidence-manifest.json").write_text(json.dumps({"scorerAnchor": {
                "revision": "0" * 40, "path": "grade.py", "sha256": v.sha(root / "grade.py")}}), encoding="utf-8")
            with patch.object(v.subprocess, "run", return_value=SimpleNamespace(returncode=0, stdout=b"scorer")):
                errors = []
                v.verify_source_inputs(root, corpus, errors)
                self.assertEqual(errors, [])
                episode = source / "replay-01/fixture/run.json"
                episode.write_text('{"changed":true}', encoding="utf-8")
                v.verify_source_inputs(root, corpus, errors)
                self.assertTrue(any("replay-01/fixture/run.json" in error for error in errors))
                episode.write_text("{}", encoding="utf-8")
                (root / "grade.py").write_bytes(b"changed scorer")
                errors = []
                v.verify_source_inputs(root, corpus, errors)
                self.assertTrue(any("scorer differs" in error for error in errors))

    def test_file_manifest_detects_changed_and_unlisted_proofs(self):
        (v.ROOT / "runtime").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=v.ROOT / "runtime") as directory:
            root = Path(directory).resolve()
            self.assertTrue(root.is_relative_to(v.ROOT / "runtime"))
            proof = root / "proof.json"
            proof.write_text("{}", encoding="utf-8")
            manifest = {"files": [{"path": "proof.json", "sha256": v.sha(proof)}]}
            (root / "evidence-manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            errors = []
            v.verify_file_manifest(root, errors)
            self.assertEqual(errors, [])
            proof.write_text('{"changed":true}', encoding="utf-8")
            v.verify_file_manifest(root, errors)
            self.assertTrue(any("hash mismatch" in error for error in errors))
            (root / "unlisted.json").write_text("{}", encoding="utf-8")
            errors = []
            v.verify_file_manifest(root, errors)
            self.assertIn("file manifest inventory mismatch", errors)

    def test_expected_inventory_is_exact(self):
        keys = v.expected_keys(["current", "candidate"], {case: 3 for case in v.CASE_TRIALS})
        self.assertEqual(len(keys), 30)
        self.assertEqual(len({(a, b) for a, b, _ in keys}), 15)
        self.assertIn(("q08", 3, "candidate"), keys)
        self.assertIn(("q17", 3, "current"), keys)

    def test_transformations_preserve_or_ablate_only_declared_data(self):
        base = "Prefix\n" + json.dumps({"event": "e", "records": [{"id": "i", "text": "Apollo", "scope": "Agent"}]}, separators=(",", ":"))
        design = {"candidateEnvelope": "Candidate\n", "conditions": {"historical-generic": "Typed\n"}}
        self.assertEqual(v.transform_context(base, design, "current", "diagnostic-01"), base)
        first = json.loads(v.transform_context(base, {}, "text-first", "layout-01").split("\n", 1)[1])
        self.assertEqual(list(first["records"][0]), ["text", "id", "scope"])
        only = json.loads(v.transform_context(base, {}, "text-only", "layout-01").split("\n", 1)[1])
        self.assertEqual(only, {"records": [{"text": "Apollo"}]})
        self.assertEqual(v.transform_context(base, design, "historical-generic", "semantics-01").split("\n", 1)[1], base.split("\n", 1)[1])

    def test_unsafe_raw_paths_are_rejected(self):
        root = v.ROOT
        with self.assertRaises(ValueError): v.safe_raw(root, "../escape.json")
        with self.assertRaises(ValueError): v.safe_raw(root, str(root / "absolute.json"))

    def test_strict_stdout_rejects_malformed_jsonl(self):
        stdout = "\n".join([
            json.dumps({"type":"thread.started", "thread_id":"t1"}),
            json.dumps({"type":"item.completed", "item":{"type":"agent_message", "text":"ok"}}),
            json.dumps({"type":"turn.completed", "usage":{"output_tokens":1}}),
        ])
        answer, thread, usage, errors = grade.parse_stdout_details(stdout)
        self.assertEqual((answer, thread, usage, errors), ("ok", "t1", {"output_tokens":1}, []))
        _, _, _, errors = grade.parse_stdout_details(stdout + "\nnot-json")
        self.assertTrue(errors)

    def test_pure_raw_projection_rejects_each_material_mutation(self):
        import copy
        import hashlib
        context = "Prefix\n{}"
        instruction = "Instruction"
        case = {"id": "q", "question": "Question", "expectedGroups": [["Apollo"]], "forbidden": []}
        stdout = "\n".join([
            json.dumps({"type": "thread.started", "thread_id": "t1"}),
            json.dumps({"type": "item.completed", "item": {"type": "agent_message", "text": "Apollo"}}),
            json.dumps({"type": "turn.completed", "usage": {"output_tokens": 1}}),
        ])
        prompt = instruction + "\n\nDonnées historiques :\n" + context + "\n\nQuestion actuelle :\nQuestion"
        raw = {"caseId": "q", "arm": "with", "command": ["<codex>", "exec"],
               "stdout": stdout, "answer": "Apollo", "threadId": "t1", "usage": {"output_tokens": 1}, "exitCode": 0,
               "valid": True, "configurationSha256": "cfg", "promptSha256": hashlib.sha256(prompt.encode()).hexdigest()}
        record = {"answer": "Apollo", "threadId": "t1", "usage": {"output_tokens": 1}, "configurationSha256": "cfg",
                  "promptSha256": raw["promptSha256"], "score": 1.0, "missingGroups": [], "forbiddenMentions": [],
                  "formatErrors": [], "executionErrors": []}
        def accepts(r=record, w=raw, c=context, i=instruction, seen=None):
            return v.verify_raw_record(r, w, case, c, i, "cfg", seen, expected_command=["<codex>", "exec"])
        self.assertEqual(accepts(seen=set()), "t1")
        mutations = []
        changed = copy.deepcopy(record); changed["answer"] = "changed"; mutations.append((changed, raw, context, instruction))
        changed = copy.deepcopy(raw); changed["answer"] = "changed"; mutations.append((record, changed, context, instruction))
        mutations.append((record, raw, "Other\n{}", instruction))
        changed = copy.deepcopy(record); changed["promptSha256"] = "0" * 64; mutations.append((changed, raw, context, instruction))
        changed = copy.deepcopy(raw); changed["promptSha256"] = "0" * 64; mutations.append((record, changed, context, instruction))
        changed = copy.deepcopy(raw); changed["configurationSha256"] = "other"; mutations.append((record, changed, context, instruction))
        changed = copy.deepcopy(record); changed["configurationSha256"] = "other"; mutations.append((changed, raw, context, instruction))
        for field, value in (("caseId", "q-other"), ("arm", "without"), ("command", ["<codex>", "changed-model"])):
            changed = copy.deepcopy(raw); changed[field] = value; mutations.append((record, changed, context, instruction))
        for args in mutations:
            with self.assertRaises(ValueError): accepts(*args)
        with self.assertRaises(ValueError): accepts(seen={"t1"})
        legacy = copy.deepcopy(record)
        del legacy["promptSha256"]
        with self.assertRaises(ValueError): accepts(r=legacy)
        self.assertEqual(v.verify_raw_record(legacy, raw, case, context, instruction, "cfg",
                                             require_record_prompt_hash=False), "t1")
        corrupt = copy.deepcopy(raw)
        corrupt["promptSha256"] = "0" * 64
        with self.assertRaises(ValueError):
            v.verify_raw_record(legacy, corrupt, case, context, instruction, "cfg",
                                require_record_prompt_hash=False)

    def test_boundary_instruction_is_reconstructed_from_protocol(self):
        protocol = v.load(v.ROOT / "protocol.json")
        design = v.load(v.ROOT / "boundary-protocol.json")
        run = v.load(v.ROOT / "boundary-01" / "run.json")
        source = v.replay_contexts(v.ROOT.parent / "recall-fixes-2026-10-23" / "replay-01", v.load(v.ROOT / "corpus.json"), [])
        base = source["q08"]
        expected = v.effective_instruction(protocol, design, run, "boundary-01", "separate-policy", base)
        self.assertEqual(expected, run["effectiveInstructions"]["separate-policy"])
        altered = dict(run)
        altered["effectiveInstructions"] = dict(run["effectiveInstructions"])
        altered["effectiveInstructions"]["separate-policy"] += "tampered"
        with self.assertRaises(ValueError): v.effective_instruction(protocol, design, altered, "boundary-01", "separate-policy", base)


if __name__ == "__main__": unittest.main()
