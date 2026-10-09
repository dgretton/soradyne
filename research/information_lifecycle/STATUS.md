# Investigation status

Started 2026-10-07. Branch: `research/information-lifecycle`.
Base: `9cbba49` (`shared-flow-demo-contracts`).

**Completed 2026-10-09 under Dana's narrowed closing scope.** Synthesis and both
independent reviews are pushed and verified; the automation is confirmed **PAUSED**.
No research, review or publication retry is queued.

## Current steering — 2026-10-08

Dana narrowed the goal after 0011: preserve long-range mathematical options through
representation, retention, identity and contracts. Near-term operation is on one local
network or Tailscale, with unavailable nodes repaired before sessions. Estimator quality,
controller tuning and broad fault/performance coverage are deferred.

**Closing budget: four further cycles maximum; all four completed.**
This documentation/scheduler change does not consume an experiment cycle. The old
converged-freeze/new-batch 0012 is cancelled, not run; the number is reused below.
PLAN's closing sequence supersedes older cycle reports' next actions and the old full
matrix completion gate. No new architectural contradiction was found during rescoping.

| Cycle | Closing deliverable | State |
|---|---|---|
| 0012 | I07 + R05: shared/drifting/replaced calibration; sensor/time/field withdrawal; retained state and identity granularity | Complete, bounded exact evidence |
| 0013 | I08 + S02: coherent nested revisions; physical bump versus known/unknown coordinate reset | Complete, bounded exact evidence |
| 0014 | F04: crash/atomic manifest boundaries for export, checkpoint, epoch close and grant update | Complete, bounded local evidence with a preserved serving correction |
| 0015 | Step 10 `SYNTHESIS.md`; push; then two independent expert reviews; commit/push reports and any immediate corrections; pause automation | Complete; reviews/corrections pushed at `d6fd28f`, automation confirmed PAUSED |

Combine or skip witnesses when prior evidence suffices; do not add replacement work.
No more than three witness cycles before synthesis. R07, F05, F06, S08, S04–S07 and
further nonlinear/SE(3) coverage are explicit must-not-be-precluded deferrals. Other
unselected rows get a synthesis audit only. Every matrix row must receive its scoped
status and reason. Owning contract changes remain proposals in the research directory.
See PLAN for the exact synthesis/review deliverables and the immediate reporting rule
for genuine new contradictions.

## Current cycle

0015: initial [SYNTHESIS.md](SYNTHESIS.md) pushed/verified at `d51cb50` before independent
reviews. Both [architecture](reviews/network_architecture.md) and
[mathematics](reviews/uncertainty_propagation.md) reviewers support closing without a new
contradiction or further research. Their original reports remain unchanged. The
[parent response](reviews/PARENT_RESPONSE.md) records accepted clarification of stale
publication, retained clock models, withdrawal/acknowledgment ownership and Gaussian
qualifiers; no scenario status changed. The mathematical reviewer reran all 259 existing
tests successfully and verified all 15 source manifests. Reports/corrections were pushed
and verified at `d6fd28f54b2139e819c71244fb901ee4461e4de1`, then the automation was
confirmed PAUSED at 2026-10-09 15:04 UTC; see [cycle report](cycles/0015.md).

## Last witness

0014: F04 contract witness completed. Under an assumed atomic durable local record
primitive, 46 crash cuts and retries preserve complete accepted results and durable
invalidations. Control and publication acknowledgments have separate meanings. Current
use is unavailable while affected replacement work is incomplete. An original live
serving gap between control receipt and durability is preserved, corrected and tested;
the original recovery-only pass did not establish that live invariant. 17 negative
controls include correct-mean/wrong-variance checkpoint recovery and invalidation rollback.
259 tests pass. Both 0014 artifacts reproduce byte for byte; all fifteen source manifests
across fourteen cycles match. No new contradiction or production/distributed/physical
safety claim. Local unacknowledged controls require retry after pre-commit crash; an
earlier external-effect promise needs stronger restart freshness than this model supplies.
Publication is verified against the remote tip at cycle close.

## Next bounded chunk

None. The bounded investigation is complete. Owning-contract adoption, Dana's D1–D7
decisions and explicitly deferred implementation scenarios are future work, not an
automatic continuation. Resume only on new human steering.

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
- I07/R05: bounded exact contract evidence in 0012 for a shared offset with two-time
  drift, prior/gain replacement, learned calibration support, sensor/time/field selectors
  and correlated fields. Representation equivalence and insufficiency are model-specific;
  real calibration estimation, nonlinear drift, clock uncertainty and rights enforcement
  remain untested. No additional cycle is needed before synthesis under the closing scope.
- I08/S02: bounded exact evidence in 0013 for dependency/covariance-bound nested
  snapshots, opaque result bindings, policy separation, known coordinate re-expression,
  physical bump ambiguity, half-open epochs and shared reset uncertainty. Payload truth,
  authenticated metadata and policy snapshots are assumed. Distributed construction,
  unknown-reset recovery, cause detection and physical response remain untested.
- F04: bounded local evidence in 0014 for export/checkpoint publication, separate
  durable grant/epoch invalidation, receipt-bound retry and acceptance/current policy.
  Original pending-control serving defect preserved and corrected. Atomic storage,
  one serialized writer and caller retry are assumptions; external-time revocation,
  distributed freshness/fencing, disk durability and garbage collection are untested.
- R07, R09-R10, F05-F06 and S04-S08 have no dedicated scenario-level coverage;
  all receive synthesis dispositions only, with explicit deferrals preserved.
- Former mandatory compounds: R03+F03 has the limited 0009 witness. The remaining
  compounds are no longer required experiments; preserve their dependencies/limits
  in synthesis without reopening deferred scenarios.
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
- 2026-10-08 human steering: finite closing program in PLAN/README/METHODS/STATUS;
  prior experiments, protocols and evidence unchanged. No new experimental result.
- [0012](cycles/0012.md): shared temporal calibration without mandatory active variables,
  revision sufficiency, learned-calibration withdrawal and correlated field marginals.
- [0013](cycles/0013.md): compatible nested revisions and uncertainty, opaque answer
  identity, distinct coordinate/motion epochs and known/unknown reset relations.
- [0014](cycles/0014.md): atomic local publication and durable invalidation, checkpoint
  coverage, idempotent receipts, distinct live/restart states and a preserved serving fix.
- [0015](cycles/0015.md): synthesis, independent reviews, separate parent response,
  final publication and confirmed automation pause.

## Scheduling

Requested: a heartbeat in the current chat every four hours, one bounded chunk,
commit/push every cycle including temporary/negative work. See README for procedure.
Automation: `information-lifecycle-investigation`, confirmed **PAUSED** after both
reviews and the response/corrections were pushed and their remote commit verified.
The automation tool returned PAUSED; the saved configuration independently confirms
it, with prompt, name, cadence and target chat preserved. Dana may restart it with new
steering. The former full-matrix criteria do not authorize additional work.
