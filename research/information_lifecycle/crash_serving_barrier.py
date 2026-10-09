"""Cycle 0014 correction: distinguish a pending live control from restart state."""

import argparse
import copy
import hashlib
import json
import platform
from pathlib import Path

from crash_boundaries import (
    EXPECTED,
    complete,
    encode,
    fixtures,
    matches,
    negative_controls,
    oracle,
    step,
    steps,
)
from crash_boundaries import (
    Store as PrimitiveStore,
)


class Store(PrimitiveStore):
    def read(self, *, enforce_gate=True):
        pending = self.pending.get("gate")
        if enforce_gate and pending is not None and pending != self.durable["gate"]:
            return {
                "status": "unavailable",
                "reason": "control received, durability pending",
            }
        return super().read(enforce_gate=enforce_gate)


def crash_matrix():
    traces = []
    for case, (old, new) in fixtures().items():
        events = steps(new)
        for cut in range(len(events) + 1):
            store = Store(old)
            prefix = events[:cut]
            for event in prefix:
                step(store, new, event)
            recovered_expected = oracle(case, prefix)
            pending_control = "stage_gate" in prefix and "persist_gate" not in prefix
            live_expected = None if pending_control else recovered_expected
            live = store.read()
            assert matches(live, live_expected), (case, prefix, "live")
            store.restart()
            recovered = store.read()
            assert matches(recovered, recovered_expected), (case, prefix, "restart")
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
                # Independent raw-record arithmetic, not checkpoint natural parameters.
                from fractions import Fraction as Q

                raw = (0, 10, 20)
                assert replay == {
                    "mean": Q(sum(raw), len(raw)),
                    "variance": Q(1, len(raw)),
                    "inputs": ("a", "b", "c"),
                }
            traces.append(
                {
                    "case": case,
                    "cut": cut,
                    "prefix": prefix,
                    "live_oracle": live_expected,
                    "restart_oracle": recovered_expected,
                    "live": live,
                    "recovered": recovered,
                    "retried": store.read(),
                    "retry_receipts": store.durable["root"]["receipts"],
                    "replay": replay,
                }
            )
    return traces


def evidence():
    traces = crash_matrix()
    negatives = negative_controls()
    old, new = fixtures()["grant"]
    original, corrected = PrimitiveStore(old), Store(old)
    for store in (original, corrected):
        step(store, new, "stage_gate")
    assert original.read()["status"] == "available"
    assert corrected.read()["status"] == "unavailable"
    negatives.append(
        {
            "control": "original_missing_pending_control_barrier",
            "original_live": original.read(),
            "corrected_live": corrected.read(),
        }
    )
    return {
        "fixtures": fixtures(),
        "crash_cuts": traces,
        "negative_controls": negatives,
        "counts": {
            "crash_cuts": len(traces),
            "negative_controls": len(negatives),
            "unavailable_live": sum(
                t["live"]["status"] == "unavailable" for t in traces
            ),
            "unavailable_restarts": sum(
                t["recovered"]["status"] == "unavailable" for t in traces
            ),
        },
        "storage_assumption": "single serialized writer; atomic durable record replacement; volatile staging; no GC",
        "local_control_effect": "durable gate installation before acknowledgment; pending live barrier; external freshness not proved",
        "supersedes": "runs/0014: live oracle omitted the pending-control barrier; original evidence preserved",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    result = {
        "cycle": "0014-corrected",
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
                "crash_serving_barrier.py",
                "test_crash_serving_barrier.py",
                "cycles/0014-serving-addendum.md",
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
