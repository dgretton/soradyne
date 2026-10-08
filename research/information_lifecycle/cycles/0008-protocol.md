# Cycle 0008 protocol — replay must recover the consumed input set

Declared before implementation/execution, 2026-10-08 UTC. Parent `1f373c6`.
Scope: partial F01/F02; exact independent scalar inference, bounded logical events.

## Hypothesis and reference

A durable log horizon alone does not identify unequal live input histories. A scalar
checkpoint with its exact consumed-input manifest can be sufficient for continuation
in this additive model. A mixed checkpoint that loses inference permission must be
discarded; its manifest cannot reconstruct missing numerical components. A displayed
answer may be usable as a checkpoint ONLY with the required sufficient state and
coverage/permission contract. Do not assert that outputs can never be sufficient.

Independent observations of a dimensionless static scalar x, flat prior. Source IDs
d,a1,a2,b1,b2,b3 are immutable application identities, not guaranteed feed sequence
numbers. Values are (0,2,10,-4,8,-6) or all zero. Variance(d)=1, each a variance Va,
each b variance Vb, with Va,Vb in {1,4}. The independent oracle normalizes weights
1/variance over the requested, currently permitted unique IDs. Candidate arithmetic
uses additive (precision, precision-times-mean). Exact fractions, no randomness.

Original worker's checkpoint C consumes {d,a1,b1}. Its final published result consumes
S={d,a1,b1,a2}; a durable result-side manifest identifies S and model configuration.
Standby sees {d,a1,b2}. Both have the same durable log horizon {D:1}; live records differ.
No candidate receives missing raw values from the oracle. Candidate initialization gets
only its declared checkpoint/output, manifest, warm inputs and current policy.

Producer A and the old worker disappear at failover. Custodian M1 survives, retaining
all initial records, or missing a1, or missing a2. A retention acknowledgment is per
record and names M1; no receipt is fabricated for the absent item. M2 has the missing
record but is unreachable until a later declared event. This is not unbounded durability
or a claim that acknowledgments alone ensure future reachability. B remains active and
later acquires b3. Custodian replay preserves origin IDs; routes do not create evidence.

## Methods and permissions

1. **manifest_checkpoint**: restore (H,eta) bound to exact C; keep C as one aggregate
   block, replay target S minus represented IDs, and deduplicate by identity. Do not
   retain replayed copies already covered by C. If P3 affects C, discard the whole
   block, retain independent allowed tail inputs, and request reconstruction. Publish
   only when exact requested allowed coverage is present.
2. **raw_replay**: rebuild S from current authorized custodian records, deduplicated.
   No checkpoint/output reuse. Explicitly unavailable until required coverage is present.
3. **horizon_warm**: negative control, accept the warm input set because its durable
   log horizon matches. Do not request live-history replay. Filter permissions and admit
   later b3, but falsely claim recovery of the requested result instead of declaring a
   different/reduced dataset. An honestly labeled new dataset would be another contract.
4. **output_seed**: negative control, initialize from the latest output mean/variance
   and replay S as additional independent evidence. Deduplicate replay records among
   themselves, but fail to account for overlap with the seed. Respect the seed's complete
   lineage on P3; this isolates overlap errors from unauthorized reuse.
5. **hold_output**: negative control, keep the old output through missing history,
   new observations and revocations. Smoothness alone must not make this pass.

Rights to restart checkpoints, manifests, raw replay and outputs are explicit trusted
fixture inputs. P3 removes b1 either before restart, during replay, after replay, or
not at those stages. Later a2 is withdrawn in every run. Authority is synchronous;
effective/learned/enforced event indices coincide. No permission widening at failover.
Rejected forbidden records must not enter candidate numerical processing. No algebraic
subtraction of a forbidden raw record from a mixed checkpoint is authorized here.

## Twelve states and grid

0. Original result on S, policy 1, before failover (common reference endpoint).
1. Replacement starts; install pre-restart b1 withdrawal if configured, then restore.
2. First half of M1's initial records is replayed, with a duplicate.
3. Install during-replay b1 withdrawal if configured.
4. Second half of M1's initial records is replayed, with a duplicate.
5. Install after-replay b1 withdrawal if configured.
6. Retry M1 replay under current policy, with duplicates.
7. B acquires b3; extend requested target with b3, explicitly authorize and deliver it.
8. Duplicate b3 and retry M1, no new independent data.
9. M2 becomes reachable and supplies the missing record (or a duplicate in complete case).
10. P3 withdraw a2 immediately from future inference.
11. Replay all remaining authorized original records and b3, including duplicates.

Replay order is forward/reverse by stable synthetic ID. Values (2) x variance pairs (4)
x M1 coverage (3) x withdrawal stage (4) x order (2) = 192 configurations per method;
960 runs, 11,520 query outcomes. Unit tests also reject mismatched model/manifest binding,
conflicting same-ID records, and resource limit violations. No crash between writes,
distributed role fencing or asynchronous authority discovery is claimed.

## Checks, interpretation and bounds

Whenever supported methods publish, require exact oracle mean AND variance, unit weight
per requested permitted input, exact coverage, and no forbidden processing/output. Record
missing IDs explicitly. After replay retry (state 6), raw recovery is available only
with complete M1 coverage. Checkpoint recovery can also bridge missing a1 under P0;
after any b1 withdrawal it requires rebuild and cannot bridge that missing component.
Missing a2 blocks both supported methods until M2 arrives. After state 9 both supported
methods must recover and remain exact after subsequent revocation/replay. Partial replay
is never advertised as full recovery. Availability loss is separate from numerical error.

Record requested/actual input IDs and weights, acquisition/receipt/policy/publication
events, mean/variance errors, coverage mismatches, permission failures, adjacent-valid
corrections, new-data count, first recovery event, replay admission actions and structural
storage counts. A gap makes a correction undefined. Retained evaluator coefficients
audit numerical influence but are not given to candidate algorithms. Preserve a witness
with identical horizons and different outputs, and exact false-precision/hold witnesses.

Bounds: at most 16 source IDs, 16 retained entries, 32 deliveries per batch, 32 logical
states. These are structural bounds, not byte/latency or hardware-safety guarantees.
Keep prior sources/artifacts unchanged. Save metrics for every configuration, several
complete representative traces, source hashes, and byte-for-byte regeneration checks.
Report limitations and next smallest recovery slice; do not expand this into all F cases.
