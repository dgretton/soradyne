# Investigation status

Started 2026-10-07. Branch: `research/information-lifecycle`.
Base: `9cbba49` (`shared-flow-demo-contracts`).

## Current cycle

0011: bounded I06 Monte Carlo angular calibration completed. 131,072 independent
four-observation datasets, six methods, 786,432 interval evaluations. Full circular
intervals are compatible with 95% coverage in all 32 phase/configuration cells under
the predeclared simultaneous test, with an exact conditional-coverage derivation for
this restricted model. Local Gaussian intervals under-cover in all eight weakest cells;
held-out coverage is about 89–90%. Correct full intervals there are typically about
+/-115 degrees, so calibration does not imply useful precision. Frozen-anchor dependence
also survives held-out testing. A library-CDF accuracy limit was caught and corrected
before formal primary/held-out runs; no statistical threshold or seed changed. 200 tests
pass. Both new artifacts reproduce byte for byte; all eleven source manifests match.
No complete family or production mechanism is declared proven. Publication is verified
against the remote tip at cycle close.

## Next bounded chunk

0012: freeze a converged first measurement batch, then add an independent batch of the
same static pose. Compare a retained first-order quadratic, exact nonlinear moments and
all-raw recomputation as the optimum moves. Predeclare both batches' geometry/noise,
information ratios and bounded perturbations. Separate valid new-data response from
approximation error; retain input identity and covariance checks.

Reason: 0010–0011 used predetermined anchors. A converged freeze followed by new evidence
is the next prerequisite before testing uncertain-landmark/calibration elimination.
The known-landmark moment formula cannot be assumed sufficient for that broader graph.
Broader SE(3), timing faults and physical consumers remain queued.

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
- I06: first deterministic SE(2) witness in 0010 with exact known landmarks, isotropic
  independent noise, a prescribed displaced linearization and full raw reference.
  Nonlinear moment sufficiency, quadratic insufficiency and local-Gaussian limitations
  are scoped to this model. 0011 adds primary/held-out angular coverage and a conditional
  calibration derivation in the same model; weak-geometry Gaussian undercoverage and
  anchor-dependent frozen failures are reproduced. Joint pose/point coverage, converged
  freeze followed by new evidence, uncertain-landmark/calibration elimination, SE(3) and
  production behavior remain open.
- I07-I08, R05/R07, R09-R10, F04-F06, S02 and S04-S08 have no dedicated scenario-level
  coverage yet. R05 has an adjacent single-record example in 0009, not its full scope.
- Mandatory compounds: R03+F03 has the limited 0009 witness; other compounds remain planned.
- Production changes: none. Distributed and hardware claims: none. Nonlinear evidence
  is limited to the explicit planar model in 0010–0011.

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
- [0010](cycles/0010.md): nonlinear likelihood versus frozen quadratic, independent
  pose/uncertainty checks, insufficient summaries and no-data rebuild corrections.
- [0011](cycles/0011.md): repeated-sample angular coverage, full circular intervals,
  weak-geometry Gaussian undercoverage, anchor dependence and numerical-reference checks.

## Scheduling

Requested: a heartbeat in the current chat every four hours, one bounded chunk,
commit/push every cycle including temporary/negative work. See README for procedure.
Automation: `information-lifecycle-investigation`, confirmed ACTIVE, same-chat
heartbeat, four-hour interval. Each run follows README and preserves evidence in Git.
Local execution requires this computer and the app to be running.
