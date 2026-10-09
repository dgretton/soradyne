"""Exact contract and independent physical-quantity checks for cycle 0013."""

import unittest
from dataclasses import FrozenInstanceError, replace
from fractions import Fraction as Q

from snapshot_epochs import (
    Issuer,
    Policy,
    compose,
    epoch_at,
    event_contract,
    evidence,
    fixtures,
    full_policy,
    naive_compose,
    reexpress_point,
    reset_evidence,
    rotate,
    rotate_covariance,
    squared_distance,
)


class SnapshotEpochTests(unittest.TestCase):
    def test_coherent_historical_answer_cannot_satisfy_explicit_new_snapshot(self):
        (old, old_cov), (new, new_cov) = fixtures()
        with self.assertRaisesRegex(ValueError, "Requested snapshot"):
            compose(old, old_cov, full_policy(), required_refs=new_cov.refs)
        self.assertEqual(
            compose(new, new_cov, full_policy(), required_refs=new_cov.refs).mean, Q(9)
        )

    def test_exact_coherent_chain_cancels_shared_anchor(self):
        for edges, covariance in fixtures():
            answer = compose(edges, covariance, full_policy())
            # Independent original-variable oracle: (10+k)+(-3-k+e)+(2+f).
            self.assertEqual(answer.mean, Q(9))
            self.assertEqual(answer.variance, Q(1) + Q(1, 4))
            self.assertEqual(len(answer.actual_inputs), 3)
            self.assertEqual(
                [e.revision for e in edges],
                [edges[0].revision, 10 + edges[0].revision, 7],
            )

    def test_all_eight_combinations_only_two_coherent(self):
        combinations = evidence()["combinations"]
        accepted = [
            c["selection"] for c in combinations if c["checked"]["status"] == "accepted"
        ]
        self.assertEqual(accepted, [(1, 1, 1), (2, 2, 2)])
        self.assertEqual(len(combinations), 8)

    def test_mixed_revision_has_false_correction_despite_matching_epochs(self):
        (old, cov), (new, _) = fixtures()
        mixed = (new[0], old[1], old[2])
        self.assertEqual(new[0].source, old[1].destination)
        self.assertEqual(
            naive_compose(mixed, cov), {"mean": Q(109), "variance": Q(25, 4)}
        )
        with self.assertRaises(ValueError):
            compose(mixed, cov, full_policy())

    def test_covariance_revision_is_part_of_answer_even_when_mean_same(self):
        (old, _), (_, new_cov) = fixtures()
        bad = naive_compose(old, new_cov)
        self.assertEqual(bad["mean"], Q(9))
        self.assertEqual(bad["variance"], Q(-35, 4))
        with self.assertRaises(ValueError):
            compose(old, new_cov, full_policy())

    def test_dependency_and_calibration_guards_are_separate(self):
        (old, old_cov), (new, _) = fixtures()
        mixed = (new[0], old[1], old[2])
        cov = replace(old_cov, refs=tuple(e.ref for e in mixed))
        with self.assertRaisesRegex(ValueError, "Dependency"):
            compose(mixed, cov, full_policy())
        middle = replace(old[1], dependencies=(new[0].ref,))
        with self.assertRaisesRegex(ValueError, "calibration"):
            compose((new[0], middle, old[2]), cov, full_policy())

    def test_arrival_orders_do_not_create_partial_new_answers(self):
        orders = evidence()["delivery_orders"]
        self.assertEqual(len(orders), 6)
        for order in orders:
            self.assertEqual(
                [p["requested_new"]["status"] for p in order["prefixes"]],
                ["unavailable", "unavailable", "accepted"],
            )
            self.assertTrue(
                all(
                    p["historical_old_explicit"]["status"] == "accepted"
                    for p in order["prefixes"]
                )
            )

    def test_coherence_does_not_override_current_permission(self):
        (old, old_cov), (new, new_cov) = fixtures()
        policy = Policy(
            "permission@2", full_policy().allowed_inputs - {"calibration-evidence@1"}
        )
        with self.assertRaisesRegex(ValueError, "permission"):
            compose(old, old_cov, policy)
        self.assertEqual(compose(new, new_cov, policy).mean, Q(9))
        answer = compose(old, old_cov, full_policy("permission@2"))
        self.assertEqual(answer.policy_revision, "permission@2")

    def test_frame_epoch_and_representation_are_not_optional(self):
        edges, cov = fixtures()[0]
        with self.assertRaisesRegex(ValueError, "epoch"):
            compose(
                (edges[0], replace(edges[1], destination="P:e1"), edges[2]),
                cov,
                full_policy(),
            )
        with self.assertRaisesRegex(ValueError, "Representation"):
            compose(
                edges, replace(cov, representation="different-units"), full_policy()
            )
        with self.assertRaisesRegex(ValueError, "Representation"):
            compose(
                (replace(edges[0], representation="different-chart"), *edges[1:]),
                cov,
                full_policy(),
            )

    def test_query_interval_intersection_is_half_open(self):
        edges, cov = fixtures()[0]
        shortened = (edges[0], edges[1], replace(edges[2], interval=(0, 10)))
        self.assertEqual(
            compose(shortened, cov, full_policy(), query_time=9).mean, Q(9)
        )
        with self.assertRaisesRegex(ValueError, "applicability"):
            compose(shortened, cov, full_policy(), query_time=10)

    def test_joint_and_marginal_payloads_must_agree(self):
        edges, cov = fixtures()[0]
        with self.assertRaisesRegex(ValueError, "uncertainty mismatch"):
            compose((replace(edges[0], variance=Q(9)), *edges[1:]), cov, full_policy())

    def test_equal_numbers_different_manifests_have_distinct_opaque_bindings(self):
        issuer = Issuer()
        answers = [compose(*world, full_policy()) for world in fixtures()]
        ids = [issuer.issue(a) for a in answers]
        self.assertNotEqual(ids[0], ids[1])
        self.assertEqual(issuer.issue(answers[0]), ids[0])
        self.assertEqual(issuer.public(ids[0])["mean"], issuer.public(ids[1])["mean"])
        self.assertEqual(
            issuer.public(ids[0])["variance"], issuer.public(ids[1])["variance"]
        )
        public_keys = set(issuer.public(ids[0]))
        self.assertTrue(
            {"query_time", "source", "destination", "revision"} <= public_keys
        )
        self.assertTrue(
            {
                "actual_inputs",
                "edges",
                "calibration",
                "policy_revision",
                "uncertainty",
            }.isdisjoint(public_keys)
        )
        with self.assertRaises(FrozenInstanceError):
            answers[0].query_time = 99
        with self.assertRaises(FrozenInstanceError):
            answers[0].edges[0].revision = 99

    def test_issuer_has_explicit_bound(self):
        issuer = Issuer()
        answer = compose(*fixtures()[0], full_policy())
        for time in range(4):
            issuer.issue(replace(answer, query_time=time))
        with self.assertRaises(ValueError):
            issuer.issue(replace(answer, query_time=5))
        with self.assertRaises(ValueError):
            issuer.public("not-an-issued-result")

    def test_known_reset_preserves_geometry_and_transforms_vectors(self):
        self.assertEqual(reexpress_point((1, 2)), (Q(98), Q(-19)))
        self.assertEqual(reexpress_point((4, 6)), (Q(94), Q(-16)))
        self.assertEqual(rotate((2, -1)), (Q(1), Q(2)))
        reset = reset_evidence()["known_reexpression"]
        self.assertEqual(reset["target_distance_squared_before"], Q(25))
        self.assertEqual(reset["target_distance_squared_after"], Q(25))
        self.assertEqual(reset["obstacle_distance_squared_before"], Q(82))
        self.assertEqual(reset["obstacle_distance_squared_after"], Q(82))
        self.assertNotEqual(
            reset["bad_only_point_reexpressed_target_distance_squared"], Q(25)
        )
        self.assertEqual(reset["bad_difference_per_publication_tick"], (Q(97), Q(-21)))
        self.assertNotEqual(
            reset["bad_difference_per_publication_tick"], reset["new"]["velocity"]
        )

    def test_covariance_axes_and_cross_sign_change_with_known_rotation(self):
        # x_new=-y_old, y_new=x_old: variances swap; covariance changes sign.
        self.assertEqual(
            rotate_covariance(((Q(1), Q(1, 2)), (Q(1, 2), Q(4)))),
            ((Q(4), Q(-1, 2)), (Q(-1, 2), Q(1))),
        )
        self.assertEqual(
            reset_evidence()["known_reexpression"]["new"]["covariance"],
            ((Q(4), Q(0)), (Q(0), Q(1))),
        )

    def test_same_point_jump_requires_different_event_interpretations(self):
        reset = reset_evidence()["same_displayed_jump"]
        self.assertEqual(reset["point_after"] - reset["point_before"], 100)
        self.assertEqual(reset["relative_after_coordinate_shift"], Q(9))
        self.assertEqual(reset["relative_after_physical_bump"], Q(-91))
        self.assertNotEqual(
            reset["relative_after_coordinate_shift"],
            reset["relative_after_physical_bump"],
        )

    def test_coordinate_and_motion_epochs_have_separate_meanings(self):
        chart, bump = event_contract("reexpression"), event_contract("physical_bump")
        self.assertEqual(chart["motion_epoch_before"], chart["motion_epoch_after"])
        self.assertNotEqual(
            chart["coordinate_epoch_before"], chart["coordinate_epoch_after"]
        )
        self.assertEqual(
            bump["coordinate_epoch_before"], bump["coordinate_epoch_after"]
        )
        self.assertNotEqual(bump["motion_epoch_before"], bump["motion_epoch_after"])
        self.assertEqual(bump["coordinate_relation_status"], "unchanged")

    def test_unknown_reset_has_no_invented_cross_epoch_bridge(self):
        for kind in ("unknown_reset", "ambiguous"):
            event = event_contract(kind)
            self.assertIsNone(event["bridge_new_from_old"])
            self.assertEqual(event["coordinate_relation_status"], "unknown")
            self.assertEqual(event["motion_epoch_after"], "unknown")
        # Supplied new-epoch points can have their own relative answer without a bridge.
        self.assertEqual(squared_distance((1, 2), (4, 6)), Q(25))
        self.assertEqual(
            [epoch_at(t) for t in (9, 10, 19, 20)], ["old", "new", "new", None]
        )

    def test_common_uncertain_bridge_does_not_become_independent_noise(self):
        result = reset_evidence()["uncertain_common_translation"]
        # Original independent errors are ep, et, delta with variances (1,4,9).
        # Relative coefficients et+delta-(ep+delta) are (-1,1,0).
        oracle = sum(c * c * v for c, v in zip((-1, 1, 0), (1, 4, 9), strict=True))
        self.assertEqual(result["relative_variance"], Q(oracle))
        self.assertEqual(result["bad_independent_relation_copies"], Q(23))


if __name__ == "__main__":
    unittest.main()
