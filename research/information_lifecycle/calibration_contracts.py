"""Cycle 0012: exact calibration/withdrawal witnesses, not production inference."""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
from dataclasses import asdict, dataclass, is_dataclass, replace
from fractions import Fraction as Q
from pathlib import Path

from baseline import Factor, Information2, ScalarGaussian

MAX_RECORDS = 8
CALIBRATION = "shared-offset"


def inverse(matrix):
    """Bounded exact elimination, independent of the explicit 2x2 GLS reference."""
    n = len(matrix)
    if not 1 <= n <= 3 or any(len(row) != n for row in matrix):
        raise ValueError("Require square matrix of dimension 1..3")
    rows = [
        [Q(x) for x in row] + [Q(i == j) for j in range(n)]
        for i, row in enumerate(matrix)
    ]
    for j in range(n):
        pivot = next((i for i in range(j, n) if rows[i][j]), None)
        if pivot is None:
            raise ValueError("Singular matrix")
        rows[j], rows[pivot] = rows[pivot], rows[j]
        divisor = rows[j][j]
        rows[j] = [v / divisor for v in rows[j]]
        for i in range(n):
            if i != j:
                scale = rows[i][j]
                rows[i] = [a - scale * b for a, b in zip(rows[i], rows[j], strict=True)]
    return tuple(tuple(row[n:]) for row in rows)


@dataclass(frozen=True)
class Calibration:
    identity: str = CALIBRATION
    revision: int = 1
    tau: Q = Q(4)
    drift: Q = Q(1)
    noise: Q = Q(1)
    gain: Q = Q(1)

    def __post_init__(self):
        for name in ("tau", "drift", "noise", "gain"):
            object.__setattr__(self, name, Q(getattr(self, name)))
        if (
            not self.identity
            or type(self.revision) is not int
            or self.revision < 1
            or min(self.tau, self.drift, self.noise, self.gain) <= 0
        ):
            raise ValueError("Invalid bounded calibration fixture")


def drift_joint(values, model, input_calibration=CALIBRATION):
    if len(values) != 2 or input_calibration != model.identity:
        raise ValueError("Require two observations bound to this calibration identity")
    # (row, value, variance); the shared prior and the drift constraint occur once.
    factors = (
        ((0, 1, 0), Q(0), model.tau),
        ((0, 0, 1), Q(0), model.drift),
        ((model.gain, 1, 0), Q(values[0]), model.noise),
        ((model.gain, 1, 1), Q(values[1]), model.noise),
    )
    h = [[Q(0) for _ in range(3)] for _ in range(3)]
    eta = [Q(0) for _ in range(3)]
    for row, value, variance in factors:
        for i in range(3):
            eta[i] += row[i] * value / variance
            for j in range(3):
                h[i][j] += row[i] * row[j] / variance
    covariance = inverse(h)
    mean = tuple(
        sum((v * e for v, e in zip(row, eta, strict=True)), Q(0)) for row in covariance
    )
    return {
        "mean_x_b0_d": mean,
        "covariance_x_b0_d": covariance,
        "x": ScalarGaussian(mean[0], covariance[0][0]),
    }


def gls_two(values, aa, ab, bb, gain=Q(1)):
    """Closed-form observation-space reference; no latent information assembly."""
    aa, ab, bb, gain = map(Q, (aa, ab, bb, gain))
    det = aa * bb - ab * ab
    if len(values) != 2 or aa <= 0 or det <= 0 or gain == 0:
        raise ValueError("Invalid two-observation covariance")
    denom = aa + bb - 2 * ab
    mean = ((bb - ab) * values[0] + (aa - ab) * values[1]) / (gain * denom)
    return ScalarGaussian(mean, det / (gain * gain * denom))


def drift_reference(values, model, independent=False, no_drift=False):
    return gls_two(
        values,
        model.tau + model.noise,
        Q(0) if independent else model.tau,
        model.tau + model.noise + (0 if no_drift else model.drift),
        model.gain,
    )


@dataclass(frozen=True)
class Record:
    record_id: str
    sensor: str
    time: int
    field: str
    value: Q
    revision: int = 1
    calibration: str = CALIBRATION

    def __post_init__(self):
        object.__setattr__(self, "value", Q(self.value))
        if (
            not self.record_id
            or not self.sensor
            or type(self.time) is not int
            or type(self.revision) is not int
            or self.revision < 1
            or self.field not in ("signal", "calibration-offset")
            or self.calibration != CALIBRATION
        ):
            raise ValueError("Invalid identified scalar record")

    @property
    def key(self):
        return f"{self.record_id}@{self.revision}:{self.field}"


def validated(records):
    records = tuple(records)
    if len(records) > MAX_RECORDS or len({r.key for r in records}) != len(records):
        raise ValueError("Input bound or duplicate/conflicting identity")
    return records


def select_survivors(records, *, sensor=None, interval=None, field=None):
    """Selectors are conjunctive; selection removes an original evidence component."""
    records = validated(records)
    if sensor is None and interval is None and field is None:
        raise ValueError("Require explicit withdrawal selector")
    if interval is not None and (
        len(interval) != 2
        or any(type(t) is not int for t in interval)
        or interval[0] >= interval[1]
    ):
        raise ValueError("Require a nonempty half-open integer interval")
    return tuple(
        r
        for r in records
        if not (
            (sensor is None or r.sensor == sensor)
            and (interval is None or interval[0] <= r.time < interval[1])
            and (field is None or r.field == field)
        )
    )


def target_joint(records):
    records = validated(records)
    if not any(r.field == "signal" for r in records):
        raise ValueError("No target information")
    factors = [Factor(Q(0), Q(1), Q(0), Q(4))]
    factors.extend(Factor(Q(r.field == "signal"), Q(1), r.value, Q(1)) for r in records)
    return Information2.from_factors(factors).freeze_x()


@dataclass(frozen=True)
class LearnedCalibration:
    identity: str
    revision: int
    mean: Q
    variance: Q
    support: tuple[str, ...]


def learned_calibration(records):
    training = [r for r in validated(records) if r.field == "calibration-offset"]
    precision = Q(1, 4) + len(training)
    return LearnedCalibration(
        CALIBRATION,
        1,
        sum((r.value for r in training), Q(0)) / precision,
        1 / precision,
        tuple(sorted(r.key for r in training)),
    )


def target_from_calibration(targets, calibration):
    targets = validated(targets)
    if not targets or any(r.field != "signal" for r in targets):
        raise ValueError("Require only retained target records")
    if any(r.calibration != calibration.identity for r in targets):
        raise ValueError("Calibration identity mismatch")
    return ScalarGaussian(
        sum((r.value for r in targets), Q(0)) / len(targets) - calibration.mean,
        calibration.variance + Q(1, len(targets)),
    )


def target_reference(records):
    records = validated(records)
    return target_from_calibration(
        [r for r in records if r.field == "signal"], learned_calibration(records)
    )


def use_derivative(targets, calibration, allowed_support, *, explicit_p2_grant=False):
    """One trusted fixture policy: P3 support check, or separately specified P2 right.

    The Boolean stands for an external grant to this exact artifact/use, not permission
    inferred from algebra or an interface intended for production callers.
    """
    if not explicit_p2_grant and not set(calibration.support) <= set(allowed_support):
        return None
    return target_from_calibration(targets, calibration)


def selector_records():
    return (
        Record("a9", "A", 9, "signal", Q(0)),
        Record("b10", "B", 10, "signal", Q(6)),
        Record("a11", "A", 11, "signal", Q(12)),
        Record("c10", "A", 10, "calibration-offset", Q(6)),
    )


def field_witness(u=Q(0), v=Q(4)):
    u, v = Q(u), Q(v)
    precision = inverse(((1, Q(1, 2)), (Q(1, 2), 1)))
    full_eta_u = precision[0][0] * u + precision[0][1] * v
    # Correct surviving method receives only u and its independently known variance.
    correct = ScalarGaussian(u, Q(1))
    return {
        "full": gls_two((u, v), 1, Q(1, 2), 1),
        "after_withdraw_v": correct,
        "bad_precision_and_information_slice": ScalarGaussian(
            full_eta_u / precision[0][0], 1 / precision[0][0]
        ),
        "bad_conditional_precision_with_only_u": ScalarGaussian(u, 1 / precision[0][0]),
    }


def evidence():
    drift_cases = []
    for tau in (1, 4):
        for q in (1, 4):
            for r in (1, 2):
                for g in (1, 2):
                    for values in ((0, 6), (1, 4)):
                        model = Calibration(tau=tau, drift=q, noise=r, gain=g)
                        joint = drift_joint(values, model)
                        reference = drift_reference(values, model)
                        assert joint["x"] == reference
                        drift_cases.append(
                            {
                                "model": model,
                                "observations": values,
                                "joint": joint,
                                "reference": reference,
                                "bad_independent": drift_reference(
                                    values, model, independent=True
                                ),
                                "bad_no_drift": drift_reference(
                                    values, model, no_drift=True
                                ),
                            }
                        )
    model = Calibration()
    revision = replace(model, revision=2, drift=Q(4))
    worlds = ((0, 6), (1, 4))
    old = [drift_joint(v, model)["x"] for v in worlds]
    new = [drift_joint(v, revision)["x"] for v in worlds]
    assert old[0] == old[1] and new[0] != new[1]
    records = selector_records()
    selectors = {
        "sensor_A": {"sensor": "A"},
        "range_10_11": {"interval": (10, 11)},
        "field_calibration": {"field": "calibration-offset"},
        "conjunction": {
            "sensor": "A",
            "interval": (10, 11),
            "field": "calibration-offset",
        },
    }
    selection_results = {}
    for name, selector in selectors.items():
        survivors = select_survivors(records, **selector)
        joint, reference = target_joint(survivors), target_reference(survivors)
        assert joint == reference
        selection_results[name] = {
            "selector": selector,
            "survivors": [r.key for r in survivors],
            "calibration_support": learned_calibration(survivors).support,
            "joint": joint,
            "reference": reference,
            "mean_change_from_initial": joint.mean - target_joint(records).mean,
        }
    calibration = learned_calibration(records)
    targets = tuple(r for r in records if r.field == "signal")
    gain_revision = replace(model, revision=2, gain=Q(2))
    fields = field_witness()
    assert fields["after_withdraw_v"] != fields["bad_precision_and_information_slice"]
    assert fields["after_withdraw_v"] != fields["bad_conditional_precision_with_only_u"]
    return {
        "drift_cases": drift_cases,
        "replacement": {
            "identity_only_manifest": ["early@1:signal", "late@1:signal"],
            "old_model": model,
            "new_model": revision,
            "worlds": worlds,
            "same_old_marginals": old,
            "different_new_marginals": new,
            "bad_append_old_and_new_drift_prior": drift_joint(
                worlds[0], replace(revision, drift=Q(4, 5))
            )["x"],
            "gain_revision": gain_revision,
            "gain_rebuilt": drift_joint(worlds[0], gain_revision)["x"],
            "bad_relabel_old_coefficients": old[0],
        },
        "selectors": {
            "records": records,
            "initial": target_joint(records),
            "cases": selection_results,
        },
        "learned_dependency": {
            "artifact": calibration,
            "original_support_of_target_result": sorted(r.key for r in records),
            "p3_reuse_forbidden_derivative": use_derivative(targets, calibration, ()),
            "p3_rebuild_from_surviving_records": target_joint(targets),
            "p3_missing_surviving_numeric_state": use_derivative((), calibration, ()),
            "p2_explicit_retained_derivative": use_derivative(
                targets, calibration, (), explicit_p2_grant=True
            ),
            "bad_ignore_withdrawal": target_from_calibration(targets, calibration),
        },
        "correlated_fields": fields,
    }


def encode(value):
    if isinstance(value, Q):
        return str(value)
    if is_dataclass(value):
        return encode(asdict(value))
    if isinstance(value, dict):
        return {key: encode(item) for key, item in value.items()}
    if isinstance(value, (tuple, list)):
        return [encode(item) for item in value]
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    document = {
        "cycle": "0012",
        "parent": "ac6abd090dfb49cfb0c7681889c5bf8bab982152",
        "design_base": "9cbba49",
        "nestbox_revision": "041f1146df2e2a271cd0489fda520bc65f2e9f70",
        "scope": "Exact linear contract witnesses; no general inference, grant or storage implementation",
        "python": platform.python_version(),
        "randomness": "none",
        "bounds": {
            "matrix_dimension": 3,
            "selector_records": MAX_RECORDS,
            "drift_cases": 32,
        },
        "source_sha256": {
            name: hashlib.sha256((root / name).read_bytes()).hexdigest()
            for name in (
                "baseline.py",
                "calibration_contracts.py",
                "test_calibration_contracts.py",
                "cycles/0012-protocol.md",
            )
        },
        "results": evidence(),
    }
    args.output_dir.mkdir(parents=True, exist_ok=False)
    output = args.output_dir / "results.json"
    output.write_text(json.dumps(encode(document), indent=2, sort_keys=True) + "\n")
    print(f"Wrote {output}; 32 exact drift comparisons and four withdrawal selectors")


if __name__ == "__main__":
    main()
