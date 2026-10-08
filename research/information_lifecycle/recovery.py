"""Cycle 0008: exact recovery from unequal input histories; research only."""

from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import platform
from dataclasses import asdict, dataclass
from fractions import Fraction as Q
from pathlib import Path

from baseline import ScalarGaussian

MODEL = "independent-static-scalar-v1"
MODES = (
    "manifest_checkpoint",
    "raw_replay",
    "horizon_warm",
    "output_seed",
    "hold_output",
)
MAX_ENTRIES, MAX_DELIVERIES, MAX_STATES = 16, 32, 32
BASE_IDS = frozenset(("d", "a1", "b1"))
TARGET_IDS = BASE_IDS | {"a2"}
WARM_IDS = frozenset(("d", "a1", "b2"))


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


def digest(value):
    return hashlib.sha256(
        json.dumps(encode(value), sort_keys=True).encode()
    ).hexdigest()


@dataclass(frozen=True)
class Record:
    identity: str
    origin: str
    value: Q
    variance: Q
    acquired: int

    def __post_init__(self):
        object.__setattr__(self, "value", Q(self.value))
        object.__setattr__(self, "variance", Q(self.variance))
        if (
            not self.identity
            or not self.origin
            or self.variance <= 0
            or type(self.acquired) is not int
            or not 0 <= self.acquired < MAX_STATES
        ):
            raise ValueError("Invalid source record")

    @property
    def information(self):
        return Q(1) / self.variance, self.value / self.variance


def manifest(records):
    records = tuple(records)
    if len(records) > MAX_ENTRIES or len({r.identity for r in records}) != len(records):
        raise ValueError("Invalid manifest size or duplicate identity")
    return tuple(sorted((r.identity, digest(r)) for r in records))


@dataclass(frozen=True)
class Checkpoint:
    model: str
    inputs: tuple[tuple[str, str], ...]
    precision: Q
    information: Q

    def __post_init__(self):
        object.__setattr__(self, "inputs", tuple(tuple(p) for p in self.inputs))
        object.__setattr__(self, "precision", Q(self.precision))
        object.__setattr__(self, "information", Q(self.information))
        if (
            self.precision <= 0
            or not self.inputs
            or len(self.inputs) > MAX_ENTRIES
            or len(dict(self.inputs)) != len(self.inputs)
        ):
            raise ValueError("Invalid checkpoint")

    @classmethod
    def make(cls, records):
        records = tuple(records)
        pairs = [r.information for r in records]
        return cls(
            MODEL,
            manifest(records),
            sum((p[0] for p in pairs), Q(0)),
            sum((p[1] for p in pairs), Q(0)),
        )

    @property
    def estimate(self):
        return ScalarGaussian(self.information / self.precision, 1 / self.precision)


class Custodian:
    """An authorized synthetic archive, independent of producer reachability."""

    def __init__(self, name, records):
        records = tuple(records)
        manifest(records)  # Validate uniqueness and resource bound before mutation.
        self.name = name
        self.records = {r.identity: r for r in records}

    def acknowledgments(self):
        return {
            identity: {
                "custodian": self.name,
                "record_hash": digest(r),
                "origin": r.origin,
            }
            for identity, r in self.records.items()
        }

    def replay(self, identities, allowed):
        requested = frozenset(identities)
        if len(requested) > MAX_ENTRIES:
            raise ValueError("Replay request too large")
        return tuple(
            self.records[k]
            for k in sorted(requested & frozenset(allowed))
            if k in self.records
        )


class Engine:
    def __init__(
        self,
        mode,
        target,
        checkpoint,
        checkpoint_id,
        output,
        warm,
        allowed,
        policy_version,
        policy_event,
    ):
        if mode not in MODES:
            raise ValueError("Unknown method")
        self.mode = mode
        self.target, self.target_version = {}, 0
        self.ledger, self.received_events = {}, {}
        self.base = None
        self.allowed, self.policy_version = frozenset(), 0
        self.policy_event = 0
        self.set_target(1, target)
        self.policy(policy_version, allowed, policy_event)
        if mode == "manifest_checkpoint":
            if (
                checkpoint.model != MODEL
                or digest(checkpoint) != checkpoint_id
                or any(self.target.get(k) != v for k, v in checkpoint.inputs)
            ):
                raise ValueError("Checkpoint/model/manifest binding mismatch")
            if set(dict(checkpoint.inputs)) <= self.allowed:
                self.base = checkpoint
        elif mode in ("output_seed", "hold_output"):
            # Both numbers suffice for this scalar likelihood, but overlap still matters.
            if mode == "hold_output" or set(self.target) <= self.allowed:
                self.base = Checkpoint(
                    MODEL,
                    tuple(sorted(self.target.items())),
                    1 / output.variance,
                    output.mean / output.variance,
                )
        elif mode == "horizon_warm":
            for r in warm:
                if r.identity in self.allowed:
                    self.ledger[r.identity] = r
                    self.received_events[r.identity] = 1
        self.peak_entries = len(self.ledger) + bool(self.base)

    def set_target(self, version, target):
        target = tuple(tuple(p) for p in target)
        if (
            type(version) is not int
            or version <= self.target_version
            or len(target) > MAX_ENTRIES
            or len(dict(target)) != len(target)
        ):
            raise ValueError("Invalid target revision/size")
        updated = dict(target)
        if any(updated.get(k) != v for k, v in self.target.items()):
            raise ValueError("This fixture only extends immutable target identities")
        self.target, self.target_version = updated, version

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
        if self.mode != "hold_output":
            self.ledger = {k: r for k, r in self.ledger.items() if k in allowed}
            self.received_events = {
                k: e for k, e in self.received_events.items() if k in allowed
            }
            if self.base is not None and not set(dict(self.base.inputs)) <= allowed:
                self.base = None

    def receive(self, record, event, live=False):
        if type(event) is not int or not 0 <= event < MAX_STATES:
            raise ValueError("Invalid delivery event")
        if record.identity not in self.allowed:
            return "reject_policy"
        if self.mode == "hold_output" or (self.mode == "horizon_warm" and not live):
            return "no_replay_requested" if not live else "ignored_new_data"
        if record.identity not in self.target:
            return "outside_target"
        if digest(record) != self.target[record.identity]:
            raise ValueError("Conflicting same-ID source payload")
        if (
            self.mode == "manifest_checkpoint"
            and self.base is not None
            and record.identity in dict(self.base.inputs)
        ):
            return "covered_by_checkpoint"
        if record.identity in self.ledger:
            return "duplicate"
        if len(self.ledger) + bool(self.base) >= MAX_ENTRIES:
            raise ValueError("Retained entry bound exceeded")
        self.ledger[record.identity] = record
        self.received_events[record.identity] = event
        self.peak_entries = max(self.peak_entries, len(self.ledger) + bool(self.base))
        return "accepted"

    def batch(self, records, event, live=False):
        if len(records) > MAX_DELIVERIES:
            raise ValueError("Delivery bound exceeded")
        return [
            {
                "id": r.identity,
                "origin": r.origin,
                "acquired": r.acquired,
                "received": event,
                "action": self.receive(r, event, live),
            }
            for r in records
        ]

    def snapshot(self):
        # Audit weights follow operations; they are never used for candidate admission/fusion.
        weights = {}
        precision = information = Q(0)
        if self.base is not None:
            precision, information = self.base.precision, self.base.information
            weights.update(dict.fromkeys(dict(self.base.inputs), 1))
        for identity, record in self.ledger.items():
            h, eta = record.information
            precision += h
            information += eta
            weights[identity] = weights.get(identity, 0) + 1
        required = set(self.target) & self.allowed
        missing = required - set(weights)
        exact_coverage = set(weights) == required
        can_publish = precision > 0 and (
            exact_coverage or self.mode in ("horizon_warm", "hold_output")
        )
        estimate = (
            ScalarGaussian(information / precision, 1 / precision)
            if can_publish
            else None
        )
        return {
            "estimate": estimate,
            "weights": weights,
            "precision": precision,
            "information": information,
            "missing": missing,
            "base_inputs": frozenset()
            if self.base is None
            else frozenset(dict(self.base.inputs)),
            "ledger_ids": frozenset(self.ledger),
            "receipt_events": dict(self.received_events),
            "entries": len(self.ledger) + bool(self.base),
            "peak_entries": self.peak_entries,
            "numeric_coefficients": 2 * (len(self.ledger) + bool(self.base)),
        }


def records_for(values, va, vb):
    if values not in ("contrast", "equal") or va <= 0 or vb <= 0:
        raise ValueError("Invalid configuration")
    vals = {"d": 0, "a1": 2, "a2": 10, "b1": -4, "b2": 8, "b3": -6}
    return {
        k: Record(
            k,
            "D" if k == "d" else k[0].upper(),
            Q(z if values == "contrast" else 0),
            Q(1) if k == "d" else (va if k[0] == "a" else vb),
            7 if k == "b3" else 0,
        )
        for k, z in vals.items()
    }


def oracle(records, identities):
    """Independent normalized-weight reference; never passed to replacement workers."""
    selected = [records[k] for k in sorted(identities)]
    if not selected:
        return None
    inverse = [1 / r.variance for r in selected]
    total = sum(inverse, Q(0))
    mean = sum(
        (r.value * (w / total) for r, w in zip(selected, inverse, strict=True)), Q(0)
    )
    return ScalarGaussian(mean, 1 / total)


def run_scenario(mode, values, va, vb, archive_case, withdrawal, order):
    if archive_case not in ("complete", "missing_base", "missing_tail"):
        raise ValueError("Unknown archive case")
    if withdrawal not in ("none", "before", "during", "after") or order not in (
        "forward",
        "reverse",
    ):
        raise ValueError("Unknown schedule")
    records = records_for(values, va, vb)
    initial_ids = set(records) - {"b3"}
    absent = {"complete": None, "missing_base": "a1", "missing_tail": "a2"}[
        archive_case
    ]
    m1 = Custodian("M1", [records[k] for k in sorted(initial_ids) if k != absent])
    m2 = Custodian("M2", [records[absent or "a1"]])
    checkpoint = Checkpoint.make(records[k] for k in sorted(BASE_IDS))
    output = oracle(records, TARGET_IDS)
    allowed, target = frozenset(initial_ids), TARGET_IDS
    version, policy_event = 1, 0
    trace, previous = [], None
    engine = None
    replay_order = sorted(m1.records, reverse=order == "reverse")
    midpoint = len(replay_order) // 2
    names = (
        "old_result",
        "restart",
        "replay_first",
        "policy_during",
        "replay_second",
        "policy_after",
        "retry",
        "new_b3",
        "duplicates",
        "m2_returns",
        "withdraw_a2",
        "final_replay",
    )
    for event, name in enumerate(names):
        admissions = []
        withdraw = (
            (event == 1 and withdrawal == "before")
            or (event == 3 and withdrawal == "during")
            or (event == 5 and withdrawal == "after")
        )
        if withdraw:
            allowed -= {"b1"}
            version, policy_event = version + 1, event
            if engine is not None:
                engine.policy(version, allowed, event)
        if event == 0:
            h, eta = (
                Checkpoint.make(records[k] for k in sorted(TARGET_IDS)).precision,
                output.mean / output.variance,
            )
            snapshot = {
                "estimate": output,
                "weights": dict.fromkeys(TARGET_IDS, 1),
                "precision": h,
                "information": eta,
                "missing": set(),
                "base_inputs": set(),
                "ledger_ids": TARGET_IDS,
                "receipt_events": {},
                "entries": len(TARGET_IDS),
                "peak_entries": len(TARGET_IDS),
                "numeric_coefficients": 2 * len(TARGET_IDS),
            }
        else:
            if event == 1:
                engine = Engine(
                    mode,
                    manifest(records[k] for k in sorted(target)),
                    checkpoint,
                    digest(checkpoint),
                    output,
                    [records[k] for k in sorted(WARM_IDS)],
                    allowed,
                    version,
                    policy_event,
                )
            if event in (2, 4, 6, 8, 11):
                ids = (
                    replay_order[:midpoint]
                    if event == 2
                    else (replay_order[midpoint:] if event == 4 else replay_order)
                )
                batch = list(m1.replay(ids, allowed))
                if order == "reverse":
                    batch.reverse()
                if batch:
                    batch.append(batch[0])
                admissions += engine.batch(batch, event)
            if event == 7:
                target = target | {"b3"}
                allowed = allowed | {"b3"}
                version, policy_event = version + 1, event
                engine.set_target(2, manifest(records[k] for k in sorted(target)))
                engine.policy(version, allowed, event)
                admissions += engine.batch([records["b3"]], event, live=True)
            if event in (8, 11):
                admissions += engine.batch(
                    [records["b3"], records["b3"]], event, live=True
                )
            if event == 9:
                admissions += engine.batch(m2.replay(m2.records, allowed), event)
            if event == 10:
                allowed -= {"a2"}
                version, policy_event = version + 1, event
                engine.policy(version, allowed, event)
            snapshot = engine.snapshot()
        expected_ids = target & allowed
        expected = oracle(records, expected_ids)
        actual = snapshot["estimate"]
        weights = snapshot["weights"]
        # Independent evaluator reconstructs the actual computation from original inputs.
        assert snapshot["precision"] == sum(
            (Q(w) / records[k].variance for k, w in weights.items()), Q(0)
        )
        assert snapshot["information"] == sum(
            (Q(w) * records[k].value / records[k].variance for k, w in weights.items()),
            Q(0),
        )
        forbidden = set(weights) - allowed
        trace.append(
            {
                "event": event,
                "name": name,
                "result": snapshot,
                "reference": expected,
                "required_ids": expected_ids,
                "target_version": 1 if event < 7 else 2,
                "allowed_ids": allowed,
                "policy_version": version,
                "policy_effective_event": policy_event,
                "policy_learned_event": policy_event,
                "policy_enforced_event": policy_event,
                "publication_event": event if actual else None,
                "admissions": admissions,
                "new_data_count": 1 if event == 7 else 0,
                "wrong_answer": actual is not None and actual != expected,
                "coverage_mismatch": actual is not None
                and set(weights) != expected_ids,
                "repeated_influence": actual is not None
                and any(w != 1 for w in weights.values()),
                "forbidden_processing": forbidden,
                "forbidden_output": bool(forbidden) and actual is not None,
                "mean_error": None if actual is None else actual.mean - expected.mean,
                "variance_error": None
                if actual is None
                else actual.variance - expected.variance,
                "mean_jump": None
                if actual is None or previous is None
                else actual.mean - previous.mean,
            }
        )
        previous = actual
    metric = {
        "mode": mode,
        "values": values,
        "va": va,
        "vb": vb,
        "archive_case": archive_case,
        "withdrawal": withdrawal,
        "order": order,
        "outcomes": len(trace),
        "available": sum(t["result"]["estimate"] is not None for t in trace),
        **{
            k: sum(bool(t[k]) for t in trace)
            for k in (
                "wrong_answer",
                "coverage_mismatch",
                "repeated_influence",
                "forbidden_processing",
                "forbidden_output",
            )
        },
        "first_recovered_event": next(
            (
                t["event"]
                for t in trace[1:]
                if t["result"]["estimate"] is not None
                and not any(
                    t[k]
                    for k in (
                        "wrong_answer",
                        "coverage_mismatch",
                        "repeated_influence",
                        "forbidden_output",
                    )
                )
            ),
            None,
        ),
        "retry_available": trace[6]["result"]["estimate"] is not None,
        "m2_available": trace[9]["result"]["estimate"] is not None,
        "peak_replacement_entries": max(t["result"]["peak_entries"] for t in trace[1:]),
        "max_mean_jump": max(
            abs(t["mean_jump"]) for t in trace if t["mean_jump"] is not None
        ),
        "total_variation": sum(
            (abs(t["mean_jump"]) for t in trace if t["mean_jump"] is not None), Q(0)
        ),
    }
    context = {
        "durable_horizon_old": {"D": 1},
        "durable_horizon_standby": {"D": 1},
        "checkpoint": checkpoint,
        "checkpoint_id": digest(checkpoint),
        "published_manifest": manifest(records[k] for k in sorted(TARGET_IDS)),
        "old_output": output,
        "warm_output": oracle(records, WARM_IDS),
        "m1_acknowledgments": m1.acknowledgments(),
        "m2_acknowledgments": m2.acknowledgments(),
        "m2_reachable_event": 9,
        "records": records,
    }
    return metric, trace, context


def grid():
    return itertools.product(
        MODES,
        ("contrast", "equal"),
        (Q(1), Q(4)),
        (Q(1), Q(4)),
        ("complete", "missing_base", "missing_tail"),
        ("none", "before", "during", "after"),
        ("forward", "reverse"),
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    metrics, representatives = [], []
    for config in grid():
        metric, trace, context = run_scenario(*config)
        metrics.append(metric)
        if (
            metric["va"] == metric["vb"] == 1
            and metric["order"] == "forward"
            and (
                (
                    metric["values"] == "contrast"
                    and (metric["archive_case"], metric["withdrawal"])
                    in (
                        ("complete", "none"),
                        ("missing_base", "none"),
                        ("missing_base", "during"),
                        ("missing_tail", "before"),
                    )
                )
                or (
                    metric["values"] == "equal"
                    and metric["archive_case"] == "complete"
                    and metric["withdrawal"] == "after"
                )
            )
        ):
            representatives.append(
                {"configuration": metric, "trace": trace, "context": context}
            )
    rendered = "".join(
        json.dumps(encode(r), sort_keys=True, separators=(",", ":")) + "\n"
        for r in representatives
    )
    (args.output_dir / "representative_runs.jsonl").write_text(rendered)
    totals = {
        mode: {
            k: sum(m[k] for m in metrics if m["mode"] == mode)
            for k in (
                "outcomes",
                "available",
                "wrong_answer",
                "coverage_mismatch",
                "repeated_influence",
                "forbidden_processing",
                "forbidden_output",
            )
        }
        for mode in MODES
    }
    root = Path(__file__).resolve().parent
    results = {
        "cycle": "0008",
        "parent_commit": "1f373c6f4606c0157bbb993f2959bce683186589",
        "python": platform.python_version(),
        "randomness": "none",
        "scope": "Exact scalar checkpoint/manifest recovery with synchronous trusted policies; no distributed crash or physical guarantee",
        "metrics": metrics,
        "totals": totals,
        "representative_runs": len(representatives),
        "traces_sha256": hashlib.sha256(rendered.encode()).hexdigest(),
        "source_sha256": {
            name: hashlib.sha256((root / name).read_bytes()).hexdigest()
            for name in (
                "baseline.py",
                "recovery.py",
                "test_recovery.py",
                "cycles/0008-protocol.md",
            )
        },
    }
    (args.output_dir / "results.json").write_text(
        json.dumps(encode(results), sort_keys=True, indent=2) + "\n"
    )
    print(json.dumps(totals, sort_keys=True, indent=2))


if __name__ == "__main__":
    main()
