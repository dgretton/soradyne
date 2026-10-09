# Independent review: uncertainty propagation and future 3D alignment

2026-10-09. Reviewer: independent uncertainty-propagation subagent. This report was
written without reading the other specialist's report or communicating with that
reviewer. It reviews the published synthesis, not any subsequent parent response.

## Review pins and scope

- Synthesis and research sources: soradyne
  `d51cb5024102098d140effaabfacb82423d1c7d3`, branch
  `research/information-lifecycle`. The parent supplied this as the verified remote
  tip before dispatch; I reviewed it using `git show`, rather than assuming the
  working-tree head remained fixed.
- Shared-flow design base: `9cbba490362647bbc9a7216735e51f1b9477d32b`.
  The current shared-flow contract was read from the reviewed synthesis snapshot.
- Owning application documents: nestbox-ng
  `ed141df17477c40a4b9365854aa385f82e1d9280`, also read with `git show`.
- Repository discipline: pinned `CLAUDE.md` and workspace
  `/Users/rim/Dev/BLANKET.md`. The authorized cross-project exception was confined
  to research review. No owning document, production source, Git state, or automation
  was changed.

References below use source line numbers at these pins. Unqualified paths are under
`research/information_lifecycle/`; application references explicitly name nestbox-ng.

The decision being reviewed is what to preserve now, not which estimator or controller
to implement. I accepted the local-network/Tailscale operating profile and the explicit
deferral of R07, F05, F06, S08, S04–S07, and broader nonlinear/SE(3) validation. I did
not treat absence of those experiments as a failure to complete this investigation.

## Independent verdict

**The synthesis is mathematically sound as a conditional preservation contract.** Its
small active state plus authorized richer archive is a reasonable minimal starting
direction. C1–C8 are sufficient semantic obligations for preserving the stated options
if “sufficient state” is actually established for each promised query, update, model
family and withdrawal selector before irreversible eviction. They are not themselves
a concrete sufficient-statistic schema or a guarantee of every unforeseen future
model. The synthesis makes that distinction sufficiently clear to close this phase.

I found **no new architectural contradiction and no reason to change the closing
priority or run another research cycle**. The information-loss findings refine T01;
the limits on continuation after information/authority loss refine T02. Nothing in the
review requires every calibration, landmark, clock parameter or historical pose to
remain an always-live optimization variable.

Two nonblocking clarifications below would improve the handoff to implementation.
Neither overturns a numerical result or requires choosing Dana's consent policy.

## Prioritized actionable findings

### U1 — P2, adoption clarification: make clock-model retention explicit in the owning time-contract proposals

**References:** `SYNTHESIS.md:55–60`, `101–115`, `278–281`, `320–329`;
nestbox-ng `docs/CONVENTIONS.md:97–109`; `docs/SUBSTRATE.md:224–263`.

S05 correctly conditions later time reassociation on a lawful replay/model path. C2/C3
also provide the right general obligations. However, the proposed owning-contract
changes do not name `SUBSTRATE.md` §4 T1–T5, whose concrete `ClockRef` surface has only
offset, scalar uncertainty and timescale. It supplies no explicit clock-model revision,
applicability interval or shared temporal-dependence contract. The current timestamp
wording uses the reference available at publication. Retaining a corrected time, a
clock ID and one marginal standard deviation is not, by itself, a promise that a later
offset/skew revision can re-associate old acquisitions or recover their correlation.

This matters mathematically even on one network. In a first-order moving-state model,
a common uncertain clock offset contributes `J_t,i Var(delta_c) J_t,j^T` between two
observations. Replacing it by independent timestamp noise changes the information just
as independent per-image calibration inflation does. A change in time association can
also cross a physical-epoch or withdrawal boundary.

**Action:** add a short explicit preservation condition and the owning §4 target:
retain original acquisition clock readings plus the applicable clock mapping/model
history, or equivalent reconstructible state, sufficient for the promised reassociation
family; preserve shared clock dependence, applicability and revisions. An opaque
reference may resolve that information privately. Unsupported or ambiguous reassociation
must remain expressible. This requires no live clock estimator, new clock algorithm or
new experiment now.

**Classification:** a concrete application of existing C1–C3/D3 to an incompletely
enumerated owning-contract target. It is not evidence that S05 was tested, a new T01
contradiction, or a demand to reopen the deferred scenario. The broad synthesis already
permits the necessary state; the clarification helps prevent a narrower timestamp
schema from being mistaken for satisfying it.

### U2 — P3, wording: retain the Gaussian and known-noise qualifiers in the short mathematical claims

**References:** `SYNTHESIS.md:79–85`, `139–145`, `251`;
`cycles/0001.md:9–19`; `cycles/0010.md:44–69`;
`cycles/0011.md:38–69`.

The detailed reports correctly use linear Gaussian models for exact finite Gaussian
elimination, and known independent isotropic Gaussian noise for the planar nonlinear
moment likelihood and circular-coverage derivation. The synthesis's abbreviated
phrases “linear joint reduction” and “independent isotropic noise” omit part of that
scope. Linearity or isotropy alone does not establish these particular finite-statistic
and interval claims for an arbitrary noise law.

**Action:** use “linear-Gaussian joint reduction” in the I01 evidence description and
“known-variance independent isotropic Gaussian noise” in C4's planar-study description.
Keep the existing caveats about uncertain landmarks, nonlinear calibration and SE(3).

**Classification:** precision of wording, not an error in the reported derivations or
an unsupported general-3D claim in the synthesis read as a whole. No rerun is needed.

## Mathematical assessment and sound choices

### Sufficiency is relative to the promised operation

C2's capability contract is the central sound choice (`SYNTHESIS.md:72–99`). For a
fixed linear-Gaussian model, the Schur complement preserves the retained-variable
joint distribution. It need not preserve the decomposition required to remove an
earlier factor, modify an eliminated calibration model, or ask a new query about an
eliminated quantity. The identical-retained-state/different-required-answer witnesses
establish precisely that loss; they do not establish a universal prohibition on
compact summaries.

The baseline implementation performs the expected Schur operations
(`baseline.py:70–78`), and its two-world construction changes the prior decomposition
while preserving the target marginal (`baseline.py:162–170`). The stronger later
witnesses keep the stated aggregate/model/identity schema fixed while changing the
discarded observation values (`cycles/0009.md:167–174`, `cycles/0012.md:83–108`).
Their conclusions are correctly about those schemas and supported operations.

The synthesis does not confuse raw images with original factor observations. Retained
corner observations plus their actual model and calibration recipe can preserve a
specified later inference family without retaining images. A future detector change
that needs image pixels is a different capability. Likewise, retention of an exact
current likelihood is not automatically retention of every future model's likelihood.
The declared-family limitation and explicit loss-of-capability response are essential.

Before the application adopts the freeze proposal, “queries” must include the intended
body-fixed point/landmark queries, not just the active body-pose marginal. For an
uncertain surveyed point, propagation of `q = R p + t` generally needs its retained
structure uncertainty and pose/structure cross terms, or a lawful reconstruction path.
A pose covariance and nominal point coordinates alone need not answer that query.
C2/C4 already require this; nestbox-ng `GLOSSARY.md:78–81` and work-breakdown E3.5
(`docs/design/work-breakdown.md:128`) do not yet implement that stronger contract.
This is an adoption gate already identified by the synthesis, not an additional demand
to keep all surveyed points live.

### Shared uncertainty and overlap are separate problems

The two-time calibration construction is correct. Integrating `(b0,d)` gives the
joint measurement covariance with off-diagonal `tau`, and the GLS formula matches the
latent information solve (`calibration_contracts.py:63–109`). A compact observation-space
model can therefore replace active calibration variables for the specified query.
Retaining only independently inflated observation variances loses shared dependence.

The drift-revision counterexample is also valid: identical old target marginals can
require different means after changing the drift variance. Model identity, numerical
model response and the actual retained observations/statistics are distinct necessities.
“New revision” does not justify appending a second independent prior. Learned calibration
support must be included in withdrawal when the policy requires that descendant's
contribution to disappear (`cycles/0012.md:110–147`). The report correctly receives that
policy as an input rather than deriving it from mathematics.

For correlated-field withdrawal, the retained-field likelihood uses the covariance
submatrix, not the corresponding block of the full precision matrix. The latter is a
conditional precision and can retain forbidden values or invent precision. The source
and report agree on this distinction (`calibration_contracts.py:238–251`;
`cycles/0012.md:149–163`).

Separately, knowing shared-variable identity and even the full covariance of two
aggregate estimates need not recover the unique-data sufficient statistic erased by
overlap. The cycle 0005 averages give a valid reduced Gaussian experiment with variance
`3/8`, while full unique data give `1/3`; the missing numerical overlap is not restored
by lineage (`cycles/0005.md:177–217`, `shared_information.py:427–465`). C3 correctly
allows a declared reduced answer or rejection instead of promising a universal fusion
rule. “Reduced information” here can mean a lossy statistic of all listed observations,
not only a smaller list of observations; representation binding in C2/C5 preserves that
distinction.

### Feedback, restart and withdrawal are counted at original information

Preserving the existing no-feedback restriction is the least demanding correct initial
contract. A dependency DAG of immutable artifacts can be acyclic while numerical
influence repeatedly returns to its source. The matched-base scalar residual is a
valid special case because its additive decomposition is stipulated; subtracting the
wrong base cancels lawful new information (`cycles/0006.md:23–38`, `174–188`). The
synthesis appropriately declines to generalize that operation after elimination or to
nonlinear posteriors (`SYNTHESIS.md:117–124`).

The same accounting applies to recovery: replay deduplication must include what the
checkpoint already represents. Published mean/variance is sufficient restart state in
the explicitly bound scalar model, but publication does not prove that sufficiency
(`cycles/0008.md:35–40`, `185–189`). C7 preserves actual consumed live-input coverage
instead of relying only on a durable log horizon. This is an information requirement
even when all nodes have been repaired before the session.

The granularity/expiry findings are T01 refinements. Independent per-record statistics
can preserve record withdrawal in the scalar model; source aggregates can support
whole-source withdrawal while losing finer removal. Unavailability after authorized
state has disappeared is an honest result, not a numerical algorithm defect. The
synthesis does not promise moving-state fixed-lag withdrawal from the static expiry
fixture (`SYNTHESIS.md:271`).

### Gauge, epochs and uncertain resets are separated correctly

The per-query observability method is valid for the stated positive-semidefinite linear
system: if `H w = q` has a solution, `q^T w` and `eta^T w` are invariant to its null-space
freedom. Gauge fixing does not add external anchoring evidence
(`observability.py:90–125`, `cycles/0007.md:40–74`). The synthesis retains query/direction
validity and explicitly leaves floating-point rank thresholds and general 3D geometry
untested. It does not infer useful precision merely from finite mathematical variance.

The snapshot witness correctly needs covariance for the same ordered edge revisions as
the means. Coherent unchanged independent edges can be reused; there is no mathematical
need for every edge to increment one global counter. This is a trusted-payload
composition witness, not a general PSD validator or a nested-estimator construction
(`snapshot_epochs.py:118–172`, `cycles/0013.md:53–65`, `216–227`). C5 says so.

A pure coordinate re-expression, physical-state change and inference update need
different meanings even when their displayed jumps agree. The deterministic planar
rotation/translation and the scalar uncertain-bridge covariance cancellation are correct.
The bridge witness assumes independence from the original errors; a learned bridge
requires additional joint dependence (`cycles/0013.md:123–170`). C5's “uncertainty and
shared dependence,” together with C4's approximation contract, leaves that option open.
It does not make a random SE(3) composition exact from marginal covariances alone.
Unknown relations remain unknown rather than silently becoming identity transforms.

### Nonlinear evidence supports preservation, not a universal uncertainty envelope

I checked the centered-moment expansion and chart conversion in
`nonlinear_freeze.py:41–55`, `109–181`. The moment statistic preserves the stated
known-landmark likelihood, including the nonlinear angle dependence. The observed
Hessian and Gauss–Newton information are deliberately different objects. Re-anchoring
the stored quadratic (`nonlinear_freeze.py:185–223`) does not restore omitted nonlinear
terms. A poor frozen pose-observation chart is not evidence that rotating an operational
body necessarily invalidates a body-relative structural survey; cycle 0010 explicitly
preserves that distinction (`cycles/0010.md:244–248`).

Cycle 0011's conditional angular-coverage derivation is valid under its model:
the dot/cross statistic is isotropic normal with mean on a circle; conditional on its
radius, the angular error has the stated von Mises concentration. This symmetry
justifies the specific confidence arc. It does not establish general equality of
Bayesian credible mass and repeated-sample coverage. The implementation keeps a
raw-SVD reference, numerical quadrature checks and family-adjusted binomial intervals
separate (`angular_coverage.py:98–136`, `171–223`, `252–271`).

C4's exclusions are appropriate: no joint SE(3), uncertain-landmark marginalization,
robust-kernel calibration, multimodal pose confidence or consumer precision follows from
these studies. Broad valid uncertainty can still be unusable for the requested task.
The proposed rebuild/model path and an output capable of expressing ambiguity preserve
later choices without requiring further coverage experiments now.

## Deferred options and owner decisions

The deferred rows are preserved conditionally, not certified. S04 needs lawful local
state and query-specific validity; S05 needs the time-model state in U1; S06 needs joint
geometry and compatible reset/pose revisions; S07 needs a retained likelihood/model
path and uncertainty richer than an unconditional single Gaussian. C2–C5 provide the
necessary representation room. F06/S08 remain feasible contract directions only if
future scheduling preserves the information/permission rules while separately proving
progress bounds. R07/F05 need authority/freshness and fencing choices; those mechanisms
are outside this mathematical review and cannot be inferred from local crash cuts.

No choice of D1–D7 is made here. In particular, mathematics does not decide whether an
authorized derivative survives raw deletion, whether withdrawal includes learned
calibration/training descendants, or whether a transition may use previously derived
state. D3 must choose withdrawal/time granularity and retention versus coarser capability
loss. D5 must choose acceptable observable quantities, uncertainty and reduced/historical
coverage. D7 must be decided before a physical consumer is promised a transition.
Algebraic cancellation can remove numerical dependence without granting processing or
retention rights. The report's supported P2/P3 alternatives must remain separate.

The existing owning contracts still contain stronger or ambiguous statements: automatic
honest survey covariance from frozen structure, pose acceptance based on residual
chi-square, child exports ingested as independent-looking `Between` factors, same-horizon
recovery, and already-published derivative survival. The synthesis explicitly proposes
refining those clauses; it does not claim that this review has changed them. Their
adoption is future implementation/contract work, not evidence that the proposed
preservation direction is internally contradictory.

## Evidence reviewed and checks actually performed

Read the pinned `SYNTHESIS.md`, `PLAN.md`, `METHODS.md`, `TENSIONS.md`; the relevant
reports for 0001 and 0005–0014; numerical source sections cited above; the pinned
application glossary, covariance/time conventions, substrate custody/time/revocation
contracts and relevant work-breakdown requirements; and the shared-flow contract.
Source inspection concentrated on elimination, shared calibration, overlap, observable
queries, nonlinear moments/coverage and snapshot/reset propagation. It was not an audit
of every line of every prototype or a hostile-producer/security review.

I independently checked the stated algebraic relations described in this report. I
also executed the existing test suite after verifying that all **31** tracked Python
source/test files equaled their blobs at the review pin. The command was:

```sh
/Users/rim/Dev/.venv-nestbox/bin/python -B -m unittest discover -s research/information_lifecycle -p 'test_*.py' -q
```

Result: **259 tests passed in 3.639 seconds**, exit code 0. A post-test comparison again
verified all 31 files against the pin. Separately, SHA-256 checks against pinned blobs
verified every entry in all **15 saved source manifests**. Those checks verify source
binding; they are not artifact regeneration or independent coverage replication.

I did not regenerate the saved Monte Carlo studies, add experiments/tests/packages,
run production GTSAM or application tests, test physical consumers, validate disk/network
durability, or inspect the other specialist's report. Existing-test success supports
the bounded implementations; the independent mathematical scope assessment above is
what supports the preservation verdict.
