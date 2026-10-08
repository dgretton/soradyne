"""Cycle 0011: bounded, paired Monte Carlo calibration of angular intervals.

Research only. Inference functions receive measurements, never simulation truth.
All prior-cycle source and artifacts remain unchanged.
"""

import argparse
import hashlib
import itertools
import json
import math
import platform
import time
from pathlib import Path

import numpy as np
import scipy
from nonlinear_freeze import (
    QUERY_POINTS,
    Z95,
    AngularReference,
    Data,
    FrozenQuadratic,
    MomentSummary,
    raw_batch,
    raw_jacobian,
    rotation,
    wrap,
)
from scipy.integrate import quad
from scipy.special import i0e
from scipy.stats import binomtest, vonmises

TRIALS = 4096
SEEDS = {"primary": 2026100801, "heldout": 2026100802}
ANCHORS = {
    "frozen_0": 0.0,
    "frozen_minus30": -math.pi / 6,
    "frozen_minus90": -math.pi / 2,
}
METHODS = ("full_arc", "observed_gaussian", "gauss_newton", *ANCHORS)
FAMILY_COMPARISONS = 192
IDS = ("p0", "p1", "p2", "p3")
QUADRATURE_RULES = {n: np.polynomial.legendre.leggauss(n) for n in (32, 64)}


def configurations():
    return [
        {"scale": scale, "offset": offset, "sigma": sigma, "truth_angle_deg": angle}
        for scale, offset, sigma, angle in itertools.product(
            (1.0, 0.1), (False, True), (0.05, 0.2), (0, 170)
        )
    ]


def generate(config, seed, cell_index, trials):
    if not 1 <= trials <= TRIALS:
        raise ValueError("trial bound exceeded")
    p = config["scale"] * np.array([[-1, -0.5], [-1, 0.5], [1, -0.5], [1, 0.5]])
    if config["offset"]:
        p += np.array([3.0, -2.0])
    truth = np.array([math.radians(config["truth_angle_deg"]), 0.3, -0.2])
    rng = np.random.Generator(
        np.random.PCG64(np.random.SeedSequence([seed, cell_index]))
    )
    z = p @ rotation(truth[0]).T + truth[1:]
    measurements = z + config["sigma"] * rng.standard_normal((trials, 4, 2))
    return p, measurements, truth


def check_inputs(p, z, sigma):
    if (
        p.ndim != 2
        or p.shape[1] != 2
        or not 2 <= len(p) <= 32
        or z.ndim != 3
        or z.shape[1:] != p.shape
        or not 1 <= len(z) <= TRIALS
        or not np.isfinite(p).all()
        or not np.isfinite(z).all()
        or not math.isfinite(sigma)
        or sigma <= 0
    ):
        raise ValueError("finite bounded point batches and positive sigma required")


def rotate_many(angles, points):
    c, s = np.cos(angles), np.sin(angles)
    x = c[:, None] * points[:, 0] - s[:, None] * points[:, 1]
    y = s[:, None] * points[:, 0] + c[:, None] * points[:, 1]
    return np.stack((x, y), axis=-1)


def pose_points(poses, points):
    return rotate_many(poses[:, 0], points) + poses[:, None, 1:]


def infer_moments(p, z, sigma):
    """Batch compact statistics; no truth, seed or physical state available here."""
    check_inputs(p, z, sigma)
    mp, mz = p.mean(axis=0), z.mean(axis=1)
    pc, zc = p - mp, z - mz[:, None, :]
    spread = float(np.sum(pc * pc))
    a = np.sum(pc * zc, axis=(1, 2))
    b = np.sum(pc[:, 0] * zc[:, :, 1] - pc[:, 1] * zc[:, :, 0], axis=1)
    rho = np.hypot(a, b)
    if spread <= 1e-12 or np.any(rho <= 1e-12 * max(1.0, spread)):
        raise ValueError("numerically unobservable angle; trial must not be discarded")
    angles = np.arctan2(b, a)
    translations = mz - rotate_many(angles, mp[None, :])[:, 0, :]
    return {
        "pose": np.column_stack((angles, translations)),
        "rho": rho,
        "kappa": rho / sigma**2,
        "spread": spread,
        "observed_variance": sigma**2 / rho,
        "gn_variance": sigma**2 / spread,
    }


def infer_raw_svd(p, z, sigma):
    """Independently structured proper-rotation reference on every raw sample."""
    check_inputs(p, z, sigma)
    mp, mz = p.mean(axis=0), z.mean(axis=1)
    pc, zc = p - mp, z - mz[:, None, :]
    cross = np.einsum("pi,npj->nij", pc, zc)
    u, singular, vh = np.linalg.svd(cross)
    v = vh.transpose(0, 2, 1).copy()
    sign = np.linalg.det(v @ u.transpose(0, 2, 1))
    v[:, :, 1] *= sign[:, None]
    r = v @ u.transpose(0, 2, 1)
    angles = np.arctan2(r[:, 1, 0], r[:, 0, 0])
    translations = mz - np.einsum("nij,j->ni", r, mp)
    return np.column_stack((angles, translations)), singular[:, 0] + sign * singular[
        :, 1
    ]


def infer_frozen(p, z, sigma, anchor_angle):
    check_inputs(p, z, sigma)
    anchor = np.array([anchor_angle, 0.0, 0.0])
    jac = raw_jacobian(Data(p, z[0], sigma, tuple(map(str, range(len(p))))), anchor)
    if np.linalg.matrix_rank(jac) != 3:
        raise ValueError("unobservable frozen linearization")
    h = jac.T @ jac / sigma**2
    residual = (p @ rotation(anchor_angle).T - z).reshape(len(z), -1)
    eta = -residual @ jac / sigma**2
    poses = anchor + np.linalg.solve(h, eta.T).T
    covariance = np.linalg.solve(h, np.eye(3))
    return poses, float(covariance[0, 0])


def library_arc_half_width(kappa):
    """Retained initial attempt: library CDF inversion is not our accepted oracle."""
    kappa = np.asarray(kappa, dtype=float)
    if not np.isfinite(kappa).all() or np.any(kappa < 0):
        raise ValueError("finite nonnegative concentration required")
    low, high = np.zeros_like(kappa), np.full_like(kappa, math.pi)
    for _ in range(50):
        mid = (low + high) / 2
        smaller = vonmises.cdf(mid, kappa) < 0.975
        low = np.where(smaller, mid, low)
        high = np.where(smaller, high, mid)
    result = (low + high) / 2
    residual = np.max(np.abs(vonmises.cdf(result, kappa) - 0.975))
    if residual >= 1e-10:
        raise ArithmeticError(f"circular quantile residual {residual}")
    return result


def central_mass(half, kappa, order=64):
    """Symmetric von Mises mass using scaled normalization and fixed quadrature."""
    half, kappa = np.broadcast_arrays(half, kappa)
    nodes, weights = QUADRATURE_RULES[order]
    angles = half[..., None] * (nodes + 1) / 2
    density = np.exp(-2 * kappa[..., None] * np.sin(angles / 2) ** 2)
    return half * np.sum(weights * density, axis=-1) / (2 * math.pi * i0e(kappa))


def full_arc_half_width(kappa):
    kappa = np.asarray(kappa, dtype=float)
    if not np.isfinite(kappa).all() or np.any(kappa < 0):
        raise ValueError("finite nonnegative concentration required")
    low = np.zeros_like(kappa)
    high = np.minimum(math.pi, 8 / np.sqrt(np.maximum(kappa, 1e-300)))
    if np.any(central_mass(high, kappa) < 0.95):
        raise ArithmeticError("quantile bracket does not contain .95 mass")
    for _ in range(50):
        mid = (low + high) / 2
        smaller = central_mass(mid, kappa) < 0.95
        low = np.where(smaller, mid, low)
        high = np.where(smaller, high, mid)
    result = (low + high) / 2
    mass64, mass32 = central_mass(result, kappa), central_mass(result, kappa, 32)
    if (
        np.max(np.abs(mass64 - 0.95)) >= 1e-10
        or np.max(np.abs(mass64 - mass32)) >= 1e-10
    ):
        raise ArithmeticError("quantile quadrature convergence check failed")
    return result


def numerical_comparison():
    result = []
    for kappa in (0, 0.001, 0.1, 1, 10, 100, 2000):

        def weight(angle, concentration=kappa):
            return math.exp(concentration * (math.cos(angle) - 1))

        denominator, _ = quad(weight, 0, math.pi, epsabs=1e-12, epsrel=1e-12, limit=100)
        values = {}
        for name, function in (
            ("library_inverse", library_arc_half_width),
            ("quadrature_inverse", full_arc_half_width),
        ):
            half = float(function(kappa))
            numerator, _ = quad(weight, 0, half, epsabs=1e-12, epsrel=1e-12, limit=100)
            values[name] = {
                "half_width_rad": half,
                "raw_mass": numerator / denominator,
                "mass_error": abs(numerator / denominator - 0.95),
            }
        assert values["quadrature_inverse"]["mass_error"] < 1e-7
        result.append({"kappa": kappa, **values})
    return result


def arc_mass(center_offset, half_width, kappa):
    offset = wrap(center_offset)
    return np.where(
        half_width >= math.pi,
        1.0,
        vonmises.cdf(offset + half_width, kappa)
        - vonmises.cdf(offset - half_width, kappa),
    )


def contains(center, half_width, truth_angle):
    return np.abs(wrap(truth_angle - center)) <= half_width


def quantiles(values):
    values = np.asarray(values)
    return dict(
        zip(
            ("p50", "p95", "p99", "max"),
            map(float, np.quantile(values, (0.5, 0.95, 0.99, 1.0), method="linear")),
        )
    )


def coverage_summary(hits):
    trials, successes = len(hits), int(np.count_nonzero(hits))
    result = binomtest(successes, trials, p=0.95)
    ci = result.proportion_ci(confidence_level=0.95, method="exact")
    simultaneous = result.proportion_ci(
        confidence_level=1 - 0.01 / FAMILY_COMPARISONS, method="exact"
    )
    classification = (
        "under"
        if simultaneous.high < 0.95
        else ("over" if simultaneous.low > 0.95 else "compatible")
    )
    return {
        "successes": successes,
        "trials": trials,
        "fraction": successes / trials,
        "ci95_exact": [float(ci.low), float(ci.high)],
        "ci99_family_exact": [float(simultaneous.low), float(simultaneous.high)],
        "classification": classification,
        "within_two_percentage_points": abs(successes / trials - 0.95) <= 0.02,
    }


def scalar_audit(p, z, sigma, moment, raw_pose, estimates):
    maxima = {
        k: 0.0
        for k in (
            "pose_error",
            "variance_relative_error",
            "full_arc_mass_error",
            "raw_quadrature_error_estimate",
        )
    }
    for trial in range(min(8, len(z))):
        data = Data(p, z[trial], sigma, IDS)
        reference, covariance = raw_batch(data)
        compact, compact_cov = MomentSummary.freeze(data).solve()
        jac = raw_jacobian(data, reference)
        gn_var = np.linalg.solve(jac.T @ jac / sigma**2, np.eye(3))[0, 0]
        for pose in (raw_pose[trial], moment["pose"][trial], compact):
            diff = pose - reference
            diff[0] = wrap(diff[0])
            maxima["pose_error"] = max(
                maxima["pose_error"], float(np.max(np.abs(diff)))
            )
        for calculated, expected in (
            (moment["observed_variance"][trial], covariance[0, 0]),
            (moment["gn_variance"], gn_var),
            (compact_cov[0, 0], covariance[0, 0]),
        ):
            maxima["variance_relative_error"] = max(
                maxima["variance_relative_error"], float(abs(calculated / expected - 1))
            )
        for mode, angle in ANCHORS.items():
            frozen_pose, frozen_cov = FrozenQuadratic.freeze(
                data, np.array([angle, 0.0, 0.0])
            ).solve()
            diff = frozen_pose - estimates[mode]["pose"][trial]
            diff[0] = wrap(diff[0])
            maxima["pose_error"] = max(
                maxima["pose_error"], float(np.max(np.abs(diff)))
            )
            maxima["variance_relative_error"] = max(
                maxima["variance_relative_error"],
                float(abs(estimates[mode]["variance"][trial] / frozen_cov[0, 0] - 1)),
            )
        angular = AngularReference(data, reference)
        half = estimates["full_arc"]["half"][trial]
        mass, error = angular.interval_mass(reference[0], (half / Z95) ** 2)
        maxima["full_arc_mass_error"] = max(
            maxima["full_arc_mass_error"], abs(mass - 0.95)
        )
        maxima["raw_quadrature_error_estimate"] = max(
            maxima["raw_quadrature_error_estimate"], error
        )
    assert maxima["pose_error"] < 1e-9, maxima
    assert maxima["variance_relative_error"] < 1e-8, maxima
    assert maxima["full_arc_mass_error"] < 1e-7, maxima
    assert maxima["raw_quadrature_error_estimate"] < 1e-8, maxima
    return maxima


def run_cell(config, phase, cell_index, trials=TRIALS):
    seed = SEEDS[phase]
    p, z, truth = generate(config, seed, cell_index, trials)
    sigma = config["sigma"]
    moment = infer_moments(p, z, sigma)
    reference, raw_curvature = infer_raw_svd(p, z, sigma)
    difference = moment["pose"] - reference
    difference[:, 0] = wrap(difference[:, 0])
    max_error = float(np.max(np.abs(difference)))
    assert max_error < 1e-9, max_error
    curvature_error = float(np.max(np.abs(raw_curvature / moment["rho"] - 1)))
    assert curvature_error < 1e-9, curvature_error
    estimates = {
        "full_arc": {
            "pose": moment["pose"],
            "half": full_arc_half_width(moment["kappa"]),
            "variance": None,
        },
        "observed_gaussian": {
            "pose": moment["pose"],
            "half": np.minimum(math.pi, Z95 * np.sqrt(moment["observed_variance"])),
            "variance": moment["observed_variance"],
        },
        "gauss_newton": {
            "pose": moment["pose"],
            "half": np.full(
                trials, min(math.pi, Z95 * math.sqrt(moment["gn_variance"]))
            ),
            "variance": np.full(trials, moment["gn_variance"]),
        },
    }
    for mode, angle in ANCHORS.items():
        poses, variance = infer_frozen(p, z, sigma, angle)
        estimates[mode] = {
            "pose": poses,
            "half": np.full(trials, min(math.pi, Z95 * math.sqrt(variance))),
            "variance": np.full(trials, variance),
        }
    audit = scalar_audit(p, z, sigma, moment, reference, estimates)
    reference_points = pose_points(reference, QUERY_POINTS)
    truth_points = pose_points(truth[None, :], QUERY_POINTS)[0]
    outcomes, hits, jumps = {}, {}, {}
    for mode, estimate in estimates.items():
        pose, half = estimate["pose"], estimate["half"]
        angular_error = wrap(pose[:, 0] - truth[0])
        query_points = pose_points(pose, QUERY_POINTS)
        query_error = np.max(
            np.linalg.norm(query_points - truth_points, axis=2), axis=1
        )
        query_jump = np.max(
            np.linalg.norm(query_points - reference_points, axis=2), axis=1
        )
        angle_jump = np.abs(wrap(pose[:, 0] - reference[:, 0]))
        hits[mode] = contains(pose[:, 0], half, truth[0])
        jumps[mode] = query_jump
        mass = arc_mass(pose[:, 0] - moment["pose"][:, 0], half, moment["kappa"])
        assert (
            np.isfinite(mass).all()
            and np.min(mass) >= -1e-10
            and np.max(mass) <= 1 + 1e-10
        )
        outcomes[mode] = {
            "coverage": coverage_summary(hits[mode]),
            "angular_rmse_rad": float(np.sqrt(np.mean(angular_error**2))),
            "local_quadratic_score_mean": None
            if estimate["variance"] is None
            else float(np.mean(angular_error**2 / estimate["variance"])),
            "half_width_rad": quantiles(half),
            "half_width_above_45_deg_fraction": float(np.mean(half > math.pi / 4)),
            "full_circle_count": int(np.count_nonzero(half >= math.pi)),
            "mean_conditional_posterior_mass_scipy_approx": float(np.mean(mass)),
            "truth_query_error_m": quantiles(query_error),
            "rebuild_query_jump_m": quantiles(query_jump),
            "rebuild_angle_jump_rad": quantiles(angle_jump),
            "repeat_jump_m": 0.0,
            "query_total_variation_m": quantiles(query_jump),
            "forbidden_output_count": 0,
            "new_observations_during_rebuild": 0,
        }
    chosen = {0, int(np.argmax(jumps["frozen_minus90"]))}
    misses = np.flatnonzero(~hits["full_arc"])
    if len(misses):
        chosen.add(int(misses[0]))
    traces = []
    for trial in sorted(chosen):
        events = []
        for mode, estimate in estimates.items():
            after_half = (
                estimates["gauss_newton"]["half"][trial]
                if mode in ANCHORS
                else estimate["half"][trial]
            )
            for tick, event, pose, half in (
                (0, "publish", estimate["pose"][trial], estimate["half"][trial]),
                (
                    1,
                    "repeat_same_input",
                    estimate["pose"][trial],
                    estimate["half"][trial],
                ),
                (2, "authorized_raw_rebuild", reference[trial], after_half),
            ):
                events.append(
                    {
                        "method": mode,
                        "tick": tick,
                        "event": event,
                        "pose": pose.tolist(),
                        "angular_half_width_rad": float(half),
                        "contains_truth": bool(contains(pose[0], half, truth[0])),
                        "interval_method": "gauss_newton"
                        if tick == 2 and mode in ANCHORS
                        else mode,
                        "represented_ids": list(IDS),
                        "permitted_ids": list(IDS),
                        "forbidden_ids": [],
                        "policy": "P0",
                        "observation_count": 4,
                        "new_observation_count": 0,
                    }
                )
        traces.append(
            {
                "phase": phase,
                "seed": seed,
                "cell_index": cell_index,
                "config": config,
                "trial": trial,
                "truth": truth.tolist(),
                "body_points": p.tolist(),
                "measurements": z[trial].tolist(),
                "events": events,
            }
        )
    metric = {
        "phase": phase,
        "seed": seed,
        "cell_index": cell_index,
        "config": config,
        "trials": trials,
        "generator": "PCG64(SeedSequence([seed,cell_index])) standard_normal((trials,4,2))",
        "measurement_sha256_le_f64": hashlib.sha256(
            z.astype("<f8").tobytes()
        ).hexdigest(),
        "moments_svd_max_pose_error": max_error,
        "moments_svd_curvature_relative_error": curvature_error,
        "scalar_audit": audit,
        "kappa": quantiles(moment["kappa"]),
        "methods": outcomes,
        "selected_trials": sorted(chosen),
        "discarded_trials": 0,
    }
    return metric, traces


def summarize(metrics):
    return {
        method: {
            "cells": len(metrics),
            "classification_counts": {
                label: sum(
                    m["methods"][method]["coverage"]["classification"] == label
                    for m in metrics
                )
                for label in ("under", "compatible", "over")
            },
            "coverage_range": [
                min(m["methods"][method]["coverage"]["fraction"] for m in metrics),
                max(m["methods"][method]["coverage"]["fraction"] for m in metrics),
            ],
            "max_rebuild_query_jump_m": max(
                m["methods"][method]["rebuild_query_jump_m"]["max"] for m in metrics
            ),
        }
        for method in METHODS
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase", choices=("primary", "heldout", "all"), required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=False)
    start = time.monotonic()
    phases = SEEDS if args.phase == "all" else (args.phase,)
    metrics, traces = [], []
    for phase in phases:
        for index, config in enumerate(configurations()):
            if time.monotonic() - start > 600:
                raise TimeoutError("600 second cycle experiment budget exhausted")
            metric, selected = run_cell(config, phase, index)
            metrics.append(metric)
            traces.extend(selected)
    root = Path(__file__).resolve().parent
    trace_path = args.output_dir / "representative_runs.jsonl"
    trace_path.write_text(
        "".join(
            json.dumps(row, sort_keys=True, allow_nan=False) + "\n" for row in traces
        )
    )
    result = {
        "cycle": "0011",
        "phase": args.phase,
        "seeds": SEEDS,
        "trials_per_cell": TRIALS,
        "dataset_count": len(metrics) * TRIALS,
        "interval_evaluations": len(metrics) * TRIALS * len(METHODS),
        "family_comparisons": FAMILY_COMPARISONS,
        "family_confidence": 0.99,
        "runtime": {
            "python": platform.python_version(),
            "numpy": np.__version__,
            "scipy": scipy.__version__,
        },
        "totals": summarize(metrics),
        "numerical_interval_comparison": numerical_comparison(),
        "metrics": metrics,
        "resource_counts": {
            "cells": len(metrics),
            "max_trials_in_memory": TRIALS,
            "points_per_trial": 4,
            "quantile_iterations": 50,
            "quadrature_nodes": 64,
            "quadrature_subdivision_limit": 100,
            "selected_trials": len(traces),
            "stored_event_records": len(traces) * len(METHODS) * 3,
        },
        "traces_sha256": hashlib.sha256(trace_path.read_bytes()).hexdigest(),
        "source_sha256": {
            name: hashlib.sha256((root / name).read_bytes()).hexdigest()
            for name in (
                "angular_coverage.py",
                "test_angular_coverage.py",
                "nonlinear_freeze.py",
                "cycles/0011-protocol.md",
            )
        },
    }
    (args.output_dir / "results.json").write_text(
        json.dumps(result, sort_keys=True, indent=2, allow_nan=False) + "\n"
    )
    print(json.dumps(result["totals"], indent=2, sort_keys=True))
    print(
        f"Experiment elapsed seconds (not in deterministic artifact): {time.monotonic() - start:.3f}"
    )


if __name__ == "__main__":
    main()
