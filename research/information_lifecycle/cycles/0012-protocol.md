# Cycle 0012 protocol — calibration and selective withdrawal contracts

Declared before implementation/execution, 2026-10-08 America/New_York
(execution begins 2026-10-09 UTC). Parent `ac6abd0`; research branch only.
First of at most four closing cycles. Scope I07 + R05; replaces the cancelled
converged-freeze/new-batch experiment. Reuse 0005 and 0009, leaving their files intact.

## Question and model

What numerical state and identities must remain available so shared calibration,
calibration drift/replacement and sensor/time/field withdrawal remain solvable?
An explicit active calibration variable is a candidate, not a required conclusion.
No estimator tuning, network/clock faults, physical motion or production code.

All models are tiny exact linear Gaussians, dimensionless measurement/state quantities,
rational means/variances, flat prior on target x and proper independent stated priors.
Fixed fixtures only: no randomness or statistical calibration claims. Acquisition times
9, 10 and 11 are exact integer ticks in one declared common clock, with half-open
selection [10,11). They illustrate a boundary convention, not clock synchronization.
Runtime publication times are distinct from those immutable acquisition times.
All data, identity, noise models and complete policy snapshots are trusted inputs.

### W1: shared temporal calibration and alternative retained representations

Let b0 ~ N(0,tau), d ~ N(0,q), b1=b0+d. Two observations are
z0=g*x+b0+e0 and z1=g*x+b0+d+e1, independent e variances r.
Candidate retains independent factors on (x,b0,d) and solves a 3x3 information system.
Independent oracle integrates the calibration in observation space: covariance
V=[[tau+r,tau],[tau,tau+q+r]], then uses explicit two-observation GLS for x.
This representation retains z, V and gain; rebuilding after a model revision also
needs its parameter/dependency recipe, not just an old V stamp.

Compare exactly on tau,q in {1,4}, r,g in {1,2}, observations (0,6) and (1,4):
32 configurations. Deliberate controls set off-diagonal covariance to zero or drop
drift. Record joint means/covariances and marginal x, not only one scalar mean.
Use tau=4,q=1,r=g=1 as the main witness: expected x=(2,14/3).
Any candidate/reference mismatch is a failed mathematical/implementation check.

### W2: calibration revision, replacement, and discarded state

The two observation worlds above have identical marginal x under tau=4,q=1,r=g=1
and identical identity-only manifests. Replace drift q by 4, replacing its model
factor rather than appending another independent prior. The worlds must give different
new x means despite identical previously collapsed state. This is an insufficiency
witness for that marginal schema, not for all possible compressed representations.

Also change g from 1 to 2 for the same explicitly selected historical inputs.
Rebuild coefficients from permitted observations/model, compare to the independent
GLS oracle, and show that relabeling the old frozen factor's model revision fails.
A different physical calibration identity is not a revision of the same quantity:
reject reuse without an explicit mapping/rebinding decision. No production registry
or universal retroactive-update policy is supplied by this experiment.

### W3: selectors and calibration learned from withdrawable evidence

Known base b~N(0,4); training c=b+noise, c=6 with variance 1; independent target
records y=x+b+noise, all variance 1. c is sensor A, time 10, field calibration-offset.
Targets (value,sensor,time) are (0,A,9), (6,B,10), (12,A,11), field signal.
Training and target records share one calibration identity; original record revision,
sensor, acquisition time and field identity survive reduction as dependency metadata.

P3 selectors are sensor A; interval [10,11); field calibration-offset; and their
conjunction. Rebuild from precisely the allowed original records. Candidate uses
two-variable information assembly (baseline.py); independent oracle conditions b on
surviving calibration training and uses mean(target)-mean(b), Var(b)+1/n.
Test exact means, variances, surviving IDs, transitive calibration support, boundary
membership, and order invariance. A field selector must remove training's downstream
influence, not merely records whose displayed output field has that name.

Compare reuse of a learned calibration derivative with support c under two EXPLICIT
policies: P3 disallows its use once c's contribution is withdrawn; P2 deletes c's raw
read access but grants use of that named derivative. The latter is not strict
contribution withdrawal. P3 may rebuild from still-authorized numerical state or report
unavailable if only the forbidden derived state survives. Metadata alone supplies no
missing values and does not grant permission. Prior 0001/0005/0009 insufficiency
witnesses support that limitation; do not claim a new universal impossibility theorem.

### W4: correlated fields within a record

Two signal fields measure x with values (u,v)=(0,4), noise covariance
R=[[1,1/2],[1/2,1]]. Withdraw v. Proper surviving likelihood uses u and R_uu=1,
so x=(0,1). Full-data x=(2,3/4). Deliberate precision-slicing controls either retain
the full information-vector component (forbidden v influence) or rebuild it from u
but retain conditional precision (false certainty). Verify that v changes cannot
change the correct surviving answer. The noise model is known independently and
remains authorized; a noise model learned from revoked fields would itself need
dependency/permission handling. No post-withdrawal processing of v is needed by the
correct method. This does not grant use of a revoked joint artifact for cancellation.

## Checks, bounds and evidence

- All mathematical comparisons use exact fractions; tolerance zero. Positive cases
  match an independently structured closed form. Negative controls must differ or
  be explicitly rejected; do not weaken conditions if one unexpectedly passes.
- Maximum matrix dimension 3, two drifting observations, at most 8 selector records,
  32 drift configurations and the four declared selector cases. No event loop,
  Monte Carlo, external input, network simulation or unbounded search.
- Test model identity mismatch, invalid selector intervals, empty target set,
  duplicate/conflicting input identity rejection, calibration dependency declarations
  and refusal of forbidden derivative reuse. These are contract witnesses, not a
  hostile-input parser or an implementation of the complete grant system.
- Save one compact `runs/0012/results.json` with all exact outputs, configuration,
  source hashes and named controls. Reproduce byte for byte in a fresh directory.
  Run the existing suite and lint; verify older source manifests unchanged.
- Record command outcomes, errors/fixes, structural resource counts, limitations and
  the minimal contract implications. No accuracy, latency or hardware-safety claim.

Application references: nestbox-ng `041f1146df2e2a271cd0489fda520bc65f2e9f70`,
CONVENTIONS §§3–6 and GLOSSARY calibration/nesting; shared-flow contracts §§1–3,5.
No owning contract will be edited. Next cycle is I08 + S02, not more calibration work.
