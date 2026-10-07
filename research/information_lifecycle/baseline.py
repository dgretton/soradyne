"""Exact, tiny research witnesses. Not a production estimator or policy engine."""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
from dataclasses import dataclass
from fractions import Fraction as Q
from pathlib import Path


@dataclass(frozen=True)
class ScalarGaussian:
    mean: Q
    variance: Q

    def __post_init__(self):
        if self.variance <= 0:
            raise ValueError("A proper scalar Gaussian needs positive variance")


@dataclass(frozen=True)
class Factor:
    """Independent likelihood: a*x + b*y = z + N(0, variance)."""

    a: Q
    b: Q
    z: Q
    variance: Q

    def __post_init__(self):
        if self.variance <= 0:
            raise ValueError("Factor variance must be positive")


@dataclass(frozen=True)
class Information2:
    h00: Q
    h01: Q
    h11: Q
    eta0: Q
    eta1: Q

    @classmethod
    def from_factors(cls, factors: list[Factor]) -> Information2:
        h00 = h01 = h11 = eta0 = eta1 = Q(0)
        for f in factors:
            w = Q(1) / f.variance
            h00 += w * f.a * f.a
            h01 += w * f.a * f.b
            h11 += w * f.b * f.b
            eta0 += w * f.a * f.z
            eta1 += w * f.b * f.z
        return cls(h00, h01, h11, eta0, eta1)

    def joint(self) -> tuple[tuple[Q, Q], tuple[Q, Q, Q]]:
        """Explicit two-by-two solution; covariance returned as (xx, xy, yy)."""
        det = self.h00 * self.h11 - self.h01**2
        if self.h00 <= 0 or det <= 0:
            raise ValueError("No proper observable joint Gaussian")
        covariance = (self.h11 / det, -self.h01 / det, self.h00 / det)
        mean = (
            (self.h11 * self.eta0 - self.h01 * self.eta1) / det,
            (self.h00 * self.eta1 - self.h01 * self.eta0) / det,
        )
        return mean, covariance

    def freeze_x(self) -> ScalarGaussian:
        """Exact Schur elimination of y in this linear Gaussian example."""
        if self.h11 <= 0:
            raise ValueError("Cannot eliminate an unconstrained y block")
        precision = self.h00 - self.h01**2 / self.h11
        information = self.eta0 - self.h01 * self.eta1 / self.h11
        if precision <= 0:
            raise ValueError("x has no proper finite-variance marginal")
        return ScalarGaussian(information / precision, Q(1) / precision)


class SummaryStore:
    """Toy version replacement with retained tombstones, no authorization semantics.

    Stable keys refer to one replaceable summary. Different keys are NOT asserted
    independent. A revision collision is an error; a delayed older record is ignored.
    """

    def __init__(self):
        self.records: dict[str, tuple[int, ScalarGaussian | None]] = {}

    def accept(self, key: str, revision: int, value: ScalarGaussian | None) -> bool:
        if not key or type(revision) is not int or revision < 1:
            raise ValueError("Require stable key and positive integer revision")
        old = self.records.get(key)
        if old is not None:
            if revision < old[0]:
                return False
            if revision == old[0]:
                if value != old[1]:
                    raise ValueError("Same revision has conflicting payloads")
                return False
        self.records[key] = (revision, value)
        return True

    def active(self) -> list[ScalarGaussian]:
        return [value for _, value in self.records.values() if value is not None]


def fuse_independent(values: list[ScalarGaussian]) -> ScalarGaussian:
    """Only valid when inputs really are independent evidence about one scalar."""
    if not values:
        raise ValueError("No evidence; do not fabricate a proper estimate")
    precision = sum((Q(1) / v.variance for v in values), Q(0))
    information = sum((v.mean / v.variance for v in values), Q(0))
    return ScalarGaussian(information / precision, Q(1) / precision)


def withdrawal_fixture(z: Q = Q(10), px: Q = Q(1), py: Q = Q(1)):
    # Independent priors x~N(0,px), y~N(0,py); withdraw only the relative observation.
    retained = [Factor(Q(1), Q(0), Q(0), px), Factor(Q(0), Q(1), Q(0), py)]
    withdrawn = Factor(Q(1), Q(-1), z, Q(1))
    return retained, withdrawn


def encode(value):
    if isinstance(value, Q):
        return {"exact": str(value), "float": float(value)}
    if isinstance(value, ScalarGaussian):
        return {"mean": encode(value.mean), "variance": encode(value.variance)}
    if isinstance(value, dict):
        return {key: encode(item) for key, item in value.items()}
    if isinstance(value, (tuple, list)):
        return [encode(item) for item in value]
    return value


def evidence() -> dict:
    retained, withdrawn = withdrawal_fixture()
    system = Information2.from_factors(retained + [withdrawn])
    mean, (xx, xy, yy) = system.joint()
    frozen = system.freeze_x()
    after = Information2.from_factors(retained).freeze_x()
    # Deliberately incorrect control: remove the raw x-block contribution AFTER
    # elimination. This does not invert the Schur complement over the shared y.
    bad_precision = Q(1) / frozen.variance - withdrawn.a**2 / withdrawn.variance
    bad_information = (
        frozen.mean / frozen.variance - withdrawn.a * withdrawn.z / withdrawn.variance
    )
    bad_subtraction = ScalarGaussian(
        bad_information / bad_precision, Q(1) / bad_precision
    )

    store = SummaryStore()
    export = ScalarGaussian(Q(2), Q(1))
    for _ in range(10):
        store.accept("child-boundary", 1, export)
    versioned = fuse_independent(store.active())  # exactly ONE active record here
    duplicated = fuse_independent([export] * 10)  # intentionally false independence
    store.accept("child-boundary", 2, None)
    store.accept("child-boundary", 1, export)

    # Two models have the SAME marginal and SAME removed measurement but different
    # post-withdrawal marginals. No function of those two retained objects alone can
    # recover both correct answers. Extra decomposition/state or replay is necessary.
    a, observation = withdrawal_fixture(z=Q(0))
    b, same_observation = withdrawal_fixture(z=Q(0), px=Q(4, 3), py=Q(1, 3))
    assert observation == same_observation
    collapsed_a = Information2.from_factors(a + [observation]).freeze_x()
    collapsed_b = Information2.from_factors(b + [same_observation]).freeze_x()
    assert collapsed_a == collapsed_b

    return encode(
        {
            "scope": "Exact linear Gaussian witnesses only; no dynamic or distributed validation",
            "policy_for_withdrawal": "P3: removed observation must not inform future inference",
            "joint": {"mean": mean, "covariance_xx_xy_yy": (xx, xy, yy)},
            "correlation": {
                "relative_variance": xx + yy - 2 * xy,
                "relative_variance_if_independent": xx + yy,
                "sum_variance": xx + yy + 2 * xy,
                "sum_variance_if_independent": xx + yy,
            },
            "freeze": frozen,
            "revision_replay": {
                "correct_after_ten_identical_exports": versioned,
                "incorrect_independent_exports": duplicated,
                "active_records_after_tombstone_and_late_replay": len(store.active()),
            },
            "withdrawal": {
                "authorized_only_rebuild": after,
                "incorrect_keep_frozen": frozen,
                "incorrect_subtract_raw_x_block_after_freeze": bad_subtraction,
            },
            "irreversible_reduction_witness": {
                "same_frozen_marginal": collapsed_a,
                "after_same_withdrawal_in_model_a": Information2.from_factors(
                    a
                ).freeze_x(),
                "after_same_withdrawal_in_model_b": Information2.from_factors(
                    b
                ).freeze_x(),
            },
        }
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    document = {
        "cycle": "0001",
        "design_base": "9cbba49",
        "python": platform.python_version(),
        "source_sha256": {
            name: hashlib.sha256((root / name).read_bytes()).hexdigest()
            for name in ("baseline.py", "test_baseline.py")
        },
        "results": evidence(),
    }
    rendered = json.dumps(document, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered)
    else:
        print(rendered, end="")


if __name__ == "__main__":
    main()
