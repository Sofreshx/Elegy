import importlib.util
import json
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("sanitize_evidence.py")
SPEC = importlib.util.spec_from_file_location("sanitize_evidence", MODULE_PATH)
sanitize_evidence = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(sanitize_evidence)


class SanitizerTests(unittest.TestCase):
    def test_plain_text_is_unchanged(self):
        result, counts = sanitize_evidence.sanitize_text("status=ok\ncount=3\n")
        self.assertEqual(result, "status=ok\ncount=3\n")
        self.assertEqual(counts, {"windows_absolute_path": 0, "neutralized_appdata_fragment": 0})

    def test_single_and_doubled_windows_paths_are_neutralized(self):
        drive = "C" + ":"
        separator = "\\"
        user_component = "Us" + "ers"
        fixture_component = "fix" + "ture"
        single_path = drive + separator + separator.join((user_component, fixture_component, "one.txt"))
        doubled_path = drive + separator + separator + separator.join((user_component, fixture_component, "two.txt"))
        source = "single=" + single_path + " doubled=" + doubled_path
        result, counts = sanitize_evidence.sanitize_text(source)
        self.assertEqual(result, "single=<local-path> doubled=<local-path>")
        self.assertEqual(counts["windows_absolute_path"], 2)

    def test_neutralized_appdata_fragment_is_normalized(self):
        app_component = "App" + "Data"
        local_component = "Lo" + "cal"
        source = "tmp=<user-home>" + "\\" + app_component + "\\" + local_component + "\\Temp\\fixture"
        result, counts = sanitize_evidence.sanitize_text(source)
        self.assertEqual(result, "tmp=<user-home>\\<local-appdata>\\Temp\\fixture")
        self.assertEqual(counts["neutralized_appdata_fragment"], 1)

    def test_real_bindings_reconstruct_all_three_files(self):
        manifest_path = MODULE_PATH.with_name("redactions.json")
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        self.assertEqual(manifest["originalCommit"], sanitize_evidence.COMMIT)
        self.assertEqual(len(manifest["files"]), 3)
        sanitize_evidence.verify_records(manifest["files"])

    def test_tampered_binding_is_rejected(self):
        manifest_path = MODULE_PATH.with_name("redactions.json")
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        tampered = json.loads(json.dumps(manifest))
        tampered["files"][0]["currentSha256"] = "0" * 64
        with self.assertRaises(ValueError):
            sanitize_evidence.verify_records(tampered["files"])


if __name__ == "__main__":
    unittest.main()
