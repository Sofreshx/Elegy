import importlib.util
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).parent
SPEC = importlib.util.spec_from_file_location("run_probe", ROOT / "run_probe.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class SourceInvariantTests(unittest.TestCase):
    def test_changed_main_database_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            db = Path(directory) / "memory.sqlite"
            db.write_bytes(b"main-a")
            before = MODULE.source_snapshot(db)
            db.write_bytes(b"main-b")
            after = MODULE.source_snapshot(db)
            self.assertFalse(MODULE.source_data_unchanged(before, after))

    def test_nonempty_changed_wal_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            db = Path(directory) / "memory.sqlite"
            wal = Path(str(db) + "-wal")
            db.write_bytes(b"main")
            wal.write_bytes(b"wal-a")
            before = MODULE.source_snapshot(db)
            wal.write_bytes(b"wal-b")
            after = MODULE.source_snapshot(db)
            self.assertFalse(MODULE.source_data_unchanged(before, after))

    def test_absent_to_empty_wal_and_new_shm_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            db = Path(directory) / "memory.sqlite"
            wal = Path(str(db) + "-wal")
            shm = Path(str(db) + "-shm")
            db.write_bytes(b"main")
            before = MODULE.source_snapshot(db)
            wal.write_bytes(b"")
            shm.write_bytes(b"administrative")
            after = MODULE.source_snapshot(db)
            self.assertTrue(MODULE.source_data_unchanged(before, after))
            self.assertTrue(after["shm"]["present"])
            self.assertNotEqual(before, after)


if __name__ == "__main__":
    unittest.main()
