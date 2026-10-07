# Investigation status

Started 2026-10-07. Branch: `research/information-lifecycle`.
Base: `9cbba49` (`shared-flow-demo-contracts`).

## Current cycle

0004: scoped entitlement/replay slice completed. Six fixtures, six initial arrival
orders, five candidates: 180 traces / 2,850 event-query checks. The scoped candidate
matches all 570 of its checks exactly; every negative control is detected. 57 total
tests passed, including a regression for a corrected grant-container mutability bug.
P2 derivative retention, P3 invalidation, surviving grant paths and explicit replay
gaps have limited exact-scalar evidence. No whole family or production mechanism is
declared proven. Publication is verified against the remote tip at cycle close.

## Next bounded chunk

0005: I03 shared evidence and shared calibration in tiny exact linear examples.
Compare source-ID deduplication, a retained shared latent/joint representation and
authorized raw rebuild with an independent batch oracle. Include a diamond with
duplicated evidence, and disjoint observation IDs sharing an uncertain calibration.
Measure mean, joint/query uncertainty, lineage and supported/unsupported behavior;
do not infer independence solely from disjoint IDs. Keep feedback policy (I04), broader
gauge tests (I05) and nonlinear work queued rather than expanding this cycle.

Reason for this priority: 0004 handles disjoint independent evidence and explicitly
rejects partial summary overlap. Test the next information-preservation assumption
before attempting general nesting or promoting this permission prototype.

## Coverage

- I01, I02, R03: initial exact witnesses established in 0001, including an
  insufficiency counterexample for selective withdrawal from collapsed state. 0004
  adds a policy-driven mixed-summary invalidation/missing-component replay witness.
- S01/S03: partial deterministic scalar evidence in 0002-0003; prescribed estimator
  traces and ideal local sensing. 0003 adds finite jerk, fixed physical response delay
  and a necessary stopping-distance lower bound for nonnegative initial acceleration.
  General consumer stability and realistic actuator/network behavior remain open.
- R01/R02/R04/R08: partial exact scalar event evidence in 0004. Specific source-
  sequence cutoff, explicit artifact permissions, grant unions and re-grant history
  ranges; trusted atomic local policies only. General enforcement remains open.
- I03: only a preliminary unsupported-overlap guard in 0004; shared correlation and
  nested/diamond composition are next. I04-I08, R05-R07, R09-R10, F01-F06, S02 and
  S04-S08: planned, no scenario-level test evidence yet.
- Mandatory compound scenarios: planned, none executed.
- Production changes: none. Distributed, nonlinear and hardware claims: none.

## Durable history

- [0001](cycles/0001.md): plan and exact-arithmetic baseline.
- [0002](cycles/0002.md): reference transitions, delayed corrections and P3 withdrawal.
- [0003](cycles/0003.md): finite jerk, stopping margin, immediate invalidation and
  physical response delay; retained counterexamples and parameter-bound sensitivity.
- [0004](cycles/0004.md): scoped grant/retention unions, retained derivatives, strict
  withdrawal, stale tokens/results and re-grant replay gaps with exact scalar inference.

## Scheduling

Requested: a heartbeat in the current chat every four hours, one bounded chunk,
commit/push every cycle including temporary/negative work. See README for procedure.
Automation: `information-lifecycle-investigation`, confirmed ACTIVE, same-chat
heartbeat, four-hour interval. Each run follows README and preserves evidence in Git.
Local execution requires this computer and the app to be running.
