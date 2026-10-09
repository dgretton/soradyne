"""Exercise the live/restarted distinction missed by the original 0014 fixture."""

import unittest

from crash_boundaries import Store as PrimitiveStore
from crash_boundaries import complete, fixtures, step
from crash_serving_barrier import Store, crash_matrix


class ServingBarrierTests(unittest.TestCase):
    def test_complete_crash_matrix_with_distinct_live_and_restart_oracles(self):
        rows = crash_matrix()
        self.assertEqual(len(rows), 46)
        self.assertEqual(sum(r["live"]["status"] == "unavailable" for r in rows), 20)
        self.assertEqual(
            sum(r["recovered"]["status"] == "unavailable" for r in rows), 18
        )

    def test_original_live_control_gap_is_preserved_as_a_negative_control(self):
        for case in ("grant", "epoch"):
            old, new = fixtures()[case]
            original, checked = PrimitiveStore(old), Store(old)
            for store in (original, checked):
                step(store, new, "stage_gate")
            self.assertEqual(original.read()["status"], "available")
            self.assertEqual(checked.read()["status"], "unavailable")

    def test_unacknowledged_volatile_control_requires_retry_after_restart(self):
        old, new = fixtures()["grant"]
        store = Store(old)
        step(store, new, "stage_gate")
        self.assertEqual(store.read()["status"], "unavailable")
        store.restart()
        self.assertEqual(store.acks, [])
        self.assertEqual(store.read()["inputs"], ("a", "b"))
        # The caller's retained request, not volatile worker state, supplies the retry.
        complete(store, new)
        self.assertEqual(store.read()["inputs"], ("a",))

    def test_pending_same_control_on_retry_does_not_hide_valid_result(self):
        old, new = fixtures()["grant"]
        store = Store(old)
        complete(store, new)
        step(store, new, "stage_gate")
        self.assertEqual(store.read()["status"], "available")


if __name__ == "__main__":
    unittest.main()
