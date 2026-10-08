# Investigation status

Started 2026-10-07. Branch: `research/information-lifecycle`.
Base: `9cbba49` (`shared-flow-demo-contracts`).

## Current cycle

0007: exact anchor-loss/observability slice completed. 24 configurations, five methods:
120 runs / 4,800 event-query outcomes. Per-query rank checks and separate permitted
summaries match all 1,920 of their checks, preserving relative precision through anchor
withdrawal. Blanket joint invalidation needlessly withholds 168 answers; treating a
gauge as absolute evidence invents 432 finite answers. Deliberate anchor retention causes
480 forbidden-processing queries, sometimes despite a numerically correct relative
answer. Coordinate expression changes, input corrections and query validity are recorded
separately. 124 total tests pass; all seven cycles' source hashes are verified.
No complete family or production mechanism is declared proven. Publication is verified
against the remote tip at cycle close.

## Next bounded chunk

0008: F01 with a narrow F02 replay case. Give two producers unequal lossy deliveries
before failover. Compare a durable horizon alone, a checkpoint bound to its actual input
manifest plus authorized replay, and explicit unavailability when retained evidence is
missing. Check exact mean/uncertainty, input identity, duplicates and current permissions;
distinguish published output from restart state. Include an authorized custodian that
survives a producer and a missing component. Keep crash-at-every-write, fencing,
floating-point/nonlinear geometry and consumer dynamics in their planned later slices.

Reason for this priority: the initial I03/I04/I05 witnesses now show what information
must remain separable and queryable. Follow the existing plan into retained-state recovery
and test whether a replacement worker can know and reconstruct what was actually used.

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
- I05/R06: partial exact two-variable evidence in 0007 for weak/singular geometry,
  origin changes, anchor/relative-constraint withdrawal, separate frozen components,
  gauge-induced false certainty and reanchoring. Per-query validity preserves lawful
  observables. Floating-point rank thresholds, nonlinear/scale/connectivity cases and
  physical consumer response remain open.
- I06-I08, R05/R07, R09-R10, F01-F06, S02 and S04-S08 have no dedicated scenario-level
  coverage yet. The next slice addresses bounded F01/F02 recovery.
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
- [0007](cycles/0007.md): query-specific observability after anchor/constraint loss,
  retained independent summaries, covariance, coordinate changes and false gauge precision.

## Scheduling

Requested: a heartbeat in the current chat every four hours, one bounded chunk,
commit/push every cycle including temporary/negative work. See README for procedure.
Automation: `information-lifecycle-investigation`, confirmed ACTIVE, same-chat
heartbeat, four-hour interval. Each run follows README and preserves evidence in Git.
Local execution requires this computer and the app to be running.
