"""Cycle 0006 independent references, feedback counterexamples and message guards."""

import unittest
from dataclasses import replace
from fractions import Fraction as Q
from itertools import pairwise

from baseline import ScalarGaussian
from feedback import (
    BOUNDARY,
    MAX_BASES,
    MAX_NODES,
    VALUES,
    ArtifactGraph,
    Base,
    Engine,
    Info,
    Message,
    Scenario,
    grid,
    observation,
    run_scenario,
    witnesses,
)


class FeedbackTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.runs = [run_scenario(*config) for config in grid()]

    def setUp(self):
        self.a = observation(Q(0), Q(1))
        self.b = observation(Q(10), Q(1))
        self.c = observation(Q(-8), Q(4))
        self.base = Base("parent-1", self.a + self.b, frozenset(("a", "b")))
        self.local = Message(
            "local-b", 1, "local", self.b, frozenset(("b",)), frozenset(("b",))
        )
        self.reply = Message(
            "reply-2",
            2,
            "posterior",
            self.base.info + self.b,
            frozenset(("a", "b")),
            frozenset(("b",)),
            self.base.artifact,
            self.base.sources,
        )

    def engine(self, mode="bound_residual"):
        engine = Engine(mode)
        engine.add_local("a", self.a)
        engine.receive(self.local)
        engine.remember(self.base)
        return engine

    def test_all_supported_answers_equal_original_evidence_reference(self):
        checked = 0
        for metrics, trace, _ in self.runs:
            if metrics["mode"] not in ("local_only", "components", "bound_residual"):
                continue
            for row in trace:
                self.assertEqual(row["estimate"], row["reference"])
                self.assertEqual(
                    row["actual_weights"], {source: Q(1) for source in row["required"]}
                )
                self.assertFalse(row["forbidden"])
                checked += 1
        self.assertEqual(checked, 1824)

    def test_negative_controls_fail_despite_acyclic_versioned_artifacts(self):
        for metrics, trace, graph in self.runs:
            self.assertTrue(metrics["all_artifacts_acyclic"])
            ranks = {key: i for i, key in enumerate(graph)}
            self.assertTrue(
                all(
                    ranks[parent] < ranks[child]
                    for child, parents in graph.items()
                    for parent in parents
                )
            )
            if metrics["mode"] in ("posterior_replace", "direct_only_revocation"):
                self.assertGreater(metrics["numerically_wrong"], 0)
                feedback = next(
                    row
                    for row in trace
                    if row["event"] == f"feedback_{metrics['rounds']:02d}"
                )
                self.assertEqual(feedback["precision_ratio"], metrics["rounds"] + 1)
                self.assertEqual(
                    feedback["actual_weights"],
                    {"a": Q(metrics["rounds"] + 1), "b": Q(metrics["rounds"] + 1)},
                )

    def test_full_lineage_withdrawal_invalidates_contaminated_posterior(self):
        for metrics, trace, _ in self.runs:
            if metrics["mode"] == "posterior_replace":
                withdrawn = [
                    t for t in trace if t["event"] in ("withdraw_a", "late_withdrawn")
                ]
                self.assertEqual(len(withdrawn), 2)
                for row in withdrawn:
                    self.assertIsNone(row["estimate"])
                    self.assertEqual(row["missing"], frozenset(("b",)))
                    self.assertFalse(row["forbidden"])
                self.assertIn(
                    "reject_policy", [d["action"] for d in withdrawn[1]["decisions"]]
                )

    def test_direct_only_removal_leaves_forbidden_returned_influence(self):
        for metrics, trace, _ in self.runs:
            if metrics["mode"] == "direct_only_revocation":
                for row in trace:
                    self.assertEqual(
                        bool(row["forbidden"]),
                        row["event"] in ("withdraw_a", "late_withdrawn"),
                    )
                    if row["forbidden"]:
                        self.assertEqual(row["forbidden"], frozenset(("a",)))
                        self.assertGreater(row["actual_weights"]["a"], 0)
                        if metrics["fixture"] == "equal":
                            self.assertEqual(row["mean_error"], 0)
                            self.assertEqual(row["mean_jump"], 0)

    def test_clean_replay_recovers_and_unsafe_feedback_can_resume(self):
        for metrics, trace, _ in self.runs:
            replay, returned = trace[-2:]
            self.assertEqual(replay["estimate"], replay["reference"])
            self.assertEqual(replay["actual_weights"], {"b": Q(1), "c": Q(1)})
            if metrics["mode"] in ("posterior_replace", "direct_only_revocation"):
                self.assertEqual(returned["estimate"].mean, returned["reference"].mean)
                self.assertEqual(returned["precision_ratio"], 2)

    def test_relay_path_preserves_results_without_creating_a_measurement(self):
        pairs = {}
        for metrics, trace, _ in self.runs:
            key = tuple(metrics[k] for k in ("mode", "fixture", "va", "vb", "rounds"))
            normalized = [
                (t["event"], t["estimate"], t["actual_weights"], t["forbidden"])
                for t in trace
            ]
            if key in pairs:
                self.assertEqual(normalized, pairs[key])
            else:
                pairs[key] = normalized

    def test_current_parent_change_does_not_change_matching_residual(self):
        engine = self.engine()
        engine.install_policy({"a", "b", "c"})
        engine.add_local("c", self.c)
        self.assertEqual(engine.receive(self.reply), "residual")
        self.assertEqual(engine.slot, self.b)
        self.assertEqual(
            engine.query({"a", "b", "c"})[0], ScalarGaussian(Q(32, 9), Q(4, 9))
        )

    def test_bound_residual_missing_or_wrong_base_is_rejected(self):
        engine = self.engine()
        self.assertEqual(
            engine.receive(replace(self.reply, base_id="absent")), "reject_missing_base"
        )
        self.assertEqual(engine.slot, self.b)
        engine = self.engine()
        bad = replace(self.reply, base_sources=frozenset(("a",)))
        self.assertEqual(engine.receive(bad), "reject_base_metadata")
        self.assertEqual(engine.slot, self.b)

    def test_unavailable_when_rejected_reply_is_only_source_of_required_b(self):
        for mode in ("local_only", "bound_residual"):
            engine = Engine(mode)
            engine.add_local("a", self.a)
            self.assertTrue(engine.receive(self.reply).startswith("reject_"))
            self.assertIsNone(engine.query({"a", "b"})[0])
            self.assertEqual(engine.query({"a", "b"})[1], "missing_coverage")

    def test_withdrawal_rejects_mixed_reply_before_attempting_subtraction(self):
        engine = self.engine()
        engine.install_policy({"b", "c"})
        engine.add_local("c", self.c)
        self.assertFalse(engine.bases)
        self.assertEqual(engine.receive(self.reply), "reject_policy")
        self.assertEqual(engine.slot, self.b)
        self.assertEqual(engine.query({"b", "c"})[0], ScalarGaussian(Q(32, 5), Q(4, 5)))

    def test_component_filter_salvages_only_permitted_originals(self):
        engine = Engine("components")
        engine.install_policy({"b", "c"})
        engine.add_local("c", self.c)
        packet = replace(
            self.reply, components=(("a", self.a), ("b", self.b), ("b", self.b))
        )
        self.assertEqual(engine.receive(packet), "components")
        self.assertEqual(engine.atoms, {"b": self.b, "c": self.c})
        self.assertEqual(engine.query({"b", "c"})[0], ScalarGaussian(Q(32, 5), Q(4, 5)))

    def test_boundary_and_base_identity_conflicts_are_explicit(self):
        engine = self.engine()
        self.assertEqual(
            engine.receive(replace(self.reply, boundary=BOUNDARY + "/new")),
            "reject_boundary",
        )
        with self.assertRaises(ValueError):
            engine.remember(replace(self.base, info=self.b))
        with self.assertRaises(ValueError):
            engine.remember(replace(self.base, boundary=BOUNDARY + "/new"))

    def test_revision_conflict_duplicate_and_unseen_stale_delivery(self):
        engine = self.engine("posterior_replace")
        newer = replace(self.reply, revision=4)
        self.assertEqual(engine.receive(newer), "posterior")
        self.assertEqual(engine.receive(newer), "duplicate")
        self.assertEqual(engine.receive(self.reply), "stale")
        self.assertEqual(engine.slot, newer.info)
        with self.assertRaises(ValueError):
            engine.receive(replace(newer, info=self.b))

    def test_conflicting_components_do_not_partially_mutate_state(self):
        engine = Engine("components")
        engine.add_local("a", self.a)
        packet = replace(self.reply, components=(("b", self.b), ("a", self.b)))
        with self.assertRaises(ValueError):
            engine.receive(packet)
        self.assertEqual(engine.atoms, {"a": self.a})

    def test_nonproper_residual_is_not_published(self):
        engine = self.engine()
        self.assertEqual(
            engine.receive(replace(self.reply, info=self.base.info)), "reject_nonproper"
        )
        self.assertEqual(engine.slot, self.b)
        with self.assertRaises(ValueError):
            Info(Q(0), Q(5)).gaussian()

    def test_wrong_base_can_look_proper_but_cancel_legitimate_new_evidence(self):
        witness = witnesses()["wrong_base"]
        self.assertEqual(witness["matched_residual"], self.b)
        self.assertEqual(witness["wrong_residual"], Info(Q(3, 4), Q(12)))
        self.assertEqual(
            witness["wrong_residual"].gaussian(), ScalarGaussian(Q(16), Q(4, 3))
        )
        self.assertEqual(witness["wrong_parent_result"], ScalarGaussian(Q(5), Q(1, 2)))
        self.assertEqual(witness["reference"], ScalarGaussian(Q(32, 9), Q(4, 9)))
        self.assertEqual(
            witness["actual_wrong_residual_weights"], {"b": Q(1), "c": Q(-1)}
        )
        self.assertEqual(witness["weak_b_nonproper_residual"], Info(Q(0), Q(9, 2)))

    def test_damped_feedback_is_bounded_but_not_the_required_posterior(self):
        witness = witnesses()["damped_feedback"]
        self.assertEqual(witness["limit"], ScalarGaussian(Q(2), Q(3, 5)))
        means = [t["estimate"].mean for t in witness["trace"]]
        self.assertEqual(means[0], 5)
        self.assertTrue(all(a > b > 2 for a, b in pairwise(means)))
        self.assertLess(means[-1] - 2, Q(1, 1000000))
        for row in witness["trace"][1:]:
            self.assertNotEqual(row["estimate"], row["reference"])

    def test_feedback_false_precision_suppresses_a_real_correction(self):
        _, trace, _ = run_scenario(
            "posterior_replace", "contrast", Q(1), Q(1), "direct", 12
        )
        new = next(t for t in trace if t["event"] == "new_c")
        self.assertEqual(new["mean_jump"], Q(-13, 105))
        self.assertEqual(new["reference"].mean - 5, Q(-13, 9))
        self.assertGreater(new["precision_ratio"], 10)

    def test_policy_revisions_and_event_frontiers_are_coherent(self):
        _, trace, _ = run_scenario(
            "bound_residual", "contrast", Q(1), Q(1), "direct", 4
        )
        by_event = {row["event"]: row for row in trace}
        self.assertEqual(by_event["initial"]["policy_version"], 1)
        self.assertEqual(by_event["new_c"]["policy_version"], 2)
        self.assertEqual(by_event["withdraw_a"]["policy_version"], 3)
        for index, row in enumerate(trace):
            self.assertEqual(row["event_index"], index)
            self.assertEqual(row["policy_effective_event"], row["policy_learned_event"])
            self.assertEqual(row["policy_learned_event"], row["policy_enforced_event"])
            self.assertLessEqual(row["policy_enforced_event"], index)
        self.assertEqual(
            by_event["new_c"]["observation_acquisition_event"], {"a": 0, "b": 0, "c": 5}
        )
        self.assertEqual(by_event["late_withdrawn"]["policy_enforced_event"], 7)

    def test_peak_base_retention_includes_pending_reply_before_policy_cleanup(self):
        for metrics, _, _ in self.runs:
            expected = (
                metrics["rounds"] + 2 if metrics["mode"] == "bound_residual" else 0
            )
            self.assertEqual(metrics["max_retained_bases"], expected)

    def test_resource_bounds_and_immutable_metadata(self):
        sources = {"a", "b"}
        base = Base("base", self.a + self.b, sources)
        components = [("b", self.b)]
        local = replace(self.local, components=components)
        sources.clear()
        components.clear()
        self.assertEqual(base.sources, frozenset(("a", "b")))
        self.assertEqual(local.components, (("b", self.b),))
        for rounds in (0, 17, True):
            with self.assertRaises(ValueError):
                run_scenario("local_only", "contrast", Q(1), Q(1), "direct", rounds)
        with self.assertRaises(ValueError):
            replace(self.local, revision=True)
        with self.assertRaises(ValueError):
            replace(self.local, components=(("b", self.b),) * 17)
        graph = ArtifactGraph()
        with self.assertRaises(ValueError):
            graph.add(("missing",))
        for _ in range(MAX_NODES):
            graph.add()
        with self.assertRaises(ValueError):
            graph.add()
        engine = Engine("bound_residual")
        for i in range(MAX_BASES):
            engine.remember(replace(self.base, artifact=str(i)))
        with self.assertRaises(ValueError):
            engine.remember(replace(self.base, artifact="one-too-many"))
        scenario = Scenario("local_only", VALUES["contrast"], Q(1), Q(1), "direct")
        with self.assertRaises(ValueError):
            scenario.deliver([(self.local, {"b": Q(1)})] * 9)

    def test_grid_counts_and_failures_are_separate_from_unavailability(self):
        self.assertEqual(len(self.runs), 240)
        self.assertEqual(sum(m["events"] for m, _, _ in self.runs), 3040)
        for mode, expected in (
            ("local_only", (0, 0, 0)),
            ("components", (0, 0, 0)),
            ("bound_residual", (0, 0, 0)),
            ("posterior_replace", (96, 416, 0)),
            ("direct_only_revocation", (0, 512, 96)),
        ):
            metrics = [m for m, _, _ in self.runs if m["mode"] == mode]
            observed = tuple(
                sum(m[key] for m in metrics)
                for key in ("unavailable", "numerically_wrong", "forbidden_outcomes")
            )
            self.assertEqual(observed, expected)
            self.assertTrue(all(m["max_artifact_nodes"] <= MAX_NODES for m in metrics))


if __name__ == "__main__":
    unittest.main()
