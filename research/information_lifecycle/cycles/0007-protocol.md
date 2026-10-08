# Cycle 0007 protocol — observability after anchor withdrawal

Declared before candidate execution, 2026-10-08. Parent: `74ac1de`.
Scope: partial I05 and R06 in a two-coordinate exact linear Gaussian model.

## Model and independent reference

Unknown coordinates (x,y) in one fixed external reference. Independent evidence:
anchor A0 observes x=5 with variance V; relative R observes y-x=3 with variance D.
Flat priors elsewhere. V in {1,100,10^12}, D in {1/4,4}. A later explicitly authorized
anchor A1 observes x=-2 with variance V/4, replacing A0 rather than granting old history.
All observations and source identities are synthetic and trusted.

A coordinate re-expression subtracts t from BOTH x and y. Grid t in {0,1000}; a
purely numerical gauge parameter g in {0,7}. Together: 24 configurations. Five queries:
x, y, d=y-x, midpoint=(x+y)/2 and -2d. The constant zero query is a separate unit case.
An arbitrary displayed/gauge coordinate is never an external-position observation.

Independent reference uses u=x and v=y-x. With both factors: u~N(a,V), v~N(3,D),
independently. For q=(q_x,q_y), q.(x,y)=(q_x+q_y)u+q_y v, so mean is
(q_x+q_y)(a-t)+3q_y and variance (q_x+q_y)^2 V+q_y^2 D. A missing component is harmless
ONLY if its coefficient is zero; otherwise the requested scalar has no finite posterior.
Missing components are not imputed as zero. Constant queries are deterministic.

The rank candidate independently assembles H=sum(c c^T / variance) and eta, with
measurement z'=z-t(c_x+c_y), then solves H v=q using exact rational row reduction.
An inconsistent system means the query is unobservable; otherwise query mean is
v^T eta and variance q^T v. Check these against the independent u/v reference and
selected non-axis-aligned rank-one factors. Do not treat the null space as zero variance.
There is no floating-point rank tolerance, Monte Carlo randomness or hardware model.
Weak but finite anchors remain mathematically observable with large uncertainty.

## Five methods

1. **rank_query:** retained independent authorized factors; rebuild the information
   pair after each policy/input event and decide observability for EACH scalar query.
2. **split_summary:** freeze and retain independent u and v scalar summaries separately,
   with their distinct source lineage/permissions. Discard raw factor objects for this
   candidate. This fixture explicitly authorizes those retained derivatives. A u or v
   summary is invalidated when its source contribution is withdrawn. Its query calculation
   uses the known linear combination, not the rank candidate's solver. This small model
   is a sufficient case for richer retained state, not a generic factorization theorem.
3. **blanket_failure:** conservative negative comparison: require an invertible full
   joint H before answering ANY nonzero query. Record unnecessarily unavailable lawful
   queries separately from incorrect available answers or unauthorized processing.
4. **hard_gauge:** negative control: when H is singular, pin free coordinates to g and
   report the resulting conditional covariance as if it described external coordinates.
   Prefer the y pivot, so relative-only data pins x=g; x-only data pins y=g; no data pins
   both. Relative observables may remain exact despite fabricated absolute confidence.
5. **retain_anchor:** negative control: use the rank-query algorithm but keep an anchor
   despite loss of its inference permission, until a newer anchor explicitly replaces
   it. Respect withdrawal of relative R. Track forbidden factor PROCESSING separately
   from forbidden outputs and from numerical query dependence. A relative answer can
   numerically cancel the anchor yet still be computed using a disallowed input under
   this fixture's explicit no-inference-use rule.

The accepted methods filter permissions BEFORE numerical assembly/use. Rights to raw
and derivative inference are stipulated policy inputs here, not inferred from numerical
cancellation. P3 is immediate and trusted; effective, learned and enforced event indices
coincide. No remote erasure or asynchronous authority discovery is claimed.

## Eight event states

0. Initial A0 and R, origin offset 0, gauge g, policy 1.
1. Re-express coordinates at origin offset t; no evidence or policy change.
2. P3 withdraw A0, retain authorized R, policy 2. No new observations.
3. Change internal gauge g -> g+11; origin and evidence unchanged.
4. Replay withdrawn A0 revision 1 (and a duplicate); it must not revive. Retained
   source identity and the current authorization govern admission, not a new route.
5. Authorize/deliver new A1 at anchor-slot revision 2, policy 3. R persists. Then replay
   old A0 revision 1; it cannot replace A1. The new anchor is an observation, not a
   change of coordinate origin. Its mean correction must not be hidden by smoothing.
6. P3 withdraw R, keep A1, policy 4. x remains observable; y/d need the missing relation.
7. P3 withdraw A1, policy 5. No nonconstant query has finite uncertainty.

24 configurations x eight states x five methods x five queries = 4,800 outcomes.
Expected availability is query-specific; unavailability is not an incorrect number.
Store current source IDs/revisions, input acquisition events, policy/event/frame epochs,
mean/variance or explicit unobservable status, canonical external-frame mean, query rank,
forbidden processing/outputs, numerical errors where comparable, invented finite answers,
needless unavailability, coordinate/canonical jumps and resource counts. Mean jumps are
undefined across unavailable states. Gauge changes should not alter any observable query.

## Checks and bounds

Exact agreement and correct unavailable status for rank_query and split_summary across
the full grid. Queries unchanged in canonical coordinates under origin re-expression;
relative outputs unchanged under origin/gauge changes and anchor withdrawal/reacquisition.
Full joint covariance when anchored is [[V,V],[V,V+D]]; test cross terms explicitly.
No finite external-position answer after anchor loss; no false global failure while
lawful relative queries remain. Empty input has rank zero and only deterministic constants.
No stale anchor revival; conflicting same-revision payloads and invalid/bound-exceeding
inputs fail explicitly. A missing relative constraint must also affect the correct queries.

Bounds: two variables, at most 16 retained factors/summary entries, 32 revision records,
eight deliveries/event, 32 event states/run. Fixed grid only; no open-ended simulation.
Report structural counts, not measured bytes or real-time deadlines. No source from prior
cycles is edited. Use existing stdlib Fraction code where appropriate; no specialized
external estimator is introduced. Preserve all negative controls, not only successes.

Save aggregate metrics for every configuration/method, complete representative traces
for V=1,D=1/4,t=1000,g=0 and V=10^12,D=4,t=1000,g=7 (all methods/queries), and source
hashes. Other traces must be reproducible from the grid. Verify byte-for-byte regeneration
and all prior source hashes. Document methods, corrections, limitations and next action.
