# Investigation status

Started 2026-10-07. Branch: `research/information-lifecycle`.
Base: `9cbba49` (`shared-flow-demo-contracts`).

## Current cycle

0003: finite-jerk/response-delay extension of S03 completed. 54 nominal profiles,
108 refinements and 54 parameter-bound profiles; 39 total tests passed. At the 0002
necessary-correction event, seven of nine jerk/delay variants cannot avoid the boundary
under this scalar model's necessary lower bound. Immediate invalidation still holds.
A separate candidate failure remains explicitly inconclusive against that lower bound.
See the report for limits. No complete scenario family, production algorithm or
hardware safety is declared proven. Publication is verified at cycle close.

## Next bounded chunk

0004: begin the queued R01/R02/R04/R08 entitlement/replay ledger. Use the same exact
scalar evidence under P1 future-only cutoff, P2 explicit derivative retention, P3
withdrawn contribution, and overlapping grants scoped to separate audiences/purposes.
Compare numerical mean/uncertainty and retained/usable records with an independent
authorized-only oracle. Include loss of one grant with another surviving, re-grant
generations, a stale cache delivery and an explicit history gap. Keep it an in-process
bounded event fixture; distributed enforcement and cryptography are not this slice.

Reason for this priority: 0003 answered the bounded stopping question and exposed
the need for a conditional consumer operating envelope. Return to information and
permission semantics now; do not expand into open-ended controller research.

## Coverage

- I01, I02, R03: initial exact witnesses established in 0001, including an
  insufficiency counterexample for selective withdrawal from collapsed state.
- S01/S03: partial deterministic scalar evidence in 0002-0003; prescribed estimator
  traces and ideal local sensing. 0003 adds finite jerk, fixed physical response delay
  and a necessary stopping-distance lower bound for nonnegative initial acceleration.
  General consumer stability and realistic actuator/network behavior remain open.
- I03-I08, R01-R02, R04-R10, F01-F06, S02, S04-S08: planned, no test evidence yet.
- Mandatory compound scenarios: planned, none executed.
- Production changes: none. Distributed, nonlinear and hardware claims: none.

## Durable history

- [0001](cycles/0001.md): plan and exact-arithmetic baseline.
- [0002](cycles/0002.md): reference transitions, delayed corrections and P3 withdrawal.
- [0003](cycles/0003.md): finite jerk, stopping margin, immediate invalidation and
  physical response delay; retained counterexamples and parameter-bound sensitivity.

## Scheduling

Requested: a heartbeat in the current chat every four hours, one bounded chunk,
commit/push every cycle including temporary/negative work. See README for procedure.
Automation: `information-lifecycle-investigation`, confirmed ACTIVE, same-chat
heartbeat, four-hour interval. Each run follows README and preserves evidence in Git.
Local execution requires this computer and the app to be running.
