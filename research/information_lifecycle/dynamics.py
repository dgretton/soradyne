"""Cycle 0002: deterministic scalar research simulation, never hardware control."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import platform
from dataclasses import asdict, dataclass
from pathlib import Path

EPS = 1e-10
MODES = (
    "direct",
    "coalesce",
    "persistence",
    "slew",
    "gated_slew",
    "unsafe_keep_on_revoke",
)


@dataclass(frozen=True)
class Estimate:
    revision: int
    frontier: int
    offset: float
    bound: float


@dataclass(frozen=True)
class Output:
    available: bool
    offset: float | None
    bound: float | None


class Consumer:
    """No access to truth, wall, plant or simulator. Time is externally supplied.

    Error bounds describe intervals, not covariance. All numerical alignment state
    is cleared at strict withdrawal except in the explicitly unsafe negative control.
    This process models the authorized consumer, not erasure of all recorded traces.
    """

    def __init__(self, mode: str, budget: float, period=0.5, dwell=0.15, rate=0.05):
        if mode not in MODES or min(budget, period, dwell, rate) <= 0:
            raise ValueError("Invalid consumer configuration")
        self.mode, self.budget = mode, budget
        self.period, self.dwell, self.rate = period, dwell, rate
        self.authorized = True
        self.latest: Estimate | None = None
        self.applied: float | None = None
        self.pending: float | None = None
        self.pending_since = 0.0
        self.next_release = 0.0
        self.last_revision = 0

    def receive(self, event: Estimate, t: float) -> bool:
        if event.bound < 0 or not all(
            math.isfinite(v) for v in (event.offset, event.bound)
        ):
            raise ValueError("Invalid estimate")
        if event.revision < 1 or event.frontier < 0:
            raise ValueError("Invalid revision/frontier")
        if not self.authorized:
            return False
        if event.revision < self.last_revision:
            return False
        if event.revision == self.last_revision:
            if event != self.latest:
                raise ValueError("Conflicting payload for the same revision")
            return False
        self.last_revision = event.revision
        self.latest = event
        if self.applied is None:
            self.applied = event.offset
            self.next_release = t + self.period
        if self.pending != event.offset:
            self.pending, self.pending_since = event.offset, t
        return True

    def revoke(self):
        self.authorized = False
        if self.mode != "unsafe_keep_on_revoke":
            self.latest = None
            self.applied = None
            self.pending = None

    def step(self, t: float, dt: float) -> Output:
        if self.latest is None or self.applied is None:
            return Output(False, None, None)
        if self.mode in ("direct", "unsafe_keep_on_revoke"):
            self.applied = self.latest.offset
        elif self.mode == "coalesce":
            if t + EPS >= self.next_release:
                self.applied = self.latest.offset
                while t + EPS >= self.next_release:
                    self.next_release += self.period
        elif self.mode == "persistence":
            if t + EPS >= self.pending_since + self.dwell:
                self.applied = self.pending
        else:
            delta = self.latest.offset - self.applied
            self.applied += max(-self.rate * dt, min(self.rate * dt, delta))
        # Triangle inequality exposes lag from the latest (possibly corrected) mean.
        effective_bound = self.latest.bound + abs(self.applied - self.latest.offset)
        available = self.mode != "gated_slew" or effective_bound <= self.budget + EPS
        return Output(available, self.applied, effective_bound)


@dataclass(frozen=True)
class Case:
    name: str
    true_offset: float
    x0: float
    v0: float
    target_global: float
    wall_global: float
    initial_offset: float
    initial_bound: float
    precision_budget: float
    event_time: float
    duration: float = 4.0
    max_acceleration: float = 2.0
    kp: float = 8.0
    kd: float = 4.0
    independent_input_period: float = 0.5
    revision_period: float = 0.05
    storm_amplitude: float = 0.12
    corrected_offset: float = 0.4


CASES = (
    Case("revision_storm", 0, 0, 0, 0.75, 1.2, 0, 0.12, 0.25, 0.5),
    Case("necessary_correction", 0.4, 0, 0.8, 1.0, 1.2, 0, 0.02, 0.08, 0.15),
    Case("strict_withdrawal", 0, 0, 0.8, 0.35, 0.5, 0, 0.02, 0.08, 0.2),
)


def ticks(seconds: float, dt: float) -> int:
    count = round(seconds / dt)
    if not math.isclose(count * dt, seconds, rel_tol=0, abs_tol=EPS):
        raise ValueError("Timeline event must fall on the selected grid")
    return count


def timeline(case: Case, dt: float) -> dict[int, Estimate | str]:
    events: dict[int, Estimate | str] = {
        0: Estimate(1, 0, case.initial_offset, case.initial_bound)
    }
    start = ticks(case.event_time, dt)
    if case.name == "revision_storm":
        for number, tick in enumerate(
            range(start, ticks(case.duration, dt), ticks(case.revision_period, dt))
        ):
            events[tick] = Estimate(
                number + 2,
                tick // ticks(case.independent_input_period, dt),
                case.storm_amplitude * (1 if number % 2 == 0 else -1),
                case.initial_bound,
            )
    elif case.name == "necessary_correction":
        events[start] = Estimate(2, 1, case.corrected_offset, case.initial_bound)
    elif case.name == "strict_withdrawal":
        events[start] = "revoke"
    else:
        raise ValueError("Unknown case")
    return events


def advance(x: float, v: float, a: float, dt: float, brake: bool = False):
    """Exact constant-acceleration segment, with a rest segment on brake stopping.

    Returns next x/v, the maximum x within the step and the acceleration segment's
    duration. The maximum detects a wall hit even if the final position is clear.
    """
    moving = dt
    if brake and v * a < 0:
        moving = min(dt, abs(v / a))
    x1 = x + v * moving + 0.5 * a * moving**2
    v1 = v + a * moving
    if brake and moving < dt:
        v1 = 0.0
    positions = [x, x1]
    if a != 0 and 0 < -v / a < moving:
        tau = -v / a
        positions.append(x + v * tau + 0.5 * a * tau**2)
    return x1, v1, max(positions), moving


def first_crossing(x: float, v: float, a: float, moving: float, wall_local: float):
    """Independent root calculation for the first nonnegative crossing time."""
    if x >= wall_local:
        return 0.0
    if abs(a) < 1e-14:
        root = (wall_local - x) / v if v > 0 else math.inf
        return root if root <= moving + EPS else None
    discriminant = v * v + 2 * a * (wall_local - x)
    if discriminant < 0:
        return None
    roots = [(-v + sign * math.sqrt(discriminant)) / a for sign in (-1, 1)]
    valid = [max(0.0, r) for r in roots if -EPS <= r <= moving + EPS]
    return min(valid) if valid else None


def simulate(case: Case, mode: str, dt=0.01, replay=False):
    if not math.isfinite(dt) or dt <= 0:
        raise ValueError("Timestep must be finite and positive")
    count = ticks(case.duration, dt)
    if count < 1 or count > 20_000:
        raise ValueError("Simulation exceeds bounded step budget")
    events = timeline(case, dt)
    consumer = Consumer(mode, case.precision_budget)
    x, v = case.x0, case.v0
    applied_previous = case.initial_offset
    acceleration_previous = 0.0
    raw_previous = case.initial_offset
    raw_variation = applied_variation = max_jump = max_rate = 0.0
    max_accel = max_jerk = 0.0
    post_event_max_jerk = post_event_acceleration_variation = 0.0
    corrections = applied_changes = 0
    frontiers = {0}
    unavailable_steps = over_budget_steps = false_bound_steps = unauthorized_steps = 0
    post_event_false_bound_steps = 0
    min_clearance = case.wall_global - (x + case.true_offset)
    crossing_time = first_unavailable = stop_after_withdrawal = None
    withdrew = False
    withdrawal_state = None
    late_errors = []
    trace = []
    latest_seen = None
    for tick in range(count):
        t = tick * dt
        event = events.get(tick)
        if event == "revoke":
            withdrew = True
            withdrawal_state = (x, v)
            consumer.revoke()
        elif isinstance(event, Estimate):
            consumer.receive(event, t)
            latest_seen = event
            frontiers.add(event.frontier)
            if tick:
                raw_variation += abs(event.offset - raw_previous)
                corrections += 1
            raw_previous = event.offset
        if replay and latest_seen is not None:
            # Repeated transport deliveries add no evidence and must not reset dwell.
            consumer.receive(latest_seen, t)
            if latest_seen.revision > 1:
                consumer.receive(events[0], t)
        out = consumer.step(t, dt)
        if out.offset is not None:
            change = abs(out.offset - applied_previous)
            applied_variation += change
            applied_changes += int(change > EPS)
            max_jump, max_rate = max(max_jump, change), max(max_rate, change / dt)
            applied_previous = out.offset
        brake = not out.available
        target_local = None if brake else case.target_global - out.offset
        if brake:
            a = -math.copysign(case.max_acceleration, v) if abs(v) > EPS else 0.0
            unavailable_steps += 1
            if first_unavailable is None:
                first_unavailable = t
        else:
            a = max(
                -case.max_acceleration,
                min(case.max_acceleration, case.kp * (target_local - x) - case.kd * v),
            )
            over_budget_steps += int(out.bound > case.precision_budget + EPS)
            misses_truth = abs(out.offset - case.true_offset) > out.bound + EPS
            false_bound_steps += int(misses_truth)
            post_event_false_bound_steps += int(
                misses_truth and t + EPS >= case.event_time
            )
            unauthorized_steps += int(withdrew)
        max_accel = max(max_accel, abs(a))
        acceleration_change = abs(a - acceleration_previous)
        max_jerk = max(max_jerk, acceleration_change / dt)
        if t + EPS >= case.event_time:
            post_event_max_jerk = max(post_event_max_jerk, acceleration_change / dt)
            post_event_acceleration_variation += acceleration_change
        acceleration_previous = a
        x1, v1, step_max, moving = advance(x, v, a, dt, brake)
        clearance = case.wall_global - (step_max + case.true_offset)
        min_clearance = min(min_clearance, clearance)
        if crossing_time is None:
            tau = first_crossing(x, v, a, moving, case.wall_global - case.true_offset)
            if tau is not None:
                crossing_time = t + tau
        if withdrew and stop_after_withdrawal is None and abs(v1) <= EPS:
            stop_after_withdrawal = t + moving
        if t + EPS >= case.duration / 2:
            late_errors.append((x1 + case.true_offset - case.target_global) ** 2)
        trace.append(
            {
                "t": t,
                "x": x,
                "v": v,
                "a": a,
                "latest_offset": None
                if consumer.latest is None
                else consumer.latest.offset,
                "applied_offset": out.offset,
                "effective_bound": out.bound,
                "available": out.available,
                "authorized": not withdrew,
                "target_local": target_local,
                "braking": brake,
                "x_next": x1,
                "v_next": v1,
                "step_min_clearance": clearance,
            }
        )
        x, v = x1, v1
    metrics = {
        "case": case.name,
        "mode": mode,
        "dt": dt,
        "steps": count,
        "raw_corrections": corrections,
        "independent_input_advances": len(frontiers) - 1,
        "revisions_per_input_advance": corrections / (len(frontiers) - 1)
        if len(frontiers) > 1
        else None,
        "raw_offset_total_variation": raw_variation,
        "applied_offset_total_variation": applied_variation,
        "applied_changes": applied_changes,
        "max_applied_jump": max_jump,
        "max_reference_rate": max_rate,
        "max_commanded_acceleration": max_accel,
        "max_finite_difference_commanded_jerk": max_jerk,
        "post_event_max_finite_difference_commanded_jerk": post_event_max_jerk,
        "post_event_commanded_acceleration_total_variation": post_event_acceleration_variation,
        "min_clearance": min_clearance,
        "first_crossing_time": crossing_time,
        "final_target_error": x + case.true_offset - case.target_global,
        "late_target_rmse": math.sqrt(sum(late_errors) / len(late_errors)),
        "final_local_x": x,
        "final_local_v": v,
        "unavailable_time": unavailable_steps * dt,
        "following_over_precision_budget_time": over_budget_steps * dt,
        "following_outside_reported_bound_time": false_bound_steps * dt,
        "post_event_following_outside_bound_time": post_event_false_bound_steps * dt,
        "unauthorized_follow_time": unauthorized_steps * dt,
        "first_unavailable_time": first_unavailable,
        "stop_time_after_withdrawal": stop_after_withdrawal,
        "withdrawal_local_state": withdrawal_state,
    }
    return metrics, trace


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    metrics = []
    refinements = []
    for case in CASES:
        for mode in MODES:
            summary, trace = simulate(case, mode)
            metrics.append(summary)
            with (args.output_dir / f"{case.name}--{mode}.csv").open(
                "w", newline=""
            ) as handle:
                writer = csv.DictWriter(handle, fieldnames=list(trace[0]))
                writer.writeheader()
                writer.writerows(trace)
            for dt in (0.005, 0.0025):
                refined, _ = simulate(case, mode, dt)
                refinements.append(refined)
    root = Path(__file__).resolve().parent
    result = {
        "cycle": "0002",
        "parent_commit": "5bb4365",
        "python": platform.python_version(),
        "scope": "Deterministic 1D consumer tests; no estimator, probabilistic covariance or hardware validation",
        "configs": [asdict(c) for c in CASES],
        "consumer_parameters": {"period_s": 0.5, "dwell_s": 0.15, "slew_m_per_s": 0.05},
        "controller_dt_s": 0.01,
        "metrics": metrics,
        "timestep_refinements": refinements,
        "source_sha256": {
            name: hashlib.sha256((root / name).read_bytes()).hexdigest()
            for name in ("dynamics.py", "test_dynamics.py", "cycles/0002-protocol.md")
        },
    }
    (args.output_dir / "results.json").write_text(
        json.dumps(result, indent=2, sort_keys=True) + "\n"
    )
    for m in metrics:
        print(
            f"{m['case']:22s} {m['mode']:23s} clearance={m['min_clearance']:.6f} "
            f"TV={m['applied_offset_total_variation']:.6f} lateRMSE={m['late_target_rmse']:.6f} "
            f"unauthorized={m['unauthorized_follow_time']:.3f}s"
        )


if __name__ == "__main__":
    main()
