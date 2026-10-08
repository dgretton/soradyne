"""Exact information, permission and retention-boundary checks for cycle 0009."""

import unittest
from dataclasses import replace
from fractions import Fraction as Q

from baseline import ScalarGaussian
from expiry import (
    HISTORICAL,
    MODES,
    STRICT,
    Archive,
    Block,
    Engine,
    fixture,
    grid,
    insufficiency_witness,
    run_scenario,
)


class ExpiryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.runs = [run_scenario(*config) for config in grid()]

    def setUp(self):
        self.records = fixture("contrast", "unit")
        self.raw = [self.records[k] for k in sorted(HISTORICAL)]

    def engine(self, mode="by_source", expires=11):
        return Engine(mode, self.raw, self.records["d"], expires)

    def test_strict_available_answers_match_full_oracle(self):
        checked = 0
        for metric, trace in self.runs:
            if metric["mode"] not in STRICT:
                continue
            for row in trace:
                self.assertFalse(row["forbidden_processing"])
                self.assertFalse(row["wrong_complete_claim"])
                if row["result"]["estimate"] is not None:
                    self.assertEqual(row["result"]["estimate"], row["reference"])
                    self.assertEqual(
                        row["result"]["weights"], dict.fromkeys(row["required_ids"], 1)
                    )
                checked += 1
        self.assertEqual(checked, 7488)

    def test_recovery_deadline_and_granularity_match_predeclared_formula(self):
        for metric, trace in self.runs:
            if metric["mode"] not in STRICT | {"honest_subset"}:
                continue
            needs_replay = metric["mode"] != "atomic" and not (
                metric["mode"] != "mixed" and metric["scope"] == "source"
            )
            w, delay, e = (
                metric["withdrawal_time"],
                metric["delay"],
                metric["archive_expiry"],
            )
            expected = (w + delay if w + delay < e else None) if needs_replay else w
            self.assertEqual(metric["first_full_recovery_time"], expected, metric)
            expected_replay = (
                ("accepted" if expected is not None else "raw_expired")
                if needs_replay
                else "not_needed"
            )
            self.assertEqual(metric["replay_outcome"], expected_replay)
            for row in trace[metric["summary_expiry"] :]:
                self.assertNotEqual(row["result"]["status"], "complete")

    def test_p2_raw_expiry_preserves_permitted_derivative_until_its_deadline(self):
        _, trace = run_scenario("atomic", "contrast", "unit", 3, 11, 6, 0, "record")
        self.assertEqual(trace[3]["archive_entries"], 0)
        self.assertEqual(trace[3]["result"]["estimate"], ScalarGaussian(Q(2), Q(1, 5)))
        self.assertEqual(
            trace[6]["result"]["estimate"], ScalarGaussian(Q(5, 2), Q(1, 4))
        )
        self.assertIsNotNone(trace[10]["result"]["estimate"])
        self.assertIsNone(trace[11]["result"]["estimate"])

    def test_same_source_summary_supports_whole_source_but_not_record_removal(self):
        _, full_source = run_scenario(
            "by_source", "contrast", "unit", 3, 11, 3, 0, "source"
        )
        _, one_record = run_scenario(
            "by_source", "contrast", "unit", 3, 11, 3, 0, "record"
        )
        self.assertEqual(
            full_source[3]["result"]["estimate"], ScalarGaussian(Q(0), Q(1, 3))
        )
        self.assertIsNone(one_record[3]["result"]["estimate"])
        self.assertEqual(one_record[3]["result"]["missing"], {"a2"})

    def test_honest_subset_is_correct_for_declared_inputs_and_marks_loss(self):
        for metric, trace in self.runs:
            if metric["mode"] != "honest_subset":
                continue
            for row in trace:
                actual = row["result"]["estimate"]
                self.assertIsNotNone(actual)
                self.assertEqual(actual, row["actual_set_reference"])
                self.assertFalse(row["forbidden_processing"])
                self.assertFalse(row["wrong_complete_claim"])
                self.assertGreaterEqual(row["full_precision_shortfall"], 0)
                self.assertEqual(
                    row["result"]["status"] == "reduced", row["coverage_mismatch"]
                )
                if row["result"]["status"] == "reduced":
                    self.assertTrue(row["result"]["missing"])
                    self.assertGreater(row["full_precision_shortfall"], 0)

    def test_reduced_answer_can_be_useful_but_differs_from_full_answer(self):
        _, trace = run_scenario(
            "honest_subset", "contrast", "unit", 3, 11, 3, 0, "record"
        )
        self.assertEqual(trace[3]["result"]["estimate"], ScalarGaussian(Q(0), Q(1, 3)))
        self.assertEqual(trace[3]["reference"], ScalarGaussian(Q(5, 2), Q(1, 4)))
        self.assertEqual(trace[3]["full_mean_error"], Q(-5, 2))
        self.assertEqual(trace[3]["full_precision_shortfall"], 1)
        self.assertEqual(trace[8]["result"]["estimate"], ScalarGaussian(Q(-2), Q(1, 4)))
        self.assertEqual(trace[8]["reference"], ScalarGaussian(Q(2, 5), Q(1, 5)))
        self.assertEqual(
            trace[11]["result"]["estimate"], ScalarGaussian(Q(-4), Q(1, 2))
        )
        self.assertEqual(trace[11]["result"]["missing"], {"a2", "b1", "b2"})

    def test_coarse_complete_fails_even_though_subset_arithmetic_is_correct(self):
        for metric, trace in self.runs:
            if metric["mode"] == "coarse_complete":
                self.assertGreater(metric["wrong_complete_claim"], 0)
                self.assertEqual(metric["actual_set_numerical_error"], 0)
                self.assertEqual(metric["forbidden_processing"], 0)
                for row in trace:
                    self.assertEqual(
                        row["wrong_complete_claim"], row["coverage_mismatch"]
                    )

    def test_held_mixed_state_is_forbidden_even_when_mean_does_not_move(self):
        _, trace = run_scenario("hold_mixed", "equal", "unit", 3, 11, 3, 0, "source")
        self.assertEqual(trace[3]["mean_jump"], 0)
        self.assertEqual(trace[3]["forbidden_processing"], {"a1", "a2"})
        self.assertTrue(trace[3]["wrong_complete_claim"])
        self.assertFalse(trace[11]["forbidden_processing"])
        self.assertTrue(trace[11]["wrong_complete_claim"])

    def test_two_worlds_have_identical_aggregate_but_different_required_answers(self):
        first, second = insufficiency_witness()
        self.assertEqual(first["retained"], second["retained"])
        self.assertEqual(first["before"], ScalarGaussian(Q(2), Q(1, 5)))
        self.assertEqual(second["before"], first["before"])
        self.assertEqual(first["required_after_a1"], ScalarGaussian(Q(5, 2), Q(1, 4)))
        self.assertEqual(second["required_after_a1"], ScalarGaussian(Q(2), Q(1, 4)))

    def test_archive_deadline_is_half_open_and_read_does_not_pin_values(self):
        archive = Archive(self.raw, 3)
        self.assertEqual(len(archive.read(HISTORICAL, HISTORICAL, 2)), 4)
        self.assertEqual(archive.read(HISTORICAL, HISTORICAL, 3), ())
        self.assertFalse(archive.records)
        self.assertEqual(archive.read(HISTORICAL, HISTORICAL, 4), ())
        with self.assertRaises(ValueError):
            archive.advance(2)

    def test_replay_started_before_expiry_can_finish_too_late(self):
        _, trace = run_scenario("mixed", "contrast", "unit", 3, 11, 2, 2, "record")
        self.assertEqual(
            trace[2]["pending"],
            {"sources": {"a2", "b1", "b2"}, "requested": 2, "due": 4},
        )
        replay = next(a for a in trace[4]["actions"] if a["kind"] == "replay")
        self.assertEqual(replay["outcome"], "raw_expired")
        self.assertFalse(replay["delivered"])
        self.assertTrue(all(t["result"]["estimate"] is None for t in trace[2:]))

    def test_rebuilt_summary_does_not_extend_original_deadline(self):
        _, trace = run_scenario("mixed", "contrast", "unit", 6, 7, 3, 2, "record")
        self.assertEqual(
            trace[5]["result"]["estimate"], ScalarGaussian(Q(5, 2), Q(1, 4))
        )
        rebuilt = trace[5]["result"]["blocks"]["rebuild:a2,b1,b2"]
        self.assertEqual(rebuilt["created"], 5)
        self.assertEqual(rebuilt["expires"], 7)
        self.assertIsNone(trace[7]["result"]["estimate"])
        self.assertEqual(trace[12]["actions"][0]["outcome"], "reject_expired")

    def test_late_relabel_cannot_renew_retention_permission(self):
        engine = self.engine(expires=7)
        engine.advance(1)
        engine.freeze()
        old = engine.blocks["a"]
        engine.advance(7)
        renewed = replace(old, identity="renamed-a", created=7, expires=15)
        self.assertEqual(engine.admit(renewed), "reject_deadline_change")
        self.assertFalse(engine.covered() & {"a1", "a2"})

    def test_future_artifact_is_not_available_yet(self):
        engine = self.engine()
        engine.advance(1)
        engine.freeze()
        engine.policy(2, engine.allowed - {"a1"}, 1)
        future = Block.make("future-a2", [self.records["a2"]], 5, 11)
        self.assertEqual(engine.admit(future), "reject_future")
        self.assertNotIn("a2", engine.covered())

    def test_new_acquisition_cannot_renew_an_old_source_identity(self):
        engine = self.engine(expires=7)
        engine.advance(1)
        engine.freeze()
        engine.advance(8)
        with self.assertRaises(ValueError):
            engine.add_new(replace(self.records["a2"], acquired=8))
        self.assertEqual(engine.derivative_deadlines["a2"], 7)
        self.assertEqual(engine.add_new(self.records["n"]), "accepted")
        self.assertEqual(engine.derivative_deadlines["n"], 16)
        self.assertNotIn("a2", engine.covered())

    def test_rejected_new_acquisition_does_not_mutate_retained_metadata(self):
        many = [replace(self.records["a1"], identity=f"r{i}") for i in range(15)]
        full = Engine("atomic", many, self.records["d"], 11)
        late = self.engine()
        late.advance(17)
        for engine in (full, late):
            before = (engine.target, engine.allowed, dict(engine.derivative_deadlines))
            with self.assertRaises(ValueError):
                engine.add_new(replace(self.records["n"], acquired=engine.time))
            self.assertEqual(
                (engine.target, engine.allowed, engine.derivative_deadlines), before
            )

    def test_policy_is_enforced_before_same_tick_replay(self):
        _, trace = run_scenario("mixed", "contrast", "unit", 6, 11, 2, 0, "record")
        self.assertEqual(
            [a["kind"] for a in trace[2]["actions"]], ["P3", "request", "replay"]
        )
        self.assertFalse(trace[2]["forbidden_processing"])
        self.assertEqual(
            trace[2]["result"]["estimate"], ScalarGaussian(Q(5, 2), Q(1, 4))
        )

    def test_policy_and_acquisition_times_are_recorded_separately(self):
        for metric, trace in self.runs:
            for row in trace:
                time, w = row["time"], metric["withdrawal_time"]
                version = 1 if time < w else (2 if time < 8 else 3)
                effective = 0 if time < w else (w if time < 8 else 8)
                self.assertEqual(row["policy_version"], version)
                for key in (
                    "policy_effective_time",
                    "policy_learned_time",
                    "policy_enforced_time",
                ):
                    self.assertEqual(row[key], effective)
                self.assertEqual(row["new_acquisitions"], 1 if time == 8 else 0)
                self.assertEqual(
                    set(row["result"]["local_raw_ids"]),
                    HISTORICAL if time == 0 else set(),
                )
                for identity, acquired in row["acquisition_times"].items():
                    self.assertEqual(acquired, 8 if identity == "n" else 0)

    def test_duplicate_overlap_and_forbidden_replay_checks(self):
        engine = self.engine()
        engine.advance(1)
        engine.freeze()
        self.assertEqual(engine.admit(engine.blocks["a"]), "duplicate")
        with self.assertRaises(ValueError):
            engine.admit(replace(engine.blocks["a"], information=Q(99)))
        with self.assertRaises(ValueError):
            engine.admit(Block.make("overlap", [self.records["a2"]], 1, 11))
        engine.policy(2, engine.allowed - {"a1"}, 1)
        self.assertEqual(
            engine.admit(Block.make("forbidden", [self.records["a1"]], 1, 11)),
            "reject_policy",
        )
        self.assertEqual(
            engine.rebuild([self.records["a1"]], {"a1"}), ("reject_policy", None)
        )
        status, block = engine.rebuild([self.records["a2"]] * 2, {"a2"})
        self.assertEqual(status, "accepted")
        self.assertEqual(block.precision, 1)
        self.assertEqual(engine.snapshot()["weights"]["a2"], 1)

    def test_immutable_metadata_and_invalid_bounds(self):
        sources = {"a1"}
        block = Block("test", sources, Q(1), Q(0), 1, 11)
        sources.clear()
        self.assertEqual(block.sources, {"a1"})
        engine = self.engine()
        allowed = set(engine.allowed)
        engine.policy(2, allowed, 0)
        allowed.clear()
        self.assertTrue(engine.allowed)
        for time in (-1, 32, True):
            with self.assertRaises(ValueError):
                engine.advance(time)
        with self.assertRaises(ValueError):
            engine.policy(2, engine.allowed, 0)
        with self.assertRaises(ValueError):
            engine.rebuild([self.records["a1"]] * 9, {"a1"})
        with self.assertRaises(ValueError):
            Block("huge", {str(i) for i in range(17)}, Q(1), Q(0), 1, 11)
        with self.assertRaises(ValueError):
            Archive(self.raw * 2, 3)
        with self.assertRaises(ValueError):
            replace(self.records["a1"], variance=0)
        with self.assertRaises(ValueError):
            self.engine(expires=32)
        with self.assertRaises(ValueError):
            engine.freeze()

    def test_grid_resource_integrals_and_permission_failures(self):
        self.assertEqual(len(self.runs), 1152)
        self.assertEqual(sum(m["outcomes"] for m, _ in self.runs), 14976)
        for metric, _ in self.runs:
            self.assertEqual(
                metric["archive_record_ticks"], 4 * metric["archive_expiry"]
            )
            self.assertLessEqual(metric["peak_retained_entries"], 6)
            self.assertEqual(metric["repeated_influence"], 0)
            self.assertEqual(metric["actual_set_numerical_error"], 0)
            if metric["mode"] != "hold_mixed":
                self.assertEqual(metric["forbidden_processing"], 0)
            else:
                self.assertEqual(
                    metric["forbidden_processing"],
                    metric["summary_expiry"] - metric["withdrawal_time"],
                )
        self.assertEqual({m["mode"] for m, _ in self.runs}, set(MODES))


if __name__ == "__main__":
    unittest.main()
