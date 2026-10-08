# Investigation status

Started 2026-10-07. Branch: `research/information-lifecycle`.
Base: `9cbba49` (`shared-flow-demo-contracts`).

## Current cycle

0006: feedback/repeated-influence slice completed. 48 configurations, five methods:
240 runs / 3,040 exact event-query outcomes. Restricted local likelihoods, original-
component accounting and exact bound-base residuals match all 1,824 of their checks.
Newest-posterior reuse fails even with acyclic artifact references; direct-only
revocation leaves forbidden influence. Wrong-base subtraction and bounded-but-wrong
feedback have retained exact witnesses. 107 total tests pass, including corrections
to policy-version metadata and transient peak base counts; numerical and authorization
results are unchanged by those fixes.
No complete family or production mechanism is declared proven. Publication is verified
against the remote tip at cycle close.

## Next bounded chunk

0007: I05 with a narrow R06 anchor-loss case. Use a tiny exact linear system where
withdrawing the last absolute anchor leaves a relative query observable. Compare
explicit observable/unobservable answers, blanket invalidation, and an artificial fixed
gauge that could fabricate absolute precision. Include consistent coordinate-origin
changes, correct relative-query covariance, and immediate withdrawal of anchor influence.
Keep broader floating-point/nonlinear geometry, consumer dynamics and recovery queued.

Reason for this priority: 0005 rejected missing absolute calibration without exposing
remaining observable queries; 0006 confirms that preserved numerical dependencies must
match the actual query. Test what useful information can survive anchor withdrawal
before expanding the recovery and nonlinear studies.

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
- I03: partial exact evidence in 0005 for shared calibration, duplicate observations,
  a two-level diamond, prior updates and withdrawal/rebuild. Known lineage and a
  shared latent do not reconstruct lost overlap values. General nesting and feedback
  remain open. A missing-prior guard is not full I05 coverage.
- I04: partial exact scalar evidence in 0006 for direct/relayed parent feedback,
  versioned replacement, matched conditioning bases, transitive withdrawal and clean
  replay. Immutable artifact acyclicity does not establish independence. General
  feedback, nonlinear messages and asynchronous authority remain open.
- I05-I08, R05-R07, R09-R10, F01-F06, S02 and S04-S08 have no dedicated scenario-level
  coverage yet. The next slice addresses a bounded I05/R06 case.
- Mandatory compound scenarios: planned, none executed.
- Production changes: none. Distributed, nonlinear and hardware claims: none.

## Durable history

- [0001](cycles/0001.md): plan and exact-arithmetic baseline.
- [0002](cycles/0002.md): reference transitions, delayed corrections and P3 withdrawal.
- [0003](cycles/0003.md): finite jerk, stopping margin, immediate invalidation and
  physical response delay; retained counterexamples and parameter-bound sensitivity.
- [0004](cycles/0004.md): scoped grant/retention unions, retained derivatives, strict
  withdrawal, stale tokens/results and re-grant replay gaps with exact scalar inference.
- [0005](cycles/0005.md): shared calibration and evidence, joint composition,
  overlap insufficiency, honest reduced covariance and strict withdrawal/rebuild.
- [0006](cycles/0006.md): parent feedback, original-evidence accounting, exact base
  binding, transitive withdrawal, false precision and delayed correction response.

## Scheduling

Requested: a heartbeat in the current chat every four hours, one bounded chunk,
commit/push every cycle including temporary/negative work. See README for procedure.
Automation: `information-lifecycle-investigation`, confirmed ACTIVE, same-chat
heartbeat, four-hour interval. Each run follows README and preserves evidence in Git.
Local execution requires this computer and the app to be running.
