# Cycle 0002 protocol — declared before running candidates

2026-10-07. Scope: a bounded slice of S01 and S03. These are synthetic deterministic
counterexample/behavior tests, not a controller proposal or physical-safety claim.
No thresholds below will be tuned to make a preferred method pass. An unmet prediction
is recorded as such; subsequent exploration gets a separately identified configuration.

## Model and information boundary

One-dimensional point mass, metres/seconds, fixed local coordinates. The simulator
alone knows the true global offset, target clearance and wall crossing. The consumer
receives local position and velocity (ideal independent encoder), the global target,
and timestamped offset estimates with explicit deterministic error bounds. These
bounds are experimental claims, NOT Gaussian standard deviations or calibrated
posterior covariances. No stochastic coverage claim is made.

Global position is local position + true offset. A consumer offset estimate maps
the global target into a local target by subtraction. The local state and velocity
are never re-expressed or differentiated through an updated global offset. This
isolates target-reference disturbances; coordinate-reset semantics are future S02.
The obstacle is an oracle clearance boundary, not an input to an obstacle planner.
Consequently, results constrain this small tracking task, not navigation generally.

Controller: a = clip(8 * (target_local - x) - 4 * v, -2, 2) m/s^2,
sampled at dt=0.01 s, piecewise-constant acceleration, exact position integration
within each step. When an output is unavailable, brake using only independent local
velocity at up to 2 m/s^2; integrate an in-step stop exactly and do not reverse.
No actuator jerk limit, sensor noise, external force or communication delay is modeled.
Finite-difference commanded jerk is a comparison metric, not a physical jerk bound.

Each run lasts at most 4 s / 400 primary steps. The runner refuses over 20,000 steps
per case. Run timestep refinements at 0.005 and 0.0025 s for every case/method; preserve
their summary outcomes. No parameter/seed search; inputs are specified below.

## Candidate response policies

- direct: apply each newest distinct estimate immediately.
- coalesce: publish the latest mean on fixed 0.5 s boundaries (initial value immediate).
  Do not mistake this rate reduction for new independent evidence or a robust estimator.
- persistence: accept a changed mean only after it persists for 0.15 s. A changed
  pending mean restarts that timer. This can reject chattering but can delay truth.
- slew: move the consumer reference toward the latest estimate at 0.05 m/s.
- gated_slew: same transition, but mark the output unavailable whenever its effective
  error bound exceeds the case's precision budget. Brake on unavailability.
- unsafe_keep_on_revoke: direct response that deliberately ignores strict withdrawal;
  this is an authorization negative control, not an admissible candidate.

Every delayed/transitioned offset reports bound = latest_bound + abs(applied-latest).
This follows the triangle inequality when the latest bound is valid; it makes transition
lag visible rather than relabeling a shifted mean with an unchanged covariance.
The non-gated candidates expose this bound but ignore the task precision budget;
record that violation. None may claim that narrow output smoothness implies precision.
Reject duplicate/stale revisions; equal revisions with unequal contents are errors.
Revisions tied to the same input frontier are not counted as new measurements.

Strict withdrawal bypasses all delays and transitions: erase alignment/reference
state, publish unavailable at the event step, and brake using independent local state.
No subsequent estimate restores authorization without an explicit new grant (re-grant
is out of scope). Test that the unsafe negative control continues unauthorized use.

## Three fixed cases

| Case | True offset; initial x,v | Target; wall (global m) | Input timeline | Precision budget |
|---|---|---|---|---|
| revision_storm | 0; 0,0 | 0.75;1.2 | Initial mean 0, bound 0.12. From t=0.5, alternate +0.12/-0.12 every 0.05 s; independent input frontier advances every 0.5 s | 0.25 m |
| necessary_correction | 0.4;0,0.8 | 1.0;1.2 | Initial mean 0, bound 0.02; at t=0.15 a new input corrects it to 0.4, bound 0.02; then no new input | 0.08 m |
| strict_withdrawal | 0;0,0.8 | 0.35;0.5 | Initial mean 0, bound 0.02; P3 withdrawal at t=0.2 with no replacement | 0.08 m |

All cases run for 4 s. Necessary correction intentionally starts from a wrong
overconfident estimate; its initial bound is falsified equally for all methods.
Measure that lead-in separately from post-event behavior. A genuine corrected estimate
can arrive between coarse publication periods: ignoring it until fresh data is not
assumed safe. These cases do not model a physical bump or a moving wall.

## Recorded measures and predeclared checks

Metrics: raw and applied correction count/total variation/max jump; revisions per
independent input advance; local reference rate; commanded acceleration and jerk;
actual min clearance including within-step position maxima; first crossing; final
and late-window target error; unavailable time; following with an excessive bound;
truth outside declared bound while following; unauthorized use time; first unavailable
time; and stop time after withdrawal. Preserve complete primary-step traces as CSV.
Don't combine authorization/clearance/error/smoothness into an opaque score.

Independent harness checks: constant acceleration and in-step maximum against closed
form; braking distance v^2/(2a); sign of the global-to-local target mapping; duplicate
and stale replay leaves the trajectory unchanged; strict revocation immediately
removes alignment state. Test a regression whose wall crossing occurs inside a step.

Predictions to test, not assumptions of success:

1. In the storm, persistence or slew reduces applied offset total variation by >=75%
   relative to direct. Report whether late target error remains <=0.05 m. Coalescing
   may alias the periodic revisions into a biased stationary reference; don't hide it.
2. Necessary correction: direct avoids the wall whereas an ungated slow transition
   may cross it. Report actual outcomes for all methods, including delayed coalescing.
   The gate should expose the transition error immediately; it is not assumed to
   guarantee a safe stop for arbitrary speed/clearance.
3. All admissible candidates have exactly zero unauthorized-follow steps after P3,
   clear alignment state at the event, and reproduce independent local braking.
   The negative control must be detected. A successful stop does not restore alignment.
4. Qualitative crossing/authorization outcomes must persist at both finer timesteps
   before interpreting them as a useful counterexample. Record numerical differences;
   do not count repeated timesteps as independent statistical trials.

No production algorithm is selected by this cycle. A failed simple policy should
produce a minimal example and a narrower contract, not immediate architectural growth.
