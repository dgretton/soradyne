# Cycle 0005 protocol — shared lineage and shared calibration

Declared before candidate execution, 2026-10-07. Parent: `8cecff0`.
Scope: I03 in tiny exact linear models, plus a bounded P3 withdrawal/rebuild sequence.
I04 feedback, general I05 gauge handling and nonlinear models remain queued.

## Model and independent reference

Dimensionless x has a flat prior. A shared latent calibration b has one prior
b ~ N(mu, tau2). Each distinct observation is z_i=x+b+e_i with independent
e_i ~ N(0, sigma2). IDs r1,r2,r3 have fixed values 0,2,8. Conditional likelihoods
share the same immutable boundary/model identity; the identity of b is significant.
No outliers, dynamics or stochastic coverage claims are modeled. Fractions are exact.

For n unique authorized observations, the independent reference is obtained by
changing variables to s=x+b: s has posterior mean average(z) and variance sigma2/n,
independent of b. Therefore (mean_x, mean_b)=(average(z)-mu, mu), with covariance
[[tau2+sigma2/n, -tau2], [-tau2, tau2]]. Check x and x+b query uncertainty as well as
the full joint matrix. The candidate instead assembles 2x2 information factors and
inverts them using the existing exact-arithmetic baseline. Neither reference reads
the other representation. A missing calibration prior must not create a finite x answer.

Fixed grid: sigma2 in {1,4}; tau2 in {1/4,1,4,16}. Layouts:

- singletons: children {r1}, {r2}, {r3};
- unequal disjoint: children {r1,r2}, {r3};
- diamond: children {r1,r2}, {r2,r3}, sharing the SAME r2 evidence.

Parent packets are built from leaf likelihoods on (x,b), before marginalizing b.
The atomic representation preserves per-observation blocks through nesting. A
compressed child sums its conditional blocks into one matrix/vector plus a lineage
set. It carries no decomposition of that sum. A scalar child marginalizes b using
the prior version at construction, retaining a mean/variance and lineage set.
None of these schemas is a privacy boundary; atomic likelihood blocks may disclose
raw-equivalent information and are assumed explicitly authorized in this fixture.

## Candidates and negative controls

1. **atomic_joint:** merge by stable observation/block identity, reject conflicting
   copies, filter withdrawn blocks before using their numerical payload, and include
   the calibration prior exactly once. Keep the shared latent at composition.
2. **compressed_joint:** accept only disjoint or exact-duplicate conditional blocks;
   partial overlap is unsupported without a numerical decomposition. Reject a block
   containing withdrawn contributions. If required lawful coverage is then missing,
   return unavailable. Retaining b is not assumed to recover an unknown overlap.
3. **raw_rebuild:** independently authorized archive replay followed by full batch
   recomputation. Return unavailable when that archive is absent; do not synthesize it.
4. **scalar_ids_only:** deduplicate/check observation lineage and calibration-version
   stamps, then incorrectly assume the remaining scalar child marginals independent.
   This negative control should fail even with disjoint observation IDs. It returns
   unavailable for partial overlap, contaminated blocks or stale calibration versions.
5. **repeat_calibration_prior:** deduplicate atomic observations correctly, but add
   the shared calibration prior once per contributing child. Detect false precision
   even when the mean agrees. It is a negative control, not an acceptable shortcut.
6. **keep_frozen_on_withdrawal:** atomic composition that ignores withdrawal. Track
   forbidden IDs independently of mean/covariance; smooth/stable output cannot pass.

Every result that claims the full authorized answer must have exactly the required
lineage and exact reference mean/covariance (where that representation exposes it).
Unsupported/unavailable outcomes are recorded separately from incorrect answers.
Returning fewer lawful inputs is not silently counted as preserving full information.

## Five fixed event states per grid/layout

0. Initial packets revision 1, mu=0, calibration prior version 1, all IDs allowed.
1. Re-publish equivalent revision 2 packets; include duplicate revision 2 and late
   revision 1 deliveries. The information set is unchanged. Replace, never append.
2. Calibration PRIOR update to mu=2, variance tau2/4, version 2, without new
   observations or changed latent definition. Conditional joint likelihoods remain
   applicable; cached scalar marginals carry the old prior and must be marked stale.
3. P3 withdraw r2 immediately; require exactly {r1,r3}. Parent raw archive is
   unavailable in this state. Atomic retained blocks may still serve permitted r1/r3.
   A compressed mixed block cannot reuse r2 or pretend to isolate r1 from its sum.
4. Authorized children rebuild revision 3 packets from {r1,r3}; empty children publish
   an empty revision/tombstone. Include old revision 1/2 and duplicate revision 3
   deliveries. The archive is available again and scalar children use prior version 2.

These are trusted local event snapshots, with equal effective/learned/enforced times
for the new policy. No grant issuer, network enforcement, receipt queue or physical
erasure implementation is claimed. Current allowed input identities are external
policy inputs, as distinguished from retention permissions in cycle 0004. Invalid
blocks are screened by metadata before numerical assembly. The evaluator retains
the synthetic truth; candidates are given only their stated numerical representation.

Archive access and retained atomic blocks are deliberately different resource
assumptions. Report their information/metadata sizes; do not claim atomic storage is
more compressed or universally authorized than raw retention. No smoothing is applied.
Record numerical jumps, unavailable states and forbidden use alongside uncertainty.

## Two explicit insufficiency witnesses

1. **Same marginals, different calibration dependence:** two child means 0,8, each
   variance 5, disjoint observation IDs. Shared bias variance 4 implies combined
   variance 9/2; independent child biases imply 5/2. The scalar means, variances and
   observation IDs alone do not identify which dependence model applies. This is a
   schema insufficiency witness, not a ban on marginals with adequate cross-covariance.
2. **Same overlapping aggregates, different unique-data answer:** worlds (0,2,8) and
   (2,0,10) give identical child sums 2 and 10 and identical lineage {r1,r2}/{r2,r3}.
   Their unique-data means are 10/3 and 4. Even aggregate joint likelihoods on (x,b)
   are identical across worlds. Without the overlap's numerical contribution or
   extra decomposition/replay, lineage metadata cannot reconstruct both answers.

For the second witness, also calculate the honest answer using only the two retained
averages with their known covariance: without calibration, each has variance 1/2
and cross-covariance 1/4. Their combined mean is 3, variance 3/8. This represents
less information than all three raw observations (variance 1/3); distinguish honest
information loss from the false precision 1/4 of treating the aggregates independent.
For this two-average calculation use direct 2x2 covariance algebra, not the candidate
information assembly. Do not label the honest reduced-data answer mathematically wrong.

## Checks, resource bounds and evidence

24 grid/layout configurations x 5 states x 6 methods = 720 recorded outcomes.
No randomness or seed; cap source observations at 16, packet deliveries at 32 and
blocks per packet at 16. Reverse delivery order and check identical outcomes as a
determinism check, not independent statistical trials. Reject revision conflicts,
boundary/model mismatch and invalid/oversized requests. Preserve all negative results.

Exact checks: independent batch agreement; full joint/query covariance; atom/whole-
block replay idempotence; no finite x estimate without a calibration prior; correct
unavailable reasons; zero forbidden contribution for admissible candidates; restored
coverage after rebuild; calibration prior counted once; source identity not inferred
from packet identity; two witness equalities and differing required answers.

Record configuration, versions, represented/forbidden/missing IDs, retained numeric
coefficient and lineage counts, mean/variance and their exact error, covariance/query
error where available, output availability and mean jumps. Store one compact JSONL
stream plus aggregate metrics/source hashes. No physical-safety, Monte Carlo coverage,
distributed-authority or performance deadline claim follows from these tests.
