"""Independent closed-form expectations and deliberately wrong controls."""

import unittest
from fractions import Fraction as Q

from baseline import (
    Factor,
    Information2,
    ScalarGaussian,
    SummaryStore,
    fuse_independent,
    withdrawal_fixture,
)


class ExactWitnesses(unittest.TestCase):
    def test_joint_matches_closed_form(self):
        priors, observation = withdrawal_fixture()
        mean, covariance = Information2.from_factors(priors + [observation]).joint()
        # H = [[2,-1],[-1,2]], eta=[10,-10]; inverse = [[2,1],[1,2]]/3.
        self.assertEqual(mean, (Q(10, 3), Q(-10, 3)))
        self.assertEqual(covariance, (Q(2, 3), Q(1, 3), Q(2, 3)))

    def test_freeze_preserves_current_scalar_marginal(self):
        priors, observation = withdrawal_fixture()
        frozen = Information2.from_factors(priors + [observation]).freeze_x()
        # Independent reference: integrating y~N(0,1) gives z|x~N(x,2).
        # Combined with x~N(0,1): precision 3/2, information 5.
        self.assertEqual(frozen, ScalarGaussian(Q(10, 3), Q(2, 3)))

    def test_discarding_correlation_can_err_in_either_direction(self):
        priors, observation = withdrawal_fixture()
        _, (xx, xy, yy) = Information2.from_factors(priors + [observation]).joint()
        self.assertEqual(xx + yy - 2 * xy, Q(2, 3))
        self.assertEqual(xx + yy + 2 * xy, Q(2))
        self.assertEqual(xx + yy, Q(4, 3))
        self.assertGreater(xx + yy, xx + yy - 2 * xy)
        self.assertLess(xx + yy, xx + yy + 2 * xy)

    def test_replay_replaces_evidence_instead_of_multiplying_it(self):
        store = SummaryStore()
        value = ScalarGaussian(Q(2), Q(1))
        for _ in range(10):
            store.accept("boundary", 1, value)
        self.assertEqual(fuse_independent(store.active()), value)
        self.assertEqual(fuse_independent([value] * 10).variance, Q(1, 10))
        newer = ScalarGaussian(Q(3), Q(2))
        store.accept("boundary", 2, newer)
        store.accept("boundary", 1, value)
        self.assertEqual(store.active(), [newer])

    def test_withdrawn_summary_cannot_return_via_delayed_replay(self):
        store = SummaryStore()
        value = ScalarGaussian(Q(2), Q(1))
        store.accept("boundary", 1, value)
        store.accept("boundary", 2, None)
        self.assertFalse(store.accept("boundary", 1, value))
        self.assertEqual(store.active(), [])
        with self.assertRaises(ValueError):
            fuse_independent(store.active())

    def test_revision_collision_is_not_silent_last_writer_wins(self):
        store = SummaryStore()
        store.accept("boundary", 1, ScalarGaussian(Q(0), Q(1)))
        with self.assertRaises(ValueError):
            store.accept("boundary", 1, ScalarGaussian(Q(1), Q(1)))

    def test_rebuild_removes_withdrawn_information(self):
        priors, observation = withdrawal_fixture()
        after = Information2.from_factors(priors).freeze_x()
        self.assertEqual(after, ScalarGaussian(Q(0), Q(1)))
        self.assertNotEqual(
            Information2.from_factors(priors + [observation]).freeze_x(), after
        )

    def test_collapsed_summary_cannot_support_general_selective_withdrawal(self):
        a, observation = withdrawal_fixture(z=Q(0))
        b, same = withdrawal_fixture(z=Q(0), px=Q(4, 3), py=Q(1, 3))
        self.assertEqual(observation, same)
        self.assertEqual(
            Information2.from_factors(a + [observation]).freeze_x(),
            ScalarGaussian(Q(0), Q(2, 3)),
        )
        self.assertEqual(
            Information2.from_factors(b + [same]).freeze_x(),
            ScalarGaussian(Q(0), Q(2, 3)),
        )
        self.assertEqual(
            Information2.from_factors(a).freeze_x(), ScalarGaussian(Q(0), Q(1))
        )
        self.assertEqual(
            Information2.from_factors(b).freeze_x(), ScalarGaussian(Q(0), Q(4, 3))
        )

    def test_no_fake_precision_when_absolute_reference_is_missing(self):
        system = Information2.from_factors([Factor(Q(1), Q(-1), Q(0), Q(1))])
        with self.assertRaises(ValueError):
            system.joint()
        with self.assertRaises(ValueError):
            system.freeze_x()
        # This harness does not yet implement relative-only queries on a singular system.


if __name__ == "__main__":
    unittest.main()
