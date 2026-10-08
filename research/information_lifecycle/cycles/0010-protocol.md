# Cycle 0010 pre-run protocol — nonlinear freezing, first bounded I06 slice

Written before candidate execution, 2026-10-08. Parent `37a38a5`. No production code.
Question: how far can a frozen first-order pose likelihood depart from the same raw
evidence, and can a small nonlinear sufficient statistic avoid that departure?

## Scope and hypotheses

One static planar rigid transform T_world_body, known exact body landmarks p_i,
measurements z_i = R(theta)p_i + t + e_i, independent e_i ~ N(0,sigma^2 I).
Known correspondences, scale, sigma, and world datum. No unknown landmarks/calibration,
robust kernels, shared noise, eliminated latent variables, or changing physical state.
This is a prerequisite to E3.5/I06, not an implementation of general survey marginalization.
All four observations and all representations have unchanged P0 read/retain/compute
rights. IDs and input/configuration identity accompany each result; neither truth nor
the evaluator's raw archive is available to a frozen candidate after construction.

H1: one Gaussian linearized at a displaced anchor can return an inaccurate rigid pose,
even with all input identities and a positive covariance. Relabeling/recentering the
same quadratic cannot restore its missing nonlinear shape.
H2: retaining centered moments is sufficient for the entire likelihood in this special
model, without re-reading raw points. It need not be sufficient for selective removal,
changed correspondences, heterogeneous models, or the application’s broader graph.
H3: even an exact nonlinear likelihood's local Gaussian covariance can misdescribe broad
angular uncertainty; likelihood preservation and Gaussian calibration are separate checks.

## Conventions and fixed sweep

Metres/radians, float64, counterclockwise planar rotation. Internal additive chart is
[theta, t_x, t_y], explicitly **not** a Lie tangent. Also report left/world tangent
covariance [omega, v_x, v_y], with v=delta_t-J*t*delta_theta. Verify point covariance in
both charts. This follows the planar restriction of Nestbox's ordering/left perturbation,
not GTSAM's right retract. Read conventions/glossary, SUBSTRATE, E3.5/E3.10/E7.1 at
nestbox-ng `041f1146df2e2a271cd0489fda520bc65f2e9f70`; import no production module.

Base landmarks (-1,-.5),(-1,.5),(1,-.5),(1,.5). Scale in {1,.1}; add body-frame centroid
offset (0,0) or (3,-2). Thus four geometries expose baseline and lever-arm effects.
Truth theta=0, t=(.3,-.2), used only by data generation and error evaluation.
sigma in {.05,.2} m. Two deterministic residual fixtures: zero and sigma times
[(.4,-.8),(-.6,.2),(.3,.5),(-.1,.1)]. These are named samples, not random trials.
Freeze anchor theta=-delta, t=(0,0), delta in {0,5,15,30,60,90,120,170} degrees.
Noisy optimum displacement may differ from delta; record actual displacement too.
4 geometries x 2 sigmas x 2 fixtures x 8 anchors = 128 configurations.
No Monte Carlo this cycle. Fixed-seed coverage is the next slice after oracle validation.

## Methods and independent references

1. Full raw batch: center point clouds, determinant-corrected SVD for proper rotation,
   then translation. Compute residuals and exact observed Hessian from individual raw
   residual derivatives. Independently check optimum against an angular profile grid.
2. Nonlinear moments: n, means of p/z, centered squared norms S_p/S_z, cross dot a
   and cross product b, plus sigma and IDs. Its sum of squared residuals is
   S_p+S_z-2(a*cos(theta)+b*sin(theta))+n*||t+R(theta)*mean_p-mean_z||^2.
   Solve theta=atan2(b,a), t=mean_z-R*mean_p. Derive local covariance with centered
   angular curvature hypot(a,b)/sigma^2 and centroid variance sigma^2/n, preserving
   cross terms on return to the pose chart. This is a local inverse-Hessian covariance,
   not an exact joint posterior covariance.
3. Frozen linear: r0+J0*[delta_theta,delta_t], once at its specified anchor; least-squares
   solve and inverse J0'J0. Interpret the resulting angle as a rigid rotation when queried.
4. Recentered linear: shift the same quadratic's chart to its own solution and solve
   again without raw data or moments. Negative control for calling this relinearization.

The raw reference uses SVD and individual residuals; the compact method uses 2D scalar
moments/trigonometry. Check raw-vs-moment objectives at 25 prespecified poses (angles
-170,-60,0,60,170 degrees crossed with translations (0,0),(.3,-.2),(1,0),(0,1),(-1,-1)).
Finite-difference raw gradients check the observed Hessian. An 8192-angle profile grid
checks the SVD optimum, with no iterative optimization initialization to tune.

For uncertainty, flat prior on translation and uniform angle on the circle make the
normalized angular marginal proportional to exp(-profile_SSE(theta)/(2*sigma^2));
translation integration contributes a theta-independent constant. Numerically integrate
raw centered residuals with bounded scipy.quad, epsabs/epsrel=1e-10, limit=100. Split
at the optimum to avoid missing a sharp mode. Measure actual posterior mass inside
each method's wrapped +/-1.959963984540054*sqrt(C_theta_theta) interval. Compare full
normalization with an independent 8192-point periodic trapezoid sum; require relative
agreement <1e-8 and quadrature error estimate <1e-8. This is deterministic conditional
posterior mass, **not** frequentist Monte Carlo coverage. Exact local Gaussian may fail
the predeclared [.94,.96] nominal-mass diagnostic in broad/weak configurations.

## Acceptance, dynamics, permissions and resources

Moment vs raw: pose error <1e-9 rad/m, covariance relative Frobenius error <1e-9,
objective relative discrepancy <1e-10 with denominator max(1,abs(raw)), point-covariance
chart equivalence <1e-10. Hessian finite difference relative error <1e-6, step=1e-5.
Grid min cannot beat SVD optimum by >1e-9 absolute SSE. Analytic noiseless witness:
frozen theta=-delta+sin(delta); record failure if this disagrees by >1e-10.
Frozen/recentered are allowed to fail accuracy and mass diagnostics; do not tune gates
or relax thresholds to make them pass. Record 1 cm max error at query body points
(0,0),(2,0),(0,2) and 1 degree angular error as illustrative diagnostics, not safety limits.

Timeline has three synthetic publication events: frozen solve at tick0, recenter/repeat
at tick1, authorized raw rebuild at tick2. Four original observations, zero new data
after tick0. Record pose/task-point jump at rebuild, total variation, covariance change,
observation count, represented/permitted IDs and forbidden IDs. No smoothing, new
physical motion, delayed policy discovery, control plant or measured-latency guarantee.
The nonlinear summary should have no rebuild correction beyond numerical noise.

Bound input count to 32; fixed sweep uses four. No queues/threads. Four methods, three
events; 8192 angle samples evaluated in bounded arrays; quad subdivision limit100.
Retain all 128 per-configuration metrics and selected full traces, source hashes and
runtime versions. Count numerical payload slots and separate identity metadata; no
constant-memory claim for unbounded provenance, no claim four-point compression wins.
Reject invalid/nonfinite input, duplicate IDs, nonpositive sigma and unobservable angle;
include proper-rotation/reflection and angle-wrap checks. Reproduce in a fresh directory.

Primary sources consulted for reference implementation: [NIST algebraic justification
of Kabsch–Umeyama](https://math.nist.gov/~JBernal/kujustf.pdf), [NumPy 1.26 SVD](https://numpy.org/doc/1.26/reference/generated/numpy.linalg.svd.html),
and [SciPy quad](https://docs.scipy.org/doc/scipy/reference/generated/scipy.integrate.quad.html).
The moment identity and uncertainty qualifications above are derived for this fixture;
these sources do not certify Nestbox's freeze contract or consumer safety.

## Follow-up witness declared after the first grid, before executing this extension

The original 15 tests passed; the first artifact-writing command failed on a NumPy
boolean at the JSON boundary. Add a serialization regression and convert the diagnostic
to Python scalars. No numerical threshold changes.

To distinguish a badly interpreted approximation from actual insufficiency of this
stored quadratic, add a two-world witness. Centered base p, sigma=.2, same four IDs,
anchor zero. For c in {.5,1.5}, compare z=(1+c)*p and z=(1-c)*p. Both have residuals
at the anchor equal to +/-c*p, zero mean and zero angular gradient, identical J'J,
eta, constant and anchor. Full raw likelihoods have different angular curvature; for
c=1.5 their preferred angles also differ by pi. Verify equality of **all** retained
quadratic fields and identity-only metadata, then compare SVD/moment solutions and
local covariance. The fixtures are possible Gaussian data, not new physical scale
parameters or Monte Carlo trials. Content hashes or extra numerical metadata are not
part of this witness; no claim about their insufficiency follows. This concerns the
specified frozen first-order representation, not every possible pose/covariance encoding.
