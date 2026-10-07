"""Independent exact references, representation failures and bounded lifecycle checks."""

import unittest
from dataclasses import replace
from fractions import Fraction as Q

from baseline import Information2, ScalarGaussian
from shared_information import (
    BOUNDARY,
    LAYOUTS,
    MODES,
    VALUES,
    Block,
    Packet,
    Prior,
    combine_correlated_pair,
    evaluate,
    latest,
    make_packets,
    oracle,
    run_grid,
    summarize,
    witnesses,
)


class SharedInformationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rows = run_grid()

    def setUp(self):
        self.prior = Prior(Q(0), Q(4), 1)
        self.allowed = frozenset(VALUES)

    def packets(
        self, layout="diamond", representation="atomic", values=None, revision=1
    ):
        return make_packets(
            LAYOUTS[layout],
            VALUES if values is None else values,
            Q(1),
            self.prior,
            revision,
            representation,
        )

    def answer(self, mode, packets, allowed=None, prior=None, **kwargs):
        allowed = self.allowed if allowed is None else frozenset(allowed)
        return evaluate(
            mode,
            packets,
            allowed,
            allowed,
            self.prior if prior is None else prior,
            **kwargs,
        )

    def test_atomic_matches_full_independent_reference_at_all_120_states(self):
        rows = [r for r in self.rows if r["mode"] == "atomic_joint"]
        self.assertEqual(len(rows), 120)
        for row in rows:
            with self.subTest(
                layout=row["layout"],
                state=row["state"],
                sigma2=row["sigma2"],
                tau2=row["initial_tau2"],
            ):
                self.assertEqual(row["result"].answer, row["reference"])
                self.assertEqual(row["result"].represented, row["allowed"])
                self.assertFalse(row["result"].forbidden)

    def test_compressed_is_exact_or_explicitly_unavailable(self):
        for row in self.rows:
            if row["mode"] != "compressed_joint":
                continue
            available = not (
                (row["layout"] == "diamond" and row["state"] < 4)
                or (row["layout"] == "unequal_disjoint" and row["state"] == 3)
            )
            self.assertEqual(row["result"].answer is not None, available)
            if available:
                self.assertEqual(row["result"].answer, row["reference"])
            self.assertFalse(row["authorization_violation"])

    def test_raw_rebuild_requires_archive_and_matches_reference(self):
        for row in self.rows:
            if row["mode"] == "raw_rebuild":
                if row["state"] == 3:
                    self.assertIsNone(row["result"].answer)
                    self.assertEqual(row["result"].reason, "archive_unavailable")
                else:
                    self.assertEqual(row["result"].answer, row["reference"])

    def test_scalar_disjoint_ids_still_false_precision(self):
        for row in self.rows:
            if row["mode"] == "scalar_ids_only" and row["result"].answer:
                self.assertTrue(row["numerically_wrong"])
                self.assertLess(row["errors"]["variance"], 0)
                self.assertFalse(row["authorization_violation"])
        result = self.answer(
            "scalar_ids_only", self.packets("unequal_disjoint", "scalar")
        )
        self.assertEqual(result.answer.mean, Q(82, 19))
        self.assertEqual(result.answer.variance, Q(45, 19))
        self.assertEqual(oracle(VALUES, Q(1), self.prior).mean, Q(10, 3))

    def test_repeated_prior_keeps_mean_but_corrupts_joint_uncertainty(self):
        for row in self.rows:
            if row["mode"] == "repeat_calibration_prior":
                self.assertEqual(row["errors"]["mean"], 0)
                self.assertLess(row["errors"]["variance"], 0)
                self.assertEqual(row["errors"]["sum_variance"], 0)
                self.assertTrue(row["numerically_wrong"])
                self.assertFalse(row["authorization_violation"])

    def test_frozen_control_fails_authorization_at_every_withdrawal(self):
        for row in self.rows:
            if row["mode"] == "keep_frozen_on_withdrawal":
                self.assertEqual(row["authorization_violation"], row["state"] == 3)
                self.assertEqual(row["numerically_wrong"], row["state"] == 3)
                self.assertEqual(row["coverage_violation"], row["state"] == 3)
                if row["state"] == 3:
                    self.assertEqual(row["result"].forbidden, frozenset(("r2",)))
                    self.assertEqual(row["mean_jump_from_previous_state"], 0)

    def test_delivery_reversal_is_exactly_equal(self):
        self.assertEqual(len(self.rows), 720)
        self.assertEqual(self.rows, run_grid(reverse=True))

    def test_revision_republication_does_not_change_estimate(self):
        for row in self.rows:
            if row["state"] == 1 and row["result"].answer is not None:
                self.assertEqual(row["mean_jump_from_previous_state"], 0)
                if row["source_revisions"]:
                    self.assertEqual(set(row["source_revisions"].values()), {2})

    def test_prior_update_changes_mean_without_new_evidence(self):
        for row in self.rows:
            if row["mode"] == "atomic_joint" and row["state"] == 2:
                self.assertEqual(row["mean_jump_from_previous_state"], -2)
                self.assertEqual(row["result"].answer.sum_variance, row["sigma2"] / 3)
                self.assertEqual(set(row["source_revisions"].values()), {2})

    def test_scalar_prior_version_is_checked(self):
        result = self.answer(
            "scalar_ids_only",
            self.packets("singletons", "scalar"),
            prior=Prior(Q(2), Q(1), 2),
        )
        self.assertEqual(result.reason, "stale_calibration")
        self.assertIsNone(result.answer)

    def test_current_empty_packet_prevents_late_resurrection(self):
        old = self.packets("singletons")[1]
        tombstone = Packet(old.child, 3, ())
        for deliveries in ([old, tombstone, old], [tombstone, old]):
            self.assertEqual(latest(deliveries), [tombstone])
        for row in self.rows:
            if row["state"] == 4 and row["mode"] == "atomic_joint":
                self.assertNotIn("r2", row["result"].represented)
                self.assertEqual(set(row["source_revisions"].values()), {3})

    def test_whole_joint_duplicate_is_idempotent_across_packet_ids(self):
        one = self.packets("unequal_disjoint", "compressed")
        duplicate = replace(one[0], child="another-path")
        before = self.answer("compressed_joint", one)
        after = self.answer("compressed_joint", one + [duplicate])
        self.assertEqual(after.answer, before.answer)
        self.assertEqual(after.represented, before.represented)

    def test_partially_overlapping_aggregate_is_rejected_before_fusion(self):
        for mode, representation in (
            ("compressed_joint", "compressed"),
            ("scalar_ids_only", "scalar"),
        ):
            result = self.answer(mode, self.packets(representation=representation))
            self.assertEqual(result.reason, "unsupported_partial_overlap")
            self.assertIsNone(result.answer)
            self.assertFalse(result.represented)

    def test_atomic_conflict_rejected_independent_of_packet_identity(self):
        packets = self.packets("singletons")
        bad = self.packets("singletons", values=VALUES | {"r1": Q(99)})[0]
        bad = replace(bad, child="other-child")
        with self.assertRaisesRegex(ValueError, "same evidence"):
            self.answer("atomic_joint", packets + [bad])

    def test_packet_revision_conflict_rejected_even_when_superseded(self):
        old = self.packets("singletons")[0]
        conflict = self.packets("singletons", values=VALUES | {"r1": Q(99)})[0]
        newer = replace(old, revision=2)
        for packets in ([newer, old, conflict], [conflict, old, newer]):
            with self.assertRaisesRegex(ValueError, "packet revision"):
                latest(packets)

    def test_boundary_change_rejected_instead_of_reinterpreting_calibration(self):
        packet = self.packets()[0]
        changed = replace(
            packet,
            blocks=(replace(packet.blocks[0], boundary=BOUNDARY + "-new-gauge"),),
        )
        with self.assertRaisesRegex(ValueError, "Boundary/model"):
            self.answer("atomic_joint", [changed])

    def test_no_calibration_prior_gives_no_finite_absolute_answer(self):
        for mode in MODES:
            representation = {
                "compressed_joint": "compressed",
                "scalar_ids_only": "scalar",
            }.get(mode, "atomic")
            packets = (
                []
                if mode == "raw_rebuild"
                else self.packets("singletons", representation)
            )
            result = evaluate(
                mode,
                packets,
                self.allowed,
                self.allowed,
                None,
                archive=VALUES if mode == "raw_rebuild" else None,
            )
            self.assertEqual(result.reason, "unobservable_absolute")
            self.assertIsNone(result.answer)
        conditional = self.packets("singletons")[0].blocks[0].payload
        with self.assertRaises(ValueError):
            conditional.joint()

    def test_invalidated_mixed_block_does_not_supply_missing_lawful_source(self):
        result = self.answer(
            "compressed_joint",
            self.packets("unequal_disjoint", "compressed"),
            allowed=("r1", "r3"),
        )
        self.assertEqual(result.reason, "missing_coverage")
        self.assertEqual(result.missing, frozenset(("r1",)))
        self.assertEqual(result.eligible, frozenset(("r3",)))
        self.assertFalse(result.forbidden)
        self.assertFalse(result.represented)

    def test_no_hidden_raw_assistance_for_compressed_candidate(self):
        packets = self.packets("diamond", "compressed")
        without = self.answer("compressed_joint", packets)
        with_extra = self.answer("compressed_joint", packets, archive=VALUES)
        self.assertEqual(without, with_extra)

    def test_shared_and_independent_biases_have_same_marginals_different_answers(self):
        witness = witnesses()["scalar_dependence"]
        shared = witness["shared_bias_combined"]
        independent = witness["independent_biases_combined"]
        self.assertEqual(shared.mean, independent.mean)
        self.assertEqual(shared.variance, Q(9, 2))
        self.assertEqual(independent.variance, Q(5, 2))
        self.assertEqual(
            tuple(m.variance for m in witness["identical_child_marginals"]), (5, 5)
        )

    def test_same_joint_aggregates_cannot_identify_unique_data_answer(self):
        witness = witnesses()["overlap"]
        a = self.packets("diamond", "compressed", witness["world_a"])
        b = self.packets("diamond", "compressed", witness["world_b"])
        self.assertEqual(a, b)
        self.assertEqual(witness["unique_mean_a"], Q(10, 3))
        self.assertEqual(witness["unique_mean_b"], Q(4))
        self.assertNotEqual(
            oracle(witness["world_a"], Q(1), self.prior),
            oracle(witness["world_b"], Q(1), self.prior),
        )

    def test_covariance_aware_reduced_answer_is_honest_but_loses_information(self):
        witness = witnesses()["overlap"]
        honest = witness["honest_reduced_answer_without_calibration"]
        naive = witness["incorrect_independent_answer_without_calibration"]
        self.assertEqual(honest, ScalarGaussian(Q(3), Q(3, 8)))
        self.assertEqual(naive, ScalarGaussian(Q(3), Q(1, 4)))
        self.assertGreater(
            honest.variance, witness["full_unique_variance_without_calibration"]
        )
        self.assertLess(
            naive.variance, witness["full_unique_variance_without_calibration"]
        )

    def test_direct_covariance_reference_with_unequal_redundant_information(self):
        # y2 = y1 + independent noise: knowing y2 adds nothing once y1 is known.
        result = combine_correlated_pair((Q(2), Q(9)), (Q(1), Q(1), Q(4)))
        self.assertEqual(result, ScalarGaussian(Q(2), Q(1)))
        with self.assertRaises(ValueError):
            combine_correlated_pair((Q(2), Q(9)), (Q(1), Q(1), Q(1)))

    def test_duplicate_child_route_cannot_supply_another_calibration_prior(self):
        packets = self.packets("singletons")
        extra = replace(packets[0], child="same-evidence-new-route")
        ordinary = self.answer("atomic_joint", packets)
        replayed = self.answer("atomic_joint", packets + [extra])
        self.assertEqual(ordinary.answer, replayed.answer)
        bad = self.answer("repeat_calibration_prior", packets + [extra])
        self.assertEqual(bad.answer.mean, ordinary.answer.mean)
        self.assertEqual(bad.answer.variance, Q(4, 3))
        self.assertEqual(ordinary.answer.variance, Q(13, 3))

    def test_storage_accounting_includes_lineage_and_duplicate_blocks(self):
        atomic = self.answer("atomic_joint", self.packets())
        compressed = self.answer(
            "compressed_joint", self.packets(representation="compressed")
        )
        scalar = self.answer("scalar_ids_only", self.packets(representation="scalar"))
        self.assertEqual(
            (
                atomic.numeric_coefficients,
                atomic.lineage_entries,
                atomic.retained_blocks,
            ),
            (20, 4, 4),
        )
        self.assertEqual(
            (
                compressed.numeric_coefficients,
                compressed.lineage_entries,
                compressed.retained_blocks,
            ),
            (10, 4, 2),
        )
        self.assertEqual(
            (
                scalar.numeric_coefficients,
                scalar.lineage_entries,
                scalar.retained_blocks,
            ),
            (4, 4, 2),
        )

    def test_mutable_inputs_are_copied_before_being_retained(self):
        sources = {"r1"}
        payload = Information2(Q(1), Q(1), Q(1), Q(0), Q(0))
        block = Block(sources, payload)
        blocks = [block]
        packet = Packet("child", 1, blocks)
        sources.add("forbidden")
        blocks.clear()
        self.assertEqual(packet.blocks, (block,))
        self.assertEqual(block.sources, frozenset(("r1",)))

    def test_invalid_and_oversized_inputs_are_rejected(self):
        packet = self.packets()[0]
        for revision in (0, -1, True, Q(1)):
            with self.assertRaises(ValueError):
                Packet("child", revision, ())
        with self.assertRaises(ValueError):
            Packet("child", 1, packet.blocks * 9)
        with self.assertRaises(ValueError):
            latest([packet] * 33)
        with self.assertRaises(ValueError):
            Block(frozenset(str(i) for i in range(17)), packet.blocks[0].payload)
        with self.assertRaises(ValueError):
            self.answer("atomic_joint", self.packets(representation="compressed"))
        with self.assertRaises(ValueError):
            self.answer("atomic_joint", self.packets(representation="scalar"))
        with self.assertRaises(ValueError):
            self.answer("raw_rebuild", [], sigma2=Q(0))
        with self.assertRaises(ValueError):
            Prior(Q(0), Q(0), 1)
        with self.assertRaises(ValueError):
            latest(
                [
                    Packet(
                        str(i),
                        1,
                        (Block(frozenset((str(i),)), packet.blocks[0].payload),),
                    )
                    for i in range(17)
                ]
            )
        with self.assertRaises(ValueError):
            evaluate("atomic_joint", [], {"r1", "r2"}, {"r1"}, self.prior)
        with self.assertRaises(TypeError):
            Block(frozenset(("r1",)), "not a Gaussian factor")

    def test_all_negative_controls_detected_without_penalizing_unavailability(self):
        metrics = summarize(self.rows)
        self.assertEqual(metrics["atomic_joint"]["available"], 120)
        self.assertEqual(metrics["compressed_joint"]["available"], 80)
        self.assertEqual(metrics["raw_rebuild"]["available"], 96)
        self.assertEqual(metrics["scalar_ids_only"]["available"], 56)
        self.assertEqual(metrics["scalar_ids_only"]["numerically_wrong"], 56)
        self.assertEqual(metrics["repeat_calibration_prior"]["numerically_wrong"], 120)
        self.assertEqual(
            metrics["keep_frozen_on_withdrawal"]["authorization_violations"], 24
        )
        for mode in ("atomic_joint", "compressed_joint", "raw_rebuild"):
            self.assertEqual(metrics[mode]["numerically_wrong"], 0)
            self.assertEqual(metrics[mode]["authorization_violations"], 0)


if __name__ == "__main__":
    unittest.main()
