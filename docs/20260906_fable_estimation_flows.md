# Flows for Distributed Estimation — Protocol Requirements

**Date**: 2026-09-06
**Author**: Claude (Fable 5.1, Claude Code), at Dana's request
**Status**: Requirements and design note. Currency **high**; settled-ness **low** (nothing
here is in the spec or code yet). Companion to a concrete-application mapping note kept
outside this repo, which works one concrete application through in that application's
own terms; this
document states what the *protocol* must provide so that any application of this class
can be built on it, without depending on that application's particulars.

---

## 1. The class of application

There is a family of applications whose data is **many parties' observations, fused by
a designated party into an estimate that everyone consumes**. Examples the spec already
gestures at: multi-device coordinate-system alignment (§Spatial Measurements),
consensus mixing of many audio tracks (§Sound), reconstruction of a call from every
participant's local recording (§Video). Others in the same shape: a shared map built from
several people's scans; a group's collaborative sensor calibration; a robot cell where
several cameras watch several machines and one process solves where everything is.

They share a structure that the current spec describes only in outline:

- **Observations are per-party, append-only, and authoritative for their author.** No
  merge is ever needed between two parties' observations; they are simply different
  facts. Some observations must be kept forever (a calibration, a survey); others are
  superseded within seconds (a tracker at 100 Hz).
- **The estimate is computed by one party at a time**, chosen deliberately, with
  standbys ready to take over. Every consumer wants the *same* estimate, so the estimate
  is convergent in the drip sense, but its convergence comes from "same inputs, same
  deterministic computation", not from a CRDT merge.
- **Estimates compose across scopes.** A small scope (a room) is solved on its own and
  contributes a summary (its boundary, with uncertainty) to a larger scope (a building)
  which must *not* also see the room's raw observations, or it will double-count.
- **Consumers ask questions**, not just read state: "what is the relation between these
  two things, at this time, with joint uncertainty?" Answering needs the estimator's
  internal state; it is a request/response, not a materialised view.
- **The observations are the sensitive part.** A trajectory of a person's head, the
  positions of cameras in their home, a probe log of their basement. The estimate is
  aggregate and comparatively public. So authorization runs *along the pipeline*: raw
  observations are readable by the estimator only; the estimate by everyone in scope;
  the summary by the next scope up.
- **Several people contribute.** The estimator for a shared scope sees everyone's raw
  observations, so choosing it is an act of trust that every contributor must consent to.

The rest of this note lists what the protocol needs so that this shape is expressible
with the flow model's own vocabulary, and where the spec or the implementation is short.

---

## 2. Boundary: the estimation scope is the flow

By the Flow Boundary Principle, a flow is drawn around the streams that must be fitted
back together invisibly. In this class the fitting *is* the estimate, and the things
that must be fitted together are the observations of one estimation scope. So **one flow
per scope**, and nested scopes are separate flows linked in the mesh. This is the spec's
principle applied without modification, and it is worth recording because the
2026-07-29 granularity plan proposed a different determinant (shareability) that this
class does not need: sharing is handled inside the flow by per-stream authorization and
between flows by references.

Long-lived things that outlive any scope (an instrument's calibration and geometry, a
person's sensor rig) are separate flows by the *independent lifecycle* guideline,
referenced from scope flows.

---

## 3. Stream categories the spec names but the implementation lacks

The spec lists drip, jet, and "no category" (query-response, read-once) streams, and its
examples use "edit streams (per-party)" as a fourth working category. Only the drip is
implemented (`FullReplicaFlow` over `ConvergentDocument`). This class needs all four,
with these precise semantics:

### 3.1 Per-party log (the "edit stream" made first-class)

- One writer (the party), append-only, strictly sequenced per writer, replayable from
  any sequence number, exactly-once per replay, in order.
- **No merge.** The fitting consumes logs; it does not fold them into one document. The
  existing per-party journal and horizon machinery in `FullReplicaFlow` is the right
  substrate; what is missing is a flow type whose fitting is not `ConvergentDocument`.
- The **horizon** (latest sequence per writer) must be exposed to the fitting, because a
  derived record needs to say which inputs it was computed from (see §5).
- Durability at the origin is the author's policy; replica durability is the flow's
  memorization policy (§6).

### 3.2 Jet

- Per-party, lossy, in order per writer, no replay, retained by no one beyond a declared
  window.
- Declared in `StreamSpec` today; nothing routes or windows it. Needs an implementation
  over the same envelope routing as everything else, with drop-under-backpressure and a
  window.

### 3.3 Drip written by a role, not by everyone

The estimate stream is a drip in every spec sense (eventually consistent, authoritative,
same answer everywhere after settling) but has a **single writer at any time**: whoever
currently holds the estimator role. Two consequences:

- The drip's backing store need not be a CRDT. A keyed register with "the current role
  holder's write wins, and a write from a former holder is rejected" is sufficient and
  simpler. `ConvergentDocument` can still be used, but the delegation policy must gate
  writes.
- Convergence across successive holders comes from determinism (same log horizon → same
  estimate), which the flow type must promise and the implementation must test.

### 3.4 Query-response

A stream on which a role holder answers requests. Semantics: timeout, typed errors
("no server", "not authorized", "stale"), any authorized party may call. The image flow
in the spec is described this way; nothing implements it. Needed for "ask the estimator
a question" and for large-asset fetch.

### 3.5 Summary (export) stream

A drip, written by the estimator role, holding the scope's boundary summary for a parent
scope. It is listed separately from the estimate stream because **it has a different
audience**: the parent scope's estimator only. It is the *only* stream of a child scope
that a parent is ever authorized to read (§7).

---

## 4. Delegation policy needs eligibility rules

The spec's delegation policies say responsibility can be transferred and roles
redistributed, and Phase 5 built host assignment with failover for a single host. This
class needs the policy to express:

1. **Capability**: the party advertises that it can fill the role (compute, software).
2. **Standing**: always-on parties preferred over intermittent ones, because consumers
   (a robot, a headset) need the estimator reachable when they act.
3. **Authorization coverage**: the party must be authorized to read *every* observation
   stream in the flow. In a multi-person flow this is the crux: the estimator is the one
   party that sees everyone's raw observations, so a party can only be delegated the
   role if every contributor has granted it read access. The delegation policy must
   check this, not assume it.
4. **Completeness**: among eligible parties, prefer the one with the most complete log
   horizon.
5. **Lease semantics**: the role is held under a renewable time-bounded lease; a write
   to a role-gated stream carries the lease and is rejected if the lease is not current;
   standbys are notified when a lease lapses.

None of this is application-specific. It belongs in the policy vocabulary of the spec's
§Delegation Policies.

---

## 5. Derived records must cite their inputs by horizon

Any record written by a role that *computes* from other streams (the estimate, the
summary) should carry the horizon of the input logs it consumed. Reasons:

- **Reproducibility**: a standby taking over can verify it reaches the same estimate.
- **Auditability**: a consumer can trace an answer back to the observations behind it.
- **Feedback prevention**: a party must never republish a derived record as if it were
  an observation (or the estimator will consume its own output as evidence). If
  observation-stream *types* have no field for a derived-record reference, and only the
  summary stream does, the loop is a type error at the stream boundary. The protocol can
  make this cheap by giving every derived record a horizon and every observation record
  a provenance that can only name raw sources.

The horizon is already tracked; exposing it on derived writes is a small, general
feature.

---

## 6. Memorization policy must be per stream and must bound replicas

The spec's memorization policy table (security at rest, robustness, retention,
provenance) is per flow. This class needs it **per stream**, because the streams of one
flow have very different sensitivity and lifetime:

| Stream | Retention | At rest on replicas | Who may replicate |
|---|---|---|---|
| observation log (single-person flow) | indefinite | device default | estimator + standbys |
| observation log (multi-person flow) | bounded: while the estimate it informs is current, plus a window | encrypted | estimator + standbys |
| observation jet | window | never rests | nobody |
| estimate, diagnostics | indefinite | open | all readers |
| summary | indefinite | open | parent's estimator + standbys |
| large assets | indefinite | encrypted | origin only unless granted |

Two rules to add to the spec's memorization section:

- **Origin custody is the author's, always.** A flow's policy governs *replicas*. The
  party that produced a record keeps it under its own policy regardless of the flow's,
  and revocation never reaches origin custody.
- **Replica custodians that leave must discard.** Losing the role, losing the grant, or
  retiring from the flow obliges a party to discard its replicas of the affected
  streams per policy, and the protocol (not the application) does the discarding.

The "bounded retention on replicas for other people's observations" row is the one that
makes it honest to say: *the estimator held your observations only while it needed
them, encrypted, and they are gone now.*

---

## 7. Nesting as the disclosure mechanism

The spec's Flow Mesh section says references between flows are not a protocol-level
construct. For this class the mesh carries an obligation the protocol must enforce:

> A parent scope reads a child scope's **summary stream only**. It is never authorized
> to read the child's observation streams.

This is simultaneously a correctness rule (no double-counting) and the privacy
mechanism: a person's private scope (their own sensors, their own room) exports a
summary with uncertainty, and a shared scope composes summaries. Nobody outside the
person's own devices ever holds their observations. It aligns with the vault's "all data
is derived" pillar: what a receiver gets is a resampled derivation appropriate to the
receiver, never the same bytes.

Protocol support needed:

- **Reference plus grant**: a flow's configuration can reference another flow and hold
  a grant on a named stream of it. Opening a referenced flow with an insufficient grant
  yields an *opaque reference* the application can display but not read.
- **Revocation of a referenced stream** must be observable by the referencing flow's
  role holders, so the estimator can drop the input and report reduced observability.
- **Joint, not marginal, summaries**: a summary over several boundary elements should be
  a joint record so the parent composes it correctly. That is an application-level
  format matter, but the protocol's summary stream should not force one record per
  element.

---

## 8. Multi-person flows need a flow-scoped live set

The spec scopes ensembles to a single capsule. A multi-person flow has parties from
several capsules. The protocol needs a **flow ensemble**: the live, reachable set of
parties of one flow, across capsules, layered over capsule ensembles. Presence,
reachability (not direct connection), lease-lapse notification, and routing all operate
at this level for shared flows. This is a topology-layer gap distinct from the
inter-capsule authorization gap already recorded in `authorization-model.md`; the two
must be designed together but they are not the same thing.

---

## 9. Time

Observations carry timestamps that must be comparable across parties to a documented
tolerance. The protocol currently has no clock facility. Minimum: an ensemble-level
clock-offset estimate per party with uncertainty (round-trip timing over any transport
gives this; BLE gives it cheaply), exposed to applications, with the application writing
the timestamp into its own record. The protocol must not stamp records on arrival and
call that the observation time.

---

## 10. Setup and the application-facing surface

`authorization-model.md` §6 already names the target API (`session_start`,
`flow_open(uuid, type)`, `flow_sync`). This class fixes what the surface must contain,
in generic form:

- identity: self id, sign, "same capsule as me" (boolean only);
- lifecycle: create flow of a registered type (which registers its stream set and
  per-stream policies in one step), retire flow, request access to a named stream of a
  flow with a mode and a plain-language reason (opens the auth surface), request
  introduction between capsules;
- streams by name: append/replay/horizon (log), publish (jet), subscribe (both),
  put/get/tombstone/watch (drip register), request/serve (query-response);
- roles: claim with capability + standing + horizon, renew, register as standby;
- presence: peers of a flow, presence events including lease-available and revoked;
- clock reference;
- a closed set of typed errors: not granted, revoked, no server, unreachable, timeout,
  schema rejected.

Applications hold flow UUIDs as opaque strings in their own storage
(`app_soradyne_boundary.txt`). Applications in languages other than Rust need a binding
to exactly this surface and nothing more; the first non-Flutter consumer will be Python.

What the application must never be able to ask: who a principal is, what other flows a
party belongs to, or anything about capsules by name.

---

## 11. Example, in the style of the spec's data-type examples

**Multi-party coordinate alignment.** A flow per estimation scope (a room, a workshop, a
site). Streams: per-party observation logs (survey-grade measurements, calibrations,
rigidity constraints), per-party observation jets (frame-rate detections, tracker
output), a per-party declarations drip (which frames and landmarks a party owns), an
estimate drip written by the aligner role (transforms with covariance and validity
epochs, each citing its input horizon), a summary drip for a parent scope (the joint
relation of this scope's boundary frames), a query-response stream for "relation between
A and B at time t with joint uncertainty", and a diagnostics drip. Roles: source,
aligner (one active under lease, standbys), memorization, sink. Policies: delegation
with eligibility (always-on, authorized for all logs, most complete horizon);
memorization per stream as in §6; retirement for short-lived sessions. Instruments are
separate long-lived flows (declaration, frozen geometry with survey uncertainty,
appearance model reference) that scope flows reference. A person's own sensors form a
private scope exporting only a summary to any shared scope. This is the spec's
§Spatial Measurements example with its streams and roles made precise.

---

## 12. Gaps, in build order

1. Generic application surface (§10) and a Python binding.
2. Per-party log flow type without a CRDT fitting (§3.1).
3. Jets (§3.2).
4. Query-response streams (§3.4).
5. Delegation with eligibility, lease, and role-gated writes (§4).
6. Per-stream memorization with origin/replica distinction and discard-on-leave (§6).
7. Reference-plus-grant and opaque references in the mesh (§7).
8. Flow ensembles and inter-capsule grants (§8, with the auth work).
9. Clock reference (§9).
10. Large-asset flow (shared with image work).

Items 2–4 are stream categories the spec already names. Items 5–7 are policies the spec
names but does not detail. Items 8–9 are new concepts.
