# Investigation status

Started 2026-10-07. Branch: `research/information-lifecycle`.
Base: `9cbba49` (`shared-flow-demo-contracts`).

## Current cycle

0009: bounded R03+F03 retention-expiry/withdrawal slice completed. 192 configurations,
six methods: 1,152 runs / 14,976 query outcomes. Three strict methods pass all 7,488
combined checks, reporting unavailable when full requested history is irrecoverable.
Per-record versus per-source decomposition supports different withdrawal units; a
two-world witness proves the stored aggregate/ID manifest insufficient for finer removal.
Explicit reduced answers preserve lawful subset information with honest coverage and
uncertainty. Replay must finish before raw expiry under the declared no-pin policy.
Admission regressions (deadline relabeling, future artifacts and mutation before rejecting
over-capacity input) were fixed; all grid metrics are unchanged. 166 tests pass; all nine
cycles' source hashes are verified.
No complete family or production mechanism is declared proven. Publication is verified
against the remote tip at cycle close.

## Next bounded chunk

0010: I06's first nonlinear freeze slice. Use a small SE(2) pose/landmark model, a frozen
linearization, controlled angular displacement from it and a full-data batch reference.
Predeclare geometry/noise, a bounded sweep and independent residual/error/uncertainty
checks. Separate approximation error from information loss or permission changes. Start
with deterministic geometry; add fixed-seed coverage only after validating the independent
reference and within the cycle budget. Keep broader SE(3), timing faults and physical
consumers queued.

Reason for this priority: the exact information, permissions and initial recovery cases
now expose what retained state can and cannot support. Follow the existing sequence into
nonlinear approximation, where exact scalar successes cannot justify a frozen pose prior.

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
- F01/F02: partial exact scalar evidence in 0008 for unequal live delivery at matching
  durable horizons, manifest-bound checkpoint/replay, missing custody coverage, producer
  disappearance, duplicate/reordered replay and P3 during recovery. A surviving custodian
  supplies authorized original records; loss of reachability is bounded by the fixture.
  General restart state, atomic persistence, distributed authority and latency remain open.
- R03+F03: first bounded compound evidence in 0009. Static scalar summaries, per-record
  versus per-source withdrawal, half-open raw/derivative expiry, replay completion deadline,
  explicit reduced coverage and a retained-state insufficiency witness. Moving-state lag
  elimination, alternate retention policies, nonlinear state and real latency remain open.
- I06-I08, R05/R07, R09-R10, F04-F06, S02 and S04-S08 have no dedicated scenario-level
  coverage yet. R05 has an adjacent single-record example in 0009, not its full scope.
- Mandatory compounds: R03+F03 has the limited 0009 witness; other compounds remain planned.
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
- [0008](cycles/0008.md): actual-input manifests, unequal live-history failover,
  custody/replay gaps, checkpoint withdrawal and output-seeded double counting.
- [0009](cycles/0009.md): withdrawal granularity versus summary decomposition,
  raw/derivative expiry, bounded replay and explicit reduced-information answers.

## Scheduling

Requested: a heartbeat in the current chat every four hours, one bounded chunk,
commit/push every cycle including temporary/negative work. See README for procedure.
Automation: `information-lifecycle-investigation`, confirmed ACTIVE, same-chat
heartbeat, four-hour interval. Each run follows README and preserves evidence in Git.
Local execution requires this computer and the app to be running.
