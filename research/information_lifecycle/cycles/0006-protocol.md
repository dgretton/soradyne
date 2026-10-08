# Cycle 0006 protocol — returning parent influence

Declared before implementation/execution, 2026-10-08 UTC (2026-10-07 local).
Parent: `425994c`. Partial I04; exact dimensionless scalar model only.

## Question and reference

Can versioned, acyclic artifact dependencies still recycle a parent's information as
new evidence? Compare the simplest restricted upstream contract with explicit original-
evidence accounting and a narrowly valid scalar residual calculation. Test withdrawal
through returned state and a delayed reply whose conditioning parent is no longer current.

Independent observations of x, flat prior: a=0, b=10, c=-8 in the contrast fixture,
and a=b=c=0 in the equal fixture. Variance(a), variance(b) each in {1,4}; variance(c)=4.
Only a,b initially exist; c arrives once later at the parent. Conditional noises are
independent, no shared calibration or nonlinear elimination in this slice. The oracle
uses the inverse-variance weighted mean and variance of the unique currently required
original observations, independent of message revisions or candidate recursion. Use
exact Fractions and zero numerical tolerance, with no random seed or coverage claim.

Grid: two value fixtures x four a/b variance pairs x two return paths (direct / a
transparent relay with a new artifact identity) x feedback rounds K in {1,4,12} = 48
configurations. Five methods = 240 runs. The immutable artifact graph always references
already-created artifacts; assert acyclicity while separately checking original evidence.
The relay contributes no measurement and preserves the original conditioning-base ID.

## Representations and methods

All methods initially receive a at the parent and b as a local child likelihood. Parent
local inputs are a and later c; b's stable child slot is replaced by newer revisions.
Returned posteriors have the arithmetic Q = P_sent + L_b in scalar information form.
The same old b is used every time; no new sample is created by recomputation. Packets
record kind, revision, immutable artifact dependencies, boundary, complete source
lineage, and original conditioning-base ID. All metadata/participants are trusted.

1. **local_only:** accepts only local likelihoods upstream. Reject parent-conditioned
   replies, retain the already admitted b likelihood, and permit downward estimates for
   consumption outside this inference loop. This is a restrictive message contract,
   not a blanket rule that shared-flow references must be acyclic.
2. **components:** returned payloads carry separable original factors. Union stable
   observation IDs, reject conflicting copies, and filter withdrawn components before
   numerical use. This can salvage lawful b from an old mixed return. It is raw-equivalent
   retained information, explicitly permitted here, not constant-memory compression.
3. **bound_residual:** for a posterior reply, require the exact retained sent-parent
   ID, boundary and lineage, and subtract its information pair: L_b = Q - P_sent.
   The child declares b as its local evidence, with no hidden additional factors;
   this assertion is trusted. Replace the stable b slot; do not append the residual
   every revision. Missing/mismatched bases or nonproper residuals are rejected.
   This is only scalar additive algebra with matching model/context, not a license
   to subtract marginalized nonlinear priors. A direct local-likelihood export is
   simpler in this model and needs no retained sent-parent history.
4. **posterior_replace:** negative control. Uses newest child posterior as if it were
   a local likelihood and adds parent local inputs. Full lineage authorization checks
   remain active; no append-only duplicate bug is needed for the failure.
5. **direct_only_revocation:** same incorrect fusion, additionally removing only the
   revoked direct input and ignoring its transitive influence in the child's state.
   This control must expose forbidden influence even when a mean is unchanged.

Scalar methods are not given the components carried only by the components method.
An evaluator-only linear coefficient map tracks actual original numerical influence;
methods cannot read it to decide admission, subtraction or fusion. Compare declared
lineage and actual influence. Coefficient maps are diagnostics, not extra candidate data.

The residual-derived b likelihood is explicitly authorized for retention/inference here:
its matched additive parent influence has been removed exactly before withdrawal, and
its declared/actual numerical dependency is b alone. This does not infer derivative
rights from a raw grant or erase its historical causal ancestry. After P3, an incoming
mixed posterior depending on a is rejected BEFORE subtraction; there is no policy
exception to clean a now-forbidden payload. Production derivative policy is unresolved.

## Fixed event sequence

1. Initialize local a and local b revision 1; query required {a,b}.
2. K rounds: send the current parent estimate, child adds the SAME b and returns a
   newer posterior (through the selected path). Deliver it, a duplicate and old local
   revision 1. Query after the complete delivery batch. Capture per-delivery decisions.
3. Construct a reply from the current parent but delay it. Add c at the parent and
   query required {a,b,c}. Deliver the delayed reply and query again. bound_residual
   must use the old sent base, not the new current parent including c.
4. Construct another pending reply from the current parent. Apply P3 withdrawal of a
   atomically; require {b,c}; query immediately. Deliver the pending mixed reply, its
   duplicate and old local revision 1; query again. No re-grant occurs.
5. Authorized child sends clean local b at a newer revision; query. One final clean
   parent-conditioned return tests whether incorrect feedback resumes after recovery.

K+7 states/run, 3,040 total event/query outcomes. Rejected feedback does not delete a
previously valid, explicitly retained local likelihood. A contaminated active posterior
is invalidated under full P3 checking; missing required b gives unavailable, not a
silently partial answer. The direct-only control deliberately skips that invalidation.
Policy is trusted/synchronous: effective, learned and enforced coincide. This is not a
simulation of remote revocation discovery, cryptographic erasure or hostile metadata.

## Additional predeclared witnesses

- **Wrong-base subtraction:** a,b unit variance, c variance 4. A pending Q=P_old+L_b
  arrives after c changes the parent. Q-P_current can have positive precision yet equal
  L_b-L_c, deleting c's legitimate influence when reused. Retain that negative result
  and actual coefficient map; also test b variance 4, where this residual has zero
  precision and nonzero information and must not become a proper Gaussian answer.
- **Bounded but wrong feedback:** a,b unit variance, contrast values, P0=L_a+L_b.
  Iterate P_next=L_a+(P_previous+L_b)/4 for 12 rounds without any new observations.
  Check the independent coefficient recurrence and its limit: weights tend to 4/3 and
  1/3, mean to 2, reported variance to 3/5, while the unique-data answer stays (5,1/2).
  This particular damping rule is a negative control; no general claim about all
  mathematically justified damping or iterative solvers follows.

## Acceptance, bounds and artifacts

Every available admissible answer must match the unique-data oracle, have exactly
required numerical dependencies, zero forbidden influence, and unchanged uncertainty
under replay/no-data feedback. Unavailable and rejected-message outcomes are separate
from incorrect answers. Test residual base/model mismatch, absent base, identity/revision
conflicts, stale deliveries, immutable containers and missing required coverage.

Measure mean/variance errors, actual original coefficients, claimed lineage, forbidden
IDs, precision ratio, consecutive mean changes/total variation, admission decisions,
active payload coefficients and retained base counts. Acyclicity and version replacement
must pass even for the negative controls, separating those properties from independence.
There is no physical consumer or smoothing of an authoritative invalid result.

Bounds: at most 16 feedback rounds, 64 query events/run, 256 artifact nodes/run, 64
retained bases, 16 source IDs, 16 components/message and 8 deliveries/event. Fail loudly
on exhaustion; do not trim evidence to pass. Byte counts and real-time deadlines are
not established by these structural bounds. No Monte Carlo randomness is used.

Store metrics for all 240 runs and representative complete traces for all five methods
in (contrast,a-var=1,b-var=1,direct,K=12) and
(contrast,a-var=1,b-var=4,relay,K=4), plus both witnesses and final source hashes.
The remaining grid traces are reproducible from the committed configuration, not all
stored. Compare artifacts byte for byte in a fresh directory. Do not edit prior cycles'
sources or evidence. Document any test/model defect without weakening acceptance.
