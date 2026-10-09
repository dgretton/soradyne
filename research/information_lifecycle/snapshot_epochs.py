"""Cycle 0013: bounded exact snapshot and reset-contract witnesses; research only."""

from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import platform
from dataclasses import asdict, dataclass, is_dataclass
from fractions import Fraction as Q
from pathlib import Path

REPRESENTATION = "scalar-additive-metres-v1"


@dataclass(frozen=True)
class Edge:
    name: str
    revision: int
    destination: str
    source: str
    mean: Q
    variance: Q
    dependencies: tuple[tuple[str, int], ...]
    inputs: tuple[str, ...]
    calibration: tuple[str, int] | None
    interval: tuple[int, int] = (0, 20)
    representation: str = REPRESENTATION

    @property
    def ref(self):
        return self.name, self.revision


@dataclass(frozen=True)
class JointUncertainty:
    identity: str
    refs: tuple[tuple[str, int], ...]
    covariance: tuple[tuple[Q, ...], ...]
    representation: str = REPRESENTATION


@dataclass(frozen=True)
class Policy:
    revision: str
    allowed_inputs: frozenset[str]


@dataclass(frozen=True)
class Answer:
    edges: tuple[Edge, ...]
    uncertainty: JointUncertainty
    query_time: int
    policy_revision: str
    actual_inputs: tuple[str, ...]
    mean: Q
    variance: Q


def fixtures():
    worlds = []
    leaf = Edge(
        "leaf", 7, "C:e0", "B:e0", Q(2), Q(1, 4), (), ("leaf-observation@1",), None
    )
    for revision, mu, variance in ((1, 0, 4), (2, 100, 9)):
        calibration = ("shared-anchor", revision)
        evidence_id = f"calibration-evidence@{revision}"
        outer = Edge(
            "outer",
            revision,
            "W:e0",
            "P:e0",
            Q(10 + mu),
            Q(variance),
            (),
            (evidence_id,),
            calibration,
        )
        middle = Edge(
            "middle",
            10 + revision,
            "P:e0",
            "C:e0",
            Q(-3 - mu),
            Q(1 + variance),
            (outer.ref,),
            (evidence_id, "body-observation@1"),
            calibration,
        )
        edges = (outer, middle, leaf)
        covariance = (
            (Q(variance), Q(-variance), Q(0)),
            (Q(-variance), Q(variance + 1), Q(0)),
            (Q(0), Q(0), Q(1, 4)),
        )
        bundle = JointUncertainty(
            f"joint@{revision}", tuple(e.ref for e in edges), covariance
        )
        worlds.append((edges, bundle))
    return tuple(worlds)


def full_policy(revision="permission@1"):
    return Policy(
        revision,
        frozenset(
            (
                "calibration-evidence@1",
                "calibration-evidence@2",
                "body-observation@1",
                "leaf-observation@1",
            )
        ),
    )


def compose(edges, uncertainty, policy, query_time=10, required_refs=None):
    """Validate this three-edge contract, then compose supplied trusted Gaussian state.

    Dependency declarations and covariance truth are assumed. This is not a general
    provenance validator, authorization service, covariance parser or snapshot protocol.
    """
    edges = tuple(edges)
    if len(edges) != 3 or len({e.name for e in edges}) != 3:
        raise ValueError("Require this bounded three-edge chain")
    if (edges[0].destination, edges[-1].source) != ("W:e0", "B:e0"):
        raise ValueError("Query endpoint/epoch mismatch")
    refs = tuple(e.ref for e in edges)
    if required_refs is not None and refs != required_refs:
        raise ValueError("Requested snapshot is not present")
    if uncertainty.refs != refs:
        raise ValueError("Uncertainty bound to another revision tuple")
    if uncertainty.representation != REPRESENTATION or any(
        e.representation != REPRESENTATION for e in edges
    ):
        raise ValueError("Representation mismatch")
    calibration = {}
    for i, edge in enumerate(edges):
        if not edge.interval[0] <= query_time < edge.interval[1]:
            raise ValueError("Query outside half-open applicability")
        if i and edges[i - 1].source != edge.destination:
            raise ValueError("Intermediate frame/epoch mismatch")
        if not set(edge.dependencies) <= set(refs):
            raise ValueError("Dependency revision missing")
        if edge.calibration is not None:
            name, revision = edge.calibration
            if name in calibration and calibration[name] != revision:
                raise ValueError("Shared calibration mismatch")
            calibration[name] = revision
    inputs = tuple(sorted({item for e in edges for item in e.inputs}))
    if not set(inputs) <= policy.allowed_inputs:
        raise ValueError("Current contribution permission missing")
    cov = uncertainty.covariance
    if len(cov) != 3 or any(len(row) != 3 for row in cov):
        raise ValueError("Uncertainty dimension mismatch")
    if any(cov[i][i] != edge.variance for i, edge in enumerate(edges)):
        raise ValueError("Marginal and joint uncertainty mismatch")
    if any(cov[i][j] != cov[j][i] for i in range(3) for j in range(3)):
        raise ValueError("Asymmetric uncertainty")
    variance = sum((sum(row, Q(0)) for row in cov), Q(0))
    if variance <= 0:
        raise ValueError("Nonpositive query variance")
    return Answer(
        edges,
        uncertainty,
        query_time,
        policy.revision,
        inputs,
        sum((e.mean for e in edges), Q(0)),
        variance,
    )


def naive_compose(edges, uncertainty):
    """Wrong: independently latest means/marginals with unbound cross terms."""
    mean = sum((e.mean for e in edges), Q(0))
    variance = sum((e.variance for e in edges), Q(0))
    variance += 2 * sum(
        (uncertainty.covariance[i][j] for i in range(3) for j in range(i + 1, 3)), Q(0)
    )
    return {"mean": mean, "variance": variance}


def attempt(edges, covariance, policy, query_time=10, required_refs=None):
    try:
        return {
            "status": "accepted",
            "answer": compose(edges, covariance, policy, query_time, required_refs),
        }
    except ValueError as error:
        return {"status": "unavailable", "reason": str(error)}


class Issuer:
    """Symbolic, in-memory opaque binding only; no cryptographic or privacy claim."""

    def __init__(self):
        self._answers = []

    def issue(self, answer):
        for i, existing in enumerate(self._answers):
            if existing == answer:
                return f"fixture-answer-{i + 1}"
        if len(self._answers) >= 4:
            raise ValueError("Issuer bound exceeded")
        self._answers.append(answer)
        return f"fixture-answer-{len(self._answers)}"

    def public(self, token):
        index = next(
            (
                i
                for i in range(len(self._answers))
                if token == f"fixture-answer-{i + 1}"
            ),
            None,
        )
        if index is None:
            raise ValueError("Unknown result revision")
        answer = self._answers[index]
        return {
            "revision": token,
            "source": answer.edges[-1].source,
            "destination": answer.edges[0].destination,
            "query_time": answer.query_time,
            "mean": answer.mean,
            "variance": answer.variance,
            "units": "metres; variance metres^2",
            "representation": REPRESENTATION,
            "status_at_issue": "valid; current use still needs permission",
        }


def rotate(vector):
    x, y = map(Q, vector)
    return -y, x


def reexpress_point(vector):
    x, y = rotate(vector)
    return x + 100, y - 20


def squared_distance(a, b):
    return sum(((Q(x) - Q(y)) ** 2 for x, y in zip(a, b, strict=True)), Q(0))


def rotate_covariance(covariance):
    # Exact evaluation of R C R^T for the declared quarter-turn, not general SE(3).
    return (
        (covariance[1][1], -covariance[1][0]),
        (-covariance[0][1], covariance[0][0]),
    )


def event_contract(kind):
    if kind not in ("reexpression", "physical_bump", "unknown_reset", "ambiguous"):
        raise ValueError("Unknown event kind")
    known = kind == "reexpression"
    physical = kind == "physical_bump"
    return {
        "kind": kind,
        "effective_time": 10,
        "clock": "fixture-common-clock",
        "old_interval": (0, 10),
        "new_interval": (10, 20),
        "coordinate_epoch_before": "chart@1",
        "coordinate_epoch_after": "chart@1" if physical else "chart@2",
        "motion_epoch_before": "body-motion@1",
        "motion_epoch_after": "body-motion@2"
        if physical
        else ("body-motion@1" if known else "unknown"),
        "coordinate_relation_status": "known"
        if known
        else ("unchanged" if physical else "unknown"),
        "bridge_new_from_old": {
            "rotation": ((0, -1), (1, 0)),
            "translation": (100, -20),
            "uncertainty": "exact in fixture",
        }
        if known
        else None,
        "cross_epoch_state_reuse": "reexpress consistently"
        if known
        else "unavailable without new state/relation",
        "within_new_epoch_relative_query": "allowed when independently supported and authorized",
        "classification_source": "trusted external input; not inferred from jump size",
    }


def epoch_at(time):
    if 0 <= time < 10:
        return "old"
    if 10 <= time < 20:
        return "new"
    return None


def reset_evidence():
    point, target, obstacle, velocity = (
        (Q(1), Q(2)),
        (Q(4), Q(6)),
        (Q(10), Q(3)),
        (Q(2), Q(-1)),
    )
    covariance = ((Q(1), Q(0)), (Q(0), Q(4)))
    p2, t2, o2 = map(reexpress_point, (point, target, obstacle))
    fictitious = tuple(a - b for a, b in zip(p2, point, strict=True))
    return {
        "known_reexpression": {
            "old": {
                "point": point,
                "target": target,
                "obstacle": obstacle,
                "velocity": velocity,
                "covariance": covariance,
            },
            "new": {
                "point": p2,
                "target": t2,
                "obstacle": o2,
                "velocity": rotate(velocity),
                "covariance": rotate_covariance(covariance),
            },
            "target_distance_squared_before": squared_distance(point, target),
            "target_distance_squared_after": squared_distance(p2, t2),
            "obstacle_distance_squared_before": squared_distance(point, obstacle),
            "obstacle_distance_squared_after": squared_distance(p2, o2),
            "bad_only_point_reexpressed_target_distance_squared": squared_distance(
                p2, target
            ),
            "bad_difference_per_publication_tick": fictitious,
            "physical_time_elapsed_at_event": 0,
        },
        "same_displayed_jump": {
            "point_before": 1,
            "point_after": 101,
            "target_before": 10,
            "target_after_coordinate_shift": 110,
            "relative_after_coordinate_shift": Q(110 - 101),
            "target_after_physical_bump": 10,
            "relative_after_physical_bump": Q(10 - 101),
        },
        "uncertain_common_translation": {
            "relation_variance": Q(9),
            "point_variance": Q(1 + 9),
            "target_variance": Q(4 + 9),
            "cross_covariance": Q(9),
            "relative_variance": Q(1 + 9 + 4 + 9 - 2 * 9),
            "bad_independent_relation_copies": Q(1 + 9 + 4 + 9),
        },
        "events": [
            event_contract(k)
            for k in ("reexpression", "physical_bump", "unknown_reset", "ambiguous")
        ],
        "boundary_queries": {str(t): epoch_at(t) for t in (9, 10, 19, 20)},
    }


def evidence():
    worlds, policy = fixtures(), full_policy()
    combinations = []
    for outer_index, middle_index, uncertainty_index in itertools.product(
        range(2), repeat=3
    ):
        edges = (worlds[outer_index][0][0], worlds[middle_index][0][1], worlds[0][0][2])
        cov = worlds[uncertainty_index][1]
        checked = attempt(edges, cov, policy)
        if checked["status"] == "accepted":
            assert (checked["answer"].mean, checked["answer"].variance) == (
                Q(9),
                Q(5, 4),
            )
        combinations.append(
            {
                "selection": (outer_index + 1, middle_index + 1, uncertainty_index + 1),
                "checked": checked,
                "bad_unbound_composition": naive_compose(edges, cov),
            }
        )
    deliveries = []
    for order in itertools.permutations(("outer", "middle", "uncertainty")):
        latest_edges, latest_cov = list(worlds[0][0]), worlds[0][1]
        prefixes = []
        for part in order:
            if part == "uncertainty":
                latest_cov = worlds[1][1]
            else:
                i = 0 if part == "outer" else 1
                latest_edges[i] = worlds[1][0][i]
            prefixes.append(
                {
                    "received": part,
                    "requested_new": attempt(
                        latest_edges,
                        latest_cov,
                        policy,
                        required_refs=worlds[1][1].refs,
                    ),
                    "historical_old_explicit": attempt(*worlds[0], policy),
                    "bad_latest_parts": naive_compose(latest_edges, latest_cov),
                }
            )
        deliveries.append({"order": order, "prefixes": prefixes})
    withdrawn = Policy(
        "permission@2", policy.allowed_inputs - {"calibration-evidence@1"}
    )
    issuer = Issuer()
    views = []
    for edges, cov in worlds:
        answer = compose(edges, cov, policy)
        views.append(issuer.public(issuer.issue(answer)))
    return {
        "oracle": {
            "derivation": "outer+middle+leaf=9+e+f; e,f independent with variances 1,1/4",
            "mean": Q(9),
            "variance": Q(5, 4),
        },
        "worlds": worlds,
        "combinations": combinations,
        "delivery_orders": deliveries,
        "policy_checks": {
            "coherent_old_after_withdrawal": attempt(*worlds[0], withdrawn),
            "coherent_new_after_withdrawal": attempt(*worlds[1], withdrawn),
            "old_with_new_policy_same_rights": attempt(
                *worlds[0], full_policy("permission@2")
            ),
        },
        "public_views": views,
        "resets": reset_evidence(),
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
    result = {
        "cycle": "0013",
        "parent": "aebef841045ad02ce408ac77e6001df0f7a3aa1b",
        "design_base": "9cbba49",
        "nestbox_revision": "041f1146df2e2a271cd0489fda520bc65f2e9f70",
        "python": platform.python_version(),
        "randomness": "none",
        "bounds": {
            "edges": 3,
            "combinations": 8,
            "delivery_orders": 6,
            "prefixes_per_order": 3,
            "points": 3,
            "event_kinds": 4,
            "issuer_answers": 4,
        },
        "source_sha256": {
            name: hashlib.sha256((root / name).read_bytes()).hexdigest()
            for name in (
                "snapshot_epochs.py",
                "test_snapshot_epochs.py",
                "cycles/0013-protocol.md",
            )
        },
        "results": evidence(),
    }
    args.output_dir.mkdir(parents=True, exist_ok=False)
    path = args.output_dir / "results.json"
    path.write_text(json.dumps(encode(result), indent=2, sort_keys=True) + "\n")
    print(
        f"Wrote {path}; eight combinations, eighteen arrival prefixes, exact reset witnesses"
    )


if __name__ == "__main__":
    main()
