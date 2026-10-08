"""Independent checks and negative controls for the first nonlinear freeze slice."""

import json
import math
import unittest
import warnings

import numpy as np
from nonlinear_freeze import (
    QUERY_POINTS,
    AngularReference,
    Data,
    FrozenQuadratic,
    J,
    MomentSummary,
    fixture,
    grid,
    insufficiency_witness,
    left_covariance,
    point_jacobian,
    raw_batch,
    raw_gradient,
    raw_hessian,
    raw_nll,
    rotation,
    run_scenario,
    transform,
    validate_run,
    wrap,
)
from scipy.integrate import IntegrationWarning


class NonlinearFreezeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        with warnings.catch_warnings():
            warnings.simplefilter("error", IntegrationWarning)
            cls.runs = [run_scenario(*config) for config in grid()]

    def test_predeclared_grid_and_all_numerical_oracles(self):
        self.assertEqual(len(self.runs), 128)
        for metric, trace in self.runs:
            with self.subTest(config=metric["config"]):
                validate_run(metric, trace)

    def test_full_batch_recovers_noiseless_rigid_pose_for_large_rotations(self):
        data = fixture(offset=True)
        for angle in (0, 1.2, -2.9, math.pi):
            truth = np.array([angle, -1.3, 2.7])
            rotated = Data(data.p, transform(truth, data.p), data.sigma, data.ids)
            pose, _ = raw_batch(rotated)
            self.assertAlmostEqual(wrap(pose[0] - angle), 0, places=12)
            np.testing.assert_allclose(pose[1:], truth[1:], atol=1e-12)

    def test_svd_enforces_proper_rotation_on_reflected_data(self):
        p = np.array([[-2, -1], [-2, 1], [2, -1], [2, 1]], dtype=float)
        z = p @ np.diag([1.0, -1.0])
        data = Data(p, z, 0.2, ("a", "b", "c", "d"))
        pose, _ = raw_batch(data)
        self.assertAlmostEqual(np.linalg.det(rotation(pose[0])), 1)
        # A forbidden reflection would have zero SSE; a proper rotation must pay 16.
        self.assertAlmostEqual(raw_nll(data, pose) * 2 * data.sigma**2, 16)
        summary_pose, _ = MomentSummary.freeze(data).solve()
        np.testing.assert_allclose(pose, summary_pose, atol=1e-12)

    def test_noiseless_sine_error_is_present_at_ninety_degrees(self):
        data = fixture()
        pose, covariance = FrozenQuadratic.freeze(
            data, np.array([-math.pi / 2, 0, 0])
        ).solve()
        self.assertAlmostEqual(pose[0], 1 - math.pi / 2)
        self.assertGreater(abs(pose[0]), 0.5)
        self.assertAlmostEqual(covariance[0, 0], data.sigma**2 / 5)
        # Positive, narrow covariance does not make this estimate accurate.
        angular = AngularReference(data, raw_batch(data)[0])
        mass, _ = angular.interval_mass(pose[0], covariance[0, 0])
        self.assertLess(mass, 1e-50)

    def test_quadratic_recenter_changes_coordinates_not_likelihood_or_answer(self):
        data = fixture(offset=True, noise="pattern")
        frozen = FrozenQuadratic.freeze(data, np.array([-1.2, 0.0, 0.0]))
        shifted = frozen.recenter(np.array([0.7, -2.0, 3.0]))
        for pose in (np.array([0.1, 0.3, -0.2]), np.array([-2.1, -1.0, 2.0])):
            self.assertAlmostEqual(frozen.nll(pose), shifted.nll(pose), places=7)
        np.testing.assert_allclose(frozen.solve()[0], shifted.solve()[0], atol=1e-11)
        np.testing.assert_allclose(frozen.solve()[1], shifted.solve()[1], atol=1e-11)

    def test_moment_likelihood_is_periodic_frozen_quadratic_is_not(self):
        data = fixture(noise="pattern")
        moment = MomentSummary.freeze(data)
        frozen = FrozenQuadratic.freeze(data, np.zeros(3))
        pose = np.array([0.3, 0.1, -0.4])
        wrapped = pose + np.array([2 * math.pi, 0, 0])
        self.assertAlmostEqual(moment.nll(pose), moment.nll(wrapped), places=10)
        self.assertGreater(abs(frozen.nll(pose) - frozen.nll(wrapped)), 100)

    def test_compact_summary_does_not_retain_raw_arrays(self):
        data = fixture()
        summary = MomentSummary.freeze(data)
        self.assertNotIn("p", vars(summary))
        self.assertNotIn("z", vars(summary))
        self.assertEqual(summary.ids, data.ids)
        self.assertEqual(
            sum(v.size for v in vars(summary).values() if isinstance(v, np.ndarray)), 4
        )

    def test_raw_hessian_matches_finite_difference_away_from_optimum(self):
        data = fixture(offset=True, noise="pattern")
        pose = np.array([0.8, -0.5, 1.4])
        step = 1e-5
        fd = np.column_stack(
            [
                (
                    raw_gradient(data, pose + step * e)
                    - raw_gradient(data, pose - step * e)
                )
                / (2 * step)
                for e in np.eye(3)
            ]
        )
        np.testing.assert_allclose(raw_hessian(data, pose), fd, rtol=1e-7, atol=1e-6)

    def test_left_tangent_conversion_agrees_with_finite_perturbation(self):
        pose = np.array([0.7, 2.0, -1.3])
        point = QUERY_POINTS[1]
        chart_to_left = np.eye(3)
        chart_to_left[1:, 0] = -J @ pose[1:]
        left_to_chart = np.linalg.solve(chart_to_left, np.eye(3))
        step = 1e-6
        numeric = np.column_stack(
            [
                (
                    transform(pose + step * left_to_chart @ e, point)
                    - transform(pose - step * left_to_chart @ e, point)
                )
                / (2 * step)
                for e in np.eye(3)
            ]
        )
        expected = np.column_stack((J @ transform(pose, point), np.eye(2)))
        np.testing.assert_allclose(numeric, expected, atol=1e-8)

    def test_cross_covariance_is_needed_for_point_queries(self):
        data = fixture(offset=True)
        pose, cov = MomentSummary.freeze(data).solve()
        jac = point_jacobian(pose, QUERY_POINTS[0])
        left = left_covariance(pose, cov)
        jl = np.column_stack((J @ transform(pose, QUERY_POINTS[0]), np.eye(2)))
        np.testing.assert_allclose(jac @ cov @ jac.T, jl @ left @ jl.T, atol=1e-12)
        self.assertGreater(
            np.linalg.norm(jl @ np.diag(np.diag(left)) @ jl.T - jac @ cov @ jac.T), 1e-4
        )

    def test_angular_interval_wrap_and_large_interval(self):
        data = fixture(scale=0.1, sigma=0.2)
        truth = np.array([math.pi - 0.01, 0.3, -0.2])
        data = Data(data.p, transform(truth, data.p), data.sigma, data.ids)
        pose, covariance = raw_batch(data)
        angular = AngularReference(data, pose)
        mass, _ = angular.interval_mass(pose[0], covariance[0, 0])
        same, _ = angular.interval_mass(pose[0] - 2 * math.pi, covariance[0, 0])
        self.assertAlmostEqual(mass, same, places=12)
        a, _ = angular.interval_mass(pose[0] + math.pi - 0.1, 0.04)
        b, _ = angular.interval_mass(pose[0] - math.pi + 0.1, 0.04)
        self.assertAlmostEqual(a, b, places=12)
        self.assertEqual(angular.interval_mass(0, 10)[0], 1)

    def test_exact_nonlinear_likelihood_does_not_guarantee_gaussian_mass(self):
        weak = next(
            m
            for m, _ in self.runs
            if m["config"]
            == {
                "scale": 0.1,
                "offset": False,
                "sigma": 0.2,
                "noise": "zero",
                "delta_deg": 0,
            }
        )
        result = weak["methods"]["nonlinear_moments"]
        self.assertLess(result["angle_error_rad"], 1e-12)
        self.assertLess(result["nominal_95_interval_actual_mass"], 0.94)
        self.assertFalse(result["mass_diagnostic_pass"])

    def test_large_rebuild_jump_without_new_observations(self):
        witness = next(
            m
            for m, _ in self.runs
            if m["config"]
            == {
                "scale": 1.0,
                "offset": False,
                "sigma": 0.05,
                "noise": "zero",
                "delta_deg": 90,
            }
        )
        self.assertGreater(
            witness["methods"]["frozen_linear"]["rebuild_max_query_jump_m"], 1
        )
        self.assertLess(
            witness["methods"]["nonlinear_moments"]["rebuild_max_query_jump_m"], 1e-12
        )
        for _, trace in self.runs:
            self.assertEqual(len(trace), 12)
            self.assertTrue(
                all(
                    row["new_observations_since_tick0"] == 0
                    and not row["forbidden_ids"]
                    for row in trace
                )
            )

    def test_coincident_points_are_unobservable_not_regularized(self):
        data = Data(np.ones((4, 2)), np.ones((4, 2)), 0.1, ("a", "b", "c", "d"))
        for solve in (
            lambda: raw_batch(data),
            lambda: MomentSummary.freeze(data).solve(),
            lambda: FrozenQuadratic.freeze(data, np.zeros(3)),
        ):
            with self.assertRaises(ValueError):
                solve()

    def test_invalid_input_and_duplicate_ids_are_rejected(self):
        data = fixture()
        cases = [
            (data.p, data.z, 0, data.ids),
            (data.p, data.z, float("nan"), data.ids),
            (data.p, data.z, 0.1, ("a", "a", "b", "c")),
            (np.zeros((33, 2)), np.zeros((33, 2)), 0.1, tuple(map(str, range(33)))),
            (data.p[:2], data.z, 0.1, data.ids),
            (data.p, np.full((4, 2), float("nan")), 0.1, data.ids),
        ]
        for args in cases:
            with self.subTest(args=args), self.assertRaises(ValueError):
                Data(*args)

    def test_all_run_metrics_and_traces_are_json_serializable(self):
        # Reproduces the first CLI failure: NumPy bool_ leaked from a comparison.
        encoded = json.dumps(self.runs, allow_nan=False)
        self.assertEqual(len(json.loads(encoded)), 128)

    def test_same_frozen_quadratic_can_require_different_uncertainty(self):
        witness = insufficiency_witness(0.5)
        a, b = witness["worlds"]
        np.testing.assert_allclose(a["raw_pose"], b["raw_pose"], atol=1e-12)
        self.assertAlmostEqual(
            b["raw_local_chart_covariance"][0][0]
            / a["raw_local_chart_covariance"][0][0],
            3,
        )
        self.assertEqual(a["frozen_pose"], b["frozen_pose"])
        self.assertEqual(a["frozen_chart_covariance"], b["frozen_chart_covariance"])

    def test_same_frozen_quadratic_can_require_opposite_orientations(self):
        witness = insufficiency_witness(1.5)
        a, b = witness["worlds"]
        self.assertAlmostEqual(abs(wrap(a["raw_pose"][0] - b["raw_pose"][0])), math.pi)
        self.assertEqual(a["frozen_pose"], b["frozen_pose"])


if __name__ == "__main__":
    unittest.main()
