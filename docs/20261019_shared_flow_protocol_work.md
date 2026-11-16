# Shared-flow protocol work for a small local deployment

Assessment: 2026-10-19. The owning requirements are
[shared-flow contracts](20260911_shared_flow_demo_contracts.md), including LC-I/V/L/H/S/A/R.
This is an implementation sequence, not a claim that F1–F11 pass. Existing transport,
identity, dissolution and full-replica document sync are building blocks; they do not
yet compose into the contracted shared-flow application surface.

## What the existing implementation establishes

`flow/stream.rs` provides read/write/subscription abstractions.
`ffi/convergent_flow_ffi.rs` exposes operations and materialization for a convergent
document. It is not yet the public opaque log/jet/drip/query-response surface with
per-stream authorization, assignment and custody controls.

`flow/types/drip_hosted.rs` retains a `HostAssignment` that accepts a higher announced
epoch. That does not establish exclusive authority under competing partitions. Its
`append_to_journal` logs storage errors and returns no durability receipt; it does not
establish the acknowledged-custodian commitment required by §3. Neither observation
requires breaking the existing document applications. Introduce the new guarantees
behind a distinct public surface and migrate consumers deliberately.

## Decisions carried forward

- A deployment's pieces, capsules, flow administrators, initiators, memory providers,
  fitting candidates and output readers are separate sets. Physical co-location and
  introduction grant no additional content authority. Single-capsule testing cannot
  substitute for F1/F4's independently controlled contributions.
- A type declares streams, views, required publishing roles and recovery capabilities.
  Publication assignment, input authorization, durability acknowledgment, application
  acceptance and removal completion are distinct. An assignment carries no implicit
  permission to read another participant's private history.
- Private-history removal intentionally preserves the defined shared output and its
  restricted support accounting. The provider neither subtracts numerical influence
  nor treats a new participation as new evidence. Rejoin grants no historical access.
- Publication is checked at acceptance/reply release, not merely when work starts.
  The October 19 §5 clarification binds exclusive output streams to the required role;
  omission or another role cannot bypass it. Retained output reads remain distinct.
- Origin durability is a selectable commitment for applications that require it;
  non-durable producers remain supported. Every acknowledgment states the promised
  storage outcome. Do not turn a best-effort local append into a durable-success claim.

## Ordered execution slices

### SP1 — Public declarations and authorization state machine

Define versioned opaque flow/stream declarations, identity and authorization-instance
handles, explicit writer/read scopes, live versus historical access, publication-role
bindings and structured outcomes. Model independent rights and grant changes with a
generic room-reading fixture. Specify the complete set of private inputs required by
a fitting candidate; one read grant is not proof of complete coverage. Changes to
required inputs must invalidate or explicitly revise eligibility/configuration.

Acceptance: F1/F4/F6/F8/F10/F11 boundary cases in process, including omitted/wrong
publication assignment, independently revocable rights, a late old participation and
a fresh enrollment. Output read/admin rights reveal no private inputs or support.
This slice does not choose application schemas or claim encrypted distributed sharing.

### SP2 — Exclusive authority and sharing protection mechanisms

Choose the coordination mechanism and its failure assumptions before connecting SP1
to multiple processes. Specify the authority set, durable terms, transfer protocol,
renewal/expiry clocks, rejection point, reader checks and behavior without coordination.
Compare a fixed-authority experimental profile (unavailable on authority loss) with a
quorum profile (progress only with sufficient connected authorities). Neither a highest
announced epoch nor a timeout-based suspicion is sufficient. A fixed-authority profile
must not be presented as the full automatic-failover contract. Candidate preference
needs a defined collection interval and hysteresis, separate from authority safety.

At the same boundary, choose how introductions, per-stream recipient membership,
end-to-end payload keys and key epochs change on withdrawal without exposing capsule
internal keys to other capsules. Route authorization and ciphertext custody are separate
from plaintext readership. Bind author, flow, stream and authorization/assignment
context so a replay cannot acquire authority in another use. Do not invent cryptographic
primitives in the adapter or rely solely on authenticated hops for the blind bridge.

Acceptance: F3/F4/F5 with competing candidates, delayed output/replies, restart with an
old term, partition/reconnect, and one reader withdrawn while another authorized use
continues. Specify unavailable/degraded results and authority-validation evidence.
Concrete mechanism/library selection remains open; the local lease sketch is not proof.

### SP3 — Durable records, custody and reconciliation

Define append acknowledgment and replay positions (including removal holes), required
custodian receipts and at-rest profiles. Persist entitlement/control state independently
of application computation. Persist pending duties, affected scope and stale-save
fences before acknowledging logical controls; completion follows cleanup and installed
late-input rules. Specify bounded retention for any staging and non-renewing retry
expiry. Application-owned caches/workers must register their cleanup obligations.

Acceptance: F2/F9/F10 with disk failure and actual process death at control/save/ack
boundaries, a missing custodian, permitted transient late input and owner retention.
An empty fitting restart cannot reset duties or restore removed ordinary history.
Retained historical artifacts must remain distinguishable from currently publishable work.

### SP4 — Executable surface across a language boundary

Expose the agreed public calls, handles, errors and events for declarations, channel
operations, permissions, assignment and reconciliation. Run F1/F2/F6 from another
language using opaque toy records. Keep provider persistence and protocol mechanics
inside the provider; adapters translate only. Unsupported declared capabilities fail
at setup rather than returning apparently successful operations.

### SP5 — Scheduling, time and mixed traffic

Define clock quality/timescale/fallback, query deadlines, cancellation, bounded queues,
live expiry and control/replay priorities. Keep renewal and removal reconciliation
running when a fitting is slow. The application supplies acquisition time and content
validity; delivery time is not a replacement. Preserve disconnect/leave distinctions.

Acceptance: F7 plus skewed clocks, slow readers, paused fitting, delayed cleanup and
large replay. Measure queue age, expiry, reconciliation latency and useful delivery;
declare fixture rates/sizes/deadlines before interpreting measurements.

### SP6 — Separate processes, then physical pieces

Run the generic scenarios on three to five processes with role placement independent
of administration and initiation. Stop the initiating process, kill the fitter, remove
one route, restart custodians and revoke one participant. Record the supported profile
and remaining gaps; then run adapter conformance and the same protocol on hardware.
Do not postpone independent ownership and withdrawal until after a single-capsule demo.

SP1 and SP2 define the protocol before the full runtime; SP3 can develop against the
declared state machine. SP4 requires those contracts and their bounded implementations.
SP5 must be included before SP6. No fresh information-lifecycle research cycle or
automation is started by this plan.
