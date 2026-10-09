# Cycle 0013 protocol — coherent revisions and reset meaning

Declared before implementation, 2026-10-09 America/New_York. Parent `aebef84`.
Second of four closing cycles: I08 + S02 only. Exact contract witnesses, not distributed
coordination, fault recovery, controller tuning or further nonlinear coverage.

## I08: nested results must bind compatible dependencies and uncertainty

Use a three-edge scalar translation chain W <- P <- C <- B, all coordinate epochs
unchanged across two inference revisions. In each coherent world:

```
outer = 10 + k
middle = -3 - k + e
leaf = 2 + f
```

k,e,f independent; e mean 0 variance 1, f mean 0 variance 1/4.
Old calibration k has mean 0 variance 4; new k mean 100 variance 9. Therefore
the independent original-variable oracle is world(B)=9+e+f, mean 9 variance 5/4
in either world. This is one shared calibration revised, not two independent inputs.
State units are metres and covariance m^2; no physical motion is implied by revision.

Outer revisions 1/2 and middle revisions 11/12 have different local counters. Each
middle names the exact outer revision and calibration revision it used. Leaf revision
7 is independently reusable. All edges use a declared scalar additive representation,
fixed frame/epoch identities and half-open applicability [0,20), queried at time 10
on one trusted clock. Cross covariance outer/middle is -4 or -9; other cross terms
are zero. An uncertainty bundle binds the full ordered edge-revision tuple, representation
and numeric covariance. Its values are trusted fixture evidence, not certified by IDs.

Enumerate 2 outer x 2 middle x 2 uncertainty bundles = eight combinations. Exactly
two are coherent. Positive candidates check dependency closure, shared calibration
binding, frame/epoch continuity, representation, applicability, covariance binding
and the current trusted permission snapshot on actual inputs. All accepted mean AND
variance must equal the independent oracle exactly. Negative control takes latest
parts independently and sums their supplied covariance without compatibility checks.

Enumerate all six arrival orders of new outer, middle and uncertainty bundle, three
prefixes per order. A request for the complete new revision must remain unavailable
until all its dependencies are present. A separately labeled old-revision query may
remain usable only while its evidence is still permitted; it must not masquerade as
the new revision. No global transaction or synchronized revision counter is assumed.
Test a new policy that revokes old calibration support, and another policy revision
that preserves rights. Coherence is not authorization; a policy-version change alone
does not necessarily make an otherwise permitted artifact unusable.

Use a bounded issuer mapping symbolic opaque result IDs to immutable answer manifests
(query endpoints/time, contributing revisions and inputs, model/calibration/representation,
coordinate epochs, uncertainty bundle and acceptance policy). Identical means/covariances
from different manifests get different result identities. The public projection can
omit private source IDs/dependencies while retaining query and validity semantics.
This is not cryptographic privacy, signatures, freshness discovery or an access-control
implementation. Public result identity must not grant provenance access or certify math.

## S02: physical event versus change of coordinates

Known planar re-expression at an instantaneous event uses
`p_new=R*p_old+t`, R=((0,-1),(1,0)), t=(100,-20). Point p=(1,2), target=(4,6),
obstacle=(10,3), velocity=(2,-1), point covariance diag(1,4). Transform all coordinates,
velocity (R*v) and covariance (R*C*R^T). Independently check exact squared distances
and expected transformed quantities. No physical time elapses at the chart switch;
one publication tick between representations is NOT a measurement time interval.
Treating that coordinate difference as velocity is an intentionally wrong control.
Moving only the point or failing to rotate covariance must also fail.

An independent scalar counterexample gives the same displayed point jump 1 -> 101
for two different events: pure coordinate-origin shift also changes target 10 -> 110;
physical bump leaves target at 10. Correct relative separations are 9 and -91.
The point jump alone cannot determine event kind. Do not synthesize that information
from smoothness, magnitude or a preferred controller response.

Represent separate coordinate and motion epoch identities, old/new relation direction,
event kind/knowledge, effective boundary and relation uncertainty. Intervals are
[0,10) and [10,20); equality belongs to the new epoch. A known re-expression supplies
a bridge, a physical bump is a new physical-state epoch rather than coordinate transport,
and unknown/ambiguous reset relation makes cross-epoch continuation unavailable.
An independently supplied within-new-epoch relative query can still be valid.
Cause classification is supplied/verified externally in this witness; no bump detector.

For an uncertain common translation bridge delta with variance 9 independent of point
and target errors of variances 1 and 4, each absolute variance increases (to 10 and 13),
their cross covariance is 9, and relative variance remains 5. Independent copies would
wrongly give 23. This exact scalar subcase names the correlation contract; it does not
establish SE(3) reset uncertainty transport or independence for general reset estimates.

## Acceptance, bounds and reproducibility

All comparisons are exact fractions, tolerance zero. Preserve expected negative controls
and the independent algebra. At most three edges, eight combination checks, six delivery
orders with three prefixes, three planar points and four event classifications. Issuer
holds at most four result bindings. No Monte Carlo, queues, unbounded search, network,
actual clocks or physical actuation. Identity/lineage authenticity and numerical payload
truth are assumptions. A passed manifest check is not a proof of underlying provenance.

Predeclare tests for mismatched dependencies/covariance, epoch endpoints, representation,
half-open query boundary, actual-input permission, immutable result binding and public
projection. Save all combination and prefix outcomes plus exact reset witnesses in
`runs/0013/results.json`, with source hashes. Require byte-for-byte reproduction, prior
manifest preservation, full existing tests and Ruff. Record defects without widening
scope. Next is F04, then synthesis/reviews; do not start either in this cycle.

Application reference: nestbox-ng `041f1146df2e2a271cd0489fda520bc65f2e9f70`,
CONVENTIONS covariance/time and GLOSSARY lookup/epochs. Shared-flow §§1–3,5 remain the
provider authority. All proposed refinements stay in this research directory.
