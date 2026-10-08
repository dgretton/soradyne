"""Cycle 0010: isolated planar likelihood experiment, not application geometry.

Pose chart: [theta, t_x, t_y]; left tangent: [omega, v_x, v_y].
Only the evaluator/reference receives raw data after summary construction.
"""

import argparse
import hashlib
import itertools
import json
import math
import platform
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import scipy
from scipy.integrate import quad

J = np.array([[0.0, -1.0], [1.0, 0.0]])
QUERY_POINTS = np.array([[0.0, 0.0], [2.0, 0.0], [0.0, 2.0]])
TRUTH = np.array([0.0, 0.3, -0.2])
DELTAS = (0, 5, 15, 30, 60, 90, 120, 170)
MODES = ("raw_batch", "nonlinear_moments", "frozen_linear", "recentered_linear")
Z95 = 1.959963984540054


def rotation(theta):
    c, s = math.cos(theta), math.sin(theta)
    return np.array([[c, -s], [s, c]])


def wrap(angle):
    return (angle + math.pi) % (2 * math.pi) - math.pi


def transform(pose, points):
    return points @ rotation(pose[0]).T + pose[1:]


def left_covariance(pose, chart_covariance):
    chart_to_left = np.eye(3)
    chart_to_left[1:, 0] = -J @ pose[1:]
    return chart_to_left @ chart_covariance @ chart_to_left.T


def point_jacobian(pose, point):
    return np.column_stack((rotation(pose[0]) @ J @ point, np.eye(2)))


def point_covariances(pose, covariance):
    return [
        point_jacobian(pose, q) @ covariance @ point_jacobian(pose, q).T
        for q in QUERY_POINTS
    ]


@dataclass(frozen=True)
class Data:
    p: np.ndarray
    z: np.ndarray
    sigma: float
    ids: tuple[str, ...]

    def __post_init__(self):
        for name in ("p", "z"):
            array = np.array(getattr(self, name), dtype=float, copy=True)
            if array.ndim != 2 or array.shape[1] != 2 or not np.isfinite(array).all():
                raise ValueError("finite Nx2 points required")
            array.setflags(write=False)
            object.__setattr__(self, name, array)
        if self.p.shape != self.z.shape or not 2 <= len(self.p) <= 32:
            raise ValueError("matching bounded point sets required")
        if len(self.ids) != len(self.p) or len(set(self.ids)) != len(self.ids):
            raise ValueError("one unique identity per correspondence required")
        if not math.isfinite(self.sigma) or self.sigma <= 0:
            raise ValueError("positive finite sigma required")


def fixture(scale=1.0, offset=False, sigma=0.05, noise="zero"):
    p = scale * np.array([[-1, -0.5], [-1, 0.5], [1, -0.5], [1, 0.5]])
    if offset:
        p += np.array([3, -2])
    z = transform(TRUTH, p)
    if noise == "pattern":
        z += sigma * np.array([[0.4, -0.8], [-0.6, 0.2], [0.3, 0.5], [-0.1, 0.1]])
    elif noise != "zero":
        raise ValueError("unknown deterministic noise fixture")
    return Data(p, z, sigma, tuple(f"p{i}" for i in range(4)))


def raw_residual(data, pose):
    return (transform(pose, data.p) - data.z).reshape(-1)


def raw_jacobian(data, pose):
    return np.vstack([point_jacobian(pose, p) for p in data.p])


def raw_nll(data, pose):
    r = raw_residual(data, pose)
    return float(r @ r / (2 * data.sigma**2))


def raw_gradient(data, pose):
    return raw_jacobian(data, pose).T @ raw_residual(data, pose) / data.sigma**2


def raw_hessian(data, pose):
    jac = raw_jacobian(data, pose)
    h = jac.T @ jac
    r = raw_residual(data, pose).reshape(-1, 2)
    h[0, 0] -= np.sum(r * (data.p @ rotation(pose[0]).T))
    return h / data.sigma**2


def raw_batch(data):
    """Proper Procrustes SVD, independent of MomentSummary's scalar solver."""
    pc, zc = data.p - data.p.mean(axis=0), data.z - data.z.mean(axis=0)
    u, singular, vh = np.linalg.svd(pc.T @ zc)
    correction = np.diag([1.0, np.linalg.det(vh.T @ u.T)])
    r = vh.T @ correction @ u.T
    # For two dimensions this corrected trace is rotational profile curvature.
    curvature = singular[0] + correction[1, 1] * singular[1]
    if curvature <= 1e-12 * max(1.0, float(np.sum(pc * pc))):
        raise ValueError("unobservable angular mode")
    pose = np.r_[
        math.atan2(r[1, 0], r[0, 0]), data.z.mean(axis=0) - r @ data.p.mean(axis=0)
    ]
    return pose, np.linalg.solve(raw_hessian(data, pose), np.eye(3))


@dataclass(frozen=True)
class MomentSummary:
    n: int
    mean_p: np.ndarray
    mean_z: np.ndarray
    sp: float
    sz: float
    a: float
    b: float
    sigma: float
    ids: tuple[str, ...]

    @classmethod
    def freeze(cls, data):
        mp, mz = data.p.mean(axis=0), data.z.mean(axis=0)
        p, z = data.p - mp, data.z - mz
        return cls(
            len(p),
            mp,
            mz,
            float(np.sum(p * p)),
            float(np.sum(z * z)),
            float(np.sum(p * z)),
            float(np.sum(p[:, 0] * z[:, 1] - p[:, 1] * z[:, 0])),
            data.sigma,
            data.ids,
        )

    def nll(self, pose):
        theta, t = pose[0], pose[1:]
        center = t + rotation(theta) @ self.mean_p - self.mean_z
        sse = (
            self.sp
            + self.sz
            - 2 * (self.a * math.cos(theta) + self.b * math.sin(theta))
        )
        return float((sse + self.n * (center @ center)) / (2 * self.sigma**2))

    def solve(self):
        rho = math.hypot(self.a, self.b)
        if rho <= 1e-12 * max(1.0, self.sp):
            raise ValueError("unobservable angular mode")
        theta = math.atan2(self.b, self.a)
        r = rotation(theta)
        pose = np.r_[theta, self.mean_z - r @ self.mean_p]
        centered_cov = np.diag([self.sigma**2 / rho] + [self.sigma**2 / self.n] * 2)
        to_pose = np.eye(3)
        to_pose[1:, 0] = -r @ J @ self.mean_p
        return pose, to_pose @ centered_cov @ to_pose.T


@dataclass(frozen=True)
class FrozenQuadratic:
    anchor: np.ndarray
    h: np.ndarray
    eta: np.ndarray
    constant: float
    ids: tuple[str, ...]

    @classmethod
    def freeze(cls, data, anchor):
        jac, r = raw_jacobian(data, anchor), raw_residual(data, anchor)
        h = jac.T @ jac / data.sigma**2
        if np.linalg.matrix_rank(jac) != 3:
            raise ValueError("unobservable linear model")
        return cls(
            np.array(anchor),
            h,
            -jac.T @ r / data.sigma**2,
            raw_nll(data, anchor),
            data.ids,
        )

    def solve(self):
        return self.anchor + np.linalg.solve(self.h, self.eta), np.linalg.solve(
            self.h, np.eye(3)
        )

    def nll(self, pose):
        d = pose - self.anchor  # unwrapped local chart, not a periodic likelihood
        return float(self.constant - self.eta @ d + 0.5 * d @ self.h @ d)

    def recenter(self, anchor):
        shift = anchor - self.anchor
        return FrozenQuadratic(
            np.array(anchor),
            self.h.copy(),
            self.eta - self.h @ shift,
            self.nll(anchor),
            self.ids,
        )


class AngularReference:
    """Raw residual quadrature; no candidate moments or covariance used."""

    def __init__(self, data, optimum):
        self.pc = data.p - data.p.mean(axis=0)
        self.zc = data.z - data.z.mean(axis=0)
        self.sigma = data.sigma
        self.mode = float(optimum[0])
        self.minimum = self.profile(self.mode)
        self.norm, self.norm_error = self.integral(-math.pi, math.pi)
        grid = self.mode + np.arange(8192) * (2 * math.pi / 8192) - math.pi
        c, s = np.cos(grid), np.sin(grid)
        rotated = np.empty((len(grid), len(self.pc), 2))
        rotated[:, :, 0] = c[:, None] * self.pc[:, 0] - s[:, None] * self.pc[:, 1]
        rotated[:, :, 1] = s[:, None] * self.pc[:, 0] + c[:, None] * self.pc[:, 1]
        sse = np.sum((rotated - self.zc) ** 2, axis=(1, 2))
        grid_norm = float(
            np.mean(np.exp(-(sse - self.minimum) / (2 * self.sigma**2))) * 2 * math.pi
        )
        self.grid_norm_relative_error = abs(grid_norm - self.norm) / self.norm
        self.grid_beats_svd_by = max(0.0, self.minimum - float(np.min(sse)))

    def profile(self, theta):
        r = self.pc @ rotation(theta).T - self.zc
        return float(np.sum(r * r))

    def density_unnormalized(self, offset):
        return math.exp(
            -(self.profile(self.mode + offset) - self.minimum) / (2 * self.sigma**2)
        )

    def integral(self, low, high):
        if low == high:
            return 0.0, 0.0
        points = [0.0] if low < 0 < high else None
        return quad(
            self.density_unnormalized,
            low,
            high,
            epsabs=1e-10,
            epsrel=1e-10,
            limit=100,
            points=points,
        )

    def interval_mass(self, center, variance):
        if variance <= 0 or not math.isfinite(variance):
            raise ValueError("positive angular variance required")
        half = Z95 * math.sqrt(variance)
        if half >= math.pi:
            return 1.0, 0.0
        center = wrap(center - self.mode)
        low, high = center - half, center + half
        if low < -math.pi:
            intervals = [(low + 2 * math.pi, math.pi), (-math.pi, high)]
        elif high > math.pi:
            intervals = [(low, math.pi), (-math.pi, high - 2 * math.pi)]
        else:
            intervals = [(low, high)]
        values = [self.integral(a, b) for a, b in intervals]
        mass = sum(v for v, _ in values) / self.norm
        error = (sum(e for _, e in values) + mass * self.norm_error) / self.norm
        return mass, error


def relative_error(a, b):
    return float(np.linalg.norm(a - b) / max(1e-30, np.linalg.norm(b)))


def pose_metrics(pose, covariance, reference, reference_cov, angular, data):
    angle_error = float(abs(wrap(pose[0] - reference[0])))
    query_error = float(
        np.max(
            np.linalg.norm(
                transform(pose, QUERY_POINTS) - transform(reference, QUERY_POINTS),
                axis=1,
            )
        )
    )
    mass, error = angular.interval_mass(float(pose[0]), float(covariance[0, 0]))
    point_covs = point_covariances(pose, covariance)
    left_cov = left_covariance(pose, covariance)
    chart_errors = []
    for q, point_cov in zip(QUERY_POINTS, point_covs):
        jl = np.column_stack((J @ transform(pose, q), np.eye(2)))
        chart_errors.append(relative_error(jl @ left_cov @ jl.T, point_cov))
    truth_error = pose - TRUTH
    truth_error[0] = wrap(truth_error[0])
    return {
        "pose": pose.tolist(),
        "chart_covariance": covariance.tolist(),
        "left_covariance": left_cov.tolist(),
        "point_covariances": [c.tolist() for c in point_covs],
        "angle_error_rad": angle_error,
        "translation_error_m": float(np.linalg.norm(pose[1:] - reference[1:])),
        "max_query_error_m": query_error,
        "covariance_relative_error": relative_error(covariance, reference_cov),
        "nominal_95_interval_actual_mass": mass,
        "quadrature_mass_error_estimate": error,
        "mass_diagnostic_pass": 0.94 <= mass <= 0.96,
        "one_degree_diagnostic_pass": angle_error <= math.pi / 180,
        "one_cm_query_diagnostic_pass": query_error <= 0.01,
        "chart_equivalence_relative_error": max(chart_errors),
        "truth_chart_mahalanobis_squared": float(
            truth_error @ np.linalg.solve(covariance, truth_error)
        ),
        "raw_nll": raw_nll(data, pose),
    }


def grid():
    return itertools.product(
        (1.0, 0.1), (False, True), (0.05, 0.2), ("zero", "pattern"), DELTAS
    )


def run_scenario(scale, offset, sigma, noise, delta):
    data = fixture(scale, offset, sigma, noise)
    reference, reference_cov = raw_batch(data)
    angular = AngularReference(data, reference)
    moment = MomentSummary.freeze(data)
    frozen = FrozenQuadratic.freeze(data, np.array([-math.radians(delta), 0.0, 0.0]))
    frozen_pose, _ = frozen.solve()
    recentered = frozen.recenter(frozen_pose)
    solutions = {
        "raw_batch": (reference, reference_cov),
        "nonlinear_moments": moment.solve(),
        "frozen_linear": frozen.solve(),
        "recentered_linear": recentered.solve(),
    }
    objectives = []
    for angle, t in itertools.product(
        (-170, -60, 0, 60, 170), ((0, 0), (0.3, -0.2), (1, 0), (0, 1), (-1, -1))
    ):
        pose = np.r_[math.radians(angle), t]
        objectives.append(
            abs(moment.nll(pose) - raw_nll(data, pose))
            / max(1.0, abs(raw_nll(data, pose)))
        )
    h = 1e-5
    fd = np.column_stack(
        [
            (
                raw_gradient(data, reference + h * e)
                - raw_gradient(data, reference - h * e)
            )
            / (2 * h)
            for e in np.eye(3)
        ]
    )
    methods = {
        name: pose_metrics(p, c, reference, reference_cov, angular, data)
        for name, (p, c) in solutions.items()
    }
    trace = []
    for mode, (pose, covariance) in solutions.items():
        kept_ids = (
            data.ids
            if mode == "raw_batch"
            else (moment.ids if mode == "nonlinear_moments" else frozen.ids)
        )
        repeat_pose, repeat_cov = (
            recentered.solve()
            if mode.endswith("linear")
            else (pose.copy(), covariance.copy())
        )
        for tick, event, p, c in (
            (0, "publish_from_retained_representation", pose, covariance),
            (1, "recenter_same_quadratic_or_repeat", repeat_pose, repeat_cov),
            (2, "authorized_raw_rebuild", reference, reference_cov),
        ):
            trace.append(
                {
                    "mode": mode,
                    "tick": tick,
                    "event": event,
                    "pose": p.tolist(),
                    "chart_covariance": c.tolist(),
                    "represented_ids": list(kept_ids),
                    "permitted_ids": list(data.ids),
                    "forbidden_ids": sorted(set(kept_ids) - set(data.ids)),
                    "acquired_observation_count": 4,
                    "new_observations_since_tick0": 0,
                    "policy": "P0",
                    "validity": "authorized_estimate_approximation_not_certified",
                }
            )
        methods[mode]["rebuild_jump_angle_rad"] = abs(
            wrap(reference[0] - repeat_pose[0])
        )
        methods[mode]["rebuild_max_query_jump_m"] = float(
            np.max(
                np.linalg.norm(
                    transform(reference, QUERY_POINTS)
                    - transform(repeat_pose, QUERY_POINTS),
                    axis=1,
                )
            )
        )
        methods[mode]["query_total_variation_m"] = methods[mode][
            "rebuild_max_query_jump_m"
        ] + float(
            np.max(
                np.linalg.norm(
                    transform(pose, QUERY_POINTS)
                    - transform(repeat_pose, QUERY_POINTS),
                    axis=1,
                )
            )
        )
        methods[mode]["rebuild_covariance_relative_change"] = relative_error(
            reference_cov, repeat_cov
        )
    return {
        "config": {
            "scale": scale,
            "offset": offset,
            "sigma": sigma,
            "noise": noise,
            "delta_deg": delta,
        },
        "actual_anchor_to_optimum_rad": abs(wrap(reference[0] - frozen.anchor[0])),
        "moment_objective_max_relative_error": max(objectives),
        "raw_hessian_finite_difference_relative_error": relative_error(
            fd, raw_hessian(data, reference)
        ),
        "angular_normalization_grid_relative_error": angular.grid_norm_relative_error,
        "angular_normalization_error_estimate": angular.norm_error,
        "grid_beats_svd_by_sse": angular.grid_beats_svd_by,
        "frozen_noiseless_formula_error": abs(
            wrap(
                frozen_pose[0] - (-math.radians(delta) + math.sin(math.radians(delta)))
            )
        )
        if noise == "zero"
        else None,
        "methods": methods,
    }, trace


def validate_run(metric, trace):
    m = metric["methods"]["nonlinear_moments"]
    assert m["angle_error_rad"] < 1e-9 and m["translation_error_m"] < 1e-9
    assert m["covariance_relative_error"] < 1e-9
    assert metric["moment_objective_max_relative_error"] < 1e-10
    assert metric["raw_hessian_finite_difference_relative_error"] < 1e-6
    assert metric["angular_normalization_grid_relative_error"] < 1e-8
    assert metric["angular_normalization_error_estimate"] < 1e-8
    assert metric["grid_beats_svd_by_sse"] < 1e-9
    if metric["frozen_noiseless_formula_error"] is not None:
        assert metric["frozen_noiseless_formula_error"] < 1e-10
    for mode in MODES:
        result = metric["methods"][mode]
        assert result["chart_equivalence_relative_error"] < 1e-10
        assert result["quadrature_mass_error_estimate"] < 1e-8
        assert -1e-10 <= result["nominal_95_interval_actual_mass"] <= 1 + 1e-10
    assert all(
        not row["forbidden_ids"] and row["new_observations_since_tick0"] == 0
        for row in trace
    )


def insufficiency_witness(c):
    """Same entire frozen representation, different required nonlinear answers."""
    if c not in (0.5, 1.5):
        raise ValueError("predeclared witness only")
    base = fixture(sigma=0.2)
    worlds, frozen = [], []
    for multiplier in (1 + c, 1 - c):
        data = Data(base.p, multiplier * base.p, base.sigma, base.ids)
        quadratic = FrozenQuadratic.freeze(data, np.zeros(3))
        frozen.append(quadratic)
        pose, covariance = raw_batch(data)
        moment_pose, moment_cov = MomentSummary.freeze(data).solve()
        assert abs(wrap(pose[0] - moment_pose[0])) < 1e-12
        assert relative_error(covariance, moment_cov) < 1e-12
        worlds.append(
            {
                "measurement_multiplier": multiplier,
                "raw_pose": pose.tolist(),
                "raw_local_chart_covariance": covariance.tolist(),
                "frozen_pose": quadratic.solve()[0].tolist(),
                "frozen_chart_covariance": quadratic.solve()[1].tolist(),
            }
        )
    for name in ("anchor", "h", "eta"):
        np.testing.assert_array_equal(
            getattr(frozen[0], name), getattr(frozen[1], name)
        )
    assert frozen[0].constant == frozen[1].constant
    assert frozen[0].ids == frozen[1].ids
    return {
        "c": c,
        "worlds": worlds,
        "identical_retained_state": {
            "anchor": frozen[0].anchor.tolist(),
            "h": frozen[0].h.tolist(),
            "eta": frozen[0].eta.tolist(),
            "constant": frozen[0].constant,
            "ids": list(frozen[0].ids),
            "policy": "P0",
        },
        "scope": "Specified first-order quadratic plus identity-only metadata, no raw-value hashes",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=False)
    metrics, traces = [], []
    for config in grid():
        metric, trace = run_scenario(*config)
        validate_run(metric, trace)
        metrics.append(metric)
        # Eight complete scenarios: broad/narrow, centered/offset, small/large anchor.
        if config[2] == 0.2 and config[3] == "zero" and config[4] in (0, 90):
            traces.append({"config": metric["config"], "events": trace})
    trace_path = args.output_dir / "representative_runs.jsonl"
    trace_path.write_text(
        "".join(json.dumps(t, sort_keys=True, allow_nan=False) + "\n" for t in traces)
    )
    totals = {
        mode: {
            "outputs": len(metrics),
            "angle_diagnostic_failures": sum(
                not m["methods"][mode]["one_degree_diagnostic_pass"] for m in metrics
            ),
            "query_diagnostic_failures": sum(
                not m["methods"][mode]["one_cm_query_diagnostic_pass"] for m in metrics
            ),
            "mass_diagnostic_failures": sum(
                not m["methods"][mode]["mass_diagnostic_pass"] for m in metrics
            ),
            "max_query_rebuild_jump_m": max(
                m["methods"][mode]["rebuild_max_query_jump_m"] for m in metrics
            ),
            "max_angle_rebuild_jump_rad": max(
                m["methods"][mode]["rebuild_jump_angle_rad"] for m in metrics
            ),
            "min_nominal_95_interval_actual_mass": min(
                m["methods"][mode]["nominal_95_interval_actual_mass"] for m in metrics
            ),
        }
        for mode in MODES
    }
    root = Path(__file__).resolve().parent
    results = {
        "cycle": "0010",
        "configuration_count": len(metrics),
        "method_output_count": len(metrics) * len(MODES),
        "event_count": len(metrics) * len(MODES) * 3,
        "random_trials": 0,
        "totals": totals,
        "runtime": {
            "python": platform.python_version(),
            "numpy": np.__version__,
            "scipy": scipy.__version__,
        },
        "resource_counts": {
            "raw_numeric_slots_including_sigma": 17,
            "moment_numeric_slots_including_count_sigma": 10,
            "quadratic_numeric_slots_symmetric_h_anchor_eta_constant": 13,
            "identity_entries_each": 4,
            "max_angle_grid_points": 8192,
            "quad_subdivision_limit": 100,
            "input_limit": 32,
            "representative_scenarios": len(traces),
            "note": "Payload slots, not Python memory/serialization sizes; construction/oracle scratch excluded",
        },
        "metrics": metrics,
        "insufficiency_witnesses": [insufficiency_witness(c) for c in (0.5, 1.5)],
        "traces_sha256": hashlib.sha256(trace_path.read_bytes()).hexdigest(),
        "source_sha256": {
            name: hashlib.sha256((root / name).read_bytes()).hexdigest()
            for name in (
                "nonlinear_freeze.py",
                "test_nonlinear_freeze.py",
                "cycles/0010-protocol.md",
            )
        },
    }
    (args.output_dir / "results.json").write_text(
        json.dumps(results, sort_keys=True, indent=2, allow_nan=False) + "\n"
    )
    print(json.dumps(totals, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
