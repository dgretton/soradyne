"""Cycle 0009: finite retained information, withdrawal and explicit reduced answers."""

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

MODES = (
    "atomic",
    "by_source",
    "mixed",
    "honest_subset",
    "coarse_complete",
    "hold_mixed",
)
STRICT = frozenset(("atomic", "by_source", "mixed"))
HISTORICAL = frozenset(("a1", "a2", "b1", "b2"))
MAX_ENTRIES, MAX_TIMES, MAX_BATCH = 16, 32, 8


def encode(value):
    if isinstance(value, Q):
        return str(value)
    if hasattr(value, "__dataclass_fields__"):
        return encode(asdict(value))
    if isinstance(value, dict):
        return {k: encode(v) for k, v in value.items()}
    if isinstance(value, (set, frozenset)):
        return sorted(value)
    if isinstance(value, (list, tuple)):
        return [encode(v) for v in value]
    return value


def check_time(time):
    if type(time) is not int or not 0 <= time < MAX_TIMES:
        raise ValueError("Time outside bounded fixture")


@dataclass(frozen=True)
class Record:
    identity: str
    value: Q
    variance: Q
    acquired: int = 0

    def __post_init__(self):
        object.__setattr__(self, "value", Q(self.value))
        object.__setattr__(self, "variance", Q(self.variance))
        check_time(self.acquired)
        if not self.identity or self.variance <= 0:
            raise ValueError("Invalid observation")


@dataclass(frozen=True)
class Block:
    identity: str
    sources: frozenset[str]
    precision: Q
    information: Q
    created: int
    expires: int

    def __post_init__(self):
        object.__setattr__(self, "sources", frozenset(self.sources))
        object.__setattr__(self, "precision", Q(self.precision))
        object.__setattr__(self, "information", Q(self.information))
        check_time(self.created)
        check_time(self.expires)
        if (
            not self.identity
            or not self.sources
            or len(self.sources) > MAX_ENTRIES
            or self.precision <= 0
            or self.expires <= self.created
        ):
            raise ValueError("Invalid retained block")

    @classmethod
    def make(cls, identity, records, created, expires):
        records = tuple(records)
        if len(records) != len({r.identity for r in records}):
            raise ValueError("Duplicate source in block")
        return cls(
            identity,
            frozenset(r.identity for r in records),
            sum((1 / r.variance for r in records), Q(0)),
            sum((r.value / r.variance for r in records), Q(0)),
            created,
            expires,
        )


class Archive:
    def __init__(self, records, expires):
        check_time(expires)
        records = tuple(records)
        if len(records) > MAX_ENTRIES or len(records) != len(
            {r.identity for r in records}
        ):
            raise ValueError("Invalid archive size")
        self.records = {r.identity: r for r in records}
        self.expires = expires
        self.time = -1

    def advance(self, time):
        check_time(time)
        if time < self.time:
            raise ValueError("Archive time reversed")
        self.time = time
        if time >= self.expires:
            self.records.clear()

    def read(self, identities, allowed, time):
        # Read at completion, with no request-time copy or retention pin.
        self.advance(time)
        requested = frozenset(identities)
        if len(requested) > MAX_ENTRIES:
            raise ValueError("Oversized replay")
        return tuple(
            self.records[k]
            for k in sorted(requested & frozenset(allowed))
            if k in self.records
        )


class Engine:
    def __init__(self, mode, historical, datum, expires):
        if mode not in MODES:
            raise ValueError("Unknown method")
        check_time(expires)
        historical = tuple(historical)
        if (
            len(historical) + 1 > MAX_ENTRIES
            or len(historical) != len({r.identity for r in historical})
            or datum.identity in {r.identity for r in historical}
        ):
            raise ValueError("Invalid local input set")
        self.mode, self.expires = mode, expires
        self.local = {r.identity: r for r in historical}
        self.blocks = {"datum": Block.make("datum", [datum], 0, 16)}
        self.target = frozenset(self.local) | {datum.identity}
        self.derivative_deadlines = dict.fromkeys(self.local, expires) | {
            datum.identity: 16
        }
        self.allowed = self.target
        self.policy_version, self.policy_time, self.time = 1, 0, 0
        self.peak_entries = self.entries()

    def entries(self):
        return len(self.local) + len(self.blocks)

    def covered(self):
        return frozenset(self.local) | frozenset(
            k for b in self.blocks.values() for k in b.sources
        )

    def advance(self, time):
        check_time(time)
        if time < self.time:
            raise ValueError("Engine time reversed")
        self.time = time
        expired = sorted(k for k, b in self.blocks.items() if time >= b.expires)
        self.blocks = {k: b for k, b in self.blocks.items() if time < b.expires}
        return expired

    def freeze(self):
        if self.time != 1 or not self.local:
            raise ValueError("Freeze once at active-window close")
        groups = {}
        for record in self.local.values():
            key = (
                record.identity
                if self.mode == "atomic"
                else (
                    "mixed"
                    if self.mode in ("mixed", "hold_mixed")
                    else record.identity[0]
                )
            )
            groups.setdefault(key, []).append(record)
        prepared = {
            key: Block.make(key, records, 1, self.expires)
            for key, records in groups.items()
        }
        if len(self.blocks) + len(prepared) > MAX_ENTRIES:
            raise ValueError("Retained entry bound exceeded")
        # Atomic representation replacement; peak counts exclude construction scratch.
        self.blocks, self.local = self.blocks | prepared, {}
        self.peak_entries = max(self.peak_entries, self.entries())

    def policy(self, version, allowed, time):
        check_time(time)
        allowed = frozenset(allowed)
        if (
            type(version) is not int
            or version <= self.policy_version
            or len(allowed) > MAX_ENTRIES
            or time != self.time
        ):
            raise ValueError("Invalid policy revision or time")
        self.allowed, self.policy_version, self.policy_time = allowed, version, time
        dropped = []
        if self.mode != "hold_mixed":
            self.local = {k: r for k, r in self.local.items() if k in allowed}
            dropped = sorted(
                k for k, b in self.blocks.items() if not b.sources <= allowed
            )
            self.blocks = {k: b for k, b in self.blocks.items() if b.sources <= allowed}
        return dropped

    def admit(self, block):
        if self.time < block.created:
            return "reject_future"
        if self.time >= block.expires:
            return "reject_expired"
        if not block.sources <= self.allowed:
            return "reject_policy"
        if not block.sources <= self.target:
            return "outside_target"
        if any(block.expires > self.derivative_deadlines[k] for k in block.sources):
            return "reject_deadline_change"
        if block.identity in self.blocks:
            if block != self.blocks[block.identity]:
                raise ValueError("Conflicting same-ID block")
            return "duplicate"
        if block.sources & self.covered():
            raise ValueError("Overlapping block would repeat evidence")
        if self.entries() >= MAX_ENTRIES:
            raise ValueError("Retained entry bound exceeded")
        self.blocks[block.identity] = block
        self.peak_entries = max(self.peak_entries, self.entries())
        return "accepted"

    def rebuild(self, records, required):
        if len(records) > MAX_BATCH:
            raise ValueError("Replay batch bound exceeded")
        if self.time >= self.expires:
            return "summary_deadline", None
        unique = {}
        for r in records:
            if r.identity not in self.allowed:
                return "reject_policy", None
            if r.identity in unique and unique[r.identity] != r:
                raise ValueError("Conflicting duplicate replay")
            unique[r.identity] = r
        required = frozenset(required)
        if set(unique) != required or required & self.covered():
            return "missing_or_wrong_coverage", None
        block = Block.make(
            "rebuild:" + ",".join(sorted(required)),
            unique.values(),
            self.time,
            self.expires,
        )
        return self.admit(block), block

    def add_new(self, record):
        if record.identity in self.target or record.acquired != self.time:
            raise ValueError("New acquisition requires a new ID at the current time")
        if len(self.target) >= MAX_ENTRIES:
            raise ValueError("Source identity bound exceeded")
        block = Block.make("new:" + record.identity, [record], self.time, 16)
        self.target |= {record.identity}
        self.derivative_deadlines[record.identity] = 16
        self.policy(
            self.policy_version + 1, self.allowed | {record.identity}, self.time
        )
        return self.admit(block)

    def snapshot(self):
        precision = information = Q(0)
        weights = {}
        for block in self.blocks.values():
            precision += block.precision
            information += block.information
            for identity in block.sources:
                weights[identity] = weights.get(identity, 0) + 1
        for identity, record in self.local.items():
            precision += 1 / record.variance
            information += record.value / record.variance
            weights[identity] = weights.get(identity, 0) + 1
        missing = (self.target & self.allowed) - set(weights)
        complete = set(weights) == self.target & self.allowed
        estimate = (
            None
            if self.mode in STRICT and not complete
            else ScalarGaussian(information / precision, 1 / precision)
        )
        status = (
            "unavailable"
            if estimate is None
            else (
                "reduced"
                if self.mode == "honest_subset" and not complete
                else "complete"
            )
        )
        return {
            "estimate": estimate,
            "status": status,
            "weights": weights,
            "missing": missing,
            "precision": precision,
            "information": information,
            "blocks": {
                k: {"sources": b.sources, "created": b.created, "expires": b.expires}
                for k, b in self.blocks.items()
            },
            "local_raw_ids": frozenset(self.local),
            "entries": self.entries(),
            "peak_entries": self.peak_entries,
            "numeric_coefficients": 2 * self.entries(),
        }


def fixture(values, variance_pair):
    if values not in ("contrast", "equal") or variance_pair not in ("unit", "weak_a"):
        raise ValueError("Unknown fixture")
    va = Q(1) if variance_pair == "unit" else Q(4)
    numbers = {"d": 0, "a1": 0, "a2": 10, "b1": -4, "b2": 4, "n": -8}
    return {
        k: Record(
            k,
            Q(v if values == "contrast" else 0),
            va if k.startswith("a") else Q(1),
            8 if k == "n" else 0,
        )
        for k, v in numbers.items()
    }


def reference(records, identities):
    selected = [records[k] for k in sorted(identities)]
    inverse = [1 / r.variance for r in selected]
    total = sum(inverse, Q(0))
    return ScalarGaussian(
        sum(
            (r.value * w / total for r, w in zip(selected, inverse, strict=True)), Q(0)
        ),
        1 / total,
    )


def run_scenario(
    mode,
    values,
    variance_pair,
    archive_expiry,
    summary_expiry,
    withdrawal_time,
    delay,
    scope,
):
    if (
        archive_expiry not in (3, 6)
        or summary_expiry not in (7, 11)
        or withdrawal_time not in (2, 3, 6)
        or delay not in (0, 2)
        or scope not in ("record", "source")
    ):
        raise ValueError("Outside declared grid")
    records = fixture(values, variance_pair)
    raw = [records[k] for k in sorted(HISTORICAL)]
    archive = Archive(raw, archive_expiry)
    engine = Engine(mode, raw, records["d"], summary_expiry)
    revoked = frozenset(("a1",)) if scope == "record" else frozenset(("a1", "a2"))
    trace, pending, duplicate_delivery, previous = [], None, None, None
    for time in range(13):
        archive.advance(time)
        expired = engine.advance(time)
        actions = []
        if time == 1:
            engine.freeze()
            actions.append({"kind": "freeze", "active_raw_after": len(engine.local)})
        if time == withdrawal_time:
            dropped = engine.policy(2, engine.allowed - revoked, time)
            actions.append({"kind": "P3", "dropped": dropped, "revoked": revoked})
            missing = (engine.target & engine.allowed) - engine.covered()
            if missing:
                pending = {"sources": missing, "requested": time, "due": time + delay}
                actions.append({"kind": "request", **pending})
        if pending is not None and time == pending["due"]:
            delivered = archive.read(pending["sources"], engine.allowed, time)
            if not delivered:
                outcome = "raw_expired" if time >= archive.expires else "missing_raw"
            else:
                outcome, block = engine.rebuild(
                    delivered + delivered[:1], pending["sources"]
                )
                if outcome == "accepted":
                    duplicate_delivery = block  # External diagnostic/delivery copy, not retained by engine.
            actions.append(
                {
                    "kind": "replay",
                    **pending,
                    "delivered": [r.identity for r in delivered],
                    "outcome": outcome,
                }
            )
            pending = None
        if time == 8:
            actions.append(
                {
                    "kind": "new_data",
                    "acquired": 8,
                    "outcome": engine.add_new(records["n"]),
                }
            )
        if time == 12 and duplicate_delivery is not None:
            actions.append(
                {
                    "kind": "late_duplicate",
                    "original_expiry": duplicate_delivery.expires,
                    "outcome": engine.admit(duplicate_delivery),
                }
            )
        snapshot = engine.snapshot()
        expected_ids = engine.target & engine.allowed
        expected = reference(records, expected_ids)
        actual = snapshot["estimate"]
        weights = snapshot["weights"]
        actual_reference = reference(records, weights)
        assert snapshot["precision"] == sum(
            (Q(w) / records[k].variance for k, w in weights.items()), Q(0)
        )
        assert snapshot["information"] == sum(
            (Q(w) * records[k].value / records[k].variance for k, w in weights.items()),
            Q(0),
        )
        forbidden = set(weights) - engine.allowed
        mismatch = set(weights) != expected_ids
        row = {
            "time": time,
            "result": snapshot,
            "reference": expected,
            "actual_set_reference": actual_reference,
            "required_ids": expected_ids,
            "allowed": engine.allowed,
            "revoked": revoked if time >= withdrawal_time else frozenset(),
            "policy_version": engine.policy_version,
            "policy_effective_time": engine.policy_time,
            "policy_learned_time": engine.policy_time,
            "policy_enforced_time": engine.policy_time,
            "acquisition_times": {k: records[k].acquired for k in engine.target},
            "publication_time": time if actual is not None else None,
            "archive_entries": len(archive.records),
            "expired_blocks": expired,
            "pending": None if pending is None else dict(pending),
            "actions": actions,
            "forbidden_processing": forbidden,
            "forbidden_output": bool(forbidden) and actual is not None,
            "coverage_mismatch": mismatch,
            "repeated_influence": any(w != 1 for w in weights.values()),
            "wrong_complete_claim": snapshot["status"] == "complete"
            and (mismatch or actual != expected or bool(forbidden)),
            "actual_set_numerical_error": actual is not None
            and actual != actual_reference,
            "full_mean_error": None if actual is None else actual.mean - expected.mean,
            "full_variance_difference": None
            if actual is None
            else actual.variance - expected.variance,
            "full_precision_shortfall": None
            if actual is None
            else 1 / expected.variance - 1 / actual.variance,
            "mean_jump": None
            if actual is None or previous is None
            else actual.mean - previous.mean,
            "new_acquisitions": 1 if time == 8 else 0,
        }
        trace.append(row)
        previous = actual
    valid_after = [
        t["time"]
        for t in trace
        if withdrawal_time <= t["time"] < summary_expiry
        and t["result"]["status"] == "complete"
        and not t["wrong_complete_claim"]
    ]
    replay = [a for t in trace for a in t["actions"] if a["kind"] == "replay"]
    metric = {
        "mode": mode,
        "values": values,
        "variance_pair": variance_pair,
        "archive_expiry": archive_expiry,
        "summary_expiry": summary_expiry,
        "withdrawal_time": withdrawal_time,
        "delay": delay,
        "scope": scope,
        "outcomes": len(trace),
        **{
            status: sum(t["result"]["status"] == status for t in trace)
            for status in ("complete", "reduced", "unavailable")
        },
        **{
            key: sum(bool(t[key]) for t in trace)
            for key in (
                "wrong_complete_claim",
                "actual_set_numerical_error",
                "forbidden_processing",
                "forbidden_output",
                "repeated_influence",
            )
        },
        "first_full_recovery_time": min(valid_after) if valid_after else None,
        "recovery_latency": min(valid_after) - withdrawal_time if valid_after else None,
        "no_full_recovery_before_expiry": not valid_after,
        "nonfull_times_before_expiry": sum(
            t["result"]["status"] != "complete"
            for t in trace
            if withdrawal_time <= t["time"] < summary_expiry
        ),
        "replay_outcome": replay[0]["outcome"] if replay else "not_needed",
        "peak_retained_entries": max(t["result"]["peak_entries"] for t in trace),
        "retained_entry_ticks": sum(t["result"]["entries"] for t in trace[:-1]),
        "archive_record_ticks": sum(t["archive_entries"] for t in trace[:-1]),
        "max_mean_jump": max(
            abs(t["mean_jump"]) for t in trace if t["mean_jump"] is not None
        ),
        "total_variation": sum(
            (abs(t["mean_jump"]) for t in trace if t["mean_jump"] is not None), Q(0)
        ),
    }
    return metric, trace


def insufficiency_witness():
    worlds = []
    for a1, a2 in ((0, 10), (2, 8)):
        records = fixture("contrast", "unit")
        records["a1"] = Record("a1", Q(a1), Q(1))
        records["a2"] = Record("a2", Q(a2), Q(1))
        retained = {
            kind: Block.make(
                kind,
                [
                    records[k]
                    for k in sorted(HISTORICAL)
                    if kind == "mixed" or k.startswith(kind)
                ],
                1,
                11,
            )
            for kind in ("a", "b", "mixed")
        }
        worlds.append(
            {
                "retained": retained,
                "required_after_a1": reference(records, {"d", "a2", "b1", "b2"}),
                "before": reference(records, HISTORICAL | {"d"}),
            }
        )
    assert worlds[0]["retained"] == worlds[1]["retained"]
    assert worlds[0]["before"] == worlds[1]["before"]
    assert worlds[0]["required_after_a1"] != worlds[1]["required_after_a1"]
    return worlds


def grid():
    return itertools.product(
        MODES,
        ("contrast", "equal"),
        ("unit", "weak_a"),
        (3, 6),
        (7, 11),
        (2, 3, 6),
        (0, 2),
        ("record", "source"),
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    metrics, examples = [], []
    representative = {
        (3, 11, 2, 0, "record"),
        (3, 11, 2, 2, "record"),
        (3, 11, 3, 0, "source"),
        (6, 7, 6, 0, "record"),
    }
    for config in grid():
        metric, trace = run_scenario(*config)
        metrics.append(metric)
        if (
            metric["values"] == "contrast"
            and metric["variance_pair"] == "unit"
            and config[3:] in representative
        ):
            examples.append({"configuration": metric, "trace": trace})
    rendered = "".join(
        json.dumps(encode(r), sort_keys=True, separators=(",", ":")) + "\n"
        for r in examples
    )
    (args.output_dir / "representative_runs.jsonl").write_text(rendered)
    totals = {
        mode: {
            key: sum(m[key] for m in metrics if m["mode"] == mode)
            for key in (
                "outcomes",
                "complete",
                "reduced",
                "unavailable",
                "wrong_complete_claim",
                "actual_set_numerical_error",
                "forbidden_processing",
                "forbidden_output",
                "repeated_influence",
            )
        }
        for mode in MODES
    }
    root = Path(__file__).resolve().parent
    results = {
        "cycle": "0009",
        "parent_commit": "bb35fca32ce59bf9b25e261be07e0989e1300cc7",
        "python": platform.python_version(),
        "randomness": "none",
        "metrics": metrics,
        "scope": "Exact static scalar R03+F03 fixture; trusted synchronous policy and logical retention deadlines; no hardware guarantee",
        "totals": totals,
        "insufficiency_witness": insufficiency_witness(),
        "representative_runs": len(examples),
        "traces_sha256": hashlib.sha256(rendered.encode()).hexdigest(),
        "source_sha256": {
            name: hashlib.sha256((root / name).read_bytes()).hexdigest()
            for name in (
                "baseline.py",
                "expiry.py",
                "test_expiry.py",
                "cycles/0009-protocol.md",
            )
        },
    }
    (args.output_dir / "results.json").write_text(
        json.dumps(encode(results), sort_keys=True, indent=2) + "\n"
    )
    print(json.dumps(totals, sort_keys=True, indent=2))


if __name__ == "__main__":
    main()
