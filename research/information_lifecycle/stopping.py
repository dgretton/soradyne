"""Cycle 0003: scalar stopping research; no hardware or production controller."""

from __future__ import annotations

import argparse
import csv
import hashlib
import itertools
import json
import math
import platform
from dataclasses import asdict, dataclass, replace
from pathlib import Path

from dynamics import Consumer, Estimate

EPS = 1e-10
CLEARANCES = (0.1, 0.3, 0.6575, 1.2)


@dataclass(frozen=True)
class Config:
    velocity: float
    acceleration: float
    jerk_limit: float
    delay: float
    acceleration_limit: float = 2.0

    def validate(self):
        if not all(math.isfinite(value) for value in asdict(self).values()):
            raise ValueError("Nonfinite configuration")
        if not (
            self.velocity >= 0
            and 0 <= self.acceleration <= self.acceleration_limit
            and self.jerk_limit > 0
            and self.acceleration_limit > 0
            and self.delay >= 0
        ):
            raise ValueError("Outside forward/nonnegative-acceleration model domain")


@dataclass(frozen=True)
class Phase:
    name: str
    duration: float
    jerk: float


def stop_plan(config: Config) -> list[Phase]:
    """Only independently authorized local state and plant limits enter this plan."""
    config.validate()
    v, a = config.velocity, config.acceleration
    j, d, limit = config.jerk_limit, config.delay, config.acceleration_limit
    delayed_v = v + a * d
    peak = min(limit, math.sqrt(j * delayed_v + a * a / 2))
    hold = (
        max(0.0, (delayed_v + a * a / (2 * j) - peak * peak / j) / peak)
        if peak
        else 0.0
    )
    phases = [
        Phase("physical_delay", d, 0.0),
        Phase("ramp_down", (a + peak) / j, -j),
        Phase("hold_deceleration", hold, 0.0),
        Phase("ramp_up", peak / j, j),
    ]
    return [phase for phase in phases if phase.duration > 0]


def distance_oracle(config: Config) -> tuple[float, float]:
    """Closed form via positive-acceleration removal and symmetric velocity area.

    Does not call the planner or step integrator. Assumes exact local state.
    """
    config.validate()
    v, a = config.velocity, config.acceleration
    j, d, limit = config.jerk_limit, config.delay, config.acceleration_limit
    delayed_v = v + a * d
    w = delayed_v + a * a / (2 * j)
    symmetric_time = (
        2 * math.sqrt(w / j) if w <= limit * limit / j else w / limit + limit / j
    )
    distance = (
        v * d
        + a * d * d / 2
        + delayed_v * a / j
        + a**3 / (3 * j * j)
        + w * symmetric_time / 2
    )
    return distance, d + a / j + symmetric_time


def first_zero_lower_bound(config: Config) -> tuple[float, float, float]:
    """Necessary excursion under strongest braking, NOT a feasible terminal rest.

    A subsequent jump from returned acceleration to zero would violate finite jerk.
    """
    config.validate()
    v, a = config.velocity, config.acceleration
    j, d, limit = config.jerk_limit, config.delay, config.acceleration_limit
    delayed_v = v + a * d
    delayed_x = v * d + a * d * d / 2
    ramp_time = min((a + math.sqrt(a * a + 2 * j * delayed_v)) / j, (a + limit) / j)
    ramp_x = delayed_v * ramp_time + a * ramp_time**2 / 2 - j * ramp_time**3 / 6
    ramp_v = max(0.0, delayed_v + a * ramp_time - j * ramp_time**2 / 2)
    return (
        delayed_x + ramp_x + ramp_v * ramp_v / (2 * limit),
        d + ramp_time + ramp_v / limit,
        a - j * ramp_time,
    )


def advance(x: float, v: float, a: float, j: float, h: float):
    """Exact constant-jerk update without zero-speed/acceleration clipping."""
    a1 = a + j * h
    v1 = v + (a + a1) * h / 2
    x1 = x + v * h + (a / 3 + a1 / 6) * h * h
    return x1, v1, a1


def simulate_stop(config: Config, dt=0.01):
    phases = stop_plan(config)
    if not math.isfinite(dt) or dt <= 0:
        raise ValueError("Invalid timestep")
    steps = sum(math.ceil(phase.duration / dt) for phase in phases)
    if steps > 20_000 or sum(p.duration for p in phases) > 10:
        raise ValueError("Trajectory exceeds step/time budget")

    # This is the old P3 event model, not a new distributed permission mechanism.
    consumer = Consumer("direct", 0.08)
    consumer.receive(Estimate(1, 0, 0.4, 0.02), 0)
    consumer.revoke()
    initial_output = consumer.step(0, dt)
    late_accepted = consumer.receive(Estimate(2, 1, 0.7, 0.02), 0)
    x, v, a, t = 0.0, config.velocity, config.acceleration, 0.0
    min_v = max_v = v
    max_a, max_j = abs(a), 0.0
    available_after_withdrawal_steps = 0
    trace = []
    for phase in phases:
        count = math.ceil(phase.duration / dt)
        for index in range(count):
            h = min(dt, phase.duration - index * dt)
            x1, v1, a1 = advance(x, v, a, phase.jerk, h)
            velocities = [v, v1]
            if phase.jerk and 0 < -a / phase.jerk < h:
                tau = -a / phase.jerk
                velocities.append(v + a * tau + phase.jerk * tau * tau / 2)
            min_v, max_v = min(min_v, *velocities), max(max_v, *velocities)
            max_a = max(max_a, abs(a1))
            max_j = max(max_j, abs(phase.jerk))
            output = consumer.step(t, h)
            available_after_withdrawal_steps += int(output.available)
            trace.append(
                {
                    "t": t,
                    "phase": phase.name,
                    "h": h,
                    "x": x,
                    "v": v,
                    "a": a,
                    "jerk": phase.jerk,
                    "x_next": x1,
                    "v_next": v1,
                    "a_next": a1,
                    "alignment_available": output.available,
                    "alignment_mean": output.offset,
                    "alignment_bound": output.bound,
                }
            )
            x, v, a, t = x1, v1, a1, t + h
    oracle_distance, oracle_time = distance_oracle(config)
    lower_distance, lower_time, terminal_a = first_zero_lower_bound(config)
    metrics = {
        "config": asdict(config),
        "dt": dt,
        "steps": len(trace),
        "distance": x,
        "stop_time": t,
        "final_velocity": v,
        "final_acceleration": a,
        "min_velocity": min_v,
        "max_velocity": max_v,
        "max_acceleration": max_a,
        "max_jerk": max_j,
        "oracle_distance": oracle_distance,
        "oracle_time": oracle_time,
        "distance_error": x - oracle_distance,
        "time_error": t - oracle_time,
        "necessary_distance_lower_bound": lower_distance,
        "lower_bound_first_zero_time": lower_time,
        "forbidden_terminal_acceleration_reset": abs(terminal_a),
        "naive_acceleration_only_distance": config.velocity**2
        / (2 * config.acceleration_limit),
        "alignment_unavailable_at": 0 if not initial_output.available else None,
        "alignment_fields_remaining": sum(
            state is not None
            for state in (consumer.latest, consumer.applied, consumer.pending)
        ),
        "late_estimate_accepted": late_accepted,
        "available_alignment_steps_after_withdrawal": available_after_withdrawal_steps,
    }
    return metrics, trace


def first_crossing(trace: list[dict], clearance: float):
    """Evaluator-only root; profiles have nonnegative velocity within tolerance."""
    for row in trace:
        if row["x_next"] > clearance + EPS:
            lo, hi = 0.0, row["h"]
            for _ in range(50):
                mid = (lo + hi) / 2
                position = (
                    row["x"]
                    + row["v"] * mid
                    + row["a"] * mid**2 / 2
                    + row["jerk"] * mid**3 / 6
                )
                if position < clearance:
                    lo = mid
                else:
                    hi = mid
            return row["t"] + (lo + hi) / 2
    return None


def assess_clearance(metrics: dict, clearance: float) -> dict:
    margin = clearance - metrics["distance"]
    if margin < -EPS:
        classification = (
            "unavoidable_under_model"
            if clearance < metrics["necessary_distance_lower_bound"] - EPS
            else "candidate_crosses_lower_bound_inconclusive"
        )
    elif margin <= EPS:
        classification = "boundary_contact"
    else:
        classification = "candidate_fits"
    naive_fits = metrics["naive_acceleration_only_distance"] < clearance - EPS
    return {
        "clearance": clearance,
        "minimum_clearance": margin,
        "classification": classification,
        "naive_claims_positive_margin": naive_fits,
        "naive_false_clear": naive_fits and margin < -EPS,
    }


def configurations():
    return [
        Config(v, a, j, d)
        for v, a, j, d in itertools.product(
            (0.3, 0.8, 1.1), (0.0, 2.0), (4.0, 10.0, 40.0), (0.0, 0.05, 0.15)
        )
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    primary, refinements, sensitivity = [], [], []
    for index, config in enumerate(configurations()):
        name = f"profile-{index:02d}"
        metrics, trace = simulate_stop(config)
        metrics["id"] = name
        metrics["clearances"] = [
            assess_clearance(metrics, c) | {"first_crossing": first_crossing(trace, c)}
            for c in CLEARANCES
        ]
        primary.append(metrics)
        anchor = config.velocity == 1.1 and config.acceleration == 2
        symmetric = (
            config.velocity in (0.3, 1.1)
            and config.acceleration == 0
            and config.jerk_limit == 4
            and config.delay == 0
        )
        # Retain the observed gap case too, without adding/tuning configurations.
        inconclusive_crossing = any(
            c["classification"] == "candidate_crosses_lower_bound_inconclusive"
            for c in metrics["clearances"]
        )
        if anchor or symmetric or inconclusive_crossing:
            with (args.output_dir / f"{name}.csv").open("w", newline="") as handle:
                writer = csv.DictWriter(
                    handle, fieldnames=list(trace[0]), lineterminator="\n"
                )
                writer.writeheader()
                writer.writerows(trace)
        for dt in (0.005, 0.0025):
            refined, _ = simulate_stop(config, dt)
            refinements.append(refined | {"id": name})

        upper_config = replace(
            config, velocity=config.velocity + 0.05, delay=config.delay + 0.02
        )
        upper, _ = simulate_stop(upper_config)
        corner_distances = [
            distance_oracle(
                replace(config, velocity=config.velocity + dv, delay=config.delay + dd)
            )[0]
            for dv, dd in itertools.product((0.0, 0.05), (0.0, 0.02))
        ]
        sensitivity.append(
            {
                "id": name,
                "upper_corner": upper,
                "corner_distances": corner_distances,
                "nominal_positive_margins_lost": [
                    c
                    for c in CLEARANCES
                    if metrics["distance"] < c - EPS and upper["distance"] > c + EPS
                ],
            }
        )
    root = Path(__file__).resolve().parent
    result = {
        "cycle": "0003",
        "parent_commit": "bb7452e25ccbfecaeb430d99017401ddf97e43d4",
        "python": platform.python_version(),
        "scope": "Deterministic scalar stopping; exact local state, fixed limits; no hardware validation",
        "clearances": CLEARANCES,
        "primary": primary,
        "refinements": refinements,
        "parameter_sensitivity": sensitivity,
        "parameter_allowance": {"speed_m_per_s": 0.05, "delay_s": 0.02},
        "source_sha256": {
            path: hashlib.sha256((root / path).read_bytes()).hexdigest()
            for path in (
                "stopping.py",
                "test_stopping.py",
                "dynamics.py",
                "cycles/0003-protocol.md",
            )
        },
    }
    (args.output_dir / "results.json").write_text(
        json.dumps(result, indent=2, sort_keys=True) + "\n"
    )
    assessments = [c for m in primary for c in m["clearances"]]
    print(
        json.dumps(
            {
                "profiles": len(primary),
                "refinements": len(refinements),
                "upper_corner_profiles": len(sensitivity),
                "comparisons": len(assessments),
                "naive_false_clear": sum(c["naive_false_clear"] for c in assessments),
                "classifications": {
                    label: sum(c["classification"] == label for c in assessments)
                    for label in sorted({c["classification"] for c in assessments})
                },
                "max_distance_error": max(abs(m["distance_error"]) for m in primary),
                "nominal_margins_lost": sum(
                    len(s["nominal_positive_margins_lost"]) for s in sensitivity
                ),
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
