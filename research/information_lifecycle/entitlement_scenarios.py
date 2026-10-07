"""Cycle 0004 fixtures and evaluator; expected rights never call Ledger helpers."""

from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import platform
from collections import Counter
from dataclasses import asdict, dataclass, is_dataclass
from fractions import Fraction as Q
from pathlib import Path

from entitlements import MODES, Grant, Ledger, Record, Scope, Summary

ALIGN = ("shared-flow", "team", "alignment")
AUDIT = ("shared-flow", "team", "audit")
PRIVATE = ("owner-archive", "owner", "archive")
VALUES = {"a1": 10, "a2": 14, "a3": 100, "a4": 18, "b1": 0}
RECORDS = {key: Record(key, Q(value), int(key[1:])) for key, value in VALUES.items()}
A12 = frozenset({"a1", "a2"})
INITIAL = A12 | {"b1"}
FROZEN = {
    "a12": Summary("a12", A12, Q(2), Q(24)),
    "ab12": Summary("ab12", INITIAL, Q(3), Q(24)),
}
A = Grant("A", 1, "raw", frozenset({"a1", "a2", "a3", "a4"}))
B = Grant("B", 1, "raw", frozenset({"b1"}))
SA = Grant("SA", 1, "summary", frozenset({"a12"}))
SM = Grant("SM", 1, "summary", frozenset({"ab12"}))
SCENARIOS = (
    "future_cutoff",
    "derivative_retention",
    "overlapping_grants",
    "strict_mixed_withdrawal",
    "mixed_without_replay",
    "regrant_gap",
)


@dataclass(frozen=True)
class Step:
    action: str
    args: tuple
    decision: str
    represented: frozenset[str]
    retained_raw: frozenset[str]
    retained_summaries: frozenset[str]
    view: Scope
    required: frozenset[str]
    require_complete: bool


class Fixture:
    """Write explicit expected sets after each event, separate from the candidate."""

    def __init__(self):
        self.steps = []
        self.used = {}
        self.raw = self.summaries = frozenset()

    def add(
        self,
        action,
        args=(),
        decision="queried",
        *,
        used=None,
        raw=None,
        summaries=None,
        view=ALIGN,
        required=frozenset(),
        complete=False,
    ):
        if used is not None:
            self.used[view] = frozenset(used)
        if raw is not None:
            self.raw = frozenset(raw)
        if summaries is not None:
            self.summaries = frozenset(summaries)
        self.steps.append(
            Step(
                action,
                args,
                decision,
                self.used.get(view, frozenset()),
                self.raw,
                self.summaries,
                view,
                frozenset(required),
                complete,
            )
        )


def scenario(name, arrival_order=("a1", "a2", "b1")):
    if name not in SCENARIOS or sorted(arrival_order) != sorted(INITIAL):
        raise ValueError("Unknown scenario/arrival order")
    fixture = Fixture()
    grants = (A, B)
    if name in ("future_cutoff", "derivative_retention"):
        grants += (SA,)
    elif name in ("strict_mixed_withdrawal", "mixed_without_replay"):
        grants += (SM,)
    elif name == "overlapping_grants":
        grants += (Grant("backup-A", 1, "raw", A.resources),)
    fixture.add("install", (ALIGN, 1, grants), "applied")
    arrived = set()
    for identity in arrival_order:
        arrived.add(identity)
        token = ("B" if identity == "b1" else "A", 1)
        fixture.add(
            "raw",
            (ALIGN, token, RECORDS[identity]),
            "stored",
            used=arrived,
            raw=arrived,
        )
        fixture.add("raw", (ALIGN, token, RECORDS[identity]), "duplicate")
    if SA in grants or SM in grants:
        identity = "a12" if SA in grants else "ab12"
        fixture.add(
            "freeze",
            (ALIGN, identity, FROZEN[identity].dependencies),
            "stored",
            summaries={identity},
        )
    fixture.add("capture", ("old", ALIGN), "captured")

    if name == "future_cutoff":
        narrowed = Grant("A", 2, "raw", A12)
        fixture.add("install", (ALIGN, 2, (narrowed, B, SA)), "applied")
        fixture.add("cache", ("old", ALIGN), "denied")
        fixture.add("raw", (ALIGN, ("A", 1), RECORDS["a3"]), "denied")
        fixture.add("raw", (ALIGN, ("A", 2), RECORDS["a3"]), "denied")
        fixture.add("raw", (ALIGN, ("A", 2), RECORDS["a1"]), "duplicate")
    elif name == "derivative_retention":
        fixture.add("install", (ALIGN, 2, (B, SA)), "applied", raw={"b1"})
        fixture.add("cache", ("old", ALIGN), "denied")
        fixture.add("raw", (ALIGN, ("A", 1), RECORDS["a1"]), "denied")
        fixture.add("freeze", (ALIGN, "a12", A12), "denied")
        retained_only = Grant("SA", 2, "summary", SA.resources, frozenset({"retain"}))
        fixture.add("install", (ALIGN, 3, (B, retained_only)), "applied", used={"b1"})
        fixture.add("summary", (ALIGN, ("SA", 2), FROZEN["a12"]), "duplicate")
        fixture.add("install", (ALIGN, 4, (B,)), "applied", summaries=set())
    elif name == "overlapping_grants":
        archive = Grant("archive-A", 1, "raw", A12)
        audit = Grant("audit-A", 1, "raw", A12)
        fixture.add(
            "install", (PRIVATE, 1, (archive,)), "applied", view=PRIVATE, used=A12
        )
        fixture.add("install", (AUDIT, 1, (audit,)), "applied", view=AUDIT, used=A12)
        fixture.add("capture", ("private", PRIVATE), "captured", view=PRIVATE)
        fixture.add("capture", ("audit", AUDIT), "captured", view=AUDIT)
        backup = grants[-1]
        fixture.add("install", (ALIGN, 2, (B, backup)), "applied")
        fixture.add("cache", ("old", ALIGN), "denied")
        fixture.add("cache", ("private", PRIVATE), "accepted", view=PRIVATE)
        fixture.add("install", (ALIGN, 3, (B,)), "applied", used={"b1"})
        fixture.add("cache", ("audit", AUDIT), "accepted", view=AUDIT)
        fixture.add("probe", view=PRIVATE)
        fixture.add("probe")
    elif name == "strict_mixed_withdrawal":
        fixture.add(
            "install",
            (ALIGN, 2, (B,)),
            "applied",
            used={"b1"},
            raw={"b1"},
            summaries=set(),
        )
        fixture.add("cache", ("old", ALIGN), "denied")
        fixture.add("summary", (ALIGN, ("SM", 1), FROZEN["ab12"]), "denied")
        fixture.add("raw", (ALIGN, ("A", 1), RECORDS["a2"]), "denied")
    elif name == "mixed_without_replay":
        fixture.add("install", (ALIGN, 2, (SM,)), "applied", raw=set())
        replay_b = Grant("B", 2, "raw", B.resources)
        fixture.add(
            "install",
            (ALIGN, 3, (replay_b,)),
            "applied",
            used=set(),
            summaries=set(),
            required={"b1"},
        )
        fixture.add("cache", ("old", ALIGN), "denied", required={"b1"})
        fixture.add(
            "summary", (ALIGN, ("SM", 1), FROZEN["ab12"]), "denied", required={"b1"}
        )
        fixture.add(
            "raw",
            (ALIGN, ("B", 2), RECORDS["b1"]),
            "stored",
            used={"b1"},
            raw={"b1"},
            required={"b1"},
        )
    else:
        required = {"a3", "a4", "b1"}
        fixture.add("install", (ALIGN, 2, (B,)), "applied", used={"b1"}, raw={"b1"})
        new_a = Grant("A", 2, "raw", frozenset({"a3", "a4"}))
        fixture.add("install", (ALIGN, 3, (new_a, B)), "applied", required=required)
        fixture.add("cache", ("old", ALIGN), "denied", required=required)
        fixture.add(
            "raw", (ALIGN, ("A", 1), RECORDS["a1"]), "denied", required=required
        )
        fixture.add(
            "raw", (ALIGN, ("A", 1), RECORDS["a3"]), "denied", required=required
        )
        fixture.add(
            "raw",
            (ALIGN, ("A", 2), RECORDS["a4"]),
            "stored",
            used={"a4", "b1"},
            raw={"a4", "b1"},
            required=required,
        )
        fixture.add(
            "raw", (ALIGN, ("A", 2), RECORDS["a4"]), "duplicate", required=required
        )
        fixture.add("probe", required=required, complete=True)
        fixture.add(
            "raw", (ALIGN, ("A", 2), RECORDS["a1"]), "denied", required=required
        )
        fixture.add(
            "raw",
            (ALIGN, ("A", 2), RECORDS["a3"]),
            "stored",
            used=required,
            raw=required,
            required=required,
            complete=True,
        )
        fixture.add(
            "install", (ALIGN, 1, grants), "stale", required=required, complete=True
        )
    if len(fixture.steps) > 80:
        raise ValueError("Fixture exceeds event budget")
    return fixture.steps


def oracle(step):
    """Only predeclared expected identities enter the independent arithmetic mean."""
    count = len(step.represented)
    missing = step.required - step.represented
    status = (
        "required_coverage_missing"
        if missing and step.require_complete
        else "available"
        if count
        else "no_evidence"
    )
    return {
        "decision": step.decision,
        "represented": step.represented,
        "missing": missing,
        "mean": Q(sum(VALUES[key] for key in step.represented), count)
        if status == "available"
        else None,
        "variance": Q(1, count) if status == "available" else None,
        "status": status,
        "retained_raw": step.retained_raw,
        "retained_summaries": step.retained_summaries,
    }


def run_scenario(name, mode="scoped", order=("a1", "a2", "b1")):
    ledger = Ledger(mode)
    cache = {}
    trace = []
    previous_means = {}
    policy_effective_times = {}
    for index, step in enumerate(scenario(name, order)):
        if step.action == "install":
            decision = ledger.install(*step.args)
        elif step.action == "raw":
            decision = ledger.receive_raw(*step.args)
        elif step.action == "summary":
            decision = ledger.receive_summary(*step.args)
        elif step.action == "freeze":
            decision = ledger.freeze(*step.args)
        elif step.action == "capture":
            key, scope = step.args
            cache[key] = ledger.query(scope)
            decision = "captured"
        elif step.action == "cache":
            key, scope = step.args
            decision = (
                "accepted" if ledger.accept_cached(scope, cache[key]) else "denied"
            )
        else:
            decision = "queried"
        result = ledger.query(step.view, step.required, step.require_complete)
        actual = {
            "decision": decision,
            "represented": result.represented,
            "missing": result.missing,
            "mean": result.mean,
            "variance": result.variance,
            "status": result.status,
            "retained_raw": frozenset(ledger.raw),
            "retained_summaries": frozenset(ledger.summaries),
        }
        expected = oracle(step)
        violations = [key for key in expected if actual[key] != expected[key]]
        previous = previous_means.get(step.view)
        jump = (
            abs(result.mean - previous)
            if result.mean is not None and previous is not None
            else None
        )
        previous_means[step.view] = result.mean
        event_time = 10 + index
        policy_event = None
        if step.action == "install":
            policy_scope, revision, _ = step.args
            effective = policy_effective_times.setdefault(
                (policy_scope, revision), event_time
            )
            policy_event = {
                "issuer": "fixture-policy-authority",
                "scope": policy_scope,
                "revision": revision,
                "effective": effective,
                "learned": event_time,
                "enforced": event_time if decision == "applied" else None,
                "disposition": decision,
            }
        trace.append(
            {
                "event": index,
                "time": event_time,
                "action": step.action,
                "args": step.args,
                "view": step.view,
                "required": step.required,
                "require_complete": step.require_complete,
                "expected": expected,
                "actual": actual,
                "violations": violations,
                "policy_revision": result.policy_revision,
                "store_revision": result.store_revision,
                "mean_jump_same_view": jump,
                "policy_event": policy_event,
                "grant_counts": {
                    "|".join(scope): len(grants)
                    for scope, (_, grants) in ledger.policies.items()
                },
            }
        )
    return {
        "scenario": name,
        "mode": mode,
        "arrival_order": order,
        "events": len(trace),
        "violating_events": sum(bool(row["violations"]) for row in trace),
        "violations_by_field": dict(
            Counter(v for row in trace for v in row["violations"])
        ),
        "first_violation": next(
            (row["event"] for row in trace if row["violations"]), None
        ),
        "unavailable_queries": sum(row["actual"]["mean"] is None for row in trace),
        "max_same_view_mean_jump": max(
            (row["mean_jump_same_view"] or Q(0) for row in trace), default=Q(0)
        ),
        "max_retained_objects": max(
            len(row["actual"]["retained_raw"])
            + len(row["actual"]["retained_summaries"])
            for row in trace
        ),
    }, trace


def encode(value):
    if isinstance(value, Q):
        return str(value)
    if is_dataclass(value):
        return encode(asdict(value))
    if isinstance(value, dict):
        return {key: encode(item) for key, item in value.items()}
    if isinstance(value, (set, frozenset)):
        return [encode(item) for item in sorted(value)]
    if isinstance(value, (tuple, list)):
        return [encode(item) for item in value]
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    metrics = []
    for name in SCENARIOS:
        with (args.output_dir / f"{name}.jsonl").open("w") as handle:
            for mode in MODES:
                for order in itertools.permutations(("a1", "a2", "b1")):
                    result, trace = run_scenario(name, mode, order)
                    metrics.append(result)
                    if order == ("a1", "a2", "b1"):
                        for row in trace:
                            handle.write(
                                json.dumps(encode(row | {"mode": mode}), sort_keys=True)
                                + "\n"
                            )
    root = Path(__file__).resolve().parent
    document = {
        "cycle": "0004",
        "parent_commit": "a563f1c8b0c3ffeed22087a8d3a89393f4b52b4d",
        "python": platform.python_version(),
        "scope": "Exact scalar inference with trusted synchronous policy events; no distributed or hardware validation",
        "values": VALUES,
        "noise_variance": "1",
        "metrics": metrics,
        "source_sha256": {
            name: hashlib.sha256((root / name).read_bytes()).hexdigest()
            for name in (
                "entitlements.py",
                "entitlement_scenarios.py",
                "test_entitlements.py",
                "cycles/0004-protocol.md",
            )
        },
    }
    (args.output_dir / "results.json").write_text(
        json.dumps(encode(document), indent=2, sort_keys=True) + "\n"
    )
    print(
        json.dumps(
            {
                mode: {
                    "runs": sum(m["mode"] == mode for m in metrics),
                    "runs_with_violations": sum(
                        m["mode"] == mode and m["violating_events"] > 0 for m in metrics
                    ),
                    "violating_events": sum(
                        m["violating_events"] for m in metrics if m["mode"] == mode
                    ),
                }
                for mode in MODES
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
