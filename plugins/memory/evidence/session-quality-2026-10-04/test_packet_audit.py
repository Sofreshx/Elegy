import unittest

from audit_packets import empty_sha, substantive_snapshot


SHA_A = "a" * 64
SHA_B = "b" * 64


class SubstantiveSnapshotTests(unittest.TestCase):
    def test_flat_changed_main_and_nonempty_wal_rejects(self):
        result = substantive_snapshot(
            {
                "sourceHashesBefore": {"probe.sqlite": SHA_A},
                "sourceHashesAfter": {"probe.sqlite": SHA_B, "probe.sqlite-wal": SHA_B},
            }
        )
        self.assertFalse(result["substantiveUnchanged"])
        self.assertTrue(result["valid"])

    def test_flat_unchanged_main_with_nonempty_wal_rejects(self):
        result = substantive_snapshot(
            {
                "sourceHashesBefore": {"probe.sqlite": SHA_A},
                "sourceHashesAfter": {"probe.sqlite": SHA_A, "probe.sqlite-wal": SHA_B},
            }
        )
        self.assertFalse(result["substantiveUnchanged"])

    def test_nested_main_or_wal_change_rejects(self):
        result = substantive_snapshot(
            {
                "sqliteSnapshot": {
                    "before": {"mainSha256": SHA_A, "walSha256": SHA_A},
                    "after": {"mainSha256": SHA_A, "walSha256": SHA_B},
                }
            }
        )
        self.assertFalse(result["substantiveUnchanged"])
        self.assertTrue(result["walCheckedSubstantively"])

    def test_absent_or_empty_administrative_sidecars_accept(self):
        result = substantive_snapshot(
            {
                "sourceHashesBefore": {"probe.sqlite": SHA_A},
                "sourceHashesAfter": {
                    "probe.sqlite": SHA_A,
                    "probe.sqlite-wal": empty_sha(),
                    "probe.sqlite-shm": SHA_B,
                },
            }
        )
        self.assertTrue(result["substantiveUnchanged"])
        self.assertTrue(result["sidecarPhysicalDifference"])

    def test_missing_snapshots_reject(self):
        result = substantive_snapshot({})
        self.assertFalse(result["valid"])
        self.assertFalse(result["substantiveUnchanged"])

    def test_recorded_durable_data_shape_rejects_main_or_wal_change(self):
        before = {"durableData": {"mainSha256": SHA_A, "walSha256": None}}
        for main, wal in ((SHA_B, None), (SHA_A, SHA_B)):
            with self.subTest(main=main, wal=wal):
                result = substantive_snapshot({
                    "sourceHashesBefore": before,
                    "sourceHashesAfter": {"durableData": {"mainSha256": main, "walSha256": wal}},
                })
                self.assertTrue(result["valid"])
                self.assertFalse(result["substantiveUnchanged"])


if __name__ == "__main__":
    unittest.main()
