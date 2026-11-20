# Shared flows: architecture constrained by demonstrations

**Status: current architectural direction, 2026-09-11; not implemented.**
This document supersedes conflicting assumptions in the September 6 cascade,
estimation-flow note, authorization model, and protocol prose. It keeps the flow as
one shared dynamic concept with constituent streams and fitting roles. It does not
introduce application schemas or numerical estimation semantics into soradyne.

**Participation amendment: 2026-10-15; adopted design, implementation pending.**
See [meeting minutes](20261015_participation_lifecycle_minutes.md). This is a local
good-faith profile, not a mandatory policy for every flow type.

**Reconciliation/inspection clarification: 2026-10-16; implementation pending.**
See [meeting minutes](20261016_reconciliation_and_inspection_minutes.md). Prompt
eventual cleanup may use bounded protected staging; permitted shared outputs retain
downstream influence. Types may expose provisional state separately from consumer
acceptance. No new callable API or implemented demonstration is claimed.

## 1. One shared concept, one flow

A flow exists through the participation of multiple people's authorized devices.
Different ownership of input streams does not require splitting the flow. A type
defines the functionality of streams and fittings; compatible subtypes/policies
choose placement, redundancy, retention, disclosure, and recovery behavior.

A flow can contain jets, replayable input streams, a convergent drip, and query-response
streams. A fitting names work between streams. A policy assigns attempts and defines
when a result may be published and how a consumer-accepted view is selected. A type
may expose coherent provisional/partial fitting state without universal quality gates;
authority, disclosure and revision binding still apply. Exclusive assignment, redundant
computation, and local materialization are alternatives only when compatible with the
type's guarantees.
Vector clocks partially order input histories; concurrent histories need the type's
merge rule, not an invented total preference between computations.

One producer can offer the same source records into several flows. Each destination
has its own permissions, delivery state, and retention rights. Application-supplied
observation identity and dependencies remain opaque payloads and are preserved.
Reusing inputs does not require a separate flow per source or a global source registry.
Independent shared concepts/lifecycles may still justify separate referenced flows.

**LC-I (evidence continuity).** Preserve application-supplied evidence identity,
revision, replacement, and support references across delivery, grant changes,
rejoining, and explicit historical backfill. A new authorization is not new evidence.
Rejoining does not itself authorize old history; backfill requires a declared scope.
The application determines inclusion and dependence. The provider must not infer
these from payload equality, authorship, or transport sequence alone.

## 2. Authority and participation

Persistent identity and grants outlive any particular currently connected ensemble.
Introduction connects trust domains; it does not merge capsules, grant every stream,
or share internal capsule keys. The exact group key mechanism remains to be designed.

Distinguish flow administration, contribution ownership, author identity, access to
inputs, eligibility to compute, authority to publish, and permission to read outputs.
A device may sign an output produced on behalf of a flow administered by another
capsule. Source permission does not imply permission for arbitrary derived disclosure.
Participants may explicitly authorize sensitive inputs for designated shared work;
cross-capsule traffic is not categorically restricted to a predefined summary kind.
Content-sensitive policy is supplied by the type/application and the person's grants,
not inferred by soradyne from opaque bytes or from the label raw/derived.

Eligibility combines the declared capability/standing and required authorizations.
Policy may preauthorize a set or class of candidates; a handoff cannot silently widen
that permission. The fitting's acceptance contract specifies the input/configuration
revision it used and what consumers may conclude from a result. Soradyne authenticates
and routes it; it does not validate application mathematics.

**LC-V (private inputs and output views).** A type may define all contributions as
private while authorizing a specified shared output to survive departure and removal
of input/participation history. Explain this at participation; continued authorized
output use needs no repeated prompt. In this profile, current and future authorized
participants in the same flow may read that output, without onward disclosure rights
to unrelated flows. This is not a universal permission for arbitrary derivatives.
Output access grants neither private source history nor contributor mappings. Keep
authentication identity separate from minimum opaque application accounting, and
minimize diagnostics. A token is not a proof of anonymity; its visibility and scope
matter. Token/key construction is not selected by this contract.

Defined output use includes the downstream influence the view is intended to preserve;
history removal is not total influence withdrawal. Within the managed available view,
there is no additional application per-use permission Boolean. This does not establish
application sufficiency for informative reuse or authorize additional disclosure.

**LC-L (leave and cutoff).** Leaving ends participation through the selected grants;
reachability loss alone does not. The local good-faith profile assumes no new input
is acquired after departure. With a usable departure timestamp, reject acquisition
strictly after it; earlier delayed input remains eligible under the prior scope and
normal checks. Without a usable timestamp, accept received input under that assumption.
This late-delivery permission does not admit new sessions or arbitrary writers.
An application supplies the acquisition/control semantics; the provider enforces
declared opaque scopes without interpreting content. Stronger offline freshness and
malicious-participant handling remain deferred.

## 3. Memory is abundant; active computation need not be

Default to substantial distributed retention when authorized, including useful derived
artifacts that avoid repeated work after reconnect. A producer need not have durable
local storage. Memory roles may retain its records while preserving origin attribution.
Acknowledged durability must state which required custodians accepted the record;
best-effort jet delivery alone is not such an acknowledgment.

Retaining evidence, keeping a fast-access representation, running a fitting, and
disclosing results are separate decisions. Policies may keep both dissolved storage
and faster materializations, with stated retention and security obligations for each.
Archives need not all remain loaded into computation. Cache keys include the relevant
input/configuration revision and representation; stale artifacts cannot masquerade as
current results merely because a memory role still has them.

Losing one grant or role removes rights obtained through it. It does not erase an
owner's archive or a copy still legitimately held under another authorization. Storage
tracks the remaining entitlements and policy obligations. Ciphertext custody is
distinct from plaintext read access; memory service is distinct from compute standby.
Retention tied to an application event requires an explicit generic lifecycle signal;
soradyne cannot recognize that event by inspecting application records.

**LC-H (history removal).** Optional removal covers the selected participant's inputs,
personal participation history, and obligated copies/caches/diagnostics in this flow.
It leaves the defined retained output, the owner's archive, unrelated authorizations,
and independently authored records outside that scope. A distinct independent right
elsewhere does not permit borrowing private material for this flow. Previously
delivered plaintext is not promised remotely erasable. Late admissible input may be
processed transiently into the authorized output but must not repopulate removed
ordinary history. Bounded protected staging under LC-A is allowed. Completion accounts
for pending work and installs the rules governing later arrivals too.

**LC-S (restricted support).** Retained output may require opaque evidence/revision
and affected-artifact accounting after private history is removed. The type states
its permitted content, restricted readers, and lifetime: while represented information
or admissible replay needs it. It is not a publicly readable participation log.
Missing support is an application limitation, not an assumed empty dependency set.
The provider preserves/enforces opaque references; applications resolve affected
content and numerical capability. They may reject unsupported changes or choose a
new computation instead of promising arbitrary subtraction from compressed outputs.

**LC-A (distinct acknowledgments).** A participation-change acknowledgment means the
grant change and applicable cutoff are installed. History-removal completion requires
the obligated custodians to acknowledge cleanup and the rules preventing restoration;
pending custodians keep completion pending. Retained authorized output may remain
available. Applications resolve affected private-history/output scopes; the provider
enforces generic custody and authority; adapters map the distinct acknowledgments.
Exact APIs and distributed completion mechanisms are not implemented by these clauses.

This profile allows prompt eventual reconciliation: a periodic/event-driven pass can
clean affected material that briefly reached protected storage. Declare/measure an
operating target, report pending/overdue duties and prevent indefinite postponement.
Roughly 5–10 seconds is an accepted healthy-local tolerance, not a universal SLA or
change to every stream's deadline. Retained output influence has its separately
authorized lifetime. No global computation barrier at request time is required.

Staging preserves scope/protection, has bounded non-renewing expiry, and cannot become
ordinary archival replay or forwarding. Retry/restart must preserve its obligations
and prevent forwarding/removal cycles. A completed pass covers selected history and
accounted pending work plus installed later-input rules; future eligible temporary
work remains subject to prompt cleanup. Fence stale fitting attempts/callbacks/saves
from restoring removed material after settlement. The mechanism may use a control
revision or equivalent; this contract does not prescribe a distributed transaction.
Grant withdrawal and exclusive publication fencing remain separate requirements.

### Concrete participation-surface implications

The surface must distinguish an opaque authorization instance from a persistent
author and an application evidence ID. Enroll devices explicitly; leaving captures
their prior input scope. New enrollments do not inherit that late-delivery exception.
Acquisition/control time is application-supplied; publication time cannot replace it.
A known cutoff without comparable acquisition time cannot establish eligibility.

Private streams need author-selective read authorization. Renewed live authorization
does not unlock earlier history; explicit historical permission selects a reader,
stream and previous participation. Administration grants neither private inputs nor
restricted accounting. Types declare private/output/accounting views; the provider
does not infer these from content.

Removal completion includes recipients and application-declared caches, not just
currently reachable memory roles. Duties survive application restarts. Permitted late
delivery carries a transient/no-retention disposition and is excluded from archival
replay; bounded protected staging under LC-A is compatible with that disposition.
Owner archives remain independent: removing a flow's private view must not
delete or silently borrow those archives. Delivery positions remain stable across
removal. Applications must explicitly declare affected private scopes; unsupported
mixed personal views cannot be mislabeled as retained output.

Event-driven retention needs an opaque application group and an authorized close
signal. Repeating the signal cannot extend its deadline; later replay cannot recreate
expired memory copies. Applications choose membership; the provider enforces expiry.
These refine future F8–F11 and retention fixtures, not implemented mechanisms or
passing demonstrations.

## 4. Routing crosses ownership boundaries without disclosing payloads

Use the Bluetooth connectivity paradigm and transport traits across the authorized
union of ensemble graphs. No permanent client/server class or privileged gateway is
introduced. A node filling a request-serving role is still a peer.

Forwarding permission, storage permission, and payload read permission are independent.
A constrained accessory may be the only bridge while lacking access to the carried
stream contents. The architecture requires end-to-end payload confidentiality and
origin authentication across relays, separately from hop security. A hop may unwrap
and rewrap its link protection without decrypting the application payload. Onion
routing and a concrete group-key protocol are open decisions, not prerequisites here.

Route changes do not change record identity, grants, or role authority. Live traffic,
control traffic, query deadlines, and replay compete for finite links: policies must
state backpressure, expiry, priority, and degraded behavior. Reachability through a
slow bridge does not imply suitability for every fast stream. This framework must
also support tactile communication; an estimation-friendly latency is not a universal
latency guarantee and archival capacity cannot compensate for an expired deadline.

## 5. Coordination and publication

Capability offers and availability observations can converge eventually. Include their
freshness, and do not confuse a suspected disconnect with certain global absence.
Assignment policies distinguish rerouting, planned transfer to a better participant,
failure recovery, and partition behavior. Better-candidate transfer needs a threshold
and backoff/hysteresis to avoid repeated unnecessary migration.

An exclusive-authority subtype must specify how authority is established, fenced,
renewed and checked by output writers/readers. Eventual availability observations
alone do not establish exclusivity. State the connectivity/coordination assumptions
under which progress is possible, and report unavailability when they fail. A different
type may permit provisional competing results if its consumers explicitly support that.

**Publication-boundary clarification (2026-10-19).** A type declares the required
publishing role on each exclusive output drip or query-response stream. This binding
is independent of memorization policy and input access. A publish/serve grant does
not substitute for current assignment authority; neither an omitted assignment nor
an assignment for another role or flow may authorize an update, retirement, or reply.
Required output grants participate in role eligibility and revocation. Check authority
when accepting output and again before releasing a reply from an asynchronous fitting
attempt, including the requester's read and server's serve permissions. A reply that
lost authority is rejected, not stamped with the successor's assignment or silently
retried. Permitted retained artifacts remain readable under their declared view;
old author identity alone does not imply a currently authorized serving attempt.
For example, a room-summary publisher cannot use an archive-maintenance assignment
to replace the room summary. These are requirements, not an implemented API or a
distributed fencing mechanism. See [protocol work](20261019_shared_flow_protocol_work.md).

Recovery must identify the retained inputs/checkpoints needed by the fitting. Publishing
a convergent output does not imply that output is sufficient to restart its producer.
Readers may request versioned outputs without being entitled to inspect the internal
computation or its input provenance. Provider mechanisms remain generic; an adapter
translates each application's contract without importing its vocabulary into the core.

**LC-R (application-controlled initialization).** An application may start an empty
fitting and defer numerical restoration. Retaining an output does not make it a valid
input or checkpoint. A type may allow explicit reuse of a sufficient retained
representation when the application accounts for its support and dependence on other
inputs. The provider preserves those references but must not manufacture a replay path
that treats existing information as new. Where recovery excludes affected informative input,
the application must also exclude informative components whose independence from it is
unestablished; differing IDs alone do not certify independence. A permitted value used
only as initialization is a distinct application operation, not automatically another
informative input. Its suitability and disclosure are application/type decisions.
Empty application startup does not reset grants or cancel custody/removal obligations.
Unknown application
dependence is surfaced by the application; the provider does not invent a transport
error or broaden authorization to hide it. Existing permission to serve an output
and readiness of a replacement computation are separate states.

Current inspectable output can coexist with the last consumer-accepted output. A type
defines the distinction and its revision/status semantics; the protocol does not infer
quality from publication or require numerical convergence before routing a permitted
provisional view. Private diagnostics retain their own access/removal policy.

## 6. Demonstrations to constrain implementation

These are proposed tests, not existing successes. Implement generic fixtures before
binding a real application. Their identifiers are local to this document.

| Demo | Setup and disturbance | Acceptance |
|---|---|---|
| F1 Shared work with plural devices | Two capsules, at least two pieces each; both contribute; compute can run in either | Introduction alone grants no content; explicit stream grants enable work; ownership does not privilege placement; outputs retain author and assignment identity |
| F2 Memory outlives a producer | Producer has no required durable local disk; another piece retains records; producer disappears | Acknowledged records replay from memory with original attribution; authorized cached derivatives serve without restarting their fitting |
| F3 Blind bridge | A constrained piece is the sole cross-capsule route; it is not a stream reader | Authorized endpoints communicate; the bridge holds no plaintext access keys; reconnection/replay preserve identities; overload expires live records rather than silently accumulating stale work |
| F4 Multiple uses, selective withdrawal | One source supplies two flows; revoke one use | The revoked audience loses future access and affected keys rotate; other authorized use and owner retention survive; inaccessible old plaintext is not falsely promised recoverable or erasable |
| F5 Move work and partition | Transfer work to a better eligible piece, then sever the bridge and delay old writes | No authorization expansion; no oscillation; old writes cannot claim new authority; exclusive mode reports unavailable when authority cannot be maintained; recovery uses retained state |
| F6 Private inputs, public result | Public output/query stream, private inputs; expose provisional and consumer-accepted views with opaque revisions | Reading either view grants no input/history access; status/revisions stay distinct; publication authority/disclosure hold without universal quality gating |
| F7 Slow archive and fast interaction together | Album input/edit/history with retained resolution derivatives plus a bidirectional tactile-like synthetic jet; disconnect mid-transfer | Album retries use retained artifacts; configured live/control deadlines are measured, missed deadlines surface as degradation, archival replay cannot monopolize the link; no claim that simulated timing establishes physical haptic stability |

F1-F6 constrain the shared-estimation transport surface. F7 prevents that use case
from narrowing the protocol to slow records or a single computational leader. Choose
payload sizes, rates, deadlines, storage commitments, and injected faults before each
fixture is called runnable; evaluate all scenarios together rather than optimizing one
by violating another. Additional demos are required for requirements not covered here.

### Participation-profile conformance additions (not implemented)

Use a generic fixture that computes a shared result from opaque private records.
These are future acceptance requirements, not claims about the existing demos/tests.

| ID | Disturbance | Acceptance / owning clause |
|---|---|---|
| F8 | Leave with/without timestamp; deliver delayed input; separately disconnect a peer | Apply acquisition cutoff or good-faith receive rule; disconnect alone does not remove participation (LC-L). |
| F9 | Remove history with one of two custodians pending; delay a pass/save and inject late input/retries | Pending/overdue status honest; bounded reconciliation preserves defined output/independent rights; stale saves cannot restore removed state; staging, if used, expires without replay, retry extension or forwarding cycles; restart preserves duties (LC-H/LC-A). |
| F10 | Rejoin, then explicitly authorize backfill | Rejoin alone grants no history; backfill preserves opaque evidence/revision/replacement IDs; application handles inclusion (LC-I). |
| F11 | Output reader inspects provisional/accepted views and requests private history/support; fitting restarts empty and requests retained-state reuse | Reject unauthorized disclosure; preserve restricted accounting/duties; permitted output use needs no repeated authorization; application determines reuse sufficiency and initialization separately; preserve support IDs (LC-V/LC-S/LC-R). |

Adapters must map LC-I/V/L/H/S/A/R to their consumer's owning clauses and tests in
their parity ledger before claiming this profile. There is no implementation claim
until generic fixtures, consumer fake, and adapter conformance all pass. Do not copy
application-specific numerical routines into provider tests.

## 7. Immediate design/implementation sequence

1. Align type, stream, grant, assignment, and custody definitions with sections 1-5.
   Settle the group-key and exclusive-assignment mechanisms against F1/F4/F5.
2. Define a generic application surface and retained-record/recovery semantics, then
   exercise it from another language through F1/F2/F6. Keep application record schemas
   outside the core.
3. Extend envelope routing and payload protection through F3; implement mixed traffic
   and replay through F7. Run separate-process and simulated accessory fixtures before
   making hardware support claims.
4. Use the application adapter's conformance and end-to-end demos to reveal missing
   requirements. Do not require the single-capsule milestones from the older cascade
   to finish before designing sharing: sharing is a first constraint.

The previous Alloy model is removed: its restrictions on authorship, raw disclosure,
origin-local storage, and unconditional discard encoded superseded decisions. The
retained lease model is a limited abstract mechanism sketch, not an implementation
or proof of network partition safety. See `models/README.md`. Replace broad formal
claims with small models tied to named scenarios once their assumptions are settled.

## 8. Constrained contributions and independent ownership (2026-11-16)

These are generic provider obligations; public methods and runtime support remain
unimplemented. A shared activity can receive both lasting deposits and borrowed
records from the same personally owned piece. Neither flow administration nor
possession of a recipient copy transfers record ownership.

| Clause | Observable contract |
|---|---|
| CR1 — Personal enrollment | Preparation creates a pending piece identity; its actual person authorizes capsule enrollment. Authentication is a replaceable boundary. Revocation affects existing handles and cannot be undone by repeated enrollment. |
| CR2 — Two-sided decisions | A destination can prepare a request, but the source person and destination person must each accept the same immutable terms. A piece cannot manufacture its person's consent. Rejection/cancellation does not grant access. |
| CR3 — Immutable terms | Bind source, destination, flow/stream, requested data class, acquisition scope/time, ownership, retention, permitted processing and disclosure. A revision needs fresh decisions and stops new submissions to its predecessor; it must not silently widen an accepted request. Prior deposits retain their original terms. |
| CR4 — Source attribution | The accepted source authenticates each submission. Preserve source and opaque evidence identity separately from storage receipt identity. Enforce declared data/acquisition boundaries, including uncertainty and late delivery. Payload classification remains application supplied. |
| CR5 — Record ownership | A deposit can belong to a different capsule owner from the authoring piece. Memorization and owner-history access follow that authenticated record binding. Borrowed records retain source ownership. Both kinds can coexist inside one flow and from one source. |
| CR6 — Receipt boundary | State exactly which memorizer commitment was completed. A local durable receipt is not a replica receipt. Submission retry preserves record identity and returns the original receipt; changed bytes/terms with that identity reject. Acknowledgment cannot precede the declared commit. |
| CR7 — Scoped withdrawal | Closing acquisition preserves accepted deposits. Borrowed-history removal acts on the accepted use, retains owner history, and never sweeps in other independently owned deposits. Old decision/submission retries cannot restore removed recipient copies. |
| CR8 — Application duties | Recipient delivery establishes its declared application cleanup obligation before returning data. Completion waits for each holder's acknowledgment, including after restart. A person/piece cannot acknowledge another holder's unfinished duty. The application drains in-flight deliveries and releases its copies before acknowledging. |

A generic fixture can ask a visiting person's sensor for selected temperature
observations, some deposited with the activity owner and some temporarily borrowed.
Closing capture preserves deposits; removing the borrowed use removes only its
recipient copies. Reopen storage, retry a lost receipt, and complete outstanding
holder duties without changing source identity or ownership.

An explicitly selected trusted-lab development profile may inject asserted actors
and omit cryptographic proof and protected identity storage when no attacker has
physical or network access. It must still execute the actual enrollment, decision,
authorization, ownership and custody transitions. This profile is not evidence of
the production protocol's cryptographic guarantees. Replacing authentication must
preserve the application-visible decision contract. Current reference experiments
outside this provider do not establish CR1–CR8 implementation here.

## 9. Durable accepted revisions under an explicit assignment (2026-11-16; extended November 19)

A minimal research subtype may configure one durable assignment authority and a
selected publishing piece, with no automatic election. PR8 permits an explicit
administering-person handoff. This is a bounded
subtype of §5, not a change to production routing or a partition-safety claim.

| Clause | Observable contract |
|---|---|
| PR1 — Declaration | The administering person explicitly declares the output, fixed role and personally enrolled initial publisher. Declaration cannot replace a publisher; PR8 governs replacement. Retained output and restricted accounting are distinct views; input submission cannot bypass the output role. Unsupported placement rejects. |
| PR2 — Assignment and access | Each publication requires current authenticated enrollment, exact assignment scope/role/holder/epoch and an independent write authorization. Revoking and regranting writing cannot revive an old attempt. Permission changes persist and repeated identical controls do not create new epochs. Connectivity loss alone cannot assign a replacement. |
| PR3 — Revisions | Retain immutable accepted revisions. A publication names its expected predecessor; a stale predecessor cannot replace newer output. Commit the result, restricted accounting, operation identity and receipt atomically at the declared memorizer boundary. |
| PR4 — Retry and recovery | A still-authorized exact retry returns its original receipt without republishing or moving the accepted head backward. Mutated retries reject. Restart preserves assignment fencing, access decisions and history. A local receipt promises no remote replica. |
| PR5 — Disclosure | Output authorization exposes result bytes and publication metadata only. Restricted input references/context require separate authority; output readers gain neither private input access nor publication rights. Recheck enrollment and access for retained reads. |
| PR6 — Input entitlement | A first subtype may restrict fitting to explicitly selected, activity-owner-owned inputs. Validate every opaque reference against current ownership and activity scope at commit; borrowed/cross-activity references reject in that subtype. The application declares complete input selection and numerical interpretation. |
| PR7 — Acceptance | The application/type selects acceptable output separately from provisional fitting state, and supplies opaque input/configuration/validity/accounting bindings. Publication revision and assignment epoch do not redefine application evidence identity or physical validity. |
| PR8 — Directed replacement | The authenticated administering person supplies the exact expected output assignment, a different currently enrolled piece of their capsule, and a stable output-scoped decision identity. Commit replacement, epoch advancement and decision atomically; stale or competing proposals reject. Exact decision retry recovers the original resulting assignment without reapplying it, even after later changes or target revocation; changed arguments reject. Recheck caller authority before recovery. Replacement neither grants writing or input access nor acknowledges previous application memorization duties. Former-assignment publications, including completed-operation retries, reject. Retained revisions and pending duties survive replacement and restart. |

For example, a person can select a replacement room-average fitting piece, recover
the same decision after a lost reply, and require its separate stream authorization
and authorized input reconstruction. The original fitting still drains its copies;
its late output cannot replace the accepted head. The decision receipt describes a
past transition, not necessarily the current assignment. This bounded output-scoped
primitive does not promise a renewable lease or role exclusivity across several
outputs; runtime/model eligibility and stopping current service remain application duties.

A temperature-fusion fixture can save accepted averages, restart the authority,
retrieve an older accepted revision, reject a late update against a changed head,
and give another capsule output-only access. It must not reveal sensor history or
restricted accounting with that grant. These clauses specify public behavior;
provider runtime methods and demonstrations remain unimplemented.

## 10. Managed fitting input selections (2026-11-16)

This extends CR8 and PR6 without weakening the owned-input subtype. A fitting may
explicitly select borrowed contributions under durable application memorization
duties. The following are generic guarantees, not an interpretation of payloads.

| Clause | Observable requirement |
|---|---|
| PI1 — Selection | An enrolled piece of the activity owner acquires an exact nonempty, duplicate-free ordered payload selection. Every reference must be owned by that capsule or currently authorized borrowed history in that activity. Acquisition atomically registers the selection and borrowed application memorization duties before delivery; unavailable references reject the whole operation. |
| PI2 — Instance | The opaque selection identity binds its piece, activity and immutable references. Each payload retains its original contribution authorization instance. Acquisition closure/revision preserves previously accepted history; removal ends the borrowed use. Retry returns the same active selection, changed arguments reject, and release cannot be undone by retry. |
| PI3 — Commit coverage | Delivery rechecks all selected references. A new accepted fitting revision using a managed selection must name exactly its full reference list and holding piece. Active selection, current borrowed authorization and all references are checked atomically with publication, independently of publishing assignment and stream authorization. The application declares selection and interprets payloads; the protocol does not prove numerical dependence. |
| PI4 — Release | The holding piece releases a selection only after outstanding deliveries, computation and private caches have drained. Removal acknowledgment rejects while any unreleased selection of that piece references the affected contribution. Pending selection metadata and duties survive restart. Recovery requires the old process stopped and private caches removed; a newly opened handle is insufficient. Listing pending selections returns no payloads and releases nothing. |
| PI5 — Retained state | Release/removal blocks new publication from stale work, preserving already accepted outputs, permitted restricted accounting, deposits and owner archives. An exact, currently publication-authorized retry of an already committed operation may recover its receipt without recreating copies or changing the output head. |

A temperature-fusion fixture can mix deposited calibration with borrowed readings,
delay a fitting while the borrowed use is removed, keep removal pending until the
job drains, reject its late publication, and recover pending duties after process
death. PR6 remains the owned-only alternative. No provider runtime implementation or
passing fixture is implied by these specifications.

## 11. Configured research service boundary (2026-11-17)

A bounded subtype may expose the contribution, accepted-revision and managed-input
contracts through a configured service. Transport must preserve these behaviors:

| Clause | Observable requirement |
|---|---|
| PT1 — Profile | Explicitly configure one authority endpoint, supported version/operations, message limits and timeout behavior. Authentication remains injected; a trusted-lab assertion shim must be deliberately selected. Unknown capabilities reject. No automatic alternate authority follows from connection failure. |
| PT2 — Public calls | Preserve typed opaque payloads and defined errors across process boundaries. Recheck enrollment and authorization on every call. A caller may inspect its own current authenticated actor identity without learning another capsule's owner. Preparing a piece creates no enrollment or access; output access still confers no private input/accounting rights. |
| PT3 — Failure | Reject malformed or unsupported framing and bound messages without partial delivery. Missing/timed-out replies may follow committed operations. Preserve original operation identities on explicit retry; do not silently retry mutations or manufacture identities. Timeout is neither cancellation nor release. Reply-size failures cannot be reported as proof that no commit occurred. Private transport buffers remain subject to memorization duties until delivery work drains; release/acknowledgment must reject while those buffers can still deliver private data. |
| PT4 — Authority | One configured durable authority survives service restart with decisions, assignments, memorization duties and accepted revisions intact. Exclude competing processes over the same configured local store; never treat cloned writable stores as coordinated authorities. Unreachability cannot replace an assigned publisher or acknowledge cleanup. |
| PT5 — Process ownership | A managed fitting process holds exclusive local ownership of its configured piece runtime through delivery, work and draining. Competing processes reject before recovery or private delivery. OS process death may release a local lock but leaves durable application memorization duties pending. Explicit recovery follows actual stopped work and cleared private caches; distinct devices retain distinct piece identities. |

A temperature-fusion fixture can run a contributor, fitting process and output-only
reader against one service without database access, lose a committed reply, restart
the service, and recover the same receipt. Another fitting process cannot release a
live process's private inputs by merely opening a handle. Concrete transport formats
and process-lock mechanisms are deployment choices. These are specifications only;
no provider runtime or demonstration is claimed.
