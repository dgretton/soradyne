# Information lifecycle synthesis

2026-10-09. Final closing cycle 0015; proposals only, not revised owning contracts.
Evidence through research commit `99aad608ed1b1ca2cca1c468b55b1bc42579afca`, based on
shared-flow design `9cbba490362647bbc9a7216735e51f1b9477d32b`. Application documents
reviewed at nestbox-ng `ed141df17477c40a4b9365854aa385f82e1d9280`.

## Conclusion

**The direction is implementable, provided “freeze” means a declared representation
with retained recovery options, not unconditional disposal of everything except a
pose and covariance.** Nothing in the investigated scenarios requires keeping every
calibration variable active, a universal global revision counter, or an atomic
transaction across the network. The evidence does require preserving numerical
dependencies, distinguishable identities, permission semantics and explicit invalid
states before committing to irreversible reductions.

The least complex starting design is an authorized evidence/revision ledger plus
replaceable summaries and rebuild from retained sufficient state. Active computation
can be small while authorized archives remain richer. If required lawful information
is gone, report reduced coverage or unavailability according to the query contract.
Incremental subtraction, general feedback, estimator choice and controller tuning can
wait. This is a recommendation about preserving options, not a measured performance
claim or a completed implementation plan for either project.

The main irreversible mistakes to avoid are:

- Treating current marginal mean/covariance and lineage IDs as enough for every future
  withdrawal, calibration revision or nonlinear relinearization.
- Treating shared calibration, overlapping observations, returned posteriors or replayed
  outputs as new independent evidence.
- Conflating physical identity, coordinate realization, physical-state epoch, solve
  revision and grant generation; calling a compatible old answer the requested current one.
- Treating byte retention, original publication permission or network reachability as
  present authority; coupling invalidation durability to completion of its replacement.
- Making smoothness part of the truth/authorization claim. A consumer transition is a
  separate, explicitly permitted operation with its own physical assumptions.

No new architectural contradiction beyond [T01/T02](TENSIONS.md) was established.
T01 remains the choice between recoverable information, permitted retention and loss
of capability. T02 remains the conflict between mandatory invalidation and physical
continuation without adequate independent state/margin. Neither is resolved by naming
more metadata. The original 0014 live-serving defect is preserved and corrected, not
counted as evidence that its first candidate met the full contract.

## Minimal contracts and representation choices

The eight obligations below are semantic, not a wire schema. Related fields can be
packed into opaque immutable references. Their authorized issuer must be able to
resolve the binding; every result reader need not see private evidence or provenance.
An ID certifies neither numerical sufficiency nor truthful producer assembly.

### C1. Evidence identity and selectable support

Identify an observation independently of delivery, destination, grant and summary
revision. Bind origin/sensor, record identity and revision, field/component selection,
acquisition-time convention and uncertainty where relevant, model/calibration revision,
and supersession/withdrawal relationships. Delivery twice or two permission paths does
not produce two observations. Keep acquisition time, delivery/receipt time, solve input
selection and publication time distinguishable.

**Granularity must match the promised withdrawal operation.** Whole-source retention
can support whole-source withdrawal but need not support removal of one record or field.
For sensor/time/field withdrawal, retain an index from that selector to all affected
factors/derivatives, including support used to learn a shared calibration. Stable
field identity is insufficient without the numerical information described by C2/C3.
Coarse invalidation is an honest alternative if its lost coverage is explicit. A raw-data
hash without accessible values/model is identification, not a recovery method.

Evidence: [0004](cycles/0004.md), [0009](cycles/0009.md), [0012](cycles/0012.md).

### C2. Retained numerical state and declared capabilities

For each frozen/exported/checkpoint representation, declare the queries and future
operations it supports under a named model, and what reconstructible state it retains.
Distinguish three things: numerical bytes, identity/dependency metadata, and current
permission to use either. None substitutes for another.

| Future operation | Numerical information that can preserve it | What cannot be assumed sufficient |
|---|---|---|
| Current linear retained-variable query | Correct joint reduction over the retained variables, with applicable cross terms | Independent marginals when the query depends on correlation |
| Selective contribution withdrawal | Adequately decomposed pre-elimination factors/statistics or lawful original-factor replay | A mixed marginal plus the list of contributing IDs |
| Shared-calibration/drift/model revision | Retained conditional/joint model and observations/statistics sufficient for that revision, or lawful rebuild | Today's target marginal and a changed calibration label |
| Nonlinear relinearization/new operating point | Original nonlinear factors/data and model, or a representation proved sufficient for the supported family | A first-order quadratic with only its anchor renamed |
| Restart plus replay | Declared sufficient checkpoint state, actual input coverage, model/dependency/epoch bindings and duplicate accounting | A published output seeded as independent evidence before replaying its inputs |

These are alternatives, not demands to retain raw sensor images or all live variables.
Retaining original factor observations with their calibration/model recipe may suffice;
reprocessing images is a separate capability. A model-specific sufficient statistic may
also suffice, but the retained rights must cover its information content. A label such
as “derived” does not make a raw-equivalent statistic less sensitive.

Before irreversible eviction, state what operations become unsupported and when replay
ceases to be available. If no retained representation supports a requested operation,
do not approximate away the withdrawal: explicitly invalidate/rebuild, or return a
permitted reduced result whose coverage is distinguishable from the full answer.

Evidence: [0001](cycles/0001.md), [0005](cycles/0005.md), [0008](cycles/0008.md),
[0009](cycles/0009.md), [0010](cycles/0010.md), [0012](cycles/0012.md).

### C3. Correlation, shared calibration and original-information accounting

Preserve shared latent/calibration identity and the numerical dependence it induces.
An explicit active calibration variable is one representation. A sufficient joint
factor or correlated observation model can be equivalent for a specified query.
Cycle 0012 integrates a two-time shared offset into a joint covariance and matches
the latent-variable answer exactly in 32 cases; independent per-observation inflation
does not. This is not a universal compression result for nonlinear camera calibration.

Distinguish calibration value/revision, drift/noise model, validity interval, and the
evidence supporting a learned calibration. Replacing calibration information replaces
the affected model contribution; it is not another independent prior. Withdrawal may
reach a target through calibration support even when its own target observations remain
permitted. For correlated fields, remaining-field likelihoods need correct marginal
noise/dependence, not arbitrary deletion of precision entries.

Dependency DAGs alone do not prevent original evidence from being counted twice. Retain
adequate overlap contributions or reject unsupported composition; known cross covariance
can support an honest reduced-information answer without recreating discarded unique-data
information. Preserve the existing no-feedback publication restriction as the simple
default. The exact scalar matched-base residual in 0006 is an alternative research
witness, not permission to introduce general posterior feedback into Nestbox.

Evidence: [0005](cycles/0005.md), [0006](cycles/0006.md), [0012](cycles/0012.md).

### C4. Uncertainty, gauge and approximation meaning

Bind uncertainty to the same variables, ordering, coordinate/tangent convention,
dependencies and revision as the numerical answer. Distinguish joint covariance from
independent marginals, local Gaussian approximation from a fuller likelihood, and
posterior mass from repeated-sample coverage. Name the linearization/chart and model
assumptions; unsupported, broad or ambiguous uncertainty must remain expressible.

A coordinate gauge choice supplies no physical anchoring evidence. Report observability
per query/direction, so lawful relative information can remain useful when an absolute
anchor is lost. Rank/singularity diagnostics must not be erased by a tight numerical
gauge prior. Production rank tolerances and uncertainty envelopes are still to be chosen.

The nonlinear evidence is deliberately narrow: 0010–0011 use known planar landmarks
and independent isotropic noise. A compact nonlinear statistic preserves that likelihood;
full circular angular intervals have model-specific calibration evidence. Local Gaussian
intervals under-cover in weak cases, and a frozen quadratic can fail away from its anchor.
This does not establish general SE(3), joint pose/point coverage, uncertain-landmark
marginalization, robust-kernel correctness or consumer precision. Retain an authorized
rebuild path and an explicit approximation contract to leave those later choices open.

Evidence: [0007](cycles/0007.md), [0010](cycles/0010.md), [0011](cycles/0011.md).

### C5. Compatible result revisions and distinguishable epochs

A result revision binds the query (scope, direction, time, required coverage), numerical
answer and uncertainty, contributing revisions, dependency compatibility, calibration/model
and representation, relevant epochs/validity, and acceptance decision. The binding may
be private behind a public opaque revision. Equal numerical values do not make two
different manifests interchangeable. Current permission is checked separately under C6.

Nested composition must select a compatible dependency set and covariance for those
same revisions. “Latest from every edge,” matching endpoint epochs, or equal local
revision counters does not establish this. Compatible snapshots may reuse unchanged
independent edges; the evidence does not require one global counter or simultaneous
computation. Historical/as-of, explicit-revision, current and reduced-coverage requests
need distinguishable semantics. An unavailable requested revision is not silently
replaced by an older coherent answer.

Keep physical/frame identity, coordinate realization, physical-state epoch and inference
revision distinguishable. Epoch events need affected scope, old/new identities, effective
half-open boundary, event kind and knowledge/ambiguity. A known re-expression supplies a
directed relation with uncertainty and shared dependence; all relevant state, targets,
obstacles and vectors transform consistently. A real bump changes physical state. An
unknown SLAM relation is unknown, not an identity transform. Jump magnitude alone does
not identify the cause. New within-epoch relative queries may remain supported without
fabricating a cross-epoch bridge.

Evidence: [0007](cycles/0007.md), [0013](cycles/0013.md).

### C6. Scoped rights, derivative policy and current-use validity

Separate source ownership, administration, author identity, retention/custody, decryption,
computation, publication, output reading and provenance reading. Bind each decision to
resource/revision or range, audience/purpose, operation and applicable grant/policy
generation. Union independent lawful retention paths without borrowing another audience's
inference rights or counting the same evidence twice. A grant change does not create a
new observation; re-grant does not silently restore historical scope.

Future-only cutoff (P1), specifically permitted derivative reuse after raw deletion (P2),
and contribution withdrawal from future inference (P3) are different policies. P3 needs
affected derivative support identified and invalidated or rebuilt from lawful state;
it must not be weakened to “stop future delivery.” P2 rights must be explicit, including
learned calibrations and derived checkpoints. Previously delivered plaintext is not
promised remotely erasable. Opaque output IDs do not prove absence of inferential leakage.

Record policy issuer/revision and distinguish effective, learned and enforced events.
Compute-time, acceptance-time and current-use authority can differ. Reachability neither
grants authority nor conclusively revokes it. The future offline/freshness policy is an
open decision; the local-network profile does not turn a stored grant into timeless consent.

Evidence: [0004](cycles/0004.md), [0009](cycles/0009.md), [0012](cycles/0012.md),
[0013](cycles/0013.md), [0014](cycles/0014.md).

### C7. Recoverable publication and durable invalidation

Name actual consumed input coverage, including lossy live inputs; a durable log horizon
alone does not identify it. A sufficient checkpoint binds numerical state, represented
inputs, dependencies/model/epochs and replay rules. Retention acknowledgments identify
the promised custodians/coverage; missing lawful state leads to explicit unavailability.

Specify an atomic local acceptance boundary and durable prerequisites. A simple candidate
persists immutable payloads/dependencies/manifest, then atomically records selected revision,
acceptance gate and retry receipt. Other layouts are possible if they guarantee the same
recoverable result. Control application and publication acknowledgments are different.
Persist invalidation independently of unfinished replacement work; restart cannot resurrect
an affected old answer after the control was durably applied. Keep control/tombstone state
recoverable while dependent surviving artifacts could otherwise become admissible again.
Operation IDs bind content; define the deduplication/receipt retention window and reject
conflicting reuse. A delayed retry confirms a historical commit, not renewed current use.

0014 tests 46 crash cuts under an **assumed** atomic record primitive and one serialized
writer. A pending restrictive control blocks live serving. If a crash loses that event
before durable commit, its local rule requires caller retry and permits the prior state
on restart. An earlier external effective-time promise requires stronger durable knowledge
or fail-closed authority refresh before serving. Neither global freshness nor filesystem
durability follows from these tests. Cross-domain fencing and partitions stay deferred.

Evidence: [0008](cycles/0008.md), [0014](cycles/0014.md), including its preserved correction.

### C8. Consumer truth and transition are separate

Publish validity, coverage, epoch/event and uncertainty changes even if the mean does
not move. Duplicate/no-information revisions must not invent precision or physical
motion. A consumer may choose a permitted transition; that transition is not the new
inference result and must not be fed back as independent evidence. Immediate necessary
invalidation/correction cannot be hidden by smoothing or delayed coalescing.

The scalar stopping witnesses show why a later controller needs independent lawful
local state and a pre-event operating margin. They do not select a safe robot/drone
fallback. Keep this consumer interface now; choose controllers and physical envelopes
during implementation. Evidence: [0002](cycles/0002.md), [0003](cycles/0003.md),
[0006](cycles/0006.md), [0013](cycles/0013.md).

## Scenario dispositions — all 32 rows

`supported` means the stated bounded capability has evidence, **not** production support
for every case named by the row. `unsupported` identifies a specific impossible/unmet
combination under stated information or physical assumptions. `policy-blocked` needs
Dana's stated consent/product decision. `deferred-but-not-precluded` means untested scope
is preserved **only if the stated numerical/contract condition is adopted**; an extension
point or an ID alone is not evidence of support. C1–C8 are proposals, not installed guarantees.

| ID | Disposition | Scoped evidence, remaining decision or preservation condition |
|---|---|---|
| I01 | supported | Exact linear joint reduction preserves retained-variable answers; keep cross terms and C2's operation limits ([0001](cycles/0001.md)). |
| I02 | supported | Bounded replacement/tombstone and local crash/retry witnesses avoid duplicate information; preserve immutable revisions, receipts and current-use checks; distributed publication remains open ([0001](cycles/0001.md), [0014](cycles/0014.md)). |
| I03 | supported | Shared latent/decomposed information handles the tested diamond; arbitrary overlap from aggregate lineage alone is unsupported; retain overlap statistics/replay or reject/reduce (C2/C3; [0005](cycles/0005.md)). |
| I04 | supported | Restrict feedback or account for original information in the exact matched-base scalar fixture; keep current no-feedback rule unless a later replacement contract proves more (C3; [0006](cycles/0006.md)). |
| I05 | supported | Exact scalar observability/gauge witnesses retain lawful relative queries; preserve null-space/query semantics for later floating-point and 3D work (C4; [0007](cycles/0007.md)). |
| I06 | deferred-but-not-precluded | Broader nonlinear/SE(3) work is deferred; retain lawful nonlinear factors/model or proved sufficient state plus chart/approximation/uncertainty meaning. Planar limits already justify C2/C4 ([0010](cycles/0010.md), [0011](cycles/0011.md)). |
| I07 | supported | Two-time linear calibration/drift and replacement witnesses show active variables are optional but retained joint dependence/revision response is necessary; general nonlinear calibration needs lawful rebuild (C2/C3; [0012](cycles/0012.md)). |
| I08 | supported | Eight bounded nested revision/covariance combinations and arrival prefixes enforce compatibility; preserve full private binding and explicit requested-revision semantics; distributed construction untested (C5; [0013](cycles/0013.md)). |
| R01 | supported | Source-sequence P1 cutoff with permitted old derivatives works in the scalar ledger; time-based late/backdated cutoff needs a separate policy (C1/C6; [0004](cycles/0004.md)). |
| R02 | policy-blocked | P2 and P3 both have witnesses; Dana must decide which derivative retention/use rights survive raw-access/deletion changes, including calibration and checkpoints (D1/D2; [0004](cycles/0004.md), [0012](cycles/0012.md)). |
| R03 | supported | Lawful decomposition/rebuild, or explicit inability after lost information, is supported; arbitrary exact withdrawal from an insufficient mixed marginal is unsupported (C2; [0001](cycles/0001.md), [0009](cycles/0009.md)). |
| R04 | supported | Independent entitlement paths preserve other lawful uses without duplicate evidence in the scoped ledger; keep audience/purpose and artifact rights separate (C6; [0004](cycles/0004.md)). |
| R05 | policy-blocked | Exact sensor/time/field and learned-calibration witnesses exist; choose selector/time semantics, promised granularity and coarse-invalidation fallback before irreversible reduction (D2/D3, C1–C3; [0012](cycles/0012.md)). |
| R06 | supported | Anchor/relative-constraint withdrawal supports query-specific scalar observables; keep gauge distinct from evidence and retain sufficient relative constraints; full scale/connectivity geometry untested (C4; [0007](cycles/0007.md)). |
| R07 | deferred-but-not-precluded | Preserve issuer/grant generations, effective/learned/enforced times, remaining entitlements and fresh-authority/unknown states; later choose expiry, key rotation and offline bounds. No claim of erasing prior plaintext (C6/C7; D6). |
| R08 | supported | One re-grant/replay gap with new scope/generation is supported; preserve observation identity and explicit historical ranges; repeated cycling is S08, deferred (C1/C6; [0004](cycles/0004.md)). |
| R09 | deferred-but-not-precluded | No role-permutation witness; preserve distinct owner/author/admin/compute/publish/custody/read principals and operation-scoped grants so later placement cannot silently widen rights (C6; shared-flow §2; D5). |
| R10 | policy-blocked | Opaque private/public revision binding is demonstrated, but audience resolution, history/backfill and provenance-disclosure consent need D4; retain private manifests and separately authorized output views (C5/C6; [0013](cycles/0013.md)). |
| F01 | supported | Exact scalar recovery uses actual input manifests/checkpoints; equal log horizons alone fail for unequal live delivery; retain checkpoint coverage or declare gaps (C7; [0008](cycles/0008.md)). |
| F02 | supported | Surviving authorized custodian replay supports bounded producer-loss recovery; bind acknowledgments to retained coverage and rights; no universal latency/replication guarantee (C6/C7; [0008](cycles/0008.md)). |
| F03 | supported | Static independent-scalar expiry plus withdrawal is supported by sufficient decomposition/replay or explicit reduced/unavailable output; moving-state lag elimination remains untested and needs C2's reconstructibility contract ([0009](cycles/0009.md)). |
| F04 | supported | 46 local crash cuts/retries under the declared atomic primitive; preserve independent durable invalidation, receipt/content binding and the corrected pending-control barrier; external-time freshness remains open (C7; [0014](cycles/0014.md)). |
| F05 | deferred-but-not-precluded | Preserve authority generation/acceptance context on both publication and direct replies, immutable revisions and unavailable/provisional outcomes; choose and test fencing/coordination assumptions later (C5–C7; D6; shared-flow §5). |
| F06 | deferred-but-not-precluded | Preserve acquisition/deadline metadata, separate durable evidence from replaceable delivery notifications, and distinguish invalidation from optional updates; later bound queues/priorities/coalescing without delaying necessary responses (C1/C7/C8). |
| S01 | supported | Scalar witnesses reject unconditional smoothing/coalescing and expose false precision from duplicate influence; preserve immediate validity and separate transitions, with tuning/3D behavior deferred (C8; [0002](cycles/0002.md), [0006](cycles/0006.md)). |
| S02 | supported | Exact known re-expression, same-jump/different-cause and shared bridge uncertainty witnesses establish event/epoch semantics; retain relation/covariance or explicit unknown; detection/physical response untested (C5; [0013](cycles/0013.md)). |
| S03 | unsupported | Unconditional safe continuation after strict withdrawal is unsupported: old-state reuse may be forbidden and stopping space insufficient; a limited gated-stop witness exists, but continuation needs D7 and a later validated envelope (C8; [0002](cycles/0002.md), [0003](cycles/0003.md)). |
| S04 | deferred-but-not-precluded | Preserve query-specific validity, lawful local tracking, epoch/reacquisition identity and uncertainty; later docking must establish stopping/acceptance margins, not infer them from a smooth transform (C4/C5/C8). |
| S05 | deferred-but-not-precluded | Preserve acquisition time, clock identity/uncertainty, receipt and publication times, model interval and actual consumed revisions; keep a lawful replay/model path for later time reassociation (C1/C2/C5). |
| S06 | deferred-but-not-precluded | Preserve full SE(3) conventions, task-point geometry, joint cross covariance, compatible snapshots and directed reset relations; later validate large-rotation/lever-arm consumer behavior (C3–C5/C8). |
| S07 | deferred-but-not-precluded | Preserve original nonlinear likelihood/model or sufficient authorized replay, robust-kernel/model revision and uncertainty/ambiguity tags; do not force all future answers into one Gaussian pose (C2/C4/C8). |
| S08 | deferred-but-not-precluded | Preserve stable evidence IDs across grants, bounded retry/replay contracts, explicit scope/generation changes and invalidity; later prove progress/backlog bounds without changing permission semantics (C1/C6–C8). |

Compound scenarios do not inherit a proof merely because their component rows have
bounded support. R03+F03 has the static 0009 witness; the other former mandatory
compounds receive no new tests. Their combined dependency, authority, retention and
consumer assumptions must all hold. No deferred scenario was reopened to complete
this table, and the cancelled converged-freeze/new-batch 0012 was not run.

## Decisions for Dana

These choices remain open; the investigation has not granted consent on anyone's behalf.
The representation should identify the choice even when an initial local profile uses
one fixed policy.

| Decision | Needed before which commitment? | Choice still required |
|---|---|---|
| D1 Derivative rights | Before promising sharing/revocation behavior | Which artifacts may remain stored, used and disclosed after source access ends? Distinguish P1, P2 and P3; avoid a universal “derived results always survive” promise. |
| D2 Withdrawal descendants | Before learned calibration or mixed summaries become authoritative | Does withdrawal remove contribution through calibration/training descendants, and are any independently authorized derivatives exempt? Specify audience/purpose and future-use scope. |
| D3 Granularity and retention | Before irreversible summarization/expiry | Promise sensor, record, time-range and/or field withdrawal? Define acquisition-time boundaries and uncertain clocks; choose retention/rebuild commitments versus explicit coarser loss. |
| D4 Views and history | Before cross-audience result exposure | Which resolutions, historical ranges/backfill, private lineage and transformed derivatives are permitted? Output-read permission must not imply provenance-read permission. |
| D5 Reduced/current/historical service | Before declaring query/role acceptance semantics | May a caller request reduced coverage or an older compatible answer? Which observables/uncertainty are adequate? Who may compute/publish each view? Do not silently substitute these for current/full requests. |
| D6 Control effect and freshness | Local acknowledgment semantics before persistence; offline/fencing mechanisms may wait | Is effect defined at durable local application, a source frontier, or an external effective time? What restart refresh/unknown behavior is required? Later choose expiry/grace, key rotation and exclusive/provisional partition behavior. |
| D7 Physical transition permission | Before physical consumers; tuning can wait | May any previously derived state be used during a bounded transition after withdrawal, or is P3 immediate? Independently establish lawful local fallback state and operating margin; no universal halt/continuation safety promise. |

Near-term network simplicity justifies deferring distributed mechanisms and physical
tuning. It does not justify deleting information needed for a later promised operation,
equating a physical bump with a coordinate reset, or leaving derivative rights implicit.
Ordinary implementation choices such as storage engine, cache sizes and solver scheduling
can be made later within these declared obligations.

## Proposed changes to the owning contracts

**Proposals only.** No edits to these owning documents or production schemas are made
by this investigation. Names/sections below refer to the pinned revisions above.
The provider keeps opaque records, generic authority/custody and publication mechanisms;
the application owns numerical sufficiency, calibration, observability and geometry.
The adapter owns translation/conformance under the existing boundary discipline.

| Target | Proposed contract change and rationale | Evidence / unresolved choice |
|---|---|---|
| nestbox-ng `docs/GLOSSARY.md`, “Calibration,” “Operation,” “Provenance”; work-breakdown E2.1/E3.5 | Replace an unconditional “only body poses remain live”/“survey covariance retained” sufficiency implication with a representation capability contract; name retained nonlinear/model state, supported revisions/withdrawals and rebuild path. Calibration may be marginalized without erasing shared dependence. | C1–C4; 0001/0010/0012; D1–D3. |
| `docs/CONVENTIONS.md` §§3,5,7; work-breakdown E3.3–E3.5 | Qualify covariance meaning and approximation/chart; expose rank/query validity without counting gauge pinning as physical precision. Preserve centralized tangent conversion and cross-covariance composition. Do not use a residual chi-square check alone as coverage validation of estimated pose. | C4; 0007/0010/0011. Exact gauge and approximation thresholds remain implementation work. |
| `docs/GLOSSARY.md` “Solver domain,” invariant 8; `CONVENTIONS.md` §6; E3.10 | Keep no-feedback/raw-factor publication restrictions. Add export replacement identity, actual support/dependency/correlation contract and safe rejection when composition/overlap is unsupported; child edges are not automatically independent `Between` observations. | C1/C3/C5; 0005/0006/0013. General residual messaging is not proposed for initial implementation. |
| `docs/CONVENTIONS.md` §4; `GLOSSARY.md` “Epoch”; E2.3/E3.7 | Distinguish coordinate realization, physical-state epoch and inference revision; event kind/knowledge and old-to-new relation are explicit. Retain half-open time and directed uncertainty-aware re-expression, with unknown bridges unavailable. | C5; 0013. Event detection remains separate. |
| `docs/GLOSSARY.md` `lookup()` and invariant 3; E7.1; `SUBSTRATE.md` §3.4 | Bind query answers to compatible contributing revisions, joint uncertainty, coverage, validity and acceptance context. Make private chain details resolvable by authorized roles while allowing an opaque public view. Add explicit historical/reduced/current request semantics. | C4–C6; 0013; D4/D5. A public opaque revision is not a privacy proof. |
| `docs/SUBSTRATE.md` §3.1 L3/L5, §3.2 F1–F3, §3.3, §5, §7 E6 | Distinguish transport replay from numerical inclusion; name actual live-input coverage and sufficient checkpoints. State acknowledged custody without assuming producer-local disk. Reconcile hard feed-window/epoch expiry with any promised later withdrawal/reconstruction capability. Same log horizon alone cannot promise equal recovered answers. | C1/C2/C7; 0008/0009/0014; D3. A lossy short-lived feed can remain an intentional profile when missing-state behavior is explicit. |
| `docs/SUBSTRATE.md` §§6.1–6.3 and §10.2 P4 | Separate rights/entitlement paths and affected support scope; specify policy/grant generation and derivative obligations. Refine the current “already published edges remain” clause into retained bytes versus permitted future use. Raw replicas alone do not describe all retained information. | C1/C3/C6; 0004/0012; D1–D4. This refines the documented P1/P2/P3 ambiguity, not automatic adoption of strict P3 everywhere. |
| `docs/SUBSTRATE.md` §§3.3,5.3,6.3,9.7,10,12; E3.1/E3.7 | Add generic durable acceptance/control acknowledgments, identity/content-bound retry and explicit startup/current-authority checks. A locally applied invalidation survives unfinished replacement. Current serving cannot be justified solely by “last solve plus age.” Add conformance cases for these boundaries. | C6/C7; 0014; D6. Storage primitive and external-time semantics need implementation decisions; no distributed transaction prescribed. |
| E7 clients and E9–E12 consumer acceptance; `SUBSTRATE.md` §3.5 | Expose immediate validity/epoch/coverage changes separately from a consumer's optional transition; replace any unconditional safety implication of “halt” with a separately validated consumer operating contract. Keep current typed errors while defining application query validity precisely. | C8; 0002/0003/0013; D7. No controller implementation is selected here. |
| soradyne `docs/20260911_shared_flow_demo_contracts.md` §§1–2 | Require opaque stable evidence/revision/dependency preservation, explicit replacement semantics, and distinguish build/acceptance/current-use authority. Preserve independently owned sources and separately granted compute/publish/read/retain roles. | C1/C5/C6; generic witnesses 0004/0013. Application support/dependence remains opaque. |
| shared-flow §3 | Extend custody acknowledgments to named coverage/custodians and retention obligations for derivatives, checkpoints and invalidation state. State remaining-entitlement behavior and capability loss after eviction. Fast materialization and lawful archive retention remain separate. | C2/C6/C7; 0008/0009/0014; D1/D3. No unconditional retention or source-local-disk requirement. |
| shared-flow §5 | Define generic immutable acceptance manifests/receipts, durable prerequisites, separate invalidation/replacement, and explicit current/historical result selection. Preserve fencing/authority context for both writer and direct-reader paths without claiming its mechanism implemented. | C5–C7; 0013/0014; D6. Compatible nested math is checked by the application. |
| shared-flow §6 F2/F4/F6 and adapter conformance | Add future generic fixtures for durable acknowledged recovery, scope-specific derivative withdrawal and opaque output/private provenance; retain F5/F7 for deferred fencing/backpressure implementation. Do not equate these research tests with passing the proposed demos. | Evidence mapping above; no new experiments or protocol edits in this closing cycle. |

Existing stricter application privacy defaults (for example personal-frame topology)
can remain deliberate profiles of a more general provider. The research does not
require broadening grants or removing those defaults. It requires that the advertised
future capabilities and consent semantics agree with the chosen retention/representation.

## Evidence level and closeout

The investigation contains exact rational witnesses, bounded event traces and one
restricted planar numerical/statistical study. They support the scoped claims above;
they do not compose into a proof of full-system stability, privacy, distributed recovery
or hardware safety. Production code and owning contracts are unchanged. Existing tests
number 259; all fifteen saved source manifests across fourteen cycles are preserved.

The synthesis is to be committed and verified remotely before two independent specialist
reviews of that same snapshot. Their original reports and the parent's response will
remain separate under `reviews/`; [cycle 0015](cycles/0015.md) records publication pins,
validation and completion. After reports and immediate corrections are pushed and
verified, pause the automation. No further research cycle is authorized by an unresolved
row, review recommendation or the former full-matrix plan.
