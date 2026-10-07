# Cycle 0003 protocol — finite jerk and stopping latency

Declared 2026-10-07, before implementing/running this slice. Parent: `bb7452e`.
Scope: one extension of S03, using the necessary-correction event state from 0002.
This is a scalar kinematic experiment, not a controller or hardware certification.

## Question and policy boundary

Can the stopping fallback fit within the same clearance once acceleration cannot
change instantaneously? Test a simple bounded-jerk stop; distinguish its feasibility
from a lower bound that no admissible braking trajectory can beat in this model.
Never interpret a failed candidate as proof that every possible stop must fail.

At event time zero, alignment is unavailable immediately. In the P3 harness its
mean, bound and pending transition are erased using the existing 0002 consumer;
late messages cannot restore permission. The brake planner receives only independent,
still-authorized local velocity, measured actuator acceleration and actuator limits.
It receives neither alignment, global task target nor obstacle position. Holding the
measured actuator acceleration during a physical response delay does not re-evaluate
the withdrawn target. This is a conditional policy assumption: independent local state
and this residual physical actuation must be permitted. Erasure of prior physical
effects is not modeled as a possible instruction.

The same kinematic calculation applies when a precision gate rejects an otherwise
authorized estimate. That gate and P3 have different semantics. This slice does not
design re-acquisition, permission transitions or time until revocation is delivered;
the delay begins after the local consumer has observed the invalidation.

## Plant, candidates and independent references

Fixed local scalar frame, metres and seconds; x(0)=0 at the invalidation event.
Initial forward velocity v >= 0 and acceleration 0 <= a <= A. A=2 m/s^2.
No external force, sensor error, collisions/contact response, quantized actuator,
communication latency or changing limits. Clearance is evaluator-only ground truth.

Candidate: hold a for delay d; apply jerk -J down to peak deceleration -b;
hold -b if needed; apply jerk +J until v=0 and a=0 together. Choose b <= A using
the velocity-area balance. Integrate constant-jerk segments with exact polynomial
updates, splitting at phase endpoints. Never clip velocity to zero while leaving
negative acceleration or reset acceleration discontinuously. All phases bounded.

Independent distance oracle: after the delay, v_d=v+a*d and x_d=v*d+a*d^2/2.
After ramping positive acceleration to zero, w=v_d+a^2/(2J), and the distance added
is v_d*a/J+a^3/(3J^2). The remaining symmetric deceleration has duration
T=2*sqrt(w/J) if w <= A^2/J, otherwise T=w/A+A/J, and distance w*T/2.
Thus the reference distance is x_d+v_d*a/J+a^3/(3J^2)+w*T/2.
This oracle does not build the candidate's phase list or call its integrator.

Separate necessary lower bound: acceleration cannot become smaller than
max(a-J*t, -A) after the delay. Integrate that strongest permissible negative jerk
until the FIRST zero of velocity, allowing a nonzero terminal acceleration.
Every admissible trajectory has at least this forward excursion before that time.
It is a lower bound, not a feasible rest state under the jerk constraint. Clearance
below it implies unavoidable boundary entry under these fixed assumptions. The gap
between that bound and the candidate distance is not declared globally resolved.

Negative controls: v^2/(2A) assumes immediate full braking and ignores both positive
initial acceleration and delay; retain every false-clear prediction. Also reject the
tempting implementation that clamps v=0 then resets a=0 after strongest braking,
because its acceleration jump violates finite jerk.

## Fixed sweep and resource budget

- Velocity v in {0.3, 0.8, 1.1} m/s.
- Initial acceleration a in {0, 2} m/s^2.
- Maximum jerk J in {4, 10, 40} m/s^3.
- Physical response delay d in {0, 0.05, 0.15} s.
- Evaluator clearance c in {0.1, 0.3, 0.6575, 1.2} m.
- Primary sampling dt=0.01 s; refinements 0.005 and 0.0025 s.

54 kinematic profiles, 216 clearance comparisons, 108 refined profiles. Clearance
does not change the braking plan, so do not pretend its repeated comparisons are
independent trajectories. All inputs deterministic; no random seed or statistical
risk estimate. Cap each trajectory at 10 s and 20,000 steps; refuse invalid requests.
These timesteps sample the same event-aligned continuous profile, not a discretized
controller's reaction time, and do not establish digital-control robustness.

Anchor: the 0002 necessary-correction event has v=1.1, a=+2 at t=0.15 s,
x_local=0.1425, true offset 0.4, wall 1.2; remaining clearance is 0.6575 m.
Its old acceleration-only stopping distance is 0.3025 m. Report all nine J,d
combinations at that state, without changing the pre-event history. Save complete
primary-step traces for these nine and two a=0,d=0 profiles (v=0.3 and 1.1,J=4),
covering triangular and saturated braking. Preserve other profiles as compact metrics.

Parameter-bound sensitivity, separate from estimator uncertainty: for each of the
54 profiles, compute a conservative candidate-distance bound when actual starting
speed may be up to 0.05 m/s higher and response delay up to 0.02 s longer. Use the
upper corner and verify all four interval corners with the independent oracle and
an upper-corner simulation. The distance is monotone in v,d for this nonnegative-a
domain. This bounds the prescribed profile with actual state available at planning;
it does NOT show that a plan computed from a wrong speed still stops correctly.
Global alignment stays unavailable with no reported posterior/covariance. Record
nominal fits that lose their margin under this parameter allowance.

## Predeclared checks, hypotheses and interpretation

- Terminal |v|, |a| <= 1e-10; no reverse velocity below -1e-10; acceleration <= A
  and jerk <= J plus 1e-10; distance/time agreement with oracle within 1e-10.
- Zero unauthorized alignment-use steps; unavailable at t=0 even during physical
  delay. P3 clears numerical alignment state; old/new estimate delivery is rejected.
- Candidate distance >= necessary lower bound >= v^2/(2A), tolerance 1e-10.
- All crossing/fit classifications away from exact equality agree at both refinements.
  Equality is boundary contact, not a strictly positive clearance margin.
- Expect at least one naive false-clear prediction and at least one anchor crossing;
  retain contrary outcomes rather than tuning parameters. Not every anchor crossing
  is assumed unavoidable; report the lower-bound comparison explicitly.
- Preserve an independent closed-form symmetric triangular example and saturated
  example, plus limit/error-handling and the forbidden acceleration-reset control.
- Report accuracy (oracle residual), parameter bounds, availability/authorization,
  distance, time, peak speed/acceleration/jerk and clearance together. No pooled score.

Return to the queued R01/R02/R04/R08 entitlement/replay work after this cycle. An
observed finite-clearance failure extends T02; it does not create a new permission
rule, justify smoothing revoked state, or demand more controller research now.
