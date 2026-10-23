import json
from pathlib import Path, PurePosixPath
import tempfile
import unittest
from unittest.mock import patch

import campaign
import run_native


class CampaignTests(unittest.TestCase):
    def test_public_normalizes_windows_paths_recursively_with_boundaries(self):
        python = r"X:\fixture\Python.EXE"
        repo = r"X:\fixture\repo"
        home = r"X:\fixture\home"
        value = {
            "argv": [
                r"x:/FIXTURE/python.exe",
                r"x:/fixture/repo/sub\file.json",
                r"X:\fixture\repo-other\keep.txt",
                r"X:/fixture/home/.cache\item",
                "ordinary/string",
            ],
            "text": r"X:\fixture\repo-other\keep.txt",
        }
        with patch.object(campaign.sys, "executable", python), \
                patch.object(campaign, "REPO", Path(repo)), \
                patch.object(campaign.Path, "home", return_value=Path(home)):
            self.assertEqual(campaign.public(value), {
                "argv": [
                    "<python>",
                    "sub/file.json",
                    r"X:\fixture\repo-other\keep.txt",
                    "<home>/.cache/item",
                    "ordinary/string",
                ],
                "text": r"X:\fixture\repo-other\keep.txt",
            })

    def test_run_native_public_normalizes_windows_paths_with_boundaries(self):
        python = r"X:\fixture\Python.EXE"
        repo = r"X:\fixture\repo"
        home = r"X:\fixture\home"
        with patch("sys.executable", python), \
                patch.object(run_native, "REPO", Path(repo)), \
                patch.object(run_native.Path, "home", return_value=Path(home)):
            self.assertEqual(run_native.public([
                r"x:/FIXTURE/python.exe",
                r"x:/fixture/repo/sub\file.json",
                r"X:\fixture\repo-other\keep.txt",
                r"X:/fixture/home/.cache\item",
                "ordinary/string",
            ]), [
                "<python>",
                "<repo>/sub/file.json",
                r"X:\fixture\repo-other\keep.txt",
                "<home>/.cache/item",
                "ordinary/string",
            ])

    def test_public_functions_preserve_posix_case_and_boundaries(self):
        repo = "/fixture/Repo"
        home = "/fixture/Home"
        campaign_value = [
            "/fixture/Repo/sub/file",
            "/fixture/repo/sub/file",
            "/fixture/Repo-other/keep",
            "/fixture/Home/cache/item",
        ]
        native_value = [*campaign_value]
        with patch.object(campaign, "REPO", PurePosixPath(repo)), \
                patch.object(campaign.Path, "home", return_value=PurePosixPath(home)), \
                patch.object(run_native, "REPO", PurePosixPath(repo)), \
                patch.object(run_native.Path, "home", return_value=PurePosixPath(home)), \
                patch("sys.executable", "/fixture/Python"):
            self.assertEqual(campaign.public(campaign_value), [
                "sub/file",
                "/fixture/repo/sub/file",
                "/fixture/Repo-other/keep",
                "<home>/cache/item",
            ])
            self.assertEqual(run_native.public(native_value), [
                "<repo>/sub/file",
                "/fixture/repo/sub/file",
                "/fixture/Repo-other/keep",
                "<home>/cache/item",
            ])
    def test_phase_specs_are_frozen(self):
        self.assertEqual(campaign.phase_spec("repeats"), ("repeats", "corpus.json", ["--replay", "replay-primary", "--cases", "q08,q09,q11,q17,q18", "--trials", "2,3"]))
        self.assertEqual(campaign.phase_spec("replay-transfer")[:2], ("replay", "transfer-corpus.json"))

    def test_preflight_rejects_source_or_binary_drift(self):
        original_sha = campaign.sha
        cache = {}
        def cached_sha(path):
            path = Path(path).resolve()
            if path not in cache:
                cache[path] = original_sha(path)
            return cache[path]
        with patch.object(campaign, "sha", side_effect=cached_sha):
            control = campaign.preflight()
        self.assertTrue(control["checks"]["all"])
        lock = campaign.load(campaign.Q1 / "source-lock.json")
        targets = [campaign.REPO / lock["sourceFiles"][0]["path"],
                   Path(control["binaries"]["cli"]["path"]),
                   campaign.ROOT / "corpus.json", campaign.PROTOCOL,
                   campaign.Q2 / "qualification_runner.py", campaign.ROOT / "assess.py", campaign.ROOT / "verify_source.py"]
        for target in targets:
            with self.subTest(target=target.name):
                def changed_sha(path):
                    return "0" * 64 if Path(path).resolve() == target.resolve() else cached_sha(path)
                with patch.object(campaign, "sha", side_effect=changed_sha):
                    with self.assertRaises(ValueError):
                        campaign.preflight()

    def test_run_phase_refuses_existing_output(self):
        with tempfile.TemporaryDirectory(dir=campaign.ROOT) as td:
            root = Path(td); (root / "replay-primary").mkdir()
            with patch.object(campaign, "ROOT", root), patch.object(campaign, "preflight", return_value={"binaries": {"cli": {"path": "cli"}, "mcp": {"path": "mcp"}}}):
                with self.assertRaises(RuntimeError): campaign.run_phase("replay-primary")

    def test_runner_failure_is_preserved(self):
        with tempfile.TemporaryDirectory(dir=campaign.ROOT) as td:
            root = Path(td); protocol = root / "protocol.json"; subject = root / "subject.json"
            protocol.write_text("{}", encoding="utf-8"); subject.write_text("{}", encoding="utf-8")
            fake = type("Result", (), {"returncode": 1, "stdout": "out", "stderr": "err"})()
            before = {"binaries": {"cli": {"path": "cli"}, "mcp": {"path": "mcp"}}}
            def fake_run(*args, **kwargs):
                output = root / "replay-primary"; output.mkdir(); (output / "run.json").write_text(json.dumps({"status": "failed"}), encoding="utf-8")
                return fake
            with patch.object(campaign, "ROOT", root), patch.object(campaign, "REPO", root), patch.object(campaign, "PROTOCOL", protocol), patch.object(campaign, "SUBJECT", subject), patch.object(campaign, "preflight", side_effect=[before, before]), patch.object(campaign.subprocess, "run", side_effect=fake_run):
                self.assertEqual(campaign.run_phase("replay-primary"), 1)
            report = json.loads((root / "replay-primary-wrapper.json").read_text(encoding="utf-8"))
            self.assertEqual(report["status"], "failed")
            self.assertEqual(report["exitCode"], 1)

    def test_wrapper_alone_prevents_retry_before_preflight(self):
        with tempfile.TemporaryDirectory(dir=campaign.ROOT) as td:
            root = Path(td)
            (root / "replay-primary-wrapper.json").write_text("{}", encoding="utf-8")
            with patch.object(campaign, "ROOT", root), patch.object(campaign, "preflight") as preflight:
                with self.assertRaises(RuntimeError):
                    campaign.run_phase("replay-primary")
                preflight.assert_not_called()

    def test_replay_after_hash_is_checked_with_a_passing_control(self):
        for changed in (False, True):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory(dir=campaign.ROOT) as td:
                root = Path(td)
                protocol, subject = root / "protocol.json", root / "subject.json"
                protocol.write_text("{}", encoding="utf-8")
                subject.write_text("{}", encoding="utf-8")
                before = {"binaries": {"cli": {"path": "cli", "sha256": campaign.EXPECTED_CLI},
                                       "mcp": {"path": "mcp", "sha256": campaign.EXPECTED_MCP}},
                          "subjectSha256": campaign.sha(subject), "protocolSha256": campaign.sha(protocol)}
                def fake_run(*args, **kwargs):
                    output = root / "replay-primary"
                    output.mkdir()
                    data = {"status": "completed", "cliSha256": campaign.EXPECTED_CLI,
                            "mcpSha256": campaign.EXPECTED_MCP,
                            "cliSha256After": "changed" if changed else campaign.EXPECTED_CLI,
                            "mcpSha256After": campaign.EXPECTED_MCP}
                    (output / "run.json").write_text(json.dumps(data), encoding="utf-8")
                    return type("Result", (), {"returncode": 0, "stdout": "", "stderr": ""})()
                with patch.object(campaign, "ROOT", root), patch.object(campaign, "REPO", root), \
                     patch.object(campaign, "PROTOCOL", protocol), patch.object(campaign, "SUBJECT", subject), \
                     patch.object(campaign, "preflight", side_effect=[before, before]), \
                     patch.object(campaign.subprocess, "run", side_effect=fake_run):
                    self.assertEqual(campaign.run_phase("replay-primary"), int(changed))


if __name__ == "__main__":
    unittest.main()
