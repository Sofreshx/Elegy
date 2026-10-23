"""Regression tests for evidence acceptance, not a substitute for live replay."""
import copy
from pathlib import Path
import tempfile
import unittest
from run_replay import invariant, require, validate_protocol
from rpc_support import source_snapshot

SCRATCH = Path(__file__).parent / 'runtime'
SCRATCH.mkdir(exist_ok=True)

class EvidenceSafetyTests(unittest.TestCase):
    def test_budget_guard_rejects_unsupported_limit_before_writes(self):
        validate_protocol({'memory': {'maxContextTokens': 600, 'scopes': ['Agent']}})
        with self.assertRaises(AssertionError):
            validate_protocol({'memory': {'maxContextTokens': 1800, 'scopes': ['Agent']}})

    def test_wrong_namespace_is_rejected(self):
        with self.assertRaises(AssertionError):
            require({'namespace': 'qual-b'}, 'qual-a')

    def test_missing_namespace_is_rejected(self):
        with self.assertRaises(AssertionError):
            require({'memory': {'id': 'irrelevant'}}, 'qual-a')

    def test_main_mutation_fails(self):
        with tempfile.TemporaryDirectory(dir=SCRATCH) as tmp:
            db = Path(tmp) / 'source.sqlite'
            db.write_bytes(b'first')
            before = source_snapshot(db)
            db.write_bytes(b'second')
            self.assertFalse(invariant(before, source_snapshot(db)))

    def test_nonempty_wal_mutation_fails(self):
        with tempfile.TemporaryDirectory(dir=SCRATCH) as tmp:
            db = Path(tmp) / 'source.sqlite'
            db.write_bytes(b'unchanged')
            before = source_snapshot(db)
            Path(str(db) + '-wal').write_bytes(b'uncommitted durable data')
            self.assertFalse(invariant(before, source_snapshot(db)))

    def test_administrative_sidecars_do_not_fake_source_mutation(self):
        with tempfile.TemporaryDirectory(dir=SCRATCH) as tmp:
            db = Path(tmp) / 'source.sqlite'
            db.write_bytes(b'unchanged')
            before = source_snapshot(db)
            Path(str(db) + '-wal').write_bytes(b'')
            Path(str(db) + '-shm').write_bytes(b'administration')
            after = source_snapshot(db)
            self.assertNotEqual(before, after)
            self.assertTrue(invariant(before, after))

    def test_absent_main_is_not_evidence_of_unchanged_source(self):
        empty = {'main': {'present': False}, 'durableData': {'mainSha256': None, 'walSha256': None}}
        with self.assertRaises(AssertionError):
            invariant(empty, copy.deepcopy(empty))


if __name__ == '__main__':
    unittest.main()
