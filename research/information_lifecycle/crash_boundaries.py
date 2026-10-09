"""Cycle 0014: abstract local durability witness, not a storage implementation."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import platform
from dataclasses import asdict, dataclass, replace
from fractions import Fraction as Q
from pathlib import Path


def encode(value):
    if isinstance(value, Q):
        return str(value)
    if hasattr(value, "__dataclass_fields__"):
        return encode(asdict(value))
    if isinstance(value, dict):
        return {k: encode(v) for k, v in value.items()}
    if isinstance(value, (tuple, list)):
        return [encode(v) for v in value]
    return value


def identity(value):
    return hashlib.sha256(
        json.dumps(encode(value), sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


@dataclass(frozen=True)
class Gate:
    revision: int = 1
    policy: str = "policy@1"
    forbidden: tuple[str, ...] = ()
    epoch: str = "E0"
    closed: tuple[tuple[str, int], ...] = ()
    event: str = "initial"


@dataclass(frozen=True)
class Manifest:
    operation: str
    revision: str
    kind: str
    payload: str
    dependency: str
    inputs: tuple[str, ...]
    epoch: str = "E0"
    coordinate_epoch: str = "chart0"
    interval: tuple[int, int] = (0, 20)
    build_policy: str = "policy@1"
    model: str = "independent-unit-variance-v1"
    representation: str = "scalar-natural-parameters-m-v1"
    calibration: str = "fixed-unit-gain@1"


@dataclass(frozen=True)
class Operation:
    name: str
    payload: dict
    dependency: dict
    manifest: Manifest
    gate: Gate | None = None


VALUES = {"a": Q(0), "b": Q(10), "c": Q(20)}


def operation(name, inputs, *, kind="export", epoch="E0", gate=None):
    payload = {
        "precision": Q(len(inputs)),
        "weighted_sum": sum((VALUES[x] for x in inputs), Q(0)),
    }
    dependency = {
        "model": "independent-unit-variance-v1",
        "calibration": "fixed-unit-gain@1",
    }
    manifest = Manifest(
        name,
        f"{name}-result",
        kind,
        identity(payload),
        identity(dependency),
        inputs,
        epoch=epoch,
        interval=(10, 20) if epoch == "E1" else (0, 20),
        build_policy=gate.policy if gate else "policy@1",
    )
    return Operation(name, payload, dependency, manifest, gate)


def fixtures():
    return {
        "export": (
            operation("export-old", ("a",)),
            operation("export-new", ("a", "b")),
        ),
        "checkpoint": (
            operation("checkpoint-old", ("a",), kind="checkpoint"),
            operation("checkpoint-new", ("a", "b"), kind="checkpoint"),
        ),
        "epoch": (
            operation("epoch-old", ("a", "b")),
            operation(
                "epoch-new",
                ("c",),
                epoch="E1",
                gate=Gate(
                    2, epoch="E1", closed=(("E0", 10),), event="physical-state-close@10"
                ),
            ),
        ),
        "grant": (
            operation("grant-old", ("a", "b")),
            operation(
                "grant-new",
                ("a",),
                gate=Gate(2, "policy@2", ("b",), event="withdraw-b"),
            ),
        ),
    }


class Rejected(ValueError):
    pass


class Store:
    """Atomic dictionary replacement is the ASSUMED primitive, never disk IO."""

    def __init__(self, initial):
        m = initial.manifest
        self.durable = {
            "gate": Gate(),
            "root": {"head": identity(m), "accepted_gate": Gate(), "receipts": {}},
            "objects": {
                identity(m): m,
                m.payload: initial.payload,
                m.dependency: initial.dependency,
            },
        }
        self.pending = {}
        # Observations held by a caller outside the crashing process.
        self.acks = []

    def restart(self):
        self.pending.clear()

    def stage(self, key, value):
        self.pending[key] = copy.deepcopy(value)

    def persist_object(self, key):
        value = self.pending[key]
        ref = identity(value)
        existing = self.durable["objects"].get(ref, value)
        if existing != value:
            raise Rejected("immutable identity conflict")
        self.durable["objects"][ref] = copy.deepcopy(value)

    def install_gate(self):
        proposed = self.pending["gate"]
        current = self.durable["gate"]
        if proposed.revision < current.revision or (
            proposed.revision == current.revision and proposed != current
        ):
            raise Rejected("stale or conflicting local control")
        self.durable["gate"] = proposed

    def validate(self, ref, *, enforce_gate=True):
        objects = self.durable["objects"]
        m = objects.get(ref)
        if not isinstance(m, Manifest) or identity(m) != ref:
            raise Rejected("missing or misbound manifest")
        payload, dep = objects.get(m.payload), objects.get(m.dependency)
        if payload is None or dep is None:
            raise Rejected("missing payload/dependency")
        if identity(payload) != m.payload or identity(dep) != m.dependency:
            raise Rejected("object binding mismatch")
        if dep != {"model": m.model, "calibration": m.calibration}:
            raise Rejected("incompatible dependency")
        if m.representation != "scalar-natural-parameters-m-v1":
            raise Rejected("unsupported representation")
        if len(set(m.inputs)) != len(m.inputs) or payload["precision"] <= 0:
            raise Rejected("invalid sufficient-state shape")
        # The checked query is current use at time 15; historical use is separate.
        if not m.interval[0] <= 15 < m.interval[1]:
            raise Rejected("outside applicability interval")
        gate = self.durable["gate"]
        if enforce_gate and (
            set(m.inputs).intersection(gate.forbidden)
            or m.epoch != gate.epoch
            or any(e == m.epoch and 15 >= end for e, end in gate.closed)
        ):
            raise Rejected("current authority/epoch excludes result")
        return m, payload

    def publish(self):
        ref = self.pending["head"]
        m = self.durable["objects"].get(ref)
        if not isinstance(m, Manifest):
            raise Rejected("missing publication manifest")
        root = self.durable["root"]
        receipt = root["receipts"].get(m.operation)
        if receipt is not None:
            if receipt["manifest"] != ref:
                raise Rejected("operation identity reused with different content")
            return  # Receipt reports old commit; does not republish it as current.
        self.validate(ref)
        # The whole head + operation receipt is a single atomic durable record.
        self.durable["root"] = {
            "head": ref,
            "accepted_gate": self.durable["gate"],
            "receipts": {
                **root["receipts"],
                m.operation: {"manifest": ref, "accepted_gate": self.durable["gate"]},
            },
        }

    def read(self, *, enforce_gate=True):
        try:
            m, p = self.validate(
                self.durable["root"]["head"], enforce_gate=enforce_gate
            )
        except Rejected as error:
            return {"status": "unavailable", "reason": str(error)}
        return {
            "status": "available",
            "revision": m.revision,
            "inputs": m.inputs,
            "epoch": m.epoch,
            "mean": p["weighted_sum"] / p["precision"],
            "variance": 1 / p["precision"],
            "accepted_gate": self.durable["root"]["accepted_gate"],
            "current_gate": self.durable["gate"],
        }

    def replay(self, records):
        m, p = self.validate(self.durable["root"]["head"])
        if m.kind != "checkpoint":
            raise Rejected("export lacks checkpoint contract")
        precision, weighted_sum = p["precision"], p["weighted_sum"]
        covered = set(m.inputs)
        for record in records:
            if record in self.durable["gate"].forbidden:
                raise Rejected("replay input forbidden")
            if record not in covered:
                precision += 1
                weighted_sum += VALUES[record]
                covered.add(record)
        return {
            "mean": weighted_sum / precision,
            "variance": 1 / precision,
            "inputs": tuple(sorted(covered)),
        }


DATA_STEPS = (
    "stage_payload",
    "persist_payload",
    "stage_dependency",
    "persist_dependency",
    "stage_manifest",
    "persist_manifest",
    "stage_head",
    "publish",
    "ack_publication",
)
CONTROL_STEPS = ("stage_gate", "persist_gate", "ack_control")


def steps(op):
    return (CONTROL_STEPS if op.gate else ()) + DATA_STEPS


def step(store, op, event):
    if event == "stage_gate":
        store.stage("gate", op.gate)
    elif event == "persist_gate":
        store.install_gate()
    elif event == "ack_control":
        if store.durable["gate"] != op.gate:
            raise Rejected("control is not durable")
        store.acks.append(("control", op.gate.event))
    elif event.startswith("stage_"):
        key = event.removeprefix("stage_")
        store.stage(key, identity(op.manifest) if key == "head" else getattr(op, key))
    elif event.startswith("persist_"):
        store.persist_object(event.removeprefix("persist_"))
    elif event == "publish":
        store.publish()
    elif event == "ack_publication":
        receipt = store.durable["root"]["receipts"].get(op.name)
        if receipt is None or receipt["manifest"] != identity(op.manifest):
            raise Rejected("publication is not durable")
        store.acks.append(("publication", op.name))
    else:
        raise ValueError(event)


def complete(store, op):
    for event in steps(op):
        step(store, op, event)


# Independent specification: literal numerical states, not decoded candidate artifacts.
EXPECTED = {
    "export": ((Q(0), Q(1), ("a",), "E0"), (Q(5), Q(1, 2), ("a", "b"), "E0")),
    "checkpoint": ((Q(0), Q(1), ("a",), "E0"), (Q(5), Q(1, 2), ("a", "b"), "E0")),
    "epoch": ((Q(5), Q(1, 2), ("a", "b"), "E0"), (Q(20), Q(1), ("c",), "E1")),
    "grant": ((Q(5), Q(1, 2), ("a", "b"), "E0"), (Q(0), Q(1), ("a",), "E0")),
}


def oracle(case, events):
    if "publish" in events:
        return EXPECTED[case][1]
    if case in ("epoch", "grant") and "persist_gate" in events:
        return None
    return EXPECTED[case][0]


def matches(answer, expected):
    if expected is None:
        return answer["status"] == "unavailable"
    return (
        answer["status"] == "available"
        and tuple(answer[k] for k in ("mean", "variance", "inputs", "epoch"))
        == expected
    )


def crash_matrix():
    traces = []
    for case, (old, new) in fixtures().items():
        events = steps(new)
        for cut in range(len(events) + 1):
            store = Store(old)
            prefix = events[:cut]
            for event in prefix:
                step(store, new, event)
            live = store.read()
            store.restart()
            recovered = store.read()
            expected = oracle(case, prefix)
            assert matches(live, expected) and matches(recovered, expected), (
                case,
                prefix,
            )
            if "ack_control" in prefix:
                assert store.durable["gate"] == new.gate
            if "ack_publication" in prefix:
                assert recovered["revision"] == new.manifest.revision
            complete(store, new)
            once = copy.deepcopy(store.durable)
            complete(store, new)
            assert store.durable == once
            assert matches(store.read(), EXPECTED[case][1])
            assert len(store.durable["root"]["receipts"]) == 1
            replay = (
                store.replay(("b", "c", "b", "c")) if case == "checkpoint" else None
            )
            if replay:
                assert replay == {
                    "mean": Q(10),
                    "variance": Q(1, 3),
                    "inputs": ("a", "b", "c"),
                }
            traces.append(
                {
                    "case": case,
                    "cut": cut,
                    "prefix": prefix,
                    "oracle": expected,
                    "live": live,
                    "recovered": recovered,
                    "retried": store.read(),
                    "retry_receipts": store.durable["root"]["receipts"],
                    "replay": replay,
                }
            )
    return traces


def negative_controls():
    failures = []
    for case, (old, new) in fixtures().items():
        # New metadata + old numerical state: both are individually complete.
        bad = replace(
            new,
            payload=old.payload,
            manifest=replace(new.manifest, payload=identity(old.payload)),
        )
        store = Store(old)
        complete(store, bad)
        answer = store.read()
        assert not matches(answer, EXPECTED[case][1])
        failures.append(
            {
                "case": case,
                "control": "split_numerical_state_and_coverage",
                "answer": answer,
                "replay": store.replay(("b", "c", "b", "c"))
                if case == "checkpoint"
                else None,
            }
        )

        store = Store(old)
        for event in steps(new):
            if event == "publish":
                break
            step(store, new, event)
        store.acks.append(("publication", new.name))  # Deliberately premature.
        store.restart()
        answer = store.read()
        assert answer.get("revision") != new.manifest.revision
        failures.append(
            {
                "case": case,
                "control": "ack_before_publication",
                "answer": answer,
                "acks": store.acks,
            }
        )

        # Force a head referencing a complete manifest but missing numerical bytes.
        store = Store(old)
        if new.gate:
            store.stage("gate", new.gate)
            store.install_gate()
        for key in ("dependency", "manifest"):
            store.stage(key, getattr(new, key))
            store.persist_object(key)
        # In grant case the replacement payload equals an old scalar? Initial a+b
        # differs, and other fixtures likewise have distinct old/new numerical bytes.
        store.durable["root"] = {
            "head": identity(new.manifest),
            "accepted_gate": store.durable["gate"],
            "receipts": {
                new.name: {
                    "manifest": identity(new.manifest),
                    "accepted_gate": store.durable["gate"],
                }
            },
        }
        store.acks.append(("publication", new.name))
        store.restart()
        answer = store.read()
        assert answer["status"] == "unavailable"
        failures.append(
            {
                "case": case,
                "control": "head_before_payload_durability",
                "answer": answer,
                "acks": store.acks,
            }
        )

        if new.gate:
            store = Store(old)
            store.stage("gate", new.gate)
            store.acks.append(("control", new.gate.event))  # Never persisted.
            store.restart()
            answer = store.read()
            assert matches(answer, EXPECTED[case][0])
            failures.append(
                {
                    "case": case,
                    "control": "volatile_control_ack",
                    "answer": answer,
                    "acks": store.acks,
                }
            )
            store.stage("gate", new.gate)
            store.install_gate()
            checked, bypass = store.read(), store.read(enforce_gate=False)
            assert matches(checked, None) and matches(bypass, EXPECTED[case][0])
            failures.append(
                {
                    "case": case,
                    "control": "trust_build_time_policy_or_epoch",
                    "checked": checked,
                    "bypass": bypass,
                }
            )
    return failures


def evidence():
    traces, negatives = crash_matrix(), negative_controls()
    return {
        "fixtures": fixtures(),
        "crash_cuts": traces,
        "negative_controls": negatives,
        "counts": {
            "crash_cuts": len(traces),
            "negative_controls": len(negatives),
            "unavailable_restarts": sum(
                t["recovered"]["status"] == "unavailable" for t in traces
            ),
        },
        "storage_assumption": "single serialized writer; atomic durable record replacement; volatile staging; no GC",
        "local_control_effect": "durable gate installation, before acknowledgment; external freshness not proved",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    result = {
        "cycle": "0014",
        "parent": "c51a554219dd4630eadceabfcf75f50e0006a8ef",
        "design_base": "9cbba49",
        "nestbox_revision": "00d435f926f69aa964d1d053bd2221a8fe248969",
        "python": platform.python_version(),
        "randomness": "none",
        "source_sha256": {
            name: hashlib.sha256((root / name).read_bytes()).hexdigest()
            for name in (
                "crash_boundaries.py",
                "test_crash_boundaries.py",
                "cycles/0014-protocol.md",
            )
        },
        "results": evidence(),
    }
    args.output_dir.mkdir(parents=True, exist_ok=False)
    path = args.output_dir / "results.json"
    path.write_text(json.dumps(encode(result), indent=2, sort_keys=True) + "\n")
    print(f"Wrote {path}: {result['results']['counts']}")


if __name__ == "__main__":
    main()
