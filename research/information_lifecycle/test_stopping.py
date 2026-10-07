"""Independent stopping references, physical constraints and validity checks."""

import copy
import math
import unittest
from dataclasses import replace

from dynamics import CASES, Consumer, Estimate, simulate
from stopping import (
    CLEARANCES,
    Config,
    advance,
    assess_clearance,
    configurations,
    distance_oracle,
    first_crossing,
    first_zero_lower_bound,
    simulate_stop,
    stop_plan,
)


class StoppingTests(unittest.TestCase):
    def assert_admissible(self, config, metrics, trace):
        self.assertLessEqual(abs(metrics["final_velocity"]), 1e-10)
        self.assertLessEqual(abs(metrics["final_acceleration"]), 1e-10)
        self.assertGreaterEqual(metrics["min_velocity"], -1e-10)
        self.assertLessEqual(
            metrics["max_acceleration"], config.acceleration_limit + 1e-10
        )
        self.assertLessEqual(metrics["max_jerk"], config.jerk_limit + 1e-10)
        previous = (0.0, config.velocity, config.acceleration)
        for row in trace:
            for observed, expected in zip((row["x"], row["v"], row["a"]), previous):
                self.assertAlmostEqual(observed, expected, delta=1e-10)
            self.assertLessEqual(
                abs(row["a_next"] - row["a"]),
                config.jerk_limit * row["h"] + 1e-10,
            )
            self.assertAlmostEqual(
                row["a_next"], row["a"] + row["jerk"] * row["h"], delta=1e-10
            )
            previous = row["x_next"], row["v_next"], row["a_next"]

    def test_constant_jerk_integral(self):
        # x=1+2h+3h^2/2-2h^3/6, v=2+3h-h^2, a=3-2h.
        x, v, a = advance(1, 2, 3, -2, 0.5)
        self.assertAlmostEqual(x, 7 / 3)
        self.assertAlmostEqual(v, 3.25)
        self.assertEqual(a, 2)

    def test_triangular_closed_form(self):
        config = Config(1, 0, 1, 0)
        metrics, trace = simulate_stop(config)
        # Two 1 s ramps, peak deceleration 1; mean speed 1/2 for 2 s.
        self.assertAlmostEqual(metrics["distance"], 1)
        self.assertAlmostEqual(metrics["stop_time"], 2)
        self.assertEqual([p.name for p in stop_plan(config)], ["ramp_down", "ramp_up"])
        self.assert_admissible(config, metrics, trace)

    def test_saturated_closed_form(self):
        config = Config(2, 0, 4, 0)
        metrics, trace = simulate_stop(config)
        # 0.5 s down, 0.5 s at -2, 0.5 s up; mean speed 1.
        self.assertAlmostEqual(metrics["distance"], 1.5)
        self.assertAlmostEqual(metrics["stop_time"], 1.5)
        self.assert_admissible(config, metrics, trace)

    def test_positive_acceleration_and_delay_hand_calculation(self):
        config = Config(1, 2, 4, 0.25)
        metrics, trace = simulate_stop(config)
        # Distances for delay/down/hold/up: 5/16 + 11/6 + 1/2 + 1/12.
        self.assertAlmostEqual(metrics["distance"], 131 / 48)
        self.assertAlmostEqual(metrics["stop_time"], 2.25)
        self.assert_admissible(config, metrics, trace)

    def test_first_zero_lower_bound_independent_examples(self):
        distance, time, acceleration = first_zero_lower_bound(Config(1, 0, 4, 0))
        self.assertAlmostEqual(distance, 23 / 48)
        self.assertAlmostEqual(time, 0.75)
        self.assertAlmostEqual(acceleration, -2)
        distance, time, acceleration = first_zero_lower_bound(Config(0.25, 0, 4, 0))
        self.assertAlmostEqual(time, math.sqrt(0.125))
        self.assertAlmostEqual(distance, (2 / 3) * 0.25 * math.sqrt(0.125))
        self.assertAlmostEqual(acceleration, -math.sqrt(2))

    def test_crossing_time_inside_a_jerk_segment(self):
        _, trace = simulate_stop(Config(1, 0, 4, 0))
        # On the first ramp, x(t)=t-2t^3/3. At t=1/4 this is 23/96.
        self.assertAlmostEqual(first_crossing(trace, 23 / 96), 0.25, delta=1e-10)

    def test_zero_state_needs_no_motion(self):
        metrics, trace = simulate_stop(Config(0, 0, 4, 0))
        self.assertEqual(trace, [])
        self.assertEqual(metrics["distance"], 0)
        self.assertEqual(metrics["stop_time"], 0)
        self.assertEqual(metrics["alignment_unavailable_at"], 0)

    def test_invalid_domains_and_resource_limits_rejected(self):
        base = Config(1, 0, 4, 0)
        for field, bad in (
            ("velocity", -1),
            ("velocity", float("nan")),
            ("acceleration", -0.1),
            ("acceleration", 3),
            ("jerk_limit", 0),
            ("delay", -1),
            ("delay", float("inf")),
            ("acceleration_limit", 0),
        ):
            with self.subTest(field=field, bad=bad), self.assertRaises(ValueError):
                simulate_stop(replace(base, **{field: bad}))
        for dt in (0, -1, float("nan"), 1e-6):
            with self.subTest(dt=dt), self.assertRaises(ValueError):
                simulate_stop(base, dt)
        with self.assertRaises(ValueError):
            simulate_stop(replace(base, delay=11))

    def test_all_profiles_match_oracle_and_constraints(self):
        for config in configurations():
            with self.subTest(config=config):
                metrics, trace = simulate_stop(config)
                self.assert_admissible(config, metrics, trace)
                self.assertLessEqual(abs(metrics["distance_error"]), 1e-10)
                self.assertLessEqual(abs(metrics["time_error"]), 1e-10)
                self.assertGreaterEqual(
                    metrics["distance"] + 1e-10,
                    metrics["necessary_distance_lower_bound"],
                )
                self.assertGreaterEqual(
                    metrics["necessary_distance_lower_bound"] + 1e-10,
                    metrics["naive_acceleration_only_distance"],
                )

    def test_clamped_velocity_and_acceleration_reset_control_is_rejected(self):
        config = Config(1, 2, 4, 0)
        metrics, trace = simulate_stop(config)
        broken = copy.deepcopy(trace)
        # Abruptly report rest at the first endpoint, retaining the commanded jerk.
        broken[0]["a_next"] = 0
        broken[0]["v_next"] = 0
        with self.assertRaises(AssertionError):
            self.assert_admissible(config, metrics, broken)
        _, _, a_first_zero = first_zero_lower_bound(config)
        self.assertGreater(abs(a_first_zero) / 0.01, config.jerk_limit)

    def test_p3_is_immediate_even_during_physical_delay(self):
        for config in configurations():
            metrics, trace = simulate_stop(config)
            with self.subTest(config=config):
                self.assertEqual(metrics["alignment_unavailable_at"], 0)
                self.assertEqual(metrics["alignment_fields_remaining"], 0)
                self.assertEqual(
                    metrics["available_alignment_steps_after_withdrawal"], 0
                )
                self.assertFalse(metrics["late_estimate_accepted"])
                for row in trace:
                    self.assertFalse(row["alignment_available"])
                    self.assertIsNone(row["alignment_mean"])
                    self.assertIsNone(row["alignment_bound"])
                if config.delay:
                    self.assertEqual(trace[0]["phase"], "physical_delay")
                    self.assertEqual(trace[0]["a_next"], config.acceleration)

    def test_anchor_reproduces_cycle_0002_event_state(self):
        _, trace = simulate(CASES[1], "gated_slew")
        event = trace[15]
        self.assertAlmostEqual(event["v"], 1.1)
        self.assertAlmostEqual(event["x"], 0.1425)
        self.assertAlmostEqual(trace[14]["a"], 2)
        self.assertAlmostEqual(
            CASES[1].wall_global - CASES[1].true_offset - event["x"], 0.6575
        )
        self.assertFalse(event["available"])
        # Precision gating retains permission; P3 separately clears all mean state.
        consumer = Consumer("gated_slew", 0.08)
        consumer.receive(Estimate(1, 0, 0, 0.02), 0)
        consumer.receive(Estimate(2, 1, 0.4, 0.02), 0.15)
        self.assertFalse(consumer.step(0.15, 0.01).available)
        self.assertTrue(consumer.authorized)
        consumer.revoke()
        self.assertFalse(consumer.step(0.15, 0.01).available)
        self.assertIsNone(consumer.latest)

    def test_naive_false_clear_and_anchor_crossing_predictions(self):
        false_clear = anchor_crossing = 0
        for config in configurations():
            metrics, trace = simulate_stop(config)
            for c in CLEARANCES:
                assessment = assess_clearance(metrics, c)
                false_clear += assessment["naive_false_clear"]
                crossed = metrics["distance"] > c + 1e-10
                self.assertEqual(first_crossing(trace, c) is not None, crossed)
                if config.velocity == 1.1 and config.acceleration == 2 and c == 0.6575:
                    anchor_crossing += crossed
        self.assertGreater(false_clear, 0)
        self.assertGreater(anchor_crossing, 0)

    def test_equality_is_contact_and_lower_bound_gap_is_not_impossibility(self):
        metrics, _ = simulate_stop(Config(1, 0, 4, 0))
        self.assertEqual(
            assess_clearance(metrics, 0.5)["classification"], "boundary_contact"
        )
        self.assertEqual(
            assess_clearance(metrics, 0.49)["classification"],
            "candidate_crosses_lower_bound_inconclusive",
        )
        self.assertEqual(
            assess_clearance(metrics, 0.4)["classification"], "unavoidable_under_model"
        )

    def test_event_aligned_refinements_preserve_distances_and_classifications(self):
        for config in configurations():
            base, _ = simulate_stop(config)
            for dt in (0.005, 0.0025):
                refined, _ = simulate_stop(config, dt)
                with self.subTest(config=config, dt=dt):
                    self.assertLessEqual(abs(refined["distance_error"]), 1e-10)
                    self.assertLessEqual(abs(refined["time_error"]), 1e-10)
                    for c in CLEARANCES:
                        self.assertEqual(
                            assess_clearance(base, c)["classification"],
                            assess_clearance(refined, c)["classification"],
                        )

    def test_parameter_upper_corner_bounds_prescribed_profile(self):
        for config in configurations():
            upper = replace(
                config, velocity=config.velocity + 0.05, delay=config.delay + 0.02
            )
            metrics, trace = simulate_stop(upper)
            self.assert_admissible(upper, metrics, trace)
            self.assertLessEqual(abs(metrics["distance_error"]), 1e-10)
            for dv in (0, 0.05):
                for dd in (0, 0.02):
                    corner_distance, _ = distance_oracle(
                        replace(
                            config,
                            velocity=config.velocity + dv,
                            delay=config.delay + dd,
                        )
                    )
                    self.assertLessEqual(
                        corner_distance, metrics["oracle_distance"] + 1e-10
                    )


if __name__ == "__main__":
    unittest.main()
