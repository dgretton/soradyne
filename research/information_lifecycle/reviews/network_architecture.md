# Independent review: network software architecture

2026-10-09. Reviewer: independent network-architecture review agent. This report
assesses the closing investigation, not a production implementation. I did not read
the other specialist's report or communicate with that reviewer.

## Revisions, evidence and checks

- Reviewed synthesis/research snapshot: soradyne
  `d51cb5024102098d140effaabfacb82423d1c7d3`, branch
  `research/information-lifecycle`. The parent supplied this as the verified pushed
  tip; I resolved the commit locally but did not independently contact the remote.
- Shared-flow design base: `9cbba490362647bbc9a7216735e51f1b9477d32b`.
  A read-only Git diff confirmed that `docs/20260911_shared_flow_demo_contracts.md`
  is unchanged between that base and the reviewed snapshot.
- Application reference: nestbox-ng
  `ed141df17477c40a4b9365854aa385f82e1d9280`. Application documents were read with
  `git show` at that pin, independently of the current checkout.
- Instructions: workspace `/Users/rim/Dev/BLANKET.md` and pinned soradyne
  `CLAUDE.md`. There is no root `CLAUDE.md` at the application pin. The authorized
  research exception permits this cross-project review; it does not change the
  production boundary.

Below, **S:** denotes a path in the pinned soradyne tree; **N:** denotes a path in
the pinned application tree. Line numbers refer to those pins. Research paths
without a repository prefix are relative to `research/information_lifecycle/` in S.

I read SYNTHESIS, PLAN, METHODS and TENSIONS; the shared-flow contract; the application
SUBSTRATE and GLOSSARY; relevant CONVENTIONS and work-breakdown sections; the reports
for 0004, 0008, 0013 and 0014; and the relevant entitlement, recovery, snapshot,
replacement and crash-model source/test sections. I checked the synthesis's other
scenario dispositions against its stated preservation conditions, not by independently
revalidating every numerical study.

I ran the **90 existing tests** in `test_baseline`, `test_entitlements`,
`test_recovery`, `test_snapshot_epochs`, `test_crash_boundaries` and
`test_crash_serving_barrier`: **90 passed, zero failures/errors**. The run used
`/Users/rim/Dev/.venv-nestbox/bin/python -B`, loading research Python modules directly
from Git blobs at the reviewed commit through an in-memory import loader. It wrote
no test artifacts or bytecode. I did not run the full 259-test suite, reproduce saved
JSON artifacts, test a filesystem durability primitive, run network processes, or
add experiments. The stale-publication trace below is source inspection, not an
additional executed test.

## Verdict

**The proposed direction is implementable and adequate to close this research phase,
with two preservation conditions worth making more explicit and one important
implementation handoff.** I found no new incompatible pair of architectural
requirements requiring a priority change. The findings below refine result validity,
T01's retained-information obligation and the provider/application boundary. They
do not justify reopening deferred research.

The eight contracts are semantic obligations with workable local realizations.
Their feasibility does not establish all combinations of consent, availability and
recovery: an implementation must choose the profile and may have to return unavailable.
The introduction's implementability claim is defensible with that reading; it would
be too strong if read as a completed distributed design or guaranteed recovery for
every policy in the matrix.

## Prioritized findings

### NA1 — P2: Preserve rejection of stale publication independently of retry receipts

**Action:** Add an explicit acceptance obligation: a first delivery or late retry of
a superseded publication cannot move the selected current result backward, even after
its retry receipt has expired. Name the scope of this ordering/selection rule and
retain the necessary predecessor, supersession or acceptance-frontier state. An
expected-head condition, an authority-scoped publication sequence, or a type-defined
compatible-head rule are possible implementations; a global counter is unnecessary.
An explicit historical selection remains a separate operation.

C7 requires content-bound operation IDs and a receipt-retention window, and correctly
says that a delayed retry confirms a historical commit. But idempotency and selection
ordering are different obligations. In `crash_boundaries.py:200–220`, an existing
receipt prevents republishing an old commit; if there is no receipt, validation checks
completeness, model, interval and authority, then unconditionally replaces the head.
No prior-head or supersession check is present. By static inspection, a completed
publication B for `{a,b,c}` followed by the first completion of an earlier prepared A
for `{a,b}` can select A when both satisfy the same gate. The existing test at
`test_crash_boundaries.py:112–123` covers a delayed **already committed** A, whose
receipt survives; it does not cover that first completion or receipt collection.

The earlier `baseline.py:81–103` has the missing semantic ingredient: stable-key
revision comparison ignores older deliveries and preserves tombstones. Preserve that
obligation explicitly when combining the replacement and persistence designs; do not
mistake immutable manifests or durable receipts for it. The shared-flow contract's
partial-order/merge rule also means a numeric total order must not be imposed across
unrelated histories.

**Classification:** small synthesis/contract clarification, required before implementing
publication or collecting receipt history. The bounded F04 claim remains accurate
under its stated sequence; this is not a newly falsified crash-cut result or a new
architectural contradiction.

Sources: `SYNTHESIS.md:207–222,252,273,282,329,333`;
`cycles/0014.md:108–115,227–232`; `baseline.py:81–103`;
`crash_boundaries.py:172–220`; `test_crash_boundaries.py:112–131`;
S:`docs/20260911_shared_flow_demo_contracts.md:16–21`.

### NA2 — P2: Make the retained clock information for S05 concrete before eviction

**Action:** Qualify C1/S05 and the time-contract proposal with a reconstructibility
requirement for measurement-time conversion. If later reassociation under a revised
clock model is promised, retain the original acquisition timestamp in its identified
clock domain/incarnation and the applicable mapping/model history, or an equivalent
representation from which that association can be recovered. Preserve uncertainty
and relevant common clock dependence. If this information is discarded, declare
time reassociation unsupported for those records.

S05 currently preserves acquisition time, clock identity/uncertainty and a lawful
replay/model path. That is directionally correct but can be implemented too weakly:
a single converted timestamp plus uncertainty and a clock label need not retain the
information needed to apply a changed offset/drift model. Two different local sample
times under different original offsets can have the same converted timestamp; their
corrected associations need not agree. A timestamp alone is not the replay state for
the clock conversion any more than a mean alone is the replay state for a solve.

The owning time rules specify conversion using the clock reference at publication
but do not currently require retaining the original time or the conversion revision.
The synthesis's proposed CONVENTIONS §4 change discusses epoch identity and directed
re-expression; it should also identify this time-preservation obligation. No clock
estimator, new timing sweep or choice of withdrawal-time policy is needed now.

**Classification:** a concrete refinement of T01 and a conditional preservation
requirement for deferred S05, not evidence that S05 is already supported. It matters
before a publisher permanently reduces measurement records, even on one local network.

Sources: `SYNTHESIS.md:55–68,93–96,279,300,325`;
N:`docs/SUBSTRATE.md:224–263`;
N:`docs/CONVENTIONS.md:93–109`.

### NA3 — P2: Assign the withdrawal-expansion and acknowledgment handoff explicitly

**Action:** Before the generic API and application invalidation path are implemented,
assign responsibility for translating a selected withdrawal policy into the affected
artifact/dependency scope, delivering that scope across private computation boundaries,
and recording when the applicable serving gate has taken effect. Distinguish an
acknowledgment that a source grant changed from one that an affected inference-serving
scope has durably applied its invalidation. If expansion/rebuild is incomplete, the
contract must identify the scope that remains unavailable. A conservative whole-scope
barrier is a valid initial implementation.

This is substantially recognized by C1/C6/C7 and D2, but the owning-contract handoff
is not yet concrete. The entitlement fixture takes complete policy snapshots;
`Ledger.query()` checks the derivative's explicit summary right, not whether a removed
raw right should propagate through that summary. The report explicitly acknowledges
that P3 requires removing that summary right. The crash fixture then receives an
already constructed gate. The evidence therefore does not establish the transformation
between these two interfaces.

The application knows numerical support, including calibration/training descendants;
the provider must not infer it from opaque records. A permitted design can have the
application register opaque dependency keys/affected scopes that generic control
mechanisms enforce, or have authorized application roles supply the invalidation set.
Either way, the authorization to retain/read the private support index and its recovery
placement must accompany the artifact's promised lifetime. A public result reader
need not receive that index. If it cannot be resolved lawfully after recovery, use
unavailable or a declared coarse invalidation, not an assumed empty dependency set.

**Classification:** implementation/contract ownership follow-up within D1–D4/D6,
not a request to choose P2 versus P3, change consent, or introduce cross-network atomic
commit. The synthesis already states the semantic requirement; this report does not
claim that omission of a wire API invalidates its conclusion.

Sources: `SYNTHESIS.md:62–68,176–215,298–303,316–334`;
`cycles/0004.md:227–242`; `entitlements.py:103–148,206–231`;
`cycles/0014.md:26–40,64–72`;
S:`docs/20260911_shared_flow_demo_contracts.md:35–48,65–70`;
N:`docs/SUBSTRATE.md:363–387,637–652`.

## Sound choices and nonissues

- **Consumed coverage is distinct from a transport horizon.** The recovery witness
  identifies two unequal live-input sets with the same durable horizon. C7 and the
  proposed replacement of SUBSTRATE E6 address a real contract mismatch. Requiring an
  actual input manifest does not require reliable delivery of every feed record;
  sufficient authorized checkpoints or explicit loss of recovery remain alternatives.
  Sources: `cycles/0008.md:15–40,55–68`; `SYNTHESIS.md:202–205,269–271,327`;
  N:`docs/SUBSTRATE.md:129–147,412–414`.
- **Dependency compatibility need not be global synchronization.** The nested fixture
  binds the same revisions and joint uncertainty while reusing an independent edge.
  It assumes an announced target tuple and trusted covariance. C5 accurately separates
  that evidence from discovering a current snapshot or constructing it in a distributed
  system. Those remaining mechanisms can wait under the stated profile.
  Sources: `cycles/0013.md:53–65,86–98,216–223`;
  `snapshot_epochs.py:118–172`; `SYNTHESIS.md:149–174,258`.
- **Invalidation durability is independent of replacement availability.** The corrected
  pending-control barrier and separate gate are useful local witnesses. The pre-commit
  restart behavior is explicitly weaker than external effective-time enforcement;
  SYNTHESIS does not hide that weakness. D6 must be chosen before a durable-control API
  is advertised, but its unresolved offline extension is not a reason for another cycle.
  Sources: `crash_serving_barrier.py:26–34,37–66`;
  `cycles/0014.md:46–81`; `SYNTHESIS.md:217–222,303`.
- **Private provenance and public results can coexist.** The in-memory opaque issuer
  demonstrates a binding/projection schema only. Predictable fixture tokens are not
  claimed as capabilities or a privacy mechanism; the synthesis expressly leaves
  inferential disclosure and per-audience consent open. I would not classify the lack
  of cryptographic enforcement in that fixture as a defect in the research claim.
  Sources: `snapshot_epochs.py:195–232`; `cycles/0013.md:100–121`;
  `SYNTHESIS.md:48–51,185–195,268,301`.
- **Role separation and the boundary are sound.** Generic identity, grants, custody,
  acceptance and opaque references belong below the application; numerical sufficiency,
  support, geometry and query validity belong above it. The proposed changes do not
  require factor-graph logic in the provider. Existing stricter application privacy
  profiles can remain. The adapter's parity and conformance work is necessary when
  proposals become owning contracts, not an experiment missing from this closeout.
  Sources: `SYNTHESIS.md:312–339`; shared-flow §§1–3,5; BLANKET §§4–7.

## Assumptions, deferrals and owner decisions

The assessment assumes authentic origins, truthful application support/manifest
assembly, adequate authorized retained numerical state, trusted applicable policy,
and an implementation that eventually supplies the declared atomic local storage
primitive. The 90 tests verify their bounded models, not those premises. Repairing
unavailable nodes before sessions helps availability planning but supplies neither
authorization freshness nor equal live histories.

R07 and F05 remain open through scoped issuer/grant/assignment generations, explicit
unknown authority, immutable result bindings, and checks on direct replies as well
as register writers. Their mechanisms can be selected later; no offline or partition
safety was demonstrated. F06/S08 retain useful separation of durable evidence,
replaceable notifications and mandatory invalidation; NA1 adds the publication
selection state that must survive bounded receipt retention. S04–S07 require the
stated lawful numerical/model state, query/epoch/uncertainty semantics and later
consumer validation; NA2 makes S05's time state more concrete. None of these rows
inherits tested support from an extension field or from its component fixtures.

Dana still owns D1–D4's derivative rights, descendant scope, selector/retention and
disclosure choices. D5 must identify whether a request asks for current, historical,
explicit-revision or reduced coverage, including the meaning of “current” for the
initial profile. D6 must identify the authority/control effect point and startup
behavior before persistence is relied upon. D7 can wait until physical consumers;
the research correctly refuses to promise that a halt or smooth transition is safe.
Engineering owners can settle local storage, operation namespaces, acceptance
preconditions and conformance tests within those decisions without asking Dana to
choose an implementation mechanism.

No new research cycle is recommended. Preserve the original evidence and this review;
record dispositions of NA1–NA3 separately and carry accepted obligations into the
owning-contract change process.
