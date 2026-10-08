"""Exact independent observability, authorization and coordinate-change checks."""

import unittest
from dataclasses import replace
from fractions import Fraction as Q

from observability import (
    MODES,
    QUERIES,
    Engine,
    Estimate,
    FrozenScalar,
    Observation,
    grid,
    information,
    pinned_estimate,
    rank_estimate,
    run_scenario,
    solve_range,
)


class ObservabilityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.runs = [run_scenario(*config) for config in grid()]

    def setUp(self):
        self.anchor = Observation("A0", "anchor", 1, (Q(1), Q(0)), Q(5), Q(100), 0)
        self.relative = Observation("R", "relative", 1, (Q(-1), Q(1)), Q(3), Q(1, 4), 0)

    def test_both_supported_methods_match_reference_for_every_query(self):
        checked = 0
        for metric, trace in self.runs:
            if metric["mode"] not in ("rank_query", "split_summary"):
                continue
            for row in trace:
                self.assertEqual(row["result"].estimate, row["reference"])
                self.assertFalse(row["forbidden_processing"])
                checked += 1
        self.assertEqual(checked, 1920)

    def test_query_specific_availability_after_each_loss(self):
        _, trace = run_scenario("rank_query", Q(1), Q(1, 4), Q(1000), Q(7))
        expected = {
            0: set(QUERIES),
            1: set(QUERIES),
            2: {"d", "minus_2d"},
            3: {"d", "minus_2d"},
            4: {"d", "minus_2d"},
            5: set(QUERIES),
            6: {"x"},
            7: set(),
        }
        for event, queries in expected.items():
            actual = {
                t["query"]
                for t in trace
                if t["event"] == event and t["result"].estimate is not None
            }
            self.assertEqual(actual, queries)

    def test_origin_change_only_changes_coordinate_expression(self):
        for metric, trace in self.runs:
            before = {t["query"]: t for t in trace if t["event"] == 0}
            after = [t for t in trace if t["event"] == 1]
            for row in after:
                old = before[row["query"]]
                self.assertEqual(row["canonical_mean"], old["canonical_mean"])
                self.assertEqual(
                    row["result"].estimate.variance, old["result"].estimate.variance
                )
                self.assertEqual(
                    row["coordinate_jump"], -metric["shift"] * sum(row["coefficients"])
                )
                self.assertEqual(row["canonical_jump"], 0)
                self.assertEqual(row["frame_epoch"], 2)

    def test_gauge_changes_cannot_change_observable_relative_answer(self):
        for metric, trace in self.runs:
            for row in trace:
                if row["event"] == 3 and row["query"] in ("d", "minus_2d"):
                    if metric["mode"] == "blanket_failure":
                        self.assertTrue(row["needlessly_unavailable"])
                    else:
                        self.assertEqual(row["canonical_jump"], 0)
                        self.assertEqual(row["result"].estimate, row["reference"])
            if metric["mode"] == "hard_gauge":
                wrong = [
                    t
                    for t in trace
                    if t["event"] == 3 and t["query"] in ("x", "y", "midpoint")
                ]
                self.assertTrue(all(t["false_available"] for t in wrong))
                self.assertTrue(all(t["canonical_jump"] == 11 for t in wrong))

    def test_weak_anchor_is_finite_and_honestly_uncertain(self):
        _, trace = run_scenario("rank_query", Q(10**12), Q(4), Q(0), Q(0))
        initial = {t["query"]: t for t in trace if t["event"] == 0}
        self.assertEqual(initial["x"]["result"].estimate, Estimate(Q(5), Q(10**12)))
        self.assertEqual(initial["d"]["result"].estimate, Estimate(Q(3), Q(4)))
        self.assertEqual(initial["y"]["result"].rank, 2)
        withdrawn = next(t for t in trace if t["event"] == 2 and t["query"] == "x")
        self.assertIsNone(withdrawn["result"].estimate)
        self.assertEqual(withdrawn["result"].rank, 1)

    def test_joint_covariance_cross_terms_preserve_relative_precision(self):
        anchor = replace(self.anchor, variance=Q(10**12))
        info = information([anchor, self.relative])
        mean, covariance = info.joint()
        self.assertEqual(mean, (Q(5), Q(8)))
        self.assertEqual(covariance, (Q(10**12), Q(10**12), Q(10**12) + Q(1, 4)))
        xx, xy, yy = covariance
        self.assertEqual(xx + yy - 2 * xy, Q(1, 4))
        self.assertGreater(xx + yy, Q(10**12))

    def test_rank_solver_is_not_hardcoded_to_the_fixture_axes(self):
        probe = Observation("probe", "probe", 1, (Q(2), Q(3)), Q(7), Q(5), 0)
        for origin in (Q(0), Q(17)):
            info = information([probe], origin)
            rank, estimate = rank_estimate(info, (Q(2), Q(3)))
            self.assertEqual(rank, 1)
            self.assertEqual(estimate, Estimate(Q(7) - 5 * origin, Q(5)))
            self.assertIsNone(rank_estimate(info, (Q(1), Q(0)))[1])
            self.assertEqual(
                rank_estimate(info, (Q(-4), Q(-6)))[1],
                Estimate(-2 * (Q(7) - 5 * origin), Q(20)),
            )
            for gauge in (Q(-100), Q(0), Q(3)):
                self.assertEqual(
                    pinned_estimate(info, (Q(2), Q(3)), gauge)[1], estimate
                )

    def test_null_direction_does_not_acquire_zero_variance(self):
        info = information([self.relative])
        self.assertEqual(solve_range(info, (Q(1), Q(1))), (1, None))
        self.assertEqual(
            rank_estimate(info, (Q(-1), Q(1))), (1, Estimate(Q(3), Q(1, 4)))
        )
        fake = pinned_estimate(info, (Q(1), Q(0)), Q(7))[1]
        self.assertEqual(fake, Estimate(Q(7), Q(0)))
        self.assertIsNone(rank_estimate(info, (Q(1), Q(0)))[1])

    def test_empty_information_only_supports_constant_queries(self):
        for mode in MODES:
            engine = Engine(mode)
            engine.policy(1, set(), 0)
            answer = engine.query((Q(0), Q(0)), Q(1000), Q(7))
            self.assertEqual(answer.estimate, Estimate(Q(0), Q(0)))
            self.assertEqual(answer.status, "deterministic")
            self.assertFalse(answer.processed_ids)
            if mode != "hard_gauge":
                self.assertIsNone(engine.query((Q(-1), Q(1))).estimate)
        self.assertEqual(solve_range(information([]), (Q(1), Q(0))), (0, None))

    def test_split_summary_retains_no_raw_observation_objects(self):
        engine = Engine("split_summary")
        engine.policy(1, {"A0", "R"}, 0)
        engine.receive_batch([self.anchor, self.relative])
        self.assertTrue(all(isinstance(v, FrozenScalar) for v in engine.slots.values()))
        self.assertTrue(all(isinstance(v, str) for v in engine.fingerprints.values()))
        engine.policy(2, {"R"}, 1)
        self.assertEqual(set(engine.slots), {"relative"})
        answer = engine.query((-1, 1))
        self.assertEqual(answer.estimate, Estimate(Q(3), Q(1, 4)))
        self.assertEqual(answer.processed_ids, frozenset(("R",)))

    def test_replay_cannot_restore_revoked_or_superseded_anchor(self):
        for mode in MODES:
            engine = Engine(mode)
            engine.policy(1, {"A0", "R"}, 0)
            engine.receive_batch([self.anchor, self.relative])
            engine.policy(2, {"R"}, 1)
            if mode != "retain_anchor":
                self.assertEqual(engine.receive(self.anchor), "reject_policy")
                self.assertNotIn("anchor", engine.slots)
            newer = replace(
                self.anchor, identity="A1", revision=2, value=Q(-2), acquisition_event=2
            )
            engine.policy(3, {"A1", "R"}, 2)
            engine.receive(newer)
            engine.receive(self.anchor)
            self.assertEqual(engine.slots["anchor"].identity, "A1")

    def test_relative_numerical_cancellation_does_not_authorize_forbidden_processing(
        self,
    ):
        _, trace = run_scenario("retain_anchor", Q(1), Q(1, 4), Q(0), Q(0))
        relative = next(t for t in trace if t["event"] == 2 and t["query"] == "d")
        self.assertEqual(relative["result"].estimate, relative["reference"])
        self.assertEqual(relative["forbidden_processing"], frozenset(("A0",)))
        self.assertTrue(relative["forbidden_output"])
        unavailable = next(t for t in trace if t["event"] == 7 and t["query"] == "d")
        self.assertIsNone(unavailable["result"].estimate)
        self.assertTrue(unavailable["forbidden_processing"])
        self.assertFalse(unavailable["forbidden_output"])

    def test_new_anchor_restores_global_answer_without_changing_relative_data(self):
        for metric, trace in self.runs:
            reanchored = {t["query"]: t for t in trace if t["event"] == 5}
            self.assertEqual(reanchored["x"]["canonical_mean"], -2)
            self.assertEqual(
                reanchored["x"]["result"].estimate.variance,
                metric["anchor_variance"] / 4,
            )
            self.assertEqual(reanchored["d"]["canonical_mean"], 3)
            self.assertEqual(
                reanchored["d"]["result"].estimate.variance, metric["relative_variance"]
            )
            self.assertEqual(
                reanchored["x"]["active_sources"]["anchor"]["acquisition_event"], 5
            )
            self.assertFalse(reanchored["x"]["forbidden_processing"])

    def test_policy_and_frame_versions_describe_distinct_events(self):
        _, trace = run_scenario("rank_query", Q(1), Q(4), Q(1000), Q(0))
        revisions = (1, 1, 2, 2, 2, 3, 4, 5)
        policy_events = (0, 0, 2, 2, 2, 5, 6, 7)
        for row in trace:
            self.assertEqual(row["policy_version"], revisions[row["event"]])
            self.assertEqual(row["policy_effective_event"], policy_events[row["event"]])
            self.assertEqual(row["policy_learned_event"], row["policy_enforced_event"])
            self.assertEqual(
                row["policy_enforced_event"], row["policy_effective_event"]
            )
            self.assertEqual(row["frame_epoch"], 1 if row["event"] == 0 else 2)

    def test_revision_conflicts_and_unseen_old_revisions_do_not_mutate_active_state(
        self,
    ):
        engine = Engine("rank_query")
        engine.policy(1, {"A0"}, 0)
        engine.receive(self.anchor)
        self.assertEqual(engine.receive(self.anchor), "duplicate")
        with self.assertRaises(ValueError):
            engine.receive(replace(self.anchor, value=Q(6)))
        self.assertEqual(engine.slots["anchor"], self.anchor)
        newer = replace(self.anchor, revision=3)
        engine.receive(newer)
        self.assertEqual(engine.receive(replace(self.anchor, revision=2)), "stale")
        self.assertEqual(engine.slots["anchor"], newer)

    def test_immutable_inputs_and_invalid_resource_requests(self):
        allowed = {"A0"}
        row = [Q(1), Q(0)]
        copied = replace(self.anchor, row=row)
        engine = Engine("rank_query")
        engine.policy(1, allowed, 0)
        allowed.clear()
        row.clear()
        self.assertEqual(engine.allowed, frozenset(("A0",)))
        self.assertEqual(copied.row, (Q(1), Q(0)))
        for revision in (0, True):
            with self.assertRaises(ValueError):
                replace(self.anchor, revision=revision)
        for bad_row in ((), (0, 0), (1, 2, 3)):
            with self.assertRaises(ValueError):
                replace(self.anchor, row=bad_row)
        with self.assertRaises(ValueError):
            replace(self.anchor, variance=Q(0))
        with self.assertRaises(ValueError):
            engine.policy(1, {"A0"}, 1)
        with self.assertRaises(ValueError):
            engine.policy(2, {str(i) for i in range(17)}, 1)
        with self.assertRaises(ValueError):
            engine.policy(2, {"A0"}, 32)
        with self.assertRaises(ValueError):
            engine.receive_batch([self.anchor] * 9)
        with self.assertRaises(ValueError):
            engine.query((Q(1),))
        for revision in range(1, 33):
            engine.receive(replace(self.anchor, revision=revision))
        with self.assertRaises(ValueError):
            engine.receive(replace(self.anchor, revision=33))
        self.assertEqual(len(engine.fingerprints), 32)

    def test_full_grid_counts_distinguish_false_certainty_and_needless_unavailability(
        self,
    ):
        self.assertEqual(len(self.runs), 120)
        self.assertEqual(sum(m["outcomes"] for m, _ in self.runs), 4800)
        expected = {
            "rank_query": (528, 0, 0, 0, 0),
            "split_summary": (528, 0, 0, 0, 0),
            "blanket_failure": (360, 0, 168, 0, 0),
            "hard_gauge": (960, 432, 0, 0, 0),
            "retain_anchor": (768, 240, 0, 480, 384),
        }
        for mode, values in expected.items():
            selected = [m for m, _ in self.runs if m["mode"] == mode]
            actual = tuple(
                sum(m[key] for m in selected)
                for key in (
                    "available",
                    "false_available",
                    "needlessly_unavailable",
                    "forbidden_processing",
                    "forbidden_output",
                )
            )
            self.assertEqual(actual, values)
            self.assertEqual(sum(m["numerically_wrong"] for m in selected), 0)


if __name__ == "__main__":
    unittest.main()
