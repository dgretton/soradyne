"""Cycle 0005: exact shared-calibration/lineage witnesses, not production code."""

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

from baseline import Factor, Information2, ScalarGaussian, fuse_independent

BOUNDARY = "linear-x-shared-b/v1"
VALUES = {"r1": Q(0), "r2": Q(2), "r3": Q(8)}
LAYOUTS = {
    "singletons": (("r1",), ("r2",), ("r3",)),
    "unequal_disjoint": (("r1", "r2"), ("r3",)),
    "diamond": (("r1", "r2"), ("r2", "r3")),
}
MODES = (
    "atomic_joint",
    "compressed_joint",
    "raw_rebuild",
    "scalar_ids_only",
    "repeat_calibration_prior",
    "keep_frozen_on_withdrawal",
)
MAX_SOURCES, MAX_DELIVERIES, MAX_BLOCKS = 16, 32, 16


@dataclass(frozen=True)
class Prior:
    mean: Q
    variance: Q
    version: int

    def __post_init__(self):
        if self.variance <= 0 or type(self.version) is not int or self.version < 1:
            raise ValueError("Invalid calibration prior")


@dataclass(frozen=True)
class Block:
    sources: frozenset[str]
    payload: Information2 | ScalarGaussian
    prior_version: int | None = None
    boundary: str = BOUNDARY

    def __post_init__(self):
        object.__setattr__(self, "sources", frozenset(self.sources))
        if not 1 <= len(self.sources) <= MAX_SOURCES or not all(self.sources):
            raise ValueError("Invalid block lineage")
        if not isinstance(self.payload, (Information2, ScalarGaussian)):
            raise TypeError("Unknown block payload")
        if isinstance(self.payload, ScalarGaussian) and (
            type(self.prior_version) is not int or self.prior_version < 1
        ):
            raise ValueError("Scalar marginal requires calibration version")
        if isinstance(self.payload, Information2) and self.prior_version is not None:
            raise ValueError("Conditional factor must not contain a prior version")


@dataclass(frozen=True)
class Packet:
    child: str
    revision: int
    blocks: tuple[Block, ...]

    def __post_init__(self):
        object.__setattr__(self, "blocks", tuple(self.blocks))
        if (
            not self.child
            or type(self.revision) is not int
            or self.revision < 1
            or len(self.blocks) > MAX_BLOCKS
            or any(not isinstance(b, Block) for b in self.blocks)
        ):
            raise ValueError("Invalid packet")


def latest(deliveries: list[Packet]) -> list[Packet]:
    """Local snapshot resolution; empty newer packets are retained tombstones."""
    if len(deliveries) > MAX_DELIVERIES:
        raise ValueError("Packet delivery bound exceeded")
    seen, current = {}, {}
    for packet in deliveries:
        key = (packet.child, packet.revision)
        if key in seen and seen[key] != packet:
            raise ValueError("Conflicting copies of a packet revision")
        seen[key] = packet
        if (
            packet.child not in current
            or current[packet.child].revision < packet.revision
        ):
            current[packet.child] = packet
    packets = [current[key] for key in sorted(current)]
    sources = frozenset(s for p in packets for b in p.blocks for s in b.sources)
    if len(sources) > MAX_SOURCES:
        raise ValueError("Source bound exceeded")
    return packets


def add_information(parts: list[Information2]) -> Information2:
    return Information2(
        *(
            sum((getattr(p, key) for p in parts), Q(0))
            for key in Information2.__dataclass_fields__
        )
    )


def prior_information(prior: Prior) -> Information2:
    return Information2.from_factors([Factor(Q(0), Q(1), prior.mean, prior.variance)])


@dataclass(frozen=True)
class Answer:
    mean: Q
    variance: Q
    joint_mean: tuple[Q, Q] | None = None
    covariance: tuple[Q, Q, Q] | None = None
    sum_variance: Q | None = None


def answer_from_joint(info: Information2) -> Answer:
    mean, cov = info.joint()
    return Answer(mean[0], cov[0], mean, cov, cov[0] + 2 * cov[1] + cov[2])


def oracle(values: dict[str, Q], sigma2: Q, prior: Prior) -> Answer:
    """Independent reference through s=x+b; no candidate factor assembly."""
    if not values or sigma2 <= 0:
        raise ValueError("Reference needs observations and positive noise")
    s_variance = sigma2 / len(values)
    x_mean = sum(values.values(), Q(0)) / len(values) - prior.mean
    return Answer(
        x_mean,
        prior.variance + s_variance,
        (x_mean, prior.mean),
        (prior.variance + s_variance, -prior.variance, prior.variance),
        s_variance,
    )


def make_packets(layout, values, sigma2, prior, revision, representation):
    """Leaf likelihoods -> child packets. Only the fixture producer reads raw data."""
    if len(values) > MAX_SOURCES or sigma2 <= 0:
        raise ValueError("Invalid observation configuration")
    packets = []
    for i, source_ids in enumerate(layout):
        atoms = tuple(
            Block(
                frozenset((source,)),
                Information2.from_factors([Factor(Q(1), Q(1), values[source], sigma2)]),
            )
            for source in source_ids
            if source in values
        )
        if representation == "atomic":
            blocks = atoms
        elif representation in ("compressed", "scalar"):
            blocks = ()
            if atoms:
                conditional = add_information([b.payload for b in atoms])
                ids = frozenset(s for b in atoms for s in b.sources)
                if representation == "compressed":
                    blocks = (Block(ids, conditional),)
                else:
                    marginal = add_information(
                        [conditional, prior_information(prior)]
                    ).freeze_x()
                    blocks = (Block(ids, marginal, prior.version),)
        else:
            raise ValueError("Unknown representation")
        packets.append(Packet(f"child-{i}", revision, blocks))
    return packets


@dataclass(frozen=True)
class Result:
    status: str
    reason: str
    represented: frozenset[str]
    eligible: frozenset[str]
    missing: frozenset[str]
    forbidden: frozenset[str]
    answer: Answer | None
    numeric_coefficients: int
    lineage_entries: int
    retained_blocks: int
    contributing_children: int


def evaluate(mode, deliveries, allowed, required, prior, archive=None, sigma2=Q(1)):
    """Compare representation capabilities given trusted current inference rights.

    This is stateless across policy snapshots: no hidden raw cache or grant engine.
    Payload screening is a synthetic API guard, not a hostile-input security parser.
    """
    allowed, required = frozenset(allowed), frozenset(required)
    if mode not in MODES or len(allowed) > MAX_SOURCES or required != allowed:
        raise ValueError("Invalid query")
    if sigma2 <= 0:
        raise ValueError("Noise variance must be positive")
    packets = latest(deliveries)
    blocks = [b for p in packets for b in p.blocks]
    if any(b.boundary != BOUNDARY for b in blocks):
        raise ValueError("Boundary/model mismatch")
    numeric = sum(5 if isinstance(b.payload, Information2) else 2 for b in blocks)
    lineage = sum(len(b.sources) for b in blocks)
    retained = len(blocks)
    children = 0

    def result(reason, eligible=frozenset(), answer=None):
        used = eligible if answer is not None else frozenset()
        return Result(
            "available" if answer is not None else "unavailable",
            reason,
            used,
            eligible,
            required - eligible,
            used - allowed,
            answer,
            numeric,
            lineage,
            retained,
            children,
        )

    if mode == "raw_rebuild":
        if archive is None:
            numeric = lineage = retained = 0
            return result("archive_unavailable")
        if len(archive) > MAX_SOURCES:
            raise ValueError("Archive bound exceeded")
        numeric, lineage, retained = len(archive), len(archive), len(archive)
        clean = {s: z for s, z in archive.items() if s in allowed}
        eligible = frozenset(clean)
        if not required <= eligible or not clean:
            return result("missing_coverage", eligible)
        if prior is None:
            return result("unobservable_absolute", eligible)
        info = Information2.from_factors(
            [Factor(Q(1), Q(1), z, sigma2) for z in clean.values()]
            + [Factor(Q(0), Q(1), prior.mean, prior.variance)]
        )
        return result("ok", eligible, answer_from_joint(info))

    atomic = mode in (
        "atomic_joint",
        "repeat_calibration_prior",
        "keep_frozen_on_withdrawal",
    )
    expected_type = ScalarGaussian if mode == "scalar_ids_only" else Information2
    if any(not isinstance(b.payload, expected_type) for b in blocks):
        raise ValueError("Wrong representation for candidate")
    if atomic and any(len(b.sources) != 1 for b in blocks):
        raise ValueError(
            "Atomic candidate requires individually separable contributions"
        )
    # The deliberately unsafe control skips ONLY the authorization screen.
    clean = [
        b for b in blocks if mode == "keep_frozen_on_withdrawal" or b.sources <= allowed
    ]
    children = sum(
        any(
            mode == "keep_frozen_on_withdrawal" or b.sources <= allowed
            for b in p.blocks
        )
        for p in packets
    )
    unique = {}
    for block in clean:
        if block.sources in unique and unique[block.sources] != block:
            raise ValueError("Conflicting numerical content for the same evidence")
        unique[block.sources] = block
    clean = list(unique.values())
    eligible = frozenset(s for b in clean for s in b.sources)
    if any(a.sources & b.sources for a, b in itertools.combinations(clean, 2)):
        return result("unsupported_partial_overlap", eligible)
    if not required <= eligible or not clean:
        return result("missing_coverage", eligible)
    if prior is None:
        return result("unobservable_absolute", eligible)
    if mode == "scalar_ids_only":
        if any(b.prior_version != prior.version for b in clean):
            return result("stale_calibration", eligible)
        estimate = fuse_independent([b.payload for b in clean])
        return result("ok", eligible, Answer(estimate.mean, estimate.variance))
    copies = children if mode == "repeat_calibration_prior" else 1
    info = add_information(
        [b.payload for b in clean] + [prior_information(prior)] * copies
    )
    return result("ok", eligible, answer_from_joint(info))


def differences(answer: Answer | None, reference: Answer):
    if answer is None:
        return None
    return {
        "mean": answer.mean - reference.mean,
        "variance": answer.variance - reference.variance,
        "joint_mean": None
        if answer.joint_mean is None
        else tuple(
            a - b for a, b in zip(answer.joint_mean, reference.joint_mean, strict=True)
        ),
        "covariance": None
        if answer.covariance is None
        else tuple(
            a - b for a, b in zip(answer.covariance, reference.covariance, strict=True)
        ),
        "sum_variance": None
        if answer.sum_variance is None
        else answer.sum_variance - reference.sum_variance,
    }


def wrong(errors):
    return errors is not None and any(
        any(value) if isinstance(value, tuple) else value != 0
        for value in errors.values()
        if value is not None
    )


def run_grid(reverse=False):
    rows = []
    for sigma2, tau2, (layout_name, layout) in itertools.product(
        (Q(1), Q(4)), (Q(1, 4), Q(1), Q(4), Q(16)), LAYOUTS.items()
    ):
        initial, updated = Prior(Q(0), tau2, 1), Prior(Q(2), tau2 / 4, 2)
        previous = {mode: None for mode in MODES}
        for state in range(5):
            allowed = frozenset(VALUES) if state < 3 else frozenset(("r1", "r3"))
            permitted = {s: VALUES[s] for s in sorted(allowed)}
            prior = initial if state < 2 else updated
            reference = oracle(permitted, sigma2, prior)
            for mode in MODES:
                representation = {
                    "compressed_joint": "compressed",
                    "scalar_ids_only": "scalar",
                }.get(mode, "atomic")
                v1 = make_packets(layout, VALUES, sigma2, initial, 1, representation)
                v2 = make_packets(layout, VALUES, sigma2, initial, 2, representation)
                deliveries = v1 if state == 0 else v2 + v1 + v2
                if state == 4:
                    v3 = make_packets(
                        layout, permitted, sigma2, updated, 3, representation
                    )
                    deliveries = v3 + v1 + v2 + v3
                if reverse:
                    deliveries = list(reversed(deliveries))
                archive = None if state == 3 else permitted
                # Raw candidate has no packet state at all; other candidates get no archive.
                candidate_packets = [] if mode == "raw_rebuild" else deliveries
                result = evaluate(
                    mode,
                    candidate_packets,
                    allowed,
                    allowed,
                    prior,
                    archive if mode == "raw_rebuild" else None,
                    sigma2,
                )
                errors = differences(result.answer, reference)
                answer = result.answer
                jump = (
                    None
                    if answer is None or previous[mode] is None
                    else answer.mean - previous[mode]
                )
                previous[mode] = None if answer is None else answer.mean
                rows.append(
                    {
                        "sigma2": sigma2,
                        "initial_tau2": tau2,
                        "layout": layout_name,
                        "state": state,
                        "mode": mode,
                        "calibration": prior,
                        "allowed": allowed,
                        "required": allowed,
                        "boundary": BOUNDARY,
                        "archive_available_at_parent": state != 3,
                        "input_calibration_versions": sorted(
                            {
                                b.prior_version
                                for p in latest(candidate_packets)
                                for b in p.blocks
                                if b.prior_version is not None
                            }
                        ),
                        "source_revisions": {
                            p.child: p.revision for p in latest(candidate_packets)
                        },
                        "result": result,
                        "reference": reference,
                        "errors": errors,
                        "numerically_wrong": wrong(errors),
                        "authorization_violation": bool(result.forbidden),
                        "coverage_violation": answer is not None
                        and result.represented != allowed,
                        "mean_jump_from_previous_state": jump,
                    }
                )
    return rows


def combine_correlated_pair(means, covariance):
    """Direct covariance algebra, separate from candidate information assembly."""
    a, cross, d = covariance
    det = a * d - cross**2
    if a <= 0 or det <= 0:
        raise ValueError("Pair covariance must be positive definite")
    inverse = ((d / det, -cross / det), (-cross / det, a / det))
    row_sums = tuple(sum(row) for row in inverse)
    precision = sum(row_sums)
    mean = sum(w * z for w, z in zip(row_sums, means, strict=True)) / precision
    return ScalarGaussian(mean, 1 / precision)


def witnesses():
    first = {"r1": Q(0), "r2": Q(2), "r3": Q(8)}
    second = {"r1": Q(2), "r2": Q(0), "r3": Q(10)}
    prior = Prior(Q(0), Q(4), 1)
    packs_a = make_packets(LAYOUTS["diamond"], first, Q(1), prior, 1, "compressed")
    packs_b = make_packets(LAYOUTS["diamond"], second, Q(1), prior, 1, "compressed")
    assert packs_a == packs_b
    # Covariance algebra for two correlated retained averages, no calibration.
    a, cross, d = Q(1, 2), Q(1, 4), Q(1, 2)
    return {
        "scalar_dependence": {
            "identical_child_marginals": (
                ScalarGaussian(Q(0), Q(5)),
                ScalarGaussian(Q(8), Q(5)),
            ),
            "disjoint_source_ids": ("left", "right"),
            "shared_bias_combined": combine_correlated_pair(
                (Q(0), Q(8)), (Q(5), Q(4), Q(5))
            ),
            "independent_biases_combined": combine_correlated_pair(
                (Q(0), Q(8)), (Q(5), Q(0), Q(5))
            ),
        },
        "overlap": {
            "world_a": first,
            "world_b": second,
            "identical_conditional_packets": packs_a,
            "unique_mean_a": sum(first.values()) / 3,
            "unique_mean_b": sum(second.values()) / 3,
            "retained_averages": (Q(1), Q(5)),
            "retained_covariance": (a, cross, d),
            "honest_reduced_answer_without_calibration": combine_correlated_pair(
                (Q(1), Q(5)), (a, cross, d)
            ),
            "full_unique_variance_without_calibration": Q(1, 3),
            "incorrect_independent_answer_without_calibration": fuse_independent(
                [ScalarGaussian(Q(1), a), ScalarGaussian(Q(5), d)]
            ),
        },
    }


def encode(value):
    if isinstance(value, Q):
        return str(value)
    if hasattr(value, "__dataclass_fields__"):
        return encode(asdict(value))
    if isinstance(value, dict):
        return {key: encode(item) for key, item in value.items()}
    if isinstance(value, (set, frozenset)):
        return sorted(value)
    if isinstance(value, (list, tuple)):
        return [encode(item) for item in value]
    return value


def summarize(rows):
    metrics = {}
    for mode in MODES:
        selected = [r for r in rows if r["mode"] == mode]
        available = [r for r in selected if r["result"].answer is not None]
        metrics[mode] = {
            "outcomes": len(selected),
            "available": len(available),
            "unavailable": len(selected) - len(available),
            "numerically_wrong": sum(r["numerically_wrong"] for r in selected),
            "authorization_violations": sum(
                r["authorization_violation"] for r in selected
            ),
            "coverage_violations": sum(r["coverage_violation"] for r in selected),
            "unavailable_reasons": dict(
                sorted(
                    Counter(
                        r["result"].reason
                        for r in selected
                        if r["result"].answer is None
                    ).items()
                )
            ),
            "max_absolute_consecutive_mean_jump": max(
                abs(r["mean_jump_from_previous_state"])
                for r in selected
                if r["mean_jump_from_previous_state"] is not None
            ),
        }
    return metrics


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    rows = run_grid()
    if rows != run_grid(reverse=True):
        raise AssertionError("Delivery reversal changed results")
    args.output_dir.mkdir(parents=True, exist_ok=True)
    rendered = [
        json.dumps(encode(row), sort_keys=True, separators=(",", ":")) for row in rows
    ]
    (args.output_dir / "outcomes.jsonl").write_text("\n".join(rendered) + "\n")
    root = Path(__file__).resolve().parent
    result = {
        "cycle": "0005",
        "parent_commit": "8cecff0e1ea1f3d71e9b628b68cb9dc04aa13bd7",
        "python": platform.python_version(),
        "scope": "Exact linear shared-latent and lineage models; no statistical coverage, distributed enforcement or hardware claim",
        "randomness": "none",
        "outcomes": len(rows),
        "reverse_delivery_equal": True,
        "metrics": summarize(rows),
        "witnesses": witnesses(),
        "source_sha256": {
            name: hashlib.sha256((root / name).read_bytes()).hexdigest()
            for name in (
                "baseline.py",
                "shared_information.py",
                "test_shared_information.py",
                "cycles/0005-protocol.md",
            )
        },
        "outcomes_sha256": hashlib.sha256(
            (args.output_dir / "outcomes.jsonl").read_bytes()
        ).hexdigest(),
    }
    (args.output_dir / "results.json").write_text(
        json.dumps(encode(result), sort_keys=True, indent=2) + "\n"
    )
    print(json.dumps(encode(result["metrics"]), sort_keys=True, indent=2))


if __name__ == "__main__":
    main()
