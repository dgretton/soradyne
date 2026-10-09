# Evidence discipline

## Closing program after cycle 0011

Dana's 2026-10-08 steering narrows further work to irreversible data-model/contract
choices (PLAN, closing sequence), at most three witness cycles then one synthesis/
review cycle. Small exact witnesses and finite local state machines suffice when
they expose the missing retained information or acceptance boundary. Do not require
new Monte Carlo, physical simulations, latency sweeps or SE(3) validation just to
complete this phase. Existing numerical evidence supports an approximation contract,
not a selected production estimator or consumer operating envelope.

Each remaining witness states which early choice it constrains, the smallest retained
state/identity that preserves the option, alternatives, and the limit of the conclusion.
An unimplemented deferred operation is not supported merely because metadata names it;
identify the sufficient numerical state or replay path, authorization and future contract
hooks it would need. Exact witnesses do not establish that those permissions are granted.
The synthesis's scenario statuses are scoped capability dispositions; evidence levels
below still apply. Independent reviewers receive the same pushed synthesis snapshot;
their original interpretations and the parent's responses remain distinguishable.

Cycle 0012 implements this narrower approach: exact latent-factor inference compared
with independently derived observation covariance and scalar conditioning formulas;
identical retained marginals that separate after a model revision; withdrawal through
calibration support; and correlated-field marginalization. Alternative fixtures have
explicitly separate policy/model worlds. No model tag alone is treated as a sufficient
statistic, and no P2 derivative right is inferred under P3 contribution withdrawal.

Cycle 0013 compares composed edge means/joint covariance to cancellation in the original
independent variables, then checks all bounded revision combinations and arrival prefixes.
Reset geometry uses exact distances, rotated vectors/covariance and a same-jump/different-
cause witness. Metadata compatibility and opaque binding are not mathematical validation,
cryptographic privacy or distributed snapshot construction. Unknown relation/cause stays
explicit; event classification is an input, not a detector tested by the simulation.

## Experiment record

Before implementing a slice, state its scenario IDs, hypothesis, policy variant,
assumptions, variables/units/gauge, data available to each role, independent oracle,
candidate methods, quantitative checks and how a negative result will be recognized.
Commit configuration and fixed seeds. Do not let a candidate read simulation truth.
Keep the truth process separate when moving to process/network simulations.

Every result is one of: established for the stated exact model; supported within a
tested numerical/statistical envelope; falsified by a counterexample; inconclusive;
or not implemented. A passed software test is not a broader mathematical proof.
Distinguish an algorithm failure from a policy choice that forbids the desired answer.

## Layers of evidence

1. Closed-form/exact rational witnesses: small scalar/vector cases, explicit covariance
   cross terms and independent calculations. Test both the valid method and deliberately
   incorrect controls. Do not hide approximate arithmetic under overly loose tolerances.
2. Linear floating-point systems: conditioning, rank deficiency and multiple scales.
   Compare against batch QR/SVD or another independently structured reference, avoid
   explicit inverses in performance implementations, and document rank/tolerance choices.
3. Nonlinear pose/calibration problems: joint tangent conventions, gauge handling,
   initialization and linearization versions. Compare batch/reduced/incremental methods
   and run independently generated Monte Carlo truth. Treat local Gaussian results as
   local approximations. Define outlier/ambiguity policy before measuring coverage.
4. Event/state simulations: reorder/delay/drop/duplicate observations, policy changes,
   revocations, crashes and publish events. Compare each accepted answer to an oracle
   built only from the evidence legally available for that answer's declared policy.
5. Dynamic consumer simulations: fixed dynamics/controller, bounds and task geometry;
   evaluate accuracy, authorization and clearance alongside correction smoothness.
6. Separate-process integration and eventually recorded hardware: only when the earlier
   questions are sufficiently resolved. No physical actuation is authorized here.

Use assertions on actual invariants, not tests that mirror the candidate's control
flow. Try mutation/negative controls such as duplicate evidence, lost cross-covariance,
stale authority and covert reuse of withdrawn information. Preserve a falsifying trace.

## Statistical and performance protocol

Predeclare seeds, trial count, noise/bias families, timing and geometry sweeps, units,
thresholds and acceptable coverage interval before inspecting outcomes. Report trial
count, failures and uncertainty intervals; do not call zero observed failures zero
risk. Compare methods with common random inputs and retain held-out seeds for final
checks. Use normalized estimation error with correct observable rank/tangent and
independent truth; fitted residuals alone do not measure state uncertainty calibration.

Record solve wall time, arrival-to-accepted-output latency, queue depth, retained and
active bytes/state size, and p50/p95/p99 where sample size supports them. Distinguish
synthetic event time from measured CPU/wall time. Benchmarks state hardware/runtime.
No timing guarantee is inferred from a single run. Stop bounded simulations explicitly
and record exhausted budgets as inconclusive, not passing.

Smoothing bias must not be hidden inside a covariance labeled as the estimator's
posterior. Report any intentionally applied transition, its lag/error bound and the
consumer's actual validity decision. Tests must reject a smooth but unauthorized,
unobservable, biased or collision-producing trajectory.

## Reproducibility and retention

Each cycle records parent commit, tested source hashes, environment, commands and
exit results. Store small results in Git: configurations, seeds, representative traces,
metrics and plots if useful. Large generated sweeps should have reproducible recipes
and compact summaries; never commit credentials, personal sensor recordings or massive
artifacts indiscriminately. Use synthetic data by default.

Experimental work is pushed even when expected to be removed. Label disposable
implementations and retain the useful failure fixture/conclusion if later deleting
them. Ordinary archival or compression of run outputs must preserve reproducibility.

Do not silently rewrite previous cycle results after changing a method. New evidence
gets a new run; mark an earlier conclusion superseded with a link and reason. If a
test was invalid, preserve the correction and its implications for dependent findings.

## Design tensions

TENSIONS records a specific pair of requirements in conflict, a minimal scenario,
evidence level, affected layer and possible decisions. First distinguish a model/test
bug or missing detail from a real incompatibility. Do not manufacture tensions for
each cycle; 'no new tension' is a valid result. Do not resolve consent, physical
validity or uncertainty claims by quietly weakening a requirement.

## References and sources

- Current shared-flow contracts: `../../docs/20260911_shared_flow_demo_contracts.md`.
- Current conventions and use cases: PLAN.md application coverage, with revision pinned
  in each applicable cycle.
- GTSAM marginal covariance and joint marginals:
  https://borglab.github.io/gtsam/marginals/
- GTSAM iSAM2 update/removal/marginalization surface:
  https://gtsam.org/doxygen/4.0.0/a03643.html
- Bounded-window candidate background:
  https://borglab.github.io/gtsam/fixedlagsmoother/
- Cycle 0010 proper-rotation batch reference, algebraic justification of Kabsch–Umeyama:
  https://math.nist.gov/~JBernal/kujustf.pdf
- NumPy SVD implementation surface used by that reference:
  https://numpy.org/doc/1.26/reference/generated/numpy.linalg.svd.html
- Bounded quadrature for raw nonlinear angular posterior mass:
  https://docs.scipy.org/doc/scipy/reference/generated/scipy.integrate.quad.html

Cycle 0010 deliberately distinguishes observed Hessian, Gauss–Newton information,
local Gaussian covariance, full conditional posterior mass and repeated-sample coverage.
Agreement of means/covariances alone does not establish equivalence of nonlinear
likelihoods. Preserve uncertainty qualifications in any later application contract.

Cycle 0011 calibrates only the angular marginal in the same restricted model, with
primary/held-out seeds and exact binomial confidence intervals. Its family comparison
count and Bonferroni confidence level were declared before execution. Compatibility
with nominal coverage is not a proof of equality; a model-specific conditional-coverage
derivation is stated separately. The full-likelihood and local-Gaussian candidates share
point estimates, so improved interval calibration must not be called improved accuracy.
Independent raw integration caught a library-CDF approximation that inverse/CDF agreement
alone missed. Preserve both the numerical counterexample and the declared tolerances.

Additional primary APIs consulted in 0011:

- https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.vonmises.html
- https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats._result_classes.BinomTestResult.proportion_ci.html
- https://numpy.org/doc/1.26/reference/random/bit_generators/pcg64.html
- https://docs.scipy.org/doc/scipy/reference/generated/scipy.special.i0e.html
- Installed NumPy 1.26.4 `numpy.polynomial.legendre.leggauss` documentation.

These sources motivate candidates; they do not certify our wrappers, privacy policy
or consumer stability. Consult current primary papers/documentation before adding a
specialized algorithm and record exactly which assumptions apply.
