"""Cycle 0006: exact message-feedback experiments, not a production estimator."""

from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import platform
from collections import Counter
from dataclasses import asdict, dataclass
from fractions import Fraction as Q
from pathlib import Path

from baseline import ScalarGaussian

BOUNDARY = "linear-scalar-feedback/v1"
MODES = (
    "local_only",
    "components",
    "bound_residual",
    "posterior_replace",
    "direct_only_revocation",
)
VALUES = {
    "contrast": {"a": Q(0), "b": Q(10), "c": Q(-8)},
    "equal": {"a": Q(0), "b": Q(0), "c": Q(0)},
}
MAX_ROUNDS, MAX_EVENTS, MAX_NODES, MAX_BASES = 16, 64, 256, 64
MAX_SOURCES, MAX_COMPONENTS, MAX_DELIVERIES = 16, 16, 8


@dataclass(frozen=True)
class Info:
    precision: Q
    information: Q

    def __add__(self, other):
        return Info(
            self.precision + other.precision, self.information + other.information
        )

    def __sub__(self, other):
        return Info(
            self.precision - other.precision, self.information - other.information
        )

    def scaled(self, scale):
        return Info(self.precision * scale, self.information * scale)

    def gaussian(self):
        if self.precision <= 0:
            raise ValueError("No proper scalar Gaussian")
        return ScalarGaussian(self.information / self.precision, 1 / self.precision)


def observation(value, variance):
    if variance <= 0:
        raise ValueError("Positive observation variance required")
    return Info(1 / variance, value / variance)


def total(parts):
    return sum(parts, Info(Q(0), Q(0)))


def combine_weights(left, right, scale=Q(1)):
    """Evaluator diagnostic only; candidate Engine never receives these maps."""
    keys = left.keys() | right.keys()
    result = {key: left.get(key, Q(0)) + scale * right.get(key, Q(0)) for key in keys}
    return {key: value for key, value in result.items() if value}


@dataclass(frozen=True)
class Base:
    artifact: str
    info: Info
    sources: frozenset[str]
    boundary: str = BOUNDARY

    def __post_init__(self):
        object.__setattr__(self, "sources", frozenset(self.sources))
        if not self.artifact or len(self.sources) > MAX_SOURCES:
            raise ValueError("Invalid base metadata")


@dataclass(frozen=True)
class Message:
    artifact: str
    revision: int
    kind: str
    info: Info
    sources: frozenset[str]
    local_sources: frozenset[str]
    base_id: str | None = None
    base_sources: frozenset[str] = frozenset()
    boundary: str = BOUNDARY
    components: tuple[tuple[str, Info], ...] = ()

    def __post_init__(self):
        for name in ("sources", "local_sources", "base_sources"):
            object.__setattr__(self, name, frozenset(getattr(self, name)))
        object.__setattr__(self, "components", tuple(self.components))
        if (
            not self.artifact
            or type(self.revision) is not int
            or self.revision < 1
            or self.kind not in ("local", "posterior")
            or not self.sources
            or len(self.sources) > MAX_SOURCES
            or len(self.components) > MAX_COMPONENTS
            or self.local_sources != frozenset(("b",))
        ):
            raise ValueError("Invalid bounded child message")
        if self.kind == "local" and (
            self.base_id is not None or self.sources != self.local_sources
        ):
            raise ValueError("Local likelihood cannot contain a parent")
        if self.kind == "posterior" and (
            not self.base_id or self.sources != self.base_sources | self.local_sources
        ):
            raise ValueError("Posterior needs complete conditioning metadata")


class ArtifactGraph:
    def __init__(self):
        self.parents = {}

    def add(self, dependencies=()):
        if len(self.parents) >= MAX_NODES:
            raise ValueError("Artifact bound exceeded")
        if any(dep not in self.parents for dep in dependencies):
            raise ValueError("Artifact must reference existing predecessors")
        name = f"artifact-{len(self.parents):03d}"
        self.parents[name] = tuple(dependencies)
        return name

    def acyclic(self):
        # Independent rank audit rather than treating admission as the only check.
        ranks = {key: index for index, key in enumerate(self.parents)}
        return all(
            ranks[parent] < ranks[child]
            for child, parents in self.parents.items()
            for parent in parents
        )


class Engine:
    """Trusted scalar message contracts; no access to evaluator coefficient maps."""

    def __init__(self, mode):
        if mode not in MODES:
            raise ValueError("Unknown method")
        self.mode = mode
        self.allowed = frozenset(("a", "b"))
        self.policy_version = 1
        self.local = {}
        self.atoms = {}
        self.slot = None
        self.slot_sources = frozenset()
        self.slot_artifact = None
        self.bases = {}
        self.peak_bases = 0
        self.seen = {}
        self.watermark = 0

    def add_local(self, source, info):
        if source not in self.allowed:
            raise ValueError("Local source is not authorized")
        if source in self.local and self.local[source] != info:
            raise ValueError("Conflicting original observation")
        if len(self.local) >= MAX_SOURCES and source not in self.local:
            raise ValueError("Source bound exceeded")
        self.local[source] = info
        if self.mode == "components":
            self.atoms[source] = info

    def install_policy(self, allowed):
        allowed = frozenset(allowed)
        if len(allowed) > MAX_SOURCES:
            raise ValueError("Policy source bound exceeded")
        if allowed != self.allowed:
            self.policy_version += 1
        self.allowed = allowed
        self.local = {s: value for s, value in self.local.items() if s in allowed}
        self.atoms = {s: value for s, value in self.atoms.items() if s in allowed}
        if self.mode != "direct_only_revocation" and not self.slot_sources <= allowed:
            self.slot, self.slot_sources, self.slot_artifact = None, frozenset(), None
        # Retain only authorized sent-parent values. Historical causal graph is separate.
        self.bases = {
            key: base for key, base in self.bases.items() if base.sources <= allowed
        }

    def remember(self, base):
        if base.boundary != BOUNDARY or not base.sources <= self.allowed:
            raise ValueError("Invalid conditioning base")
        if self.mode != "bound_residual":
            return
        if base.artifact in self.bases and self.bases[base.artifact] != base:
            raise ValueError("Conflicting base identity")
        if len(self.bases) >= MAX_BASES and base.artifact not in self.bases:
            raise ValueError("Retained base bound exceeded")
        self.bases[base.artifact] = base
        self.peak_bases = max(self.peak_bases, len(self.bases))

    def receive(self, message):
        if message.revision in self.seen:
            if self.seen[message.revision] != message:
                raise ValueError("Conflicting message revision")
            return "duplicate"
        if len(self.seen) >= MAX_EVENTS:
            raise ValueError("Revision history bound exceeded")
        self.seen[message.revision] = message
        if message.revision < self.watermark:
            return "stale"
        self.watermark = message.revision
        if message.boundary != BOUNDARY:
            return "reject_boundary"
        if self.mode == "local_only" and message.kind != "local":
            return "reject_feedback"
        if self.mode == "components":
            if frozenset(s for s, _ in message.components) != message.sources:
                return "reject_missing_components"
            incoming = dict(self.atoms)
            for source, info in message.components:
                if source not in self.allowed:
                    continue
                if source in incoming and incoming[source] != info:
                    raise ValueError("Conflicting original observation")
                info.gaussian()
                incoming[source] = info
            self.atoms = incoming
            self.slot_artifact = message.artifact
            return "components"
        if (
            self.mode != "direct_only_revocation"
            and not message.sources <= self.allowed
        ):
            return "reject_policy"
        if message.kind == "local":
            residual, sources, action = message.info, message.sources, "local"
        elif self.mode == "bound_residual":
            base = self.bases.get(message.base_id)
            if base is None:
                return "reject_missing_base"
            if (
                base.boundary != message.boundary
                or base.sources != message.base_sources
            ):
                return "reject_base_metadata"
            residual = message.info - base.info
            sources, action = message.local_sources, "residual"
        else:
            residual, sources, action = message.info, message.sources, "posterior"
        if residual.precision <= 0:
            return "reject_nonproper"
        self.slot, self.slot_sources, self.slot_artifact = (
            residual,
            sources,
            message.artifact,
        )
        return action

    def active(self):
        if self.mode == "components":
            return total(self.atoms.values()), frozenset(self.atoms)
        parts = list(self.local.values())
        if self.slot is not None:
            parts.append(self.slot)
        return total(parts), frozenset(self.local) | self.slot_sources

    def query(self, required):
        info, sources = self.active()
        if not frozenset(required) <= sources:
            return None, "missing_coverage", sources
        if info.precision <= 0:
            return None, "unobservable", sources
        return info.gaussian(), "ok", sources


def reference(values, variances, required):
    # Normalized inverse-variance weighting, independent of recursive message state.
    normalization = sum((1 / variances[s] for s in required), Q(0))
    weights = {s: (1 / variances[s]) / normalization for s in required}
    return ScalarGaussian(
        sum((weights[s] * values[s] for s in required), Q(0)), 1 / normalization
    )


class Scenario:
    def __init__(self, mode, values, va, vb, path):
        if path not in ("direct", "relay"):
            raise ValueError("Unknown return path")
        self.values = values
        self.variances = {"a": va, "b": vb, "c": Q(4)}
        self.originals = {s: observation(values[s], self.variances[s]) for s in values}
        self.engine = Engine(mode)
        self.path = path
        self.graph = ArtifactGraph()
        self.origin_artifacts = {s: self.graph.add() for s in ("a", "b")}
        self.shadow_slot = {}
        self.shadow_atoms = {"a": Q(1)}
        self.base_weights = {}
        self.trace = []
        self.policy_event = 0
        self.acquisitions = {"a": 0, "b": 0}
        self.current_base = None
        self.current_weights = None
        self.engine.add_local("a", self.originals["a"])

    def local_message(self, revision):
        artifact = self.graph.add((self.origin_artifacts["b"],))
        return Message(
            artifact,
            revision,
            "local",
            self.originals["b"],
            frozenset(("b",)),
            frozenset(("b",)),
            components=(("b", self.originals["b"]),)
            if self.engine.mode == "components"
            else (),
        )

    def prepare_reply(self, revision):
        base = self.current_base
        self.engine.remember(base)
        self.base_weights[base.artifact] = dict(self.current_weights)
        parent_artifact = base.artifact
        if self.path == "relay":
            parent_artifact = self.graph.add((parent_artifact,))
        artifact = self.graph.add((parent_artifact, self.origin_artifacts["b"]))
        components = ()
        if self.engine.mode == "components":
            components = tuple(sorted(self.engine.atoms.items())) + (
                ("b", self.originals["b"]),
            )
        message = Message(
            artifact,
            revision,
            "posterior",
            base.info + self.originals["b"],
            base.sources | {"b"},
            frozenset(("b",)),
            base.artifact,
            base.sources,
            components=components,
        )
        return message, combine_weights(self.current_weights, {"b": Q(1)})

    def deliver(self, deliveries):
        if len(deliveries) > MAX_DELIVERIES:
            raise ValueError("Delivery batch bound exceeded")
        decisions = []
        for message, weights in deliveries:
            action = self.engine.receive(message)
            if action in ("local", "posterior"):
                self.shadow_slot = dict(weights)
            elif action == "residual":
                self.shadow_slot = combine_weights(
                    weights, self.base_weights[message.base_id], Q(-1)
                )
            elif action == "components":
                for source, _ in message.components:
                    if source in self.engine.allowed:
                        self.shadow_atoms[source] = Q(1)
            decisions.append(
                {
                    "artifact": message.artifact,
                    "revision": message.revision,
                    "base_id": message.base_id,
                    "action": action,
                }
            )
        return decisions

    def policy(self, allowed):
        if frozenset(allowed) != self.engine.allowed:
            self.policy_event = len(self.trace)
        self.engine.install_policy(allowed)
        self.shadow_atoms = {s: w for s, w in self.shadow_atoms.items() if s in allowed}
        if self.engine.slot is None:
            self.shadow_slot = {}

    def record(self, event, decisions=()):
        if len(self.trace) >= MAX_EVENTS:
            raise ValueError("Query event bound exceeded")
        allowed = self.engine.allowed
        info, _ = self.engine.active()
        weights = (
            self.shadow_atoms
            if self.engine.mode == "components"
            else combine_weights({s: Q(1) for s in self.engine.local}, self.shadow_slot)
        )
        # The diagnostic map must reproduce the actual candidate's numerical state.
        rebuilt = total(self.originals[s].scaled(w) for s, w in weights.items())
        assert rebuilt == info, (event, rebuilt, info)
        estimate, reason, claimed = self.engine.query(allowed)
        expected = reference(self.values, self.variances, allowed)
        numerical = estimate is not None and estimate != expected
        used = frozenset(weights) if estimate is not None else frozenset()
        forbidden = used - allowed
        missing = allowed - claimed
        previous = self.trace[-1]["estimate"] if self.trace else None
        jump = (
            None
            if estimate is None or previous is None
            else estimate.mean - previous.mean
        )
        dependencies = [self.origin_artifacts[s] for s in self.engine.local]
        if self.engine.slot_artifact is not None:
            dependencies.append(self.engine.slot_artifact)
        artifact = self.graph.add(dependencies)
        self.current_base = Base(artifact, info, claimed)
        self.current_weights = dict(weights)
        self.trace.append(
            {
                "event": event,
                "event_index": len(self.trace),
                "policy_version": self.engine.policy_version,
                "policy_effective_event": self.policy_event,
                "policy_learned_event": self.policy_event,
                "policy_enforced_event": self.policy_event,
                "observation_acquisition_event": dict(self.acquisitions),
                "required": allowed,
                "estimate": estimate,
                "reference": expected,
                "status": reason,
                "claimed_sources": claimed,
                "actual_weights": dict(weights),
                "forbidden": forbidden,
                "missing": missing,
                "numerically_wrong": numerical,
                "mean_error": None
                if estimate is None
                else estimate.mean - expected.mean,
                "variance_error": None
                if estimate is None
                else estimate.variance - expected.variance,
                "precision_ratio": None
                if estimate is None
                else expected.variance / estimate.variance,
                "mean_jump": jump,
                "decisions": list(decisions),
                "artifact": artifact,
                "acyclic": self.graph.acyclic(),
                "artifact_nodes": len(self.graph.parents),
                "retained_bases": len(self.engine.bases),
                "retained_bases_peak": self.engine.peak_bases,
                "revision_records": len(self.engine.seen),
                "active_numeric_coefficients": 2
                * (
                    len(self.engine.atoms)
                    if self.engine.mode == "components"
                    else len(self.engine.local) + (self.engine.slot is not None)
                ),
            }
        )


def run_scenario(mode, fixture, va, vb, path, rounds):
    if type(rounds) is not int or not 1 <= rounds <= MAX_ROUNDS:
        raise ValueError("Feedback round bound exceeded")
    scenario = Scenario(mode, VALUES[fixture], va, vb, path)
    original = scenario.local_message(1)
    scenario.record("initial", scenario.deliver([(original, {"b": Q(1)})]))
    for k in range(1, rounds + 1):
        reply = scenario.prepare_reply(k + 1)
        scenario.record(
            f"feedback_{k:02d}",
            scenario.deliver([reply, reply, (original, {"b": Q(1)})]),
        )
    delayed = scenario.prepare_reply(rounds + 2)
    scenario.policy(frozenset(("a", "b", "c")))
    scenario.origin_artifacts["c"] = scenario.graph.add()
    scenario.acquisitions["c"] = len(scenario.trace)
    scenario.engine.add_local("c", scenario.originals["c"])
    scenario.shadow_atoms["c"] = Q(1)
    scenario.record("new_c")
    scenario.record("delayed_base", scenario.deliver([delayed]))
    pending = scenario.prepare_reply(rounds + 3)
    scenario.policy(frozenset(("b", "c")))
    scenario.record("withdraw_a")
    scenario.record(
        "late_withdrawn", scenario.deliver([pending, pending, (original, {"b": Q(1)})])
    )
    clean = scenario.local_message(rounds + 4)
    scenario.record("clean_replay", scenario.deliver([(clean, {"b": Q(1)})]))
    fresh = scenario.prepare_reply(rounds + 5)
    scenario.record("clean_feedback", scenario.deliver([fresh]))
    trace = scenario.trace
    metrics = {
        "mode": mode,
        "fixture": fixture,
        "va": va,
        "vb": vb,
        "path": path,
        "rounds": rounds,
        "events": len(trace),
        "unavailable": sum(t["estimate"] is None for t in trace),
        "numerically_wrong": sum(t["numerically_wrong"] for t in trace),
        "forbidden_outcomes": sum(bool(t["forbidden"]) for t in trace),
        "all_artifacts_acyclic": all(t["acyclic"] for t in trace),
        "mean_total_variation": sum(
            (abs(t["mean_jump"]) for t in trace if t["mean_jump"] is not None), Q(0)
        ),
        "max_mean_jump": max(
            abs(t["mean_jump"]) for t in trace if t["mean_jump"] is not None
        ),
        "max_precision_ratio": max(
            t["precision_ratio"] for t in trace if t["precision_ratio"] is not None
        ),
        "max_artifact_nodes": max(t["artifact_nodes"] for t in trace),
        "max_retained_bases": scenario.engine.peak_bases,
        "max_revision_records": max(t["revision_records"] for t in trace),
        "max_active_numeric_coefficients": max(
            t["active_numeric_coefficients"] for t in trace
        ),
        "admission_actions": dict(
            sorted(Counter(d["action"] for t in trace for d in t["decisions"]).items())
        ),
    }
    return metrics, trace, scenario.graph.parents


def witnesses():
    a, b, c = (
        observation(Q(0), Q(1)),
        observation(Q(10), Q(1)),
        observation(Q(-8), Q(4)),
    )
    old, current = a + b, a + b + c
    pending = old + b
    wrong = pending - current
    weak_b = observation(Q(10), Q(4))
    invalid = (a + weak_b + weak_b) - (a + weak_b + c)
    damped = []
    state = a + b
    alpha = Q(1, 4)
    for k in range(13):
        wa = (1 - alpha ** (k + 1)) / (1 - alpha)
        wb = alpha**k + alpha * (1 - alpha**k) / (1 - alpha)
        assert state == a.scaled(wa) + b.scaled(wb)
        damped.append(
            {
                "round": k,
                "weights": {"a": wa, "b": wb},
                "estimate": state.gaussian(),
                "reference": (a + b).gaussian(),
            }
        )
        state = a + (state + b).scaled(alpha)
    return {
        "wrong_base": {
            "old": old,
            "current": current,
            "pending_reply": pending,
            "matched_residual": pending - old,
            "wrong_residual": wrong,
            "actual_wrong_residual_weights": {"b": Q(1), "c": Q(-1)},
            "wrong_parent_result": (a + c + wrong).gaussian(),
            "reference": current.gaussian(),
            "weak_b_nonproper_residual": invalid,
        },
        "damped_feedback": {
            "alpha": alpha,
            "trace": damped,
            "limit": ScalarGaussian(Q(2), Q(3, 5)),
            "reference": (a + b).gaussian(),
        },
    }


def encode(value):
    if isinstance(value, Q):
        return str(value)
    if hasattr(value, "__dataclass_fields__"):
        return encode(asdict(value))
    if isinstance(value, dict):
        return {key: encode(item) for key, item in value.items()}
    if isinstance(value, (frozenset, set)):
        return sorted(value)
    if isinstance(value, (list, tuple)):
        return [encode(item) for item in value]
    return value


def grid():
    return itertools.product(
        MODES, VALUES, (Q(1), Q(4)), (Q(1), Q(4)), ("direct", "relay"), (1, 4, 12)
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    metrics, saved = [], []
    for config in grid():
        metric, trace, graph = run_scenario(*config)
        metrics.append(metric)
        mode, fixture, va, vb, path, rounds = config
        if (
            fixture == "contrast"
            and va == 1
            and (
                (vb == 1 and path == "direct" and rounds == 12)
                or (vb == 4 and path == "relay" and rounds == 4)
            )
        ):
            key = f"{mode}-b{vb}-{path}-k{rounds}"
            saved.append(
                {"config": metric, "trace": trace, "artifact_graph": graph, "key": key}
            )
    traces = "".join(
        json.dumps(encode(run), sort_keys=True, separators=(",", ":")) + "\n"
        for run in saved
    )
    (args.output_dir / "representative_runs.jsonl").write_text(traces)
    totals = {
        mode: {
            key: sum(m[key] for m in metrics if m["mode"] == mode)
            for key in (
                "events",
                "unavailable",
                "numerically_wrong",
                "forbidden_outcomes",
            )
        }
        for mode in MODES
    }
    root = Path(__file__).resolve().parent
    document = {
        "cycle": "0006",
        "parent_commit": "425994c79765dbad2b7a1465970bb64a2ed241c2",
        "python": platform.python_version(),
        "randomness": "none",
        "runs": len(metrics),
        "scope": "Exact scalar additive feedback and trusted synchronous P3; no physical, nonlinear or distributed validation",
        "metrics": metrics,
        "totals": totals,
        "witnesses": witnesses(),
        "source_sha256": {
            name: hashlib.sha256((root / name).read_bytes()).hexdigest()
            for name in (
                "baseline.py",
                "feedback.py",
                "test_feedback.py",
                "cycles/0006-protocol.md",
            )
        },
        "traces_sha256": hashlib.sha256(traces.encode()).hexdigest(),
    }
    (args.output_dir / "results.json").write_text(
        json.dumps(encode(document), indent=2, sort_keys=True) + "\n"
    )
    print(json.dumps(totals, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
