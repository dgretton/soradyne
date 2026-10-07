# Investigation plan: evidence, permissions and continuous operation

2026-10-07. This is an investigation plan, not a promised implementation or a proof
that every combination of desired behavior is possible. Prefer the least complex
method that passes the required information, authorization and stability checks.

## Questions the program must answer

1. What information must a frozen or exported representation retain so later results
   remain correct, uncertainty remains honest and revisions do not duplicate evidence?
2. Which permission changes allow reuse of retained derivatives, which require
   recomputation, and which make the requested inference impossible with lawful data?
3. Can an implementation update/recover fast enough to keep up with observations
   and access changes, with bounded memory and backlog?
4. How do corrected alignments affect consumers between measurements? Can a simpler
   transition protocol avoid preventable control disturbances without hiding a real
   bump, using forbidden information, understating uncertainty or delaying necessary
   loss-of-validity signals?
5. Which generic revision, entitlement, retention and publication contracts are needed
   from the provider, and which choices remain application mathematics or control?

## Application coverage

The application motivating the investigation is Nestbox: surveyed rigid bodies,
headset-to-room alignment across resets, cameras witnessing a shop cell, manipulation
using surveyed points, and a drone moving between sites with unequal local/global
precision. Use the current `/Users/rim/Dev/nestbox-ng` conventions, glossary,
SUBSTRATE and work breakdown E3/E7/E9-E12; record their commit when testing them.
Do not consult the retired original implementation.

The provider direction is `docs/20260911_shared_flow_demo_contracts.md` F1-F7.
Its choices supersede conflicting earlier custody/ownership assumptions. Its demos
are still proposed. Model the consumer's stronger requirements explicitly; do not
mistake a narrow consumer profile for a universal protocol rule.

## Permission semantics to vary independently

An entitlement is scoped by subject, resource/revision, audience/purpose, allowed
operation and validity interval. Test overlapping independent entitlement paths.
Keep transport reachability and current authorization distinct. Keep permission to
retain ciphertext, decrypt, compute, cache derivatives, publish and read results distinct.

Use these policy variants as experimental inputs, not silent product decisions:

- P0: uninterrupted authorized use, including checkpoint/replay.
- P1: future-only cutoff; previously authorized derivatives may remain reusable.
- P2: input/read retention removed, with an explicit entitlement to use specified
  retained summaries. No hidden assumption that raw deletion also deletes derivatives.
- P3: withdrawal of contribution from FUTURE inference, including frozen priors and
  cached derivatives. Previously delivered plaintext is not claimed remotely erasable.
- P4: one use/audience/grant withdrawn; another valid entitlement remains.
- P5: expiry or offline inability to establish current authority; policy distinguishes
  pending, expired, unknown and confirmed revocation, with any allowed grace explicit.
- P6: write/publish/role authority changes independently of read/retain authority.
- P7: restored access, possibly with gaps or altered audience/resolution/calibration.

For each event store issuer and policy version, effective time, time learned by each
participant and enforcement time. Define measurement time, receipt time, accepted
input frontier, solve revision and publication time separately. Test the policies
above on the same measurements so a policy choice cannot be disguised as math.

## Candidate ladder (simplest first)

A. An authorized raw-factor ledger with stable identities and full batch recomputation
   is the correctness reference. Its latency/storage may fail; it is not automatically
   the production answer. Use an independent closed form on tiny cases to check it.
B. Replaceable versioned summaries, plus replay/rebuild from retained authorized
   evidence. Start with exact linear joint elimination, with a provenance manifest.
C. Retain shared calibration/latent variables or small joint boundary factors instead
   of pretending their effects are independent. Bound separator size and measure cost.
D. Incremental updates/removals and fixed-lag checkpoints, if they reproduce the
   reference where exactness is expected and meet explicit approximation limits elsewhere.
E. Conservative unknown-correlation fusion, richer retained conditionals or selective
   replay only when a simpler method demonstrably fails. Document lost information,
   assumptions and cost; do not add an algorithm merely because it is sophisticated.

For each: establish supported policies/topologies, information loss, computational
cost, recoverability and observable failure response. Algebraic information subtraction
is exact only in the representation/linearization where its assumptions hold; subtraction
after eliminating shared variables is a hypothesis to test, not an automatic inverse.

## Scenario matrix

All entries below are PLANNED unless STATUS gives a run and its limited evidence
level. Each row needs a named test or simulation, an oracle and explicit failure.
An expected-to-fail naive method is a negative control, never an accepted solution.

| ID | Scenario and disturbance | Methods/comparison | Evidence required |
|---|---|---|---|
| I01 | Freeze a linear jointly Gaussian body/calibration block | Full solve vs exact joint elimination vs independent marginals | Mean AND joint/query uncertainty agree for retained variables; demonstrate lost cross terms in naive case |
| I02 | Repeated/reordered/duplicated export, then replacement and withdrawal | Stable summary revision vs append-as-new-evidence | No extra information from replay; newest authorized revision replaces atomically; stale revision cannot resurrect |
| I03 | Two children share raw evidence or calibration; diamond ancestry | Shared latent/lineage vs independent summaries | No double counting; uncertain overlap yields explicit conservative/unsupported outcome |
| I04 | Parent feeds information back, directly or through another child | Restricted topology vs explicit message accounting | Feedback is rejected or accounted for; an acyclic graph alone is not the oracle |
| I05 | Gauge changes; singular/weakly observable geometry | Relative queries vs artificial fixed gauge | Gauge-invariant query consistency; correct null spaces; no fictitious absolute precision |
| I06 | Freeze/relinearize a nonlinear SE(2), then SE(3) survey | Batch reference vs frozen linear prior vs rebuild | Monte Carlo coverage/error and approximation range; large rotations, poor baselines and ambiguous configurations |
| I07 | Calibration drift or replacement, shared tags, temporal correlation | Retained calibration variables vs independent per-observation noise | Shared error does not vanish with repeated images; distinguish simultaneous common-mode and independent noise |
| I08 | Query across nested domains with concurrent updates | Consistent revision snapshot vs mixed edge versions | Transform, covariance, epoch and provenance refer to one coherent result; cross-domain relative accuracy preserved |
| R01 | Future-only cutoff, previously authorized results retained | P1 vs fresh replay | No future input use; documented old-result reuse; no accidental global erasure |
| R02 | Delete raw replicas while specifically permitting frozen derivative | P2 vs P3 | Different legal state produces deliberately different behavior; cached artifact permissions independently checked |
| R03 | Withdraw one contribution before and after marginalization | Raw rebuild vs frozen-prior retention vs naive subtraction | Agreement with authorized-only oracle or explicit inability; expose irreversible reduction |
| R04 | Revoke one of several grants or one destination of reused evidence | Entitlement union and purpose-scoped lineage | Other lawful use survives; withdrawn use does not regain access through another audience's cache |
| R05 | Partial sensor/time-range/field withdrawal, including shared calibration | Fine-grained replay vs coarse invalidation | No forbidden contribution reused; measure how much valid information coarse invalidation loses |
| R06 | Losing the last anchor, scale constraint or connectivity edge | Rank/observability checks vs forced regularization | Relative observables retained where justified; unavailable directions not reported as precise |
| R07 | Expiry/offline revocation/late signed messages/key rotation | Effective/learned/enforced timelines | Defined staleness bounds; no stale acceptance or future disclosure; do not claim cryptographic erasure of prior plaintext |
| R08 | Re-grant after deletion or data gap | New grant generation + replay vs stale cache revival | Explicit gaps, idempotent replay and no resurrection beyond new entitlement |
| R09 | Admin, producer, compute, custodian and reader are different owners | Permute roles and revoke independently | Changes affect the correct rights; author identity survives delegation and recovery |
| R10 | Private-to-shared and reduced-resolution audience change | Per-audience output/replay policy | No unconsented historical backfill or finer disclosure; revoked output audience cannot query input provenance |
| F01 | Unequal lossy feed delivery, then failover | Durable horizon only vs checkpoint/input manifest vs invalidate | Recovered result reflects actual input set; output is not assumed a restart checkpoint |
| F02 | Producer disappears; authorized memory role survives | Replay from custodian; partial durability acknowledgments | Committed coverage matches acknowledgments; missing evidence is explicit |
| F03 | Fixed-lag eviction/checkpoint expiry followed by withdrawal | Retain decomposable state vs rebuild vs unavailable | No hidden reuse; quantify archive cost, active memory and rebuild deadline |
| F04 | Crash during export, checkpoint, epoch close or grant update | Atomic manifest/publish boundaries and idempotent retry | No mixed authority/data revision; cold restart yields allowed state for every interruption point |
| F05 | Role transfer, partition, stale replies and rejoining old holder | Exclusive fencing vs explicitly provisional subtype | No invalid output accepted; allowed loss of availability measured, not disguised as safety |
| F06 | Input/correction storm competes with archive replay | Queue bounds, coalescing, priorities, expiry | Live/control deadlines and p99 latency; no unbounded stale corrections; preserve durable evidence separately |
| S01 | Contradictory alignment revisions faster than new measurements | Raw switching, coalescing, hysteresis, explicit transition candidates | Measure correction/update ratio, total variation, error and validity; duplicate publications cause no new motion |
| S02 | Genuine physical bump vs pure coordinate-origin/SLAM reset | Epoch/reset handling and consistent transformation of all consumers | Real movement not smoothed away; pure re-expression does not create fictitious velocity or obstacle motion |
| S03 | Withdrawal shifts estimate during a moving manipulation task | Immediate change, permitted old-result transition, controlled hold/replan | Task clearance, stopping/replan feasibility and exposure to stale transform; P3 cannot secretly blend forbidden state |
| S04 | Anchor loss and reacquisition while approaching a dock | Validity gate, local tracking, staged reacceptance | No precisely wrong docking; characterize availability and false-stop tradeoff at explicit latency/clearance |
| S05 | Slow data, bursty corrections, delay, clock skew and out-of-order sensing | Acquisition-time association vs publish-time association | Error/coverage and transition behavior across bounded sweeps; no invented high-rate evidence |
| S06 | Large rotation/lever arm with small translation; coupled frame paths | SE(3) interpolation/relative queries and synchronized snapshots | Task-point error and velocity/acceleration effects; no naive scalar mix of radians/metres |
| S07 | Biased/ambiguous observations, robust-kernel switching or multimodality | Batch candidates vs local updates and validity decisions | Detect mode switches; honest uncertainty/ambiguity; smooth wrong answer must fail |
| S08 | Repeated loss/regrant/transfer cycles under finite compute | Hysteresis, bounded revision backlog, replay coalescing | No chattering, starvation or artificial certainty; progress after faults cease under stated assumptions |

## Dynamic stability experiments

Do not put a cosmetic low-pass filter on authoritative transforms and call the
problem solved. Distinguish the inference result, the consumer's chosen reference
frame/transition, and the controller's trajectory. Attach revision, effective time,
validity and relevant uncertainty to each. Never feed a smoothed result back as
independent evidence. A continuity transition is permitted only if its information
use remains authorized; a strict withdrawal may require immediate invalidation.

Begin with a one-dimensional discrete-time point mass approaching a target/obstacle,
known bounded acceleration, explicit controller and a fixed local coordinate frame.
Vary measurement frequency, correction frequency, latency, jerk/slew limits, clearance
and initial velocity. Compare identical disturbances with: direct correction; duplicate
suppression/coalescing; hysteresis; bounded reference transition when permitted;
controlled braking/replan when validity fails. Add SE(2), then SE(3), only after the
scalar mechanisms are understood. Include counterexamples where smoothing delays a
necessary correction and causes failure. A complete reference transition must keep
state, target, obstacle and velocity coordinates consistent.

Measure task error and min clearance, correction translation/rotation and total
variation, update frequency relative to new independent data, induced velocity,
acceleration and jerk, settled-time/overshoot, uncertainty coverage, false-valid time,
unnecessary interruptions, recovery time, compute latency/backlog and memory.
Compare conservative hold with continued motion only under the scenario's dynamics:
an abrupt stop is not universally safe either. Report a feasible envelope or failure,
never a universal physical-safety claim from this model.

## Interaction coverage and sequence

The matrix is a maintained coverage argument, not a claim of enumerating all futures.
Start with one-factor cases, then deterministic pairwise combinations across policy,
representation, topology, observation loss and correction rate. Mandatory compound
cases: R03+F03, R04+I03, R07+F05, R08+F04, R06+S04, S02+F05,
S03+F06, I07+S07 and F01+S05. Add randomized event traces after semantics are clear;
save seeds and shrink every failure to a small reproducer. Test disallowed combinations
as unsupported rather than hoping a solver makes them legal.

Suggested bounded cycles (adapt to evidence, not a deadline):

1. Exact arithmetic baselines: I01, I02, R03 small witnesses, independent answers.
2. S01 dynamic harness plus S03 policy split; establish controller/clearance metrics
   early because correction-induced failure is a primary concern.
3. R01/R02/R04/R08 entitlement transitions with numerical replay and explicit P3.
4. I03/I04/I05 shared lineage, repeated messages and observability.
5. F01/F02/F03 reproducible recovery and retained-state tradeoffs.
6. I06/I07 nonlinear, calibration, Monte Carlo and GTSAM comparison.
7. R05/R06/R07/R09/R10 selective rights and unavailable answers, including public outputs.
8. F04/F05/F06 asynchronous event faults and bounded processing.
9. S02/S04-S08, I08 and mandatory compound cases; expand physically realistic dynamics
   and numerical sweeps where simpler experiments exposed uncertainty.
10. Synthesize a minimal design and its supported envelope, explicit rejected cases,
    remaining policy decisions, and proposed changes to the owning contracts.

Every numbered item can take multiple runs; one run must not claim a whole family
tested from one toy case. The scheduler should always choose a manageable next slice.

## Completion criteria

Each row has a reproducer and evidence at its intended level, or an explicit justified
unsupported/policy-blocked result with the next decision identified. Mandatory compound
cases have been exercised. Recommend the simplest surviving approach using correctness,
authorization, stability, latency and memory evidence together. Publish limitations,
negative results and promotion proposals. Do not merge research into production or
close open safety/consent choices merely to complete the checklist.
