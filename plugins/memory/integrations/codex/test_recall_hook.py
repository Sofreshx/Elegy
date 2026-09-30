import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


HOOK = Path(__file__).with_name("recall_hook.py")


class RecallHookTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name).resolve()
        self.config = self.root / "config.json"
        self.executable = self.root / "child.py"

    def tearDown(self):
        self.tmp.cleanup()

    def write_config(self, **values):
        config = {"mode": "inject", "projectRoot": str(self.root)}
        config.update(values)
        self.config.write_text(json.dumps(config), encoding="utf-8")

    def write_child(self, source):
        self.executable.write_text(textwrap.dedent(source), encoding="utf-8")

    def run_hook(self, event=None):
        if event is None:
            event = {
                "cwd": str(self.root),
                "hook_event_name": "UserPromptSubmit",
                "session_id": "session-1",
                "turn_id": "turn-1",
                "prompt": "hello",
                "transcript_path": str(self.root / "transcript.jsonl"),
            }
        return subprocess.run(
            [sys.executable, str(HOOK), "--config", str(self.config), "--executable", str(self.executable)],
            input=json.dumps(event).encode(),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_mode_off_does_not_launch_child(self):
        marker = self.root / "called"
        self.write_config(mode="off")
        self.write_child(f"Path({str(marker)!r}).write_text('called')")
        result = self.run_hook()
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, b"{}")
        self.assertFalse(marker.exists())

    def test_injection_uses_envelope(self):
        self.write_config()
        self.write_child(
            """
            import json, sys
            assert sys.argv[1] == "contextual-recall"
            assert sys.argv[2] == "--config"
            request = json.loads(sys.stdin.read())
            assert request["sessionId"] == "session-1"
            print(json.dumps({"status":"ok","data":{"schemaVersion":"memory-contextual-recall/v1","mode":"inject","status":"selected","additionalContext":"remember this"}}))
            """
        )
        result = self.run_hook()
        self.assertEqual(json.loads(result.stdout), {"hookSpecificOutput": {"hookEventName": "UserPromptSubmit", "additionalContext": "remember this"}})

    def test_observe_and_bad_envelopes_are_discarded(self):
        self.write_config()
        for envelope in (
            {"status": "ok", "data": {"schemaVersion": "memory-contextual-recall/v1", "mode": "observe", "status": "selected", "additionalContext": "x"}},
            {"status": "ok", "data": {"schemaVersion": "wrong", "mode": "inject", "status": "selected", "additionalContext": "x"}},
            {"status": "ok", "data": {"schemaVersion": "memory-contextual-recall/v1", "mode": "inject", "status": "empty", "additionalContext": "x"}},
        ):
            self.write_child("import json; print(json.dumps(" + repr(envelope) + "))")
            self.assertEqual(self.run_hook().stdout, b"{}")

    def test_null_transcript_still_recalls_current_prompt(self):
        self.write_config()
        self.write_child("import json; print(json.dumps({'status':'ok','data':{'schemaVersion':'memory-contextual-recall/v1','mode':'inject','status':'selected','additionalContext':'remember this'}}))")
        event = {"hook_event_name": "UserPromptSubmit", "cwd": str(self.root), "session_id": "s", "turn_id": "t", "prompt": "p", "transcript_path": None}
        self.assertIn("hookSpecificOutput", json.loads(self.run_hook(event).stdout))

    def test_malformed_input_and_wrong_event_fail_open(self):
        self.write_config()
        self.write_child("raise SystemExit(9)")
        result = subprocess.run([sys.executable, str(HOOK), "--config", str(self.config), "--executable", str(self.executable)], input=b"[]", stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.assertEqual(result.stdout, b"{}")
        event = {"hook_event_name": "Other", "cwd": str(self.root), "session_id": "s", "turn_id": "t", "prompt": "p", "transcript_path": ""}
        self.assertEqual(self.run_hook(event).stdout, b"{}")
        missing_marker = {"cwd": str(self.root), "session_id": "s", "turn_id": "t", "prompt": "p", "transcript_path": ""}
        self.assertEqual(self.run_hook(missing_marker).stdout, b"{}")

    def test_cwd_outside_project_is_rejected(self):
        self.write_config()
        self.write_child("print('{}')")
        event = {"hook_event_name": "UserPromptSubmit", "cwd": str(self.root.parent), "session_id": "s", "turn_id": "t", "prompt": "p", "transcript_path": ""}
        self.assertEqual(self.run_hook(event).stdout, b"{}")

    def test_valid_request_with_failing_child_fails_open(self):
        self.write_config()
        marker = self.root / "child-received-request.json"
        self.write_child(f"""
            import json, sys
            from pathlib import Path
            request = json.loads(sys.stdin.read())
            Path({str(marker)!r}).write_text(json.dumps(request), encoding="utf-8")
            print(json.dumps({{"status":"ok","data":{{"schemaVersion":"memory-contextual-recall/v1","mode":"inject","status":"selected","additionalContext":"must be discarded"}}}}))
            print("synthetic child failure", file=sys.stderr)
            raise SystemExit(9)
        """)
        result = self.run_hook()
        self.assertEqual(json.loads(marker.read_text(encoding="utf-8"))["sessionId"], "session-1")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, b"{}")
        self.assertEqual(result.stderr, b"")

    def test_shell_like_prompt_is_data(self):
        self.write_config()
        self.write_child("import json,sys; request=json.loads(sys.stdin.read()); print(json.dumps({'status':'ok','data':{'schemaVersion':'memory-contextual-recall/v1','mode':'inject','status':'selected','additionalContext':request['prompt']}}))")
        prompt = "$(touch pwned); `echo nope` & whoami"
        event = {"hook_event_name": "UserPromptSubmit", "cwd": str(self.root), "session_id": "s", "turn_id": "t", "prompt": prompt, "transcript_path": ""}
        result = self.run_hook(event)
        self.assertEqual(json.loads(result.stdout)["hookSpecificOutput"]["additionalContext"], prompt)
        self.assertFalse((self.root / "pwned").exists())

    def test_context_byte_budget_is_enforced(self):
        self.write_config(maxContextTokens=4)
        self.write_child("print('{\"status\":\"ok\",\"data\":{\"schemaVersion\":\"memory-contextual-recall/v1\",\"mode\":\"inject\",\"status\":\"selected\",\"additionalContext\":\"ééé\"}}')")
        self.assertEqual(self.run_hook().stdout, b"{}")

    def test_recent_context_reads_known_transcript_items(self):
        self.write_config(includeRecentContext=True)
        transcript = self.root / "transcript.jsonl"
        rows = [
            {"type": "response_item", "payload": {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "old question"}]}},
            {"type": "unknown", "payload": {"text": "ignore"}},
            {"type": "response_item", "payload": {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "old answer"}]}},
        ]
        transcript.write_text("\n".join(json.dumps(row) for row in rows), encoding="utf-8")
        self.write_child("import json,sys; print(json.dumps({'status':'ok','data':{'schemaVersion':'memory-contextual-recall/v1','mode':'inject','status':'selected','additionalContext':json.loads(sys.stdin.read())['recentContext'][0]}}))")
        result = self.run_hook()
        self.assertEqual(json.loads(result.stdout)["hookSpecificOutput"]["additionalContext"], "old question")

    def test_recent_context_selects_latest_four_and_trusted_transcript_root(self):
        transcript_root = self.root / "transcripts"
        transcript_root.mkdir()
        self.write_config(includeRecentContext=True, transcriptRoot=str(transcript_root))
        transcript = transcript_root / "session.jsonl"
        rows = [{"type": "response_item", "payload": {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": f"item-{i}"}]}} for i in range(6)]
        transcript.write_text("\n".join(json.dumps(row) for row in rows), encoding="utf-8")
        self.write_child("import json,sys; print(json.dumps({'status':'ok','data':{'schemaVersion':'memory-contextual-recall/v1','mode':'inject','status':'selected','additionalContext':json.dumps(json.loads(sys.stdin.read())['recentContext'])}}))")
        event = {"hook_event_name": "UserPromptSubmit", "cwd": str(self.root), "session_id": "s", "turn_id": "t", "prompt": "p", "transcript_path": str(transcript)}
        result = subprocess.run([sys.executable, str(HOOK), "--config", str(self.config), "--executable", str(self.executable)], input=json.dumps(event).encode(), stdout=subprocess.PIPE, check=False)
        self.assertEqual(json.loads(json.loads(result.stdout)["hookSpecificOutput"]["additionalContext"]), ["item-2", "item-3", "item-4", "item-5"])

    def test_unsupported_response_item_schema_yields_no_recent_context(self):
        self.write_config(includeRecentContext=True)
        transcript = self.root / "transcript.jsonl"
        transcript.write_text(json.dumps({"type": "response_item", "payload": {"type": "message", "role": "assistant", "content": [{"type": "tool_call", "text": "secret"}]}}), encoding="utf-8")
        self.write_child("import json,sys; print(json.dumps({'status':'ok','data':{'schemaVersion':'memory-contextual-recall/v1','mode':'inject','status':'selected','additionalContext':str(json.loads(sys.stdin.read())['recentContext'])}}))")
        result = self.run_hook()
        self.assertEqual(json.loads(result.stdout)["hookSpecificOutput"]["additionalContext"], "[]")

    def test_timeout_and_oversized_input_fail_open(self):
        self.write_config()
        self.write_child("import time; time.sleep(2)")
        self.assertEqual(self.run_hook().stdout, b"{}")
        oversized = b"{" + b"x" * 65536 + b"}"
        result = subprocess.run([sys.executable, str(HOOK), "--config", str(self.config), "--executable", str(self.executable)], input=oversized, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.assertEqual(result.stdout, b"{}")

    def test_continuous_child_output_is_bounded(self):
        self.write_config()
        self.write_child("import sys,time; sys.stdout.write('x' * 70000); sys.stdout.flush(); time.sleep(2)")
        result = self.run_hook()
        self.assertEqual(result.stdout, b"{}")


if __name__ == "__main__":
    unittest.main()
