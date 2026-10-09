# Investigation plan: preserve future mathematical and contract options

Started 2026-10-07; scope redirected by Dana on 2026-10-08 after cycle 0011.
This is an investigation plan, not a promised implementation or a proof
that every combination of desired behavior is possible. Prefer the least complex
method that passes the required information, authorization and stability checks.

## Binding closing scope

Near-term Nestbox runs on one local network or Tailscale, with unavailable nodes
repaired before each session. This is an operating assumption for prioritization,
not a claim that crashes, inconsistent updates or future distributed use cannot occur.
The remaining question is what representation, retention, identity and contracts
must preserve so correct withdrawal, recovery and consistent results stay possible.
Estimator selection, controller tuning and broad reliability/performance validation
are deferred. Preserve the earlier evidence and its limitations.

Spend **at most four more cycles after 0011**, including synthesis and the requested
reviews: 0012–0015 below. This steering/documentation update is not an experiment
cycle. Combine or skip a remaining witness if existing evidence answers its contract
question; do not use saved time to add scenarios. After at most three witness cycles,
go directly to step 10. An inconclusive witness becomes an explicit synthesis limit,
not an excuse to extend the program. Publication retries do not authorize new research.

The previously queued 0012 (converged freeze plus a new batch) is **cancelled**.
0010–0011 already justify an approximation contract distinguishing retained likelihood,
linearization/chart, model revision and uncertainty meaning. More estimator coverage
would refine its numerical envelope without deciding another necessary data-model
choice. The cycle number 0012 is reused for the calibration/withdrawal contract witness.

## Questions the program must answer

1. What information must a frozen or exported representation retain so later results
   remain correct, uncertainty remains honest and revisions do not duplicate evidence?
2. Which permission changes allow reuse of retained derivatives, which require
   recomputation, and which make the requested inference impossible with lawful data?
3. Which input, model, calibration, dependency and policy identities must a result or
   checkpoint carry so recovery and composition remain well-defined?
4. Which epoch/reset and validity semantics let later consumers distinguish physical
   change from coordinate re-expression without hiding loss of authority or information?
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

This is the full inventory, not the remaining execution queue. STATUS gives the
existing evidence; the closing sequence below alone authorizes further experiments.
Only I07, I08, R05, S02 and F04 receive new witnesses. Every row must appear in the
synthesis, even when deferred. An executed row needs an oracle and explicit failure;
an unexecuted row needs an honest status and the contract choices that leave it open.
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

Historical methodology, preserved for later implementation work. Do not run further
controller/dynamics experiments in the closing program. S02 is limited to reset,
epoch and re-expression semantics, using a small exact coordinate witness.

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
The original compound list was R03+F03, R04+I03, R07+F05, R08+F04, R06+S04,
S02+F05, S03+F06, I07+S07 and F01+S05. It is no longer a mandatory execution or
completion gate. Record existing compound evidence and otherwise retain the relevant
dependencies in the synthesis. Do not reopen deferred rows via compound tests.

Original ten-step structure (historical steps 1–9 are not a new work queue):

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

### Closing cycle 0012 — I07 + R05: calibration and withdrawal granularity

Completed within the bounded contract scope; see [report](cycles/0012.md). The witness
description below records its intended scope, not a queue for further calibration work.

Use tiny exact linear models with one shared calibration quantity and, only as needed,
two time segments connected by an explicit drift constraint. Compare retaining the
joint calibration dependency or an adequate conditional/decomposed representation with
independently inflating observation noise or keeping only a collapsed posterior.
Change/replace calibration information without treating a model replacement as a new
independent observation. Exercise withdrawal by sensor, half-open acquisition-time
range and field, including a calibration estimate learned from subsequently withdrawn
records. Distinguish withdrawing those supporting observations from permission to keep
an independently authorized calibration derivative.

Reuse 0005/0009 evidence; add only missing witnesses. Compare authorized-only exact
recomputation against retained-state candidates, including covariance/cross terms.
Decide what must be identifiable: source record/revision, field/component, measurement
time convention, calibration identity/revision/validity interval, shared latent or
dependency identity, and support of derived calibration. Determine whether an explicit
retained variable is necessary in each tested case or whether a sufficient joint factor
or replay preserves the same option. Do not declare one storage representation universal.
For correlated fields, test or state why deleting one component requires the correct
remaining marginal/conditional likelihood rather than arbitrary matrix-entry deletion.
Separate metadata identifying a withdrawal from numerical state sufficient to perform it.

Deliver a short contract conclusion with supported granularity, lost-information
counterexample if present, and explicit coarse-invalidation/rebuild alternatives.
No camera calibration optimizer, drift estimator or nonlinear accuracy sweep.

### Remaining cycle 0013 — I08 + S02: snapshots and epoch meaning

Construct a small nested-domain example where individually valid edge revisions form
an inconsistent composite. Compare a dependency-bound revision manifest/consistent cut
with selecting the latest edge independently. Ask what an opaque result revision must
bind internally, without giving result readers permission to inspect private lineage.
Include input and calibration revisions, coordinate epochs, model/representation version,
query time/coverage and the applicable authorization decision. Do not prescribe a single
global counter or global transaction unless the witness actually requires it.

Use an exact coordinate example to distinguish a real body bump from a pure origin/SLAM
reset. A pure re-expression transforms all relevant state and covariance consistently;
physical motion is a different event. A SLAM reset with an unknown old-to-new relation
must remain unknown/invalid, not be labeled a harmless known transform. Record old/new
epoch identity, event kind and effective interval, relation direction and uncertainty
when available, and unavailable/ambiguous semantics when the cause is not established.
Do not infer cause from jump magnitude alone. No controller tuning or partition protocol.

### Remaining cycle 0014 — F04: atomic acceptance and recovery boundaries

Use a finite local state machine, explicit durable versus volatile state and crash
injection after each modeled write/publish transition for export replacement, checkpoint,
epoch close and grant update. Include idempotent retry and a restart acceptance oracle.
Ask which payload/dependency/policy/epoch fields must become accepted as one manifest,
what must be durable before an acknowledgment, and when an old result must be rejected
even if a replacement is unfinished. Do not count a grant revocation as applied if a
crash can silently restore its old accepted result. Distinguish data completeness,
authority freshness and publication; they need not be one cross-network transaction.

Name the assumed local storage primitive and ordering. Establish contract obligations
under that primitive, not filesystem durability or distributed consensus. Concurrent
publisher fencing and partitions remain F05, deferred. Reuse 0008/0009 identities and
retention assumptions where possible; do not build a production storage engine.

### Remaining cycle 0015 — step 10, synthesis and independent reviews

Write `SYNTHESIS.md` with:

1. The minimal contracts and representation choices supported by the evidence:
   what must be recorded, retained and identified; numerical versus identity-only
   state; granularity; revision/epoch binding; authorization and atomic acceptance.
   Preserve alternatives when evidence establishes an obligation but not one format.
2. One row for **every** scenario ID in this plan, classified `supported`, `unsupported`,
   `policy-blocked`, or `deferred-but-not-precluded`. Scope each label to a named model
   or contract capability; `supported` does not mean a production implementation exists.
   Link evidence and give a one-line reason, the preservation condition for deferred
   rows, and the owner's outstanding decision for policy-blocked rows. Unsupported
   combinations must identify what information/permission is absent, not imply that
   the entire future scenario is mathematically impossible.
3. Remaining policy/consent decisions for Dana, including derivative retention/use,
   contribution withdrawal scope, calibration descendants, time/field granularity,
   history/backfill and disclosure, acceptable reduced/unavailable results and any
   transition grace. Mark decisions not needed for the near-term profile separately.
4. Proposed changes to the owning `nestbox-ng` contracts and the shared-flow contracts,
   with target document/section, rationale, evidence and unresolved choices. Keep these
   **proposals only**, in this research directory; do not edit the owning contracts.
   Preserve generic provider mechanisms and application-owned mathematical semantics.

Commit and push the synthesis and verify its remote commit **before launching reviews**.
Then launch two separate expert subagents, each reading that same pushed snapshot:

- Network software system architecture: identity, dependency snapshots, durable/recovery
  boundaries, authority, interface ownership and whether deferral preserves future options.
- Uncertainty propagation for 3D alignment with quantities changing over time:
  sufficiency, correlations/shared calibration, marginalization/withdrawal, gauge/epochs,
  approximation semantics and whether the evidence supports each mathematical claim.

Use independent prompts with the same user scope, fresh context where practical, and
no access to the other's report until both initial reviews are complete. They are
independent interpretations, not a vote or a claim of external human certification.
Pin reviewed commits and source references. Each agent writes only its own report under
`reviews/`; the parent owns Git operations. Request concrete issues, assumptions, policy
choices, nonissues/refinements and verdict, distinguishing contradictions from T01/T02.
Preserve both reports. Record parent responses separately, correcting clear immediate
errors if warranted, but do not turn recommendations into new investigation cycles.
Push both reports and any response/corrections, verify the remote, then pause automation
`information-lifecycle-investigation` and send Dana a short conclusion. Do not pause at
the first synthesis push while the requested reviews are still outstanding.

## Explicit deferrals: must not be precluded

No new cycles on R07 (offline/expiry/key rotation), F05 (partitions/fencing), F06
(correction storms/backlog), S08 (repeated loss/regrant), S04–S07 (physical consumer
scenarios), or further nonlinear/SE(3) accuracy/coverage beyond the existing approximation
contract evidence. Include each as `deferred-but-not-precluded` in the synthesis with
the interfaces/state it will require, and identify any missing preservation condition.
Do not claim it is preserved merely because the design has an unspecified extension point.
Other unselected rows, including R09/R10, receive an evidence/contract audit for synthesis,
not dedicated experiments. No further S01/S03 controller work. Unresolved policy or
unsupported combinations within deferred scenarios stay explicit alongside the deferral.

## Completion criteria

The closing cycle limit is respected; the five selected scenarios have small witnesses
or an explicit evidence limit; every scenario is classified with reasons and retained
future options; `SYNTHESIS.md` and both independent expert reports are committed and
verified on the remote; clear immediate corrections have an explicit disposition; the
automation is confirmed paused and Dana receives the short conclusion. Full matrix
testing, mandatory compounds, production performance and hardware safety are not
completion gates. Do not close policy choices merely to complete the checklist.

If a **new architectural contradiction** emerges (incompatible requirements with a
concrete witness, not a refinement of T01/T02), record it in TENSIONS and tell Dana
immediately in this chat. Do not wait until cycle publication or silently choose which
requirement to weaken. Continue only independent work while any needed priority decision
is pending; this does not authorize more cycles.
