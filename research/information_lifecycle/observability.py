"""Cycle 0007: exact query observability after withdrawal; research only."""

from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import platform
from dataclasses import asdict, dataclass
from fractions import Fraction as Q
from pathlib import Path

from baseline import Factor, Information2

MODES = (
    "rank_query",
    "split_summary",
    "blanket_failure",
    "hard_gauge",
    "retain_anchor",
)
QUERIES = {
    "x": (Q(1), Q(0)),
    "y": (Q(0), Q(1)),
    "d": (Q(-1), Q(1)),
    "midpoint": (Q(1, 2), Q(1, 2)),
    "minus_2d": (Q(2), Q(-2)),
}
MAX_ENTRIES, MAX_REVISIONS, MAX_DELIVERIES, MAX_STATES = 16, 32, 8, 32


@dataclass(frozen=True)
class Estimate:
    mean: Q
    variance: Q

    def __post_init__(self):
        if self.variance < 0:
            raise ValueError("Negative variance")


@dataclass(frozen=True)
class Observation:
    identity: str
    family: str
    revision: int
    row: tuple[Q, Q]
    value: Q
    variance: Q
    acquisition_event: int

    def __post_init__(self):
        object.__setattr__(self, "row", tuple(Q(c) for c in self.row))
        object.__setattr__(self, "value", Q(self.value))
        object.__setattr__(self, "variance", Q(self.variance))
        if (
            not self.identity
            or not self.family
            or type(self.revision) is not int
            or self.revision < 1
            or len(self.row) != 2
            or not any(self.row)
            or self.variance <= 0
            or type(self.acquisition_event) is not int
            or self.acquisition_event < 0
        ):
            raise ValueError("Invalid observation")


@dataclass(frozen=True)
class FrozenScalar:
    identity: str
    family: str
    revision: int
    mean: Q
    variance: Q
    acquisition_event: int


def information(records, origin=Q(0)):
    return Information2.from_factors(
        [
            Factor(r.row[0], r.row[1], r.value - origin * sum(r.row), r.variance)
            for r in records
        ]
    )


def solve_range(info, target):
    """Exact two-column elimination. Free variables are zero, not known constants."""
    if len(target) != 2:
        raise ValueError("Two-variable query required")
    rows = [[info.h00, info.h01, Q(target[0])], [info.h01, info.h11, Q(target[1])]]
    pivots = []
    for column in range(2):
        pivot = next((i for i in range(len(pivots), 2) if rows[i][column]), None)
        if pivot is None:
            continue
        index = len(pivots)
        rows[index], rows[pivot] = rows[pivot], rows[index]
        divisor = rows[index][column]
        rows[index] = [value / divisor for value in rows[index]]
        for i in range(2):
            if i != index:
                scale = rows[i][column]
                rows[i] = [
                    a - scale * b for a, b in zip(rows[i], rows[index], strict=True)
                ]
        pivots.append(column)
    if any(not any(row[:2]) and row[2] for row in rows):
        return len(pivots), None
    solution = [Q(0), Q(0)]
    for index, column in enumerate(pivots):
        solution[column] = rows[index][2]
    return len(pivots), tuple(solution)


def rank_estimate(info, query):
    rank, solution = solve_range(info, query)
    if solution is None:
        return rank, None
    mean = info.eta0 * solution[0] + info.eta1 * solution[1]
    variance = sum((a * b for a, b in zip(query, solution, strict=True)), Q(0))
    return rank, Estimate(mean, variance)


def pinned_estimate(info, query, gauge):
    """Deliberately incorrect external uncertainty: free coordinates treated as known."""
    rank, _ = solve_range(info, (Q(0), Q(0)))
    if rank == 2:
        mean, (xx, xy, yy) = info.joint()
    elif info.h11:
        mean = (gauge, (info.eta1 - info.h01 * gauge) / info.h11)
        xx, xy, yy = Q(0), Q(0), 1 / info.h11
    elif info.h00:
        mean = (info.eta0 / info.h00, gauge)
        xx, xy, yy = 1 / info.h00, Q(0), Q(0)
    else:
        mean = (gauge, gauge)
        xx = xy = yy = Q(0)
    a, b = query
    return rank, Estimate(
        a * mean[0] + b * mean[1], a * a * xx + 2 * a * b * xy + b * b * yy
    )


@dataclass(frozen=True)
class Result:
    estimate: Estimate | None
    status: str
    rank: int
    processed_ids: frozenset[str]


class Engine:
    def __init__(self, mode):
        if mode not in MODES:
            raise ValueError("Unknown candidate")
        self.mode = mode
        self.allowed = frozenset()
        self.policy_version = 0
        self.policy_event = 0
        self.slots = {}
        self.watermarks = {}
        self.fingerprints = {}

    def policy(self, version, allowed, event):
        allowed = frozenset(allowed)
        if (
            type(version) is not int
            or version <= self.policy_version
            or len(allowed) > MAX_ENTRIES
            or type(event) is not int
            or not 0 <= event < MAX_STATES
        ):
            raise ValueError("Invalid policy revision or bounds")
        self.allowed, self.policy_version, self.policy_event = allowed, version, event
        self.slots = {
            family: value
            for family, value in self.slots.items()
            if value.identity in allowed
            or (self.mode == "retain_anchor" and family == "anchor")
        }

    def receive(self, observation):
        expected = {"anchor": (Q(1), Q(0)), "relative": (Q(-1), Q(1))}
        if (
            observation.family not in expected
            or observation.row != expected[observation.family]
        ):
            raise ValueError(
                "This retained representation supports only declared u/v factors"
            )
        if observation.identity not in self.allowed and not (
            self.mode == "retain_anchor" and observation.family == "anchor"
        ):
            return "reject_policy"
        key = (observation.family, observation.revision)
        digest = hashlib.sha256(
            json.dumps(encode(observation), sort_keys=True).encode()
        ).hexdigest()
        if key in self.fingerprints:
            if self.fingerprints[key] != digest:
                raise ValueError("Conflicting same-revision payload")
            return "duplicate"
        if len(self.fingerprints) >= MAX_REVISIONS:
            raise ValueError("Revision history bound exceeded")
        if observation.revision < self.watermarks.get(observation.family, 0):
            return "stale"
        if observation.family not in self.slots and len(self.slots) >= MAX_ENTRIES:
            raise ValueError("Retained input bound exceeded")
        self.fingerprints[key] = digest
        self.watermarks[observation.family] = observation.revision
        if self.mode == "split_summary":
            # Independent canonical u or v summary; no raw Observation object retained.
            payload = FrozenScalar(
                observation.identity,
                observation.family,
                observation.revision,
                observation.value,
                observation.variance,
                observation.acquisition_event,
            )
        else:
            payload = observation
        self.slots[observation.family] = payload
        return "accepted"

    def receive_batch(self, observations):
        if len(observations) > MAX_DELIVERIES:
            raise ValueError("Delivery bound exceeded")
        return [
            {"identity": r.identity, "revision": r.revision, "action": self.receive(r)}
            for r in observations
        ]

    def query(self, query, origin=Q(0), gauge=Q(0)):
        if len(query) != 2:
            raise ValueError("Two-variable query required")
        query = tuple(Q(value) for value in query)
        if not any(query):
            return Result(
                Estimate(Q(0), Q(0)), "deterministic", len(self.slots), frozenset()
            )
        if self.mode == "split_summary":
            coefficients = {"anchor": sum(query), "relative": query[1]}
            if any(
                coefficient and family not in self.slots
                for family, coefficient in coefficients.items()
            ):
                return Result(None, "unobservable", len(self.slots), frozenset())
            mean = variance = Q(0)
            used = set()
            for family, coefficient in coefficients.items():
                if not coefficient:
                    continue
                summary = self.slots[family]
                mean += coefficient * (
                    summary.mean - origin if family == "anchor" else summary.mean
                )
                variance += coefficient**2 * summary.variance
                used.add(summary.identity)
            return Result(
                Estimate(mean, variance), "available", len(self.slots), frozenset(used)
            )
        info = information(self.slots.values(), origin)
        if self.mode == "hard_gauge":
            rank, estimate = pinned_estimate(info, query, gauge)
        else:
            rank, estimate = rank_estimate(info, query)
            if self.mode == "blanket_failure" and rank < 2:
                estimate = None
        return Result(
            estimate,
            "available" if estimate is not None else "unobservable",
            rank,
            frozenset(r.identity for r in self.slots.values()),
        )


def reference(query, allowed, anchor_variance, relative_variance, origin):
    """Independent u=x, v=y-x reference, with a missing-coordinate validity test."""
    u_weight, v_weight = sum(query), query[1]
    anchor = (
        (Q(5), anchor_variance)
        if "A0" in allowed
        else ((Q(-2), anchor_variance / 4) if "A1" in allowed else None)
    )
    if (u_weight and anchor is None) or (v_weight and "R" not in allowed):
        return None
    mean = u_weight * (anchor[0] - origin) if u_weight else Q(0)
    variance = u_weight**2 * anchor[1] if u_weight else Q(0)
    if v_weight:
        mean += v_weight * 3
        variance += v_weight**2 * relative_variance
    return Estimate(mean, variance)


def run_scenario(mode, anchor_variance, relative_variance, shift, gauge):
    if anchor_variance <= 0 or relative_variance <= 0:
        raise ValueError("Positive variances required")
    engine = Engine(mode)
    a0 = Observation("A0", "anchor", 1, (Q(1), Q(0)), Q(5), anchor_variance, 0)
    relative = Observation(
        "R", "relative", 1, (Q(-1), Q(1)), Q(3), relative_variance, 0
    )
    a1 = Observation("A1", "anchor", 2, (Q(1), Q(0)), Q(-2), anchor_variance / 4, 5)
    trace, previous = [], {}
    names = (
        "initial",
        "origin_change",
        "withdraw_anchor",
        "gauge_change",
        "old_replay",
        "new_anchor",
        "withdraw_relative",
        "withdraw_all",
    )
    origin = Q(0)
    frame_epoch = 1
    current_gauge = gauge
    allowed = frozenset(("A0", "R"))
    for event, name in enumerate(names):
        if event >= MAX_STATES:
            raise ValueError("Event bound exceeded")
        admissions = []
        if event == 0:
            engine.policy(1, allowed, event)
            admissions = engine.receive_batch([a0, relative])
        elif event == 1:
            origin, frame_epoch = shift, 2
        elif event == 2:
            allowed = frozenset(("R",))
            engine.policy(2, allowed, event)
        elif event == 3:
            current_gauge = gauge + 11
        elif event == 4:
            admissions = engine.receive_batch([a0, a0])
        elif event == 5:
            allowed = frozenset(("A1", "R"))
            engine.policy(3, allowed, event)
            admissions = engine.receive_batch([a1, a0])
        elif event == 6:
            allowed = frozenset(("A1",))
            engine.policy(4, allowed, event)
        elif event == 7:
            allowed = frozenset()
            engine.policy(5, allowed, event)
        for query_name, query in QUERIES.items():
            result = engine.query(query, origin, current_gauge)
            expected = reference(
                query, allowed, anchor_variance, relative_variance, origin
            )
            estimate = result.estimate
            forbidden = result.processed_ids - allowed
            canonical = (
                None if estimate is None else estimate.mean + origin * sum(query)
            )
            old_estimate, old_canonical = previous.get(query_name, (None, None))
            row = {
                "event": event,
                "name": name,
                "query": query_name,
                "coefficients": query,
                "origin": origin,
                "frame_epoch": frame_epoch,
                "gauge": current_gauge,
                "allowed": allowed,
                "policy_version": engine.policy_version,
                "policy_effective_event": engine.policy_event,
                "policy_learned_event": engine.policy_event,
                "policy_enforced_event": engine.policy_event,
                "admissions": admissions,
                "active_sources": {
                    family: {
                        "identity": value.identity,
                        "revision": value.revision,
                        "acquisition_event": value.acquisition_event,
                    }
                    for family, value in engine.slots.items()
                },
                "result": result,
                "reference": expected,
                "canonical_mean": canonical,
                "false_available": expected is None and estimate is not None,
                "needlessly_unavailable": expected is not None and estimate is None,
                "numerically_wrong": estimate is not None
                and expected is not None
                and estimate != expected,
                "mean_error": None
                if estimate is None or expected is None
                else estimate.mean - expected.mean,
                "variance_error": None
                if estimate is None or expected is None
                else estimate.variance - expected.variance,
                "forbidden_processing": forbidden,
                "forbidden_output": bool(forbidden) and estimate is not None,
                "coordinate_jump": None
                if estimate is None or old_estimate is None
                else estimate.mean - old_estimate.mean,
                "canonical_jump": None
                if canonical is None or old_canonical is None
                else canonical - old_canonical,
                "active_payload_coefficients": len(engine.slots)
                * (2 if mode == "split_summary" else 4),
                "revision_fingerprints": len(engine.fingerprints),
            }
            trace.append(row)
            previous[query_name] = estimate, canonical
    metrics = {
        "mode": mode,
        "anchor_variance": anchor_variance,
        "relative_variance": relative_variance,
        "shift": shift,
        "gauge": gauge,
        "outcomes": len(trace),
        "available": sum(t["result"].estimate is not None for t in trace),
        **{
            key: sum(bool(t[key]) for t in trace)
            for key in (
                "false_available",
                "needlessly_unavailable",
                "numerically_wrong",
                "forbidden_processing",
                "forbidden_output",
            )
        },
        "max_payload_coefficients": max(
            t["active_payload_coefficients"] for t in trace
        ),
        "max_revision_fingerprints": max(t["revision_fingerprints"] for t in trace),
        "max_coordinate_jump": max(
            abs(t["coordinate_jump"]) for t in trace if t["coordinate_jump"] is not None
        ),
        "max_canonical_jump": max(
            abs(t["canonical_jump"]) for t in trace if t["canonical_jump"] is not None
        ),
    }
    return metrics, trace


def grid():
    return itertools.product(
        MODES, (Q(1), Q(100), Q(10**12)), (Q(1, 4), Q(4)), (Q(0), Q(1000)), (Q(0), Q(7))
    )


def encode(value):
    if isinstance(value, Q):
        return str(value)
    if hasattr(value, "__dataclass_fields__"):
        return encode(asdict(value))
    if isinstance(value, dict):
        return {key: encode(item) for key, item in value.items()}
    if isinstance(value, (set, frozenset)):
        return sorted(value)
    if isinstance(value, (tuple, list)):
        return [encode(item) for item in value]
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    metrics, representatives = [], []
    for config in grid():
        metric, trace = run_scenario(*config)
        metrics.append(metric)
        _mode, va, vr, shift, gauge = config
        if (va, vr, shift, gauge) in (
            (Q(1), Q(1, 4), Q(1000), Q(0)),
            (Q(10**12), Q(4), Q(1000), Q(7)),
        ):
            representatives.append({"configuration": metric, "trace": trace})
    rendered = "".join(
        json.dumps(encode(run), sort_keys=True, separators=(",", ":")) + "\n"
        for run in representatives
    )
    (args.output_dir / "representative_runs.jsonl").write_text(rendered)
    totals = {
        mode: {
            key: sum(m[key] for m in metrics if m["mode"] == mode)
            for key in (
                "outcomes",
                "available",
                "false_available",
                "needlessly_unavailable",
                "numerically_wrong",
                "forbidden_processing",
                "forbidden_output",
            )
        }
        for mode in MODES
    }
    root = Path(__file__).resolve().parent
    results = {
        "cycle": "0007",
        "parent_commit": "74ac1dec1edfdaa133773191f1616e5d3a310e40",
        "python": platform.python_version(),
        "randomness": "none",
        "scope": "Exact two-variable query observability with trusted synchronous permissions; no floating-point, nonlinear or physical validation",
        "metrics": metrics,
        "totals": totals,
        "representative_runs": len(representatives),
        "trace_sha256": hashlib.sha256(rendered.encode()).hexdigest(),
        "source_sha256": {
            name: hashlib.sha256((root / name).read_bytes()).hexdigest()
            for name in (
                "baseline.py",
                "observability.py",
                "test_observability.py",
                "cycles/0007-protocol.md",
            )
        },
    }
    (args.output_dir / "results.json").write_text(
        json.dumps(encode(results), sort_keys=True, indent=2) + "\n"
    )
    print(json.dumps(totals, sort_keys=True, indent=2))


if __name__ == "__main__":
    main()
