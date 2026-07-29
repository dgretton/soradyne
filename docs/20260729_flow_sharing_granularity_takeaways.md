# Flow Granularity & Selective Sharing — Takeaways and Open Questions

**Date**: 2026-07-29
**Status of conclusions**: currency **medium-high** (grounded in the current
protocol spec and auth model), settled-ness **low** (a live design conversation;
decisions are provisional, several depend on undesigned inter-capsule auth).
**Source**: `20260729_flow_sharing_granularity_conversation.md` (verbatim transcript).

This document distills the design conversation into (a) decisions taken, (b) the
core conceptual model that emerged, (c) known issues to design around, and
(d) open questions. It is a working note, not a spec — nothing here has been
written into `rim-self-data-flows.tex` yet.

---

## Decisions taken (provisional)

1. **Collection flow is blessed as a first-class, protocol-level flow type.**
   The generalized photo-album pattern — a convergent set of references (by UUID)
   to sub-flows, plus collection-level metadata — is promoted from an app-level
   convention (how the spec's "Flow Mesh" currently frames it) to an official flow
   type with defined semantics. Rationale: access-transitivity across references
   (see Issue A) cannot be solved purely at the app layer, and blessing the type
   gives the future coherence/synchronization primitive a natural home.

2. **Giantt relations are extrinsic-only.** Edges (`A REQUIRES B`, etc.) live on
   the **chart** (the collection flow), not on the item-flows. Items carry content
   (title, status, notes, duration) but no relations. Consequence, accepted
   deliberately: **there is no single global task graph** — only charts, each its
   own graph, freely inconsistent with the others, reconciled by lightweight
   fork-fix-reshare rather than by any enforced invariant. Justified by Dana's
   framing that giantt items "aren't meaningful by themselves — they exist to be
   related."

3. **The synchronization/coherence primitive is bookmarked to live on the
   collection flow type.** Not designed or built now; a reserved policy slot. The
   collection already knows the set of related flows, so "these referents form a
   coherence group; frame-align their updates" hangs off it naturally.

---

## Core conceptual model that emerged

### Logical atomization, physical aggregation
The apparent conflict between "atomize flows to the indivisible unit" and "that's
thousands of heavyweight flows" dissolves because the two statements are at
different layers:

- **Logical**: push each flow boundary down to the finest granularity at which
  you'd ever want *either* independent sharing *or* independent continuity. (The
  spec's Flow Boundary Principle and participant-scope guideline — "a mesh of many
  small flows is often better" — already endorse this.)
- **Physical**: a flow's irreducible footprint is a UUID + a type-hash pointer +
  its op journal. Thousands of UUIDs is nothing. Per-flow cost (transport
  connections, sync loops, journal files) collapses under multiplexing: one
  connection per *peer* carries all flows (`RoutedEnvelope` + a flow-id demux
  field), one sync loop services all flows, journals become one-file-per-peer with
  flow-id-tagged ops. The current `FullReplicaFlow`-per-directory heaviness is an
  implementation artifact, not fundamental. (Same optimization `transport_architecture.md`
  makes one layer down for TCP-emulating-BLE.)

### Continuity is preserved because the item is the flow
"Delete from flow A, add to flow B" destroys identity and falsifies mass-flow
history. Instead: the **item is the continuity-unit flow**; locations ("my room,"
"rim workshop," living-space description) are **collection flows** referencing it.
Moving an item = re-pointing a reference (and/or setting a `location` field on the
item-flow), never delete+create. Because logical flows are physically batched,
"split a subset out to share it" is a *re-partition* that preserves each item's
UUID — not data-structure surgery.

### Graduated disclosure via cached hints (an emergent capability)
Caching a field into a shared collection **is** a deliberate, per-field disclosure
to that collection's audience — sharing granularity *finer than per-flow*, for
free, out of blessing the collection type. E.g. share a chart with `title` +
`duration` hints cached in, grant no item-flow access → recipient sees "Foo,
~3 days," can lay it out and add edges, but cannot open the item. Model: the
item's owner opts specific fields into a collection's cache, accepting they are
visible to that collection's audience. (Requires strict cache discipline — see
Issue C.)

### Two orthogonal axes drive the split decision
Split a flow wherever **independent shareability** OR **independent lifecycle/
continuity** demands it. Where neither does, keep it bundled (multi-item flows
remain legitimate as an optimization, not a compromise). Album-level order/captions
stay in the album flow; photos are separate because you *might* share one.

---

## Known issues to design around (all judged manageable, none fatal)

**A. Access transitivity across the mesh.** Sharing a container flow must interact
with access to its referents. Sharing a collection = share the collection-flow + a
selected subset of referenced sub-flows; the rest are **opaque references** the
recipient sees but cannot open. Hard protocol requirement: the mesh must tolerate
dangling/unauthorized references gracefully. Soft product default: pre-select
everything in the collection you own, let the user deselect. **Depends on
inter-capsule auth, which is undesigned.**

**B. Convergence ≠ validity.** Acyclicity is a global constraint, not expressible
as a CRDT invariant. A shared chart can converge to `{A→B, B→A}` (a cycle) and is
*supposed to*. Resolution: graph validity (acyclicity, time-constraint feasibility,
orphans) is **post-materialization validation** (`GraphDoctor`), never write-time
enforcement. Extrinsic edges make this the normal case, not an edge case — which is
good, because it forces validation to be always-on. "All tasks with all
dependencies" is a *synthesized* chart (union of edges across charts) and may be
inconsistent; treat it as its own validatable artifact, never as ground truth.

**C. Derived state is chart-relative.** Blocked/ready/critical-path have no global
answer under extrinsic edges — the same item can be blocked in one chart, ready in
another. Raw status (done/not-done) lives on the item-flow and converges; derived
readiness is computed per-chart and must only be displayed within a chart's
context. Never show a "blocked" badge in a chart-agnostic list view.

**D. Cached hints are a cache.** Denormalized fields (duration, etc.) exist
authoritatively on the item-flow and provisionally in the chart; they will diverge.
Rules: hint is a provisional render placeholder (low-res-image pattern), replaced
once the item-flow resolves; **never authoritative; never written back** to the
item-flow.

**E. Aggregate queries over partially-shared collections are inherently partial.**
"Total mass of stuff at the workshop" can only sum item-flows you can read. Honest
and acceptable, but accept it consciously.

**F. Structured CRDT set elements.** Extrinsic edges are set elements of shape
`{from, to, type}`, not scalars — this pushes against the current largely-scalar
`Value` enum. Either string-encode edges (`"A>REQUIRES>B"`) or do the "schematized
nested Value" work `app_soradyne_boundary.txt` floated. Now on the critical path.

**G. Migration.** Current giantt stores relations on items (bidirectional, global).
Extrinsic-only requires a one-time collapse of those into chart(s) — trivially, into
each user's default/personal chart. Small, but should be written down.

---

## Open questions

1. **Collection flow type definition** (the proposed next work item): what exactly
   does it contain? Candidate contents discussed — the reference-set, the per-field
   hint cache, the access-transitivity policy, and the reserved (dormant) coherence
   slot. Not yet drafted.

2. **Inter-capsule auth** remains *the* blocking open problem (already flagged in
   `authorization-model.md`): per-stream E2E keys, invite/accept, revocation-by-key-
   rotation. Every sharing scenario here depends on it. Sequencing proposed:
   (1) commit to logical-atomization + collection-flow pattern (design-only, no
   crypto); (2) resolve extrinsic relations for giantt [done, provisionally];
   (3) inter-capsule auth + mesh access-transitivity together.

3. **Coherence groups** (frame-synchronized atomic updates across N independently-
   shareable flows, e.g. per-DOF robot joint flows): genuinely new primitive, no
   home in the current drip/jet dichotomy. Deferred until hardware forces it;
   reserved on the collection type.

4. **Auto-grant vs. prompt** policy for co-sharing referenced sub-flows when a
   collection is shared — product/UX decision, dependent on Issue A's protocol
   support.

5. **Intrinsic relations** — parked. The conversation settled on extrinsic-only for
   giantt, but noted that a genuinely intrinsic "fact about the world" dependency
   (as opposed to a per-chart planning edge) has no home in the extrinsic-only model.
   Revisit if a use case demands a canonical, cross-chart-authoritative dependency.

---

## What has NOT changed / been written

- `rim-self-data-flows.tex` is unmodified. None of the above is in the spec yet.
- No code changes. `FullReplicaFlow`, the `Value` enum, giantt's on-item relations,
  and the app schemas are all as they were.
- Inter-capsule auth is still undesigned.
