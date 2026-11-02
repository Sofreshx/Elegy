"""Isolated activation-helper fixtures; never opens an installed user database."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("install_shared_memory", ROOT / "scripts/install-shared-memory.py")
INSTALL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(INSTALL)


class InstallFixture(unittest.TestCase):
    def setUp(self):
        fixture_root = ROOT / "target/qualification"
        fixture_root.mkdir(parents=True, exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(prefix="activation-", dir=fixture_root)
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.config = self.root / ".codex/config.toml"
        self.config.parent.mkdir()
        self.database = self.root / ".elegy/memory.db"
        self.database.parent.mkdir()
        with contextlib.closing(sqlite3.connect(self.database)) as connection:
            connection.execute("create table memories (text text)")
            connection.execute("insert into memories values ('fictional initial memory')")
            connection.commit()
        self.original = {"command": str(self.root / "previous-memory.exe"), "env": {"ELEGY_DB_PATH": str(self.database)}}
        self.config.write_text('model = "fictional-model"\n\n[preferences]\ncolor = "blue"\n\n'
                               + INSTALL.fragment(self.original) + '\n[mcp_servers.other]\ncommand = "fictional-other"\n', encoding="utf-8")
        self.commit = "a" * 40
        self.destination = self.config.parent / "bin/elegy-memory" / self.commit
        self.backup = self.config.parent / "private-data/elegy-memory-dot/backups" / ("before-shared-" + self.commit[:7])
        self.profile = self.root / "fictional-profile.yaml"
        self.profile.write_text("name: fictional-profile\n", encoding="utf-8")
        artifacts = {}
        for name in ("elegy-memory.exe", "elegy-memory-mcp-stdio.exe"):
            source = self.root / name
            source.write_bytes(("fictional executable bytes: " + name).encode())
            artifacts[name] = {"path": str(source), "sha256": INSTALL.digest(source)}
        self.desired = {"command": str(self.destination / "elegy-memory-mcp-stdio.exe"), "env": {
            "ELEGY_DB_PATH": str(self.database), "ELEGY_MCP_AGENT_ID": "fictional-actor",
            "ELEGY_MCP_SHARED_SPACE": "fictional-space", "ELEGY_MCP_SHARED_AGENTS": '["fictional-actor"]'}}
        self.plan = {"sourceCommit": self.commit, "codexConfig": str(self.config), "database": str(self.database),
                     "installDirectory": str(self.destination), "backupDirectory": str(self.backup),
                     "expectedCurrentMemoryEntrySha256": INSTALL.entry_hash(self.original),
                     "codexMemoryEntry": self.desired, "artifacts": artifacts,
                     "tunnelProfile": str(self.profile), "tunnelProfileSha256": INSTALL.digest(self.profile)}
        self.plan_file = self.root / "reviewed-plan.json"

    def run_install(self, mode=None):
        self.plan_file.write_text(json.dumps(self.plan), encoding="utf-8")
        args = ["install-shared-memory.py", "--plan", str(self.plan_file), "--plan-sha256", INSTALL.digest(self.plan_file)]
        if mode:
            args.append(mode)
        with mock.patch.object(sys, "argv", args), contextlib.redirect_stdout(io.StringIO()) as output:
            INSTALL.main()
        return json.loads(output.getvalue())

    def binding(self):
        return tomllib.loads(self.config.read_text(encoding="utf-8"))["mcp_servers"]["elegy-memory"]

    def receipt(self):
        return json.loads((self.backup / "install-receipt.json").read_text(encoding="utf-8"))

    def test_dry_run_never_opens_or_writes_database(self):
        original = self.database.read_bytes(), self.database.stat().st_mtime_ns, self.config.read_bytes()
        with mock.patch.object(INSTALL.sqlite3, "connect", side_effect=AssertionError("dry-run opened SQLite")):
            self.assertTrue(self.run_install()["dryRun"])
        self.assertEqual(original, (self.database.read_bytes(), self.database.stat().st_mtime_ns, self.config.read_bytes()))
        self.assertFalse(self.backup.exists())
        self.assertFalse(self.destination.exists())

    def test_apply_and_rollback_preserve_config_and_new_memories(self):
        before = tomllib.loads(self.config.read_text(encoding="utf-8"))
        self.run_install("--apply-approved")
        self.assertEqual(self.binding(), self.desired)
        receipt = self.receipt()
        snapshot = Path(receipt["backupPath"])
        self.assertEqual(INSTALL.digest(snapshot), receipt["backupSha256"])
        with contextlib.closing(sqlite3.connect(snapshot)) as connection:
            self.assertEqual(connection.execute("pragma integrity_check").fetchone()[0], "ok")
        with contextlib.closing(sqlite3.connect(self.database)) as connection:
            connection.execute("insert into memories values ('fictional new contribution')")
            connection.commit()
        self.config.write_text(self.config.read_text(encoding="utf-8") + '\n[new_preferences]\nkeep = true\n', encoding="utf-8")
        self.assertFalse(self.run_install("--rollback-config-approved")["databaseRestoredOrDeleted"])
        after = tomllib.loads(self.config.read_text(encoding="utf-8"))
        self.assertEqual({key: value for key, value in after.items() if key != "new_preferences"}, before)
        self.assertEqual(after["new_preferences"], {"keep": True})
        with contextlib.closing(sqlite3.connect(self.database)) as connection:
            self.assertEqual(connection.execute("select count(*) from memories").fetchone()[0], 2)

    def test_completed_apply_and_rollback_can_be_repeated(self):
        self.run_install("--apply-approved")
        receipt = self.receipt()
        self.assertTrue(self.run_install("--apply-approved")["configurationUpdated"])
        self.assertEqual(self.receipt()["backupSha256"], receipt["backupSha256"])
        self.run_install("--rollback-config-approved")
        self.assertTrue(self.run_install("--rollback-config-approved")["configRestored"])

    def test_snapshot_receipt_failure_preserves_candidate_and_retry_succeeds(self):
        write = INSTALL.atomic_text

        def fail_verification_receipt(path, text, **options):
            if path.name == "install-receipt.json" and json.loads(text).get("phase") == "snapshot-ready":
                raise OSError("fictional receipt failure")
            return write(path, text, **options)

        with mock.patch.object(INSTALL, "atomic_text", side_effect=fail_verification_receipt):
            with self.assertRaises(OSError):
                self.run_install("--apply-approved")
        candidates = {path: path.read_bytes() for path in self.backup.glob("*.sqlite3")}
        self.assertTrue(candidates)
        self.assertEqual(self.binding(), self.original)
        self.assertTrue(self.run_install("--apply-approved")["installed"])
        for path, data in candidates.items():
            self.assertEqual(path.read_bytes(), data)
        self.assertNotIn(Path(self.receipt()["backupPath"]), candidates)

    def test_partial_artifact_failure_reuses_verified_backup_on_retry(self):
        copy = INSTALL.shutil.copyfileobj
        calls = 0

        def fail_copy(source, output):
            nonlocal calls
            calls += 1
            if calls == 2:
                output.write(b"fictional interrupted copy")
                raise OSError("fictional copy failure")
            return copy(source, output)

        with mock.patch.object(INSTALL.shutil, "copyfileobj", side_effect=fail_copy):
            with self.assertRaises(OSError):
                self.run_install("--apply-approved")
        receipt = self.receipt()
        self.assertEqual(receipt["phase"], "snapshot-ready")
        snapshot = Path(receipt["backupPath"])
        snapshot_bytes = snapshot.read_bytes()
        self.assertTrue(self.run_install("--apply-approved")["installed"])
        self.assertEqual(self.receipt()["backupPath"], str(snapshot))
        self.assertEqual(snapshot.read_bytes(), snapshot_bytes)

    def test_original_binding_survives_failure_before_first_receipt(self):
        with mock.patch.object(INSTALL, "atomic_text", side_effect=OSError("fictional write failure")):
            with self.assertRaises(OSError):
                self.run_install("--apply-approved")
        self.assertEqual(self.binding(), self.original)
        self.assertTrue(self.run_install("--apply-approved")["installed"])

    def test_interrupted_original_file_write_can_be_retried(self):
        with mock.patch.object(INSTALL.os, "fsync", side_effect=OSError("fictional original flush failure")):
            with self.assertRaises(OSError):
                self.run_install("--apply-approved")
        # There must be no unverified original file that a retry will trust.
        self.assertFalse((self.backup / "codex-memory-before.toml").exists())
        self.assertTrue(self.run_install("--apply-approved")["installed"])

    def test_failure_after_config_write_is_safely_completed_on_retry(self):
        write = INSTALL.atomic_text

        def fail_final_receipt(path, text, **options):
            if path.name == "install-receipt.json" and json.loads(text).get("phase") == "configured":
                raise OSError("fictional final receipt failure")
            return write(path, text, **options)

        with mock.patch.object(INSTALL, "atomic_text", side_effect=fail_final_receipt):
            with self.assertRaises(OSError):
                self.run_install("--apply-approved")
        self.assertEqual(self.binding(), self.desired)
        self.assertEqual(self.receipt()["phase"], "snapshot-ready")
        self.assertTrue(self.run_install("--apply-approved")["installed"])

    def test_rollback_refuses_a_concurrent_binding(self):
        self.run_install("--apply-approved")
        concurrent = {"command": "fictional-concurrent.exe", "env": {}}
        self.config.write_text(INSTALL.replace_entry(self.config.read_text(encoding="utf-8"), concurrent), encoding="utf-8")
        with self.assertRaises(ValueError):
            self.run_install("--rollback-config-approved")
        self.assertEqual(self.binding(), concurrent)

    def edit_during_config_flush(self, mode):
        atomic = INSTALL.atomic_text
        sync = INSTALL.os.fsync
        concurrent_text = None

        def flush_and_edit(descriptor):
            nonlocal concurrent_text
            sync(descriptor)
            concurrent_text = self.config.read_text(encoding="utf-8") + '\n[late_preferences]\nkeep = "fictional concurrent edit"\n'
            self.config.write_text(concurrent_text, encoding="utf-8")

        def prepare(path, text, **options):
            if path == self.config:
                with mock.patch.object(INSTALL.os, "fsync", side_effect=flush_and_edit):
                    return atomic(path, text, **options)
            return atomic(path, text, **options)

        with mock.patch.object(INSTALL, "atomic_text", side_effect=prepare), self.assertRaises(ValueError):
            self.run_install(mode)
        self.assertIsNotNone(concurrent_text)
        self.assertEqual(self.config.read_text(encoding="utf-8"), concurrent_text)

    def test_apply_rechecks_complete_config_after_temporary_file_flush(self):
        self.edit_during_config_flush("--apply-approved")
        self.assertEqual(self.binding(), self.original)

    def test_rollback_rechecks_complete_config_after_temporary_file_flush(self):
        self.run_install("--apply-approved")
        self.edit_during_config_flush("--rollback-config-approved")
        self.assertEqual(self.binding(), self.desired)

    def test_differing_installed_binary_is_never_overwritten(self):
        self.destination.mkdir(parents=True)
        target = self.destination / "elegy-memory.exe"
        target.write_bytes(b"fictional preexisting different version")
        with self.assertRaises(ValueError):
            self.run_install("--apply-approved")
        self.assertEqual(target.read_bytes(), b"fictional preexisting different version")
        self.assertEqual(self.binding(), self.original)

    def test_concurrent_binding_edit_is_preserved(self):
        copy = INSTALL.shutil.copyfileobj
        concurrent = {"command": "fictional-concurrent.exe", "env": {}}

        def edit_binding(source, output):
            copy(source, output)
            self.config.write_text(INSTALL.replace_entry(self.config.read_text(encoding="utf-8"), concurrent), encoding="utf-8")

        with mock.patch.object(INSTALL.shutil, "copyfileobj", side_effect=edit_binding):
            with self.assertRaises(ValueError):
                self.run_install("--apply-approved")
        self.assertEqual(self.binding(), concurrent)
        with self.assertRaises(ValueError):
            self.run_install("--apply-approved")

    def test_unrelated_concurrent_preferences_survive_apply(self):
        copy = INSTALL.shutil.copyfileobj

        def edit_preferences(source, output):
            copy(source, output)
            if "[concurrent_preferences]" not in self.config.read_text(encoding="utf-8"):
                self.config.write_text(self.config.read_text(encoding="utf-8") + '\n[concurrent_preferences]\nkeep = true\n', encoding="utf-8")

        with mock.patch.object(INSTALL.shutil, "copyfileobj", side_effect=edit_preferences):
            self.run_install("--apply-approved")
        self.assertTrue(tomllib.loads(self.config.read_text(encoding="utf-8"))["concurrent_preferences"]["keep"])

    def test_invalid_database_never_changes_config(self):
        self.database.write_bytes(b"fictional invalid SQLite")
        with self.assertRaises(sqlite3.DatabaseError):
            self.run_install("--apply-approved")
        self.assertEqual(self.binding(), self.original)

    def test_tampered_snapshot_and_original_binding_block_resume(self):
        self.run_install("--apply-approved")
        snapshot = Path(self.receipt()["backupPath"])
        snapshot.write_bytes(b"fictional tampered snapshot")
        with self.assertRaises(ValueError):
            self.run_install("--apply-approved")
        original_path = self.backup / "codex-memory-before.toml"
        original_path.write_text(INSTALL.fragment({"command": "fictional-other.exe", "env": {}}), encoding="utf-8")
        with self.assertRaises(ValueError):
            self.run_install("--rollback-config-approved")
        self.assertEqual(self.binding(), self.desired)

    def test_plan_artifact_profile_credentials_and_binding_are_checked(self):
        cases = [
            (lambda: self.plan["artifacts"]["elegy-memory.exe"].update(sha256="0" * 64)),
            (lambda: self.plan.update(tunnelProfileSha256="0" * 64)),
            (lambda: self.desired["env"].update(CONTROL_PLANE_API_KEY="fictional-value")),
            (lambda: self.desired["env"].update(ELEGY_EMBEDDING_MODEL="sk-fictionalcredential")),
            (lambda: self.desired.update(command=str(self.root / "wrong-binding.exe"))),
        ]
        original_plan = json.dumps(self.plan)
        for mutate in cases:
            with self.subTest(case=cases.index(mutate)):
                self.plan = json.loads(original_plan)
                self.desired = self.plan["codexMemoryEntry"]
                mutate()
                with self.assertRaises(ValueError):
                    self.run_install("--apply-approved")
                self.assertFalse(self.backup.exists())

    def test_unreviewed_plan_digest_is_rejected(self):
        self.plan_file.write_text(json.dumps(self.plan), encoding="utf-8")
        args = ["installer", "--plan", str(self.plan_file), "--plan-sha256", "0" * 64]
        with mock.patch.object(sys, "argv", args), self.assertRaises(ValueError):
            INSTALL.main()
        self.assertFalse(self.backup.exists())

    @unittest.skipUnless(os.name == "nt", "Windows junction fixture")
    def test_reparse_parent_is_rejected(self):
        actual = self.root / "actual"
        actual.mkdir()
        junction = self.root / "junction"
        result = subprocess.run(["cmd.exe", "/c", "mklink", "/J", str(junction), str(actual)], capture_output=True, timeout=10)
        if result.returncode != 0 and b"Access is denied" in result.stderr + result.stdout:
            self.skipTest("Execution environment disallows creating a junction; detection is covered separately")
        self.assertEqual(result.returncode, 0, "fictional junction fixture could not be created")
        self.addCleanup(lambda: junction.rmdir() if junction.exists() else None)
        with self.assertRaises(ValueError):
            INSTALL.safe_path(str(junction / "missing-file"))

    def test_detected_junction_parent_is_rejected(self):
        with mock.patch.object(Path, "is_junction", return_value=True), self.assertRaises(ValueError):
            INSTALL.safe_path(str(self.root / "fictional-file"))

    def test_atomic_text_flushes_file_before_replace(self):
        path = self.root / "fictional-atomic.txt"
        path.write_text("original", encoding="utf-8")
        with mock.patch.object(INSTALL.os, "fsync", side_effect=OSError("fictional flush failure")) as sync:
            with self.assertRaises(OSError):
                INSTALL.atomic_text(path, "replacement")
        sync.assert_called_once()
        self.assertEqual(path.read_text(encoding="utf-8"), "original")
        self.assertFalse(list(self.root.glob("*.tmp")))


if __name__ == "__main__":
    unittest.main()
