# Investigation status

Started 2026-10-07. Branch: `research/information-lifecycle`.
Base: `9cbba49` (`shared-flow-demo-contracts`).

## Current cycle

0001: exact scalar/two-variable baselines for I01, I02 and R03. Completed;
9 tests passed, lint and whitespace checks passed. See the cycle report and source-
hashed results. Publication is verified against the remote tip at cycle close.
This is partial evidence only, not completion of those scenario families.

## Next bounded chunk

0002: implement S01's minimal one-dimensional consumer simulation, with a fixed
controller, measurement cadence, clearance and bounded duration. Compare direct
application of oscillating revisions, coalescing/hysteresis, and a permitted bounded
transition. Include a strict-withdrawal invalidation/controlled-braking case (S03)
and a real-correction counterexample where blindly smoothing fails. Record policy
variant and all dynamics; do not claim physical safety from the toy simulation.

## Coverage

- I01, I02, R03: initial exact witnesses established in 0001, including an
  insufficiency counterexample for selective withdrawal from collapsed state.
- I03-I08, R01-R02, R04-R10, F01-F06, S01-S08: planned, no test evidence yet.
- Mandatory compound scenarios: planned, none executed.
- Production changes: none. Distributed, nonlinear and hardware claims: none.

## Durable history

- [0001](cycles/0001.md): plan and exact-arithmetic baseline.

## Scheduling

Requested: a heartbeat in the current chat every four hours, one bounded chunk,
commit/push every cycle including temporary/negative work. See README for procedure.
Automation: `information-lifecycle-investigation`, confirmed ACTIVE, same-chat
heartbeat, four-hour interval. Each run follows README and preserves evidence in Git.
Local execution requires this computer and the app to be running.
