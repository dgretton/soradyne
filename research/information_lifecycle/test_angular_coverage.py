"""Cycle 0011 numerical checks; no held-out seed is sampled by this suite."""

import json
import math
import unittest
import warnings

import numpy as np
from angular_coverage import (
    ANCHORS,
    FAMILY_COMPARISONS,
    METHODS,
    TRIALS,
    arc_mass,
    configurations,
    contains,
    coverage_summary,
    full_arc_half_width,
    generate,
    infer_frozen,
    infer_moments,
    infer_raw_svd,
    run_cell,
)
from nonlinear_freeze import Data, FrozenQuadratic, MomentSummary, rotation, wrap
from scipy.integrate import IntegrationWarning, quad


class AngularCoverageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # This is primary-only numerical smoke evidence, not the final coverage test.
        with warnings.catch_warnings():
            warnings.simplefilter("error", IntegrationWarning)
            cls.smoke = [
                run_cell(configurations()[i], "primary", i, 64) for i in (0, 15)
            ]

    def test_full_arc_matches_independent_quadrature_at_declared_concentrations(self):
        for kappa in (0, 0.001, 0.1, 1, 10, 100, 2000):
            with self.subTest(kappa=kappa):
                h = float(full_arc_half_width(kappa))

                def weight(angle, concentration=kappa):
                    return math.exp(concentration * (math.cos(angle) - 1))

                denominator, _ = quad(
                    weight, 0, math.pi, epsabs=1e-12, epsrel=1e-12, limit=100
                )
                numerator, _ = quad(weight, 0, h, epsabs=1e-12, epsrel=1e-12, limit=100)
                self.assertLess(abs(numerator / denominator - 0.95), 1e-7)

    def test_uniform_arc_and_concentration_monotonicity(self):
        values = full_arc_half_width(np.array([0, 0.1, 1, 10, 100, 2000]))
        self.assertAlmostEqual(values[0], 0.95 * math.pi, places=12)
        self.assertTrue(np.all(np.diff(values) < 0))
        self.assertTrue(np.all(values > 0))

    def test_circular_membership_wraps_both_sides(self):
        self.assertTrue(contains(math.pi - 0.01, 0.03, -math.pi + 0.01))
        self.assertFalse(contains(math.pi - 0.01, 0.005, -math.pi + 0.01))
        self.assertTrue(contains(0.0, math.pi, math.pi))
        self.assertTrue(contains(-4 * math.pi, 0.01, 2 * math.pi))

    def test_arc_mass_wrap_and_full_circle(self):
        self.assertAlmostEqual(
            float(arc_mass(3.0, 0.5, 1.2)),
            float(arc_mass(3.0 - 2 * math.pi, 0.5, 1.2)),
            places=12,
        )
        self.assertAlmostEqual(
            float(arc_mass(math.pi, 0.5, 0.0)), 0.5 / math.pi, places=12
        )
        self.assertEqual(float(arc_mass(2.0, math.pi, 100)), 1)

    def test_exact_binomial_intervals_include_nonzero_risk_at_boundaries(self):
        zero = coverage_summary(np.zeros(100, dtype=bool))
        all_hits = coverage_summary(np.ones(100, dtype=bool))
        self.assertEqual(zero["ci95_exact"][0], 0)
        self.assertAlmostEqual(zero["ci95_exact"][1], 1 - 0.025**0.01, places=12)
        self.assertEqual(all_hits["ci95_exact"][1], 1)
        self.assertLess(all_hits["ci95_exact"][0], 1)
        self.assertLess(all_hits["ci99_family_exact"][0], all_hits["ci95_exact"][0])

    def test_family_classifier_does_not_depend_on_point_interval_alone(self):
        compatible = coverage_summary(np.arange(4096) < 3891)
        under = coverage_summary(np.arange(4096) < 3500)
        over = coverage_summary(np.ones(4096, dtype=bool))
        self.assertEqual(compatible["classification"], "compatible")
        self.assertEqual(under["classification"], "under")
        self.assertEqual(over["classification"], "over")
        self.assertEqual(FAMILY_COMPARISONS, 2 * 16 * len(METHODS))

    def test_generator_is_reproducible_and_cell_streams_differ(self):
        cfg = configurations()[0]
        p, a, truth = generate(cfg, 7, 0, 16)
        _, b, _ = generate(cfg, 7, 0, 16)
        _, c, _ = generate(cfg, 7, 1, 16)
        np.testing.assert_array_equal(a, b)
        self.assertFalse(np.array_equal(a, c))
        self.assertEqual(a.shape, (16, 4, 2))
        np.testing.assert_array_equal(truth, [0.0, 0.3, -0.2])
        self.assertEqual(p.shape, (4, 2))

    def test_batch_reference_and_summary_agree_with_scalar_paths(self):
        cfg = configurations()[15]
        p, z, _ = generate(cfg, 42, 15, 16)
        moment = infer_moments(p, z, cfg["sigma"])
        raw, _ = infer_raw_svd(p, z, cfg["sigma"])
        for i in range(16):
            data = Data(p, z[i], cfg["sigma"], ("a", "b", "c", "d"))
            pose, cov = MomentSummary.freeze(data).solve()
            self.assertLess(abs(wrap(pose[0] - raw[i, 0])), 1e-12)
            np.testing.assert_allclose(pose[1:], raw[i, 1:], atol=1e-12)
            self.assertAlmostEqual(cov[0, 0], moment["observed_variance"][i], places=12)

    def test_frozen_batch_equals_original_quadratic(self):
        cfg = configurations()[6]
        p, z, _ = generate(cfg, 42, 6, 16)
        for angle in ANCHORS.values():
            poses, var = infer_frozen(p, z, cfg["sigma"], angle)
            for i in (0, 7, 15):
                data = Data(p, z[i], cfg["sigma"], ("a", "b", "c", "d"))
                pose, cov = FrozenQuadratic.freeze(
                    data, np.array([angle, 0.0, 0.0])
                ).solve()
                np.testing.assert_allclose(pose, poses[i], atol=1e-11)
                self.assertAlmostEqual(var, cov[0, 0], places=12)

    def test_full_likelihood_rotation_equivariance(self):
        cfg = configurations()[14]
        p, z, _ = generate(cfg, 19, 14, 16)
        original = infer_moments(p, z, cfg["sigma"])
        angle = 1.1
        rotated = infer_moments(p, z @ rotation(angle).T, cfg["sigma"])
        np.testing.assert_allclose(
            wrap(rotated["pose"][:, 0] - original["pose"][:, 0]), angle, atol=1e-12
        )
        np.testing.assert_allclose(rotated["kappa"], original["kappa"], atol=1e-12)
        np.testing.assert_allclose(
            full_arc_half_width(rotated["kappa"]),
            full_arc_half_width(original["kappa"]),
            atol=1e-12,
        )

    def test_frozen_anchor_is_not_rotation_equivariant(self):
        cfg = configurations()[0]
        p, z, _ = generate(cfg, 19, 0, 16)
        a, _ = infer_frozen(p, z, cfg["sigma"], 0)
        b, _ = infer_frozen(p, z @ rotation(1.1).T, cfg["sigma"], 0)
        self.assertGreater(float(np.mean(np.abs(wrap(b[:, 0] - a[:, 0]) - 1.1))), 0.1)

    def test_smoke_has_all_trials_no_forbidden_ids_and_no_new_rebuild_data(self):
        for metric, traces in self.smoke:
            self.assertEqual(metric["discarded_trials"], 0)
            self.assertEqual(set(metric["methods"]), set(METHODS))
            for outcome in metric["methods"].values():
                self.assertEqual(outcome["coverage"]["trials"], 64)
                self.assertEqual(outcome["new_observations_during_rebuild"], 0)
            for trace in traces:
                self.assertEqual(len(trace["events"]), 18)
                for event in trace["events"]:
                    self.assertEqual(event["represented_ids"], event["permitted_ids"])
                    self.assertFalse(event["forbidden_ids"])
                    self.assertEqual(event["observation_count"], 4)

    def test_rebuild_removes_approximation_jump_without_claiming_truth(self):
        for metric, _ in self.smoke:
            self.assertLess(
                metric["methods"]["full_arc"]["rebuild_query_jump_m"]["max"], 1e-10
            )
            self.assertGreater(
                metric["methods"]["full_arc"]["truth_query_error_m"]["max"], 0
            )
            self.assertGreater(
                metric["methods"]["frozen_minus90"]["rebuild_query_jump_m"]["max"], 0.1
            )

    def test_numerical_rank_failure_is_loud_instead_of_dropping_a_trial(self):
        p = np.ones((4, 2))
        z = np.ones((2, 4, 2))
        with self.assertRaises(ValueError):
            infer_moments(p, z, 0.1)
        with self.assertRaises(ValueError):
            infer_frozen(p, z, 0.1, 0)

    def test_invalid_numeric_inputs_and_bounds_are_rejected(self):
        cfg = configurations()[0]
        p, z, _ = generate(cfg, 19, 0, 16)
        for concentration in (-1, float("nan"), float("inf")):
            with self.assertRaises(ValueError):
                full_arc_half_width(concentration)
        for noise in (0, float("nan")):
            with self.assertRaises(ValueError):
                infer_moments(p, z, noise)
        for trials in (0, TRIALS + 1):
            with self.assertRaises(ValueError):
                generate(cfg, 19, 0, trials)

    def test_all_smoke_outputs_are_json_serializable(self):
        self.assertEqual(len(json.loads(json.dumps(self.smoke, allow_nan=False))), 2)


if __name__ == "__main__":
    unittest.main()
