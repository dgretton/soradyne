# Cycle 0009 protocol — retention expiry and withdrawal granularity

Declared before implementation/execution, 2026-10-08 UTC. Parent `bb35fca`.
Scope: bounded R03+F03 compound, exact independent scalar information and logical time.

## Model and hypotheses

Static dimensionless x, independent Gaussian observations, flat prior before permanent
datum d=0 with variance 1. At time 0 acquire a1=0,a2=10,b1=-4,b2=4, or all zeros.
Variance pairs (Va,Vb) are (1,1) or (4,1); all A observations have Va, all B have Vb.
At time 8 a new permitted n=-8 (zero in equal fixture), variance 1, arrives. Identity is
application-supplied and stable. Candidate means/variances are exact Fractions.

Reference normalizes inverse-variance weights over specified permitted unique inputs.
The strict query requires d and ALL still-permitted historical observations plus n once
acquired. Expiry does not silently change that requested input set. An explicit reduced
query contract may instead return the posterior on its declared surviving subset, with
missing IDs and reduced status. Compare that answer to its own subset oracle AND report
its difference/lost precision relative to the full requested answer.

At t=1, local raw records leave the active window and become retained summaries. An
independent authorized archive deletes its raw records at E in {3,6}. Summary artifacts
expire at C in {7,11}; the experiment deliberately has E<C. Availability intervals are
half-open: t>=expiry is too late. Raw expiry preserves separately authorized derivatives
(P2), not strict contribution withdrawal. Artifact expiry removes that artifact's use
right; it is not permission to relabel the same artifact with a fresh expiry. Rebuilt
summaries of these historical records share the fixed original deadline C. No extension
or unbounded cache renewal is authorized. The d and n blocks expire at 16, beyond the run.

P3 at W in {2,3,6} withdraws either a1 alone or both A records. Raw and derivative
inference are denied for those IDs immediately. Remaining raw replay is explicitly
authorized while the archive retains it. Replay reads data at completion time W+D,
with D in {0,2}; a pending request does not pin raw records or preserve a hidden copy.
Retaining a request-time copy would be a different retention contract, not tested here.
Current permission and both retention deadlines are checked before numerical use.

## Six methods

1. **atomic**: freeze each independent record separately (four summaries). P3 drops
   only affected summaries. This scalar single-record summary is raw-equivalent
   information, requiring explicit derivative rights; it is not a privacy reduction.
2. **by_source**: two summaries, A and B. Whole-A withdrawal can remove A exactly.
   Withdrawing a1 makes the mixed A summary unusable; replay is needed for a2.
3. **mixed**: one A+B summary. Any P3 invalidates it; recover all permitted missing
   inputs from the archive when possible, otherwise report unavailable.
4. **honest_subset**: same retention/replay as by_source, but explicitly publishes
   reduced status with actual coverage when full coverage is absent. A strict caller
   must not mistake that for full recovery. No old forbidden information is retained.
5. **coarse_complete**: negative control, same arithmetic/state as by_source but
   silently claims full coverage after dropping an affected group or expiry.
6. **hold_mixed**: negative control, retain the mixed checkpoint despite P3 until its
   normal expiry; always claim full coverage. Tests must reject its unauthorized use,
   even with equal means or small jumps. It honors ordinary expiry, isolating P3 misuse.

All methods preserve d and add n once. Rebuilt state uses only missing permitted IDs,
with exact disjoint coverage; duplicate replay cannot increase precision or renew expiry.
Strict methods publish only with complete requested coverage. The subset method reports
its numerical uncertainty for the actual subset, not falsely for the requested full set.
Its availability is a separately declared product contract, not universally sufficient
for control. No smoothing or physical consumer is modeled.

## Events, bounds and comparisons

Query after events at every integer t=0..12. At t=1 freeze then clear local raw. At every
time enforce archive/artifact expiry first, then P3 if due, then finish due replay, then
acquire n if t=8. A failed replay is not magically retried from an expired archive.
Record the immediate policy enforcement before any due replay; D=0 can lawfully rebuild
in that same logical event but cannot keep the withdrawn checkpoint as a bridge.

At t=12, attempt a duplicate of any successfully rebuilt historical summary with its
original deadline; it must be rejected as expired. No candidate can read the evaluator's
source records. Historical evaluator traces are retained for audit, not claimed erased.

Two value fixtures x two variance pairs x two E x two C x three W x two D x two withdrawal
scopes = 192 configurations/method, 1,152 runs, 14,976 time-query outcomes. No randomness.
Each run has at most six source IDs, six live numerical entries, four archive records,
one bounded replay request, 13 times and at most eight delivered records in a batch.
Generic guards cap 16 IDs/entries, 32 times and eight records/batch. Report coefficient
counts and stored-entry time integrals, not measured bytes or production deadlines.

Check mean AND variance, authorization, actual input weights, exact requested/declared
coverage, missing IDs, replay completion/failure reason, policy effective/learned/enforced
times, acquisition times, adjacent-valid corrections and response to n together. Track
time until a valid full reconstruction, permanent-within-run gaps, and loss after C.
Gaps make adjacent jumps undefined; a smaller jump is not automatically preferable.

Predictions: atomic preserves full answers after any P3 while t<C. By-source does so
immediately for whole-A withdrawal; single-record withdrawal needs a2 replay. Mixed
needs replay for either scope. Required replay succeeds only when W+D<E (and before C),
otherwise the requested information remains absent. At C the historical artifacts
expire and strict full-history answers must become unavailable; subset answers retain
d and n where available with honest weaker information. Failed/expired replay cannot
restore a hidden component. Forbidden processing must be zero in all except hold_mixed.

Preserve a two-world insufficiency witness: (a1,a2)=(0,10) or (2,8), unit variances,
otherwise identical inputs. The stored source/mixed sums, IDs and timestamps match,
but after removing a1 the required a2-bearing answer differs. This proves insufficiency
of THIS numerical aggregate plus identity-only manifest, not of arbitrary richer metadata.
No source-value hashes or raw values are secretly provided as an extra channel.

Keep prior sources/results unchanged. Save all run metrics, representative traces, the
two-world witness and source hashes; verify byte-for-byte regeneration. Document any
failed check or revised interpretation. Do not claim general fixed-lag/nonlinear or
distributed timing correctness from this static exact compound case.
