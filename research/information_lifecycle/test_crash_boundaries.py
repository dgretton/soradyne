"""Allowed-state, durability-obligation and raw-replay checks for cycle 0014."""

import copy
import unittest
from dataclasses import replace
from fractions import Fraction as Q

from crash_boundaries import (
    Gate,
    Rejected,
    Store,
    complete,
    crash_matrix,
    fixtures,
    identity,
    negative_controls,
    operation,
    step,
    steps,
)


class CrashBoundaryTests(unittest.TestCase):
    def test_every_declared_crash_cut_and_retry(self):
        traces = crash_matrix()
        self.assertEqual(len(traces), 46)
        self.assertEqual(
            sum(t["recovered"]["status"] == "unavailable" for t in traces), 18
        )
        for case, expected_cuts in (
            ("export", 10),
            ("checkpoint", 10),
            ("epoch", 13),
            ("grant", 13),
        ):
            self.assertEqual(
                len([t for t in traces if t["case"] == case]), expected_cuts
            )

    def test_all_negative_controls_falsify_the_claimed_obligation(self):
        rows = negative_controls()
        self.assertEqual(len(rows), 16)
        bad_checkpoint = next(
            r
            for r in rows
            if r["case"] == "checkpoint"
            and r["control"] == "split_numerical_state_and_coverage"
        )
        # Wrong coverage skips b although its numerical information was never loaded.
        self.assertEqual(bad_checkpoint["replay"]["mean"], Q(10))
        self.assertEqual(bad_checkpoint["replay"]["variance"], Q(1, 2))
        self.assertNotEqual(bad_checkpoint["replay"]["variance"], Q(1, 3))

    def test_unacknowledged_publication_can_survive_and_retry_is_idempotent(self):
        old, new = fixtures()["export"]
        store = Store(old)
        for event in steps(new)[:-1]:
            step(store, new, event)
        self.assertEqual(store.acks, [])
        store.restart()
        before = copy.deepcopy(store.durable)
        complete(store, new)
        self.assertEqual(store.durable, before)
        self.assertEqual(store.read()["revision"], new.manifest.revision)

    def test_unacknowledged_control_can_survive_and_must_still_block(self):
        for case in ("epoch", "grant"):
            old, new = fixtures()[case]
            store = Store(old)
            step(store, new, "stage_gate")
            step(store, new, "persist_gate")
            store.restart()
            self.assertEqual(store.acks, [])
            self.assertEqual(store.read()["status"], "unavailable")

    def test_only_staged_control_does_not_count_as_applied(self):
        old, new = fixtures()["grant"]
        store = Store(old)
        step(store, new, "stage_gate")
        with self.assertRaisesRegex(Rejected, "not durable"):
            step(store, new, "ack_control")
        store.restart()
        self.assertEqual(store.durable["gate"], Gate())
        self.assertEqual(store.read()["mean"], Q(5))

    def test_publication_ack_needs_durable_receipt(self):
        old, new = fixtures()["export"]
        store = Store(old)
        for event in steps(new)[:-2]:
            step(store, new, event)
        with self.assertRaisesRegex(Rejected, "not durable"):
            step(store, new, "ack_publication")

    def test_checkpoint_and_replay_match_independent_unique_raw_mean(self):
        old, new = fixtures()["checkpoint"]
        store = Store(old)
        complete(store, new)
        for deliveries in (("b", "c", "b", "c"), ("c", "b"), ("a", "b", "c")):
            answer = store.replay(deliveries)
            raw = (0, 10, 20)
            self.assertEqual(answer["mean"], Q(sum(raw), len(raw)))
            self.assertEqual(answer["variance"], Q(1, len(raw)))
            self.assertEqual(answer["inputs"], ("a", "b", "c"))

    def test_export_does_not_implicitly_allow_checkpoint_use(self):
        old, new = fixtures()["export"]
        store = Store(old)
        complete(store, new)
        with self.assertRaisesRegex(Rejected, "checkpoint contract"):
            store.replay(("c",))

    def test_delayed_committed_retry_does_not_restore_old_head(self):
        old, first = fixtures()["export"]
        store = Store(old)
        complete(store, first)
        second = operation("export-third", ("a", "b", "c"))
        complete(store, second)
        before = copy.deepcopy(store.durable)
        complete(store, first)
        self.assertEqual(store.durable, before)
        self.assertEqual(store.read()["revision"], second.manifest.revision)
        self.assertEqual(len(store.durable["root"]["receipts"]), 2)

    def test_operation_identity_cannot_be_reused_for_changed_content(self):
        old, new = fixtures()["export"]
        store = Store(old)
        complete(store, new)
        impostor = operation(new.name, ("c",))
        with self.assertRaisesRegex(Rejected, "reused"):
            complete(store, impostor)
        self.assertEqual(store.read()["mean"], Q(5))

    def test_durable_policy_rechecked_at_publish(self):
        old, new = fixtures()["export"]
        store = Store(old)
        for event in steps(new)[:-2]:
            step(store, new, event)
        store.stage("gate", Gate(2, "policy@2", ("b",), event="withdraw-b"))
        store.install_gate()
        with self.assertRaisesRegex(Rejected, "authority/epoch"):
            step(store, new, "publish")
        # The older a-only result is still lawful. A gate is not a universal cache purge.
        self.assertEqual(store.read()["inputs"], ("a",))

    def test_harmless_policy_revision_preserves_current_lawful_artifact(self):
        old, _ = fixtures()["grant"]
        store = Store(old)
        store.stage("gate", Gate(2, "policy@2"))
        store.install_gate()
        self.assertEqual(store.read()["mean"], Q(5))

    def test_build_acceptance_and_current_policy_are_distinct(self):
        old, new = fixtures()["export"]
        store = Store(old)
        store.stage("gate", Gate(2, "policy@2"))
        store.install_gate()
        complete(store, new)
        receipt = store.durable["root"]["receipts"][new.name]
        self.assertEqual(new.manifest.build_policy, "policy@1")
        self.assertEqual(receipt["accepted_gate"].policy, "policy@2")
        store.stage("gate", Gate(3, "policy@3"))
        store.install_gate()
        store.restart()
        answer = store.read()
        self.assertEqual(answer["accepted_gate"].policy, "policy@2")
        self.assertEqual(answer["current_gate"].policy, "policy@3")
        self.assertEqual(answer["mean"], Q(5))

    def test_local_gate_revisions_cannot_roll_back_or_conflict(self):
        old, new = fixtures()["grant"]
        store = Store(old)
        store.stage("gate", new.gate)
        store.install_gate()
        for stale in (Gate(), Gate(2, "policy@2")):
            store.stage("gate", stale)
            with self.assertRaisesRegex(Rejected, "stale or conflicting"):
                store.install_gate()
        self.assertEqual(store.read()["status"], "unavailable")

    def test_missing_durable_dependency_prevents_publication(self):
        old, new = fixtures()["export"]
        store = Store(old)
        for event in steps(new)[:-2]:
            step(store, new, event)
        del store.durable["objects"][new.manifest.dependency]
        with self.assertRaisesRegex(Rejected, "missing payload/dependency"):
            step(store, new, "publish")

    def test_incompatible_model_binding_is_rejected(self):
        old, new = fixtures()["export"]
        store = Store(old)
        wrong = replace(
            new, manifest=replace(new.manifest, calibration="different-calibration")
        )
        with self.assertRaisesRegex(Rejected, "incompatible dependency"):
            complete(store, wrong)

    def test_each_semantic_binding_changes_manifest_identity(self):
        _, new = fixtures()["export"]
        m = new.manifest
        variants = (
            replace(m, inputs=("c",)),
            replace(m, epoch="E1"),
            replace(m, coordinate_epoch="chart1"),
            replace(m, build_policy="policy@2"),
            replace(m, interval=(0, 10)),
            replace(m, calibration="other"),
            replace(m, model="other"),
            replace(m, representation="other"),
            replace(m, payload="other"),
            replace(m, dependency="other"),
        )
        self.assertTrue(all(identity(v) != identity(m) for v in variants))

    def test_acknowledged_invalidation_survives_without_replacement(self):
        for case in ("grant", "epoch"):
            old, new = fixtures()[case]
            store = Store(old)
            for event in steps(new)[:3]:
                step(store, new, event)
            store.restart()
            self.assertEqual(store.read()["status"], "unavailable")
            self.assertNotIn(identity(new.manifest), store.durable["objects"])
            self.assertIn(("control", new.gate.event), store.acks)

    def test_immutable_old_result_is_retained_but_not_currently_usable(self):
        old, new = fixtures()["epoch"]
        store = Store(old)
        complete(store, new)
        self.assertIn(identity(old.manifest), store.durable["objects"])
        with self.assertRaisesRegex(Rejected, "authority/epoch"):
            store.validate(identity(old.manifest))
        self.assertEqual(store.read()["epoch"], "E1")


if __name__ == "__main__":
    unittest.main()
