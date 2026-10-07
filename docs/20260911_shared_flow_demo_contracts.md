# Shared flows: architecture constrained by demonstrations

**Status: current architectural direction, 2026-09-11; not implemented.**
This document supersedes conflicting assumptions in the September 6 cascade,
estimation-flow note, authorization model, and protocol prose. It keeps the flow as
one shared dynamic concept with constituent streams and fitting roles. It does not
introduce application schemas or numerical estimation semantics into soradyne.

## 1. One shared concept, one flow

A flow exists through the participation of multiple people's authorized devices.
Different ownership of input streams does not require splitting the flow. A type
defines the functionality of streams and fittings; compatible subtypes/policies
choose placement, redundancy, retention, disclosure, and recovery behavior.

A flow can contain jets, replayable input streams, a convergent drip, and query-response
streams. A fitting names work between streams. A policy assigns attempts and defines
when a result is accepted/published. Exclusive assignment, redundant computation, and
local materialization are alternatives only when compatible with the type's guarantees.
Vector clocks partially order input histories; concurrent histories need the type's
merge rule, not an invented total preference between computations.

One producer can offer the same source records into several flows. Each destination
has its own permissions, delivery state, and retention rights. Application-supplied
observation identity and dependencies remain opaque payloads and are preserved.
Reusing inputs does not require a separate flow per source or a global source registry.
Independent shared concepts/lifecycles may still justify separate referenced flows.

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

Recovery must identify the retained inputs/checkpoints needed by the fitting. Publishing
a convergent output does not imply that output is sufficient to restart its producer.
Readers may request versioned outputs without being entitled to inspect the internal
computation or its input provenance. Provider mechanisms remain generic; an adapter
translates each application's contract without importing its vocabulary into the core.

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
| F6 Private inputs, public result | Public output/query stream, private inputs; consumer receives an opaque revision | Reading results grants no input/history access; the fitting can serve a selected view without exposing its entire internal state |
| F7 Slow archive and fast interaction together | Album input/edit/history with retained resolution derivatives plus a bidirectional tactile-like synthetic jet; disconnect mid-transfer | Album retries use retained artifacts; configured live/control deadlines are measured, missed deadlines surface as degradation, archival replay cannot monopolize the link; no claim that simulated timing establishes physical haptic stability |

F1-F6 constrain the shared-estimation transport surface. F7 prevents that use case
from narrowing the protocol to slow records or a single computational leader. Choose
payload sizes, rates, deadlines, storage commitments, and injected faults before each
fixture is called runnable; evaluate all scenarios together rather than optimizing one
by violating another. Additional demos are required for requirements not covered here.

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
