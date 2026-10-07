"""Exact numerical references and counterexamples for scoped lifecycle events."""

import itertools
import unittest
from dataclasses import replace
from fractions import Fraction as Q

from entitlement_scenarios import (
    A12,
    ALIGN,
    AUDIT,
    INITIAL,
    MODES,
    PRIVATE,
    RECORDS,
    SA,
    SCENARIOS,
    A,
    B,
    run_scenario,
)
from entitlements import Grant, Ledger, Record


class EventFixtureTests(unittest.TestCase):
    def test_scoped_candidate_matches_every_predeclared_fixture_and_order(self):
        for name in SCENARIOS:
            for order in itertools.permutations(("a1", "a2", "b1")):
                metrics, trace = run_scenario(name, order=order)
                with self.subTest(name=name, order=order):
                    self.assertEqual([r for r in trace if r["violations"]], [])
                    self.assertEqual(metrics["violating_events"], 0)
                    self.assertLessEqual(metrics["events"], 80)
                    self.assertLessEqual(metrics["max_retained_objects"], 32)

    def test_each_negative_control_is_detected_in_every_arrival_order(self):
        for mode in MODES[1:]:
            for order in itertools.permutations(("a1", "a2", "b1")):
                count = sum(
                    run_scenario(name, mode, order)[0]["violating_events"]
                    for name in SCENARIOS
                )
                with self.subTest(mode=mode, order=order):
                    self.assertGreater(count, 0)

    def test_closed_form_numerical_answers_are_preserved(self):
        _, cutoff = run_scenario("future_cutoff")
        self.assertEqual(
            (cutoff[-1]["actual"]["mean"], cutoff[-1]["actual"]["variance"]),
            (Q(8), Q(1, 3)),
        )
        _, overlap = run_scenario("overlapping_grants")
        private = next(
            row
            for row in overlap
            if row["action"] == "probe" and row["view"] == PRIVATE
        )
        self.assertEqual(
            (private["actual"]["mean"], private["actual"]["variance"]), (Q(12), Q(1, 2))
        )
        _, strict = run_scenario("strict_mixed_withdrawal")
        self.assertEqual(
            (strict[-1]["actual"]["mean"], strict[-1]["actual"]["variance"]),
            (Q(0), Q(1)),
        )
        _, gap = run_scenario("regrant_gap")
        partial = next(
            row for row in gap if row["actual"]["represented"] == {"a4", "b1"}
        )
        self.assertEqual(
            (partial["actual"]["mean"], partial["actual"]["variance"]), (Q(9), Q(1, 2))
        )
        self.assertEqual(
            (gap[-1]["actual"]["mean"], gap[-1]["actual"]["variance"]),
            (Q(118, 3), Q(1, 3)),
        )

    def test_retained_derivative_does_not_imply_raw_or_inference_permission(self):
        _, trace = run_scenario("derivative_retention")
        retained = next(
            row for row in trace if row["action"] == "install" and row["args"][1] == 2
        )
        self.assertEqual(retained["actual"]["retained_raw"], {"b1"})
        self.assertEqual(retained["actual"]["mean"], 8)
        retain_only = next(
            row for row in trace if row["action"] == "install" and row["args"][1] == 3
        )
        self.assertEqual(retain_only["actual"]["retained_summaries"], {"a12"})
        self.assertEqual(retain_only["actual"]["represented"], {"b1"})

    def test_missing_component_becomes_unavailable_until_authorized_replay(self):
        _, trace = run_scenario("mixed_without_replay")
        unavailable = next(
            row for row in trace if row["action"] == "install" and row["args"][1] == 3
        )
        self.assertIsNone(unavailable["actual"]["mean"])
        self.assertIsNone(unavailable["actual"]["variance"])
        self.assertEqual(unavailable["actual"]["missing"], {"b1"})
        self.assertEqual(trace[-1]["actual"]["variance"], 1)

    def test_required_coverage_does_not_masquerade_as_complete(self):
        _, trace = run_scenario("regrant_gap")
        required = next(row for row in trace if row["action"] == "probe")
        self.assertEqual(required["actual"]["represented"], {"a4", "b1"})
        self.assertEqual(required["actual"]["missing"], {"a3"})
        self.assertEqual(required["actual"]["status"], "required_coverage_missing")
        self.assertIsNone(required["actual"]["mean"])

    def test_unauthorized_admission_is_detected_even_when_estimate_matches(self):
        _, trace = run_scenario("future_cutoff", "trust_historical_grant")
        violation = next(row for row in trace if row["violations"])
        self.assertIn("decision", violation["violations"])
        self.assertIn("retained_raw", violation["violations"])
        self.assertEqual(violation["actual"]["mean"], violation["expected"]["mean"])
        self.assertEqual(
            violation["actual"]["variance"], violation["expected"]["variance"]
        )

    def test_stale_policy_delivery_is_not_logged_as_reenforcement(self):
        _, trace = run_scenario("regrant_gap")
        stale = trace[-1]["policy_event"]
        self.assertEqual(stale["effective"], trace[0]["time"])
        self.assertEqual(stale["learned"], trace[-1]["time"])
        self.assertIsNone(stale["enforced"])
        self.assertEqual(stale["disposition"], "stale")
        self.assertEqual(trace[-1]["policy_revision"], 3)


class LedgerBoundaryTests(unittest.TestCase):
    def populated(self):
        ledger = Ledger()
        ledger.install(ALIGN, 1, (A, B, SA))
        for identity in sorted(INITIAL):
            ledger.receive_raw(
                ALIGN, ("B" if identity == "b1" else "A", 1), RECORDS[identity]
            )
        return ledger

    def test_replay_and_freeze_never_count_an_identity_twice(self):
        ledger = self.populated()
        ledger.freeze(ALIGN, "a12", ("a1", "a1", "a2"))
        self.assertEqual(ledger.summaries["a12"].precision, 2)
        self.assertEqual(ledger.query(ALIGN).variance, Q(1, 3))
        stamp = ledger.query(ALIGN)
        self.assertEqual(
            ledger.receive_raw(ALIGN, ("A", 1), RECORDS["a1"]), "duplicate"
        )
        self.assertTrue(ledger.accept_cached(ALIGN, stamp))

    def test_conflicting_retained_identity_is_rejected(self):
        ledger = self.populated()
        with self.assertRaises(ValueError):
            ledger.receive_raw(ALIGN, ("A", 1), replace(RECORDS["a1"], value=Q(99)))
        self.assertEqual(ledger.raw["a1"].value, 10)

    def test_grant_generation_cannot_roll_back_or_change_contents(self):
        ledger = self.populated()
        with self.assertRaises(ValueError):
            ledger.install(ALIGN, 2, (replace(A, resources=A12), B, SA))
        self.assertEqual(ledger.policies[ALIGN][0], 1)
        ledger.install(ALIGN, 2, (B,))
        with self.assertRaises(ValueError):
            ledger.install(ALIGN, 3, (A, B))
        self.assertEqual(ledger.policies[ALIGN][0], 2)
        ledger.install(ALIGN, 3, (replace(A, generation=2), B))
        self.assertEqual(ledger.receive_raw(ALIGN, ("A", 1), RECORDS["a1"]), "denied")
        self.assertEqual(ledger.receive_raw(ALIGN, ("A", 2), RECORDS["a1"]), "stored")

    def test_grant_copies_mutable_inputs_before_installation(self):
        resources = {"a1"}
        operations = {"retain", "infer"}
        grant = Grant("mutable-caller", 1, "raw", resources, operations)
        ledger = Ledger()
        ledger.install(ALIGN, 1, (grant,))
        resources.add("a2")
        operations.remove("infer")
        self.assertEqual(grant.resources, {"a1"})
        self.assertEqual(grant.operations, {"retain", "infer"})
        self.assertEqual(
            ledger.receive_raw(ALIGN, (grant.identity, 1), RECORDS["a2"]), "denied"
        )
        self.assertEqual(
            ledger.receive_raw(ALIGN, (grant.identity, 1), RECORDS["a1"]), "stored"
        )
        self.assertEqual(ledger.query(ALIGN).mean, 10)

    def test_policy_conflict_and_old_snapshot_do_not_mutate_state(self):
        ledger = self.populated()
        with self.assertRaises(ValueError):
            ledger.install(ALIGN, 1, (B,))
        ledger.install(ALIGN, 2, (B,))
        self.assertEqual(ledger.install(ALIGN, 1, (A, B, SA)), "stale")
        self.assertEqual(set(ledger.raw), {"b1"})

    def test_retention_and_inference_are_independent_operations(self):
        ledger = Ledger()
        retained = Grant("keep", 1, "raw", A12, frozenset({"retain"}))
        infer = Grant("compute", 1, "raw", A12, frozenset({"infer"}))
        ledger.install(PRIVATE, 1, (retained,))
        ledger.receive_raw(PRIVATE, ("keep", 1), RECORDS["a1"])
        self.assertIsNone(ledger.query(PRIVATE).mean)
        self.assertIsNone(ledger.query(ALIGN).mean)
        ledger.install(ALIGN, 1, (infer,))
        self.assertEqual(ledger.query(ALIGN).mean, 10)
        self.assertEqual(
            ledger.receive_raw(ALIGN, ("compute", 1), RECORDS["a2"]), "denied"
        )
        ledger.install(ALIGN, 2, ())
        self.assertEqual(set(ledger.raw), {"a1"})
        self.assertIsNone(ledger.query(ALIGN).mean)

    def test_mean_can_stay_identical_while_uncertainty_must_change(self):
        ledger = Ledger()
        grant = Grant("equal", 1, "raw", frozenset({"c1", "c2"}))
        ledger.install(ALIGN, 1, (grant,))
        for key in ("c1", "c2"):
            ledger.receive_raw(ALIGN, ("equal", 1), Record(key, Q(0), 1))
        before = ledger.query(ALIGN)
        ledger.install(
            ALIGN, 2, (replace(grant, generation=2, resources=frozenset({"c2"})),)
        )
        after = ledger.query(ALIGN)
        self.assertEqual(before.mean, after.mean)
        self.assertEqual((before.variance, after.variance), (Q(1, 2), Q(1)))

    def test_partial_summary_overlap_is_explicitly_unsupported(self):
        ledger = self.populated()
        second = Grant("second", 1, "summary", frozenset({"a2b1"}))
        ledger.install(ALIGN, 2, (A, B, SA, second))
        ledger.freeze(ALIGN, "a12", A12)
        ledger.freeze(ALIGN, "a2b1", {"a2", "b1"})
        result = ledger.query(ALIGN)
        self.assertEqual(result.status, "overlap_unsupported")
        self.assertIsNone(result.mean)
        self.assertIsNone(result.variance)

    def test_cache_checks_scope_policy_and_input_revision(self):
        ledger = self.populated()
        cached = ledger.query(ALIGN)
        self.assertFalse(ledger.accept_cached(AUDIT, cached))
        ledger.receive_raw(ALIGN, ("A", 1), RECORDS["a3"])
        self.assertFalse(ledger.accept_cached(ALIGN, cached))
        current = ledger.query(ALIGN)
        ledger.install(ALIGN, 2, (A, B, SA))
        self.assertFalse(ledger.accept_cached(ALIGN, current))

    def test_declared_retention_and_grant_budgets(self):
        ledger = Ledger()
        resources = frozenset(f"r{i}" for i in range(33))
        ledger.install(ALIGN, 1, (Grant("many", 1, "raw", resources),))
        for index in range(32):
            ledger.receive_raw(ALIGN, ("many", 1), Record(f"r{index}", Q(0), 0))
        with self.assertRaises(ValueError):
            ledger.receive_raw(ALIGN, ("many", 1), Record("r32", Q(0), 0))
        self.assertEqual(len(ledger.raw), 32)
        with self.assertRaises(ValueError):
            ledger.install(
                PRIVATE, 1, tuple(Grant(f"g{i}", 1, "raw", {"r0"}) for i in range(17))
            )


if __name__ == "__main__":
    unittest.main()
