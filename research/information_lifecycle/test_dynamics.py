"""Independent plant checks and predeclared behavioral/authorization expectations."""

import unittest

from dynamics import CASES, MODES, Consumer, Estimate, advance, first_crossing, simulate


class PlantReferenceTests(unittest.TestCase):
    def test_step_budget_and_invalid_timestep_are_rejected(self):
        for dt in (0, -0.01, float("nan"), 0.0001):
            with self.subTest(dt=dt), self.assertRaises(ValueError):
                simulate(CASES[0], "direct", dt)

    def test_constant_acceleration(self):
        x, v, xmax, moving = advance(1, 2, 3, 0.5)
        self.assertAlmostEqual(x, 2.375)
        self.assertAlmostEqual(v, 3.5)
        self.assertAlmostEqual(xmax, 2.375)
        self.assertEqual(moving, 0.5)

    def test_in_step_braking_matches_stopping_distance(self):
        x, v, xmax, moving = advance(0.1, 0.3, -2, 0.5, brake=True)
        self.assertAlmostEqual(x, 0.1 + 0.3**2 / 4)
        self.assertEqual(v, 0)
        self.assertAlmostEqual(xmax, x)
        self.assertAlmostEqual(moving, 0.15)

    def test_wall_crossing_inside_step_is_not_missed(self):
        # x starts/ends at 0 but peaks at .5; wall .4 is crossed at (1-sqrt(.2))/2.
        x, _, xmax, moving = advance(0, 2, -4, 1)
        self.assertEqual(x, 0)
        self.assertAlmostEqual(xmax, 0.5)
        self.assertAlmostEqual(
            first_crossing(0, 2, -4, moving, 0.4), (1 - 0.2**0.5) / 2
        )

    def test_target_mapping_uses_inverse_offset(self):
        # Immediately after the .4 correction, a global target of 1 is local .6.
        _, trace = simulate(CASES[1], "direct")
        self.assertAlmostEqual(trace[15]["target_local"], 0.6)


class ConsumerContractTests(unittest.TestCase):
    def test_duplicate_and_stale_replay_preserves_every_trajectory(self):
        for case in CASES:
            for mode in MODES:
                with self.subTest(case=case.name, mode=mode):
                    self.assertEqual(
                        simulate(case, mode), simulate(case, mode, replay=True)
                    )

    def test_conflicting_revision_is_rejected(self):
        c = Consumer("direct", 0.1)
        c.receive(Estimate(1, 0, 0, 0.02), 0)
        with self.assertRaises(ValueError):
            c.receive(Estimate(1, 0, 1, 0.02), 0)

    def test_strict_revocation_clears_alignment_and_rejects_late_estimates(self):
        for mode in MODES[:-1]:
            with self.subTest(mode=mode):
                c = Consumer(mode, 0.1)
                c.receive(Estimate(1, 0, 0.4, 0.02), 0)
                c.revoke()
                self.assertIsNone(c.latest)
                self.assertIsNone(c.applied)
                self.assertIsNone(c.pending)
                self.assertFalse(c.receive(Estimate(2, 1, 0.5, 0.02), 1))
                self.assertFalse(c.step(1, 0.01).available)

    def test_transition_lag_is_exposed_as_an_error_bound(self):
        c = Consumer("slew", 0.08)
        c.receive(Estimate(1, 0, 0, 0.02), 0)
        c.receive(Estimate(2, 1, 0.4, 0.02), 0.15)
        out = c.step(0.15, 0.01)
        self.assertAlmostEqual(out.offset, 0.0005)
        self.assertAlmostEqual(out.bound, 0.4195)
        self.assertGreater(out.bound, 0.08)


class ScenarioPredictions(unittest.TestCase):
    def test_coalescing_can_alias_a_storm_and_delay_a_needed_correction(self):
        # Preserve the observed failures as negative controls, not accepted methods.
        storm, _ = simulate(CASES[0], "coalesce")
        corrected, _ = simulate(CASES[1], "coalesce")
        self.assertGreater(storm["late_target_rmse"], 0.05)
        self.assertIsNotNone(corrected["first_crossing_time"])

    def test_storm_variation_reduction_and_error_are_measured_together(self):
        direct, _ = simulate(CASES[0], "direct")
        for mode in ("persistence", "slew"):
            result, _ = simulate(CASES[0], mode)
            with self.subTest(mode=mode):
                self.assertLessEqual(
                    result["applied_offset_total_variation"],
                    direct["applied_offset_total_variation"] * 0.25,
                )
                self.assertLessEqual(result["late_target_rmse"], 0.05)

    def test_necessary_correction_smoothing_can_fail(self):
        direct, _ = simulate(CASES[1], "direct")
        slow, _ = simulate(CASES[1], "slew")
        guarded, _ = simulate(CASES[1], "gated_slew")
        self.assertIsNone(direct["first_crossing_time"])
        self.assertIsNotNone(slow["first_crossing_time"])
        self.assertGreater(slow["following_over_precision_budget_time"], 0)
        self.assertAlmostEqual(guarded["first_unavailable_time"], 0.15)

    def test_withdrawal_has_no_unauthorized_follow_and_uses_local_braking(self):
        for mode in MODES[:-1]:
            result, trace = simulate(CASES[2], mode)
            with self.subTest(mode=mode):
                self.assertEqual(result["unauthorized_follow_time"], 0)
                self.assertAlmostEqual(result["first_unavailable_time"], 0.2)
                x, v = result["withdrawal_local_state"]
                self.assertAlmostEqual(result["final_local_x"], x + v**2 / 4)
                self.assertAlmostEqual(result["final_local_v"], 0)
                self.assertTrue(
                    all(row["applied_offset"] is None for row in trace[20:])
                )
        unsafe, _ = simulate(CASES[2], "unsafe_keep_on_revoke")
        self.assertGreater(unsafe["unauthorized_follow_time"], 0)

    def test_qualitative_outcomes_survive_timestep_refinement(self):
        for case in CASES:
            for mode in MODES:
                reference, _ = simulate(case, mode)
                for dt in (0.005, 0.0025):
                    result, _ = simulate(case, mode, dt)
                    with self.subTest(case=case.name, mode=mode, dt=dt):
                        self.assertEqual(
                            reference["first_crossing_time"] is None,
                            result["first_crossing_time"] is None,
                        )
                        self.assertEqual(
                            reference["unauthorized_follow_time"] == 0,
                            result["unauthorized_follow_time"] == 0,
                        )


if __name__ == "__main__":
    unittest.main()
