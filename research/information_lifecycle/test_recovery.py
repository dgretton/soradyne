"""Independent reference and negative-control checks for cycle 0008."""

import unittest
from dataclasses import replace
from fractions import Fraction as Q

from baseline import ScalarGaussian
from recovery import (
    BASE_IDS,
    MODEL,
    MODES,
    TARGET_IDS,
    WARM_IDS,
    Checkpoint,
    Custodian,
    Engine,
    digest,
    grid,
    manifest,
    oracle,
    records_for,
    run_scenario,
)


class RecoveryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.runs = [run_scenario(*config) for config in grid()]

    def setUp(self):
        self.records = records_for("contrast", Q(1), Q(1))
        self.checkpoint = Checkpoint.make(self.records[k] for k in sorted(BASE_IDS))
        self.target = manifest(self.records[k] for k in sorted(TARGET_IDS))
        self.allowed = set(self.records) - {"b3"}

    def engine(self, mode="manifest_checkpoint", **kwargs):
        parameters = {
            "mode": mode,
            "target": self.target,
            "checkpoint": self.checkpoint,
            "checkpoint_id": digest(self.checkpoint),
            "output": oracle(self.records, TARGET_IDS),
            "warm": [self.records[k] for k in sorted(WARM_IDS)],
            "allowed": self.allowed,
            "policy_version": 1,
            "policy_event": 0,
        }
        parameters.update(kwargs)
        return Engine(**parameters)

    def test_supported_answers_match_independent_oracle_and_unique_inputs(self):
        checked = 0
        for metric, trace, _ in self.runs:
            if metric["mode"] not in ("manifest_checkpoint", "raw_replay"):
                continue
            for row in trace:
                self.assertFalse(row["forbidden_processing"])
                self.assertFalse(row["wrong_answer"])
                if row["result"]["estimate"] is not None:
                    self.assertEqual(row["result"]["estimate"], row["reference"])
                    self.assertEqual(
                        row["result"]["weights"], dict.fromkeys(row["required_ids"], 1)
                    )
                checked += 1
        self.assertEqual(checked, 4608)

    def test_recovery_availability_follows_information_and_permission(self):
        for metric, trace, _ in self.runs:
            mode = metric["mode"]
            if mode not in ("manifest_checkpoint", "raw_replay"):
                continue
            enough = metric["archive_case"] == "complete" or (
                mode == "manifest_checkpoint"
                and metric["archive_case"] == "missing_base"
                and metric["withdrawal"] == "none"
            )
            self.assertEqual(metric["retry_available"], enough, metric)
            self.assertTrue(metric["m2_available"], metric)
            for row in trace[9:]:
                self.assertIsNotNone(row["result"]["estimate"])
            if not enough:
                missing = "a1" if metric["archive_case"] == "missing_base" else "a2"
                self.assertEqual(trace[6]["result"]["missing"], {missing})

    def test_same_durable_horizon_does_not_imply_same_live_result(self):
        metric, trace, context = run_scenario(
            "horizon_warm", "contrast", Q(1), Q(1), "complete", "none", "forward"
        )
        self.assertEqual(
            context["durable_horizon_old"], context["durable_horizon_standby"]
        )
        self.assertEqual(context["old_output"], ScalarGaussian(Q(2), Q(1, 4)))
        self.assertEqual(context["warm_output"], ScalarGaussian(Q(10, 3), Q(1, 3)))
        self.assertEqual(trace[1]["mean_jump"], Q(4, 3))
        self.assertEqual(trace[1]["new_data_count"], 0)
        self.assertTrue(trace[1]["coverage_mismatch"])
        self.assertEqual(metric["forbidden_processing"], 0)

    def test_equal_numbers_cannot_certify_matching_input_coverage(self):
        _, trace, _ = run_scenario(
            "horizon_warm", "equal", Q(1), Q(1), "complete", "before", "forward"
        )
        self.assertEqual(trace[1]["result"]["estimate"], trace[1]["reference"])
        self.assertFalse(trace[1]["wrong_answer"])
        self.assertTrue(trace[1]["coverage_mismatch"])

    def test_output_seed_recounts_replay_even_with_duplicate_suppression(self):
        _, trace, _ = run_scenario(
            "output_seed", "contrast", Q(1), Q(1), "complete", "none", "forward"
        )
        self.assertEqual(trace[6]["result"]["estimate"], ScalarGaussian(Q(2), Q(1, 8)))
        self.assertEqual(trace[6]["reference"], ScalarGaussian(Q(2), Q(1, 4)))
        self.assertEqual(set(trace[6]["result"]["weights"].values()), {2})
        self.assertEqual(
            trace[7]["result"]["estimate"], ScalarGaussian(Q(10, 9), Q(1, 9))
        )
        self.assertEqual(trace[7]["reference"], ScalarGaussian(Q(2, 5), Q(1, 5)))
        self.assertEqual(trace[7]["mean_jump"], Q(-8, 9))
        self.assertEqual(trace[8]["mean_jump"], 0)

    def test_hold_is_smooth_but_stale_and_forbidden(self):
        metric, trace, _ = run_scenario(
            "hold_output", "contrast", Q(1), Q(1), "complete", "before", "forward"
        )
        self.assertEqual(metric["total_variation"], 0)
        self.assertEqual(trace[1]["forbidden_processing"], {"b1"})
        self.assertTrue(trace[7]["wrong_answer"])
        self.assertEqual(trace[10]["forbidden_processing"], {"b1", "a2"})

    def test_partial_checkpoint_never_advertises_complete_recovery(self):
        engine = self.engine()
        snap = engine.snapshot()
        self.assertIsNone(snap["estimate"])
        self.assertEqual(snap["missing"], {"a2"})
        self.assertEqual(engine.receive(self.records["a2"], 2), "accepted")
        self.assertEqual(engine.snapshot()["estimate"], ScalarGaussian(Q(2), Q(1, 4)))

    def test_checkpoint_avoids_overlap_and_retains_no_raw_base_records(self):
        engine = self.engine()
        for identity in sorted(BASE_IDS):
            self.assertEqual(
                engine.receive(self.records[identity], 2), "covered_by_checkpoint"
            )
        self.assertFalse(engine.ledger)
        self.assertTrue(
            all(isinstance(value, str) for pair in engine.base.inputs for value in pair)
        )
        engine.receive(self.records["a2"], 2)
        self.assertEqual(engine.receive(self.records["a2"], 3), "duplicate")
        self.assertEqual(engine.snapshot()["weights"], dict.fromkeys(TARGET_IDS, 1))

    def test_output_can_be_sufficient_if_explicitly_bound_as_checkpoint(self):
        # The failure is not the fact that these numbers were also published.
        output = oracle(self.records, TARGET_IDS)
        bound_output = Checkpoint(
            MODEL, self.target, 1 / output.variance, output.mean / output.variance
        )
        engine = self.engine(
            checkpoint=bound_output, checkpoint_id=digest(bound_output)
        )
        for identity in sorted(TARGET_IDS):
            self.assertEqual(
                engine.receive(self.records[identity], 2), "covered_by_checkpoint"
            )
        self.assertEqual(engine.snapshot()["estimate"], output)
        extended = TARGET_IDS | {"b3"}
        engine.set_target(2, manifest(self.records[k] for k in sorted(extended)))
        engine.policy(2, self.allowed | {"b3"}, 7)
        engine.receive(self.records["b3"], 7, live=True)
        self.assertEqual(
            engine.snapshot()["estimate"], ScalarGaussian(Q(2, 5), Q(1, 5))
        )
        self.assertEqual(engine.snapshot()["weights"], dict.fromkeys(extended, 1))

    def test_revocation_discards_mixed_base_before_reconstruction(self):
        engine = self.engine()
        engine.receive(self.records["a2"], 2)
        engine.policy(2, self.allowed - {"b1"}, 3)
        self.assertIsNone(engine.base)
        self.assertEqual(engine.snapshot()["missing"], {"d", "a1"})
        self.assertEqual(engine.receive(self.records["b1"], 4), "reject_policy")
        for identity in ("d", "a1"):
            engine.receive(self.records[identity], 4)
        self.assertEqual(engine.snapshot()["estimate"], ScalarGaussian(Q(4), Q(1, 3)))
        self.assertNotIn("b1", engine.snapshot()["weights"])

    def test_output_seed_respects_revocation_despite_its_overlap_bug(self):
        for metric, _, _ in self.runs:
            if metric["mode"] == "output_seed":
                self.assertEqual(metric["forbidden_processing"], 0)

    def test_checkpoint_rebuild_can_pause_when_raw_replay_stays_available(self):
        _, checkpoint_trace, _ = run_scenario(
            "manifest_checkpoint",
            "contrast",
            Q(1),
            Q(1),
            "complete",
            "after",
            "forward",
        )
        _, raw_trace, _ = run_scenario(
            "raw_replay", "contrast", Q(1), Q(1), "complete", "after", "forward"
        )
        self.assertIsNotNone(checkpoint_trace[4]["result"]["estimate"])
        self.assertIsNone(checkpoint_trace[5]["result"]["estimate"])
        self.assertEqual(checkpoint_trace[5]["result"]["missing"], {"d", "a1"})
        self.assertEqual(
            raw_trace[5]["result"]["estimate"], ScalarGaussian(Q(4), Q(1, 3))
        )
        self.assertEqual(
            checkpoint_trace[6]["result"]["estimate"],
            raw_trace[5]["result"]["estimate"],
        )

    def test_missing_archive_is_not_misreported_as_acknowledged(self):
        for metric, _, context in self.runs:
            absent = {"complete": None, "missing_base": "a1", "missing_tail": "a2"}[
                metric["archive_case"]
            ]
            acknowledgments = context["m1_acknowledgments"]
            self.assertNotIn("b3", acknowledgments)
            if absent:
                self.assertNotIn(absent, acknowledgments)
                self.assertEqual(context["m2_acknowledgments"][absent]["origin"], "A")
            for identity, ack in acknowledgments.items():
                self.assertEqual(
                    ack["record_hash"], digest(context["records"][identity])
                )
                self.assertEqual(ack["custodian"], "M1")

    def test_custodian_replay_preserves_origin_and_filters_permissions(self):
        custodian = Custodian("M1", [self.records[k] for k in sorted(TARGET_IDS)])
        replay = custodian.replay(TARGET_IDS, {"a1", "a2"})
        self.assertEqual(replay, (self.records["a1"], self.records["a2"]))
        self.assertTrue(all(r.origin == "A" for r in replay))
        with self.assertRaises(ValueError):
            Custodian("M1", [self.records["a1"], self.records["a1"]])

    def test_current_manifest_and_checkpoint_binding_are_checked(self):
        for cp in (
            replace(self.checkpoint, model="other-model"),
            replace(self.checkpoint, information=Q(99)),
            replace(self.checkpoint, inputs=manifest([self.records["b2"]])),
        ):
            with self.assertRaises(ValueError):
                self.engine(checkpoint=cp)
        with self.assertRaises(ValueError):
            self.engine(
                checkpoint=replace(self.checkpoint, model="other-model"),
                checkpoint_id=digest(replace(self.checkpoint, model="other-model")),
            )
        engine = self.engine()
        with self.assertRaises(ValueError):
            engine.receive(replace(self.records["a1"], value=Q(99)), 2)
        self.assertFalse(engine.ledger)
        self.assertEqual(engine.base.model, MODEL)

    def test_policy_frontiers_acquisition_and_receipt_are_distinct(self):
        for metric, trace, _ in self.runs:
            stage = {"before": 1, "during": 3, "after": 5, "none": None}[
                metric["withdrawal"]
            ]
            version, effective = 1, 0
            for row in trace:
                event = row["event"]
                if event in (7, 10) or event == stage:
                    version, effective = version + 1, event
                self.assertEqual(row["policy_version"], version)
                self.assertEqual(row["policy_effective_event"], effective)
                self.assertEqual(row["policy_learned_event"], effective)
                self.assertEqual(row["policy_enforced_event"], effective)
                for admission in row["admissions"]:
                    self.assertEqual(
                        admission["acquired"], 7 if admission["id"] == "b3" else 0
                    )
                    self.assertEqual(admission["received"], event)
                self.assertEqual(row["target_version"], 1 if event < 7 else 2)

    def test_reordered_replay_has_same_quiescent_result(self):
        runs = {}
        for metric, trace, _ in self.runs:
            key = tuple(
                metric[k]
                for k in ("mode", "values", "va", "vb", "archive_case", "withdrawal")
            )
            state = [
                (trace[i]["result"]["estimate"], trace[i]["result"]["weights"])
                for i in (6, 9, 10, 11)
            ]
            if key in runs:
                self.assertEqual(runs[key], state)
            else:
                runs[key] = state

    def test_duplicate_replay_creates_no_further_jump(self):
        for metric, trace, _ in self.runs:
            if metric["mode"] in (
                "manifest_checkpoint",
                "raw_replay",
                "hold_output",
                "horizon_warm",
            ):
                self.assertEqual(
                    trace[7]["result"]["estimate"], trace[8]["result"]["estimate"]
                )
                self.assertEqual(
                    trace[7]["result"]["weights"], trace[8]["result"]["weights"]
                )

    def test_invalid_bounds_and_mutable_inputs(self):
        allowed = set(self.allowed)
        target = [list(p) for p in self.target]
        engine = self.engine(allowed=allowed, target=target)
        allowed.clear()
        target.clear()
        self.assertEqual(set(engine.target), TARGET_IDS)
        self.assertEqual(engine.allowed, self.allowed)
        for version in (0, 1, True):
            with self.assertRaises(ValueError):
                engine.policy(version, self.allowed, 2)
        for event in (-1, 32, True):
            with self.assertRaises(ValueError):
                engine.policy(2, self.allowed, event)
            with self.assertRaises(ValueError):
                engine.receive(self.records["a2"], event)
        with self.assertRaises(ValueError):
            engine.policy(2, {str(i) for i in range(17)}, 2)
        with self.assertRaises(ValueError):
            engine.batch([self.records["a1"]] * 33, 2)
        with self.assertRaises(ValueError):
            engine.set_target(2, [(str(i), str(i)) for i in range(17)])
        with self.assertRaises(ValueError):
            engine.set_target(2, self.target[:-1])
        with self.assertRaises(ValueError):
            replace(self.records["a1"], variance=0)
        with self.assertRaises(ValueError):
            manifest([self.records["a1"]] * 17)

    def test_fixed_grid_and_no_unmodeled_resource_growth(self):
        self.assertEqual(len(self.runs), 960)
        self.assertEqual(sum(m["outcomes"] for m, _, _ in self.runs), 11520)
        for mode in MODES:
            metrics = [m for m, _, _ in self.runs if m["mode"] == mode]
            self.assertEqual(len(metrics), 192)
            self.assertTrue(all(m["peak_replacement_entries"] <= 6 for m in metrics))
        self.assertTrue(
            any(
                m["wrong_answer"] for m, _, _ in self.runs if m["mode"] == "output_seed"
            )
        )
        self.assertTrue(
            all(
                m["coverage_mismatch"]
                for m, _, _ in self.runs
                if m["mode"] == "horizon_warm"
            )
        )

    def test_retained_entry_exhaustion_is_explicit(self):
        records = [replace(self.records["a1"], identity=f"r{i}") for i in range(16)]
        target = manifest(records)
        cp = Checkpoint.make(records)
        engine = self.engine(
            mode="output_seed",
            target=target,
            checkpoint=cp,
            checkpoint_id=digest(cp),
            output=cp.estimate,
            allowed={r.identity for r in records},
            warm=[],
        )
        for record in records[:15]:
            engine.receive(record, 2)
        with self.assertRaises(ValueError):
            engine.receive(records[15], 2)
        self.assertEqual(engine.snapshot()["entries"], 16)
        self.assertNotIn(records[15].identity, engine.ledger)


if __name__ == "__main__":
    unittest.main()
