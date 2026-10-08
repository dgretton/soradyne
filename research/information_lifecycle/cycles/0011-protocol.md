# Cycle 0011 pre-run protocol — angular coverage under repeated measurements

Written before Monte Carlo execution, 2026-10-08; parent `c69306f`. Bounded I06
continuation of 0010. No production changes, physical plant or actuation.

## Question and exact-model calculation

Does a nominal 95% angular interval cover the true orientation in 95% of repeated
independent Gaussian datasets? Compare local approximations with the entire nonlinear
angular likelihood, using exactly the same data for every method. Preserve 0010's code
and artifacts unchanged. Its conditional posterior-mass result is not itself an
empirical coverage measurement.

The model is unchanged: four exact known body points, z_i=R(theta)*p_i+t+e_i,
independent e_i~N(0,sigma^2 I), known sigma/correspondence/scale/world datum. No latent
landmarks, calibration, outliers, temporal correlation or permission changes. P0 grants
explicitly permit raw and derivative retention, inference and replay. IDs are p0..p3.
Authority is trusted local input; audit represented/permitted IDs without claiming
distributed enforcement. Additive pose chart [theta,t_x,t_y]; world-side covariance
conventions remain those of 0010 and nestbox-ng `041f1146df2e2a271cd0489fda520bc65f2e9f70`.
This cycle calibrates the **angular marginal**, not a joint SE(2) confidence region.

Let centered p have S=sum||p'||^2. The moment vector (a,b) from 0010 has sampling law

    (a,b) ~ N(S*[cos(theta),sin(theta)], sigma^2*S*I_2).

This follows by summing independent linear projections of the Gaussian measurement
noise; their cross-covariance is zero. For rho=hypot(a,b), alpha=atan2(b,a), polar
coordinates give alpha-theta conditional on rho a von Mises density proportional to
exp((rho/sigma^2)*cos(alpha-theta)). The same density is the angular posterior under
uniform angle and flat translation priors. Thus a symmetric arc with conditional mass
.95 has .95 frequentist coverage for every fixed theta and rho in this particular
model. This is an algebraic prediction to test, not a general identification of Bayesian
credible sets with frequentist confidence sets. Unknown sigma, correlations and uncertain
geometry change its premises.

## Candidates and independent checks

Six methods per dataset, all queried on the same four observations:

1. `full_arc`: compact nonlinear moments give alpha and kappa=rho/sigma^2. Find h in
   [0,pi] with vonmises.CDF(h,kappa)=.975, using 50 bounded bisections. Interval is the
   circular arc |wrap(theta-alpha)|<=h, not an unwrapped interval cut at +/-pi.
2. `observed_gaussian`: same nonlinear mode, h=min(pi,1.959963984540054/sqrt(kappa)).
   This is the local observed-Hessian approximation, not the whole posterior variance.
3. `gauss_newton`: same nonlinear mode, h=min(pi,1.959963984540054*sigma/sqrt(S)).
   This is the angular Schur variance of the fitted raw J'J.
4–6. `frozen_0`, `frozen_minus30`, `frozen_minus90`: solve the first-order residual
   approximation once at prescribed anchors (0,0,0),(-pi/6,0,0),(-pi/2,0,0). Report
   h from its J0'J0. These anchors are fixed before sampling and do not read truth.

The raw pose reference is a batched proper-rotation SVD, separate from scalar moment
inference. All trials require wrapped angular and translation agreement <1e-9 for
moments versus SVD. At the first eight trials in every cell, compare scalar 0010
raw_batch/MomentSummary/FrozenQuadratic implementations with the batched paths; local
observed/Gauss–Newton angular variances must agree to relative 1e-8. Independently
integrate raw profile residuals for full-arc mass at those trials, requiring |mass-.95|
<1e-7. Check quantile CDF residual <1e-10 for all samples. Before Monte Carlo, check
kappa={0,.001,.1,1,10,100,2000} against independent normalized quadrature. CDF/library
errors, numeric rank failures or exhausted limits must fail visibly, not discard trials.

## Fixed statistical design

Base points (-1,-.5),(-1,.5),(1,-.5),(1,.5); scale in {1,.1}; centroid offset in
{(0,0),(3,-2)}; sigma in {.05,.2} m; fixed truth theta in {0,170 degrees},
t=(.3,-.2). Cartesian-product order: scale, offset, sigma, theta. Sixteen cells.
Truth generates measurements and evaluates error only; inference receives p,z,sigma,
identities and the predetermined anchor, never true theta/t or a selected-noise score.

4096 independent datasets per cell per phase; four point observations per dataset.
Primary seed=2026100801. Held-out seed=2026100802, first executed only after numerical
checks and primary results are inspected with a frozen implementation. Use explicit
NumPy Generator(PCG64(SeedSequence([phase_seed,cell_index]))), drawing standard_normal
with shape (4096,4,2) and scaling by sigma. Cells have separate generator streams.
Total 131,072 distinct datasets, 786,432 interval evaluations. Methods within a dataset
are paired; they are not extra independent observations. No outlier rejection, adaptive
trial counts, tuning, resampling failures or optional stopping. If a bug is found after
held-out execution, label its reuse rather than silently treating it as untouched.

Report per-cell successes/trials, point estimate and exact Clopper–Pearson 95% interval.
There are 2 phases * 16 cells * 6 methods = 192 coverage comparisons. Also report exact
intervals at confidence 1-.01/192: a Bonferroni family bound of at least 99%, without
assuming comparisons are independent. Label undercoverage when this simultaneous
interval's upper limit is <.95, overcoverage when its lower limit is >.95, otherwise
compatible. Compatibility does not prove equality or a deployment tolerance. Predeclare
a separate practical diagnostic |empirical coverage-.95|<=.02; do not relabel it safety.
Hypotheses: full_arc is compatible everywhere; local Gaussian approximations can fail
in weak geometry; frozen methods can fail at displaced anchors. Failed hypotheses remain
results. Tests should enforce the numerical model, not change thresholds to rescue them.

Also report angular RMS error, angular squared-error/local-variance mean where applicable
(local quadratic score, not a chi-square guarantee), arc half-width quantiles, fraction
of arcs wider than 45 degrees, truth-relative task-point error and rebuild discrepancy.
Record mean conditional posterior mass inside each reported arc. This is a separate
quantity from fixed-truth empirical coverage, particularly for anchor-dependent methods.
Use percentile method='linear' with p50/p95/p99/max and record units. No pooling across
cells to hide a failure. Primary and held-out results remain distinguishable.

## Dynamics, retained evidence and bounds

Synthetic publications: tick0 inference, tick1 repeated same result, tick2 authorized
raw rebuild with the same four observations. No new measurements between them, no
smoothing. Full/nonlinear methods retain their own uncertainty convention on rebuilding;
frozen methods rebuild to the raw nonlinear mode with Gauss–Newton angular width.
Measure wrapped angle jump and maximum task-point jump at (0,0),(2,0),(0,2), versus
truth error and interval coverage separately. Tick1 contributes zero variation; a
numerical rebuild is not an acquisition. No physical response or latency claim.

Run one cell at a time; input batch bound 4096, point bound 32 (four used), 50 quantile
iterations, quadrature limit 100. Whole CLI run bound 600 seconds, checked between cells;
at most 32 cells and fixed bounded numerical calls per cell. Store per-cell summaries,
measurement hashes and selected complete traces (trial 0, first full-arc miss, worst
frozen_minus90 point jump; deduplicate selections), plus seeds/configurations/source
hashes and versions. Full trials regenerate; do not store megabytes of repeated arrays.
Record actual runtime separately from deterministic artifacts; no timing guarantee.
Require NumPy/SciPy already installed; no production/GTSAM imports or new packages.

Primary documentation consulted: [SciPy von Mises](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.vonmises.html),
[exact binomial intervals](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats._result_classes.BinomTestResult.proportion_ci.html),
and [NumPy PCG64](https://numpy.org/doc/1.26/reference/random/bit_generators/pcg64.html).
The conditional-coverage identity above is derived for this fixture; numerical APIs do
not establish application or hardware safety.

## Numerical correction before the formal primary/held-out runs

The first 16-test check found that inverting SciPy's von Mises CDF at kappa=100
gives an arc whose independently integrated mass differs from .95 by 1.40285e-6,
above the declared 1e-7 requirement. No tolerance or statistical design is changed.
Keep that failing numerical method as a reproducible comparison. Replace the accepted
quantile with 50 bisections of the normalized symmetric integral, using 64-point
Gauss–Legendre quadrature and scaled normalization pi*i0e(kappa). Its bracket is
[0,min(pi,8/sqrt(kappa))], with pi at kappa=0, and must explicitly bracket .95.
Require final integral residual <1e-10 and agreement with a separate 32-point rule
<1e-10, in addition to the existing raw adaptive-quadrature checks. Batch scratch is
at most 4096x64 entries per array. Retain the concentration-grid comparison in evidence.

The optional mean posterior-mass diagnostic for arbitrary shifted arcs still uses the
library CDF and is labeled `scipy_approx`; it does not choose accepted arc widths,
empirical hits or coverage classifications. Its small numerical approximation is not
treated as a coverage defect. Primary smoke checks sampled only 64 trials of cells 0/15;
the formal primary and all held-out samples have not yet been executed at this correction.
Primary APIs for the correction: [NumPy leggauss](https://numpy.org/doc/1.26/reference/generated/numpy.polynomial.legendre.leggauss.html)
and [SciPy i0e](https://docs.scipy.org/doc/scipy/reference/generated/scipy.special.i0e.html).
